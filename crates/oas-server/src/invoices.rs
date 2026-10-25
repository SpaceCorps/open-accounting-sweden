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
        parsed.push((
            description,
            quantity,
            if unit.is_empty() { "st".into() } else { unit },
            unit_price,
        ));
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
}

/// Build a draft invoice from a project's tracked hours that have not been invoiced yet. An hourly
/// project becomes one line of hours at its rate; a fixed-fee project becomes one line at the fee.
/// The entries are marked invoiced so they cannot be billed twice.
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
    let billed: std::collections::HashSet<i64> = conn
        .prepare("SELECT entry_id FROM invoiced_entry")?
        .query_map([], |r| r.get(0))?
        .filter_map(Result::ok)
        .collect();
    let open: Vec<_> = entries
        .into_iter()
        .filter(|e| !e.running && !billed.contains(&e.id))
        .collect();
    if open.is_empty() {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "conflict",
            "no uninvoiced time for that project in that range",
        ));
    }
    let project_name = open[0].project.clone();
    let (rate_type, rate): (String, Option<i64>) = conn
        .query_row(
            "SELECT rate_type, rate FROM project WHERE name = ?1",
            params![project_name],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| ApiError::invalid("unknown project"))?;
    let first = open
        .iter()
        .map(|e| e.date.as_str())
        .min()
        .unwrap_or("")
        .to_string();
    let last = open
        .iter()
        .map(|e| e.date.as_str())
        .max()
        .unwrap_or("")
        .to_string();
    let hours: f64 = open.iter().map(|e| e.hours).sum();
    let line = match (rate_type.as_str(), rate) {
        ("hourly", Some(r)) => LineBody {
            description: format!("{project_name}: consulting, {first} to {last}"),
            quantity: format!("{hours:.2}"),
            unit: Some("h".into()),
            unit_price: format_amount(r),
        },
        ("fixed", Some(r)) => LineBody {
            description: format!("{project_name}: fixed fee, work {first} to {last}"),
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
    let lines: Vec<(String, i64, String, i64)> = conn
        .prepare("SELECT description, quantity, unit, unit_price FROM invoice_line WHERE invoice_id = ?1 ORDER BY id")?
        .query_map(params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .filter_map(Result::ok)
        .collect();
    let t = totals(&lines, vat_rate);
    let entries: i64 = conn.query_row(
        "SELECT COUNT(*) FROM invoiced_entry WHERE invoice_id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    Ok(json!({
        "id": id,
        "label": number.map(|n| format!("{series}{n}")),
        "number": number, "status": status, "customer": customer_json(conn, customer_id)?,
        "issue_date": issue, "due_date": due, "service_date": service, "currency": currency,
        "vat_rate": vat_rate, "notes": notes, "created_by": created_by, "issued_by": issued_by, "issued_at": issued_at,
        "ledger_draft_id": draft, "time_entries": entries,
        "lines": lines.iter().map(|(d, q, u, p)| json!({
            "description": d, "quantity": format_amount(*q), "unit": u, "unit_price": format_amount(*p),
            "net": format_amount(line_net(*q, *p)),
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
    let (status, vat_rate, issue_date, customer_id): (String, i64, String, i64) = conn
        .query_row(
            "SELECT status, vat_rate, issue_date, customer_id FROM invoice WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
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
        "SELECT COALESCE(MAX(number), 0) + 1 FROM invoice WHERE series = 'F'",
        [],
        |r| r.get(0),
    )?;
    let nd = NewDraft {
        event_date: issue_date,
        description: format!("Faktura F{number} {customer}"),
        counterparty: Some(customer),
        lines: vec![
            LineIn {
                account: 1510,
                debit: Some(format_amount(t.gross)),
                credit: None,
                vat_code: None,
            },
            LineIn {
                account: income,
                debit: None,
                credit: Some(format_amount(t.net)),
                vat_code: Some(code.into()),
            },
            LineIn {
                account: vat_acct,
                debit: None,
                credit: Some(format_amount(t.vat)),
                vat_code: None,
            },
        ],
        documents: vec![],
        bank_transaction_id: None,
    };
    let draft_id = ledger::create_draft(&mut conn, &actor, &nd)?;
    conn.execute(
        "UPDATE invoice SET status = 'issued', number = ?1, issued_by = ?2, issued_at = ?3, ledger_draft_id = ?4 WHERE id = ?5",
        params![number, actor.name, utc_string(Utc::now()), draft_id, id],
    )?;
    ledger::audit(
        &conn,
        &actor,
        "invoice.issue",
        &id.to_string(),
        &format!("F{number}"),
    )?;
    Ok(Json(
        json!({"invoice": invoice_json(&conn, id)?, "ledger_draft": ledger::draft_view(&conn, draft_id)?}),
    ))
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn multiline(s: &str) -> String {
    esc(s).replace('\n', "<br>")
}

/// A printable invoice page. Print it to PDF from the browser. Draft invoices carry a watermark.
pub async fn invoice_html(
    State(st): State<Shared>,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    let conn = st.conn.lock().unwrap();
    let inv = invoice_json(&conn, id)?;
    let s = seller(&conn)?;
    let c = &inv["customer"];
    let draft = inv["status"] == "draft";
    let label = inv["label"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| "DRAFT".into());
    let currency = inv["currency"].as_str().unwrap_or("SEK");
    let mut rows = String::new();
    for l in inv["lines"].as_array().cloned().unwrap_or_default() {
        rows.push_str(&format!(
            "<tr><td>{}</td><td class=n>{} {}</td><td class=n>{}</td><td class=n>{}</td></tr>",
            esc(l["description"].as_str().unwrap_or("")),
            esc(l["quantity"].as_str().unwrap_or("")),
            esc(l["unit"].as_str().unwrap_or("")),
            esc(l["unit_price"].as_str().unwrap_or("")),
            esc(l["net"].as_str().unwrap_or("")),
        ));
    }
    let service = match inv["service_date"].as_str() {
        Some(d) if Some(d) != inv["issue_date"].as_str() => {
            format!("<div>Date of supply: {}</div>", esc(d))
        }
        _ => String::new(),
    };
    let html = format!(
        "<!doctype html><html lang=sv><head><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><title>Invoice {label}</title><style>
body{{font:14px/1.5 system-ui,sans-serif;margin:0;color:#111;background:#fff}}main{{max-width:780px;margin:2rem auto;padding:0 1.5rem;position:relative}}
h1{{font-size:1.6rem;margin:0}}.row{{display:flex;justify-content:space-between;gap:2rem;margin:1.5rem 0}}.muted{{color:#555}}
table{{width:100%;border-collapse:collapse;margin:1.5rem 0}}th,td{{padding:.5rem .4rem;border-bottom:1px solid #ddd;text-align:left}}.n{{text-align:right;white-space:nowrap}}
.tot{{margin-left:auto;width:16rem}}.tot div{{display:flex;justify-content:space-between;padding:.2rem 0}}.tot .g{{font-weight:700;border-top:2px solid #111;margin-top:.3rem;padding-top:.4rem}}
.wm{{position:absolute;top:30%;left:10%;font-size:6rem;color:rgba(200,0,0,.12);transform:rotate(-20deg);pointer-events:none}}
@media print{{main{{margin:0}}}}</style></head><body><main>{wm}
<div class=row><div><h1>Invoice {label}</h1><div class=muted>Faktura</div></div><div style=\"text-align:right\"><div>Invoice date: {issue}</div><div>Due date: {due}</div>{service}</div></div>
<div class=row><div><strong>{sname}</strong><br>{saddr}<br>Org no: {sorg}<br>VAT no: {svat}{fskatt}</div><div><strong>{cname}</strong><br>{caddr}{cvat}</div></div>
<table><thead><tr><th>Description</th><th class=n>Quantity</th><th class=n>Unit price ({cur})</th><th class=n>Amount ({cur})</th></tr></thead><tbody>{rows}</tbody></table>
<div class=tot><div><span>Total excl. VAT</span><span>{net}</span></div><div><span>VAT {rate}%</span><span>{vat}</span></div><div class=g><span>To pay ({cur})</span><span>{gross}</span></div></div>
<p class=muted>{pay}</p><p class=muted>{notes}</p></main></body></html>",
        label = esc(&label),
        wm = if draft { "<div class=wm>DRAFT</div>" } else { "" },
        issue = esc(inv["issue_date"].as_str().unwrap_or("")),
        due = esc(inv["due_date"].as_str().unwrap_or("")),
        service = service,
        sname = esc(&s.name),
        saddr = multiline(&s.address),
        sorg = esc(&s.org),
        svat = esc(&s.vat_number),
        fskatt = if s.f_skatt { "<br>Approved for F-tax (godkänd för F-skatt)" } else { "" },
        cname = esc(c["name"].as_str().unwrap_or("")),
        caddr = multiline(c["address"].as_str().unwrap_or("")),
        cvat = match c["vat_number"].as_str() { Some(v) if !v.is_empty() => format!("<br>VAT no: {}", esc(v)), _ => String::new() },
        cur = esc(currency),
        rows = rows,
        net = esc(inv["totals"]["net"].as_str().unwrap_or("")),
        rate = inv["vat_rate"],
        vat = esc(inv["totals"]["vat"].as_str().unwrap_or("")),
        gross = esc(inv["totals"]["gross"].as_str().unwrap_or("")),
        pay = multiline(&s.payment),
        notes = multiline(inv["notes"].as_str().unwrap_or("")),
    );
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
