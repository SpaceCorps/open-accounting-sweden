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
oas income [--month YYYY-MM]   (earned so far and a rough month-end projection, excl. VAT, from tracked time, fixed fees and non-time invoices)
oas time edit ID --message TEXT   (changes only the message of a logged entry)
oas time list [--from D] [--to D] [--project P] [--activity A]
oas time report [--week | --month | --from D --to D] [--project P] [--activity A]
oas time export [--week | --month | --from D --to D] [--project P] [--activity A] [--out FILE]
oas project add NAME [--alias A]... [--client C] [--hourly SEK | --fixed SEK] | list | rate PROJECT --hourly SEK | --fixed SEK | --clear   (rates are admin only)
oas activity add NAME [--message TEMPLATE] | list
oas company set NAME [--org-number N] [--contact-name N --contact-phone P --contact-email E] | show   (the AGI file needs the contact)
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
oas vat report [--quarter 2026-Q3] | config | verify-box BOX [--unverify] | box-not-used BOX [--used] | map-account ACCOUNT --box BOX
oas employee add NAME --identity YYYYMMDDNNNC | list | deactivate ID
oas payroll run --period 2026-10 --pay-date 2026-10-25 --line EMPLOYEE_ID=GROSS[:TAX] ...   (admin; TAX in SEK, or GROSS:30% for a percentage)
oas payroll list | show ID | approve ID | delete ID | agi ID [--out FILE] | rates | set-rate KEY VALUE | verify-rate KEY [--unverify] | not-used KEY [--used]
oas customer add NAME --address STREET,POSTCODE,CITY [--org-number N --vat-number V --email E] | list
oas invoice create --customer C --line 'Description|12.5|h|900' ... [--vat 25] [--due-days 30] | from-time --customer C --project P [--from D --to D] | list | show ID | issue ID | delete ID | html ID --out FILE
oas agent-readme

## Invoices
- Domestic customers only (country SE), VAT 25, 12 or 6 percent. `invoice from-time` bills a project's
  uninvoiced tracked hours at its hourly rate (or its fixed fee) and marks those entries invoiced so
  they cannot be billed twice. Set the rate first with `oas project rate`.
- Anyone can create a draft. Only an admin runs `invoice issue`, which assigns the next gap-free
  number, locks the invoice and proposes the ledger voucher (1510 / 3041-3043 / 2611-2631); a human
  still approves that voucher. Issuing needs the company address and the customer's address.
- Issued invoices cannot be changed or deleted. EU and export invoices, credit notes and payment
  matching are not built yet.

