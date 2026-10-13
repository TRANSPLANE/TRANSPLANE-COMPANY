import { useCallback, useEffect, useRef, useState, type FormEvent } from "react";
import { getCredential, request } from "./api";
import { ErrorNotice, Loading } from "./components";
import { money, number } from "./format";
import { ArrowRight, CheckCircle2, Flag, XCircle } from "./icons";
import { getLocale, useLocale } from "./locale";
import { studioCopy as text, studioWeek } from "./studioCopy";
import type { Company, Session, Snapshot } from "./types";
import "./studio-game.css";

export interface StudioAllocation { target: string; hours: number }
export interface StudioOrder { id: string; title: string; available_week: number; deadline_week: number; total_hours: number; remaining_hours: number; payment_minor: number; paid_minor: number; status: "offered" | "active" | "late" | "delivered" | "expired" }
export interface StudioWeekResult {
  week: number; allocations: StudioAllocation[]; used_hours: number; unused_hours: number;
  cash_before_minor: number; cash_after_minor: number; cash_delta_minor: number; product_before_hours: number; product_after_hours: number;
  order_outcomes: { id: string; status: StudioOrder["status"]; worked_hours: number; remaining_hours: number; payment_minor: number; explanation: string }[];
  explanation: string[]; goal: { cash_met: boolean; product_met: boolean; commitments_met: boolean; success: boolean };
}
export interface StudioRun { id: string; revision: number; ruleset: "studio-v1"; week: number; cash_minor: number; product_hours: number; orders: StudioOrder[]; history: StudioWeekResult[] }
export interface StudioState {
  active: boolean; can_start: boolean; reason?: string; run: StudioRun | null;
  rules: { weekly_capacity_hours: number; weekly_overhead_minor: number; product_target_hours: number; cash_target_minor: number; total_weeks: number; late_payment_percent: number };
  event: { scene: string } | null;
  choices: { id: string; title: string; benefit: string; tradeoff: string; allocations: StudioAllocation[]; preview: StudioWeekResult }[];
}
export type StudioSelection = { option_id: string } | { allocations: StudioAllocation[] };
export function canWriteStudio(session: Session | null) {
  return session?.role === "owner" || session?.kind === "owner" || Boolean(session && ["company.write", "finance.write", "reports.write", "goals.write"].every((scope) => session.scopes.includes(scope)));
}
export interface StudioSession {
  state: StudioState | null; loading: boolean; error: string; busy: boolean;
  reload: () => Promise<void>; start: () => Promise<StudioState>; choose: (selection: StudioSelection) => Promise<StudioState>; preview: (allocations: StudioAllocation[]) => Promise<StudioWeekResult>;
}
interface StudioReceipt { status: string; studio: StudioState }
export const studioApi = {
  read: (companyId: string, credential: string, signal?: AbortSignal) => request<StudioState>(`/companies/${encodeURIComponent(companyId)}/studio`, { signal }, credential),
  start: (companyId: string, credential: string, input: { request_id: string; expected_company_revision: number }) => request<StudioReceipt>(`/companies/${encodeURIComponent(companyId)}/studio/start`, { method: "POST", body: JSON.stringify(input) }, credential),
  choose: (companyId: string, credential: string, input: { request_id: string; run_id: string; expected_run_revision: number } & StudioSelection) => request<StudioReceipt>(`/companies/${encodeURIComponent(companyId)}/studio/choose`, { method: "POST", body: JSON.stringify(input) }, credential),
  preview: (companyId: string, credential: string, input: { run_id: string; expected_run_revision: number; allocations: StudioAllocation[] }) => request<StudioWeekResult | { preview: StudioWeekResult; run_id: string; run_revision: number }>(`/companies/${encodeURIComponent(companyId)}/studio/preview`, { method: "POST", body: JSON.stringify(input) }, credential),
};

