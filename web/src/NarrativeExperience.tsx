import { arr, date, number, obj, str } from "./format";
import { ArrowRight, Flag } from "./icons";
import { narrative as text } from "./narrativeCopy";
import type { Json, RecordEntry } from "./types";

function different(value: Json | undefined, existing: string[]) {
  const content = str(value).trim();
  return content && !existing.some((item) => item.trim() === content) ? content : "";
}

export function NarrativeExperience({ decision, records = [], select }: {
  decision: RecordEntry; records?: RecordEntry[]; select?: (record: RecordEntry) => void;
}) {
  const experience = obj(decision.data.experience);
  const scene = str(experience.scene).trim();
  const situation = different(decision.data.situation, [scene]);
  const whyNow = different(experience.why_now, [situation, scene]);
  const stakes = different(experience.stakes, [situation, scene, whyNow]);
  const cast = arr(experience.cast).map(obj).filter((person) => str(person.name));
  const callbacks = arr(experience.callbacks).map(obj).filter((callback) => str(callback.summary));
  if (!scene && !situation && !whyNow && !stakes && !cast.length && !callbacks.length) return null;
  return <div className="event-experience">
    {scene && <p className="scene-setting story-prose">{scene}</p>}
    {situation && <p className="story-prose">{situation}</p>}
    {whyNow && <p className="scene-trigger story-prose">{whyNow}</p>}
    {cast.length > 0 && <div className="scene-cast">{cast.map((person, index) => <figure className="cast-voice" key={str(person.id) || index}>
      <figcaption><strong>{str(person.name)}</strong>{str(person.role) && <span>{str(person.role)}</span>}</figcaption>
      {str(person.concern) && <p className="cast-concern">{str(person.concern)}</p>}
      {str(person.line) && <blockquote>{str(person.line)}</blockquote>}
    </figure>)}</div>}
    {callbacks.length > 0 && <div className="scene-callbacks"><span>{text.earlierChoice}</span>{callbacks.map((callback, index) => {
      const source = records.find((record) => record.id === str(callback.decision_id));
      return source && select ? <button type="button" className="record-link" onClick={() => select(source)} key={index}>{str(callback.summary)}<ArrowRight size={14} aria-hidden="true" /></button> : <p key={index}>{str(callback.summary)}</p>;
    })}</div>}
    {stakes && <div className="scene-stakes"><strong>{text.atStake}</strong><p>{stakes}</p></div>}
  </div>;
}

export function NarrativeChapter({ decision }: { decision: RecordEntry }) {
  const chapter = obj(obj(decision.data.experience).chapter);
  if (!str(chapter.title)) return null;
  return <div className="event-chapter"><span>{str(chapter.title)}</span>{typeof chapter.turn === "number" && typeof chapter.planned_turns === "number" && <small>{text.chapterTurn} {number(chapter.turn)} / {number(chapter.planned_turns)}</small>}</div>;
}

export function narrativeCastName(castId: string, decisions: RecordEntry[]) {
  for (const decision of decisions) {
    const person = arr(obj(decision.data.experience).cast).map(obj).find((entry) => str(entry.id) === castId);
    if (person && str(person.name)) return str(person.name);
  }
  return "";
}

export function NarrativeAftermath({ record, decisions, summary = "", includeChapter = true }: {
  record: RecordEntry; decisions: RecordEntry[]; summary?: string; includeChapter?: boolean;
}) {
  const aftermath = obj(record.data.aftermath);
  const immediate = arr(aftermath.immediate).map((item) => different(item, [summary])).filter(Boolean);
  const reactions = arr(aftermath.reactions).map(obj).filter((item) => str(item.line));
  const relationships = arr(aftermath.relationships).map(obj).filter((item) => str(item.change));
  const commitments = arr(aftermath.commitments).map(obj).filter((item) => str(item.title));
  const commitmentStatus = (status: string) => status === "fulfilled" ? text.promiseFulfilled : status === "broken" ? text.promiseBroken : status === "cancelled" ? text.promiseCancelled : text.promisePending;
  return <div className="narrative-aftermath">
    {immediate.length > 0 && <section className="aftermath-immediate"><h3>{text.immediateResult}</h3><ul>{immediate.map((item, index) => <li key={index}>{item}</li>)}</ul></section>}
    {reactions.length > 0 && <section className="aftermath-reactions"><h3>{text.characterResponse}</h3>{reactions.map((reaction, index) => {
      const name = narrativeCastName(str(reaction.cast_id), decisions);
      return <figure className="cast-voice" key={index}>{name && <figcaption><strong>{name}</strong></figcaption>}<blockquote>{str(reaction.line)}</blockquote></figure>;
    })}</section>}
    {relationships.length > 0 && <section className="aftermath-relationships"><h3>{text.relationshipChange}</h3><ul>{relationships.map((relationship, index) => {
      const name = narrativeCastName(str(relationship.cast_id), decisions);
      return <li key={index}>{name && <strong>{name}</strong>}<span>{str(relationship.change)}</span></li>;
    })}</ul></section>}
    {commitments.length > 0 && <section className="aftermath-commitments"><h3>{text.commitments}</h3><ul>{commitments.map((commitment, index) => {
      const name = narrativeCastName(str(commitment.cast_id), decisions);
      const status = str(commitment.status);
      return <li key={str(commitment.id) || index} className={`commitment-${status}`}><div><strong>{str(commitment.title)}</strong>{name && <span>{name}</span>}</div><div className="commitment-state"><span>{commitmentStatus(status)}</span>{str(commitment.due_on) && <span>{text.dueOn} {date(str(commitment.due_on))}</span>}</div></li>;
    })}</ul></section>}
    {includeChapter && <ChapterAftermath record={record} decisions={decisions} summaryAlreadyShown={summary} />}
  </div>;
}

export function ChapterAftermath({ record, decisions, onPause, paused = false, summaryAlreadyShown = "" }: { record: RecordEntry; decisions: RecordEntry[]; onPause?: () => void; paused?: boolean; summaryAlreadyShown?: string }) {
  const chapter = obj(obj(record.data.aftermath).chapter);
  const closed = str(chapter.status) === "closed";
  const summary = different(chapter.summary, [summaryAlreadyShown]);
  const takeaway = different(chapter.takeaway, [summaryAlreadyShown, summary]);
  if (!closed && !summary && !takeaway) return null;
  const setup = decisions.map((decision) => obj(obj(decision.data.experience).chapter)).find((candidate) => str(candidate.id) === str(chapter.id));
  return <section className={`chapter-aftermath${closed ? " chapter-closed" : ""}`}>
    <div className="chapter-ending-heading"><Flag size={22} aria-hidden="true" /><h3>{closed ? text.chapterClosed : str(setup?.title) || text.chapterResult}</h3>{closed && str(setup?.title) && <span>{str(setup?.title)}</span>}</div>
    {summary && <p>{summary}</p>}
    {takeaway && takeaway !== summary && <p className="chapter-takeaway">{takeaway}</p>}
    {closed && onPause && !paused && <button type="button" className="text-button chapter-pause" onClick={onPause}>{text.pauseHere}</button>}
  </section>;
}
