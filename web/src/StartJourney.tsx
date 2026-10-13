import { useEffect, useRef, useState, type FormEvent } from "react";
import { ApiError, getCredential } from "./api";
import { CopyButton, ErrorNotice } from "./components";
import { ArrowRight, Building2, Check, Flag, GitBranch, X } from "./icons";
import { useLocale } from "./locale";
import type { Company } from "./types";
import { forgetJourney, journeyAckCurrent, journeyApi, journeyAttempt, journeyDraft, journeyEntryAddress, journeyErrors, journeyPrompt, recoverJourney, rememberJourney, type JourneyAttempt, type JourneyDraft, type JourneyMode, type JourneyTheme } from "./journeyApi";
import { journeyCopy as text } from "./journeyCopy";
import "./journey.css";
import { studioCopy } from "./studioCopy";

export interface StartJourneyProps {
  initialMode?: JourneyMode;
  owner?: boolean;
  credential?: string;
  onCreated: (company: Company) => Promise<void>;
  onClose: () => void;
  onAuthorize?: () => void;
  onStudio?: () => void;
  returnFocus?: HTMLElement | null;
}
const errorMessage = (error: unknown, fallback: string) => error instanceof Error ? error.message : fallback;

export function WelcomeJourney({ owner, onStart, onHelp }: {
  owner: boolean; onStart: (mode: JourneyMode) => void; onHelp: () => void;
}) {
  useLocale();
  const origin = typeof location === "undefined" ? "http://localhost" : location.origin;
  return <section className="welcome-journey" aria-labelledby="welcome-journey-title">
    <div className="journey-welcome-heading"><Flag size={32} /><h1 id="welcome-journey-title">{owner ? text.welcome : text.readonlyTitle}</h1></div>
    <p>{owner ? text.welcomeHelp : text.readonly}</p>
    {owner && <div className="welcome-mode-actions">
      <button className="welcome-mode-button" onClick={() => onStart("simulation")}><GitBranch size={28} /><span><strong>{text.simulation}</strong><span>{text.simulationHelp}</span></span><ArrowRight size={24} /></button>
      <button className="welcome-mode-button" onClick={() => onStart("real")}><Building2 size={28} /><span><strong>{text.real}</strong><span>{text.realHelp}</span></span><ArrowRight size={24} /></button>
    </div>}
    <div className="journey-welcome-links"><button className="text-button" onClick={onHelp}>{text.help}</button>{!owner && <CopyButton label={text.copyEntry} value={journeyEntryAddress(origin)} />}</div>
  </section>;
}

export function restoreJourneyFocus(target?: HTMLElement | null) {
  if (target?.isConnected && target.getClientRects().length && !target.closest('[hidden], [inert], dialog:not([open])')) target.focus({ preventScroll: true });
}

