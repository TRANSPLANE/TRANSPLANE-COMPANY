import { Coin, Wallet, Flag, Sprout } from "./icons";
import { copy } from "./copy";
import { words } from "./labels";
import { money, number } from "./format";
import type { Snapshot } from "./types";

export function GameHud({ snapshot }: { snapshot: Snapshot }) {
  const { company, metrics } = snapshot;
  return (
    <div className="metric-strip resource-hud" aria-label={copy.hudLabel}>
      <div className="metric primary-metric">
        <div className="hud-label">
          <Coin size={24} />
          <span>{words.views151}</span>
        </div>
        <strong className="number">
          {money(metrics.cash_balance_minor, company.currency)}
        </strong>
        <span className="metric-context">
          {metrics.cash_balance_minor == null ? words.views152 : words.views153}
        </span>
      </div>
      <div className="metric">
        <div className="hud-label">
          <Sprout size={24} />
          <span>MRR</span>
        </div>
        <strong className="number">
          {money(metrics.mrr_minor, company.currency)}
        </strong>
        <span className="metric-context">{words.views154}</span>
      </div>
      <div className="metric">
        <div className="hud-label">
          <Wallet size={24} />
          <span>{words.views155}</span>
        </div>
        <strong className="number">
          {money(metrics.net_operating_cashflow_minor, company.currency)}
        </strong>
        <span className="metric-context">{words.views156}</span>
      </div>
      <div className="metric">
        <div className="hud-label">
          <Flag size={24} />
          <span>{words.views157}</span>
        </div>
        <strong className="number">
          {metrics.runway_months == null ? (
            words.views158
          ) : (
            <>
              {number(metrics.runway_months, {
                minimumFractionDigits: 1,
                maximumFractionDigits: 1,
              })}
              <small>{words.views159}</small>
            </>
          )}
        </strong>
        <span className="metric-context">
          {copy.runway[metrics.runway_status] || metrics.runway_status}
        </span>
      </div>
    </div>
  );
}
