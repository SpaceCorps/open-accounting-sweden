import {
  Badge,
  Button,
  Checkbox,
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
  TimeCalendar,
  calendarColorForKey,
  getVisibleRange,
  type CalendarView,
  type TimeCalendarEvent,
} from "@spacecorps/components/ui";
import {
  Banknote,
  CalendarDays,
  Download,
  List,
  Pencil,
  Play,
  Plus,
  Square,
  Trash2,
} from "lucide-react";
import { useEffect, useState } from "react";
import {
  ApiError,
  api,
  download,
  type Activity,
  type Me,
  type Project,
  type Report,
  type TimeEntry,
} from "../api";
import { monthStart, today, useData } from "../hooks";
import {
  EmptyNote,
  Field,
  Kpi,
  PageHeader,
  Section,
  TextField,
  notifyError,
  notifyOk,
  ConfirmDialog,
  prioritize,
} from "../kit";
import { count, hours, money, moneyWhole } from "../format";
import { entryColumns } from "./shared";

const LAST_PROJECT_KEY = "oas-last-project";
const VIEW_KEY = "oas-time-view";

/** "09:30" plus minutes, capped at 23:59 so a late slot never wraps to the next day. */
function addMinutes(time: string, minutes: number) {
  const [h, m] = time.split(":").map(Number);
  const total = Math.min(h * 60 + m + minutes, 23 * 60 + 59);
  return `${String(Math.floor(total / 60)).padStart(2, "0")}:${String(total % 60).padStart(2, "0")}`;
}

function readView(): "calendar" | "list" {
  try {
    return localStorage.getItem(VIEW_KEY) === "list" ? "list" : "calendar";
  } catch {
    return "calendar";
  }
}

/** Tracked time as a month or week calendar; days and empty slots open Log time, entries open for editing. */
function EntriesCalendar(props: {
  filter: URLSearchParams;
  reloadKey: number;
  onLog: (start: { date: string; time?: string }) => void;
  onEdit: (entry: TimeEntry) => void;
}) {
  const [date, setDate] = useState(today());
  const [view, setView] = useState<CalendarView>("month");
  const range = getVisibleRange(date, view);
  // A night shift that starts the evening before the first visible day still shows on that day.
  const qs = new URLSearchParams(props.filter);
  qs.set("from", dayBefore(range.from));
  qs.set("to", range.to);
  const data = useData<{ entries: TimeEntry[] }>(`/time?${qs.toString()}`);
  const { reload } = data;
  const { reloadKey } = props;
  useEffect(() => {
    if (reloadKey > 0) reload();
  }, [reloadKey, reload]);
  const events: TimeCalendarEvent<TimeEntry>[] = (data.data?.entries ?? []).map((e) => ({
    id: String(e.id),
    date: e.date,
    start: e.start,
    end: e.end ?? undefined,
    hours: e.running ? undefined : e.hours,
    title: e.project,
    description: e.message ? `${e.activity}: ${e.message}` : e.activity,
    color: calendarColorForKey(e.project),
    running: e.running,
    meta: e,
  }));
  return (
    <TimeCalendar
      events={events}
      date={date}
      view={view}
      onNavigate={(n) => {
        setDate(n.date);
        setView(n.view);
      }}
      onViewChange={setView}
      onDayClick={(d) => props.onLog({ date: d })}
      onSlotClick={(d, time) => props.onLog({ date: d, time })}
      onEventClick={(ev) => ev.meta && props.onEdit(ev.meta)}
      formatHours={hours}
      loading={data.loading}
      timeZone="Europe/Stockholm"
    />
  );
}

function dayBefore(iso: string) {
  const d = new Date(`${iso}T12:00:00Z`);
  d.setUTCDate(d.getUTCDate() - 1);
  return d.toISOString().slice(0, 10);
}

