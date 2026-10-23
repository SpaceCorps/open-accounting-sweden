//! Payroll for a small AB: employees, monthly pay runs, employer contributions, a draft ledger
//! voucher, and the AGI (arbetsgivardeklaration) XML file.
//!
//! Every rate starts UNVERIFIED and the owner marks each one after checking it against
//! Skatteverket. Only cash salary is supported: no taxable benefits, no absence or holiday pay,
//! no corrections to an earlier month. Preliminary tax is entered per person (from Skatteverket's
//! tax table) or as a percentage; the tax tables are not built in.

use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use chrono::{Datelike, Duration, NaiveDate, Utc, Weekday};
use oas_core::{ContribRates, employer_contribution, format_amount, parse_amount, parse_personnummer, utc_string};
use rusqlite::{Connection, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::ledger::{self, LineIn, NewDraft};
use crate::{Actor, ApiError, ApiResult, Role, Shared};

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS employee(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  identity TEXT NOT NULL UNIQUE,
  birth_year INTEGER NOT NULL,
  active INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS pay_run(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  period TEXT NOT NULL UNIQUE,
  pay_date TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'approved')),
  created_by TEXT NOT NULL,
  created_at TEXT NOT NULL,
  approved_by TEXT,
  approved_at TEXT,
  ledger_draft_id INTEGER
);
CREATE TABLE IF NOT EXISTS pay_line(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id INTEGER NOT NULL REFERENCES pay_run(id) ON DELETE CASCADE,
  employee_id INTEGER NOT NULL REFERENCES employee(id),
  gross INTEGER NOT NULL CHECK (gross > 0),
  tax INTEGER NOT NULL CHECK (tax >= 0),
  contribution INTEGER NOT NULL CHECK (contribution >= 0),
  rate_note TEXT NOT NULL DEFAULT '',
  UNIQUE (run_id, employee_id)
);
CREATE TABLE IF NOT EXISTS payroll_rate(
  key TEXT PRIMARY KEY,
  label TEXT NOT NULL,
  value TEXT NOT NULL,
  source TEXT NOT NULL,
  verified INTEGER NOT NULL DEFAULT 0,
  verified_by TEXT,
  verified_at TEXT
);
CREATE TRIGGER IF NOT EXISTS pay_line_locked_update BEFORE UPDATE ON pay_line
  WHEN (SELECT status FROM pay_run WHERE id = OLD.run_id) = 'approved'
  BEGIN SELECT RAISE(ABORT, 'an approved pay run cannot be changed'); END;
CREATE TRIGGER IF NOT EXISTS pay_line_locked_delete BEFORE DELETE ON pay_line
  WHEN (SELECT status FROM pay_run WHERE id = OLD.run_id) = 'approved'
  BEGIN SELECT RAISE(ABORT, 'an approved pay run cannot be changed'); END;
CREATE TRIGGER IF NOT EXISTS pay_run_locked_delete BEFORE DELETE ON pay_run
  WHEN OLD.status = 'approved'
  BEGIN SELECT RAISE(ABORT, 'an approved pay run cannot be deleted'); END;
";

const NOTE: &str = "Skatteverket pages as summarised by research on 2026-10-23; not yet checked by the owner. The youth reduction formula did not reproduce Skatteverket example 15 exactly, so validate it with the AGI test service";

// (key, label, default value)
const RATES: &[(&str, &str, &str)] = &[
    ("contrib_standard", "Employer contributions, born 1959 or later (percent)", "31.42"),
    ("contrib_senior", "Employer contributions, born 1938-1958 (percent)", "10.21"),
    ("contrib_over79", "Employer contributions, born 1937 or earlier (percent)", "0"),
    ("contrib_youth", "Reduced contributions for young employees (percent)", "20.81"),
    ("contrib_youth_cap", "Monthly pay up to which the youth rate applies (SEK)", "25000"),
    ("contrib_youth_from_birth", "Youth reduction: first birth year", "2003"),
    ("contrib_youth_to_birth", "Youth reduction: last birth year", "2007"),
    ("contrib_youth_start", "Youth reduction: first period (YYYY-MM)", "2026-04"),
    ("contrib_youth_end", "Youth reduction: last period (YYYY-MM)", "2027-09"),
];

