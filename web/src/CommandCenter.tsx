import { useEffect, useRef, useState } from "react";
import { ApiError, getCredential } from "./api";
import { CopyButton, Empty, ErrorNotice, Loading } from "./components";
import { arr, date, num, number, obj, str } from "./format";
import { AlertCircle, Check, FileText, Flag } from "./icons";
import { useLocale } from "./locale";
import { GameEvidence, operatingPrompt } from "./CompanyNetwork";
import { GameProgress } from "./GameProgress";
import { gameSystemsApi } from "./gameSystemsApi";
import { gameSystemsCopy as text, gameLabel } from "./gameSystemsCopy";
import { alertMatches, orderedAlerts, orderedQuests, questBlockers, questMatches, StableGameRequest } from "./gameSystemsState";
import type { AlertItem, GameAction, GameEntityReference, GameReference, GameState, QuestItem } from "./gameSystemsTypes";
import type { Company, RecordEntry, Snapshot } from "./types";
import "./game-systems.css";

export function alertAgentPrompt(companyId: string, alert: AlertItem) {
  return operatingPrompt(companyId, text.alertPrompt, `${text.promptAlert}: ${alert.id}${alert.revision == null ? "" : ` / ${alert.revision}`}\n${alert.data.title}`);
}
export function questAgentPrompt(companyId: string, quest: QuestItem) {
  return operatingPrompt(companyId, text.questPrompt, `${text.promptQuest}: ${quest.id} / ${quest.revision}\n${quest.data.title}`);
}
export function safeGameAction(item: { actions?: GameAction[] }, operation: "quest.accept" | "alert.ack") {
  return item.actions?.find((action) => action.operation === operation);
}

