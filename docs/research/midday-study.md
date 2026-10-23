# Midday study: what open-accounting-sweden can learn

Source: shallow clone of `github.com/midday-ai/midday` (main, read only, nothing executed), studied 2026-10-23. Paths below are relative to the Midday repo. Not checked: the live product UI, midday.ai docs and changelog (code only, so UI behaviour is inferred from components). Everything here is described in our own words; no Midday code or schema text is reproduced.

## Licence finding (read first)

- `LICENSE` is the GNU AGPL-3.0, copyright Midday Labs AB. The README (`## License`) adds that AGPL covers "non-commercial use" and that commercial use or deployments with a setup fee need a separate commercial licence. That extra wording is a non-standard restriction of unclear legal weight; not verified with a lawyer.
- Consequence for us (MIT, public): do not copy or port code, SQL, prompts, schemas or doc text. Even a Rust translation of their scorer risks being a derivative work. Ideas, algorithm shapes and UX concepts are not copyrightable, so we describe and re-derive them independently. Names like `tracker_entries` are referenced only to point readers at the source. Whoever implements a feature should not paste from the files cited here.

## Product

Midday (README `## Features`): time tracker, invoicing, "Magic Inbox", Vault, CSV export for accountants, AI assistant. It targets freelancers and small firms, mainly US, Canada and EU.

- **Time tracker.** Dashboard components `apps/dashboard/src/components/tracker-*.tsx`: calendar with month and week views (`tracker/calendar-*.tsx`), timer (`tracker-timer.tsx`), project picker, status, schedule, CSV export, and "create invoice from tracked time" (`tracker-create-invoice.tsx`). The timer is global: one running timer per user, polled via `trackerEntries.getTimerStatus`, with a guard against a stale refetch restarting a just-stopped timer.
- **Project choice while tracking.** `tracker-select-project.tsx` is a single combobox with placeholder "Search or create project". Typing a name that does not exist offers inline create (`onCreate`), so a project is made without leaving the flow. Compare our design (confirm before create): theirs is faster but invites typo-projects, which suits a GUI and not an agent.
- **Export.** `tracker-export-csv.tsx` exports a date range (default current month), per project, with date, time as hours and minutes, and a total row. It formats client-side with the user's date format. We found no formula-injection escaping in this component (not checked further).
- **Invoicing.** `invoices` table: lines as JSON, a `token` for a public view link, viewed and reminder timestamps, a recurring table (`invoice_recurring`), templates, products. Customers have tags.
- **Inbox.** Email forwarding address, Gmail and Outlook connectors (`packages/inbox/src/providers/`), Slack and WhatsApp uploads, status flow, suggestions UI.
- **Vault.** Uploaded files are AI-classified into title, summary, date, language and tags; stuck items (over 10 minutes) are flagged for retry (`docs/document-processing.md`).
- **Banking.** Providers in `packages/banking/src/providers/`: `enablebanking`, `gocardless`, `plaid`, `teller`.
- **Assistant.** `apps/api/src/chat/` runs an LLM that uses the same MCP tools.

## Architecture

- **Monorepo** (bun workspaces, turbo): `apps/{api,dashboard,desktop,website,worker}`, `packages/*` (db, documents, inbox, banking, accounting, invoice, cli, encryption, jobs, ...). README lists Next.js, Supabase (Postgres, storage, realtime, auth), Railway, Trigger.dev, Typesense, Gemini and OpenAI.
- **API layer.** Hono REST under `apps/api/src/rest/routers/*` (e.g. `tracker-entries.ts`, `inbox.ts`) plus tRPC for the dashboard (`apps/api/src/trpc/routers/*`). Both share query functions in `packages/db/src/queries/`.
- **ORM and schema.** Drizzle, one file `packages/db/src/schema.ts` (about 4,200 lines, roughly 60 tables).
- **Jobs.** BullMQ worker (`apps/worker/src/processors/*`, `queues`, `schedulers`); the document doc describes deterministic job ids for dedupe.
- **Multi-tenancy.** Nearly every table has `team_id`; Postgres RLS policies call a private function `get_teams_for_authenticated_user()` (e.g. on `tracker_entries`, `tracker_projects`, `inbox`).
- **Data model (names only).**
  - `tracker_entries`: start, stop, date, duration in seconds, project, assignee, description, rate, currency, billed flag. No activity type, no timezone column, no overlap constraint seen.
  - `tracker_projects`: name, status, billable, rate, estimate, customer, full-text vector.
  - `transactions`: amount, currency, base amount and currency, name, merchant and counterparty, `internal_id` (provider id), status, tax amount, rate and type, category slug. One signed row per bank line; there is no postings table.
  - `inbox`: file path, amount, currency, date, display name, invoice number, tax fields, status, `transaction_id`, `grouped_inbox_id`.
  - `transaction_attachments`, `transaction_match_suggestions`, `documents` (path tokens, summary, tags, several FTS vectors), `invoices`, `customers`, `api_keys`, `oauth_*`, `accounting_sync_records`, `activities`.

