export type RecordKind =
  | "account"
  | "transaction"
  | "business"
  | "customer"
  | "resource"
  | "subscription"
  | "goal"
  | "route"
  | "forecast"
  | "report"
  | "file"
  | "skill"
  | "network_entity"
  | "relationship"
  | "profit_entry"
  | "quest"
  | "alert"
  | "decision"
  | "simulation_turn";
export type Json =
  | null
  | boolean
  | number
  | string
  | Json[]
  | { [key: string]: Json };
export interface Company {
  id: string;
  name: string;
  currency: string;
  timezone: string;
  description?: string;
  opening_date?: string;
  revision: number;
  created_at: string;
  updated_at: string;
  mode?: "real" | "simulation";
  baseline_company_id?: string | null;
  baseline_revision?: number | null;
  simulation_date?: string | null;
}
export interface RecordEntry {
  id: string;
  company_id: string;
  kind: RecordKind;
  revision: number;
  company_revision?: number;
  data: Record<string, Json>;
  created_at: string;
  updated_at: string;
}
export interface GoalProgress {
  id: string;
  current: number | null;
  target: number | null;
  progress: number | null;
  status: string;
}
export interface CashflowPoint {
  month: string;
  inflow_minor: number;
  outflow_minor: number;
  net_minor: number;
}
export interface Metrics {
  as_of: string;
  currency: string;
  cash_balance_minor: number | null;
  operating_inflow_minor: number | null;
  operating_outflow_minor: number | null;
  net_operating_cashflow_minor: number | null;
  mrr_minor: number | null;
  arr_minor: number | null;
  runway_months: number | null;
  runway_status: string;
  monthly_cashflow: CashflowPoint[];
  goal_progress: GoalProgress[];
  data_revision: number;
}
export interface DomainEvent {
  id: string;
  event_type?: string;
  occurred_at?: string;
  created_at?: string;
  record_id?: string;
  kind?: string;
  payload?: Record<string, Json>;
}
export interface Snapshot {
  company: Company;
  metrics: Metrics;
  records: Partial<Record<RecordKind, RecordEntry[]>>;
  events: DomainEvent[];
  extensions?: ExtensionResult[];
}
export interface ExtensionField {
  id: string | null;
  label: string;
  value: string | number | boolean | null;
  format: "number" | "money" | "percent" | "text";
  unit: string | null;
}
export interface ExtensionResult {
  id: string;
  name: string;
  request_id: string;
  updated_at: string;
  enabled?: boolean;
  metrics: ExtensionField[];
  views: { id: string; title: string; fields: ExtensionField[] }[];
}
export interface Session {
  role?: string;
  kind?: string;
  company_id?: string | null;
  scopes: string[];
}
export interface TokenEntry {
  id: string;
  company_id?: string | null;
  scopes: string[];
  created_at?: string;
  revoked?: boolean;
  revoked_at?: string | null;
  owner?: boolean;
  kind?: string;
}
export interface Plugin {
  id: string;
  name: string;
  version: string;
  description?: string;
  author?: string;
  compatibility?: string;
  source?: string;
  permissions?: string[];
  enabled?: boolean;
  status?: string;
  contributions?: Record<string, Json>;
  data_policy?: Record<string, Json>;
  manifest?: Json;
}
export interface PluginList {
  catalog: Plugin[];
  installed: Plugin[];
}