pub fn seed(conn: &Connection) -> rusqlite::Result<()> {
    for (k, label, v) in RATES {
        conn.execute(
            "INSERT OR IGNORE INTO payroll_rate(key, label, value, source) VALUES (?1, ?2, ?3, ?4)",
            params![k, label, v, NOTE],
        )?;
    }
    Ok(())
}

fn require_admin(actor: &Actor) -> Result<(), ApiError> {
    if actor.role == Role::Admin {
        Ok(())
    } else {
        Err(ApiError::new(StatusCode::FORBIDDEN, "forbidden", "payroll changes need the admin role"))
    }
}

fn mask(identity: &str) -> String {
    format!("{}-****", &identity[..8])
}

fn load_rates(conn: &Connection) -> Result<(ContribRates, Vec<String>), ApiError> {
    let rows: Vec<(String, String, bool)> = conn
        .prepare("SELECT key, value, verified FROM payroll_rate")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? != 0)))?
        .filter_map(Result::ok)
        .collect();
    let get = |k: &str| rows.iter().find(|r| r.0 == k).map(|r| r.1.clone()).unwrap_or_default();
    let f = |k: &str| get(k).parse::<f64>().map_err(|_| ApiError::invalid(format!("payroll rate {k} is not a number")));
    let i = |k: &str| get(k).parse::<i32>().map_err(|_| ApiError::invalid(format!("payroll rate {k} is not a whole number")));
    let rates = ContribRates {
        standard: f("contrib_standard")?,
        senior: f("contrib_senior")?,
        over79: f("contrib_over79")?,
        youth: f("contrib_youth")?,
        youth_cap: (f("contrib_youth_cap")? * 100.0).round() as i64,
        youth_from_birth: i("contrib_youth_from_birth")?,
        youth_to_birth: i("contrib_youth_to_birth")?,
        youth_start: get("contrib_youth_start"),
        youth_end: get("contrib_youth_end"),
    };
    let unverified = rows.iter().filter(|r| !r.2).map(|r| r.0.clone()).collect();
    Ok((rates, unverified))
}

pub async fn list_rates(State(st): State<Shared>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    let rows: Vec<Value> = conn
        .prepare("SELECT key, label, value, source, verified, verified_by, verified_at FROM payroll_rate ORDER BY key")?
        .query_map([], |r| {
            Ok(json!({"key": r.get::<_, String>(0)?, "label": r.get::<_, String>(1)?, "value": r.get::<_, String>(2)?,
                      "source": r.get::<_, String>(3)?, "verified": r.get::<_, i64>(4)? != 0,
                      "verified_by": r.get::<_, Option<String>>(5)?, "verified_at": r.get::<_, Option<String>>(6)?}))
        })?
        .filter_map(Result::ok)
        .collect();
    Ok(Json(json!({"rates": rows})))
}

#[derive(Deserialize)]
pub struct RateIn {
    value: String,
}

pub async fn set_rate(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Path(key): Path<String>,
    Json(b): Json<RateIn>,
) -> ApiResult {
    require_admin(&actor)?;
    let value = b.value.trim().to_string();
    if value.is_empty() || value.len() > 20 || value.chars().any(|c| c.is_control()) {
        return Err(ApiError::invalid("value is empty or too long"));
    }
    let conn = st.conn.lock().unwrap();
    // Apply inside a transaction so a value the calculation cannot read is rolled back.
    let tx = conn.unchecked_transaction()?;
    let n = tx.execute(
        "UPDATE payroll_rate SET value = ?1, source = 'set by the owner', verified = 0, verified_by = NULL, verified_at = NULL WHERE key = ?2",
        params![value, key],
    )?;
    if n == 0 {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such payroll rate"));
    }
    load_rates(&tx)?;
    tx.commit()?;
    ledger::audit(&conn, &actor, "payroll.rate", &key, &value)?;
    Ok(Json(json!({"key": key, "value": value, "verified": false})))
}

#[derive(Deserialize)]
pub struct VerifyIn {
    verified: bool,
}

pub async fn verify_rate(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Path(key): Path<String>,
    Json(b): Json<VerifyIn>,
) -> ApiResult {
    require_admin(&actor)?;
    let conn = st.conn.lock().unwrap();
    let n = if b.verified {
        conn.execute(
            "UPDATE payroll_rate SET verified = 1, verified_by = ?1, verified_at = ?2 WHERE key = ?3",
            params![actor.name, utc_string(Utc::now()), key],
        )?
    } else {
        conn.execute("UPDATE payroll_rate SET verified = 0, verified_by = NULL, verified_at = NULL WHERE key = ?1", params![key])?
    };
    if n == 0 {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such payroll rate"));
    }
    ledger::audit(&conn, &actor, if b.verified { "payroll.verify" } else { "payroll.unverify" }, &key, "")?;
    Ok(Json(json!({"key": key, "verified": b.verified})))
}

