# Swedish AB bookkeeping legal requirements for a self-built digital bookkeeping system (as of 2026)

Source check date: 2026-10-22 for all sources. Primary texts actually read: Bokföringslagen (BFL) on riksdagen.se (via fetch summary, quotes short), SFS 2024:342 (PDF text extracted locally), BFNAR 2013:2 consolidated version (PDF text extracted locally, "senaste ändrad genom BFNAR 2017:6"). Everything else is search-snippet level and marked as such.

URLs:
- BFL: https://www.riksdagen.se/sv/dokument-och-lagar/dokument/svensk-forfattningssamling/bokforingslag-19991078_sfs-1999-1078/
- SFS 2024:342: https://svenskforfattningssamling.se/sites/default/files/sfs/2024-05/SFS2024-342.pdf
- BFNAR 2013:2 (kons.): https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf

## 1. Vouchers (verifikationer): required contents, numbering, timing

### Takeaway
Every affärshändelse needs a verifikation (BFL 5 kap. 6 §) containing the items in 5 kap. 7 §. Booking must follow 5 kap. 2 § and BFNAR 2013:2 ch. 3: cash payments by the next working day, other events "så snart det kan ske", with limited permitted deferral (up to 50 days after month-end, with the documents kept in order in the meantime). Voucher numbers must form an unbroken series that the system can verify.

