//! Username and password accounts with expiring sessions.
//!
//! Passwords are hashed with Argon2id. A successful login returns a random session token
//! that is stored only as a SHA-256 hash and works as a bearer token for the CLI and web panel.
//! Failed logins are rate limited per username and globally.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rand_core::{OsRng, RngCore};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::State;
use axum::http::StatusCode;
use axum::{Extension, Json};
use chrono::{Duration as ChronoDuration, Utc};
use oas_core::{norm, utc_string};
use rusqlite::{Connection, params};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{Actor, ApiError, ApiResult, Role, Shared};

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS app_user(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  username TEXT NOT NULL UNIQUE COLLATE NOCASE,
  password_hash TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('admin', 'agent')),
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS session(
  token_hash TEXT PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES app_user(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS api_token(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id INTEGER NOT NULL REFERENCES app_user(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  token_hash TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL,
  last_used_at TEXT
);
";

const WINDOW: Duration = Duration::from_secs(15 * 60);
const MAX_PER_USER: u32 = 5;
const MAX_GLOBAL: u32 = 60;

#[derive(Default)]
pub struct Limiter(Mutex<HashMap<String, (u32, Instant)>>);

impl Limiter {
    fn blocked(&self, key: &str, max: u32) -> bool {
        let mut m = self.0.lock().unwrap();
        match m.get(key) {
            Some((n, since)) if since.elapsed() < WINDOW => *n >= max,
            Some(_) => {
                m.remove(key);
                false
            }
            None => false,
        }
    }
    fn fail(&self, key: &str) {
        let mut m = self.0.lock().unwrap();
        let e = m.entry(key.to_string()).or_insert((0, Instant::now()));
        if e.1.elapsed() >= WINDOW {
            *e = (0, Instant::now());
        }
        e.0 += 1;
    }
    fn clear(&self, key: &str) {
        self.0.lock().unwrap().remove(key);
    }
}

pub fn hash_password(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", "could not hash password"))
}

fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|h| Argon2::default().verify_password(password.as_bytes(), &h).is_ok())
        .unwrap_or(false)
}

pub fn token_hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

/// Look up a session token and return the actor it belongs to.
pub fn actor_for_session(conn: &Connection, token: &str) -> Option<Actor> {
    let hash = token_hash(token);
    conn.query_row(
        "SELECT u.id, u.username, u.role FROM session s JOIN app_user u ON u.id = s.user_id
         WHERE s.token_hash = ?1 AND s.expires_at > ?2",
        params![hash, utc_string(Utc::now())],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
    )
    .ok()
    .map(|(id, name, role)| Actor {
        role: if role == "admin" { Role::Admin } else { Role::Agent },
        name,
        user_id: Some(id),
        session: Some(hash),
    })
}

/// Look up a permanent access token ("oas_" prefix) and return the actor it belongs to.
pub fn actor_for_pat(conn: &Connection, token: &str) -> Option<Actor> {
    let hash = token_hash(token);
    let found = conn
        .query_row(
            "SELECT t.id, u.id, u.username, u.role FROM api_token t JOIN app_user u ON u.id = t.user_id
             WHERE t.token_hash = ?1",
            params![hash],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)),
        )
        .ok()?;
    let _ = conn.execute(
        "UPDATE api_token SET last_used_at = ?1 WHERE id = ?2",
        params![utc_string(Utc::now()), found.0],
    );
    Some(Actor {
        role: if found.3 == "admin" { Role::Admin } else { Role::Agent },
        name: found.2,
        user_id: Some(found.1),
        session: None,
    })
}

fn random_hex() -> String {
    let mut raw = [0u8; 32];
    OsRng.fill_bytes(&mut raw);
    raw.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Deserialize)]
pub struct NewToken {
    name: String,
}

