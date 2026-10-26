//! open-accounting-sweden server: time tracking API and a small web panel.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Query, State};
use axum::http::{HeaderValue, Request, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use oas_core::{
    Activity, DEFAULT_TIMEZONE, Match, Project, TimeEntry, csv_row, hours_between, match_project,
    norm, parse_local, parse_tz, utc_string,
};
use rusqlite::{Connection, params};
use serde::Deserialize;
use serde_json::{Value, json};
use subtle::ConstantTimeEq;

mod auth;
mod inbox;
mod invoices;
mod ledger;
mod payroll;
mod vat;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Role {
    Admin,
    Agent,
}

impl Role {
    fn name(self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Agent => "agent",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Actor {
    role: Role,
    name: String,
    user_id: Option<i64>,
    session: Option<String>,
}

struct AppState {
    conn: Mutex<Connection>,
    token: String,
    agent_token: Option<String>,
    limiter: auth::Limiter,
    dummy_hash: String,
    docs_dir: std::path::PathBuf,
}

type Shared = Arc<AppState>;
type ApiResult = Result<Json<Value>, ApiError>;

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    projects: Vec<String>,
}

impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            projects: vec![],
        }
    }
    fn invalid(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_input", message)
    }
    fn with_projects(mut self, projects: Vec<String>) -> Self {
        self.projects = projects;
        self
    }
}

impl From<rusqlite::Error> for ApiError {
    fn from(e: rusqlite::Error) -> Self {
        eprintln!("database error: {e}");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "database error",
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = json!({"error": {
            "code": self.code, "message": self.message, "projects": self.projects
        }});
        (self.status, Json(body)).into_response()
    }
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS activity(
  name TEXT PRIMARY KEY,
  message TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS project(
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE,
  aliases TEXT NOT NULL DEFAULT '[]',
  client TEXT,
  archived INTEGER NOT NULL DEFAULT 0,
  rate_type TEXT NOT NULL DEFAULT 'none',
  rate INTEGER,
  currency TEXT NOT NULL DEFAULT 'SEK'
);
CREATE TABLE IF NOT EXISTS time_entry(
  id INTEGER PRIMARY KEY,
  start_utc TEXT NOT NULL,
  end_utc TEXT,
  tz TEXT NOT NULL,
  activity TEXT NOT NULL REFERENCES activity(name),
  project_id INTEGER NOT NULL REFERENCES project(id),
  message TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL,
  source TEXT NOT NULL
);
";

fn open_db(path: &str) -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.execute_batch(SCHEMA)?;
    for (col, decl) in [
        ("rate_type", "TEXT NOT NULL DEFAULT 'none'"),
        ("rate", "INTEGER"),
        ("currency", "TEXT NOT NULL DEFAULT 'SEK'"),
    ] {
        let has: i64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM pragma_table_info('project') WHERE name = '{col}'"),
            [],
            |r| r.get(0),
        )?;
        if has == 0 {
            conn.execute_batch(&format!("ALTER TABLE project ADD COLUMN {col} {decl}"))?;
        }
    }
    // Who logged an entry, so the web form can preselect each user's last project. Older rows have none.
    let has: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('time_entry') WHERE name = 'created_by'",
        [],
        |r| r.get(0),
    )?;
    if has == 0 {
        conn.execute_batch("ALTER TABLE time_entry ADD COLUMN created_by TEXT")?;
    }
    // Deleted entries are kept, marked, and left out everywhere; the audit log says who and when.
    for (col, decl) in [("deleted_at", "TEXT"), ("deleted_by", "TEXT")] {
        let has: i64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM pragma_table_info('time_entry') WHERE name = '{col}'"),
            [],
            |r| r.get(0),
        )?;
        if has == 0 {
            conn.execute_batch(&format!("ALTER TABLE time_entry ADD COLUMN {col} {decl}"))?;
        }
    }
    conn.execute_batch(ledger::SCHEMA)?;
    conn.execute_batch(auth::SCHEMA)?;
    conn.execute_batch(inbox::SCHEMA)?;
    conn.execute_batch(vat::SCHEMA)?;
    conn.execute_batch(payroll::SCHEMA)?;
    conn.execute_batch(invoices::SCHEMA)?;
    ledger::migrate(&conn)?;
    ledger::seed(&conn)?;
    vat::seed(&conn)?;
    payroll::seed(&conn)?;
    for (name, message) in [
        ("meeting", "Meeting: {topic}"),
        ("building", "Building: {topic}"),
        ("strategizing", "Strategy: {topic}"),
    ] {
        conn.execute(
            "INSERT OR IGNORE INTO activity(name, message) VALUES (?1, ?2)",
            params![name, message],
        )?;
    }
    Ok(conn)
}

async fn auth(State(st): State<Shared>, mut req: Request<Body>, next: Next) -> Response {
    let given = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("")
        .to_string();
    // Both comparisons always run so timing does not reveal which token matched.
    let is_admin: bool = given.as_bytes().ct_eq(st.token.as_bytes()).into();
    let is_agent: bool = match &st.agent_token {
        Some(t) => given.as_bytes().ct_eq(t.as_bytes()).into(),
        None => false,
    };
    let actor = if is_admin {
        Some(Actor {
            role: Role::Admin,
            name: "admin-token".into(),
            user_id: None,
            session: None,
        })
    } else if is_agent {
        Some(Actor {
            role: Role::Agent,
            name: "agent-token".into(),
            user_id: None,
            session: None,
        })
    } else if given.starts_with("oas_") {
        let conn = st.conn.lock().unwrap();
        auth::actor_for_pat(&conn, &given)
    } else if given.len() == 64 {
        let conn = st.conn.lock().unwrap();
        auth::actor_for_session(&conn, &given)
    } else {
        None
    };
    if let Some(actor) = actor {
        req.extensions_mut().insert(actor);
        next.run(req).await
    } else {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "auth_required",
            "missing or invalid token",
        )
        .into_response()
    }
}

async fn security_headers(State(has_ui): State<bool>, req: Request<Body>, next: Next) -> Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    h.insert("x-frame-options", HeaderValue::from_static("DENY"));
    h.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    // The built web app needs no inline scripts. The small fallback page does.
    let csp = if has_ui {
        "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; \
         script-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'"
    } else {
        "default-src 'self'; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'"
    };
    h.insert("content-security-policy", HeaderValue::from_static(csp));
    res
}

