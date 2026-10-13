import { words, phrase } from "./labels";
import { useEffect, useMemo, useState } from "react";
import {
  ArrowDownLeft,
  ArrowUpRight,
  Check,
  ChevronRight,
  Download,
  Flag,
  Search,
} from "./icons";
import { api, getCredential } from "./api";
import { Extensions } from "./Extensions";
import { CompanyScene } from "./CompanyScene";
import { EvidenceTrace } from "./EvidenceTrace";
import { ForecastContext } from "./ForecastContext";
import { FilePreview } from "./FilePreview";
import { filePreviewCopy } from "./filePreviewCopy";
import { copy, recordCopy } from "./copy";
import {
  Empty,
  ErrorNotice,
  GoLink,
  Loading,
  RecordButton,
  Section,
  Status,
} from "./components";
import {
  arr,
  date,
  money,
  number,
  month,
  percent,
  num,
  obj,
  size,
  sourceText,
  str,
  title,
} from "./format";
import type { GoalProgress, RecordEntry, Snapshot } from "./types";

type ViewProps = { snapshot: Snapshot; select: (record: RecordEntry) => void };
const records = (snapshot: Snapshot, kind: keyof Snapshot["records"]) =>
  snapshot.records[kind] || [];

export function Overview({ snapshot, select }: ViewProps) {
  const { company, metrics } = snapshot;
  const goals = records(snapshot, "goal").filter((g) =>
    ["active", "blocked", "planned"].includes(str(g.data.status)),
  );
  const route = [...records(snapshot, "route")]
    .sort((a, b) => a.updated_at.localeCompare(b.updated_at))
    .at(-1);
  const reports = [...records(snapshot, "report")].sort((a, b) =>
    a.updated_at.localeCompare(b.updated_at),
  );
  const transactions = [...records(snapshot, "transaction")]
    .sort((a, b) =>
      str(b.data.occurred_on).localeCompare(str(a.data.occurred_on)),
    )
    .slice(0, 5);
  return (
    <>
      <CompanyScene snapshot={snapshot} />
      <div className="overview-columns">
        <Section
          title={words.views160}
          action={<GoLink tab="growth">{words.views161}</GoLink>}
        >
          <div className="goal-preview">
            {goals.length ? (
              goals
                .slice(0, 2)
                .map((g) => (
                  <GoalItem
                    key={g.id}
                    goal={g}
                    progress={metrics.goal_progress.find((p) => p.id === g.id)}
                    currency={company.currency}
                    select={select}
                  />
                ))
            ) : (
              <Empty compact>{copy.empty.goal}</Empty>
            )}
          </div>
        </Section>
        <Section title={words.views162}>
          <Cashflow snapshot={snapshot} />
        </Section>
      </div>
      <Extensions snapshot={snapshot} />
      <div className="overview-bottom">
        <Section
          title={words.views163}
          action={<GoLink tab="operations">{words.views164}</GoLink>}
        >
          {transactions.length ? (
            <TransactionTable
              entries={transactions}
              snapshot={snapshot}
              select={select}
            />
          ) : (
            <Empty compact>{copy.empty.transaction}</Empty>
          )}
        </Section>
        <Section
          title={words.views165}
          action={<GoLink tab="reports">{words.views166}</GoLink>}
        >
          {reports.length ? (
            [...reports]
              .reverse()
              .slice(0, 3)
              .map((r) => (
                <button
                  key={r.id}
                  className="report-preview"
                  onClick={() => select(r)}
                >
                  <span className="report-kind">
                    {str(r.data.type) === "monthly"
                      ? words.views167
                      : words.views168}
                  </span>
                  <strong>{title(r)}</strong>
                  <span>
                    {date(str(r.data.period_start))} —{" "}
                    {date(str(r.data.period_end))}
                  </span>
                  <ChevronRight size={18} aria-hidden="true" />
                </button>
              ))
          ) : (
            <Empty compact>{copy.empty.report}</Empty>
          )}
        </Section>
      </div>
    </>
  );
}

