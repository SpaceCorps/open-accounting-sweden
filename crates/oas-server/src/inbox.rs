//! Receipts archive, bank transaction import, receipt matching and booking of bank lines.
//!
//! Receipts are stored content-addressed on disk and never changed or deleted. Matching only
//! suggests; a person confirms. Booking a bank line creates a draft voucher that still needs an
//! admin to approve it.

use std::path::PathBuf;

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use chrono::{NaiveDate, Utc};
use oas_core::{format_amount, parse_amount, parse_bank_csv, score_match, utc_string};
use rusqlite::{Connection, params};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::ledger::{self, LineIn, NewDraft};
use crate::{Actor, ApiError, ApiResult, Role, Shared};

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS document(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  sha256 TEXT NOT NULL UNIQUE,
  filename TEXT NOT NULL,
  mime TEXT NOT NULL,
  size INTEGER NOT NULL,
  uploaded_by TEXT NOT NULL,
  uploaded_at TEXT NOT NULL,
  source TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS document_meta(
  document_id INTEGER PRIMARY KEY REFERENCES document(id),
  vendor TEXT NOT NULL DEFAULT '',
  amount INTEGER,
  currency TEXT NOT NULL DEFAULT 'SEK',
  doc_date TEXT,
  notes TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS bank_transaction(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  import_hash TEXT NOT NULL UNIQUE,
  booking_date TEXT NOT NULL,
  amount INTEGER NOT NULL,
  currency TEXT NOT NULL DEFAULT 'SEK',
  counterparty TEXT NOT NULL DEFAULT '',
  reference TEXT NOT NULL DEFAULT '',
  imported_at TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'new' CHECK (status IN ('new', 'booked', 'ignored'))
);
CREATE TABLE IF NOT EXISTS match_suggestion(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  bank_transaction_id INTEGER NOT NULL REFERENCES bank_transaction(id),
  document_id INTEGER NOT NULL REFERENCES document(id),
  score REAL NOT NULL,
  name_score REAL NOT NULL,
  amount_score REAL NOT NULL,
  date_score REAL NOT NULL,
  currency_score REAL NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'confirmed', 'declined')),
  created_at TEXT NOT NULL,
  decided_by TEXT,
  decided_at TEXT,
  UNIQUE (bank_transaction_id, document_id)
);
CREATE TABLE IF NOT EXISTS draft_document(
  draft_id INTEGER NOT NULL REFERENCES draft(id) ON DELETE CASCADE,
  document_id INTEGER NOT NULL REFERENCES document(id),
  PRIMARY KEY (draft_id, document_id)
);
CREATE TABLE IF NOT EXISTS draft_bank(
  draft_id INTEGER PRIMARY KEY REFERENCES draft(id) ON DELETE CASCADE,
  bank_transaction_id INTEGER NOT NULL UNIQUE REFERENCES bank_transaction(id)
);
CREATE TABLE IF NOT EXISTS voucher_document(
  voucher_id INTEGER NOT NULL REFERENCES voucher(id),
  document_id INTEGER NOT NULL REFERENCES document(id),
  PRIMARY KEY (voucher_id, document_id)
);
CREATE TABLE IF NOT EXISTS bank_booking(
  bank_transaction_id INTEGER PRIMARY KEY REFERENCES bank_transaction(id),
  voucher_id INTEGER NOT NULL REFERENCES voucher(id)
);
CREATE TRIGGER IF NOT EXISTS document_no_update BEFORE UPDATE ON document
  BEGIN SELECT RAISE(ABORT, 'receipts are immutable'); END;
CREATE TRIGGER IF NOT EXISTS document_no_delete BEFORE DELETE ON document
  BEGIN SELECT RAISE(ABORT, 'receipts are immutable'); END;
CREATE TRIGGER IF NOT EXISTS voucher_document_no_update BEFORE UPDATE ON voucher_document
  BEGIN SELECT RAISE(ABORT, 'ledger is append-only'); END;
CREATE TRIGGER IF NOT EXISTS voucher_document_no_delete BEFORE DELETE ON voucher_document
  BEGIN SELECT RAISE(ABORT, 'ledger is append-only'); END;
