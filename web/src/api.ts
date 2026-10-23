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
