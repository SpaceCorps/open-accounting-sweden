//! `oas`: command line interface for an open-accounting-sweden instance.
//! YAML on stdout by default, `--json` for agents. Errors are an envelope on stderr
//! with a stable exit code.

use std::io::{BufRead, IsTerminal, Write};
use std::path::PathBuf;

use chrono::{Datelike, Duration, Local};
use clap::{Parser, Subcommand};
use serde_json::{Value, json};

const AGENT_README: &str = "\
# oas - agent operating manual

oas talks to a self-hosted open-accounting-sweden instance. Output is YAML on stdout,
or JSON with --json. Errors are a structured envelope on stderr (JSON with --json).

## Rules
- Authenticate first. Humans: `oas auth login <url> --username U`. Agents: a human creates a user
  with role agent, signs in as it, runs `oas token create <name>`, and gives you the permanent
  `oas_...` token; then `oas auth login <url> --token oas_...`. Agent users can propose, never approve.
- Never invent a project. If the project is missing or unknown the command fails with
  `project_required` or `project_not_found` and lists known projects. Ask the human,
  then retry with the confirmed name, or add --create-project once they agree to create it.
- Prompts only appear on an interactive terminal. Agents never get a prompt.
- Timed entries use local time in --tz (default Europe/Stockholm).

## Commands
oas auth login <url> [--username U]   (prompts for the password; or --token T)
oas auth status | logout
oas user add USERNAME --role admin|agent | list | passwd
oas token create NAME [--save] | list | revoke ID
oas time log --activity A [--project P] --date YYYY-MM-DD --from HH:MM --to HH:MM [--message M] [--create-project]
oas time start --activity A [--project P] [--message M] [--create-project]
oas time stop
oas time list [--from D] [--to D] [--project P] [--activity A]
oas time report [--week | --month | --from D --to D] [--project P] [--activity A]
oas time export [--week | --month | --from D --to D] [--project P] [--activity A] [--out FILE]
oas project add NAME [--alias A]... [--client C] | list
oas activity add NAME [--message TEMPLATE] | list
oas company set NAME [--org-number N] | show
oas account list | add NUMBER NAME --kind T|S|K|I
oas voucher propose --date D --description TEXT [--counterparty C] --debit ACCOUNT=AMOUNT ... --credit ACCOUNT=AMOUNT ...
oas voucher drafts | list [--from D] [--to D] | show ID | approve ID | reject ID | correct ID
oas ledger verify | trial-balance [--year Y | --from D --to D] | audit
oas export sie [--year Y] [--out FILE]
oas doc add FILE [--vendor V] [--amount A] [--currency SEK] [--date D] [--notes N]
oas doc list [--status unlinked|draft|booked] | meta ID [--vendor V --amount A --currency C --date D --notes N] | download ID [--out FILE]
oas bank import FILE.csv | list [--status new|booked|ignored] | match | ignore ID
oas bank book ID [--description T] --debit ACCOUNT=AMOUNT ... --credit ACCOUNT=AMOUNT ... [--doc ID]...
oas match list [--status pending|confirmed|declined] | confirm ID | decline ID
oas agent-readme

## Receipts and bank lines
- Read each receipt yourself and pass what you read: `oas doc add receipt.pdf --vendor 'Amazon Web
  Services' --amount 1250.00 --currency SEK --date 2026-10-20`. Matching uses vendor, amount,
  currency and date, so fill them in. Uploading the same file twice returns the existing document.
- `oas bank import statement.csv` takes a Lunar or other bank CSV (comma or semicolon, English or
  Swedish headers: date/datum, amount/belopp, text/description, reference). Re-importing is safe.
- Matching only suggests. Only an admin runs `match confirm` or `match decline`; do not try to.
- `oas bank book` turns a bank line into a DRAFT voucher: give only the counter lines, the bank
  account line (1930) is added from the sign of the bank amount. Confirmed receipts attach
  automatically. A human still approves the draft with `oas voucher approve`.
- Receipts and posted vouchers cannot be changed or deleted.

## Ledger rules (read before posting)
- Agents only PROPOSE. `voucher propose` creates a draft. Approving, rejecting and correcting
  need the admin token and fail with `forbidden` (exit 7) for an agent token. Tell the human
  which draft id to approve; never try to work around it.
- Posted vouchers are append-only and gap-free. A mistake is fixed with `voucher correct`
  (a reversing voucher), then a new proposal.
- A voucher must balance (total debit = total credit), amounts in SEK with at most 2 decimals.
- Run `oas ledger verify` before relying on the books; it checks gaps, balance and the hash chain.
- The starter chart of accounts is small. Add accounts with `oas account add`.
- This is bookkeeping preparation, not tax or legal advice.