fn load_projects(conn: &Connection) -> Result<Vec<Project>, ApiError> {
    let mut stmt =
        conn.prepare("SELECT id, name, aliases, client, archived, rate_type, rate, currency FROM project ORDER BY name")?;
    let rows = stmt.query_map([], |r| {
        let aliases: String = r.get(2)?;
        Ok(Project {
            id: r.get(0)?,
            name: r.get(1)?,
            aliases: serde_json::from_str(&aliases).unwrap_or_default(),
            client: r.get(3)?,
            archived: r.get::<_, i64>(4)? != 0,
            rate_type: r.get(5)?,
            rate: r.get::<_, Option<i64>>(6)?.map(oas_core::format_amount),
            currency: r.get(7)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn load_activities(conn: &Connection) -> Result<Vec<Activity>, ApiError> {
    let mut stmt = conn.prepare("SELECT name, message FROM activity ORDER BY name")?;
    let rows = stmt.query_map([], |r| {
        Ok(Activity {
            name: r.get(0)?,
            message: r.get(1)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn names(projects: &[Project], idx: &[usize]) -> Vec<String> {
    idx.iter().map(|&i| projects[i].name.clone()).collect()
}

/// Resolve a typed project name to a project id, creating it only when asked to.
fn resolve_project(conn: &Connection, given: Option<&str>, create: bool) -> Result<i64, ApiError> {
    let projects = load_projects(conn)?;
    let active: Vec<String> = projects
        .iter()
        .filter(|p| !p.archived)
        .map(|p| p.name.clone())
        .collect();
    let Some(name) = given.map(str::trim).filter(|n| !n.is_empty()) else {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "project_required",
            "no project given; pick one or create a new project",
        )
        .with_projects(active));
    };
    match match_project(&projects, name) {
        Match::Exact(i) => Ok(projects[i].id),
        Match::Suggest(idx) if !create => Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "project_not_found",
            format!("no project named '{name}'; did you mean one of these?"),
        )
        .with_projects(names(&projects, &idx))),
        Match::None if !create => Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "project_not_found",
            format!("no project named '{name}'; create it to continue"),
        )
        .with_projects(active)),
        _ => {
            conn.execute("INSERT INTO project(name) VALUES (?1)", params![name])
                .map_err(|_| ApiError::invalid("project name already exists"))?;
            Ok(conn.last_insert_rowid())
        }
    }
}

fn tz_of(name: Option<&str>) -> Result<Tz, ApiError> {
    parse_tz(name.unwrap_or(DEFAULT_TIMEZONE)).map_err(ApiError::invalid)
}

fn parse_utc(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_default()
}

fn clean(value: &str, field: &str, max: usize) -> Result<String, ApiError> {
    let v = value.trim();
    if v.chars().count() > max || v.chars().any(|c| c.is_control() && c != '\n') {
        return Err(ApiError::invalid(format!(
            "{field} is too long or has control characters"
        )));
    }
    Ok(v.to_string())
}

fn resolve_message(
    conn: &Connection,
    activity: &str,
    message: Option<&str>,
) -> Result<String, ApiError> {
    match message.map(str::trim).filter(|m| !m.is_empty()) {
        Some(m) => clean(m, "message", 500),
        None => {
            let template: String = conn
                .query_row(
                    "SELECT message FROM activity WHERE name = ?1",
                    params![activity],
                    |r| r.get(0),
                )
                .unwrap_or_default();
            // The {topic} placeholder is only filled when a message is given.
            Ok(template
                .replace("{topic}", "")
                .trim()
                .trim_end_matches(':')
                .trim()
                .to_string())
        }
    }
}

fn check_activity(conn: &Connection, name: &str) -> Result<String, ApiError> {
    let name = norm(name);
    let found: Option<String> = conn
        .query_row(
            "SELECT name FROM activity WHERE name = ?1",
            params![name],
            |r| r.get(0),
        )
        .ok();
    found.ok_or_else(|| {
        let known: Vec<String> = load_activities(conn)
            .unwrap_or_default()
            .into_iter()
            .map(|a| a.name)
            .collect();
        ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_input",
            format!("unknown activity '{name}'; known: {}", known.join(", ")),
        )
    })
}

fn check_overlap(
    conn: &Connection,
    start: DateTime<Utc>,
    end: Option<DateTime<Utc>>,
    except: Option<i64>,
) -> Result<(), ApiError> {
    let end_s = end
        .map(utc_string)
        .unwrap_or_else(|| "9999-12-31T00:00:00Z".into());
    let clash: i64 = conn.query_row(
        "SELECT COUNT(*) FROM time_entry
         WHERE start_utc < ?2 AND COALESCE(end_utc, '9999-12-31T00:00:00Z') > ?1
           AND deleted_at IS NULL AND id != ?3",
        params![utc_string(start), end_s, except.unwrap_or(-1)],
        |r| r.get(0),
    )?;
    if clash > 0 {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "conflict",
            "this time overlaps an existing entry (or a running timer)",
        ));
    }
    Ok(())
}

fn load_entries(conn: &Connection) -> Result<Vec<TimeEntry>, ApiError> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.start_utc, e.end_utc, e.tz, e.activity, e.message, p.name, p.client, p.rate_type, p.rate, p.currency
         FROM time_entry e JOIN project p ON p.id = e.project_id
         WHERE e.deleted_at IS NULL
         ORDER BY e.start_utc",
    )?;
    let now = Utc::now();
    let rows = stmt.query_map([], |r| {
        let start = parse_utc(&r.get::<_, String>(1)?);
        let end_s: Option<String> = r.get(2)?;
        let tz_name: String = r.get(3)?;
        let tz: Tz = tz_name.parse().unwrap_or(chrono_tz::Europe::Stockholm);
        let running = end_s.is_none();
        let end = end_s.as_deref().map(parse_utc);
        let ls = start.with_timezone(&tz);
        let hours = hours_between(start, end.unwrap_or(now));
        let rate_type: String = r.get(8)?;
        let rate: Option<i64> = r.get(9)?;
        let hourly = if rate_type == "hourly" { rate } else { None };
        Ok(TimeEntry {
            id: r.get(0)?,
            date: ls.format("%Y-%m-%d").to_string(),
            start: ls.format("%H:%M").to_string(),
            end: end.map(|e| e.with_timezone(&tz).format("%H:%M").to_string()),
            hours,
            timezone: tz_name,
            project: r.get(6)?,
            client: r.get(7)?,
            activity: r.get(4)?,
            message: r.get(5)?,
            running,
            currency: r.get(10)?,
            rate: hourly.map(oas_core::format_amount),
            amount: hourly.map(|rt| oas_core::format_amount((hours * rt as f64).round() as i64)),
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[derive(Deserialize, Default)]
pub(crate) struct Filters {
    pub(crate) from: Option<String>,
    pub(crate) to: Option<String>,
    pub(crate) project: Option<String>,
    pub(crate) activity: Option<String>,
}

pub(crate) fn filtered(conn: &Connection, f: &Filters) -> Result<Vec<TimeEntry>, ApiError> {
    let project_name = match f.project.as_deref().filter(|p| !p.trim().is_empty()) {
        Some(p) => {
            let projects = load_projects(conn)?;
            match match_project(&projects, p) {
                Match::Exact(i) => Some(projects[i].name.clone()),
                Match::Suggest(idx) => {
                    return Err(ApiError::new(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "project_not_found",
                        format!("no project named '{p}'"),
                    )
                    .with_projects(names(&projects, &idx)));
                }
                Match::None => {
                    return Err(ApiError::new(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "project_not_found",
                        format!("no project named '{p}'"),
                    ));
                }
            }
        }
        None => None,
    };
    let activity = f.activity.as_deref().map(norm).filter(|a| !a.is_empty());
    Ok(load_entries(conn)?
        .into_iter()
        .filter(|e| f.from.as_deref().is_none_or(|d| e.date.as_str() >= d))
        .filter(|e| f.to.as_deref().is_none_or(|d| e.date.as_str() <= d))
        .filter(|e| project_name.as_ref().is_none_or(|p| &e.project == p))
        .filter(|e| activity.as_ref().is_none_or(|a| &e.activity == a))
        .collect())
}

async fn health() -> Json<Value> {
    Json(json!({"status": "ok"}))
}

async fn web() -> Html<&'static str> {
    Html(include_str!("../web/index.html"))
}

async fn list_activities(State(st): State<Shared>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    Ok(Json(json!({"activities": load_activities(&conn)?})))
}

#[derive(Deserialize)]
struct NewActivity {
    name: String,
    message: Option<String>,
}

async fn add_activity(State(st): State<Shared>, Json(b): Json<NewActivity>) -> ApiResult {
    let name = norm(&clean(&b.name, "name", 40)?);
    if name.is_empty() {
        return Err(ApiError::invalid("activity name is empty"));
    }
    let message = clean(b.message.as_deref().unwrap_or(""), "message", 500)?;
    let conn = st.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO activity(name, message) VALUES (?1, ?2)
         ON CONFLICT(name) DO UPDATE SET message = excluded.message",
        params![name, message],
    )?;
    Ok(Json(
        json!({"activity": {"name": name, "message": message}}),
    ))
}

async fn list_projects(State(st): State<Shared>, Extension(actor): Extension<Actor>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    Ok(Json(json!({
        "projects": load_projects(&conn)?,
        "default_project": default_project(&conn, &actor.name)?,
    })))
}

/// The project a new entry should start on in the web form: the one this user logged last, else the
/// last one anybody logged, else the newest active project. Archived projects are never suggested.
/// The CLI does not use this; an entry without a project is still refused there.
fn default_project(conn: &Connection, user: &str) -> Result<Option<String>, ApiError> {
    use rusqlite::OptionalExtension;
    const LAST_ENTRY: &str = "SELECT p.name FROM time_entry t JOIN project p ON p.id = t.project_id
         WHERE p.archived = 0 AND t.deleted_at IS NULL AND (?1 IS NULL OR t.created_by = ?1)
         ORDER BY t.created_at DESC, t.id DESC LIMIT 1";
    let last = |by: Option<&str>| {
        conn.query_row(LAST_ENTRY, params![by], |r| r.get(0))
            .optional()
    };
    let newest = || {
        conn.query_row(
            "SELECT name FROM project WHERE archived = 0 ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()
    };
    Ok(match last(Some(user))? {
        Some(p) => Some(p),
        None => match last(None)? {
            Some(p) => Some(p),
            None => newest()?,
        },
    })
}

#[derive(Deserialize)]
struct NewProject {
    name: String,
    #[serde(default)]
    aliases: Vec<String>,
    client: Option<String>,
    rate_type: Option<String>,
    rate: Option<String>,
    currency: Option<String>,
}

/// Validate a rate setting: "none" takes no rate, "hourly" and "fixed" need a positive one.
fn parse_rate(
    rate_type: Option<&str>,
    rate: Option<&str>,
    currency: Option<&str>,
) -> Result<(String, Option<i64>, String), ApiError> {
    let rt = rate_type
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| {
            if rate.is_some_and(|r| !r.trim().is_empty()) {
                "hourly".into()
            } else {
                "none".into()
            }
        });
    let cur = currency
        .map(|c| c.trim().to_uppercase())
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| "SEK".into());
    if cur.len() != 3 || !cur.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(ApiError::invalid(
            "currency must be a three-letter code such as SEK",
        ));
    }
    match rt.as_str() {
        "none" => Ok(("none".into(), None, cur)),
        "hourly" | "fixed" => {
            let amount = oas_core::parse_amount(rate.unwrap_or("")).map_err(ApiError::invalid)?;
            if amount == 0 || amount > 10_000_000_000 {
                return Err(ApiError::invalid("the rate must be above zero"));
            }
            Ok((rt, Some(amount), cur))
        }
        _ => Err(ApiError::invalid("rate_type must be none, hourly or fixed")),
    }
}

