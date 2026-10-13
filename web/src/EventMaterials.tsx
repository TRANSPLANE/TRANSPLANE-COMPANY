import { useEffect, useId, useState } from "react";
import { arr, obj, str } from "./format";
import { Check, FileText } from "./icons";
import { narrative as text } from "./narrativeCopy";
import { narrativeCastName } from "./NarrativeExperience";
import type { Json, RecordEntry } from "./types";

export function materialStorageKey(companyId: string, decisionId: string) {
  return `transplane-company-materials:${JSON.stringify([companyId, decisionId])}`;
}
export function materialEntries(decision: RecordEntry) {
  return arr(obj(decision.data.experience).materials).map(obj).filter((entry) => str(entry.id) && str(entry.title) && str(entry.body) && ["letter", "person", "note"].includes(str(entry.kind)));
}
export function materialsFingerprint(materials: Record<string, Json>[]) {
  const content = JSON.stringify(materials.map((material) => ({ id: str(material.id), title: str(material.title), body: str(material.body), kind: str(material.kind), cast_id: str(material.cast_id), after: arr(material.after).map((id) => str(id)).sort() })));
  let first = 2166136261;
  let second = 5381;
  for (let index = 0; index < content.length; index++) {
    const code = content.charCodeAt(index);
    first = Math.imul(first ^ code, 16777619);
    second = Math.imul(second, 33) ^ code;
  }
  return `${content.length}:${(first >>> 0).toString(16)}:${(second >>> 0).toString(16)}`;
}
export function availableMaterials(materials: Record<string, Json>[], read: string[]) {
  return materials.filter((material) => arr(material.after).every((dependency) => read.includes(str(dependency))));
}
export function openMaterial(materials: Record<string, Json>[], read: string[], id: string) {
  if (!availableMaterials(materials, read).some((material) => str(material.id) === id)) return read;
  return read.includes(id) ? read : [...read, id];
}
function storedReading(key: string, fingerprint: string, materials: Record<string, Json>[]) {
  try {
    const saved = JSON.parse(sessionStorage.getItem(key) || "null") as { fingerprint?: unknown; read?: unknown } | null;
    return saved?.fingerprint === fingerprint && Array.isArray(saved.read) ? saved.read.filter((id): id is string => typeof id === "string" && materials.some((material) => str(material.id) === id)) : [];
  } catch { return []; }
}

export function EventMaterials({ decision }: { decision: RecordEntry }) {
  const materials = materialEntries(decision);
  const storageKey = materialStorageKey(decision.company_id, decision.id);
  const fingerprint = materialsFingerprint(materials);
  const [reading, setReading] = useState(() => ({ key: storageKey, fingerprint, read: storedReading(storageKey, fingerprint, materials), opened: "" }));
  const panelId = useId();
  const current = reading.key === storageKey && reading.fingerprint === fingerprint ? reading : { key: storageKey, fingerprint, read: storedReading(storageKey, fingerprint, materials), opened: "" };
  useEffect(() => {
    setReading((saved) => saved.key === storageKey && saved.fingerprint === fingerprint ? saved : { key: storageKey, fingerprint, read: storedReading(storageKey, fingerprint, materials), opened: "" });
    try {
      const saved = JSON.parse(sessionStorage.getItem(storageKey) || "null") as { fingerprint?: unknown } | null;
      if (saved && saved.fingerprint !== fingerprint) sessionStorage.setItem(storageKey, JSON.stringify({ fingerprint, read: [] }));
    } catch { /* Material edits still reset the current reading view without storage. */ }
  }, [storageKey, fingerprint]);
  const visible = availableMaterials(materials, current.read);
  const opened = visible.find((material) => str(material.id) === current.opened);
  function read(id: string) {
    const next = openMaterial(materials, current.read, id);
    if (!visible.some((material) => str(material.id) === id)) return;
    setReading({ key: storageKey, fingerprint, read: next, opened: id });
    try { sessionStorage.setItem(storageKey, JSON.stringify({ fingerprint, read: next })); } catch { /* Keep exploration available without local storage. */ }
  }
  const materialKind = (kind: string) => kind === "letter" ? text.materialLetter : kind === "person" ? text.materialPerson : text.materialNote;
  if (!visible.length) return null;
  return <section className="event-materials" aria-labelledby={`${panelId}-title`}>
    <h3 id={`${panelId}-title`}>{text.exploreMaterials}</h3>
    <div className="material-choices">{visible.map((material) => {
      const id = str(material.id);
      const isOpen = opened === material;
      return <button key={id} type="button" className={`material-button${isOpen ? " material-open" : ""}`} onClick={() => read(id)} aria-expanded={isOpen} aria-controls={panelId}>
        <FileText size={20} aria-hidden="true" /><span><small>{materialKind(str(material.kind))}</small><strong>{str(material.title)}</strong></span>{current.read.includes(id) && <><Check size={18} aria-hidden="true" /><span className="sr-only">{text.materialRead}</span></>}
      </button>;
    })}</div>
    <div id={panelId} className="material-reader" hidden={!opened}>
      {opened && <><div className="material-heading"><h4>{str(opened.title)}</h4><button className="text-button" type="button" onClick={() => setReading({ ...current, opened: "" })}>{text.closeMaterial}</button></div>{str(opened.cast_id) && narrativeCastName(str(opened.cast_id), [decision]) && <p className="material-person">{narrativeCastName(str(opened.cast_id), [decision])}</p>}<p className="story-prose">{str(opened.body)}</p></>}
    </div>
  </section>;
}
