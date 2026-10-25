//! Shared types and pure logic for open-accounting-sweden: time tracking models,
//! project matching, local-time parsing and CSV escaping. No I/O.

use chrono::{DateTime, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

pub const DEFAULT_TIMEZONE: &str = "Europe/Stockholm";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Activity {
    pub name: String,
    pub message: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub aliases: Vec<String>,
    pub client: Option<String>,
    pub archived: bool,
    /// "none", "hourly" or "fixed".
    pub rate_type: String,
    /// The hourly rate or the fixed fee as a decimal string, when one is set.
    pub rate: Option<String>,
    pub currency: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TimeEntry {
    pub id: i64,
    pub date: String,
    pub start: String,
    pub end: Option<String>,
    pub hours: f64,
    pub timezone: String,
    pub project: String,
    pub client: Option<String>,
    pub activity: String,
    pub message: String,
    pub running: bool,
    pub currency: String,
    /// The project's hourly rate, when it has one.
    pub rate: Option<String>,
    /// Hours times the hourly rate. Empty for projects without an hourly rate.
    pub amount: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum Match {
    /// Index of the project whose name or alias equals the query.
    Exact(usize),
    /// Indexes of projects that look similar; the caller must confirm.
    Suggest(Vec<usize>),
    None,
}

pub fn norm(s: &str) -> String {
    s.trim().to_lowercase()
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Match a typed project name against known projects (names and aliases).
/// Archived projects are never matched.
pub fn match_project(projects: &[Project], query: &str) -> Match {
    let q = norm(query);
    if q.is_empty() {
        return Match::None;
    }
    let names = |p: &Project| -> Vec<String> {
        std::iter::once(p.name.as_str())
            .chain(p.aliases.iter().map(String::as_str))
            .map(norm)
            .collect()
    };
    for (i, p) in projects.iter().enumerate() {
        if !p.archived && names(p).iter().any(|n| *n == q) {
            return Match::Exact(i);
        }
    }
    let threshold = match q.chars().count() {
        0..=3 => 0,
        4..=6 => 1,
        _ => 2,
    };
    let similar: Vec<usize> = projects
        .iter()
        .enumerate()
        .filter(|(_, p)| !p.archived)
        .filter(|(_, p)| {
            names(p).iter().any(|n| {
                n.contains(&q) || q.contains(n.as_str()) || levenshtein(n, &q) <= threshold
            })
        })
        .map(|(i, _)| i)
        .collect();
    if similar.is_empty() {
        Match::None
    } else {
        Match::Suggest(similar)
    }
}

pub fn parse_tz(name: &str) -> Result<Tz, String> {
    name.parse::<Tz>()
        .map_err(|_| format!("unknown timezone '{name}'"))
}

/// Parse a local date (YYYY-MM-DD) and time (HH:MM) in a timezone into UTC.
pub fn parse_local(date: &str, time: &str, tz: Tz) -> Result<DateTime<Utc>, String> {
    let d = NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| format!("invalid date '{date}', expected YYYY-MM-DD"))?;
    let t = NaiveTime::parse_from_str(time, "%H:%M")
        .map_err(|_| format!("invalid time '{time}', expected HH:MM"))?;
    tz.from_local_datetime(&d.and_time(t))
        .earliest()
        .map(|dt| dt.with_timezone(&Utc))
        .ok_or_else(|| format!("{date} {time} does not exist in {tz}"))
}

pub fn utc_string(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub fn hours_between(start: DateTime<Utc>, end: DateTime<Utc>) -> f64 {
    let secs = (end - start).num_seconds().max(0) as f64;
    (secs / 36.0).round() / 100.0
}

/// Escape one CSV field. Cells that a spreadsheet could run as a formula get a
/// leading apostrophe.
pub fn csv_field(value: &str) -> String {
    let mut v = value.to_string();
    if v.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        v.insert(0, '\'');
    }
    if v.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v
    }
}

pub fn csv_row(fields: &[String]) -> String {
    fields
        .iter()
        .map(|f| csv_field(f))
        .collect::<Vec<_>>()
        .join(",")
}

/// Parse a decimal amount such as "1250", "1250.5" or "1 250,50" into öre.
/// Negative numbers and more than two decimals are rejected.
pub fn parse_amount(input: &str) -> Result<i64, String> {
    let s: String = input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '_')
        .collect();
    let s = s.replace(',', ".");
    if s.is_empty() {
        return Err("amount is empty".into());
    }
    let (whole, frac) = match s.split_once('.') {
        Some((w, f)) => (w, f),
        None => (s.as_str(), ""),
    };
    if whole.is_empty() && frac.is_empty()
        || !whole.chars().all(|c| c.is_ascii_digit())
        || !frac.chars().all(|c| c.is_ascii_digit())
        || frac.len() > 2
        || whole.len() > 12
    {
        return Err(format!(
            "invalid amount '{input}': use digits with at most 2 decimals"
        ));
    }
    let w: i64 = if whole.is_empty() {
        0
    } else {
        whole.parse().map_err(|_| "amount too large")?
    };
    let f: i64 = format!("{frac:0<2}").parse().unwrap_or(0);
    Ok(w * 100 + f)
}

/// Format öre as a decimal string with two decimals, e.g. 125050 -> "1250.50".
pub fn format_amount(ore: i64) -> String {
    let sign = if ore < 0 { "-" } else { "" };
    let a = ore.abs();
    format!("{sign}{}.{:02}", a / 100, a % 100)
}

/// Encode text as code page 437 (SIE "PC8"). Unknown characters become '?'.
pub fn to_cp437(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| match c {
            c if (c as u32) < 0x80 => c as u8,
            'Ç' => 0x80,
            'ü' => 0x81,
            'é' => 0x82,
            'ä' => 0x84,
            'å' => 0x86,
            'ö' => 0x94,
            'Ä' => 0x8E,
            'Å' => 0x8F,
            'É' => 0x90,
            'Ö' => 0x99,
            'Ü' => 0x9A,
            '€' => b'E',
            _ => b'?',
        })
        .collect()
}

