import {
  Badge, Button, DataTable, Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader,
  DialogTitle, Input, Tabs, TabsContent, TabsList, TabsTrigger,
  type DataTableColumn, type DataTableRowAction,
} from "@spacecorps/components/ui";
import { Check, Download, Eye, Pencil, Plus, ShieldAlert, ShieldCheck, Trash2, UserMinus } from "lucide-react";
import { useState } from "react";
import { api, download, type Me } from "../api";
import { useData } from "../hooks";
import { ConfirmDialog, EmptyNote, Kpi, PageHeader, Section, TextField, notifyError, notifyOk } from "../kit";

interface Employee { id: number; name: string; identity: string; birth_year: number; active: boolean }
interface RunLine { employee_id: number; name: string; identity: string; gross: string; tax: string; net: string; contribution: string; rate_note: string }
interface Run {
  id: number; period: string; pay_date: string; status: "draft" | "approved"; lines: RunLine[];
  totals: { gross: string; tax: string; net: string; contributions: string; to_skattekonto: string };
  agi_due: string | null; agi_due_note: string; unverified_rates: string[]; ready_for_agi: boolean; ledger_draft_id: number | null;
}
interface Rate { key: string; label: string; value: string; source: string; verified: boolean; verified_by: string | null }

function AddEmployee({ open, onOpenChange, onDone }: { open: boolean; onOpenChange: (o: boolean) => void; onDone: () => void }) {
  const [name, setName] = useState("");
  const [identity, setIdentity] = useState("");
  async function save() {
    try {
      await api("/employees", { method: "POST", body: { name, identity } });
      notifyOk("Employee added", name);
      setName(""); setIdentity("");
      onOpenChange(false);
      onDone();
    } catch (e) {
      notifyError(e);
    }
  }
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Add employee</DialogTitle>
          <DialogDescription>Include yourself if you take a salary. The identity number is kept for the AGI file and shown only to admins.</DialogDescription>
        </DialogHeader>
        <div className="oas-form">
          <TextField label="Name" value={name} onChange={setName} />
          <TextField label="Personnummer" value={identity} onChange={setIdentity} placeholder="YYYYMMDDNNNC" hint="12 digits; a samordningsnummer also works" autoComplete="off" />
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>Cancel</Button>
          <Button disabled={!name.trim() || identity.replace(/\D/g, "").length !== 12} onClick={() => void save()}>Add</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

interface RowIn { employee_id: number; name: string; gross: string; tax: string; percent: string }

function NewRun({ open, onOpenChange, employees, onDone }: { open: boolean; onOpenChange: (o: boolean) => void; employees: Employee[]; onDone: () => void }) {
  const d = new Date();
  const [period, setPeriod] = useState(`${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`);
  const [payDate, setPayDate] = useState(new Date(d.getFullYear(), d.getMonth(), 25).toLocaleDateString("sv-SE"));
  const [rows, setRows] = useState<Record<number, RowIn>>({});
  const active = employees.filter((e) => e.active);
  const get = (e: Employee): RowIn => rows[e.id] ?? { employee_id: e.id, name: e.name, gross: "", tax: "", percent: "" };
  const set = (e: Employee, patch: Partial<RowIn>) => setRows((r) => ({ ...r, [e.id]: { ...get(e), ...patch } }));
  const filled = active.map(get).filter((r) => r.gross.trim() !== "");

  async function save() {
    try {
      await api("/payroll/runs", {
        method: "POST",
        body: {
          period, pay_date: payDate,
          lines: filled.map((r) => ({
            employee_id: r.employee_id, gross: r.gross,
            ...(r.tax.trim() !== "" ? { tax: r.tax } : { tax_percent: parseFloat(r.percent || "0") }),
          })),
        },
      });
      notifyOk("Pay run created", `${period}: review the numbers, then approve.`);
      setRows({});
      onOpenChange(false);
      onDone();
    } catch (e) {
      notifyError(e);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent style={{ maxWidth: 720 }}>
        <DialogHeader>
          <DialogTitle>New pay run</DialogTitle>
          <DialogDescription>Gross cash salary only. Take the preliminary tax from Skatteverket's tax table, or enter a percentage. Leave gross empty to skip a person this month.</DialogDescription>
        </DialogHeader>
        <div className="oas-form">
          <div className="oas-form-row">
            <TextField label="Period" value={period} onChange={setPeriod} placeholder="2026-10" />
            <TextField label="Pay date" type="date" value={payDate} onChange={setPayDate} />
          </div>
          {active.length === 0 ? <EmptyNote>Add an employee first.</EmptyNote> : (
            <>
              <div className="oas-lines oas-muted" style={{ gridTemplateColumns: "1fr 8rem 8rem 6rem" }}><span>Employee</span><span>Gross (SEK)</span><span>Tax (SEK)</span><span>or tax %</span></div>
              {active.map((e) => {
                const r = get(e);
                return (
                  <div className="oas-lines" style={{ gridTemplateColumns: "1fr 8rem 8rem 6rem" }} key={e.id}>
                    <span>{e.name}</span>
                    <Input inputMode="decimal" value={r.gross} onChange={(ev) => set(e, { gross: ev.target.value })} placeholder="0.00" />
                    <Input inputMode="decimal" value={r.tax} onChange={(ev) => set(e, { tax: ev.target.value })} placeholder="0.00" />
                    <Input inputMode="decimal" value={r.percent} onChange={(ev) => set(e, { percent: ev.target.value })} placeholder="30" />
                  </div>
                );
              })}
            </>
          )}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>Cancel</Button>
          <Button disabled={filled.length === 0 || filled.some((r) => r.tax.trim() === "" && r.percent.trim() === "")} onClick={() => void save()}>Create pay run</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function RunDialog({ run, onClose }: { run: Run | null; onClose: () => void }) {
  const cols: DataTableColumn<RunLine>[] = [
    { name: "name", header: "Employee" },
    { name: "identity", header: "Identity" },
    { name: "gross", header: "Gross", align: "Right" },
    { name: "tax", header: "Tax", align: "Right" },
    { name: "net", header: "Net", align: "Right" },
    { name: "contribution", header: "Employer contributions", align: "Right" },
    { name: "rate_note", header: "Rate used", wrapText: true },
  ];
  return (
    <Dialog open={run !== null} onOpenChange={(o) => !o && onClose()}>
      <DialogContent style={{ maxWidth: 900 }}>
        {run ? (
          <>
            <DialogHeader>
              <DialogTitle>Pay run {run.period}</DialogTitle>
              <DialogDescription>
                Paid {run.pay_date}. Gross {run.totals.gross}, tax {run.totals.tax}, contributions {run.totals.contributions}; {run.totals.to_skattekonto} goes to the tax account. AGI due {run.agi_due ?? "?"} (unverified).
              </DialogDescription>
            </DialogHeader>
            <DataTable columns={cols} rows={run.lines} getRowId={(l) => String(l.employee_id)} paginated={false} />
          </>
        ) : null}
      </DialogContent>
    </Dialog>
  );
}

export function PayrollPage({ me }: { me: Me }) {
  const employees = useData<{ employees: Employee[] }>("/employees");
  const runs = useData<{ runs: Run[] }>("/payroll/runs");
  const rates = useData<{ rates: Rate[] }>("/payroll/rates");
  const [addOpen, setAddOpen] = useState(false);
  const [newRunOpen, setNewRunOpen] = useState(false);
  const [viewing, setViewing] = useState<Run | null>(null);
  const [confirm, setConfirm] = useState<{ run: Run; what: "approve" | "delete" } | null>(null);
  const [editRate, setEditRate] = useState<Rate | null>(null);
  const [rateValue, setRateValue] = useState("");

  const reload = () => { employees.reload(); runs.reload(); rates.reload(); };
  const unverified = (rates.data?.rates ?? []).filter((r) => !r.verified);

  async function act(run: Run, what: "approve" | "delete") {
    setConfirm(null);
    try {
      if (what === "approve") {
        await api(`/payroll/runs/${run.id}/approve`, { method: "POST" });
        notifyOk(`Pay run ${run.period} approved`, "A draft voucher was proposed in the ledger. Approve it there.");
      } else {
        await api(`/payroll/runs/${run.id}/delete`, { method: "POST" });
        notifyOk("Pay run deleted", run.period);
      }
      reload();
    } catch (e) {
      notifyError(e);
    }
  }

  async function setVerified(key: string, verified: boolean) {
    try {
      await api(`/payroll/rates/${key}/verify`, { method: "POST", body: { verified } });
      notifyOk(verified ? "Rate marked verified" : "Rate marked unverified");
      reload();
    } catch (e) {
      notifyError(e);
    }
  }

  async function saveRate() {
    if (!editRate) return;
    try {
      await api(`/payroll/rates/${editRate.key}`, { method: "POST", body: { value: rateValue } });
      notifyOk("Rate changed", "It is unverified until you check it.");
      setEditRate(null);
      reload();
    } catch (e) {
      notifyError(e);
    }
  }

  if (!me.can_approve) {
    return (
      <>
        <PageHeader title="Payroll" />
        <EmptyNote>Payroll is visible to admins only.</EmptyNote>
      </>
    );
  }

  const empColumns: DataTableColumn<Employee>[] = [
    { name: "name", header: "Name" },
    { name: "identity", header: "Identity" },
    { name: "birth_year", header: "Born", width: "90px" },
    { name: "active", header: "Status", accessor: (e) => (e.active ? "active" : "inactive"), cell: (v) => <Badge variant={v === "active" ? "success" : "secondary"}>{v as string}</Badge> },
  ];
  const empActions = (e: Employee): DataTableRowAction<Employee>[] =>
    e.active ? [{ tag: "deactivate", label: "Deactivate", icon: <UserMinus aria-hidden="true" />, tooltip: "Deactivate", variant: "destructive" }] : [];

  const runColumns: DataTableColumn<Run>[] = [
    { name: "period", header: "Period", width: "100px" },
    { name: "pay_date", header: "Paid" },
    { name: "gross", header: "Gross", align: "Right", accessor: (r) => parseFloat(r.totals.gross), cell: (_, r) => r.totals.gross },
    { name: "tax", header: "Tax", align: "Right", accessor: (r) => parseFloat(r.totals.tax), cell: (_, r) => r.totals.tax },
    { name: "contrib", header: "Contributions", align: "Right", accessor: (r) => parseFloat(r.totals.contributions), cell: (_, r) => r.totals.contributions },
    { name: "agi", header: "AGI due", accessor: (r) => r.agi_due ?? "" },
    {
      name: "status", header: "Status",
      accessor: (r) => (r.status === "draft" ? "draft" : r.ready_for_agi ? "ready for AGI" : "approved, rates unverified"),
      cell: (v, r) => <Badge variant={r.status === "draft" ? "info" : r.ready_for_agi ? "success" : "warning"}>{v as string}</Badge>,
    },
  ];
  const runActions = (r: Run): DataTableRowAction<Run>[] => [
    { tag: "view", label: "View", icon: <Eye aria-hidden="true" />, tooltip: "View lines" },
    ...(r.status === "draft"
      ? [
          { tag: "approve", label: "Approve", icon: <Check aria-hidden="true" />, tooltip: "Approve and propose the ledger draft" },
          { tag: "delete", label: "Delete", icon: <Trash2 aria-hidden="true" />, tooltip: "Delete draft", variant: "destructive" as const },
        ]
      : [{ tag: "agi", label: "AGI file", icon: <Download aria-hidden="true" />, tooltip: "Download AGI XML (unverified structure)" }]),
  ];

  const rateColumns: DataTableColumn<Rate>[] = [
    { name: "label", header: "Rate or limit" },
    { name: "value", header: "Value", width: "110px" },
    { name: "verified", header: "Status", accessor: (r) => (r.verified ? "verified" : "UNVERIFIED"), cell: (v) => <Badge variant={v === "verified" ? "success" : "warning"}>{v as string}</Badge> },
    { name: "source", header: "Source", wrapText: true },
  ];
  const rateActions = (r: Rate): DataTableRowAction<Rate>[] => [
    r.verified
      ? { tag: "unverify", label: "Mark unverified", icon: <ShieldAlert aria-hidden="true" />, tooltip: "Mark unverified" }
      : { tag: "verify", label: "I checked this", icon: <ShieldCheck aria-hidden="true" />, tooltip: "I checked this against Skatteverket" },
    { tag: "edit", label: "Change value", icon: <Pencil aria-hidden="true" />, tooltip: "Change value" },
  ];

  const list = runs.data?.runs ?? [];
  return (
    <>
      <PageHeader
        title="Payroll"
        description="Monthly pay runs for you and your employees, employer contributions, a ledger draft and the AGI file. Cash salary only."
        actions={<Button onClick={() => setNewRunOpen(true)}><Plus /> New pay run</Button>}
      />
      {unverified.length > 0 ? (
        <div className="oas-banner" role="alert">
          <span><Badge variant="warning">unverified</Badge> {unverified.length} rates and limits are unverified, and the AGI file structure is unverified until it passes Skatteverket's AGI test service. Do not file from these numbers until you have checked them.</span>
        </div>
      ) : null}
      <div className="oas-kpis">
        <Kpi label="Employees" value={(employees.data?.employees ?? []).filter((e) => e.active).length} />
        <Kpi label="Pay runs" value={list.length} hint={`${list.filter((r) => r.status === "draft").length} draft`} />
        <Kpi label="Unverified rates" value={unverified.length} badge={unverified.length ? <Badge variant="warning">check them</Badge> : <Badge variant="success">all checked</Badge>} />
      </div>

      <Tabs defaultValue="runs">
        <TabsList>
          <TabsTrigger value="runs">Pay runs</TabsTrigger>
          <TabsTrigger value="employees">Employees</TabsTrigger>
          <TabsTrigger value="rates">Rates and limits</TabsTrigger>
        </TabsList>
        <TabsContent value="runs">
          <Section title="Pay runs" description="Approve a run to lock it and propose the ledger voucher. The AGI file is available after approval.">
            {list.length === 0 ? <EmptyNote>No pay runs yet.</EmptyNote> : (
              <DataTable
                columns={runColumns}
                rows={list}
                getRowId={(r) => String(r.id)}
                rowActions={runActions}
                onRowAction={({ tag, row }) => {
                  if (tag === "view") setViewing(row);
                  else if (tag === "agi") void download(`/payroll/runs/${row.id}/agi.xml`, `agi-${row.period}.xml`).catch(notifyError);
                  else setConfirm({ run: row, what: tag as "approve" | "delete" });
                }}
                defaultSort={{ column: "period", direction: "Descending" }}
                defaultPageSize={12}
                caption="Pay runs"
              />
            )}
          </Section>
        </TabsContent>
        <TabsContent value="employees">
          <Section title="Employees" actions={<Button size="sm" onClick={() => setAddOpen(true)}><Plus /> Add employee</Button>}>
            {(employees.data?.employees ?? []).length === 0 ? <EmptyNote>No employees yet. Add yourself and anyone you pay.</EmptyNote> : (
              <DataTable
                columns={empColumns}
                rows={employees.data?.employees ?? []}
                getRowId={(e) => String(e.id)}
                rowActions={empActions}
                onRowAction={({ row }) => void api(`/employees/${row.id}/deactivate`, { method: "POST" }).then(() => { notifyOk("Employee deactivated", row.name); reload(); }).catch(notifyError)}
                paginated={false}
                caption="Employees"
              />
            )}
          </Section>
        </TabsContent>
        <TabsContent value="rates">
          <Section title="Rates and limits" description="Check each against Skatteverket's pages, then mark it verified. Changing a value clears its verification.">
            <DataTable
              columns={rateColumns}
              rows={rates.data?.rates ?? []}
              getRowId={(r) => r.key}
              rowActions={rateActions}
              onRowAction={({ tag, row }) => {
                if (tag === "edit") { setEditRate(row); setRateValue(row.value); }
                else void setVerified(row.key, tag === "verify");
              }}
              paginated={false}
              caption="Payroll rates"
            />
          </Section>
        </TabsContent>
      </Tabs>

      <AddEmployee open={addOpen} onOpenChange={setAddOpen} onDone={reload} />
      <NewRun open={newRunOpen} onOpenChange={setNewRunOpen} employees={employees.data?.employees ?? []} onDone={reload} />
      <RunDialog run={viewing} onClose={() => setViewing(null)} />
      <ConfirmDialog
        open={confirm !== null}
        title={confirm?.what === "approve" ? "Approve this pay run?" : "Delete this draft pay run?"}
        description={confirm?.what === "approve" ? `Pay run ${confirm.run.period} will be locked and a draft voucher proposed in the ledger.` : `Pay run ${confirm?.run.period ?? ""} will be removed.`}
        confirmLabel={confirm?.what === "approve" ? "Approve" : "Delete"}
        onConfirm={() => confirm && void act(confirm.run, confirm.what)}
        onCancel={() => setConfirm(null)}
      />
      <Dialog open={editRate !== null} onOpenChange={(o) => !o && setEditRate(null)}>
        <DialogContent>
          <DialogHeader><DialogTitle>Change value</DialogTitle><DialogDescription>{editRate?.label}. It stays unverified until you check it.</DialogDescription></DialogHeader>
          <TextField label="Value" value={rateValue} onChange={setRateValue} />
          <DialogFooter><Button variant="outline" onClick={() => setEditRate(null)}>Cancel</Button><Button onClick={() => void saveRate()}>Save</Button></DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
