//! Customer invoices for services sold in Sweden: customers, draft invoices (optionally built from
//! tracked hours), a gap-free number series assigned at issue, a draft ledger voucher for the sale,
//! and a printable HTML invoice.
//!
//! Scope: domestic sales with 25, 12 or 6 percent VAT. EU and export invoices, credit notes and
//! payment matching are not built. The mandatory contents follow Skatteverket's list for a full
//! invoice (checked 2026-10-23): issue date, a unique sequential number, seller's VAT number, buyer
//! and seller name and address, quantity and nature of the services, the date of supply when it
//! differs, taxable amount and unit price, the VAT rate and the VAT amount.

use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use chrono::{Duration, NaiveDate, Utc};
use oas_core::{format_amount, parse_amount, utc_string};
use rusqlite::{Connection, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::ledger::{self, LineIn, NewDraft};
use crate::{Actor, ApiError, ApiResult, Filters, Role, Shared};

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS customer(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE,
  org_number TEXT NOT NULL DEFAULT '',
  vat_number TEXT NOT NULL DEFAULT '',
  address TEXT NOT NULL DEFAULT '',
  country TEXT NOT NULL DEFAULT 'SE',
  email TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS invoice_customer(
  invoice_id INTEGER PRIMARY KEY REFERENCES invoice(id),
  snapshot TEXT NOT NULL
);
CREATE TRIGGER IF NOT EXISTS invoice_customer_no_update BEFORE UPDATE ON invoice_customer
  BEGIN SELECT RAISE(ABORT, 'an issued invoice cannot be changed'); END;
CREATE TRIGGER IF NOT EXISTS invoice_customer_no_delete BEFORE DELETE ON invoice_customer
  BEGIN SELECT RAISE(ABORT, 'an issued invoice cannot be changed'); END;
CREATE TABLE IF NOT EXISTS invoice(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  series TEXT NOT NULL DEFAULT 'F',
  number INTEGER,
  customer_id INTEGER NOT NULL REFERENCES customer(id),
  status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'issued')),
  issue_date TEXT NOT NULL,
  due_date TEXT NOT NULL,
  service_date TEXT,
  currency TEXT NOT NULL DEFAULT 'SEK',
  vat_rate INTEGER NOT NULL CHECK (vat_rate IN (25, 12, 6)),
  notes TEXT NOT NULL DEFAULT '',
  created_by TEXT NOT NULL,
  created_at TEXT NOT NULL,
  issued_by TEXT,
  issued_at TEXT,
  ledger_draft_id INTEGER,
  UNIQUE (series, number)
);
CREATE TABLE IF NOT EXISTS invoice_line(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  invoice_id INTEGER NOT NULL REFERENCES invoice(id) ON DELETE CASCADE,
  description TEXT NOT NULL,
  quantity INTEGER NOT NULL CHECK (quantity > 0),
  unit TEXT NOT NULL DEFAULT 'st',
  unit_price INTEGER NOT NULL CHECK (unit_price >= 0)
);
CREATE TABLE IF NOT EXISTS invoiced_entry(
  entry_id INTEGER PRIMARY KEY REFERENCES time_entry(id),
  invoice_id INTEGER NOT NULL REFERENCES invoice(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS invoiced_period(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  project TEXT NOT NULL,
  from_date TEXT NOT NULL,
  to_date TEXT NOT NULL,
  invoice_id INTEGER NOT NULL REFERENCES invoice(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS invoiced_period_project ON invoiced_period(project, from_date);
CREATE TABLE IF NOT EXISTS invoice_credit(
  credit_id INTEGER PRIMARY KEY REFERENCES invoice(id) ON DELETE CASCADE,
  original_id INTEGER NOT NULL UNIQUE REFERENCES invoice(id)
);
CREATE TRIGGER IF NOT EXISTS invoice_issued_no_update BEFORE UPDATE ON invoice
  WHEN OLD.status = 'issued'
  BEGIN SELECT RAISE(ABORT, 'an issued invoice cannot be changed'); END;
CREATE TRIGGER IF NOT EXISTS invoice_issued_no_delete BEFORE DELETE ON invoice
  WHEN OLD.status = 'issued'
  BEGIN SELECT RAISE(ABORT, 'an issued invoice cannot be deleted'); END;
CREATE TRIGGER IF NOT EXISTS invoice_line_issued_no_update BEFORE UPDATE ON invoice_line
  WHEN (SELECT status FROM invoice WHERE id = OLD.invoice_id) = 'issued'
  BEGIN SELECT RAISE(ABORT, 'an issued invoice cannot be changed'); END;
CREATE TRIGGER IF NOT EXISTS invoice_line_issued_no_delete BEFORE DELETE ON invoice_line
  WHEN (SELECT status FROM invoice WHERE id = OLD.invoice_id) = 'issued'
  BEGIN SELECT RAISE(ABORT, 'an issued invoice cannot be changed'); END;
";

fn require_admin(actor: &Actor) -> Result<(), ApiError> {
    if actor.role == Role::Admin {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "only an admin can issue or delete invoices",
        ))
    }
}

fn clean(s: &str, field: &str, max: usize) -> Result<String, ApiError> {
    let v = s.trim();
    if v.chars().count() > max || v.chars().any(|c| c.is_control() && c != '\n') {
        return Err(ApiError::invalid(format!(
            "{field} is too long or has control characters"
        )));
    }
    Ok(v.to_string())
}

fn date(s: &str, field: &str) -> Result<NaiveDate, ApiError> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")
        .map_err(|_| ApiError::invalid(format!("{field} must be YYYY-MM-DD")))
}

/// Hundredths of a unit times a unit price in öre, rounded to whole öre.
fn line_net(quantity: i64, unit_price: i64) -> i64 {
    ((quantity as i128 * unit_price as i128 + 50) / 100) as i64
}

#[derive(Deserialize)]
pub struct NewCustomer {
    name: String,
    org_number: Option<String>,
    vat_number: Option<String>,
    address: Option<String>,
    country: Option<String>,
    email: Option<String>,
}

pub async fn add_customer(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Json(b): Json<NewCustomer>,
) -> ApiResult {
    let name = clean(&b.name, "name", 100)?;
    if name.is_empty() {
        return Err(ApiError::invalid("customer name is empty"));
    }
    let country = b.country.as_deref().unwrap_or("SE").trim().to_uppercase();
    if country.len() != 2 || !country.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(ApiError::invalid(
            "country must be a two-letter code such as SE",
        ));
    }
    let org = clean(b.org_number.as_deref().unwrap_or(""), "org_number", 30)?;
    let vat = clean(b.vat_number.as_deref().unwrap_or(""), "vat_number", 30)?;
    let address = clean(b.address.as_deref().unwrap_or(""), "address", 300)?;
    let email = clean(b.email.as_deref().unwrap_or(""), "email", 100)?;
    let conn = st.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO customer(name, org_number, vat_number, address, country, email, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![name, org, vat, address, country, email, utc_string(Utc::now())],
    )
    .map_err(|_| ApiError::new(StatusCode::CONFLICT, "conflict", "a customer with that name already exists"))?;
    let id = conn.last_insert_rowid();
    ledger::audit(&conn, &actor, "customer.add", &id.to_string(), &name)?;
    Ok(Json(json!({"customer": customer_json(&conn, id)?})))
}

