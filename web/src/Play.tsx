import { useEffect, useRef, useState, type FormEvent } from "react";
import { api } from "./api";
import { CopyButton, Empty, ErrorNotice } from "./components";
import { copy, recordCopy } from "./copy";
import { EvidenceTrace } from "./EvidenceTrace";
import { arr, date, money, num, number, obj, percent, str, title } from "./format";
import { ArrowRight, CheckCircle2, Coin, Flag, XCircle } from "./icons";
import { narrative as text } from "./narrativeCopy";
import type { Json, RecordEntry, RecordKind, Snapshot } from "./types";
import "./gameplay-orientation.css";
import { GameProgress } from "./GameProgress";
import type { GameState } from "./gameSystemsTypes";

function ordered(records: RecordEntry[] = []) {
  return [...records].sort((a, b) => b.created_at.localeCompare(a.created_at) || b.id.localeCompare(a.id));
}
async function exactRecordVersion(companyId: string, kind: RecordKind, id: string, revision: number | null) {
  let offset = 0;
  for (;;) {
    const response = await api.records(companyId, kind, offset, true, id);
    const record = response.records.find((r) => r.id === id && r.revision === revision);
    if (record) return record;
    if (!response.has_more || response.next_offset == null || response.next_offset <= offset) throw new Error(text.resultUnavailable);
    offset = response.next_offset;
  }
}
export function decisionStatus(record: RecordEntry) {
  const status = str(record.data.status);
  return status === "processing" ? text.processing : status === "selected" ? text.selectedStatus : status === "resolved" ? text.resolved : status === "cancelled" ? text.cancelled : text.open;
}

export function agentDecisionPrompt(snapshot: Snapshot, decision?: RecordEntry) {
  return [
    text.promptIntro,
    `${text.promptEntry}: ${location.origin}/api/v1/agent-entry`,
    `${text.promptCompany}: ${snapshot.company.id}`,
    `${text.promptMode}: ${snapshot.company.mode || "real"}`,
    decision ? `${text.promptDecision}: ${decision.id} / ${decision.revision}` : "",
    decision ? `${text.promptSelection}: ${decisionSelectionText(decision)}` : "",
    `${text.promptPlayerOutput}\n${location.origin}/api/v1/agent-guide?topic=player-output`,
    decision ? text.promptResolve : text.promptNext,
    snapshot.company.mode === "simulation" ? text.promptSimulation : "",
    text.promptClock,
  ].filter(Boolean).join("\n\n");
}

function routeContext(snapshot: Snapshot, decision?: RecordEntry) {
  const route = (decision ? snapshot.records.route?.find((r) => r.id === str(decision.data.route_id)) : undefined) || [...(snapshot.records.route || [])].sort((a, b) => b.updated_at.localeCompare(a.updated_at))[0];
  const stages = arr(route?.data.stages).map(obj);
  const requested = str(decision?.data.stage_id);
  const allGoals = snapshot.records.goal || [];
  const pendingGoal = (g: RecordEntry) => !["archived", "cancelled"].includes(str(g.data.status)) && snapshot.metrics.goal_progress.find((p) => p.id === g.id)?.status !== "achieved";
  const stage = stages.find((s) => requested && str(s.id) === requested) || stages.find((s) => arr(s.goal_ids).some((id) => allGoals.some((g) => g.id === str(id) && pendingGoal(g))));
  const linked = decision && arr(decision.data.goal_ids).length ? arr(decision.data.goal_ids) : stage ? arr(stage.goal_ids) : arr(route?.data.goal_ids);
  const goals = linked.length ? linked.map((id) => allGoals.find((g) => g.id === str(id))).filter((g): g is RecordEntry => Boolean(g)) : allGoals;
  const goal = goals.find(pendingGoal) || allGoals.find(pendingGoal);
  const goalStage = goal && stages.find((s) => arr(s.goal_ids).some((id) => str(id) === goal.id));
  return { route, stage: goal ? goalStage || (linked.some((id) => str(id) === goal.id) ? stage : undefined) : stage, goal };
}

