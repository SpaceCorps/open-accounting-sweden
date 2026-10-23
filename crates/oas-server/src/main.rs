//! open-accounting-sweden server: time tracking API and a small web panel.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{HeaderValue, Request, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
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

struct AppState {
    conn: Mutex<Connection>,
    token: String,
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
        Self { status, code, message: message.into(), projects: vec![] }
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
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", "database error")
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
  archived INTEGER NOT NULL DEFAULT 0
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

async fn auth(State(st): State<Shared>, req: Request<Body>, next: Next) -> Response {
    let given = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    let ok: bool = given.as_bytes().ct_eq(st.token.as_bytes()).into();
    if ok {
        next.run(req).await
    } else {
        ApiError::new(StatusCode::UNAUTHORIZED, "auth_required", "missing or invalid token")
            .into_response()
    }
}

async fn security_headers(req: Request<Body>, next: Next) -> Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    h.insert("x-frame-options", HeaderValue::from_static("DENY"));
    h.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    h.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'self'; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'",
        ),
    );
    res
}

fn load_projects(conn: &Connection) -> Result<Vec<Project>, ApiError> {
    let mut stmt =
        conn.prepare("SELECT id, name, aliases, client, archived FROM project ORDER BY name")?;
    let rows = stmt.query_map([], |r| {
        let aliases: String = r.get(2)?;
        Ok(Project {
            id: r.get(0)?,
            name: r.get(1)?,
            aliases: serde_json::from_str(&aliases).unwrap_or_default(),
            client: r.get(3)?,
            archived: r.get::<_, i64>(4)? != 0,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn load_activities(conn: &Connection) -> Result<Vec<Activity>, ApiError> {
    let mut stmt = conn.prepare("SELECT name, message FROM activity ORDER BY name")?;
    let rows = stmt.query_map([], |r| Ok(Activity { name: r.get(0)?, message: r.get(1)? }))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn names(projects: &[Project], idx: &[usize]) -> Vec<String> {
    idx.iter().map(|&i| projects[i].name.clone()).collect()
}

/// Resolve a typed project name to a project id, creating it only when asked to.
fn resolve_project(
    conn: &Connection,
    given: Option<&str>,
    create: bool,
) -> Result<i64, ApiError> {
    let projects = load_projects(conn)?;
    let active: Vec<String> =
        projects.iter().filter(|p| !p.archived).map(|p| p.name.clone()).collect();
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
    DateTime::parse_from_rfc3339(s).map(|d| d.with_timezone(&Utc)).unwrap_or_default()
}

fn clean(value: &str, field: &str, max: usize) -> Result<String, ApiError> {
    let v = value.trim();
    if v.chars().count() > max || v.chars().any(|c| c.is_control() && c != '\n') {
        return Err(ApiError::invalid(format!("{field} is too long or has control characters")));
    }
    Ok(v.to_string())
}

fn resolve_message(conn: &Connection, activity: &str, message: Option<&str>) -> Result<String, ApiError> {
    match message.map(str::trim).filter(|m| !m.is_empty()) {
        Some(m) => clean(m, "message", 500),
        None => {
            let template: String = conn
                .query_row("SELECT message FROM activity WHERE name = ?1", params![activity], |r| {
                    r.get(0)
                })
                .unwrap_or_default();
            // The {topic} placeholder is only filled when a message is given.
            Ok(template.replace("{topic}", "").trim().trim_end_matches(':').trim().to_string())
        }
    }
}

fn check_activity(conn: &Connection, name: &str) -> Result<String, ApiError> {
    let name = norm(name);
    let found: Option<String> = conn
        .query_row("SELECT name FROM activity WHERE name = ?1", params![name], |r| r.get(0))
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
) -> Result<(), ApiError> {
    let end_s = end.map(utc_string).unwrap_or_else(|| "9999-12-31T00:00:00Z".into());
    let clash: i64 = conn.query_row(
        "SELECT COUNT(*) FROM time_entry
         WHERE start_utc < ?2 AND COALESCE(end_utc, '9999-12-31T00:00:00Z') > ?1",
        params![utc_string(start), end_s],
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
        "SELECT e.id, e.start_utc, e.end_utc, e.tz, e.activity, e.message, p.name, p.client
         FROM time_entry e JOIN project p ON p.id = e.project_id
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
        Ok(TimeEntry {
            id: r.get(0)?,
            date: ls.format("%Y-%m-%d").to_string(),
            start: ls.format("%H:%M").to_string(),
            end: end.map(|e| e.with_timezone(&tz).format("%H:%M").to_string()),
            hours: hours_between(start, end.unwrap_or(now)),
            timezone: tz_name,
            project: r.get(6)?,
            client: r.get(7)?,
            activity: r.get(4)?,
            message: r.get(5)?,
            running,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[derive(Deserialize, Default)]
struct Filters {
    from: Option<String>,
    to: Option<String>,
    project: Option<String>,
    activity: Option<String>,
}

fn filtered(conn: &Connection, f: &Filters) -> Result<Vec<TimeEntry>, ApiError> {
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
    Ok(Json(json!({"activity": {"name": name, "message": message}})))
}

async fn list_projects(State(st): State<Shared>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    Ok(Json(json!({"projects": load_projects(&conn)?})))
}

#[derive(Deserialize)]
struct NewProject {
    name: String,
    #[serde(default)]
    aliases: Vec<String>,
    client: Option<String>,
}

async fn add_project(State(st): State<Shared>, Json(b): Json<NewProject>) -> ApiResult {
    let name = clean(&b.name, "name", 80)?;
    if name.is_empty() {
        return Err(ApiError::invalid("project name is empty"));
    }
    let aliases = b
        .aliases
        .iter()
        .map(|a| clean(a, "alias", 80))
        .collect::<Result<Vec<_>, _>>()?;
    let client = b.client.as_deref().map(|c| clean(c, "client", 80)).transpose()?;
    let conn = st.conn.lock().unwrap();
    let existing = load_projects(&conn)?;
    if matches!(match_project(&existing, &name), Match::Exact(_)) {
        return Err(ApiError::new(StatusCode::CONFLICT, "conflict", "project already exists"));
    }
    conn.execute(
        "INSERT INTO project(name, aliases, client) VALUES (?1, ?2, ?3)",
        params![name, serde_json::to_string(&aliases).unwrap(), client],
    )
    .map_err(|_| ApiError::new(StatusCode::CONFLICT, "conflict", "project already exists"))?;
    let id = conn.last_insert_rowid();
    Ok(Json(json!({"project": Project { id, name, aliases, client, archived: false }})))
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
}

async fn log_time(State(st): State<Shared>, Json(b): Json<LogEntry>) -> ApiResult {
    let tz = tz_of(b.timezone.as_deref())?;
    let start = parse_local(&b.date, &b.from, tz).map_err(ApiError::invalid)?;
    let mut end = parse_local(&b.date, &b.to, tz).map_err(ApiError::invalid)?;
    if end <= start {
        end = parse_local(&b.date, &b.to, tz)
            .map_err(ApiError::invalid)?
            .checked_add_signed(chrono::Duration::days(1))
            .ok_or_else(|| ApiError::invalid("invalid end time"))?;
        if end - start > chrono::Duration::hours(16) {
            return Err(ApiError::invalid("end must be after start"));
        }
    }
    let conn = st.conn.lock().unwrap();
    let activity = check_activity(&conn, &b.activity)?;
    let message = resolve_message(&conn, &activity, b.message.as_deref())?;
    check_overlap(&conn, start, Some(end))?;
    let project_id = resolve_project(&conn, b.project.as_deref(), b.create_project)?;
    conn.execute(
        "INSERT INTO time_entry(start_utc, end_utc, tz, activity, project_id, message, created_at, source)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'api')",
        params![
            utc_string(start),
            utc_string(end),
            tz.name(),
            activity,
            project_id,
            message,
            utc_string(Utc::now())
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

async fn start_timer(State(st): State<Shared>, Json(b): Json<StartTimer>) -> ApiResult {
    let tz = tz_of(b.timezone.as_deref())?;
    let conn = st.conn.lock().unwrap();
    let running: i64 =
        conn.query_row("SELECT COUNT(*) FROM time_entry WHERE end_utc IS NULL", [], |r| r.get(0))?;
    if running > 0 {
        return Err(ApiError::new(StatusCode::CONFLICT, "conflict", "a timer is already running"));
    }
    let activity = check_activity(&conn, &b.activity)?;
    let message = resolve_message(&conn, &activity, b.message.as_deref())?;
    let now = Utc::now();
    check_overlap(&conn, now, None)?;
    let project_id = resolve_project(&conn, b.project.as_deref(), b.create_project)?;
    conn.execute(
        "INSERT INTO time_entry(start_utc, end_utc, tz, activity, project_id, message, created_at, source)
         VALUES (?1, NULL, ?2, ?3, ?4, ?5, ?6, 'api')",
        params![utc_string(now), tz.name(), activity, project_id, message, utc_string(now)],
    )?;
    let id = conn.last_insert_rowid();
    let entry = load_entries(&conn)?.into_iter().find(|e| e.id == id);
    Ok(Json(json!({"entry": entry})))
}

async fn stop_timer(State(st): State<Shared>) -> ApiResult {
    let conn = st.conn.lock().unwrap();
    let id: Option<i64> = conn
        .query_row("SELECT id FROM time_entry WHERE end_utc IS NULL", [], |r| r.get(0))
        .ok();
    let Some(id) = id else {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "not_found", "no timer is running"));
    };
    conn.execute(
        "UPDATE time_entry SET end_utc = ?1 WHERE id = ?2",
        params![utc_string(Utc::now()), id],
    )?;
    let entry = load_entries(&conn)?.into_iter().find(|e| e.id == id);
    Ok(Json(json!({"entry": entry})))
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
    })))
}

async fn export_csv(State(st): State<Shared>, Query(f): Query<Filters>) -> Result<Response, ApiError> {
    let conn = st.conn.lock().unwrap();
    let entries = filtered(&conn, &f)?;
    let mut out = String::from("date,start,end,hours,timezone,project,client,activity,message\n");
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
        ]));
        out.push('\n');
    }
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (header::CONTENT_DISPOSITION, "attachment; filename=\"time-entries.csv\""),
        ],
        out,
    )
        .into_response())
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
    let bind = std::env::var("OAS_BIND").unwrap_or_else(|_| {
        format!("0.0.0.0:{}", std::env::var("PORT").unwrap_or_else(|_| "8080".into()))
    });
    let state: Shared = Arc::new(AppState { conn: Mutex::new(conn), token });

    let api = Router::new()
        .route("/activities", get(list_activities).post(add_activity))
        .route("/projects", get(list_projects).post(add_project))
        .route("/time", get(list_time).post(log_time))
        .route("/time/start", post(start_timer))
        .route("/time/stop", post(stop_timer))
        .route("/time/report", get(report))
        .route("/time/export.csv", get(export_csv))
        .layer(middleware::from_fn_with_state(state.clone(), auth));

    let app = Router::new()
        .route("/", get(web))
        .route("/health", get(health))
        .nest("/api", api)
        .layer(middleware::from_fn(security_headers))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&bind).await.unwrap_or_else(|e| {
        eprintln!("cannot bind {bind}: {e}");
        std::process::exit(1);
    });
    eprintln!("oas-server listening on {bind}, database {db_path}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .unwrap();
}