fn customer_json(conn: &Connection, id: i64) -> Result<Value, ApiError> {
    conn.query_row(
        "SELECT name, org_number, vat_number, address, country, email FROM customer WHERE id = ?1",
        params![id],
        |r| {
            Ok(json!({"id": id, "name": r.get::<_, String>(0)?, "org_number": r.get::<_, String>(1)?,
                      "vat_number": r.get::<_, String>(2)?, "address": r.get::<_, String>(3)?,
                      "country": r.get::<_, String>(4)?, "email": r.get::<_, String>(5)?}))
        },
    )
    .map_err(|_| ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such customer"))
}

/// The customer as an invoice shows it: the details saved when it was issued, or the customer's
/// current details while it is a draft.
fn invoice_customer(
    conn: &Connection,
    invoice_id: i64,
    customer_id: i64,
) -> Result<Value, ApiError> {
    use rusqlite::OptionalExtension;
    let saved: Option<String> = conn
        .query_row(
            "SELECT snapshot FROM invoice_customer WHERE invoice_id = ?1",
            params![invoice_id],
            |r| r.get(0),
        )
        .optional()?;
    match saved.and_then(|s| serde_json::from_str(&s).ok()) {
        Some(v) => Ok(v),
        None => customer_json(conn, customer_id),
    }
}

fn customer_id_of(conn: &Connection, invoice_id: i64) -> Result<i64, ApiError> {
    Ok(conn.query_row(
        "SELECT customer_id FROM invoice WHERE id = ?1",
        params![invoice_id],
        |r| r.get(0),
    )?)
}

/// Invoices issued before snapshots existed get one from the customer as it is now. Nothing could
/// edit a customer then, so these are still the details that were invoiced.
pub fn backfill_customer_snapshots(conn: &Connection) -> Result<(), ApiError> {
    let ids: Vec<(i64, i64)> = conn
        .prepare(
            "SELECT i.id, i.customer_id FROM invoice i
             WHERE i.status = 'issued' AND NOT EXISTS (SELECT 1 FROM invoice_customer c WHERE c.invoice_id = i.id)",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(Result::ok)
        .collect();
    for (invoice_id, customer_id) in ids {
        conn.execute(
            "INSERT INTO invoice_customer(invoice_id, snapshot) VALUES (?1, ?2)",
            params![invoice_id, customer_json(conn, customer_id)?.to_string()],
        )?;
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct CustomerPatch {
    name: Option<String>,
    org_number: Option<String>,
    vat_number: Option<String>,
    address: Option<String>,
    email: Option<String>,
}

/// Change a customer's details. Draft invoices follow; issued invoices keep the details they were
/// issued with. The old and new values go to the audit log.
pub async fn edit_customer(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Json(b): Json<CustomerPatch>,
) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    let before = customer_json(&conn, id)?;
    let field = |v: &Option<String>, key: &str, max: usize| -> Result<String, ApiError> {
        match v {
            Some(s) => clean(s, key, max),
            None => Ok(before[key].as_str().unwrap_or("").to_string()),
        }
    };
    let name = field(&b.name, "name", 100)?;
    if name.is_empty() {
        return Err(ApiError::invalid("customer name is empty"));
    }
    let org = field(&b.org_number, "org_number", 30)?;
    let vat = field(&b.vat_number, "vat_number", 30)?;
    let address = field(&b.address, "address", 300)?;
    let email = field(&b.email, "email", 100)?;
    conn.execute(
        "UPDATE customer SET name = ?1, org_number = ?2, vat_number = ?3, address = ?4, email = ?5 WHERE id = ?6",
        params![name, org, vat, address, email, id],
    )
    .map_err(|_| ApiError::new(StatusCode::CONFLICT, "conflict", "a customer with that name already exists"))?;
    let after = customer_json(&conn, id)?;
    if after != before {
        ledger::audit(
            &conn,
            &actor,
            "customer.edit",
            &id.to_string(),
            &json!({"old": before, "new": after}).to_string(),
        )?;
    }
    Ok(Json(json!({"customer": after})))
}

/// Delete a customer that no invoice uses. A customer on an invoice stays, so the invoice can
/// always be read in full.
pub async fn delete_customer(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    let before = customer_json(&conn, id)?;
    let invoices: i64 = conn.query_row(
        "SELECT COUNT(*) FROM invoice WHERE customer_id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    if invoices > 0 {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "in_use",
            format!(
                "this customer is on {invoices} invoice(s) and cannot be deleted; delete its draft invoices first"
            ),
        ));
    }
    conn.execute("DELETE FROM customer WHERE id = ?1", params![id])?;
    ledger::audit(
        &conn,
        &actor,
        "customer.delete",
        &id.to_string(),
        &before.to_string(),
    )?;
    Ok(Json(json!({"deleted": id})))
}

pub async fn list_customers(State(st): State<Shared>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM customer ORDER BY name")?
        .query_map([], |r| r.get(0))?
        .filter_map(Result::ok)
        .collect();
    let list: Result<Vec<Value>, ApiError> =
        ids.into_iter().map(|id| customer_json(&conn, id)).collect();
    Ok(Json(json!({"customers": list?})))
}

#[derive(Deserialize)]
pub struct LineBody {
    description: String,
    /// Quantity as a decimal, for example 12.5 hours.
    quantity: String,
    unit: Option<String>,
    /// Unit price excluding VAT, in SEK.
    unit_price: String,
}

#[derive(Deserialize)]
pub struct NewInvoice {
    customer_id: i64,
    vat_rate: Option<i64>,
    issue_date: Option<String>,
    due_days: Option<i64>,
    service_date: Option<String>,
    notes: Option<String>,
    lines: Vec<LineBody>,
}

fn default_dates(
    issue: &Option<String>,
    due_days: Option<i64>,
) -> Result<(String, String), ApiError> {
    let issue_d = match issue.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(s) => date(s, "issue_date")?,
        None => Utc::now().date_naive(),
    };
    let days = due_days.unwrap_or(30);
    if !(0..=365).contains(&days) {
        return Err(ApiError::invalid("due_days must be between 0 and 365"));
    }
    Ok((
        issue_d.to_string(),
        (issue_d + Duration::days(days)).to_string(),
    ))
}