async fn add_project(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Json(b): Json<NewProject>,
) -> ApiResult {
    let name = clean(&b.name, "name", 80)?;
    if name.is_empty() {
        return Err(ApiError::invalid("project name is empty"));
    }
    let aliases = b
        .aliases
        .iter()
        .map(|a| clean(a, "alias", 80))
        .collect::<Result<Vec<_>, _>>()?;
    let client = b
        .client
        .as_deref()
        .map(|c| clean(c, "client", 80))
        .transpose()?;
    let has_rate = b.rate_type.as_deref().is_some_and(|t| t != "none")
        || b.rate.as_deref().is_some_and(|r| !r.trim().is_empty());
    if has_rate && actor.role != Role::Admin {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "only an admin can set a project rate",
        ));
    }
    let (rate_type, rate, currency) = parse_rate(
        b.rate_type.as_deref(),
        b.rate.as_deref(),
        b.currency.as_deref(),
    )?;
    let conn = st.conn.lock().unwrap();
    let existing = load_projects(&conn)?;
    if matches!(match_project(&existing, &name), Match::Exact(_)) {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "conflict",
            "project already exists",
        ));
    }
    conn.execute(
        "INSERT INTO project(name, aliases, client, rate_type, rate, currency) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![name, serde_json::to_string(&aliases).unwrap(), client, rate_type, rate, currency],
    )
    .map_err(|_| ApiError::new(StatusCode::CONFLICT, "conflict", "project already exists"))?;
    let id = conn.last_insert_rowid();
    Ok(Json(json!({"project": Project {
        id, name, aliases, client, archived: false, rate_type, rate: rate.map(oas_core::format_amount), currency,
    }})))
}

#[derive(Deserialize)]
struct RateBody {
    rate_type: Option<String>,
    rate: Option<String>,
    currency: Option<String>,
}

/// Set or clear a project's hourly rate or fixed fee. Admin only.
async fn set_project_rate(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Json(b): Json<RateBody>,
) -> ApiResult {
    if actor.role != Role::Admin {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "only an admin can set a project rate",
        ));
    }
    let (rate_type, rate, currency) = parse_rate(
        b.rate_type.as_deref(),
        b.rate.as_deref(),
        b.currency.as_deref(),
    )?;
    let conn = st.conn.lock().unwrap();
    let n = conn.execute(
        "UPDATE project SET rate_type = ?1, rate = ?2, currency = ?3 WHERE id = ?4",
        params![rate_type, rate, currency, id],
    )?;
    if n == 0 {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "no such project",
        ));
    }
    ledger::audit(
        &conn,
        &actor,
        "project.rate",
        &id.to_string(),
        &format!(
            "{rate_type} {}",
            rate.map(oas_core::format_amount).unwrap_or_default()
        ),
    )?;
    let project = load_projects(&conn)?.into_iter().find(|p| p.id == id);
    Ok(Json(json!({"project": project})))
}

#[derive(Deserialize)]
struct LogEntry {
    activity: String,
    project: Option<String>,
    #[serde(default)]
    create_project: bool,
    date: String,
    from: String,
    to: String,
    message: Option<String>,
    timezone: Option<String>,
    /// The entry runs past midnight: an end before the start means the next day.
    #[serde(default)]
    overnight: bool,
    /// Confirms an entry longer than 12 hours.
    #[serde(default)]
    long: bool,
}

/// Hours above which an entry needs an explicit confirmation, so a slip like 11:25 to 00:25
/// (meant as 12:25) is caught instead of booking 13 hours.
const LONG_ENTRY_HOURS: i64 = 12;

/// Start and end of an entry from its local date and times. An end before the start is refused
/// unless the entry is marked overnight, and an entry over 12 hours needs `long`.
fn entry_span(
    date: &str,
    from: &str,
    to: &str,
    tz: Tz,
    overnight: bool,
    long: bool,
) -> Result<(DateTime<Utc>, DateTime<Utc>), ApiError> {
    let start = parse_local(date, from, tz).map_err(ApiError::invalid)?;
    let mut end = parse_local(date, to, tz).map_err(ApiError::invalid)?;
    if end <= start {
        if !overnight {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "end_before_start",
                format!(
                    "the entry ends at {to}, before it starts at {from}; check the times, or mark it overnight if it runs past midnight"
                ),
            ));
        }
        end = end
            .checked_add_signed(chrono::Duration::days(1))
            .ok_or_else(|| ApiError::invalid("invalid end time"))?;
    }
    if end - start > chrono::Duration::hours(LONG_ENTRY_HOURS) && !long {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "long_entry",
            format!(
                "{from} to {to} is {:.1} hours; confirm an entry over {LONG_ENTRY_HOURS} hours explicitly",
                (end - start).num_minutes() as f64 / 60.0
            ),
        ));
    }
    Ok((start, end))
}

async fn log_time(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Json(b): Json<LogEntry>,
) -> ApiResult {
    let tz = tz_of(b.timezone.as_deref())?;
    let (start, end) = entry_span(&b.date, &b.from, &b.to, tz, b.overnight, b.long)?;
    let conn = st.conn.lock().unwrap();
    let activity = check_activity(&conn, &b.activity)?;
    let message = resolve_message(&conn, &activity, b.message.as_deref())?;
    check_overlap(&conn, start, Some(end), None)?;
    let project_id = resolve_project(&conn, b.project.as_deref(), b.create_project)?;
    conn.execute(
        "INSERT INTO time_entry(start_utc, end_utc, tz, activity, project_id, message, created_at, source, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'api', ?8)",
        params![
            utc_string(start),
            utc_string(end),
            tz.name(),
            activity,
            project_id,
            message,
            utc_string(Utc::now()),
            actor.name
        ],
    )?;
    let id = conn.last_insert_rowid();
    let entry = load_entries(&conn)?.into_iter().find(|e| e.id == id);
    Ok(Json(json!({"entry": entry})))
}

#[derive(Deserialize)]
struct StartTimer {
    activity: String,
    project: Option<String>,
    #[serde(default)]
    create_project: bool,
    message: Option<String>,
    timezone: Option<String>,
}

async fn start_timer(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Json(b): Json<StartTimer>,
) -> ApiResult {
    let tz = tz_of(b.timezone.as_deref())?;
    let conn = st.conn.lock().unwrap();
    let running: i64 = conn.query_row(
        "SELECT COUNT(*) FROM time_entry WHERE end_utc IS NULL AND deleted_at IS NULL",
        [],
        |r| r.get(0),
    )?;
    if running > 0 {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "conflict",
            "a timer is already running",
        ));
    }
    let activity = check_activity(&conn, &b.activity)?;
    let message = resolve_message(&conn, &activity, b.message.as_deref())?;
    let now = Utc::now();
    check_overlap(&conn, now, None, None)?;
    let project_id = resolve_project(&conn, b.project.as_deref(), b.create_project)?;
    conn.execute(
        "INSERT INTO time_entry(start_utc, end_utc, tz, activity, project_id, message, created_at, source, created_by)
         VALUES (?1, NULL, ?2, ?3, ?4, ?5, ?6, 'api', ?7)",
        params![utc_string(now), tz.name(), activity, project_id, message, utc_string(now), actor.name],
    )?;
    let id = conn.last_insert_rowid();
    let entry = load_entries(&conn)?.into_iter().find(|e| e.id == id);
    Ok(Json(json!({"entry": entry})))
}