export function CommandCenter({ company, owner, state, loading = false, error = "", onReload, onChanged, onSelect, snapshot }: {
  company: Company; companyRevision?: number; owner: boolean; state: GameState | null; loading?: boolean; error?: string;
  onReload: () => Promise<void>; onChanged: () => Promise<void>; onSelect: (record: RecordEntry) => void; onGuide?: () => void;
  snapshot?: Snapshot;
}) {
  const locale = useLocale();
  const credential = getCredential();
  const scope = `${company.id}\u0000${credential}\u0000${locale}`;
  const activeScope = useRef(scope); activeScope.current = scope;
  const [category, setCategory] = useState("");
  const [questStatus, setQuestStatus] = useState("");
  const [alertFilter, setAlertFilter] = useState("active");
  const [selectedAlertId, setSelectedAlertId] = useState<string | null>(null);
  const [localError, setLocalError] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState("");
  const [revealQuest, setRevealQuest] = useState("");
  const pending = useRef(false);
  const stableRequest = useRef(new StableGameRequest());
  const referenceController = useRef<AbortController | null>(null);
  const current = state?.company_id === company.id ? state : null;
  useEffect(() => {
    activeScope.current = scope;
    setCategory(""); setQuestStatus(""); setAlertFilter("active"); setSelectedAlertId(null); setLocalError(""); setNotice(""); setBusy(""); pending.current = false; stableRequest.current.clear();
    return () => { activeScope.current = ""; referenceController.current?.abort(); };
  }, [scope]);
  useEffect(() => {
    if (!revealQuest) return;
    const target = document.getElementById(`quest-${encodeURIComponent(revealQuest)}`);
    target?.scrollIntoView({ block: "center", behavior: "auto" });
    target?.focus({ preventScroll: true });
    setRevealQuest("");
  }, [revealQuest]);
  function showQuest(id: string) { setCategory(""); setQuestStatus(""); setRevealQuest(id); }

  async function execute(action: GameAction, key: string) {
    if (!owner || pending.current) return;
    const mutationScope = scope;
    pending.current = true; setBusy(key); setLocalError(""); setNotice("");
    const requestKey = `${scope}:${action.operation}:${action.expected_revision}:${JSON.stringify(action.input)}`;
    try {
      const result = await gameSystemsApi.operate(company.id, action.operation, action.input, stableRequest.current.get(requestKey), action.expected_revision, credential);
      if (activeScope.current !== mutationScope || getCredential() !== credential) return;
      stableRequest.current.clear();
      if (action.operation === "alert.ack") setNotice(text.acknowledged);
      if (action.operation === "simulation.draw_event") { setSelectedAlertId(result.record?.id || null); setAlertFilter("active"); setNotice(text.drawAgent); }
      await onReload();
      await onChanged().catch(() => { /* Snapshot refresh has its own recovery notice. */ });
    } catch (cause) {
      if (activeScope.current !== mutationScope || getCredential() !== credential) return;
      if (cause instanceof ApiError && cause.status === 409) {
        stableRequest.current.clear(); setLocalError(text.conflict); await onReload();
      } else setLocalError(cause instanceof Error ? cause.message : text.mutationFailed);
    } finally {
      if (activeScope.current === mutationScope) { pending.current = false; setBusy(""); }
    }
  }
  async function openReference(reference: GameReference) {
    referenceController.current?.abort();
    const controller = new AbortController(); referenceController.current = controller;
    setLocalError("");
    try {
      const record = await gameSystemsApi.reference(company.id, reference, controller.signal, credential);
      if (!controller.signal.aborted && activeScope.current === scope && getCredential() === credential) onSelect(record);
    } catch (cause) {
      if (!controller.signal.aborted && activeScope.current === scope) setLocalError(cause instanceof Error ? cause.message : text.referenceUnavailable);
    }
  }
  async function openEntity(reference: GameEntityReference) {
    referenceController.current?.abort();
    const controller = new AbortController(); referenceController.current = controller;
    setLocalError("");
    try {
      const record = await gameSystemsApi.currentReference(company.id, reference, controller.signal, credential);
      if (!controller.signal.aborted && activeScope.current === scope && getCredential() === credential) onSelect(record);
    } catch (cause) {
      if (!controller.signal.aborted && activeScope.current === scope) setLocalError(cause instanceof Error ? cause.message : text.referenceUnavailable);
    }
  }
  const alerts = current ? orderedAlerts(current.alerts.filter((alert) => alertMatches(alert, alertFilter))) : [];
  const selectedAlert = alerts.find((alert) => alert.id === selectedAlertId) || alerts[0];
  const quests = orderedQuests(current?.quests.filter((quest) => questMatches(quest, current.quests, category, questStatus)) || []);
  return <div className="command-center game-system-surface">
    <header className="game-system-heading"><h2><Flag size={24} />{text.command}</h2>{current && <span className="game-command-count">{number(current.quests.filter((quest) => ["open", "accepted"].includes(quest.data.status)).length)} {text.activeQuestCount}</span>}</header>
    {(error || localError) && <ErrorNotice message={localError || error} retry={() => void onReload()} />}
    {notice && <p className="game-action-notice" role="status"><Check size={18} />{notice}</p>}
    {loading && !current ? <Loading /> : current && <>
      <section className="command-rank-band" aria-label={text.progression}><GameProgress state={current} /><span>{text.shownAt} {date(current.as_of)}</span><button className="text-button" onClick={() => document.getElementById("experience-sources")?.scrollIntoView({ block: "start", behavior: "auto" })}>{text.experienceSources}</button></section>
      {current.mode === "simulation" && <section className="game-event-draw" aria-labelledby="event-draw-title"><div className="game-letter-seal" aria-hidden="true"><FileText size={32} /></div><div className="game-event-draw-copy"><h3 id="event-draw-title">{text.drawTitle}</h3><p>{text.drawDescription}</p></div><div className="game-event-draw-actions"><button className="primary-button" disabled={!owner || !current.draw.available || Boolean(busy) || !current.draw.action} onClick={() => { if (current.draw.action?.operation === "simulation.draw_event") void execute(current.draw.action, "draw"); }}>
        {busy === "draw" ? text.drawPending : text.draw}</button>{!current.draw.available && <span>{text.drawCurrent}{current.draw.cooldown_until && <> · {text.drawCooldown} {date(current.draw.cooldown_until)}</>}</span>}</div></section>}
      <section className="game-letters-section" aria-labelledby="game-letters-title"><div className="game-subheading"><h3 id="game-letters-title"><AlertCircle size={22} />{text.alerts}</h3><label><span className="sr-only">{text.alertFilter}</span><select value={alertFilter} onChange={(event) => setAlertFilter(event.target.value)}><option value="active">{text.active}</option><option value="unread">{text.unread}</option><option value="history">{text.history}</option><option value="all">{text.all}</option></select></label></div>
        {alerts.length ? <div className="game-letter-workspace"><ul className="game-letter-list">{alerts.map((alert) => <li key={alert.id}><button className={`game-letter-selector severity-${alert.data.severity}`} aria-pressed={selectedAlert?.id === alert.id} onClick={() => setSelectedAlertId(alert.id)}><span className="game-letter-meta"><span>{text[alert.data.severity]}</span><span>{alert.unread ? text.unread : text.read}</span></span><strong>{alert.data.title}</strong>{alert.data.window_start && <small>{date(alert.data.window_start)}</small>}</button></li>)}</ul>
          {selectedAlert && <AlertLetter alert={selectedAlert} companyId={company.id} quests={current.quests} owner={owner} busy={Boolean(busy)} onAction={(action) => void execute(action, `alert:${selectedAlert.id}`)} onReference={openReference} onEntity={openEntity} onQuest={showQuest} snapshot={snapshot} />}
        </div> : <Empty compact>{text.noAlerts}</Empty>}
      </section>
      <section className="game-quests-section" aria-labelledby="game-quests-title"><div className="game-subheading"><h3 id="game-quests-title"><Flag size={22} />{text.quests}</h3><CopyButton value={operatingPrompt(company.id, text.prepareQuestPrompt)} label={text.prepareQuest} /></div>
        <div className="game-quest-filters"><label><span>{text.category}</span><select value={category} onChange={(event) => setCategory(event.target.value)}><option value="">{text.all}</option>{(["main", "side", "daily"] as const).map((value) => <option key={value} value={value}>{text[value]}</option>)}</select></label><label><span>{text.questState}</span><select value={questStatus} onChange={(event) => setQuestStatus(event.target.value)}><option value="">{text.all}</option>{(["open", "accepted", "blocked", "completed", "failed", "cancelled"] as const).map((value) => <option key={value} value={value}>{text[value]}</option>)}</select></label></div>
        {quests.length ? <ul className="game-quest-list">{quests.map((quest) => <li key={quest.id}><QuestCard quest={quest} companyId={company.id} quests={current.quests} owner={owner} busy={Boolean(busy)} onAction={(action) => void execute(action, `quest:${quest.id}`)} onReference={openReference} onQuest={showQuest} /></li>)}</ul> : <Empty compact>{current.quests.length ? text.noResults : text.noQuests}</Empty>}
      </section>
      <ExperienceJournal state={current} onReference={openReference} />
      {!owner && <p className="game-fine-print">{text.commandsReadOnly}</p>}
    </>}
  </div>;
}