export function Play({ snapshot, owner, select, onChanged, onGuide, onAuthorize, gameState, onCommand }: {
  snapshot: Snapshot;
  owner: boolean;
  select: (record: RecordEntry) => void;
  onChanged: () => Promise<void>;
  onGuide?: () => void;
  onAuthorize?: () => void;
  gameState?: GameState | null;
  onCommand?: () => void;
}) {
  const decisions = ordered(snapshot.records.decision);
  const decision = decisions.find((d) => ["open", "selected", "processing"].includes(str(d.data.status)));
  const settled = decisions.find((d) => str(d.data.status) === "resolved");
  const lastTurn = ordered(snapshot.records.simulation_turn)[0];
  const simulation = snapshot.company.mode === "simulation";
  const seenKey = `transplane-company-outcome-seen:${snapshot.company.id}`;
  const [seen, setSeen] = useState(() => {
    try { return sessionStorage.getItem(seenKey) || ""; } catch { return ""; }
  });
  const [previous, setPrevious] = useState(false);
  const settlementId = settled ? `${settled.id}:${settled.revision}` : "";
  const showSettlement = settled && (!decision || seen !== settlementId || previous);
  const { route, stage, goal } = routeContext(snapshot, decision);
  const phase = showSettlement ? "outcome" : decision ? ["selected", "processing"].includes(str(decision.data.status)) ? "waiting" : "choice" : settled || lastTurn ? "next" : "opening";
  const phaseLabel = phase === "outcome" ? text.phaseOutcome : phase === "waiting" ? text.phaseWaiting : phase === "choice" ? text.phaseChoice : phase === "next" ? text.phaseNext : text.phaseOpening;
  function continueToDecision() {
    setPrevious(false);
    setSeen(settlementId);
    try { sessionStorage.setItem(seenKey, settlementId); } catch { /* Keep the local view state without storage. */ }
  }
  return (
    <div className="play-world">
      <section className="play-scene" aria-label={snapshot.company.name}>
        <img src="/assets/company-pixel-scene.png" alt="" width="2172" height="724" fetchPriority="high" />
        <div className="play-scene-name">
          <h1>{snapshot.company.name}</h1>
          {route && <span>{title(route)}</span>}
        </div>
        <div className="play-resource-bar" aria-label={copy.hudLabel}>
          <span className={simulation ? "simulation-badge" : "real-badge"}>{simulation ? text.simulation : text.real}</span>
          <span><Coin size={20} aria-hidden="true" /><span className="sr-only">{text.cash}</span><strong>{money(snapshot.metrics.cash_balance_minor, snapshot.company.currency)}</strong></span>
          <span><span className="sr-only">{text.date}</span>{date(simulation ? snapshot.company.simulation_date || snapshot.metrics.as_of : snapshot.metrics.as_of)}</span>
          {simulation && <span>{text.turn} {snapshot.records.simulation_turn?.length || 0}</span>}
          {gameState?.company_id === snapshot.company.id && <GameProgress state={gameState} onOpen={onCommand} compact />}
        </div>
      </section>
      <div className="play-story-stage">
        <div className={`play-phase play-phase-${phase}`}><span role="status">{phaseLabel}</span>{onGuide && <button className="text-button" onClick={onGuide}>{text.guide}</button>}</div>
        <MissionBrief snapshot={snapshot} stage={stage} goal={goal} />
        {showSettlement ? (
          <article className="narrative-dialogue settlement-dialogue">
            <div className="dialogue-kicker">{text.outcome}</div>
            <h2>{title(settled)}</h2>
            <p className="story-prose">{str(obj(settled.data.resolution).summary)}</p>
            <details className="outcome-evidence">
              <summary>{text.previousSituation}</summary>
              <p className="story-prose">{str(settled.data.situation)}</p>
              {arr(settled.data.options).length > 0 && <ol className="record-list">{arr(settled.data.options).map((option, i) => {
                const savedOption = obj(option);
                return <li key={i}><strong>{str(savedOption.title)}</strong>{str(savedOption.description) && <p>{str(savedOption.description)}</p>}{arr(savedOption.assumptions).length > 0 && <p>{arr(savedOption.assumptions).map((assumption) => str(assumption)).join(" · ")}</p>}</li>;
              })}</ol>}
              <EvidenceTrace record={settled} />
            </details>
            <SavedChoice decision={settled} />
            <DecisionSettlement decision={settled} snapshot={snapshot} select={select} />
            {decision ? <div className="next-decision"><button className="primary-button" onClick={continueToDecision}>{text.play}<ArrowRight size={18} aria-hidden="true" /></button></div> : <NextDecision snapshot={snapshot} onGuide={onGuide} onAuthorize={owner ? onAuthorize : undefined} />}
          </article>
        ) : !decision ? (
          <article className="narrative-dialogue">
            <div className="dialogue-kicker">{lastTurn ? text.outcome : text.journey}</div>
            <h2>{lastTurn ? title(lastTurn) : text.openingTitle}</h2>
            {(lastTurn ? str(lastTurn.data.story) : str(route?.data.story || route?.data.narrative) || snapshot.company.description) && <p className="story-prose">{lastTurn ? str(lastTurn.data.story) : str(route?.data.story || route?.data.narrative) || snapshot.company.description}</p>}
            {lastTurn && <><LegacyOutcome turn={lastTurn} /><button className="text-button" onClick={() => select(lastTurn)}>{text.inspect}</button></>}
            <NextDecision snapshot={snapshot} initial={!lastTurn && !settled} onGuide={onGuide} onAuthorize={owner ? onAuthorize : undefined} />
          </article>
        ) : null}
        {decision && <div hidden={Boolean(showSettlement)}>
          {settled && <div className="dialogue-foot"><button className="text-button" onClick={() => setPrevious(true)}>{text.previousOutcome}</button></div>}
          <DecisionScene key={`${snapshot.company.id}:${decision.id}`} decision={decision} snapshot={snapshot} owner={owner} select={select} onChanged={onChanged} onAuthorize={onAuthorize} />
        </div>}
      </div>
    </div>
  );
}