fn check_customer(conn: &Connection, id: i64) -> Result<(), ApiError> {
    let country: Option<String> = conn
        .query_row(
            "SELECT country FROM customer WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .ok();
    match country.as_deref() {
        None => Err(ApiError::invalid(format!("unknown customer {id}"))),
        Some("SE") => Ok(()),
        Some(_) => Err(ApiError::invalid(
            "only customers in Sweden are supported; EU and export invoices are not built yet",
        )),
    }
}

fn vat_rate_ok(r: Option<i64>) -> Result<i64, ApiError> {
    match r.unwrap_or(25) {
        v @ (25 | 12 | 6) => Ok(v),
        _ => Err(ApiError::invalid("vat_rate must be 25, 12 or 6")),
    }
}

/// Validate one invoice line: description, quantity above zero, unit price, unit defaulting to "st".
fn parse_line(l: &LineBody) -> Result<(String, i64, String, i64), ApiError> {
    let description = clean(&l.description, "description", 200)?;
    if description.is_empty() {
        return Err(ApiError::invalid("every line needs a description"));
    }
    let quantity = parse_amount(&l.quantity).map_err(ApiError::invalid)?;
    if quantity == 0 {
        return Err(ApiError::invalid("quantity must be above zero"));
    }
    let unit_price = parse_amount(&l.unit_price).map_err(ApiError::invalid)?;
    let unit = clean(l.unit.as_deref().unwrap_or("st"), "unit", 12)?;
    Ok((
        description,
        quantity,
        if unit.is_empty() { "st".into() } else { unit },
        unit_price,
    ))
}

fn insert_invoice(
    conn: &Connection,
    actor: &Actor,
    b: &NewInvoice,
    entry_ids: &[i64],
) -> Result<i64, ApiError> {
    check_customer(conn, b.customer_id)?;
    let vat = vat_rate_ok(b.vat_rate)?;
    let (issue, due) = default_dates(&b.issue_date, b.due_days)?;
    let service = match b.service_date.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(s) => Some(date(s, "service_date")?.to_string()),
        None => None,
    };
    let notes = clean(b.notes.as_deref().unwrap_or(""), "notes", 500)?;
    if b.lines.is_empty() || b.lines.len() > 100 {
        return Err(ApiError::invalid(
            "an invoice needs between 1 and 100 lines",
        ));
    }
    let mut parsed = vec![];
    for l in &b.lines {
        parsed.push(parse_line(l)?);
    }
    conn.execute(
        "INSERT INTO invoice(customer_id, issue_date, due_date, service_date, vat_rate, notes, created_by, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![b.customer_id, issue, due, service, vat, notes, actor.name, utc_string(Utc::now())],
    )?;
    let id = conn.last_insert_rowid();
    for (d, q, u, p) in &parsed {
        conn.execute(
            "INSERT INTO invoice_line(invoice_id, description, quantity, unit, unit_price) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, d, q, u, p],
        )?;
    }
    for e in entry_ids {
        conn.execute(
            "INSERT INTO invoiced_entry(entry_id, invoice_id) VALUES (?1, ?2)",
            params![e, id],
        )?;
    }
    ledger::audit(conn, actor, "invoice.create", &id.to_string(), "")?;
    Ok(id)
}

pub async fn create_invoice(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Json(b): Json<NewInvoice>,
) -> ApiResult {
    let mut conn = st.conn.lock().unwrap();
    let tx = conn.transaction()?;
    let id = insert_invoice(&tx, &actor, &b, &[])?;
    tx.commit()?;
    Ok(Json(json!({"invoice": invoice_json(&conn, id)?})))
}

/// The invoice must exist and still be a draft; issued invoices are locked.
fn draft_or_err(conn: &Connection, id: i64) -> Result<(), ApiError> {
    let status: Option<String> = conn
        .query_row(
            "SELECT status FROM invoice WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .ok();
    match status.as_deref() {
        None => Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "no such invoice",
        )),
        Some("draft") => Ok(()),
        Some(_) => Err(ApiError::new(
            StatusCode::CONFLICT,
            "conflict",
            "this invoice is issued and cannot be changed",
        )),
    }
}

/// Add a line, such as a fixed cost, to a draft invoice. The line can sit next to billed time.
pub async fn add_line(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<i64>,
    Json(b): Json<LineBody>,
) -> ApiResult {
    let mut conn = st.conn.lock().unwrap();
    let tx = conn.transaction()?;
    draft_or_err(&tx, id)?;
    let (d, q, u, p) = parse_line(&b)?;
    let count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM invoice_line WHERE invoice_id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    if count >= 100 {
        return Err(ApiError::invalid("an invoice has at most 100 lines"));
    }
    tx.execute(
        "INSERT INTO invoice_line(invoice_id, description, quantity, unit, unit_price) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, d, q, u, p],
    )?;
    ledger::audit(&tx, &actor, "invoice.line_add", &id.to_string(), &d)?;
    tx.commit()?;
    Ok(Json(json!({"invoice": invoice_json(&conn, id)?})))
}

/// Remove one line from a draft invoice. The last line stays: delete the draft instead.
pub async fn remove_line(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Path((id, line_id)): Path<(i64, i64)>,
) -> ApiResult {
    let mut conn = st.conn.lock().unwrap();
    let tx = conn.transaction()?;
    draft_or_err(&tx, id)?;
    let count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM invoice_line WHERE invoice_id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    if count <= 1 {
        return Err(ApiError::invalid(
            "an invoice needs at least one line; delete the draft instead",
        ));
    }
    let n = tx.execute(
        "DELETE FROM invoice_line WHERE id = ?1 AND invoice_id = ?2",
        params![line_id, id],
    )?;
    if n == 0 {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "no such line on this invoice",
        ));
    }
    ledger::audit(
        &tx,
        &actor,
        "invoice.line_remove",
        &id.to_string(),
        &line_id.to_string(),
    )?;
    tx.commit()?;
    Ok(Json(json!({"invoice": invoice_json(&conn, id)?})))
}

#[derive(Deserialize)]
pub struct FromTime {
    customer_id: i64,
    project: String,
    from: Option<String>,
    to: Option<String>,
    vat_rate: Option<i64>,
    due_days: Option<i64>,
    issue_date: Option<String>,
    notes: Option<String>,
    /// Replaces the default line text.
    description: Option<String>,
}