## Payroll
- Payroll is admin only; agents see masked identity numbers. Employees need a valid personnummer.
- Preliminary tax is whatever you pass per line (take it from Skatteverket's tax table for the
  employee's table and column, or give a percentage). The tables are not built in.
- Only cash salary is supported: no taxable benefits, absence, holiday pay or corrections.
- Every contribution rate starts UNVERIFIED. `ready_for_agi` stays false until the owner has
  checked each rate against Skatteverket and run `oas payroll verify-rate`. Do not tell the owner
  the AGI file is ready while it is false. The XML structure itself is also unverified until it
  passes Skatteverket's AGI test service.
- `oas payroll approve` locks the run and proposes a ledger draft (7210, 7510, 2710, 2731, 1930);
  a human still approves that draft.

## VAT
- Give each ledger line a VAT code where it applies, with --debit ACCOUNT=AMOUNT@CODE (for
  example 6540=1000@RCEU). The code goes on the NET line: RCEU or RCNONEU for a reverse charge
  purchase of services from an EU or non-EU supplier, S25/S12/S6 for a sale, SEUSVC for a service
  sold to another EU country. VAT itself goes on the VAT accounts (2611, 2614, 2641, 2645 ...),
  which are mapped to boxes. Reverse charge needs three lines: net cost with @RCEU, input VAT on
  2645 and output VAT on 2614.
- `oas vat report --quarter 2026-Q3` computes every box and lists checks. It never files anything.
- Every box starts UNVERIFIED. Only the owner marks a box verified, after checking it against
  Skatteverket's SKV 4700. Do not tell the owner a return is ready while `ready_to_file` is false.

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
    /// Employees
    Employee {
        #[command(subcommand)]
        action: EmployeeCmd,
    },
    /// Monthly payroll and the AGI file
    Payroll {
        #[command(subcommand)]
        action: PayrollCmd,
    },
    /// Customers
    Customer {
        #[command(subcommand)]
        action: CustomerCmd,
    },
    /// Customer invoices
    Invoice {
        #[command(subcommand)]
        action: InvoiceCmd,
    },
    /// VAT return preparation
    Vat {
        #[command(subcommand)]
        action: VatCmd,
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
    /// Income this month: earned so far and a month-end projection (excl. VAT)
    Income {
        /// Month as YYYY-MM, default the current month
        #[arg(long)]
        month: Option<String>,
    },
    /// Operating manual for agents
    AgentReadme,
}

#[derive(Subcommand)]
enum EmployeeCmd {
    Add {
        name: String,
        /// Personnummer or samordningsnummer, 12 digits
        #[arg(long)]
        identity: String,
    },
    List,
    Deactivate {
        id: i64,
    },
}

#[derive(Subcommand)]
enum PayrollCmd {
    /// Create a monthly pay run (admin only)
    Run {
        #[arg(long)]
        period: String,
        #[arg(long)]
        pay_date: String,
        /// EMPLOYEE_ID=GROSS[:TAX], TAX in SEK or as a percentage like 30%; repeatable
        #[arg(long = "line")]
        lines: Vec<String>,
    },
    List,
    Show {
        id: i64,
    },
    Approve {
        id: i64,
    },
    Delete {
        id: i64,
    },
    /// Write the AGI XML for an approved run
    Agi {
        id: i64,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Rates,
    SetRate {
        key: String,
        value: String,
    },
    VerifyRate {
        key: String,
        #[arg(long)]
        unverify: bool,
    },
    /// Mark a rate as not used by this company (it stops blocking the AGI file)
    NotUsed {
        key: String,
        /// Undo: the rate counts again
        #[arg(long)]
        used: bool,
    },
}

#[derive(Subcommand)]
enum CustomerCmd {
    Add {
        name: String,
        #[arg(long)]
        org_number: Option<String>,
        #[arg(long)]
        vat_number: Option<String>,
        /// Postal address (use \n for line breaks)
        #[arg(long)]
        address: Option<String>,
        /// Two-letter country code, default SE (only SE is supported for invoices)
        #[arg(long)]
        country: Option<String>,
        #[arg(long)]
        email: Option<String>,
    },
    List,
}

#[derive(Subcommand)]
enum InvoiceCmd {
    /// Create a draft invoice with one or more lines
    Create {
        #[arg(long)]
        customer: String,
        /// DESCRIPTION|QUANTITY|UNIT|UNIT_PRICE, repeatable, for example "Consulting|12.5|h|900"
        #[arg(long = "line")]
        lines: Vec<String>,
        /// VAT rate: 25, 12 or 6 (default 25)
        #[arg(long)]
        vat: Option<i64>,
        #[arg(long)]
        issue_date: Option<String>,
        /// Days until payment is due, default 30
        #[arg(long)]
        due_days: Option<i64>,
        #[arg(long)]
        notes: Option<String>,
    },
    /// Create a draft from a project's uninvoiced tracked time
    FromTime {
        #[arg(long)]
        customer: String,
        #[arg(long)]
        project: String,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        vat: Option<i64>,
        #[arg(long)]
        due_days: Option<i64>,
        #[arg(long)]
        issue_date: Option<String>,
        #[arg(long)]
        notes: Option<String>,
    },
    List,
    Show {
        id: i64,
    },
    /// Assign the invoice number, lock it and propose the ledger voucher (admin only)
    Issue {
        id: i64,
    },
    /// Delete a draft invoice (admin only)
    Delete {
        id: i64,
    },
    /// Write the printable invoice as an HTML file (open it in a browser and print to PDF)
    Html {
        id: i64,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Subcommand)]
enum VatCmd {
    /// Compute the boxes for a quarter and list what is still missing
    Report {
        /// For example 2026-Q3; defaults to the current quarter
        #[arg(long)]
        quarter: Option<String>,
    },
    /// Show boxes, VAT codes and account mappings
    Config,
    /// Mark a box as checked against Skatteverket (admin only)
    VerifyBox {
        #[arg(name = "BOX")]
        vat_box: String,
        #[arg(long)]
        unverify: bool,
    },
    /// Mark a box as never used by this company (stops blocking readiness; amounts in it still fail the report)
    BoxNotUsed {
        #[arg(name = "BOX")]
        vat_box: String,
        #[arg(long)]
        used: bool,
    },
    /// Map an account to an output or input VAT box (admin only; clears that box's verification)
    MapAccount {
        account: i64,
        #[arg(long = "box")]
        vat_box: String,
    },
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
    Import {
        file: PathBuf,
    },
    List {
        #[arg(long)]
        status: Option<String>,
    },
    /// Re-run matching of receipts against bank lines
    Match,
    Ignore {
        id: i64,
    },
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
    Confirm {
        id: i64,
    },
    Decline {
        id: i64,
    },
}

#[derive(Subcommand)]
enum CompanyCmd {
    Set {
        name: String,
        #[arg(long)]
        org_number: Option<String>,
        /// Technical contact named in the AGI file
        #[arg(long)]
        contact_name: Option<String>,
        #[arg(long)]
        contact_phone: Option<String>,
        #[arg(long)]
        contact_email: Option<String>,
        /// Company address for invoices (use \n for line breaks)
        #[arg(long)]
        address: Option<String>,
        /// VAT number; defaults to SE + the organisation number + 01 when empty
        #[arg(long)]
        vat_number: Option<String>,
        /// Payment details printed on invoices, for example "Bankgiro 123-4567"
        #[arg(long)]
        payment_info: Option<String>,
        /// The company is approved for F-tax (printed on invoices)
        #[arg(long)]
        f_skatt: bool,
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
    Show {
        id: i64,
    },
    Approve {
        id: i64,
    },
    Reject {
        id: i64,
    },
    /// Post a reversing voucher for a posted voucher
    Correct {
        id: i64,
    },
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
    Revoke {
        id: i64,
    },
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
    /// Change the message of a logged entry
    Edit {
        id: i64,
        #[arg(long)]
        message: String,
    },
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
        /// Hourly rate in SEK (admin only)
        #[arg(long, conflicts_with = "fixed")]
        hourly: Option<String>,
        /// Fixed fee in SEK (admin only)
        #[arg(long)]
        fixed: Option<String>,
        /// Currency, default SEK
        #[arg(long)]
        currency: Option<String>,
    },
    List,
    /// Set or clear a project's hourly rate or fixed fee (admin only)
    Rate {
        /// Project name or alias
        project: String,
        #[arg(long, conflicts_with_all = ["fixed", "clear"])]
        hourly: Option<String>,
        #[arg(long, conflicts_with = "clear")]
        fixed: Option<String>,
        #[arg(long)]
        currency: Option<String>,
        /// Remove the rate
        #[arg(long)]
        clear: bool,
    },
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
        Self {
            code: code.into(),
            message: message.into(),
            projects: vec![],
        }
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
    PathBuf::from(home)
        .join(".config")
        .join("oas")
        .join("config.json")
}

fn load_config() -> Result<Config, Failure> {
    let stored: Option<Value> = std::fs::read_to_string(config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());
    let field = |env: &str, key: &str| {
        std::env::var(env).ok().or_else(|| {
            stored
                .as_ref()
                .and_then(|v| v[key].as_str().map(String::from))
        })
    };
    match (field("OAS_URL", "url"), field("OAS_TOKEN", "token")) {
        (Some(url), Some(token)) => Ok(Config {
            url: url.trim_end_matches('/').into(),
            token,
        }),
        _ => Err(Failure::new(
            "auth_required",
            "not logged in; run `oas auth login <url>`",
        )),
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
    if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    }
}

fn resolve_range(r: &Range) -> (Option<String>, Option<String>) {
    let today = Local::now().date_naive();
    if r.week {
        let monday = today - Duration::days(today.weekday().num_days_from_monday() as i64);
        return (
            Some(monday.to_string()),
            Some((monday + Duration::days(6)).to_string()),
        );
    }
    if r.month {
        let first = today.with_day(1).unwrap_or(today);
        let next = if today.month() == 12 {
            chrono::NaiveDate::from_ymd_opt(today.year() + 1, 1, 1)
        } else {
            chrono::NaiveDate::from_ymd_opt(today.year(), today.month() + 1, 1)
        }
        .unwrap_or(today);
        return (
            Some(first.to_string()),
            Some((next - Duration::days(1)).to_string()),
        );
    }
    (r.from.clone(), r.to.clone())
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into()
}

fn request(
    cfg: &Config,
    method: &str,
    path: &str,
    body: Option<&Value>,
) -> Result<(u16, String), Failure> {
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
    let mut resp =
        result.map_err(|e| Failure::new("network", format!("cannot reach {}: {e}", cfg.url)))?;
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

fn post_bytes(
    cfg: &Config,
    path: &str,
    content_type: &str,
    bytes: &[u8],
) -> Result<Value, Failure> {
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
        serde_json::from_str(&text)
            .map_err(|_| Failure::new("internal", "invalid JSON from server"))
    } else {
        Err(failure_from(&text))
    }
}

fn mime_for(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .as_deref()
    {
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

fn meta_query(
    vendor: &Option<String>,
    amount: &Option<String>,
    currency: &Option<String>,
    date: &Option<String>,
    notes: &Option<String>,
) -> Vec<String> {
    let mut parts = vec![];
    for (k, v) in [
        ("vendor", vendor),
        ("amount", amount),
        ("currency", currency),
        ("date", date),
        ("notes", notes),
    ] {
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
        message: e["message"]
            .as_str()
            .unwrap_or(
                "the reply is not from an open-accounting-sweden server; check the instance URL (oas auth login <url>)",
            )
            .into(),
        projects: e["projects"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn call(cfg: &Config, method: &str, path: &str, body: Option<&Value>) -> Result<Value, Failure> {
    let (status, text) = request(cfg, method, path, body)?;
    if (200..300).contains(&status) {
        serde_json::from_str(&text)
            .map_err(|_| Failure::new("internal", "invalid JSON from server"))
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
                eprintln!(
                    "No project given. Existing projects: {}",
                    f.projects.join(", ")
                );
                let name = ask("Project (type a new name to create it): ");
                if name.is_empty() {
                    return Err(f);
                }
                body["project"] = json!(name);
            }
            Err(f) if interactive && f.code == "project_not_found" => {
                let typed = body["project"].as_str().unwrap_or("").to_string();
                if f.projects.is_empty() {
                    if ask(&format!("Project '{typed}' not found. Create it? [y/N] "))
                        .to_lowercase()
                        == "y"
                    {
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
        Cmd::Auth {
            action:
                AuthCmd::Login {
                    url,
                    username,
                    token,
                },
        } => {
            let url = url.trim_end_matches('/').to_string();
            let token = match (token, username) {
                (Some(t), _) => t.trim().to_string(),
                (None, user) => {
                    let user = match user {
                        Some(u) => u,
                        None if std::io::stdin().is_terminal() => ask("Username: "),
                        None => {
                            return Err(Failure::new(
                                "invalid_input",
                                "give --username or --token",
                            ));
                        }
                    };
                    let password = read_password("Password: ")?;
                    let anon = Config {
                        url: url.clone(),
                        token: String::new(),
                    };
                    let body = json!({"username": user, "password": password, "ttl_hours": 720});
                    let v = call(&anon, "POST", "/login", Some(&body))?;
                    v["token"].as_str().unwrap_or_default().to_string()
                }
            };
            let cfg = Config {
                url: url.clone(),
                token,
            };
            let me = call(&cfg, "GET", "/me", None)?;
            save_config(&url, &cfg.token)?;
            print_value(
                &json!({"status": "ok", "url": url, "username": me["username"],
                "role": me["role"]}),
                json_out,
            );
            Ok(())
        }
        Cmd::Auth {
            action: AuthCmd::Logout,
        } => {
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
                    if std::io::stdin().is_terminal()
                        && read_password("Repeat new password: ")? != new
                    {
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
                        print_value(
                            &json!({"status": "ok", "id": v["id"], "name": v["name"],
                            "saved": "this machine now uses the permanent token"}),
                            json_out,
                        );
                    } else {
                        print_value(&v, json_out);
                    }
                }
                TokenCmd::List => print_value(&call(&cfg, "GET", "/tokens", None)?, json_out),
                TokenCmd::Revoke { id } => print_value(
                    &call(&cfg, "POST", &format!("/tokens/{id}/revoke"), None)?,
                    json_out,
                ),
            }
            Ok(())
        }
        Cmd::Auth {
            action: AuthCmd::Status,
        } => {
            let cfg = load_config()?;
            let me = call(&cfg, "GET", "/me", None)?;
            print_value(
                &json!({"status": "ok", "url": cfg.url, "username": me["username"],
                "role": me["role"], "can_approve": me["can_approve"]}),
                json_out,
            );
            Ok(())
        }
        Cmd::Time { action } => {
            let cfg = load_config()?;
            match action {
                TimeCmd::Log {
                    activity,
                    project,
                    date,
                    from,
                    to,
                    message,
                    tz,
                    create_project,
                } => {
                    let body = json!({"activity": activity, "project": project, "date": date,
                        "from": from, "to": to, "message": message, "timezone": tz,
                        "create_project": create_project});
                    print_value(&post_with_project(&cfg, "/time", body)?, json_out);
                }
                TimeCmd::Start {
                    activity,
                    project,
                    message,
                    tz,
                    create_project,
                } => {
                    let body = json!({"activity": activity, "project": project,
                        "message": message, "timezone": tz, "create_project": create_project});
                    print_value(&post_with_project(&cfg, "/time/start", body)?, json_out);
                }
                TimeCmd::Stop => print_value(&call(&cfg, "POST", "/time/stop", None)?, json_out),
                TimeCmd::Edit { id, message } => {
                    let body = json!({"message": message});
                    print_value(
                        &call(&cfg, "POST", &format!("/time/{id}/message"), Some(&body))?,
                        json_out,
                    );
                }
                TimeCmd::List(r) => {
                    let (f, t) = resolve_range(&r);
                    let q = query_string(&r, &f, &t);
                    print_value(&call(&cfg, "GET", &format!("/time{q}"), None)?, json_out);
                }
                TimeCmd::Report(r) => {
                    let (f, t) = resolve_range(&r);
                    let q = query_string(&r, &f, &t);
                    print_value(
                        &call(&cfg, "GET", &format!("/time/report{q}"), None)?,
                        json_out,
                    );
                }
                TimeCmd::Export { range, out, format } => {
                    if format != "csv" {
                        return Err(Failure::new(
                            "invalid_input",
                            "only --format csv is supported",
                        ));
                    }
                    let (f, t) = resolve_range(&range);
                    let q = query_string(&range, &f, &t);
                    let (status, text) =
                        request(&cfg, "GET", &format!("/time/export.csv{q}"), None)?;
                    if !(200..300).contains(&status) {
                        return Err(failure_from(&text));
                    }
                    match out {
                        Some(path) => {
                            std::fs::write(&path, &text)
                                .map_err(|e| Failure::new("internal", e.to_string()))?;
                            print_value(
                                &json!({"status": "ok", "file": path.display().to_string()}),
                                json_out,
                            );
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
                CompanyCmd::Set {
                    name,
                    org_number,
                    contact_name,
                    contact_phone,
                    contact_email,
                    address,
                    vat_number,
                    payment_info,
                    f_skatt,
                } => {
                    let body = json!({"name": name, "org_number": org_number, "contact_name": contact_name,
                        "contact_phone": contact_phone, "contact_email": contact_email,
                        "address": address.map(|a| a.replace("\\n", "\n")), "vat_number": vat_number,
                        "payment_info": payment_info.map(|a| a.replace("\\n", "\n")), "f_skatt": f_skatt});
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
                VoucherCmd::Propose {
                    date,
                    description,
                    counterparty,
                    debits,
                    credits,
                } => {
                    let mut lines = vec![];
                    for (list, side) in [(&debits, "debit"), (&credits, "credit")] {
                        for item in list {
                            let (acc, amount) = item.split_once('=').ok_or_else(|| {
                                Failure::new(
                                    "invalid_input",
                                    format!("expected ACCOUNT=AMOUNT, got '{item}'"),
                                )
                            })?;
                            let account: i64 = acc.trim().parse().map_err(|_| {
                                Failure::new("invalid_input", format!("invalid account '{acc}'"))
                            })?;
                            let (amount, vat) = match amount.split_once('@') {
                                Some((a, v)) => (a, Some(v.trim().to_string())),
                                None => (amount, None),
                            };
                            lines.push(
                                json!({"account": account, side: amount.trim(), "vat_code": vat}),
                            );
                        }
                    }
                    let body = json!({"event_date": date, "description": description,
                        "counterparty": counterparty, "lines": lines});
                    print_value(&call(&cfg, "POST", "/drafts", Some(&body))?, json_out);
                }
                VoucherCmd::Drafts => print_value(&call(&cfg, "GET", "/drafts", None)?, json_out),
                VoucherCmd::List { from, to } => {
                    let mut parts = vec![];
                    if let Some(f) = from {
                        parts.push(format!("from={}", encode(&f)));
                    }
                    if let Some(t) = to {
                        parts.push(format!("to={}", encode(&t)));
                    }
                    let q = if parts.is_empty() {
                        String::new()
                    } else {
                        format!("?{}", parts.join("&"))
                    };
                    print_value(
                        &call(&cfg, "GET", &format!("/vouchers{q}"), None)?,
                        json_out,
                    );
                }
                VoucherCmd::Show { id } => print_value(
                    &call(&cfg, "GET", &format!("/vouchers/{id}"), None)?,
                    json_out,
                ),
                VoucherCmd::Approve { id } => print_value(
                    &call(&cfg, "POST", &format!("/drafts/{id}/approve"), None)?,
                    json_out,
                ),
                VoucherCmd::Reject { id } => print_value(
                    &call(&cfg, "POST", &format!("/drafts/{id}/reject"), None)?,
                    json_out,
                ),
                VoucherCmd::Correct { id } => print_value(
                    &call(&cfg, "POST", &format!("/vouchers/{id}/correct"), None)?,
                    json_out,
                ),
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
                    if let Some(y) = year {
                        parts.push(format!("year={y}"));
                    }
                    if let Some(f) = from {
                        parts.push(format!("from={}", encode(&f)));
                    }
                    if let Some(t) = to {
                        parts.push(format!("to={}", encode(&t)));
                    }
                    let q = if parts.is_empty() {
                        String::new()
                    } else {
                        format!("?{}", parts.join("&"))
                    };
                    print_value(
                        &call(&cfg, "GET", &format!("/reports/trial-balance{q}"), None)?,
                        json_out,
                    );
                }
            }
            Ok(())
        }
        Cmd::Export {
            action: ExportCmd::Sie { year, out },
        } => {
            let cfg = load_config()?;
            let q = year.map(|y| format!("?year={y}")).unwrap_or_default();
            let (status, bytes) = request_bytes(&cfg, &format!("/export/sie{q}"))?;
            if !(200..300).contains(&status) {
                return Err(failure_from(&String::from_utf8_lossy(&bytes)));
            }
            match out {
                Some(path) => {
                    std::fs::write(&path, &bytes)
                        .map_err(|e| Failure::new("internal", e.to_string()))?;
                    print_value(
                        &json!({"status": "ok", "file": path.display().to_string(),
                        "bytes": bytes.len()}),
                        json_out,
                    );
                }
                None => {
                    std::io::stdout()
                        .write_all(&bytes)
                        .map_err(|e| Failure::new("internal", e.to_string()))?;
                }
            }
            Ok(())
        }
        Cmd::Employee { action } => {
            let cfg = load_config()?;
            match action {
                EmployeeCmd::Add { name, identity } => {
                    let body = json!({"name": name, "identity": identity});
                    print_value(&call(&cfg, "POST", "/employees", Some(&body))?, json_out);
                }
                EmployeeCmd::List => print_value(&call(&cfg, "GET", "/employees", None)?, json_out),
                EmployeeCmd::Deactivate { id } => print_value(
                    &call(&cfg, "POST", &format!("/employees/{id}/deactivate"), None)?,
                    json_out,
                ),
            }
            Ok(())
        }
        Cmd::Payroll { action } => {
            let cfg = load_config()?;
            match action {
                PayrollCmd::Run {
                    period,
                    pay_date,
                    lines,
                } => {
                    let mut out = vec![];
                    for item in &lines {
                        let (emp, rest) = item.split_once('=').ok_or_else(|| {
                            Failure::new(
                                "invalid_input",
                                format!("expected EMPLOYEE_ID=GROSS[:TAX], got '{item}'"),
                            )
                        })?;
                        let employee_id: i64 = emp.trim().parse().map_err(|_| {
                            Failure::new("invalid_input", format!("invalid employee id '{emp}'"))
                        })?;
                        let (gross, tax) = match rest.split_once(':') {
                            Some((g, t)) => (g.trim(), Some(t.trim())),
                            None => (rest.trim(), None),
                        };
                        let mut line = json!({"employee_id": employee_id, "gross": gross});
                        match tax {
                            Some(t) if t.ends_with('%') => {
                                let p: f64 = t.trim_end_matches('%').parse().map_err(|_| {
                                    Failure::new(
                                        "invalid_input",
                                        format!("invalid tax percentage '{t}'"),
                                    )
                                })?;
                                line["tax_percent"] = json!(p);
                            }
                            Some(t) => line["tax"] = json!(t),
                            None => {}
                        }
                        out.push(line);
                    }
                    let body = json!({"period": period, "pay_date": pay_date, "lines": out});
                    print_value(&call(&cfg, "POST", "/payroll/runs", Some(&body))?, json_out);
                }
                PayrollCmd::List => {
                    print_value(&call(&cfg, "GET", "/payroll/runs", None)?, json_out)
                }
                PayrollCmd::Show { id } => print_value(
                    &call(&cfg, "GET", &format!("/payroll/runs/{id}"), None)?,
                    json_out,
                ),
                PayrollCmd::Approve { id } => print_value(
                    &call(&cfg, "POST", &format!("/payroll/runs/{id}/approve"), None)?,
                    json_out,
                ),
                PayrollCmd::Delete { id } => print_value(
                    &call(&cfg, "POST", &format!("/payroll/runs/{id}/delete"), None)?,
                    json_out,
                ),
                PayrollCmd::Agi { id, out } => {
                    let (status, bytes) =
                        request_bytes(&cfg, &format!("/payroll/runs/{id}/agi.xml"))?;
                    if !(200..300).contains(&status) {
                        return Err(failure_from(&String::from_utf8_lossy(&bytes)));
                    }
                    match out {
                        Some(path) => {
                            std::fs::write(&path, &bytes)
                                .map_err(|e| Failure::new("internal", e.to_string()))?;
                            print_value(
                                &json!({"status": "ok", "file": path.display().to_string(), "bytes": bytes.len(),
                                "warning": "UNVERIFIED structure: validate with Skatteverket's AGI test service before use"}),
                                json_out,
                            );
                        }
                        None => std::io::stdout()
                            .write_all(&bytes)
                            .map_err(|e| Failure::new("internal", e.to_string()))?,
                    }
                }
                PayrollCmd::Rates => {
                    print_value(&call(&cfg, "GET", "/payroll/rates", None)?, json_out)
                }
                PayrollCmd::SetRate { key, value } => {
                    let body = json!({"value": value});
                    print_value(
                        &call(
                            &cfg,
                            "POST",
                            &format!("/payroll/rates/{}", encode(&key)),
                            Some(&body),
                        )?,
                        json_out,
                    );
                }
                PayrollCmd::NotUsed { key, used } => {
                    let body = json!({"not_used": !used});
                    print_value(
                        &call(
                            &cfg,
                            "POST",
                            &format!("/payroll/rates/{}/unused", encode(&key)),
                            Some(&body),
                        )?,
                        json_out,
                    );
                }
                PayrollCmd::VerifyRate { key, unverify } => {
                    let body = json!({"verified": !unverify});
                    print_value(
                        &call(
                            &cfg,
                            "POST",
                            &format!("/payroll/rates/{}/verify", encode(&key)),
                            Some(&body),
                        )?,
                        json_out,
                    );
                }
            }
            Ok(())
        }
        Cmd::Income { month } => {
            let cfg = load_config()?;
            let q = month
                .map(|m| format!("?month={}", encode(&m)))
                .unwrap_or_default();
            print_value(&call(&cfg, "GET", &format!("/income{q}"), None)?, json_out);
            Ok(())
        }
        Cmd::Customer { action } => {
            let cfg = load_config()?;
            match action {
                CustomerCmd::Add {
                    name,
                    org_number,
                    vat_number,
                    address,
                    country,
                    email,
                } => {
                    let body = json!({"name": name, "org_number": org_number, "vat_number": vat_number,
                        "address": address.map(|a| a.replace("\\n", "\n")), "country": country, "email": email});
                    print_value(&call(&cfg, "POST", "/customers", Some(&body))?, json_out);
                }
                CustomerCmd::List => print_value(&call(&cfg, "GET", "/customers", None)?, json_out),
            }
            Ok(())
        }
        Cmd::Invoice { action } => {
            let cfg = load_config()?;
            let customer_id = |name: &str| -> Result<i64, Failure> {
                let v = call(&cfg, "GET", "/customers", None)?;
                let wanted = name.trim().to_lowercase();
                v["customers"]
                    .as_array()
                    .and_then(|l| {
                        l.iter().find(|c| {
                            c["name"]
                                .as_str()
                                .is_some_and(|n| n.to_lowercase() == wanted)
                        })
                    })
                    .and_then(|c| c["id"].as_i64())
                    .ok_or_else(|| {
                        Failure::new(
                            "not_found",
                            format!("no customer named '{name}'; add it with `oas customer add`"),
                        )
                    })
            };
            match action {
                InvoiceCmd::Create {
                    customer,
                    lines,
                    vat,
                    issue_date,
                    due_days,
                    notes,
                } => {
                    let mut out = vec![];
                    for l in &lines {
                        let p: Vec<&str> = l.split('|').collect();
                        if p.len() != 4 {
                            return Err(Failure::new(
                                "invalid_input",
                                format!("expected DESCRIPTION|QUANTITY|UNIT|UNIT_PRICE, got '{l}'"),
                            ));
                        }
                        out.push(json!({"description": p[0].trim(), "quantity": p[1].trim(), "unit": p[2].trim(), "unit_price": p[3].trim()}));
                    }
                    let body = json!({"customer_id": customer_id(&customer)?, "vat_rate": vat, "issue_date": issue_date,
                        "due_days": due_days, "notes": notes, "lines": out});
                    print_value(&call(&cfg, "POST", "/invoices", Some(&body))?, json_out);
                }
                InvoiceCmd::FromTime {
                    customer,
                    project,
                    from,
                    to,
                    vat,
                    due_days,
                    issue_date,
                    notes,
                } => {
                    let body = json!({"customer_id": customer_id(&customer)?, "project": project, "from": from, "to": to,
                        "vat_rate": vat, "due_days": due_days, "issue_date": issue_date, "notes": notes});
                    print_value(
                        &call(&cfg, "POST", "/invoices/from-time", Some(&body))?,
                        json_out,
                    );
                }
                InvoiceCmd::List => print_value(&call(&cfg, "GET", "/invoices", None)?, json_out),
                InvoiceCmd::Show { id } => print_value(
                    &call(&cfg, "GET", &format!("/invoices/{id}"), None)?,
                    json_out,
                ),
                InvoiceCmd::Issue { id } => print_value(
                    &call(&cfg, "POST", &format!("/invoices/{id}/issue"), None)?,
                    json_out,
                ),
                InvoiceCmd::Delete { id } => print_value(
                    &call(&cfg, "POST", &format!("/invoices/{id}/delete"), None)?,
                    json_out,
                ),
                InvoiceCmd::Html { id, out } => {
                    let (status, bytes) = request_bytes(&cfg, &format!("/invoices/{id}/html"))?;
                    if !(200..300).contains(&status) {
                        return Err(failure_from(&String::from_utf8_lossy(&bytes)));
                    }
                    std::fs::write(&out, &bytes)
                        .map_err(|e| Failure::new("internal", e.to_string()))?;
                    print_value(
                        &json!({"status": "ok", "file": out.display().to_string(), "bytes": bytes.len()}),
                        json_out,
                    );
                }
            }
            Ok(())
        }
        Cmd::Vat { action } => {
            let cfg = load_config()?;
            match action {
                VatCmd::Report { quarter } => {
                    let q = quarter
                        .map(|q| format!("?quarter={}", encode(&q)))
                        .unwrap_or_default();
                    print_value(
                        &call(&cfg, "GET", &format!("/vat/report{q}"), None)?,
                        json_out,
                    );
                }
                VatCmd::Config => print_value(&call(&cfg, "GET", "/vat/config", None)?, json_out),
                VatCmd::VerifyBox { vat_box, unverify } => {
                    let body = json!({"verified": !unverify});
                    print_value(
                        &call(
                            &cfg,
                            "POST",
                            &format!("/vat/boxes/{}/verify", encode(&vat_box)),
                            Some(&body),
                        )?,
                        json_out,
                    );
                }
                VatCmd::BoxNotUsed { vat_box, used } => {
                    let body = json!({"not_used": !used});
                    print_value(
                        &call(
                            &cfg,
                            "POST",
                            &format!("/vat/boxes/{}/unused", encode(&vat_box)),
                            Some(&body),
                        )?,
                        json_out,
                    );
                }
                VatCmd::MapAccount { account, vat_box } => {
                    let body = json!({"account": account, "box": vat_box});
                    print_value(&call(&cfg, "POST", "/vat/accounts", Some(&body))?, json_out);
                }
            }
            Ok(())
        }
        Cmd::Doc { action } => {
            let cfg = load_config()?;
            match action {
                DocCmd::Add {
                    file,
                    vendor,
                    amount,
                    currency,
                    date,
                    notes,
                } => {
                    let bytes = std::fs::read(&file).map_err(|e| {
                        Failure::new(
                            "invalid_input",
                            format!("cannot read {}: {e}", file.display()),
                        )
                    })?;
                    let name = file
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("upload")
                        .to_string();
                    let mut parts = vec![
                        format!("filename={}", encode(&name)),
                        "source=cli".to_string(),
                    ];
                    parts.extend(meta_query(&vendor, &amount, &currency, &date, &notes));
                    let v = post_bytes(
                        &cfg,
                        &format!("/documents?{}", parts.join("&")),
                        mime_for(&file),
                        &bytes,
                    )?;
                    print_value(&v, json_out);
                }
                DocCmd::List { status } => {
                    let q = status
                        .map(|s| format!("?status={}", encode(&s)))
                        .unwrap_or_default();
                    print_value(
                        &call(&cfg, "GET", &format!("/documents{q}"), None)?,
                        json_out,
                    );
                }
                DocCmd::Meta {
                    id,
                    vendor,
                    amount,
                    currency,
                    date,
                    notes,
                } => {
                    let body = json!({"vendor": vendor, "amount": amount, "currency": currency, "date": date, "notes": notes});
                    print_value(
                        &call(&cfg, "POST", &format!("/documents/{id}/meta"), Some(&body))?,
                        json_out,
                    );
                }
                DocCmd::Download { id, out } => {
                    let (status, bytes) = request_bytes(&cfg, &format!("/documents/{id}/file"))?;
                    if !(200..300).contains(&status) {
                        return Err(failure_from(&String::from_utf8_lossy(&bytes)));
                    }
                    match out {
                        Some(path) => {
                            std::fs::write(&path, &bytes)
                                .map_err(|e| Failure::new("internal", e.to_string()))?;
                            print_value(
                                &json!({"status": "ok", "file": path.display().to_string(), "bytes": bytes.len()}),
                                json_out,
                            );
                        }
                        None => std::io::stdout()
                            .write_all(&bytes)
                            .map_err(|e| Failure::new("internal", e.to_string()))?,
                    }
                }
            }
            Ok(())
        }
        Cmd::Bank { action } => {
            let cfg = load_config()?;
            match action {
                BankCmd::Import { file } => {
                    let bytes = std::fs::read(&file).map_err(|e| {
                        Failure::new(
                            "invalid_input",
                            format!("cannot read {}: {e}", file.display()),
                        )
                    })?;
                    print_value(
                        &post_bytes(&cfg, "/bank/import", "text/csv", &bytes)?,
                        json_out,
                    );
                }
                BankCmd::List { status } => {
                    let q = status
                        .map(|s| format!("?status={}", encode(&s)))
                        .unwrap_or_default();
                    print_value(
                        &call(&cfg, "GET", &format!("/bank/transactions{q}"), None)?,
                        json_out,
                    );
                }
                BankCmd::Match => print_value(&call(&cfg, "POST", "/bank/match", None)?, json_out),
                BankCmd::Ignore { id } => print_value(
                    &call(
                        &cfg,
                        "POST",
                        &format!("/bank/transactions/{id}/ignore"),
                        None,
                    )?,
                    json_out,
                ),
                BankCmd::Book {
                    id,
                    description,
                    debits,
                    credits,
                    docs,
                    bank_account,
                } => {
                    let mut lines = vec![];
                    for (list, side) in [(&debits, "debit"), (&credits, "credit")] {
                        for item in list {
                            let (acc, amount) = item.split_once('=').ok_or_else(|| {
                                Failure::new(
                                    "invalid_input",
                                    format!("expected ACCOUNT=AMOUNT, got '{item}'"),
                                )
                            })?;
                            let account: i64 = acc.trim().parse().map_err(|_| {
                                Failure::new("invalid_input", format!("invalid account '{acc}'"))
                            })?;
                            let (amount, vat) = match amount.split_once('@') {
                                Some((a, v)) => (a, Some(v.trim().to_string())),
                                None => (amount, None),
                            };
                            lines.push(
                                json!({"account": account, side: amount.trim(), "vat_code": vat}),
                            );
                        }
                    }
                    let body = json!({"description": description, "lines": lines, "documents": docs,
                        "bank_account": bank_account});
                    print_value(
                        &call(
                            &cfg,
                            "POST",
                            &format!("/bank/transactions/{id}/book"),
                            Some(&body),
                        )?,
                        json_out,
                    );
                }
            }
            Ok(())
        }
        Cmd::Match { action } => {
            let cfg = load_config()?;
            match action {
                MatchCmd::List { status } => {
                    let q = status
                        .map(|s| format!("?status={}", encode(&s)))
                        .unwrap_or_default();
                    print_value(&call(&cfg, "GET", &format!("/matches{q}"), None)?, json_out);
                }
                MatchCmd::Confirm { id } => print_value(
                    &call(&cfg, "POST", &format!("/matches/{id}/confirm"), None)?,
                    json_out,
                ),
                MatchCmd::Decline { id } => print_value(
                    &call(&cfg, "POST", &format!("/matches/{id}/decline"), None)?,
                    json_out,
                ),
            }
            Ok(())
        }
        Cmd::Project { action } => {
            let cfg = load_config()?;
            match action {
                ProjectCmd::Add {
                    name,
                    aliases,
                    client,
                    hourly,
                    fixed,
                    currency,
                } => {
                    let (rate_type, rate) = match (hourly, fixed) {
                        (Some(h), _) => (Some("hourly"), Some(h)),
                        (None, Some(f)) => (Some("fixed"), Some(f)),
                        _ => (None, None),
                    };
                    let body = json!({"name": name, "aliases": aliases, "client": client,
                        "rate_type": rate_type, "rate": rate, "currency": currency});
                    print_value(&call(&cfg, "POST", "/projects", Some(&body))?, json_out);
                }
                ProjectCmd::Rate {
                    project,
                    hourly,
                    fixed,
                    currency,
                    clear,
                } => {
                    let projects = call(&cfg, "GET", "/projects", None)?;
                    let wanted = project.trim().to_lowercase();
                    let found = projects["projects"].as_array().and_then(|list| {
                        list.iter().find(|p| {
                            p["name"]
                                .as_str()
                                .is_some_and(|n| n.to_lowercase() == wanted)
                                || p["aliases"].as_array().is_some_and(|a| {
                                    a.iter().any(|x| {
                                        x.as_str().is_some_and(|x| x.to_lowercase() == wanted)
                                    })
                                })
                        })
                    });
                    let Some(p) = found else {
                        let names: Vec<String> = projects["projects"]
                            .as_array()
                            .map(|l| {
                                l.iter()
                                    .filter_map(|p| p["name"].as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();
                        return Err(Failure {
                            code: "project_not_found".into(),
                            message: format!("no project named '{project}'"),
                            projects: names,
                        });
                    };
                    let id = p["id"].as_i64().unwrap_or(0);
                    let (rate_type, rate) = match (hourly, fixed, clear) {
                        (Some(h), _, false) => ("hourly", Some(h)),
                        (None, Some(f), false) => ("fixed", Some(f)),
                        (None, None, true) => ("none", None),
                        _ => {
                            return Err(Failure::new(
                                "invalid_input",
                                "give --hourly AMOUNT, --fixed AMOUNT or --clear",
                            ));
                        }
                    };
                    let body = json!({"rate_type": rate_type, "rate": rate, "currency": currency});
                    print_value(
                        &call(&cfg, "POST", &format!("/projects/{id}/rate"), Some(&body))?,
                        json_out,
                    );
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
                ActivityCmd::List => {
                    print_value(&call(&cfg, "GET", "/activities", None)?, json_out)
                }
            }
            Ok(())
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let json_out = cli.json;
    if let Err(f) = run(cli) {
        let mut error = json!({"code": f.code, "message": f.message});
        // Only project errors carry the known projects, so an agent can ask which one was meant.
        if f.code.starts_with("project_") {
            error["projects"] = json!(f.projects);
        }
        let envelope = json!({ "error": error });
        if json_out {
            eprintln!("{}", serde_json::to_string(&envelope).unwrap_or_default());
        } else {
            eprint!("{}", serde_norway::to_string(&envelope).unwrap_or_default());
        }
        std::process::exit(f.exit_code());
    }
}