function MissionBrief({ snapshot, stage, goal }: { snapshot: Snapshot; stage?: Record<string, Json>; goal?: RecordEntry }) {
  const progress = goal && snapshot.metrics.goal_progress.find((p) => p.id === goal.id);
  const achieved = snapshot.metrics.goal_progress.filter((p) => p.status === "achieved");
  const recordedGoals = snapshot.records.goal || [];
  return <aside className="mission-brief" aria-label={text.inspectGoals}>
    <div className="mission-current"><Flag size={24} aria-hidden="true" /><div>
      {stage && <span className="mission-stage">{str(stage.name || stage.title)}</span>}
      <span className="mission-label">{text.activeGoal}</span>
      {goal ? <a className="mission-title" href="#growth">{title(goal)}<ArrowRight size={16} aria-hidden="true" /></a> : <p className="mission-title">{text.objectiveMissing}</p>}
      {goal && <div className="mission-progress"><span>{text.currentProgress}: {progress?.progress == null ? copy.unknown : percent(progress.progress)}</span>{progress?.progress != null && <progress aria-label={`${title(goal)} · ${text.currentProgress}`} max={1} value={Math.max(0, Math.min(1, progress.progress))} />}</div>}
    </div></div>
    {recordedGoals.length > 0 && <details className="mission-log"><summary><CheckCircle2 size={18} aria-hidden="true" /><span>{text.achievedGoals}<strong>{number(achieved.length)}</strong></span></summary><ul>{recordedGoals.map((record) => {
      const state = snapshot.metrics.goal_progress.find((p) => p.id === record.id);
      return <li key={record.id}><span>{title(record)}</span><small>{state ? copy.statuses[state.status] || state.status : copy.unknown}{state?.progress != null && ` · ${percent(state.progress)}`}</small></li>;
    })}</ul><a href="#growth">{text.missionLog}<ArrowRight size={14} aria-hidden="true" /></a></details>}
  </aside>;
}

