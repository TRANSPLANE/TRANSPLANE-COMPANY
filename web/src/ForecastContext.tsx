import { CopyButton } from "./components";
import { arr, date, obj, str } from "./format";
import { forecastContextCopy as text } from "./forecastContextCopy";
import type { Json, RecordEntry } from "./types";
import "./forecast-context.css";

function day(value: Json | undefined) {
  const input = str(value);
  if (!/^\d{4}-\d{2}-\d{2}$/.test(input)) return "";
  const parsed = new Date(`${input}T12:00:00Z`);
  return Number.isFinite(parsed.getTime()) && parsed.toISOString().slice(0, 10) === input ? input : "";
}
export function forecastSourceTiming(source: Record<string, Json>, asOf: Json | undefined) {
  const baseline = day(asOf);
  const expires = day(source.expires_on);
  const observed = day(source.observed_on);
  const retrieved = day(source.retrieved_on);
  return {
    observed, retrieved, expires,
    expired: Boolean(baseline && expires && expires < baseline),
    afterBaseline: Boolean(baseline && (observed > baseline || retrieved > baseline)),
    freshness: !baseline || !expires || !observed || !retrieved ? "unknown" : expires < baseline ? "expired" : "dated",
  };
}
export function forecastUpdatePrompt(record: RecordEntry, origin: string) {
  return [text.promptIntro, `${text.promptEntry}: ${new URL("/api/v1/agent-entry", origin).href}`, `${text.promptCompany}: ${record.company_id}`, `${text.promptForecast}: ${record.id} / ${record.revision}`, `${text.promptBaseline}: ${str(record.data.as_of) || text.notRecorded}`, text.promptRead, text.promptSources, text.promptFields].join("\n\n");
}
function sourceKind(kind: string) { return kind === "company" || kind === "public" || kind === "world" || kind === "assumption" ? text[kind] : text.unknownKind; }
export function ForecastContext({ record }: { record: RecordEntry }) {
  const sources = arr(record.data.context_sources).map(obj);
  const method = str(record.data.analysis_method);
  const limitations = arr(record.data.limitations).map((value) => str(value)).filter(Boolean);
  const origin = typeof location === "undefined" ? "" : location.origin;
  const prompt = origin ? forecastUpdatePrompt(record, origin) : "";
  return <section className="forecast-context"><div className="forecast-context-heading"><h3>{text.title}</h3>{prompt && <CopyButton value={prompt} label={text.update} />}</div>{sources.length ? <ul className="forecast-context-sources">{sources.map((source, index) => {
    const timing = forecastSourceTiming(source, record.data.as_of);
    return <li key={index}><div className="forecast-source-heading"><strong>{str(source.title) || text.notRecorded}</strong><span>{sourceKind(str(source.kind))}</span></div><dl className="forecast-source-times"><div><dt>{text.observed}</dt><dd>{timing.observed ? <time dateTime={timing.observed}>{date(timing.observed)}</time> : text.notRecorded}</dd></div><div><dt>{text.retrieved}</dt><dd>{timing.retrieved ? <time dateTime={timing.retrieved}>{date(timing.retrieved)}</time> : text.notRecorded}</dd></div>{timing.expires && <div><dt>{text.expires}</dt><dd><time dateTime={timing.expires}>{date(timing.expires)}</time></dd></div>}</dl>{timing.expired && <p className="forecast-source-stale">{text.expired}</p>}{timing.afterBaseline && <p className="forecast-source-timing">{text.afterBaseline}</p>}{(str(source.reference) || str(source.summary)) && <details className="forecast-source-details"><summary>{text.details}</summary>{str(source.summary) && <p>{str(source.summary)}</p>}{str(source.reference) && <p className="forecast-source-reference"><span>{text.reference}</span>{str(source.reference)}</p>}</details>}</li>;
  })}</ul> : <p className="forecast-source-unknown">{text.legacy}</p>}{method && <details className="forecast-analysis-method"><summary>{text.method}</summary><p>{method}</p></details>}{limitations.length > 0 && <details className="forecast-limitations"><summary>{text.limitations}</summary><ul>{limitations.map((value, index) => <li key={index}>{value}</li>)}</ul></details>}{prompt && <details className="forecast-update-task"><summary>{text.taskDetails}</summary><pre>{prompt}</pre></details>}</section>;
}