### Cited Findings
- Duty: "För varje affärshändelse ska det finnas en verifikation." (5 kap. 6 §). Received information (e.g. supplier invoice) can serve as verifikation, supplemented under 5 kap. 7-8 §§. If the company has received nothing, it must compile its own verifikation (BFNAR 5.2). A hänvisningsverifikation is allowed (5 kap. 6 § st. 2, BFNAR 5.13-5.14; the system documentation or the reference voucher must say where the original is kept) — [BFL](https://www.riksdagen.se/sv/dokument-och-lagar/dokument/svensk-forfattningssamling/bokforingslag-19991078_sfs-1999-1078/); [BFNAR 2013:2](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)
- Mandatory contents, 5 kap. 7 §: date the voucher was compiled (sammanställts); date the event occurred; what it concerns; amount; counterparty (motpart); where relevant, reference to underlying documents and where they are available; a verifikationsnummer or other identifier; and any other info needed to link voucher to booked event "utan svårighet" — [BFL](https://www.riksdagen.se/sv/dokument-och-lagar/dokument/svensk-forfattningssamling/bokforingslag-19991078_sfs-1999-1078/)
- BFNAR 5.4: the 5 kap. 7 § data must be varaktiga (not erasable/made illegible). 5.5: "vad affärshändelsen avser" must allow identification and understanding. 5.6: motpart info must allow identification. 5.1: a verifikation documents an event or adjustment and may not at the same time be the registreringsordning/systematisk ordning presentation. 5.12: kontering on the voucher if needed to secure link voucher-post — [BFNAR 2013:2](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)
- 5 kap. 8 § (BFL): details may be omitted if inclusion is difficult and omission fits good practice; BFNAR 5.7 gives the anonymous-customer cash-sale example for omitting motpart — [BFL](https://www.riksdagen.se/sv/dokument-och-lagar/dokument/svensk-forfattningssamling/bokforingslag-19991078_sfs-1999-1078/); [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)
- Completing a received voucher (5 kap. 6 §, BFNAR 5.8): must not change received content; the voucher must show when the completion was made, by whom, and what was added — [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)
- Numbering (BFNAR 5.9-5.10): series of numbers/identifiers must be unbroken for a time period, run in sequence without skipped numbers, and "det ska i bokföringssystemet gå att kontrollera att verifikationsserien är obruten". Multiple series allowed if the books remain överskådliga and in good order; system documentation must then describe the series and periods (9.6) — [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)
- Joint vouchers: similar events may share one voucher (5 kap. 6 § st. 3; BFNAR ch. 6) — e.g. samlingsfaktura (period up to one month), automatically generated events (system needs internal control; BFNAR 6.3 lists replacement data: compile date, period, transaction type, amount per type, total, plus reference to system-produced breakdowns), same-day similar events — [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)
- Timing, 5 kap. 2 § BFL: cash in/out payments "senast påföljande arbetsdag"; other events as soon as possible (så snart det kan ske). BFNAR 3.1-3.3: next working day = first day after the event on which work is done in the business; if several days gap, book by the day after the event; "så snart det kan ske" means as soon as the company has sufficient information to book acceptably — [BFL](https://www.riksdagen.se/sv/dokument-och-lagar/dokument/svensk-forfattningssamling/bokforingslag-19991078_sfs-1999-1078/); [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)
- Receivables/payables booking may wait until invoice issued/received or should have been (BFNAR 3.4), applied consistently over the year and to both issued and received invoices — [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)
- Permitted deferral (BFNAR 3.5-3.8), only if vouchers are kept ordered as received/compiled in the meantime: up to 50 days after end of the month of the event (3.6); if annual net turnover normally <= SEK 3m, up to 50 days after end of the quarter (3.7); if <= SEK 1m and <= 50 vouchers / 250 events per year, up to 60 days after end of financial year (3.8). Deferral rules do not apply to cash receipts for registreringsordning presentation (cash payments ordering stays timely). Further deferral only for "särskilda hinder av tillfällig natur" (3.13) — [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)
- General standard: bookkeeping must be done "i överensstämmelse med god redovisningssed" (4 kap. 2 §); BFN is responsible for developing god redovisningssed (8 kap. 1 §) — [BFL](https://www.riksdagen.se/sv/dokument-och-lagar/dokument/svensk-forfattningssamling/bokforingslag-19991078_sfs-1999-1078/)

### Inferences
- A custom system should: generate/enforce gap-free numbered series with a verification check; store both date compiled and date of event separately; make voucher fields immutable after posting; support attached originals plus a "kompletteringslogg" (who/when/what).
- 50-day month-based deferral is a legal allowance, not a design target; the safe design is to post as documents arrive.

### Gaps
- Not read: BFN's own vägledning on verifikationer (bfn.se); no check whether BFN adopted any amendment to BFNAR 2013:2 after 2017:6 (a search snippet mentions a vägledning updated 2024-09-16 but not confirmed). Verify at bfn.se before finalizing.
- Exact BFL 5 kap. 7 § wording is quoted via a fetch tool's excerpts, not the full statute text.

## 2. Double entry, grundbokföring/huvudbokföring, behandlingshistorik, corrections

### Takeaway
BFL 5 kap. 1 § requires each transaction to be presented both in registreringsordning (journal/grundbok) and systematisk ordning (ledger/huvudbok). Records must be varaktiga; corrections must be done with a separate correction entry that states when and by whom, and the original must remain visible. Behandlingshistorik and systemdokumentation are required by 5 kap. 11 §.

### Cited Findings
- 5 kap. 1 § BFL: transactions must be recorded in order of registration and in systematic order, enabling completeness checks — [BFL](https://www.riksdagen.se/sv/dokument-och-lagar/dokument/svensk-forfattningssamling/bokforingslag-19991078_sfs-1999-1078/). Note: the term "double-entry" itself is not used in the sources read; double-entry is the practical consequence of the two presentations plus konteringsrules (inference, not a quoted rule).
- Mandatory fields per bokföringspost (BFNAR 2.3): registreringsordning, redovisningsperiod, verifikationsnummer/identifier, kontering, bokfört belopp. Registration order must show within each account and within each voucher series (2.2) — [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)
- Varaktighet (2.1): what is booked must not be erasable or made illegible — [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)
- Corrections: 5 kap. 5 § BFL — if a post is corrected, it must be stated when and by whom — [BFL](https://www.riksdagen.se/sv/dokument-och-lagar/dokument/svensk-forfattningssamling/bokforingslag-19991078_sfs-1999-1078/). BFNAR 2.17: in computer-based books a post must be corrected through a separate rättelsepost (manual books may strike through if no totals affected); 2.18: a rättelsepost is a new bokföringspost and must itself be documented by a verifikation. Correcting a verifikation (5.15-5.16): original data must clearly remain apparent, and a replaced voucher remains räkenskapsinformation — [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)
- 5 kap. 11 § BFL: company must prepare descriptions of the bookkeeping system's organisation and structure (systemdokumentation) and of processing (behandlingshistorik) so each entry can be traced — [BFL](https://www.riksdagen.se/sv/dokument-och-lagar/dokument/svensk-forfattningssamling/bokforingslag-19991078_sfs-1999-1078/); 4 kap. 1 p. 2 (as amended by SFS 2024:342) repeats the duty — [SFS 2024:342](https://svenskforfattningssamling.se/sites/default/files/sfs/2024-05/SFS2024-342.pdf)
- Behandlingshistorik content (BFNAR 9.16): shows the bokföringsposter added to the system and, in computer-based systems, registration date; also must describe changes in the bookkeeping system that affect how posts are treated, and when those changes were introduced. 9.15: system documentation describes how/where behandlingshistorik is created if needed for overview — [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)
- Reconciliation (2.15-2.16): books must be reconciled regularly under fixed routines; deviations and how handled must be documented — [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)
- Anläggningsregister (ch. 4): required unless few assets and books give sufficient overview; AB must also record revaluation fields (4.5-4.6) — [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)

### Inferences
- A compliant custom ledger should be append-only: no UPDATE/DELETE of posted entries; corrections as new reversing/correcting entries linked to the original, each with its own voucher, user id and timestamp; code/rule-changes (e.g. automatic konteringsregler, VAT logic versions) logged with date introduced (9.16 second paragraph).
- BFNAR does not prescribe audit-log fields beyond registration date, who/when for corrections and for voucher completions; recording user and timestamp on everything is a sensible superset.

### Gaps
- No BFN text found specifying more detailed log requirements (e.g. user ID on all entries); none seems to exist in BFNAR 2013:2.

## 3. Systemdokumentation and arkivbeskrivning/arkivplan

### Takeaway
BFNAR 2013:2 ch. 9 defines the content: kontoplan, account-use description, samlingsplan, arkivplan, plus conditional descriptions (voucher identification, verifieringskedjor, behandlingsregler, informationsflöden, conversion to readable form, behandlingshistorik). The term used is "arkivplan" (8.3), not "arkivbeskrivning".

### Cited Findings
- Basic content (9.2): a) kontoplan, b) description of how accounts are used and aggregated to årsbokslut/årsredovisning if not apparent from kontoplan, c) samlingsplan, d) arkivplan per 8.3. Companies with "enkla bokföringssystem" (manual column journal, or standard bookkeeping program with other parts manual) may skip these; but "Används automatkonteringar i bokföringsprogrammet anses bokföringssystemet inte vara enkelt" (9.5) — [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)
- Definitions: kontoplan (9.3: account names and numbers); samlingsplan (9.4: documentation of the system's structure and relationships between its parts); behandlingsregel (9.10: instruction for calculating amounts, kontering, aggregation etc.); verifieringskedja (9.8: references/identifiers allowing an event to be followed from verifikation through registreringsordning and systematisk ordning to årsbokslut/årsredovisning and back) — [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)
- Conditional content: how vouchers are identified if several series/non-numeric ids (9.6); verifieringskedjor description in cases listed (9.7); behandlingsregler that automatically affect account/amount or create new posts (9.9); informationsflöden if the system has several computer-based parts (9.11, covering all flows incl. parts creating/processing posts); how stored machine-readable information can be converted to readable form or microfilm (9.12); where/how behandlingshistorik is created (9.15) — [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)
- Arkivplan (8.3): required "om det behövs" to oversee archived information; must show what is archived and where it is kept, and if needed how the archive is built — [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)
- Legal basis: 5 kap. 11 § BFL (see section 2).

### Inferences
- A self-built system with automatic postings is by definition not an "enkelt bokföringssystem" (9.5), so full documentation of 9.2, 9.9, 9.11 and 9.12 applies. Documentation should include: chart of accounts and mapping to K2/K3 statements, architecture/data-flow diagram, automatic posting rules with version history, voucher numbering scheme, verification chain description (voucher ID -> journal entry -> ledger -> report), backup/restore and export-to-readable-format procedure, archive plan.

### Gaps
- No separate BFN document titled "arkivbeskrivning" was found; BFN guidance pages (bfn.se vägledning on systemdokumentation) were not fetched.

## 4. Retention, format, readability, location

### Takeaway
Räkenskapsinformation must be kept until the end of the 7th year after the calendar year in which the financial year ended, in Sweden (with EU-storage exception if notified to Skatteverket), durable, easily accessible, in orderly state, and producible as paper via immediate printout. Text below is the version in force from 1 July 2024 (SFS 2024:342).

### Cited Findings
- 7 kap. 1 § (as amended): räkenskapsinformation kept as paper, or as electronic document that "kan tas fram i en pappershandling genom en omedelbar utskrift". Received or self-created electronic documents must be kept "i det format och med det innehåll de hade" when received / when compiled — [SFS 2024:342](https://svenskforfattningssamling.se/sites/default/files/sfs/2024-05/SFS2024-342.pdf)
- 7 kap. 2 §: documents must be "varaktiga och lätt åtkomliga", kept "fram till och med det sjunde året efter utgången av det kalenderår då räkenskapsåret avslutades", stored "i Sverige, i ett ordnat skick och på ett betryggande och överskådligt sätt". Equipment and systems needed to produce paper copies must be kept available in Sweden for the whole period — [SFS 2024:342](https://svenskforfattningssamling.se/sites/default/files/sfs/2024-05/SFS2024-342.pdf)
- Example (inference from the statute): FY = calendar year 2026 -> kept through 31 Dec 2033.
- 7 kap. 3 §: paper vouchers may temporarily be stored abroad for special reasons if in line with good practice. 7 kap. 3 a §: electronic documents/equipment/systems may be stored in another EU country if (1) the location and every change of it is notified to Skatteverket, (2) the company on request gives Skatteverket or Tullverket immediate electronic access for control during the retention period, and (3) printout in Sweden is possible immediately. The same applies to non-EU countries with mutual-assistance legal instruments of similar scope (listed directives 2010/24/EU, 2011/16/EU, Reg. 904/2010) — [SFS 2024:342](https://svenskforfattningssamling.se/sites/default/files/sfs/2024-05/SFS2024-342.pdf)
- 7 kap. 4 §: Skatteverket may, for special reasons, permit electronic storage abroad even if 3 a § isn't met, with conditions/time limits — [SFS 2024:342](https://svenskforfattningssamling.se/sites/default/files/sfs/2024-05/SFS2024-342.pdf)
- 7 kap. 7 §: Skatteverket may in individual cases permit destruction before the retention period ends — [SFS 2024:342](https://svenskforfattningssamling.se/sites/default/files/sfs/2024-05/SFS2024-342.pdf)
- BFNAR 8.1-8.2: "betryggande" means stored so it does not risk being destroyed or lost; machine-readable data still in use (or on media where data is added/updated) must be backed up; frequency judged per company; "En säkerhetskopia ska förvaras åtskild från den kopierade räkenskapsinformationen." — [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)
- BFNAR 7.8: encryption allowed only if motivated by security and the information can be presented in clear text immediately (some delay accepted for legacy technology, 7.1, 8.4). 7.2: received electronic info that never took physical form may be converted to the company's system format if content isn't changed/lost; 7.3-7.4 on web-fetched info and info received both electronically and on paper. 7.5: self-created info is "sammanställd" when it reaches the form in which it is meant to be presented. 7.7: received non-durable media must be transferred to a durable one — [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)
- Cloud/foreign SaaS: BFNAR 8.4 and BFL 7 kap. 3 a § are the relevant rules; no source read explicitly addresses cloud providers.
- Liability for archiving after liquidation (8.13) rests with likvidator — [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)

### Inferences
- A self-hosted or cloud system must keep data (and the software able to print/export it) available in Sweden, or in the EU with notification to Skatteverket (and be able to give Skatteverket immediate electronic access). Hosting outside EU is restricted to listed-treaty countries or by Skatteverket permission. Because 7 kap. 1 § requires electronic records to be "kept in the format they had when received/compiled", store originals unaltered (e.g. original PDF/UBL XML) alongside any derived/converted copy, with hashes optional but sensible.
- Retention of the software/"system": the equipment and systems needed to produce paper must stay available for the whole retention period - a custom system needs a long-term export (e.g. human-readable PDF/CSV + SIE) or a maintained runtime.

### Gaps
- Skatteverket's own guidance pages (skatteverket.se) were not fetched. Riksdagen's consolidated page appears to show the same text as SFS 2024:342, but I did not diff them.
- Whether Skatteverket's EU-location notification has a specific form/process: not researched.
- Practical definition of "omedelbar utskrift" and "lätt åtkomliga": no BFN interpretation found beyond the BFNAR points above.

## 5. Scanning paper receipts and destroying originals; electronic invoices; legal change 2024

### Takeaway
7 kap. 6 § BFL allows destroying paper originals after transfer to another paper/electronic document if the transfer carries no risk of change or loss given technical methods and organisational measures. The text is current since 1 July 2024; the former rule that paper originals of received/third-party material had to be kept until the fourth year was reported removed by secondary sources, but this was not verified in the text I read.

### Cited Findings
- 7 kap. 6 § (as amended, in force 1 July 2024): company may destroy paper and electronic documents used to preserve räkenskapsinformation if the information is transferred to another paper or electronic document; allowed only if "överföringen med hänsyn till tekniska metoder, organisatoriska åtgärder och övriga omständigheter inte innebär risk för att räkenskapsinformationen förändras eller försvinner" — [SFS 2024:342](https://svenskforfattningssamling.se/sites/default/files/sfs/2024-05/SFS2024-342.pdf)
- SFS 2024:342 is dated 30 May 2024, published 4 June 2024, "träder i kraft den 1 juli 2024"; amends 4 kap. 1 and 4 §§ and 7 kap. 1-7 §§; transitional rule: information stored as microfilm may continue to be stored that way. Prop. 2023/24:78, bet. 2023/24:CU21 — [SFS 2024:342](https://svenskforfattningssamling.se/sites/default/files/sfs/2024-05/SFS2024-342.pdf)
- Secondary (not verified in statute text): a vendor blog and FAR Balans (May 2024) describe the change as allowing digitized receipts and invoices without keeping paper; earlier practice was a minimum 3-year (until fourth year) retention of paper originals. Treat as secondhand — [Spiris](https://www.spiris.se/blogg/foretagsredovisning/arkivering-bokforing); [FAR Balans May 2024](https://www.faronline.se/dokument/balans/2024/maj/balans_2024053001/)
- BFNAR 7.6: when information is transferred under 7 kap. 6 §, "ska det säkerställas att sambandet mellan det ursprungliga materialet och bokföringsposten kan följas" (link between original and booking must be traceable). 7.7: if both original and new copy exist (when 7 kap. 6 § destruction isn't used), both are kept for the full period — [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)
- Electronic invoices: BFNAR 7.2-7.4 apply (received e-invoice may be converted to the company's format if content isn't changed or lost; if received both electronically and on paper close together, the intended presentation form may be kept). A 2022 FAR snippet says a forwarded invoice e-mail needn't be saved if the invoice is in a compliant invoice-handling system (secondary, pre-2024) — [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf); [FAR Balans 2022](https://www.faronline.se/dokument/balans/2022/nr-5/balans_2022_n05_a0010/)
- Fixed document-quality threshold: neither the law nor BFNAR 2013:2 sets DPI or file-format rules for scans; only the "no risk of alteration or loss" standard in 7 kap. 6 §.

### Inferences
- A scanning workflow should document (as the "organisatoriska åtgärder"): scan quality check (legible, complete both sides), no post-scan editing, linking of scan to voucher number, checksum/immutable storage, backup, and a written procedure kept in systemdokumentation, before shredding originals.
- Practical caution: original-form rule means self-created e-vouchers and e-invoices (UBL/PDF) should be stored in received format.

### Gaps
- Skatteverket/BFN guidance on scanning quality after July 2024 not found/fetched. Whether other laws (e.g. customs, VAT law on certain documents, warranty, or the tax-agency demand for originals in specific cases) require retaining paper: not researched.
- E-invoicing specific rules (Peppol, 2019:??? lag om elektronisk faktura vid offentlig upphandling) not researched; for B2B the BFL rules above apply. Mention only as gap.
- VAT invoice content rules (mervärdesskattelagen 17 kap.) are outside this scope and not researched.

## 6. Foreign currency, cash vs accrual (kontantmetoden)

### Takeaway
Foreign-currency events are converted at the avistakurs on the event date (BFNAR 2.4), with allowed approximations. Kontantmetoden (booking when paid) is permitted by BFL 5 kap. 2 § st. 3 for companies with normal annual net turnover <= SEK 3m, but the sources read give conflicting signals on whether it is practically available to an AB.

### Cited Findings
- BFNAR 2.4: events in a foreign currency are converted to the reporting currency at spot rate on the date of the event; 2.5: may use an approximate rate if it doesn't deviate too much, or the rate applicable under mervärdesskattelagen (1994:200); 2.6: for kontantmetod companies the rate at payment may be used; 2.7: hedged events may follow hedge valuation rules — [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)
- Kontantmetod text, 5 kap. 2 § st. 3: a company whose "årliga nettoomsättning normalt uppgår till högst tre miljoner kronor" may delay booking until payment; at year-end all unpaid receivables and debts must be booked. St. 4 excludes banking/insurance companies etc. As read via fetch tool, the paragraph does not itself exclude aktiebolag — [BFL](https://www.riksdagen.se/sv/dokument-och-lagar/dokument/svensk-forfattningssamling/bokforingslag-19991078_sfs-1999-1078/); contradicted in tone by a FAR database summary found via search stating the method is for bookkeeping-obligated parties "som inte behöver upprätta årsredovisning" with turnover <= 3 mkr (which would exclude AB), but that result was a search paraphrase and may reflect older or differing text — [FAR Rättserien kontantmetoden](https://www.faronline.se/dokument/rattserien/redovisa-ratt/k/rr_kontantmetoden/)
- Net turnover definition 1 kap. 2 § BFL: income from goods and services in normal business less discounts, VAT and similar taxes — [BFL](https://www.riksdagen.se/sv/dokument-och-lagar/dokument/svensk-forfattningssamling/bokforingslag-19991078_sfs-1999-1078/)
- BFNAR 3.14-3.15: kontantmetod users still must keep vouchers ordered; deferral rules count time from the payment date — [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)

### Inferences
- Even if kontantmetoden is formally allowed in BFL for a small AB, an AB must also prepare an annual report (årsredovisning) and tax declaration on accrual principles; a design should support accrual (periodisering, kundfordringar/leverantörsskulder, bokslutstransaktioner per BFNAR 2.19) as the default. Treat any claim that an AB may use pure kontantmetod as UNVERIFIED (also from my background knowledge, tax rules (inkomstskattelagen) are narrower than BFL, but I did not verify this in a source).

### Gaps
- Conflict above on AB eligibility unresolved: need the exact text of BFL 5 kap. 2 § st. 3 (riksdagen.se direct read), BFN's kontantmetod guidance, and Skatteverket/verksamt.se pages.

## 7. Board and VD obligations; consequences of non-compliance

### Takeaway
The board has overall responsibility for organisation and control of bookkeeping and finances; VD must ensure bookkeeping is done according to law (per HD judgment B 1981-16, citing ABL). Non-compliance risks criminal liability for bokföringsbrott (11 kap. 5 § brottsbalken), plus administrative consequences (förseningsavgift, skattetillägg) which I did not verify from primary sources.

### Cited Findings
- HD case B 1981-16 (2017) refers to ABL 8 kap. 4 § for the board's duty to organise the company so that bookkeeping and funds management can be controlled in a satisfactory way, and to the VD's duty (ABL 8 kap. 29 §) to take measures needed for bookkeeping to comply with law; board members and VD can incur liability for bokföringsbrott; the board cannot just delegate and must follow up that delegation works — [HD B 1981-16](https://www.domstol.se/globalassets/filer/domstol/hogstadomstolen/avgoranden/2017/b-1981-16.pdf) (via a search summary; section numbers differ in another older source citing 8 kap. 3 §, so verify in current ABL text)
- Bokföringsbrott, 11 kap. 5 § brottsbalken (per search summaries of HD/Ekobrottsmyndigheten material): ordinary gradation fängelse up to 2 years; ringa: böter or fängelse up to 6 months; grovt: fängelse 6 months to 6 years; both intent and negligence are covered; prosecution of ringa brott requires public-interest assessment (11 kap. 8 §) — [Ekobrottsmyndigheten arf 2023-1](https://www.ekobrottsmyndigheten.se/wp-content/uploads/2023/04/arf-2023-1-bokforingsbrott.pdf); [HD B 3645-14](https://www.domstol.se/globalassets/filer/domstol/hogstadomstolen/avgoranden/2015/b-3645-14.pdf) (secondhand via search summary)
- Bolagsverket penalty page could not be fetched (bot-verification). Skatteverket skattetillägg pages were not fetched.

### Inferences
- Ledger features (immutable history, gap-free numbering, timely posting, archiving) directly map to the offences covered by 11 kap. 5 § BrB (failing to book, booking incorrectly, failing to keep records).

### Gaps
- Förseningsavgift for late annual report (Bolagsverket) amounts and escalation: not obtained. Skattetillägg (skatteförfarandelagen 49 kap.) and kontrolluppgifter/VAT-related penalties: not obtained. Bookkeeping-specific "förseningsavgift" does not exist in BFL as far as I can tell, but not verified. Revision/obligation to an auditor not in scope. These need verksamt.se / Bolagsverket / Skatteverket fetches.
- ABL paragraph numbers for the board duties must be checked against the current text.

## 8. Certification of bookkeeping software; what a custom system must document

### Takeaway
No source read mentions any approval or certification requirement for bookkeeping software; BFL 4 kap. 2 § only requires compliance with god redovisningssed and BFN develops that. (One exception: cash registers (kassaregister) must be certified under SFL 39 kap. 8 § when used, per BFNAR 1.12 — relevant only if the AB sells with a till.)

### Cited Findings
- BFL imposes duties on the company (4 kap. 1 §), not on software vendors; there is no certification regime in the BFL text or in BFNAR 2013:2 as read — [BFL](https://www.riksdagen.se/sv/dokument-och-lagar/dokument/svensk-forfattningssamling/bokforingslag-19991078_sfs-1999-1078/); [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf) (absence-of-evidence finding)
- BFNAR 1.12 / 9.13: cash register certification under skatteförfarandelagen (2011:1244) 39 kap. 8 §; for cash registers the system documentation must describe components and settings — [BFNAR](https://www.bfn.se/wp-content/uploads/2020/06/bfnar13-2-kons2.pdf)
- Documentation a custom system must produce: see section 3 (systemdokumentation incl. kontoplan, samlingsplan, arkivplan, conditional items) and behandlingshistorik (section 2).

### Inferences
- Using a custom system is permitted but moves the burden of proving compliance onto the company: the systemdokumentation is the main evidence.

### Gaps
- No positive confirmation from Skatteverket/BFN/verksamt.se that no approval is needed (only that none appears in the law and general advice). Also not covered: K2/K3 årsredovisning rules, SIE file export expectations, E-invoice law, GDPR vs 7-year retention.

## Overall notes for the report writer
- Most important current-law change: SFS 2024:342 (in force 1 July 2024) rewrote BFL 4 kap. 1, 4 §§ and 7 kap. 1-7 §§ (digitization/paper retention). BFNAR 2013:2 consolidated text read is dated through BFNAR 2017:6 and still refers to 7 kap. 1 § "första stycket 3" and 7 kap. 7.7 about paper originals, so some BFNAR cross-references may be stale relative to the 2024 law; BFN may have updated/will update the general advice (not verified).
- No 2026-specific changes were found; nothing found about upcoming changes for 2026-2027 (not researched exhaustively).
- Secondhand/uncertain items are marked above; verify penalties, kontantmetod/AB, ABL section numbers and post-2024 BFN guidance before relying on them.