export function AlertLetter({ alert, companyId, quests, owner, busy, onAction, onReference, onQuest, onEntity, snapshot }: {
  alert: AlertItem; companyId: string; quests: QuestItem[]; owner: boolean; busy: boolean;
  onAction: (action: GameAction) => void; onReference: (reference: GameReference) => void;
  onQuest?: (id: string) => void; onEntity?: (reference: GameEntityReference) => void; snapshot?: Snapshot;
}) {
  const { data } = alert;
  const action = safeGameAction(alert, "alert.ack");
  const quest = quests.find((item) => item.id === data.quest_id);
  return <article className={`game-letter-detail severity-${data.severity}`}>
    <div className="game-letter-meta"><span>{text[data.type]} · {text[data.basis]}</span><span>{gameLabel("statuses", data.status)}</span></div>
    <h4>{data.title}</h4><p className="game-letter-body">{data.body}</p>
    {(data.window_start || data.window_end) && <p className="game-period">{date(data.window_start)}{data.window_end && <> – {date(data.window_end)}</>}</p>}
    {quest && <p className="game-letter-related">{text.relatedQuest}: <button className="text-button" onClick={() => onQuest?.(quest.id)}>{quest.data.title}</button></p>}
    {data.entity_refs && data.entity_refs.length > 0 && <div className="game-letter-related"><span>{text.relatedPeople}</span><ul>{data.entity_refs.map((reference, index) => {
      const record = snapshot?.records[reference.kind]?.find((item) => item.id === reference.id);
      const label = alert.related_entities?.find((item) => item.id === reference.id && item.kind === reference.kind)?.label || (record ? str(record.data.name || record.data.title) : "");
      return <li key={`${reference.kind}:${reference.id}`}><button className="text-button" onClick={() => onEntity?.(reference)}>{label || `${gameLabel("nodeTypes", reference.kind)} ${number(index + 1)}`}</button></li>;
    })}</ul></div>}
    {data.assumptions && data.assumptions.length > 0 && <details><summary>{text.assumptions}</summary><ul>{data.assumptions.map((assumption, index) => <li key={index}>{assumption}</li>)}</ul></details>}
    {data.source_refs && data.source_refs.length > 0 && <details className="game-evidence"><summary>{text.evidence}</summary><GameEvidence refs={data.source_refs} onReference={onReference} /></details>}
    <div className="game-letter-actions">{alert.unread && action && <button className="secondary-button" disabled={!owner || busy} onClick={() => onAction(action)}>{text.acknowledge}</button>}<CopyButton value={alertAgentPrompt(companyId, alert)} label={text.copyAlert} /></div>
  </article>;
}

