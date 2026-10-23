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
    name.parse::<Tz>().map_err(|_| format!("unknown timezone '{name}'"))
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
    fields.iter().map(|f| csv_field(f)).collect::<Vec<_>>().join(",")
}

/// Parse a decimal amount such as "1250", "1250.5" or "1 250,50" into öre.
/// Negative numbers and more than two decimals are rejected.
pub fn parse_amount(input: &str) -> Result<i64, String> {
    let s: String = input.chars().filter(|c| !c.is_whitespace() && *c != '_').collect();
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
        return Err(format!("invalid amount '{input}': use digits with at most 2 decimals"));
    }
    let w: i64 = if whole.is_empty() { 0 } else { whole.parse().map_err(|_| "amount too large")? };
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
}
