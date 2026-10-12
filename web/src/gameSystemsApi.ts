import { request } from "./api";
import type { Json, RecordEntry, RecordKind } from "./types";
import type { CompanyNetworkState, GameReference, GameState, ProfitPeriod } from "./gameSystemsTypes";
import { gameSystemsCopy as text } from "./gameSystemsCopy";

const companyPath = (companyId: string) => `/companies/${encodeURIComponent(companyId)}`;
export const gameSystemsApi = {
  network(companyId: string, period?: ProfitPeriod, signal?: AbortSignal, credential?: string) {
    const query = period ? `?${new URLSearchParams({ period_start: period.period_start, period_end: period.period_end, basis: period.basis })}` : "";
    return request<CompanyNetworkState>(`${companyPath(companyId)}/network${query}`, { signal }, credential);
  },
  state: (companyId: string, signal?: AbortSignal, credential?: string) =>
    request<GameState>(`${companyPath(companyId)}/game-state`, { signal }, credential),
  operate: (companyId: string, operation: string, input: Json, requestId: string, revision?: number | null, credential?: string) =>
    request<{ record?: RecordEntry; [key: string]: unknown }>(`${companyPath(companyId)}/operations/${encodeURIComponent(operation)}`, {
      method: "POST", body: JSON.stringify({ request_id: requestId, expected_revision: revision, input }),
    }, credential),
  async reference(companyId: string, reference: GameReference, signal?: AbortSignal, credential?: string): Promise<RecordEntry> {
    let offset = 0;
    for (;;) {
      const query = new URLSearchParams({ kind: reference.kind, id: reference.id, history: "true", offset: String(offset) });
      const response = await request<{ records: RecordEntry[]; has_more: boolean; next_offset: number | null }>(
        `${companyPath(companyId)}/records?${query}`, { signal }, credential,
      );
      const found = response.records.find((record) => record.id === reference.id && record.revision === reference.revision && record.company_id === companyId);
      if (found) return found;
      if (!response.has_more || response.next_offset == null || response.next_offset <= offset) throw new Error(text.referenceUnavailable);
      offset = response.next_offset;
    }
  },
  async currentReference(companyId: string, reference: { kind: RecordKind; id: string }, signal?: AbortSignal, credential?: string): Promise<RecordEntry> {
    const query = new URLSearchParams({ kind: reference.kind, id: reference.id });
    const response = await request<{ records: RecordEntry[] }>(`${companyPath(companyId)}/records?${query}`, { signal }, credential);
    const found = response.records.find((record) => record.id === reference.id && record.company_id === companyId);
    if (!found) throw new Error(text.referenceUnavailable);
    return found;
  },
};
export function networkNodeReference(node: { kind: string; record_id: string; revision: number }): GameReference | null {
  return node.kind === "company" ? null : { kind: node.kind as RecordKind, id: node.record_id, revision: node.revision };
}