export function studioAttempt(input: object, previous?: { signature: string; request_id: string }) {
  const signature = JSON.stringify(input);
  return previous?.signature === signature ? previous : { signature, request_id: crypto.randomUUID() };
}
export function validStudioAllocation(allocations: StudioAllocation[], capacity = 20) {
  return allocations.every((entry) => Number.isInteger(entry.hours) && entry.hours >= 0 && entry.hours <= capacity && entry.hours % 2 === 0) && allocations.reduce((total, entry) => total + entry.hours, 0) <= capacity;
}
export function studioTargets(run: StudioRun) {
  return [{ id: "product", title: text.product }, ...run.orders.filter((order) => order.available_week <= run.week && ["offered", "active", "late"].includes(order.status)).map((order) => ({ id: order.id, title: order.title }))];
}
export function useStudioSession(company: Company | null): StudioSession {
  const locale = useLocale();
  const credential = getCredential();
  const companyId = company?.id || "";
  const scope = JSON.stringify([companyId, credential]);
  const currentScope = useRef(scope); currentScope.current = scope;
  const alive = useRef(true);
  const companyRef = useRef(company); companyRef.current = company;
  const epoch = useRef(0);
  const controller = useRef<AbortController | null>(null);
  const attempt = useRef<{ signature: string; request_id: string } | undefined>(undefined);
  const [value, setValue] = useState<{ scope: string; state: StudioState | null; loading: boolean; error: string; busy: boolean }>({ scope: "", state: null, loading: false, error: "", busy: false });
  const valid = () => alive.current && currentScope.current === scope && getCredential() === credential;
  const reload = useCallback(async () => {
    if (!companyId || !credential) return;
    const generation = ++epoch.current;
    controller.current?.abort(); controller.current = new AbortController();
    const signal = controller.current.signal;
    setValue((old) => old.scope === scope ? { ...old, loading: !old.state, error: "" } : { scope, state: null, loading: true, error: "", busy: false });
    try {
      const state = await studioApi.read(companyId, credential, signal);
      if (valid() && generation === epoch.current && !signal.aborted) setValue((old) => ({ scope, state, loading: false, error: "", busy: old.scope === scope && old.busy }));
    } catch (error) {
      if (valid() && generation === epoch.current && !signal.aborted) setValue((old) => ({ scope, state: old.scope === scope ? old.state : null, loading: false, error: error instanceof Error ? error.message : text.loadError, busy: false }));
    }
  }, [companyId, credential, scope]);
  useEffect(() => { void reload(); return () => controller.current?.abort(); }, [reload, company?.revision, locale]);
  useEffect(() => { alive.current = true; return () => { alive.current = false; controller.current?.abort(); }; }, []);
  const state = value.scope === scope ? value.state : null;
  async function mutate(input: object, send: (requestId: string) => Promise<StudioReceipt>) {
    if (!valid()) throw new Error(text.changedSession);
    attempt.current = studioAttempt({ companyId, ...input }, attempt.current);
    setValue((old) => ({ ...old, busy: true, error: "" }));
    try {
      const receipt = await send(attempt.current.request_id);
      if (!valid()) throw new Error(text.changedSession);
      ++epoch.current; controller.current?.abort();
      setValue({ scope, state: receipt.studio, loading: false, error: "", busy: false });
      return receipt.studio;
    } catch (error) {
      if (valid()) setValue((old) => ({ ...old, busy: false }));
      throw error;
    }
  }
  return {
    state, loading: value.scope === scope ? value.loading : Boolean(companyId), error: value.scope === scope ? value.error : "", busy: value.scope === scope && value.busy, reload,
    start: () => { const input = { expected_company_revision: companyRef.current?.revision || 0 }; return mutate({ operation: "start", ...input }, (request_id) => studioApi.start(companyId, credential, { request_id, ...input })); },
    choose: (selection) => {
      if (!state?.run) return Promise.reject(new Error(text.loadError));
      const input = { run_id: state.run.id, expected_run_revision: state.run.revision, ...selection };
      return mutate({ operation: "choose", ...input }, (request_id) => studioApi.choose(companyId, credential, { request_id, ...input }));
    },
    preview: async (allocations) => {
      if (!state?.run) throw new Error(text.loadError);
      const response = await studioApi.preview(companyId, credential, { run_id: state.run.id, expected_run_revision: state.run.revision, allocations });
      if (!valid()) throw new Error(text.changedSession);
      return "preview" in response ? response.preview : response;
    },
  };
}

