import type { AlertItem, CompanyNetworkState, NetworkEdge, NetworkNode, QuestItem } from "./gameSystemsTypes";

export interface NetworkFilters { query: string; role: string; relationship: string; income: string }
export function filteredNetwork(state: CompanyNetworkState, filters: NetworkFilters) {
  const query = filters.query.trim().toLocaleLowerCase();
  const matches = (node: NetworkNode) => {
    if (node.kind === "company") return false;
    if (query && ![node.label, node.role, node.description, node.type].join(" ").toLocaleLowerCase().includes(query)) return false;
    if (filters.role && node.type !== filters.role && node.role !== filters.role) return false;
    if (filters.income === "revenue" && !(node.profit.revenue_minor != null && node.profit.revenue_minor > 0)) return false;
    if (filters.income === "profit" && !(node.profit.status === "complete" && node.profit.profit_minor != null && node.profit.profit_minor > 0)) return false;
    if (filters.income === "unknown" && node.profit.status === "complete") return false;
    return true;
  };
  const qualifyingEdges = state.edges.filter((edge) => !filters.relationship || edge.type === filters.relationship);
  const relationNodes = new Set(qualifyingEdges.flatMap((edge) => [edge.source, edge.target]));
  const matchesNodes = state.nodes.filter((node) => matches(node) && (!filters.relationship || relationNodes.has(node.id)));
  const ids = new Set(matchesNodes.map((node) => node.id));
  const root = state.nodes.find((node) => node.kind === "company");
  if (root && matchesNodes.length) ids.add(root.id);
  const nodes = state.nodes.filter((node) => ids.has(node.id));
  const edges = qualifyingEdges.filter((edge) => ids.has(edge.source) && ids.has(edge.target));
  return { nodes, edges, count: matchesNodes.length };
}
export interface NetworkPoint { x: number; y: number }
export function networkGeometry(nodes: NetworkNode[]) {
  const root = nodes.find((node) => node.kind === "company");
  const others = nodes.filter((node) => node !== root).sort((a, b) => a.type.localeCompare(b.type) || a.label.localeCompare(b.label) || a.id.localeCompare(b.id));
  let rings = 1;
  while (others.length > 4 * rings * (rings + 1)) rings++;
  const width = Math.max(680, rings * 448 + 232);
  const height = Math.max(520, rings * 280 + 176);
  const center = { x: width / 2, y: height / 2 };
  const points = new Map<string, NetworkPoint>();
  if (root) points.set(root.id, center);
  let index = 0;
  for (let ring = 1; ring <= rings; ring++) {
    const slots: { x: number; y: number }[] = [];
    for (let x = -ring; x <= ring; x++) slots.push({ x, y: -ring });
    for (let y = -ring + 1; y <= ring; y++) slots.push({ x: ring, y });
    for (let x = ring - 1; x >= -ring; x--) slots.push({ x, y: ring });
    for (let y = ring - 1; y > -ring; y--) slots.push({ x: -ring, y });
    for (const slot of slots) {
      if (index >= others.length) break;
      points.set(others[index++].id, { x: center.x + slot.x * 224, y: center.y + slot.y * 140 });
    }
  }
  return { width, height, points };
}
export function networkNeighbors(node: NetworkNode, nodes: NetworkNode[], edges: NetworkEdge[]) {
  return edges.filter((edge) => edge.source === node.id || edge.target === node.id).flatMap((edge) => {
    const peer = nodes.find((item) => item.id === (edge.source === node.id ? edge.target : edge.source));
    return peer ? [{ edge, node: peer }] : [];
  });
}
export function networkLinePoints(source: NetworkPoint, target: NetworkPoint) {
  const dx = target.x - source.x, dy = target.y - source.y;
  if (dx === 0 && dy === 0) return null;
  const inset = Math.min(dx ? 92 / Math.abs(dx) : Infinity, dy ? 38 / Math.abs(dy) : Infinity);
  return { x1: source.x + dx * inset, y1: source.y + dy * inset, x2: target.x - dx * inset, y2: target.y - dy * inset };
}
export function questBlockers(quest: QuestItem, quests: QuestItem[]) {
  return (quest.data.depends_on || []).filter((id) => {
    const prerequisite = quests.find((item) => item.id === id);
    return !prerequisite || prerequisite.data.status !== "completed" || prerequisite.completion_valid === false;
  });
}
export function questMatches(quest: QuestItem, quests: QuestItem[], category: string, status: string) {
  if (category && quest.data.category !== category) return false;
  if (status === "blocked") return (quest.blocked || questBlockers(quest, quests).length > 0) && ["open", "accepted"].includes(quest.data.status);
  return !status || quest.data.status === status;
}
export function orderedQuests(quests: QuestItem[]) {
  const state = { accepted: 0, open: 1, failed: 2, completed: 3, cancelled: 4 };
  const category = { main: 0, side: 1, daily: 2 };
  return [...quests].sort((a, b) => state[a.data.status] - state[b.data.status] || category[a.data.category] - category[b.data.category] || (a.data.deadline || "9999-12-31").localeCompare(b.data.deadline || "9999-12-31") || a.id.localeCompare(b.id));
}
export function alertMatches(alert: AlertItem, filter: string) {
  if (filter === "unread") return alert.unread && alert.applicable !== false;
  if (filter === "active") return ["active", "upcoming"].includes(alert.data.status) && alert.applicable !== false;
  if (filter === "history") return ["resolved", "dismissed"].includes(alert.data.status) || alert.applicable === false;
  return true;
}
export function orderedAlerts(alerts: AlertItem[]) {
  const severity = { critical: 0, warning: 1, info: 2 };
  return [...alerts].sort((a, b) => Number(b.unread) - Number(a.unread) || severity[a.data.severity] - severity[b.data.severity] || (a.data.window_start || "").localeCompare(b.data.window_start || "") || a.id.localeCompare(b.id));
}
export class StableGameRequest {
  private pending: { key: string; id: string } | null = null;
  get(key: string) { if (this.pending?.key !== key) this.pending = { key, id: crypto.randomUUID() }; return this.pending.id; }
  clear() { this.pending = null; }
}