export function QuestCard({ quest, companyId, quests, owner, busy, onAction, onReference, onQuest }: {
  quest: QuestItem; companyId: string; quests: QuestItem[]; owner: boolean; busy: boolean;
  onAction: (action: GameAction) => void; onReference: (reference: GameReference) => void;
  onQuest?: (id: string) => void;
}) {
  const { data } = quest;
  const blocked = quest.blocked || questBlockers(quest, quests).length > 0;
  const action = safeGameAction(quest, "quest.accept");
  const canAccept = owner && !busy && !blocked && data.status === "open" && Boolean(action);
  return <article id={`quest-${encodeURIComponent(quest.id)}`} tabIndex={-1} className={`game-quest-card quest-${data.status}`}>
    <div className="game-quest-mark" aria-hidden="true">{data.status === "completed" && quest.completion_valid !== false ? <Check size={22} /> : <Flag size={22} />}</div>
    <div className="game-quest-main"><div className="game-quest-meta"><span>{text[data.category]}</span><span>{blocked && ["open", "accepted"].includes(data.status) ? text.blocked : text[data.status]}</span>{quest.overdue && <span className="game-overdue">{text.overdue}</span>}{quest.derived && <span>{text.sourceGoal}</span>}</div><h4>{data.title}</h4>{data.description && data.description !== data.outcome && <p>{data.description}</p>}
      {data.outcome && <p className="game-quest-outcome"><strong>{text.outcome}</strong> {data.outcome}</p>}
      <div className="game-quest-timing">{data.deadline && <span>{text.deadline} {date(data.deadline)}</span>}{quest.progress != null && <span>{text.progress} {number(quest.progress, { style: "percent", maximumFractionDigits: 0 })}</span>}</div>
      {data.depends_on && data.depends_on.length > 0 && <details className="game-quest-prerequisites"><summary>{text.dependsOn}</summary><ul>{data.depends_on.map((id) => { const item = quests.find((candidate) => candidate.id === id); return <li key={id}>{item ? <button className="text-button" onClick={() => onQuest?.(id)}>{item.data.title}</button> : text.unavailablePrerequisite}{item && <span> · {text[item.data.status]}</span>}</li>; })}</ul></details>}
      {data.completion?.summary && <p className="game-quest-result">{data.completion.summary}</p>}
      {quest.completion_valid === false && data.status === "completed" && <p className="game-overdue">{text.completionUnverified}</p>}
      {(data.source_refs?.length || data.completion?.result_refs?.length) ? <details className="game-evidence"><summary>{text.evidence}</summary><GameEvidence refs={[...(data.source_refs || []), ...(data.completion?.result_refs || [])].filter((reference, index, refs) => refs.findIndex((item) => item.kind === reference.kind && item.id === reference.id && item.revision === reference.revision) === index)} onReference={onReference} /></details> : null}
      <div className="game-quest-actions">{action && data.status === "open" && <button className="primary-button" disabled={!canAccept} onClick={() => onAction(action)}>{text.accept}</button>}{["open", "accepted"].includes(data.status) && <CopyButton value={questAgentPrompt(companyId, quest)} label={text.copyQuest} />}</div>
    </div>
  </article>;
}

