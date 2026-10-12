import { words } from "./labels";
import { copy } from "./copy";
import { getLocale } from "./locale";
import type { Json, RecordEntry } from "./types";
export const field = (record: RecordEntry, key: string) => record.data[key];
let companyTimezone = "UTC";
export function setFormatTimezone(timezone: string) {
  companyTimezone = timezone;
}
export const str = (value: Json | undefined, fallback = ""): string =>
  typeof value === "string"
    ? value
    : typeof value === "number"
      ? String(value)
      : fallback;
export const num = (value: Json | undefined): number | null =>
  typeof value === "number" && Number.isFinite(value) ? value : null;
export const obj = (value: Json | undefined): Record<string, Json> =>
  value && typeof value === "object" && !Array.isArray(value) ? value : {};
export const arr = (value: Json | undefined): Json[] =>
  Array.isArray(value) ? value : [];
export const title = (record: RecordEntry) =>
  str(record.data.title || record.data.name, record.id);
export const money = (
  minor: number | null | undefined,
  currency: string,
  digits?: number,
): string => {
  if (minor == null) return words.format062;
  if (!Number.isSafeInteger(minor)) return copy.precisionExceeded;
  const decimals =
    new Intl.NumberFormat(getLocale(), {
      style: "currency",
      currency,
    }).resolvedOptions().maximumFractionDigits ?? 2;
  return new Intl.NumberFormat(getLocale(), {
    style: "currency",
    currency,
    minimumFractionDigits: digits ?? decimals,
    maximumFractionDigits: digits ?? decimals,
  }).format(minor / 10 ** decimals);
};
export const date = (value: string | undefined, timezone?: string): string => {
  if (!value) return words.format063;
  const day = /^\d{4}-\d{2}-\d{2}$/.test(value);
  const parsed = new Date(day ? `${value}T12:00:00Z` : value);
  if (!Number.isFinite(parsed.getTime())) return value;
  return new Intl.DateTimeFormat(getLocale(), {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    timeZone: day ? "UTC" : timezone || companyTimezone,
  }).format(parsed);
};
export const number = (value: number, options: Intl.NumberFormatOptions = {}) =>
  new Intl.NumberFormat(getLocale(), options).format(value);
export const percent = (value: number, digits = 1) =>
  number(value, { style: "percent", maximumFractionDigits: digits });
export const month = (value: string, year = false) =>
  /^\d{4}-\d{2}$/.test(value)
    ? new Intl.DateTimeFormat(getLocale(), {
        month: "short",
        year: year ? "numeric" : undefined,
        timeZone: "UTC",
      }).format(new Date(`${value}-15T12:00:00Z`))
    : value;
export const size = (bytes: number | null) =>
  bytes == null
    ? ""
    : bytes < 1024
      ? `${number(bytes)} B`
      : bytes < 1048576
        ? `${number(bytes / 1024, { minimumFractionDigits: 1, maximumFractionDigits: 1 })} KB`
        : `${number(bytes / 1048576, { minimumFractionDigits: 1, maximumFractionDigits: 1 })} MB`;
export const sourceText = (record: RecordEntry) => {
  const s = obj(record.data.source);
  return str(s.reference || s.kind) || str(record.data.source);
};
