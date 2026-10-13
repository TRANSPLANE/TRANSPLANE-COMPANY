import { useEffect, useState } from "react";
import { ApiError, getCredential, request } from "./api";
import { str } from "./format";
import type { RecordEntry } from "./types";

export interface AgentLink {
  company_id: string;
  connected: boolean;
  label: string | null;
  state: "handoff_required" | "queued" | "delivered" | "processing" | "interrupted" | "offline" | "awaiting_choice";
  pending_work: { kind: "process_selection" | "prepare_event"; request_key: string; decision_id?: string; decision_revision?: number } | null;
  current_claim?: { decision_id: string; decision_revision: number } | null;
}
export type ChoiceDelivery = "checking" | "handoff" | "queued" | "delivered" | "processing" | "interrupted" | "offline" | "unknown";
export const readAgentLink = (companyId: string, credential: string, signal?: AbortSignal) => request<AgentLink>(`/companies/${encodeURIComponent(companyId)}/agent-link`, { signal }, credential);
export function choiceDelivery(decision: RecordEntry, link: AgentLink | null, ready = false): ChoiceDelivery {
  const sameCompany = link?.company_id === decision.company_id;
  const sameClaim = sameCompany && link?.current_claim?.decision_id === decision.id && link.current_claim.decision_revision >= decision.revision;
  if (str(decision.data.status) === "processing" || sameClaim) return sameClaim && link?.state === "interrupted" ? "interrupted" : "processing";
  if (!ready) return "checking";
  if (!sameCompany || !link) return "unknown";
  const work = link.pending_work;
  const sameSelection = work?.kind === "process_selection" && work.decision_id === decision.id && work.decision_revision === decision.revision;
  if (sameSelection && link.state === "queued") return "queued";
  if (sameSelection && link.state === "delivered") return "delivered";
  if (link.state === "offline") return "offline";
  if (!link.connected && link.state === "handoff_required") return "handoff";
  return "unknown";
}

export function useAgentLink(companyId: string, eventKey: string, enabled: boolean) {
  const credential = getCredential();
  const scope = JSON.stringify([companyId, credential, eventKey]);
  const [value, setValue] = useState<{ scope: string; status: AgentLink | null; ready: boolean }>({ scope: "", status: null, ready: false });
  useEffect(() => {
    if (!enabled) return;
    let live = true;
    let running = false;
    let poll = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let controller: AbortController | null = null;
    async function read() {
      if (!live || running || getCredential() !== credential) return;
      if (timer) clearTimeout(timer);
      if (document.visibilityState === "hidden") { timer = setTimeout(() => void read(), 5000); return; }
      running = true;
      controller = new AbortController();
      const timeout = setTimeout(() => controller?.abort(), 10000);
      try {
        const status = await readAgentLink(companyId, credential, controller.signal);
        if (live && getCredential() === credential) setValue({ scope, status: status.company_id === companyId && typeof status.connected === "boolean" ? status : null, ready: true });
      } catch (error) {
        if (error instanceof ApiError && [403, 404, 405].includes(error.status)) poll = false;
        if (live && getCredential() === credential) setValue({ scope, status: null, ready: true });
      } finally {
        clearTimeout(timeout);
        running = false;
        if (live && poll) timer = setTimeout(() => void read(), 5000);
      }
    }
    void read();
    const visible = () => { if (document.visibilityState !== "hidden") void read(); };
    document.addEventListener("visibilitychange", visible);
    return () => { live = false; controller?.abort(); if (timer) clearTimeout(timer); document.removeEventListener("visibilitychange", visible); };
  }, [companyId, credential, eventKey, enabled, scope]);
  return value.scope === scope ? { status: value.status, ready: value.ready } : { status: null, ready: false };
}