/// Quote a SIE text field: wrap in double quotes and escape quotes and newlines.
pub fn sie_quote(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    format!("\"{}\"", cleaned.replace('\\', "\\\\").replace('"', "\\\""))
}

// ---------------------------------------------------------------------------------------------
// Bank statement import
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct BankRow {
    pub date: String,
    /// Signed amount in öre: money in is positive, money out is negative.
    pub amount: i64,
    pub currency: String,
    pub counterparty: String,
    pub reference: String,
}

/// Parse a signed amount such as "-1 250,50", "1250.5" or "−99" into öre.
pub fn parse_signed_amount(input: &str) -> Result<i64, String> {
    let cleaned: String = input
        .trim()
        .replace(['\u{2212}', '\u{2013}'], "-")
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '\u{a0}')
        .collect();
    let (neg, rest) = match cleaned.strip_prefix('-') {
        Some(r) => (true, r.to_string()),
        None => (false, cleaned.trim_start_matches('+').to_string()),
    };
    // A dot or comma followed by exactly three digits and then another separator is a thousands mark.
    let value = parse_amount(&rest)?;
    Ok(if neg { -value } else { value })
}

fn normalise_date(s: &str) -> Option<String> {
    let t = s.trim();
    for fmt in ["%Y-%m-%d", "%Y/%m/%d", "%d.%m.%Y", "%d/%m/%Y", "%Y%m%d"] {
        if let Ok(d) = NaiveDate::parse_from_str(t.get(..10.min(t.len())).unwrap_or(t), fmt)
            .or_else(|_| NaiveDate::parse_from_str(t, fmt))
        {
            return Some(d.format("%Y-%m-%d").to_string());
        }
    }
    None
}

fn split_csv_line(line: &str, delim: char) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            c if c == delim && !quoted => out.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    out.push(cur);
    out.into_iter().map(|f| f.trim().to_string()).collect()
}

fn find_col(header: &[String], names: &[&str]) -> Option<usize> {
    header.iter().position(|h| {
        let h = h.trim().trim_start_matches('\u{feff}').to_lowercase();
        names.iter().any(|n| h == *n)
    })
}

