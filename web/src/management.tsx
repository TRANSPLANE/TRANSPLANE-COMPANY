import { words, phrase } from "./labels";
import { useEffect, useState, type FormEvent } from "react";
import {
  Archive,
  Check,
  Copy,
  KeyRound,
  Package,
  Plus,
  Power,
  Shield,
  Trash2,
} from "./icons";
import { api, ApiError } from "./api";
import {
  CopyButton,
  Empty,
  ErrorNotice,
  Loading,
  Modal,
  Section,
  Status,
} from "./components";
import { copy, recordCopy } from "./copy";
import { date } from "./format";
import { PageHeading } from "./views";
import type { Company, Json, Plugin, PluginList, TokenEntry } from "./types";
import { useLocale } from "./locale";
import { Skills } from "./Skills";
import { skillsCopy } from "./skillsCopy";
import "./plugins.css";

function errorText(error: unknown) {
  return error instanceof Error ? error.message : words.management064;
}
export function OwnerManagement({
  company,
  onClose,
}: {
  company?: Company;
  onClose: () => void;
}) {
  const [tokens, setTokens] = useState<TokenEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [scope, setScope] = useState("read");
  const [allowSkillMaintenance, setAllowSkillMaintenance] = useState(false);
  const [issued, setIssued] = useState("");
  const [busy, setBusy] = useState("");
  const [backup, setBackup] = useState<{ path: string; sha256: string } | null>(
    null,
  );
  const [revoke, setRevoke] = useState<TokenEntry | null>(null);
  async function load() {
    setError("");
    try {
      const data = await api.tokens();
      setTokens(data.tokens || []);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setLoading(false);
    }
  }
  useEffect(() => {
    void load();
  }, []);
  async function issue(event: FormEvent) {
    event.preventDefault();
    if (!company) return;
    setBusy("token");
    setError("");
    try {
      const result = await api.issueToken(
        company.id,
        [...(scope === "write"
          ? [
              "company.read",
              "company.write",
              "finance.write",
              "goals.write",
              "reports.write",
              "files.write",
            ]
          : ["company.read"]), ...(allowSkillMaintenance ? ["skills.write"] : [])],
      );
      setIssued(result.token || result.credential || "");
      await load();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy("");
    }
  }
  async function revokeToken() {
    if (!revoke) return;
    setBusy(revoke.id);
    setError("");
    try {
      await api.revokeToken(revoke.id);
      setRevoke(null);
      await load();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy("");
    }
  }
  async function createBackup() {
    setBusy("backup");
    setError("");
    try {
      const result = await api.backup(company?.id);
      setBackup(result);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy("");
    }
  }
  return (
    <Modal title={words.management065} onClose={onClose} wide>
      {error && <ErrorNotice message={error} />}
      <Section title={words.management066}>
        {company ? (
          <form onSubmit={issue} className="permission-form">
            <label>
              {words.management067}
              <input value={company.name} readOnly />
            </label>
            <label>
              {words.management068}
              <select value={scope} onChange={(e) => setScope(e.target.value)}>
                <option value="read">{words.management069}</option>
                <option value="write">{words.management070}</option>
              </select>
            </label>
            <label className="skill-permission-toggle">
              <input type="checkbox" checked={allowSkillMaintenance} onChange={(event) => setAllowSkillMaintenance(event.target.checked)} />
              {skillsCopy.allowMaintenance}
            </label>
            <button className="primary-button" disabled={!!busy}>
              <Plus size={16} aria-hidden="true" />
              {busy === "token" ? words.management071 : words.management072}
            </button>
          </form>
        ) : (
          <p className="muted">{words.management073}</p>
        )}
        {issued && (
          <div className="issued-credential" role="status">
            <div>
              <KeyRound size={17} aria-hidden="true" />
              <strong>{words.management074}</strong>
            </div>
            <p>{words.management075}</p>
            <textarea
              value={issued}
              readOnly
              aria-label={words.management076}
              rows={3}
            />
            <CopyButton value={issued} label={words.management077} />
            <button className="text-button" onClick={() => setIssued("")}>
              {words.management078}
            </button>
          </div>
        )}
        {loading ? (
          <Loading />
        ) : (
          <div className="token-list">
            {tokens
              .filter(
                (t) =>
                  t.kind !== "owner" &&
                  !t.owner &&
                  (!company || t.company_id === company.id),
              )
              .map((t) => (
                <div className="token-row" key={t.id}>
                  <Shield size={17} aria-hidden="true" />
                  <div>
                    <strong>{t.id}</strong>
                    <span>
                      {t.scopes.join(" · ")} · {date(t.created_at)}
                    </span>
                  </div>
                  {t.revoked || t.revoked_at ? (
                    <span className="muted">{words.management079}</span>
                  ) : (
                    <button
                      className="text-button danger"
                      onClick={() => setRevoke(t)}
                    >
                      {words.management080}
                    </button>
                  )}
                </div>
              ))}
            {!tokens.filter(
              (t) =>
                t.kind !== "owner" &&
                !t.owner &&
                (!company || t.company_id === company.id),
            ).length && <p className="muted">{words.management081}</p>}
          </div>
        )}
        {revoke && (
          <div className="confirm-inline">
            <p>{words.management082}</p>
            <div>
              <button
                className="secondary-button"
                onClick={() => setRevoke(null)}
              >
                {words.management083}
              </button>
              <button
                className="danger-button"
                onClick={() => void revokeToken()}
                disabled={!!busy}
              >
                {words.management084}
              </button>
            </div>
          </div>
        )}
      </Section>
      <Section title={words.management085}>
        <p className="muted">
          {words.management086}
          {company ? company.name : words.management087}
          {words.management088}
        </p>
        <button
          className="secondary-button"
          onClick={() => void createBackup()}
          disabled={!!busy}
        >
          <Archive size={17} aria-hidden="true" />
          {busy === "backup" ? words.management089 : copy.backup}
        </button>
        {backup && (
          <div className="backup-result" role="status">
            <strong>
              <Check size={16} aria-hidden="true" />
              {words.management090}
            </strong>
            <code>{backup.path}</code>
            {backup.sha256 && (
              <details>
                <summary>{words.management091}</summary>
                <code>{backup.sha256}</code>
              </details>
            )}
            {backup.path && (
              <CopyButton value={backup.path} label={words.management092} />
            )}
          </div>
        )}
      </Section>
    </Modal>
  );
}

