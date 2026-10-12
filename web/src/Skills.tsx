import { useEffect, useRef, useState, type FormEvent, type ReactNode } from "react";
import { ApiError } from "./api";
import { CopyButton, Empty, ErrorNotice, Loading, Section } from "./components";
import { recordCopy } from "./copy";
import { date } from "./format";
import { FileText, Plus, Power, Search, Trash2, X } from "./icons";
import { getLocale, useLocale } from "./locale";
import { Markdown } from "./Markdown";
import { skillsApi, skillReadAddress, type SkillDocument, type SkillMetadata, type SkillMode, type SkillResponse, type SkillWrite } from "./skillsApi";
import { skillsCopy as text } from "./skillsCopy";
import "./skills.css";

const message = (error: unknown, fallback: string) => error instanceof Error ? error.message : fallback;
export function skillDraft(skill?: SkillDocument, copy = false): SkillWrite {
  return {
    id: copy || !skill ? crypto.randomUUID() : skill.id,
    name: skill?.name || "",
    title: skill?.title || "",
    description: skill?.description || "",
    body: skill?.body || "",
    enabled: copy ? true : skill?.enabled ?? true,
    tags: [...(skill?.tags || [])],
    modes: [...(skill?.modes || ["real", "simulation"])],
    type: skill?.type || "procedure",
    source_refs: [...(skill?.source_refs || [])],
    ...(copy && skill?.read_only ? { copy_from: { id: skill.id, revision: skill.revision, content_sha256: skill.content_sha256, language: getLocale() } } : {}),
  };
}
export function skillMatches(skill: SkillMetadata, query: string, status: string) {
  if (status === "enabled" && (!skill.effective || skill.deleted)) return false;
  if (status === "disabled" && (skill.enabled || skill.deleted)) return false;
  if (status === "deleted" && !skill.deleted) return false;
  if (status === "all" && skill.deleted) return false;
  const searchable = `${skill.title} ${skill.name} ${skill.description} ${(skill.tags || []).join(" ")}`.toLocaleLowerCase();
  return searchable.includes(query.trim().toLocaleLowerCase());
}
export function skillDraftChanged(draft: SkillWrite, baseline: SkillWrite) {
  return JSON.stringify(draft) !== JSON.stringify(baseline);
}
function skillDisplayBody(skill: SkillDocument) {
  const heading = skill.body.match(/^\s*# ([^\n]+)\r?\n/);
  return heading && heading[1].trim() === skill.title.trim()
    ? skill.body.slice(heading[0].length)
    : skill.body;
}
export function skillWriteInput(draft: SkillWrite, tagsText: string): SkillWrite {
  return { ...draft, tags: [...new Set(tagsText.split(/[,，]/).map((tag) => tag.trim()).filter(Boolean))] };
}
export function mergeSkillSources(draft: SkillWrite, latest: SkillDocument): SkillWrite {
  const source_refs = [...draft.source_refs];
  for (const ref of latest.source_refs || [])
    if (!source_refs.some((item) => item.kind === ref.kind && item.id === ref.id && item.revision === ref.revision)) source_refs.push(ref);
  return { ...draft, source_refs };
}
export function mergeSkillCopy(draft: SkillWrite, latest: SkillDocument, replace = false): SkillWrite {
  const template = skillDraft(latest, true);
  return replace ? { ...template, id: draft.id } : { ...mergeSkillSources(draft, latest), copy_from: template.copy_from };
}

export function Skills({ owner, companyId, companyRevision, refreshVersion = 0, onChanged }: {
  owner: boolean;
  companyId: string;
  companyRevision: number;
  refreshVersion?: number;
  onChanged: () => Promise<void>;
}) {
  const locale = useLocale();
  const [items, setItems] = useState<SkillMetadata[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState("all");
  const [reader, setReader] = useState<SkillMetadata | null>(null);
  const [editor, setEditor] = useState<{ initial?: SkillDocument; copy?: boolean } | null>(null);
  const [remove, setRemove] = useState<SkillMetadata | null>(null);
  const [busy, setBusy] = useState("");
  const loadVersion = useRef(0);
  const mutationRequest = useRef<{ key: string; id: string } | null>(null);
  const dialogTrigger = useRef<HTMLElement | null>(null);

  async function load(signal?: AbortSignal) {
    const version = ++loadVersion.current;
    try {
      const result = await skillsApi.list(companyId, owner, signal);
      if (signal?.aborted || version !== loadVersion.current) return;
      setItems(result.items || []);
      setError("");
    } catch (error) {
      if (!signal?.aborted && version === loadVersion.current) setError(message(error, text.readError));
    } finally {
      if (!signal?.aborted && version === loadVersion.current) setLoading(false);
    }
  }
  useEffect(() => {
    const controller = new AbortController();
    void load(controller.signal);
    return () => controller.abort();
  }, [companyId, companyRevision, refreshVersion, owner, locale]);
  useEffect(() => { if (!owner) { setEditor(null); setRemove(null); } }, [owner]);

  async function changed() {
    await load();
    await onChanged().catch(() => { /* The company snapshot exposes its own refresh error. */ });
  }
  function requestId(key: string) {
    if (mutationRequest.current?.key !== key) mutationRequest.current = { key, id: crypto.randomUUID() };
    return mutationRequest.current.id;
  }
  async function mutate(skill: SkillMetadata, action: "enable" | "delete") {
    setBusy(skill.id);
    setError("");
    try {
      const id = requestId(`${action}:${skill.id}:${skill.revision}`);
      if (action === "delete") await skillsApi.remove(companyId, skill, id);
      else await skillsApi.setEnabled(companyId, skill, !skill.enabled, id);
      mutationRequest.current = null;
      setRemove(null);
      await changed();
    } catch (error) {
      setError(error instanceof ApiError && error.status === 409 ? text.conflict : message(error, text.saveError));
      if (error instanceof ApiError && error.status === 409) {
        setRemove(null);
        await load();
      }
    } finally { setBusy(""); }
  }
  async function beginEdit(skill: SkillMetadata, copy = false) {
    setBusy(skill.id);
    setError("");
    try {
      const result = await skillsApi.read(companyId, skill.id);
      setReader(null);
      setEditor({ initial: result.skill, copy });
    } catch (error) { setError(message(error, text.readError)); }
    finally { setBusy(""); }
  }
  const filtered = items.filter((skill) => skillMatches(skill, query, status));
  return (
    <Section title={text.heading} className="skills-section" action={owner && (
      <button className="secondary-button" onClick={(event) => { dialogTrigger.current = event.currentTarget; setEditor({}); }}><Plus size={17} aria-hidden="true" />{text.add}</button>
    )}>
      <div className="skills-content">
      <div className="skills-intro"><p>{text.help}</p>
      {!owner && <p>{text.agentReadOnly}</p>}</div>
      <div className="skills-toolbar">
        <label className="search-field"><Search size={17} aria-hidden="true" /><input aria-label={text.search} placeholder={text.search} value={query} onChange={(event) => setQuery(event.target.value)} /></label>
        <select aria-label={text.heading} value={status} onChange={(event) => setStatus(event.target.value)}>
          <option value="all">{text.all}</option><option value="enabled">{text.enabled}</option><option value="disabled">{text.disabled}</option>
          {owner && <option value="deleted">{text.deleted}</option>}
        </select>
        <button className="text-button" onClick={() => void load()} disabled={!!busy}>{text.refresh}</button>
      </div>
      {error && <ErrorNotice message={error} retry={() => void load()} />}
      {loading ? <Loading /> : filtered.length ? (
        <ul className="skill-list">
          {filtered.map((skill) => <li className={`skill-item ${skill.deleted ? "skill-deleted" : ""}`} key={skill.id}>
            <div className="skill-main">
              <div className="skill-heading"><div className="skill-title"><span className="skill-document-icon"><FileText size={20} aria-hidden="true" /></span><button className="record-link" onClick={(event) => { dialogTrigger.current = event.currentTarget; setReader(skill); }}>{skill.title || skill.name}</button></div><SkillState skill={skill} /></div>
              {skill.description && <p>{skill.description}</p>}
              <SkillMeta skill={skill} />
            </div>
            {owner && <div className="skill-actions">
              <button className="secondary-button" disabled={!!busy} onClick={(event) => { dialogTrigger.current = event.currentTarget; void beginEdit(skill, skill.read_only); }}>{skill.read_only ? text.copy : skill.deleted ? text.restore : text.edit}</button>
              {!skill.read_only && !skill.deleted && <>
                <button className="text-button" disabled={!!busy} onClick={() => void mutate(skill, "enable")}><Power size={15} aria-hidden="true" />{skill.enabled ? text.disable : text.enable}</button>
                <button className="icon-button danger" disabled={!!busy} aria-label={`${text.remove} ${skill.title || skill.name}`} onClick={(event) => { dialogTrigger.current = event.currentTarget; setRemove(skill); }}><Trash2 size={17} aria-hidden="true" /></button>
              </>}
            </div>}
          </li>)}
        </ul>
      ) : <Empty compact>{!items.length ? text.empty : text.noResults}</Empty>}
      </div>
      {reader && <SkillReader key={`${reader.id}:${reader.revision}`} companyId={companyId} metadata={reader} owner={owner} returnFocus={dialogTrigger.current} onClose={() => setReader(null)} onEdit={(copy) => void beginEdit(reader, copy)} />}
      {editor && <SkillEditor key={editor.initial?.id || "new"} companyId={companyId} initial={editor.initial} copy={editor.copy} returnFocus={dialogTrigger.current} latestRevision={items.find((skill) => skill.id === editor.initial?.id)?.revision} onClose={() => setEditor(null)} onSaved={async () => { setEditor(null); await changed(); }} />}
      {remove && <SkillDialog title={text.deleteTitle} returnFocus={dialogTrigger.current} onClose={() => !busy && setRemove(null)}>
        <p>{remove.title || remove.name}</p><p className="field-help">{text.deleteHelp}</p>
        <div className="form-actions"><button className="secondary-button" onClick={() => setRemove(null)} disabled={!!busy}>{text.cancel}</button><button className="danger-button" disabled={!!busy} onClick={() => void mutate(remove, "delete")}>{text.remove}</button></div>
      </SkillDialog>}
    </Section>
  );
}

function SkillState({ skill }: { skill: SkillMetadata }) {
  const status = skill.deleted ? "deleted" : skill.enabled ? "enabled" : "disabled";
  return <span className={`status status-${status}`}>{text[status]}</span>;
}
function SkillMeta({ skill }: { skill: SkillMetadata }) {
  return <div className="skill-meta">
    <span>{text[skill.source]}{skill.plugin_id ? ` · ${skill.plugin_id}` : ""}</span>
    <span>{text[skill.type || "procedure"]}</span>
    {(skill.modes || []).map((mode) => <span key={mode}>{text[mode]}</span>)}
    {(skill.tags || []).map((tag) => <span key={tag}>{tag}</span>)}
    {!skill.effective && skill.enabled && !skill.deleted && <span>{text.unavailable}</span>}
  </div>;
}

export function SkillDialog({ title, children, onClose, className = "", returnFocus }: { title: string; children: ReactNode; onClose: () => void; className?: string; returnFocus?: HTMLElement | null }) {
  const ref = useRef<HTMLDialogElement>(null);
  const trigger = useRef(returnFocus || (typeof document !== "undefined" && document.activeElement instanceof HTMLElement ? document.activeElement : null));
  useEffect(() => {
    const dialog = ref.current;
    dialog?.showModal();
    return () => { dialog?.close(); if (trigger.current?.isConnected) trigger.current.focus({ preventScroll: true }); };
  }, []);
  return <dialog ref={ref} className={`modal wide skill-dialog ${className}`} aria-label={title} onCancel={(event) => { event.preventDefault(); onClose(); }}>
    <div className="modal-header"><h2>{title}</h2><button className="icon-button" aria-label={text.close} onClick={onClose}><X size={20} aria-hidden="true" /></button></div>{children}
  </dialog>;
}

function SkillReader({ companyId, metadata, owner, onClose, onEdit, returnFocus }: {
  companyId: string; metadata: SkillMetadata; owner: boolean; onClose: () => void; onEdit: (copy: boolean) => void; returnFocus?: HTMLElement | null;
}) {
  const [loaded, setLoaded] = useState<SkillResponse | null>(null);
  const [revision, setRevision] = useState(metadata.revision);
  const [versions, setVersions] = useState<SkillMetadata[]>([]);
  const [historyLoaded, setHistoryLoaded] = useState(false);
  const [historyBusy, setHistoryBusy] = useState(false);
  const [error, setError] = useState("");
  const [historyError, setHistoryError] = useState("");
  const [loading, setLoading] = useState(true);
  const locale = useLocale();
  async function load(signal?: AbortSignal) {
    setLoading(true); setError("");
    try {
      const response = await skillsApi.read(companyId, metadata.id, revision, signal);
      if (!signal?.aborted) setLoaded(response);
    } catch (error) { if (!signal?.aborted) setError(message(error, text.readError)); }
    finally { if (!signal?.aborted) setLoading(false); }
  }
  useEffect(() => {
    const controller = new AbortController(); void load(controller.signal); return () => controller.abort();
  }, [companyId, metadata.id, revision, locale]);
  async function history() {
    if (historyLoaded || historyBusy) return;
    setHistoryBusy(true); setHistoryError("");
    try { const result = await skillsApi.history(companyId, metadata.id); setVersions(result.items.sort((a, b) => b.revision - a.revision)); setHistoryLoaded(true); }
    catch (error) { setHistoryError(message(error, text.readError)); }
    finally { setHistoryBusy(false); }
  }
  return <SkillDialog title={loaded?.skill.title || metadata.title || metadata.name} onClose={onClose} returnFocus={returnFocus}>
    {error && <ErrorNotice message={error} retry={() => void load()} />}
    {loading ? <Loading /> : loaded && <>
      <div className="skill-document-meta"><span>{text.version} {loaded.skill.revision}</span><SkillState skill={loaded.skill} />{loaded.skill.updated_at && <span>{text.updated} {date(loaded.skill.updated_at)}</span>}</div>
      {loaded.skill.description && <p className="skill-document-description">{loaded.skill.description}</p>}
      <SkillMeta skill={loaded.skill} />
      <div className="skill-document-body"><Markdown source={skillDisplayBody(loaded.skill)} /></div>
      {!!loaded.skill.source_refs?.length && <div className="skill-sources"><strong>{text.sources}</strong><ul>{loaded.skill.source_refs.map((ref) => <li key={`${ref.kind}:${ref.id}:${ref.revision}`}>{recordCopy[ref.kind]} · {ref.id} · {text.version} {ref.revision}</li>)}</ul></div>}
      {loaded.skill.copied_from && <div className="skill-sources"><strong>{text.origin}</strong><p>{loaded.skill.copied_from.id} · {text.version} {loaded.skill.copied_from.revision}{loaded.skill.copied_from.plugin_version ? ` · ${loaded.skill.copied_from.plugin_version}` : ""}</p><details className="skill-document-raw"><summary>{text.originalDocument}</summary><pre>{loaded.skill.copied_from.document}</pre></details></div>}
      <div className="skill-document-actions">
        <CopyButton label={text.copyAddress} value={skillReadAddress(companyId, metadata.id, location.origin, loaded.skill.revision)} />
        {owner && <button className="secondary-button" onClick={() => onEdit(metadata.read_only)}>{metadata.read_only ? text.copy : metadata.deleted ? text.restore : text.edit}</button>}
      </div>
      <details className="skill-document-raw"><summary>{text.rawDocument}</summary><pre>{loaded.document}</pre></details>
      {!metadata.read_only && <details className="skill-history" onToggle={(event) => { if (event.currentTarget.open) void history(); }}>
        <summary>{text.history}</summary>
        {historyError && <ErrorNotice message={historyError} retry={() => void history()} />}
        {historyBusy ? <Loading /> : <div className="revision-list">{versions.map((version) => <div key={version.revision}><button className="record-link" onClick={() => setRevision(version.revision)}>{text.version} {version.revision}</button><span>{version.updated_at && date(version.updated_at)}</span><SkillState skill={version} /></div>)}</div>}
      </details>}
    </>}
  </SkillDialog>;
}

export function SkillEditor({ companyId, initial, copy = false, latestRevision, onClose, onSaved, returnFocus }: {
  companyId: string; initial?: SkillDocument; copy?: boolean; latestRevision?: number; onClose: () => void; onSaved: () => Promise<void>; returnFocus?: HTMLElement | null;
}) {
  useLocale();
  const [draft, setDraft] = useState(() => skillDraft(initial, copy));
  const [baseline, setBaseline] = useState(() => skillDraft(initial, copy));
  const [expectedRevision, setExpectedRevision] = useState(copy ? undefined : initial?.revision);
  const [tagsText, setTagsText] = useState((initial?.tags || []).join(", "));
  const [preview, setPreview] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [conflict, setConflict] = useState(false);
  const [discard, setDiscard] = useState(false);
  const [latest, setLatest] = useState<SkillResponse | null>(null);
  const [latestBusy, setLatestBusy] = useState(false);
  const request = useRef<{ key: string; id: string } | null>(null);
  // New drafts use a stable identifier for retries and dirty detection.
  const dirty = skillDraftChanged(skillWriteInput(draft, tagsText), { ...baseline, id: draft.id });
  const stale = !copy && expectedRevision != null && latestRevision != null && latestRevision > expectedRevision;
  function change<K extends keyof SkillWrite>(key: K, value: SkillWrite[K]) { setDraft((current) => ({ ...current, [key]: value })); }
  function close() { if (busy) return; if (dirty) setDiscard(true); else onClose(); }
  async function loadLatest() {
    if (!initial) return;
    setLatestBusy(true);
    try { setLatest(await skillsApi.read(companyId, initial.id)); setError(""); }
    catch (error) { setError(message(error, text.readError)); }
    finally { setLatestBusy(false); }
  }
  function merge(replace: boolean) {
    if (!latest) return;
    if (copy) {
      setDraft((draft) => mergeSkillCopy(draft, latest.skill, replace));
      if (replace) setTagsText((latest.skill.tags || []).join(", "));
      setBaseline({ ...skillDraft(latest.skill, true), id: draft.id });
      setExpectedRevision(undefined); setConflict(false); setLatest(null); setError(""); return;
    }
    const current = skillDraft(latest.skill);
    if (replace) { setDraft(current); setTagsText(current.tags.join(", ")); }
    else setDraft((draft) => mergeSkillSources(draft, latest.skill));
    setBaseline(current); setExpectedRevision(latest.skill.revision); setConflict(false); setLatest(null); setError("");
  }
  function independentCopy() {
    setDraft(({ copy_from: _source, ...draft }) => draft);
    setConflict(false); setLatest(null); setError("");
  }
  async function save(event: FormEvent) {
    event.preventDefault(); setError("");
    if (!/^[a-z0-9][a-z0-9-]{0,62}$/.test(draft.name)) { setError(text.nameInvalid); return; }
    if (!draft.modes.length) { setError(text.modeRequired); return; }
    if (!draft.body.trim()) { setError(text.bodyRequired); setPreview(false); return; }
    if (draft.body.includes("\0") || draft.body.trimStart().startsWith("---") || new TextEncoder().encode(draft.body).length > 128 * 1024) { setError(text.bodyInvalid); setPreview(false); return; }
    const input = skillWriteInput(draft, tagsText);
    const key = JSON.stringify({ input, expectedRevision });
    if (request.current?.key !== key) request.current = { key, id: crypto.randomUUID() };
    setBusy(true);
    try { await skillsApi.save(companyId, input, expectedRevision, request.current.id); await onSaved(); }
    catch (error) {
      if (error instanceof ApiError && error.status === 409) { setConflict(true); setError(draft.copy_from ? text.copyConflict : text.conflict); }
      else setError(message(error, text.saveError));
    } finally { setBusy(false); }
  }
  return <SkillDialog title={initial && !copy ? text.edit : text.add} onClose={close} returnFocus={returnFocus} className="skill-editor">
    {discard && <div className="skill-discard" role="alert"><strong>{text.discardTitle}</strong><p>{text.discardHelp}</p><div className="skill-conflict-actions"><button className="secondary-button" onClick={() => setDiscard(false)}>{text.keepEditing}</button><button className="danger-button" onClick={onClose}>{text.discard}</button></div></div>}
    {error && <ErrorNotice message={error} />}
    {initial?.deleted && !copy && <p className="field-help">{text.editingDeleted}</p>}
    {(stale || conflict) && <div className="skill-conflict" role="status"><p>{conflict ? draft.copy_from ? text.copyConflict : text.conflict : text.stale}</p><button className="secondary-button" disabled={latestBusy || busy} onClick={() => void loadLatest()}>{latestBusy ? text.pendingMerge : text.loadLatest}</button>
      {draft.copy_from && <button className="text-button" disabled={busy} onClick={independentCopy}>{text.independentCopy}</button>}
      {latest && <><details className="skill-latest" open><summary>{text.latest} · {text.version} {latest.skill.revision}</summary><h3>{latest.skill.title}</h3><p>{latest.skill.description}</p><Markdown source={latest.skill.body} /></details><div className="skill-conflict-actions"><button className="secondary-button" onClick={() => merge(false)} disabled={busy}>{copy ? text.keepCopyDraft : text.keepDraft}</button><button className="text-button" onClick={() => merge(true)} disabled={busy}>{text.replaceDraft}</button></div></>}
    </div>}
    <form onSubmit={save} className="stack-form">
      <fieldset disabled={busy}>
        <div className="skill-editor-columns">
          <label>{text.title}<input value={draft.title} onChange={(event) => change("title", event.target.value)} required /></label>
          <label>{text.name}<input value={draft.name} onChange={(event) => change("name", event.target.value)} required maxLength={63} autoCapitalize="none" spellCheck={false} /><span className="skill-field-help">{text.nameHint}</span></label>
        </div>
        <label>{text.description}<textarea value={draft.description} onChange={(event) => change("description", event.target.value)} rows={3} required /></label>
        <div className="skill-editor-columns">
          <label>{text.type}<select value={draft.type} onChange={(event) => change("type", event.target.value as SkillWrite["type"])}><option value="procedure">{text.procedure}</option><option value="experience">{text.experience}</option></select></label>
          <label>{text.tags}<input value={tagsText} onChange={(event) => setTagsText(event.target.value)} placeholder={text.tagsHint} /></label>
        </div>
        <fieldset><legend>{text.modes}</legend><div className="skill-mode-options">{(["real", "simulation"] as SkillMode[]).map((mode) => <label key={mode}><input type="checkbox" checked={draft.modes.includes(mode)} onChange={(event) => change("modes", event.target.checked ? [...draft.modes, mode] : draft.modes.filter((entry) => entry !== mode))} />{text[mode]}</label>)}</div></fieldset>
        <div className="skill-body-field"><div className="skill-body-controls"><span>{text.body}</span><button className="text-button" type="button" aria-pressed={preview} onClick={() => setPreview(!preview)}>{preview ? text.write : text.preview}</button></div>
        <p className="skill-field-help">{text.bodyHint}</p>
        {preview && <div className="skill-preview"><Markdown source={draft.body} /></div>}
        <textarea className="skill-body-input" aria-label={text.body} hidden={preview} value={draft.body} onChange={(event) => change("body", event.target.value)} rows={14} required={!preview} />
        </div>
      </fieldset>
      <div className="form-actions"><button type="button" className="secondary-button" disabled={busy} onClick={close}>{text.cancel}</button><button className="primary-button" disabled={busy || stale || conflict}>{busy ? text.saving : text.save}</button></div>
    </form>
  </SkillDialog>;
}