function DecisionScene({ decision, snapshot, owner, select, onChanged, onAuthorize }: {
  decision: RecordEntry; snapshot: Snapshot; owner: boolean;
  select: (r: RecordEntry) => void; onChanged: () => Promise<void>;
  onAuthorize?: () => void;
}) {
  const [optionId, setOptionId] = useState("");
  const [freeform, setFreeform] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [saved, setSaved] = useState<RecordEntry | null>(null);
  const [editingRevision, setEditingRevision] = useState<number | null>(null);
  const attempt = useRef<{ input: string; id: string } | null>(null);
  const options = arr(decision.data.options).map(obj);
  const shown = saved && decision.revision < saved.revision ? saved : decision;
  const processing = str(shown.data.status) === "processing";
  const selected = str(shown.data.status) === "selected" || processing;
  const editing = editingRevision != null && !processing;
  const valid = optionId === "__freeform" ? freeform.trim().length > 0 : options.some((o) => str(o.id) === optionId);
  async function submit(event: FormEvent) {
    event.preventDefault();
    if (!valid || busy || !owner || processing) return;
    const selection = optionId === "__freeform" ? { text: freeform.trim() } : { option_id: optionId };
    const submittedDecision = editingRevision == null ? shown : { ...shown, revision: editingRevision };
    const serialized = JSON.stringify({ decision: submittedDecision.id, revision: submittedDecision.revision, selection });
    if (attempt.current?.input !== serialized) attempt.current = { input: serialized, id: crypto.randomUUID() };
    setBusy(true); setError("");
    try {
      const response = await api.chooseDecision(snapshot.company.id, submittedDecision, selection, attempt.current.id);
      setSaved(response.record);
      setEditingRevision(null);
      await onChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : copy.connectFailed);
    } finally { setBusy(false); }
  }
  function editChoice() {
    const draft = decisionChoiceDraft(shown);
    setOptionId(draft.optionId);
    setFreeform(draft.freeform);
    setEditingRevision(shown.revision);
    setError("");
  }
  return (
    <article className="narrative-dialogue" aria-labelledby="current-decision-title">
      <div className="dialogue-kicker">{text.currentEvent}{str(decision.data.deadline) && <span>{text.deadline} {date(str(decision.data.deadline))}</span>}</div>
      <h2 id="current-decision-title">{title(decision)}</h2>
      <p className="story-prose">{str(decision.data.situation)}</p>
      {selected && !editing ? (
        <div className="selected-choice" role="status">
          <div className="selection-stamp"><CheckCircle2 size={20} aria-hidden="true" /><strong>{processing ? text.processing : text.selected}</strong></div>
          <SavedChoice decision={shown} />
          <p>{processing ? text.processingHelp : text.waiting}</p>
          {!processing && <><div className="agent-handoff-actions"><CopyButton value={agentDecisionPrompt(snapshot, shown)} label={text.copySelection} />{owner && <button className="text-button" type="button" onClick={editChoice}>{text.changeChoice}</button>}</div><p className="handoff-boundary">{text.handoffHelp}</p><details className="agent-handoff"><summary>{text.handoffDetails}</summary><pre>{agentDecisionPrompt(snapshot, shown)}</pre>{owner && onAuthorize && <button className="text-button" onClick={onAuthorize}>{text.handoffAuthorize}</button>}</details></>}
        </div>
      ) : (
        <form onSubmit={submit} className="decision-form">
          <fieldset disabled={!owner || busy}>
            <legend>{text.choose}</legend>
            {options.map((option, i) => (
              <label className={`decision-option ${optionId === str(option.id) ? "chosen-option" : ""}`} key={str(option.id)}>
                <input type="radio" name="business-choice" value={str(option.id)} checked={optionId === str(option.id)} onChange={() => setOptionId(str(option.id))} />
                <span className="choice-letter" aria-hidden="true">{String.fromCharCode(65 + i)}</span>
                <span className="choice-content"><strong>{str(option.title)}</strong>{str(option.description) && <span>{str(option.description)}</span>}{num(option.estimated_cost_minor) != null && <small>{text.expectedCost}: {money(num(option.estimated_cost_minor), snapshot.company.currency)}</small>}{arr(option.assumptions).length > 0 && <span className="choice-assumptions">{recordCopy.assumptions}: {arr(option.assumptions).map((a) => str(a)).join(" · ")}</span>}</span>
              </label>
            ))}
            {decision.data.allow_freeform === true && (
              <div className={`freeform-option ${optionId === "__freeform" ? "chosen-option" : ""}`}>
                <label className="freeform-label"><input type="radio" name="business-choice" value="__freeform" checked={optionId === "__freeform"} onChange={() => setOptionId("__freeform")} />{text.freeform}</label>
                {optionId === "__freeform" && <textarea aria-label={text.freeform} value={freeform} onChange={(e) => setFreeform(e.target.value)} placeholder={text.freeformHint} rows={3} maxLength={4000} />}
              </div>
            )}
          </fieldset>
          {!owner && <p className="field-help">{text.readOnly}</p>}
          {owner && <div className="dialogue-foot"><button className="primary-button decision-confirm" type="submit" aria-busy={busy} disabled={!valid || busy}>{busy ? text.submitting : editing ? text.saveReplacement : text.confirm}<ArrowRight size={18} aria-hidden="true" /></button>{editing && <button className="text-button" type="button" disabled={busy} onClick={() => { setEditingRevision(null); setError(""); }}>{text.cancelChange}</button>}</div>}
        </form>
      )}
      {error && <ErrorNotice message={error} retry={() => void onChanged()} />}
      <div className="dialogue-foot"><button className="text-button" onClick={() => select(decision)}>{text.inspect}</button><a href="#chronicle">{text.chronicle}<ArrowRight size={16} aria-hidden="true" /></a></div>
    </article>
  );
}