async fn stop_timer(State(st): State<Shared>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    let id: Option<i64> = conn
        .query_row(
            "SELECT id FROM time_entry WHERE end_utc IS NULL AND deleted_at IS NULL",
            [],
            |r| r.get(0),
        )
        .ok();
    let Some(id) = id else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "no timer is running",
        ));
    };
    conn.execute(
        "UPDATE time_entry SET end_utc = ?1 WHERE id = ?2",
        params![utc_string(Utc::now()), id],
    )?;
    let entry = load_entries(&conn)?.into_iter().find(|e| e.id == id);
    Ok(Json(json!({"entry": entry})))
}

#[derive(Deserialize)]
struct EditEntry {
    message: String,
}

/// Change the message of a time entry. Times, project and activity stay as logged.
async fn edit_entry(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Json(b): Json<EditEntry>,
) -> ApiResult {
    let message = clean(&b.message, "message", 500)?;
    let conn = st.conn.lock().unwrap();
    let n = conn.execute(
        "UPDATE time_entry SET message = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![message, id],
    )?;
    if n == 0 {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "no such time entry",
        ));
    }
    ledger::audit(&conn, &actor, "time.message", &id.to_string(), &message)?;
    let entry = load_entries(&conn)?.into_iter().find(|e| e.id == id);
    Ok(Json(json!({"entry": entry})))
}

/// A time entry as stored, for the edit and delete checks.
struct StoredEntry {
    start: DateTime<Utc>,
    end: Option<DateTime<Utc>>,
    tz: Tz,
    activity: String,
    project_id: i64,
    message: String,
    created_by: Option<String>,
}

/// Load an entry that may be changed by this actor: it exists, is not deleted, belongs to the
/// actor (or the actor is an admin), and is not on an invoice.
fn changeable_entry(conn: &Connection, actor: &Actor, id: i64) -> Result<StoredEntry, ApiError> {
    use rusqlite::OptionalExtension;
    let row = conn
        .query_row(
            "SELECT start_utc, end_utc, tz, activity, project_id, message, created_by
             FROM time_entry WHERE id = ?1 AND deleted_at IS NULL",
            params![id],
            |r| {
                let tz: String = r.get(2)?;
                Ok(StoredEntry {
                    start: parse_utc(&r.get::<_, String>(0)?),
                    end: r.get::<_, Option<String>>(1)?.as_deref().map(parse_utc),
                    tz: tz.parse().unwrap_or(chrono_tz::Europe::Stockholm),
                    activity: r.get(3)?,
                    project_id: r.get(4)?,
                    message: r.get(5)?,
                    created_by: r.get(6)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such time entry"))?;
    let owner = row.created_by.as_deref() == Some(actor.name.as_str());
    if !owner && !matches!(actor.role, Role::Admin) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "only the person who logged this entry, or an admin, can change it",
        ));
    }
    let invoiced: i64 = conn.query_row(
        "SELECT COUNT(*) FROM invoiced_entry WHERE entry_id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    if invoiced > 0 {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "invoiced",
            "this entry is on an invoice and cannot be changed; delete the draft invoice first",
        ));
    }
    Ok(row)
}

fn entry_json(conn: &Connection, id: i64) -> Result<Value, ApiError> {
    Ok(json!(load_entries(conn)?.into_iter().find(|e| e.id == id)))
}

#[derive(Deserialize)]
struct EntryPatch {
    date: Option<String>,
    from: Option<String>,
    to: Option<String>,
    activity: Option<String>,
    project: Option<String>,
    message: Option<String>,
    /// Defaults to whether the entry already runs past midnight.
    overnight: Option<bool>,
    /// Defaults to whether the entry is already over 12 hours.
    long: Option<bool>,
}

/// Change any part of a time entry. Every change is written to the audit log with the old and new
/// values.
async fn patch_entry(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Json(b): Json<EntryPatch>,
) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    let old = changeable_entry(&conn, &actor, id)?;
    let before = entry_json(&conn, id)?;
    let local_start = old.start.with_timezone(&old.tz);
    let (mut start, mut end) = (old.start, old.end);
    if b.date.is_some() || b.from.is_some() || b.to.is_some() {
        let date = b
            .date
            .clone()
            .unwrap_or_else(|| local_start.format("%Y-%m-%d").to_string());
        let from = b
            .from
            .clone()
            .unwrap_or_else(|| local_start.format("%H:%M").to_string());
        match (old.end, &b.to) {
            // A running timer keeps running unless an end is given.
            (None, None) => {
                start = parse_local(&date, &from, old.tz).map_err(ApiError::invalid)?;
                if start > Utc::now() {
                    return Err(ApiError::invalid(
                        "a running entry cannot start in the future",
                    ));
                }
            }
            (old_end, to) => {
                let to = match (to, old_end) {
                    (Some(t), _) => t.clone(),
                    (None, Some(e)) => e.with_timezone(&old.tz).format("%H:%M").to_string(),
                    (None, None) => unreachable!("handled above"),
                };
                let was_overnight = old_end.is_some_and(|e| {
                    e.with_timezone(&old.tz).date_naive() > local_start.date_naive()
                });
                let was_long = old_end
                    .is_some_and(|e| e - old.start > chrono::Duration::hours(LONG_ENTRY_HOURS));
                let (s, e) = entry_span(
                    &date,
                    &from,
                    &to,
                    old.tz,
                    b.overnight.unwrap_or(was_overnight),
                    b.long.unwrap_or(was_long),
                )?;
                start = s;
                end = Some(e);
            }
        }
        check_overlap(&conn, start, end, Some(id))?;
    }
    let activity = match &b.activity {
        Some(a) => check_activity(&conn, a)?,
        None => old.activity.clone(),
    };
    let project_id = match &b.project {
        Some(p) => resolve_project(&conn, Some(p), false)?,
        None => old.project_id,
    };
    let message = match &b.message {
        Some(m) => clean(m, "message", 500)?,
        None => old.message.clone(),
    };
    // An edit that changes nothing is not saved and leaves no audit row.
    if (start, end, &activity, project_id, &message)
        == (
            old.start,
            old.end,
            &old.activity,
            old.project_id,
            &old.message,
        )
    {
        return Ok(Json(json!({"entry": before, "changed": false})));
    }
    conn.execute(
        "UPDATE time_entry SET start_utc = ?1, end_utc = ?2, activity = ?3, project_id = ?4, message = ?5
         WHERE id = ?6",
        params![
            utc_string(start),
            end.map(utc_string),
            activity,
            project_id,
            message,
            id
        ],
    )?;
    let after = entry_json(&conn, id)?;
    ledger::audit(
        &conn,
        &actor,
        "time.edit",
        &id.to_string(),
        &json!({"old": before, "new": after}).to_string(),
    )?;
    Ok(Json(json!({"entry": after, "changed": true})))
}

/// Delete a time entry. The row is kept and marked deleted, and the audit log records the entry
/// as it was.
async fn delete_entry(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    changeable_entry(&conn, &actor, id)?;
    let before = entry_json(&conn, id)?;
    conn.execute(
        "UPDATE time_entry SET deleted_at = ?1, deleted_by = ?2 WHERE id = ?3",
        params![utc_string(Utc::now()), actor.name, id],
    )?;
    ledger::audit(
        &conn,
        &actor,
        "time.delete",
        &id.to_string(),
        &json!({"old": before}).to_string(),
    )?;
    Ok(Json(json!({"deleted": id})))
}

#[derive(Deserialize, Default)]
struct IncomeQuery {
    /// Month as YYYY-MM; defaults to the current month in Stockholm.
    month: Option<String>,
}

fn weekdays(from: chrono::NaiveDate, to: chrono::NaiveDate) -> i64 {
    use chrono::Datelike;
    let mut d = from;
    let mut n = 0;
    while d <= to {
        if d.weekday().number_from_monday() <= 5 {
            n += 1;
        }
        d += chrono::Duration::days(1);
    }
    n
}

