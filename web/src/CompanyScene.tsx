import { Flag, Building2 } from "./icons";
import { date, str, title } from "./format";
import type { Snapshot } from "./types";
import { copy } from "./copy";

export function CompanyScene({ snapshot }: { snapshot: Snapshot }) {
  const routes = [...(snapshot.records.route || [])].sort((a, b) =>
    a.updated_at.localeCompare(b.updated_at),
  );
  const route = routes.at(-1);
  const resources = snapshot.records.resource || [];
  return (
    <section className="company-scene">
      <div className="scene-title">
        <Building2 size={24} />
        <div>
          <h1>{snapshot.company.name}</h1>
          <p>
            {date(snapshot.metrics.as_of)} · {snapshot.company.currency}
          </p>
        </div>
        {route && (
          <div className="scene-chapter">
            <Flag size={24} />
            <span>{title(route)}</span>
          </div>
        )}
      </div>
      <img
        src="/assets/company-pixel-scene.png"
        alt=""
        width="2172"
        height="724"
        fetchPriority="high"
      />
      <div className="scene-footer">
        {snapshot.company.description && (
          <p className="company-description">{snapshot.company.description}</p>
        )}
        {resources.length > 0 && (
          <div className="company-party" aria-label={copy.resources}>
            {resources.slice(0, 4).map((r) => (
              <span key={r.id}>
                <span className="party-type">
                  {copy.resourceTypes[str(r.data.type)] || str(r.data.type)}
                </span>
                {title(r)}
              </span>
            ))}
          </div>
        )}
      </div>
    </section>
  );
}