export function SavedChoice({ decision }: { decision: RecordEntry }) {
  const selection = decisionSelectionText(decision);
  return selection ? <div className="saved-choice"><span>{text.chosen}</span><p>{selection}</p></div> : null;
}
export function decisionSelectionText(decision: RecordEntry) {
  const selection = obj(decision.data.selection);
  const optionId = str(selection.option_id);
  return str(selection.text) || str(arr(decision.data.options).map(obj).find((o) => str(o.id) === optionId)?.title) || optionId;
}
export function decisionChoiceDraft(decision: RecordEntry) {
  const selection = obj(decision.data.selection);
  return { optionId: str(selection.option_id) || "__freeform", freeform: str(selection.text) };
}

function NextDecision({ snapshot, initial = false, onGuide, onAuthorize }: { snapshot: Snapshot; initial?: boolean; onGuide?: () => void; onAuthorize?: () => void }) {
  const prompt = agentDecisionPrompt(snapshot);
  return <div className="next-decision"><h3>{initial ? text.noDecision : text.next}</h3>{initial && <p>{text.noDecisionHelp}</p>}<div className="agent-handoff-actions"><CopyButton value={prompt} label={text.nextCopy} />{initial && onGuide && <button className="text-button" onClick={onGuide}>{text.guide}</button>}</div><p className="handoff-boundary">{text.handoffHelp}</p><details className="agent-handoff"><summary>{text.handoffDetails}</summary><pre>{prompt}</pre>{onAuthorize && <button className="text-button" onClick={onAuthorize}>{text.handoffAuthorize}</button>}</details></div>;
}

export function LegacyOutcome({ turn }: { turn: RecordEntry }) {
  const outcomes = arr(turn.data.outcomes);
  return outcomes.length > 0 ? <ul className="narrative-outcomes">{outcomes.map((value, i) => {
    const outcome = obj(value); const status = str(outcome.status);
    return <li key={i} className={`outcome-${status}`}>{status === "success" ? <CheckCircle2 size={20} aria-hidden="true" /> : status === "failure" ? <XCircle size={20} aria-hidden="true" /> : <Flag size={20} aria-hidden="true" />}<div><strong>{str(outcome.title)}</strong><span>{status === "success" ? text.goalSuccess : status === "failure" ? text.goalFailure : text.goalOngoing}</span>{str(outcome.evidence) && <p>{str(outcome.evidence)}</p>}</div></li>;
  })}</ul> : null;
}

