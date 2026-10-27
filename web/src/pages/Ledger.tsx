import {
  Badge,
  Button,
  DataTable,
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Input,
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
} from "@spacecorps/components/ui";
import { Download, Plus, ShieldCheck, Trash2 } from "lucide-react";
import { useState } from "react";
import { api, download, type Account, type Draft, type Me, type Voucher } from "../api";
import { useData } from "../hooks";
import {
  EmptyNote,
  Field,
  Kpi,
  PageHeader,
  Section,
  TextField,
  VatCodeSelect,
  notifyError,
  notifyOk,
  type VatCode,
} from "../kit";
import { amount, parseAmountInput } from "../format";
import { DraftsTable, VouchersTable } from "./shared";

interface DraftLine {
  account: string;
  debit: string;
  credit: string;
  vat: string;
}

function NewDraftDialog({
  open,
  onOpenChange,
  accounts,
  onDone,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  accounts: Account[];
  onDone: () => void;
}) {
  const vatCfg = useData<{ codes: VatCode[] }>("/vat/config");
  const blank = (): DraftLine[] => [
    { account: "", debit: "", credit: "", vat: "" },
    { account: "", debit: "", credit: "", vat: "" },
  ];
  const [date, setDate] = useState(new Date().toLocaleDateString("sv-SE"));
  const [description, setDescription] = useState("");
  const [counterparty, setCounterparty] = useState("");
  const [lines, setLines] = useState<DraftLine[]>(blank());
  const num = parseAmountInput;
  const debit = lines.reduce((s, l) => s + num(l.debit), 0);
  const credit = lines.reduce((s, l) => s + num(l.credit), 0);
  const balanced = Math.abs(debit - credit) < 0.005 && debit > 0;
  const set = (i: number, patch: Partial<DraftLine>) =>
    setLines((ls) => ls.map((l, j) => (j === i ? { ...l, ...patch } : l)));

  async function save() {
    try {
      await api("/drafts", {
        method: "POST",
        body: {
          event_date: date,
          description,
          counterparty: counterparty || null,
          lines: lines
            .filter((l) => l.account)
            .map((l) => ({
              account: Number(l.account),
              ...(num(l.debit) > 0 ? { debit: l.debit } : { credit: l.credit }),
              vat_code: l.vat || null,
            })),
        },
      });
      notifyOk("Draft proposed", "It needs approval before it is posted.");
      setDescription("");
      setCounterparty("");
      setLines(blank());
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
          <DialogTitle>New draft voucher</DialogTitle>
          <DialogDescription>
            Debit and credit must balance. Posting happens only when an admin approves the draft.
          </DialogDescription>
        </DialogHeader>
        <form
          className="oas-dialog-form"
          onSubmit={(e) => {
            e.preventDefault();
            void save();
          }}
        >
          <DialogBody>
            <div className="oas-form">
              <div className="oas-form-row">
                <TextField label="Event date" type="date" value={date} onChange={setDate} />
                <TextField label="Description" value={description} onChange={setDescription} />
                <TextField label="Counterparty" value={counterparty} onChange={setCounterparty} />
              </div>
              <div className="oas-form">
                <div className="oas-lines oas-muted">
                  <span>Account</span>
                  <span>Debit</span>
                  <span>Credit</span>
                  <span>VAT code</span>
                  <span />
                </div>
                {lines.map((l, i) => (
                  <div className="oas-lines" key={i}>
                    <Select value={l.account} onValueChange={(v) => set(i, { account: v })}>
                      <SelectTrigger>
                        <SelectValue placeholder="Choose account" />
                      </SelectTrigger>
                      <SelectContent>
                        {accounts.map((a) => (
                          <SelectItem key={a.number} value={String(a.number)}>
                            {a.number} {a.name}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                    <Input
                      inputMode="decimal"
                      value={l.debit}
                      onChange={(e) => set(i, { debit: e.target.value, credit: "" })}
                      placeholder="0,00"
                    />
                    <Input
                      inputMode="decimal"
                      value={l.credit}
                      onChange={(e) => set(i, { credit: e.target.value, debit: "" })}
                      placeholder="0,00"
                    />
                    <VatCodeSelect
                      value={l.vat}
                      onChange={(v) => set(i, { vat: v })}
                      codes={vatCfg.data?.codes ?? []}
                    />
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      aria-label="Remove line"
                      disabled={lines.length <= 2}
                      onClick={() => setLines((ls) => ls.filter((_, j) => j !== i))}
                    >
                      <Trash2 />
                    </Button>
                  </div>
                ))}
                <div className="oas-actions">
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() =>
                      setLines((ls) => [...ls, { account: "", debit: "", credit: "", vat: "" }])
                    }
                  >
                    <Plus /> Add line
                  </Button>
                  <Badge variant={balanced ? "success" : "warning"}>
                    debit {amount(debit)} / credit {amount(credit)}{" "}
                    {balanced ? "balanced" : "not balanced"}
                  </Badge>
                </div>
              </div>
            </div>
          </DialogBody>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button
              disabled={
                !balanced ||
                !description.trim() ||
                lines.some((l) => (num(l.debit) > 0 || num(l.credit) > 0) && !l.account)
              }
              type="submit"
            >
              Propose draft
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function NewAccountDialog({
  open,
  onOpenChange,
  onDone,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  onDone: () => void;
}) {
  const [number, setNumber] = useState("");
  const [name, setName] = useState("");
  const [kind, setKind] = useState("K");
  async function save() {
    try {
      await api("/accounts", { method: "POST", body: { number: Number(number), name, kind } });
      notifyOk("Account added", `${number} ${name}`);
      setNumber("");
      setName("");
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
          <DialogTitle>Add account</DialogTitle>
          <DialogDescription>Use BAS account numbers (1000-9999).</DialogDescription>
        </DialogHeader>
        <form
          className="oas-dialog-form"
          onSubmit={(e) => {
            e.preventDefault();
            void save();
          }}
        >
          <DialogBody>
            <div className="oas-form">
              <TextField label="Number" value={number} onChange={setNumber} placeholder="6540" />
              <TextField label="Name" value={name} onChange={setName} />
              <Field label="Type">
                <Select value={kind} onValueChange={setKind}>
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="T">Asset</SelectItem>
                    <SelectItem value="S">Liability or equity</SelectItem>
                    <SelectItem value="K">Cost</SelectItem>
                    <SelectItem value="I">Income</SelectItem>
                  </SelectContent>
                </Select>
              </Field>
            </div>
          </DialogBody>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button disabled={!number || !name.trim()} type="submit">
              Add
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

interface TrialRow {
  account: number;
  name: string;
  debit: string;
  credit: string;
  balance: string;
}
interface AuditRow {
  at: string;
  actor: string;
  action: string;
  object: string;
  detail: string;
}

export function Ledger({ me }: { me: Me }) {
  const year = new Date().getFullYear();
  const [tbYear, setTbYear] = useState(String(year));
  const [draftOpen, setDraftOpen] = useState(false);
  const [accountOpen, setAccountOpen] = useState(false);

  const vouchers = useData<{ vouchers: Voucher[] }>("/vouchers");
  const drafts = useData<{ drafts: Draft[] }>("/drafts");
  const accounts = useData<{ accounts: Account[] }>("/accounts");
  const verify = useData<{ ok: boolean; vouchers: number; issues: string[] }>("/ledger/verify");
  const trial = useData<{
    accounts: TrialRow[];
    total_debit: string;
    total_credit: string;
    balanced: boolean;
  }>(`/reports/trial-balance?year=${encodeURIComponent(tbYear)}`);
  const audit = useData<{ audit: AuditRow[] }>("/audit");

  const refresh = () => {
    vouchers.reload();
    drafts.reload();
    verify.reload();
    trial.reload();
    audit.reload();
  };

  async function runVerify() {
    try {
      const v = await api<{ ok: boolean; vouchers: number; issues: string[] }>("/ledger/verify");
      verify.reload();
      if (v.ok)
        notifyOk(
          "Ledger verified",
          `${v.vouchers} vouchers, no gaps, balanced, hash chain intact.`,
        );
      else notifyError(new Error(v.issues.join("; ")));
    } catch (e) {
      notifyError(e);
    }
  }

  async function exportSie() {
    try {
      await download(`/export/sie?year=${encodeURIComponent(tbYear)}`, `oas-${tbYear}.se`);
    } catch (e) {
      notifyError(e);
    }
  }

  const pendingCount = drafts.data?.drafts.length ?? 0;
  const trialColumns: DataTableColumn<TrialRow>[] = [
    { name: "account", header: "Account", accessor: (r) => `${r.account} ${r.name}` },
    { name: "debit", header: "Debit", align: "Right", footer: trial.data?.total_debit },
    { name: "credit", header: "Credit", align: "Right", footer: trial.data?.total_credit },
    { name: "balance", header: "Balance", align: "Right" },
  ];
  const accountColumns: DataTableColumn<Account>[] = [
    { name: "number", header: "Number", width: "100px" },
    { name: "name", header: "Name" },
    {
      name: "kind",
      header: "Type",
      accessor: (a) =>
        ({ T: "Asset", S: "Liability or equity", K: "Cost", I: "Income" })[a.kind] ?? a.kind,
    },
  ];
  const auditColumns: DataTableColumn<AuditRow>[] = [
    { name: "at", header: "When", width: "180px" },
    { name: "actor", header: "Actor" },
    { name: "action", header: "Action" },
    { name: "object", header: "Object" },
    { name: "detail", header: "Detail" },
  ];

  return (
    <>
      <PageHeader
        title="Ledger"
        description="Append-only double-entry books. Postings are proposed, then approved by an admin."
        actions={
          <>
            <Button variant="outline" onClick={() => void runVerify()}>
              <ShieldCheck /> Verify
            </Button>
            <Button variant="outline" onClick={() => void exportSie()}>
              <Download /> Export SIE ({tbYear})
            </Button>
            <Button onClick={() => setDraftOpen(true)}>
              <Plus /> New draft
            </Button>
          </>
        }
      />

      <div className="oas-kpis oas-kpis-3">
        <Kpi label="Posted vouchers" value={vouchers.data?.vouchers.length ?? 0} />
        <Kpi
          label="Drafts to approve"
          value={pendingCount}
          badge={
            pendingCount ? (
              <Badge variant="warning">needs review</Badge>
            ) : (
              <Badge variant="success">all clear</Badge>
            )
          }
        />
        <Kpi
          label="Integrity"
          value={verify.data?.ok ? "Verified" : verify.data ? "Problems" : "..."}
          badge={
            verify.data ? (
              <Badge variant={verify.data.ok ? "success" : "destructive"}>
                {verify.data.ok ? "ok" : "check issues"}
              </Badge>
            ) : null
          }
          hint={
            verify.data && !verify.data.ok ? verify.data.issues[0] : "gaps, balance, hash chain"
          }
        />
      </div>

      <Tabs defaultValue={pendingCount ? "drafts" : "vouchers"}>
        <TabsList>
          <TabsTrigger value="vouchers">Vouchers</TabsTrigger>
          <TabsTrigger value="drafts">Drafts{pendingCount ? ` (${pendingCount})` : ""}</TabsTrigger>
          <TabsTrigger value="trial">Trial balance</TabsTrigger>
          <TabsTrigger value="accounts">Accounts</TabsTrigger>
          <TabsTrigger value="audit">Audit log</TabsTrigger>
        </TabsList>
        <TabsContent value="vouchers">
          <Section title="Posted vouchers" description="Select a row to see its lines.">
            {(vouchers.data?.vouchers ?? []).length === 0 ? (
              <EmptyNote>No vouchers posted yet.</EmptyNote>
            ) : (
              <VouchersTable
                vouchers={vouchers.data?.vouchers ?? []}
                canCorrect={me.can_approve}
                onChanged={refresh}
                pageSize={25}
              />
            )}
          </Section>
        </TabsContent>
        <TabsContent value="drafts">
          <Section
            title="Drafts"
            description={
              me.can_approve
                ? "Approve to post, or reject to discard."
                : "Only an admin can approve drafts."
            }
          >
            {pendingCount === 0 ? (
              <EmptyNote>No drafts waiting.</EmptyNote>
            ) : (
              <DraftsTable
                drafts={drafts.data?.drafts ?? []}
                canApprove={me.can_approve}
                onChanged={refresh}
              />
            )}
          </Section>
        </TabsContent>
        <TabsContent value="trial">
          <Section
            title="Trial balance"
            description="Sums per account for the chosen year, by event date."
            actions={
              <div style={{ width: 120 }}>
                <Input
                  aria-label="Year"
                  value={tbYear}
                  onChange={(e) => setTbYear(e.target.value)}
                  inputMode="numeric"
                />
              </div>
            }
          >
            {(trial.data?.accounts ?? []).length === 0 ? (
              <EmptyNote>No postings in {tbYear}.</EmptyNote>
            ) : (
              <>
                <p className="oas-muted" style={{ marginBottom: ".75rem" }}>
                  <Badge variant={trial.data?.balanced ? "success" : "destructive"}>
                    {trial.data?.balanced ? "balanced" : "not balanced"}
                  </Badge>
                </p>
                <DataTable
                  columns={trialColumns}
                  rows={trial.data?.accounts ?? []}
                  getRowId={(r) => String(r.account)}
                  paginated={false}
                />
              </>
            )}
          </Section>
        </TabsContent>
        <TabsContent value="accounts">
          <Section
            title="Chart of accounts"
            description="A small starter set. Add the accounts you use."
            actions={
              me.can_approve ? (
                <Button size="sm" onClick={() => setAccountOpen(true)}>
                  <Plus /> Add account
                </Button>
              ) : null
            }
          >
            <DataTable
              columns={accountColumns}
              rows={accounts.data?.accounts ?? []}
              getRowId={(a) => String(a.number)}
              defaultPageSize={25}
            />
          </Section>
        </TabsContent>
        <TabsContent value="audit">
          <Section
            title="Audit log"
            description="Most recent 200 actions. The log cannot be edited."
          >
            <DataTable
              columns={auditColumns}
              rows={audit.data?.audit ?? []}
              getRowId={(r) => `${r.at}-${r.action}-${r.object}`}
              defaultPageSize={25}
            />
          </Section>
        </TabsContent>
      </Tabs>

      <NewDraftDialog
        open={draftOpen}
        onOpenChange={setDraftOpen}
        accounts={accounts.data?.accounts ?? []}
        onDone={refresh}
      />
      <NewAccountDialog open={accountOpen} onOpenChange={setAccountOpen} onDone={accounts.reload} />
    </>
  );
}
