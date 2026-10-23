//! Append-only double-entry ledger with propose-and-approve postings.
//!
//! Agents (and humans) propose drafts. Only the admin role approves a draft, which
//! assigns the next gap-free voucher number in the series and chains a hash to the
//! previous voucher. Posted vouchers and entries can never be updated or deleted;
//! corrections are new reversing vouchers that link to the original.

use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use chrono::{Datelike, NaiveDate, Utc};
use oas_core::{format_amount, parse_amount, sie_quote, to_cp437};
use rusqlite::{Connection, params};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{Actor, ApiError, ApiResult, Role, Shared};

const SERIES: &str = "A";
const MAX_ORE: i64 = 10_000_000_000_000;

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS company(
  id INTEGER PRIMARY KEY CHECK (id = 1),
  name TEXT NOT NULL,
  org_number TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS account(
  number INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('T','S','K','I'))
);
CREATE TABLE IF NOT EXISTS draft(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  event_date TEXT NOT NULL,
  description TEXT NOT NULL,
  counterparty TEXT NOT NULL DEFAULT '',
  created_by TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS draft_line(
  id INTEGER PRIMARY KEY,
  draft_id INTEGER NOT NULL REFERENCES draft(id) ON DELETE CASCADE,
  account INTEGER NOT NULL REFERENCES account(number),
  debit INTEGER NOT NULL DEFAULT 0,
  credit INTEGER NOT NULL DEFAULT 0,
  CHECK (debit >= 0 AND credit >= 0 AND (debit = 0 OR credit = 0) AND debit + credit > 0)
);
CREATE TABLE IF NOT EXISTS voucher(
  id INTEGER PRIMARY KEY,
  series TEXT NOT NULL,
  number INTEGER NOT NULL,
  compile_date TEXT NOT NULL,
  event_date TEXT NOT NULL,
  description TEXT NOT NULL,
  counterparty TEXT NOT NULL DEFAULT '',
  created_by TEXT NOT NULL,
  approved_by TEXT NOT NULL,
  created_at TEXT NOT NULL,
  corrects_voucher_id INTEGER REFERENCES voucher(id),
  prev_hash TEXT NOT NULL,
  hash TEXT NOT NULL,
  UNIQUE (series, number)
);
CREATE TABLE IF NOT EXISTS entry(
  id INTEGER PRIMARY KEY,
  voucher_id INTEGER NOT NULL REFERENCES voucher(id),
  line_no INTEGER NOT NULL,
  account INTEGER NOT NULL REFERENCES account(number),
  debit INTEGER NOT NULL,
  credit INTEGER NOT NULL,
  CHECK (debit >= 0 AND credit >= 0 AND (debit = 0 OR credit = 0) AND debit + credit > 0)
);
CREATE TABLE IF NOT EXISTS audit_log(
  id INTEGER PRIMARY KEY,
  at TEXT NOT NULL,
  actor TEXT NOT NULL,
  action TEXT NOT NULL,
  object TEXT NOT NULL,
  detail TEXT NOT NULL DEFAULT ''
);
CREATE TRIGGER IF NOT EXISTS voucher_no_update BEFORE UPDATE ON voucher
  BEGIN SELECT RAISE(ABORT, 'ledger is append-only'); END;
CREATE TRIGGER IF NOT EXISTS voucher_no_delete BEFORE DELETE ON voucher
  BEGIN SELECT RAISE(ABORT, 'ledger is append-only'); END;
CREATE TRIGGER IF NOT EXISTS entry_no_update BEFORE UPDATE ON entry
  BEGIN SELECT RAISE(ABORT, 'ledger is append-only'); END;
CREATE TRIGGER IF NOT EXISTS entry_no_delete BEFORE DELETE ON entry
  BEGIN SELECT RAISE(ABORT, 'ledger is append-only'); END;
CREATE TRIGGER IF NOT EXISTS audit_no_update BEFORE UPDATE ON audit_log
  BEGIN SELECT RAISE(ABORT, 'audit log is append-only'); END;
CREATE TRIGGER IF NOT EXISTS audit_no_delete BEFORE DELETE ON audit_log
  BEGIN SELECT RAISE(ABORT, 'audit log is append-only'); END;
";

// A small starter set of commonly used BAS account numbers for a service company.
// Names are informal; BAS licensing for bundling the full plan is still an open question.
const SEED: &[(i64, &str, &str)] = &[
    (1510, "Kundfordringar", "T"),
    (1930, "Foretagskonto", "T"),
    (2081, "Aktiekapital", "S"),
    (2091, "Balanserad vinst eller forlust", "S"),
    (2440, "Leverantorsskulder", "S"),
    (2611, "Utgaende moms forsaljning Sverige 25 procent", "S"),
    (2614, "Utgaende moms omvand skattskyldighet 25 procent", "S"),
    (2641, "Debiterad ingaende moms", "S"),
    (2645, "Beraknad ingaende moms forvarv fran utlandet", "S"),
    (2710, "Personalskatt", "S"),
    (2731, "Avrakning lagstadgade sociala avgifter", "S"),
    (3041, "Forsaljning tjanster Sverige 25 procent moms", "I"),
    (5410, "Forbrukningsinventarier", "K"),
    (6540, "IT-tjanster", "K"),
    (6570, "Bankkostnader", "K"),
    (7210, "Loner till tjansteman", "K"),
    (7510, "Lagstadgade sociala avgifter", "K"),
];

pub fn seed(conn: &Connection) -> rusqlite::Result<()> {
    for (n, name, kind) in SEED {
        conn.execute(
            "INSERT OR IGNORE INTO account(number, name, kind) VALUES (?1, ?2, ?3)",
            params![n, name, kind],
        )?;
    }
    Ok(())
}

fn require_admin(role: Role) -> Result<(), ApiError> {
    if role == Role::Admin {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "this token can propose and read but cannot approve, reject or correct",
        ))
    }
}

