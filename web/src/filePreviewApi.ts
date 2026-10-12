import { ApiError, getCredential } from "./api";
import { getLocale } from "./locale";
import { filePreviewCopy as text } from "./filePreviewCopy";
import type { RecordEntry } from "./types";

export interface FilePreviewDescriptor {
  file_id: string;
  revision: number;
  name: string;
  size_bytes: number;
  sha256: string;
  format: "markdown" | "text" | "image" | "pdf" | "unsupported";
  mime_type: string;
  previewable: boolean;
  reason?: "unsupported_format" | "too_large" | "invalid_content";
  content_text?: string;
  content_url?: string;
  download_url: string;
}
export interface LoadedFilePreview {
  descriptor: FilePreviewDescriptor;
  blob?: Blob;
}
export const textPreviewLimit = 1024 * 1024;
export const binaryPreviewLimit = 10 * 1024 * 1024;
const rasterTypes = new Set(["image/png", "image/jpeg", "image/gif", "image/webp"]);

function filePath(record: RecordEntry) {
  return `/api/v1/companies/${encodeURIComponent(record.company_id)}/files/${encodeURIComponent(record.id)}`;
}
function ensureActive(signal: AbortSignal, credential: string) {
  if (signal.aborted || credential !== getCredential())
    throw new DOMException("Aborted", "AbortError");
}
async function authorizedFetch(path: string, signal: AbortSignal, credential: string) {
  ensureActive(signal, credential);
  const response = await fetch(path, {
    signal,
    cache: "no-store",
    headers: { Authorization: `Bearer ${credential}`, "Accept-Language": getLocale() },
  });
  ensureActive(signal, credential);
  if (response.status === 401 && credential === getCredential())
    window.dispatchEvent(new Event("credential-invalid"));
  if (!response.ok) {
    await response.body?.cancel();
    throw new ApiError(response.status, response.status === 403 ? text.forbidden : response.status === 404 ? text.unavailable : text.failed);
  }
  return response;
}

/** Limit stream consumption independently of Content-Length. */
export async function readBoundedBody(response: Response, limit: number, signal: AbortSignal): Promise<Uint8Array<ArrayBuffer>> {
  const declared = response.headers.get("Content-Length");
  if (declared != null && Number(declared) > limit) {
    await response.body?.cancel();
    throw new Error(text.tooLarge);
  }
  if (!response.body) return new Uint8Array();
  const reader = response.body.getReader();
  const abort = () => { void reader.cancel().catch(() => {}); };
  signal.addEventListener("abort", abort, { once: true });
  const chunks: Uint8Array[] = [];
  let total = 0;
  try {
    while (true) {
      signal.throwIfAborted();
      const { done, value } = await reader.read();
      signal.throwIfAborted();
      if (done) break;
      total += value.byteLength;
      if (total > limit) throw new Error(text.tooLarge);
      chunks.push(value);
    }
    const bytes = new Uint8Array(total);
    let offset = 0;
    for (const chunk of chunks) {
      bytes.set(chunk, offset);
      offset += chunk.byteLength;
    }
    return bytes;
  } catch (error) {
    await reader.cancel().catch(() => {});
    throw error;
  } finally {
    signal.removeEventListener("abort", abort);
    reader.releaseLock();
  }
}

export function validateFilePreview(value: FilePreviewDescriptor, record: RecordEntry): FilePreviewDescriptor {
  if (
    !value || value.file_id !== record.id || value.revision !== record.revision ||
    !Number.isSafeInteger(value.size_bytes) || value.size_bytes < 0 ||
    typeof value.name !== "string" || typeof value.sha256 !== "string" || typeof value.previewable !== "boolean" ||
    !["markdown", "text", "image", "pdf", "unsupported"].includes(value.format)
  ) throw new Error(text.failed);
  if (!value.previewable || value.format === "unsupported") return value;
  if (value.format === "markdown" || value.format === "text") {
    if (value.size_bytes > textPreviewLimit || typeof value.content_text !== "string" ||
      new TextEncoder().encode(value.content_text).byteLength > textPreviewLimit || value.content_text.includes("\0"))
      throw new Error(text.invalidContent);
  } else if (value.size_bytes > binaryPreviewLimit ||
    (value.format === "image" && !rasterTypes.has(value.mime_type)) ||
    (value.format === "pdf" && value.mime_type !== "application/pdf")) {
    throw new Error(text.invalidContent);
  }
  return value;
}

export async function loadFilePreview(record: RecordEntry, signal: AbortSignal, credential = getCredential()): Promise<LoadedFilePreview> {
  const response = await authorizedFetch(`${filePath(record)}/preview?revision=${record.revision}`, signal, credential);
  // A 1 MiB document may expand as JSON escapes. The response still has a strict cap.
  const metadata = await readBoundedBody(response, textPreviewLimit * 6 + 64 * 1024, signal);
  ensureActive(signal, credential);
  const descriptor = validateFilePreview(JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(metadata)), record);
  if (!descriptor.previewable || !["image", "pdf"].includes(descriptor.format)) return { descriptor };
  const content = await authorizedFetch(`${filePath(record)}/preview/content?revision=${record.revision}`, signal, credential);
  const mime = content.headers.get("Content-Type")?.split(";")[0].trim().toLowerCase();
  if (mime !== descriptor.mime_type) {
    await content.body?.cancel();
    throw new Error(text.invalidContent);
  }
  const bytes = await readBoundedBody(content, binaryPreviewLimit, signal);
  ensureActive(signal, credential);
  if (bytes.byteLength !== descriptor.size_bytes) throw new Error(text.invalidContent);
  return { descriptor, blob: new Blob([bytes], { type: descriptor.mime_type }) };
}

export async function downloadPreviewFile(record: RecordEntry, signal: AbortSignal, credential = getCredential()) {
  const response = await authorizedFetch(`${filePath(record)}?revision=${record.revision}`, signal, credential);
  const blob = await response.blob();
  ensureActive(signal, credential);
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  const relative = typeof record.data.relative_path === "string" ? record.data.relative_path : record.id;
  link.download = relative.split("/").at(-1) || record.id;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
