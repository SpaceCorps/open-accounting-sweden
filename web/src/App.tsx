import {
  Button, Card, CardContent, CardDescription, CardHeader, CardTitle, Sidebar, SidebarContent,
  SidebarFooter, SidebarGroup, SidebarGroupContent, SidebarGroupLabel, SidebarHeader,
  SidebarMenu, SidebarMenuButton, SidebarMenuItem, SidebarProvider, SidebarTrigger, Toaster,
} from "@spacecorps/components/ui";
import { ThemeProvider, useTheme } from "@spacecorps/components/theme";
import { BookOpen, Clock, Inbox, Landmark, LayoutDashboard, LogOut, Moon, Percent, Settings, Sun, Users } from "lucide-react";
import { useCallback, useEffect, useState, type FormEvent } from "react";
import { ApiError, api, getToken, setToken, type Me } from "./api";
import { useHashPath } from "./hooks";
import { ErrorNote, TextField } from "./kit";
import { BankPage } from "./pages/Bank";
import { Dashboard } from "./pages/Dashboard";
import { InboxPage } from "./pages/Inbox";
import { Ledger } from "./pages/Ledger";
import { PayrollPage } from "./pages/Payroll";
import { SettingsPage } from "./pages/Settings";
import { VatPage } from "./pages/Vat";
import { TimePage } from "./pages/Time";

const NAV = [
  { path: "/", label: "Dashboard", icon: LayoutDashboard },
  { path: "/time", label: "Time", icon: Clock },
  { path: "/inbox", label: "Inbox", icon: Inbox },
  { path: "/bank", label: "Bank", icon: Landmark },
  { path: "/ledger", label: "Ledger", icon: BookOpen },
  { path: "/vat", label: "VAT", icon: Percent },
  { path: "/payroll", label: "Payroll", icon: Users },
  { path: "/settings", label: "Settings", icon: Settings },
];

function Login({ onDone }: { onDone: () => void }) {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const res = await api<{ token: string }>("/login", {
        method: "POST",
        body: { username, password, ttl_hours: 12 },
      });
      setToken(res.token);
      setPassword("");
      onDone();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Sign in failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="oas-login">
      <Card className="oas-login-card">
        <CardHeader>
          <CardTitle>open-accounting-sweden</CardTitle>
          <CardDescription>Sign in with your username and password.</CardDescription>
        </CardHeader>
        <CardContent>
          <form className="oas-form" onSubmit={submit}>
            <TextField label="Username" value={username} onChange={setUsername} autoComplete="username" />
            <TextField label="Password" type="password" value={password} onChange={setPassword} autoComplete="current-password" />
            <ErrorNote message={error} />
            <Button type="submit" disabled={busy || !username || !password}>
              {busy ? "Signing in..." : "Sign in"}
            </Button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}

function ThemeToggle() {
  const { theme, setTheme } = useTheme();
  const dark = theme === "dark";
  return (
    <Button variant="ghost" size="icon" aria-label="Toggle theme" onClick={() => setTheme(dark ? "light" : "dark")}>
      {dark ? <Sun /> : <Moon />}
    </Button>
  );
}

function Shell({ me, onSignOut }: { me: Me; onSignOut: () => void }) {
  const path = useHashPath();
  const current = NAV.find((n) => n.path === path) ?? NAV[0];
  return (
    <SidebarProvider>
      <Sidebar variant="inset">
        <SidebarHeader>
          <div className="oas-brand">
            <span className="oas-brand-mark" aria-hidden="true" />
            <span className="oas-topbar-title">Accounting</span>
          </div>
        </SidebarHeader>
        <SidebarContent>
          <SidebarGroup>
            <SidebarGroupLabel>Workspace</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu>
                {NAV.map((item) => (
                  <SidebarMenuItem key={item.path}>
                    <SidebarMenuButton asChild isActive={item.path === current.path}>
                      <a href={`#${item.path}`}>
                        <item.icon />
                        <span>{item.label}</span>
                      </a>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                ))}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </SidebarContent>
        <SidebarFooter>
          <div className="oas-muted" style={{ padding: "0 .5rem" }}>
            {me.username} ({me.role})
          </div>
          <Button variant="ghost" className="oas-signout" onClick={onSignOut}>
            <LogOut /> Sign out
          </Button>
        </SidebarFooter>
      </Sidebar>
      {/* The floating island: the sidebar sits directly on the shell background, and the page lives in
          a rounded, bordered panel beside it. The header stays put and the page scrolls inside. */}
      <div className="oas-island-wrap">
        <div className="oas-island">
          <header className="oas-topbar">
            <SidebarTrigger />
            <span className="oas-topbar-title">{current.label}</span>
            <span style={{ marginLeft: "auto" }}><ThemeToggle /></span>
          </header>
          <main className="oas-scroll"><div className="oas-main">
          {current.path === "/" && <Dashboard me={me} />}
          {current.path === "/time" && <TimePage />}
          {current.path === "/inbox" && <InboxPage />}
          {current.path === "/bank" && <BankPage me={me} />}
          {current.path === "/ledger" && <Ledger me={me} />}
          {current.path === "/vat" && <VatPage me={me} />}
          {current.path === "/payroll" && <PayrollPage me={me} />}
          {current.path === "/settings" && <SettingsPage me={me} />}
          </div></main>
        </div>
      </div>
    </SidebarProvider>
  );
}

export function App() {
  const [me, setMe] = useState<Me | null>(null);
  const [checking, setChecking] = useState(Boolean(getToken()));

  const load = useCallback(async () => {
    if (!getToken()) {
      setMe(null);
      setChecking(false);
      return;
    }
    try {
      setMe(await api<Me>("/me"));
    } catch {
      setMe(null);
    } finally {
      setChecking(false);
    }
  }, []);

  useEffect(() => {
    void load();
    const on = () => setMe(null);
    window.addEventListener("oas-unauthorized", on);
    return () => window.removeEventListener("oas-unauthorized", on);
  }, [load]);

  async function signOut() {
    try {
      await api("/logout", { method: "POST" });
    } catch {
      /* the session may already be gone */
    }
    setToken(null);
    setMe(null);
  }

  return (
    <ThemeProvider defaultTheme="system" storageKey="oas-theme">
      {checking ? null : me ? <Shell me={me} onSignOut={signOut} /> : <Login onDone={load} />}
      <Toaster />
    </ThemeProvider>
  );
}