export function Plugins({
  owner,
  companyId,
  companyRevision,
  onChanged,
}: {
  owner: boolean;
  companyId: string;
  companyRevision: number;
  onChanged: () => Promise<void>;
}) {
  const locale = useLocale();
  const [data, setData] = useState<PluginList>({ catalog: [], installed: [] });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState("");
  const [install, setInstall] = useState(false);
  const [manifest, setManifest] = useState("");
  const [config, setConfig] = useState("{}");
  const [remove, setRemove] = useState<Plugin | null>(null);
  const [skillsRefresh, setSkillsRefresh] = useState(0);
  async function load() {
    setError("");
    try {
      setData(await api.plugins(companyId, owner));
    } catch (e) {
      setError(errorText(e));
    } finally {
      setLoading(false);
    }
  }
  useEffect(() => {
    void load();
  }, [owner, companyId, locale]);
  async function mutate(id: string, action: () => Promise<unknown>) {
    setBusy(id);
    setError("");
    try {
      await action();
      await load();
      setSkillsRefresh((value) => value + 1);
      setRemove(null);
      await onChanged();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy("");
    }
  }
  async function installManifest(event: FormEvent) {
    event.preventDefault();
    let value: Json;
    try {
      value = JSON.parse(manifest);
    } catch {
      setError(words.management093);
      return;
    }
    let settings: Json;
    try {
      settings = JSON.parse(config);
    } catch {
      setError(recordCopy.configError);
      return;
    }
    await mutate("install", async () => {
      await api.installPlugin(value, settings);
      setInstall(false);
      setManifest("");
    });
  }

  return (
    <div className="plugins-page">
      <PageHeading title={words.management096} context={words.management097} />
      <Skills key={companyId} owner={owner} companyId={companyId} companyRevision={companyRevision} refreshVersion={skillsRefresh} onChanged={onChanged} />
      {!owner && <p className="plugin-access-note muted">{recordCopy.pluginReadOnly}</p>}
      {error && <ErrorNotice message={error} />}
      {loading ? (
        <Loading />
      ) : (
        <>
          {owner && (
            <Section title={words.management100} className="plugin-installed-section" action={
              <div className="plugin-toolbar">
                <button className="secondary-button" disabled={!!busy} onClick={() => setInstall(true)}>
                  <Plus size={17} aria-hidden="true" />{words.management098}
                </button>
                <button className="text-button" disabled={!!busy} onClick={() => {
                  setLoading(true); setSkillsRefresh((value) => value + 1); void load();
                }}>{words.management099}</button>
              </div>
            }>
              {data.installed.length ? (
                <div className="plugin-list">
                  {data.installed.map((p) => (
                    <article
                      className="plugin-item"
                      key={`${p.id}-${p.version}`}
                    >
                      <div className="plugin-icon">
                        <Package size={23} aria-hidden="true" />
                      </div>
                      <div className="plugin-body">
                        <div className="plugin-title">
                          <h3>{p.name || p.id}</h3>
                          <span className="number">{p.version}</span>
                          <Status value={p.enabled ? "enabled" : "disabled"} />
                        </div>
                        {p.description && <p>{p.description}</p>}
                        <PluginMeta plugin={p} />
                      </div>
                      <div className="plugin-actions">
                        <button
                          className="secondary-button"
                          disabled={!!busy}
                          onClick={() =>
                            void mutate(p.id, () =>
                              api.setPluginEnabled(p.id, !p.enabled),
                            )
                          }
                        >
                          <Power size={15} aria-hidden="true" />
                          {p.enabled
                            ? words.management101
                            : words.management102}
                        </button>
                        <button
                          className="icon-button danger"
                          disabled={!!busy}
                          aria-label={phrase("management103", p.name)}
                          onClick={() => setRemove(p)}
                        >
                          <Trash2 size={17} aria-hidden="true" />
                        </button>
                      </div>
                    </article>
                  ))}
                </div>
              ) : (
                <Empty compact>{copy.empty.plugin}</Empty>
              )}
            </Section>
          )}
          <Section title={words.management104} className="plugin-catalog-section">
            {data.catalog.length ? (
              <div className="plugin-list">
                {data.catalog.map((p) => (
                  <article className="plugin-item" key={`${p.id}-${p.version}`}>
                    <div className="plugin-icon">
                      <Package size={23} aria-hidden="true" />
                    </div>
                    <div className="plugin-body">
                      <div className="plugin-title">
                        <h3>{p.name || p.id}</h3>
                        <span className="number">{p.version}</span>
                      </div>
                      {p.description && <p>{p.description}</p>}
                      <PluginMeta plugin={p} />
                    </div>
                    {owner && <div className="plugin-actions">{data.installed.some(
                        (i) => i.id === p.id && i.version === p.version,
                      ) ? (
                      <span className="plugin-installed-state muted">{words.management105}</span>
                    ) : (
                      <button
                        className="secondary-button"
                        disabled={!!busy}
                        onClick={() =>
                          void mutate(p.id, () =>
                            api.installPlugin(
                              p.manifest || (p as unknown as Json),
                            ),
                          )
                        }
                      >
                        {words.management106}
                        {p.version}
                      </button>
                    )}</div>}
                  </article>
                ))}
              </div>
            ) : (
              <Empty compact>{words.management107}</Empty>
            )}
          </Section>
        </>
      )}
      {install && (
        <Modal title={words.management108} onClose={() => setInstall(false)}>
          <form onSubmit={installManifest} className="stack-form">
            <p>{words.management109}</p>
            <label>
              {words.management110}
              <textarea
                className="code-input"
                value={manifest}
                onChange={(e) => setManifest(e.target.value)}
                rows={14}
                required
                spellCheck={false}
              />
            </label>
            <label>
              {recordCopy.config}
              <textarea
                className="code-input"
                value={config}
                onChange={(e) => setConfig(e.target.value)}
                rows={4}
                spellCheck={false}
              />
            </label>
            <p className="muted">{words.management111}</p>
            <div className="form-actions">
              <button
                type="button"
                className="secondary-button"
                onClick={() => setInstall(false)}
              >
                {words.management112}
              </button>
              <button className="primary-button" disabled={!!busy}>
                {words.management113}
              </button>
            </div>
          </form>
        </Modal>
      )}
      {remove && (
        <Modal
          title={phrase("management114", remove.name || remove.id)}
          onClose={() => setRemove(null)}
        >
          <p>{words.management115}</p>
          <div className="form-actions">
            <button
              className="secondary-button"
              onClick={() => setRemove(null)}
            >
              {words.management116}
            </button>
            <button
              className="danger-button"
              disabled={!!busy}
              onClick={() =>
                void mutate(remove.id, () => api.uninstallPlugin(remove.id))
              }
            >
              {words.management117}
            </button>
          </div>
        </Modal>
      )}
    </div>
  );
}
function PluginMeta({ plugin }: { plugin: Plugin }) {
  return (
    <div className="plugin-meta">
      {plugin.author && <span>{plugin.author}</span>}
      {plugin.source && (
        <span>
          {words.management118}
          {plugin.source}
        </span>
      )}
      {plugin.compatibility && (
        <span>
          {words.management119}
          {plugin.compatibility}
        </span>
      )}
      {plugin.permissions?.length ? (
        <span>
          {words.management120}
          {plugin.permissions.join("、")}
        </span>
      ) : null}
    </div>
  );
}
