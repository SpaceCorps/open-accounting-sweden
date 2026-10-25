//! Quarterly VAT (moms) report prepared from the ledger.
//!
//! Every box of the return is derived from posted vouchers: base boxes from lines that carry a
//! VAT code, output and input VAT boxes from the accounts mapped to them. Every box and mapping
//! starts as UNVERIFIED. The report says so until the owner has checked each box against the
//! SKV 4700 form and marked it verified. Nothing is filed from here.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use chrono::{Datelike, Duration, NaiveDate, Utc, Weekday};
use oas_core::{format_amount, utc_string};
use rusqlite::{Connection, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{Actor, ApiError, ApiResult, Role, Shared};

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS vat_box(
  box TEXT PRIMARY KEY,
  label TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('base', 'output', 'input', 'result')),
  source TEXT NOT NULL,
  verified INTEGER NOT NULL DEFAULT 0,
  verified_by TEXT,
  verified_at TEXT
);
CREATE TABLE IF NOT EXISTS vat_code(
  code TEXT PRIMARY KEY,
  label TEXT NOT NULL,
  base_box TEXT REFERENCES vat_box(box),
  base_side TEXT CHECK (base_side IN ('credit', 'debit'))
);
CREATE TABLE IF NOT EXISTS vat_account(
  account INTEGER PRIMARY KEY REFERENCES account(number),
  box TEXT NOT NULL REFERENCES vat_box(box),
  side TEXT NOT NULL CHECK (side IN ('credit', 'debit'))
);
";

const RESEARCH: &str =
    "Skatteverket page as summarised by research on 2026-10-22; not yet checked by the owner";
const MEMORY: &str = "general knowledge of form SKV 4700; no source checked";

// (box, label, kind, source)
const BOXES: &[(&str, &str, &str, &str)] = &[
    ("05", "Momspliktig forsaljning (excl. VAT)", "base", MEMORY),
    ("10", "Utgaende moms 25 procent", "output", MEMORY),
    ("11", "Utgaende moms 12 procent", "output", MEMORY),
    ("12", "Utgaende moms 6 procent", "output", MEMORY),
    (
        "21",
        "Inkop tjanster fran annat EU-land (omvand skattskyldighet)",
        "base",
        RESEARCH,
    ),
    (
        "22",
        "Inkop tjanster fran land utanfor EU (omvand skattskyldighet)",
        "base",
        RESEARCH,
    ),
    (
        "30",
        "Utgaende moms 25 procent, omvand skattskyldighet",
        "output",
        RESEARCH,
    ),
    (
        "31",
        "Utgaende moms 12 procent, omvand skattskyldighet",
        "output",
        RESEARCH,
    ),
    (
        "32",
        "Utgaende moms 6 procent, omvand skattskyldighet",
        "output",
        RESEARCH,
    ),
    (
        "39",
        "Forsaljning tjanster till annat EU-land",
        "base",
        RESEARCH,
    ),
    ("48", "Ingaende moms att dra av", "input", RESEARCH),
    (
        "49",
        "Moms att betala (+) eller fa tillbaka (-)",
        "result",
        MEMORY,
    ),
];

// (code, label, base box, base side)
const CODES: &[(&str, &str, Option<&str>, Option<&str>)] = &[
    (
        "S25",
        "Sale in Sweden, 25 percent",
        Some("05"),
        Some("credit"),
    ),
    (
        "S12",
        "Sale in Sweden, 12 percent",
        Some("05"),
        Some("credit"),
    ),
    (
        "S6",
        "Sale in Sweden, 6 percent",
        Some("05"),
        Some("credit"),
    ),
    (
        "RCEU",
        "Purchase of services from another EU country (reverse charge)",
        Some("21"),
        Some("debit"),
    ),
    (
        "RCNONEU",
        "Purchase of services from outside the EU (reverse charge)",
        Some("22"),
        Some("debit"),
    ),
    (
        "SEUSVC",
        "Sale of services to another EU country",
        Some("39"),
        Some("credit"),
    ),
    ("I25", "Domestic purchase with input VAT", None, None),
    ("NONE", "No VAT", None, None),
];

// (account, box, side)
const ACCOUNTS: &[(i64, &str, &str)] = &[
    (2611, "10", "credit"),
    (2621, "11", "credit"),
    (2631, "12", "credit"),
    (2614, "30", "credit"),
    (2624, "31", "credit"),
    (2634, "32", "credit"),
    (2640, "48", "debit"),
    (2641, "48", "debit"),
    (2645, "48", "debit"),
];