/// Create a permanent personal access token for the signed-in user. The token is shown once.
pub async fn create_token(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Json(b): Json<NewToken>,
) -> ApiResult {
    let Some(user_id) = actor.user_id else {
        return Err(ApiError::invalid("sign in as a user (username and password) to create access tokens"));
    };
    let name = b.name.trim().to_string();
    if name.is_empty() || name.chars().count() > 60 || name.chars().any(|c| c.is_control()) {
        return Err(ApiError::invalid("token name must be 1-60 characters"));
    }
    let conn = st.conn.lock().unwrap();
    let count: i64 =
        conn.query_row("SELECT COUNT(*) FROM api_token WHERE user_id = ?1", params![user_id], |r| r.get(0))?;
    if count >= 20 {
        return Err(ApiError::invalid("limit of 20 access tokens reached; revoke one first"));
    }
    let token = format!("oas_{}", random_hex());
    conn.execute(
        "INSERT INTO api_token(user_id, name, token_hash, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![user_id, name, token_hash(&token), utc_string(Utc::now())],
    )?;
    let id = conn.last_insert_rowid();
    crate::ledger::audit(&conn, &actor, "token.create", &id.to_string(), &name)?;
    Ok(Json(json!({"token": token, "id": id, "name": name, "role": actor.role.name(),
                   "note": "store this token now; it is not shown again"})))
}

pub async fn list_tokens(State(st): State<Shared>, Extension(actor): Extension<Actor>) -> ApiResult {
    let Some(user_id) = actor.user_id else {
        return Err(ApiError::invalid("sign in as a user to list access tokens"));
    };
    let conn = st.conn.lock().unwrap();
    let rows: Vec<Value> = conn
        .prepare("SELECT id, name, created_at, last_used_at FROM api_token WHERE user_id = ?1 ORDER BY id")?
        .query_map(params![user_id], |r| {
            Ok(json!({"id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?,
                      "created_at": r.get::<_, String>(2)?, "last_used_at": r.get::<_, Option<String>>(3)?}))
        })?
        .filter_map(Result::ok)
        .collect();
    Ok(Json(json!({"tokens": rows})))
}

pub async fn revoke_token(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> ApiResult {
    let Some(user_id) = actor.user_id else {
        return Err(ApiError::invalid("sign in as a user to revoke access tokens"));
    };
    let conn = st.conn.lock().unwrap();
    let n = conn.execute("DELETE FROM api_token WHERE id = ?1 AND user_id = ?2", params![id, user_id])?;
    if n == 0 {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such access token"));
    }
    crate::ledger::audit(&conn, &actor, "token.revoke", &id.to_string(), "")?;
    Ok(Json(json!({"status": "ok"})))
}

fn check_username(u: &str) -> Result<String, ApiError> {
    let u = u.trim();
    let ok = (3..=40).contains(&u.chars().count())
        && u.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if ok {
        Ok(u.to_string())
    } else {
        Err(ApiError::invalid("username must be 3-40 characters: letters, digits, dot, dash or underscore"))
    }
}

fn check_password(p: &str, username: &str) -> Result<(), ApiError> {
    if p.chars().count() < 12 || p.chars().count() > 200 {
        return Err(ApiError::invalid("password must be 12-200 characters"));
    }
    if norm(p) == norm(username) {
        return Err(ApiError::invalid("password must not equal the username"));
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct LoginIn {
    username: String,
    password: String,
    ttl_hours: Option<i64>,
}

pub async fn login(State(st): State<Shared>, Json(b): Json<LoginIn>) -> ApiResult {
    let key = norm(&b.username);
    if st.limiter.blocked(&key, MAX_PER_USER) || st.limiter.blocked("*", MAX_GLOBAL) {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "too many failed logins; wait 15 minutes",
        ));
    }
    let row: Option<(i64, String, String)> = {
        let conn = st.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, password_hash, role FROM app_user WHERE username = ?1",
            params![b.username.trim()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok()
    };
    // Verify against a dummy hash for unknown users so timing does not reveal them.
    let hash = row.as_ref().map(|r| r.1.as_str()).unwrap_or(&st.dummy_hash);
    let ok = verify_password(&b.password, hash) && row.is_some();
    let (Some((id, _, role)), true) = (row, ok) else {
        st.limiter.fail(&key);
        st.limiter.fail("*");
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "auth_required",
            "invalid username or password",
        ));
    };
    st.limiter.clear(&key);
    let token = random_hex();
    let ttl = b.ttl_hours.unwrap_or(12).clamp(1, 720);
    let now = Utc::now();
    let expires = now + ChronoDuration::hours(ttl);
    let conn = st.conn.lock().unwrap();
    conn.execute("DELETE FROM session WHERE expires_at < ?1", params![utc_string(now)])?;
    conn.execute(
        "INSERT INTO session(token_hash, user_id, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
        params![token_hash(&token), id, utc_string(now), utc_string(expires)],
    )?;
    Ok(Json(json!({"token": token, "username": b.username.trim(), "role": role,
                   "expires_at": utc_string(expires)})))
}

