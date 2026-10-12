import { words, phrase } from "./labels";
import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  AlertCircle,
  ArrowRight,
  Check,
  FileText,
  LoaderCircle,
  X,
} from "./icons";
import { copy, recordCopy } from "./copy";
import { api } from "./api";
import { EvidenceTrace } from "./EvidenceTrace";
import { RecordPresentation, recordTitle } from "./RecordPresentation";
import { FilePreview } from "./FilePreview";
import { filePreviewCopy } from "./filePreviewCopy";
import { date, money, num, number, sourceText, str, title } from "./format";
import type { RecordEntry, Snapshot } from "./types";

export function Status({ value }: { value: string }) {
  return (
    <span className={`status status-${value}`}>
      {copy.statuses[value] || value}
    </span>
  );
}
export function Empty({
  children,
  compact = false,
}: {
  children: ReactNode;
  compact?: boolean;
}) {
  return (
    <div className={`empty ${compact ? "compact" : ""}`}>
      <FileText size={24} aria-hidden="true" />
      <p>{children}</p>
    </div>
  );
}
export function Loading() {
  return (
    <div className="loading-state" role="status">
      <LoaderCircle size={22} className="loading-icon" aria-hidden="true" />
      {copy.loading}
    </div>
  );
}
export function ErrorNotice({
  message,
  retry,
}: {
  message: string;
  retry?: () => void;
}) {
  return (
    <div className="error-notice" role="alert">
      <AlertCircle size={20} aria-hidden="true" />
      <span>{message}</span>
      {retry && (
        <button className="text-button" onClick={retry}>
          {copy.retry}
        </button>
      )}
    </div>
  );
}
export function Section({
  title,
  action,
  children,
  className = "",
}: {
  title: string;
  action?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={`section ${className}`}>
      <div className="section-heading">
        <h2>{title}</h2>
        {action}
      </div>
      {children}
    </section>
  );
}
export function GoLink({
  tab,
  children,
}: {
  tab: string;
  children: ReactNode;
}) {
  return (
    <a className="section-link" href={`#${tab}`}>
      {children}
      <ArrowRight size={16} aria-hidden="true" />
    </a>
  );
}
export function RecordButton({
  record,
  onSelect,
}: {
  record: RecordEntry;
  onSelect: (record: RecordEntry) => void;
}) {
  return (
    <button className="record-link" onClick={() => onSelect(record)}>
      {title(record)}
    </button>
  );
}