pub fn seed(conn: &Connection) -> rusqlite::Result<()> {
    let has: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('vat_box') WHERE name = 'not_used'",
        [],
        |r| r.get(0),
    )?;
    if has == 0 {
        conn.execute_batch("ALTER TABLE vat_box ADD COLUMN not_used INTEGER NOT NULL DEFAULT 0")?;
    }
    for (b, label, kind, source) in BOXES {
        conn.execute(
            "INSERT OR IGNORE INTO vat_box(box, label, kind, source) VALUES (?1, ?2, ?3, ?4)",
            params![b, label, kind, source],
        )?;
    }
    for (code, label, bb, side) in CODES {
        conn.execute(
            "INSERT OR IGNORE INTO vat_code(code, label, base_box, base_side) VALUES (?1, ?2, ?3, ?4)",
            params![code, label, bb, side],
        )?;
    }
    for (acc, b, side) in ACCOUNTS {
        // The account may be missing from a custom chart; the mapping is then skipped.
        let _ = conn.execute(
            "INSERT OR IGNORE INTO vat_account(account, box, side) VALUES (?1, ?2, ?3)",
            params![acc, b, side],
        );
    }
    Ok(())
}

fn require_admin(actor: &Actor) -> Result<(), ApiError> {
    if actor.role == Role::Admin {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "only an admin can change or verify the VAT mapping",
        ))
    }
}

pub async fn config(State(st): State<Shared>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    let boxes: Vec<Value> = conn
        .prepare("SELECT box, label, kind, source, verified, verified_by, verified_at, not_used FROM vat_box ORDER BY box")?
        .query_map([], |r| {
            Ok(json!({"box": r.get::<_, String>(0)?, "label": r.get::<_, String>(1)?, "kind": r.get::<_, String>(2)?,
                      "source": r.get::<_, String>(3)?, "verified": r.get::<_, i64>(4)? != 0,
                      "verified_by": r.get::<_, Option<String>>(5)?, "verified_at": r.get::<_, Option<String>>(6)?,
                      "not_used": r.get::<_, i64>(7)? != 0}))
        })?
        .filter_map(Result::ok)
        .collect();
    let codes: Vec<Value> = conn
        .prepare("SELECT code, label, base_box, base_side FROM vat_code ORDER BY code")?
        .query_map([], |r| {
            Ok(json!({"code": r.get::<_, String>(0)?, "label": r.get::<_, String>(1)?,
                      "base_box": r.get::<_, Option<String>>(2)?, "base_side": r.get::<_, Option<String>>(3)?}))
        })?
        .filter_map(Result::ok)
        .collect();
    let accounts: Vec<Value> = conn
        .prepare("SELECT v.account, a.name, v.box, v.side FROM vat_account v JOIN account a ON a.number = v.account ORDER BY v.account")?
        .query_map([], |r| {
            Ok(json!({"account": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?,
                      "box": r.get::<_, String>(2)?, "side": r.get::<_, String>(3)?}))
        })?
        .filter_map(Result::ok)
        .collect();
    Ok(Json(
        json!({"boxes": boxes, "codes": codes, "accounts": accounts}),
    ))
}

#[derive(Deserialize)]
pub struct VerifyIn {
    verified: bool,
}

pub async fn verify_box(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Path(b): Path<String>,
    Json(body): Json<VerifyIn>,
) -> ApiResult {
    require_admin(&actor)?;
    let conn = st.conn.lock().unwrap();
    let n = if body.verified {
        conn.execute(
            "UPDATE vat_box SET verified = 1, verified_by = ?1, verified_at = ?2, not_used = 0 WHERE box = ?3",
            params![actor.name, utc_string(Utc::now()), b],
        )?
    } else {
        conn.execute("UPDATE vat_box SET verified = 0, verified_by = NULL, verified_at = NULL WHERE box = ?1", params![b])?
    };
    if n == 0 {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "no such VAT box",
        ));
    }
    crate::ledger::audit(
        &conn,
        &actor,
        if body.verified {
            "vat.verify"
        } else {
            "vat.unverify"
        },
        &b,
        "",
    )?;
    Ok(Json(json!({"box": b, "verified": body.verified})))
}

#[derive(Deserialize)]
pub struct UnusedIn {
    not_used: bool,
}