#[derive(Deserialize)]
pub struct NewEmployee {
    name: String,
    identity: String,
}

pub async fn add_employee(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Json(b): Json<NewEmployee>,
) -> ApiResult {
    require_admin(&actor)?;
    let name = b.name.trim().to_string();
    if name.is_empty() || name.chars().count() > 100 || name.chars().any(|c| c.is_control()) {
        return Err(ApiError::invalid("name is empty or too long"));
    }
    let (identity, birth_year) = parse_personnummer(&b.identity).map_err(ApiError::invalid)?;
    let conn = st.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO employee(name, identity, birth_year, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![name, identity, birth_year, utc_string(Utc::now())],
    )
    .map_err(|_| ApiError::new(StatusCode::CONFLICT, "conflict", "an employee with that identity number already exists"))?;
    let id = conn.last_insert_rowid();
    ledger::audit(&conn, &actor, "employee.add", &id.to_string(), &name)?;
    Ok(Json(json!({"employee": {"id": id, "name": name, "identity": identity, "birth_year": birth_year, "active": true}})))
}

pub async fn list_employees(State(st): State<Shared>, Extension(actor): Extension<Actor>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    let rows: Vec<Value> = conn
        .prepare("SELECT id, name, identity, birth_year, active FROM employee ORDER BY id")?
        .query_map([], |r| {
            let identity: String = r.get(2)?;
            Ok(json!({"id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?,
                      "identity": if actor.role == Role::Admin { identity.clone() } else { mask(&identity) },
                      "birth_year": r.get::<_, i64>(3)?, "active": r.get::<_, i64>(4)? != 0}))
        })?
        .filter_map(Result::ok)
        .collect();
    Ok(Json(json!({"employees": rows})))
}

pub async fn deactivate_employee(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<i64>,
) -> ApiResult {
    require_admin(&actor)?;
    let conn = st.conn.lock().unwrap();
    let n = conn.execute("UPDATE employee SET active = 0 WHERE id = ?1", params![id])?;
    if n == 0 {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such employee"));
    }
    ledger::audit(&conn, &actor, "employee.deactivate", &id.to_string(), "")?;
    Ok(Json(json!({"status": "ok"})))
}

#[derive(Deserialize)]
pub struct LineIn2 {
    employee_id: i64,
    gross: String,
    /// Preliminary tax withheld, in SEK. Give this or tax_percent.
    tax: Option<String>,
    tax_percent: Option<f64>,
}

#[derive(Deserialize)]
pub struct NewRun {
    period: String,
    pay_date: String,
    lines: Vec<LineIn2>,
}

