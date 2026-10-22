# open-accounting-sweden

Self-hostable, agentic-first bookkeeping for Swedish small companies (aktiebolag). It keeps the books and the receipts, and prepares every number and document needed for the statutory reports, so an agent (or a human) can review and submit them on time.

> Status: pre-alpha, design phase. Nothing here is tax or legal advice. The company owner stays legally responsible for the books and filings.

## Goals

- **Agent-first.** A Rust CLI is the primary interface. Every command has `--json`, stable exit codes and an `agent-readme` command, like the other SpaceCorps CLIs.
- **Self-hosted.** One server binary plus a database, deployable on Sliplane or any container host.
- **Compliant by construction.** Double-entry ledger on the BAS chart of accounts, immutable vouchers (verifikationer), a change log, receipt archiving for 7 years, SIE export, system documentation.
- **Report prep.** Deadline-driven preparation of VAT (moms), employer declarations (AGI), the annual report and the tax return (INK2), each with its numbers and the supporting receipts.
- **Secure.** Authenticated CLI sessions, encrypted secrets, audit trail, least-privilege tokens, encrypted backups.

## Planned architecture

| Part | Choice |
| :--- | :--- |
| Server and CLI | Rust (Edition 2024), shared core crate |
| Web UI | Vite+ (`vp`), React, `@spacecorps/components` from the SpaceCorps storybook |
| Storage | PostgreSQL for the ledger, object storage (S3-compatible or local volume) for receipts |
| Auth | Per-instance login for the CLI with revocable tokens kept in the OS keystore |
| Output | YAML by default, `--json` for agents, structured error envelopes |

## Open questions (to settle before code)

1. The full current legal requirements: Bokföringslagen and BFNAR rules, VAT, AGI, annual report and INK2 formats, deadlines, retention, and what counts as an acceptable digital system. These must be researched and cited, not assumed.
2. Which filings can be submitted through APIs and which need manual entry in the Skatteverket and Bolagsverket e-services.
3. Data model for vouchers, VAT codes (including reverse charge on foreign SaaS), payroll and benefits.
4. Threat model for holding payroll and financial data.

## License

TBD.