export function StartJourney({ initialMode, owner = true, credential = getCredential(), onCreated, onClose, onAuthorize, onStudio, returnFocus }: StartJourneyProps) {
  useLocale();
  const recovery = useRef(recoverJourney(credential));
  const [step, setStep] = useState(initialMode || recovery.current ? 1 : 0);
  const [draft, setDraft] = useState(() => recovery.current?.draft || journeyDraft(initialMode));
  const baseline = useRef(JSON.stringify(draft));
  const [errors, setErrors] = useState<Partial<Record<keyof JourneyDraft, string>>>({});
  const [error, setError] = useState("");
  const [uncertain, setUncertain] = useState(!!recovery.current);
  const [pending, setPending] = useState(false);
  const pendingRef = useRef(false);
  const attempt = useRef<JourneyAttempt | null>(recovery.current?.attempt || null);
  const [created, setCreated] = useState<Company | null>(null);
  const [connected, setConnected] = useState(false);
  const [discard, setDiscard] = useState(false);
  const dialog = useRef<HTMLDialogElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const mounted = useRef(true);
  const session = useRef({ owner, credential });
  session.current = { owner, credential };
  const triggerFocus = useRef<HTMLElement | null>(null);
  const logicalFocus = useRef(returnFocus);
  logicalFocus.current = returnFocus;
  const closeTrigger = useRef<HTMLElement | null>(null);
  const discardKeep = useRef<HTMLButtonElement>(null);
  const origin = typeof location === "undefined" ? "http://localhost" : location.origin;
  const dirty = JSON.stringify(draft) !== baseline.current;
  const locked = pending || uncertain;
  const preview = {
    custom: null,
    bootstrap: { resources: text.bootstrapResources, conflict: text.bootstrapConflict },
    launch: { resources: text.launchResources, conflict: text.launchConflict },
    runway: { resources: text.runwayResources, conflict: text.runwayConflict },
  }[draft.theme];

  useEffect(() => {
    mounted.current = true;
    triggerFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dialog.current?.showModal();
    return () => { mounted.current = false; dialog.current?.close(); restoreJourneyFocus(logicalFocus.current || triggerFocus.current); };
  }, []);
  useEffect(() => {
    heading.current?.focus({ preventScroll: true });
    dialog.current?.scrollTo({ top: 0 });
  }, [step]);
  useEffect(() => { if (discard) discardKeep.current?.focus(); }, [discard]);

  function change<K extends keyof JourneyDraft>(key: K, value: JourneyDraft[K]) {
    if (pendingRef.current || uncertain || !owner) return;
    setDraft((current) => ({ ...current, [key]: value, ...(key === "confirmed" ? {} : { confirmed: false }) }));
    setErrors((current) => ({ ...current, [key]: undefined }));
  }
  function close() {
    if (pendingRef.current) return;
    if (!created && (dirty || uncertain)) {
      closeTrigger.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      setDiscard(true);
    } else onClose();
  }
  function keep() { setDiscard(false); restoreJourneyFocus(closeTrigger.current); }
  function current(sentCredential: string) {
    return journeyAckCurrent(session.current.owner, sentCredential, getCredential(), mounted.current) && sentCredential === session.current.credential;
  }
  async function enter(company = created, closeAfter = true) {
    if (!company || pendingRef.current) return;
    const sentCredential = credential;
    pendingRef.current = true; setPending(true); setError("");
    try {
      if (!connected) await onCreated(company);
      if (!current(sentCredential)) return;
      setConnected(true);
      if (closeAfter) onClose();
    } catch { if (current(sentCredential)) setError(text.updatedError); }
    finally { pendingRef.current = false; if (mounted.current) setPending(false); }
  }
  async function submit(event: FormEvent) {
    event.preventDefault();
    if (pendingRef.current || created) return;
    if (!owner) { setError(text.readonlyCreate); return; }
    const fieldErrors = journeyErrors(draft);
    setErrors(fieldErrors);
    if (Object.keys(fieldErrors).length) {
      if (fieldErrors.currency || fieldErrors.timezone || fieldErrors.opening_date) {
        const settings = dialog.current?.querySelector<HTMLDetailsElement>(".journey-settings"); if (settings) settings.open = true;
      }
      requestAnimationFrame(() => dialog.current?.querySelector<HTMLElement>("[aria-invalid='true']")?.focus());
      return;
    }
    const sentCredential = credential;
    // An unconfirmed response freezes the exact request, including its company ID.
    const operation = uncertain && attempt.current ? attempt.current : journeyAttempt(draft, attempt.current);
    attempt.current = operation;
    rememberJourney(sentCredential, draft, operation);
    pendingRef.current = true; setPending(true); setError("");
    try {
      const result = await journeyApi.create(operation, sentCredential);
      forgetJourney(sentCredential, operation.request_id);
      if (!current(sentCredential)) { if (mounted.current) setError(text.credentialChanged); return; }
      setCreated(result.company); setUncertain(false); setStep(2);
      try {
        await onCreated(result.company);
        if (current(sentCredential)) setConnected(true);
      } catch { if (current(sentCredential)) setError(text.updatedError); }
    } catch (error) {
      if (!current(sentCredential)) { if (mounted.current) setError(text.credentialChanged); return; }
      // A transport/server failure may follow a committed write. Exact replay is safe.
      const ambiguous = !(error instanceof ApiError && error.status >= 400 && error.status < 500);
      if (!ambiguous) forgetJourney(sentCredential, operation.request_id);
      setUncertain(ambiguous);
      setError(errorMessage(error, text.createError));
    } finally { pendingRef.current = false; if (mounted.current) setPending(false); }
  }
  const labels = [text.step1, text.step2, text.step3];
  const title = !owner ? text.readonlyTitle : step === 0 ? text.choose : step === 1 ? text.dossier : text.created;
  return <dialog ref={dialog} className="modal journey-dialog" aria-labelledby="journey-dialog-title" onCancel={(event) => { event.preventDefault(); close(); }} onClick={(event) => {
    if (event.target !== event.currentTarget) return;
    const bounds = event.currentTarget.getBoundingClientRect();
    if (event.clientX < bounds.left || event.clientX > bounds.right || event.clientY < bounds.top || event.clientY > bounds.bottom) close();
  }}>
    <div className="modal-header"><h2 id="journey-dialog-title" ref={heading} tabIndex={-1}>{title}</h2><button className="icon-button" disabled={pending} aria-label={text.close} onClick={close}><X size={22} /></button></div>
    <div className="journey-content">
      {owner && <ol className="journey-steps" aria-label={text.start}>{labels.map((label, index) => <li key={index} aria-current={index === step ? "step" : undefined} className={index < step ? "journey-step-done" : ""}>{index < step ? <Check size={18} /> : <span className="journey-step-marker" aria-hidden="true" />}{label}</li>)}</ol>}
      {!owner ? <><p>{text.readonly}</p><CopyButton label={text.copyEntry} value={journeyEntryAddress(origin)} /></> : <>
        {error && <ErrorNotice message={error} />}
        {uncertain && <p className="journey-warning" role="status">{text.uncertain}</p>}
        {step === 0 && <>
          <div className="journey-mode-field">{(["simulation", "real"] as const).map((mode) => <button type="button" key={mode} className="journey-mode-option" onClick={() => { change("mode", mode); setStep(1); }}>
            {mode === "simulation" ? <GitBranch size={28} aria-hidden="true" /> : <Building2 size={28} aria-hidden="true" />}<span><strong>{text[mode]}</strong><span>{text[`${mode}Help`]}</span></span><ArrowRight size={22} aria-hidden="true" />
          </button>)}</div>
          <div className="journey-footer"><button className="text-button" onClick={close}>{text.cancel}</button></div>
        </>}
        {step === 1 && <form className="journey-form" noValidate onSubmit={(event) => void submit(event)}>
          <fieldset disabled={locked}>
            <div className="journey-mode-summary"><span>{text.mode}</span><strong>{text[draft.mode]}</strong></div>
            {draft.mode === "simulation" && <fieldset className="journey-theme-field"><legend>{text.scenario}</legend><p>{text.scenarioHelp}</p>{(["custom", "bootstrap", "launch", "runway"] as JourneyTheme[]).map((theme) => <label key={theme} className="journey-theme-option"><input type="radio" name="journey-theme" checked={draft.theme === theme} onChange={() => change("theme", theme)} /><span><strong>{text[theme]}</strong><span>{text[`${theme}Help`]}</span></span></label>)}{preview && <section className="journey-theme-preview" aria-live="polite" aria-labelledby="journey-preview-title"><h3 id="journey-preview-title">{text.preview}</h3><dl><div><dt>{text.previewResources}</dt><dd>{preview.resources}</dd></div><div><dt>{text.previewConflict}</dt><dd>{preview.conflict}</dd></div></dl></section>}</fieldset>}
            <label className="journey-field"><span>{text.name}</span><input required value={draft.name} autoComplete="organization" maxLength={240} placeholder={text.namePlaceholder} aria-invalid={errors.name ? true : undefined} aria-describedby={errors.name ? "journey-name-error" : undefined} onChange={(event) => change("name", event.target.value)} />{errors.name && <span className="journey-field-error" id="journey-name-error">{errors.name}</span>}</label>
            <label className="journey-field"><span>{text.description}</span><textarea value={draft.description} rows={3} maxLength={8000} placeholder={draft.mode === "simulation" ? text.simulationDescription : text.realDescription} onChange={(event) => change("description", event.target.value)} /></label>
            <details className="journey-settings"><summary>{text.settings}<span>{draft.currency} · {draft.timezone} · {draft.opening_date}</span></summary><p>{text.settingsHelp}</p>
              <label className="journey-field"><span>{text.currency}</span><input value={draft.currency} maxLength={3} autoCapitalize="characters" spellCheck={false} aria-invalid={errors.currency ? true : undefined} aria-describedby={errors.currency ? "journey-currency-error" : undefined} onChange={(event) => change("currency", event.target.value.toUpperCase())} />{errors.currency && <span className="journey-field-error" id="journey-currency-error">{errors.currency}</span>}</label>
              <label className="journey-field"><span>{text.timezone}</span><input value={draft.timezone} list="journey-timezones" spellCheck={false} aria-invalid={errors.timezone ? true : undefined} aria-describedby={errors.timezone ? "journey-timezone-error" : undefined} onChange={(event) => change("timezone", event.target.value)} /><datalist id="journey-timezones"><option value="Asia/Shanghai" /><option value="UTC" /><option value="America/New_York" /><option value="Europe/London" /><option value="Asia/Tokyo" /></datalist>{errors.timezone && <span className="journey-field-error" id="journey-timezone-error">{errors.timezone}</span>}</label>
              <label className="journey-field"><span>{text.opening}</span><input type="date" value={draft.opening_date} aria-invalid={errors.opening_date ? true : undefined} aria-describedby={errors.opening_date ? "journey-date-error" : undefined} onChange={(event) => change("opening_date", event.target.value)} />{errors.opening_date && <span className="journey-field-error" id="journey-date-error">{errors.opening_date}</span>}</label>
            </details>
            <label className="journey-confirm"><input required type="checkbox" checked={draft.confirmed} aria-invalid={errors.confirmed ? true : undefined} aria-describedby={errors.confirmed ? "journey-confirm-error" : undefined} onChange={(event) => change("confirmed", event.target.checked)} /><span>{text.confirm}</span></label>{errors.confirmed && <span className="journey-field-error" id="journey-confirm-error">{errors.confirmed}</span>}
          </fieldset>
          <div className="journey-footer"><button type="submit" className="primary-button" disabled={pending}>{pending ? text.creating : uncertain ? text.retry : text.create}</button><button type="button" className="secondary-button" disabled={locked} onClick={() => setStep(0)}>{text.back}</button><button type="button" className="text-button" disabled={pending} onClick={close}>{text.cancel}</button></div>
        </form>}
        {step === 2 && created && <div className="journey-handoff">
          <div className="journey-company"><Building2 size={28} /><div><strong>{created.name}</strong><span>{created.mode === "simulation" ? text.simulation : text.real} · {created.currency} · {created.opening_date}</span></div></div>
          {created.mode === "simulation" && created.currency === "CNY" && onStudio && <div className="journey-footer"><button className="primary-button" disabled={pending || !connected} onClick={onStudio}>{studioCopy.title}<ArrowRight size={18} aria-hidden="true" /></button></div>}
          <p>{text.handoffHelp}</p>
          <CopyButton label={text.copyPrompt} value={journeyPrompt(created, origin)} />
          <p className="journey-copy-note">{text.notSent}</p>
          <details className="journey-prompt"><summary>{text.viewPrompt}</summary><pre>{journeyPrompt(created, origin)}</pre></details>
          <div className="journey-agent-access"><p>{text.grantHelp}</p>{onAuthorize && <button className="text-button" disabled={pending || !connected} onClick={() => { onClose(); onAuthorize(); }}>{text.grant}</button>}</div>
          <div className="journey-footer"><button className="primary-button" disabled={pending} onClick={() => void enter()}>{text.enter}<ArrowRight size={18} /></button><button className="text-button" disabled={pending} onClick={close}>{text.close}</button></div>
        </div>}
      </>}
      {discard && <section className="journey-discard" aria-labelledby="journey-discard-title"><strong id="journey-discard-title">{text.discardTitle}</strong><p>{uncertain ? text.discardUncertain : text.discardHelp}</p><div className="journey-footer"><button ref={discardKeep} className="secondary-button" onClick={keep}>{text.keep}</button><button className="text-button" onClick={onClose}>{text.discard}</button></div></section>}
    </div>
  </dialog>;
}