export function Modal({
  title,
  children,
  onClose,
  wide = false,
  returnFocus,
}: {
  title: string;
  children: ReactNode;
  onClose: () => void;
  wide?: boolean;
  returnFocus?: HTMLElement | null;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const triggerRef = useRef(
    returnFocus || (document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null),
  );
  useEffect(() => {
    const dialog = ref.current;
    dialog?.showModal();
    return () => {
      dialog?.close();
      if (triggerRef.current?.isConnected && triggerRef.current.getClientRects().length
        && !triggerRef.current.closest('[hidden], [inert], dialog:not([open])'))
        triggerRef.current.focus({ preventScroll: true });
    };
  }, []);
  return (
    <dialog
      ref={ref}
      aria-label={title}
      className={`modal ${wide ? "wide" : ""}`}
      onCancel={onClose}
      onClose={(event) => {
        if (!event.currentTarget.open) onClose();
      }}
      onClick={(event) => {
        if (event.target === ref.current) {
          const r = ref.current.getBoundingClientRect();
          if (
            event.clientX < r.left ||
            event.clientX > r.right ||
            event.clientY < r.top ||
            event.clientY > r.bottom
          )
            onClose();
        }
      }}
    >
      <div className="modal-header">
        <h2>{title}</h2>
        <button
          className="icon-button"
          aria-label={words.components026}
          onClick={onClose}
        >
          <X size={20} />
        </button>
      </div>
      {children}
    </dialog>
  );
}

export function RecordDetails({
  record,
  snapshot,
  onClose,
}: {
  record: RecordEntry;
  snapshot: Snapshot;
  onClose: () => void;
}) {
  const [version, setVersion] = useState<RecordEntry | null>(null);
  const [baseline, setBaseline] = useState<Snapshot | null>(null);
  const [baselineLoading, setBaselineLoading] = useState(false);
  const [baselineError, setBaselineError] = useState("");
  const [preview, setPreview] = useState<RecordEntry | null>(null);
  const shown = version || record;
  async function showBaseline() {
    const revision = num(shown.data.baseline_revision);
    if (revision == null) return;
    setBaselineLoading(true);
    setBaselineError("");
    try {
      setBaseline(await api.history(shown.company_id, revision));
    } catch {
      setBaselineError(recordCopy.baselineError);
    } finally {
      setBaselineLoading(false);
    }
  }
  return (
    <Modal title={recordTitle(shown)} onClose={onClose} wide>
      <div className="detail-meta">
        <span>
          {words.components027}
          {shown.revision}
        </span>
        <span>
          {words.components028}
          {date(shown.updated_at)}
        </span>
      </div>
      <RecordPresentation record={shown} snapshot={snapshot} />
      {shown.kind === "file" && <div className="file-detail-actions"><button className="secondary-button" onClick={() => setPreview(shown)}>{filePreviewCopy.preview}</button></div>}
      {preview && <FilePreview record={preview} onClose={() => setPreview(null)} />}
      {num(shown.data.baseline_revision) != null && (
        <button
          className="secondary-button"
          onClick={() => void showBaseline()}
          disabled={baselineLoading}
        >
          {recordCopy.baseline}
        </button>
      )}
      {baselineError && <ErrorNotice message={baselineError} />}
      {baseline && (
        <Modal
          title={recordCopy.baselineTitle}
          onClose={() => setBaseline(null)}
          wide
        >
          <h3>{baseline.company.name}</h3>
          <dl className="detail-fields">
            <div>
              <dt>{recordCopy.dataVersion}</dt>
              <dd>{baseline.metrics.data_revision}</dd>
            </div>
            <div>
              <dt>{recordCopy.dataDate}</dt>
              <dd>{date(baseline.metrics.as_of)}</dd>
            </div>
            <div>
              <dt>{recordCopy.cash}</dt>
              <dd className="number">
                {money(
                  baseline.metrics.cash_balance_minor,
                  baseline.company.currency,
                )}
              </dd>
            </div>
            <div>
              <dt>MRR</dt>
              <dd className="number">
                {money(baseline.metrics.mrr_minor, baseline.company.currency)}
              </dd>
            </div>
          </dl>
          {(baseline.records.goal || []).map((g) => (
            <div className="baseline-goal" key={g.id}>
              <strong>{title(g)}</strong>
              <Status
                value={
                  baseline.metrics.goal_progress.find((p) => p.id === g.id)
                    ?.status || str(g.data.status)
                }
              />
            </div>
          ))}
        </Modal>
      )}
      <EvidenceTrace record={shown} />
      <RecordHistory record={record} onVersion={setVersion} onPreview={setPreview} />
      <details className="raw-record">
        <summary>{words.components032}</summary>
        <pre>{JSON.stringify(shown, null, 2)}</pre>
      </details>
    </Modal>
  );
}
export async function copyText(value: string) {
  if (navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(value);
      return;
    } catch {
      /* Use the native fallback where clipboard permissions are unavailable. */
    }
  }
  const textarea = document.createElement("textarea");
  textarea.value = value;
  document.body.append(textarea);
  textarea.select();
  const copied = document.execCommand("copy");
  textarea.remove();
  if (!copied) throw new Error(copy.copyFailed);
}
export function CopyButton({
  value,
  label = words.components060,
}: {
  value: string;
  label?: string;
}) {
  const ref = useRef<HTMLButtonElement>(null);
  const [failed, setFailed] = useState(false);
  const [copied, setCopied] = useState(false);
  useEffect(() => { setFailed(false); setCopied(false); }, [value, label]);
  async function handleCopy() {
    try {
      await copyText(value);
      setFailed(false);
    } catch {
      setFailed(true);
      return;
    }
    setCopied(true);
  }
  return (
    <button
      ref={ref}
      className="secondary-button copy-button"
      data-copied={copied ? "true" : undefined}
      aria-label={copied ? copy.copied : label}
      onClick={() => void handleCopy()}
    >
      <span className="copy-label">{failed ? copy.copyFailed : label}</span>
      <span className="copied-label">
        <Check size={15} aria-hidden="true" />
        {copy.copied}
      </span>
    </button>
  );
}

function RecordHistory({
  record,
  onVersion,
  onPreview,
}: {
  record: RecordEntry;
  onVersion: (record: RecordEntry) => void;
  onPreview: (record: RecordEntry) => void;
}) {
  const [versions, setVersions] = useState<RecordEntry[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  async function load() {
    setLoading(true);
    setError("");
    try {
      const result = await api.records(
        record.company_id,
        record.kind,
        0,
        true,
        record.id,
      );
      setVersions(
        result.records
          .filter((r) => r.id === record.id)
          .sort((a, b) => b.revision - a.revision),
      );
      setLoaded(true);
    } catch {
      setError(recordCopy.historyError);
    } finally {
      setLoading(false);
    }
  }
  return (
    <details
      className="record-history"
      onToggle={(event) => {
        if (event.currentTarget.open && !loaded && !loading) void load();
      }}
    >
      <summary>{recordCopy.history}</summary>
      {error && <ErrorNotice message={error} retry={() => void load()} />}
      {loading ? (
        <Loading />
      ) : (
        <div className="revision-list">
          {versions.map((r) => (
            <div key={r.revision}>
              <button className="record-link" onClick={() => onVersion(r)}>
                {recordCopy.version} {r.revision}
              </button>
              <span>{date(r.updated_at)}</span>
              {r.kind === "file" && (
                <div className="file-history-actions">
                <button className="text-button" onClick={() => onPreview(r)}>{filePreviewCopy.preview}</button>
                <button
                  className="text-button"
                  onClick={() =>
                    void api
                      .downloadFile(
                        r.company_id,
                        r.id,
                        str(r.data.relative_path).split("/").at(-1) || title(r),
                        r.revision,
                      )
                      .catch(() => setError(recordCopy.historyError))
                  }
                >
                  {recordCopy.downloadVersion}
                </button>
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </details>
  );
}
