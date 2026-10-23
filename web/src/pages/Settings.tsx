import {
  Badge, Button, DataTable, Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader,
  DialogTitle, Select, SelectContent, SelectItem, SelectTrigger, SelectValue, type DataTableColumn,
} from "@spacecorps/components/ui";
import { KeyRound, Plus, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { api, type Me } from "../api";
import { useData } from "../hooks";
import { ConfirmDialog, EmptyNote, Field, PageHeader, Section, TextField, notifyError, notifyOk } from "../kit";

interface User { username: string; role: string; created_at: string }
interface Token { id: number; name: string; created_at: string; last_used_at: string | null }

function CompanySection({ canEdit }: { canEdit: boolean }) {
  const company = useData<{ company: { name: string; org_number: string } | null }>("/company");
  const [name, setName] = useState("");
  const [org, setOrg] = useState("");
  useEffect(() => {
    if (company.data?.company) {
      setName(company.data.company.name);
      setOrg(company.data.company.org_number);
    }
  }, [company.data]);
  async function save() {
    try {
      await api("/company", { method: "POST", body: { name, org_number: org } });
      notifyOk("Company saved");
      company.reload();
    } catch (e) {
      notifyError(e);
    }
  }
  return (
    <Section title="Company" description="Used in the SIE export header.">
      <div className="oas-form" style={{ maxWidth: 420 }}>
        <TextField label="Company name" value={name} onChange={setName} />
        <TextField label="Organisation number" value={org} onChange={setOrg} placeholder="559000-0000" />
        <div><Button disabled={!canEdit || !name.trim()} onClick={() => void save()}>Save</Button></div>
        {!canEdit ? <p className="oas-muted">Only an admin can change company details.</p> : null}
      </div>
    </Section>
  );
}

function PasswordSection() {
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [again, setAgain] = useState("");
  async function save() {
    if (next !== again) return notifyError(new Error("The new passwords do not match."));
    try {
      await api("/me/password", { method: "POST", body: { current, new: next } });
      notifyOk("Password changed", "Your other sessions were signed out.");
      setCurrent(""); setNext(""); setAgain("");
    } catch (e) {
      notifyError(e);
    }
  }
  return (
    <Section title="Password" description="At least 12 characters.">
      <div className="oas-form" style={{ maxWidth: 420 }}>
        <TextField label="Current password" type="password" value={current} onChange={setCurrent} autoComplete="current-password" />
        <TextField label="New password" type="password" value={next} onChange={setNext} autoComplete="new-password" />
        <TextField label="Repeat new password" type="password" value={again} onChange={setAgain} autoComplete="new-password" />
        <div><Button disabled={!current || next.length < 12} onClick={() => void save()}>Change password</Button></div>
      </div>
    </Section>
  );
}

function TokensSection() {
  const tokens = useData<{ tokens: Token[] }>("/tokens");
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [created, setCreated] = useState<string | null>(null);
  const [revoking, setRevoking] = useState<Token | null>(null);

  async function create() {
    try {
      const res = await api<{ token: string }>("/tokens", { method: "POST", body: { name } });
      setCreated(res.token);
      setName("");
      tokens.reload();
    } catch (e) {
      notifyError(e);
    }
  }
  async function revoke(t: Token) {
    setRevoking(null);
    try {
      await api(`/tokens/${t.id}/revoke`, { method: "POST" });
      notifyOk("Token revoked", t.name);
      tokens.reload();
    } catch (e) {
      notifyError(e);
    }
  }
  const columns: DataTableColumn<Token>[] = [
    { name: "name", header: "Name" },
    { name: "created_at", header: "Created" },
    { name: "last_used_at", header: "Last used", accessor: (t) => t.last_used_at ?? "never" },
  ];
  return (
    <Section title="Access tokens" description="Permanent tokens for the CLI and agents. They act with your role." actions={<Button size="sm" onClick={() => { setCreated(null); setOpen(true); }}><Plus /> New token</Button>}>
      {tokens.error ? <EmptyNote>Sign in with a username and password to manage access tokens.</EmptyNote> : (tokens.data?.tokens ?? []).length === 0 ? <EmptyNote>No tokens yet.</EmptyNote> : (
        <DataTable
          columns={columns}
          rows={tokens.data?.tokens ?? []}
          getRowId={(t) => String(t.id)}
          paginated={false}
          rowActions={[{ tag: "revoke", label: "Revoke", icon: <Trash2 aria-hidden="true" />, tooltip: "Revoke token", variant: "destructive" }]}
          onRowAction={({ row }) => setRevoking(row)}
        />
      )}
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{created ? "Copy your token now" : "New access token"}</DialogTitle>
            <DialogDescription>{created ? "It is shown only once and stored only as a hash." : "Name it after where it will be used, for example laptop."}</DialogDescription>
          </DialogHeader>
          {created ? <code className="oas-code">{created}</code> : <TextField label="Name" value={name} onChange={setName} />}
          <DialogFooter>
            {created ? <Button onClick={() => { void navigator.clipboard.writeText(created); notifyOk("Copied"); }}><KeyRound /> Copy</Button> : <Button disabled={!name.trim()} onClick={() => void create()}>Create</Button>}
            <Button variant="outline" onClick={() => setOpen(false)}>{created ? "Done" : "Cancel"}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <ConfirmDialog open={revoking !== null} title="Revoke this token?" description={revoking ? `Anything using "${revoking.name}" stops working immediately.` : ""} confirmLabel="Revoke" onConfirm={() => revoking && void revoke(revoking)} onCancel={() => setRevoking(null)} />
    </Section>
  );
}

function UsersSection() {
  const users = useData<{ users: User[] }>("/users");
  const [open, setOpen] = useState(false);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [role, setRole] = useState("agent");
  async function save() {
    try {
      await api("/users", { method: "POST", body: { username, password, role } });
      notifyOk("User added", username);
      setUsername(""); setPassword("");
      setOpen(false);
      users.reload();
    } catch (e) {
      notifyError(e);
    }
  }
  const columns: DataTableColumn<User>[] = [
    { name: "username", header: "Username" },
    { name: "role", header: "Role", cell: (v) => <Badge variant={v === "admin" ? "primary" : "secondary"}>{v as string}</Badge> },
    { name: "created_at", header: "Created" },
  ];
  return (
    <Section title="Users" description="Admins can approve postings. Agents can propose and read." actions={<Button size="sm" onClick={() => setOpen(true)}><Plus /> Add user</Button>}>
      <DataTable columns={columns} rows={users.data?.users ?? []} getRowId={(u) => u.username} paginated={false} />
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <DialogHeader><DialogTitle>Add user</DialogTitle><DialogDescription>Create an agent user for each agent so it can get its own permanent token.</DialogDescription></DialogHeader>
          <div className="oas-form">
            <TextField label="Username" value={username} onChange={setUsername} autoComplete="off" />
            <TextField label="Password" type="password" value={password} onChange={setPassword} autoComplete="new-password" hint="At least 12 characters" />
            <Field label="Role">
              <Select value={role} onValueChange={setRole}>
                <SelectTrigger><SelectValue /></SelectTrigger>
                <SelectContent><SelectItem value="admin">admin</SelectItem><SelectItem value="agent">agent</SelectItem></SelectContent>
              </Select>
            </Field>
          </div>
          <DialogFooter><Button variant="outline" onClick={() => setOpen(false)}>Cancel</Button><Button disabled={username.length < 3 || password.length < 12} onClick={() => void save()}>Add user</Button></DialogFooter>
        </DialogContent>
      </Dialog>
    </Section>
  );
}

export function SettingsPage({ me }: { me: Me }) {
  return (
    <>
      <PageHeader title="Settings" description="Company details, users, access tokens and your password." />
      <CompanySection canEdit={me.can_approve} />
      {me.can_approve ? <UsersSection /> : null}
      <TokensSection />
      <PasswordSection />
    </>
  );
}
