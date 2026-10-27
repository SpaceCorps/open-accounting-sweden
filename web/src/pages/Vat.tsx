import {
  Badge,
  DataTable,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
  type DataTableColumn,
  type DataTableRowAction,
} from "@spacecorps/components/ui";
import { EyeOff, ShieldAlert, ShieldCheck } from "lucide-react";
import { useState } from "react";
import { api, type Me } from "../api";
import { useData } from "../hooks";
import { Field, Kpi, PageHeader, Section, notifyError, notifyOk, prioritize } from "../kit";
import { amount, moneyWhole } from "../format";

interface Box {
  box: string;
  label: string;
  kind: string;
  amount: string;
  vouchers: string[];
  verified: boolean;
  verified_by: string | null;
  mapping_source: string;
  not_used: boolean;
}
interface Check {
  check: string;
  ok: boolean;
  detail: string;
}
interface Report {
  quarter: string;
  from: string;
  to: string;
  due_date: string;
  due_date_note: string;
  vouchers_in_period: number;
  boxes: Box[];
  to_pay: string;
  checks: Check[];
  ready_to_file: boolean;
  filing: string;
  warning: string | null;
}
interface Config {
  boxes: {
    box: string;
    label: string;
    kind: string;
    source: string;
    verified: boolean;
    verified_by: string | null;
    verified_at: string | null;
  }[];
  codes: { code: string; label: string; base_box: string | null; base_side: string | null }[];
  accounts: { account: number; name: string; box: string; side: string }[];
}

function quarters() {
  const now = new Date();
  const out: string[] = [];
  let y = now.getFullYear();
  let q = Math.floor(now.getMonth() / 3) + 1;
  for (let i = 0; i < 8; i++) {
    out.push(`${y}-Q${q}`);
    q -= 1;
    if (q === 0) {
      q = 4;
      y -= 1;
    }
  }
  return out;
}

