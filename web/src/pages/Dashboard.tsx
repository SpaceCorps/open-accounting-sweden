import { Badge, Button, Progress } from "@spacecorps/components/ui";
import { DataTable } from "@spacecorps/components/ui";
import { useData, monthStart, today, weekStart } from "../hooks";
import type { Draft, Me, Report, TimeEntry, Voucher } from "../api";
import { Kpi, PageHeader, Section, hoursText } from "../kit";
import { DraftsTable, VouchersTable, entryColumns } from "./shared";

export function Dashboard({ me }: { me: Me }) {
  const t = today();
  const week = useData<Report>(`/time/report?from=${weekStart()}&to=${t}`);
  const month = useData<Report>(`/time/report?from=${monthStart()}&to=${t}`);
  const entries = useData<{ entries: TimeEntry[] }>(`/time?from=${monthStart()}&to=${t}`);
  const drafts = useData<{ drafts: Draft[] }>("/drafts");
  const vouchers = useData<{ vouchers: Voucher[] }>("/vouchers");
  const verify = useData<{ ok: boolean; vouchers: number; issues: string[] }>("/ledger/verify");

  const refreshLedger = () => {
    drafts.reload();
    vouchers.reload();
    verify.reload();
  };

  const projects = Object.entries(month.data?.by_project ?? {}).sort((a, b) => b[1] - a[1]);
  const maxHours = Math.max(1, ...projects.map(([, h]) => h));
  const pending = drafts.data?.drafts ?? [];
  const recent = (entries.data?.entries ?? []).slice().reverse().slice(0, 8);
  const posted = (vouchers.data?.vouchers ?? []).slice().reverse().slice(0, 6);

  return (
    <>
      <PageHeader
        title="Dashboard"
        description={`Signed in as ${me.username}. ${me.can_approve ? "You can approve postings." : "This account can propose but not approve."}`}
        actions={
          <>
            <Button variant="outline" asChild><a href="#/time">Log time</a></Button>
            <Button asChild><a href="#/ledger">Open ledger</a></Button>
          </>
        }
      />

      <div className="oas-kpis">
        <Kpi label="Hours this week" value={hoursText(week.data?.total_hours ?? 0)} hint={`${week.data?.entries ?? 0} entries`} />
        <Kpi label="Hours this month" value={hoursText(month.data?.total_hours ?? 0)} hint={`${projects.length} projects`} />
        <Kpi label="Drafts to approve" value={pending.length} badge={pending.length ? <Badge variant="warning">needs review</Badge> : <Badge variant="success">all clear</Badge>} />
        <Kpi
          label="Ledger"
          value={`${verify.data?.vouchers ?? 0} vouchers`}
          badge={verify.data ? <Badge variant={verify.data.ok ? "success" : "destructive"}>{verify.data.ok ? "verified" : "problems found"}</Badge> : null}
          hint={verify.data && !verify.data.ok ? verify.data.issues[0] : undefined}
        />
      </div>

      <div className="oas-grid-2">
        <Section title="Hours by project" description="This month">
          {projects.length === 0 ? (
            <p className="oas-muted">No time logged this month.</p>
          ) : (
            <div className="oas-bars">
              {projects.map(([name, h]) => (
                <div className="oas-bar-row" key={name}>
                  <span>{name}</span>
                  <Progress value={(h / maxHours) * 100} />
                  <span>{h.toFixed(2)}</span>
                </div>
              ))}
            </div>
          )}
        </Section>
        <Section title="Hours by activity" description="This month">
          {Object.keys(month.data?.by_activity ?? {}).length === 0 ? (
            <p className="oas-muted">No time logged this month.</p>
          ) : (
            <div className="oas-bars">
              {Object.entries(month.data?.by_activity ?? {}).sort((a, b) => b[1] - a[1]).map(([name, h]) => (
                <div className="oas-bar-row" key={name}>
                  <span>{name}</span>
                  <Progress value={(h / Math.max(1, month.data?.total_hours ?? 1)) * 100} />
                  <span>{h.toFixed(2)}</span>
                </div>
              ))}
            </div>
          )}
        </Section>
      </div>

      <Section title="Drafts waiting for approval" description="Proposed by you or an agent. Only an admin can approve." actions={<Button variant="outline" size="sm" asChild><a href="#/ledger">All drafts</a></Button>}>
        {pending.length === 0 ? <p className="oas-muted">Nothing to approve.</p> : <DraftsTable drafts={pending} canApprove={me.can_approve} onChanged={refreshLedger} />}
      </Section>

      <div className="oas-grid-2">
        <Section title="Recent time entries" actions={<Button variant="outline" size="sm" asChild><a href="#/time">All time</a></Button>}>
          {recent.length === 0 ? <p className="oas-muted">No entries this month.</p> : <DataTable columns={entryColumns} rows={recent} getRowId={(e) => String(e.id)} paginated={false} />}
        </Section>
        <Section title="Recent vouchers" actions={<Button variant="outline" size="sm" asChild><a href="#/ledger">Ledger</a></Button>}>
          {posted.length === 0 ? <p className="oas-muted">No vouchers posted yet.</p> : <VouchersTable vouchers={posted} canCorrect={me.can_approve} onChanged={refreshLedger} pageSize={6} />}
        </Section>
      </div>
    </>
  );
}
