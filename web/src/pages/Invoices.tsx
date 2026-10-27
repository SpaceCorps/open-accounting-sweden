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
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
  type DataTableColumn,
  type DataTableRowAction,
} from "@spacecorps/components/ui";
import { Check, Eye, FileText, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { api, openInvoicePage, type Customer, type Invoice, type Me, type Project } from "../api";
import { useData } from "../hooks";
import {
  ConfirmDialog,
  EmptyNote,
  Field,
  Kpi,
  PageHeader,
  Section,
  TextField,
  notifyError,
  notifyOk,
  prioritize,
} from "../kit";
import { money } from "../format";

function NewCustomer({
  open,
  onOpenChange,
  onDone,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  onDone: () => void;
}) {
  const [name, setName] = useState("");
  const [address, setAddress] = useState("");
  const [org, setOrg] = useState("");
  const [email, setEmail] = useState("");
  async function save() {
    try {
      await api("/customers", {
        method: "POST",
        body: { name, address: address.replace(/, */g, "\n"), org_number: org, email },
      });
      notifyOk("Customer added", name);
      setName("");
      setAddress("");
      setOrg("");
      setEmail("");
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
          <DialogTitle>New customer</DialogTitle>
          <DialogDescription>
            Customers in Sweden only for now. The address is required to issue an invoice.
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
              <TextField label="Name" value={name} onChange={setName} />
              <TextField
                label="Address"
                value={address}
                onChange={setAddress}
                hint="Street, postcode and city, separated by commas"
              />
              <TextField label="Organisation number" value={org} onChange={setOrg} />
              <TextField label="Email" value={email} onChange={setEmail} />
            </div>
          </DialogBody>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button disabled={!name.trim()} type="submit">
              Add
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function NewInvoice({
  open,
  onOpenChange,
  customers,
  projects,
  onDone,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  customers: Customer[];
  projects: Project[];
  onDone: () => void;
}) {
  const [mode, setMode] = useState("time");
  const [customer, setCustomer] = useState("");
  const [project, setProject] = useState("");
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [vat, setVat] = useState("25");
  const [dueDays, setDueDays] = useState("30");
  const [description, setDescription] = useState("");
  const [quantity, setQuantity] = useState("1");
  const [unitPrice, setUnitPrice] = useState("");
  const priced = projects.filter((p) => !p.archived && p.rate_type !== "none");

  async function save() {
    try {
      const common = {
        customer_id: Number(customer),
        vat_rate: Number(vat),
        due_days: Number(dueDays),
      };
      if (mode === "time") {
        await api("/invoices/from-time", {
          method: "POST",
          body: { ...common, project, from: from || null, to: to || null },
        });
      } else {
        await api("/invoices", {
          method: "POST",
          body: {
            ...common,
            lines: [{ description, quantity, unit: "st", unit_price: unitPrice }],
          },
        });
      }
      notifyOk("Draft invoice created", "Review it, then issue it to give it a number.");
      onOpenChange(false);
      onDone();
    } catch (e) {
      notifyError(e);
    }
  }

  const ready = customer && (mode === "time" ? project : description.trim() && unitPrice.trim());
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>New invoice</DialogTitle>
          <DialogDescription>
            It stays a draft until you issue it. Tracked hours already on an invoice are never
            billed twice.
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
              <Field label="Customer">
                <Select value={customer} onValueChange={setCustomer}>
                  <SelectTrigger>
                    <SelectValue placeholder="Choose a customer" />
                  </SelectTrigger>
                  <SelectContent>
                    {customers.map((c) => (
                      <SelectItem key={c.id} value={String(c.id)}>
                        {c.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>
              <Tabs value={mode} onValueChange={setMode}>
                <TabsList>
                  <TabsTrigger value="time">From tracked time</TabsTrigger>
                  <TabsTrigger value="manual">One line</TabsTrigger>
                </TabsList>
                <TabsContent value="time">
                  <div className="oas-form" style={{ marginTop: ".75rem" }}>
                    <Field label="Project" hint="Only projects with an hourly rate or fixed fee">
                      <Select value={project} onValueChange={setProject}>
                        <SelectTrigger>
                          <SelectValue placeholder="Choose a project" />
                        </SelectTrigger>
                        <SelectContent>
                          {priced.map((p) => (
                            <SelectItem key={p.id} value={p.name}>
                              {p.name}, {money(p.rate, p.currency)}
                              {p.rate_type === "hourly" ? " / h" : " fixed"}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </Field>
                    <div className="oas-form-row">
                      <TextField label="From" type="date" value={from} onChange={setFrom} />
                      <TextField label="To" type="date" value={to} onChange={setTo} />
                    </div>
                  </div>
                </TabsContent>
                <TabsContent value="manual">
                  <div className="oas-form" style={{ marginTop: ".75rem" }}>
                    <TextField label="Description" value={description} onChange={setDescription} />
                    <div className="oas-form-row">
                      <TextField label="Quantity" value={quantity} onChange={setQuantity} />
                      <TextField
                        label="Unit price (SEK, excl. VAT)"
                        value={unitPrice}
                        onChange={setUnitPrice}
                      />
                    </div>
                  </div>
                </TabsContent>
              </Tabs>
              <div className="oas-form-row">
                <Field label="VAT">
                  <Select value={vat} onValueChange={setVat}>
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="25">25%</SelectItem>
                      <SelectItem value="12">12%</SelectItem>
                      <SelectItem value="6">6%</SelectItem>
                    </SelectContent>
                  </Select>
                </Field>
                <TextField label="Days until due" value={dueDays} onChange={setDueDays} />
              </div>
            </div>
          </DialogBody>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button disabled={!ready} type="submit">
              Create draft
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function ViewInvoice({ invoice, onClose }: { invoice: Invoice | null; onClose: () => void }) {
  const cols: DataTableColumn<Invoice["lines"][number]>[] = [
    { name: "description", header: "Description", wrapText: true },
    { name: "quantity", header: "Qty", align: "Right", accessor: (l) => `${l.quantity} ${l.unit}` },
    { name: "unit_price", header: "Unit price", align: "Right" },
    { name: "net", header: "Amount", align: "Right" },
  ];
  return (
    <Dialog open={invoice !== null} onOpenChange={(o) => !o && onClose()}>
      <DialogContent style={{ maxWidth: 760 }}>
        {invoice ? (
          <>
            <DialogHeader>
              <DialogTitle>
                {invoice.label ?? "Draft invoice"} for {invoice.customer.name}
              </DialogTitle>
              <DialogDescription>
                Issued {invoice.issue_date}, due {invoice.due_date}. VAT {invoice.vat_rate}%:{" "}
                {money(invoice.totals.vat, invoice.currency)}. Total{" "}
                {money(invoice.totals.gross, invoice.currency)}.
              </DialogDescription>
            </DialogHeader>
            <DialogBody>
              <DataTable
                columns={cols}
                rows={invoice.lines}
                getRowId={(_, i) => String(i)}
                paginated={false}
              />
            </DialogBody>
            <DialogFooter>
              <Button
                variant="outline"
                onClick={() => void openInvoicePage(invoice.id).catch(notifyError)}
              >
                <FileText /> Open printable invoice
              </Button>
            </DialogFooter>
          </>
        ) : null}
      </DialogContent>
    </Dialog>
  );
}

export function InvoicesPage({ me }: { me: Me }) {
  const invoices = useData<{ invoices: Invoice[] }>("/invoices");
  const customers = useData<{ customers: Customer[] }>("/customers");
  const projects = useData<{ projects: Project[] }>("/projects");
  const [newInvoice, setNewInvoice] = useState(false);
  const [newCustomer, setNewCustomer] = useState(false);
  const [viewing, setViewing] = useState<Invoice | null>(null);
  const [confirm, setConfirm] = useState<{ inv: Invoice; what: "issue" | "delete" } | null>(null);

  const reload = () => {
    invoices.reload();
    customers.reload();
    projects.reload();
  };
  const list = invoices.data?.invoices ?? [];

  async function act(inv: Invoice, what: "issue" | "delete") {
    setConfirm(null);
    try {
      const res = await api<{ invoice: Invoice }>(`/invoices/${inv.id}/${what}`, {
        method: "POST",
      });
      if (what === "issue")
        notifyOk(
          `Issued ${res.invoice.label}`,
          "A draft voucher was proposed in the ledger. Approve it there.",
        );
      else notifyOk("Draft deleted");
      reload();
    } catch (e) {
      notifyError(e);
    }
  }

  const columns: DataTableColumn<Invoice>[] = [
    {
      name: "label",
      header: "No",
      width: "90px",
      accessor: (i) => i.number ?? 1e9,
      // A draft has no number until it is issued; the status column already says draft.
      cell: (_, i) => i.label ?? <span className="oas-muted">—</span>,
    },
    { name: "customer", header: "Customer", accessor: (i) => i.customer.name },
    { name: "issue_date", header: "Date", width: "120px" },
    { name: "due_date", header: "Due", width: "120px" },
    {
      name: "net",
      header: "Net",
      help: "Amount excluding VAT",
      align: "Right",
      accessor: (i) => parseFloat(i.totals.net),
      cell: (_, i) => money(i.totals.net, i.currency),
    },
    {
      name: "status",
      header: "Status",
      cell: (v) => <Badge variant={v === "issued" ? "success" : "info"}>{v as string}</Badge>,
    },
  ];
  const actions = (i: Invoice): DataTableRowAction<Invoice>[] => [
    { tag: "view", label: "View", icon: <Eye aria-hidden="true" />, tooltip: "View lines" },
    ...(i.status === "draft" && me.can_approve
      ? [
          {
            tag: "issue",
            label: "Issue",
            icon: <Check aria-hidden="true" />,
            tooltip: "Number it, lock it and propose the ledger voucher",
          },
          {
            tag: "delete",
            label: "Delete",
            icon: <Trash2 aria-hidden="true" />,
            tooltip: "Delete draft",
            variant: "destructive" as const,
          },
        ]
      : []),
  ];
  const custColumns: DataTableColumn<Customer>[] = [
    { name: "name", header: "Name" },
    { name: "org_number", header: "Org no" },
    {
      name: "address",
      header: "Address",
      accessor: (c) => c.address.replace(/\n/g, ", "),
      wrapText: true,
    },
    { name: "email", header: "Email" },
  ];

  return (
    <>
      <PageHeader
        title="Invoices"
        description="Bill tracked hours or a single line. Issuing numbers the invoice and proposes the ledger voucher."
        actions={
          <>
            <Button variant="outline" onClick={() => setNewCustomer(true)}>
              <Plus /> Customer
            </Button>
            <Button onClick={() => setNewInvoice(true)}>
              <Plus /> New invoice
            </Button>
          </>
        }
      />
      <div className="oas-kpis oas-kpis-3">
        <Kpi label="Drafts" value={list.filter((i) => i.status === "draft").length} />
        <Kpi label="Issued" value={list.filter((i) => i.status === "issued").length} />
        <Kpi label="Customers" value={customers.data?.customers.length ?? 0} />
      </div>
      <Tabs defaultValue="invoices">
        <TabsList>
          <TabsTrigger value="invoices">Invoices</TabsTrigger>
          <TabsTrigger value="customers">Customers</TabsTrigger>
        </TabsList>
        <TabsContent value="invoices">
          <Section
            title="Invoices"
            description="Issuing needs your company address (Settings) and the customer's address."
          >
            {list.length === 0 ? (
              <EmptyNote>
                No invoices yet. Add a customer, then create one from tracked time.
              </EmptyNote>
            ) : (
              <DataTable
                columns={prioritize(columns, { status: 1, label: 2, issue_date: 3, due_date: 4 })}
                rows={list}
                getRowId={(i) => String(i.id)}
                getRowLabel={(i) => `${i.label ?? "Draft"} ${i.customer.name}`}
                rowActions={actions}
                onRowAction={({ tag, row }) =>
                  tag === "view"
                    ? setViewing(row)
                    : setConfirm({ inv: row, what: tag as "issue" | "delete" })
                }
                onRowClick={(row) => setViewing(row)}
                defaultSort={{ column: "label", direction: "Descending" }}
                defaultPageSize={25}
                caption="Invoices"
              />
            )}
          </Section>
        </TabsContent>
        <TabsContent value="customers">
          <Section title="Customers">
            {(customers.data?.customers ?? []).length === 0 ? (
              <EmptyNote>No customers yet.</EmptyNote>
            ) : (
              <DataTable
                columns={prioritize(custColumns, { email: 2, org_number: 3, address: 4 })}
                rows={customers.data?.customers ?? []}
                getRowId={(c) => String(c.id)}
                getRowLabel={(c) => c.name}
                paginated={false}
              />
            )}
          </Section>
        </TabsContent>
      </Tabs>
      <NewCustomer open={newCustomer} onOpenChange={setNewCustomer} onDone={reload} />
      <NewInvoice
        open={newInvoice}
        onOpenChange={setNewInvoice}
        customers={customers.data?.customers ?? []}
        projects={projects.data?.projects ?? []}
        onDone={reload}
      />
      <ViewInvoice invoice={viewing} onClose={() => setViewing(null)} />
      <ConfirmDialog
        open={confirm !== null}
        title={confirm?.what === "issue" ? "Issue this invoice?" : "Delete this draft?"}
        description={
          confirm?.what === "issue"
            ? "It gets the next invoice number and can no longer be changed or deleted. A draft voucher is proposed in the ledger."
            : "The draft is removed and its tracked hours can be invoiced again."
        }
        confirmLabel={confirm?.what === "issue" ? "Issue" : "Delete"}
        onConfirm={() => confirm && void act(confirm.inv, confirm.what)}
        onCancel={() => setConfirm(null)}
      />
    </>
  );
}
