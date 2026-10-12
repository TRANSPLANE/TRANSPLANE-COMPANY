import type { Json, RecordKind } from "./types";

export interface GameReference { kind: RecordKind; id: string; revision: number }
export interface GameAction { operation: string; input: Json; expected_revision: number | null }
export interface GameEntityReference { kind: RecordKind; id: string }
export interface NetworkProfit {
  status: "unknown" | "partial" | "complete";
  revenue_minor: number | null;
  cost_minor: number | null;
  profit_minor: number | null;
  cost_coverage: string | null;
  basis: "cash" | "recognized" | null;
  period_start: string | null;
  period_end: string | null;
  entry_id: string | null;
  entry_revision: number | null;
  source_refs?: GameReference[]; note?: string | null; assumptions?: string | null;
}
export interface NetworkNode {
  id: string; kind: string; record_id: string; label: string; type: string;
  role: string | null; description: string | null; status: string | null;
  revision: number; source_refs: GameReference[]; profit: NetworkProfit;
}
export interface NetworkEdge {
  id: string; source: string; target: string; type: string; status: string;
  label: string | null; revision: number; derived: boolean; source_refs: GameReference[];
}
export interface ProfitPeriod { period_start: string; period_end: string; basis: "cash" | "recognized" }
export interface CompanyNetworkState {
  version: number; company_id: string; company_revision: number; as_of: string;
  currency: string; mode: "real" | "simulation";
  period: ProfitPeriod | null; profit_periods: ProfitPeriod[];
  nodes: NetworkNode[]; edges: NetworkEdge[];
}
export interface QuestData {
  id?: string; title: string; description?: string; outcome?: string;
  category: "main" | "side" | "daily";
  status: "open" | "accepted" | "completed" | "failed" | "cancelled";
  deadline?: string; depends_on?: string[]; source_refs?: GameReference[];
  completion?: { summary?: string; result_refs?: GameReference[] };
}
export interface QuestItem {
  id: string; revision: number; derived: boolean; read_only: boolean; data: QuestData;
  completion_valid?: boolean; progress?: number | null; overdue?: boolean;
  blocked?: boolean; blockers?: string[];
  actions?: GameAction[]; company_id?: string; kind?: RecordKind;
  created_at?: string; updated_at?: string; company_revision?: number;
}
export interface AlertData {
  id?: string; title: string; body: string; type: "opportunity" | "risk" | "event";
  severity: "info" | "warning" | "critical";
  status: "upcoming" | "active" | "resolved" | "dismissed";
  basis: "observed" | "forecast" | "simulation";
  assumptions?: string[]; source_refs?: GameReference[];
  window_start?: string; window_end?: string; entity_refs?: GameEntityReference[];
  quest_id?: string; acknowledgement?: Json;
}
export interface AlertItem {
  id: string; revision: number | null; derived: boolean; read_only: boolean;
  data: AlertData; unread: boolean; applicable?: boolean;
  actions?: GameAction[]; company_id?: string; kind?: RecordKind;
  created_at?: string; updated_at?: string; company_revision?: number;
  related_entities?: { kind: RecordKind; id: string; revision: number; label: string | null }[];
}
export interface ExperienceAward {
  id?: string; category?: string; kind?: string; rule?: string; title?: string; xp: number;
  source_refs?: GameReference[]; evidence?: GameReference[]; source?: GameReference;
  shared_with?: GameReference[];
  [key: string]: unknown;
}
export interface GameProgression {
  xp: number; level: number; level_name: string; current_threshold: number;
  next_threshold: number | null; progress: number; awards: ExperienceAward[];
  rules: Json;
}
export interface GameState {
  version?: number; schema_version?: number; company_id: string; company_revision: number; as_of: string;
  mode: "real" | "simulation";
  progression: GameProgression; quests: QuestItem[]; alerts: AlertItem[];
  draw: { available: boolean; cooldown_until: string | null; deck_version: number | string;
    last_event: Json; reason: string | null; action?: GameAction | null };
}