pub(crate) fn audit(conn: &Connection, actor: &Actor, action: &str, object: &str, detail: &str) -> Result<(), ApiError> {
    conn.execute(
        "INSERT INTO audit_log(at, actor, action, object, detail) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![oas_core::utc_string(Utc::now()), actor.name, action, object, detail],
    )?;
    Ok(())
}

#[derive(Deserialize)]
pub(crate) struct LineIn {
    pub(crate) account: i64,
    pub(crate) debit: Option<String>,
    pub(crate) credit: Option<String>,
}

/// Validate lines and return (account, debit, credit) in öre.
fn validate_lines(conn: &Connection, lines: &[LineIn]) -> Result<Vec<(i64, i64, i64)>, ApiError> {
    if lines.len() < 2 {
        return Err(ApiError::invalid("a voucher needs at least two lines"));
    }
    let mut out = vec![];
    let (mut dsum, mut csum) = (0i64, 0i64);
    for l in lines {
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM account WHERE number = ?1",
            params![l.account],
            |r| r.get(0),
        )?;
        if exists == 0 {
            return Err(ApiError::invalid(format!("unknown account {}", l.account)));
        }
        let d = l.debit.as_deref().map(parse_amount).transpose().map_err(ApiError::invalid)?.unwrap_or(0);
        let c = l.credit.as_deref().map(parse_amount).transpose().map_err(ApiError::invalid)?.unwrap_or(0);
        if (d > 0) == (c > 0) {
            return Err(ApiError::invalid(format!(
                "line for account {} needs exactly one of debit or credit above zero",
                l.account
            )));
        }
        if d > MAX_ORE || c > MAX_ORE {
            return Err(ApiError::invalid("amount too large"));
        }
        dsum += d;
        csum += c;
        out.push((l.account, d, c));
    }
    if dsum != csum {
        return Err(ApiError::invalid(format!(
            "voucher does not balance: debit {} vs credit {}",
            format_amount(dsum),
            format_amount(csum)
        )));
    }
    Ok(out)
}

fn check_date(s: &str) -> Result<(), ApiError> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| ApiError::invalid(format!("invalid date '{s}', expected YYYY-MM-DD")))
}

fn clean_text(s: &str, field: &str, max: usize) -> Result<String, ApiError> {
    let v = s.trim();
    if v.chars().count() > max || v.chars().any(|c| c.is_control()) {
        return Err(ApiError::invalid(format!("{field} is too long or has control characters")));
    }
    Ok(v.to_string())
}

fn account_name(conn: &Connection, n: i64) -> String {
    conn.query_row("SELECT name FROM account WHERE number = ?1", params![n], |r| r.get(0))
        .unwrap_or_default()
}