/// Build a draft invoice from a project's tracked hours that have not been invoiced yet. An hourly
/// project becomes one line of hours at its rate (and needs uninvoiced time). A fixed-fee project
/// becomes one line at the fee whether or not time was logged; `from` and `to` are then the billed
/// period, which is recorded so the same fee cannot be billed twice for overlapping periods.
/// Entries that are uninvoiced in the range are marked invoiced so they cannot be billed twice.
pub async fn from_time(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Json(b): Json<FromTime>,
) -> ApiResult {
    let mut conn = st.conn.lock().unwrap();
    let entries = crate::filtered(
        &conn,
        &Filters {
            from: b.from.clone(),
            to: b.to.clone(),
            project: Some(b.project.clone()),
            activity: None,
        },
    )?;
    let projects = crate::load_projects(&conn)?;
    let project_name = match oas_core::match_project(&projects, &b.project) {
        oas_core::Match::Exact(i) => projects[i].name.clone(),
        _ => return Err(ApiError::invalid("unknown project")),
    };
    let billed: std::collections::HashSet<i64> = conn
        .prepare("SELECT entry_id FROM invoiced_entry")?
        .query_map([], |r| r.get(0))?
        .filter_map(Result::ok)
        .collect();
    let open: Vec<_> = entries
        .into_iter()
        .filter(|e| !e.running && !billed.contains(&e.id))
        .collect();
    let (rate_type, rate): (String, Option<i64>) = conn
        .query_row(
            "SELECT rate_type, rate FROM project WHERE name = ?1",
            params![project_name],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| ApiError::invalid("unknown project"))?;
    let fixed = rate_type == "fixed";
    if open.is_empty() && !fixed {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "conflict",
            "no uninvoiced time for that project in that range",
        ));
    }
    let mut first = open.iter().map(|e| e.date.as_str()).min().map(String::from);
    let mut last = open.iter().map(|e| e.date.as_str()).max().map(String::from);
    let mut period: Option<(String, String)> = None;
    if fixed {
        fn given(v: &Option<String>) -> Option<&str> {
            v.as_deref().map(str::trim).filter(|s| !s.is_empty())
        }
        let (pf, pt) = match (given(&b.from), given(&b.to), &first, &last) {
            (Some(f), Some(t), _, _) => (date(f, "from")?.to_string(), date(t, "to")?.to_string()),
            (_, _, Some(f), Some(t)) if !open.is_empty() => (f.clone(), t.clone()),
            _ => {
                return Err(ApiError::invalid(
                    "a fixed fee without logged time needs both from and to, the period the fee covers",
                ));
            }
        };
        if pf > pt {
            return Err(ApiError::invalid("from must not be after to"));
        }
        // The period is what the fee covers, even when the logged time is narrower.
        first = Some(pf.clone());
        last = Some(pt.clone());
        period = Some((pf, pt));
    }
    let first = first.unwrap_or_default();
    let last = last.unwrap_or_default();
    let hours: f64 = open.iter().map(|e| e.hours).sum();
    // Hours are billed rounded up to whole hours.
    let whole_hours = (hours - 1e-6).ceil().max(0.0) as i64;
    let custom = b
        .description
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(String::from);
    let line = match (rate_type.as_str(), rate) {
        ("hourly", Some(r)) => LineBody {
            description: custom
                .clone()
                .unwrap_or_else(|| format!("Datakonsult - {project_name}, {first} till {last}")),
            quantity: whole_hours.to_string(),
            unit: Some("h".into()),
            unit_price: format_amount(r),
        },
        ("fixed", Some(r)) => LineBody {
            description: custom
                .unwrap_or_else(|| format!("Datakonsult - {project_name}, {first} till {last}")),
            quantity: "1".into(),
            unit: Some("st".into()),
            unit_price: format_amount(r),
        },
        _ => {
            return Err(ApiError::invalid(format!(
                "project {project_name} has no hourly rate or fixed fee; set one first"
            )));
        }
    };
    if let Some((pf, pt)) = &period {
        let clash: Option<(String, String, i64)> = conn
            .query_row(
                "SELECT from_date, to_date, invoice_id FROM invoiced_period
                 WHERE project = ?1 AND from_date <= ?3 AND to_date >= ?2 LIMIT 1",
                params![project_name, pf, pt],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .ok();
        if let Some((cf, ct, inv)) = clash {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "conflict",
                format!(
                    "the fixed fee for {project_name} is already on invoice {inv} for {cf} to {ct}, which overlaps {pf} to {pt}; delete that draft or choose a later period"
                ),
            ));
        }
    }
    let ids: Vec<i64> = open.iter().map(|e| e.id).collect();
    let new = NewInvoice {
        customer_id: b.customer_id,
        vat_rate: b.vat_rate,
        issue_date: b.issue_date.clone(),
        due_days: b.due_days,
        service_date: if first == last {
            Some(first.clone())
        } else {
            None
        },
        notes: b.notes.clone(),
        lines: vec![line],
    };
    let tx = conn.transaction()?;
    let id = insert_invoice(&tx, &actor, &new, &ids)?;
    if let Some((pf, pt)) = &period {
        tx.execute(
            "INSERT INTO invoiced_period(project, from_date, to_date, invoice_id) VALUES (?1, ?2, ?3, ?4)",
            params![project_name, pf, pt, id],
        )?;
    }
    tx.commit()?;
    Ok(Json(json!({"invoice": invoice_json(&conn, id)?})))
}

struct Totals {
    net: i64,
    vat: i64,
    gross: i64,
}

fn totals(lines: &[(String, i64, String, i64)], vat_rate: i64) -> Totals {
    let net: i64 = lines.iter().map(|l| line_net(l.1, l.3)).sum();
    let vat = (net as i128 * vat_rate as i128 + 50) / 100;
    // Whole öre; Swedish invoices commonly round the total to kronor, which is not done here.
    Totals {
        net,
        vat: vat as i64,
        gross: net + vat as i64,
    }
}

#[expect(
    clippy::type_complexity,
    reason = "one tuple per SQL row keeps the column order next to the query"
)]
fn invoice_json(conn: &Connection, id: i64) -> Result<Value, ApiError> {
    let (series, number, customer_id, status, issue, due, service, currency, vat_rate, notes, created_by, issued_by, issued_at, draft): (
        String, Option<i64>, i64, String, String, String, Option<String>, String, i64, String, String, Option<String>, Option<String>, Option<i64>,
    ) = conn
        .query_row(
            "SELECT series, number, customer_id, status, issue_date, due_date, service_date, currency, vat_rate, notes, created_by, issued_by, issued_at, ledger_draft_id FROM invoice WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?, r.get(11)?, r.get(12)?, r.get(13)?)),
        )
        .map_err(|_| ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such invoice"))?;
    let line_ids: Vec<i64> = conn
        .prepare("SELECT id FROM invoice_line WHERE invoice_id = ?1 ORDER BY id")?
        .query_map(params![id], |r| r.get(0))?
        .filter_map(Result::ok)
        .collect();
    let lines: Vec<(String, i64, String, i64)> = conn
        .prepare("SELECT description, quantity, unit, unit_price FROM invoice_line WHERE invoice_id = ?1 ORDER BY id")?
        .query_map(params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .filter_map(Result::ok)
        .collect();
    let t = totals(&lines, vat_rate);
    // A credit note stores positive lines and shows them negated, so it reverses its original.
    let credit_of: Option<i64> = conn
        .query_row(
            "SELECT original_id FROM invoice_credit WHERE credit_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .ok();
    let credited_by: Option<i64> = conn
        .query_row(
            "SELECT credit_id FROM invoice_credit WHERE original_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .ok();
    let label_of = |other: Option<i64>| -> Option<String> {
        other.and_then(|o| {
            conn.query_row(
                "SELECT series || number FROM invoice WHERE id = ?1 AND number IS NOT NULL",
                params![o],
                |r| r.get(0),
            )
            .ok()
        })
    };
    let credit_of_label = label_of(credit_of);
    let sg: i64 = if credit_of.is_some() { -1 } else { 1 };
    let t = Totals {
        net: sg * t.net,
        vat: sg * t.vat,
        gross: sg * t.gross,
    };
    let entries: i64 = conn.query_row(
        "SELECT COUNT(*) FROM invoiced_entry WHERE invoice_id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    Ok(json!({
        "id": id,
        "label": number.map(|n| format!("{series}{n}")),
        "number": number, "status": status, "customer": invoice_customer(conn, id, customer_id)?,
        "issue_date": issue, "due_date": due, "service_date": service, "currency": currency,
        "vat_rate": vat_rate, "notes": notes, "created_by": created_by, "issued_by": issued_by, "issued_at": issued_at,
        "ledger_draft_id": draft, "time_entries": entries,
        "credit_of": credit_of, "credit_of_label": credit_of_label, "credited_by": credited_by,
        "lines": lines.iter().zip(&line_ids).map(|((d, q, u, p), lid)| json!({
            "id": lid, "description": d, "quantity": format_amount(sg * *q), "unit": u, "unit_price": format_amount(*p),
            "net": format_amount(sg * line_net(*q, *p)),
        })).collect::<Vec<_>>(),
        "totals": {"net": format_amount(t.net), "vat": format_amount(t.vat), "gross": format_amount(t.gross)},
    }))
}

pub async fn list_invoices(State(st): State<Shared>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM invoice ORDER BY id DESC")?
        .query_map([], |r| r.get(0))?
        .filter_map(Result::ok)
        .collect();
    let list: Result<Vec<Value>, ApiError> =
        ids.into_iter().map(|id| invoice_json(&conn, id)).collect();
    Ok(Json(json!({"invoices": list?})))
}

pub async fn get_invoice(State(st): State<Shared>, Path(id): Path<i64>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    Ok(Json(json!({"invoice": invoice_json(&conn, id)?})))
}

pub async fn delete_invoice(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<i64>,
) -> ApiResult {
    require_admin(&actor)?;
    let conn = st.conn.lock().unwrap();
    let status: Option<String> = conn
        .query_row(
            "SELECT status FROM invoice WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .ok();
    match status.as_deref() {
        None => {
            return Err(ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                "no such invoice",
            ));
        }
        Some("draft") => {}
        Some(_) => {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "conflict",
                "an issued invoice cannot be deleted",
            ));
        }
    }
    conn.execute("DELETE FROM invoice WHERE id = ?1", params![id])?;
    ledger::audit(&conn, &actor, "invoice.delete", &id.to_string(), "")?;
    Ok(Json(json!({"status": "ok"})))
}

