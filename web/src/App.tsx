import { words } from "./labels";
import { useCallback, useEffect, useRef, useState, type FormEvent } from "react";
import {
  ArrowRight,
  FileText,
  FolderOpen,
  Flag,
  GitBranch,
  LayoutDashboard,
  LogOut,
  Package,
  Plus,
  RefreshCw,
  Settings2,
  Sprout,
  Wallet,
} from "./icons";
import { api, ApiError, getCredential, saveCredential } from "./api";
import {
  CopyButton,
  ErrorNotice,
  Loading,
  RecordDetails,
} from "./components";
import { copy, type Tab } from "./copy";
import { date, setFormatTimezone } from "./format";
import { OwnerManagement, Plugins } from "./management";
import { Simulation } from "./Simulation";
import { GameHud } from "./GameHud";
import { BrandMark } from "./BrandMark";
import { Chronicle, Play } from "./Play";
import { narrative } from "./narrativeCopy";
import { LanguageSelector } from "./LanguageSelector";
import { useLocale } from "./locale";
import { useCompanySnapshot } from "./useCompanySnapshot";
import { companyFromLocation } from "./companyLocation";
import { StartJourney, WelcomeJourney } from "./StartJourney";
import { journeyCopy } from "./journeyCopy";
import { Playbook } from "./Playbook";
import { Files, Growth, Operations, Reports } from "./views";
import { CompanyNetwork } from "./CompanyNetwork";
import { CommandCenter } from "./CommandCenter";
import { GameNotifications } from "./GameProgress";
import { useGameState } from "./useGameState";
import { StudioGame, useStudioSession, canWriteStudio } from "./StudioGame";
import { studioCopy } from "./studioCopy";
import type { Company, RecordEntry, Session } from "./types";

const icons = {
  overview: LayoutDashboard,
  chronicle: FileText,
  operations: Wallet,
  growth: Sprout,
  network: GitBranch,
  command: Flag,
  simulation: GitBranch,
  reports: FileText,
  files: FolderOpen,
  plugins: Package,
};
type AppTab = Tab | "studio" | "narrative";
function tabFromHash(): AppTab {
  const tab = location.hash.slice(1);
  if (tab === "play") return "overview";
  if (tab === "studio" || tab === "narrative") return tab;
  return tab in copy.nav ? (tab as Tab) : "overview";
}
function messageFor(error: unknown) {
  if (error instanceof ApiError) {
    if (error.status === 401) return copy.unauthorized;
    if (error.status === 403) return copy.forbidden;
    if (error.status >= 500) return copy.connectFailed;
    return error.message;
  }
  return copy.connectFailed;
}