CREATE TRIGGER IF NOT EXISTS bank_booking_no_update BEFORE UPDATE ON bank_booking
  BEGIN SELECT RAISE(ABORT, 'ledger is append-only'); END;
CREATE TRIGGER IF NOT EXISTS bank_booking_no_delete BEFORE DELETE ON bank_booking
  BEGIN SELECT RAISE(ABORT, 'ledger is append-only'); END;
";

const SUGGEST_AT: f64 = 0.6;
const MAX_ROWS: usize = 5000;

fn require_admin(actor: &Actor) -> Result<(), ApiError> {
    if actor.role == Role::Admin {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "this token can propose and read but cannot confirm, decline or ignore",
        ))
    }
}

fn clean(s: &str, field: &str, max: usize) -> Result<String, ApiError> {
    let v = s.trim();
    if v.chars().count() > max || v.chars().any(|c| c.is_control()) {
        return Err(ApiError::invalid(format!("{field} is too long or has control characters")));
    }
    Ok(v.to_string())
}

fn safe_filename(raw: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = base
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, '.' | '_' | '-' | ' ') { c } else { '_' })
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.').to_string();
    let short: String = trimmed.chars().take(120).collect();
    if short.is_empty() { "upload".into() } else { short }
}

fn safe_mime(raw: &str) -> String {
    let m = raw.split(';').next().unwrap_or("").trim().to_lowercase();
    if !m.is_empty() && m.len() <= 100 && m.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '+' | '.' | '-')) {
        m
    } else {
        "application/octet-stream".into()
    }
}

fn doc_path(dir: &std::path::Path, sha: &str) -> PathBuf {
    dir.join(&sha[..2]).join(sha)
}

fn opt_amount(v: &Option<String>) -> Result<Option<i64>, ApiError> {
    match v.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => parse_amount(s).map(Some).map_err(ApiError::invalid),
        None => Ok(None),
    }
}

fn opt_date(v: &Option<String>) -> Result<Option<String>, ApiError> {
    match v.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map(|_| Some(s.to_string()))
            .map_err(|_| ApiError::invalid(format!("invalid date '{s}', expected YYYY-MM-DD"))),
        None => Ok(None),
    }
}

fn currency(v: &Option<String>) -> Result<String, ApiError> {
    let c = v.as_deref().unwrap_or("SEK").trim().to_uppercase();
    if c.len() == 3 && c.chars().all(|ch| ch.is_ascii_alphabetic()) {
        Ok(c)
    } else {
        Err(ApiError::invalid("currency must be a three-letter code such as SEK"))
    }
}

type DocRow = (String, String, i64, String, String, String, String, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>);

