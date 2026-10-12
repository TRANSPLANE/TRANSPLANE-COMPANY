import { copy } from "./copy";
import { date, money, num, obj } from "./format";
import type { RecordEntry } from "./types";

export function TurnState({
  record,
  currency,
}: {
  record: RecordEntry;
  currency: string;
}) {
  const metrics = obj(record.data.metrics_snapshot);
  if (!Object.keys(metrics).length) return null;
  return (
    <div className="turn-settlement">
      <div>
        <h3>{copy.turnState}</h3>
        {typeof metrics.as_of === "string" && (
          <span>{date(metrics.as_of)}</span>
        )}
      </div>
      <dl>
        <div>
          <dt>{copy.turnCash}</dt>
          <dd className="number">
            {money(num(metrics.cash_balance_minor), currency)}
          </dd>
        </div>
        <div>
          <dt>{copy.turnMrr}</dt>
          <dd className="number">{money(num(metrics.mrr_minor), currency)}</dd>
        </div>
      </dl>
    </div>
  );
}