pub(crate) fn draft_view(conn: &Connection, id: i64) -> Result<Value, ApiError> {
    let d = conn
        .query_row(
            "SELECT event_date, description, counterparty, created_by, created_at FROM draft WHERE id = ?1",
            params![id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            },
        )
        .map_err(|_| ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such draft"))?;
    let mut stmt =
        conn.prepare("SELECT account, debit, credit FROM draft_line WHERE draft_id = ?1 ORDER BY id")?;
    let lines: Vec<Value> = stmt
        .query_map(params![id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)))?
        .filter_map(Result::ok)
        .map(|(a, d, c)| {
            json!({"account": a, "name": account_name(conn, a),
                   "debit": format_amount(d), "credit": format_amount(c)})
        })
        .collect();
    let documents: Vec<Value> = conn
        .prepare("SELECT d.id, d.filename FROM draft_document dd JOIN document d ON d.id = dd.document_id WHERE dd.draft_id = ?1 ORDER BY d.id")?
        .query_map(params![id], |r| Ok(json!({"id": r.get::<_, i64>(0)?, "filename": r.get::<_, String>(1)?})))?
        .filter_map(Result::ok)
        .collect();
    let bank_tx: Option<i64> = conn
        .query_row("SELECT bank_transaction_id FROM draft_bank WHERE draft_id = ?1", params![id], |r| r.get(0))
        .ok();
    Ok(json!({"id": id, "event_date": d.0, "description": d.1, "counterparty": d.2,
              "created_by": d.3, "created_at": d.4, "lines": lines, "documents": documents,
              "bank_transaction_id": bank_tx}))
}