## Exit codes
0 ok, 1 server or unexpected, 2 invalid input / project problem, 3 auth_required,
4 not_found, 5 conflict (overlap, timer running, already corrected), 6 network, 7 forbidden,
8 integrity_failed (ledger verify found problems).
";

#[derive(Parser)]
#[command(name = "oas", version, about = "CLI for open-accounting-sweden")]
struct Cli {
    /// Print JSON instead of YAML
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Log in to an instance and store the token
    Auth {
        #[command(subcommand)]
        action: AuthCmd,
    },
    /// Time tracking
    Time {
        #[command(subcommand)]
        action: TimeCmd,
    },
    /// Projects
    Project {
        #[command(subcommand)]
        action: ProjectCmd,
    },
    /// Activity types
    Activity {
        #[command(subcommand)]
        action: ActivityCmd,
    },
    /// Users
    User {
        #[command(subcommand)]
        action: UserCmd,
    },
    /// Permanent access tokens for the CLI and agents
    Token {
        #[command(subcommand)]
        action: TokenCmd,
    },
    /// Company details used in exports
    Company {
        #[command(subcommand)]
        action: CompanyCmd,
    },
    /// Chart of accounts
    Account {
        #[command(subcommand)]
        action: AccountCmd,
    },
    /// Vouchers: propose, approve, correct
    Voucher {
        #[command(subcommand)]
        action: VoucherCmd,
    },
    /// Ledger checks and reports
    Ledger {
        #[command(subcommand)]
        action: LedgerCmd,
    },
    /// Export for accountants and other systems
    Export {
        #[command(subcommand)]
        action: ExportCmd,
    },
    /// Receipts and invoices
    Doc {
        #[command(subcommand)]
        action: DocCmd,
    },
    /// Bank statement import and booking
    Bank {
        #[command(subcommand)]
        action: BankCmd,
    },
    /// Receipt to bank line suggestions
    Match {
        #[command(subcommand)]
        action: MatchCmd,
    },
    /// Operating manual for agents
    AgentReadme,
}