function statusName(status: StudioOrder["status"]) { return text[status]; }
function targetName(target: string, run: StudioRun) { return target === "product" ? text.product : run.orders.find((order) => order.id === target)?.title || text.order; }
function signedCash(value: number) { return `${value > 0 ? "+" : ""}${money(value, "CNY")}`; }
export function studioPaymentCause(order: StudioWeekResult["order_outcomes"][number], run: StudioRun, rules: StudioState["rules"]) {
  const original = run.orders.find((entry) => entry.id === order.id);
  if (order.payment_minor > 0 && original) return order.payment_minor === original.payment_minor ? text.onTimePayment : text.latePayment.replace("{percent}", String(rules.late_payment_percent));
  return order.status === "late" ? text.lateUnfinished : "";
}
export function localizeStudioState(state: StudioState | null): StudioState | null {
  if (!state?.run || state.run.ruleset !== "studio-v1") return state;
  const metadata = {
    deliver: { title: text.deliverTitle, benefit: text.deliverBenefit, tradeoff: text.deliverTradeoff },
    product: { title: text.productTitle, benefit: text.productBenefit, tradeoff: text.productTradeoff },
    balance: { title: text.balanceTitle, benefit: text.balanceBenefit, tradeoff: text.balanceTradeoff },
  };
  const scene = [text.scene1, text.scene2, text.scene3, text.scene4][state.run.week - 1];
  return { ...state, event: state.event && scene ? { ...state.event, scene } : state.event, choices: state.choices.map((choice) => ({ ...choice, ...(metadata[choice.id as keyof typeof metadata] || {}) })) };
}
function StudioConstraints({ run, rules }: { run: StudioRun; rules: StudioState["rules"] }) {
  const open = run.orders.filter((order) => order.available_week <= run.week && ["offered", "active", "late"].includes(order.status));
  return <div className="studio-current-constraints"><p>{text.capacity}: {number(rules.weekly_capacity_hours)} {text.hours} · {text.overhead}: {money(rules.weekly_overhead_minor, "CNY")}</p>{open.length > 0 && <ul>{open.map((order) => <li key={order.id}><strong>{order.title}</strong><span>{text.remaining} {number(order.remaining_hours)} {text.hours} · {text.deadline} {studioWeek(text.week, order.deadline_week)}</span></li>)}</ul>}<p className="studio-order-terms">{text.acceptRule}<br />{text.offerRule}</p></div>;
}
export function StudioAllocationView({ allocations, run, capacity }: { allocations: StudioAllocation[]; run: StudioRun; capacity: number }) {
  const allocated = allocations.filter((entry) => entry.hours > 0);
  const used = allocated.reduce((sum, entry) => sum + entry.hours, 0);
  return <section className="studio-allocation" aria-label={text.plan}><div className="studio-allocation-head"><strong>{text.plan}</strong><span>{number(used)} / {number(capacity)} {text.hours}</span></div><div className="studio-time-track" aria-hidden="true">{allocated.map((entry) => <span key={entry.target} className={entry.target === "product" ? "studio-time-product" : "studio-time-order"} style={{ flexGrow: entry.hours }} />)}{used < capacity && <span className="studio-time-unused" style={{ flexGrow: capacity - used }} />}</div><ul>{allocated.map((entry) => <li key={entry.target}><span>{targetName(entry.target, run)}</span><strong>{number(entry.hours)} {text.hours}</strong></li>)}{used < capacity && <li><span>{text.unused}</span><strong>{number(capacity - used)} {text.hours}</strong></li>}</ul></section>;
}
export function StudioResult({ result, run, rules, preview = false }: { result: StudioWeekResult; run: StudioRun; rules: StudioState["rules"]; preview?: boolean }) {
  const orderOutcomes = result.order_outcomes.filter((outcome) => {
    const order = run.orders.find((entry) => entry.id === outcome.id);
    return !order || order.available_week <= result.week;
  });
  return <section className={`studio-result${preview ? " studio-preview" : ""}`} aria-label={preview ? text.previewTitle : studioWeek(text.result, result.week)}><h3>{preview ? text.previewTitle : studioWeek(text.result, result.week)}</h3><dl className="studio-result-values"><div><dt>{text.before}</dt><dd>{money(result.cash_before_minor, "CNY")}</dd></div><div><dt>{text.after}</dt><dd>{money(result.cash_after_minor, "CNY")}</dd></div><div><dt>{text.change}</dt><dd>{signedCash(result.cash_delta_minor)}</dd></div><div><dt>{text.productChange}</dt><dd>+{number(result.product_after_hours - result.product_before_hours)} {text.hours}</dd></div></dl><p className="studio-cost-cause">{text.weeklyCost}: {money(rules.weekly_overhead_minor, "CNY")}{result.unused_hours > 0 && ` · ${text.savedTime}: ${number(result.unused_hours)} ${text.hours}`}</p>{orderOutcomes.length > 0 && <ul className="studio-order-results">{orderOutcomes.map((order) => <li key={order.id}><div><strong>{targetName(order.id, run)}</strong><span>{statusName(order.status)}</span></div><p>{text.worked}: {number(order.worked_hours)} {text.hours} · {text.afterHours}: {number(order.remaining_hours)} {text.hours}{order.payment_minor > 0 && ` · ${text.received}: ${money(order.payment_minor, "CNY")}`}</p>{studioPaymentCause(order, run, rules) && <p className="studio-payment-cause">{studioPaymentCause(order, run, rules)}</p>}</li>)}</ul>}{getLocale() === "en" && result.explanation.length > 0 && <details className="studio-result-basis"><summary>{text.resultBasis}</summary><ul>{result.explanation.map((explanation, index) => <li key={index}>{explanation}</li>)}</ul></details>}</section>;
}
export function StudioEnding({ result, rules, stopped, onStop }: { result: StudioWeekResult; rules: StudioState["rules"]; stopped: boolean; onStop: () => void }) {
  const goals = [{ title: `${text.cashTarget} ${money(rules.cash_target_minor, "CNY")}`, met: result.goal.cash_met }, { title: `${text.productTarget} ${number(rules.product_target_hours)} ${text.hours}`, met: result.goal.product_met }, { title: text.commitmentsTarget, met: result.goal.commitments_met }];
  return <section className="studio-ending"><h2>{text.chapterEnd}</h2><strong>{result.goal.success ? text.success : text.incomplete}</strong><ul>{goals.map((goal) => <li key={goal.title}>{goal.met ? <CheckCircle2 size={20} aria-hidden="true" /> : <XCircle size={20} aria-hidden="true" />}<span>{goal.title}</span><strong>{goal.met ? text.met : text.notMet}</strong></li>)}</ul>{stopped ? <p role="status">{text.stopped}</p> : <button className="text-button" onClick={onStop}>{text.stop}</button>}</section>;
}