export function DecisionSettlement({ decision, snapshot, select }: { decision: RecordEntry; snapshot: Snapshot; select?: (r: RecordEntry) => void }) {
  const resolution = obj(decision.data.resolution);
  const beforeRevision = num(decision.data.baseline_revision);
  const afterRevision = num(resolution.company_revision);
  const [before, setBefore] = useState<Snapshot | null>(null);
  const [after, setAfter] = useState<Snapshot | null>(null);
  const [records, setRecords] = useState<RecordEntry[]>([]);
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  const [reload, setReload] = useState(0);
  useEffect(() => {
    let live = true; setBusy(true); setError(""); setBefore(null); setAfter(null); setRecords([]);
    async function load() {
      if (beforeRevision == null || afterRevision == null) { setBusy(false); setError(text.baselineUnavailable); return; }
      try {
        const [baseline, result] = await Promise.all([api.history(decision.company_id, beforeRevision), api.history(decision.company_id, afterRevision)]);
        if (!live) return;
        setBefore(baseline); setAfter(result);
        const refs = arr(resolution.result_refs).map(obj);
        const linked = await Promise.all(refs.map((ref) => exactRecordVersion(decision.company_id, str(ref.kind) as RecordKind, str(ref.id), num(ref.revision))));
        if (live) setRecords(linked);
      } catch (e) { if (live) setError(e instanceof Error ? e.message : text.baselineUnavailable); }
      finally { if (live) setBusy(false); }
    }
    void load(); return () => { live = false; };
  }, [decision.company_id, decision.id, decision.revision, beforeRevision, afterRevision, reload]);
  const beforeCash = before?.metrics.cash_balance_minor;
  const afterCash = after?.metrics.cash_balance_minor;
  const delta = beforeCash == null || afterCash == null ? null : afterCash - beforeCash;
  const turns = records.filter((r) => r.kind === "simulation_turn");
  return <div className="decision-settlement">
    {busy && <p role="status">{text.loadingOutcome}</p>}
    {error && <ErrorNotice message={error} retry={() => setReload((x) => x + 1)} />}
    {before && after && <>
      <dl className="settlement-cash"><div><dt>{text.before}</dt><dd>{money(beforeCash, snapshot.company.currency)}</dd></div><ArrowRight size={22} aria-hidden="true" /><div><dt>{text.after}</dt><dd>{money(afterCash, snapshot.company.currency)}</dd></div>{delta != null && <div className="cash-change"><dt>{text.cashChange}</dt><dd>{delta > 0 ? "+" : ""}{money(delta, snapshot.company.currency)}</dd></div>}</dl>
      {turns.length ? turns.map((turn) => <LegacyOutcome key={turn.id} turn={turn} />) : arr(decision.data.goal_ids).length > 0 && <ul className="narrative-goal-progress">{arr(decision.data.goal_ids).map((id) => {
        const progress = after.metrics.goal_progress.find((g) => g.id === str(id));
        const goal = after.records.goal?.find((g) => g.id === str(id));
        return goal && progress ? <li key={goal.id}><strong>{title(goal)}</strong><span>{copy.statuses[progress.status] || progress.status}{progress.progress != null && ` · ${percent(progress.progress)}`}</span></li> : null;
      })}</ul>}
    </>}
    {records.length > 0 && <details className="outcome-evidence"><summary>{text.linkedResults}</summary>{records.map((record) => <div key={`${record.kind}:${record.id}:${record.revision}`}>{select ? <button className="record-link" onClick={() => select(record)}>{title(record)}</button> : <h3>{title(record)}</h3>}<EvidenceTrace record={record} /></div>)}</details>}
    {select && <button className="text-button" onClick={() => select(decision)}>{text.inspect}</button>}
  </div>;
}

export function Chronicle({ snapshot, select }: { snapshot: Snapshot; select: (r: RecordEntry) => void }) {
  const records = ordered([...(snapshot.records.decision || []), ...(snapshot.records.simulation_turn || []).filter((turn) => !(snapshot.records.decision || []).some((d) => arr(obj(d.data.resolution).result_refs).some((ref) => str(obj(ref).id) === turn.id && str(obj(ref).kind) === "simulation_turn")))]);
  return <div className="chronicle-page"><div className="chronicle-heading"><h1>{text.chronicle}</h1><a href="#overview">{text.play}<ArrowRight size={18} aria-hidden="true" /></a></div>{records.length ? <ol className="chronicle-list">{records.map((record) => <li key={`${record.kind}:${record.id}`}><article><div className="chronicle-date">{date(str(record.data.occurred_on, record.created_at))}{record.kind === "decision" && <span>{decisionStatus(record)}</span>}</div><h2><button className="record-link" onClick={() => select(record)}>{title(record)}</button></h2>{record.kind === "decision" ? <><SavedChoice decision={record} />{str(obj(record.data.resolution).summary) && <p className="chronicle-prose">{str(obj(record.data.resolution).summary)}</p>}</> : <><div className="saved-choice"><span>{text.chosen}</span><p>{str(record.data.action)}</p></div><p className="chronicle-prose">{str(record.data.story)}</p><LegacyOutcome turn={record} /></>}<button className="text-button" onClick={() => select(record)}>{text.inspect}</button></article></li>)}</ol> : <Empty>{text.noHistory}</Empty>}</div>;
}