/// Parse a bank statement CSV. The delimiter (comma or semicolon) and common English and Swedish
/// column names are detected: date, amount, text or counterparty, reference, currency. Rows with an
/// unreadable date or amount are reported in the second return value.
pub fn parse_bank_csv(text: &str) -> Result<(Vec<BankRow>, Vec<String>), String> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let first = lines.next().ok_or("the file is empty")?;
    let delim = if first.matches(';').count() > first.matches(',').count() {
        ';'
    } else {
        ','
    };
    let header = split_csv_line(first, delim);
    let date_c = find_col(
        &header,
        &[
            "date",
            "datum",
            "bokföringsdatum",
            "bokforingsdatum",
            "booking date",
            "transaction date",
            "transaktionsdatum",
        ],
    )
    .ok_or("no date column found (expected date, datum or bokföringsdatum)")?;
    let amount_c = find_col(&header, &["amount", "belopp", "sum", "summa"])
        .ok_or("no amount column found (expected amount or belopp)")?;
    let text_c = find_col(
        &header,
        &[
            "counterparty",
            "text",
            "description",
            "beskrivning",
            "mottagare",
            "name",
            "namn",
            "merchant",
        ],
    );
    let ref_c = find_col(
        &header,
        &["reference", "referens", "message", "meddelande", "note"],
    );
    let cur_c = find_col(&header, &["currency", "valuta"]);
    let mut rows = vec![];
    let mut errors = vec![];
    for (n, line) in lines.enumerate() {
        let f = split_csv_line(line, delim);
        let get = |i: Option<usize>| i.and_then(|i| f.get(i)).cloned().unwrap_or_default();
        let Some(date) = normalise_date(&get(Some(date_c))) else {
            errors.push(format!(
                "row {}: unreadable date '{}'",
                n + 2,
                get(Some(date_c))
            ));
            continue;
        };
        let Ok(amount) = parse_signed_amount(&get(Some(amount_c))) else {
            errors.push(format!(
                "row {}: unreadable amount '{}'",
                n + 2,
                get(Some(amount_c))
            ));
            continue;
        };
        let currency = {
            let c = get(cur_c).to_uppercase();
            if c.len() == 3 { c } else { "SEK".to_string() }
        };
        rows.push(BankRow {
            date,
            amount,
            currency,
            counterparty: get(text_c),
            reference: get(ref_c),
        });
    }
    Ok((rows, errors))
}

// ---------------------------------------------------------------------------------------------
// Receipt to bank transaction matching
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct MatchScores {
    pub total: f64,
    pub name: f64,
    pub amount: f64,
    pub date: f64,
    pub currency: f64,
}

fn tokens(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.chars().count() >= 3)
        .map(String::from)
        .collect()
}

/// How alike a receipt's vendor and a bank line's text are, from 0 to 1.
pub fn name_score(vendor: &str, bank_text: &str) -> f64 {
    let a = tokens(vendor);
    let b = tokens(bank_text);
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let shared = a
        .iter()
        .filter(|t| {
            b.iter()
                .any(|u| u == *t || u.contains(t.as_str()) || t.contains(u.as_str()))
        })
        .count();
    (shared as f64 / a.len().min(b.len()) as f64).min(1.0)
}

/// Amount closeness from 0 to 1 using the relative difference of the absolute amounts.
pub fn amount_score(tx_ore: i64, doc_ore: i64) -> f64 {
    let (t, d) = (tx_ore.abs() as f64, doc_ore.abs() as f64);
    if t == 0.0 || d == 0.0 {
        return 0.0;
    }
    let rel = (t - d).abs() / t.max(d);
    match rel {
        r if r <= 0.0 => 1.0,
        r if r <= 0.005 => 0.97,
        r if r <= 0.02 => 0.85,
        r if r <= 0.05 => 0.6,
        _ => 0.0,
    }
}

/// Date closeness from 0 to 1 by the number of days between the receipt and the bank booking.
pub fn date_score(tx_date: &str, doc_date: &str) -> f64 {
    let (Ok(a), Ok(b)) = (
        NaiveDate::parse_from_str(tx_date, "%Y-%m-%d"),
        NaiveDate::parse_from_str(doc_date, "%Y-%m-%d"),
    ) else {
        return 0.0;
    };
    match (a - b).num_days().abs() {
        0..=3 => 1.0,
        4..=7 => 0.8,
        8..=14 => 0.5,
        15..=30 => 0.2,
        _ => 0.0,
    }
}

