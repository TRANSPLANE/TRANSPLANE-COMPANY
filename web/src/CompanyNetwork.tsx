import { useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import { getCredential } from "./api";
import { CopyButton, Empty, ErrorNotice, Loading } from "./components";
import { date, money, number } from "./format";
import { Building2, CircleDot, FileText, GitBranch, Search } from "./icons";
import { useLocale } from "./locale";
import { gameSystemsApi, networkNodeReference } from "./gameSystemsApi";
import { gameSystemsCopy as text, gameLabel } from "./gameSystemsCopy";
import { filteredNetwork, networkGeometry, networkNeighbors, networkLinePoints, type NetworkFilters } from "./gameSystemsState";
import type { CompanyNetworkState, GameReference, NetworkEdge, NetworkNode, ProfitPeriod } from "./gameSystemsTypes";
import type { Company, RecordEntry } from "./types";
import "./game-systems.css";

const emptyFilters: NetworkFilters = { query: "", role: "", relationship: "", income: "" };
export function operatingPrompt(companyId: string, instruction: string, extra?: string) {
  return [instruction, `${text.promptEntry}: ${location.origin}/api/v1/agent-entry`, `${text.promptCompany}: ${companyId}`,
    extra || "", `${text.promptOutput}: ${location.origin}/api/v1/agent-guide?topic=player-output`].filter(Boolean).join("\n\n");
}
const periodKey = (period: ProfitPeriod | null | undefined) => period ? `${period.period_start}/${period.period_end}/${period.basis}` : "";
export function networkRoleLabel(node: Pick<NetworkNode, "role" | "type">) {
  return gameLabel("nodeTypes", !node.role || node.role === "network_entity" ? node.type : node.role);
}

export function CompanyNetwork({ company, companyRevision, onSelect, onGuide, onReady, refreshVersion = 0 }: {
  company: Company; companyRevision: number; owner?: boolean;
  onSelect: (record: RecordEntry) => void; onGuide?: () => void;
  onReady?: (ready: boolean) => void; refreshVersion?: number;
}) {
  const locale = useLocale();
  const credential = getCredential();
  const [period, setPeriod] = useState<ProfitPeriod | undefined>();
  const scope = `${company.id}\u0000${credential}\u0000${locale}\u0000${periodKey(period)}`;
  const [loaded, setLoaded] = useState<{ scope: string; state: CompanyNetworkState } | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [filters, setFilters] = useState(emptyFilters);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [zoom, setZoom] = useState(1);
  const [view, setView] = useState<"graph" | "list">("graph");
  const [resetVersion, setResetVersion] = useState(0);
  const [referenceBusy, setReferenceBusy] = useState(false);
  const viewport = useRef<HTMLDivElement>(null);
  const generation = useRef(0);
  const activeScope = useRef(scope);
  activeScope.current = scope;
  const data = loaded?.scope === scope ? loaded.state : null;
  const readController = useRef<AbortController | null>(null);
  const referenceController = useRef<AbortController | null>(null);
  const drag = useRef<{ x: number; y: number; left: number; top: number } | null>(null);

  const load = useCallback(async () => {
    readController.current?.abort();
    const controller = new AbortController();
    readController.current = controller;
    const version = ++generation.current;
    setLoading(true); setError("");
    try {
      const state = await gameSystemsApi.network(company.id, period, controller.signal, credential);
      if (controller.signal.aborted || version !== generation.current || activeScope.current !== scope || getCredential() !== credential || state.company_id !== company.id) return;
      setLoaded({ scope, state });
    } catch (cause) {
      if (!controller.signal.aborted && version === generation.current && activeScope.current === scope)
        setError(cause instanceof Error ? cause.message : text.noNetworkLoad);
    } finally {
      if (!controller.signal.aborted && version === generation.current && activeScope.current === scope) setLoading(false);
    }
  }, [company.id, credential, scope, period]);
  useEffect(() => { void load(); return () => { generation.current++; readController.current?.abort(); }; }, [load, companyRevision, refreshVersion]);
  useEffect(() => { onReady?.(Boolean(data && !loading && !error && data.company_revision === companyRevision)); }, [data, loading, error, companyRevision, onReady]);
  useEffect(() => { setPeriod(undefined); setFilters(emptyFilters); setSelectedId(null); setZoom(1); setReferenceBusy(false); return () => referenceController.current?.abort(); }, [company.id, credential]);
  const filtered = useMemo(() => data ? filteredNetwork(data, filters) : { nodes: [], edges: [], count: 0 }, [data, filters]);
  const geometry = useMemo(() => networkGeometry(filtered.nodes), [filtered.nodes]);
  const centered = useRef("");
  useEffect(() => {
    if (view !== "graph") { centered.current = ""; return; }
    const positionKey = `${scope}:${resetVersion}:${zoom}`;
    if (!data || centered.current === positionKey) return;
    const element = viewport.current;
    if (!element) return;
    element.scrollTo({ left: Math.max(0, (geometry.width * zoom - element.clientWidth) / 2), top: Math.max(0, (geometry.height * zoom - element.clientHeight) / 2) });
    centered.current = positionKey;
  }, [data, scope, view, resetVersion, geometry.width, geometry.height, zoom]);
  const selected = data?.nodes.find((node) => node.id === selectedId) || null;
  useEffect(() => { if (selectedId && !filtered.nodes.some((node) => node.id === selectedId)) setSelectedId(null); }, [filtered.nodes, selectedId]);
  const roles = [...new Set(data?.nodes.filter((node) => node.kind !== "company").flatMap((node) => [node.type, node.role || ""]).filter(Boolean) || [])].sort();
  const relations = [...new Set(data?.edges.map((edge) => edge.type) || [])].sort();
  const resetView = () => { setZoom(1); setResetVersion((value) => value + 1); };
  async function openReference(reference: GameReference) {
    referenceController.current?.abort();
    const controller = new AbortController(); referenceController.current = controller;
    const operationScope = scope;
    setReferenceBusy(true); setError("");
    try {
      const record = await gameSystemsApi.reference(company.id, reference, controller.signal, credential);
      if (!controller.signal.aborted && activeScope.current === operationScope && getCredential() === credential) onSelect(record);
    } catch (cause) {
      if (!controller.signal.aborted && activeScope.current === operationScope) setError(cause instanceof Error ? cause.message : text.referenceUnavailable);
    } finally { if (!controller.signal.aborted && activeScope.current === operationScope) setReferenceBusy(false); }
  }
  const updateFilter = (key: keyof NetworkFilters, value: string) => setFilters((current) => ({ ...current, [key]: value }));
  function chooseNode(id: string) {
    if (!filtered.nodes.some((node) => node.id === id)) setFilters(emptyFilters);
    setSelectedId(id);
  }
  return <div className="company-network game-system-surface">
    <header className="game-system-heading"><h2><GitBranch size={24} />{text.network}</h2>
      <CopyButton value={operatingPrompt(company.id, text.networkPrompt)} label={text.copyNetwork} />
    </header>
    <div className="network-filters">
      <label className="network-search"><span>{text.search}</span><span className="network-search-input"><Search size={18} /><input type="search" value={filters.query} onChange={(event) => updateFilter("query", event.target.value)} /></span></label>
      <label><span>{text.role}</span><select value={filters.role} onChange={(event) => updateFilter("role", event.target.value)}><option value="">{text.all}</option>{roles.map((role) => <option key={role} value={role}>{gameLabel("nodeTypes", role)}</option>)}</select></label>
      <label><span>{text.relationship}</span><select value={filters.relationship} onChange={(event) => updateFilter("relationship", event.target.value)}><option value="">{text.all}</option>{relations.map((relation) => <option key={relation} value={relation}>{gameLabel("relationships", relation)}</option>)}</select></label>
      <label><span>{text.income}</span><select value={filters.income} onChange={(event) => updateFilter("income", event.target.value)}><option value="">{text.anyIncome}</option><option value="revenue">{text.withRevenue}</option><option value="profit">{text.withProfit}</option><option value="unknown">{text.unknownProfit}</option></select></label>
    </div>
    {error && <ErrorNotice message={error} retry={() => void load()} />}
    {loading && !data ? <Loading /> : data && <>
      <div className="network-period-row">
        <label><span>{text.period}</span><select value={periodKey(period)} onChange={(event) => setPeriod(data.profit_periods.find((item) => periodKey(item) === event.target.value))}>
          <option value="">{text.latestPeriod}</option>{data.profit_periods.map((item) => <option key={periodKey(item)} value={periodKey(item)}>{date(item.period_start)} – {date(item.period_end)} · {item.basis === "cash" ? text.cash : text.recognized}</option>)}
        </select></label>
        {data.period ? <span>{date(data.period.period_start)} – {date(data.period.period_end)} · {data.period.basis === "cash" ? text.cash : text.recognized}</span> : <span>{text.profitNoPeriod}</span>}
      </div>
      <div className="network-toolbar"><div className="game-view-switch" role="group" aria-label={text.network}><button aria-pressed={view === "graph"} onClick={() => setView("graph")}>{text.graph}</button><button aria-pressed={view === "list"} onClick={() => setView("list")}>{text.list}</button></div>
        <span className="network-count">{number(filtered.count)} {text.peopleCount} · {number(filtered.edges.length)} {text.connectionsCount}</span>
        <button className="text-button" onClick={() => { setFilters(emptyFilters); resetView(); }}>{text.clearFilters}</button>
      </div>
      {!filtered.count ? <Empty>{data.nodes.filter((node) => node.kind !== "company").length ? text.filteredEmpty : <>{text.networkEmpty}<span className="game-empty-action"><CopyButton value={operatingPrompt(company.id, text.networkPrompt)} label={text.prepareNetwork} />{onGuide && <button className="text-button" onClick={onGuide}>{text.reload}</button>}</span></>}</Empty> : <div className="network-workspace">
        <section className="network-map-pane" aria-label={view === "graph" ? text.graph : text.list}>
          {view === "graph" && <><div className="network-map-controls"><button className="secondary-button" onClick={() => setZoom((current) => Math.max(0.5, current - 0.25))} disabled={zoom <= 0.5} aria-label={text.zoomOut}>−</button><span>{number(zoom, { style: "percent" })}</span><button className="secondary-button" onClick={() => setZoom((current) => Math.min(2, current + 0.25))} disabled={zoom >= 2} aria-label={text.zoomIn}>+</button><button className="text-button" onClick={resetView}>{text.reset}</button></div>
            <div ref={viewport} className="network-map-viewport" tabIndex={0} aria-label={text.graphLabel} onPointerDown={(event) => {
              if (event.pointerType === "touch" || (event.target as HTMLElement).closest("button")) return;
              const element = event.currentTarget; drag.current = { x: event.clientX, y: event.clientY, left: element.scrollLeft, top: element.scrollTop }; element.setPointerCapture(event.pointerId);
            }} onPointerMove={(event) => { if (!drag.current) return; event.currentTarget.scrollLeft = drag.current.left - (event.clientX - drag.current.x); event.currentTarget.scrollTop = drag.current.top - (event.clientY - drag.current.y); }} onPointerUp={() => { drag.current = null; }} onPointerCancel={() => { drag.current = null; }}>
              <NetworkCanvas nodes={filtered.nodes} edges={filtered.edges} selectedId={selectedId} zoom={zoom} onNode={setSelectedId} />
            </div></>}
          {(view === "list") && <ul className="network-contact-list">{filtered.nodes.map((node) => <li key={node.id}><button className="network-contact" aria-pressed={node.id === selectedId} onClick={() => setSelectedId(node.id)}><span><strong>{node.label}</strong><small>{networkRoleLabel(node)}</small></span><span><span className="sr-only">{text.revenue} </span>{money(node.profit.revenue_minor, data.currency)}</span></button></li>)}</ul>}
        </section>
        <aside className="network-inspector" aria-label={text.selected}>
          {selected ? <NetworkInspector node={selected} state={data} onNode={chooseNode} onReference={openReference} busy={referenceBusy} /> : <Empty compact>{text.selectNode}</Empty>}
        </aside>
      </div>}
    </>}
  </div>;
}

export function NetworkCanvas({ nodes, edges, selectedId, zoom = 1, onNode }: { nodes: NetworkNode[]; edges: NetworkEdge[]; selectedId: string | null; zoom?: number; onNode: (id: string) => void }) {
  const marker = useId().replace(/:/g, "");
  const geometry = networkGeometry(nodes);
  return <div className="network-map-scale" style={{ width: geometry.width * zoom, height: geometry.height * zoom }}><div className="network-map-canvas" style={{ width: geometry.width, height: geometry.height, transform: `scale(${zoom})` }}>
    <svg className="network-map-lines" viewBox={`0 0 ${geometry.width} ${geometry.height}`} width={geometry.width} height={geometry.height} aria-hidden="true">
      <defs><marker id={`network-arrow-${marker}`} viewBox="0 0 8 8" refX="7" refY="4" markerWidth="8" markerHeight="8" orient="auto" markerUnits="userSpaceOnUse"><path d="M0 0h4v2h2v2h2v2H6v2H4V6H0z" fill="currentColor" /></marker></defs>
      {edges.map((edge) => { const source = geometry.points.get(edge.source), target = geometry.points.get(edge.target); if (!source || !target) return null; const coordinates = networkLinePoints(source, target); if (!coordinates) return null; const relevant = !selectedId || edge.source === selectedId || edge.target === selectedId;
        return <line key={edge.id} {...coordinates} markerEnd={`url(#network-arrow-${marker})`} className={`${relevant ? "connected" : "muted"} ${edge.status === "prospective" ? "prospective" : ""} ${edge.status === "ended" ? "ended" : ""}`} />; })}
    </svg>
    {nodes.map((node) => { const point = geometry.points.get(node.id); if (!point) return null; const connected = !selectedId || node.id === selectedId || edges.some((edge) => [edge.source, edge.target].includes(selectedId) && [edge.source, edge.target].includes(node.id));
      return <button key={node.id} className={`network-node ${node.kind === "company" ? "company-root" : ""} ${connected ? "" : "distant"}`} style={{ left: point.x, top: point.y }} aria-pressed={node.id === selectedId} onClick={() => onNode(node.id)} title={node.label}>
        {node.kind === "company" || node.type === "organization" ? <Building2 size={20} /> : node.kind === "business" ? <FileText size={20} /> : <CircleDot size={20} />}
        <span><strong>{node.label || gameLabel("nodeTypes", node.type)}</strong><small>{networkRoleLabel(node)}</small></span>
      </button>; })}
  </div></div>;
}

export function NetworkInspector({ node, state, onNode, onReference, busy }: { node: NetworkNode; state: CompanyNetworkState; onNode: (id: string) => void; onReference: (reference: GameReference) => void; busy: boolean }) {
  const peers = networkNeighbors(node, state.nodes, state.edges);
  const recordRef = networkNodeReference(node);
  const p = node.profit;
  return <>
    <div className="network-person"><span className="game-small-label">{networkRoleLabel(node)}</span><h3>{node.label}</h3>{node.status && <span className="game-record-status">{gameLabel("statuses", node.status)}</span>}{node.description && <p>{node.description}</p>}
      {recordRef && <button className="text-button" disabled={busy} onClick={() => onReference(recordRef)}>{text.openRecord}</button>}
    </div>
    {node.kind !== "company" && <section className="network-return"><h4>{text.profit}</h4>{p.period_start && p.period_end && <p className="game-period">{date(p.period_start)} – {date(p.period_end)} · {p.basis === "cash" ? text.cash : text.recognized}</p>}
      <dl><div><dt>{text.revenue}</dt><dd>{money(p.revenue_minor, state.currency)}</dd></div><div><dt>{text.cost}</dt><dd>{money(p.cost_minor, state.currency)}</dd></div><div className="profit-total"><dt>{text.profit}</dt><dd>{money(p.status === "complete" ? p.profit_minor : null, state.currency)}</dd></div></dl>
      <p className={`network-cost-state cost-${p.status}`}>{p.status === "complete" ? text.profitComplete : p.status === "partial" ? text.profitPartial : text.profitUnknown}</p>
      {p.note && <p>{p.note}</p>}{p.assumptions && <details><summary>{text.assumptions}</summary><p>{p.assumptions}</p></details>}
      {p.entry_id && p.entry_revision != null && <button className="text-button" disabled={busy} onClick={() => onReference({ kind: "profit_entry" as GameReference["kind"], id: p.entry_id!, revision: p.entry_revision! })}>{text.evidence}</button>}
    </section>}
    <section className="network-neighbors"><h4>{text.linkedPeople}</h4>{peers.length ? <ul>{peers.map(({ edge, node: peer }) => <li key={edge.id}><button className="text-button" onClick={() => onNode(peer.id)}>{peer.label}</button><span>{edge.source === node.id ? text.outgoing : text.incoming} · {edge.label || gameLabel("relationships", edge.type)} · {gameLabel("statuses", edge.status)}</span>{edge.source_refs.length > 0 && <details><summary>{text.evidence}</summary><GameEvidence refs={edge.source_refs} onReference={onReference} busy={busy} /></details>}</li>)}</ul> : <p>{text.noLinks}</p>}</section>
    {node.source_refs.length > 0 && <details className="game-evidence"><summary>{text.evidence}</summary><GameEvidence refs={node.source_refs} onReference={onReference} busy={busy} /></details>}
  </>;
}
export function GameEvidence({ refs, onReference, busy = false }: { refs: GameReference[]; onReference: (reference: GameReference) => void; busy?: boolean }) {
  return <ul className="game-evidence-list">{refs.map((reference, index) => <li key={`${reference.kind}:${reference.id}:${reference.revision}`}><button className="text-button" onClick={() => onReference(reference)} disabled={busy}>{text.evidence} {number(index + 1)}</button></li>)}</ul>;
}