/// Income for a month, excluding VAT, in SEK: tracked hours at hourly rates, fixed fees (counted in the
/// month of the project's first tracked entry) and invoices not built from tracked time. The projection
/// scales only the hourly part by the share of workdays (Monday to Friday) that have passed.
async fn income(State(st): State<Shared>, Query(q): Query<IncomeQuery>) -> ApiResult {
    use chrono::Datelike;
    let conn = st.conn.lock().unwrap();
    let today = Utc::now()
        .with_timezone(&chrono_tz::Europe::Stockholm)
        .date_naive();
    let first = match q.month.as_deref().filter(|m| !m.trim().is_empty()) {
        Some(m) => chrono::NaiveDate::parse_from_str(&format!("{}-01", m.trim()), "%Y-%m-%d")
            .map_err(|_| ApiError::invalid("month must look like 2026-10"))?,
        None => today.with_day(1).unwrap_or(today),
    };
    let next = if first.month() == 12 {
        chrono::NaiveDate::from_ymd_opt(first.year() + 1, 1, 1)
    } else {
        chrono::NaiveDate::from_ymd_opt(first.year(), first.month() + 1, 1)
    }
    .unwrap_or(first);
    let last = next - chrono::Duration::days(1);
    let (from_s, to_s) = (first.to_string(), last.to_string());

    let all = load_entries(&conn)?;
    let projects = load_projects(&conn)?;
    let mut hourly: i64 = 0;
    let mut hours = 0.0;
    let mut fixed: i64 = 0;
    for e in all.iter().filter(|e| e.date >= from_s && e.date <= to_s) {
        let Some(p) = projects.iter().find(|p| p.name == e.project) else {
            continue;
        };
        if p.currency != "SEK" {
            continue;
        }
        hours += e.hours;
        if p.rate_type == "hourly" {
            hourly += e
                .amount
                .as_deref()
                .and_then(|a| oas_core::parse_amount(a).ok())
                .unwrap_or(0);
        }
    }
    for p in projects
        .iter()
        .filter(|p| p.rate_type == "fixed" && p.currency == "SEK")
    {
        let earliest = all
            .iter()
            .filter(|e| e.project == p.name)
            .map(|e| e.date.as_str())
            .min();
        if earliest.is_some_and(|d| d >= from_s.as_str() && d <= to_s.as_str()) {
            fixed += p
                .rate
                .as_deref()
                .and_then(|a| oas_core::parse_amount(a).ok())
                .unwrap_or(0);
        }
    }
    // Invoices that are not built from tracked time, by invoice date, excluding VAT.
    let other: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM((l.quantity * l.unit_price + 50) / 100), 0)
             FROM invoice_line l JOIN invoice i ON i.id = l.invoice_id
             WHERE i.issue_date >= ?1 AND i.issue_date <= ?2 AND i.currency = 'SEK'
               AND NOT EXISTS (SELECT 1 FROM invoiced_entry x WHERE x.invoice_id = i.id)",
            params![from_s, to_s],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let issued_net: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM((l.quantity * l.unit_price + 50) / 100), 0)
             FROM invoice_line l JOIN invoice i ON i.id = l.invoice_id
             WHERE i.status = 'issued' AND i.issue_date >= ?1 AND i.issue_date <= ?2 AND i.currency = 'SEK'",
            params![from_s, to_s],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let elapsed_end = if today < first {
        first - chrono::Duration::days(1)
    } else {
        today.min(last)
    };
    let elapsed = if elapsed_end < first {
        0
    } else {
        weekdays(first, elapsed_end)
    };
    let total_days = weekdays(first, last);
    let projected_hourly = if elapsed > 0 && elapsed < total_days {
        ((hourly as f64) / elapsed as f64 * total_days as f64).round() as i64
    } else {
        hourly
    };
    let earned = hourly + fixed + other;
    let projected = projected_hourly + fixed + other;
    Ok(Json(json!({
        "month": first.format("%Y-%m").to_string(), "currency": "SEK",
        "excl_vat": true,
        "earned_so_far": oas_core::format_amount(earned),
        "projected": oas_core::format_amount(projected),
        "tracked_hours": (hours * 100.0_f64).round() / 100.0,
        "parts": {
            "hourly": oas_core::format_amount(hourly),
            "fixed_fees": oas_core::format_amount(fixed),
            "other_invoices": oas_core::format_amount(other),
        },
        "invoiced_issued_net": oas_core::format_amount(issued_net),
        "workdays_elapsed": elapsed, "workdays_total": total_days,
        "note": "Excludes VAT. The projection scales only the hourly part by workdays (Monday to Friday) passed; fixed fees and other invoices are counted as they are. It is a rough estimate, not a forecast of what will be paid.",
    })))
}

async fn list_time(State(st): State<Shared>, Query(f): Query<Filters>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    Ok(Json(json!({"entries": filtered(&conn, &f)?})))
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

async fn report(State(st): State<Shared>, Query(f): Query<Filters>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    let entries = filtered(&conn, &f)?;
    let mut by_project: BTreeMap<String, f64> = BTreeMap::new();
    let mut by_activity: BTreeMap<String, f64> = BTreeMap::new();
    let mut by_day: BTreeMap<String, f64> = BTreeMap::new();
    let mut total = 0.0;
    // Amounts per project: hourly projects add up hours times rate; a fixed fee counts once.
    let mut amount_by_project: BTreeMap<String, (i64, String)> = BTreeMap::new();
    {
        let projects = load_projects(&conn)?;
        for e in &entries {
            let Some(p) = projects.iter().find(|p| p.name == e.project) else {
                continue;
            };
            match p.rate_type.as_str() {
                "hourly" => {
                    let a = e
                        .amount
                        .as_deref()
                        .and_then(|a| oas_core::parse_amount(a).ok())
                        .unwrap_or(0);
                    amount_by_project
                        .entry(p.name.clone())
                        .or_insert((0, p.currency.clone()))
                        .0 += a;
                }
                "fixed" => {
                    let fee = p
                        .rate
                        .as_deref()
                        .and_then(|a| oas_core::parse_amount(a).ok())
                        .unwrap_or(0);
                    amount_by_project.insert(p.name.clone(), (fee, p.currency.clone()));
                }
                _ => {}
            }
        }
    }
    let mut totals_by_currency: BTreeMap<String, i64> = BTreeMap::new();
    for (amt, cur) in amount_by_project.values() {
        *totals_by_currency.entry(cur.clone()).or_default() += amt;
    }
    for e in &entries {
        total += e.hours;
        *by_project.entry(e.project.clone()).or_default() += e.hours;
        *by_activity.entry(e.activity.clone()).or_default() += e.hours;
        *by_day.entry(e.date.clone()).or_default() += e.hours;
    }
    let r = |m: BTreeMap<String, f64>| -> BTreeMap<String, f64> {
        m.into_iter().map(|(k, v)| (k, round2(v))).collect()
    };
    Ok(Json(json!({
        "from": f.from, "to": f.to,
        "entries": entries.len(),
        "total_hours": round2(total),
        "by_project": r(by_project),
        "by_activity": r(by_activity),
        "by_day": r(by_day),
        "amount_by_project": amount_by_project.iter().map(|(k, (a, c))| (k.clone(), json!({"amount": oas_core::format_amount(*a), "currency": c}))).collect::<BTreeMap<_, _>>(),
        "total_amount": totals_by_currency.iter().map(|(c, a)| (c.clone(), oas_core::format_amount(*a))).collect::<BTreeMap<_, _>>(),
    })))
}

async fn export_csv(
    State(st): State<Shared>,
    Query(f): Query<Filters>,
) -> Result<Response, ApiError> {
    let conn = st.conn.lock().unwrap();
    let entries = filtered(&conn, &f)?;
    let mut out = String::from(
        "date,start,end,hours,timezone,project,client,activity,message,rate,amount,currency\n",
    );
    for e in entries {
        out.push_str(&csv_row(&[
            e.date,
            e.start,
            e.end.unwrap_or_default(),
            format!("{:.2}", e.hours),
            e.timezone,
            e.project,
            e.client.unwrap_or_default(),
            e.activity,
            e.message,
            e.rate.unwrap_or_default(),
            e.amount.unwrap_or_default(),
            e.currency,
        ]));
        out.push('\n');
    }
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"time-entries.csv\"",
            ),
        ],
        out,
    )
        .into_response())
}