## Receipt and document matching

Primary source: `docs/inbox-matching.md` (a good, short read) and `packages/db/src/utils/transaction-matching.ts`, `packages/db/src/queries/{transaction,inbox}-matching.ts`.

- **Extraction.** `process-attachment` job (`apps/worker/src/processors/inbox/process-attachment.ts`) converts HEIC to JPEG and calls an LLM classifier (`packages/documents/src/classifier/classifier.ts`; Gemini flash-class model via Vercel AI SDK, structured output) to get vendor, amount, currency, date, tax and invoice number. Prompts live in `packages/documents/src/prompts/`. Dedupe uses a reference id conflict and file path check, not a content hash that I could verify.
- **Candidate retrieval** is SQL: same team, date window, `pg_trgm` word similarity on names, plus amount proximity (same currency, or base currency for cross-currency).
- **Signals.** Four sub-scores: name (token similarity and containment), amount, currency, date (aware of invoice versus expense). Amount tiers by relative difference: within 1 percent scores about 0.98, 2 percent 0.95, 5 percent 0.85, 10 percent 0.6. It also recognises "amount differs by a VAT rate" (net versus gross). Boosts for exact amount with some name and date support; a zero-name-similarity penalty (confidence times about 0.55) and decline-memory penalty.
- **Learning.** Per-team alias memory (receipt name to bank-line name pairs), decayed hard-negative memory from declines and unmatches, never re-suggest a dismissed pair.
- **Thresholds.** Defaults: suggest at 0.6, auto-match at 0.9. Per-team calibration from labelled history nudges the suggest threshold within 0.55 to 0.85 (max 0.03 per step), auto threshold is suggest plus 0.24 clamped to 0.88 to 0.95. Auto-match also needs a "pattern safety gate" (repeated confirmations of that pair).
- **False matches.** Suggestions have statuses `pending`, `confirmed`, `declined`, `expired`, `unmatched`; users can unmatch an auto-match, which counts as weaker negative feedback. Every suggestion stores its four sub-scores and `match_details`, making it explainable. A read-only evaluation script (`packages/db/src/scripts/matching-eval-db.ts`) lists likely false positives and negatives. Their doc says they dropped an embedding-based matcher for this deterministic one.
- Matching runs both ways: new inbox item against transactions, and new transaction against open inbox items (`match-transactions-bidirectional.ts`).

## Agentic surface

- **MCP server** in `apps/api/src/mcp/` (`server.ts`, `tools/*.ts`), with tools per domain, e.g. in `tools/tracker.ts` list, get, create, update and delete for projects and entries. REST route `rest/routers/mcp.ts`.
- **CLI** `packages/cli` (`midday ...`): domains transactions, invoices, customers, tracker (start, stop, status, projects), documents, inbox, reports, search. Flags `--json` (default when piped), `--agent` (JSON, no prompts, no spinners), `--stdin`; browser OAuth login or `MIDDAY_API_KEY` via `--token-stdin` (`packages/cli/README.md`).
- **Auth and scoping.** API keys and OAuth apps (`api_keys`, `oauth_applications`, `oauth_access_tokens`). Keys are stored as both an encrypted value and a hash (`key_encrypted`, `key_hash`). Scopes are `<resource>.read` and `<resource>.write` plus `apis.all` and `apis.read` presets (`apps/api/src/utils/scopes.ts`). Dashboard users use Supabase JWTs verified against a JWKS (`apps/api/src/utils/auth.ts`).
- **Webhooks** are inbound only (`rest/routers/webhooks/*`: plaid, teller, stripe, polar, whatsapp, telegram, inbox). I found no outbound webhook feature (not exhaustively checked).
- Gap versus our principle: scopes are read and write only. There is no propose-versus-approve split, and agents can write records directly. Nothing resembles a draft state for agent writes.

## Security and self-hosting