/// Mark a box as one this company never uses. It stops blocking readiness, but any amount that still
/// lands in it makes the report fail until the box is verified.
pub async fn set_not_used(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Path(b): Path<String>,
    Json(body): Json<UnusedIn>,
) -> ApiResult {
    require_admin(&actor)?;
    let conn = st.conn.lock().unwrap();
    let n = if body.not_used {
        conn.execute("UPDATE vat_box SET not_used = 1, verified = 0, verified_by = NULL, verified_at = NULL WHERE box = ?1 AND box != '49'", params![b])?
    } else {
        conn.execute("UPDATE vat_box SET not_used = 0 WHERE box = ?1", params![b])?
    };
    if n == 0 {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "no such VAT box, or box 49 which is always used",
        ));
    }
    crate::ledger::audit(
        &conn,
        &actor,
        if body.not_used {
            "vat.not_used"
        } else {
            "vat.used"
        },
        &b,
        "",
    )?;
    Ok(Json(json!({"box": b, "not_used": body.not_used})))
}

#[derive(Deserialize)]
pub struct MapIn {
    account: i64,
    #[serde(rename = "box")]
    vat_box: String,
}

/// Map an account to an output or input VAT box. A change clears the verified flag of that box.
pub async fn map_account(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Json(b): Json<MapIn>,
) -> ApiResult {
    require_admin(&actor)?;
    let conn = st.conn.lock().unwrap();
    let kind: Option<String> = conn
        .query_row(
            "SELECT kind FROM vat_box WHERE box = ?1",
            params![b.vat_box],
            |r| r.get(0),
        )
        .ok();
    let side = match kind.as_deref() {
        Some("output") => "credit",
        Some("input") => "debit",
        Some(_) => {
            return Err(ApiError::invalid(
                "only output or input VAT boxes can be mapped to accounts",
            ));
        }
        None => return Err(ApiError::invalid("unknown VAT box")),
    };
    let known: i64 = conn.query_row(
        "SELECT COUNT(*) FROM account WHERE number = ?1",
        params![b.account],
        |r| r.get(0),
    )?;
    if known == 0 {
        return Err(ApiError::invalid(format!("unknown account {}", b.account)));
    }
    conn.execute(
        "INSERT INTO vat_account(account, box, side) VALUES (?1, ?2, ?3)
         ON CONFLICT(account) DO UPDATE SET box = excluded.box, side = excluded.side",
        params![b.account, b.vat_box, side],
    )?;
    conn.execute(
        "UPDATE vat_box SET verified = 0, verified_by = NULL, verified_at = NULL WHERE box = ?1",
        params![b.vat_box],
    )?;
    crate::ledger::audit(&conn, &actor, "vat.map", &b.account.to_string(), &b.vat_box)?;
    Ok(Json(
        json!({"account": b.account, "box": b.vat_box, "side": side}),
    ))
}

#[derive(Deserialize, Default)]
pub struct ReportQuery {
    quarter: Option<String>,
}

fn parse_quarter(q: &str) -> Result<(i32, u32), ApiError> {
    let bad = || ApiError::invalid("quarter must look like 2026-Q3");
    let (y, n) = q.split_once("-Q").ok_or_else(bad)?;
    let year: i32 = y.parse().map_err(|_| bad())?;
    let n: u32 = n.parse().map_err(|_| bad())?;
    if !(1..=4).contains(&n) || !(2000..=2100).contains(&year) {
        return Err(bad());
    }
    Ok((year, n))
}

/// Quarter bounds and the filing deadline: the 12th of the second month after the quarter (the 17th
/// in January and August), moved to Monday if it falls on a weekend. Public holidays are not
/// considered.
fn quarter_dates(year: i32, n: u32) -> (String, String, String) {
    let start_month = (n - 1) * 3 + 1;
    let from = NaiveDate::from_ymd_opt(year, start_month, 1).unwrap();
    let end_month = start_month + 2;
    let next = if end_month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, end_month + 1, 1).unwrap()
    };
    let to = next - Duration::days(1);
    let (dy, dm) = {
        let m = end_month + 2;
        if m > 12 {
            (year + 1, m - 12)
        } else {
            (year, m)
        }
    };
    let day = if dm == 1 || dm == 8 { 17 } else { 12 };
    let mut due = NaiveDate::from_ymd_opt(dy, dm, day).unwrap();
    while matches!(due.weekday(), Weekday::Sat | Weekday::Sun) {
        due += Duration::days(1);
    }
    (from.to_string(), to.to_string(), due.to_string())
}

fn current_quarter() -> String {
    let d = Utc::now().date_naive();
    format!("{}-Q{}", d.year(), (d.month() - 1) / 3 + 1)
}