export function VatPage({ me }: { me: Me }) {
  const [quarter, setQuarter] = useState(quarters()[1] ?? quarters()[0]);
  const report = useData<Report>(`/vat/report?quarter=${quarter}`);
  const cfg = useData<Config>("/vat/config");
  const r = report.data;

  const reload = () => {
    report.reload();
    cfg.reload();
  };

  async function setVerified(box: string, verified: boolean) {
    try {
      await api(`/vat/boxes/${box}/verify`, { method: "POST", body: { verified } });
      notifyOk(verified ? `Box ${box} marked verified` : `Box ${box} marked unverified`);
      reload();
    } catch (e) {
      notifyError(e);
    }
  }

  const boxColumns: DataTableColumn<Box>[] = [
    { name: "box", header: "Box", width: "70px" },
    { name: "label", header: "Meaning" },
    {
      name: "amount",
      header: "Amount (SEK)",
      align: "Right",
      accessor: (b) => parseFloat(b.amount),
      cell: (_, b) => amount(b.amount),
    },
    { name: "vouchers", header: "Vouchers", accessor: (b) => b.vouchers.join(", ") },
    {
      name: "verified",
      header: "Mapping",
      accessor: (b) => (b.not_used ? "not used" : b.verified ? "verified" : "UNVERIFIED"),
      cell: (v) => (
        <Badge variant={v === "verified" ? "success" : v === "not used" ? "secondary" : "warning"}>
          {v as string}
        </Badge>
      ),
    },
  ];
  const boxActions = (b: Box): DataTableRowAction<Box>[] =>
    me.can_approve
      ? [
          b.verified
            ? {
                tag: "unverify",
                label: "Mark unverified",
                icon: <ShieldAlert aria-hidden="true" />,
                tooltip: "Mark unverified",
              }
            : {
                tag: "verify",
                label: "I checked this box",
                icon: <ShieldCheck aria-hidden="true" />,
                tooltip: "I checked this box against SKV 4700",
              },
          ...(b.box === "49"
            ? []
            : [
                b.not_used
                  ? {
                      tag: "used",
                      label: "Counts again",
                      icon: <ShieldCheck aria-hidden="true" />,
                      tooltip: "We use this box after all",
                    }
                  : {
                      tag: "unused",
                      label: "Not used by us",
                      icon: <EyeOff aria-hidden="true" />,
                      tooltip: "We never use this box",
                    },
              ]),
        ]
      : [];

  const checkColumns: DataTableColumn<Check>[] = [
    {
      name: "ok",
      header: "",
      width: "90px",
      accessor: (c) => (c.ok ? "ok" : "fix"),
      cell: (v) => <Badge variant={v === "ok" ? "success" : "destructive"}>{v as string}</Badge>,
    },
    { name: "check", header: "Check" },
    { name: "detail", header: "Detail", wrapText: true },
  ];

  return (
    <>
      <PageHeader
        title="VAT"
        description="Quarterly VAT return prepared from the ledger. Nothing is filed from here: enter the figures in Skatteverket's e-service and sign there."
        actions={
          <Field label="Quarter">
            <Select value={quarter} onValueChange={setQuarter}>
              <SelectTrigger style={{ minWidth: 140 }}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {quarters().map((q) => (
                  <SelectItem key={q} value={q}>
                    {q}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Field>
        }
      />

      {r?.warning ? (
        <div className="oas-banner" role="alert">
          <span>
            <Badge variant="warning">unverified</Badge> {r.warning}
          </span>
        </div>
      ) : null}

      <div className="oas-kpis oas-kpis-4">
        <Kpi
          label="VAT to pay (box 49)"
          value={moneyWhole(r?.to_pay ?? 0)}
          hint="negative means a refund"
        />
        <Kpi
          label="Filing deadline"
          value={r?.due_date ?? "..."}
          hint="unverified, see the note below"
        />
        <Kpi
          label="Ready to file"
          value={r?.ready_to_file ? "Yes" : "No"}
          badge={
            <Badge variant={r?.ready_to_file ? "success" : "warning"}>
              {r?.ready_to_file
                ? "all checks pass"
                : `${(r?.checks ?? []).filter((c) => !c.ok).length} to fix`}
            </Badge>
          }
        />
        <Kpi
          label="Vouchers in period"
          value={r?.vouchers_in_period ?? 0}
          hint={r ? `${r.from} to ${r.to}` : ""}
        />
      </div>

      <Tabs defaultValue="boxes">
        <TabsList>
          <TabsTrigger value="boxes">Boxes</TabsTrigger>
          <TabsTrigger value="checks">Checks</TabsTrigger>
          <TabsTrigger value="mapping">Mapping</TabsTrigger>
        </TabsList>
        <TabsContent value="boxes">
          <Section
            title={`Boxes for ${quarter}`}
            description="Amounts are exact SEK from posted vouchers. Check each box against the SKV 4700 form, then mark it verified."
          >
            <DataTable
              columns={prioritize(boxColumns, { label: 1, vouchers: 2, verified: 3 })}
              rows={r?.boxes ?? []}
              getRowId={(b) => b.box}
              rowActions={boxActions}
              onRowAction={({ tag, row }) => {
                if (tag === "unused" || tag === "used")
                  void api(`/vat/boxes/${row.box}/unused`, {
                    method: "POST",
                    body: { not_used: tag === "unused" },
                  })
                    .then(() => {
                      notifyOk(tag === "unused" ? "Box marked not used" : "Box counts again");
                      reload();
                    })
                    .catch(notifyError);
                else void setVerified(row.box, tag === "verify");
              }}
              paginated={false}
              caption="VAT boxes"
            />
            {r ? (
              <p className="oas-muted" style={{ marginTop: ".75rem" }}>
                {r.filing}
              </p>
            ) : null}
            {r ? (
              <p className="oas-muted" style={{ marginTop: ".5rem" }}>
                Deadline: {r.due_date_note}
              </p>
            ) : null}
          </Section>
        </TabsContent>
        <TabsContent value="checks">
          <Section
            title="Before you file"
            description="Every check must pass, including the owner's verification of every box."
          >
            <DataTable
              columns={prioritize(checkColumns, { detail: 2 })}
              rows={r?.checks ?? []}
              getRowId={(c) => c.check}
              paginated={false}
              caption="Readiness checks"
            />
          </Section>
        </TabsContent>
        <TabsContent value="mapping">
          <Section
            title="Account to box mapping"
            description="Output and input VAT accounts are summed into these boxes. Changing a mapping clears that box's verification."
          >
            <DataTable
              columns={
                [
                  { name: "account", header: "Account", width: "90px" },
                  { name: "name", header: "Name" },
                  { name: "box", header: "Box", width: "80px" },
                  {
                    name: "side",
                    header: "Counts",
                    cell: (v) => (v === "credit" ? "credit minus debit" : "debit minus credit"),
                  },
                ] as DataTableColumn<Config["accounts"][number]>[]
              }
              rows={cfg.data?.accounts ?? []}
              getRowId={(a) => String(a.account)}
              paginated={false}
              caption="Account mapping"
            />
          </Section>
          <div style={{ height: "1.5rem" }} />
          <Section
            title="VAT codes"
            description="Put a code on the net line of a voucher so its base amount lands in the right box."
          >
            <DataTable
              columns={
                [
                  { name: "code", header: "Code", width: "110px" },
                  { name: "label", header: "Meaning" },
                  { name: "base_box", header: "Base box", accessor: (c) => c.base_box ?? "none" },
                ] as DataTableColumn<Config["codes"][number]>[]
              }
              rows={cfg.data?.codes ?? []}
              getRowId={(c) => c.code}
              paginated={false}
              caption="VAT codes"
            />
          </Section>
        </TabsContent>
      </Tabs>
    </>
  );
}
