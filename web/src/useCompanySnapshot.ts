import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError, getCredential } from "./api";
import { watchCompany } from "./liveUpdates";
import type { Locale } from "./locale";
import type { Session, Snapshot } from "./types";

export function useCompanySnapshot(companyId: string, session: Session | null, locale: Locale) {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const channel = useRef<ReturnType<typeof watchCompany<Snapshot>> | null>(null);
  const previousSession = useRef<Session | null>(null);

  useEffect(() => {
    if (!companyId || !session) {
      setSnapshot(null);
      setLoading(false);
      return;
    }
    const credential = getCredential();
    let live = true;
    const sameSession = previousSession.current === session;
    previousSession.current = session;
    setSnapshot((current) => sameSession && current?.company.id === companyId ? current : null);
    setLoading(true);
    setError(null);
    const current = watchCompany<Snapshot>({
      snapshot: (signal) => api.snapshot(companyId, signal),
      changes: (revision, waitMs, signal) => api.changes(companyId, revision, waitMs, signal),
      revision: (value) => value.company.revision,
      received: (value) => {
        if (!live || credential !== getCredential()) return;
        setSnapshot(value);
        setLoading(false);
      },
      failed: (cause) => {
        if (!live || credential !== getCredential()) return;
        setError(cause);
        setLoading(false);
        if (cause instanceof ApiError && [401, 403, 404].includes(cause.status)) setSnapshot(null);
      },
      healthy: () => { if (live) setError(null); },
      active: () => document.visibilityState !== "hidden" && navigator.onLine !== false,
      subscribe: (wake) => {
        document.addEventListener("visibilitychange", wake);
        window.addEventListener("focus", wake);
        window.addEventListener("online", wake);
        window.addEventListener("offline", wake);
        return () => {
          document.removeEventListener("visibilitychange", wake);
          window.removeEventListener("focus", wake);
          window.removeEventListener("online", wake);
          window.removeEventListener("offline", wake);
        };
      },
    });
    channel.current = current;
    return () => {
      live = false;
      current.stop();
      if (channel.current === current) channel.current = null;
    };
  }, [companyId, session, locale]);

  const reload = useCallback(async () => {
    const current = channel.current;
    try {
      await current?.refresh();
    } catch (cause) {
      if (channel.current === current) setError(cause);
      throw cause;
    }
  }, []);

  const currentSnapshot = snapshot?.company.id === companyId && session === previousSession.current ? snapshot : null;
  return { snapshot: currentSnapshot, loading: loading && !currentSnapshot, error, reload };
}
