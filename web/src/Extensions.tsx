import { recordCopy, copy } from "./copy";
import { date, money, number, percent } from "./format";
import { Status } from "./components";
import type { ExtensionField, Snapshot } from "./types";

export function Extensions({ snapshot }: { snapshot: Snapshot }) {
  const results = (snapshot.extensions || []).filter(
    (e) => e.metrics.length || e.views.length,
  );
  if (!results.length) return null;
  return (
    <div className="extensions">
      {results.map((extension) => (
        <section key={extension.id} className="extension">
          <div className="section-heading">
            <h2>{extension.name}</h2>
            {extension.enabled === false && <Status value="disabled" />}
            <span className="muted">{date(extension.updated_at)}</span>
          </div>
          {extension.metrics.length > 0 && (
            <dl className="extension-metrics">
              {extension.metrics.map((field, index) => (
                <div key={field.id || index}>
                  <dt>{field.label}</dt>
                  <dd className={field.format === "text" ? "" : "number"}>
                    {extensionValue(field, snapshot.company.currency)}
                    {field.unit && <small>{field.unit}</small>}
                  </dd>
                </div>
              ))}
            </dl>
          )}
          {extension.views.map((view) => (
            <details className="extension-view" key={view.id}>
              <summary>{view.title}</summary>
              <dl>
                {view.fields.map((field, index) => (
                  <div key={field.id || index}>
                    <dt>{field.label}</dt>
                    <dd>
                      {extensionValue(field, snapshot.company.currency)}
                      {field.unit && <span> {field.unit}</span>}
                    </dd>
                  </div>
                ))}
              </dl>
            </details>
          ))}
          <details className="extension-provenance">
            <summary>{recordCopy.source}</summary>
            <dl>
              <div>
                <dt>{recordCopy.plugin}</dt>
                <dd>{extension.id}</dd>
              </div>
              <div>
                <dt>{recordCopy.request}</dt>
                <dd>{extension.request_id}</dd>
              </div>
              <div>
                <dt>{copy.updated}</dt>
                <dd>{extension.updated_at}</dd>
              </div>
            </dl>
          </details>
        </section>
      ))}
    </div>
  );
}

function extensionValue(field: ExtensionField, currency: string): string {
  if (field.value == null) return copy.unknown;
  if (field.format === "money" && typeof field.value === "number")
    return money(field.value, currency);
  if (field.format === "percent" && typeof field.value === "number")
    return percent(field.value);
  if (field.format === "number" && typeof field.value === "number")
    return number(field.value);
  if (typeof field.value === "boolean")
    return field.value ? recordCopy.yes : recordCopy.no;
  return String(field.value);
}
