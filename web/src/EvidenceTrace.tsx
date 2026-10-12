import { copy, recordCopy } from "./copy";
import { arr, obj, str, date, number } from "./format";
import type { Json, RecordEntry } from "./types";

export function EvidenceTrace({ record }: { record: RecordEntry }) {
  const research = arr(record.data.research)
    .map(obj)
    .filter((r) => str(r.title) || str(r.claim));
  const calculation = obj(record.data.calculation);
  const hasCalculation = Object.keys(calculation).some(
    (key) => calculation[key] != null && calculation[key] !== "",
  );
  if (!research.length && !hasCalculation) return null;
  return (
    <details className="evidence-trace">
      <summary>{copy.evidence}</summary>
      {hasCalculation && (
        <div className="calculation-trace">
          {calculation.inputs != null && (
            <section>
              <h3>{copy.calculationInputs}</h3>
              <DataValue value={calculation.inputs} />
            </section>
          )}
          {str(calculation.method) && (
            <section>
              <h3>{copy.calculationMethod}</h3>
              <p>{str(calculation.method)}</p>
            </section>
          )}
          {calculation.result != null && (
            <section>
              <h3>{copy.calculationResult}</h3>
              <DataValue value={calculation.result} />
            </section>
          )}
        </div>
      )}
      {research.length > 0 && (
        <section className="research-trace">
          <h3>{copy.research}</h3>
          <ul>
            {research.map((source, index) => {
              const url = sourceUrl(str(source.url));
              return (
                <li key={index}>
                  <div>
                    {url ? (
                      <a href={url} target="_blank" rel="noopener noreferrer">
                        {str(source.title, url)}
                      </a>
                    ) : (
                      str(source.title)
                    )}
                    {str(source.observed_on) && (
                      <time>{date(str(source.observed_on))}</time>
                    )}
                  </div>
                  {str(source.claim) && <p>{str(source.claim)}</p>}
                </li>
              );
            })}
          </ul>
        </section>
      )}
    </details>
  );
}
function DataValue({ value }: { value: Json }) {
  if (value == null) return null;
  if (Array.isArray(value))
    return (
      <ul className="trace-values">
        {value.map((v, i) => (
          <li key={i}>
            <DataValue value={v} />
          </li>
        ))}
      </ul>
    );
  if (typeof value === "object")
    return (
      <dl className="trace-values">
        {Object.entries(value).map(([label, v]) => (
          <div key={label}>
            <dt>{label}</dt>
            <dd>
              <DataValue value={v} />
            </dd>
          </div>
        ))}
      </dl>
    );
  if (typeof value === "boolean")
    return <span>{value ? recordCopy.yes : recordCopy.no}</span>;
  if (typeof value === 'number') return <span>{number(value)}</span>;
  return <span>{String(value)}</span>;
}
function sourceUrl(value: string): string | null {
  try {
    const url = new URL(value);
    return ["https:", "http:"].includes(url.protocol) &&
      !url.username &&
      !url.password
      ? url.href
      : null;
  } catch {
    return null;
  }
}