fn doc_json(conn: &Connection, id: i64) -> Result<Value, ApiError> {
    let row: DocRow = conn
        .query_row(
            "SELECT d.filename, d.mime, d.size, d.uploaded_by, d.uploaded_at, d.source, d.sha256,
                    m.vendor, m.amount, m.currency, m.doc_date, m.notes
             FROM document d LEFT JOIN document_meta m ON m.document_id = d.id WHERE d.id = ?1",
            params![id],
            |r| {
                Ok((
                    r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?,
                    r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?, r.get(11)?,
                ))
            },
        )
        .map_err(|_| ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such document"))?;
    let vouchers: Vec<String> = conn
        .prepare("SELECT v.series || v.number FROM voucher_document vd JOIN voucher v ON v.id = vd.voucher_id WHERE vd.document_id = ?1")?
        .query_map(params![id], |r| r.get(0))?
        .filter_map(Result::ok)
        .collect();
    let drafts: Vec<i64> = conn
        .prepare("SELECT draft_id FROM draft_document WHERE document_id = ?1")?
        .query_map(params![id], |r| r.get(0))?
        .filter_map(Result::ok)
        .collect();
    let status = if !vouchers.is_empty() {
        "booked"
    } else if !drafts.is_empty() {
        "draft"
    } else {
        "unlinked"
    };
    Ok(json!({
        "id": id, "filename": row.0, "mime": row.1, "size": row.2, "uploaded_by": row.3,
        "uploaded_at": row.4, "source": row.5, "sha256": row.6,
        "vendor": row.7.unwrap_or_default(), "amount": row.8.map(format_amount),
        "currency": row.9.unwrap_or_else(|| "SEK".into()), "date": row.10, "notes": row.11.unwrap_or_default(),
        "vouchers": vouchers, "drafts": drafts, "status": status,
    }))
}

#[derive(Deserialize)]
pub struct UploadQuery {
    filename: Option<String>,
    vendor: Option<String>,
    amount: Option<String>,
    currency: Option<String>,
    date: Option<String>,
    notes: Option<String>,
    source: Option<String>,
}

pub async fn upload_document(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Query(q): Query<UploadQuery>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> ApiResult {
    if body.is_empty() {
        return Err(ApiError::invalid("the file is empty"));
    }
    let filename = safe_filename(q.filename.as_deref().unwrap_or("upload"));
    let mime = safe_mime(headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or(""));
    let vendor = clean(q.vendor.as_deref().unwrap_or(""), "vendor", 100)?;
    let notes = clean(q.notes.as_deref().unwrap_or(""), "notes", 500)?;
    let amount = opt_amount(&q.amount)?;
    let cur = currency(&q.currency)?;
    let date = opt_date(&q.date)?;
    let source = match q.source.as_deref() {
        Some(s @ ("web" | "cli" | "agent")) => s,
        _ => "api",
    };
    let sha = format!("{:x}", Sha256::digest(&body));
    let conn = st.conn.lock().unwrap();
    let existing: Option<i64> =
        conn.query_row("SELECT id FROM document WHERE sha256 = ?1", params![sha], |r| r.get(0)).ok();
    if let Some(id) = existing {
        let mut v = doc_json(&conn, id)?;
        v["duplicate"] = json!(true);
        return Ok(Json(json!({"document": v})));
    }
    let path = doc_path(&st.docs_dir, &sha);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| {
            eprintln!("cannot create document dir: {e}");
            ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", "could not store the file")
        })?;
    }
    if !path.exists() {
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, &body)
            .and_then(|_| std::fs::rename(&tmp, &path))
            .map_err(|e| {
                eprintln!("cannot write document: {e}");
                ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", "could not store the file")
            })?;
    }
    conn.execute(
        "INSERT INTO document(sha256, filename, mime, size, uploaded_by, uploaded_at, source)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![sha, filename, mime, body.len() as i64, actor.name, utc_string(Utc::now()), source],
    )?;
    let id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO document_meta(document_id, vendor, amount, currency, doc_date, notes) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, vendor, amount, cur, date, notes],
    )?;
    ledger::audit(&conn, &actor, "document.upload", &id.to_string(), &filename)?;
    run_matching(&conn)?;
    let mut v = doc_json(&conn, id)?;
    v["duplicate"] = json!(false);
    Ok(Json(json!({"document": v})))
}

#[derive(Deserialize, Default)]
pub struct DocFilter {
    status: Option<String>,
}

pub async fn list_documents(State(st): State<Shared>, Query(f): Query<DocFilter>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM document ORDER BY id DESC")?
        .query_map([], |r| r.get(0))?
        .filter_map(Result::ok)
        .collect();
    let mut docs = vec![];
    for id in ids {
        let d = doc_json(&conn, id)?;
        if f.status.as_deref().is_none_or(|s| d["status"] == s) {
            docs.push(d);
        }
    }
    Ok(Json(json!({"documents": docs})))
}

pub async fn download_document(State(st): State<Shared>, Path(id): Path<i64>) -> Result<Response, ApiError> {
    let (sha, filename, mime): (String, String, String) = {
        let conn = st.conn.lock().unwrap();
        conn.query_row("SELECT sha256, filename, mime FROM document WHERE id = ?1", params![id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .map_err(|_| ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such document"))?
    };
    let bytes = std::fs::read(doc_path(&st.docs_dir, &sha))
        .map_err(|_| ApiError::new(StatusCode::NOT_FOUND, "not_found", "the stored file is missing"))?;
    // Only types a browser cannot run are labelled as themselves; everything else downloads as binary.
    let safe = ["application/pdf", "image/png", "image/jpeg", "image/webp", "image/gif"];
    let ct = if safe.contains(&mime.as_str()) { mime } else { "application/octet-stream".to_string() };
    let mut res = bytes.into_response();
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&ct).unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    h.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename.replace('"', "_")))
            .unwrap_or(HeaderValue::from_static("attachment")),
    );
    Ok(res)
}