#[derive(Subcommand)]
enum DocCmd {
    /// Upload a receipt or invoice with what you read from it
    Add {
        file: PathBuf,
        #[arg(long)]
        vendor: Option<String>,
        #[arg(long)]
        amount: Option<String>,
        #[arg(long)]
        currency: Option<String>,
        #[arg(long)]
        date: Option<String>,
        #[arg(long)]
        notes: Option<String>,
    },
    List {
        #[arg(long)]
        status: Option<String>,
    },
    /// Replace the details read from a document
    Meta {
        id: i64,
        #[arg(long)]
        vendor: Option<String>,
        #[arg(long)]
        amount: Option<String>,
        #[arg(long)]
        currency: Option<String>,
        #[arg(long)]
        date: Option<String>,
        #[arg(long)]
        notes: Option<String>,
    },
    Download {
        id: i64,
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum BankCmd {
    Import { file: PathBuf },
    List {
        #[arg(long)]
        status: Option<String>,
    },
    /// Re-run matching of receipts against bank lines
    Match,
    Ignore { id: i64 },
    /// Create a draft voucher from a bank line
    Book {
        id: i64,
        #[arg(long)]
        description: Option<String>,
        /// ACCOUNT=AMOUNT, repeatable
        #[arg(long = "debit")]
        debits: Vec<String>,
        /// ACCOUNT=AMOUNT, repeatable
        #[arg(long = "credit")]
        credits: Vec<String>,
        /// Receipt document id, repeatable
        #[arg(long = "doc")]
        docs: Vec<i64>,
        /// Bank account number, default 1930
        #[arg(long)]
        bank_account: Option<i64>,
    },
}

#[derive(Subcommand)]
enum MatchCmd {
    List {
        #[arg(long)]
        status: Option<String>,
    },
    Confirm { id: i64 },
    Decline { id: i64 },
}

#[derive(Subcommand)]
enum CompanyCmd {
    Set {
        name: String,
        #[arg(long)]
        org_number: Option<String>,
    },
    Show,
}

#[derive(Subcommand)]
enum AccountCmd {
    List,
    Add {
        number: i64,
        name: String,
        /// T asset, S liability or equity, K cost, I income
        #[arg(long)]
        kind: String,
    },
}

#[derive(Subcommand)]
enum VoucherCmd {
    /// Propose a draft voucher (needs approval by an admin)
    Propose {
        #[arg(long)]
        date: String,
        #[arg(long)]
        description: String,
        #[arg(long)]
        counterparty: Option<String>,
        /// ACCOUNT=AMOUNT, repeatable
        #[arg(long = "debit")]
        debits: Vec<String>,
        /// ACCOUNT=AMOUNT, repeatable
        #[arg(long = "credit")]
        credits: Vec<String>,
    },
    Drafts,
    List {
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
    },
    Show { id: i64 },
    Approve { id: i64 },
    Reject { id: i64 },
    /// Post a reversing voucher for a posted voucher
    Correct { id: i64 },
}

#[derive(Subcommand)]
enum LedgerCmd {
    Verify,
    TrialBalance {
        #[arg(long)]
        year: Option<i32>,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
    },
    Audit,
}

#[derive(Subcommand)]
enum ExportCmd {
    /// SIE 4 file for one calendar year
    Sie {
        #[arg(long)]
        year: Option<i32>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum AuthCmd {
    Login {
        url: String,
        /// Sign in with this username (the password is prompted or read from stdin)
        #[arg(long)]
        username: Option<String>,
        /// Sign in with an access token instead of a username and password
        #[arg(long)]
        token: Option<String>,
    },
    Status,
    Logout,
}

#[derive(Subcommand)]
enum UserCmd {
    Add {
        username: String,
        #[arg(long)]
        role: String,
    },
    List,
    /// Change your own password
    Passwd,
}

#[derive(Subcommand)]
enum TokenCmd {
    /// Create a permanent personal access token (shown once)
    Create {
        name: String,
        /// Store it as this machine's login instead of printing it
        #[arg(long)]
        save: bool,
    },
    List,
    Revoke { id: i64 },
}

#[derive(clap::Args, Clone)]
struct Range {
    #[arg(long, conflicts_with_all = ["month", "from", "to"])]
    week: bool,
    #[arg(long, conflicts_with_all = ["from", "to"])]
    month: bool,
    #[arg(long)]
    from: Option<String>,
    #[arg(long)]
    to: Option<String>,
    #[arg(long)]
    project: Option<String>,
    #[arg(long)]
    activity: Option<String>,
}

#[derive(Subcommand)]
enum TimeCmd {
    Log {
        #[arg(long)]
        activity: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        date: String,
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        message: Option<String>,
        #[arg(long)]
        tz: Option<String>,
        #[arg(long)]
        create_project: bool,
    },
    Start {
        #[arg(long)]
        activity: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        message: Option<String>,
        #[arg(long)]
        tz: Option<String>,
        #[arg(long)]
        create_project: bool,
    },
    Stop,
    List(Range),
    Report(Range),
    Export {
        #[command(flatten)]
        range: Range,
        /// Write to a file instead of stdout
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, default_value = "csv")]
        format: String,
    },
}

#[derive(Subcommand)]
enum ProjectCmd {
    Add {
        name: String,
        #[arg(long = "alias")]
        aliases: Vec<String>,
        #[arg(long)]
        client: Option<String>,
    },
    List,
}

#[derive(Subcommand)]
enum ActivityCmd {
    Add {
        name: String,
        #[arg(long)]
        message: Option<String>,
    },
    List,
}

struct Failure {
    code: String,
    message: String,
    projects: Vec<String>,
}

impl Failure {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into(), projects: vec![] }
    }
    fn exit_code(&self) -> i32 {
        match self.code.as_str() {
            "invalid_input" | "project_required" | "project_not_found" => 2,
            "auth_required" => 3,
            "not_found" => 4,
            "conflict" => 5,
            "network" => 6,
            "forbidden" => 7,
            "integrity_failed" => 8,
            _ => 1,
        }
    }
}

struct Config {
    url: String,
    token: String,
}