struct Seller {
    name: String,
    org: String,
    address: String,
    vat_number: String,
    payment: String,
    f_skatt: bool,
}

fn seller(conn: &Connection) -> Result<Seller, ApiError> {
    let row: (String, String, String, String, String, i64) = conn
        .query_row(
            "SELECT name, org_number, address, vat_number, payment_info, f_skatt FROM company WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .map_err(|_| ApiError::invalid("set the company details first (oas company set ...)"))?;
    let digits: String = row.1.chars().filter(|c| c.is_ascii_digit()).collect();
    let vat_number = if row.3.trim().is_empty() && digits.len() == 10 {
        format!("SE{digits}01")
    } else {
        row.3.clone()
    };
    Ok(Seller {
        name: row.0,
        org: row.1,
        address: row.2,
        vat_number,
        payment: row.4,
        f_skatt: row.5 != 0,
    })
}

/// What a full invoice still lacks, per Skatteverket's list. Empty means it can be issued.
fn missing(conn: &Connection, id: i64) -> Result<Vec<String>, ApiError> {
    let s = seller(conn)?;
    let mut out = vec![];
    if s.name.trim().is_empty() {
        out.push("the company name".into());
    }
    if s.address.trim().is_empty() {
        out.push("the company address (oas company set --address ...)".into());
    }
    if s.vat_number.trim().is_empty() {
        out.push(
            "the company VAT number (derived from the organisation number when that has 10 digits)"
                .into(),
        );
    }
    let (cust, addr): (String, String) = conn.query_row(
        "SELECT c.name, c.address FROM invoice i JOIN customer c ON c.id = i.customer_id WHERE i.id = ?1",
        params![id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if addr.trim().is_empty() {
        out.push(format!("an address for the customer {cust}"));
    }
    Ok(out)
}

/// Issue an invoice: assign the next number in the series, lock it, and propose a ledger voucher
/// (debit accounts receivable, credit sales and output VAT) for an admin to approve.
pub async fn issue_invoice(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<i64>,
) -> ApiResult {
    require_admin(&actor)?;
    let mut conn = st.conn.lock().unwrap();
    let (status, vat_rate, issue_date, customer_id, series): (String, i64, String, i64, String) =
        conn.query_row(
            "SELECT status, vat_rate, issue_date, customer_id, series FROM invoice WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .map_err(|_| ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such invoice"))?;
    if status != "draft" {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "conflict",
            "this invoice is already issued",
        ));
    }
    let gaps = missing(&conn, id)?;
    if !gaps.is_empty() {
        return Err(ApiError::invalid(format!(
            "the invoice is missing: {}",
            gaps.join("; ")
        )));
    }
    let lines: Vec<(String, i64, String, i64)> = conn
        .prepare("SELECT description, quantity, unit, unit_price FROM invoice_line WHERE invoice_id = ?1")?
        .query_map(params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .filter_map(Result::ok)
        .collect();
    let t = totals(&lines, vat_rate);
    let customer: String = conn.query_row(
        "SELECT name FROM customer WHERE id = ?1",
        params![customer_id],
        |r| r.get(0),
    )?;
    let (income, vat_acct, code) = match vat_rate {
        25 => (3041, 2611, "S25"),
        12 => (3042, 2621, "S12"),
        _ => (3043, 2631, "S6"),
    };
    let number: i64 = conn.query_row(
        "SELECT COALESCE(MAX(number), 0) + 1 FROM invoice WHERE series = ?1",
        params![series],
        |r| r.get(0),
    )?;
    let credit_of: Option<i64> = conn
        .query_row(
            "SELECT original_id FROM invoice_credit WHERE credit_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .ok();
    let is_credit = credit_of.is_some();
    // A credit note posts the reverse of an invoice: receivable credited, sales and VAT debited.
    let side = |ore: i64, debit: bool| -> (Option<String>, Option<String>) {
        if debit {
            (Some(format_amount(ore)), None)
        } else {
            (None, Some(format_amount(ore)))
        }
    };
    let (gd, gc) = side(t.gross, !is_credit);
    let (nd_, nc) = side(t.net, is_credit);
    let (vd, vc) = side(t.vat, is_credit);
    let nd = NewDraft {
        event_date: issue_date,
        description: format!(
            "{} {series}{number} {customer}",
            if is_credit {
                "Kreditfaktura"
            } else {
                "Faktura"
            }
        ),
        counterparty: Some(customer),
        lines: vec![
            LineIn {
                account: 1510,
                debit: gd,
                credit: gc,
                vat_code: None,
            },
            LineIn {
                account: income,
                debit: nd_,
                credit: nc,
                vat_code: Some(code.into()),
            },
            LineIn {
                account: vat_acct,
                debit: vd,
                credit: vc,
                vat_code: None,
            },
        ],
        documents: vec![],
        bank_transaction_id: None,
    };
    let draft_id = ledger::create_draft(&mut conn, &actor, &nd)?;
    conn.execute(
        "INSERT OR IGNORE INTO invoice_customer(invoice_id, snapshot) VALUES (?1, ?2)",
        params![
            id,
            customer_json(&conn, customer_id_of(&conn, id)?)?.to_string()
        ],
    )?;
    conn.execute(
        "UPDATE invoice SET status = 'issued', number = ?1, issued_by = ?2, issued_at = ?3, ledger_draft_id = ?4 WHERE id = ?5",
        params![number, actor.name, utc_string(Utc::now()), draft_id, id],
    )?;
    if let Some(orig) = credit_of {
        // The credited invoice no longer bills its time or period, so both can be billed again.
        conn.execute(
            "DELETE FROM invoiced_entry WHERE invoice_id = ?1",
            params![orig],
        )?;
        conn.execute(
            "DELETE FROM invoiced_period WHERE invoice_id = ?1",
            params![orig],
        )?;
    }
    ledger::audit(
        &conn,
        &actor,
        "invoice.issue",
        &id.to_string(),
        &format!("{series}{number}"),
    )?;
    Ok(Json(
        json!({"invoice": invoice_json(&conn, id)?, "ledger_draft": ledger::draft_view(&conn, draft_id)?}),
    ))
}

/// Cancel an issued invoice by crediting it: a draft credit note (series K) with the same lines,
/// shown negated. Issuing it posts the reverse voucher and releases the time and fee period of the
/// credited invoice, so they can be invoiced again. One credit note per invoice.
pub async fn credit_invoice(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<i64>,
) -> ApiResult {
    require_admin(&actor)?;
    let mut conn = st.conn.lock().unwrap();
    let (status, series, number, customer_id, vat_rate): (String, String, Option<i64>, i64, i64) =
        conn.query_row(
            "SELECT status, series, number, customer_id, vat_rate FROM invoice WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .map_err(|_| ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such invoice"))?;
    let conflict = |m: &str| ApiError::new(StatusCode::CONFLICT, "conflict", m.to_string());
    if status != "issued" {
        return Err(conflict("only an issued invoice can be credited"));
    }
    let is_credit: i64 = conn.query_row(
        "SELECT COUNT(*) FROM invoice_credit WHERE credit_id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    if is_credit > 0 {
        return Err(conflict("a credit note cannot be credited"));
    }
    let existing: i64 = conn.query_row(
        "SELECT COUNT(*) FROM invoice_credit WHERE original_id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    if existing > 0 {
        return Err(conflict("this invoice already has a credit note"));
    }
    let label = format!("{series}{}", number.unwrap_or_default());
    let today = Utc::now().date_naive().to_string();
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO invoice(series, customer_id, issue_date, due_date, vat_rate, notes, created_by, created_at)
         VALUES ('K', ?1, ?2, ?2, ?3, ?4, ?5, ?6)",
        params![
            customer_id,
            today,
            vat_rate,
            format!("Kreditfaktura avseende faktura {label}"),
            actor.name,
            utc_string(Utc::now())
        ],
    )?;
    let cid = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO invoice_line(invoice_id, description, quantity, unit, unit_price)
         SELECT ?1, description, quantity, unit, unit_price FROM invoice_line WHERE invoice_id = ?2 ORDER BY id",
        params![cid, id],
    )?;
    tx.execute(
        "INSERT INTO invoice_credit(credit_id, original_id) VALUES (?1, ?2)",
        params![cid, id],
    )?;
    ledger::audit(&tx, &actor, "invoice.credit", &id.to_string(), &label)?;
    tx.commit()?;
    Ok(Json(json!({"invoice": invoice_json(&conn, cid)?})))
}