fn check_period(p: &str) -> Result<(), ApiError> {
    NaiveDate::parse_from_str(&format!("{p}-01"), "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| ApiError::invalid("period must look like 2026-10"))
}

pub async fn create_run(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Json(b): Json<NewRun>,
) -> ApiResult {
    require_admin(&actor)?;
    check_period(&b.period)?;
    NaiveDate::parse_from_str(&b.pay_date, "%Y-%m-%d").map_err(|_| ApiError::invalid("pay_date must be YYYY-MM-DD"))?;
    if b.lines.is_empty() {
        return Err(ApiError::invalid("a pay run needs at least one line"));
    }
    let mut conn = st.conn.lock().unwrap();
    let (rates, _) = load_rates(&conn)?;
    let tx = conn.transaction()?;
    let exists: i64 = tx.query_row("SELECT COUNT(*) FROM pay_run WHERE period = ?1", params![b.period], |r| r.get(0))?;
    if exists > 0 {
        return Err(ApiError::new(StatusCode::CONFLICT, "conflict", format!("there is already a pay run for {}", b.period)));
    }
    tx.execute(
        "INSERT INTO pay_run(period, pay_date, created_by, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![b.period, b.pay_date, actor.name, utc_string(Utc::now())],
    )?;
    let run_id = tx.last_insert_rowid();
    let mut seen = std::collections::HashSet::new();
    for l in &b.lines {
        if !seen.insert(l.employee_id) {
            return Err(ApiError::invalid(format!("employee {} appears twice", l.employee_id)));
        }
        let (birth_year, active): (i32, i64) = tx
            .query_row("SELECT birth_year, active FROM employee WHERE id = ?1", params![l.employee_id], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|_| ApiError::invalid(format!("unknown employee {}", l.employee_id)))?;
        if active == 0 {
            return Err(ApiError::invalid(format!("employee {} is not active", l.employee_id)));
        }
        let gross = parse_amount(&l.gross).map_err(ApiError::invalid)?;
        if gross == 0 || gross > 100_000_000 {
            return Err(ApiError::invalid("gross pay must be above zero and below 1 000 000 SEK"));
        }
        let tax = match (&l.tax, l.tax_percent) {
            (Some(t), _) => parse_amount(t).map_err(ApiError::invalid)?,
            (None, Some(p)) if (0.0..=70.0).contains(&p) => ((gross as f64 * p / 100.0) / 100.0).round() as i64 * 100,
            _ => return Err(ApiError::invalid("give tax (SEK) or tax_percent (0-70) for every line")),
        };
        if tax > gross {
            return Err(ApiError::invalid("tax cannot exceed gross pay"));
        }
        let (contribution, note) = employer_contribution(gross, birth_year, &b.period, &rates);
        tx.execute(
            "INSERT INTO pay_line(run_id, employee_id, gross, tax, contribution, rate_note) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![run_id, l.employee_id, gross, tax, contribution, note],
        )?;
    }
    ledger::audit(&tx, &actor, "payroll.run.create", &b.period, "")?;
    tx.commit()?;
    Ok(Json(json!({"run": run_json(&conn, run_id, true)?})))
}

/// AGI and salary payment deadline: the 12th of the following month (the 17th in January and
/// August), moved to the next weekday. Public holidays are not considered. Unverified.
fn agi_due(period: &str) -> Option<String> {
    let first = NaiveDate::parse_from_str(&format!("{period}-01"), "%Y-%m-%d").ok()?;
    let (y, m) = if first.month() == 12 { (first.year() + 1, 1) } else { (first.year(), first.month() + 1) };
    let day = if m == 1 || m == 8 { 17 } else { 12 };
    let mut d = NaiveDate::from_ymd_opt(y, m, day)?;
    while matches!(d.weekday(), Weekday::Sat | Weekday::Sun) {
        d += Duration::days(1);
    }
    Some(d.to_string())
}

fn run_json(conn: &Connection, id: i64, admin: bool) -> Result<Value, ApiError> {
    let run = conn
        .query_row(
            "SELECT period, pay_date, status, created_by, created_at, approved_by, approved_at, ledger_draft_id FROM pay_run WHERE id = ?1",
            params![id],
            |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?, r.get::<_, Option<String>>(5)?, r.get::<_, Option<String>>(6)?, r.get::<_, Option<i64>>(7)?))
            },
        )
        .map_err(|_| ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such pay run"))?;
    let lines: Vec<(i64, String, String, i64, i64, i64, String)> = conn
        .prepare(
            "SELECT e.id, e.name, e.identity, l.gross, l.tax, l.contribution, l.rate_note
             FROM pay_line l JOIN employee e ON e.id = l.employee_id WHERE l.run_id = ?1 ORDER BY e.id",
        )?
        .query_map(params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))?
        .filter_map(Result::ok)
        .collect();
    let (g, t, c) = lines.iter().fold((0, 0, 0), |a, l| (a.0 + l.3, a.1 + l.4, a.2 + l.5));
    let (_, unverified) = load_rates(conn)?;
    let line_json: Vec<Value> = lines
        .iter()
        .map(|l| {
            json!({"employee_id": l.0, "name": l.1, "identity": if admin { l.2.clone() } else { mask(&l.2) },
                   "gross": format_amount(l.3), "tax": format_amount(l.4), "net": format_amount(l.3 - l.4),
                   "contribution": format_amount(l.5), "rate_note": l.6})
        })
        .collect();
    Ok(json!({
        "id": id, "period": run.0, "pay_date": run.1, "status": run.2, "created_by": run.3, "created_at": run.4,
        "approved_by": run.5, "approved_at": run.6, "ledger_draft_id": run.7,
        "lines": line_json,
        "totals": {"gross": format_amount(g), "tax": format_amount(t), "net": format_amount(g - t),
                   "contributions": format_amount(c), "to_skattekonto": format_amount(t + c)},
        "agi_due": agi_due(&run.0),
        "agi_due_note": "Declaration and payment of tax and contributions are due on the same day: the 12th of the following month (the 17th in January and August), moved to the next weekday. For turnover up to 40 MSEK. Public holidays not considered; unverified",
        "unverified_rates": unverified,
        "ready_for_agi": run.2 == "approved" && load_rates(conn).map(|r| r.1.is_empty()).unwrap_or(false),
    }))
}