/// The whole HTTP app: the API under /api, health, login, and the web app or fallback page.
fn router(state: Shared, web_dir: Option<&str>) -> Router {
    let api = Router::new()
        .route("/activities", get(list_activities).post(add_activity))
        .route("/projects", get(list_projects).post(add_project))
        .route("/projects/{id}/rate", post(set_project_rate))
        .route("/time", get(list_time).post(log_time))
        .route("/income", get(income))
        .route("/time/start", post(start_timer))
        .route("/time/{id}/message", post(edit_entry))
        .route(
            "/time/{id}",
            axum::routing::patch(patch_entry).delete(delete_entry),
        )
        .route("/time/stop", post(stop_timer))
        .route("/time/report", get(report))
        .route("/time/export.csv", get(export_csv))
        .route("/me", get(ledger::me))
        .route("/logout", post(auth::logout))
        .route("/me/password", post(auth::change_password))
        .route("/users", get(auth::list_users).post(auth::add_user))
        .route("/tokens", get(auth::list_tokens).post(auth::create_token))
        .route("/tokens/{id}/revoke", post(auth::revoke_token))
        .route(
            "/company",
            get(ledger::get_company).post(ledger::set_company),
        )
        .route(
            "/accounts",
            get(ledger::list_accounts).post(ledger::add_account),
        )
        .route("/drafts", get(ledger::list_drafts).post(ledger::propose))
        .route("/drafts/{id}/approve", post(ledger::approve))
        .route("/drafts/{id}/reject", post(ledger::reject))
        .route("/vouchers", get(ledger::list_vouchers))
        .route("/vouchers/{id}", get(ledger::get_voucher))
        .route("/vouchers/{id}/correct", post(ledger::correct))
        .route("/reports/trial-balance", get(ledger::trial_balance))
        .route("/ledger/verify", get(ledger::verify))
        .route("/audit", get(ledger::audit_list))
        .route("/export/sie", get(ledger::export_sie))
        .route(
            "/documents",
            get(inbox::list_documents).post(inbox::upload_document),
        )
        .route("/documents/{id}/file", get(inbox::download_document))
        .route("/documents/{id}/meta", post(inbox::set_meta))
        .route("/bank/import", post(inbox::import_bank))
        .route("/bank/transactions", get(inbox::list_transactions))
        .route("/bank/transactions/{id}/ignore", post(inbox::ignore_tx))
        .route("/bank/transactions/{id}/book", post(inbox::book_tx))
        .route("/bank/match", post(inbox::run_match))
        .route("/vat/config", get(vat::config))
        .route("/vat/report", get(vat::report))
        .route("/vat/boxes/{box}/verify", post(vat::verify_box))
        .route("/vat/boxes/{box}/unused", post(vat::set_not_used))
        .route("/vat/accounts", post(vat::map_account))
        .route(
            "/employees",
            get(payroll::list_employees).post(payroll::add_employee),
        )
        .route(
            "/employees/{id}/deactivate",
            post(payroll::deactivate_employee),
        )
        .route("/payroll/rates", get(payroll::list_rates))
        .route("/payroll/rates/{key}", post(payroll::set_rate))
        .route("/payroll/rates/{key}/verify", post(payroll::verify_rate))
        .route("/payroll/rates/{key}/unused", post(payroll::set_not_used))
        .route(
            "/payroll/runs",
            get(payroll::list_runs).post(payroll::create_run),
        )
        .route("/payroll/runs/{id}", get(payroll::get_run))
        .route("/payroll/runs/{id}/delete", post(payroll::delete_run))
        .route("/payroll/runs/{id}/approve", post(payroll::approve_run))
        .route("/payroll/runs/{id}/agi.xml", get(payroll::agi_xml))
        .route(
            "/customers",
            get(invoices::list_customers).post(invoices::add_customer),
        )
        .route(
            "/invoices",
            get(invoices::list_invoices).post(invoices::create_invoice),
        )
        .route("/invoices/from-time", post(invoices::from_time))
        .route("/invoices/{id}", get(invoices::get_invoice))
        .route("/invoices/{id}/issue", post(invoices::issue_invoice))
        .route("/invoices/{id}/delete", post(invoices::delete_invoice))
        .route("/invoices/{id}/html", get(invoices::invoice_html))
        .route("/matches", get(inbox::list_matches))
        .route("/matches/{id}/confirm", post(inbox::confirm_match))
        .route("/matches/{id}/decline", post(inbox::decline_match))
        .layer(DefaultBodyLimit::max(26 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), auth));

    let has_ui = web_dir.is_some();
    let mut app = Router::new()
        .route("/health", get(health))
        .route("/api/login", post(auth::login))
        .nest("/api", api);
    app = match &web_dir {
        Some(dir) => {
            let index = std::path::Path::new(dir).join("index.html");
            app.fallback_service(
                tower_http::services::ServeDir::new(dir)
                    .fallback(tower_http::services::ServeFile::new(index)),
            )
        }
        None => app.route("/", get(web)),
    };
    app.layer(middleware::from_fn_with_state(has_ui, security_headers))
        .with_state(state)
}

#[tokio::main]
async fn main() {
    let token = std::env::var("OAS_TOKEN").unwrap_or_default();
    if token.len() < 24 {
        eprintln!("OAS_TOKEN must be set to a secret of at least 24 characters");
        std::process::exit(2);
    }
    let db_path = std::env::var("OAS_DB").unwrap_or_else(|_| "oas.db".into());
    let conn = open_db(&db_path).unwrap_or_else(|e| {
        eprintln!("cannot open database {db_path}: {e}");
        std::process::exit(1);
    });
    // Loopback by default, so a local run is not reachable from the network (and Windows does not
    // ask for a firewall rule). The container sets OAS_HOST=0.0.0.0; PORT comes from the host platform.
    let bind = std::env::var("OAS_BIND").unwrap_or_else(|_| {
        format!(
            "{}:{}",
            std::env::var("OAS_HOST").unwrap_or_else(|_| "127.0.0.1".into()),
            std::env::var("PORT").unwrap_or_else(|_| "8080".into())
        )
    });
    let agent_token = std::env::var("OAS_AGENT_TOKEN")
        .ok()
        .filter(|t| !t.is_empty());
    if let Some(t) = &agent_token
        && (t.len() < 24 || *t == token)
    {
        eprintln!("OAS_AGENT_TOKEN must be at least 24 characters and differ from OAS_TOKEN");
        std::process::exit(2);
    }
    // Receipts live next to the database unless OAS_DATA_DIR says otherwise.
    let data_dir = std::env::var("OAS_DATA_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::Path::new(&db_path)
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_default()
        });
    let docs_dir = data_dir.join("documents");
    let dummy_hash = auth::hash_password("dummy password used only for timing").unwrap_or_default();
    let state: Shared = Arc::new(AppState {
        conn: Mutex::new(conn),
        token,
        agent_token,
        limiter: auth::Limiter::default(),
        dummy_hash,
        docs_dir,
    });

    // OAS_WEB_DIR points at the built web app (web/dist). Without it a small fallback page is served.
    let web_dir = std::env::var("OAS_WEB_DIR")
        .ok()
        .filter(|d| std::path::Path::new(d).join("index.html").is_file());
    let app = router(state, web_dir.as_deref());

    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .unwrap_or_else(|e| {
            eprintln!("cannot bind {bind}: {e}");
            std::process::exit(1);
        });
    eprintln!(
        "oas-server listening on {bind}, database {db_path}, web app {}",
        web_dir.as_deref().unwrap_or("fallback page")
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .unwrap();
}

#[cfg(test)]
mod tests {
    //! API tests against an in-memory database, through the same router the server runs.
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    const ADMIN: &str = "test-admin-token-0123456789abcdef";
    const AGENT: &str = "test-agent-token-0123456789abcdef";

    fn app() -> Router {
        let state: Shared = Arc::new(AppState {
            conn: Mutex::new(open_db(":memory:").unwrap()),
            token: ADMIN.into(),
            agent_token: Some(AGENT.into()),
            limiter: auth::Limiter::default(),
            dummy_hash: auth::hash_password("dummy").unwrap(),
            docs_dir: std::env::temp_dir().join("oas-server-tests"),
        });
        router(state, None)
    }

