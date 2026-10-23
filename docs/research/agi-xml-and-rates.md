# Swedish AGI XML format and 2026 payroll parameters (small AB, as of 2026-10-23)

Research date 2026-10-23. Primary sources are Skatteverket pages, the official XSDs and the official example-file annex. Section-level "Gaps" list what could not be verified. The XSD and example-file details were read directly from the published files (not summaries), except where stated.

## 1. AGI XML structure (Teknisk beskrivning 1.1.18.2, schema 1.1)

### Takeaway
The file is one UTF-8 XML document, root `Skatteverket omrade="Arbetsgivardeklaration"`, with an instance namespace (default) and a component namespace (prefix `agd:`). One `Blankett` per HU (one per employer and period) and one `Blankett` per IU (one per employee and specification). All amounts are whole kronor (integers). The XSDs and 19 example XML files are published by Skatteverket, and the XSDs were read in full.

### Cited Findings

**Versions and files**
- Current technical description is 1.1.18.2. Schema version is still 1.1 (XSD dated 2025-06-26). 1.1.18.2 adds the temporary contribution reduction for ages 18/19-22/23 from April 2026, stops re-opening declarations older than 6 years, and lowers the file-transfer max size from 500 MB to 300 MB. 1.1.18 / 1.1.18.1 removed Växa-stöd from January 2026, added FK463 (Annat driftstöd) from 202601, adjusted regional deduction to GBER, raised the pension-fee-only age limit from 67 to 68 from January 2026, and added age category Alder_5 from 202604. — [Teknisk beskrivning 1.1.18.2](https://www.skatteverket.se/foretag/arbetsgivare/lamnaarbetsgivardeklaration/tekniskbeskrivningochtesttjanst/tekniskbeskrivning1113.4.7eada0316ed67d7282a791.html) (also reachable at [tekniskbeskrivning11182](https://www.skatteverket.se/foretag/arbetsgivare/lamnaarbetsgivardeklaration/tekniskbeskrivningochtesttjanst/tekniskbeskrivning11182.4.7eada0316ed67d7282a791.html))
- The technical description is published as web pages; no single PDF of the main text was found on the page. Annexes (labelled 1.1.18.1, still current for 1.1.18.2):
  - Struktur XML-dokument (xlsx): https://www.skatteverket.se/download/18.1522bf3f19aea8075ba80e/1765448416038/Bilaga%20Struktur,%20XML-dokument_1.1.18.1.xlsx
  - Fältlista (xlsx): https://www.skatteverket.se/download/18.1522bf3f19aea8075ba80b/1765448363471/Bilaga%20F%C3%A4ltlista_1.1.18.1.xlsx
  - Kontroller (xlsx): https://www.skatteverket.se/download/18.1522bf3f19aea8075ba80c/1765448380675/Bilaga%20Kontroller%201.1.18.1.xlsx
  - Summering och beräkningsregler (xlsx): https://www.skatteverket.se/download/18.1522bf3f19aea8075ba503c/1768990791934/Bilaga%20Summering%20och%20Ber%C3%A4kningsregler..xlsx
  - Land- och ersättningskoder (xlsx): https://www.skatteverket.se/download/18.1522bf3f19aea8075baa84/1765545277710/Bilaga%20Land-%20och%20ers%C3%A4ttningskoder.xlsx
  - Exempelfiler (pdf, 85 pages, 19 examples): https://www.skatteverket.se/download/18.1522bf3f19aea8075ba80a/1765448338286/Bilaga%20Exempelfiler_1.1.18.1.pdf
  - All example files as XML (zip): https://www.skatteverket.se/download/18.1522bf3f19aea8075ba810/1765448451193/Exempelfiler_XML.zip
  - XSD instance part: https://www.skatteverket.se/download/18.6491ed82199c4722dae6cc/1760546683779/arbetsgivardeklaration_1.1.xsd
  - XSD component part: https://www.skatteverket.se/download/18.6491ed82199c4722dae6cd/1760546712759/arbetsgivardeklaration_component_1.1.xsd
  - (all from the 1.1.18.2 page above)
- Schemaläge: the XSD instance imports the component XSD with `schemaLocation="arbetsgivardeklaration_component_1.1.xsd"`. The example files use `xsi:schemaLocation` pointing at `http://xmls.skatteverket.se/se/skatteverket/da/arbetsgivardeklaration/arbetsgivardeklaration_1.1.xsd`. — [instance XSD](https://www.skatteverket.se/download/18.6491ed82199c4722dae6cc/1760546683779/arbetsgivardeklaration_1.1.xsd), [Exempelfiler pdf](https://www.skatteverket.se/download/18.1522bf3f19aea8075ba80a/1765448338286/Bilaga%20Exempelfiler_1.1.18.1.pdf)

**Namespaces**
- Instance (default) namespace: `http://xmls.skatteverket.se/se/skatteverket/da/instans/schema/1.1` (`elementFormDefault="qualified"`). Only the root element `Skatteverket` lives here. — [instance XSD](https://www.skatteverket.se/download/18.6491ed82199c4722dae6cc/1760546683779/arbetsgivardeklaration_1.1.xsd)
- Component namespace: `http://xmls.skatteverket.se/se/skatteverket/da/komponent/schema/1.1`, conventionally prefix `agd:`. All other elements (Avsandare, Blankett, HU, IU, ...) are `agd:`-qualified. — [component XSD](https://www.skatteverket.se/download/18.6491ed82199c4722dae6cd/1760546712759/arbetsgivardeklaration_component_1.1.xsd)
- The technical description recommends setting a namespace on the instance schema with prefix `root`, so the root is written `root:Skatteverket`. Example 9 in the annex is a "file without the prefix agd:" (i.e. default-namespace form is also valid). — [1.1.18.2 page](https://www.skatteverket.se/foretag/arbetsgivare/lamnaarbetsgivardeklaration/tekniskbeskrivningochtesttjanst/tekniskbeskrivning1113.4.7eada0316ed67d7282a791.html), [Exempelfiler](https://www.skatteverket.se/download/18.1522bf3f19aea8075ba80a/1765448338286/Bilaga%20Exempelfiler_1.1.18.1.pdf)

**Root and element order (from the XSD)**
- Root `Skatteverket`, required attribute `omrade` (xs:string), value "Arbetsgivardeklaration". Children in order: `Avsandare`, `Blankettgemensamt`, then either (1+ `Blankett`, optionally 0+ `Franvarouppgift`) or (1+ `Franvarouppgift` only). — [instance XSD](https://www.skatteverket.se/download/18.6491ed82199c4722dae6cc/1760546683779/arbetsgivardeklaration_1.1.xsd)
- `Avsandare` (sequence, all required): `Programnamn` (text50), `Organisationsnummer` (IDENTITET), `TekniskKontaktperson` {Namn text50, Telefon text20, Epostadress; optional Utdelningsadress1/2, Postnummer, Postort}, `Skapad` (xs:dateTime, e.g. `2026-03-02T09:42:25`). No faltkod on these. — [component XSD](https://www.skatteverket.se/download/18.6491ed82199c4722dae6cd/1760546712759/arbetsgivardeklaration_component_1.1.xsd)
- `Blankettgemensamt`: 0..n `Arbetsgivare`, each with `AgRegistreradId` (faltkod 201 in the XSD, but the examples write it without faltkod here) and 1..n `Kontaktperson` {Namn, Telefon, Epostadress required; Sakomrade optional}. Example 12 shows an empty `<agd:Blankettgemensamt></agd:Blankettgemensamt>` ("utan kontaktperson"). — [component XSD](https://www.skatteverket.se/download/18.6491ed82199c4722dae6cd/1760546712759/arbetsgivardeklaration_component_1.1.xsd), [Exempelfiler](https://www.skatteverket.se/download/18.1522bf3f19aea8075ba80a/1765448338286/Bilaga%20Exempelfiler_1.1.18.1.pdf)
- `Blankett` = `Arendeinformation` + `Blankettinnehall`. `Arendeinformation` (all): `Arendeagare` (IDENTITET, required; the employer id), `Period` (text20, required; YYYYMM in the examples), `Arendenummer` (optional). `Blankettinnehall` contains exactly one `HU` or `IU`. — [component XSD](https://www.skatteverket.se/download/18.6491ed82199c4722dae6cd/1760546712759/arbetsgivardeklaration_component_1.1.xsd)
- A data element's `faltkod` attribute is mandatory and fixed to the field code. XML special chars `&`, `'`, `"` must be escaped; `<` and `>` are not allowed in values. File is UTF-8 (BOM optional), file name free (max 256 chars). — [1.1.18.2 page](https://www.skatteverket.se/foretag/arbetsgivare/lamnaarbetsgivardeklaration/tekniskbeskrivningochtesttjanst/tekniskbeskrivning1113.4.7eada0316ed67d7282a791.html)
- Limits: test service 100 MB, file-transfer service and open API 300 MB, max 1 000 unique employer declarations (per employer and period) per file in test service and e-tjänst. — same page.

**HU (huvuduppgift) elements, element name = faltkod (XSD type)**
- `ArbetsgivareHUGROUP/AgRegistreradId` = 201 (required, IDENTITET); `RedovisningsPeriod` = 006 (required, YYYYMM); `SummaArbAvgSlf` = 487 (BELOPP12, total employer contributions and SLF); `SummaSkatteavdr` = 497 (BELOPP12, total withheld tax); `TotalSjuklonekostnad` = 499; `UlagSlfVinstandel` 481; `SlfVinstandelFK486` 486; `UlagSkatteavdrPensFors` 491; `SkatteavdrPensFors` 495; `UlagSkatteavdrRantaUtd` 492; `SkatteavdrRantaUtd` 496; `UlagFoU` 470 / `AvdragFoU` 475; `UlagRegionaltStod` 471 / `AvdragRegionaltStod` 476; `UlagRederiRegress` 472 / `AvdragRederiRegress` 477; `AnnatDriftstod` 463; `EjFastDriftstalleISv` 302 (KRYSS). All optional except 201 and 006 per the XSD (the XSD does not itself force 487/497; the control rules in the Kontroller annex may). — [component XSD](https://www.skatteverket.se/download/18.6491ed82199c4722dae6cd/1760546712759/arbetsgivardeklaration_component_1.1.xsd)

**IU (individuppgift) elements for a plain salaried employee**
- Required in XSD: `ArbetsgivareIUGROUP/AgRegistreradId` (201), `BetalningsmottagareIUGROUP` (with `BetalningsmottagareIDChoice`), `RedovisningsPeriod` (006), `Specifikationsnummer` (570, xs:long 1..9 999 999 999; examples write `001`).
- Payee id: `BetalningsmottagarId` = 215 (IDENTITET), optionally with `Fodelsetid` 222 and/or `AnnatId` 224; or `Fodelsetid` alone; or `AnnatId` alone. Optional names: `Fornamn` 216, `Efternamn` 217.
- Money fields (all non-negative integers, kronor): `KontantErsattningUlagAG` 011 (cash pay, basis for employer contributions); `SkatteplBilformanUlagAG` 013 (taxable car benefit); `DrivmVidBilformanUlagAG` 018 (fuel in car benefit); `BetForDrivmVidBilformanUlagAG` 098; `SkatteplOvrigaFormanerUlagAG` 012 (other benefits); `AvdrPrelSkatt` 001 (withheld preliminary tax); `AvrakningAvgiftsfriErs` 010; `AndraKostnadsers` 020; `Tjanstepension` 030; `Hyresersattning` 039; `ErsattningsKod1/Belopp1` 004/005 (sick pay etc., code range 100-999). KRYSS flags (value `1`): `Bilersattning` 050, `Traktamente` 051, `ForstaAnstalld` 062 (first employee), `Borttag` 205, `EjFastDriftstalleIndivid` 303, `Lokalanstalld` 253, etc.
- Optional workplace address `ArbetsplatsensGatuadress` 245 and `ArbetsplatsensOrt` 246 appear in nearly all examples.
- All 100+ IU fields: see the component XSD and Fältlista above. — [component XSD](https://www.skatteverket.se/download/18.6491ed82199c4722dae6cd/1760546712759/arbetsgivardeklaration_component_1.1.xsd)

**Formats (from XSD simple types)**
- Period `REDOVISNINGSPERIOD`: 6 chars `YYYYMM`, pattern `20(18(0[7-9]|1[012])|19(0[1-9]|1[012])|[2-9][0-9](0[1-9]|1[012]))`, i.e. 201807 onward. No hyphen.
- `IDENTITET` (maxLength 12, digits only, no hyphen, no 'T'): personnummer or samordningsnummer as 12 digits `YYYYMMDDNNNC` (pattern allows 19xx/20xx; samordningsnummer day+60); organisation number as 12 digits with a `16` prefix (pattern `16(1x|2x|3x|5x|6[0-4]|66|68|7x|8x|9x)[2-9]\d{7}`), e.g. `165560269986` for org no 556026-9986. Hyphens are not allowed.
- `FODELSETID`: 11 chars `YYYYMMDDNNN`.
- Amounts: `BELOPP6/7/8/10/12` are `xs:long`, min 0, max 10^n-1. No decimals, no negative values. KRYSS is boolean pattern `1|true`.
- `SPECIFIKATIONSNUMMER`: long 1..9 999 999 999.
- Numbers of decimals/rounding: the XSD gives integers only; the rounding rule for contributions (FK487) is in the Summering och beräkningsregler annex, which was not read. — [component XSD](https://www.skatteverket.se/download/18.6491ed82199c4722dae6cd/1760546712759/arbetsgivardeklaration_component_1.1.xsd)

**Minimal valid example (from Skatteverket's Exempelfil 1, trimmed to one HU and one IU; originals from the annex).** The namespace line `XMLSchemainstance` in the extracted PDF text is almost certainly the PDF dropping a hyphen; the correct URI is `http://www.w3.org/2001/XMLSchema-instance`.
```xml
<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<Skatteverket omrade="Arbetsgivardeklaration"
  xmlns="http://xmls.skatteverket.se/se/skatteverket/da/instans/schema/1.1"
  xmlns:agd="http://xmls.skatteverket.se/se/skatteverket/da/komponent/schema/1.1"
  xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
  xsi:schemaLocation="http://xmls.skatteverket.se/se/skatteverket/da/instans/schema/1.1 http://xmls.skatteverket.se/se/skatteverket/da/arbetsgivardeklaration/arbetsgivardeklaration_1.1.xsd">
  <agd:Avsandare>
    <agd:Programnamn>Programmakarna AB</agd:Programnamn>
    <agd:Organisationsnummer>190002039006</agd:Organisationsnummer>
    <agd:TekniskKontaktperson>
      <agd:Namn>Valle Vadman</agd:Namn>
      <agd:Telefon>23-2-4-244454</agd:Telefon>
      <agd:Epostadress>valle.vadman@programmakarna.se</agd:Epostadress>
    </agd:TekniskKontaktperson>
    <agd:Skapad>2026-05-02T07:42:25</agd:Skapad>
  </agd:Avsandare>
  <agd:Blankettgemensamt></agd:Blankettgemensamt>
  <agd:Blankett>
    <agd:Arendeinformation>
      <agd:Arendeagare>165560269986</agd:Arendeagare>
      <agd:Period>202604</agd:Period>
    </agd:Arendeinformation>
    <agd:Blankettinnehall>
      <agd:HU>
        <agd:ArbetsgivareHUGROUP>
          <agd:AgRegistreradId faltkod="201">165560269986</agd:AgRegistreradId>
        </agd:ArbetsgivareHUGROUP>
        <agd:RedovisningsPeriod faltkod="006">202604</agd:RedovisningsPeriod>
        <agd:SummaArbAvgSlf faltkod="487">26094</agd:SummaArbAvgSlf>
        <agd:SummaSkatteavdr faltkod="497">25140</agd:SummaSkatteavdr>
      </agd:HU>
    </agd:Blankettinnehall>
  </agd:Blankett>
  <agd:Blankett>
    <agd:Arendeinformation>
      <agd:Arendeagare>165560269986</agd:Arendeagare>
      <agd:Period>202604</agd:Period>
    </agd:Arendeinformation>
    <agd:Blankettinnehall>
      <agd:IU>
        <agd:ArbetsgivareIUGROUP>
          <agd:AgRegistreradId faltkod="201">165560269986</agd:AgRegistreradId>
        </agd:ArbetsgivareIUGROUP>
        <agd:BetalningsmottagareIUGROUP>
          <agd:BetalningsmottagareIDChoice>
            <agd:BetalningsmottagarId faltkod="215">198202252386</agd:BetalningsmottagarId>
          </agd:BetalningsmottagareIDChoice>
        </agd:BetalningsmottagareIUGROUP>
        <agd:RedovisningsPeriod faltkod="006">202604</agd:RedovisningsPeriod>
        <agd:Specifikationsnummer faltkod="570">001</agd:Specifikationsnummer>
        <agd:KontantErsattningUlagAG faltkod="011">28500</agd:KontantErsattningUlagAG>
        <agd:AvdrPrelSkatt faltkod="001">9300</agd:AvdrPrelSkatt>
      </agd:IU>
    </agd:Blankettinnehall>
  </agd:Blankett>
</Skatteverket>
```
(Assembled from the header/Avsandare/HU of Exempelfil 12 and the IU of Exempelfil 1; the HU totals in the annex example belong to a larger file than this single-employee extract, so do not copy those totals - compute 487 and 497 yourself. The assembled file itself was not run through the test service.) — [Exempelfiler pdf](https://www.skatteverket.se/download/18.1522bf3f19aea8075ba80a/1765448338286/Bilaga%20Exempelfiler_1.1.18.1.pdf)
- Licence/terms: the XSD and the annex carry no explicit licence in the files read (the XSD has a metadata block crediting Skatteverket, "IDAG projektet"). No licence text was found on the technical description page. Treat as published public technical specification; do not assume an open licence.

**Corrections and removal**
- Correct an individual entry by sending a new file with an IU having the same `Specifikationsnummer` (570), `RedovisningsPeriod` (006) and `BetalningsmottagarId` (215) as the original; otherwise it is added as an extra entry. Wrong person: remove the wrong IU and submit a new IU with the right id. — [1.1.18.2 page](https://www.skatteverket.se/foretag/arbetsgivare/lamnaarbetsgivardeklaration/tekniskbeskrivningochtesttjanst/tekniskbeskrivning11182.4.7eada0316ed67d7282a791.html)
- Removal: an IU with only `Borttag` (205, value `1`), employer id, period, `Specifikationsnummer` and payee id, the last four matching the original. Annex example 10 verbatim structure: IU = ArbetsgivareIUGROUP, BetalningsmottagareIUGROUP, RedovisningsPeriod, `<agd:Specifikationsnummer faltkod="570">001</agd:Specifikationsnummer>`, `<agd:Borttag faltkod="205">1</agd:Borttag>`. — same page, [Exempelfiler](https://www.skatteverket.se/download/18.1522bf3f19aea8075ba80a/1765448338286/Bilaga%20Exempelfiler_1.1.18.1.pdf)
- Specifikationsnummer is mandatory even if the employee has only one IU in the period; max 10 digits, no zero value, must be unique per employer/period/payee, and you choose the numbering yourself. — [Skatteverket, Så fyller du i arbetsgivardeklarationen](https://www.skatteverket.se/foretag/arbetsgivare/lamnaarbetsgivardeklaration/safyllerduiarbetsgivardeklarationen.4.2cf1b5cd163796a5c8b66a8.html) (via search snippet, page not fetched in full)
- Corrections allowed up to six years back; tax withheld normally cannot be corrected downward. — [Rätta en arbetsgivardeklaration](https://www.skatteverket.se/foretag/arbetsgivare/lamnaarbetsgivardeklaration/rattaenarbetsgivardeklaration.4.2cf1b5cd163796a5c8b6698.html) (search snippet only)

### Inferences
- A re-send of an HU for the same employer and period replaces the earlier HU (the whole declaration is a "latest wins" per period). The pages read do not state this in so many words for HU; confirm in the Kontroller annex or via the test service.
- With 2-3 employees the file is one HU `Blankett` plus one IU `Blankett` per employee, all with the same `Period`. Use `Specifikationsnummer` 1, 2, 3... stable per employee so later corrections hit the same entry.
- Org number: use `16` + 10-digit org number (no hyphen). Personnummer: 12 digits, no hyphen.
- Optional elements may simply be omitted (XSD has minOccurs=0); Skatteverket examples omit zero amounts.

### Gaps
- Full text of the Teknisk beskrivning main body (mandatory-field matrix, section 10 file format, check rules s_/ba_/b_ numbers) was not retrievable as a single document; only the page summary and annexes (xlsx/pdf) exist. The xlsx annexes (Struktur, Fältlista, Kontroller, Summering) were not opened, so exact mandatory-ness per field (beyond the XSD) and the FK487 rounding rule are unverified.
- The zip of example XML files was not opened (examples were read from the PDF annex via text extraction).
- Whether the `Arbetsgivare` block in `Blankettgemensamt` is required by control rules is unverified (the XSD allows it empty, and Example 12 omits it).
- Does the HU `Period`/`Arendeagare` org id need `16` prefix: the XSD pattern implies yes for organisations, but Skatteverket's Example 1 value `165560269986` is consistent; not verified against a real org-number conversion beyond that.
- The correct `xsi` URI (hyphen) is inferred; PDF text extraction dropped it.

## 2. Public AGI test/validation service

### Takeaway
The "Testa arbetsgivardeklaration" test service needs no login, no BankID and no registered authorization. Max file 100 MB. The result is a report saying approved or listing the errors and failed controls.

### Cited Findings
- URL given on the page: https://sso.test.skatteverket.se/agd_tt/da_testtjanst_web/login.do?method=test — [Teknisk beskrivning och testtjänst](https://www.skatteverket.se/foretag/arbetsgivare/lamnaarbetsgivardeklaration/tekniskbeskrivningochtesttjanst.4.309a41aa1672ad0c8377c8b.html)
- "You can test a file there without registered authorization or e-identification." The service checks the file is well-formed XML following the technical description and runs the material checks. — same page; [1.1.18.2 page](https://www.skatteverket.se/foretag/arbetsgivare/lamnaarbetsgivardeklaration/tekniskbeskrivningochtesttjanst/tekniskbeskrivning1113.4.7eada0316ed67d7282a791.html)
- Response: a report, either approved or with the errors found and the controls that failed; the report can be saved and printed. A file with only individual entries cannot be checked against cross-checks that need both employer-level and individual-level data (so send HU + IU together). — [1.1.18.2 page](https://www.skatteverket.se/foretag/arbetsgivare/lamnaarbetsgivardeklaration/tekniskbeskrivningochtesttjanst/tekniskbeskrivning11182.4.7eada0316ed67d7282a791.html)
- Real submission channels: the e-tjänst (file upload with BankID/e-legitimation), the file-transfer service, and an open API documented at the developer portal https://www7.skatteverket.se/portal/apier-och-oppna-data/utvecklarportalen/ . — same page

### Inferences
- A web upload form (browse and submit a file) is the likely interaction, but this was not observed. A programmatic test through a script is not documented and probably not intended.

### Gaps
- Upload steps and the exact response format (HTML vs file) are not described on the pages read; the service itself was not opened (no navigation to the test URL was done). Error code format (e.g. check ids) unverified.
- API (open API/file transfer) authentication details (OAuth/BankID/certificate) not researched.

## 3. Employer contributions 2026

### Takeaway
Standard total 31.42% for born 1959 or later (since 2020 unchanged); 10.21% (old-age pension fee only) for born 1938-1958; 0% for born 1937 or earlier. From 2026-04-01 through 2027-09-30, employees born 2003-2007 (19-23 in 2026) carry 20.81% on the first 25 000 kr per month and 31.42% above. In AGI there is no special reduction field in the XML; Skatteverket calculates from the payee's birth year. Växa-stöd is gone from AGI from January 2026.

### Cited Findings
- Component rates 2026: sjukförsäkring 3.55%, föräldraförsäkring 2.00%, ålderspension 10.21%, efterlevandepension 0.30%, arbetsmarknad 2.64%, arbetsskada 0.10%, allmän löneavgift 12.62%, total 31.42%. — [Skatteverket, Arbetsgivaravgifter](https://www.skatteverket.se/foretag/arbetsgivare/arbetsgivaravgifterochskatteavdrag/arbetsgivaravgifter) (the page shows 31.42% as the total for 2020-2025 and 2026)
- Table from 1 April 2026: born 1959 or later 31.42% (underlag/avgift codes 420/430); born 1938-1958 10.21% (422/7022, 432/9022); born 2003-2007 20.81% (7001/9001); born 1937 or earlier 0% (423/433). — [Skatteverket, Arbetsgivaravgifter](https://skatteverket.se/arbetsgivaravgifter)
- Reduction: pay "under perioden 1 april 2026-30 september 2027"; pension fee plus half of the other fees = 20.81%, "upp till 25 000 kronor per kalendermånad"; above that the full 31.42%. In 2026 it covers born 2003-2007. — same page
- Pension-fee-only age limit rises from 67 to 68 years from January 2026 (hence "born 1938-1958" rather than 1937 in 2026, compared with the 67-limit earlier). — [1.1.18.2 page](https://www.skatteverket.se/foretag/arbetsgivare/lamnaarbetsgivardeklaration/tekniskbeskrivningochtesttjanst/tekniskbeskrivning1113.4.7eada0316ed67d7282a791.html)
- AGI treatment: no separate field for the reduction; the technical description just adds age category Alder_5 from 202604 (with new internal codes and calculation rules), and annex example 15 shows an IU for a person born 2007 with only the usual fields (`KontantErsattningUlagAG` 011 = 25800, `SkatteplBilformanUlagAG` 013 = 2500, `AvdrPrelSkatt` 001 = 8000), no extra flag. The employer reports the reduced total in the HU `SummaArbAvgSlf` (487). Example 15 HU (202604) has 487 = 18881 and 497 = 28140. — [Exempelfiler](https://www.skatteverket.se/download/18.1522bf3f19aea8075ba80a/1765448338286/Bilaga%20Exempelfiler_1.1.18.1.pdf)
- Växa-stöd: fields FK062/FK063 are valid only to 202512; the support is not handled in AGI from January 2026 (prop. 2025/26:24). The 2026 tables on the contributions page have no Växa-stöd rows. — [1.1.18.2 page](https://www.skatteverket.se/foretag/arbetsgivare/lamnaarbetsgivardeklaration/tekniskbeskrivningochtesttjanst/tekniskbeskrivning1113.4.7eada0316ed67d7282a791.html), [Arbetsgivaravgifter](https://skatteverket.se/arbetsgivaravgifter)
- The reduction proposal reference: prop. 2025/26:66. Press item: [Lägre arbetsgivaravgifter för ungdomar](https://skatteverket.se/omoss/pressochmedia/nyheter/2026/nyheter/lagrearbetsgivaravgifterforungdomar.5.70685bee19c85dd5dd02b10.html) (found in search; not fetched).
- Särskild löneskatt: SLF/SLP 24.26%. — [Arbetsgivaravgifter](https://www.skatteverket.se/foretag/arbetsgivare/arbetsgivaravgifterochskatteavdrag/arbetsgivaravgifter)

### Inferences
- Calculation for payroll: for an employee born 2003-2007 paid on or after 2026-04-01: contributions = round(20.81% x min(pay, 25 000)) + round(31.42% x max(pay - 25 000, 0)) (basis includes taxable benefits that are avgiftspliktiga; per-month cap applies to total basis per employee/month). The exact rounding and whether the 25 000 cap covers benefits is not verified. A rough check of Example 15 (HU 487 = 18881) gave about 18 945 with this formula, a 64 kr difference I could not explain (may be rounding rules, or benefit handling), so verify with the test service before relying on it.
- The reduction is time-limited; the app should key rates on payment date (1 Apr 2026 to 30 Sep 2027) and birth year (2003-2007 in 2026; the eligible birth years shift in 2027 per Skatteverket's wording "under 2026 omfattar nedsättningen ... födda 2003-2007").
- A small AB with a founder born 1959 or later and 2-3 staff in the usual ages pays 31.42% on all pay.

### Gaps
- Exact AGI-side calculation/rounding rules (Summering och beräkningsregler annex) not read.
- "Belopp och procent 2026" / "Procentsatser och beloppsgränser" pages themselves were not fetched; figures come from the Arbetsgivaravgifter page.
- Whether the reduction depends on age at year start or birth year in 2027 (the "18 but not 23" vs "19-23" wording differs between sources; the 1.1.18.2 page says "fyllt 18 men inte 23", the contributions page says born 2003-2007 for 2026).
- Employer contribution rate for pay to someone born 1959+ who is 66-67 etc. was not checked beyond the birth-year table.

## 4. Preliminary tax (skatteavdrag): tabell, column, tables, jämkning, engångsskatt

### Takeaway
Table number comes from the employee's total tax rate (kommunalskatt + regionskatt + begravningsavgift, + Svenska kyrkan fee if member), rounded to a whole number; column from income type and age (column 1 for wage under 66 at start of year). Skatteverket publishes 2026 monthly tables as PDF per table number (29-42) and collection PDFs; the kommunal tax-rate file is TXT/XLSX. Engångsskatt uses a separate table by annual income; jämkning decisions override tables.

### Cited Findings
- Tabell number = sum of kommunal, region, begravningsavgift (and Svenska kyrkan fee for members), rounded; example in the page: 32.50 gives table 32, 32.51 gives 33. Employee's A-skattsedel shows which table applies. — [Skattetabeller](https://www.skatteverket.se/foretag/arbetsgivare/arbetsgivaravgifterochskatteavdrag/skattetabeller)
- Columns: 1 = wage/pay for those under 66 at start of year with right to jobbskatteavdrag; 2 = pension etc. 66+ without jobbskatteavdrag; 3 = wage for those 66+ (also born 1937 or earlier); 4 = sick pay/activity compensation under 66; 5 = other pension-qualifying benefits; 6 = pension etc. under 66 not pension-qualifying. — same page, [Så läser du skattetabellen](https://www.skatteverket.se/privat/skatter/arbeteochinkomst/skattetabeller/salaserdutabellen.4.319dc1451507f2f99e875f.html)
- 2026 monthly tables (PDF), tabell 29-42, URL pattern `https://www.skatteverket.se/download/18.1522bf3f19aea8075ba6NN/<ts>/allmanna-tabeller-manad-tabell-NN.pdf`, e.g.:
  - 29: https://www.skatteverket.se/download/18.1522bf3f19aea8075ba618/1765290402351/allmanna-tabeller-manad-tabell-29.pdf
  - 30: .../18.1522bf3f19aea8075ba619/1765290422021/allmanna-tabeller-manad-tabell-30.pdf
  - 31: .../18.1522bf3f19aea8075ba61a/1765290436723/allmanna-tabeller-manad-tabell-31.pdf
  - 32: .../18.1522bf3f19aea8075ba61b/1765290451072/allmanna-tabeller-manad-tabell-32.pdf
  - 33: .../18.1522bf3f19aea8075ba61c/1765290465443/allmanna-tabeller-manad-tabell-33.pdf
  - 34: .../18.1522bf3f19aea8075ba61d/1765290478710/allmanna-tabeller-manad-tabell-34.pdf
  - 35: .../18.1522bf3f19aea8075ba61e/1765290492845/allmanna-tabeller-manad-tabell-35.pdf
  - 36: .../18.1522bf3f19aea8075ba61f/1765290503313/allmanna-tabeller-manad-tabell-36.pdf
  - 37: .../18.1522bf3f19aea8075ba624/1765290517552/allmanna-tabeller-manad-tabell-37.pdf
  - 38: .../18.1522bf3f19aea8075ba625/1765290530129/allmanna-tabeller-manad-tabell-38.pdf
  - 39: .../18.1522bf3f19aea8075ba626/1765290546606/allmanna-tabeller-manad-tabell-39.pdf
  - 40: .../18.1522bf3f19aea8075ba627/1765290562409/allmanna-tabeller-manad-tabell-40.pdf
  - 41: .../18.1522bf3f19aea8075ba628/1765290580161/allmanna-tabeller-manad-tabell-41.pdf
  - 42: .../18.1522bf3f19aea8075ba62a/1765290595185/allmanna-tabeller-manad-tabell-42.pdf
  (prefix `https://www.skatteverket.se/download/` for the "..." entries.) — [Skattetabeller](https://www.skatteverket.se/foretag/arbetsgivare/arbetsgivaravgifterochskatteavdrag/skattetabeller)
- Collection PDFs: monthly https://www.skatteverket.se/download/18.1522bf3f19aea8075ba5ae/1765287092986/allmanna-tabeller-manad.pdf ; two-week https://www.skatteverket.se/download/18.1522bf3f19aea8075ba5ab/1765286998299/allmanna-tabeller-2-veckors.pdf — same page
- Kommunal tax rates 2026: https://www.skatteverket.se/download/18.1522bf3f19aea8075ba428/1765291540367/skattesatser-kommuner-2026.txt and https://www.skatteverket.se/download/18.1522bf3f19aea8075ba429/1765179297305/skattesatser-kommuner-2026.xlsx — same page
- Machine-readable table data: Skatteverket's "Teknisk beskrivning för skattetabeller (SKV 433, utgåva 36, Dec 2025)" describes the text/Excel file format for software vendors: https://skatteverket.se/download/18.1522bf3f19aea8075ba55c/1765284655603/teknisk-beskrivning-skv-433-2026-utgava-36.pdf (not opened), page https://www.skatteverket.se/foretag/etjansterochblanketter/blanketterbroschyrer/broschyrer/info/433.4.39f16f103821c58f680007825.html and "Special versions for software companies" page https://skatteverket.se/specialversionerforprogramforetagmfl.4.319dc1451507f2f99e86ee.html . The search result says tables are also offered in Excel and text formats.
- Legal basis: SKVFS 2025:20 (allmänna skattetabeller 2026) and SKVFS 2025:21 (engångsskatt) — [FAR via search](https://www.faronline.se/dokument/skatteverket/forfattningssamling2/2025/skvfs20250021/).
- Engångsbelopp (bonus, retroactive pay, semesterersättning etc.): withhold by engångstabell if the employee also has regular pay from you; 30% if no regular pay; follow a jämkning/SINK decision. Column 1 table 2026: 0-25 041 kr annual income 0%; 25 042-82 800 10%; 82 801-192 000 21%; 192 001-477 600 26%; 477 601-660 000 34%; over 660 000 54%. Annual income estimate is the employee's expected annual pay including the lump sum. — [Skatteavdrag på engångsbelopp](https://www.skatteverket.se/foretag/arbetsgivare/arbetsgivaravgifterochskatteavdrag/skatteavdrag/engangsbelopp.4.361dc8c15312eff6fd3225e.html)
- Jämkning: Skatteverket decision gives a percentage the main employer withholds instead of the table rate; other payers withhold 30%; decision valid for the calendar year; also applies to lump sums when a fixed percentage is decided. Help table 2026: https://www.skatteverket.se/download/18.1522bf3f19aea8075ba50d/1765274367380/hjalptabell-2026.pdf — [Jämkning](https://skatteverket.se/jamkning), [engångsbelopp page](https://www.skatteverket.se/foretag/arbetsgivare/arbetsgivaravgifterochskatteavdrag/skatteavdrag/engangsbelopp.4.361dc8c15312eff6fd3225e.html)
- Withheld tax is reported per employee in IU field 001 `AvdrPrelSkatt` and totalled in HU 497. — [component XSD](https://www.skatteverket.se/download/18.6491ed82199c4722dae6cd/1760546712759/arbetsgivardeklaration_component_1.1.xsd)
- Licence/terms: none stated on the tables page (explicitly absent). Treat as public Skatteverket publications; clarify before bundling.

### Inferences
- A developer should store per employee: tabell number, column (almost always 1), and optional jämkning percent. Look up the withholding by exact monthly salary row; the tables are in ranges of 100 kr step in the PDFs (not verified here).
- The 2026 engångsskatt rates came from the Skatteverket page, but the 0% limit 25 041 etc. depend on the column; use column 1 for ordinary employees under 66.

### Gaps
- The PDFs and SKV 433 were not opened; the file layout of the machine-readable table files (record layout) and the exact download URLs for the txt/xls table data (not PDF) were not found; only PDFs per table and the municipal-rates TXT/XLSX are confirmed.
- Terms of use/licence for embedding table data: not stated on the pages read.
- Details of Skatteverket's "ställningstagande" for end-of-employment payments only cited, not read.

## 5. Owner who is also an employee of their own AB (fåmansföretag)

### Takeaway
No special AGI field or XML structure for company owners: salary is reported like any employee (IU with 011/001, contributions in HU). 3:12 rules (utdelning vs lön, lönebaserat utrymme) only affect the owner's own tax return. The one payroll-relevant point is that a real salary to the owner triggers normal employer contributions and withholding.

### Cited Findings
- If the owner is employed by their own AB and gets salary, the company must pay employer contributions and withhold preliminary tax and file monthly employer declarations. — [Företagarna](https://www.foretagarna.se/driva-eget-foretag/ekonomi/sa-forbereder-du-dig-for-skattearet-2024---10-tips-fran-skatteradgivaren) (secondary source)
- Fåmansföretag = AB where four or fewer owners hold more than 50% of the votes (56 kap. 2 § IL). Salary is employment income (tjänst); dividends are capital income. — [Skatteverket, Fåmansföretag och fåmanshandelsbolag (chapter)](https://www.skatteverket.se/download/18.18e1b10334ebe8bc8000115033/kapitel) (search snippet only)
- Owners of fåmansföretag cannot receive some tax-free benefits (e.g. certain training benefits) from the company; such benefits are taxed. — [Företagarna handbok 3:12](https://www.foretagarna.se/driva-eget-foretag/handbocker-och-guider/handbok-312-reglerna/sarregler-for-delagare-av-famansforetag-och-famanshandelsbolag-som-inte-galler-utdelning-eller-kapitalvinst/) (secondary)
- The XSD has no owner/delägare-specific field in HU or IU. — [component XSD](https://www.skatteverket.se/download/18.6491ed82199c4722dae6cd/1760546712759/arbetsgivardeklaration_component_1.1.xsd)

### Inferences
- Reporting an owner's salary is the same as any employee's. The salary level chosen to meet the 3:12 löneuttagskrav (lönebaserat utrymme) is a tax-planning input, not an AGI rule; dividend payments are not reported in AGI (kontrolluppgift KU31 / tax return, outside scope).
- If the owner's spouse/relative is paid, their market-rate portion is employment income; contributions follow the person taxed (secondary forum-quoted rule; unverified).
- The first employee in the business sets `ForstaAnstalld` (062) in the IU - relevant to "first employee" reductions, not to owners specifically.

### Gaps
- No Skatteverket page was fetched that states "no special AGI field for fåmansföretagare"; the conclusion rests on the XSD having no such field. 3:12 specifics (gränsbelopp, 2026 löneuttagskrav) not researched here.
- The Skatteverket chapter on fåmansföretag was seen only as a search result.

## 6. Deadlines 2026 (monthly reporting, turnover up to 40 MSEK)

### Takeaway
Confirmed: the AGI and the payment of contributions and withheld tax fall due on the same day, normally the 12th (Skatteverket: "De allra flesta ska deklarera den 12:e"); the 17th applies in January and August; if that day is a weekend/holiday it moves to the next business day.

### Cited Findings
- Table (payout month: due date): Dec 2025: 19 Jan 2026; Jan: 12 Feb; Feb: 12 Mar; Mar: 13 Apr; Apr: 12 May; May: 12 Jun; Jun: 13 Jul; Jul: 17 Aug; Aug: 14 Sep; Sep: 12 Oct; Oct: 12 Nov; Nov: 14 Dec; Dec 2026: 18 Jan 2027. Payment is due on the same date as the declaration for this group. — [När ska jag lämna arbetsgivardeklaration?](https://skatteverket.se/foretag/arbetsgivare/lamnaarbetsgivardeklaration/narskajaglamnaarbetsgivardeklaration.4.361dc8c15312eff6fd13c11.html)
- Employers above 40 MSEK declare on the 26th, but pay on "den 12, i januari den 17". — same page

### Inferences
- The "17th in January and August" is the nominal date, which rolls forward over weekends: 17 Jan 2026 was a Saturday, giving 19 Jan (Monday, with the 18th-19th... as listed) and 17 Aug 2026 is a Monday. The 12th rolls forward when on a weekend (e.g. 12 Apr 2026 Sunday gives 13 Apr; 12 Jul gives 13 Jul; 12 Sep gives 14 Sep; 12 Dec gives 14 Dec). The nominal date for December 2026 is 12 Jan 2027? The page says 18 Jan 2027, so for the January deadline the nominal date is the 17th (Sunday, giving Monday 18 Jan). So the rule "17th in January and August" is consistent.

### Gaps
- The page does not state the January/August rule in words for the small-company group; it is inferred from the dates in the table.
- Salary payment date rules (when salary must be paid) were not covered by any source; only AGI/payment deadlines were confirmed.