function LogTimeDialog(props: {
  open: boolean;
  mode: "log" | "timer";
  onOpenChange: (o: boolean) => void;
  activities: Activity[];
  projects: Project[];
  /** The server's pick: this user's last project, else the newest one. */
  defaultProject: string | null;
  /** Where the dialog was opened from the calendar: that day, and the clicked time if any. */
  start?: { date: string; time?: string } | null;
  onDone: () => void;
}) {
  const [activity, setActivity] = useState("meeting");
  const [project, setProject] = useState("");
  // Start each new entry on the last project used; the browser's memory covers an older server.
  const { open, defaultProject } = props;
  useEffect(() => {
    if (open) setProject(defaultProject ?? localStorage.getItem(LAST_PROJECT_KEY) ?? "");
  }, [open, defaultProject]);
  const [date, setDate] = useState(today());
  const [from, setFrom] = useState("09:00");
  const [to, setTo] = useState("10:00");
  const [message, setMessage] = useState("");
  const { start } = props;
  useEffect(() => {
    if (!open || !start) return;
    setDate(start.date);
    if (start.time) {
      setFrom(start.time);
      setTo(addMinutes(start.time, 60));
    }
  }, [open, start]);
  const [overnight, setOvernight] = useState(false);
  const [long, setLong] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [suggest, setSuggest] = useState<{ typed: string; options: string[] } | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(projectName: string, create: boolean) {
    setBusy(true);
    setError(null);
    try {
      const body: Record<string, unknown> = {
        activity,
        project: projectName || null,
        message: message || null,
        create_project: create,
      };
      if (props.mode === "log") Object.assign(body, { date, from, to, overnight, long });
      await api(props.mode === "log" ? "/time" : "/time/start", { method: "POST", body });
      notifyOk(
        props.mode === "log" ? "Time logged" : "Timer started",
        `${activity}, ${projectName}`,
      );
      localStorage.setItem(LAST_PROJECT_KEY, projectName);
      setSuggest(null);
      setMessage("");
      props.onOpenChange(false);
      props.onDone();
    } catch (e) {
      if (e instanceof ApiError && e.code === "project_required") {
        setError("Choose a project, or type a new project name.");
      } else if (e instanceof ApiError && e.code === "project_not_found") {
        setSuggest({ typed: projectName, options: e.projects });
      } else {
        setError(e instanceof Error ? e.message : "Failed");
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open={props.open} onOpenChange={props.onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{props.mode === "log" ? "Log time" : "Start timer"}</DialogTitle>
          <DialogDescription>
            {props.mode === "log"
              ? "Add a finished block of work."
              : "The timer runs until you stop it."}{" "}
            Times are in Europe/Stockholm.
          </DialogDescription>
        </DialogHeader>
        <form
          className="oas-dialog-form"
          onSubmit={(e) => {
            e.preventDefault();
            void submit(project, false);
          }}
        >
          <DialogBody>
            <div className="oas-form">
              <div className="oas-form-row">
                <Field label="Activity">
                  <Select value={activity} onValueChange={setActivity}>
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {props.activities.map((a) => (
                        <SelectItem key={a.name} value={a.name}>
                          {a.name}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </Field>
                <Field label="Project" hint="Type a name; unknown names offer to create a project.">
                  <Input
                    list="oas-projects"
                    value={project}
                    onChange={(e) => {
                      setProject(e.target.value);
                      setSuggest(null);
                    }}
                    placeholder="Project"
                  />
                  <datalist id="oas-projects">
                    {props.projects
                      .filter((p) => !p.archived)
                      .map((p) => (
                        <option key={p.id} value={p.name} />
                      ))}
                  </datalist>
                </Field>
              </div>
              {props.mode === "log" ? (
                <div className="oas-form-row">
                  <TextField label="Date" type="date" value={date} onChange={setDate} />
                  <TextField label="From" type="time" value={from} onChange={setFrom} />
                  <TextField label="To" type="time" value={to} onChange={setTo} />
                </div>
              ) : null}
              {props.mode === "log" ? (
                <TimeRangeCheck
                  from={from}
                  to={to}
                  overnight={overnight}
                  onOvernight={setOvernight}
                  long={long}
                  onLong={setLong}
                />
              ) : null}
              <TextField
                label="Message"
                value={message}
                onChange={setMessage}
                placeholder="Optional, defaults to the activity template"
              />
              {suggest ? (
                <div className="oas-banner">
                  <span>No project named "{suggest.typed}".</span>
                  <span className="oas-actions">
                    {suggest.options.map((o) => (
                      <Button
                        type="button"
                        key={o}
                        size="sm"
                        variant="outline"
                        onClick={() => {
                          setProject(o);
                          void submit(o, false);
                        }}
                      >
                        Use {o}
                      </Button>
                    ))}
                    <Button
                      type="button"
                      size="sm"
                      onClick={() => void submit(suggest.typed, true)}
                    >
                      Create "{suggest.typed}"
                    </Button>
                  </span>
                </div>
              ) : null}
              {error ? <p className="oas-error">{error}</p> : null}
            </div>
          </DialogBody>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => props.onOpenChange(false)}>
              Cancel
            </Button>
            <Button disabled={busy || !project.trim()} type="submit">
              {props.mode === "log" ? "Log time" : "Start"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function rateText(p: Project) {
  if (p.rate_type === "hourly" && p.rate) return `${money(p.rate, p.currency)} / hour`;
  if (p.rate_type === "fixed" && p.rate) return `${money(p.rate, p.currency)} fixed`;
  return "";
}

function RateFields(props: {
  type: string;
  amount: string;
  currency: string;
  onType: (v: string) => void;
  onAmount: (v: string) => void;
  onCurrency: (v: string) => void;
}) {
  return (
    <>
      <Field label="Pricing">
        <Select value={props.type} onValueChange={props.onType}>
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="none">No rate</SelectItem>
            <SelectItem value="hourly">Hourly rate</SelectItem>
            <SelectItem value="fixed">Fixed fee</SelectItem>
          </SelectContent>
        </Select>
      </Field>
      {props.type !== "none" ? (
        <div className="oas-form-row">
          <TextField
            label={props.type === "hourly" ? "Rate per hour" : "Fee"}
            value={props.amount}
            onChange={props.onAmount}
            placeholder="900.00"
          />
          <TextField label="Currency" value={props.currency} onChange={props.onCurrency} />
        </div>
      ) : null}
    </>
  );
}

function NewProjectDialog({
  open,
  onOpenChange,
  onDone,
  canSetRate,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  onDone: () => void;
  canSetRate: boolean;
}) {
  const [name, setName] = useState("");
  const [client, setClient] = useState("");
  const [aliases, setAliases] = useState("");
  const [rateType, setRateType] = useState("none");
  const [rate, setRate] = useState("");
  const [currency, setCurrency] = useState("SEK");
  async function save() {
    try {
      await api("/projects", {
        method: "POST",
        body: {
          name,
          client: client || null,
          aliases: aliases
            .split(",")
            .map((a) => a.trim())
            .filter(Boolean),
          rate_type: rateType,
          rate: rateType === "none" ? null : rate,
          currency,
        },
      });
      notifyOk("Project created", name);
      setName("");
      setClient("");
      setAliases("");
      setRateType("none");
      setRate("");
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
          <DialogTitle>New project</DialogTitle>
          <DialogDescription>
            Aliases are other names that should match this project when you log time.
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
              <TextField label="Client" value={client} onChange={setClient} />
              <TextField
                label="Aliases"
                value={aliases}
                onChange={setAliases}
                hint="Comma separated"
              />
              {canSetRate ? (
                <RateFields
                  type={rateType}
                  amount={rate}
                  currency={currency}
                  onType={setRateType}
                  onAmount={setRate}
                  onCurrency={setCurrency}
                />
              ) : null}
            </div>
          </DialogBody>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button disabled={!name.trim() || (rateType !== "none" && !rate.trim())} type="submit">
              Create
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function RateDialog({
  project,
  onClose,
  onDone,
}: {
  project: Project | null;
  onClose: () => void;
  onDone: () => void;
}) {
  const [type, setType] = useState("none");
  const [amount, setAmount] = useState("");
  const [currency, setCurrency] = useState("SEK");
  const [loaded, setLoaded] = useState<number | null>(null);
  if (project && loaded !== project.id) {
    setLoaded(project.id);
    setType(project.rate_type);
    setAmount(project.rate ?? "");
    setCurrency(project.currency);
  }
  async function save() {
    if (!project) return;
    try {
      await api(`/projects/${project.id}/rate`, {
        method: "POST",
        body: { rate_type: type, rate: type === "none" ? null : amount, currency },
      });
      notifyOk("Pricing saved", project.name);
      onClose();
      onDone();
    } catch (e) {
      notifyError(e);
    }
  }
  return (
    <Dialog open={project !== null} onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Pricing for {project?.name}</DialogTitle>
          <DialogDescription>
            An hourly rate multiplies tracked hours. A fixed fee counts once for the project.
            Changing it applies to all entries, including past ones.
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
              <RateFields
                type={type}
                amount={amount}
                currency={currency}
                onType={setType}
                onAmount={setAmount}
                onCurrency={setCurrency}
              />
            </div>
          </DialogBody>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              Cancel
            </Button>
            <Button disabled={type !== "none" && !amount.trim()} type="submit">
              Save
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** Minutes from midnight for "HH:MM", or null when it is not a time. */
function minutesOf(time: string): number | null {
  const m = /^(\d{2}):(\d{2})$/.exec(time);
  return m ? Number(m[1]) * 60 + Number(m[2]) : null;
}

/** Hours from `from` to `to`, running into the next day when `overnight`; null when unreadable. */
function spanHours(from: string, to: string, overnight: boolean): number | null {
  const a = minutesOf(from);
  const b = minutesOf(to);
  if (a === null || b === null) return null;
  const end = b <= a ? (overnight ? b + 24 * 60 : null) : b;
  return end === null ? null : (end - a) / 60;
}

/**
 * The duration of the times being entered, with the two checks the server makes: an end before
 * the start needs "runs past midnight", and an entry over 12 hours needs a confirmation.
 */
function TimeRangeCheck(props: {
  from: string;
  to: string;
  overnight: boolean;
  onOvernight: (v: boolean) => void;
  long: boolean;
  onLong: (v: boolean) => void;
}) {
  const a = minutesOf(props.from);
  const b = minutesOf(props.to);
  if (a === null || b === null) return null;
  const endsEarlier = b <= a;
  const span = spanHours(props.from, props.to, props.overnight);
  return (
    <div className="oas-form">
      {endsEarlier ? (
        <div className="oas-banner" role="status">
          <span>
            Ends at {props.to}, before it starts at {props.from}. Check the times, or mark it as
            running past midnight.
          </span>
          <label className="oas-check">
            <Checkbox
              checked={props.overnight}
              onCheckedChange={(c) => props.onOvernight(c === true)}
            />
            <span>Runs past midnight</span>
          </label>
        </div>
      ) : null}
      {span !== null && span > 12 ? (
        <div className="oas-banner" role="status">
          <span>This entry is {hours(span)}. Entries over 12 hours need a confirmation.</span>
          <label className="oas-check">
            <Checkbox checked={props.long} onCheckedChange={(c) => props.onLong(c === true)} />
            <span>Yes, {hours(span)} is right</span>
          </label>
        </div>
      ) : null}
      {span !== null && span <= 12 ? <p className="oas-muted">{hours(span)}</p> : null}
    </div>
  );
}

function EditEntryDialog({
  entry,
  activities,
  projects,
  onClose,
  onDone,
}: {
  entry: TimeEntry | null;
  activities: Activity[];
  projects: Project[];
  onClose: () => void;
  onDone: () => void;
}) {
  const [date, setDate] = useState("");
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [activity, setActivity] = useState("");
  const [project, setProject] = useState("");
  const [message, setMessage] = useState("");
  const [overnight, setOvernight] = useState(false);
  const [long, setLong] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [loaded, setLoaded] = useState<number | null>(null);
  if (entry && loaded !== entry.id) {
    setLoaded(entry.id);
    setDate(entry.date);
    setFrom(entry.start);
    setTo(entry.end ?? "");
    setActivity(entry.activity);
    setProject(entry.project);
    setMessage(entry.message);
    setOvernight(entry.end !== null && entry.end <= entry.start);
    setLong(entry.hours > 12);
  }
  const close = () => {
    setLoaded(null);
    onClose();
  };

  async function save() {
    if (!entry) return;
    setBusy(true);
    try {
      const body: Record<string, unknown> = {
        date,
        from,
        activity,
        project,
        message,
        overnight,
        long,
      };
      if (!entry.running) body.to = to;
      await api(`/time/${entry.id}`, { method: "PATCH", body });
      localStorage.setItem(LAST_PROJECT_KEY, project);
      notifyOk("Entry saved");
      close();
      onDone();
    } catch (e) {
      notifyError(e);
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (!entry) return;
    setConfirming(false);
    try {
      await api(`/time/${entry.id}`, { method: "DELETE" });
      notifyOk(
        "Entry deleted",
        `${entry.date} ${entry.start}-${entry.end ?? ""}, ${entry.project}`,
      );
      close();
      onDone();
    } catch (e) {
      notifyError(e);
    }
  }

  const span = entry?.running ? null : spanHours(from, to, overnight);
  const blocked = !entry?.running && (span === null || (span > 12 && !long));
  return (
    <>
      <Dialog open={entry !== null && !confirming} onOpenChange={(o) => !o && close()}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Edit entry</DialogTitle>
            <DialogDescription>
              Times are in Europe/Stockholm. Changes are kept in the audit log.
            </DialogDescription>
          </DialogHeader>
          <form
            className="oas-dialog-form"
            onSubmit={(e) => {
              e.preventDefault();
              if (!blocked) void save();
            }}
          >
            <DialogBody>
              <div className="oas-form">
                <div className="oas-form-row">
                  <TextField label="Date" type="date" value={date} onChange={setDate} />
                  <TextField label="From" type="time" value={from} onChange={setFrom} />
                  {entry?.running ? (
                    <Field label="To">
                      <span className="oas-muted">Running</span>
                    </Field>
                  ) : (
                    <TextField label="To" type="time" value={to} onChange={setTo} />
                  )}
                </div>
                {entry?.running ? null : (
                  <TimeRangeCheck
                    from={from}
                    to={to}
                    overnight={overnight}
                    onOvernight={setOvernight}
                    long={long}
                    onLong={setLong}
                  />
                )}
                <div className="oas-form-row">
                  <Field label="Activity">
                    <Select value={activity} onValueChange={setActivity}>
                      <SelectTrigger>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {activities.map((a) => (
                          <SelectItem key={a.name} value={a.name}>
                            {a.name}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </Field>
                  <Field label="Project">
                    <Input
                      list="oas-edit-projects"
                      value={project}
                      onChange={(e) => setProject(e.target.value)}
                    />
                    <datalist id="oas-edit-projects">
                      {projects
                        .filter((p) => !p.archived)
                        .map((p) => (
                          <option key={p.id} value={p.name} />
                        ))}
                    </datalist>
                  </Field>
                </div>
                <TextField label="Message" value={message} onChange={setMessage} />
              </div>
            </DialogBody>
            <DialogFooter>
              <Button
                type="button"
                variant="destructive"
                className="oas-footer-start"
                onClick={() => setConfirming(true)}
              >
                <Trash2 /> Delete
              </Button>
              <Button type="button" variant="outline" onClick={close}>
                Cancel
              </Button>
              <Button type="submit" disabled={busy || blocked || !project.trim()}>
                Save
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
      <ConfirmDialog
        open={confirming}
        title="Delete this entry?"
        description={
          entry
            ? `${entry.date} ${entry.start}-${entry.end ?? "running"}, ${entry.project}, ${hours(entry.hours)}. It disappears from reports and income; the audit log keeps a copy.`
            : ""
        }
        confirmLabel="Delete entry"
        onConfirm={() => void remove()}
        onCancel={() => setConfirming(false)}
      />
    </>
  );
}

function NewActivityDialog({
  open,
  onOpenChange,
  onDone,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  onDone: () => void;
}) {
  const [name, setName] = useState("");
  const [message, setMessage] = useState("");
  async function save() {
    try {
      await api("/activities", { method: "POST", body: { name, message } });
      notifyOk("Activity saved", name);
      setName("");
      setMessage("");
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
          <DialogTitle>Activity type</DialogTitle>
          <DialogDescription>
            The message template pre-fills entries of this type. Saving an existing name updates its
            template.
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
              <TextField
                label="Name"
                value={name}
                onChange={setName}
                placeholder="meeting, building, strategizing..."
              />
              <TextField
                label="Message template"
                value={message}
                onChange={setMessage}
                placeholder="Meeting: {topic}"
              />
            </div>
          </DialogBody>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button disabled={!name.trim()} type="submit">
              Save
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

export function TimePage({ me }: { me: Me }) {
  const [from, setFrom] = useState(monthStart());
  const [to, setTo] = useState(today());
  const [project, setProject] = useState("all");
  const [activity, setActivity] = useState("all");
  const [dialog, setDialog] = useState<"log" | "timer" | null>(null);
  const [logStart, setLogStart] = useState<{ date: string; time?: string } | null>(null);
  const [view, setViewState] = useState(readView);
  const setView = (v: "calendar" | "list") => {
    setViewState(v);
    try {
      localStorage.setItem(VIEW_KEY, v);
    } catch {
      /* the choice just is not remembered */
    }
  };
  const [calendarKey, setCalendarKey] = useState(0);
  const [newProject, setNewProject] = useState(false);
  const [pricing, setPricing] = useState<Project | null>(null);
  const [editing, setEditing] = useState<TimeEntry | null>(null);
  const [newActivity, setNewActivity] = useState(false);

  const qs = new URLSearchParams({ from, to });
  if (project !== "all") qs.set("project", project);
  if (activity !== "all") qs.set("activity", activity);
  const q = qs.toString();
  // The calendar picks its own dates; it keeps the project and activity filters.
  const filterQs = new URLSearchParams(qs);
  filterQs.delete("from");
  filterQs.delete("to");

  const entries = useData<{ entries: TimeEntry[] }>(`/time?${q}`);
  const report = useData<Report>(`/time/report?${q}`);
  const projects = useData<{ projects: Project[]; default_project?: string | null }>("/projects");
  const activities = useData<{ activities: Activity[] }>("/activities");
  const running = (entries.data?.entries ?? []).find((e) => e.running);

  const reloadAll = () => {
    setCalendarKey((k) => k + 1);
    entries.reload();
    report.reload();
    projects.reload();
    activities.reload();
  };

  async function stop() {
    try {
      await api("/time/stop", { method: "POST" });
      notifyOk("Timer stopped");
      reloadAll();
    } catch (e) {
      notifyError(e);
    }
  }

  async function exportCsv() {
    try {
      await download(`/time/export.csv?${q}`, "time-entries.csv");
    } catch (e) {
      notifyError(e);
    }
  }

  const projectColumns: DataTableColumn<Project>[] = [
    { name: "name", header: "Project" },
    { name: "client", header: "Client", accessor: (p) => p.client ?? "" },
    { name: "aliases", header: "Aliases", accessor: (p) => p.aliases.join(", ") },
    {
      name: "rate",
      header: "Pricing",
      accessor: (p) => rateText(p),
      cell: (v) => (v ? String(v) : <span className="oas-muted">none</span>),
    },
    {
      name: "hours",
      header: "Hours (range)",
      align: "Right",
      accessor: (p) => report.data?.by_project[p.name] ?? 0,
      cell: (v) => hours(v as number),
    },
    {
      name: "amount",
      header: "Amount (range)",
      align: "Right",
      accessor: (p) => parseFloat(report.data?.amount_by_project[p.name]?.amount ?? "0"),
      cell: (_, p) => {
        const a = report.data?.amount_by_project[p.name];
        return a ? money(a.amount, a.currency) : "—";
      },
    },
    {
      name: "archived",
      header: "Status",
      accessor: (p) => (p.archived ? "archived" : "active"),
      cell: (v) => <Badge variant={v === "active" ? "success" : "secondary"}>{v as string}</Badge>,
    },
  ];
  const activityColumns: DataTableColumn<Activity>[] = [
    { name: "name", header: "Activity" },
    { name: "message", header: "Message template" },
    {
      name: "hours",
      header: "Hours (range)",
      align: "Right",
      accessor: (a) => report.data?.by_activity[a.name] ?? 0,
      cell: (v) => hours(v as number),
    },
  ];

  return (
    <>
      <PageHeader
        title="Time"
        description="Log hours against projects and activity types."
        actions={
          <>
            <Button variant="outline" onClick={() => void exportCsv()}>
              <Download /> Export CSV
            </Button>
            <Button
              variant="outline"
              onClick={() => setDialog("timer")}
              disabled={Boolean(running)}
            >
              <Play /> Start timer
            </Button>
            <Button onClick={() => setDialog("log")}>
              <Plus /> Log time
            </Button>
          </>
        }
      />

      {running ? (
        <div className="oas-banner">
          <span>
            <Badge variant="info">running</Badge> {running.activity} on {running.project} since{" "}
            {running.start}
          </span>
          <Button size="sm" onClick={() => void stop()}>
            <Square /> Stop timer
          </Button>
        </div>
      ) : null}

      <div className="oas-filters">
        <TextField label="From" type="date" value={from} onChange={setFrom} />
        <TextField label="To" type="date" value={to} onChange={setTo} />
        <Field label="Project">
          <Select value={project} onValueChange={setProject}>
            <SelectTrigger style={{ minWidth: 180 }}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All projects</SelectItem>
              {(projects.data?.projects ?? []).map((p) => (
                <SelectItem key={p.id} value={p.name}>
                  {p.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>
        <Field label="Activity">
          <Select value={activity} onValueChange={setActivity}>
            <SelectTrigger style={{ minWidth: 180 }}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All activities</SelectItem>
              {(activities.data?.activities ?? []).map((a) => (
                <SelectItem key={a.name} value={a.name}>
                  {a.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>
      </div>

      <div className="oas-kpis oas-kpis-4">
        <Kpi
          label="Total hours"
          value={hours(report.data?.total_hours ?? 0)}
          hint={count(report.data?.entries ?? 0, "entry", "entries")}
        />
        <Kpi
          label="Billable amount"
          value={
            Object.entries(report.data?.total_amount ?? {})
              .map(([c, a]) => moneyWhole(a, c))
              .join(" + ") || moneyWhole(0)
          }
          hint="hourly rates and fixed fees, in range"
        />
        <Kpi
          label="Projects"
          value={Object.keys(report.data?.by_project ?? {}).length}
          hint="with time in range"
        />
        <Kpi
          label="Days worked"
          value={Object.keys(report.data?.by_day ?? {}).length}
          hint="with entries in range"
        />
      </div>

      <Tabs defaultValue="entries">
        <TabsList>
          <TabsTrigger value="entries">Entries</TabsTrigger>
          <TabsTrigger value="projects">Projects</TabsTrigger>
          <TabsTrigger value="activities">Activity types</TabsTrigger>
        </TabsList>
        <TabsContent value="entries">
          <Section
            title="Entries"
            actions={
              <div className="oas-actions" role="group" aria-label="View">
                {(["calendar", "list"] as const).map((v) => (
                  <Button
                    key={v}
                    type="button"
                    size="sm"
                    variant={view === v ? "default" : "outline"}
                    aria-pressed={view === v}
                    onClick={() => setView(v)}
                  >
                    {v === "calendar" ? <CalendarDays /> : <List />}
                    {v === "calendar" ? "Calendar" : "List"}
                  </Button>
                ))}
              </div>
            }
          >
            {view === "calendar" ? (
              <EntriesCalendar
                filter={filterQs}
                reloadKey={calendarKey}
                onLog={(start) => {
                  setLogStart(start);
                  setDialog("log");
                }}
                onEdit={setEditing}
              />
            ) : (entries.data?.entries ?? []).length === 0 ? (
              <EmptyNote>No entries in this range.</EmptyNote>
            ) : (
              <DataTable
                columns={entryColumns.map((c) =>
                  c.name === "hours"
                    ? { ...c, footer: hours(report.data?.total_hours ?? 0) }
                    : c.name === "date"
                      ? { ...c, footer: "Total" }
                      : c,
                )}
                rows={(entries.data?.entries ?? []).slice().reverse()}
                getRowId={(e) => String(e.id)}
                defaultPageSize={25}
                rowActions={[
                  {
                    tag: "edit",
                    label: "Edit entry",
                    icon: <Pencil aria-hidden="true" />,
                    tooltip: "Edit entry",
                  },
                ]}
                onRowAction={({ row }) => setEditing(row)}
                caption="Time entries"
              />
            )}
          </Section>
        </TabsContent>
        <TabsContent value="projects">
          <Section
            title="Projects"
            actions={
              <Button size="sm" onClick={() => setNewProject(true)}>
                <Plus /> New project
              </Button>
            }
          >
            {(projects.data?.projects ?? []).length === 0 ? (
              <EmptyNote>
                No projects yet. Create one, or type a new name when logging time.
              </EmptyNote>
            ) : (
              <DataTable
                columns={prioritize(projectColumns, {
                  aliases: 2,
                  client: 3,
                  rate: 4,
                  archived: 5,
                })}
                rows={projects.data?.projects ?? []}
                getRowId={(p) => String(p.id)}
                paginated={false}
                rowActions={
                  me.can_approve
                    ? [
                        {
                          tag: "pricing",
                          label: "Set pricing",
                          icon: <Banknote aria-hidden="true" />,
                          tooltip: "Hourly rate or fixed fee",
                        },
                      ]
                    : []
                }
                onRowAction={({ row }) => setPricing(row)}
              />
            )}
          </Section>
        </TabsContent>
        <TabsContent value="activities">
          <Section
            title="Activity types"
            actions={
              <Button size="sm" onClick={() => setNewActivity(true)}>
                <Plus /> New activity
              </Button>
            }
          >
            <DataTable
              columns={prioritize(activityColumns, { message: 2 })}
              rows={activities.data?.activities ?? []}
              getRowId={(a) => a.name}
              paginated={false}
            />
          </Section>
        </TabsContent>
      </Tabs>

      <LogTimeDialog
        open={dialog !== null}
        mode={dialog ?? "log"}
        onOpenChange={(o) => {
          if (!o) {
            setDialog(null);
            setLogStart(null);
          }
        }}
        start={logStart}
        activities={activities.data?.activities ?? []}
        projects={projects.data?.projects ?? []}
        defaultProject={projects.data?.default_project ?? null}
        onDone={reloadAll}
      />
      <NewProjectDialog
        open={newProject}
        onOpenChange={setNewProject}
        onDone={reloadAll}
        canSetRate={me.can_approve}
      />
      <EditEntryDialog
        entry={editing}
        activities={activities.data?.activities ?? []}
        projects={projects.data?.projects ?? []}
        onClose={() => setEditing(null)}
        onDone={reloadAll}
      />
      <RateDialog project={pricing} onClose={() => setPricing(null)} onDone={reloadAll} />
      <NewActivityDialog open={newActivity} onOpenChange={setNewActivity} onDone={reloadAll} />
    </>
  );
}
