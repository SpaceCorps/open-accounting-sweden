import {
  Badge,
  DataTable,
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  type DataTableColumn,
  type DataTableRowAction,
} from "@spacecorps/components/ui";
import { Check, Eye, Undo2, X } from "lucide-react";
import { useState } from "react";
import { api, type Draft, type Line, type TimeEntry, type Voucher } from "../api";
import { ConfirmDialog, notifyError, notifyOk, prioritize } from "../kit";
import { amount, hours, money } from "../format";

export const sumDebit = (lines: Line[]) =>
  lines.reduce((s, l) => s + parseFloat(l.debit), 0).toFixed(2);

const lineColumns: DataTableColumn<Line>[] = [
  { name: "account", header: "Account", accessor: (l) => `${l.account} ${l.name}` },
  { name: "debit", header: "Debit", align: "Right", accessor: (l) => amount(l.debit) },
  { name: "credit", header: "Credit", align: "Right", accessor: (l) => amount(l.credit) },
];

export function VoucherDialog({
  voucher,
  onClose,
}: {
  voucher: Voucher | null;
  onClose: () => void;
}) {
  return (
    <Dialog open={voucher !== null} onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        {voucher ? (
          <>
            <DialogHeader>
              <DialogTitle>
                {voucher.label}: {voucher.description}
              </DialogTitle>
              <DialogDescription>
                Event {voucher.event_date}, compiled {voucher.compile_date}. Proposed by{" "}
                {voucher.created_by}, approved by {voucher.approved_by}.
                {voucher.corrects ? ` Reverses voucher #${voucher.corrects}.` : ""}
              </DialogDescription>
            </DialogHeader>
            <DialogBody>
              <DataTable
                columns={lineColumns}
                rows={voucher.lines}
                getRowId={(_, i) => String(i)}
                paginated={false}
              />
              <p className="oas-muted oas-mono">hash {voucher.hash.slice(0, 16)}...</p>
            </DialogBody>
          </>
        ) : null}
      </DialogContent>
    </Dialog>
  );
}

export function DraftsTable({
  drafts,
  canApprove,
  onChanged,
}: {
  drafts: Draft[];
  canApprove: boolean;
  onChanged: () => void;
}) {
  const [rejecting, setRejecting] = useState<Draft | null>(null);

  const columns: DataTableColumn<Draft>[] = [
    { name: "id", header: "ID", width: "70px" },
    { name: "event_date", header: "Date", width: "120px" },
    { name: "description", header: "Description" },
    { name: "counterparty", header: "Counterparty" },
    {
      name: "amount",
      header: "Amount",
      align: "Right",
      accessor: (d) => parseFloat(sumDebit(d.lines)),
      cell: (v) => money(v as number),
    },
    { name: "created_by", header: "Proposed by" },
  ];
  const actions: DataTableRowAction<Draft>[] = canApprove
    ? [
        {
          tag: "approve",
          label: "Approve",
          icon: <Check aria-hidden="true" />,
          tooltip: "Approve and post",
        },
        {
          tag: "reject",
          label: "Reject",
          icon: <X aria-hidden="true" />,
          tooltip: "Reject draft",
          variant: "destructive",
        },
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
        columns={prioritize(columns, { event_date: 1, counterparty: 2, id: 3, created_by: 4 })}
        rows={drafts}
        getRowId={(d) => String(d.id)}
        getRowLabel={(d) => `Draft ${d.id} ${d.description}`}
        rowActions={actions}
        onRowAction={({ tag, row }) => (tag === "approve" ? void approve(row) : setRejecting(row))}
        defaultPageSize={10}
        caption="Drafts waiting for approval"
      />
      <ConfirmDialog
        open={rejecting !== null}
        title="Reject this draft?"
        description={
          rejecting ? `"${rejecting.description}" will be discarded. This cannot be undone.` : ""
        }
        confirmLabel="Reject draft"
        onConfirm={() => rejecting && void reject(rejecting)}
        onCancel={() => setRejecting(null)}
      />
    </>
  );
}

