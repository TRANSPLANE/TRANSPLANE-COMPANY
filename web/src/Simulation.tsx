import { words, phrase } from "./labels";
import { useState } from "react";
import {
  CheckCircle2,
  ChevronRight,
  CircleDot,
  GitBranch,
  XCircle,
} from "./icons";
import { api } from "./api";
import { EvidenceTrace } from "./EvidenceTrace";
import { TurnState } from "./TurnState";
import { agentPlaybookPrompt } from "./Playbook";
import { arr, date, obj, str, title } from "./format";
import {
  CopyButton,
  Empty,
  ErrorNotice,
  RecordButton,
  Section,
} from "./components";
import { PageHeading } from "./views";
import type { Company, Json, RecordEntry, Snapshot } from "./types";

export function Simulation({
  snapshot,
  companies,
  owner,
  select,
  onFork,
  onCompany,
}: {
  snapshot: Snapshot;
  companies: Company[];
  owner: boolean;
  select: (r: RecordEntry) => void;
  onFork: (companyId: string) => Promise<void>;
  onCompany: (id: string) => void;
}) {
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const simulation = snapshot.company.mode === "simulation";
  const turns = [...(snapshot.records.simulation_turn || [])].sort((a, b) =>
    a.created_at.localeCompare(b.created_at),
  );
  const branches = companies.filter(
    (c) => c.baseline_company_id === snapshot.company.id,
  );
  async function fork() {
    setBusy(true);
    setError("");
    try {
      const result = await api.fork(
        snapshot.company,
        phrase("simulation121", snapshot.company.name),
      );
      await onFork(result.company.id);
    } catch (e) {
      setError(e instanceof Error ? e.message : words.simulation122);
    } finally {
      setBusy(false);
    }
  }
  const agentPrompt = agentPlaybookPrompt(snapshot.company);
  return (
    <>
      <PageHeading
        title={words.simulation124}
        context={simulation ? words.simulation125 : words.simulation126}
      />
      {error && <ErrorNotice message={error} />}
      {simulation ? (
        <>
          <div className="simulation-intro">
            <div>
              <strong>{words.simulation127}</strong>
              <p>{words.simulation128}</p>
              {snapshot.company.baseline_company_id && (
                <span className="muted">
                  {words.simulation129}
                  {snapshot.company.baseline_company_id} {words.simulation130}
                  {snapshot.company.baseline_revision}
                </span>
              )}
            </div>
            <CopyButton value={agentPrompt} label={words.simulation131} />
          </div>
          <Section title={words.simulation132}>
            {turns.length ? (
              <ol className="simulation-timeline">
                {turns.map((turn, i) => (
                  <li key={turn.id}>
                    <div className="turn-index">{i + 1}</div>
                    <article className="turn-content">
                      <div className="turn-heading">
                        <RecordButton record={turn} onSelect={select} />
                        <span>
                          {date(str(turn.data.occurred_on, turn.created_at))}
                        </span>
                      </div>
                      {str(turn.data.action) && (
                        <div className="turn-action">
                          <span>{words.simulation133}</span>
                          <p>{str(turn.data.action)}</p>
                        </div>
                      )}
                      {str(turn.data.story) && (
                        <p className="turn-story">{str(turn.data.story)}</p>
                      )}
                      <div className="turn-outcomes">
                        {arr(turn.data.outcomes).map((o, j) => {
                          const outcome = obj(o);
                          const status = str(outcome.status);
                          return (
                            <div key={j} className={`outcome-${status}`}>
                              {status === "success" ? (
                                <CheckCircle2 size={18} aria-hidden="true" />
                              ) : status === "failure" ? (
                                <XCircle size={18} aria-hidden="true" />
                              ) : (
                                <CircleDot size={18} aria-hidden="true" />
                              )}
                              <div>
                                <strong>{str(outcome.title)}</strong>
                                <span>
                                  {status === "success"
                                    ? words.simulation134
                                    : status === "failure"
                                      ? words.simulation135
                                      : words.simulation136}
                                </span>
                                {str(outcome.evidence) && (
                                  <p>{str(outcome.evidence)}</p>
                                )}
                              </div>
                            </div>
                          );
                        })}
                      </div>
                      {arr(turn.data.assumptions).length > 0 && (
                        <details>
                          <summary>{words.simulation137}</summary>
                          <ul>
                            {arr(turn.data.assumptions).map((a, j) => (
                              <li key={j}>{str(a, JSON.stringify(a))}</li>
                            ))}
                          </ul>
                        </details>
                      )}
                      <TurnState
                        record={turn}
                        currency={snapshot.company.currency}
                      />
                      <EvidenceTrace record={turn} />
                      <div className="turn-version">
                        {words.simulation138}
                        {str(
                          turn.data.before_revision,
                          words.simulation139,
                        )} →{" "}
                        {str(turn.data.after_revision, words.simulation140)}
                      </div>
                    </article>
                  </li>
                ))}
              </ol>
            ) : (
              <Empty>{words.simulation141}</Empty>
            )}
          </Section>
        </>
      ) : (
        <>
          <div className="simulation-intro">
            <div>
              <h2>{words.simulation142}</h2>
              <p>{words.simulation143}</p>
            </div>
            {owner && (
              <button
                className="primary-button"
                onClick={() => void fork()}
                disabled={busy}
              >
                <GitBranch size={17} aria-hidden="true" />
                {busy ? words.simulation144 : words.simulation145}
              </button>
            )}
          </div>
          <Section title={words.simulation146}>
            {branches.length ? (
              <div className="branch-list">
                {branches.map((c) => (
                  <button
                    key={c.id}
                    className="branch-row"
                    onClick={() => onCompany(c.id)}
                  >
                    <GitBranch size={20} aria-hidden="true" />
                    <span>
                      <strong>{c.name}</strong>
                      <small>
                        {words.simulation147}
                        {c.baseline_revision} · {date(c.created_at)}
                      </small>
                    </span>
                    <span className="simulation-badge">
                      {words.simulation148}
                    </span>
                    <ChevronRight size={18} aria-hidden="true" />
                  </button>
                ))}
              </div>
            ) : (
              <Empty compact>{words.simulation149}</Empty>
            )}
          </Section>
        </>
      )}
    </>
  );
}
