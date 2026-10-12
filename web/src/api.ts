import { words, phrase } from "./labels";
import type {
  Company,
  Json,
  Plugin,
  PluginList,
  RecordEntry,
  RecordKind,
  Session,
  Snapshot,
  TokenEntry,
} from "./types";
import { arr, obj, str } from "./format";
import { getLocale } from "./locale";

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
  }
}
const credentialKey = "transplane-company-credential";
export const getCredential = () => sessionStorage.getItem(credentialKey) || "";
export const saveCredential = (value: string) =>
  value
    ? sessionStorage.setItem(credentialKey, value)
    : sessionStorage.removeItem(credentialKey);

export async function request<T>(
  path: string,
  options: RequestInit = {},
  credential = getCredential(),
): Promise<T> {
  const headers = new Headers(options.headers);
  headers.set("Accept-Language", getLocale());
  if (credential) headers.set("Authorization", `Bearer ${credential}`);
  if (options.body) headers.set("Content-Type", "application/json");
  const response = await fetch(`/api/v1${path}`, { ...options, headers });
  if (!response.ok) {
    if (response.status === 401 && credential === getCredential())
      window.dispatchEvent(new Event("credential-invalid"));
    let message = response.statusText;
    try {
      const body = await response.json();
      message = body.error?.message || body.message || body.error || message;
    } catch {
      /* Non-JSON response */
    }
    throw new ApiError(
      response.status,
      typeof message === "string" ? message : words.api001,
    );
  }
  if (response.status === 204) return undefined as T;
  return response.json() as Promise<T>;
}
export const api = {
  session: (credential?: string) =>
    request<Session>("/session", {}, credential),
  companies: (signal?: AbortSignal) => request<{ companies: Company[] }>("/companies", { signal }),
  snapshot: (id: string, signal?: AbortSignal) =>
    request<Snapshot>(`/companies/${encodeURIComponent(id)}/snapshot`, {
      signal,
    }),
  changes: (id: string, revision: number, waitMs: number, signal?: AbortSignal) =>
    request<{ company_id: string; changed: boolean; revision: number }>(
      `/companies/${encodeURIComponent(id)}/changes?after_revision=${revision}&wait_ms=${waitMs}`,
      { signal },
    ),
  tokens: () => request<{ tokens: TokenEntry[] }>("/admin/tokens"),
  issueToken: (companyId: string, scopes: string[]) =>
    request<{ token: string; credential?: string; id?: string }>(
      "/admin/tokens",
      {
        method: "POST",
        body: JSON.stringify({ company_id: companyId, scopes, owner: false }),
      },
    ),
  revokeToken: (id: string) =>
    request(`/admin/tokens/${encodeURIComponent(id)}`, { method: "DELETE" }),
  async plugins(companyId: string, owner: boolean): Promise<PluginList> {
    if (!owner) {
      const catalog = await request<{ catalog: Json[]; installed: Json[] }>(
        `/companies/${encodeURIComponent(companyId)}/plugins`,
      );
      return {
        catalog: (catalog.catalog || []).map(normalizePlugin),
        installed: [],
      };
    }
    const catalog = await request<{ catalog: Json[]; installed: Json[] }>(
      "/admin/plugins/catalog",
    );
    return {
      catalog: (catalog.catalog || []).map(normalizePlugin),
      installed: (catalog.installed || []).map(normalizePlugin),
    };
  },
  installPlugin: (manifest: Json, config: Json = {}) =>
    request("/admin/plugins/install", {
      method: "POST",
      body: JSON.stringify({ manifest, config }),
    }),
  setPluginEnabled: (id: string, enabled: boolean) =>
    request(`/admin/plugins/${encodeURIComponent(id)}/enabled`, {
      method: "POST",
      body: JSON.stringify({ enabled }),
    }),
  uninstallPlugin: (id: string) =>
    request(`/admin/plugins/${encodeURIComponent(id)}?purge=false`, {
      method: "DELETE",
    }),
  backup: (companyId?: string) =>
    request<{ path: string; sha256: string; snapshot?: string }>(
      "/admin/backups",
      { method: "POST", body: JSON.stringify({ company_id: companyId }) },
    ),
  fork: (company: Company, name: string) =>
    request<{ company: Company }>(
      `/companies/${encodeURIComponent(company.id)}/operations/simulation.fork`,
      {
        method: "POST",
        body: JSON.stringify({
          request_id: crypto.randomUUID(),
          expected_revision: company.revision,
          input: { name },
        }),
      },
    ),
  discovery: () => request<Json>("/agent-entry", {}, ""),
  chooseDecision: (
    companyId: string,
    decision: RecordEntry,
    selection: { option_id?: string; text?: string },
    requestId: string,
  ) =>
    request<{ record: RecordEntry }>(
      `/companies/${encodeURIComponent(companyId)}/operations/decision.select`,
      {
        method: "POST",
        body: JSON.stringify({
          request_id: requestId,
          expected_revision: decision.revision,
          input: { id: decision.id, ...selection },
        }),
      },
    ),
  history: (companyId: string, revision: number) =>
    request<Snapshot>(
      `/companies/${encodeURIComponent(companyId)}/history/${revision}`,
    ),
  records: (
    companyId: string,
    kind: RecordKind,
    offset = 0,
    history = false,
    id?: string,
    signal?: AbortSignal,
  ) =>
    request<{
      records: RecordEntry[];
      has_more: boolean;
      limit: number;
      offset: number;
      total: number;
      next_offset: number | null;
    }>(
      `/companies/${encodeURIComponent(companyId)}/records?kind=${kind}&offset=${offset}&history=${history}${id ? `&id=${encodeURIComponent(id)}` : ""}`,
      { signal },
    ),
  async downloadFile(
    companyId: string,
    fileId: string,
    filename: string,
    revision?: number,
  ) {
    const credential = getCredential();
    const response = await fetch(
      `/api/v1/companies/${encodeURIComponent(companyId)}/files/${encodeURIComponent(fileId)}${revision == null ? "" : `?revision=${revision}`}`,
      {
        headers: {
          Authorization: `Bearer ${credential}`,
          "Accept-Language": getLocale(),
        },
      },
    );
    if (response.status === 401 && credential === getCredential())
      window.dispatchEvent(new Event("credential-invalid"));
    if (!response.ok) throw new ApiError(response.status, words.api002);
    const url = URL.createObjectURL(await response.blob());
    const a = document.createElement("a");
    a.href = url;
    a.download = filename;
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  },
};

function normalizePlugin(value: Json): Plugin {
  const entry = obj(value);
  const manifest = obj(entry.manifest || value);
  return {
    id: str(entry.id || manifest.id),
    name: str(entry.name || manifest.name),
    version: str(entry.version || manifest.version),
    description: str(entry.description || manifest.description),
    author: str(manifest.author),
    compatibility: str(manifest.compatibility || manifest.api_version),
    source: str(manifest.source),
    permissions: arr(manifest.permissions).map((p) => str(p)),
    enabled: entry.enabled === true,
    manifest: entry.manifest || value,
  };
}
