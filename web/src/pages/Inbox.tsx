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
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  type DataTableColumn,
  type DataTableRowAction,
} from "@spacecorps/components/ui";
import { Download, Pencil, Upload } from "lucide-react";
import { useRef, useState } from "react";
import { api, download, uploadDocument, type Doc } from "../api";
import { useData } from "../hooks";
import {
  EmptyNote,
  Kpi,
  PageHeader,
  Section,
  TextField,
  notifyError,
  notifyOk,
  prioritize,
} from "../kit";
import { money } from "../format";

const statusVariant = { unlinked: "warning", draft: "info", booked: "success" } as const;

function EditDialog({
  doc,
  onClose,
  onDone,
}: {
  doc: Doc | null;
  onClose: () => void;
  onDone: () => void;
}) {
  const [vendor, setVendor] = useState("");
  const [amount, setAmount] = useState("");
  const [currency, setCurrency] = useState("SEK");
  const [date, setDate] = useState("");
  const [notes, setNotes] = useState("");
  const [loaded, setLoaded] = useState<number | null>(null);
  if (doc && loaded !== doc.id) {
    setLoaded(doc.id);
    setVendor(doc.vendor);
    setAmount(doc.amount ?? "");
    setCurrency(doc.currency);
    setDate(doc.date ?? "");
    setNotes(doc.notes);
  }
  async function save() {
    if (!doc) return;
    try {
      await api(`/documents/${doc.id}/meta`, {
        method: "POST",
        body: { vendor, amount, currency, date, notes },
      });
      notifyOk("Details saved", doc.filename);
      onClose();
      onDone();
    } catch (e) {
      notifyError(e);
    }
  }
  return (
    <Dialog open={doc !== null} onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Receipt details</DialogTitle>
          <DialogDescription>
            {doc?.filename}. These are used to match the receipt to a bank line. The file itself
            never changes.
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
              <TextField label="Vendor" value={vendor} onChange={setVendor} />
              <div className="oas-form-row">
                <TextField
                  label="Amount"
                  value={amount}
                  onChange={setAmount}
                  placeholder="1 250,00"
                />
                <TextField label="Currency" value={currency} onChange={setCurrency} />
                <TextField label="Receipt date" type="date" value={date} onChange={setDate} />
              </div>
              <TextField label="Notes" value={notes} onChange={setNotes} />
            </div>
          </DialogBody>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit">Save</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

export function InboxPage() {
  const [status, setStatus] = useState("all");
  const docs = useData<{ documents: Doc[] }>(
    status === "all" ? "/documents" : `/documents?status=${status}`,
  );
  const all = useData<{ documents: Doc[] }>("/documents");
  const [vendor, setVendor] = useState("");
  const [amount, setAmount] = useState("");
  const [currency, setCurrency] = useState("SEK");
  const [date, setDate] = useState("");
  const [busy, setBusy] = useState(false);
  const [editing, setEditing] = useState<Doc | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  const reload = () => {
    docs.reload();
    all.reload();
  };
  const list = all.data?.documents ?? [];
  const count = (s: string) => list.filter((d) => d.status === s).length;

  async function upload(files: FileList | null) {
    if (!files || files.length === 0) return;
    setBusy(true);
    try {
      const single = files.length === 1;
      let dup = 0;
      for (const f of Array.from(files)) {
        const res = await uploadDocument(f, single ? { vendor, amount, currency, date } : {});
        if (res.document.duplicate) dup += 1;
      }
      notifyOk(
        `Uploaded ${files.length - dup} file${files.length - dup === 1 ? "" : "s"}`,
        dup ? `${dup} already existed` : undefined,
      );
      setVendor("");
      setAmount("");
      setDate("");
      if (fileRef.current) fileRef.current.value = "";
      reload();
    } catch (e) {
      notifyError(e);
    } finally {
      setBusy(false);
    }
  }

  const columns: DataTableColumn<Doc>[] = [
    {
      name: "uploaded_at",
      header: "Uploaded",
      width: "120px",
      accessor: (d) => d.uploaded_at.slice(0, 10),
    },
    { name: "filename", header: "File" },
    { name: "vendor", header: "Vendor" },
    {
      name: "amount",
      header: "Amount",
      align: "Right",
      accessor: (d) => (d.amount ? parseFloat(d.amount) : 0),
      cell: (_, d) => (d.amount ? money(d.amount, d.currency) : "—"),
    },
    { name: "date", header: "Receipt date", accessor: (d) => d.date ?? "" },
    {
      name: "status",
      header: "Status",
      cell: (v, d) => (
        <Badge variant={statusVariant[d.status]}>
          {d.status === "booked" ? `booked ${d.vouchers.join(", ")}` : String(v)}
        </Badge>
      ),
    },
    {
      name: "source",
      header: "Via",
      cell: (v) => <Badge variant="secondary">{v as string}</Badge>,
    },
  ];
  const actions: DataTableRowAction<Doc>[] = [
    {
      tag: "download",
      label: "Download",
      icon: <Download aria-hidden="true" />,
      tooltip: "Download original",
    },
    {
      tag: "edit",
      label: "Edit details",
      icon: <Pencil aria-hidden="true" />,
      tooltip: "Edit details",
    },
  ];

  return (
    <>
      <PageHeader
        title="Inbox"
        description="Receipts and invoices, kept unchanged for seven years and linked to the vouchers they support."
      />
      <div className="oas-kpis oas-kpis-3">
        <Kpi
          label="Unlinked receipts"
          value={count("unlinked")}
          badge={
            count("unlinked") ? (
              <Badge variant="warning">need booking</Badge>
            ) : (
              <Badge variant="success">all linked</Badge>
            )
          }
        />
        <Kpi label="In a draft" value={count("draft")} />
        <Kpi label="Booked" value={count("booked")} />
      </div>

      <Section
        title="Upload"
        description="Fill in what the receipt says so it can be matched to a bank line. Several files at once are uploaded without details."
      >
        <div className="oas-form">
          <div className="oas-form-row">
            <TextField
              label="Vendor"
              value={vendor}
              onChange={setVendor}
              placeholder="Amazon Web Services"
            />
            <TextField label="Amount" value={amount} onChange={setAmount} placeholder="1 250,00" />
            <TextField label="Currency" value={currency} onChange={setCurrency} />
            <TextField label="Receipt date" type="date" value={date} onChange={setDate} />
          </div>
          <div className="oas-actions">
            <input
              ref={fileRef}
              type="file"
              multiple
              accept="application/pdf,image/*,.txt,.csv"
              hidden
              onChange={(e) => void upload(e.target.files)}
            />
            <Button disabled={busy} onClick={() => fileRef.current?.click()}>
              <Upload /> {busy ? "Uploading..." : "Choose files"}
            </Button>
          </div>
        </div>
      </Section>

      <Section
        title="Documents"
        actions={
          <Select value={status} onValueChange={setStatus}>
            <SelectTrigger style={{ minWidth: 160 }}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All</SelectItem>
              <SelectItem value="unlinked">Unlinked</SelectItem>
              <SelectItem value="draft">In a draft</SelectItem>
              <SelectItem value="booked">Booked</SelectItem>
            </SelectContent>
          </Select>
        }
      >
        {(docs.data?.documents ?? []).length === 0 ? (
          <EmptyNote>No documents here yet.</EmptyNote>
        ) : (
          <DataTable
            columns={prioritize(columns, {
              status: 1,
              date: 2,
              filename: 3,
              uploaded_at: 4,
              source: 5,
            })}
            rows={docs.data?.documents ?? []}
            getRowId={(d) => String(d.id)}
            getRowLabel={(d) => d.vendor || d.filename}
            collapseRowActionsBelow={500}
            rowActions={actions}
            onRowAction={({ tag, row }) => {
              if (tag === "edit") setEditing(row);
              else void download(`/documents/${row.id}/file`, row.filename).catch(notifyError);
            }}
            defaultPageSize={25}
            caption="Receipts and invoices"
          />
        )}
      </Section>
      <EditDialog doc={editing} onClose={() => setEditing(null)} onDone={reload} />
    </>
  );
}
