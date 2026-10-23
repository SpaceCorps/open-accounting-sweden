import {
  Badge, DataTable, Dialog, DialogBody, DialogContent, DialogDescription, DialogHeader, DialogTitle,
  type DataTableColumn, type DataTableRowAction,
} from "@spacecorps/components/ui";
import { Check, Eye, Undo2, X } from "lucide-react";
import { useState } from "react";
import { api, type Draft, type Line, type TimeEntry, type Voucher } from "../api";
import { ConfirmDialog, notifyError, notifyOk } from "../kit";

export const sumDebit = (lines: Line[]) =>
  lines.reduce((s, l) => s + parseFloat(l.debit), 0).toFixed(2);

const lineColumns: DataTableColumn<Line>[] = [
  { name: "account", header: "Account", accessor: (l) => `${l.account} ${l.name}` },
  { name: "debit", header: "Debit", align: "Right", accessor: (l) => l.debit },
  { name: "credit", header: "Credit", align: "Right", accessor: (l) => l.credit },
];

export function VoucherDialog({ voucher, onClose }: { voucher: Voucher | null; onClose: () => void }) {
  return (
    <Dialog open={voucher !== null} onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        {voucher ? (
          <>
            <DialogHeader>
              <DialogTitle>{voucher.label}: {voucher.description}</DialogTitle>
              <DialogDescription>
                Event {voucher.event_date}, compiled {voucher.compile_date}. Proposed by {voucher.created_by}, approved by {voucher.approved_by}.
                {voucher.corrects ? ` Reverses voucher #${voucher.corrects}.` : ""}
              </DialogDescription>
            </DialogHeader>
<DialogBody>
            <DataTable columns={lineColumns} rows={voucher.lines} getRowId={(_, i) => String(i)} paginated={false} />
            <p className="oas-muted oas-mono">hash {voucher.hash.slice(0, 16)}...</p>
</DialogBody>
</>
        ) : null}
      </DialogContent>
    </Dialog>
  );
}

export function DraftsTable({ drafts, canApprove, onChanged }: { drafts: Draft[]; canApprove: boolean; onChanged: () => void }) {
  const [rejecting, setRejecting] = useState<Draft | null>(null);

  const columns: DataTableColumn<Draft>[] = [
    { name: "id", header: "ID", width: "70px" },
    { name: "event_date", header: "Date", width: "120px" },
    { name: "description", header: "Description" },
    { name: "counterparty", header: "Counterparty" },
    { name: "amount", header: "Amount", align: "Right", accessor: (d) => parseFloat(sumDebit(d.lines)), cell: (v) => (v as number).toFixed(2) },
    { name: "created_by", header: "Proposed by" },
  ];
  const actions: DataTableRowAction<Draft>[] = canApprove
    ? [
        { tag: "approve", label: "Approve", icon: <Check aria-hidden="true" />, tooltip: "Approve and post" },
        { tag: "reject", label: "Reject", icon: <X aria-hidden="true" />, tooltip: "Reject draft", variant: "destructive" },
      ]
    : [];

  async function approve(d: Draft) {
    try {
      const res = await api<{ voucher: Voucher }>(`/drafts/${d.id}/approve`, { method: "POST" });
      notifyOk(`Posted ${res.voucher.label}`, d.description);
      onChanged();
    } catch (e) {
      notifyError(e);
    }
  }

  async function reject(d: Draft) {
    setRejecting(null);
    try {
      await api(`/drafts/${d.id}/reject`, { method: "POST" });
      notifyOk("Draft rejected", d.description);
      onChanged();
    } catch (e) {
      notifyError(e);
    }
  }

  return (
    <>
      <DataTable
        columns={columns}
        rows={drafts}
        getRowId={(d) => String(d.id)}
        rowActions={actions}
        onRowAction={({ tag, row }) => (tag === "approve" ? void approve(row) : setRejecting(row))}
        defaultPageSize={10}
        caption="Drafts waiting for approval"
      />
      <ConfirmDialog
        open={rejecting !== null}
        title="Reject this draft?"
        description={rejecting ? `"${rejecting.description}" will be discarded. This cannot be undone.` : ""}
        confirmLabel="Reject draft"
        onConfirm={() => rejecting && void reject(rejecting)}
        onCancel={() => setRejecting(null)}
      />
    </>
  );
}