    async fn call(
        app: &Router,
        token: Option<&str>,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> (u16, Value) {
        let mut req = Request::builder().method(method).uri(path);
        if let Some(t) = token {
            req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
        }
        let req = match body {
            Some(b) => req
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(b.to_string())),
            None => req.body(Body::empty()),
        }
        .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        let status = res.status().as_u16();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    fn reverse_charge_voucher() -> Value {
        json!({
            "event_date": "2026-07-15",
            "description": "AWS July",
            "counterparty": "Amazon Web Services",
            "lines": [
                {"account": 6540, "debit": "1000", "vat_code": "RCEU"},
                {"account": 2645, "debit": "250"},
                {"account": 2614, "credit": "250"},
                {"account": 1930, "credit": "1000"},
            ],
        })
    }

    #[tokio::test]
    async fn the_api_needs_a_token() {
        let app = app();
        assert_eq!(call(&app, None, "GET", "/api/projects", None).await.0, 401);
        assert_eq!(
            call(
                &app,
                Some("wrong-token-wrong-token-wrong"),
                "GET",
                "/api/projects",
                None
            )
            .await
            .0,
            401
        );
        assert_eq!(
            call(&app, Some(ADMIN), "GET", "/api/projects", None)
                .await
                .0,
            200
        );
    }

    #[tokio::test]
    async fn agents_propose_but_only_admins_approve() {
        let app = app();
        let (status, draft) = call(
            &app,
            Some(AGENT),
            "POST",
            "/api/drafts",
            Some(reverse_charge_voucher()),
        )
        .await;
        assert_eq!(status, 200, "{draft}");
        let id = draft["draft"]["id"]
            .as_i64()
            .or(draft["id"].as_i64())
            .expect("draft id");
        let approve = format!("/api/drafts/{id}/approve");
        assert_eq!(call(&app, Some(AGENT), "POST", &approve, None).await.0, 403);
        let (status, posted) = call(&app, Some(ADMIN), "POST", &approve, None).await;
        assert_eq!(status, 200, "{posted}");
        let (_, verify) = call(&app, Some(ADMIN), "GET", "/api/ledger/verify", None).await;
        assert_eq!(verify["ok"], true, "{verify}");
        assert_eq!(verify["vouchers"], 1);
    }

    #[tokio::test]
    async fn an_unbalanced_voucher_is_refused() {
        let app = app();
        let body = json!({
            "event_date": "2026-07-16",
            "description": "x",
            "lines": [{"account": 6540, "debit": "10"}, {"account": 1930, "credit": "9"}],
        });
        let (status, err) = call(&app, Some(ADMIN), "POST", "/api/drafts", Some(body)).await;
        assert_eq!(status, 400, "{err}");
        assert_eq!(err["error"]["code"], "invalid_input");
    }

    #[tokio::test]
    async fn reverse_charge_lands_in_boxes_21_30_and_48() {
        let app = app();
        let (_, draft) = call(
            &app,
            Some(ADMIN),
            "POST",
            "/api/drafts",
            Some(reverse_charge_voucher()),
        )
        .await;
        let id = draft["draft"]["id"]
            .as_i64()
            .or(draft["id"].as_i64())
            .unwrap();
        call(
            &app,
            Some(ADMIN),
            "POST",
            &format!("/api/drafts/{id}/approve"),
            None,
        )
        .await;
        let (status, report) = call(
            &app,
            Some(ADMIN),
            "GET",
            "/api/vat/report?quarter=2026-Q3",
            None,
        )
        .await;
        assert_eq!(status, 200, "{report}");
        let amount = |b: &str| {
            report["boxes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|x| x["box"] == b)
                .unwrap()["amount"]
                .clone()
        };
        assert_eq!(amount("21"), "1000.00");
        assert_eq!(amount("30"), "250.00");
        assert_eq!(amount("48"), "250.00");
        assert_eq!(report["ready_to_file"], false, "boxes start unverified");
    }

    #[tokio::test]
    async fn users_sign_in_with_a_password_and_agents_cannot_add_users() {
        let app = app();
        let user =
            json!({"username": "astrid", "password": "a long enough password", "role": "admin"});
        assert_eq!(
            call(&app, Some(AGENT), "POST", "/api/users", Some(user.clone()))
                .await
                .0,
            403
        );
        assert_eq!(
            call(&app, Some(ADMIN), "POST", "/api/users", Some(user))
                .await
                .0,
            200
        );
        let bad = json!({"username": "astrid", "password": "not the password", "ttl_hours": 1});
        assert_eq!(
            call(&app, None, "POST", "/api/login", Some(bad)).await.0,
            401
        );
        let good =
            json!({"username": "astrid", "password": "a long enough password", "ttl_hours": 1});
        let (status, session) = call(&app, None, "POST", "/api/login", Some(good)).await;
        assert_eq!(status, 200, "{session}");
        let token = session["token"].as_str().unwrap();
        let (status, me) = call(&app, Some(token), "GET", "/api/me", None).await;
        assert_eq!(status, 200);
        assert_eq!(me["username"], "astrid");
    }

    #[tokio::test]
    async fn the_form_defaults_to_each_users_last_project() {
        let app = app();
        let default = |token: &'static str| {
            let app = app.clone();
            async move {
                call(&app, Some(token), "GET", "/api/projects", None)
                    .await
                    .1["default_project"]
                    .clone()
            }
        };
        assert_eq!(default(ADMIN).await, Value::Null, "no projects yet");
        for name in ["Acme", "Beta", "Gamma"] {
            call(
                &app,
                Some(ADMIN),
                "POST",
                "/api/projects",
                Some(json!({"name": name})),
            )
            .await;
        }
        assert_eq!(
            default(ADMIN).await,
            "Gamma",
            "no entries: the newest project"
        );
        let log = |token: &'static str, project: &'static str, from: &'static str| {
            let app = app.clone();
            async move {
                let body = json!({"activity": "meeting", "project": project, "date": "2026-10-21",
                    "from": from, "to": format!("{}:30", &from[..2])});
                assert_eq!(
                    call(&app, Some(token), "POST", "/api/time", Some(body))
                        .await
                        .0,
                    200
                );
            }
        };
        log(ADMIN, "Acme", "09:00").await;
        log(AGENT, "Beta", "11:00").await;
        assert_eq!(default(ADMIN).await, "Acme", "the admin's own last entry");
        assert_eq!(default(AGENT).await, "Beta", "the agent's own last entry");
    }

    async fn log(app: &Router, token: &str, body: Value) -> (u16, Value) {
        call(app, Some(token), "POST", "/api/time", Some(body)).await
    }

    fn entry(project: &str, date: &str, from: &str, to: &str) -> Value {
        json!({"activity": "meeting", "project": project, "date": date, "from": from, "to": to})
    }

    async fn app_with_project(name: &str) -> Router {
        let app = app();
        let (status, _) = call(
            &app,
            Some(ADMIN),
            "POST",
            "/api/projects",
            Some(json!({"name": name})),
        )
        .await;
        assert_eq!(status, 200);
        app
    }

    async fn audit_actions(app: &Router) -> Vec<(String, String)> {
        let (_, body) = call(app, Some(ADMIN), "GET", "/api/audit", None).await;
        let rows = body
            .as_object()
            .and_then(|o| o.values().find_map(|v| v.as_array()))
            .cloned()
            .unwrap_or_default();
        rows.iter()
            .map(|r| {
                (
                    r["action"].as_str().unwrap_or("").to_string(),
                    r["detail"].as_str().unwrap_or("").to_string(),
                )
            })
            .collect()
    }

    #[tokio::test]
    async fn an_end_before_the_start_is_refused_unless_marked_overnight() {
        let app = app_with_project("MOHV").await;
        // The slip that booked 13 hours: 11:25 to 00:25, meant as 12:25.
        let (status, err) = log(&app, ADMIN, entry("MOHV", "2026-10-26", "11:25", "00:25")).await;
        assert_eq!(status, 400, "{err}");
        assert_eq!(err["error"]["code"], "end_before_start");
        // Marked overnight it is still 13 hours, which needs its own confirmation.
        let mut overnight = entry("MOHV", "2026-10-26", "11:25", "00:25");
        overnight["overnight"] = json!(true);
        let (status, err) = log(&app, ADMIN, overnight.clone()).await;
        assert_eq!(status, 400, "{err}");
        assert_eq!(err["error"]["code"], "long_entry");
        // A real night shift goes through.
        let mut shift = entry("MOHV", "2026-10-20", "22:00", "02:00");
        shift["overnight"] = json!(true);
        let (status, body) = log(&app, ADMIN, shift).await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(body["entry"]["hours"], 4.0);
    }

    #[tokio::test]
    async fn a_same_day_entry_over_twelve_hours_needs_confirming() {
        let app = app_with_project("MOHV").await;
        let (status, err) = log(&app, ADMIN, entry("MOHV", "2026-10-21", "06:00", "19:00")).await;
        assert_eq!(status, 400, "{err}");
        assert_eq!(err["error"]["code"], "long_entry");
        let mut long = entry("MOHV", "2026-10-21", "06:00", "19:00");
        long["long"] = json!(true);
        assert_eq!(log(&app, ADMIN, long).await.0, 200);
    }

