# open-accounting-sweden

Self-hostable, agentic-first bookkeeping for Swedish small companies (aktiebolag). It keeps the books and the receipts, and prepares every number and document needed for the statutory reports, so an agent (or a human) can review and submit them on time.

> Status: pre-alpha. Working today: time tracking (projects, activity types, CSV export), an append-only double-entry ledger with propose-and-approve postings, a hash-chained integrity check, SIE 4 export, a trial balance, the `oas` CLI and a web panel. Not built yet: VAT, payroll/AGI, annual report, receipts archive, bank import. Nothing here is tax or legal advice. The company owner stays legally responsible for the books and filings.

## Try it

```bash
OAS_TOKEN=$(openssl rand -hex 32) OAS_AGENT_TOKEN=$(openssl rand -hex 32) cargo run -p oas-server
cargo run -p oas-cli -- auth login http://127.0.0.1:8080
```

`OAS_TOKEN` is a bootstrap admin token (can approve postings). Use it once to create your own account, then sign in with a username and password:

```bash
oas auth login http://127.0.0.1:8080 --token <OAS_TOKEN>
oas user add yourname --role admin        # prompts for a password (12+ characters)
oas auth login http://127.0.0.1:8080 --username yourname
oas token create laptop --save            # optional permanent access token for this machine
```

The web panel signs in with the same username and password. Users with role `agent` can propose and read but never approve, so create one for each agent, sign in as it and run `oas token create` to get a permanent `oas_...` token. Passwords are hashed with Argon2id, sessions expire, failed logins are rate limited, and tokens are stored only as hashes. Run `oas agent-readme` for the full command list.

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

MIT, see [LICENSE](LICENSE). Open source and free to self-host.