export default function App() {
  const locale = useLocale();
  const [session, setSession] = useState<Session | null>(null);
  const [authLoading, setAuthLoading] = useState(Boolean(getCredential()));
  const [authError, setAuthError] = useState("");
  const [companies, setCompanies] = useState<Company[]>([]);
  const [companyId, setCompanyId] = useState("");
  const [error, setError] = useState("");
  const [tab, setTab] = useState<AppTab>(tabFromHash);
  const [selected, setSelected] = useState<RecordEntry | null>(null);
  const [management, setManagement] = useState(false);
  const [journeyMode, setJourneyMode] = useState<"real" | "simulation" | "choose" | null>(null);
  const [playbook, setPlaybook] = useState(false);
  const [companyLoading, setCompanyLoading] = useState(false);
  const [refreshVersion, setRefreshVersion] = useState(0);
  const [networkReady, setNetworkReady] = useState<{ key: string; ready: boolean } | null>(null);
  const { snapshot, loading, error: snapshotError, reload: reloadSnapshot } = useCompanySnapshot(companyId, session, locale);
  const game = useGameState(snapshot?.company.id || "", snapshot?.company.revision || 0);
  const studio = useStudioSession(snapshot?.company || null);
  const hasStudio = Boolean(studio.state?.run || studio.state?.can_start);
  const showingStudio = tab === "studio" || tab === "overview" && hasStudio;
  const networkReadyKey = `${companyId}:${snapshot?.company.revision}:${locale}:${refreshVersion}`;
  const networkReadiness = useCallback((ready: boolean) => setNetworkReady({ key: networkReadyKey, ready }), [networkReadyKey]);
  const mainRef = useRef<HTMLElement>(null);
  const newCompanyRef = useRef<HTMLButtonElement>(null);
  const playbookRef = useRef<HTMLButtonElement>(null);
  const companyListEpoch = useRef(0);
  setFormatTimezone(snapshot?.company.timezone || "UTC");
  useEffect(() => {
    setError("");
    setAuthError("");
  }, [locale]);
  const owner = session?.role === "owner" || session?.kind === "owner";
  const writable = owner || Boolean(session?.scopes.includes("company.write"));
  const studioWritable = canWriteStudio(session);
  const gameReady = Boolean(game.state && !game.loading && !game.error && game.state.company_id === snapshot?.company.id && game.state.company_revision === snapshot?.company.revision);

  function logout() {
    companyListEpoch.current += 1;
    saveCredential("");
    setSession(null);
    setCompanies([]);
    setCompanyId("");
    setManagement(false);
    setJourneyMode(null);
    setPlaybook(false);
    setSelected(null);
    setAuthError("");
  }
  useEffect(() => {
    if (!getCredential()) return;
    let live = true;
    api
      .session()
      .then((s) => {
        if (live) setSession(s);
      })
      .catch((e) => {
        if (live) {
          saveCredential("");
          setAuthError(messageFor(e));
        }
      })
      .finally(() => {
        if (live) setAuthLoading(false);
      });
    return () => {
      live = false;
    };
  }, []);
  async function loadCompanies(background = false, signal?: AbortSignal) {
    const credential = getCredential();
    const epoch = companyListEpoch.current;
    if (!background) setCompanyLoading(true);
    try {
      const response = await api.companies(signal);
      if (signal?.aborted || credential !== getCredential() || epoch !== companyListEpoch.current) return;
      setCompanies(response.companies || []);
      setError("");
      const requested = new URL(location.href).searchParams.get("company");
      setCompanyId((current) => companyFromLocation(response.companies, current, requested));
    } catch (e) {
      if (!signal?.aborted && credential === getCredential()) setError(messageFor(e));
    } finally {
      if (credential === getCredential() && !background) setCompanyLoading(false);
    }
  }
  useEffect(() => {
    if (!session) return;
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout> | undefined;
    let busy = false;
    const active = () => document.visibilityState !== "hidden" && navigator.onLine !== false;
    async function update(background: boolean) {
      if (busy || controller.signal.aborted || !active()) return;
      busy = true;
      clearTimeout(timer);
      try { await loadCompanies(background, controller.signal); }
      finally {
        busy = false;
        if (!companyId && !controller.signal.aborted && active()) timer = setTimeout(() => void update(true), 5000);
      }
    }
    const wake = () => {
      clearTimeout(timer);
      if (active()) void update(true);
    };
    if (!companyId) void update(false);
    window.addEventListener("focus", wake);
    window.addEventListener("online", wake);
    document.addEventListener("visibilitychange", wake);
    return () => {
      controller.abort();
      clearTimeout(timer);
      window.removeEventListener("focus", wake);
      window.removeEventListener("online", wake);
      document.removeEventListener("visibilitychange", wake);
    };
  }, [session, Boolean(companyId)]);
  useEffect(() => {
    const changed = () => {
      setTab(tabFromHash());
      setSelected(null);
      mainRef.current?.focus({ preventScroll: true });
    };
    window.addEventListener("hashchange", changed);
    return () => window.removeEventListener("hashchange", changed);
  }, []);
  useEffect(() => {
    setSelected(null);
    setError("");
  }, [companyId, session]);
  useEffect(() => {
    if (!companyId) return;
    const url = new URL(location.href);
    url.searchParams.set("company", companyId);
    history.replaceState(null, "", `${url.pathname}${url.search}${url.hash}`);
  }, [companyId]);
  useEffect(() => {
    if (!snapshot) return;
    setCompanies((current) => current.map((company) => company.id === snapshot.company.id ? snapshot.company : company));
  }, [snapshot]);
  useEffect(() => {
    const invalid = () => {
      logout();
      setAuthError(copy.unauthorized);
    };
    window.addEventListener("credential-invalid", invalid);
    return () => window.removeEventListener("credential-invalid", invalid);
  }, []);
  function refresh() {
    setRefreshVersion((value) => value + 1);
    if (!companyId) void loadCompanies();
    else { void reloadSnapshot().catch(() => { /* The snapshot hook exposes the error. */ }); void game.reload(); void studio.reload(); }
  }
  async function afterFork(id: string) {
    await loadCompanies();
    setCompanyId(id);
  }
  function chooseCompany(id: string) {
    setCompanyId(id);
    setSelected(null);
    location.hash = "overview";
  }
  function startJourney(mode?: "real" | "simulation") {
    setPlaybook(false);
    setJourneyMode(mode || "choose");
  }
  async function afterCreated(company: Company) {
    companyListEpoch.current += 1;
    setCompanies((current) => current.some((entry) => entry.id === company.id)
      ? current.map((entry) => entry.id === company.id ? company : entry)
      : [...current, company]);
    chooseCompany(company.id);
  }
  function authorizeAgent() {
    setJourneyMode(null);
    setPlaybook(false);
    setManagement(true);
  }
  if (authLoading)
    return (
      <div className="auth-page">
        <Loading />
      </div>
    );
  if (!session) return <Login initialError={authError} onLogin={setSession} />;
  return (
    <div className={`app-shell narrative-shell ${["overview", "studio", "narrative"].includes(tab) ? "playing" : "reading-records"}`}>
      <a
        className="skip-link"
        href="#main-content"
        onClick={(event) => {
          event.preventDefault();
          mainRef.current?.focus();
          mainRef.current?.scrollIntoView({ block: "start" });
        }}
      >
        {words.app003}
      </a>
      <aside className="sidebar">
        <a className="brand" href="#overview" aria-label={words.app004}>
          <BrandMark />
          <span>
            {copy.brand}
            <small>{copy.product}</small>
          </span>
        </a>
        <div className="company-selector">
          <label htmlFor="company-select">{words.app005}</label>
          <select
            id="company-select"
            value={companyId}
            disabled={!companies.length}
            onChange={(e) => chooseCompany(e.target.value)}
          >
            {!companies.length && <option value="">{words.app006}</option>}
            {companies.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
                {` · ${c.mode === "simulation" ? narrative.simulation : narrative.real}`}
              </option>
            ))}
          </select>
          {owner && <button ref={newCompanyRef} className="text-button company-start-button" onClick={() => startJourney()}><Plus size={16} aria-hidden="true" />{journeyCopy.newCompany}</button>}
        </div>
        <nav aria-label={words.app008} className="primary-game-nav">
          {(["overview", "growth", "network", "command", "chronicle"] as Tab[]).map((key) => {
            const name = key === "growth" ? narrative.journey : copy.nav[key];
            const Icon = icons[key as Tab];
            return (
              <a
                key={key}
                href={`#${key}`}
                className={`nav-item ${tab === key ? "active" : ""}`}
                aria-current={tab === key ? "page" : undefined}
              >
                <Icon size={19} aria-hidden="true" />
                <span>{name}</span>
              </a>
            );
          })}
          <details className="records-menu" key={companyId}>
            <summary className={`nav-item ${["operations", "simulation", "reports", "files", "plugins"].includes(tab) ? "active" : ""}`}><FolderOpen size={19} aria-hidden="true" /><span>{narrative.records}</span></summary>
            <div className="records-menu-items">
              {(["operations", "simulation", "reports", "files", "plugins"] as Tab[]).map((key) => {
                const Icon = icons[key];
                return <a key={key} href={`#${key}`} onClick={(event) => event.currentTarget.closest("details")?.removeAttribute("open")} className={`nav-item ${tab === key ? "active" : ""}`} aria-current={tab === key ? "page" : undefined}>{<Icon size={19} aria-hidden="true" />}<span>{copy.nav[key]}</span></a>;
              })}
            </div>
          </details>
        </nav>
        <div className="sidebar-footer">
          <button ref={playbookRef} className="footer-button" onClick={() => setPlaybook(true)}><FileText size={18} aria-hidden="true" />{journeyCopy.help}</button>
          {snapshot && (
            <div className="mode-indicator">
              <span
                className={
                  snapshot.company.mode === "simulation"
                    ? "simulation-badge"
                    : "real-badge"
                }
              >
                {snapshot.company.mode === "simulation"
                  ? words.app009
                  : words.app010}
              </span>
            </div>
          )}
          {owner && (
            <button
              className="footer-button"
              onClick={() => setManagement(true)}
            >
              <Settings2 size={18} aria-hidden="true" />
              {copy.management}
            </button>
          )}
          <button className="footer-button" onClick={logout}>
            <LogOut size={18} aria-hidden="true" />
            {copy.logout}
          </button>
        </div>
      </aside>
      <div className="workspace">
        <header className="topbar">
          <div className="breadcrumb">
            <span>{snapshot?.company.name || "TransPlane Company"}</span>
            <span aria-hidden="true">/</span>
            <strong>{tab === "studio" ? studioCopy.title : tab === "narrative" ? studioCopy.narrative : copy.nav[tab]}</strong>
          </div>
          <div className="topbar-actions">
            {snapshot && <GameNotifications state={game.state} onOpen={() => { location.hash = "command"; }} />}
            <LanguageSelector />
            {snapshot && (
              <span className="updated-at">
                {copy.updated}{" "}
                {date(snapshot.company.updated_at, snapshot.company.timezone)}
              </span>
            )}
            <button
              className="icon-button"
              onClick={refresh}
              disabled={loading || companyLoading}
              aria-label={copy.refresh}
            >
              <RefreshCw size={18} aria-hidden="true" />
            </button>
          </div>
        </header>
        <main id="main-content" ref={mainRef} tabIndex={-1}
          data-company-id={snapshot?.company.id}
          data-company-revision={snapshot?.company.revision}
          data-company-view={tab}
          data-view-ready={Boolean(snapshot && !loading && !companyLoading && !error && !snapshotError && gameReady && (!showingStudio || studio.state && !studio.loading && !studio.error) && (tab !== "network" || networkReady?.key === networkReadyKey && networkReady.ready))}>
          {!["overview", "studio", "narrative"].includes(tab) && snapshot?.company.mode === "simulation" && (
            <div className="simulation-banner">
              <GitBranch size={17} aria-hidden="true" />
              <strong>{words.app011}</strong>
              {snapshot.company.simulation_date && (
                <span>
                  {copy.simulationTime} {date(snapshot.company.simulation_date)}
                </span>
              )}
              <span>{words.app012}</span>
            </div>
          )}
          {Boolean(error || snapshotError) && <ErrorNotice message={error || messageFor(snapshotError)} retry={refresh} />}
          {loading || companyLoading ? (
            <Loading />
          ) : !companyId ? (
            <WelcomeJourney owner={owner} onStart={startJourney} onHelp={() => setPlaybook(true)} />
          ) : snapshot ? (
            <>
              {showingStudio && <StudioGame key={companyId} snapshot={snapshot} writable={studioWritable} session={studio} onChanged={reloadSnapshot} onNarrative={() => { location.hash = "narrative"; }} />}
              {tab === "overview" && !hasStudio && studio.loading && snapshot.company.mode === "simulation" && snapshot.company.currency === "CNY" && <Loading />}
              {(tab === "narrative" || tab === "overview" && !hasStudio && !(studio.loading && snapshot.company.mode === "simulation" && snapshot.company.currency === "CNY")) && <>
                {hasStudio && <div className="studio-return"><button className="text-button" onClick={() => { location.hash = "studio"; }}>{studioCopy.title}<ArrowRight size={16} aria-hidden="true" /></button></div>}
                <Play key={companyId} snapshot={snapshot} owner={writable} select={setSelected} onChanged={reloadSnapshot} onGuide={() => setPlaybook(true)} onAuthorize={owner ? authorizeAgent : undefined} gameState={game.state} onCommand={() => { location.hash = "command"; }} />
              </>}
              {tab === "chronicle" && (
                <Chronicle snapshot={snapshot} select={setSelected} />
              )}
              {tab === "operations" && (
                <><GameHud snapshot={snapshot} /><Operations snapshot={snapshot} select={setSelected} /></>
              )}
              {tab === "growth" && (
                <Growth snapshot={snapshot} select={setSelected} />
              )}
              {tab === "network" && <CompanyNetwork key={companyId} company={snapshot.company} companyRevision={snapshot.company.revision} owner={writable} onSelect={setSelected} onReady={networkReadiness} refreshVersion={refreshVersion} />}
              {tab === "command" && <CommandCenter key={companyId} company={snapshot.company} snapshot={snapshot} owner={writable} state={game.state} loading={game.loading} error={game.error} onReload={game.reload} onChanged={reloadSnapshot} onSelect={setSelected} />}
              {tab === "simulation" && (
                <Simulation
                  snapshot={snapshot}
                  companies={companies}
                  owner={owner}
                  select={setSelected}
                  onFork={afterFork}
                  onCompany={chooseCompany}
                />
              )}
              {tab === "reports" && (
                <Reports snapshot={snapshot} select={setSelected} />
              )}
              {tab === "files" && (
                <Files snapshot={snapshot} select={setSelected} />
              )}
              {tab === "plugins" && (
                <Plugins
                  owner={owner}
                  companyId={companyId}
                  companyRevision={snapshot.company.revision}
                  onChanged={reloadSnapshot}
                />
              )}
            </>
          ) : null}
        </main>
        <footer className="workspace-footer">
          <span>
            {snapshot
              ? `${snapshot.company.currency} · ${snapshot.company.timezone || "Asia/Shanghai"}`
              : ""}
          </span>
        </footer>
      </div>
      {selected && snapshot && (
        <RecordDetails
          record={selected}
          snapshot={snapshot}
          onClose={() => setSelected(null)}
        />
      )}
      {management && (
        <OwnerManagement
          company={snapshot?.company || companies.find((company) => company.id === companyId)}
          onClose={() => setManagement(false)}
        />
      )}
      {journeyMode && owner && <StartJourney initialMode={journeyMode === "choose" ? undefined : journeyMode} owner={owner} onCreated={afterCreated} onClose={() => setJourneyMode(null)} onAuthorize={authorizeAgent} onStudio={() => { setJourneyMode(null); location.hash = "studio"; }} returnFocus={newCompanyRef.current} />}
      {playbook && <Playbook company={snapshot?.company} owner={owner} onClose={() => setPlaybook(false)} onStart={startJourney} onAuthorize={owner && companyId ? authorizeAgent : undefined} onSimulation={snapshot?.company.mode === "real" ? () => { setPlaybook(false); location.hash = "simulation"; } : undefined} returnFocus={playbookRef.current} />}
    </div>
  );
}