/// "72000.00" as "72 000,00": spaces between thousands and a decimal comma, as on Swedish invoices.
fn sv(s: &str) -> String {
    let (neg, rest) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s),
    };
    let (int, frac) = rest.split_once('.').unwrap_or((rest, "00"));
    let mut out = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push('\u{a0}');
        }
        out.push(c);
    }
    format!("{}{out},{frac}", if neg { "-" } else { "" })
}

/// A quantity without decimals when it is whole.
fn sv_qty(s: &str) -> String {
    match s.strip_suffix(".00") {
        Some(whole) => sv(&format!("{whole}.00"))
            .trim_end_matches(",00")
            .to_string(),
        None => sv(s),
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// The seller's payment text, split at line breaks and at the " · " that older texts use between
/// items. A leading IBAN, BIC, Bankgiro, Plusgiro or Account becomes a label and value row, like
/// "Bankgiro: 722-4769" on a Swedish invoice; other items are plain lines. Empty text gives nothing.
fn payment_block(s: &str, lang: &str) -> String {
    let mut out = String::new();
    for item in s
        .lines()
        .flat_map(|l| l.split(" \u{b7} "))
        .map(str::trim)
        .filter(|x| !x.is_empty())
    {
        let (first, rest) = item.split_once(char::is_whitespace).unwrap_or((item, ""));
        let label = match first.to_lowercase().as_str() {
            "iban" => Some("Iban"),
            "bic" | "bic/swift" | "swift" => Some("Bic"),
            "bankgiro" => Some("Bankgiro"),
            "plusgiro" => Some("Plusgiro"),
            "account" | "konto" => Some(if lang == "en" { "Account" } else { "Konto" }),
            _ => None,
        };
        match label {
            Some(l) if !rest.trim().is_empty() => out.push_str(&format!(
                "<div class=kv><span>{l}:</span><span>{}</span></div>",
                esc(rest.trim())
            )),
            _ => out.push_str(&format!("<div class=pl>{}</div>", esc(item))),
        }
    }
    out
}

fn multiline(s: &str) -> String {
    esc(s).replace('\n', "<br>")
}

/// Texts of the printed invoice in one language.
struct Words {
    html_lang: &'static str,
    invoice: &'static str,
    credit_note: &'static str,
    draft: &'static str,
    customer_address: &'static str,
    number: &'static str,
    date: &'static str,
    terms: &'static str,
    due: &'static str,
    your_ref: &'static str,
    our_ref: &'static str,
    f_tax: &'static str,
    description: &'static str,
    quantity: &'static str,
    unit: &'static str,
    unit_price: &'static str,
    subtotal: &'static str,
    vat: &'static str,
    rounding: &'static str,
    total: &'static str,
    interest: &'static str,
    objections: &'static str,
    org_short: &'static str,
    vat_short: &'static str,
    payment: &'static str,
    amount: &'static str,
}

fn words(lang: &str) -> Words {
    if lang == "en" {
        Words {
            html_lang: "en",
            invoice: "INVOICE",
            credit_note: "CREDIT NOTE",
            draft: "DRAFT",
            customer_address: "Customer address",
            number: "Invoice no.:",
            date: "Invoice date:",
            terms: "Payment terms:",
            due: "Due date:",
            your_ref: "Your reference:",
            our_ref: "Our reference:",
            f_tax: "Approved for F-tax",
            description: "Description",
            quantity: "Quantity",
            unit: "Unit",
            unit_price: "Unit price",
            subtotal: "Subtotal:",
            vat: "VAT",
            rounding: "Rounding:",
            total: "Total amount:",
            interest: "After the due date, interest on late payment is charged under the Swedish Interest Act",
            objections: "Any objections to this invoice must be made within 10 days",
            org_short: "Org. no.",
            vat_short: "VAT no.",
            payment: "Payment details",
            amount: "Amount",
        }
    } else {
        Words {
            html_lang: "sv",
            invoice: "FAKTURA",
            credit_note: "KREDITFAKTURA",
            draft: "UTKAST",
            customer_address: "Kundadress",
            number: "Faktura nr.:",
            date: "Fakturadatum:",
            terms: "Betalningsvillkor:",
            due: "Förfallodag:",
            your_ref: "Er referens:",
            our_ref: "Vår referens:",
            f_tax: "Godkänd för F-skatt",
            description: "Beskrivning",
            quantity: "Mängd",
            unit: "Enhet",
            unit_price: "A pris",
            subtotal: "Summa:",
            vat: "Mervärdeskatt",
            rounding: "Öresutjämning:",
            total: "Total SUMMA:",
            interest: "Efter förfallodag debiteras dröjsmålsränta enligt räntelagen",
            objections: "Eventuella invändningar mot denna faktura skall ske inom 10 dagar",
            org_short: "Org.nr",
            vat_short: "Momsreg.nr",
            payment: "Betalningsuppgifter",
            amount: "Belopp",
        }
    }
}

#[derive(Deserialize, Default)]
pub struct HtmlQuery {
    /// "sv" (the default) or "en".
    lang: Option<String>,
}

/// A printable invoice page on A4, laid out like a Swedish consultant invoice. Print it to PDF from
/// the browser. `?lang=en` prints it in English; the default is Swedish. Drafts carry a watermark.
pub async fn invoice_html(
    State(st): State<Shared>,
    Path(id): Path<i64>,
    axum::extract::Query(q): axum::extract::Query<HtmlQuery>,
) -> Result<Response, ApiError> {
    let lang = q.lang.as_deref().unwrap_or("sv");
    if !matches!(lang, "sv" | "en") {
        return Err(ApiError::invalid("lang must be sv or en"));
    }
    let w = words(lang);
    let conn = st.conn.lock().unwrap();
    let inv = invoice_json(&conn, id)?;
    let s = seller(&conn)?;
    let c = &inv["customer"];
    let draft = inv["status"] == "draft";
    let is_credit = !inv["credit_of"].is_null();
    let label = inv["label"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| "-".into());
    let currency = inv["currency"].as_str().unwrap_or("SEK");
    let mut rows = String::new();
    for l in inv["lines"].as_array().cloned().unwrap_or_default() {
        let unit = l["unit"].as_str().unwrap_or("");
        let unit = if lang == "en" && unit == "st" {
            "pcs"
        } else {
            unit
        };
        rows.push_str(&format!(
            "<tr><td>{}</td><td class='n q'>{}</td><td class=u>{}</td><td class='n p'>{}</td><td class='n a'>{}</td></tr>",
            esc(l["description"].as_str().unwrap_or("")),
            esc(&sv_qty(l["quantity"].as_str().unwrap_or(""))),
            esc(unit),
            esc(&sv(l["unit_price"].as_str().unwrap_or(""))),
            esc(&sv(l["net"].as_str().unwrap_or(""))),
        ));
    }
    let short = |d: &str| d.get(2..).unwrap_or(d).to_string();
    let issue_s = inv["issue_date"].as_str().unwrap_or("");
    let due_s = inv["due_date"].as_str().unwrap_or("");
    let terms = match (
        chrono::NaiveDate::parse_from_str(issue_s, "%Y-%m-%d"),
        chrono::NaiveDate::parse_from_str(due_s, "%Y-%m-%d"),
    ) {
        (Ok(i), Ok(d)) if !is_credit => {
            let n = (d - i).num_days();
            if lang == "en" {
                format!("{n} days net")
            } else {
                format!("{n} dagar netto")
            }
        }
        _ => String::new(),
    };
    // Reference rows only show when there is something to show.
    let meta_row = |label: &str, value: &str| -> String {
        if value.is_empty() || value == "-" {
            String::new()
        } else {
            format!("<div><span>{label}</span><b>{}</b></div>", esc(value))
        }
    };
    let meta = [
        meta_row(w.number, &label),
        meta_row(w.date, &short(issue_s)),
        meta_row(w.due, &short(due_s)),
        meta_row(w.terms, &terms),
        meta_row(w.your_ref, inv["your_reference"].as_str().unwrap_or("")),
        meta_row(w.our_ref, inv["our_reference"].as_str().unwrap_or("")),
    ]
    .concat();
    let ids = [
        format!("{} {}", w.org_short, s.org),
        format!("{} {}", w.vat_short, s.vat_number),
    ]
    .into_iter()
    .chain(s.f_skatt.then(|| w.f_tax.to_string()))
    .map(|x| esc(&x))
    .collect::<Vec<_>>()
    .join(" &nbsp;·&nbsp; ");
    let payment = payment_block(&s.payment, lang);
    let pay_box = if payment.is_empty() {
        String::new()
    } else {
        format!("<div class=pay><h3>{}</h3>{payment}</div>", w.payment)
    };
    let notes = inv["notes"].as_str().unwrap_or("");
    let notes_html = if notes.trim().is_empty() {
        String::new()
    } else {
        format!("<p class=notes>{}</p>", multiline(notes))
    };
    let html = format!(
        "<!doctype html><html lang={hl}><head><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><title>{title} {label}</title><style>
@page{{size:A4;margin:0}}
*{{box-sizing:border-box}}
body{{font:13px/1.5 'Helvetica Neue',Helvetica,Arial,sans-serif;margin:0;color:#1b1f24;background:#e9ebee}}
.page{{width:210mm;min-height:297mm;margin:0 auto;background:#fff;padding:16mm 18mm 12mm;position:relative;display:flex;flex-direction:column}}
.head{{display:flex;justify-content:space-between;align-items:flex-start;padding-bottom:7mm;border-bottom:2px solid #1b1f24}}
.co{{font-size:19px;font-weight:700;letter-spacing:-.01em}}.co small{{display:block;font-size:12px;font-weight:400;color:#5b6470;letter-spacing:0;margin-top:1mm;line-height:1.45}}
.ti{{text-align:right}}.ti h1{{margin:0;font-size:26px;font-weight:300;letter-spacing:.14em}}.ti div{{color:#5b6470;margin-top:1mm}}
.cols{{display:flex;justify-content:space-between;gap:12mm;margin:10mm 0 9mm}}
.cust{{flex:1}}.lbl{{font-size:10.5px;text-transform:uppercase;letter-spacing:.09em;color:#5b6470;margin-bottom:1.5mm}}
.cust .nm{{font-size:15px;font-weight:700;margin-bottom:.5mm}}
.meta{{width:78mm}}.meta div{{display:flex;justify-content:space-between;gap:6mm;padding:1.1mm 0;border-bottom:1px solid #e3e6ea}}.meta span{{color:#5b6470}}
table{{width:100%;border-collapse:collapse;margin-top:2mm}}
th{{font-size:10.5px;text-transform:uppercase;letter-spacing:.07em;color:#5b6470;font-weight:600;text-align:left;padding:2.5mm 2mm;background:#f3f4f6;border-bottom:1px solid #d5d9de}}
td{{padding:3.2mm 2mm;border-bottom:1px solid #e3e6ea;vertical-align:top}}
th.n,td.n{{text-align:right;white-space:nowrap}}th.q,td.q{{width:20mm}}th.u,td.u{{width:14mm;text-align:left;white-space:nowrap;padding-left:3mm}}th.p,td.p{{width:30mm}}th.a,td.a{{width:34mm}}
.totals{{display:flex;justify-content:flex-end;margin-top:6mm}}.totals>div{{width:84mm}}
.totals p{{display:flex;justify-content:space-between;margin:0;padding:1.3mm 2mm}}.totals p span:first-child{{color:#5b6470}}
.totals .g{{margin-top:2mm;padding:3mm 2mm;border-top:2px solid #1b1f24;font-size:16px;font-weight:700}}.totals .g span:first-child{{color:#1b1f24}}
.pay{{margin-top:12mm;background:#f6f7f9;border-radius:2mm;padding:4mm 5mm}}.pay h3{{margin:0 0 2mm;font-size:10.5px;text-transform:uppercase;letter-spacing:.09em;color:#5b6470;font-weight:600}}
.kv{{display:grid;grid-template-columns:24mm 1fr;padding:.5mm 0}}.kv span:first-child{{color:#5b6470}}.kv span:last-child{{font-weight:600}}.pl{{padding:.5mm 0;color:#3b424b}}
.notes{{margin:6mm 0 0;color:#3b424b}}
.fine{{margin-top:auto;padding-top:10mm;color:#5b6470;font-size:11.5px}}.fine p{{margin:0 0 4mm}}
.foot{{border-top:1px solid #d5d9de;padding-top:3mm;font-size:11px;color:#5b6470;display:flex;justify-content:space-between;gap:6mm}}
.wm{{position:absolute;top:42%;left:18%;font-size:110px;font-weight:700;color:rgba(200,0,0,.08);transform:rotate(-20deg);pointer-events:none}}
@media print{{body{{background:#fff}}.page{{margin:0}}}}</style></head><body><div class=page>{wm}
<div class=head><div class=co>{sname}<small>{saddr}</small></div><div class=ti><h1>{title}</h1><div>{label_line}</div></div></div>
<div class=cols><div class=cust><div class=lbl>{ca}</div><div class=nm>{cname}</div>{caddr}</div><div class=meta>{meta}</div></div>
<table><thead><tr><th>{l_desc}</th><th class='n q'>{l_qty}</th><th class=u>{l_unit}</th><th class='n p'>{l_up}</th><th class='n a'>{l_amount} ({cur})</th></tr></thead><tbody>{rows}</tbody></table>
<div class=totals><div><p><span>{l_sub}</span><span>{net}</span></p><p><span>{l_vatw} {rate}%</span><span>{vat}</span></p><p><span>{l_round}</span><span>0,00</span></p><p class=g><span>{l_total}</span><span>{gross} {cur}</span></p></div></div>
{pay_box}{notes_html}
<div class=fine><p>{interest}.<br>{objections}.</p><div class=foot><span>{ids}</span></div></div></div></body></html>",
        hl = w.html_lang,
        title = if is_credit { w.credit_note } else { w.invoice },
        label = esc(&label),
        label_line = if label == "-" { w.draft.to_string() } else { esc(&label) },
        wm = if draft { format!("<div class=wm>{}</div>", w.draft) } else { String::new() },
        sname = esc(&s.name),
        saddr = s.address.split([',', '\n']).map(str::trim).filter(|p| !p.is_empty()).map(esc).collect::<Vec<_>>().join("<br>"),
        ca = w.customer_address,
        cname = esc(c["name"].as_str().unwrap_or("")),
        caddr = multiline(&c["address"].as_str().unwrap_or("").replace(", ", "\n")),
        meta = meta,
        l_desc = w.description,
        l_qty = w.quantity,
        l_unit = w.unit,
        l_up = w.unit_price,
        l_amount = w.amount,
        l_sub = w.subtotal,
        l_vatw = w.vat,
        l_round = w.rounding,
        l_total = w.total,
        cur = esc(currency),
        interest = w.interest,
        objections = w.objections,
        rows = rows,
        net = esc(&sv(inv["totals"]["net"].as_str().unwrap_or(""))),
        rate = inv["vat_rate"],
        vat = esc(&sv(inv["totals"]["vat"].as_str().unwrap_or(""))),
        gross = esc(&sv(inv["totals"]["gross"].as_str().unwrap_or(""))),
        pay_box = pay_box,
        notes_html = notes_html,
        ids = ids,
    );
    let _ = currency;
    let mut res = html.into_response();
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    // The page is self-contained: no scripts, and nothing it shows can run code.
    h.insert(
        "content-security-policy",
        HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'"),
    );
    Ok(res)
}

#[cfg(test)]
mod payment_block_tests {
    use super::payment_block;

    #[test]
    fn known_labels_become_rows_and_the_rest_stack() {
        let one_line = "IBAN SE22 9710 \u{b7} BIC/Swift LUNADK2B \u{b7} Account 9710-37 303 60 \u{b7} Lunar Bank A/S";
        let html = payment_block(one_line, "sv");
        assert_eq!(
            html,
            "<div class=kv><span>Iban:</span><span>SE22 9710</span></div><div class=kv><span>Bic:</span><span>LUNADK2B</span></div><div class=kv><span>Konto:</span><span>9710-37 303 60</span></div><div class=pl>Lunar Bank A/S</div>"
        );
        assert!(payment_block("Account 1", "en").contains("<span>Account:</span>"));
        assert!(payment_block("Bankgiro 123-4567\n\nIBAN SE22", "sv").contains("Bankgiro:"));
    }

    #[test]
    fn empty_text_and_markup_are_handled() {
        assert_eq!(payment_block("  \n ", "sv"), "");
        assert!(payment_block("<b>x</b>", "sv").contains("&lt;b&gt;x&lt;/b&gt;"));
    }

    #[test]
    fn amounts_use_swedish_separators() {
        assert_eq!(super::sv("72000.00"), "72\u{a0}000,00");
        assert_eq!(super::sv("-1234567.50"), "-1\u{a0}234\u{a0}567,50");
        assert_eq!(super::sv("900.00"), "900,00");
        assert_eq!(super::sv_qty("80.00"), "80");
        assert_eq!(super::sv_qty("-1.00"), "-1");
        assert_eq!(super::sv_qty("2.50"), "2,50");
    }
}