fn config_path() -> PathBuf {
    if let Ok(dir) = std::env::var("OAS_CONFIG_DIR") {
        return PathBuf::from(dir).join("config.json");
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config").join("oas").join("config.json")
}

fn load_config() -> Result<Config, Failure> {
    let stored: Option<Value> = std::fs::read_to_string(config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());
    let field = |env: &str, key: &str| {
        std::env::var(env)
            .ok()
            .or_else(|| stored.as_ref().and_then(|v| v[key].as_str().map(String::from)))
    };
    match (field("OAS_URL", "url"), field("OAS_TOKEN", "token")) {
        (Some(url), Some(token)) => Ok(Config { url: url.trim_end_matches('/').into(), token }),
        _ => Err(Failure::new("auth_required", "not logged in; run `oas auth login <url>`")),
    }
}

fn save_config(url: &str, token: &str) -> Result<(), Failure> {
    let path = config_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Failure::new("internal", e.to_string()))?;
    }
    let body = json!({"url": url, "token": token}).to_string();
    std::fs::write(&path, body).map_err(|e| Failure::new("internal", e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn query_string(range: &Range, from: &Option<String>, to: &Option<String>) -> String {
    let mut parts = vec![];
    if let Some(f) = from {
        parts.push(format!("from={}", encode(f)));
    }
    if let Some(t) = to {
        parts.push(format!("to={}", encode(t)));
    }
    if let Some(p) = &range.project {
        parts.push(format!("project={}", encode(p)));
    }
    if let Some(a) = &range.activity {
        parts.push(format!("activity={}", encode(a)));
    }
    if parts.is_empty() { String::new() } else { format!("?{}", parts.join("&")) }
}

fn resolve_range(r: &Range) -> (Option<String>, Option<String>) {
    let today = Local::now().date_naive();
    if r.week {
        let monday = today - Duration::days(today.weekday().num_days_from_monday() as i64);
        return (Some(monday.to_string()), Some((monday + Duration::days(6)).to_string()));
    }
    if r.month {
        let first = today.with_day(1).unwrap_or(today);
        let next = if today.month() == 12 {
            chrono::NaiveDate::from_ymd_opt(today.year() + 1, 1, 1)
        } else {
            chrono::NaiveDate::from_ymd_opt(today.year(), today.month() + 1, 1)
        }
        .unwrap_or(today);
        return (Some(first.to_string()), Some((next - Duration::days(1)).to_string()));
    }
    (r.from.clone(), r.to.clone())
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().http_status_as_error(false).build().into()
}

fn request(cfg: &Config, method: &str, path: &str, body: Option<&Value>) -> Result<(u16, String), Failure> {
    let url = format!("{}/api{}", cfg.url, path);
    let bearer = format!("Bearer {}", cfg.token);
    let a = agent();
    let result = match (method, body) {
        ("GET", _) => a.get(&url).header("Authorization", &bearer).call(),
        (_, Some(b)) => a
            .post(&url)
            .header("Authorization", &bearer)
            .content_type("application/json")
            .send(b.to_string().as_bytes()),
        _ => a
            .post(&url)
            .header("Authorization", &bearer)
            .content_type("application/json")
            .send(b"{}".as_slice()),
    };
    let mut resp = result.map_err(|e| Failure::new("network", format!("cannot reach {}: {e}", cfg.url)))?;
    let status = resp.status().as_u16();
    let text = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| Failure::new("network", format!("bad response: {e}")))?;
    Ok((status, text))
}

fn request_bytes(cfg: &Config, path: &str) -> Result<(u16, Vec<u8>), Failure> {
    let url = format!("{}/api{}", cfg.url, path);
    let mut resp = agent()
        .get(&url)
        .header("Authorization", &format!("Bearer {}", cfg.token))
        .call()
        .map_err(|e| Failure::new("network", format!("cannot reach {}: {e}", cfg.url)))?;
    let status = resp.status().as_u16();
    let bytes = resp
        .body_mut()
        .read_to_vec()
        .map_err(|e| Failure::new("network", format!("bad response: {e}")))?;
    Ok((status, bytes))
}

fn post_bytes(cfg: &Config, path: &str, content_type: &str, bytes: &[u8]) -> Result<Value, Failure> {
    let url = format!("{}/api{}", cfg.url, path);
    let mut resp = agent()
        .post(&url)
        .header("Authorization", &format!("Bearer {}", cfg.token))
        .content_type(content_type)
        .send(bytes)
        .map_err(|e| Failure::new("network", format!("cannot reach {}: {e}", cfg.url)))?;
    let status = resp.status().as_u16();
    let text = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| Failure::new("network", format!("bad response: {e}")))?;
    if (200..300).contains(&status) {
        serde_json::from_str(&text).map_err(|_| Failure::new("internal", "invalid JSON from server"))
    } else {
        Err(failure_from(&text))
    }
}

fn mime_for(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref() {
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("txt") => "text/plain",
        Some("csv") => "text/csv",
        _ => "application/octet-stream",
    }
}

fn meta_query(vendor: &Option<String>, amount: &Option<String>, currency: &Option<String>, date: &Option<String>, notes: &Option<String>) -> Vec<String> {
    let mut parts = vec![];
    for (k, v) in [("vendor", vendor), ("amount", amount), ("currency", currency), ("date", date), ("notes", notes)] {
        if let Some(v) = v {
            parts.push(format!("{k}={}", encode(v)));
        }
    }
    parts
}

