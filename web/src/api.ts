export class ApiError extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string,
    public projects: string[] = [],
  ) {
    super(message);
  }
}

const KEY = "oas_token";

export const getToken = () => sessionStorage.getItem(KEY);
export const setToken = (t: string | null) =>
  t ? sessionStorage.setItem(KEY, t) : sessionStorage.removeItem(KEY);

export async function api<T = any>(
  path: string,
  init: { method?: string; body?: unknown } = {},
): Promise<T> {
  const token = getToken();
  const res = await fetch("/api" + path, {
    method: init.method ?? "GET",
    headers: {
      "Content-Type": "application/json",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    body: init.body !== undefined ? JSON.stringify(init.body) : undefined,
  });
  if (res.status === 401 && path !== "/login") {
    setToken(null);
    window.dispatchEvent(new Event("oas-unauthorized"));
  }
  const text = await res.text();
  let data: any = {};
  try {
    data = text ? JSON.parse(text) : {};
  } catch {
    data = {};
  }
  if (!res.ok) {
    throw new ApiError(
      res.status,
      data.error?.code ?? "error",
      data.error?.message ?? res.statusText,
      data.error?.projects ?? [],
    );
  }
  return data as T;
}

export async function download(path: string, filename: string) {
  const res = await fetch("/api" + path, { headers: { Authorization: `Bearer ${getToken()}` } });
  if (!res.ok) {
    const data = await res.json().catch(() => ({}));
    throw new ApiError(res.status, data.error?.code ?? "error", data.error?.message ?? "download failed");
  }
  const url = URL.createObjectURL(await res.blob());
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

export interface Me { username: string; role: "admin" | "agent"; can_approve: boolean }
export interface Project { id: number; name: string; aliases: string[]; client: string | null; archived: boolean }
export interface Activity { name: string; message: string }
export interface TimeEntry {
  id: number; date: string; start: string; end: string | null; hours: number; timezone: string;
  project: string; client: string | null; activity: string; message: string; running: boolean;
}
export interface Line { account: number; name: string; debit: string; credit: string }
export interface Draft {
  id: number; event_date: string; description: string; counterparty: string;
  created_by: string; created_at: string; lines: Line[];
}
export interface Voucher {
  id: number; label: string; series: string; number: number; compile_date: string; event_date: string;
  description: string; counterparty: string; created_by: string; approved_by: string;
  corrects: number | null; hash: string; lines: Line[];
}
export interface Account { number: number; name: string; kind: string }
export interface Report {
  entries: number; total_hours: number; by_project: Record<string, number>;
  by_activity: Record<string, number>; by_day: Record<string, number>;
}

export interface Doc {
  id: number; filename: string; mime: string; size: number; uploaded_by: string; uploaded_at: string;
  source: string; vendor: string; amount: string | null; currency: string; date: string | null;
  notes: string; vouchers: string[]; drafts: number[]; status: "unlinked" | "draft" | "booked";
  duplicate?: boolean;
}
export interface BankTx {
  id: number; date: string; amount: string; currency: string; counterparty: string; reference: string;
  status: "new" | "booked" | "ignored"; voucher: string | null; draft: number | null; pending_matches: number;
}
export interface MatchItem {
  id: number; status: string; score: number;
  scores: { name: number; amount: number; date: number; currency: number };
  transaction: BankTx; document: Doc;
}

async function rawUpload<T>(path: string, contentType: string, body: BodyInit): Promise<T> {
  const res = await fetch("/api" + path, {
    method: "POST",
    headers: { "Content-Type": contentType, Authorization: `Bearer ${getToken()}` },
    body,
  });
  if (res.status === 401) {
    setToken(null);
    window.dispatchEvent(new Event("oas-unauthorized"));
  }
  const data = await res.json().catch(() => ({}));
  if (!res.ok) {
    throw new ApiError(res.status, data.error?.code ?? "error", data.error?.message ?? res.statusText);
  }
  return data as T;
}

export function uploadDocument(file: File, meta: Record<string, string> = {}) {
  const q = new URLSearchParams({ filename: file.name, source: "web" });
  for (const [k, v] of Object.entries(meta)) if (v) q.set(k, v);
  return rawUpload<{ document: Doc }>(`/documents?${q}`, file.type || "application/octet-stream", file);
}

export function importBankCsv(file: File) {
  return rawUpload<{ imported: number; duplicates: number; errors: string[]; new_suggestions: number }>(
    "/bank/import", "text/csv", file,
  );
}
