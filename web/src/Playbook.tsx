import { CopyButton, Modal } from "./components";
import { ArrowRight, Building2, Flag, GitBranch } from "./icons";
import { playbookCopy as text } from "./playbookCopy";
import { narrative } from "./narrativeCopy";
import type { Company } from "./types";
import "./gameplay-orientation.css";

export function agentPlaybookPrompt(company?: Company) {
  return [
    text.prompt,
    `${text.entry}: ${location.origin}/api/v1/agent-entry`,
    company ? `${text.company}: ${company.id}\n${text.mode}: ${company.mode || "real"}` : "",
    `${narrative.promptPlayerOutput}\n${location.origin}/api/v1/agent-guide?topic=player-output`,
    company ? text.promptResume : "",
    !company ? text.promptFirst : company.mode === "simulation" ? text.promptSimulation : text.promptReal,
  ].filter(Boolean).join("\n\n");
}

export function Playbook({ company, owner, onClose, onStart, onAuthorize, onSimulation, returnFocus }: {
  company?: Company;
  owner: boolean;
  onClose: () => void;
  onStart: (mode: "real" | "simulation") => void;
  onAuthorize?: () => void;
  onSimulation?: () => void;
  returnFocus?: HTMLElement | null;
}) {
  const simulation = company?.mode === "simulation";
  const handoff = agentPlaybookPrompt(company);
  function start(mode: "real" | "simulation") { onClose(); onStart(mode); }
  function go(action: () => void) { onClose(); action(); }
  return <Modal title={text.title} onClose={onClose} returnFocus={returnFocus} wide>
    <div className="playbook">
      <div className="playbook-intro"><Flag size={28} aria-hidden="true" /><div><strong>{company ? company.name : text.newGame}</strong>{company && <span>{simulation ? text.simulation : text.real}</span>}<p>{!company ? text.firstIntro : simulation ? text.simulationIntro : text.realIntro}</p></div></div>
      {!company && <div className="playbook-actions"><button className="primary-button" disabled={!owner} onClick={() => start("simulation")}><GitBranch size={18} aria-hidden="true" />{text.startSimulation}</button><button className="secondary-button" disabled={!owner} onClick={() => start("real")}><Building2 size={18} aria-hidden="true" />{text.startReal}</button></div>}
      {!company && !owner && <p className="field-help">{text.companyReadOnly}</p>}
      <section className="playbook-section" aria-labelledby="playbook-steps-title"><h3 id="playbook-steps-title">{text.stepsTitle}</h3><ol className="playbook-steps">{[[text.read, text.readHelp], [text.choose, text.chooseHelp], [text.resolve, text.resolveHelp]].map(([name, help], index) => <li key={name}><span className="playbook-step-number" aria-hidden="true">{index + 1}</span><div><strong>{name}</strong><p>{help}</p></div></li>)}</ol></section>
      <details className="playbook-section playbook-disclosure"><summary>{text.chatTitle}</summary><p>{text.chatIntro}</p><ul className="playbook-chat">{[text.chatContinue, text.chatChoose, text.chatStatus, text.chatMenu, ...(!simulation && company ? [text.chatReal] : [])].map((example) => <li key={example}><q>{example}</q></li>)}</ul><p className="field-help">{text.chatPause}</p></details>
      <details className="playbook-section playbook-disclosure"><summary>{text.companiesTitle}</summary><p>{text.companiesHelp}</p><p>{company && !simulation ? text.branchHelp : text.modeHelp}</p><div className="playbook-actions"><button className="secondary-button" disabled={!owner} onClick={() => start("simulation")}>{text.startSimulation}</button><button className="secondary-button" disabled={!owner} onClick={() => start("real")}>{text.startReal}</button>{company && !simulation && onSimulation && <button className="text-button" disabled={!owner} onClick={() => go(onSimulation)}>{text.openBranch}<ArrowRight size={16} aria-hidden="true" /></button>}</div>{!owner && <p className="field-help">{text.companyReadOnly}</p>}</details>
      <section className="playbook-section" aria-labelledby="playbook-agent-title"><h3 id="playbook-agent-title">{text.handoffTitle}</h3><p>{text.handoffHelp}</p><div className="playbook-actions"><CopyButton value={handoff} label={text.copyHandoff} />{owner && onAuthorize && <button className="text-button" onClick={() => go(onAuthorize)}>{text.authorize}</button>}</div><details className="agent-handoff"><summary>{text.handoffPreview}</summary><pre>{handoff}</pre><p>{text.authorizeHelp}</p></details></section>
    </div>
  </Modal>;
}