#[derive(Deserialize)]
pub struct MetaIn {
    vendor: Option<String>,
    amount: Option<String>,
    currency: Option<String>,
    date: Option<String>,
    notes: Option<String>,
}

pub async fn set_meta(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<i64>,
    Json(b): Json<MetaIn>,
) -> ApiResult {
    let vendor = clean(b.vendor.as_deref().unwrap_or(""), "vendor", 100)?;
    let notes = clean(b.notes.as_deref().unwrap_or(""), "notes", 500)?;
    let amount = opt_amount(&b.amount)?;
    let cur = currency(&b.currency)?;
    let date = opt_date(&b.date)?;
    let conn = st.conn.lock().unwrap();
    let exists: i64 = conn.query_row("SELECT COUNT(*) FROM document WHERE id = ?1", params![id], |r| r.get(0))?;
    if exists == 0 {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such document"));
    }
    conn.execute(
        "INSERT INTO document_meta(document_id, vendor, amount, currency, doc_date, notes) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(document_id) DO UPDATE SET vendor = excluded.vendor, amount = excluded.amount,
           currency = excluded.currency, doc_date = excluded.doc_date, notes = excluded.notes",
        params![id, vendor, amount, cur, date, notes],
    )?;
    ledger::audit(&conn, &actor, "document.meta", &id.to_string(), &vendor)?;
    run_matching(&conn)?;
    Ok(Json(json!({"document": doc_json(&conn, id)?})))
}

type TxRow = (String, i64, String, String, String, String, String);