export function VouchersTable({ vouchers, canCorrect, onChanged, pageSize = 10 }: { vouchers: Voucher[]; canCorrect: boolean; onChanged: () => void; pageSize?: number }) {
  const [viewing, setViewing] = useState<Voucher | null>(null);
  const [correcting, setCorrecting] = useState<Voucher | null>(null);

  const columns: DataTableColumn<Voucher>[] = [
    { name: "label", header: "No", width: "80px", accessor: (v) => v.number, cell: (_, v) => v.label },
    { name: "event_date", header: "Date", width: "120px" },
    { name: "description", header: "Description" },
    { name: "amount", header: "Amount", align: "Right", accessor: (v) => parseFloat(sumDebit(v.lines)), cell: (v) => (v as number).toFixed(2) },
    { name: "approved_by", header: "Approved by" },
    {
      name: "corrects",
      header: "Type",
      accessor: (v) => (v.corrects ? "Correction" : "Posting"),
      cell: (v) => <Badge variant={v === "Correction" ? "warning" : "secondary"}>{v as string}</Badge>,
    },
  ];
  const actions = (v: Voucher): DataTableRowAction<Voucher>[] => [
    { tag: "view", label: "View lines", icon: <Eye aria-hidden="true" />, tooltip: "View lines" },
    ...(canCorrect && !v.corrects
      ? [{ tag: "correct", label: "Correct", icon: <Undo2 aria-hidden="true" />, tooltip: "Post a reversing voucher" }]
      : []),
  ];

  async function correct(v: Voucher) {
    setCorrecting(null);
    try {
      const res = await api<{ voucher: Voucher }>(`/vouchers/${v.id}/correct`, { method: "POST" });
      notifyOk(`Posted ${res.voucher.label}`, `Reverses ${v.label}`);
      onChanged();
    } catch (e) {
      notifyError(e);
    }
  }

  return (
    <>
      <DataTable
        columns={columns}
        rows={vouchers}
        getRowId={(v) => String(v.id)}
        rowActions={actions}
        onRowAction={({ tag, row }) => (tag === "view" ? setViewing(row) : setCorrecting(row))}
        onRowClick={(row) => setViewing(row)}
        defaultPageSize={pageSize}
        defaultSort={{ column: "label", direction: "Descending" }}
        caption="Posted vouchers"
      />
      <VoucherDialog voucher={viewing} onClose={() => setViewing(null)} />
      <ConfirmDialog
        open={correcting !== null}
        title="Post a correction?"
        description={correcting ? `A reversing voucher will be posted for ${correcting.label}. The original stays in the ledger; posted vouchers are never edited or deleted.` : ""}
        confirmLabel="Post correction"
        onConfirm={() => correcting && void correct(correcting)}
        onCancel={() => setCorrecting(null)}
      />
    </>
  );
}

export const entryColumns: DataTableColumn<TimeEntry>[] = [
  { name: "date", header: "Date", width: "120px" },
  { name: "start", header: "Start", width: "80px" },
  { name: "end", header: "End", width: "80px", accessor: (e) => e.end ?? "", cell: (v, e) => (e.running ? <Badge variant="info">running</Badge> : (v as string)) },
  { name: "hours", header: "Hours", align: "Right", width: "90px", cell: (v) => (v as number).toFixed(2) },
  { name: "amount", header: "Amount", align: "Right", accessor: (e) => (e.amount ? `${e.amount} ${e.currency}` : "") },
  { name: "project", header: "Project" },
  { name: "activity", header: "Activity", cell: (v) => <Badge variant="secondary">{v as string}</Badge> },
  { name: "message", header: "Message" },
];