pub async fn report(State(st): State<Shared>, Query(q): Query<ReportQuery>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    let quarter = q.quarter.unwrap_or_else(current_quarter);
    let (year, n) = parse_quarter(&quarter)?;
    let (from, to, due) = quarter_dates(year, n);

    // Base boxes: lines that carry a VAT code with a base box.
    let mut amounts: std::collections::BTreeMap<String, i64> = std::collections::BTreeMap::new();
    let mut contributing: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
        Default::default();
    {
        let mut stmt = conn.prepare(
            "SELECT c.base_box, c.base_side, e.debit, e.credit, v.series || v.number
             FROM entry e JOIN voucher v ON v.id = e.voucher_id JOIN vat_code c ON c.code = e.vat_code
             WHERE c.base_box IS NOT NULL AND v.event_date >= ?1 AND v.event_date <= ?2",
        )?;
        let rows = stmt.query_map(params![from, to], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        for row in rows.filter_map(Result::ok) {
            let amt = if row.1 == "credit" {
                row.3 - row.2
            } else {
                row.2 - row.3
            };
            *amounts.entry(row.0.clone()).or_default() += amt;
            contributing.entry(row.0).or_default().insert(row.4);
        }
    }
    // Output and input VAT boxes: mapped accounts.
    {
        let mut stmt = conn.prepare(
            "SELECT m.box, m.side, e.debit, e.credit, v.series || v.number
             FROM entry e JOIN voucher v ON v.id = e.voucher_id JOIN vat_account m ON m.account = e.account
             WHERE v.event_date >= ?1 AND v.event_date <= ?2",
        )?;
        let rows = stmt.query_map(params![from, to], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        for row in rows.filter_map(Result::ok) {
            let amt = if row.1 == "credit" {
                row.3 - row.2
            } else {
                row.2 - row.3
            };
            *amounts.entry(row.0.clone()).or_default() += amt;
            contributing.entry(row.0).or_default().insert(row.4);
        }
    }
    let output: i64 = ["10", "11", "12", "30", "31", "32"]
        .iter()
        .map(|b| amounts.get(*b).copied().unwrap_or(0))
        .sum();
    let input = amounts.get("48").copied().unwrap_or(0);
    amounts.insert("49".into(), output - input);

    let meta: Vec<(String, String, String, String, bool, Option<String>, bool)> = conn
        .prepare("SELECT box, label, kind, source, verified, verified_by, not_used FROM vat_box ORDER BY box")?
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get::<_, i64>(4)? != 0, r.get(5)?, r.get::<_, i64>(6)? != 0))
        })?
        .filter_map(Result::ok)
        .collect();
    let boxes: Vec<Value> = meta
        .iter()
        .map(|(b, label, kind, source, verified, by, not_used)| {
            json!({"box": b, "label": label, "kind": kind, "not_used": not_used,
                   "amount": format_amount(amounts.get(b).copied().unwrap_or(0)),
                   "vouchers": contributing.get(b).map(|s| s.iter().cloned().collect::<Vec<_>>()).unwrap_or_default(),
                   "verified": verified, "verified_by": by, "mapping_source": source})
        })
        .collect();
    let unverified: Vec<String> = meta
        .iter()
        .filter(|m| !m.4 && !m.6)
        .map(|m| m.0.clone())
        .collect();
    let misused: Vec<String> = meta
        .iter()
        .filter(|m| m.6 && amounts.get(&m.0).copied().unwrap_or(0) != 0)
        .map(|m| m.0.clone())
        .collect();

    // Checks that tell the owner what is missing before this can be filed.
    let mut checks: Vec<Value> = vec![];
    let mut check = |name: &str, ok: bool, detail: String| {
        checks.push(json!({"check": name, "ok": ok, "detail": detail}));
    };
    check(
        "every box mapping verified by the owner",
        unverified.is_empty(),
        if unverified.is_empty() {
            "all boxes verified".into()
        } else {
            format!(
                "UNVERIFIED boxes: {}. Check each against SKV 4700, then run `oas vat verify-box <box>`.",
                unverified.join(", ")
            )
        },
    );
    check(
        "no amount in a box marked not used",
        misused.is_empty(),
        if misused.is_empty() {
            "none".into()
        } else {
            format!(
                "amounts landed in boxes marked not used: {}. Verify those boxes, or fix the voucher.",
                misused.join(", ")
            )
        },
    );
    let rc_base = amounts.get("21").copied().unwrap_or(0) + amounts.get("22").copied().unwrap_or(0);
    let rc_out = amounts.get("30").copied().unwrap_or(0);
    let expected = (rc_base as f64 * 0.25).round() as i64;
    check(
        "reverse charge output VAT is 25 percent of the reverse charge purchases",
        (rc_out - expected).abs() <= 100,
        format!(
            "boxes 21+22 = {}, box 30 = {}, 25 percent of the base = {}. Only valid if all reverse charge purchases were at 25 percent.",
            format_amount(rc_base),
            format_amount(rc_out),
            format_amount(expected)
        ),
    );
    let untagged: Vec<String> = conn
        .prepare(
            "SELECT DISTINCT v.series || v.number FROM voucher v
             JOIN entry e ON e.voucher_id = v.id JOIN vat_account m ON m.account = e.account
             WHERE v.event_date >= ?1 AND v.event_date <= ?2 AND v.corrects_voucher_id IS NULL
               AND NOT EXISTS (SELECT 1 FROM entry x WHERE x.voucher_id = v.id AND x.vat_code IS NOT NULL)",
        )?
        .query_map(params![from, to], |r| r.get(0))?
        .filter_map(Result::ok)
        .collect();
    check(
        "every voucher with VAT has a VAT code on its net line",
        untagged.is_empty(),
        if untagged.is_empty() {
            "all tagged".into()
        } else {
            format!(
                "no VAT code on any line of: {}. The base boxes are understated.",
                untagged.join(", ")
            )
        },
    );
    let no_receipt: Vec<String> = conn
        .prepare(
            "SELECT DISTINCT v.series || v.number FROM voucher v
             JOIN entry e ON e.voucher_id = v.id JOIN vat_account m ON m.account = e.account
             WHERE v.event_date >= ?1 AND v.event_date <= ?2 AND v.corrects_voucher_id IS NULL
               AND NOT EXISTS (SELECT 1 FROM voucher_document d WHERE d.voucher_id = v.id)",
        )?
        .query_map(params![from, to], |r| r.get(0))?
        .filter_map(Result::ok)
        .collect();
    check(
        "every voucher with VAT has a receipt attached",
        no_receipt.is_empty(),
        if no_receipt.is_empty() {
            "all have receipts".into()
        } else {
            format!("no receipt on: {}.", no_receipt.join(", "))
        },
    );
    let unbooked: i64 = conn.query_row(
        "SELECT COUNT(*) FROM bank_transaction WHERE status = 'new' AND booking_date >= ?1 AND booking_date <= ?2",
        params![from, to],
        |r| r.get(0),
    )?;
    check(
        "no unbooked bank lines in the period",
        unbooked == 0,
        format!("{unbooked} bank lines still to book"),
    );
    let pending: i64 = conn.query_row(
        "SELECT COUNT(*) FROM draft WHERE event_date >= ?1 AND event_date <= ?2",
        params![from, to],
        |r| r.get(0),
    )?;
    check(
        "no drafts waiting for approval in the period",
        pending == 0,
        format!("{pending} drafts waiting"),
    );
    let loose: i64 = conn.query_row(
        "SELECT COUNT(*) FROM document d JOIN document_meta m ON m.document_id = d.id
         WHERE m.doc_date >= ?1 AND m.doc_date <= ?2
           AND NOT EXISTS (SELECT 1 FROM voucher_document v WHERE v.document_id = d.id)",
        params![from, to],
        |r| r.get(0),
    )?;
    check(
        "no receipts dated in the period left unbooked",
        loose == 0,
        format!("{loose} receipts not linked to a voucher"),
    );
    let vouchers: i64 = conn.query_row(
        "SELECT COUNT(*) FROM voucher WHERE event_date >= ?1 AND event_date <= ?2",
        params![from, to],
        |r| r.get(0),
    )?;

    let ready = checks.iter().all(|c| c["ok"] == json!(true));
    Ok(Json(json!({
        "quarter": quarter, "from": from, "to": to,
        "due_date": due,
        "due_date_note": "12th of the second month after the quarter (17th in January and August), moved to the next weekday. Public holidays are not considered. Unverified; confirm on skatteverket.se.",
        "vouchers_in_period": vouchers,
        "boxes": boxes,
        "to_pay": format_amount(output - input),
        "checks": checks,
        "ready_to_file": ready,
        "filing": "Enter the figures in Skatteverket's e-service and sign there. An XML upload format is not generated because its layout has not been verified. Amounts are exact SEK; the form's rounding rules are unverified.",
        "warning": if unverified.is_empty() { Value::Null } else { json!("The box mapping is UNVERIFIED. Do not file from this report until every box is checked against Skatteverket's SKV 4700.") },
    })))
}