function Login({
  initialError,
  onLogin,
}: {
  initialError: string;
  onLogin: (session: Session) => void;
}) {
  const [credential, setCredential] = useState("");
  const locale = useLocale();
  const [show, setShow] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(initialError);
  const inputRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    setError("");
  }, [locale]);
  async function submit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError("");
    try {
      const value = credential.trim();
      const session = await api.session(value);
      saveCredential(value);
      setCredential("");
      onLogin(session);
    } catch (e) {
      setError(messageFor(e));
      inputRef.current?.focus();
    } finally {
      setBusy(false);
    }
  }
  return (
    <main className="auth-page">
      <div className="auth-content">
        <div className="auth-language">
          <LanguageSelector />
        </div>
        <div className="auth-brand">
          <BrandMark size={56} />
          <span>TRANSPLANE-COMPANY</span>
        </div>
        <div className="auth-heading">
          <h1>{copy.loginTitle}</h1>
          <p>{copy.loginDescription}</p>
        </div>
        <form onSubmit={submit} className="auth-form">
          <label htmlFor="credential">{copy.credential}</label>
          <div className="credential-field">
            <input
              ref={inputRef}
              id="credential"
              name="credential"
              type={show ? "text" : "password"}
              value={credential}
              onChange={(e) => setCredential(e.target.value)}
              required
              autoComplete="current-password"
              spellCheck={false}
              aria-invalid={!!error}
              aria-describedby={error ? "login-error" : "credential-help"}
            />
            <button
              type="button"
              onClick={() => setShow((s) => !s)}
              aria-pressed={show}
            >
              {show ? words.app014 : words.app015}
            </button>
          </div>
          <p id="credential-help" className="field-help">
            {words.app016}
          </p>
          {error && (
            <div id="login-error">
              <ErrorNotice message={error} />
            </div>
          )}
          <button className="primary-button" type="submit" disabled={busy}>
            {busy ? words.app017 : copy.login}
            <ArrowRight size={18} aria-hidden="true" />
          </button>
        </form>
        <div className="auth-guide">
          <h2>{words.app018}</h2>
          <p>{words.app019}</p>
          <CopyButton
            value={`${location.origin}/api/v1/agent-entry`}
            label={words.app020}
          />
        </div>
      </div>
    </main>
  );
}