fn tx_json(conn: &Connection, id: i64) -> Result<Value, ApiError> {
    let row: TxRow = conn
        .query_row(
            "SELECT booking_date, amount, currency, counterparty, reference, status, imported_at
             FROM bank_transaction WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
        )
        .map_err(|_| ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such bank transaction"))?;
    let voucher: Option<String> = conn
        .query_row(
            "SELECT v.series || v.number FROM bank_booking b JOIN voucher v ON v.id = b.voucher_id WHERE b.bank_transaction_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .ok();
    let draft: Option<i64> = conn
        .query_row("SELECT draft_id FROM draft_bank WHERE bank_transaction_id = ?1", params![id], |r| r.get(0))
        .ok();
    let pending: i64 = conn.query_row(
        "SELECT COUNT(*) FROM match_suggestion WHERE bank_transaction_id = ?1 AND status = 'pending'",
        params![id],
        |r| r.get(0),
    )?;
    Ok(json!({"id": id, "date": row.0, "amount": format_amount(row.1), "currency": row.2,
              "counterparty": row.3, "reference": row.4, "status": row.5, "imported_at": row.6,
              "voucher": voucher, "draft": draft, "pending_matches": pending}))
}

pub async fn import_bank(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    body: Bytes,
) -> ApiResult {
    if body.len() > 5 * 1024 * 1024 {
        return Err(ApiError::invalid("the file is larger than 5 MB"));
    }
    let text = String::from_utf8_lossy(&body).into_owned();
    let (rows, errors) = parse_bank_csv(&text).map_err(ApiError::invalid)?;
    if rows.len() > MAX_ROWS {
        return Err(ApiError::invalid(format!("more than {MAX_ROWS} rows; split the file")));
    }
    let mut conn = st.conn.lock().unwrap();
    let tx = conn.transaction()?;
    let (mut imported, mut duplicates) = (0, 0);
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for r in &rows {
        let base = format!("{}|{}|{}|{}|{}", r.date, r.amount, r.currency, r.counterparty, r.reference);
        let n = seen.entry(base.clone()).or_insert(0);
        *n += 1;
        let hash = format!("{:x}", Sha256::digest(format!("{base}|{n}").as_bytes()));
        let changed = tx.execute(
            "INSERT OR IGNORE INTO bank_transaction(import_hash, booking_date, amount, currency, counterparty, reference, imported_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                hash,
                r.date,
                r.amount,
                r.currency,
                r.counterparty.chars().take(200).collect::<String>(),
                r.reference.chars().take(200).collect::<String>(),
                utc_string(Utc::now())
            ],
        )?;
        if changed == 1 {
            imported += 1;
        } else {
            duplicates += 1;
        }
    }
    ledger::audit(&tx, &actor, "bank.import", "csv", &format!("{imported} new, {duplicates} duplicates"))?;
    tx.commit()?;
    let suggested = run_matching(&conn)?;
    Ok(Json(json!({"imported": imported, "duplicates": duplicates, "errors": errors, "new_suggestions": suggested})))
}

#[derive(Deserialize, Default)]
pub struct TxFilter {
    status: Option<String>,
}

pub async fn list_transactions(State(st): State<Shared>, Query(f): Query<TxFilter>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM bank_transaction ORDER BY booking_date DESC, id DESC")?
        .query_map([], |r| r.get(0))?
        .filter_map(Result::ok)
        .collect();
    let mut out = vec![];
    for id in ids {
        let t = tx_json(&conn, id)?;
        if f.status.as_deref().is_none_or(|s| t["status"] == s) {
            out.push(t);
        }
    }
    Ok(Json(json!({"transactions": out})))
}

pub async fn ignore_tx(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<i64>,
) -> ApiResult {
    require_admin(&actor)?;
    let conn = st.conn.lock().unwrap();
    let n = conn.execute("UPDATE bank_transaction SET status = 'ignored' WHERE id = ?1 AND status = 'new'", params![id])?;
    if n == 0 {
        return Err(ApiError::new(StatusCode::CONFLICT, "conflict", "only a new, unbooked transaction can be ignored"));
    }
    ledger::audit(&conn, &actor, "bank.ignore", &id.to_string(), "")?;
    Ok(Json(json!({"transaction": tx_json(&conn, id)?})))
}

type TxMatchRow = (i64, String, i64, String, String, String);
type DocMatchRow = (i64, String, Option<i64>, String, Option<String>);

/// Score every unbooked bank transaction against every receipt that is not yet linked, and store
/// new suggestions. Declined pairs stay declined. Returns how many suggestions were added.
pub(crate) fn run_matching(conn: &Connection) -> Result<usize, ApiError> {
    let txs: Vec<TxMatchRow> = conn
        .prepare("SELECT id, booking_date, amount, currency, counterparty, reference FROM bank_transaction WHERE status = 'new'")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?
        .filter_map(Result::ok)
        .collect();
    let docs: Vec<DocMatchRow> = conn
        .prepare(
            "SELECT d.id, m.vendor, m.amount, m.currency, m.doc_date FROM document d
             JOIN document_meta m ON m.document_id = d.id
             WHERE NOT EXISTS (SELECT 1 FROM voucher_document v WHERE v.document_id = d.id)
               AND NOT EXISTS (SELECT 1 FROM draft_document x WHERE x.document_id = d.id)",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .filter_map(Result::ok)
        .collect();
    let mut added = 0;
    for (txid, date, amount, cur, cp, reference) in &txs {
        let text = format!("{cp} {reference}");
        for (docid, vendor, damount, dcur, ddate) in &docs {
            let Some(s) = score_match(*amount, cur, date, &text, *damount, dcur, ddate.as_deref(), vendor) else {
                continue;
            };
            if s.total < SUGGEST_AT {
                continue;
            }
            added += conn.execute(
                "INSERT OR IGNORE INTO match_suggestion(bank_transaction_id, document_id, score, name_score, amount_score, date_score, currency_score, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![txid, docid, s.total, s.name, s.amount, s.date, s.currency, utc_string(Utc::now())],
            )?;
        }
    }
    Ok(added)
}

pub async fn run_match(State(st): State<Shared>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    Ok(Json(json!({"new_suggestions": run_matching(&conn)?})))
}

#[derive(Deserialize, Default)]
pub struct MatchFilter {
    status: Option<String>,
}

type SuggestionRow = (i64, i64, i64, f64, f64, f64, f64, f64, String);

pub async fn list_matches(State(st): State<Shared>, Query(f): Query<MatchFilter>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    let status = f.status.unwrap_or_else(|| "pending".into());
    let rows: Vec<SuggestionRow> = conn
        .prepare(
            "SELECT id, bank_transaction_id, document_id, score, name_score, amount_score, date_score, currency_score, status
             FROM match_suggestion WHERE status = ?1 ORDER BY score DESC, id",
        )?
        .query_map(params![status], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?))
        })?
        .filter_map(Result::ok)
        .collect();
    let mut out = vec![];
    for (id, txid, docid, score, n, a, d, c, status) in rows {
        out.push(json!({
            "id": id, "status": status, "score": (score * 100.0).round() / 100.0,
            "scores": {"name": n, "amount": a, "date": d, "currency": c},
            "transaction": tx_json(&conn, txid)?, "document": doc_json(&conn, docid)?,
        }));
    }
    Ok(Json(json!({"matches": out})))
}

