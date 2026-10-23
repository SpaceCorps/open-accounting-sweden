import {
  Badge, Button, Checkbox, DataTable, Dialog, DialogBody, DialogContent, DialogDescription, DialogFooter,
  DialogHeader, DialogTitle, Input, Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
  Tabs, TabsContent, TabsList, TabsTrigger, type DataTableColumn, type DataTableRowAction,
} from "@spacecorps/components/ui";
import { BookPlus, Check, EyeOff, Plus, RefreshCw, Trash2, Upload, X } from "lucide-react";
import { useRef, useState } from "react";
import { api, importBankCsv, type Account, type BankTx, type Doc, type Me, type MatchItem } from "../api";
import { useData } from "../hooks";
import { EmptyNote, Kpi, PageHeader, Section, TextField, VatCodeSelect, notifyError, notifyOk, type VatCode } from "../kit";

interface Row { account: string; debit: string; credit: string; vat: string }
const num = (s: string) => (s.trim() === "" ? 0 : parseFloat(s.replace(",", ".")) || 0);

function BookDialog(props: {
  tx: BankTx | null;
  accounts: Account[];
  docs: Doc[];
  confirmedDocIds: number[];
  onClose: () => void;
  onDone: () => void;
}) {
  const { tx } = props;
  const vatCfg = useData<{ codes: VatCode[] }>("/vat/config");
  const [description, setDescription] = useState("");
  const [rows, setRows] = useState<Row[]>([{ account: "", debit: "", credit: "", vat: "" }]);
  const [picked, setPicked] = useState<number[]>([]);
  const [loaded, setLoaded] = useState<number | null>(null);

  if (tx && loaded !== tx.id) {
    setLoaded(tx.id);
    setDescription(tx.counterparty || tx.reference);
    const out = parseFloat(tx.amount) < 0;
    setRows([{ account: "", debit: out ? Math.abs(parseFloat(tx.amount)).toFixed(2) : "", credit: out ? "" : parseFloat(tx.amount).toFixed(2), vat: "" }]);
    setPicked(props.confirmedDocIds);
  }

  const total = tx ? Math.abs(parseFloat(tx.amount)) : 0;
  const outgoing = tx ? parseFloat(tx.amount) < 0 : true;
  const debit = rows.reduce((s, r) => s + num(r.debit), 0);
  const credit = rows.reduce((s, r) => s + num(r.credit), 0);
  // The bank line is added on the opposite side, so the other lines must net to the bank amount.
  const net = outgoing ? debit - credit : credit - debit;
  const balanced = Math.abs(net - total) < 0.005;
  const set = (i: number, patch: Partial<Row>) => setRows((rs) => rs.map((r, j) => (j === i ? { ...r, ...patch } : r)));
  const selectable = props.docs.filter((d) => d.status === "unlinked" || props.confirmedDocIds.includes(d.id));

  async function save() {
    if (!tx) return;
    try {
      await api(`/bank/transactions/${tx.id}/book`, {
        method: "POST",
        body: {
          description,
          documents: picked,
          lines: rows.filter((r) => r.account).map((r) => ({
            account: Number(r.account),
            ...(num(r.debit) > 0 ? { debit: r.debit } : { credit: r.credit }),
            vat_code: r.vat || null,
          })),
        },
      });
      notifyOk("Draft created", "An admin must approve it before it is posted.");
      props.onClose();
      props.onDone();
    } catch (e) {
      notifyError(e);
    }
  }

  return (
    <Dialog open={tx !== null} onOpenChange={(o) => !o && props.onClose()}>
      <DialogContent style={{ maxWidth: 720 }}>
        <DialogHeader>
          <DialogTitle>Book bank line</DialogTitle>
          <DialogDescription>
            {tx ? `${tx.date}, ${tx.counterparty || tx.reference}, ${tx.amount} ${tx.currency}. ` : ""}
            Choose the other accounts; the bank account (1930) line is added for you.
          </DialogDescription>
        </DialogHeader>
<DialogBody>
        <div className="oas-form">
          <TextField label="Description" value={description} onChange={setDescription} />
          <div className="oas-lines oas-muted"><span>Account</span><span>Debit</span><span>Credit</span><span>VAT code</span><span /></div>
          {rows.map((r, i) => (
            <div className="oas-lines" key={i}>
              <Select value={r.account} onValueChange={(v) => set(i, { account: v })}>
                <SelectTrigger><SelectValue placeholder="Choose account" /></SelectTrigger>
                <SelectContent>
                  {props.accounts.filter((a) => a.number !== 1930).map((a) => <SelectItem key={a.number} value={String(a.number)}>{a.number} {a.name}</SelectItem>)}
                </SelectContent>
              </Select>
              <Input inputMode="decimal" value={r.debit} onChange={(e) => set(i, { debit: e.target.value, credit: "" })} placeholder="0.00" />
              <Input inputMode="decimal" value={r.credit} onChange={(e) => set(i, { credit: e.target.value, debit: "" })} placeholder="0.00" />
              <VatCodeSelect value={r.vat} onChange={(v) => set(i, { vat: v })} codes={vatCfg.data?.codes ?? []} />
              <Button variant="ghost" size="icon" aria-label="Remove line" disabled={rows.length <= 1} onClick={() => setRows((rs) => rs.filter((_, j) => j !== i))}><Trash2 /></Button>
            </div>
          ))}
          <div className="oas-actions">
            <Button variant="outline" size="sm" onClick={() => setRows((rs) => [...rs, { account: "", debit: "", credit: "", vat: "" }])}><Plus /> Add line</Button>
            <Badge variant={balanced ? "success" : "warning"}>
              other lines net {net.toFixed(2)}, bank {total.toFixed(2)} {balanced ? "balanced" : "not balanced"}
            </Badge>
          </div>
          <div className="oas-form">
            <span className="oas-muted">Receipts to attach</span>
            {selectable.length === 0 ? <span className="oas-muted">No unlinked receipts. Upload one in the Inbox.</span> : selectable.map((d) => (
              <label key={d.id} className="oas-actions" style={{ cursor: "pointer" }}>
                <Checkbox checked={picked.includes(d.id)} onCheckedChange={(c) => setPicked((p) => (c ? [...p, d.id] : p.filter((x) => x !== d.id)))} />
                <span>{d.filename}</span>
                <span className="oas-muted">{d.vendor} {d.amount ? `${d.amount} ${d.currency}` : ""} {d.date ?? ""}</span>
                {props.confirmedDocIds.includes(d.id) ? <Badge variant="success">confirmed match</Badge> : null}
              </label>
            ))}
          </div>
        </div>
</DialogBody>
<DialogFooter>
          <Button variant="outline" onClick={props.onClose}>Cancel</Button>
          <Button disabled={!balanced || rows.some((r) => (num(r.debit) > 0 || num(r.credit) > 0) && !r.account) || !rows.some((r) => r.account)} onClick={() => void save()}>Create draft</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export function BankPage({ me }: { me: Me }) {
  const tx = useData<{ transactions: BankTx[] }>("/bank/transactions");
  const matches = useData<{ matches: MatchItem[] }>("/matches?status=pending");
  const confirmed = useData<{ matches: MatchItem[] }>("/matches?status=confirmed");
  const docs = useData<{ documents: Doc[] }>("/documents");
  const accounts = useData<{ accounts: Account[] }>("/accounts");
  const [booking, setBooking] = useState<BankTx | null>(null);
  const [busy, setBusy] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  const reload = () => { tx.reload(); matches.reload(); confirmed.reload(); docs.reload(); };
  const list = tx.data?.transactions ?? [];
  const byStatus = (s: string) => list.filter((t) => t.status === s);
  const pending = matches.data?.matches ?? [];

  async function importFile(files: FileList | null) {
    if (!files?.[0]) return;
    setBusy(true);
    try {
      const r = await importBankCsv(files[0]);
      notifyOk(`Imported ${r.imported} lines`, `${r.duplicates} already known, ${r.new_suggestions} new suggestions${r.errors.length ? `, ${r.errors.length} unreadable rows` : ""}`);
      if (fileRef.current) fileRef.current.value = "";
      reload();
    } catch (e) {
      notifyError(e);
    } finally {
      setBusy(false);
    }
  }

  async function rerun() {
    try {
      const r = await api<{ new_suggestions: number }>("/bank/match", { method: "POST" });
      notifyOk("Matching done", `${r.new_suggestions} new suggestions`);
      reload();
    } catch (e) {
      notifyError(e);
    }
  }

  async function decide(m: MatchItem, what: "confirm" | "decline") {
    try {
      await api(`/matches/${m.id}/${what}`, { method: "POST" });
      notifyOk(what === "confirm" ? "Match confirmed" : "Match declined");
      reload();
    } catch (e) {
      notifyError(e);
    }
  }

  async function ignore(t: BankTx) {
    try {
      await api(`/bank/transactions/${t.id}/ignore`, { method: "POST" });
      notifyOk("Line ignored", t.counterparty);
      reload();
    } catch (e) {
      notifyError(e);
    }
  }

  const money = (t: BankTx) => `${t.amount} ${t.currency}`;
  const txColumns: DataTableColumn<BankTx>[] = [
    { name: "date", header: "Date", width: "120px" },
    { name: "counterparty", header: "Counterparty" },
    { name: "reference", header: "Reference" },
    { name: "amount", header: "Amount", align: "Right", accessor: (t) => parseFloat(t.amount), cell: (_, t) => money(t) },
    {
      name: "state",
      header: "State",
      accessor: (t) => (t.voucher ? `booked ${t.voucher}` : t.draft ? `draft ${t.draft}` : t.pending_matches ? `${t.pending_matches} suggestion` : t.status),
      cell: (v, t) => <Badge variant={t.voucher ? "success" : t.draft ? "info" : t.pending_matches ? "warning" : "secondary"}>{String(v)}</Badge>,
    },
  ];
  const toBookActions = (t: BankTx): DataTableRowAction<BankTx>[] =>
    t.draft
      ? []
      : [
          { tag: "book", label: "Book", icon: <BookPlus aria-hidden="true" />, tooltip: "Create a draft voucher" },
          ...(me.can_approve ? [{ tag: "ignore", label: "Ignore", icon: <EyeOff aria-hidden="true" />, tooltip: "Ignore this line" }] : []),
        ];

  const matchColumns: DataTableColumn<MatchItem>[] = [
    { name: "score", header: "Score", width: "80px", accessor: (m) => m.score, cell: (v) => <Badge variant={(v as number) >= 0.9 ? "success" : "warning"}>{(v as number).toFixed(2)}</Badge> },
    { name: "bank", header: "Bank line", accessor: (m) => `${m.transaction.date} ${m.transaction.counterparty}`, cell: (_, m) => `${m.transaction.date} ${m.transaction.counterparty} ${money(m.transaction)}` },
    { name: "receipt", header: "Receipt", accessor: (m) => m.document.filename, cell: (_, m) => `${m.document.filename}: ${m.document.vendor} ${m.document.amount ?? ""} ${m.document.date ?? ""}` },
    { name: "why", header: "Why", accessor: (m) => m.scores.name, cell: (_, m) => `name ${m.scores.name.toFixed(2)}, amount ${m.scores.amount.toFixed(2)}, date ${m.scores.date.toFixed(2)}` },
  ];
  const matchActions: DataTableRowAction<MatchItem>[] = me.can_approve
    ? [
        { tag: "confirm", label: "Confirm", icon: <Check aria-hidden="true" />, tooltip: "Confirm match" },
        { tag: "decline", label: "Decline", icon: <X aria-hidden="true" />, tooltip: "Decline match", variant: "destructive" },
      ]
    : [];

  return (
    <>
      <PageHeader
        title="Bank"
        description="Import bank statements, match receipts, and book lines as draft vouchers."
        actions={
          <>
            <input ref={fileRef} type="file" accept=".csv,text/csv" hidden onChange={(e) => void importFile(e.target.files)} />
            <Button variant="outline" onClick={() => void rerun()}><RefreshCw /> Match receipts</Button>
            <Button disabled={busy} onClick={() => fileRef.current?.click()}><Upload /> {busy ? "Importing..." : "Import CSV"}</Button>
          </>
        }
      />
      <div className="oas-kpis">
        <Kpi label="To book" value={byStatus("new").length} badge={byStatus("new").length ? <Badge variant="warning">open</Badge> : <Badge variant="success">done</Badge>} />
        <Kpi label="Receipt suggestions" value={pending.length} />
        <Kpi label="Booked" value={byStatus("booked").length} />
      </div>

      <Tabs defaultValue="new">
        <TabsList>
          <TabsTrigger value="new">To book ({byStatus("new").length})</TabsTrigger>
          <TabsTrigger value="matches">Suggestions ({pending.length})</TabsTrigger>
          <TabsTrigger value="booked">Booked</TabsTrigger>
          <TabsTrigger value="ignored">Ignored</TabsTrigger>
        </TabsList>
        <TabsContent value="new">
          <Section title="Bank lines to book" description="Import a Lunar CSV export (comma or semicolon separated). Importing the same file twice is safe.">
            {byStatus("new").length === 0 ? <EmptyNote>Nothing to book. Import a bank statement to begin.</EmptyNote> : (
              <DataTable
                columns={txColumns}
                rows={byStatus("new")}
                getRowId={(t) => String(t.id)}
                rowActions={toBookActions}
                onRowAction={({ tag, row }) => (tag === "book" ? setBooking(row) : void ignore(row))}
                defaultPageSize={15}
                caption="Bank lines to book"
              />
            )}
          </Section>
        </TabsContent>
        <TabsContent value="matches">
          <Section title="Receipt suggestions" description={me.can_approve ? "Confirm a match to attach the receipt when you book the line." : "Only an admin can confirm or decline suggestions."}>
            {pending.length === 0 ? <EmptyNote>No suggestions. Upload receipts with vendor, amount and date to get matches.</EmptyNote> : (
              <DataTable columns={matchColumns} rows={pending} getRowId={(m) => String(m.id)} rowActions={matchActions} onRowAction={({ tag, row }) => void decide(row, tag as "confirm" | "decline")} defaultPageSize={10} caption="Suggested matches" />
            )}
          </Section>
        </TabsContent>
        <TabsContent value="booked">
          <Section title="Booked lines">
            {byStatus("booked").length === 0 ? <EmptyNote>No lines booked yet.</EmptyNote> : <DataTable columns={txColumns} rows={byStatus("booked")} getRowId={(t) => String(t.id)} defaultPageSize={15} />}
          </Section>
        </TabsContent>
        <TabsContent value="ignored">
          <Section title="Ignored lines">
            {byStatus("ignored").length === 0 ? <EmptyNote>Nothing ignored.</EmptyNote> : <DataTable columns={txColumns} rows={byStatus("ignored")} getRowId={(t) => String(t.id)} defaultPageSize={15} />}
          </Section>
        </TabsContent>
      </Tabs>

      <BookDialog
        tx={booking}
        accounts={accounts.data?.accounts ?? []}
        docs={docs.data?.documents ?? []}
        confirmedDocIds={(confirmed.data?.matches ?? []).filter((m) => m.transaction.id === booking?.id).map((m) => m.document.id)}
        onClose={() => setBooking(null)}
        onDone={reload}
      />
    </>
  );
}
