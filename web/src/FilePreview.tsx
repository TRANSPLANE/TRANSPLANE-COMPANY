import { useEffect, useRef, useState } from "react";
import { ApiError, getCredential } from "./api";
import { Download, LoaderCircle, X } from "./icons";
import { number, size, str, title } from "./format";
import { useLocale } from "./locale";
import { Markdown } from "./Markdown";
import { downloadPreviewFile, loadFilePreview, type FilePreviewDescriptor, type LoadedFilePreview } from "./filePreviewApi";
import { filePreviewCopy as text } from "./filePreviewCopy";
import type { RecordEntry } from "./types";
import "./file-preview.css";

type PreviewState = {
  key: string;
  result?: LoadedFilePreview;
  url?: string;
  error?: string;
};

export function FilePreview({ record, onClose }: { record: RecordEntry; onClose: () => void }) {
  useLocale();
  const credential = getCredential();
  const key = `${record.company_id}\0${record.id}\0${record.revision}\0${credential}`;
  const [state, setState] = useState<PreviewState>({ key });
  const [attempt, setAttempt] = useState(0);
  const [downloading, setDownloading] = useState(false);
  const [downloadError, setDownloadError] = useState("");
  const dialog = useRef<HTMLDialogElement>(null);
  const trigger = useRef(typeof document !== "undefined" && document.activeElement instanceof HTMLElement ? document.activeElement : null);
  const closeRef = useRef(onClose);
  closeRef.current = onClose;
  const downloadController = useRef<AbortController | null>(null);

  useEffect(() => {
    const element = dialog.current;
    element?.showModal();
    const invalid = () => closeRef.current();
    window.addEventListener("credential-invalid", invalid);
    return () => {
      downloadController.current?.abort();
      window.removeEventListener("credential-invalid", invalid);
      element?.close();
      if (trigger.current?.isConnected) trigger.current.focus({ preventScroll: true });
    };
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    let url: string | undefined;
    setState({ key });
    setDownloadError("");
    downloadController.current?.abort();
    setDownloading(false);
    void loadFilePreview(record, controller.signal, credential)
      .then((result) => {
        if (controller.signal.aborted || credential !== getCredential()) return;
        if (result.blob) url = URL.createObjectURL(result.blob);
        setState({ key, result, url });
      })
      .catch((error: unknown) => {
        if (controller.signal.aborted || credential !== getCredential()) return;
        const message = error instanceof ApiError ? error.message : text.failed;
        setState({ key, error: message });
      });
    return () => {
      controller.abort();
      downloadController.current?.abort();
      if (url) URL.revokeObjectURL(url);
    };
  }, [key, attempt]);

  const visible = state.key === key ? state : { key };
  const descriptor = visible.result?.descriptor;
  const filename = descriptor?.name || str(record.data.relative_path).split("/").at(-1) || title(record);
  const loading = !visible.result && !visible.error;
  async function download() {
    const controller = new AbortController();
    downloadController.current?.abort();
    downloadController.current = controller;
    setDownloading(true);
    setDownloadError("");
    try {
      await downloadPreviewFile(record, controller.signal, credential);
    } catch {
      if (!controller.signal.aborted && credential === getCredential()) setDownloadError(text.downloadFailed);
    } finally {
      if (!controller.signal.aborted && credential === getCredential()) setDownloading(false);
    }
  }
  return (
    <dialog ref={dialog} className="modal wide file-preview-dialog" aria-label={`${text.preview}: ${filename}`}
      onCancel={() => closeRef.current()}
      onClose={(event) => { if (!event.currentTarget.open) closeRef.current(); }}>
      <div className="modal-header">
        <h2>{filename}</h2>
        <button className="icon-button" aria-label={text.close} onClick={onClose}><X size={20} /></button>
      </div>
      <div className="file-preview-meta">
        <span>{text.version} {number(record.revision)}</span>
        {descriptor && <span>{size(descriptor.size_bytes)}</span>}
      </div>
      <div className="file-preview-content" aria-busy={loading}>
        {loading && <div className="file-preview-loading" role="status"><LoaderCircle size={20} aria-hidden="true" />{text.loading}</div>}
        {visible.error && <div className="error-notice" role="alert"><span>{visible.error}</span><button className="text-button" onClick={() => setAttempt((value) => value + 1)}>{text.retry}</button></div>}
        {descriptor && <FilePreviewBody descriptor={descriptor} url={visible.url} filename={filename} />}
      </div>
      {downloadError && <p className="file-preview-error" role="alert">{downloadError}</p>}
      <div className="file-preview-actions">
        <button className="secondary-button" disabled={downloading} onClick={() => void download()}><Download size={16} aria-hidden="true" />{downloading ? text.downloading : text.download}</button>
      </div>
    </dialog>
  );
}

export function FilePreviewBody({ descriptor, url, filename }: { descriptor: FilePreviewDescriptor; url?: string; filename: string }) {
  if (!descriptor.previewable || descriptor.format === "unsupported") {
    const fallback = descriptor.reason === "too_large" ? text.tooLarge : descriptor.reason === "invalid_content" ? text.invalidContent : text.unsupported;
    return <p className="file-preview-fallback">{fallback}</p>;
  }
  switch (descriptor.format) {
    case "markdown": return <Markdown source={descriptor.content_text || ""} />;
    case "text": return <pre className="file-preview-text">{descriptor.content_text}</pre>;
    case "image": return url ? <img className="file-preview-image" src={url} alt={filename} /> : null;
    case "pdf": return url ? <>
      <iframe className="file-preview-pdf" src={url} title={filename} sandbox="allow-same-origin" />
      <p className="file-preview-fallback">{text.pdfFallback}</p>
    </> : null;
    default: return null;
  }
}