function Cashflow({ snapshot }: { snapshot: Snapshot }) {
  const points = snapshot.metrics.monthly_cashflow.slice(-6);
  if (!points.length) return <Empty compact>{copy.empty.transaction}</Empty>;
  const max = Math.max(...points.map((p) => p.net_minor), 0);
  const min = Math.min(...points.map((p) => p.net_minor), 0);
  const span = Math.max(max - min, 100);
  const y = (value: number) => 20 + ((max - value) / span) * 120;
  const x = (i: number) => 56 + (i * 320) / Math.max(points.length - 1, 1);
  return (
    <figure className="cashflow-figure">
      <figcaption>
        {words.views169}
        {snapshot.company.currency}
      </figcaption>
      <dl className="mobile-cashflow-list">
        {points.map((p) => (
          <div key={p.month}>
            <dt>{month(p.month, true)}</dt>
            <dd className="number">
              {money(p.net_minor, snapshot.company.currency)}
            </dd>
          </div>
        ))}
      </dl>
      <svg viewBox="0 0 400 178" role="img" aria-label={words.views170}>
        <line x1="56" x2="376" y1={y(0)} y2={y(0)} className="zero-line" />
        <text x="0" y={y(max) + 4}>
          {money(max, snapshot.company.currency, 0)}
        </text>
        {min < 0 && (
          <text x="0" y={y(min) + 4}>
            {money(min, snapshot.company.currency, 0)}
          </text>
        )}
        <polyline
          points={points.map((p, i) => `${x(i)},${y(p.net_minor)}`).join(" ")}
          className="chart-line"
        />
        {points.map((p, i) => (
          <g key={p.month}>
            <circle
              cx={x(i)}
              cy={y(p.net_minor)}
              r="3"
              className="chart-point"
            />
            <text x={x(i)} y="168" textAnchor="middle">
              {month(p.month)}
            </text>
          </g>
        ))}
      </svg>
      <details className="chart-details">
        <summary>{words.views172}</summary>
        <div className="table-scroll">
          <table>
            <thead>
              <tr>
                <th>{words.views173}</th>
                <th className="numeric">{words.views174}</th>
                <th className="numeric">{words.views175}</th>
                <th className="numeric">{words.views176}</th>
              </tr>
            </thead>
            <tbody>
              {points.map((p) => (
                <tr key={p.month}>
                  <td>{month(p.month, true)}</td>
                  <td className="numeric">
                    {money(p.inflow_minor, snapshot.company.currency)}
                  </td>
                  <td className="numeric">
                    {money(p.outflow_minor, snapshot.company.currency)}
                  </td>
                  <td className="numeric">
                    {money(p.net_minor, snapshot.company.currency)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </details>
    </figure>
  );
}

export function Operations({ snapshot, select }: ViewProps) {
  const [filter, setFilter] = useState("all");
  const [query, setQuery] = useState("");
  const [offset, setOffset] = useState(0);
  const [ledger, setLedger] = useState<{
    records: RecordEntry[];
    has_more: boolean;
    limit: number;
    total?: number;
    next_offset?: number | null;
  }>({ records: [], has_more: false, limit: 500 });
  const [pageLoading, setPageLoading] = useState(true);
  const [ledgerError, setLedgerError] = useState("");
  useEffect(() => {
    const controller = new AbortController();
    setPageLoading(true);
    setLedgerError("");
    api
      .records(
        snapshot.company.id,
        "transaction",
        offset,
        false,
        undefined,
        controller.signal,
      )
      .then((result) => {
        if (!controller.signal.aborted) setLedger(result);
      })
      .catch((e) => {
        if (!controller.signal.aborted)
          setLedgerError(e instanceof Error ? e.message : copy.connectFailed);
      })
      .finally(() => {
        if (!controller.signal.aborted) setPageLoading(false);
      });
    return () => controller.abort();
  }, [snapshot.company.id, snapshot.company.revision, offset]);
  const transactions = [...ledger.records]
    .filter(
      (r) =>
        (filter === "all" || r.data.category === filter) &&
        `${title(r)} ${str(r.data.description)} ${sourceText(r)}`
          .toLowerCase()
          .includes(query.toLowerCase()),
    )
    .sort((a, b) =>
      str(b.data.occurred_on).localeCompare(str(a.data.occurred_on)),
    );
  return (
    <>
      <PageHeading
        title={words.views177}
        context={`${date(snapshot.metrics.as_of)} · ${snapshot.company.currency}`}
      />
      <div className="operating-summary">
        <dl>
          <div>
            <dt>{words.views178}</dt>
            <dd>
              {money(
                snapshot.metrics.operating_inflow_minor,
                snapshot.company.currency,
              )}
            </dd>
          </div>
          <div>
            <dt>{words.views179}</dt>
            <dd>
              {money(
                snapshot.metrics.operating_outflow_minor,
                snapshot.company.currency,
              )}
            </dd>
          </div>
          <div>
            <dt>ARR</dt>
            <dd>
              {money(snapshot.metrics.arr_minor, snapshot.company.currency)}
            </dd>
          </div>
        </dl>
      </div>
      <Extensions snapshot={snapshot} />
      <Section title={words.views180}>
        <div className="filter-row">
          <label className="search-field">
            <Search size={16} aria-hidden="true" />
            <span className="sr-only">{words.views181}</span>
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={recordCopy.searchPage}
            />
          </label>
          <label>
            <span className="sr-only">{words.views183}</span>
            <select value={filter} onChange={(e) => setFilter(e.target.value)}>
              <option value="all">{words.views184}</option>
              {Object.entries(copy.categories).map(([v, label]) => (
                <option key={v} value={v}>
                  {label}
                </option>
              ))}
            </select>
          </label>
        </div>
        {ledgerError && <ErrorNotice message={ledgerError} />}
        {pageLoading ? (
          <Loading />
        ) : transactions.length ? (
          <TransactionTable
            entries={transactions}
            snapshot={snapshot}
            select={select}
          />
        ) : (
          <Empty compact>
            {query || filter !== "all"
              ? words.views185
              : copy.empty.transaction}
          </Empty>
        )}
        <div className="pagination">
          <span>
            {ledger.records.length === 0 ? 0 : offset + 1}–
            {offset + ledger.records.length} {recordCopy.entries}
          </span>
          <div>
            <button
              className="secondary-button"
              disabled={offset === 0 || pageLoading}
              onClick={() => setOffset((o) => Math.max(0, o - ledger.limit))}
            >
              {recordCopy.previous}
            </button>
            <button
              className="secondary-button"
              disabled={!ledger.has_more || pageLoading}
              onClick={() =>
                setOffset((o) => ledger.next_offset ?? o + ledger.limit)
              }
            >
              {recordCopy.next}
            </button>
          </div>
        </div>
      </Section>
      <div className="split-sections">
        <Section title={words.views186}>
          {records(snapshot, "account").length ? (
            <div className="entity-list">
              {records(snapshot, "account").map((a) => (
                <div key={a.id} className="entity-row">
                  <RecordButton record={a} onSelect={select} />
                  <span className="muted">{words.views187}</span>
                  <span className="number">
                    {money(
                      num(a.data.opening_balance_minor),
                      snapshot.company.currency,
                    )}
                  </span>
                </div>
              ))}
            </div>
          ) : (
            <Empty compact>{copy.empty.account}</Empty>
          )}
        </Section>
        <Section title={words.views188}>
          <EntityList
            entries={records(snapshot, "business")}
            empty={copy.empty.business}
            select={select}
          />
        </Section>
      </div>
      <div className="split-sections">
        <Section title={words.views189}>
          <EntityList
            entries={records(snapshot, "customer")}
            empty={copy.empty.customer}
            select={select}
          />
        </Section>
        <Section title={words.views190}>
          <EntityList
            entries={records(snapshot, "resource")}
            empty={copy.empty.resource}
            select={select}
            resource
          />
        </Section>
      </div>
      <Section title={words.views191}>
        {records(snapshot, "subscription").length ? (
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th>{words.views192}</th>
                  <th>{words.views193}</th>
                  <th>{words.views194}</th>
                  <th className="numeric">{words.views195}</th>
                  <th>{words.views196}</th>
                </tr>
              </thead>
              <tbody>
                {records(snapshot, "subscription").map((r) => (
                  <tr key={r.id}>
                    <td>
                      <button className="record-link" onClick={() => select(r)}>
                        {title(
                          records(snapshot, "customer").find(
                            (c) => c.id === r.data.customer_id,
                          ) || r,
                        )}
                      </button>
                    </td>
                    <td>
                      {num(r.data.interval_months) === 12
                        ? words.views197
                        : words.views198}
                    </td>
                    <td>
                      <Status value={str(r.data.status)} />
                    </td>
                    <td className="numeric">
                      {money(
                        num(r.data.amount_minor),
                        snapshot.company.currency,
                      )}
                    </td>
                    <td>{date(str(r.data.starts_on))}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <Empty compact>{copy.empty.subscription}</Empty>
        )}
      </Section>
    </>
  );
}

function TransactionTable({
  entries,
  snapshot,
  select,
}: { entries: RecordEntry[] } & ViewProps) {
  return (
    <div className="table-scroll">
      <table className="transaction-table">
        <thead>
          <tr>
            <th>{words.views199}</th>
            <th>{words.views200}</th>
            <th>{words.views201}</th>
            <th className="numeric">{words.views202}</th>
          </tr>
        </thead>
        <tbody>
          {entries.map((r) => (
            <tr key={r.id}>
              <td>
                <button className="transaction-name" onClick={() => select(r)}>
                  <span className="transaction-direction">
                    {r.data.direction === "inflow" ? (
                      <ArrowDownLeft size={16} aria-label={words.views203} />
                    ) : (
                      <ArrowUpRight size={16} aria-label={words.views204} />
                    )}
                  </span>
                  <span>
                    <strong>{str(r.data.description, title(r))}</strong>
                    {sourceText(r) && <small>{sourceText(r)}</small>}
                  </span>
                </button>
              </td>
              <td>
                {copy.categories[str(r.data.category)] || str(r.data.category)}
              </td>
              <td className="nowrap">{date(str(r.data.occurred_on))}</td>
              <td className="numeric nowrap">
                {r.data.direction === "inflow" ? "+" : "−"}
                {money(num(r.data.amount_minor), snapshot.company.currency)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
function EntityList({
  entries,
  empty,
  select,
  resource = false,
}: {
  entries: RecordEntry[];
  empty: string;
  select: ViewProps["select"];
  resource?: boolean;
}) {
  return entries.length ? (
    <div className="entity-list">
      {entries.map((r) => (
        <div key={r.id} className="entity-row">
          <div>
            <RecordButton record={r} onSelect={select} />
            {str(r.data.role || r.data.description) && (
              <p>{str(r.data.role || r.data.description)}</p>
            )}
          </div>
          {resource && (
            <span className="muted">
              {copy.resourceTypes[str(r.data.type)] || str(r.data.type)}
            </span>
          )}
          {str(r.data.status) && <Status value={str(r.data.status)} />}
        </div>
      ))}
    </div>
  ) : (
    <Empty compact>{empty}</Empty>
  );
}

function GoalItem({
  goal,
  progress,
  currency,
  select,
}: {
  goal: RecordEntry;
  progress?: GoalProgress;
  currency: string;
  select: ViewProps["select"];
}) {
  const status = progress?.status || str(goal.data.status);
  const percentage =
    progress?.progress == null
      ? null
      : Math.max(0, Math.min(100, progress.progress * 100));
  const monetary = ["mrr", "cash_balance"].includes(str(goal.data.metric));
  const value = (n: number | null | undefined) =>
    n == null ? words.views205 : monetary ? money(n, currency, 0) : number(n);
  return (
    <article className={`goal-item goal-${status}`}>
      <div className="goal-topline">
        <button className="record-link" onClick={() => select(goal)}>
          {title(goal)}
        </button>
        <Status value={status} />
      </div>
      {str(goal.data.outcome) && <p>{str(goal.data.outcome)}</p>}
      {percentage != null && (
        <>
          <div className="goal-progress-label">
            <span>{copy.metrics[str(goal.data.metric)] || words.views206}</span>
            <strong className="number">
              {value(progress?.current)}
              <span> / {value(progress?.target)}</span>
            </strong>
          </div>
          <progress
            value={percentage}
            max={100}
            aria-label={`${title(goal)}，${percent(percentage / 100, 0)}`}
          />
        </>
      )}
      {str(goal.data.evidence) && (
        <p className="goal-evidence">
          <Check size={14} aria-hidden="true" />
          {str(goal.data.evidence)}
        </p>
      )}
      <div className="goal-footer">
        {str(goal.data.deadline) && (
          <span>
            {words.views207}
            {date(str(goal.data.deadline))}
          </span>
        )}
        <span>
          {words.views208}
          {goal.revision}
        </span>
      </div>
    </article>
  );
}

export function Growth({ snapshot, select }: ViewProps) {
  const routes = [...records(snapshot, "route")].sort((a, b) =>
    a.updated_at.localeCompare(b.updated_at),
  );
  const [routeId, setRouteId] = useState("");
  const route = routes.find((r) => r.id === routeId) || routes.at(-1);
  const goals = records(snapshot, "goal");
  const routeGoals = new Set(
    [
      ...arr(route?.data.goal_ids),
      ...arr(route?.data.stages).flatMap((s) => arr(obj(s).goal_ids)),
    ].map((v) => str(v)),
  );
  return (
    <>
      <PageHeading title={words.views209} context={words.views210} />
      {routes.length > 1 && (
        <label className="route-selector">
          {words.views211}
          <select
            value={route?.id || ""}
            onChange={(e) => setRouteId(e.target.value)}
          >
            {[...routes].reverse().map((r) => (
              <option key={r.id} value={r.id}>
                {title(r)} {words.views212}
                {r.revision}
              </option>
            ))}
          </select>
        </label>
      )}
      {route && (
        <div className="route-intro">
          <h2>{title(route)}</h2>
          {str(route.data.story) && <p>{str(route.data.story)}</p>}
          <button className="text-button" onClick={() => select(route)}>
            {words.views213}
          </button>
        </div>
      )}
      {route && arr(route.data.stages).length > 0 ? (
        <ol className="growth-path">
          {arr(route.data.stages).map((s, i) => {
            const stage = obj(s);
            const ids = arr(stage.goal_ids).map((v) => str(v));
            const stageGoals = goals.filter((g) => ids.includes(g.id));
            const complete =
              stageGoals.length > 0 &&
              stageGoals.every(
                (g) =>
                  snapshot.metrics.goal_progress.find((p) => p.id === g.id)
                    ?.status === "achieved",
              );
            return (
              <li
                key={str(stage.id, String(i))}
                className={complete ? "stage-complete" : ""}
              >
                <div className="stage-node" aria-hidden="true">
                  {complete ? <Check size={18} /> : i + 1}
                </div>
                <div className="stage-content">
                  <h3>{str(stage.title, words.views214)}</h3>
                  {stageGoals.length ? (
                    stageGoals.map((g) => (
                      <GoalItem
                        key={g.id}
                        goal={g}
                        progress={snapshot.metrics.goal_progress.find(
                          (p) => p.id === g.id,
                        )}
                        currency={snapshot.company.currency}
                        select={select}
                      />
                    ))
                  ) : (
                    <Empty compact>{words.views215}</Empty>
                  )}
                </div>
              </li>
            );
          })}
        </ol>
      ) : (
        <Section title={words.views216}>
          {goals.length ? (
            goals
              .filter((g) => g.data.status !== "archived")
              .map((g) => (
                <GoalItem
                  key={g.id}
                  goal={g}
                  progress={snapshot.metrics.goal_progress.find(
                    (p) => p.id === g.id,
                  )}
                  currency={snapshot.company.currency}
                  select={select}
                />
              ))
          ) : (
            <Empty>{copy.empty.goal}</Empty>
          )}
        </Section>
      )}
      {route &&
        routeGoals.size > 0 &&
        goals.some((g) => !routeGoals.has(g.id)) && (
          <Section title={words.views217}>
            {goals
              .filter((g) => !routeGoals.has(g.id))
              .map((g) => (
                <GoalItem
                  key={g.id}
                  goal={g}
                  progress={snapshot.metrics.goal_progress.find(
                    (p) => p.id === g.id,
                  )}
                  currency={snapshot.company.currency}
                  select={select}
                />
              ))}
          </Section>
        )}
      <Section title={words.views218}>
        <div className="history-list">
          {goals
            .filter(
              (g) =>
                snapshot.metrics.goal_progress.find((p) => p.id === g.id)
                  ?.status === "achieved" || str(g.data.status) === "archived",
            )
            .map((g) => (
              <div key={g.id}>
                <Check size={16} aria-hidden="true" />
                <RecordButton record={g} onSelect={select} />
                <span>{date(g.updated_at)}</span>
              </div>
            ))}
        </div>
        {!goals.some((g) =>
          ["achieved", "archived"].includes(
            snapshot.metrics.goal_progress.find((p) => p.id === g.id)?.status ||
              str(g.data.status),
          ),
        ) && <p className="muted">{words.views219}</p>}
      </Section>
    </>
  );
}

export function Reports({ snapshot, select }: ViewProps) {
  const reports = [...records(snapshot, "report")].sort((a, b) =>
    b.updated_at.localeCompare(a.updated_at),
  );
  const forecasts = [...records(snapshot, "forecast")].sort((a, b) =>
    b.updated_at.localeCompare(a.updated_at),
  );
  const [type, setType] = useState("all");
  const shown = reports.filter((r) => type === "all" || r.data.type === type);
  return (
    <>
      <PageHeading title={words.views220} context={words.views221} />
      <Section
        title={words.views222}
        action={
          <label>
            <span className="sr-only">{words.views223}</span>
            <select value={type} onChange={(e) => setType(e.target.value)}>
              <option value="all">{words.views224}</option>
              <option value="weekly">{words.views225}</option>
              <option value="monthly">{words.views226}</option>
            </select>
          </label>
        }
      >
        {shown.length ? (
          <div className="report-list">
            {shown.map((r) => (
              <article key={r.id} className="report-item">
                <div className="report-date">
                  <span>
                    {str(r.data.type) === "monthly"
                      ? words.views227
                      : words.views228}
                  </span>
                  <strong>{date(str(r.data.period_end))}</strong>
                </div>
                <div className="report-copy">
                  <RecordButton record={r} onSelect={select} />
                  <p>
                    {date(str(r.data.period_start))} —{" "}
                    {date(str(r.data.period_end))}
                  </p>
                  <span>
                    {words.views229}
                    {date(str(r.data.as_of))} {words.views230}
                    {str(r.data.baseline_revision, words.views231)}{" "}
                    {words.views232}
                    {r.revision}
                  </span>
                  {str(r.data.body) && (
                    <p className="report-excerpt">
                      {str(r.data.body).slice(0, 180)}
                    </p>
                  )}
                </div>
                <button
                  className="icon-button"
                  onClick={() => select(r)}
                  aria-label={phrase("views233", title(r))}
                >
                  <ChevronRight size={20} />
                </button>
              </article>
            ))}
          </div>
        ) : (
          <Empty>{copy.empty.report}</Empty>
        )}
      </Section>
      <Section title={words.views234}>
        {forecasts.length ? (
          forecasts.map((f) => (
            <Forecast
              key={f.id}
              record={f}
              currency={snapshot.company.currency}
              select={select}
            />
          ))
        ) : (
          <Empty compact>{copy.empty.forecast}</Empty>
        )}
      </Section>
    </>
  );
}
function Forecast({
  record,
  currency,
  select,
}: {
  record: RecordEntry;
  currency: string;
  select: ViewProps["select"];
}) {
  return (
    <article className="forecast">
      <div className="forecast-header">
        <RecordButton record={record} onSelect={select} />
        <span>
          {words.views235}
          {date(str(record.data.as_of))} {words.views236}
          {str(record.data.baseline_revision, words.views237)}
        </span>
      </div>
      {arr(record.data.assumptions).length > 0 && (
        <div className="forecast-assumptions">
          <h3>{words.views238}</h3>
          <ul>
            {arr(record.data.assumptions).map((a, i) => (
              <li key={i}>{str(a, JSON.stringify(a))}</li>
            ))}
          </ul>
        </div>
      )}
      <ForecastContext record={record} />
      <EvidenceTrace record={record} />
      <div className="forecast-scenarios">
        {arr(record.data.scenarios).map((s, i) => {
          const scenario = obj(s);
          return (
            <div key={i}>
              <h3>{str(scenario.name, words.views239)}</h3>
              <div className="table-scroll">
                <table>
                  <thead>
                    <tr>
                      <th>{words.views240}</th>
                      <th className="numeric">{words.views241}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {arr(scenario.points).map((p, j) => {
                      const point = obj(p);
                      return (
                        <tr key={j}>
                          <td>{date(str(point.date))}</td>
                          <td className="numeric">
                            {money(num(point.cash_balance_minor), currency)}
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            </div>
          );
        })}
      </div>
    </article>
  );
}

export function Files({ snapshot, select }: ViewProps) {
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState("all");
  const [error, setError] = useState("");
  const [downloading, setDownloading] = useState("");
  const [preview, setPreview] = useState<RecordEntry | null>(null);
  const credential = getCredential();
  useEffect(() => { setPreview(null); }, [snapshot.company.id, credential]);
  const files = records(snapshot, "file");
  const categories = useMemo(
    () => [...new Set(files.map((f) => str(f.data.category)).filter(Boolean))],
    [files],
  );
  const shown = files.filter(
    (f) =>
      (category === "all" || f.data.category === category) &&
      `${title(f)} ${str(f.data.relative_path)}`
        .toLowerCase()
        .includes(query.toLowerCase()),
  );
  async function download(record: RecordEntry) {
    setError("");
    setDownloading(record.id);
    try {
      await api.downloadFile(
        snapshot.company.id,
        record.id,
        str(record.data.relative_path).split("/").at(-1) || title(record),
        record.revision,
      );
    } catch {
      setError(words.views242);
    } finally {
      setDownloading("");
    }
  }
  return (
    <>
      <PageHeading title={words.views243} context={words.views244} />
      <div className="filter-row">
        <label className="search-field">
          <Search size={17} aria-hidden="true" />
          <span className="sr-only">{words.views245}</span>
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={words.views246}
          />
        </label>
        <label>
          <span className="sr-only">{words.views247}</span>
          <select
            value={category}
            onChange={(e) => setCategory(e.target.value)}
          >
            <option value="all">{words.views248}</option>
            {categories.map((c) => (
              <option key={c} value={c}>
                {c}
              </option>
            ))}
          </select>
        </label>
      </div>
      {error && <ErrorNotice message={error} />}
      {shown.length ? (
        <div className="file-list">
          {shown.map((f) => (
            <article key={f.id} className="file-row">
              <div className="file-extension" aria-hidden="true">
                {str(f.data.relative_path)
                  .split(".")
                  .at(-1)
                  ?.slice(0, 5)
                  .toLowerCase() || "file"}
              </div>
              <div className="file-main">
                <RecordButton record={f} onSelect={select} />
                <p>{str(f.data.relative_path)}</p>
                <span>
                  {str(f.data.category)} · {date(f.updated_at)} {words.views249}
                  {f.revision}
                </span>
              </div>
              <span className="file-size number">
                {size(num(f.data.size_bytes))}
              </span>
              <div className="file-actions">
              <button className="text-button" onClick={() => setPreview(f)}>{filePreviewCopy.preview}</button>
              <button
                className="icon-button"
                onClick={() => void download(f)}
                disabled={downloading === f.id}
                aria-label={phrase("views250", title(f))}
              >
                <Download size={18} />
              </button>
              </div>
            </article>
          ))}
        </div>
      ) : (
        <Empty>
          {query || category !== "all" ? words.views251 : copy.empty.file}
        </Empty>
      )}
      {preview?.company_id === snapshot.company.id && <FilePreview record={preview} onClose={() => setPreview(null)} />}
    </>
  );
}

export function PageHeading({
  title,
  context,
}: {
  title: string;
  context?: string;
}) {
  return (
    <div className="page-heading">
      <h1>{title}</h1>
      {context && <p>{context}</p>}
    </div>
  );
}
