import { request } from "./api";
import type { Json, RecordEntry, RecordKind } from "./types";

export type SkillMode = "real" | "simulation";
export type SkillType = "procedure" | "experience";
export interface SkillCopyReference { id: string; revision: number; content_sha256: string; language: "zh-CN" | "en" }
export interface SkillOrigin extends SkillCopyReference { source: "builtin" | "plugin" | "company"; plugin_version?: string; document: string }
export interface SkillReference { kind: RecordKind; id: string; revision: number }
export interface SkillMetadata {
  id: string;
  name: string;
  title: string;
  description: string;
  source: "builtin" | "plugin" | "company";
  enabled: boolean;
  effective: boolean;
  deleted: boolean;
  revision: number;
  read_only: boolean;
  content_sha256: string;
  company_id?: string;
  updated_at?: string;
  plugin_id?: string;
  tags?: string[];
  modes?: SkillMode[];
  type?: SkillType;
  read_url: string;
  document_url: string;
  history_url?: string;
}
export interface SkillDocument extends SkillMetadata {
  body: string;
  source_refs?: SkillReference[];
  copied_from?: SkillOrigin;
}
export interface SkillResponse { version: number; skill: SkillDocument; document: string }
export interface SkillWrite {
  id: string;
  name: string;
  title: string;
  description: string;
  body: string;
  enabled: boolean;
  tags: string[];
  modes: SkillMode[];
  type: SkillType;
  source_refs: SkillReference[];
  copy_from?: SkillCopyReference;
}

const path = (companyId: string) => `/companies/${encodeURIComponent(companyId)}/skills`;
function operation(companyId: string, name: string, input: Json, expectedRevision?: number, requestId: string = crypto.randomUUID()) {
  return request<{ record: RecordEntry }>(`/companies/${encodeURIComponent(companyId)}/operations/${name}`, {
    method: "POST",
    body: JSON.stringify({ request_id: requestId, expected_revision: expectedRevision, input }),
  });
}
export const skillsApi = {
  list: (companyId: string, includeDeleted: boolean, signal?: AbortSignal) =>
    request<{ version: number; company_id: string; items: SkillMetadata[] }>(`${path(companyId)}?include_deleted=${includeDeleted}`, { signal }),
  read: (companyId: string, id: string, revision?: number, signal?: AbortSignal) =>
    request<SkillResponse>(`${path(companyId)}/${encodeURIComponent(id)}${revision == null ? "" : `?revision=${revision}`}`, { signal }),
  history: (companyId: string, id: string, signal?: AbortSignal) =>
    request<{ items: SkillMetadata[] }>(`${path(companyId)}/${encodeURIComponent(id)}/history`, { signal }),
  save: (companyId: string, input: SkillWrite, expectedRevision?: number, requestId?: string) =>
    operation(companyId, "skill.put", input as unknown as Json, expectedRevision, requestId),
  setEnabled: (companyId: string, skill: SkillMetadata, enabled: boolean, requestId?: string) =>
    operation(companyId, "skill.set_enabled", { id: skill.id, enabled }, skill.revision, requestId),
  remove: (companyId: string, skill: SkillMetadata, requestId?: string) =>
    operation(companyId, "skill.delete", { id: skill.id }, skill.revision, requestId),
};
export function skillReadAddress(companyId: string, id: string, origin: string, revision?: number): string {
  const suffix = revision == null ? "" : `?revision=${revision}`;
  return new URL(`/api/v1${path(companyId)}/${encodeURIComponent(id)}${suffix}`, origin).href;
}
