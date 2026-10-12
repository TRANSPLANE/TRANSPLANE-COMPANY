import { Flag, ArrowRight } from "./icons";
import { number } from "./format";
import { useLocale } from "./locale";
import { gameSystemsCopy as text } from "./gameSystemsCopy";
import type { GameState } from "./gameSystemsTypes";
import "./game-systems.css";

export function GameProgress({ state, onOpen, compact = false }: { state: GameState | null; onOpen?: () => void; compact?: boolean }) {
  useLocale();
  if (!state) return null;
  const p = state.progression;
  const progress = Math.max(0, Math.min(1, p.progress));
  const content = <>
    <span className="game-rank-emblem" aria-hidden="true"><Flag size={20} /></span>
    <span className="game-rank-copy"><span className="game-rank-title">{text.level} {number(p.level)} · {p.level_name}</span>
      <span className="game-rank-meter" role="progressbar" aria-label={text.progression} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(progress * 100)}><span style={{ width: `${progress * 100}%` }} /></span>
      <span className="game-rank-numbers">{text.xp} {number(p.xp)}{p.next_threshold == null ? ` · ${text.maxLevel}` : ` / ${number(p.next_threshold)}`}</span>
    </span>
    {onOpen && <ArrowRight size={18} />}
  </>;
  return onOpen
    ? <button className={`game-rank ${compact ? "compact" : ""}`} onClick={onOpen} aria-label={`${text.progression}: ${text.level} ${number(p.level)}, ${p.level_name}, ${text.xp} ${number(p.xp)}`}>{content}</button>
    : <div className={`game-rank ${compact ? "compact" : ""}`} aria-label={text.progression}>{content}</div>;
}

export function GameNotifications({ state, onOpen }: { state: GameState | null; onOpen: () => void }) {
  useLocale();
  if (!state) return null;
  const unread = state.alerts.filter((alert) => alert.unread && alert.applicable !== false).length;
  return <button className={`game-letter-entry ${unread ? "has-unread" : ""}`} onClick={onOpen} aria-label={`${text.newLetters}: ${number(unread)}`}>
    <svg viewBox="0 0 24 24" width="20" height="20" shapeRendering="crispEdges" aria-hidden="true"><path fill="currentColor" d="M2 5h20v14H2V5zm2 2v2h2v2h2v2h8v-2h2V9h2V7H4zm0 5v5h16v-5h-2v2h-2v1H8v-1H6v-2H4z" /></svg>
    <span>{text.alerts}</span>{unread > 0 && <strong>{number(unread)}</strong>}
  </button>;
}