pub async fn logout(State(st): State<Shared>, Extension(actor): Extension<Actor>) -> ApiResult {
    if let Some(hash) = &actor.session {
        let conn = st.conn.lock().unwrap();
        conn.execute("DELETE FROM session WHERE token_hash = ?1", params![hash])?;
    }
    Ok(Json(json!({"status": "ok"})))
}

#[derive(Deserialize)]
pub struct NewUser {
    username: String,
    password: String,
    role: String,
}

pub async fn add_user(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Json(b): Json<NewUser>,
) -> ApiResult {
    if actor.role != Role::Admin {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "forbidden", "only an admin can add users"));
    }
    let username = check_username(&b.username)?;
    check_password(&b.password, &username)?;
    let role = b.role.trim().to_lowercase();
    if role != "admin" && role != "agent" {
        return Err(ApiError::invalid("role must be admin or agent"));
    }
    let hash = hash_password(&b.password)?;
    let conn = st.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO app_user(username, password_hash, role, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![username, hash, role, utc_string(Utc::now())],
    )
    .map_err(|_| ApiError::new(StatusCode::CONFLICT, "conflict", "username already exists"))?;
    crate::ledger::audit(&conn, &actor, "user.add", &username, &role)?;
    Ok(Json(json!({"user": {"username": username, "role": role}})))
}

pub async fn list_users(State(st): State<Shared>, Extension(actor): Extension<Actor>) -> ApiResult {
    if actor.role != Role::Admin {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "forbidden", "only an admin can list users"));
    }
    let conn = st.conn.lock().unwrap();
    let rows: Vec<Value> = conn
        .prepare("SELECT username, role, created_at FROM app_user ORDER BY username")?
        .query_map([], |r| {
            Ok(json!({"username": r.get::<_, String>(0)?, "role": r.get::<_, String>(1)?,
                      "created_at": r.get::<_, String>(2)?}))
        })?
        .filter_map(Result::ok)
        .collect();
    Ok(Json(json!({"users": rows})))
}

#[derive(Deserialize)]
pub struct ChangePassword {
    current: String,
    new: String,
}

pub async fn change_password(
    State(st): State<Shared>,
    Extension(actor): Extension<Actor>,
    Json(b): Json<ChangePassword>,
) -> ApiResult {
    let Some(user_id) = actor.user_id else {
        return Err(ApiError::invalid("this is a token login with no password; sign in as a user"));
    };
    check_password(&b.new, &actor.name)?;
    let current_hash: String = {
        let conn = st.conn.lock().unwrap();
        conn.query_row("SELECT password_hash FROM app_user WHERE id = ?1", params![user_id], |r| r.get(0))?
    };
    if !verify_password(&b.current, &current_hash) {
        return Err(ApiError::new(StatusCode::UNAUTHORIZED, "auth_required", "current password is wrong"));
    }
    let hash = hash_password(&b.new)?;
    let conn = st.conn.lock().unwrap();
    conn.execute("UPDATE app_user SET password_hash = ?1 WHERE id = ?2", params![hash, user_id])?;
    // Sign out every other session of this user.
    conn.execute(
        "DELETE FROM session WHERE user_id = ?1 AND token_hash != ?2",
        params![user_id, actor.session.clone().unwrap_or_default()],
    )?;
    crate::ledger::audit(&conn, &actor, "user.password", &actor.name, "")?;
    Ok(Json(json!({"status": "ok"})))
}