    #[tokio::test]
    async fn an_entry_can_be_corrected_and_the_change_is_audited() {
        let app = app_with_project("MOHV").await;
        call(
            &app,
            Some(ADMIN),
            "POST",
            "/api/projects",
            Some(json!({"name": "Intern"})),
        )
        .await;
        let (_, body) = log(&app, ADMIN, entry("MOHV", "2026-10-26", "11:25", "12:00")).await;
        let id = body["entry"]["id"].as_i64().unwrap();
        // An end before the start is refused on edit too.
        let (status, err) = call(
            &app,
            Some(ADMIN),
            "PATCH",
            &format!("/api/time/{id}"),
            Some(json!({"to": "00:25"})),
        )
        .await;
        assert_eq!(status, 400, "{err}");
        let patch =
            json!({"to": "12:25", "project": "Intern", "activity": "building", "message": "Fixed"});
        let (status, body) = call(
            &app,
            Some(ADMIN),
            "PATCH",
            &format!("/api/time/{id}"),
            Some(patch),
        )
        .await;
        assert_eq!(status, 200, "{body}");
        let e = &body["entry"];
        assert_eq!(
            (e["start"].as_str(), e["end"].as_str()),
            (Some("11:25"), Some("12:25"))
        );
        assert_eq!(
            (e["project"].as_str(), e["activity"].as_str()),
            (Some("Intern"), Some("building"))
        );
        assert_eq!(e["message"], "Fixed");
        assert_eq!(e["hours"], 1.0);
        // Repeating the same edit changes nothing and adds no audit row.
        let same =
            json!({"to": "12:25", "project": "Intern", "activity": "building", "message": "Fixed"});
        let (status, body) = call(
            &app,
            Some(ADMIN),
            "PATCH",
            &format!("/api/time/{id}"),
            Some(same),
        )
        .await;
        assert_eq!((status, &body["changed"]), (200, &json!(false)), "{body}");
        let audit = audit_actions(&app).await;
        assert_eq!(audit.iter().filter(|(a, _)| a == "time.edit").count(), 1);
        let edit = audit
            .iter()
            .find(|(a, _)| a == "time.edit")
            .expect("an audit row for the edit");
        let detail: Value = serde_json::from_str(&edit.1).unwrap();
        assert_eq!(detail["old"]["end"], "12:00");
        assert_eq!(detail["new"]["end"], "12:25");
    }

    #[tokio::test]
    async fn an_edit_cannot_overlap_another_entry() {
        let app = app_with_project("MOHV").await;
        log(&app, ADMIN, entry("MOHV", "2026-10-26", "09:00", "10:00")).await;
        let (_, body) = log(&app, ADMIN, entry("MOHV", "2026-10-26", "11:00", "12:00")).await;
        let id = body["entry"]["id"].as_i64().unwrap();
        let (status, _) = call(
            &app,
            Some(ADMIN),
            "PATCH",
            &format!("/api/time/{id}"),
            Some(json!({"from": "09:30"})),
        )
        .await;
        assert_eq!(status, 409);
        // Moving within its own old slot is fine.
        let (status, body) = call(
            &app,
            Some(ADMIN),
            "PATCH",
            &format!("/api/time/{id}"),
            Some(json!({"from": "11:15"})),
        )
        .await;
        assert_eq!(status, 200, "{body}");
    }

    #[tokio::test]
    async fn only_the_owner_or_an_admin_can_change_an_entry() {
        let app = app_with_project("MOHV").await;
        let (_, body) = log(&app, ADMIN, entry("MOHV", "2026-10-26", "09:00", "10:00")).await;
        let admins = body["entry"]["id"].as_i64().unwrap();
        let (_, body) = log(&app, AGENT, entry("MOHV", "2026-10-26", "10:00", "11:00")).await;
        let agents = body["entry"]["id"].as_i64().unwrap();
        let msg = Some(json!({"message": "x"}));
        assert_eq!(
            call(
                &app,
                Some(AGENT),
                "PATCH",
                &format!("/api/time/{admins}"),
                msg.clone()
            )
            .await
            .0,
            403
        );
        assert_eq!(
            call(
                &app,
                Some(AGENT),
                "DELETE",
                &format!("/api/time/{admins}"),
                None
            )
            .await
            .0,
            403
        );
        assert_eq!(
            call(
                &app,
                Some(AGENT),
                "PATCH",
                &format!("/api/time/{agents}"),
                msg.clone()
            )
            .await
            .0,
            200
        );
        assert_eq!(
            call(
                &app,
                Some(ADMIN),
                "PATCH",
                &format!("/api/time/{agents}"),
                msg
            )
            .await
            .0,
            200
        );
    }

    #[tokio::test]
    async fn a_deleted_entry_is_gone_everywhere_but_the_audit_log() {
        let app = app_with_project("MOHV").await;
        let (_, body) = log(&app, ADMIN, entry("MOHV", "2026-10-26", "11:25", "12:25")).await;
        let id = body["entry"]["id"].as_i64().unwrap();
        let (status, body) = call(
            &app,
            Some(ADMIN),
            "DELETE",
            &format!("/api/time/{id}"),
            None,
        )
        .await;
        assert_eq!(status, 200, "{body}");
        let (_, list) = call(
            &app,
            Some(ADMIN),
            "GET",
            "/api/time?from=2026-10-01&to=2026-10-31",
            None,
        )
        .await;
        assert_eq!(list["entries"].as_array().unwrap().len(), 0);
        let (_, report) = call(
            &app,
            Some(ADMIN),
            "GET",
            "/api/time/report?from=2026-10-01&to=2026-10-31",
            None,
        )
        .await;
        assert_eq!(report["total_hours"], 0.0);
        // The slot is free again, and a second delete finds nothing.
        assert_eq!(
            log(&app, ADMIN, entry("MOHV", "2026-10-26", "11:25", "12:25"))
                .await
                .0,
            200
        );
        assert_eq!(
            call(
                &app,
                Some(ADMIN),
                "DELETE",
                &format!("/api/time/{id}"),
                None
            )
            .await
            .0,
            404
        );
        let audit = audit_actions(&app).await;
        let del = audit
            .iter()
            .find(|(a, _)| a == "time.delete")
            .expect("an audit row for the delete");
        assert!(del.1.contains("\"start\":\"11:25\""), "{}", del.1);
    }

    #[tokio::test]
    async fn an_invoiced_entry_cannot_be_changed_or_deleted() {
        let app = app();
        let project = json!({"name": "MOHV", "rate_type": "hourly", "rate": "900"});
        assert_eq!(
            call(&app, Some(ADMIN), "POST", "/api/projects", Some(project))
                .await
                .0,
            200
        );
        let (_, body) = log(&app, ADMIN, entry("MOHV", "2026-10-21", "11:00", "12:00")).await;
        let id = body["entry"]["id"].as_i64().unwrap();
        let (_, customer) = call(
            &app,
            Some(ADMIN),
            "POST",
            "/api/customers",
            Some(json!({"name": "Kund AB", "address": "Gatan 1"})),
        )
        .await;
        let customer_id = customer["customer"]["id"].as_i64().expect("customer id");
        let inv = json!({"customer_id": customer_id, "project": "MOHV", "from": "2026-10-01", "to": "2026-10-31"});
        let (status, body) = call(
            &app,
            Some(ADMIN),
            "POST",
            "/api/invoices/from-time",
            Some(inv),
        )
        .await;
        assert_eq!(status, 200, "{body}");
        let (status, err) = call(
            &app,
            Some(ADMIN),
            "PATCH",
            &format!("/api/time/{id}"),
            Some(json!({"to": "12:30"})),
        )
        .await;
        assert_eq!(status, 409, "{err}");
        assert_eq!(err["error"]["code"], "invoiced");
        assert_eq!(
            call(
                &app,
                Some(ADMIN),
                "DELETE",
                &format!("/api/time/{id}"),
                None
            )
            .await
            .0,
            409
        );
    }

    #[test]
    fn starter_accounts_are_spelled_in_swedish_and_old_spellings_are_updated() {
        let conn = open_db(":memory:").unwrap();
        let name = |n: i64| -> String {
            conn.query_row("SELECT name FROM account WHERE number = ?1", [n], |r| {
                r.get(0)
            })
            .unwrap()
        };
        assert_eq!(name(7210), "Löner till tjänstemän");
        // A book from before the change keeps the ASCII name until the next start; a renamed account stays as it is.
        conn.execute(
            "UPDATE account SET name = 'Loner till tjansteman' WHERE number = 7210",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE account SET name = 'Huvudkonto SEB' WHERE number = 1930",
            [],
        )
        .unwrap();
        ledger::seed(&conn).unwrap();
        assert_eq!(name(7210), "Löner till tjänstemän");
        assert_eq!(name(1930), "Huvudkonto SEB");
    }
}