fn voucher_lines(conn: &Connection, voucher_id: i64) -> Result<Vec<(i64, i64, i64)>, ApiError> {
    let mut stmt =
        conn.prepare("SELECT account, debit, credit FROM entry WHERE voucher_id = ?1 ORDER BY line_no")?;
    let rows = stmt.query_map(params![voucher_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn voucher_view(conn: &Connection, id: i64) -> Result<Value, ApiError> {
    let row = conn
        .query_row(
            "SELECT series, number, compile_date, event_date, description, counterparty, created_by,
                    approved_by, created_at, corrects_voucher_id, hash FROM voucher WHERE id = ?1",
            params![id],
            |r| {
                Ok(json!({
                    "id": id,
                    "label": format!("{}{}", r.get::<_, String>(0)?, r.get::<_, i64>(1)?),
                    "series": r.get::<_, String>(0)?,
                    "number": r.get::<_, i64>(1)?,
                    "compile_date": r.get::<_, String>(2)?,
                    "event_date": r.get::<_, String>(3)?,
                    "description": r.get::<_, String>(4)?,
                    "counterparty": r.get::<_, String>(5)?,
                    "created_by": r.get::<_, String>(6)?,
                    "approved_by": r.get::<_, String>(7)?,
                    "created_at": r.get::<_, String>(8)?,
                    "corrects": r.get::<_, Option<i64>>(9)?,
                    "hash": r.get::<_, String>(10)?,
                }))
            },
        )
        .map_err(|_| ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such voucher"))?;
    let lines: Vec<Value> = voucher_lines(conn, id)?
        .into_iter()
        .map(|(a, d, c)| {
            json!({"account": a, "name": account_name(conn, a),
                   "debit": format_amount(d), "credit": format_amount(c)})
        })
        .collect();
    let documents: Vec<Value> = conn
        .prepare("SELECT d.id, d.filename FROM voucher_document vd JOIN document d ON d.id = vd.document_id WHERE vd.voucher_id = ?1 ORDER BY d.id")?
        .query_map(params![id], |r| Ok(json!({"id": r.get::<_, i64>(0)?, "filename": r.get::<_, String>(1)?})))?
        .filter_map(Result::ok)
        .collect();
    let mut v = row;
    v["lines"] = json!(lines);
    v["documents"] = json!(documents);
    Ok(v)
}

fn canonical(
    prev_hash: &str,
    series: &str,
    number: i64,
    compile: &str,
    event: &str,
    desc: &str,
    cp: &str,
    corrects: Option<i64>,
    lines: &[(i64, i64, i64)],
) -> String {
    let body: Vec<String> = lines.iter().map(|(a, d, c)| format!("{a}:{d}:{c}")).collect();
    let text = format!(
        "{prev_hash}|{series}|{number}|{compile}|{event}|{desc}|{cp}|{}|{}",
        corrects.map(|c| c.to_string()).unwrap_or_default(),
        body.join(";")
    );
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

/// Insert a posted voucher with the next gap-free number. Call inside a transaction.
#[allow(clippy::too_many_arguments)]
fn post_voucher(
    conn: &Connection,
    event_date: &str,
    description: &str,
    counterparty: &str,
    created_by: &str,
    approved_by: &str,
    created_at: &str,
    corrects: Option<i64>,
    lines: &[(i64, i64, i64)],
) -> Result<i64, ApiError> {
    let number: i64 = conn.query_row(
        "SELECT COALESCE(MAX(number), 0) + 1 FROM voucher WHERE series = ?1",
        params![SERIES],
        |r| r.get(0),
    )?;
    let prev_hash: String = conn
        .query_row(
            "SELECT hash FROM voucher WHERE series = ?1 ORDER BY number DESC LIMIT 1",
            params![SERIES],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "genesis".into());
    let compile = Utc::now().format("%Y-%m-%d").to_string();
    let hash = canonical(&prev_hash, SERIES, number, &compile, event_date, description, counterparty, corrects, lines);
    conn.execute(
        "INSERT INTO voucher(series, number, compile_date, event_date, description, counterparty,
            created_by, approved_by, created_at, corrects_voucher_id, prev_hash, hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![SERIES, number, compile, event_date, description, counterparty, created_by,
                approved_by, created_at, corrects, prev_hash, hash],
    )?;
    let id = conn.last_insert_rowid();
    for (i, (a, d, c)) in lines.iter().enumerate() {
        conn.execute(
            "INSERT INTO entry(voucher_id, line_no, account, debit, credit) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, i as i64 + 1, a, d, c],
        )?;
    }
    Ok(id)
}

pub async fn me(Extension(actor): Extension<Actor>) -> Json<Value> {
    Json(json!({"username": actor.name, "role": actor.role.name(), "can_approve": actor.role == Role::Admin}))
}

pub async fn list_accounts(State(st): State<Shared>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    let mut stmt = conn.prepare("SELECT number, name, kind FROM account ORDER BY number")?;
    let rows: Vec<Value> = stmt
        .query_map([], |r| {
            Ok(json!({"number": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?,
                      "kind": r.get::<_, String>(2)?}))
        })?
        .filter_map(Result::ok)
        .collect();
    Ok(Json(json!({"accounts": rows})))
}

#[derive(Deserialize)]
pub struct NewAccount {
    number: i64,
    name: String,
    kind: String,
}

pub async fn add_account(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Json(b): Json<NewAccount>,
) -> ApiResult {
    require_admin(actor.role)?;
    if !(1000..=9999).contains(&b.number) {
        return Err(ApiError::invalid("account number must be 1000-9999"));
    }
    let kind = b.kind.trim().to_uppercase();
    if !["T", "S", "K", "I"].contains(&kind.as_str()) {
        return Err(ApiError::invalid("kind must be T (asset), S (liability/equity), K (cost) or I (income)"));
    }
    let name = clean_text(&b.name, "name", 100)?;
    let conn = st.conn.lock().unwrap();
    conn.execute("INSERT INTO account(number, name, kind) VALUES (?1, ?2, ?3)", params![b.number, name, kind])
        .map_err(|_| ApiError::new(StatusCode::CONFLICT, "conflict", "account already exists"))?;
    audit(&conn, &actor, "account.add", &b.number.to_string(), &name)?;
    Ok(Json(json!({"account": {"number": b.number, "name": name, "kind": kind}})))
}

#[derive(Deserialize)]
pub struct CompanyIn {
    name: String,
    org_number: Option<String>,
}

pub async fn get_company(State(st): State<Shared>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    let c: Option<(String, String)> = conn
        .query_row("SELECT name, org_number FROM company WHERE id = 1", [], |r| Ok((r.get(0)?, r.get(1)?)))
        .ok();
    Ok(Json(json!({"company": c.map(|(n, o)| json!({"name": n, "org_number": o}))})))
}

pub async fn set_company(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Json(b): Json<CompanyIn>,
) -> ApiResult {
    require_admin(actor.role)?;
    let name = clean_text(&b.name, "name", 100)?;
    let org = clean_text(b.org_number.as_deref().unwrap_or(""), "org_number", 20)?;
    if name.is_empty() {
        return Err(ApiError::invalid("company name is empty"));
    }
    let conn = st.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO company(id, name, org_number) VALUES (1, ?1, ?2)
         ON CONFLICT(id) DO UPDATE SET name = excluded.name, org_number = excluded.org_number",
        params![name, org],
    )?;
    audit(&conn, &actor, "company.set", "1", &name)?;
    Ok(Json(json!({"company": {"name": name, "org_number": org}})))
}

#[derive(Deserialize)]
pub(crate) struct NewDraft {
    pub(crate) event_date: String,
    pub(crate) description: String,
    pub(crate) counterparty: Option<String>,
    pub(crate) lines: Vec<LineIn>,
    #[serde(default)]
    pub(crate) documents: Vec<i64>,
    #[serde(default)]
    pub(crate) bank_transaction_id: Option<i64>,
}

/// Validate and store a draft voucher, with optional receipts and the bank transaction it books.
pub(crate) fn create_draft(conn: &mut Connection, actor: &Actor, b: &NewDraft) -> Result<i64, ApiError> {
    check_date(&b.event_date)?;
    let description = clean_text(&b.description, "description", 200)?;
    if description.is_empty() {
        return Err(ApiError::invalid("description is empty"));
    }
    let counterparty = clean_text(b.counterparty.as_deref().unwrap_or(""), "counterparty", 100)?;
    let lines = validate_lines(conn, &b.lines)?;
    for d in &b.documents {
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM document WHERE id = ?1", params![d], |r| r.get(0))?;
        if n == 0 {
            return Err(ApiError::invalid(format!("unknown document {d}")));
        }
    }
    if let Some(txid) = b.bank_transaction_id {
        let status: Option<String> = conn
            .query_row("SELECT status FROM bank_transaction WHERE id = ?1", params![txid], |r| r.get(0))
            .ok();
        match status.as_deref() {
            None => return Err(ApiError::invalid(format!("unknown bank transaction {txid}"))),
            Some("new") => {}
            Some(other) => {
                return Err(ApiError::new(
                    StatusCode::CONFLICT,
                    "conflict",
                    format!("bank transaction {txid} is already {other}"),
                ));
            }
        }
    }
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO draft(event_date, description, counterparty, created_by, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![b.event_date, description, counterparty, actor.name.as_str(), oas_core::utc_string(Utc::now())],
    )?;
    let id = tx.last_insert_rowid();
    for (a, d, c) in &lines {
        tx.execute(
            "INSERT INTO draft_line(draft_id, account, debit, credit) VALUES (?1, ?2, ?3, ?4)",
            params![id, a, d, c],
        )?;
    }
    for d in &b.documents {
        tx.execute("INSERT OR IGNORE INTO draft_document(draft_id, document_id) VALUES (?1, ?2)", params![id, d])?;
    }
    if let Some(txid) = b.bank_transaction_id {
        tx.execute("INSERT INTO draft_bank(draft_id, bank_transaction_id) VALUES (?1, ?2)", params![id, txid])
            .map_err(|_| ApiError::new(StatusCode::CONFLICT, "conflict", "that bank transaction already has a draft"))?;
    }
    audit(&tx, actor, "draft.propose", &id.to_string(), &description)?;
    tx.commit()?;
    Ok(id)
}

pub async fn propose(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Json(b): Json<NewDraft>,
) -> ApiResult {
    let mut conn = st.conn.lock().unwrap();
    let id = create_draft(&mut conn, &actor, &b)?;
    Ok(Json(json!({"draft": draft_view(&conn, id)?})))
}

pub async fn list_drafts(State(st): State<Shared>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM draft ORDER BY id")?
        .query_map([], |r| r.get(0))?
        .filter_map(Result::ok)
        .collect();
    let drafts: Result<Vec<Value>, ApiError> = ids.into_iter().map(|id| draft_view(&conn, id)).collect();
    Ok(Json(json!({"drafts": drafts?})))
}

pub async fn approve(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<i64>,
) -> ApiResult {
    require_admin(actor.role)?;
    let mut conn = st.conn.lock().unwrap();
    let tx = conn.transaction()?;
    let (event_date, description, counterparty, created_by, created_at): (String, String, String, String, String) =
        tx.query_row(
            "SELECT event_date, description, counterparty, created_by, created_at FROM draft WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .map_err(|_| ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such draft"))?;
    let lines: Vec<(i64, i64, i64)> = tx
        .prepare("SELECT account, debit, credit FROM draft_line WHERE draft_id = ?1 ORDER BY id")?
        .query_map(params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .filter_map(Result::ok)
        .collect();
    let dsum: i64 = lines.iter().map(|l| l.1).sum();
    let csum: i64 = lines.iter().map(|l| l.2).sum();
    if dsum != csum || lines.len() < 2 {
        return Err(ApiError::invalid("draft does not balance"));
    }
    let vid = post_voucher(&tx, &event_date, &description, &counterparty, &created_by, actor.name.as_str(), &created_at, None, &lines)?;
    tx.execute(
        "INSERT INTO voucher_document(voucher_id, document_id)
         SELECT ?1, document_id FROM draft_document WHERE draft_id = ?2",
        params![vid, id],
    )?;
    let bank_tx: Option<i64> = tx
        .query_row("SELECT bank_transaction_id FROM draft_bank WHERE draft_id = ?1", params![id], |r| r.get(0))
        .ok();
    if let Some(txid) = bank_tx {
        tx.execute("INSERT INTO bank_booking(bank_transaction_id, voucher_id) VALUES (?1, ?2)", params![txid, vid])?;
        tx.execute("UPDATE bank_transaction SET status = 'booked' WHERE id = ?1", params![txid])?;
    }
    tx.execute("DELETE FROM draft WHERE id = ?1", params![id])?;
    audit(&tx, &actor, "voucher.approve", &vid.to_string(), &format!("draft {id}"))?;
    tx.commit()?;
    Ok(Json(json!({"voucher": voucher_view(&conn, vid)?})))
}

pub async fn reject(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<i64>,
) -> ApiResult {
    require_admin(actor.role)?;
    let conn = st.conn.lock().unwrap();
    let n = conn.execute("DELETE FROM draft WHERE id = ?1", params![id])?;
    if n == 0 {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such draft"));
    }
    audit(&conn, &actor, "draft.reject", &id.to_string(), "")?;
    Ok(Json(json!({"status": "ok"})))
}

#[derive(Deserialize, Default)]
pub struct Range {
    from: Option<String>,
    to: Option<String>,
    year: Option<i32>,
}

pub async fn list_vouchers(State(st): State<Shared>, Query(q): Query<Range>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    let ids: Vec<i64> = conn
        .prepare(
            "SELECT id FROM voucher
             WHERE (?1 IS NULL OR event_date >= ?1) AND (?2 IS NULL OR event_date <= ?2)
             ORDER BY series, number",
        )?
        .query_map(params![q.from, q.to], |r| r.get(0))?
        .filter_map(Result::ok)
        .collect();
    let vouchers: Result<Vec<Value>, ApiError> = ids.into_iter().map(|id| voucher_view(&conn, id)).collect();
    Ok(Json(json!({"vouchers": vouchers?})))
}

pub async fn get_voucher(State(st): State<Shared>, Path(id): Path<i64>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    Ok(Json(json!({"voucher": voucher_view(&conn, id)?})))
}

pub async fn correct(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<i64>,
) -> ApiResult {
    require_admin(actor.role)?;
    let mut conn = st.conn.lock().unwrap();
    let tx = conn.transaction()?;
    let (series, number, event_date, description, counterparty): (String, i64, String, String, String) = tx
        .query_row(
            "SELECT series, number, event_date, description, counterparty FROM voucher WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .map_err(|_| ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such voucher"))?;
    let already: i64 = tx.query_row(
        "SELECT COUNT(*) FROM voucher WHERE corrects_voucher_id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    if already > 0 {
        return Err(ApiError::new(StatusCode::CONFLICT, "conflict", "voucher is already corrected"));
    }
    let reversed: Vec<(i64, i64, i64)> =
        voucher_lines(&tx, id)?.into_iter().map(|(a, d, c)| (a, c, d)).collect();
    let text = format!("Rattelse av {series}{number}: {description}");
    let vid = post_voucher(
        &tx, &event_date, &text, &counterparty, actor.name.as_str(), actor.name.as_str(),
        &oas_core::utc_string(Utc::now()), Some(id), &reversed,
    )?;
    audit(&tx, &actor, "voucher.correct", &vid.to_string(), &format!("reverses {series}{number}"))?;
    tx.commit()?;
    Ok(Json(json!({"voucher": voucher_view(&conn, vid)?})))
}

pub async fn trial_balance(State(st): State<Shared>, Query(q): Query<Range>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    let (from, to) = year_bounds(&q);
    let mut stmt = conn.prepare(
        "SELECT e.account, a.name, SUM(e.debit), SUM(e.credit)
         FROM entry e JOIN voucher v ON v.id = e.voucher_id JOIN account a ON a.number = e.account
         WHERE (?1 IS NULL OR v.event_date >= ?1) AND (?2 IS NULL OR v.event_date <= ?2)
         GROUP BY e.account ORDER BY e.account",
    )?;
    let rows: Vec<(i64, String, i64, i64)> = stmt
        .query_map(params![from, to], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .filter_map(Result::ok)
        .collect();
    let (td, tc) = rows.iter().fold((0, 0), |(d, c), r| (d + r.2, c + r.3));
    let lines: Vec<Value> = rows
        .iter()
        .map(|(a, n, d, c)| {
            json!({"account": a, "name": n, "debit": format_amount(*d), "credit": format_amount(*c),
                   "balance": format_amount(d - c)})
        })
        .collect();
    Ok(Json(json!({"from": from, "to": to, "accounts": lines,
                   "total_debit": format_amount(td), "total_credit": format_amount(tc),
                   "balanced": td == tc})))
}

fn year_bounds(q: &Range) -> (Option<String>, Option<String>) {
    if let Some(y) = q.year {
        return (Some(format!("{y}-01-01")), Some(format!("{y}-12-31")));
    }
    (q.from.clone(), q.to.clone())
}

pub async fn verify(State(st): State<Shared>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    let mut issues: Vec<String> = vec![];
    let series: Vec<String> = conn
        .prepare("SELECT DISTINCT series FROM voucher ORDER BY series")?
        .query_map([], |r| r.get(0))?
        .filter_map(Result::ok)
        .collect();
    let mut total = 0;
    for s in &series {
        let rows: Vec<(i64, i64, String, String, String, String, String, Option<i64>, String, String)> = conn
            .prepare(
                "SELECT id, number, compile_date, event_date, description, counterparty, prev_hash,
                        corrects_voucher_id, hash, series FROM voucher WHERE series = ?1 ORDER BY number",
            )?
            .query_map(params![s], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?,
                    r.get(7)?, r.get(8)?, r.get(9)?))
            })?
            .filter_map(Result::ok)
            .collect();
        let mut prev = "genesis".to_string();
        for (i, (id, number, compile, event, desc, cp, prev_hash, corrects, hash, ser)) in rows.iter().enumerate() {
            total += 1;
            if *number != i as i64 + 1 {
                issues.push(format!("{s}: gap or disorder at number {number} (expected {})", i + 1));
            }
            if *prev_hash != prev {
                issues.push(format!("{s}{number}: prev_hash does not match previous voucher"));
            }
            let lines = voucher_lines(&conn, *id)?;
            let (d, c) = lines.iter().fold((0, 0), |(d, c), l| (d + l.1, c + l.2));
            if d != c {
                issues.push(format!("{s}{number}: entries do not balance"));
            }
            let expect = canonical(prev_hash, ser, *number, compile, event, desc, cp, *corrects, &lines);
            if expect != *hash {
                issues.push(format!("{s}{number}: hash mismatch, the voucher was altered"));
            }
            prev = hash.clone();
        }
    }
    let triggers: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name IN
         ('voucher_no_update','voucher_no_delete','entry_no_update','entry_no_delete')",
        [],
        |r| r.get(0),
    )?;
    if triggers != 4 {
        issues.push("append-only triggers are missing".into());
    }
    Ok(Json(json!({"ok": issues.is_empty(), "vouchers": total, "issues": issues})))
}

