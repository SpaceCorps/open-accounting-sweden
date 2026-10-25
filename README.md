# open-accounting-sweden

Self-hostable, agentic-first bookkeeping for Swedish small companies (aktiebolag). It keeps the books and the receipts, and prepares every number and document needed for the statutory reports, so an agent (or a human) can review and submit them on time.

> Status: pre-alpha. Nothing here is tax or legal advice. The company owner stays legally responsible for the books and filings.

## What works today

- **Time tracking:** projects, activity types with message templates, timer, CSV export.
- **Ledger:** append-only double-entry books, propose-and-approve postings, hash-chained integrity check, SIE 4 export, trial balance, audit log.
- **Receipts and bank:** immutable receipts archive (web, CLI or agent upload), bank CSV import, explainable receipt matching, bank lines booked as draft vouchers.
- **Invoices:** customers, invoices built from tracked hours at the project's hourly rate or fixed fee (hours can't be billed twice), a gap-free number series assigned at issue, a draft ledger voucher for the sale, and a printable HTML invoice. Domestic customers and 25, 12 or 6 percent VAT only.
- **VAT:** VAT codes on ledger lines and a quarterly report with readiness checks. Every box mapping starts UNVERIFIED until the owner checks it against SKV 4700. Nothing is filed automatically.
- **Payroll:** employees, monthly pay runs, employer contributions by age class, a locked run that proposes a ledger voucher, and an AGI XML file that validates against Skatteverket's published schema. Every rate starts UNVERIFIED and preliminary tax is entered per person.
- **Access:** username and password login, expiring sessions, permanent access tokens, and agent users that can propose but never approve.

Not built yet: EU and export invoices, credit notes, invoice payment matching, Skatteverket's tax tables, taxable benefits, absence and holiday pay, the annual report and INK2, live bank feeds.

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

## Tests and checks

There is no hosted CI. Run every check a PR must pass with:

```bash
scripts/check.sh            # fmt, clippy (warnings are errors), tests, web check and build, smoke test
```

Or one at a time:

```bash
cargo test --workspace      # core logic, API tests against an in-memory database, CLI error envelope
scripts/smoke.sh            # end to end against a throwaway server: auth, ledger rules, VAT, receipts, payroll
```

The smoke test needs `curl`, `sqlite3` and `python3` on the PATH (on Windows: `winget install SQLite.SQLite`).

To look at the web panel with realistic data, `scripts/demo.sh` starts a throwaway server with a month of demo bookkeeping on port 18990. Sign in as `demo` / `demo-password-123`.

## Web app

The web panel (`web/`) is a Vite+ and React app built on the SpaceCorps design system, [`@spacecorps/components`](https://github.com/SpaceCorps/storybook) (MIT): sidebar shell, dashboard, time, inbox, bank, ledger, VAT, payroll and settings. `oas-server` serves it when `OAS_WEB_DIR` points at the build.

`web/package.json` takes the library from a checkout of the storybook repository next to this one (`../storybook` from the repository root), so clone it there first, at the commit the `Dockerfile` pins:

```bash
git clone https://github.com/SpaceCorps/storybook ../storybook
git -C ../storybook checkout <STORYBOOK_REF from the Dockerfile>
(cd ../storybook && pnpm install && vp pack)
cd web && pnpm install && vp build                         # or `vp dev` with oas-server running on :8080
cd .. && OAS_WEB_DIR=$PWD/web/dist cargo run -p oas-server
```

The Docker image does the same: it clones the storybook repository at `STORYBOOK_REF`, builds it, then installs the web app from its committed lockfile, so deploying needs no registry token. To pick up library changes, bump `STORYBOOK_REF`, check out that commit in `../storybook`, rebuild it and run `pnpm install` in `web/` to refresh the lockfile.

## Architecture

| Part | Choice |
| :--- | :--- |
| Server and CLI | Rust (Edition 2024), shared core crate |
| Web UI | Vite+ (`vp`), React, `@spacecorps/components` from the SpaceCorps storybook |
| Storage | SQLite (WAL) on one volume for the MVP, receipts as files next to it; PostgreSQL is the planned option later |
| Auth | Username and password (Argon2id), expiring sessions, revocable access tokens; agent users cannot approve |
| Output | YAML by default, `--json` for agents, structured error envelopes |

## Open questions

See section 7 of [the legal requirements report](docs/reports/legal-requirements.md) for the full list. The ones that matter most:

1. Whether a single company can get production API access from Skatteverket for its own filings (until answered, filings are prepared here and entered in the e-service).
2. Bolagsverket's requirements for submitting digital annual reports.
3. BAS licensing for bundling the chart of accounts, and the SIE 4C record syntax.
4. Every VAT box, payroll rate and deadline is marked unverified until the owner checks it against Skatteverket.

## License

MIT, see [LICENSE](LICENSE). Open source and free to self-host.