pub async fn list_runs(State(st): State<Shared>, Extension(actor): Extension<Actor>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM pay_run ORDER BY period DESC")?
        .query_map([], |r| r.get(0))?
        .filter_map(Result::ok)
        .collect();
    let admin = actor.role == Role::Admin;
    let runs: Result<Vec<Value>, ApiError> = ids.into_iter().map(|id| run_json(&conn, id, admin)).collect();
    Ok(Json(json!({"runs": runs?})))
}

pub async fn get_run(State(st): State<Shared>, Extension(actor): Extension<Actor>, Path(id): Path<i64>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    Ok(Json(json!({"run": run_json(&conn, id, actor.role == Role::Admin)?})))
}

pub async fn delete_run(State(st): State<Shared>, Extension(actor): Extension<Actor>, Path(id): Path<i64>) -> ApiResult {
    require_admin(&actor)?;
    let conn = st.conn.lock().unwrap();
    let status: Option<String> = conn.query_row("SELECT status FROM pay_run WHERE id = ?1", params![id], |r| r.get(0)).ok();
    match status.as_deref() {
        None => return Err(ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such pay run")),
        Some("draft") => {}
        Some(_) => return Err(ApiError::new(StatusCode::CONFLICT, "conflict", "an approved pay run cannot be deleted")),
    }
    conn.execute("DELETE FROM pay_run WHERE id = ?1", params![id])?;
    ledger::audit(&conn, &actor, "payroll.run.delete", &id.to_string(), "")?;
    Ok(Json(json!({"status": "ok"})))
}

/// Approve a pay run: it can no longer change, and a draft voucher is proposed for the ledger.
/// Salaries (7210) and employer contributions (7510) are debited; withheld tax (2710), contributions
/// payable (2731) and the net salary paid from the bank (1930) are credited.
pub async fn approve_run(State(st): State<Shared>, Extension(actor): Extension<Actor>, Path(id): Path<i64>) -> ApiResult {
    require_admin(&actor)?;
    let mut conn = st.conn.lock().unwrap();
    let (period, pay_date, status): (String, String, String) = conn
        .query_row("SELECT period, pay_date, status FROM pay_run WHERE id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(|_| ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such pay run"))?;
    if status != "draft" {
        return Err(ApiError::new(StatusCode::CONFLICT, "conflict", "this pay run is already approved"));
    }
    let (g, t, c): (i64, i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(gross),0), COALESCE(SUM(tax),0), COALESCE(SUM(contribution),0) FROM pay_line WHERE run_id = ?1",
        params![id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let line = |account: i64, debit: Option<i64>, credit: Option<i64>| LineIn {
        account,
        debit: debit.filter(|v| *v > 0).map(format_amount),
        credit: credit.filter(|v| *v > 0).map(format_amount),
        vat_code: None,
    };
    let mut lines = vec![line(7210, Some(g), None), line(2710, None, Some(t)), line(1930, None, Some(g - t))];
    if c > 0 {
        lines.push(line(7510, Some(c), None));
        lines.push(line(2731, None, Some(c)));
    }
    lines.retain(|l| l.debit.is_some() || l.credit.is_some());
    let nd = NewDraft {
        event_date: pay_date,
        description: format!("Payroll {period}"),
        counterparty: None,
        lines,
        documents: vec![],
        bank_transaction_id: None,
    };
    let draft_id = ledger::create_draft(&mut conn, &actor, &nd)?;
    conn.execute(
        "UPDATE pay_run SET status = 'approved', approved_by = ?1, approved_at = ?2, ledger_draft_id = ?3 WHERE id = ?4",
        params![actor.name, utc_string(Utc::now()), draft_id, id],
    )?;
    ledger::audit(&conn, &actor, "payroll.run.approve", &id.to_string(), &period)?;
    Ok(Json(json!({"run": run_json(&conn, id, true)?, "ledger_draft": ledger::draft_view(&conn, draft_id)?})))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Whole kronor for the AGI file, rounded to the nearest krona.
fn kr(ore: i64) -> i64 {
    (ore as f64 / 100.0).round() as i64
}

/// Whether a 10-digit organisation number fits the pattern Skatteverket's schema accepts for the
/// employer id (16 + number): first two digits 1x, 2x, 3x, 5x, 60-64, 66, 68, 7x, 8x or 9x, and a
/// third digit of 2 or more.
fn plausible_org(d: &str) -> bool {
    if d.len() != 10 || !d.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let first_two: u32 = d[..2].parse().unwrap_or(0);
    let third: u32 = d[2..3].parse().unwrap_or(0);
    let ok_prefix = matches!(first_two / 10, 1 | 2 | 3 | 5 | 7 | 8 | 9) || matches!(first_two, 60..=64 | 66 | 68);
    ok_prefix && third >= 2
}

/// AGI XML for an approved pay run, following the structure of Skatteverket's schema 1.1 and its
/// published example files (technical description 1.1.18.2). It has NOT yet been run through
/// Skatteverket's AGI test service, so treat it as unverified until it passes there.
pub async fn agi_xml(State(st): State<Shared>, Extension(actor): Extension<Actor>, Path(id): Path<i64>) -> Result<Response, ApiError> {
    require_admin(&actor)?;
    let conn = st.conn.lock().unwrap();
    let (period, status): (String, String) = conn
        .query_row("SELECT period, status FROM pay_run WHERE id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|_| ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such pay run"))?;
    if status != "approved" {
        return Err(ApiError::new(StatusCode::CONFLICT, "conflict", "approve the pay run before creating the AGI file"));
    }
    let (org, name, cname, phone, email): (String, String, String, String, String) = conn
        .query_row(
            "SELECT org_number, name, contact_name, contact_phone, contact_email FROM company WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .map_err(|_| ApiError::invalid("set the company first: oas company set NAME --org-number ... --contact-name ... --contact-phone ... --contact-email ..."))?;
    let org_digits: String = org.chars().filter(|c| c.is_ascii_digit()).collect();
    if !plausible_org(&org_digits) {
        return Err(ApiError::invalid("the organisation number does not look like a Swedish organisation number (10 digits, for example 556026-9986)"));
    }
    if cname.is_empty() || phone.is_empty() || email.is_empty() {
        return Err(ApiError::invalid("the AGI file needs a technical contact: oas company set ... --contact-name --contact-phone --contact-email"));
    }
    let lines: Vec<(i64, String, i64, i64, i64)> = conn
        .prepare(
            "SELECT e.id, e.identity, l.gross, l.tax, l.contribution FROM pay_line l JOIN employee e ON e.id = l.employee_id
             WHERE l.run_id = ?1 ORDER BY e.id",
        )?
        .query_map(params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .filter_map(Result::ok)
        .collect();
    let (_, unverified) = load_rates(&conn)?;
    let agid = format!("16{org_digits}");
    let period_code = period.replace('-', "");
    let sum_tax: i64 = lines.iter().map(|l| kr(l.3)).sum();
    let sum_contrib: i64 = lines.iter().map(|l| kr(l.4)).sum();
    let mut x = String::new();
    x.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\n");
    x.push_str(&format!(
        "<!-- Generated by open-accounting-sweden for {}. Structure follows Skatteverket schema 1.1 but has not been validated with Skatteverket's AGI test service. Unverified rates: {}. -->\n",
        xml_escape(&name),
        if unverified.is_empty() { "none".to_string() } else { unverified.join(", ") }
    ));
    x.push_str(&agi_body(&agid, &period_code, &Contact { name: &cname, phone: &phone, email: &email }, &lines, sum_tax, sum_contrib));
    let mut res = x.into_response();
    let h = res.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/xml; charset=utf-8"));
    h.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"agi-{period}.xml\"")).unwrap_or(HeaderValue::from_static("attachment")),
    );
    Ok(res)
}