export function ExperienceJournal({ state, onReference }: { state: GameState; onReference: (reference: GameReference) => void }) {
  const p = state.progression;
  const rules = obj(p.rules);
  const ruleRows: { label: string; value: number }[] = [
    { label: text.sourceGoal, value: num(rules.goal_xp) ?? 0 }, { label: gameLabel("awardKinds", "decision"), value: num(rules.decision_xp) ?? 0 },
    { label: gameLabel("awardKinds", "weekly"), value: num(rules.weekly_report_xp) ?? 0 }, { label: gameLabel("awardKinds", "monthly"), value: num(rules.monthly_report_xp) ?? 0 },
    ...Object.entries(obj(rules.quest_xp)).map(([category, value]) => ({ label: `${text.quests} · ${text[category as "main" | "side" | "daily"] || category}`, value: num(value) ?? 0 })),
  ].filter((row) => row.value > 0);
  return <section id="experience-sources" className="game-experience-journal"><div className="game-subheading"><h3>{text.experienceSources}</h3><span>{number(p.awards.length)} {text.sourceCount}</span></div>
    {p.awards.length ? <ol className="game-experience-list">{p.awards.map((award, index) => {
      const refs = [...(award.source_refs || []), ...(award.shared_with || [])].filter((reference, i, all) => all.findIndex((item) => item.kind === reference.kind && item.id === reference.id && item.revision === reference.revision) === i);
      return <li key={award.id || index}><div><strong>{award.title || gameLabel("awardKinds", award.rule || award.kind || award.category || "")}</strong>{refs.length ? <details><summary>{text.evidence}</summary><GameEvidence refs={refs} onReference={onReference} /></details> : null}</div><span>+{number(award.xp)} {text.xp}</span></li>;
    })}</ol> : <Empty compact>{text.noAwards}</Empty>}
    <details className="game-experience-rules"><summary>{text.experienceRules}</summary><dl>{ruleRows.map((rule) => <div key={rule.label}><dt>{rule.label}</dt><dd>{number(rule.value)} {text.xp}</dd></div>)}</dl>{str(rules.meaning) && <p>{str(rules.meaning)}</p>}{str(rules.deduplication) && <p>{str(rules.deduplication)}</p>}
      {arr(rules.thresholds).length > 0 && <ol className="game-level-thresholds">{arr(rules.thresholds).map((value, index) => <li key={index}>{text.level} {number(index + 1)} · {number(num(value) || 0)} {text.xp}</li>)}</ol>}
    </details>
  </section>;
}
