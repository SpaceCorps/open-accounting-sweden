# Swedish employer obligations and AGI (arbetsgivardeklaration på individnivå) for a small AB, 2026

Checked 2026-10-22. Method: web search plus fetches of Skatteverket pages. Several Skatteverket pages (PDF of Belopp och procent 2026, Wolters Kluwer) could not be read in full, so some figures are via search-engine summaries of secondary sources. Items flagged [SECONDHAND] or [UNVERIFIED] need confirmation before they go into product logic.

## Registration, monthly AGI content (huvuduppgift / individuppgift), and ledger derivation

### Takeaway
AGI is one monthly XML-shaped declaration with a huvuduppgift (employer totals: contributions and withheld tax) and one individuppgift per payee (cash pay, benefits, withheld tax, FK-numbered fields). Skatteverket cross-checks the huvuduppgift totals against its own calculation from the individuppgifter.

### Cited Findings
- Report employer contributions and withheld tax in the month after pay or benefit is paid out; a zero declaration is still filed if nothing is to report. Salary paid in January for December work belongs in the January declaration. — [Skatteverket, När ska jag lämna arbetsgivardeklaration](https://skatteverket.se/foretag/arbetsgivare/lamnaarbetsgivardeklaration/narskajaglamnaarbetsgivardeklaration.4.361dc8c15312eff6fd13c11.html)
- Deregistration: declarations must be filed for every month up to and including the deregistration date. — same source
- Huvuduppgift fields (tech. description 1.1.18.2): FK487 SummaArbAvgSlf (total employer contributions and särskild löneskatt; checked against Skatteverket's own calculation, control B_006); FK497 SummaSkatteavdr (total tax withheld, control B_007); FK486 profit-sharing foundation addition; FK475 R&D deduction; FK476 regional support deduction; FK477 shipping-company deduction; FK495 tax withheld on pension from insurers; FK496 tax withheld on interest/dividends. Pension, interest and dividend amounts are reported as totals on the huvuduppgift, not per recipient. — [Skatteverket, Teknisk beskrivning](https://www.skatteverket.se/foretag/arbetsgivare/lamnaarbetsgivardeklaration/tekniskbeskrivningochtesttjanst/tekniskbeskrivning1116.4.7eada0316ed67d7282a791.html) (a page-text summary; the page itself says full element names are in a separate XML structure attachment)
- Individuppgift fields named on the page: FK001 AvdrPrelSkatt (preliminary tax withheld); FK274 AvdrSkattSINK; FK275 AvdrSkattASINK; FK114 skattebefrielse enligt avtal; FK276 no tax deduction/not taxable in Sweden; FK253 local employee; FK094 embassy employee; FK011 KontantErsattningUlagAG (taxable cash pay subject to contributions); FK131 cash pay not subject to contributions; FK013 car benefit (taxable, contributions apply); FK018 fuel benefit with car (market value x 1.2); FK098 amount employee paid for fuel benefit; FK132 / FK133 taxable benefits not subject to contributions; FK010 offset of previously reported tax-free compensation; FK305 social security convention country. — same source
- Identity: FK215 BetalningsmottagarId (personnummer / samordningsnummer / organisationsnummer); FK222 birth date + up to 3 digits; FK224 other ID; FK252 TIN. If no Swedish ID, extra mandatory fields (FK216, FK217, address FK218-FK220 or FK230+FK221, FK077, FK078, FK081). — same source
- Mandatory on every huvud- and individuppgift: FK006 RedovisningsPeriod and FK201 AgRegistreradId. Individuppgift also needs FK570 Specifikationsnummer (unique per recipient, period and employer), one identity field, and exactly one of the seven tax fields (FK001, 274, 275, 114, 276, 253, 094); enter 0 if the tax field applies but nothing was withheld. — same source
- Corrected sjukavdrag-type "frånvaro" (absence) entries cannot be corrected or added after the first declaration for the period; draft removal uses FK820 Borttagsmarkering. — same source
- The same technical description mentions sickness-benefit cost fields on the huvuduppgift only for periods 201807 to 202406, i.e. that employer compensation field is not applicable to 2026. — same source
- Skatteverket's guide on filling in the declaration exists at [Så här fyller du i arbetsgivardeklaration](https://www.skatteverket.se/foretag/arbetsgivare/lamnaarbetsgivardeklaration/safyllerduiarbetsgivardeklarationen.4.2cf1b5cd163796a5c8b66a8.html) (found in search, not read).

### Inferences
- Ledger derivation (from BAS postings below): FK011 per person = sum of cash pay posted to 7010/7210/7220 etc. that is subject to contributions; FK001 per person = amounts credited to 2710 for that person; FK487 = the contribution amount credited to 2731 (7510 debit); FK497 = total 2710. A product should reconcile sum(individual FK001) = FK497 and recompute contributions per person (rate x base, rounded as Skatteverket does) because Skatteverket checks the totals.
- Registration as employer is done via Skatteverket's business registration (skatte- och arbetsgivarregistrering) and gives an F-skatt/arbetsgivarregistrering; AGI cannot be filed before registration. [UNVERIFIED: I did not retrieve a Skatteverket source on the registration procedure.]

### Gaps
- Full list of FK numbers for all benefit types (food, housing, other benefits, tax-free items reported with FK, sjuklön-related absence reporting) and the XML element names/namespace: the namespace URI and element list are in Skatteverket's XSD/structure attachment, which was not retrieved.
- Registration process source not retrieved.

## Due dates, exceptions and penalties

### Takeaway
Small companies (turnover up to 40 MSEK) file and pay by the 12th (17th for January and August payouts' declarations, per table; weekends/holidays roll forward). Over 40 MSEK: declaration on the 26th but payment on the 12th. Late fee 625 SEK (SOURCE CONFLICT on 1,250).

### Cited Findings
- 2026 table (payout month -> deadline for up to 40 MSEK / over 40 MSEK): Dec 2025 -> 19 Jan 2026 / 26 Jan; Jan -> 12 Feb / 26 Feb; Feb -> 12 Mar / 26 Mar; Mar -> 13 Apr / 27 Apr; Apr -> 12 May / 26 May; May -> 12 Jun / 26 Jun; Jun -> 13 Jul / 27 Jul; Jul -> 17 Aug / 26 Aug; Aug -> 14 Sep / 28 Sep; Sep -> 12 Oct / 26 Oct; Oct -> 12 Nov / 26 Nov; Nov -> 14 Dec / 28 Dec; Dec 2026 -> 18 Jan 2027 / 26 Jan 2027. — [Skatteverket](https://skatteverket.se/foretag/arbetsgivare/lamnaarbetsgivardeklaration/narskajaglamnaarbetsgivardeklaration.4.361dc8c15312eff6fd13c11.html)
- Turnover up to 40 MSEK: payment due the same date as the declaration. Over 40 MSEK: payment due on the 12th (17th in January) while the declaration is normally due the 26th. — same source
- Late fee (förseningsavgift) is 625 SEK; Skatteverket first sends a proposed decision the employer can answer in writing. — same source
- Another secondary source states 625 SEK first time and up to 1,250 SEK after föreläggande; a different snippet said 1,250 SEK applies if an order to file was not followed or the deadline was missed in one of the three preceding periods. — [Spiris](https://www.spiris.se/blogg/skatter/glomt-skattedeklarationen), [FAR Online summary](https://www.faronline.se/dokument/rl_arbetsgivardeklaration/) [SECONDHAND, conflicting; verify SFL ch. 48]
- Skatteverket's 2014 ställningstagande (updated 2021, partly outdated) says skattetillägg normally cannot be charged when an AGI difference can be corrected using kontrolluppgifter. — [FAR Online](https://www.faronline.se/dokument/skatteverket/stallningstaganden2/2014/skvstallningst2014030401/) [SECONDHAND]

### Inferences
- Because the AB is small, plan for the 12th rule with the Jan/Aug exception already embedded in the table (17 Aug for July; 19 Jan, 18 Jan 2027 for December payouts due to weekends).

### Gaps
- Skattetillägg percentages for wrong/missing AGI data, and the formal 'kontrolluppgifter replaced by AGI' penalty rules (kontrollavgift) were not confirmed from primary sources.
- Whether the 12th/26th wording "17th in January and August" from secondary sources matches the 2026 table (table shows the 17th for July payouts in August and 19 Jan for Dec) - the table above is the authoritative Skatteverket source.

## Submission channels, authentication, test environment

### Takeaway
AGI can be entered manually in Skatteverket's e-service, uploaded as an XML file, or sent via API. Current technical description is 1.1.18.2. A public test service validates files without login; the API uses OAuth2 with Skatteverket-issued client credentials and e-legitimation or organisation certificate, and filing is completed by an authorised person signing.

### Cited Findings
- Current technical description: version 1.1.18.2 (the only version listed; no dates; schema versions not stated on page). Test service: checks XML format and conformance to the technical description plus material checks; needs no registered authorisation or e-legitimation; returns pass/fail report with errors. Test link: https://sso.test.skatteverket.se/agd_tt/da_testtjanst_web/login.do?method=test — [Skatteverket, Teknisk beskrivning och testtjänst](https://www.skatteverket.se/foretag/arbetsgivare/lamnaarbetsgivardeklaration/tekniskbeskrivningochtesttjanst.4.309a41aa1672ad0c8377c8b.html)
- File format: XML, UTF-8; root element needs attribute omrade="Arbetsgivardeklaration"; filename up to 256 chars; `&`, `'`, `"` must be escaped; `<`/`>` not allowed in values. Limits: 100 MB in test service, 300 MB in file transfer service and open API (reduced from 500 MB); max 1,000 unique employer+period declarations per file in test service and e-service upload; signing/submission has no size limit. — [Skatteverket, Teknisk beskrivning](https://www.skatteverket.se/foretag/arbetsgivare/lamnaarbetsgivardeklaration/tekniskbeskrivningochtesttjanst/tekniskbeskrivning1116.4.7eada0316ed67d7282a791.html)
- API onboarding: request access through a form on the API's page in the Developer Portal (both test and production; production needs an agreement). Client ID and secret by email for test; for production one by email and one by SMS; test and production keys are not interchangeable. OAuth2 (Authorization Code or Client Credentials depending on API) with e-legitimation or organisation legitimation; test BankID for personal login; organisation certificates currently only accepted if issued by Expisoft. Required roles: e-legitimation user needs firmatecknare, deklarationsombud or registreringsombud arbetsgivardeklaration; organisation legitimation needs registreringsombud arbetsgivardeklaration. — [Skatteverket, Börja använda API:er](https://skatteverket.se/omoss/digitalasamarbeten/borjaanvandaapier.4.77f3e230191ef88bf1d585.html)
- AGI API (search-result summary of the developer portal documents): "Inlämning" service description v1 (version 1.8, 2021-01-08) and "Hantera redovisningsperiod" (saved March 2024). A file delivered via the API sits in a per-period staging area and is not treated as submitted until an authorised representative signs it in Skatteverket's e-service, which yields a receipt. Requests must carry an SKV-client_correlationid header. Environments: development, test, production. — [Tjänstebeskrivning AGD inlämning v1](https://www7.skatteverket.se/portal-wapi/open/apier-och-oppna-data/utvecklarportalen/v1/getFile/tjanstebeskrivning-agd-inlamning-v1), [Hantera redovisningsperiod](https://www7.skatteverket.se/portal-wapi/open/apier-och-oppna-data/utvecklarportalen/v1/getFile/tjanstebeskrivning-agd-hantera-redovisningsperiod) [SECONDHAND: documents not opened directly]
- Correction/removal mechanics: a corrected individuppgift with the same period, identity and Specifikationsnummer replaces the original; wrong recipient = remove (Borttag FK205 with FK201, FK006, FK570 and identity) then add anew; a mismatching Specifikationsnummer creates an extra individuppgift instead of replacing. — [Skatteverket, Teknisk beskrivning](https://www.skatteverket.se/foretag/arbetsgivare/lamnaarbetsgivardeklaration/tekniskbeskrivningochtesttjanst/tekniskbeskrivning1116.4.7eada0316ed67d7282a791.html)

### Inferences
- For an open-source accounting tool, the realistic path is: generate XML per 1.1.18.2, validate in the public test service, then either upload manually in the e-service, or become an API consumer (requires Skatteverket agreement, client credentials and an organisation certificate/BankID-signing by the customer). Final signing by a human with the right role seems unavoidable in the API flow.

### Gaps
- Exact current XSD schema version number/namespace and whether a 1.1.18.x successor exists after the version listed; not retrievable from the page.
- Current API (Inlämning) latest version beyond v1.8 not verified; terms/costs for production access not verified.

## Rates: arbetsgivaravgifter 2026, young/old reductions, växa-stöd

### Takeaway
Full rate 31.42% (born 1959 or later); 10.21% for born 1938-1958; 0% for 1937 or earlier. A temporary reduced rate of 20.81% applies to pay up to 25,000 SEK/month for ages 19-23 from 1 April 2026 to 30 September 2027. Växa-stödet is no longer netted in AGI from 2026; it is claimed afterwards.

### Cited Findings
- 2026: born 1959 or later 31.42%; born 1938-1958 10.21%; born 1937 or earlier 0%; särskild löneskatt 24.26% (pension costs rate cited by Fortnox, not confirmed on Skatteverket page). Prisbasbelopp 59,200; inkomstbasbelopp 83,400. — [Fortnox, Belopp och procentsatser 2026](https://www.fortnox.se/fortnox-foretagsguide/driva-foretag/skatt/belopp-och-procentsatser) [SECONDHAND; the primary is Skatteverket's belopp PDF which could not be parsed]
- Youth reduction: 1 April 2026 to 30 September 2027, employees born 2003-2007 (19-23), pay up to 25,000 SEK/month; employer pays old-age pension fee plus half of other fees, 20.81% total in 2026 (rate for 2027 part of the period not stated). Skatteverket page does not say how it is reported in AGI. — [Skatteverket news](https://skatteverket.se/omoss/pressochmedia/nyheter/2026/nyheter/lagrearbetsgivaravgifterforungdomar.5.70685bee19c85dd5dd02b10.html); govt bill 2025/26:66 per [Arbetsförmedlingen](https://arbetsformedlingen.se/for-arbetsgivare/tips-inspiration-och-nyheter/nyheter/2026-02-25-sankta-arbetsgivaravgifter-for-unga)
- Växa-stödet: since 1 Jan 2025 reduced rate (10.21% on pay up to 35,000 SEK/month, first and second employee, first 24 months). From 2026 the reduction is no longer applied in AGI; full contributions are paid and the reduction is requested afterwards one month at a time via a separate electronic form, within one year after the month; it is de minimis state aid (prop. 2025/26:24). — [Företagarna](https://www.foretagarna.se/nyheter/riks/2025/oktober/vaxa-stodet-blir-krangligare-fran-arsskiftet/), [EY](https://www.ey.com/sv_se/insights/private-business/tillfallig-nedsattning-av-arbetsgivaravgiften) [SECONDHAND]
- Egenavgifter (self-employed, for reference): total 28.97%. — Fortnox page above

### Inferences
- For a young owner-employee (born 2003-2007) the 20.81% reduction could apply to pay up to 25,000 SEK/month; combining with växa-stöd is not addressed in the sources.

### Gaps
- Component breakdown of 31.42% (sjukförsäkring 3.55? etc.) not retrieved; do not hardcode components without source.
- AGI field/mechanism for the youth reduction not found.

## Förmåner and tax-free limits 2026

### Takeaway
Key 2026 figures: kostförmån 62/124/310; traktamente inrikes 300/150; milersättning 25 kr/mil own car; julgåva 600; friskvård 5,000 (secondary).

### Cited Findings
- Kostförmån: breakfast 62, lunch or dinner 124, full board (3+ meals) 310 SEK. — [Fortnox 2026](https://www.fortnox.se/fortnox-foretagsguide/driva-foretag/skatt/belopp-och-procentsatser); consistent with search summary of Skatteverket's belopp PDF
- Traktamente inrikes: full day 300, half day 150, night 150 (only if no employer-paid lodging), after 3 months 210/day, after 2 years 150/day. Meal reductions (full day): all-day meals -> 270, lunch+dinner -> 210, lunch or dinner -> 105, breakfast -> 60. Requires overnight and > 50 km from workplace/home (SECOND-HAND summary). Utrikes: tax-free share reduced 85/70/35/15% for full/lunch+dinner/one/breakfast. — Fortnox above, [Skatteverket traktamente](https://www.skatteverket.se/foretagochorganisationer/arbetsgivare/lonochersattning/traktamente.4.361dc8c15312eff6fd1703e.html) (found in search, not opened)
- Milersättning: own car 25 kr/mil; company car 12 kr/mil (not fully electric), 9.50 kr/mil (fully electric). Azets reports that from 1 July 2026 mileage may also be paid for plug-in hybrid charged free at workplace. — Fortnox above; [Azets](https://www.azets.com/sv-se/resurser/blogg/lagandringar-inom-loneomradet-fran-1-januari-2026) [SECONDHAND]
- Drivmedelsförmån with bilförmån: market value x 1.2 (FK018). — [Skatteverket tech. description](https://www.skatteverket.se/foretag/arbetsgivare/lamnaarbetsgivardeklaration/tekniskbeskrivningochtesttjanst/tekniskbeskrivning1116.4.7eada0316ed67d7282a791.html)
- Gåvor (incl. VAT): julgåva 600, jubileumsgåva 1,800, minnesgåva 15,000 SEK. — Fortnox above
- Friskvård: 5,000 SEK per person/year, small value, same terms for all; exceeding it taxes the whole amount; massage-like activities without exercise element up to 1,000 per occasion; equipment purchase/rental does not count. — [Fortnox friskvårdsbidrag](https://www.fortnox.se/fortnox-foretagsguide/driva-foretag/lon/friskvardsbidrag), [Spiris](https://www.spiris.se/blogg/personalhantering/friskvardsbidrag) [SECONDHAND]
- Telefon/dator: equipment necessary for the work is generally not a taxable benefit; becomes taxable if taken outside the workplace without being essential. Representation: 300 SEK excl. VAT (VAT deductibility figure). — [Skatteverket, Arbetsredskap](https://www.skatteverket.se/privat/skatter/arbeteochinkomst/formaner/arbetsredskap.4.3016b5d91791bf54679426.html); Fortnox
- Lägsta SGI 14,200; max sjukpenninggrundande 592,000; max pensionsgrundande income 673,038; skiktgräns 643,000. — Fortnox above [SECONDHAND]

### Gaps
- Bilförmån formula/values for 2026 (nybilspris, riksbankslåneränta etc.), hemarbete/distansarbete ersättning rules, utlägg treatment, and telefon-förmån schablon were not found.
- Utlägg (expense reimbursements) rules not retrieved.

## Owner-director salary, 3:12 and lönekrav

### Takeaway
From income year 2026 (declared 2027) the old förenklingsregel/huvudregel are replaced by one gränsbelopp model and the lönekrav (salary withdrawal requirement) and 4% capital-share requirement are abolished; salary now matters only for the lönebaserat utrymme.

### Cited Findings
- Grundbelopp = 4 inkomstbasbelopp of the year before the income year: 322,400 SEK for 2026 (4 x 80,600), divided among shares. Lönebaserat utrymme = 50% x (löneunderlag x ownership share - 8 IBB [644,800 SEK for 2026]). Löneunderlag = prior-year cash pay to employees in the company and subsidiaries (benefits excluded). Cap: lönebaserat utrymme may not exceed 50 times the owner's/related person's prior-year cash pay from the group. Interest only on omkostnadsbelopp above 100,000 SEK at statslåneränta + 9% = 11.55% for 2026. Saved dividend room no longer gets interest. — [Skatteverket, Ändrade regler för delägare i fåmansföretag](https://www.skatteverket.se/foretag/drivaforetag/foretagsformer/famansforetag/andradereglerinforinkomstdeklarationen2027.4.4a54dc8b19aa6175a152359.html)
- Skatteverket example: gränsbelopp 1,250,000 (322,400 + 177,600 + 0 + saved 750,000); dividend within the limit taxed two-thirds as capital at 30% (i.e. 20% effective). — same source
- Wolters Kluwer's secondary claim of "4 IBB = 322,400 using IBB 83,400" does not add up; 322,400 is based on the 2025 IBB of 80,600, so the year-before rule applies. — same Skatteverket page; [Wolters Kluwer search summary]

### Inferences
- For a tiny AB the owner's salary no longer needs to hit a threshold to access the grundbelopp; salary (and employee pay) only enlarges the lönebaserade utrymmet beyond 8 IBB of löneunderlag. A salary that creates a tax-optimal mix is an advisory matter, not to be hardcoded.
- The request mentions 'lönekrav rules for the underlag'; under the 2026 rules these no longer exist for new calculations. Older (pre-2026) rules (e.g. 6 IBB + 5% of payroll) are not covered by sources fetched here.

### Gaps
- Qualified-share (kvalificerade andelar) rules, 20% rate vs. tjänst taxation above the gränsbelopp not detailed.

## Kontrolluppgifter (what AGI replaced), sjuklön/sjukavdrag, payslip, retention, pensions, BAS accounts

### Takeaway
AGI (from January 2019) replaced the separate annual kontrolluppgifter for pay/benefits (KU10 etc.) and the monthly aggregate declaration. Retention is 7 years for räkenskapsinformation. Payslips are customary rather than strictly regulated.

### Cited Findings
- AGI, introduced January 2019, gives Skatteverket ongoing individual-level pay information, filed monthly. — [Bokio/Björn Lundén guides](https://bjornlunden.com/se/bjornkoll/blogg/arbetsgivardeklaration-pa-individniva-agi/) [SECONDHAND]
- Retention: räkenskapsinformation including payroll lists and payslip copies must be kept 7 years (to end of the 7th year after the end of the calendar year the financial year ended; bokföringslagen 7 kap. 2 §). — [Lawline](https://lawline.se/answers/forvaring-av-rakenskapsinformation), [FAR Online](https://www.faronline.se/dokument/rattserien/ratt-lon/a/rl_arkivering/) [SECONDHAND]
- Payslip: no law lists required contents; SFL requires the employer to give written account of tax withheld; collective agreements may regulate. Customary contents: employer and employee details, period, pay items, deductions, gross/net, vacation data, accumulated amounts. — [Lawline](https://lawline.se/answers/vilka-uppgifter-ska-finnas-med-i-en-lonespecifikation), [Fortnox](https://www.fortnox.se/fortnox-foretagsguide/ekonomisk-ordlista/lonespecifikation) [SECONDHAND]
- BAS postings: 7010 (arbetare), 7210 (tjänstemän), 7220 (företagsledare) for salaries; 2710 personalskatt; 7510 lagstadgade sociala avgifter (debit) against 2731 avgifter till skattekonto (credit) (some systems use 2730); 1930 bank for net pay; settlement via 1630 skattekonto. Typical entry: debit 7210 and 7510; credit 1930, 2710, 2731. Accrued holiday pay in older BAS was 2920 (upplupna semesterlöner) - not verified for current BAS; year-end accrued fees 2940. — [Fortnox, Bokföra löneutbetalning](https://www.fortnox.se/fortnox-foretagsguide/bokforingstips/loneutbetalning), [Fortnox sociala avgifter](https://www.fortnox.se/fortnox-foretagsguide/bokforingstips/sociala-avgifter), [Björn Lundén](https://bjornlunden.com/se/bjornkoll/blogg/bokfora-lon-konteringar-och-exempel/) [SECONDHAND]

### Gaps
- Sjuklön/sjukavdrag reporting in AGI (frånvarouppgift fields, karensavdrag), semester basics: no Skatteverket source found.
- Tjänstepension/ITP obligations (ITP applies via collective agreement; särskild löneskatt 24.26% on pension costs per Fortnox): no primary source retrieved; no BAS source confirming 7411/7412/7413 accounts (from memory only, UNVERIFIED).
- Error correction at the portal level (rättelse via e-service) only covered via the XML mechanics above.
- bas.se and utvecklare.skatteverket.se were not directly fetched.