struct Contact<'a> {
    name: &'a str,
    phone: &'a str,
    email: &'a str,
}

fn agi_body(agid: &str, period: &str, c: &Contact, lines: &[(i64, String, i64, i64, i64)], sum_tax: i64, sum_contrib: i64) -> String {
    let created = Utc::now().format("%Y-%m-%dT%H:%M:%S");
    let mut x = String::new();
    x.push_str("<Skatteverket omrade=\"Arbetsgivardeklaration\"\n  xmlns=\"http://xmls.skatteverket.se/se/skatteverket/da/instans/schema/1.1\"\n  xmlns:agd=\"http://xmls.skatteverket.se/se/skatteverket/da/komponent/schema/1.1\"\n  xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"\n  xsi:schemaLocation=\"http://xmls.skatteverket.se/se/skatteverket/da/instans/schema/1.1 http://xmls.skatteverket.se/se/skatteverket/da/arbetsgivardeklaration/arbetsgivardeklaration_1.1.xsd\">\n");
    x.push_str(&format!(
        "  <agd:Avsandare>\n    <agd:Programnamn>open-accounting-sweden</agd:Programnamn>\n    <agd:Organisationsnummer>{agid}</agd:Organisationsnummer>\n    <agd:TekniskKontaktperson>\n      <agd:Namn>{}</agd:Namn>\n      <agd:Telefon>{}</agd:Telefon>\n      <agd:Epostadress>{}</agd:Epostadress>\n    </agd:TekniskKontaktperson>\n    <agd:Skapad>{created}</agd:Skapad>\n  </agd:Avsandare>\n",
        xml_escape(c.name), xml_escape(c.phone), xml_escape(c.email)
    ));
    x.push_str("  <agd:Blankettgemensamt></agd:Blankettgemensamt>\n");
    x.push_str(&format!(
        "  <agd:Blankett>\n    <agd:Arendeinformation>\n      <agd:Arendeagare>{agid}</agd:Arendeagare>\n      <agd:Period>{period}</agd:Period>\n    </agd:Arendeinformation>\n    <agd:Blankettinnehall>\n      <agd:HU>\n        <agd:ArbetsgivareHUGROUP>\n          <agd:AgRegistreradId faltkod=\"201\">{agid}</agd:AgRegistreradId>\n        </agd:ArbetsgivareHUGROUP>\n        <agd:RedovisningsPeriod faltkod=\"006\">{period}</agd:RedovisningsPeriod>\n        <agd:SummaArbAvgSlf faltkod=\"487\">{sum_contrib}</agd:SummaArbAvgSlf>\n        <agd:SummaSkatteavdr faltkod=\"497\">{sum_tax}</agd:SummaSkatteavdr>\n      </agd:HU>\n    </agd:Blankettinnehall>\n  </agd:Blankett>\n"
    ));
    for (emp_id, identity, gross, tax, _) in lines {
        // The specification number is the employee id, so a later correction hits the same entry.
        x.push_str(&format!(
            "  <agd:Blankett>\n    <agd:Arendeinformation>\n      <agd:Arendeagare>{agid}</agd:Arendeagare>\n      <agd:Period>{period}</agd:Period>\n    </agd:Arendeinformation>\n    <agd:Blankettinnehall>\n      <agd:IU>\n        <agd:ArbetsgivareIUGROUP>\n          <agd:AgRegistreradId faltkod=\"201\">{agid}</agd:AgRegistreradId>\n        </agd:ArbetsgivareIUGROUP>\n        <agd:BetalningsmottagareIUGROUP>\n          <agd:BetalningsmottagareIDChoice>\n            <agd:BetalningsmottagarId faltkod=\"215\">{identity}</agd:BetalningsmottagarId>\n          </agd:BetalningsmottagareIDChoice>\n        </agd:BetalningsmottagareIUGROUP>\n        <agd:RedovisningsPeriod faltkod=\"006\">{period}</agd:RedovisningsPeriod>\n        <agd:Specifikationsnummer faltkod=\"570\">{emp_id}</agd:Specifikationsnummer>\n        <agd:KontantErsattningUlagAG faltkod=\"011\">{}</agd:KontantErsattningUlagAG>\n        <agd:AvdrPrelSkatt faltkod=\"001\">{}</agd:AvdrPrelSkatt>\n      </agd:IU>\n    </agd:Blankettinnehall>\n  </agd:Blankett>\n",
            kr(*gross),
            kr(*tax)
        ));
    }
    x.push_str("</Skatteverket>\n");
    x
}
