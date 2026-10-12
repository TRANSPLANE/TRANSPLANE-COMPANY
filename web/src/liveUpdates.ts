export interface ChangeNotice {
  changed: boolean;
  revision: number;
}

export interface LiveUpdateOptions<T> {
  snapshot: (signal: AbortSignal) => Promise<T>;
  changes: (revision: number, waitMs: number, signal: AbortSignal) => Promise<ChangeNotice>;
  revision: (snapshot: T) => number;
  received: (snapshot: T) => void;
  failed: (error: unknown) => void;
  healthy: () => void;
  active: () => boolean;
  subscribe: (wake: () => void) => () => void;
  // Injected only by transport tests; production retries use abortable timers.
  delay?: (ms: number, signal: AbortSignal) => Promise<void>;
}

function delay(ms: number, signal: AbortSignal) {
  return new Promise<void>((resolve) => {
    const finish = () => {
      clearTimeout(timer);
      signal.removeEventListener("abort", finish);
      resolve();
    };
    const timer = setTimeout(finish, ms);
    signal.addEventListener("abort", finish, { once: true });
    if (signal.aborted) finish();
  });
}

function terminal(error: unknown) {
  const status = (error as { status?: number } | null)?.status;
  return status === 401 || status === 403 || status === 404;
}

/** One company/session/locale channel. Idle checks never download full snapshots. */
export function watchCompany<T>(options: LiveUpdateOptions<T>) {
  let stopped = false;
  let revision: number | null = null;
  let poll: AbortController | null = null;
  let waiting: AbortController | null = null;
  let snapshotRequest: AbortController | null = null;
  let refreshing: Promise<void> | null = null;
  let queuedRefresh = false;
  let immediate = false;
  let resume: (() => void) | null = null;

  function wake() {
    if (stopped) return;
    immediate = true;
    poll?.abort();
    waiting?.abort();
    resume?.();
  }
  const unsubscribe = options.subscribe(wake);

  // A refresh requested during an in-flight load gets one follow-up load. This
  // keeps a successful local write from inheriting a response begun before it.
  function refresh(): Promise<void> {
    if (stopped) return Promise.resolve();
    queuedRefresh = true;
    if (refreshing) return refreshing;
    refreshing = (async () => {
      do {
        queuedRefresh = false;
        snapshotRequest = new AbortController();
        const value = await options.snapshot(snapshotRequest.signal);
        if (stopped || snapshotRequest.signal.aborted) return;
        revision = options.revision(value);
        options.received(value);
        options.healthy();
      } while (queuedRefresh && !stopped);
    })().finally(() => {
      refreshing = null;
      snapshotRequest = null;
    });
    return refreshing;
  }

  async function run() {
    let failures = 0;
    while (!stopped) {
      if (!options.active()) {
        await new Promise<void>((resolve) => { resume = resolve; });
        resume = null;
        continue;
      }
      try {
        if (revision === null) await refresh();
        else {
          const controller = new AbortController();
          poll = controller;
          const notice = await options.changes(revision, immediate ? 0 : 25000, controller.signal);
          if (stopped || controller.signal.aborted) continue;
          immediate = false;
          if (notice.changed && notice.revision !== revision) await refresh();
          else options.healthy();
        }
        failures = 0;
      } catch (error) {
        if (stopped || poll?.signal.aborted || snapshotRequest?.signal.aborted) continue;
        options.failed(error);
        if (terminal(error)) return;
        failures += 1;
        const controller = new AbortController();
        waiting = controller;
        await (options.delay || delay)(Math.min(1000 * 2 ** (failures - 1), 10000), controller.signal);
        waiting = null;
      } finally {
        poll = null;
      }
    }
  }
  void run();

  return {
    refresh,
    stop() {
      stopped = true;
      unsubscribe();
      poll?.abort();
      waiting?.abort();
      snapshotRequest?.abort();
      resume?.();
    },
  };
}