pub async fn audit_list(State(st): State<Shared>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    let rows: Vec<Value> = conn
        .prepare("SELECT at, actor, action, object, detail FROM audit_log ORDER BY id DESC LIMIT 200")?
        .query_map([], |r| {
            Ok(json!({"at": r.get::<_, String>(0)?, "actor": r.get::<_, String>(1)?,
                      "action": r.get::<_, String>(2)?, "object": r.get::<_, String>(3)?,
                      "detail": r.get::<_, String>(4)?}))
        })?
        .filter_map(Result::ok)
        .collect();
    Ok(Json(json!({"audit": rows})))
}

/// SIE 4 export for one calendar year. The record layout follows the commonly documented
/// SIE 4 format; it has not been checked against the official 4C specification.
pub async fn export_sie(State(st): State<Shared>, Query(q): Query<Range>) -> Result<Response, ApiError> {
    let conn = st.conn.lock().unwrap();
    let year = q.year.unwrap_or_else(|| Utc::now().year());
    let (from, to) = (format!("{year}-01-01"), format!("{year}-12-31"));
    let compact = |d: &str| d.replace('-', "");
    let (name, org): (String, String) = conn
        .query_row("SELECT name, org_number FROM company WHERE id = 1", [], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap_or_else(|_| ("Unnamed company".into(), String::new()));
    let mut out = String::new();
    out.push_str("#FLAGGA 0\r\n");
    out.push_str("#PROGRAM \"open-accounting-sweden\" 0.1\r\n");
    out.push_str("#FORMAT PC8\r\n");
    out.push_str(&format!("#GEN {}\r\n", Utc::now().format("%Y%m%d")));
    out.push_str("#SIETYP 4\r\n");
    out.push_str(&format!("#FNAMN {}\r\n", sie_quote(&name)));
    if !org.is_empty() {
        out.push_str(&format!("#ORGNR {org}\r\n"));
    }
    out.push_str(&format!("#RAR 0 {} {}\r\n", compact(&from), compact(&to)));
    let accounts: Vec<(i64, String, String)> = conn
        .prepare("SELECT number, name, kind FROM account ORDER BY number")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .filter_map(Result::ok)
        .collect();
    for (n, name, kind) in &accounts {
        out.push_str(&format!("#KONTO {n} {}\r\n", sie_quote(name)));
        out.push_str(&format!("#KTYP {n} {kind}\r\n"));
    }
    let sums: Vec<(i64, i64)> = conn
        .prepare(
            "SELECT e.account, SUM(e.debit) - SUM(e.credit) FROM entry e JOIN voucher v ON v.id = e.voucher_id
             WHERE v.event_date >= ?1 AND v.event_date <= ?2 GROUP BY e.account ORDER BY e.account",
        )?
        .query_map(params![from, to], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(Result::ok)
        .collect();
    for (a, bal) in &sums {
        let kind = accounts.iter().find(|x| x.0 == *a).map(|x| x.2.as_str()).unwrap_or("T");
        let tag = if kind == "T" || kind == "S" { "#UB" } else { "#RES" };
        out.push_str(&format!("{tag} 0 {a} {}\r\n", format_amount(*bal)));
    }
    let vouchers: Vec<(i64, String, i64, String, String, String, String)> = conn
        .prepare(
            "SELECT id, series, number, event_date, description, compile_date, approved_by FROM voucher
             WHERE event_date >= ?1 AND event_date <= ?2 ORDER BY series, number",
        )?
        .query_map(params![from, to], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))
        })?
        .filter_map(Result::ok)
        .collect();
    for (id, series, number, event, desc, compile, approver) in &vouchers {
        out.push_str(&format!(
            "#VER {} {} {} {} {} {}\r\n{{\r\n",
            sie_quote(series), sie_quote(&number.to_string()), compact(event), sie_quote(desc),
            compact(compile), sie_quote(approver)
        ));
        for (a, d, c) in voucher_lines(&conn, *id)? {
            out.push_str(&format!("#TRANS {a} {{}} {}\r\n", format_amount(d - c)));
        }
        out.push_str("}\r\n");
    }
    Ok((
        [
            (header::CONTENT_TYPE, "text/plain; charset=ibm437".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"oas-{year}.se\"")),
        ],
        to_cp437(&out),
    )
        .into_response())
}