async fn decide(st: Shared, actor: Actor, id: i64, status: &str) -> ApiResult {
    require_admin(&actor)?;
    let conn = st.conn.lock().unwrap();
    let n = conn.execute(
        "UPDATE match_suggestion SET status = ?1, decided_by = ?2, decided_at = ?3 WHERE id = ?4 AND status = 'pending'",
        params![status, actor.name, utc_string(Utc::now()), id],
    )?;
    if n == 0 {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "not_found", "no pending suggestion with that id"));
    }
    ledger::audit(&conn, &actor, &format!("match.{status}"), &id.to_string(), "")?;
    Ok(Json(json!({"status": status})))
}

pub async fn confirm_match(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<i64>,
) -> ApiResult {
    decide(st, actor, id, "confirmed").await
}

pub async fn decline_match(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<i64>,
) -> ApiResult {
    decide(st, actor, id, "declined").await
}

#[derive(Deserialize)]
pub struct BookIn {
    description: Option<String>,
    counterparty: Option<String>,
    /// Lines against the other side of the bank line; the bank account line is added for you.
    lines: Vec<LineIn>,
    #[serde(default)]
    documents: Vec<i64>,
    bank_account: Option<i64>,
}

/// Turn a bank line into a draft voucher. The bank account line follows the sign of the bank
/// amount; the caller supplies the counter lines. Confirmed receipt matches are attached.
pub async fn book_tx(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<i64>,
    Json(b): Json<BookIn>,
) -> ApiResult {
    let mut conn = st.conn.lock().unwrap();
    let (date, amount, cp, reference, status): (String, i64, String, String, String) = conn
        .query_row(
            "SELECT booking_date, amount, counterparty, reference, status FROM bank_transaction WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .map_err(|_| ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such bank transaction"))?;
    if status != "new" {
        return Err(ApiError::new(StatusCode::CONFLICT, "conflict", format!("this transaction is already {status}")));
    }
    let bank_account = b.bank_account.unwrap_or(1930);
    let mut lines = b.lines;
    let bank_amount = format_amount(amount.abs());
    lines.push(if amount < 0 {
        LineIn { account: bank_account, debit: None, credit: Some(bank_amount), vat_code: None }
    } else {
        LineIn { account: bank_account, debit: Some(bank_amount), credit: None, vat_code: None }
    });
    let mut documents = b.documents;
    let confirmed: Vec<i64> = conn
        .prepare("SELECT document_id FROM match_suggestion WHERE bank_transaction_id = ?1 AND status = 'confirmed'")?
        .query_map(params![id], |r| r.get(0))?
        .filter_map(Result::ok)
        .collect();
    for d in confirmed {
        if !documents.contains(&d) {
            documents.push(d);
        }
    }
    let description = match b.description.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(d) => d.to_string(),
        None if !cp.is_empty() => cp.clone(),
        None => reference.clone(),
    };
    let nd = NewDraft {
        event_date: date,
        description,
        counterparty: b.counterparty.or(Some(cp)),
        lines,
        documents,
        bank_transaction_id: Some(id),
    };
    let draft_id = ledger::create_draft(&mut conn, &actor, &nd)?;
    Ok(Json(json!({"draft": ledger::draft_view(&conn, draft_id)?})))
}
