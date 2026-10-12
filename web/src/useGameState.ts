import { useCallback, useEffect, useRef, useState } from "react";
import { getCredential } from "./api";
import { gameSystemsApi } from "./gameSystemsApi";
import { gameSystemsCopy as text } from "./gameSystemsCopy";
import type { GameState } from "./gameSystemsTypes";
import { useLocale } from "./locale";

export function useGameState(companyId: string, companyRevision: number) {
  const locale = useLocale();
  const credential = getCredential();
  const scope = `${companyId}\u0000${credential}\u0000${locale}`;
  const [loaded, setLoaded] = useState<{ scope: string; state: GameState } | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const generation = useRef(0);
  const activeScope = useRef(scope);
  activeScope.current = scope;
  const controller = useRef<AbortController | null>(null);
  const reload = useCallback(async () => {
    controller.current?.abort();
    if (!companyId || !credential) {
      generation.current++; setLoaded(null); setLoading(false); setError(""); return;
    }
    const abort = new AbortController();
    controller.current = abort;
    const version = ++generation.current;
    setError("");
    setLoading(true);
    try {
      const state = await gameSystemsApi.state(companyId, abort.signal, credential);
      if (abort.signal.aborted || version !== generation.current || activeScope.current !== scope || getCredential() !== credential || state.company_id !== companyId) return;
      setLoaded({ scope, state });
    } catch (cause) {
      if (!abort.signal.aborted && version === generation.current && activeScope.current === scope)
        setError(cause instanceof Error ? cause.message : text.mutationFailed);
    } finally {
      if (!abort.signal.aborted && version === generation.current && activeScope.current === scope) setLoading(false);
    }
  }, [companyId, credential, scope]);
  useEffect(() => {
    void reload();
    return () => { generation.current++; controller.current?.abort(); };
  }, [reload, companyRevision, locale]);
  return { state: loaded?.scope === scope ? loaded.state : null, loading, error, reload };
}