export function VouchersTable({
  vouchers,
  canCorrect,
  onChanged,
  pageSize = 10,
  compact = false,
}: {
  vouchers: Voucher[];
  canCorrect: boolean;
  onChanged: () => void;
  pageSize?: number;
  /** Fewer columns, for a half-width card. */
  compact?: boolean;
}) {
  const [viewing, setViewing] = useState<Voucher | null>(null);
  const [correcting, setCorrecting] = useState<Voucher | null>(null);

  const columns: DataTableColumn<Voucher>[] = [
    {
      name: "label",
      header: "No",
      width: "80px",
      accessor: (v) => v.number,
      cell: (_, v) => v.label,
    },
    { name: "event_date", header: "Date", width: "120px" },
    { name: "description", header: "Description" },
    {
      name: "amount",
      header: "Amount",
      align: "Right",
      accessor: (v) => parseFloat(sumDebit(v.lines)),
      cell: (v) => money(v as number),
    },
    ...(compact
      ? []
      : ([
          { name: "approved_by", header: "Approved by" },
          {
            name: "corrects",
            header: "Type",
            accessor: (v) => (v.corrects ? "Correction" : "Posting"),
            cell: (v) => (
              <Badge variant={v === "Correction" ? "warning" : "secondary"}>{v as string}</Badge>
            ),
          },
        ] satisfies DataTableColumn<Voucher>[])),
  ];
  const actions = (v: Voucher): DataTableRowAction<Voucher>[] => [
    { tag: "view", label: "View lines", icon: <Eye aria-hidden="true" />, tooltip: "View lines" },
    ...(canCorrect && !v.corrects
      ? [
          {
            tag: "correct",
            label: "Correct",
            icon: <Undo2 aria-hidden="true" />,
            tooltip: "Post a reversing voucher",
          },
        ]
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
        columns={prioritize(columns, { event_date: 1, label: 2, approved_by: 3, corrects: 4 })}
        rows={vouchers}
        getRowId={(v) => String(v.id)}
        getRowLabel={(v) => `${v.label} ${v.description}`}
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
        description={
          correcting
            ? `A reversing voucher will be posted for ${correcting.label}. The original stays in the ledger; posted vouchers are never edited or deleted.`
            : ""
        }
        confirmLabel="Post correction"
        onConfirm={() => correcting && void correct(correcting)}
        onCancel={() => setCorrecting(null)}
      />
    </>
  );
}

const entryColumnDefs: DataTableColumn<TimeEntry>[] = [
  { name: "date", header: "Date", width: "120px" },
  { name: "start", header: "Start", width: "80px" },
  {
    name: "end",
    header: "End",
    width: "80px",
    accessor: (e) => e.end ?? "",
    cell: (v, e) => (e.running ? <Badge variant="info">running</Badge> : (v as string)),
  },
  {
    name: "hours",
    header: "Hours",
    align: "Right",
    width: "90px",
    cell: (v) => hours(v as number),
  },
  {
    name: "amount",
    header: "Amount",
    align: "Right",
    width: "110px",
    accessor: (e) => (e.amount ? parseFloat(e.amount) : 0),
    cell: (_, e) => (e.amount ? money(e.amount, e.currency) : "—"),
  },
  { name: "project", header: "Project" },
  {
    name: "activity",
    header: "Activity",
    width: "110px",
    cell: (v) => <Badge variant="secondary">{v as string}</Badge>,
  },
  { name: "message", header: "Message" },
];

export const entryColumns = prioritize(entryColumnDefs, {
  project: 1,
  amount: 2,
  start: 3,
  end: 3,
  message: 4,
  activity: 5,
});

/** Date, project, hours and amount: the entry columns that fit a half-width card. */
export const compactEntryColumns: DataTableColumn<TimeEntry>[] = [
  "date",
  "project",
  "hours",
  "amount",
]
  .map((name) => entryColumns.find((c) => c.name === name))
  .filter((c) => c !== undefined)
  // These four fit a half-width card; only on a phone do project and amount move into the row details.
  .map((c) => ({
    ...c,
    priority: undefined,
    hideBelow: c.name === "amount" || c.name === "project" ? 340 : undefined,
  }));
