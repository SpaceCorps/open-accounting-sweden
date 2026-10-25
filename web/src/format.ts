// Display formatting. Numbers follow Swedish conventions (space between thousands, decimal comma)
// with non-breaking spaces, so an amount never wraps across lines. Dates stay ISO (yyyy-mm-dd).

const twoDecimals = new Intl.NumberFormat("sv-SE", {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});
const hourDecimals = new Intl.NumberFormat("sv-SE", {
  minimumFractionDigits: 1,
  maximumFractionDigits: 2,
});
const wholeNumber = new Intl.NumberFormat("sv-SE", { maximumFractionDigits: 0 });
const NBSP = "\u00a0";
const nowrap = (s: string) => s.replace(/\s/g, NBSP);

const toNumber = (v: string | number) => (typeof v === "number" ? v : parseFloat(v));

/** An amount without its currency, for columns whose header names the currency. */
export function amount(v: string | number | null | undefined): string {
  if (v === null || v === undefined || v === "") return "";
  const n = toNumber(v);
  return Number.isFinite(n) ? nowrap(twoDecimals.format(n)) : String(v);
}

/** An amount with its currency: "150 200,00 kr", or "1 250,00 EUR" for other currencies. */
export function money(v: string | number | null | undefined, currency = "SEK"): string {
  const a = amount(v);
  if (!a) return "";
  return `${a}${NBSP}${currency === "SEK" ? "kr" : currency}`;
}

/** A rounded amount for headline figures: "150 200 kr". */
export function moneyWhole(v: string | number | null | undefined, currency = "SEK"): string {
  if (v === null || v === undefined || v === "") return "";
  const n = toNumber(v);
  if (!Number.isFinite(n)) return String(v);
  const a = nowrap(wholeNumber.format(n));
  return `${a}${NBSP}${currency === "SEK" ? "kr" : currency}`;
}

export function hours(h: number): string {
  return `${nowrap(hourDecimals.format(h))}${NBSP}h`;
}

/** "1 entry", "3 entries". */
export function count(n: number, singular: string, plural = `${singular}s`): string {
  return `${n} ${n === 1 ? singular : plural}`;
}

/** A UTC timestamp as local "yyyy-mm-dd hh:mm". */
export function dateTime(iso: string | null | undefined): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}
