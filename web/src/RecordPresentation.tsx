import type { ReactNode } from "react";
import { copy, recordCopy as text } from "./copy";
import {
  arr,
  date,
  money,
  num,
  number,
  obj,
  size,
  sourceText,
  str,
  title,
} from "./format";
import { Status } from "./components";
import { TurnState } from "./TurnState";
import { narrative } from "./narrativeCopy";
import { DecisionSettlement, decisionSelectionText, decisionStatus } from "./Play";
import type { Json, RecordEntry, RecordKind, Snapshot } from "./types";
import { gameSystemsCopy as gameText, gameLabel } from "./gameSystemsCopy";

export function recordTitle(record: RecordEntry): string {
  return str(
    record.data.title || record.data.name || record.data.description,
    text[record.kind],
  );
}

export function RecordPresentation({
  record,
  snapshot,
}: {
  record: RecordEntry;
  snapshot: Snapshot;
}) {
  const d = record.data;
  const currency = snapshot.company.currency;
  const named = (kind: RecordKind, id: Json | undefined) => {
    const key = str(id);
    const found = snapshot.records[kind]?.find((r) => r.id === key);
    return found ? title(found) : key;
  };
  const status = str(d.status) ? <Status value={str(d.status)} /> : undefined;
  const commonDescription =
    str(d.description) && str(d.description) !== recordTitle(record) ? (
      <p className="record-description">{str(d.description)}</p>
    ) : null;
  const source = sourceText(record) ? (
    <section className="record-section">
      <h3>{text.source}</h3>
      <p>{sourceText(record)}</p>
    </section>
  ) : null;
  const currencyTarget = ["mrr", "cash_balance"].includes(str(d.metric));
  const stagedGoals = new Set(
    arr(d.stages)
      .flatMap((stage) => arr(obj(stage).goal_ids))
      .map((id) => str(id)),
  );
  const unassignedGoals = arr(d.goal_ids).filter(
    (id) => !stagedGoals.has(str(id)),
  );
  const currencyValue = (v: Json | undefined) => money(num(v), currency);
  let content: ReactNode;
  switch (record.kind) {
    case "network_entity":
      content = <>{commonDescription}<Facts items={[[text.type, gameLabel("nodeTypes", str(d.type))], [text.role, str(d.role)], [text.status, status]]} /></>;
      break;
    case "relationship": {
      const referenceName = (value: Json | undefined) => {
        const reference = obj(value);
        if (reference.kind === "company") return snapshot.company.name;
        const linked = snapshot.records[str(reference.kind) as RecordKind]?.find((item) => item.id === str(reference.id));
        return linked ? recordTitle(linked) : gameText.networkObject;
      };
      content = <>{commonDescription}<Facts items={[[gameText.startObject, referenceName(d.source)], [gameText.targetObject, referenceName(d.target)], [text.type, gameLabel("relationships", str(d.type))], [text.status, gameLabel("statuses", str(d.status))], [text.start, str(d.since) ? date(str(d.since)) : undefined], [text.end, str(d.until) ? date(str(d.until)) : undefined]]} /></>;
      break;
    }
    case "profit_entry": {
      const subject = obj(d.subject);
      const linked = snapshot.records[str(subject.kind) as RecordKind]?.find((item) => item.id === str(subject.id));
      content = <><Facts items={[[gameText.subject, subject.kind === "company" ? snapshot.company.name : linked ? recordTitle(linked) : gameText.networkObject], [gameText.period, `${date(str(d.period_start))} – ${date(str(d.period_end))}`], [gameText.basis, d.basis === "cash" ? gameText.cash : gameText.recognized], [gameText.revenue, currencyValue(d.revenue_minor)], [gameText.cost, currencyValue(d.cost_minor)], [text.status, d.cost_coverage === "complete" ? gameText.profitComplete : gameText.profitPartial]]} />{str(d.note) && <p>{str(d.note)}</p>}{str(d.assumptions) && <Assumptions values={[d.assumptions]} />}</>;
      break;
    }
    case "quest": {
      const completion = obj(d.completion);
      content = <>{commonDescription}<Facts items={[[text.category, gameText[str(d.category) as "main" | "side" | "daily"] || str(d.category)], [text.status, gameLabel("statuses", str(d.status))], [text.deadline, str(d.deadline) ? date(str(d.deadline)) : undefined]]} />{str(d.outcome) && <section className="record-section"><h3>{gameText.outcome}</h3><p>{str(d.outcome)}</p></section>}{str(completion.summary) && <section className="record-section"><h3>{gameText.results}</h3><p>{str(completion.summary)}</p></section>}</>;
      break;
    }
    case "alert":
      content = <><Facts items={[[text.type, gameText[str(d.type) as "risk" | "opportunity" | "event"] || str(d.type)], [text.status, gameLabel("statuses", str(d.status))], [gameText.basis, gameText[str(d.basis) as "observed" | "forecast" | "simulation"] || str(d.basis)], [text.start, str(d.window_start) ? date(str(d.window_start)) : undefined], [text.end, str(d.window_end) ? date(str(d.window_end)) : undefined]]} />{str(d.body) && <p className="story-prose">{str(d.body)}</p>}<Assumptions values={arr(d.assumptions)} /></>;
      break;
    case "decision": {
      const resolution = obj(d.resolution);
      const state = str(d.status);
      content = <>
        <Facts items={[[text.status, decisionStatus(record)], [narrative.deadline, str(d.deadline) ? date(str(d.deadline)) : undefined]]} />
        <section className="record-section"><h3>{narrative.currentEvent}</h3><p className="story-prose">{str(d.situation)}</p></section>
        {arr(d.options).length > 0 && <section className="record-section"><h3>{narrative.choose}</h3><ol className="record-list">{arr(d.options).map((option, i) => { const o = obj(option); return <li key={i}><strong>{str(o.title)}</strong>{str(o.description) && <p>{str(o.description)}</p>}{num(o.estimated_cost_minor) != null && <p>{narrative.expectedCost}: {money(num(o.estimated_cost_minor), currency)}</p>}{arr(o.assumptions).length > 0 && <Assumptions values={arr(o.assumptions)} />}</li>; })}</ol></section>}
        {decisionSelectionText(record) && <section className="record-section"><h3>{narrative.chosen}</h3><p>{decisionSelectionText(record)}</p></section>}
        {str(resolution.summary) && <section className="record-section"><h3>{narrative.outcome}</h3><p className="story-prose">{str(resolution.summary)}</p></section>}
        {state === "resolved" && <DecisionSettlement decision={record} snapshot={snapshot} />}
        {source}
      </>;
      break;
    }
    case "simulation_turn":
      content = (
        <>
          <Facts
            items={[
              [
                text.occurredOn,
                d.occurred_on ? date(str(d.occurred_on)) : undefined,
              ],
            ]}
          />
          {str(d.action) && (
            <section className="record-section">
              <h3>{text.choice}</h3>
              <p>{str(d.action)}</p>
            </section>
          )}
          {str(d.story) && (
            <section className="record-section">
              <h3>{text.story}</h3>
              <p>{str(d.story)}</p>
            </section>
          )}
          {arr(d.outcomes).length > 0 && (
            <section className="record-section">
              <h3>{text.outcomes}</h3>
              <ul className="record-outcomes">
                {arr(d.outcomes).map((value, i) => {
                  const o = obj(value);
                  const outcome = str(o.status);
                  return (
                    <li key={i} className={`outcome-${outcome}`}>
                      <div>
                        <strong>{str(o.title)}</strong>
                        <span>
                          {outcome === "success"
                            ? text.success
                            : outcome === "failure"
                              ? text.failure
                              : text.ongoing}
                        </span>
                      </div>
                      {str(o.evidence) && <p>{str(o.evidence)}</p>}
                    </li>
                  );
                })}
              </ul>
            </section>
          )}
          <Assumptions values={arr(d.assumptions)} />
          <TurnState record={record} currency={currency} />
          <Facts
            items={[
              [text.beforeVersion, num(d.before_revision)],
              [text.afterVersion, num(d.after_revision)],
            ]}
          />
        </>
      );
      break;
    case "route":
      content = (
        <>
          {str(d.story) && <p className="record-description">{str(d.story)}</p>}
          {arr(d.stages).length > 0 && (
            <section className="record-section">
              <h3>{text.stages}</h3>
              <ol className="record-stages">
                {arr(d.stages).map((value, i) => {
                  const stage = obj(value);
                  return (
                    <li key={i}>
                      <h4>{str(stage.title)}</h4>
                      {arr(stage.goal_ids).length > 0 && (
                        <ul>
                          {arr(stage.goal_ids).map((id, j) => (
                            <li key={j}>{named("goal", id)}</li>
                          ))}
                        </ul>
                      )}
                    </li>
                  );
                })}
              </ol>
            </section>
          )}
          {unassignedGoals.length > 0 && (
            <section className="record-section">
              <h3>{text.goals}</h3>
              <ul className="record-list">
                {unassignedGoals.map((id, i) => (
                  <li key={i}>{named("goal", id)}</li>
                ))}
              </ul>
            </section>
          )}
        </>
      );
      break;
    case "report":
      content = (
        <>
          <Facts
            items={[
              [
                text.type,
                str(d.type) === "monthly"
                  ? text.monthlyReport
                  : text.weeklyReport,
              ],
              [
                text.period,
                `${date(str(d.period_start))} – ${date(str(d.period_end))}`,
              ],
              [text.asOf, d.as_of ? date(str(d.as_of)) : undefined],
              [text.baselineVersion, num(d.baseline_revision)],
            ]}
          />
          {str(d.body) && <div className="report-body">{str(d.body)}</div>}
        </>
      );
      break;
    case "forecast":
      content = (
        <>
          <Facts
            items={[
              [text.asOf, d.as_of ? date(str(d.as_of)) : undefined],
              [text.baselineVersion, num(d.baseline_revision)],
            ]}
          />
          <Assumptions values={arr(d.assumptions)} />
          {arr(d.scenarios).length > 0 && (
            <section className="record-section">
              <h3>{text.scenarios}</h3>
              {arr(d.scenarios).map((value, i) => {
                const scenario = obj(value);
                return (
                  <div className="record-scenario" key={i}>
                    <h4>{str(scenario.name, text.scenario)}</h4>
                    <div className="table-scroll">
                      <table>
                        <thead>
                          <tr>
                            <th>{text.occurredOn}</th>
                            <th className="numeric">{text.projectedCash}</th>
                          </tr>
                        </thead>
                        <tbody>
                          {arr(scenario.points).map((p, j) => {
                            const point = obj(p);
                            return (
                              <tr key={j}>
                                <td>{date(str(point.date))}</td>
                                <td className="numeric">
                                  {currencyValue(point.cash_balance_minor)}
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
            </section>
          )}
        </>
      );
      break;
    case "account":
      content = (
        <>
          {commonDescription}
          <Facts
            items={[
              [text.openingBalance, currencyValue(d.opening_balance_minor)],
              [text.start, d.opening_on ? date(str(d.opening_on)) : undefined],
            ]}
          />
        </>
      );
      break;
    case "transaction":
      content = (
        <>
          {commonDescription}
          <Facts
            items={[
              [text.amount, currencyValue(d.amount_minor)],
              [
                text.direction,
                d.direction === "inflow" ? text.inflow : text.outflow,
              ],
              [
                text.category,
                copy.categories[str(d.category)] || str(d.category),
              ],
              [text.account, named("account", d.account_id)],
              [text.customer, named("customer", d.customer_id) || undefined],
              [
                text.occurredOn,
                d.occurred_on ? date(str(d.occurred_on)) : undefined,
              ],
            ]}
          />
          {source}
        </>
      );
      break;
    case "business":
      content = (
        <>
          {commonDescription}
          <Facts
            items={[
              [text.type, d.type === "product" ? text.product : text.project],
              [text.status, status],
            ]}
          />
        </>
      );
      break;
    case "customer":
      content = (
        <>
          {commonDescription}
          <Facts items={[[text.status, status]]} />
        </>
      );
      break;
    case "resource":
      content = (
        <>
          {commonDescription}
          <Facts
            items={[
              [text.type, copy.resourceTypes[str(d.type)] || str(d.type)],
              [text.role, str(d.role) || undefined],
              [text.status, status],
            ]}
          />
        </>
      );
      break;
    case "subscription":
      content = (
        <>
          {commonDescription}
          <Facts
            items={[
              [text.customer, named("customer", d.customer_id)],
              [text.amount, currencyValue(d.amount_minor)],
              [
                text.interval,
                num(d.interval_months) === 12 ? text.yearly : text.monthly,
              ],
              [text.status, status],
              [text.start, d.starts_on ? date(str(d.starts_on)) : undefined],
              [text.end, d.ends_on ? date(str(d.ends_on)) : undefined],
              [
                text.discount,
                d.recurring_discount_minor != null
                  ? currencyValue(d.recurring_discount_minor)
                  : undefined,
              ],
              [
                text.tax,
                d.tax_minor != null ? currencyValue(d.tax_minor) : undefined,
              ],
            ]}
          />
        </>
      );
      break;
    case "goal":
      content = (
        <>
          {str(d.outcome) && (
            <p className="record-description">{str(d.outcome)}</p>
          )}
          <Facts
            items={[
              [text.status, status],
              [
                text.metric,
                copy.metrics[str(d.metric)] || str(d.metric) || undefined,
              ],
              [
                text.target,
                num(d.target) == null
                  ? undefined
                  : currencyTarget
                    ? currencyValue(d.target)
                    : num(d.target),
              ],
              [text.deadline, d.deadline ? date(str(d.deadline)) : undefined],
              [text.stage, str(d.stage) || undefined],
            ]}
          />
          {arr(d.depends_on).length > 0 && (
            <section className="record-section">
              <h3>{text.dependencies}</h3>
              <ul className="record-list">
                {arr(d.depends_on).map((id, i) => (
                  <li key={i}>{named("goal", id)}</li>
                ))}
              </ul>
            </section>
          )}
          {str(d.evidence) && (
            <section className="record-section">
              <h3>{text.evidence}</h3>
              <p>{str(d.evidence)}</p>
            </section>
          )}
        </>
      );
      break;
    case "file":
      content = (
        <>
          {commonDescription}
          <Facts
            items={[
              [text.path, str(d.relative_path)],
              [text.category, str(d.category) || undefined],
              [
                text.fileSize,
                num(d.size_bytes) == null ? undefined : size(num(d.size_bytes)),
              ],
            ]}
          />
        </>
      );
      break;
    default:
      content = commonDescription;
  }
  return <div className="record-presentation">{content}</div>;
}
function Facts({ items }: { items: [string, ReactNode][] }) {
  const shown = items.filter(([, value]) => value != null && value !== "");
  return shown.length ? (
    <dl className="detail-fields">
      {shown.map(([label, value], i) => (
        <div key={i}>
          <dt>{label}</dt>
          <dd>{typeof value === "number" ? number(value) : value}</dd>
        </div>
      ))}
    </dl>
  ) : null;
}
function Assumptions({ values }: { values: Json[] }) {
  const textValues = values.filter(
    (v) => typeof v === "string" || typeof v === "number",
  );
  return textValues.length ? (
    <section className="record-section">
      <h3>{text.assumptions}</h3>
      <ul className="record-list">
        {textValues.map((v, i) => (
          <li key={i}>{typeof v === "number" ? number(v) : str(v)}</li>
        ))}
      </ul>
    </section>
  ) : null;
}
