import { getCredential, request } from "./api";
import { getLocale } from "./locale";
import { journeyCopy as text } from "./journeyCopy";
import type { Company } from "./types";

export type JourneyMode = "real" | "simulation";
export type JourneyTheme = "custom" | "bootstrap" | "launch" | "runway";
export interface JourneyDraft {
  mode: JourneyMode;
  name: string;
  description: string;
  theme: JourneyTheme;
  currency: string;
  timezone: string;
  opening_date: string;
  confirmed: boolean;
}
export interface JourneyInput {
  id: string;
  name: string;
  description: string;
  mode: JourneyMode;
  currency: string;
  timezone: string;
  opening_date: string;
}
export interface JourneyAttempt {
  request_id: string;
  input: JourneyInput;
  signature: string;
}
// Retain an unconfirmed opening across dialog close/reopen in this page session.
// Credentials are used only as private in-memory keys, never in prompts or storage.
const unresolvedOpenings = new Map<string, { draft: JourneyDraft; attempt: JourneyAttempt }>();
export function rememberJourney(credential: string, draft: JourneyDraft, attempt: JourneyAttempt) {
  unresolvedOpenings.set(credential, { draft: { ...draft }, attempt });
}
export function recoverJourney(credential: string) { return unresolvedOpenings.get(credential); }
export function forgetJourney(credential: string, requestId: string) {
  if (unresolvedOpenings.get(credential)?.attempt.request_id === requestId) unresolvedOpenings.delete(credential);
}
export function journeyDraft(initialMode: JourneyMode = "simulation", now = new Date()): JourneyDraft {
  let timezone = "UTC";
  try { timezone = Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC"; } catch { /* Explicitly displayed fallback. */ }
  const opening_date = new Intl.DateTimeFormat("sv-SE", { timeZone: timezone, year: "numeric", month: "2-digit", day: "2-digit" }).format(now);
  return { mode: initialMode, name: "", description: "", theme: "custom", currency: getLocale() === "zh-CN" ? "CNY" : "USD", timezone, opening_date, confirmed: false };
}
export function journeyErrors(draft: JourneyDraft): Partial<Record<keyof JourneyDraft, string>> {
  const errors: Partial<Record<keyof JourneyDraft, string>> = {};
  if (!draft.name.trim()) errors.name = text.requiredName;
  if (!/^[A-Z]{3}$/.test(draft.currency.trim())) errors.currency = text.invalidCurrency;
  try { new Intl.DateTimeFormat("en", { timeZone: draft.timezone.trim() }).format(); } catch { errors.timezone = text.invalidTimezone; }
  if (!draft.timezone.trim()) errors.timezone = text.invalidTimezone;
  const date = new Date(`${draft.opening_date}T00:00:00Z`);
  if (!/^\d{4}-\d{2}-\d{2}$/.test(draft.opening_date) || !Number.isFinite(date.valueOf()) || date.toISOString().slice(0, 10) !== draft.opening_date) errors.opening_date = text.invalidDate;
  if (!draft.confirmed) errors.confirmed = text.confirmRequired;
  return errors;
}
export function journeyInput(draft: JourneyDraft, id: string): JourneyInput {
  const intent = draft.mode === "simulation" && draft.theme !== "custom" ? `${text.intentPrefix}${text[draft.theme]}` : "";
  return { id, name: draft.name.trim(), description: [draft.description.trim(), intent].filter(Boolean).join("\n\n"), mode: draft.mode, currency: draft.currency.trim(), timezone: draft.timezone.trim(), opening_date: draft.opening_date };
}
export function journeyAttempt(draft: JourneyDraft, previous?: JourneyAttempt | null): JourneyAttempt {
  const input = journeyInput(draft, previous?.input.id || crypto.randomUUID());
  const signature = JSON.stringify(input);
  if (previous?.signature === signature) return previous;
  return { request_id: crypto.randomUUID(), input, signature };
}
export const journeyApi = {
  create: (attempt: JourneyAttempt, credential = getCredential()) => request<{ company: Company; status: string; replayed: boolean }>("/companies", {
    method: "POST", body: JSON.stringify({ request_id: attempt.request_id, input: attempt.input }),
  }, credential),
};
export function journeyAckCurrent(owner: boolean, sentCredential: string, currentCredential: string, mounted: boolean) {
  return owner && mounted && sentCredential === currentCredential;
}
export function journeyEntryAddress(origin: string) { return new URL("/api/v1/agent-entry", origin).href; }
export function journeyPrompt(company: Company, origin: string): string {
  const simulation = company.mode === "simulation";
  const skill = new URL(`/api/v1/skills/builtin:${simulation ? "simulation-gm" : "customer-interviews"}/document`, origin).href;
  const page = new URL("/", origin); page.searchParams.set("company", company.id); page.hash = "overview";
  const playerGuide = new URL("/api/v1/agent-guide?topic=player-output", origin).href;
  return [text.promptHeading, `${text.promptPlayerGuide}${playerGuide}`, text.promptPlayerStyle, `${text.promptEntry}${journeyEntryAddress(origin)}`, `${text.promptCompany}${company.id}`, `${text.promptName}${company.name}`, `${text.promptMode}${simulation ? text.simulation : text.real}`, `${text.promptSettings}${company.currency} · ${company.timezone} · ${company.opening_date || ""}`, company.description ? `${text.promptDescription}\n${company.description}` : "", text.promptExisting, `${text.promptSkill}${skill}`, simulation ? text.promptSimulation : text.promptReal, text.promptContinue, `${text.promptView}${page.href}`].filter(Boolean).join("\n\n");
}