/// Score a receipt against a bank transaction. Returns None when the amount does not fit at all,
/// because an amount mismatch is never worth a suggestion.
#[allow(clippy::too_many_arguments)]
pub fn score_match(
    tx_amount: i64,
    tx_currency: &str,
    tx_date: &str,
    tx_text: &str,
    doc_amount: Option<i64>,
    doc_currency: &str,
    doc_date: Option<&str>,
    doc_vendor: &str,
) -> Option<MatchScores> {
    let amount = amount_score(tx_amount, doc_amount?);
    if amount == 0.0 {
        return None;
    }
    let currency = if tx_currency.eq_ignore_ascii_case(doc_currency) {
        1.0
    } else {
        0.3
    };
    let date = doc_date.map(|d| date_score(tx_date, d)).unwrap_or(0.4);
    let name = name_score(doc_vendor, tx_text);
    let total = 0.45 * amount + 0.25 * name + 0.2 * date + 0.1 * currency;
    Some(MatchScores {
        total,
        name,
        amount,
        date,
        currency,
    })
}

// ---------------------------------------------------------------------------------------------
// Payroll: identity numbers and employer contributions
// ---------------------------------------------------------------------------------------------

/// Validate a 12-digit Swedish personnummer or samordningsnummer (YYYYMMDDNNNC, hyphen optional)
/// with the Luhn check on the last ten digits. Returns the 12 digits and the birth year.
pub fn parse_personnummer(input: &str) -> Result<(String, i32), String> {
    let digits: String = input
        .chars()
        .filter(|c| !matches!(c, '-' | ' ' | '+'))
        .collect();
    if digits.len() != 12 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err("identity must be 12 digits: YYYYMMDDNNNC".into());
    }
    let year: i32 = digits[..4]
        .parse()
        .map_err(|_| "invalid year".to_string())?;
    let month: u32 = digits[4..6]
        .parse()
        .map_err(|_| "invalid month".to_string())?;
    let mut day: u32 = digits[6..8]
        .parse()
        .map_err(|_| "invalid day".to_string())?;
    if day > 60 {
        day -= 60; // samordningsnummer
    }
    if !(1900..=2100).contains(&year) || NaiveDate::from_ymd_opt(year, month, day).is_none() {
        return Err("identity does not contain a valid birth date".into());
    }
    let sum: u32 = digits[2..]
        .chars()
        .enumerate()
        .map(|(i, c)| {
            let d = c.to_digit(10).unwrap();
            let v = if i % 2 == 0 { d * 2 } else { d };
            if v > 9 { v - 9 } else { v }
        })
        .sum();
    if sum % 10 != 0 {
        return Err("identity fails the check digit test".into());
    }
    Ok((digits, year))
}

/// Employer contribution rates in percent. Every value is unverified until the owner checks it.
#[derive(Debug, Clone)]
pub struct ContribRates {
    pub standard: f64,
    pub senior: f64,
    pub over79: f64,
    pub youth: f64,
    /// Monthly pay in öre up to which the youth rate applies.
    pub youth_cap: i64,
    pub youth_from_birth: i32,
    pub youth_to_birth: i32,
    /// First and last period (YYYY-MM) of the youth reduction.
    pub youth_start: String,
    pub youth_end: String,
}