- Security model: RLS keyed on team membership, plus the API using service connections and checks in code. `packages/encryption` uses AES-256-GCM for secrets. `SECURITY.md` is a disclosure contact only.
- Backups: nothing in the repo; delegated to Supabase.
- **Self-hosting is not realistically supported.** The README says docs for local dev are "being worked on"; production relies on Supabase, Trigger.dev, Typesense, Railway, Polar, bank aggregators and hosted LLMs. No compose file for running the product was found (only `docker-compose.test.yml`). The README's commercial-use clause adds a licence hurdle. This is the biggest opening for us.

## Gaps relative to Swedish bookkeeping law

Midday is expense and invoice tracking, not a ledger. Searched the code for double-entry, vouchers and SIE and found none in the product core.

- No double-entry: a transaction has one signed amount and a category, no debit and credit lines, no chart of accounts.
- No voucher series or gap-free numbering; invoice numbers are free text.
- No append-only or immutability: rows are editable and deletable (RLS includes update and delete policies on entries), and teams can be deleted by a worker job (`processors/teams/delete-team.ts`), which conflicts with BFL retention.
- No SIE, VAT return boxes, or AGI. Tax fields are informational (rate and type per row).
- Retention and archive: no seven-year rule, no immutable original storage.
- The one Swedish link is a Fortnox accounting export provider (`packages/accounting/src/providers/fortnox.ts`) that pushes vouchers into Fortnox, which shows that Midday leaves the legal ledger to another system.
- Time tracking has no timezone column and no overlap prevention; entries are not tied to activity types.

## What not to copy

- Single 4,000-line schema file, nullable `team_id` everywhere (several `team_id` columns are nullable), and cascade deletes from team and project down to entries. Our ledger must never cascade.
- Hard dependence on hosted services and a cloud LLM for core function.
- Auto-matching that attaches a document with no human gate. Fine for attachments, but our approval principle should cover any posting.
- Edit and delete of financial rows by ordinary users.
- Inline create-on-type for projects in agent flows.
- Any code, prompts or schema text (AGPL, see above).

## Prioritised takeaways for open-accounting-sweden

1. **Treat Midday's licence as a clean-room boundary.** Record in the repo contributor guide that no one pastes or translates Midday code, and that this note is the only derived input.
2. **Receipt matching as an explainable, deterministic score**, not an embedding. Use four signals (name, amount, currency, date), store sub-scores and the reason on every suggestion, and store the exact status lifecycle (pending, confirmed, declined, expired, unmatched). Our `document` to `voucher` link should go through such a suggestion record.
3. **Two thresholds, human by default.** Suggest at a moderate score; auto-link only at a high score and only for attachments to a draft, never for posting. Add a pair-history gate before any auto-link.
4. **Learn from feedback safely:** per-company alias memory, decayed negatives, never re-suggest a declined pair. Keep it per company.
5. **Swedish matching signals Midday lacks:** net versus gross by Swedish VAT rates (25, 12, 6 percent) is a useful idea they already have; add OCR number and Bankgiro or Plusgiro reference matching, and invoice number against bank text.
6. **Ship an offline evaluation command** (`oas inbox eval`): read-only, lists likely false positives and negatives. It makes threshold changes reviewable.
7. **Agent surface:** copy the shape (CLI `--json` and `--agent` with no prompts, same operations exposed as MCP tools, stdin input, API-key login) but add what they lack: propose versus approve scopes, `oas` agent tokens that cannot approve, and an audit log row per agent call.
8. **Time tracking UX:** one global running timer per user, a project combobox, and calendar week and month views. Keep our confirm-before-create rule and activity types; add the timezone and overlap checks they lack. Keep a "billed" flag and later "invoice from hours".
9. **Document processing resilience:** a processing status with retry, a stale-after-10-minutes flag, and graceful degradation so a failed extraction never blocks filing the original. Make extraction pluggable so a local model works and is optional (self-hosted, EU data).
10. **Self-host as the differentiator.** One compose file, SQLite or Postgres, local storage, no required SaaS. Document it from day one; Midday has none.
11. **Do not copy their data model for money.** Keep our append-only vouchers, corrections as linked entries, gap-free series, and no cascade deletes. Midday's transaction table is a good model for imported bank lines only, which should feed drafts.
12. **CSV export:** keep our formula-injection escaping (not seen in Midday), and also offer an accountant-friendly export per period plus SIE4, which Midday cannot produce.
