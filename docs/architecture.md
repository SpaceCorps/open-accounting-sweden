# Architecture and data model (draft)

Status: draft for review. It builds on [the legal requirements report](reports/legal-requirements.md), which grades each claim as primary, secondary or unverified. Nothing here is legal or tax advice.

## Design principles

1. **The ledger is the product.** Legality turns on ledger integrity and archive design, which are well specified. Filing automation depends on onboarding with Skatteverket and Bolagsverket, which is not. Build the first part solidly and the second part as exports.
2. **Prepare, check, explain. Never file unattended.** Every filing route ends in a human signature (BankID in Skatteverket or Bolagsverket). The system produces filing packs; a person signs.
3. **Agents propose, humans approve.** An agent may create draft postings, but only an authenticated human approval turns a draft into a posted voucher or a filing pack into "ready".
4. **Rules are data.** Rates, limits, thresholds, deadlines and box mappings live in versioned, sourced tables that a human re-verifies each year, not in code.
5. **Valid by construction.** Exports (SIE4, AGI XML, SRU, VAT XML, iXBRL) are generated and validated, and can be tested against Skatteverket's public AGI test service.

## Components

| Component | Responsibility |
| :--- | :--- |
| `oas-core` (Rust crate) | Ledger, voucher rules, VAT and payroll logic, rule tables, export generators. No I/O. |
| `oas-server` (Rust) | HTTP API, auth, storage, scheduler for deadlines, archive and backups. |
| `oas` (Rust CLI) | Primary interface: authenticated against an instance, YAML output by default and `--json`, stable exit codes, `agent-readme`. |
| `web/` (Vite+ and React) | Review UI for approvals, receipts, reports and the deadline calendar, using `@spacecorps/components`. |
| PostgreSQL | Ledger, rule tables, audit log. |
| Document store | Original receipts and invoices, immutable, content-addressed. S3-compatible or a local volume. |

## Ledger integrity

These follow the primary-source requirements in the report (BFL 5 kap., BFNAR 2013:2 chapters 2, 5 and 9).

- **Append-only.** Posted entries can't be updated or deleted. The application database role has insert and select only on ledger tables, and triggers reject update and delete.
- **Corrections** are new linked entries (`corrects_voucher_id`) with their own voucher, user and timestamp. The original stays visible.
- **Voucher series** are gap-free per series, with a periodic integrity check. Each voucher stores the compile date and the event date separately.
- **Tamper evidence:** each voucher stores a hash chained to the previous one in its series. This goes beyond the law, so it is a nice-to-have, but it is cheap.
- **Behandlingshistorik:** every posting and every change to posting rules is recorded with actor and time. Rule-table versions are part of the history.
- **System documentation** is generated from the system: chart of accounts, account-use descriptions, posting-rule version history, verification chains, information flows and the archive plan.

## Core data model

| Table | Key fields |
| :--- | :--- |
| `company` | org number, legal name, financial year start, VAT period, employer registration |
| `financial_year` | company, start, end, status (open, closed) |
| `account` | BAS number, name, K2 allowed, VAT code default (licensing of the BAS plan is an open question) |
| `voucher` | series, number, compile date, event date, description, counterparty, status (draft, posted), created by, approved by, hash, corrects |
| `entry` | voucher, account, debit, credit, currency, rate, dimension or object, VAT code |
| `document` | content hash, original filename, mime, received at, stored path, scan metadata, linked voucher |
| `counterparty` | name, country, VAT number, EU or non-EU, VIES check result |
| `vat_code` | code, rate, ruta mapping, reverse-charge flag, valid from and to |
| `employee` | identity, salary terms, birth year (contribution rate), benefit settings |
| `pay_run` | month, payout date, status, AGI specification numbers |
| `pay_line` | employee, type, amount, tax, contribution, benefit values |
| `rule_table` | name, version, valid from and to, source URL, checked on, checked by |
| `filing_pack` | type, period, status (draft, ready, signed, filed), generated files, supporting documents, hash |
| `deadline` | type, period, due date, source, status |
| `audit_log` | actor (human or agent token), action, object, timestamp, request id |

## Reporting packs

Each filing pack is a folder of generated files plus a checklist of numbers and linked receipts.

| Pack | Generated files | Notes |
| :--- | :--- | :--- |
| VAT | SKV 4700 numbers, XML for e-service upload | Reverse charge: purchase in ruta 21 (EU seller) or 22 (non-EU), output VAT in 30/31/32, input VAT in 48. |
| AGI | XML to technical description 1.1.18.2 | Per-person specification numbers kept for later corrections. Validate on the public test service. |
| Year-end | SIE4 export, trial balance, K2 or K3 mapped statements | Accountants and other systems can read SIE. |
| INK2 | `info.sru`, `blanketter.sru` | Signed afterwards in Mina sidor. |
| Annual report | iXBRL file against the Swedish taxonomy | BankID signatures by board. Bolagsverket route is an open question. |

## CLI sketch

```
oas auth login <instance-url>      # BankID or token-based login, stored in the OS keystore
oas inbox add <file>               # upload a receipt or invoice
oas voucher propose --json         # agents create drafts
oas voucher approve <id>           # human only
oas vat pack --period 2026-Q4
oas agi pack --month 2026-11
oas year pack --year 2026
oas deadlines --next 30d
oas agent-readme [--json]
```

Conventions follow the other SpaceCorps CLIs: multi-account `--account`, YAML by default, `--json`, structured error envelope on stderr with stable exit codes, no empty stdout.

## Security

- Per-user and per-agent tokens with scopes. Agent tokens can propose and read, but not approve or sign.
- Secrets and bank credentials are encrypted at rest. The CLI keeps tokens in the OS keystore.
- Encrypted backups are stored separately from the data and restore-tested. Encryption must still allow immediate readable output.
- Data stays in Sweden, or in the EU with a Skatteverket notice (BFL 7 kap. 3 a §).
- Payroll data brings GDPR duties. The report did not research these with primary sources.

## Deployment

One container for the server and one for Postgres, deployable on Sliplane or any host. Volumes hold the document store and backups. Retention is seven years after the financial year, and the system must keep a way to print or read records for the whole period.

## Build order

1. Ledger, voucher rules, document archive and integrity checks, with SIE4 export and a generated system description.
2. CLI with auth, inbox and the propose-and-approve workflow.
3. VAT logic and pack, including reverse charge.
4. Payroll and AGI XML.
5. Year-end: closing, K2 mapping, SRU and iXBRL.
6. Web review UI.
7. Bank import and API clients, only after access is confirmed.

## Questions to settle first

Two answers could change the product, and each can be settled with one email before the architecture is frozen:

1. Whether a single company can get production API access from Skatteverket for its own filings.
2. Bolagsverket's requirements for submitting annual reports digitally (authentication, agreement, test environment, fee).

Also unresolved are BAS licensing for an open-source project, the kontantmetod question for an AB, and the thresholds for K2 and audit duty. The full prioritised list is in section 7 of the report.