/// Employer contribution in öre for one employee and month, rounded to whole kronor, plus a
/// plain-language note on which rate applied. Benefit and special cases are not covered.
pub fn employer_contribution(
    base: i64,
    birth_year: i32,
    period: &str,
    r: &ContribRates,
) -> (i64, String) {
    if base <= 0 {
        return (0, "no pay".into());
    }
    let whole_kr = |ore: f64| ((ore / 100.0).round() * 100.0) as i64;
    if birth_year >= 1959 {
        let youth = birth_year >= r.youth_from_birth
            && birth_year <= r.youth_to_birth
            && period >= r.youth_start.as_str()
            && period <= r.youth_end.as_str();
        if youth {
            let low = base.min(r.youth_cap);
            let high = base - low;
            let c = low as f64 * r.youth / 100.0 + high as f64 * r.standard / 100.0;
            return (
                whole_kr(c),
                format!(
                    "youth rate {}% up to the monthly cap, {}% above",
                    r.youth, r.standard
                ),
            );
        }
        (
            whole_kr(base as f64 * r.standard / 100.0),
            format!("standard rate {}%", r.standard),
        )
    } else if birth_year >= 1938 {
        (
            whole_kr(base as f64 * r.senior / 100.0),
            format!("reduced rate {}% (born 1938-1958)", r.senior),
        )
    } else {
        (
            whole_kr(base as f64 * r.over79 / 100.0),
            format!("rate {}% (born 1937 or earlier)", r.over79),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amounts_round_trip() {
        assert_eq!(parse_amount("1250"), Ok(125000));
        assert_eq!(parse_amount("1 250,5"), Ok(125050));
        assert_eq!(parse_amount("0.07"), Ok(7));
        assert_eq!(format_amount(125050), "1250.50");
        assert_eq!(format_amount(-7), "-0.07");
        assert!(parse_amount("-5").is_err());
        assert!(parse_amount("1.234").is_err());
        assert!(parse_amount("abc").is_err());
        assert!(parse_amount("").is_err());
    }

    #[test]
    fn cp437_encodes_swedish_letters() {
        assert_eq!(to_cp437("åäö"), vec![0x86, 0x84, 0x94]);
        assert_eq!(to_cp437("ÅÄÖ"), vec![0x8F, 0x8E, 0x99]);
        assert_eq!(to_cp437("abc"), b"abc".to_vec());
    }

    #[test]
    fn sie_quote_escapes() {
        assert_eq!(sie_quote("a \"b\""), "\"a \\\"b\\\"\"");
    }

    fn project(id: i64, name: &str, aliases: &[&str], archived: bool) -> Project {
        Project {
            id,
            name: name.into(),
            aliases: aliases.iter().map(|s| s.to_string()).collect(),
            client: None,
            archived,
            rate_type: "none".into(),
            rate: None,
            currency: "SEK".into(),
        }
    }

    #[test]
    fn exact_match_by_name_and_alias() {
        let ps = vec![project(1, "Acme Corp", &["acme"], false)];
        assert_eq!(match_project(&ps, "acme corp"), Match::Exact(0));
        assert_eq!(match_project(&ps, " ACME "), Match::Exact(0));
    }

    #[test]
    fn typo_is_suggested_not_matched() {
        let ps = vec![project(1, "Spacecorps", &[], false)];
        assert_eq!(match_project(&ps, "spacecorp"), Match::Suggest(vec![0]));
    }

    #[test]
    fn unknown_name_has_no_match() {
        let ps = vec![project(1, "Acme", &[], false)];
        assert_eq!(match_project(&ps, "zzzzz"), Match::None);
    }

    #[test]
    fn archived_projects_are_skipped() {
        let ps = vec![project(1, "Old", &[], true)];
        assert_eq!(match_project(&ps, "old"), Match::None);
    }

    #[test]
    fn local_time_converts_to_utc() {
        let tz = parse_tz("Europe/Stockholm").unwrap();
        // 21 Oct 2026 is CEST (UTC+2): 11:00 local is 09:00 UTC.
        let dt = parse_local("2026-10-21", "11:00", tz).unwrap();
        assert_eq!(utc_string(dt), "2026-10-21T09:00:00Z");
    }

    #[test]
    fn one_hour_is_one_hour() {
        let tz = parse_tz("Europe/Stockholm").unwrap();
        let a = parse_local("2026-10-21", "11:00", tz).unwrap();
        let b = parse_local("2026-10-21", "12:00", tz).unwrap();
        assert_eq!(hours_between(a, b), 1.0);
    }

    #[test]
    fn csv_escapes_formulas_and_quotes() {
        assert_eq!(csv_field("=1+1"), "'=1+1");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field("plain"), "plain");
    }

    #[test]
    fn signed_amounts() {
        assert_eq!(parse_signed_amount("-1 250,50"), Ok(-125050));
        assert_eq!(parse_signed_amount("\u{2212}99"), Ok(-9900));
        assert_eq!(parse_signed_amount("+12.5"), Ok(1250));
        assert!(parse_signed_amount("x").is_err());
    }

    #[test]
    fn parses_semicolon_swedish_csv() {
        let csv = "Bokföringsdatum;Text;Belopp;Saldo\n2026-10-21;AMAZON WEB SERVICES;-1 250,00;10 000,00\n2026-10-22;Kund AB faktura 12;12 500,00;22 500,00\nbad;x;1;1\n";
        let (rows, errors) = parse_bank_csv(csv).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].amount, -125000);
        assert_eq!(rows[0].counterparty, "AMAZON WEB SERVICES");
        assert_eq!(rows[1].amount, 1250000);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn parses_comma_csv_with_quotes() {
        let csv = "date,description,amount,currency\n2026-10-23,\"GitHub, Inc\",-40.00,USD\n";
        let (rows, _) = parse_bank_csv(csv).unwrap();
        assert_eq!(rows[0].counterparty, "GitHub, Inc");
        assert_eq!(rows[0].currency, "USD");
        assert_eq!(rows[0].amount, -4000);
    }

    #[test]
    fn csv_needs_date_and_amount() {
        assert!(parse_bank_csv("a,b\n1,2\n").is_err());
        assert!(parse_bank_csv("").is_err());
    }

    #[test]
    fn matching_prefers_same_amount_name_and_date() {
        let good = score_match(
            -125000,
            "SEK",
            "2026-10-21",
            "AMAZON WEB SERVICES EMEA",
            Some(125000),
            "SEK",
            Some("2026-10-20"),
            "Amazon Web Services",
        )
        .unwrap();
        let weak = score_match(
            -125000,
            "SEK",
            "2026-10-21",
            "SPOTIFY",
            Some(125000),
            "SEK",
            Some("2026-09-01"),
            "Telia",
        )
        .unwrap();
        assert!(good.total > 0.9, "{good:?}");
        assert!(weak.total < good.total);
        assert!(
            score_match(
                -125000,
                "SEK",
                "2026-10-21",
                "x",
                Some(50000),
                "SEK",
                None,
                "x"
            )
            .is_none()
        );
        assert!(score_match(-125000, "SEK", "2026-10-21", "x", None, "SEK", None, "x").is_none());
    }

    #[test]
    fn name_score_handles_partial_overlap() {
        assert!(name_score("GitHub", "GITHUB INC SAN FRANCISCO") > 0.9);
        assert_eq!(name_score("", "anything"), 0.0);
        assert_eq!(name_score("Telia", "Spotify"), 0.0);
    }

    #[test]
    fn personnummer_luhn() {
        // 19900101-0017 is a commonly used valid test number: check digit logic only.
        let (d, y) = parse_personnummer("199001010017").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(d.len(), 12);
        assert_eq!(y, 1990);
        assert!(parse_personnummer("199001010018").is_err());
        assert!(parse_personnummer("19900101").is_err());
        assert!(parse_personnummer("199013010017").is_err());
    }

    fn rates() -> ContribRates {
        ContribRates {
            standard: 31.42,
            senior: 10.21,
            over79: 0.0,
            youth: 20.81,
            youth_cap: 2_500_000,
            youth_from_birth: 2003,
            youth_to_birth: 2007,
            youth_start: "2026-04".into(),
            youth_end: "2027-09".into(),
        }
    }

    #[test]
    fn contributions_by_age_class() {
        let r = rates();
        // 50 000 kr in a month, born 1985: 31.42% = 15 710 kr
        assert_eq!(
            employer_contribution(5_000_000, 1985, "2026-10", &r).0,
            1_571_000
        );
        // born 1950: 10.21% of 50 000 = 5 105 kr
        assert_eq!(
            employer_contribution(5_000_000, 1950, "2026-10", &r).0,
            510_500
        );
        // born 1930: nothing
        assert_eq!(employer_contribution(5_000_000, 1930, "2026-10", &r).0, 0);
        assert_eq!(employer_contribution(0, 1985, "2026-10", &r).0, 0);
    }

    #[test]
    fn youth_reduction_applies_only_inside_its_window() {
        let r = rates();
        // 30 000 kr, born 2004, October 2026: 25 000 at 20.81% + 5 000 at 31.42% = 5 202.5 + 1 571 = 6 773.5 -> 6 774 (rounded to whole kronor)
        let (c, note) = employer_contribution(3_000_000, 2004, "2026-10", &r);
        assert_eq!(c, 677_400, "{note}");
        // before the window starts the standard rate applies: 30 000 * 31.42% = 9 426
        assert_eq!(
            employer_contribution(3_000_000, 2004, "2026-03", &r).0,
            942_600
        );
        // outside the birth years
        assert_eq!(
            employer_contribution(3_000_000, 1990, "2026-10", &r).0,
            942_600
        );
    }
}