fn failure_from(text: &str) -> Failure {
    let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let e = &v["error"];
    Failure {
        code: e["code"].as_str().unwrap_or("internal").into(),
        message: e["message"].as_str().unwrap_or("unexpected server response").into(),
        projects: e["projects"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
            .unwrap_or_default(),
    }
}

fn call(cfg: &Config, method: &str, path: &str, body: Option<&Value>) -> Result<Value, Failure> {
    let (status, text) = request(cfg, method, path, body)?;
    if (200..300).contains(&status) {
        serde_json::from_str(&text).map_err(|_| Failure::new("internal", "invalid JSON from server"))
    } else {
        Err(failure_from(&text))
    }
}

fn ask(prompt: &str) -> String {
    eprint!("{prompt}");
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    let _ = std::io::stdin().lock().read_line(&mut line);
    line.trim().to_string()
}

/// POST a time entry. On a TTY, ask for a missing project or offer to create one;
/// otherwise return the error so an agent can ask its human.
fn post_with_project(cfg: &Config, path: &str, mut body: Value) -> Result<Value, Failure> {
    let interactive = std::io::stdin().is_terminal() && std::io::stderr().is_terminal();
    loop {
        match call(cfg, "POST", path, Some(&body)) {
            Ok(v) => return Ok(v),
            Err(f) if interactive && f.code == "project_required" => {
                eprintln!("No project given. Existing projects: {}", f.projects.join(", "));
                let name = ask("Project (type a new name to create it): ");
                if name.is_empty() {
                    return Err(f);
                }
                body["project"] = json!(name);
            }
            Err(f) if interactive && f.code == "project_not_found" => {
                let typed = body["project"].as_str().unwrap_or("").to_string();
                if f.projects.is_empty() {
                    if ask(&format!("Project '{typed}' not found. Create it? [y/N] ")).to_lowercase() == "y" {
                        body["create_project"] = json!(true);
                    } else {
                        return Err(f);
                    }
                } else {
                    for (i, p) in f.projects.iter().enumerate() {
                        eprintln!("  {}. {}", i + 1, p);
                    }
                    let answer = ask(&format!(
                        "Pick a number, 'n' to create '{typed}', anything else to cancel: "
                    ));
                    if let Ok(n) = answer.parse::<usize>() {
                        match f.projects.get(n.wrapping_sub(1)) {
                            Some(p) => body["project"] = json!(p),
                            None => return Err(f),
                        }
                    } else if answer.to_lowercase() == "n" {
                        body["create_project"] = json!(true);
                    } else {
                        return Err(f);
                    }
                }
            }
            Err(f) => return Err(f),
        }
    }
}

fn read_password(prompt: &str) -> Result<String, Failure> {
    if std::io::stdin().is_terminal() {
        rpassword::prompt_password(prompt).map_err(|e| Failure::new("invalid_input", e.to_string()))
    } else {
        let mut line = String::new();
        std::io::stdin()
            .lock()
            .read_line(&mut line)
            .map_err(|e| Failure::new("invalid_input", e.to_string()))?;
        Ok(line.trim_end_matches(['\n', '\r']).to_string())
    }
}

fn print_value(v: &Value, json_out: bool) {
    if json_out {
        println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
    } else {
        print!("{}", serde_norway::to_string(v).unwrap_or_default());
    }
}

fn run(cli: Cli) -> Result<(), Failure> {
    let json_out = cli.json;
    match cli.command {
        Cmd::AgentReadme => {
            if json_out {
                print_value(&json!({"readme": AGENT_README}), true);
            } else {
                print!("{AGENT_README}");
            }
            Ok(())
        }
        Cmd::Auth { action: AuthCmd::Login { url, username, token } } => {
            let url = url.trim_end_matches('/').to_string();
            let token = match (token, username) {
                (Some(t), _) => t.trim().to_string(),
                (None, user) => {
                    let user = match user {
                        Some(u) => u,
                        None if std::io::stdin().is_terminal() => ask("Username: "),
                        None => return Err(Failure::new("invalid_input", "give --username or --token")),
                    };
                    let password = read_password("Password: ")?;
                    let anon = Config { url: url.clone(), token: String::new() };
                    let body = json!({"username": user, "password": password, "ttl_hours": 720});
                    let v = call(&anon, "POST", "/login", Some(&body))?;
                    v["token"].as_str().unwrap_or_default().to_string()
                }
            };
            let cfg = Config { url: url.clone(), token };
            let me = call(&cfg, "GET", "/me", None)?;
            save_config(&url, &cfg.token)?;
            print_value(&json!({"status": "ok", "url": url, "username": me["username"],
                "role": me["role"]}), json_out);
            Ok(())
        }
        Cmd::Auth { action: AuthCmd::Logout } => {
            let cfg = load_config()?;
            let _ = call(&cfg, "POST", "/logout", None);
            let _ = std::fs::remove_file(config_path());
            print_value(&json!({"status": "ok"}), json_out);
            Ok(())
        }
        Cmd::User { action } => {
            let cfg = load_config()?;
            match action {
                UserCmd::Add { username, role } => {
                    let p1 = read_password("New password (12+ characters): ")?;
                    if std::io::stdin().is_terminal() && read_password("Repeat password: ")? != p1 {
                        return Err(Failure::new("invalid_input", "passwords do not match"));
                    }
                    let body = json!({"username": username, "password": p1, "role": role});
                    print_value(&call(&cfg, "POST", "/users", Some(&body))?, json_out);
                }
                UserCmd::List => print_value(&call(&cfg, "GET", "/users", None)?, json_out),
                UserCmd::Passwd => {
                    let current = read_password("Current password: ")?;
                    let new = read_password("New password (12+ characters): ")?;
                    if std::io::stdin().is_terminal() && read_password("Repeat new password: ")? != new {
                        return Err(Failure::new("invalid_input", "passwords do not match"));
                    }
                    let body = json!({"current": current, "new": new});
                    print_value(&call(&cfg, "POST", "/me/password", Some(&body))?, json_out);
                }
            }
            Ok(())
        }
        Cmd::Token { action } => {
            let cfg = load_config()?;
            match action {
                TokenCmd::Create { name, save } => {
                    let v = call(&cfg, "POST", "/tokens", Some(&json!({"name": name})))?;
                    if save {
                        let token = v["token"].as_str().unwrap_or_default();
                        save_config(&cfg.url, token)?;
                        print_value(&json!({"status": "ok", "id": v["id"], "name": v["name"],
                            "saved": "this machine now uses the permanent token"}), json_out);
                    } else {
                        print_value(&v, json_out);
                    }
                }
                TokenCmd::List => print_value(&call(&cfg, "GET", "/tokens", None)?, json_out),
                TokenCmd::Revoke { id } => {
                    print_value(&call(&cfg, "POST", &format!("/tokens/{id}/revoke"), None)?, json_out)
                }
            }
            Ok(())
        }
        Cmd::Auth { action: AuthCmd::Status } => {
            let cfg = load_config()?;
            let me = call(&cfg, "GET", "/me", None)?;
            print_value(&json!({"status": "ok", "url": cfg.url, "username": me["username"],
                "role": me["role"], "can_approve": me["can_approve"]}), json_out);
            Ok(())
        }
        Cmd::Time { action } => {
            let cfg = load_config()?;
            match action {
                TimeCmd::Log { activity, project, date, from, to, message, tz, create_project } => {
                    let body = json!({"activity": activity, "project": project, "date": date,
                        "from": from, "to": to, "message": message, "timezone": tz,
                        "create_project": create_project});
                    print_value(&post_with_project(&cfg, "/time", body)?, json_out);
                }
                TimeCmd::Start { activity, project, message, tz, create_project } => {
                    let body = json!({"activity": activity, "project": project,
                        "message": message, "timezone": tz, "create_project": create_project});
                    print_value(&post_with_project(&cfg, "/time/start", body)?, json_out);
                }
                TimeCmd::Stop => print_value(&call(&cfg, "POST", "/time/stop", None)?, json_out),
                TimeCmd::List(r) => {
                    let (f, t) = resolve_range(&r);
                    let q = query_string(&r, &f, &t);
                    print_value(&call(&cfg, "GET", &format!("/time{q}"), None)?, json_out);
                }
                TimeCmd::Report(r) => {
                    let (f, t) = resolve_range(&r);
                    let q = query_string(&r, &f, &t);
                    print_value(&call(&cfg, "GET", &format!("/time/report{q}"), None)?, json_out);
                }
                TimeCmd::Export { range, out, format } => {
                    if format != "csv" {
                        return Err(Failure::new("invalid_input", "only --format csv is supported"));
                    }
                    let (f, t) = resolve_range(&range);
                    let q = query_string(&range, &f, &t);
                    let (status, text) = request(&cfg, "GET", &format!("/time/export.csv{q}"), None)?;
                    if !(200..300).contains(&status) {
                        return Err(failure_from(&text));
                    }
                    match out {
                        Some(path) => {
                            std::fs::write(&path, &text)
                                .map_err(|e| Failure::new("internal", e.to_string()))?;
                            print_value(&json!({"status": "ok", "file": path.display().to_string()}), json_out);
                        }
                        None => print!("{text}"),
                    }
                }
            }
            Ok(())
        }
        Cmd::Company { action } => {
            let cfg = load_config()?;
            match action {
                CompanyCmd::Set { name, org_number } => {
                    let body = json!({"name": name, "org_number": org_number});
                    print_value(&call(&cfg, "POST", "/company", Some(&body))?, json_out);
                }
                CompanyCmd::Show => print_value(&call(&cfg, "GET", "/company", None)?, json_out),
            }
            Ok(())
        }
        Cmd::Account { action } => {
            let cfg = load_config()?;
            match action {
                AccountCmd::List => print_value(&call(&cfg, "GET", "/accounts", None)?, json_out),
                AccountCmd::Add { number, name, kind } => {
                    let body = json!({"number": number, "name": name, "kind": kind});
                    print_value(&call(&cfg, "POST", "/accounts", Some(&body))?, json_out);
                }
            }
            Ok(())
        }
        Cmd::Voucher { action } => {
            let cfg = load_config()?;
            match action {
                VoucherCmd::Propose { date, description, counterparty, debits, credits } => {
                    let mut lines = vec![];
                    for (list, side) in [(&debits, "debit"), (&credits, "credit")] {
                        for item in list {
                            let (acc, amount) = item.split_once('=').ok_or_else(|| {
                                Failure::new("invalid_input", format!("expected ACCOUNT=AMOUNT, got '{item}'"))
                            })?;
                            let account: i64 = acc.trim().parse().map_err(|_| {
                                Failure::new("invalid_input", format!("invalid account '{acc}'"))
                            })?;
                            lines.push(json!({"account": account, side: amount.trim()}));
                        }
                    }
                    let body = json!({"event_date": date, "description": description,
                        "counterparty": counterparty, "lines": lines});
                    print_value(&call(&cfg, "POST", "/drafts", Some(&body))?, json_out);
                }
                VoucherCmd::Drafts => print_value(&call(&cfg, "GET", "/drafts", None)?, json_out),
                VoucherCmd::List { from, to } => {
                    let mut parts = vec![];
                    if let Some(f) = from { parts.push(format!("from={}", encode(&f))); }
                    if let Some(t) = to { parts.push(format!("to={}", encode(&t))); }
                    let q = if parts.is_empty() { String::new() } else { format!("?{}", parts.join("&")) };
                    print_value(&call(&cfg, "GET", &format!("/vouchers{q}"), None)?, json_out);
                }
                VoucherCmd::Show { id } => {
                    print_value(&call(&cfg, "GET", &format!("/vouchers/{id}"), None)?, json_out)
                }
                VoucherCmd::Approve { id } => {
                    print_value(&call(&cfg, "POST", &format!("/drafts/{id}/approve"), None)?, json_out)
                }
                VoucherCmd::Reject { id } => {
                    print_value(&call(&cfg, "POST", &format!("/drafts/{id}/reject"), None)?, json_out)
                }
                VoucherCmd::Correct { id } => {
                    print_value(&call(&cfg, "POST", &format!("/vouchers/{id}/correct"), None)?, json_out)
                }
            }
            Ok(())
        }
        Cmd::Ledger { action } => {
            let cfg = load_config()?;
            match action {
                LedgerCmd::Verify => {
                    let v = call(&cfg, "GET", "/ledger/verify", None)?;
                    print_value(&v, json_out);
                    if v["ok"] != json!(true) {
                        return Err(Failure::new(
                            "integrity_failed",
                            "ledger verification found problems; see the issues above",
                        ));
                    }
                }
                LedgerCmd::Audit => print_value(&call(&cfg, "GET", "/audit", None)?, json_out),
                LedgerCmd::TrialBalance { year, from, to } => {
                    let mut parts = vec![];
                    if let Some(y) = year { parts.push(format!("year={y}")); }
                    if let Some(f) = from { parts.push(format!("from={}", encode(&f))); }
                    if let Some(t) = to { parts.push(format!("to={}", encode(&t))); }
                    let q = if parts.is_empty() { String::new() } else { format!("?{}", parts.join("&")) };
                    print_value(&call(&cfg, "GET", &format!("/reports/trial-balance{q}"), None)?, json_out);
                }
            }
            Ok(())
        }
        Cmd::Export { action: ExportCmd::Sie { year, out } } => {
            let cfg = load_config()?;
            let q = year.map(|y| format!("?year={y}")).unwrap_or_default();
            let (status, bytes) = request_bytes(&cfg, &format!("/export/sie{q}"))?;
            if !(200..300).contains(&status) {
                return Err(failure_from(&String::from_utf8_lossy(&bytes)));
            }
            match out {
                Some(path) => {
                    std::fs::write(&path, &bytes).map_err(|e| Failure::new("internal", e.to_string()))?;
                    print_value(&json!({"status": "ok", "file": path.display().to_string(),
                        "bytes": bytes.len()}), json_out);
                }
                None => {
                    std::io::stdout().write_all(&bytes).map_err(|e| Failure::new("internal", e.to_string()))?;
                }
            }
            Ok(())
        }
        Cmd::Doc { action } => {
            let cfg = load_config()?;
            match action {
                DocCmd::Add { file, vendor, amount, currency, date, notes } => {
                    let bytes = std::fs::read(&file)
                        .map_err(|e| Failure::new("invalid_input", format!("cannot read {}: {e}", file.display())))?;
                    let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("upload").to_string();
                    let mut parts = vec![format!("filename={}", encode(&name)), "source=cli".to_string()];
                    parts.extend(meta_query(&vendor, &amount, &currency, &date, &notes));
                    let v = post_bytes(&cfg, &format!("/documents?{}", parts.join("&")), mime_for(&file), &bytes)?;
                    print_value(&v, json_out);
                }
                DocCmd::List { status } => {
                    let q = status.map(|s| format!("?status={}", encode(&s))).unwrap_or_default();
                    print_value(&call(&cfg, "GET", &format!("/documents{q}"), None)?, json_out);
                }
                DocCmd::Meta { id, vendor, amount, currency, date, notes } => {
                    let body = json!({"vendor": vendor, "amount": amount, "currency": currency, "date": date, "notes": notes});
                    print_value(&call(&cfg, "POST", &format!("/documents/{id}/meta"), Some(&body))?, json_out);
                }
                DocCmd::Download { id, out } => {
                    let (status, bytes) = request_bytes(&cfg, &format!("/documents/{id}/file"))?;
                    if !(200..300).contains(&status) {
                        return Err(failure_from(&String::from_utf8_lossy(&bytes)));
                    }
                    match out {
                        Some(path) => {
                            std::fs::write(&path, &bytes).map_err(|e| Failure::new("internal", e.to_string()))?;
                            print_value(&json!({"status": "ok", "file": path.display().to_string(), "bytes": bytes.len()}), json_out);
                        }
                        None => std::io::stdout().write_all(&bytes).map_err(|e| Failure::new("internal", e.to_string()))?,
                    }
                }
            }
            Ok(())
        }
        Cmd::Bank { action } => {
            let cfg = load_config()?;
            match action {
                BankCmd::Import { file } => {
                    let bytes = std::fs::read(&file)
                        .map_err(|e| Failure::new("invalid_input", format!("cannot read {}: {e}", file.display())))?;
                    print_value(&post_bytes(&cfg, "/bank/import", "text/csv", &bytes)?, json_out);
                }
                BankCmd::List { status } => {
                    let q = status.map(|s| format!("?status={}", encode(&s))).unwrap_or_default();
                    print_value(&call(&cfg, "GET", &format!("/bank/transactions{q}"), None)?, json_out);
                }
                BankCmd::Match => print_value(&call(&cfg, "POST", "/bank/match", None)?, json_out),
                BankCmd::Ignore { id } => {
                    print_value(&call(&cfg, "POST", &format!("/bank/transactions/{id}/ignore"), None)?, json_out)
                }
                BankCmd::Book { id, description, debits, credits, docs, bank_account } => {
                    let mut lines = vec![];
                    for (list, side) in [(&debits, "debit"), (&credits, "credit")] {
                        for item in list {
                            let (acc, amount) = item.split_once('=').ok_or_else(|| {
                                Failure::new("invalid_input", format!("expected ACCOUNT=AMOUNT, got '{item}'"))
                            })?;
                            let account: i64 = acc.trim().parse().map_err(|_| {
                                Failure::new("invalid_input", format!("invalid account '{acc}'"))
                            })?;
                            lines.push(json!({"account": account, side: amount.trim()}));
                        }
                    }
                    let body = json!({"description": description, "lines": lines, "documents": docs,
                        "bank_account": bank_account});
                    print_value(&call(&cfg, "POST", &format!("/bank/transactions/{id}/book"), Some(&body))?, json_out);
                }
            }
            Ok(())
        }
        Cmd::Match { action } => {
            let cfg = load_config()?;
            match action {
                MatchCmd::List { status } => {
                    let q = status.map(|s| format!("?status={}", encode(&s))).unwrap_or_default();
                    print_value(&call(&cfg, "GET", &format!("/matches{q}"), None)?, json_out);
                }
                MatchCmd::Confirm { id } => {
                    print_value(&call(&cfg, "POST", &format!("/matches/{id}/confirm"), None)?, json_out)
                }
                MatchCmd::Decline { id } => {
                    print_value(&call(&cfg, "POST", &format!("/matches/{id}/decline"), None)?, json_out)
                }
            }
            Ok(())
        }
        Cmd::Project { action } => {
            let cfg = load_config()?;
            match action {
                ProjectCmd::Add { name, aliases, client } => {
                    let body = json!({"name": name, "aliases": aliases, "client": client});
                    print_value(&call(&cfg, "POST", "/projects", Some(&body))?, json_out);
                }
                ProjectCmd::List => print_value(&call(&cfg, "GET", "/projects", None)?, json_out),
            }
            Ok(())
        }
        Cmd::Activity { action } => {
            let cfg = load_config()?;
            match action {
                ActivityCmd::Add { name, message } => {
                    let body = json!({"name": name, "message": message});
                    print_value(&call(&cfg, "POST", "/activities", Some(&body))?, json_out);
                }
                ActivityCmd::List => print_value(&call(&cfg, "GET", "/activities", None)?, json_out),
            }
            Ok(())
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let json_out = cli.json;
    if let Err(f) = run(cli) {
        let envelope = json!({"error": {"code": f.code, "message": f.message, "projects": f.projects}});
        if json_out {
            eprintln!("{}", serde_json::to_string(&envelope).unwrap_or_default());
        } else {
            eprint!("{}", serde_norway::to_string(&envelope).unwrap_or_default());
        }
        std::process::exit(f.exit_code());
    }
}
