# Technical standards and integration options for a self-hosted Swedish bookkeeping system (2026)

Research date: 2026-10-22. Most findings come from web search summaries plus a few fetched pages. Several official PDFs (SIE 4C, BAS 2026, Skatteverket agreements) could not be text-extracted, so their contents are NOT verified here. Items from model background knowledge are labelled as such, in Inferences or Gaps only.

## 1. SIE file format (SIE 4)

### Takeaway
SIE 1-4 is the "classic" line-based format; SIE-Gruppen's current spec is "ver 4C" dated 2025-08-06. SIE 4 = type 3 plus all vouchers (#VER/#TRANS); 4i = vouchers only for subsystem import. SIE 5 is an XML successor (2018) that is much less widely supported.

### Cited Findings
- SIE types: Type 1 year balances; Type 2 adds period balances; Type 3 adds object balances; Type 4 adds all voucher entries for the fiscal year; Type 4i is vouchers only, used by subsystems (payroll, invoicing) to generate accounting orders for import — [sie.se/format](https://sie.se/format/)
- SIE 5 was introduced in 2018 as an XML-based successor with English tags, adds ledgers/asset registers, electronic-document handling and XMLDsig signing; the SIE 5 spec linked there is dated 2016-12-09 — [sie.se/format](https://sie.se/format/)
- Current SIE 1-4 spec: "SIE filformat ver 4C", 2025-08-06 — [PDF](https://sie.se/wp-content/uploads/2026/02/SIE_filformat_ver_4C_2025-08-06.pdf). Earlier 4B (2008-09-30) is at [sie.se](https://sie.se/wp-content/uploads/2020/05/SIE_filformat_ver_4B_080930.pdf), with an English copy hosted by PayEx at [wiki.payex.com](https://wiki.payex.com/xwiki/wiki/developer/download/Sandbox/Invoice%20Service/WebHome/SIE_filformat_ver_4B_ENGLISH.pdf)
- Record types named in the 4C document's listing include #ADRESS, #KONTO, #OBJEKT, #RES, #SIETYP, #TRANS, #UB, #VER; #FORMAT specifies the character set; #FLAGGA is a separate import flag — [search summary of the 4C PDF](https://sie.se/wp-content/uploads/2026/02/SIE_filformat_ver_4C_2025-08-06.pdf)
- Typical header order in a sample file: #FLAGGA 0, #PROGRAM, #FORMAT PC8, #GEN, #SIETYP 4, #KPTYP, #VALUTA SEK, #FNAMN, #ORGNR, then #KONTO, #KTYP, and #VER blocks with braces enclosing #TRANS — [PayEx SIE4 sample](https://developer.payex.com/xwiki/wiki/developer/view/Main/Invoicing/invoice-service/Technical%20reference/SIE4)
- Vendor docs list #SIETYP, #ORGNR, #PROGRAM, #GEN, #FNAMN as header items and dates as YYYYMMDD (secondary source) — [Deltek](https://help.deltek.com/product/maconomy/documentation/BPMReporting/CountryReports/Country_Sweden_Standard_Import_Export.html)
- sie.se page states no licensing terms for spec/sample files; contact is info@sie.se (Föreningen SIE-Gruppen) — [sie.se/format](https://sie.se/format/)

### Inferences
- From background knowledge (unverified here): SIE4 files are conventionally CP437 ("PC8") encoded, which is a common source of import bugs; #IB/#UB/#RES carry balances with year index (0 = current, -1 = previous), #RAR defines fiscal years, #DIM/#OBJEKT define dimensions. Verify against the 4C PDF before implementing.
- A self-hosted tool should implement SIE4 export (full) and SIE4 import (opening balances and vouchers) first; SIE 5 is optional.

### Gaps
- Could not extract the 4C PDF text, so exact record syntax, mandatory fields, and encoding rules are not confirmed. Spec reuse/licence terms unknown.
- What specific accountants/systems (Fortnox, Visma, Bokio) require/accept on import was not researched.

## 2. BAS kontoplan

### Takeaway
BAS 2026 is the current plan (PDF and XLS, also English), published by BAS-intressenternas Förening. The file for 2026 is dated 2026-04-14. Accounts marked "#" should not be used under K2. Licensing terms for software use were not found.

### Cited Findings
- bas.se offers BAS 2026 "for all company types except those preparing förenklat årsbokslut", plus BAS 2018 for K1 and K1 Mini, an archive back to BAS 2005, PDF and XLS formats, and an English 2026 version; licence questions go to info@bas.se — [bas.se/kontoplaner](https://www.bas.se/kontoplaner/)
- The 2026 PDF is `BAS_kontoplan_2026_v2.pdf`, 39 pages, created 14 April 2026 from Excel; a search summary says its header reads v 1.1 despite the filename (unclear/contradictory naming) — [PDF](https://www.bas.se/wp-content/uploads/2026/04/BAS_kontoplan_2026_v2.pdf)
- Per a search summary of the PDF: intangible assets are in class 10 (1012 capitalised software, 1040 licences); accounts marked # should not be used when K2 applies. Differences from 2025 are listed in a comparison Excel on bas.se (not opened) — [PDF](https://www.bas.se/wp-content/uploads/2026/04/BAS_kontoplan_2026_v2.pdf)

### Inferences
- Background knowledge (unverified): classes are 1 assets, 2 equity and liabilities, 3 revenue, 4 goods/materials/purchases, 5-6 other external costs, 7 personnel/depreciation, 8 financial items and appropriations/tax. A 4-digit account structure is the normal standard.
- Safe approach: bundle the plan only after confirming licence with BAS (info@bas.se), or have users import it.

### Gaps
- BAS licensing terms for embedding in commercial or open-source software: NOT found. Needs direct inquiry or reading "Allmänt om kontoplanen" on bas.se.
- Exact kontoklass descriptions and K2 vs K3 account differences not verified from the PDF.
- Whether a machine-readable official BAS dataset (with licence) exists was not established.

## 3. SRU files (tax return data)

### Takeaway
SRU is Skatteverket's file-transfer format for income tax data: a pair of files, info.sru and blanketter.sru, specified in brochure SKV 269. For INK2, API-based submission now exists, which may supersede file transfer.

### Cited Findings
- Two files, info.sru and blanketter.sru, must be sent together; blanketter.sru max 5 MB; renaming breaks transfer; ID numbers need a century digit (e.g. 16 for legal entities) with no hyphen; info.sru records are in fixed order and the block field names form and period (e.g. INK2S-2019P1) — [Skatteverket file transfer / SKV 269 results](https://skatteverket.se/filoverforing.4.46ae6b26141980f1e2d5e.html); spec editions at [SKV269_27](https://www.skatteverket.se/download/18.96cca41179bad4b1aad958/1636640681760/SKV269_27.pdf)
- A receipt is returned confirming reading, listing errors — [Skatteverket](https://skatteverket.se/foretag/inkomstdeklaration/forredovisningsbyraer/tekniskinformationomfiloverforing.4.13948c0e18e810bfa0cca8.html)
- A vendor claims all INK2 forms can be submitted via Skatteverket's newer service, making old SRU transfer unnecessary (vendor claim, unverified) — [Visma Spcs](https://support.vismaspcs.se/skatt-bokslut/en-se/content/online-help/tax-onlinehelp-ef-submission.htm)
- INK2 retrieve API returns pre-filled declaration data in PDF or SRU and shows whether a return was submitted — [tjänstebeskrivning ink2 api hämta](https://www7.skatteverket.se/portal-wapi/open/apier-och-oppna-data/utvecklarportalen/v1/getFile/tjanstebeskrivning-ink2-api-hamta)

### Inferences
- SRU generation per form/field code (SRU codes mapped from BAS accounts, e.g. via the Skatteverket/BAS mapping) is a self-contained, implementable task; no API registration required for manual upload.

### Gaps
- Current SKV 269 edition for 2026 not identified; whether manual SRU upload by a company is still offered for INK2 in 2026 not confirmed.

## 4. Skatteverket APIs and developer program

### Takeaway
Skatteverket runs a developer portal (Utvecklarportalen) with REST/JSON APIs and OAuth2 authorization code grant. Each API has a free sandbox ("komplett testtjänst") and a production ("driftsatt") tier requiring approval and a signed agreement with a "programvaruföretag". Key filings (moms, AGI, INK2) are prepared via API but the end user signs/submits at Skatteverket with e-legitimation.

### Cited Findings
- Sandbox: apply via "Begär tillgång till våra API:er", accept användarvillkor, receive client_id and client_secret by email; test data and sandbox URI per API — [Skatteverket anslutning och support](https://www.skatteverket.se/omoss/digitalasamarbeten/anslutningochsupportforvaraapierochoppnadata.4.7c708f0e16bed42cd055bf0.html)
- Production: company must meet API-specific requirements and sign agreement; apply via "Begär tillgång till driftsatt tjänst"; credentials delivered by email and SMS; register redirect URIs (https only); processing currently slow due to high volume; support weekdays 08-16 — same source
- APIs are REST/JSON, OAuth2 Authorization Code Grant — same source
- Moms (VAT) API went live November 2025; Fortnox connected February 2026. It creates/manages VAT return basis, fetches submitted returns and decided amounts; the signing that submits the return still occurs at Skatteverket (deep link to digital signing service) — [Fortnox](https://www.fortnox.se/fortnox-foretagsguide/driva-foretag/skatt/forenkla-momsdeklarationen-med-skatteverkets-nya-api); terms in [allmänna villkor momsdeklaration](https://www7.skatteverket.se/portal-wapi/open/apier-och-oppna-data/utvecklarportalen/v1/getFile/allmanna-villkor-momsdeklaration) (PDF, dated 2025-11-13, text not readable here)
- AGI (arbetsgivardeklaration) inlämning API: used by an employer's payroll system to upload basis and create review material; employer or authorised representative signs and submits in Mina sidor with e-identification; test host api.test.skatteverket.se, prod api.skatteverket.se; TLS 1.2, OAuth 2.0 ACG with organisation authenticated by organisation certificate; 300 MB file limit for the open API; tjänstebeskrivning v1.7; Skatteverket's feed lists disruptions in 2026 — [tjänstebeskrivning AGD inlämning](https://www7.skatteverket.se/portal-wapi/open/apier-och-oppna-data/utvecklarportalen/v1/getFile/tjanstebeskrivning-agd-inlamning); [avtal](https://www7.skatteverket.se/portal-wapi/open/apier-och-oppna-data/utvecklarportalen/v1/getFile/avtal-api-arbetsgivardeklaration-inlamning)
- Also in portal: AGD "hantera redovisningsperiod" v1.2, Skattekonto API v1.0 (payout account, day balance, past/upcoming transactions, payout block; for companies and representatives), "Fråga om skatteavdrag" API — [AGD hantera](https://www7.skatteverket.se/portal-wapi/open/apier-och-oppna-data/utvecklarportalen/v1/getFile/tjanstebeskrivning-agd-hantera-redovisningsperiod-v1/pdf/1.9.1/tjanstebeskrivning-agd-hantera-redovisningsperiod-v1.pdf), [skattekonto](https://www7.skatteverket.se/portal-wapi/open/apier-och-oppna-data/utvecklarportalen/v1/getFile/tjanstebeskrivning-skattekonto), [skatteavdrag](https://www7.skatteverket.se/portal-wapi/open/apier-och-oppna-data/utvecklarportalen/v1/getFile/avtal-api-fraga-om-skatteavdrag)
- INK2 submission API (v1.0, doc dated 2022-09-01) lets approved software load INK2 basis into the taxpayer's own space; taxpayer reviews and submits via Mina sidor with e-identification; client id/secret auth; Visma says INK3/INK4 can also be sent this way from 2025-P1 — [avtal ink2api inlämning](https://www7.skatteverket.se/portal-wapi/open/apier-och-oppna-data/utvecklarportalen/v1/getFile/avtal-ink2api-inlamning-v1), [Visma](https://support.vismaspcs.se/skatt-bokslut/en-se/content/online-help/tax-onlinehelp-ef-submission.htm)
- Agreements are with a "Programvaruföretag" per API; Skatteverket must approve the application — same agreements. There is also a page for "riktade partner-API:er" — [Skatteverket](https://www.skatteverket.se/omoss/digitalasamarbeten/kontaktaossomapieroppnadataochandradigitalasamarbeten/begartillgangtilldriftsattatjanster/begartillgangtillriktadepartnerapier.4.569165a01749e7ae7891ed8.html)

### Inferences
- Realistic automation level: prepare and upload (moms, AGI, INK2) via API, then human signs with BankID at Skatteverket (deep link). Fully unattended filing is not possible via these APIs as described.
- Whether a single company can obtain production access for its own filings (as a "programvaruföretag") is unclear: agreements are drafted for software vendors, and no source found states an exception for self-use. A self-hosted open-source tool would plausibly be required to register as a vendor per deployment/operator, or the user pre-uses the sandbox only. Flag as UNCERTAIN.
- Manual routes remain: SRU/file upload for INK2, Skatteverket's own e-tjänster for moms, AGI file upload (XML) in Skatteverket's e-tjänst.

### Gaps
- Full API list and statuses (kontrolluppgifter, F-skatt, ROT/RUT) not confirmed; the portal could not be fetched (utvecklare.skatteverket.se not resolvable, skatteverket.se/apierochoppnadata/utvecklarportalen 404). Likewise "Syntolkning" test data not found (the term may be a mis-recollection of test data; unverified).
- Ombud / delegated authorization mechanics (fullmakt via Skatteverket, "ombud" accounts) not documented in sources found.
- Eligibility, fees, certificate procurement for the organisation certificate (e.g. from a CA such as Telia/Nets) not found.
- API-agreement contents for moms (PDF unreadable).

## 5. Bolagsverket APIs

### Takeaway
Bolagsverket offers a free, contract-free "Värdefulla datamängder" API (OAuth2, REST/JSON) for reading company info and fetching filed annual reports. Annual report submission is done via digital filing with iXBRL through vendor software plus BankID signing; no public open submission API was found.

### Cited Findings
- Värdefulla datamängder API launched 3 Feb 2025; free, no contract; input org number; returns name, registration, business description, SNI codes and list of digitally filed annual reports; documents downloaded as ZIP by document ID; REST/JSON over HTTPS, OAuth2; 60 requests/minute/user; weekly file of newly filed annual reports as alternative — [Bolagsverket API page](https://bolagsverket.se/apierochoppnadata/hamtaforetagsinformation/vardefulladatamangder/apiforvardefulladatamangder.5513.html), [launch news](https://bolagsverket.se/apierochoppnadata/nyheterochreleaser/2025/vardefulladatamangderlanserasden3februarimerdataforsammakostnad.5504.html)
- The broader "API för att hämta företagsinformation" is paid (connection fee and per-transaction fees) with an agreement — [Bolagsverket](https://bolagsverket.se/apierochoppnadata/apiforatthamtaforetagsinformation.3988.html); agreement template [PDF](https://bolagsverket.se/download/18.37b754ba19ceb00666a562d5/1788438045725/avtal-om-tjanst-for-foretagsinformation-via-api-foretag-privat.pdf)
- Unofficial Elixir client uses OAuth2 client credentials with scope `vardefulla-datamangder:read`; token URL conflicting between README versions — [hex.pm bolagsverket_ex](https://readme.hex.pm/bolagsverket_ex/0.1.0) (third-party, unverified)
- Annual report digital filing: a gateway between accounting software and Bolagsverket using iXBRL; needs vendor software contracted with Bolagsverket; representative signs with BankID; over 1 million digital annual reports in 2024 — [Faronline](https://www.faronline.se/dokument/rattserien/ratt-bolagsratt/d/rb_digitalarsredovisning/), [Svenskt Näringsliv submission](https://www.regeringen.se/contentassets/93c8255c3891469aa76f62c51dab8a6a/svenskt-naringsliv.pdf), [RSM](https://www.rsm.global/sweden/sv/node/216), [Wolters Kluwer](https://www.wolterskluwer.com/sv-se/expert-insights/fakta-om-digital-inlamning-av-arsredovisning)
- Digital filing was not yet mandatory for all AB per Spiris (may be outdated) — [Spiris](https://www.spiris.se/blogg/teknik-sakerhet/digital-inlamning-av-arsredovisning)

### Inferences
- A self-hosted tool can generate iXBRL (K2/K3 taxonomy) but submitting needs Bolagsverket's inlämning service; vendor registration is likely required. Fallback: export iXBRL/PDF and file via Bolagsverket's web e-service or paper.

### Gaps
- Bolagsverket's official docs on the annual report inlämning service (vendor requirements, technical spec, 2026 status) not found; bolagsverket.se pages blocked fetching with a CAPTCHA.
- Official token endpoint and registration steps for the value-datasets API unconfirmed.

## 6. Peppol / e-invoicing

### Takeaway
Since 1 April 2019 Swedish public bodies must receive structured e-invoices; suppliers to them must send Peppol BIS Billing 3. Svefaktura is no longer recommended (since 2021). Private B2B Peppol is voluntary, and a self-hosted system would need an access point provider.

### Cited Findings
- Public-sector e-invoicing law applies from 1 April 2019; PDFs do not qualify; Försäkringskassan accepts Peppol BIS Billing 3 — [Försäkringskassan](https://www.forsakringskassan.se/download/18.2807d9c217c1f0156733e0/1680168372496/att-skicka-e-faktura-till-forsakringskassan.pdf), [Stripe overview](https://stripe.com/se/resources/more/e-invoicing-in-sweden)
- SFTI/DIGG no longer recommend Svefaktura for public-sector invoicing from 1 April 2021 and it can no longer be sent over Peppol (per Stripe page, secondary) — [Stripe](https://stripe.com/at/resources/more/e-invoicing-in-sweden)
- Peppol ID for Swedish organisations: organisation number with ICD 0007 per SFTI recommendation (secondary) — [B2Brouter](https://www.b2brouter.net/se/svenska-kommuner-valkomnar-peppol-och-eus-standard-for-e-faktura/)
- Private sector mandate: not automatic, may be agreed contractually (Stripe) — [Stripe](https://stripe.com/se/resources/more/e-invoicing-in-sweden)

### Inferences
- Background knowledge (unverified): bank e-faktura/AutoGiro and Peppol AP providers (e.g. Pagero, Basware, B2Brouter) are options; open-source Peppol components include phax's `phase4`/`peppol-commons` (Java) — verify.

### Gaps
- No authoritative DIGG/SFTI source retrieved for private-sector rules or the AP certification requirements; Peppol access point costs not researched. Bank e-faktura details not researched.

## 7. Open banking / PSD2 AISP options (incl. Lunar)

### Takeaway
Lunar has its own TPP Open Banking API (needs eIDAS TPP certificates and OAuth2). For a small company, the practical route is an aggregator that supports "restricted production" on own accounts (Enable Banking), but Lunar coverage in Enable Banking is unconfirmed. GoCardless Bank Account Data appears closed to new signups; Tink is sales-led.

### Cited Findings
- Lunar Open Banking: TPPs must register with valid eIDAS certificates; AIS scope covers accounts, transactions, attachments; OAuth2 authorization code flow; refresh via `offline`; base URL openbanking.prod.lunar.app/aisp-pisp; business accounts not addressed — [Lunar docs](https://docs.openbanking.lunar.app/api-overview); Lunar changelog adds an account usage field (private vs organisation) — [Lunar changelog](https://docs.openbanking.lunar.app/changelog). A separate banking-services portal exists — [docs.banking-services.lunar.app](https://docs.banking-services.lunar.app/)
- Enable Banking: production apps can be activated in restricted mode by linking your own accounts (only linked accounts returned, no KYB/contract), but not for offering a service to others; volume-based pricing with monthly minimum; licensed TPPs may use own eIDAS certs; business and personal accounts accessible for most banks; Swedish BankID SCA — [Enable Banking FAQ](https://enablebanking.com/docs/faq)
- GoCardless Bank Account Data: stopped accepting new accounts from July 2025 per Actual Budget/Firefly III docs (not confirmed by GoCardless) — [Actual Budget](https://actualbudget.org/docs/advanced/bank-sync/gocardless/), [Firefly III](https://docs.firefly-iii.org/how-to/data-importer/import/gocardless/)
- Tink: pricing on request, Standard/Enterprise plans, sandbox available — [itechguides](https://www.itechguides.com/products/tink/) (weak source)

### Inferences
- Under PSD2, accessing accounts as a TPP normally requires an AISP licence or registration; a company reading its own accounts via an aggregator in restricted mode avoids this. Direct use of Lunar's API without eIDAS certs is likely not possible for an individual company. UNCERTAIN.
- Fallbacks: CSV/Camt/SIE-style statement import from Lunar app, if available (not verified).

### Gaps
- Whether Lunar (business) is in Enable Banking's ASPSP list; Neonomics not researched; Lunar business-account CSV export not researched; consent re-authorization period (up to 180 days under PSD2 RTS) not sourced.

## 8. OCR / receipt extraction and legal acceptance

### Takeaway
Since 1 July 2024 original paper need not be kept if digitised copies exist; verification content requirements (BFL 5 kap. 7 §) still apply; retention 7 years. Sources found are commercial guides, not primary law.

### Cited Findings
- From 1 July 2024 the requirement to keep accounting material in original form was abolished if kept digitally — [Pleo](https://blog.pleo.io/sv/digitalisera-kvitton), [Sparkreceipt](https://sparkreceipt.com/sv-se/blogg/spara-kvitton-foretag/)
- Retention 7 years from end of financial year; info must be available immediately and readable; business owner remains responsible — same sources
- Verification content: date compiled, date of transaction, what it concerns, amount, counterparty; Skatteverket chapter on verifications — [Skatteverket kap41](https://skatteverket.se/download/18.deeebd105a602bfe38000123/1708608305612/kap41.pdf)

### Inferences
- OCR output should be stored as an aid alongside the immutable source image; a human or rule check should confirm extracted fields. No source on required accuracy; BFNAR 2013:2 not retrieved.

### Gaps
- Primary law text (BFL, BFNAR 2013:2 / 2023 updates), VAT receipt requirements (mervärdesskattelagen 17 kap.) not retrieved.

## 9. Security and data-protection obligations

### Takeaway
Not researched with primary sources in this pass.

### Cited Findings
- Retention, readability and responsibility rules from section 8 apply. Skatteverket says it uses onboarding, agreements and technical security solutions for API access — [Skatteverket anslutning](https://www.skatteverket.se/omoss/digitalasamarbeten/anslutningochsupportforvaraapierochoppnadata.4.7c708f0e16bed42cd055bf0.html)

### Inferences
- Background knowledge (unverified): GDPR applies to employee data (personnummer, salaries); IMY guidance; requires encryption at rest/in transit, role-based access, audit logging, backup with tested restore, data processing agreements where vendors involved; BFL retention (7 years) vs GDPR erasure tension.

### Gaps
- All of GDPR/IMY/BFL security specifics and BFL "arkivering" backup requirements need sourcing.

## 10. Open-source libraries

### Takeaway
Little reusable Swedish-specific OSS was found: a Rust SIE4 parser (MIT, stale since 2023) and a Ruby SIE gem (MIT). No SIE/BAS/AGI libraries in TypeScript found yet.

### Cited Findings
- `sie4` Rust crate: MIT, v0.1.1 (Nov 2023), repo akeamc/sie4, "SIE4 parser" — [lib.rs](https://lib.rs/crates/sie4)
- `barsoom/sie` Ruby gem: MIT, parser and generator for SIE types 1-4, 151 commits, 17 stars, last commit date unknown — [GitHub](https://github.com/barsoom/sie)
- `bolagsverket_ex` Elixir client for Värdefulla datamängder, early version; licence not checked — [hex.pm](https://readme.hex.pm/bolagsverket_ex/0.1.0)

### Inferences
- A new SIE4 reader/writer, SRU generator and Skatteverket API clients would likely be written from the specs.

### Gaps
- No search of GitHub for TypeScript/Python/Go SIE, AGI XML, BAS datasets or Peppol libraries was completed; licences beyond the above not checked.

## Key-question summaries
- Automation: moms (API since Nov 2025), AGI and INK2 can be prepared/uploaded via Skatteverket APIs with OAuth2 code grant plus organisation certificate, but final signing is by a human with e-legitimation at Skatteverket. Annual report: iXBRL via vendor gateway plus BankID signing.
- Vendor registration: apply for sandbox (client credentials by email), then production needs meeting API requirements and an agreement as "programvaruföretag". Whether an individual company can do this for its own filings is not confirmed (uncertain).
- Lunar ingestion: Lunar's own TPP API (eIDAS certs), or Enable Banking restricted mode if Lunar is supported (unconfirmed), or manual file import.
- Open source: see section 10.