export function StudioGame({ snapshot, writable, session, onChanged, onNarrative }: { snapshot: Snapshot; writable: boolean; session: StudioSession; onChanged: () => Promise<void>; onNarrative?: () => void }) {
  useLocale();
  const { loading, busy } = session;
  const state = localizeStudioState(session.state);
  const run = state?.run;
  const [selection, setSelection] = useState("");
  const [draft, setDraft] = useState<Record<string, number>>({});
  const [customPreview, setCustomPreview] = useState<{ signature: string; revision: number; result: StudioWeekResult } | null>(null);
  const [previewBusy, setPreviewBusy] = useState(false);
  const [error, setError] = useState("");
  const [reviewWeek, setReviewWeek] = useState<number | null>(null);
  const [stopped, setStopped] = useState(false);
  const seenKey = `transplane-studio-seen:${snapshot.company.id}:${run?.id || ""}`;
  const readSeen = () => { try { return Number(sessionStorage.getItem(seenKey) || 0); } catch { return 0; } };
  const [seenState, setSeenState] = useState(() => ({ key: seenKey, value: readSeen() }));
  const seen = seenState.key === seenKey ? seenState.value : readSeen();
  useEffect(() => { setSelection(""); setCustomPreview(null); setDraft({}); setError(""); }, [run?.id, run?.revision]);
  const last = run?.history.at(-1);
  const result = run?.history.find((entry) => entry.week === reviewWeek) || (last && (last.week > seen || Boolean(run && run.week > (state?.rules.total_weeks || 4))) ? last : undefined);
  const choice = state?.choices.find((entry) => entry.id === selection);
  const targets = run ? studioTargets(run) : [];
  const allocations = selection === "__custom" ? targets.map((target) => ({ target: target.id, hours: draft[target.id] || 0 })).filter((entry) => entry.hours > 0) : choice?.allocations || [];
  const signature = JSON.stringify(allocations);
  const preview = choice?.preview || (customPreview?.signature === signature && customPreview.revision === run?.revision ? customPreview.result : null);
  const valid = Boolean(run && validStudioAllocation(allocations, state?.rules.weekly_capacity_hours));
  const currentInput = useRef({ scope: "", revision: 0, signature: "" });
  currentInput.current = { scope: `${snapshot.company.id}:${run?.id}`, revision: run?.revision || 0, signature };
  async function previewCustom() {
    if (!valid || previewBusy || !run) { setError(text.hoursInvalid); return; }
    const sent = { ...currentInput.current };
    setPreviewBusy(true); setError("");
    try {
      const outcome = await session.preview(allocations);
      if (sent.scope === currentInput.current.scope && sent.revision === currentInput.current.revision && sent.signature === currentInput.current.signature) setCustomPreview({ signature, revision: sent.revision, result: outcome });
    } catch (cause) { setError(cause instanceof Error ? cause.message : text.loadError); }
    finally { setPreviewBusy(false); }
  }
  async function start() {
    if (!writable || busy || !state?.can_start) return;
    setError("");
    try { await session.start(); await onChanged().catch(() => {}); }
    catch (cause) { setError(cause instanceof Error ? cause.message : text.writeError); }
  }
  async function settle(event: FormEvent) {
    event.preventDefault();
    if (!writable || busy || !run) return;
    if (!selection) { setError(text.pendingChoice); return; }
    if (!valid) { setError(text.hoursInvalid); return; }
    if (!preview) { setError(text.previewRequired); return; }
    setError("");
    try { await session.choose(selection === "__custom" ? { allocations } : { option_id: selection }); setReviewWeek(null); await onChanged().catch(() => {}); }
    catch (cause) { setError(cause instanceof Error ? cause.message : text.writeError); }
  }
  function continueWeek() { if (!last) return; setSeenState({ key: seenKey, value: last.week }); setReviewWeek(null); try { sessionStorage.setItem(seenKey, String(last.week)); } catch { /* Reading remains available in this page. */ } }
  if (loading && !state) return <Loading />;
  if (!state) return <div className="studio-game"><ErrorNotice message={session.error || text.loadError} retry={() => void session.reload()} />{onNarrative && <button className="text-button" onClick={onNarrative}>{text.continueStory}</button>}</div>;
  if (!run) return <section className="studio-game studio-start"><div className="studio-heading"><h1>{text.title}</h1><span>{text.simulation}</span></div>{error && <ErrorNotice message={error} />}{state.can_start ? <><p>{text.startHelp}</p><dl className="studio-start-resources"><div><dt>{text.startingCash}</dt><dd>{money(600000, "CNY")}</dd></div><div><dt>{text.capacity}</dt><dd>{number(state.rules.weekly_capacity_hours)} {text.hours}</dd></div><div><dt>{text.overhead}</dt><dd>{money(state.rules.weekly_overhead_minor, "CNY")}</dd></div></dl><StudioGoals rules={state.rules} /><p className="studio-late-rule">{text.lateRule.replace("{percent}", String(state.rules.late_payment_percent))}</p>{!writable && <p>{text.readOnly}</p>}<button className="primary-button" disabled={!writable || busy} onClick={() => void start()}>{busy ? text.starting : text.start}<ArrowRight size={18} aria-hidden="true" /></button></> : <p>{snapshot.company.mode !== "simulation" ? text.realUnavailable : snapshot.company.currency !== "CNY" ? text.currencyUnavailable : text.existingUnavailable}</p>}{onNarrative && <button className="text-button studio-narrative" onClick={onNarrative}>{text.continueStory}</button>}</section>;
  return <div className="studio-game"><header className="studio-heading"><div><h1>{text.title}</h1><span>{result ? studioWeek(text.result, result.week) : studioWeek(text.week, run.week)} · {snapshot.company.name}</span></div><span>{text.simulation}</span></header><div className="studio-resource-line"><span>{text.cash}<strong>{money(result?.cash_after_minor ?? run.cash_minor, "CNY")}</strong></span><span>{text.product}<strong>{number(result?.product_after_hours ?? run.product_hours)} / {number(state.rules.product_target_hours)} {text.hours}</strong></span></div><StudioGoals rules={state.rules} />{(error || session.error) && <ErrorNotice message={error || session.error} retry={() => void session.reload()} />}{result ? <><StudioAllocationView allocations={result.allocations} run={run} capacity={state.rules.weekly_capacity_hours} /><StudioResult result={result} run={run} rules={state.rules} />{result.week === state.rules.total_weeks && <StudioEnding result={result} rules={state.rules} stopped={stopped} onStop={() => setStopped(true)} />}{run.week <= state.rules.total_weeks && <button className="primary-button studio-next" onClick={continueWeek}>{studioWeek(text.next, run.week)}<ArrowRight size={18} aria-hidden="true" /></button>}{run.week > state.rules.total_weeks && result.week !== state.rules.total_weeks && <button className="text-button" onClick={() => setReviewWeek(state.rules.total_weeks)}>{studioWeek(text.recap, state.rules.total_weeks)}</button>}</> : <div className="studio-week-layout"><section className="studio-current-event"><div className="studio-scene"><h2>{studioWeek(text.week, run.week)}</h2>{state.event?.scene && <p>{state.event.scene}</p>}</div><StudioConstraints run={run} rules={state.rules} /><form noValidate className="studio-choice-form" onSubmit={(event) => void settle(event)}><fieldset disabled={!writable || busy}><legend>{text.choose}</legend>{state.choices.map((entry) => <label className={`studio-choice${selection === entry.id ? " is-selected" : ""}`} key={entry.id}><input type="radio" name="studio-choice" checked={selection === entry.id} onChange={() => { setSelection(entry.id); setError(""); }} /><span><strong>{entry.title}</strong><span>{text.benefit}: {entry.benefit}</span><span>{text.tradeoff}: {entry.tradeoff}</span></span></label>)}<label className="studio-choice studio-custom-choice"><input type="radio" name="studio-choice" checked={selection === "__custom"} onChange={() => { setSelection("__custom"); setError(""); }} /><strong>{text.custom}</strong></label>{selection === "__custom" && <div className="studio-custom-fields">{targets.map((target) => <label key={target.id}><span>{target.title}</span><input type="number" min={0} max={state.rules.weekly_capacity_hours} step={2} value={draft[target.id] || 0} onChange={(event) => { setDraft((old) => ({ ...old, [target.id]: Number(event.target.value) })); setError(""); }} /><span>{text.hours}</span></label>)}{!valid && <p className="studio-input-error" role="status">{text.hoursInvalid}</p>}<button type="button" className="secondary-button" disabled={previewBusy || !valid} onClick={() => void previewCustom()}>{previewBusy ? text.previewing : text.preview}</button></div>}</fieldset><StudioAllocationView allocations={allocations} run={run} capacity={state.rules.weekly_capacity_hours} />{preview && <StudioResult result={preview} run={run} rules={state.rules} preview />}{!writable && <p>{text.readOnly}</p>}<button className="primary-button" type="submit" disabled={!writable || busy || previewBusy || !selection || !preview || !valid}>{busy ? text.settling : text.settle}<ArrowRight size={18} aria-hidden="true" /></button></form></section><details className="studio-orders"><summary>{text.orders}</summary><p className="studio-week-cost">{text.overhead}: {money(state.rules.weekly_overhead_minor, "CNY")}</p><ul>{run.orders.filter((order) => order.available_week <= run.week).map((order) => <li key={order.id}><div><strong>{order.title}</strong><span>{statusName(order.status)}</span></div><dl><div><dt>{text.remaining}</dt><dd>{number(order.remaining_hours)} {text.hours}</dd></div><div><dt>{text.deadline}</dt><dd>{studioWeek(text.week, order.deadline_week)}</dd></div><div><dt>{order.status === "delivered" ? text.received : text.payment}</dt><dd>{money(order.status === "delivered" ? order.paid_minor : order.payment_minor, "CNY")}</dd></div></dl></li>)}</ul><p className="studio-late-rule">{text.lateRule.replace("{percent}", String(state.rules.late_payment_percent))}</p></details></div>}{run.history.length > 0 && <nav className="studio-history" aria-label={text.history}>{run.history.map((entry) => <button key={entry.week} className="text-button" onClick={() => setReviewWeek(entry.week)} aria-current={result?.week === entry.week ? "page" : undefined}>{studioWeek(text.recap, entry.week)}</button>)}</nav>}{onNarrative && <button className="text-button studio-narrative" onClick={onNarrative}>{text.continueStory}</button>}</div>;
}
function StudioGoals({ rules }: { rules: StudioState["rules"] }) {
  return <div className="studio-goal"><Flag size={18} aria-hidden="true" /><span>{text.goal}: {text.cashTarget} {money(rules.cash_target_minor, "CNY")} · {text.productTarget} {number(rules.product_target_hours)} {text.hours} · {text.commitmentsTarget}</span></div>;
}
