use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::Utc;
use company_core::{Company, DomainError, Record, date, identifier, integer, text};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Any, AnyPool, Row, Transaction, any::AnyPoolOptions};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::sync::Mutex;
use uuid::Uuid;
macro_rules! query {
    ($q:expr) => {
        sqlx::query(sqlx::AssertSqlSafe(($q).to_string()))
    };
}
mod fork;
mod game_systems;
mod network;
mod play;
mod plugins;
mod portability;
mod preview;
mod skills;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub data_dir: PathBuf,
    pub database_url: Option<String>,
}
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            data_dir: "company-data".into(),
            database_url: None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationRequest {
    pub request_id: String,
    #[serde(default)]
    pub expected_revision: Option<i64>,
    pub input: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchOperation {
    pub operation: String,
    pub request: OperationRequest,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Principal {
    pub kind: String,
    pub subject: String,
    pub company_id: Option<String>,
    pub scopes: Vec<String>,
}
impl Principal {
    pub fn authorize(&self, company: Option<&str>, scope: &str) -> Result<()> {
        if self.kind == "owner" {
            return Ok(());
        }
        if ["admin", "backup.deliver"].contains(&scope)
            || company.is_none()
            || self.company_id.as_deref() != company
        {
            return Err(DomainError::new("forbidden", "凭据未获此公司或管理能力的授权").into());
        }
        let broad = if scope == "skills.write" {
            "skills.write"
        } else if scope.ends_with(".read") {
            "company.read"
        } else {
            "company.write"
        };
        if !self.scopes.iter().any(|s| s == scope || s == broad) {
            return Err(DomainError::new("forbidden", "凭据未获此能力的授权").into());
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct App {
    pool: AnyPool,
    write_pool: AnyPool,
    root: Arc<PathBuf>,
    pg: bool,
    write_lock: Arc<Mutex<()>>,
}
fn now() -> String {
    Utc::now().to_rfc3339()
}
fn hash(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}
fn err(code: &str, message: impl Into<String>) -> anyhow::Error {
    DomainError::new(code, message).into()
}
fn new_id() -> String {
    Uuid::new_v4().simple().to_string()
}
fn key(s: Option<&str>) -> &str {
    s.unwrap_or("_")
}
fn sqlite_busy(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(error) => error
            .code()
            .and_then(|code| code.parse::<i32>().ok())
            .is_some_and(|code| matches!(code & 255, 5 | 6)),
        _ => false,
    }
}
fn sql(pg: bool, q: &str) -> String {
    if !pg {
        return q.into();
    }
    let mut n = 0;
    q.chars()
        .map(|c| {
            if c == '?' {
                n += 1;
                format!("${n}")
            } else {
                c.to_string()
            }
        })
        .collect()
}

impl App {
    pub async fn open(config: AppConfig) -> Result<Self> {
        sqlx::any::install_default_drivers();
        std::fs::create_dir_all(&config.data_dir)?;
        let root = std::fs::canonicalize(&config.data_dir)?;
        for name in [
            "config",
            "data",
            "objects",
            "companies",
            "plugins",
            "backups",
        ] {
            std::fs::create_dir_all(root.join(name))?;
        }
        let url = config.database_url.unwrap_or_else(|| {
            let path = root
                .join("data/company.sqlite")
                .to_string_lossy()
                .trim_start_matches(r"\\?\")
                .replace('\\', "/");
            let encoded: String = path
                .bytes()
                .map(|b| {
                    if !b.is_ascii_alphanumeric() && b != b'-' && b != b'_' && b != b'.' {
                        format!("%{b:02X}")
                    } else {
                        (b as char).to_string()
                    }
                })
                .collect();
            format!("sqlite://{encoded}?mode=rwc")
        });
        let pg = url.starts_with("postgres");
        if !pg && !url.starts_with("sqlite:") {
            bail!("Only SQLite and PostgreSQL are supported");
        }
        let pool = AnyPoolOptions::new()
            .max_connections(if pg { 20 } else { 12 })
            .acquire_timeout(Duration::from_secs(10))
            .after_connect(move |conn, _| {
                Box::pin(async move {
                    if !pg {
                        for q in [
                            "PRAGMA busy_timeout=500",
                            "PRAGMA foreign_keys=ON",
                            "PRAGMA synchronous=FULL",
                        ] {
                            query!(q).execute(&mut *conn).await?;
                        }
                    }
                    Ok(())
                })
            })
            .connect(&url)
            .await?;
        // A busy read pool must not starve commits. Both pools use the same database.
        let write_pool = AnyPoolOptions::new()
            .max_connections(if pg { 4 } else { 2 })
            .acquire_timeout(Duration::from_secs(5))
            .after_connect(move |conn, _| {
                Box::pin(async move {
                    if !pg {
                        for statement in [
                            "PRAGMA busy_timeout=500",
                            "PRAGMA foreign_keys=ON",
                            "PRAGMA synchronous=FULL",
                        ] {
                            query!(statement).execute(&mut *conn).await?;
                        }
                    }
                    Ok(())
                })
            })
            .connect(&url)
            .await?;
        if !pg {
            query!("PRAGMA journal_mode=WAL").execute(&pool).await?;
            let row = query!("SELECT sqlite_version() AS version")
                .fetch_one(&pool)
                .await?;
            let version: String = row.try_get("version")?;
            let parts: Vec<u32> = version.split('.').filter_map(|s| s.parse().ok()).collect();
            if parts.as_slice() < [3, 51, 3].as_slice() {
                bail!(
                    "SQLite {version} lacks the required WAL fix; rebuild with the locked bundled engine"
                );
            }
        }
        let app = Self {
            pool,
            write_pool,
            root: Arc::new(root),
            pg,
            write_lock: Arc::new(Mutex::new(())),
        };
        app.migrate().await?;
        Ok(app)
    }
    async fn migrate(&self) -> Result<()> {
        for q in [
            "CREATE TABLE IF NOT EXISTS schema_versions(version BIGINT PRIMARY KEY, applied_at TEXT NOT NULL)",
            "CREATE TABLE IF NOT EXISTS companies(id TEXT PRIMARY KEY, revision BIGINT NOT NULL, data TEXT NOT NULL)",
            "CREATE TABLE IF NOT EXISTS company_versions(id TEXT NOT NULL,revision BIGINT NOT NULL,data TEXT NOT NULL,PRIMARY KEY(id,revision))",
            "CREATE TABLE IF NOT EXISTS records(company_id TEXT NOT NULL,kind TEXT NOT NULL,id TEXT NOT NULL,revision BIGINT NOT NULL,data TEXT NOT NULL,PRIMARY KEY(company_id,kind,id))",
            "CREATE TABLE IF NOT EXISTS record_versions(company_id TEXT NOT NULL,kind TEXT NOT NULL,id TEXT NOT NULL,revision BIGINT NOT NULL,data TEXT NOT NULL,PRIMARY KEY(company_id,kind,id,revision))",
            "CREATE TABLE IF NOT EXISTS cash_entries(company_id TEXT NOT NULL,id TEXT NOT NULL,account_id TEXT NOT NULL,amount_minor BIGINT NOT NULL,direction TEXT NOT NULL,category TEXT NOT NULL,occurred_on TEXT NOT NULL,PRIMARY KEY(company_id,id))",
            "CREATE INDEX IF NOT EXISTS cash_company_date ON cash_entries(company_id,occurred_on,category,direction)",
            "CREATE TABLE IF NOT EXISTS receipts(company_key TEXT NOT NULL,request_id TEXT NOT NULL,operation TEXT NOT NULL,input_hash TEXT NOT NULL,result TEXT NOT NULL,actor TEXT NOT NULL,created_at TEXT NOT NULL,PRIMARY KEY(company_key,request_id))",
            "CREATE TABLE IF NOT EXISTS events(id TEXT PRIMARY KEY,company_id TEXT NOT NULL,kind TEXT NOT NULL,revision BIGINT NOT NULL,data TEXT NOT NULL)",
            "CREATE INDEX IF NOT EXISTS events_company_revision ON events(company_id,revision)",
            "CREATE TABLE IF NOT EXISTS credentials(id TEXT PRIMARY KEY,token_hash TEXT UNIQUE NOT NULL,kind TEXT NOT NULL,subject TEXT NOT NULL,company_id TEXT NOT NULL,scopes TEXT NOT NULL,revoked BIGINT NOT NULL,created_at TEXT NOT NULL)",
            "CREATE TABLE IF NOT EXISTS plugins(id TEXT PRIMARY KEY,version TEXT NOT NULL,enabled BIGINT NOT NULL,manifest TEXT NOT NULL,config TEXT NOT NULL)",
            "CREATE TABLE IF NOT EXISTS plugin_data(company_id TEXT NOT NULL,plugin_id TEXT NOT NULL,id TEXT NOT NULL,data TEXT NOT NULL,PRIMARY KEY(company_id,plugin_id,id))",
        ] {
            query!(q).execute(&self.pool).await?;
        }
        query!(&sql(self.pg,"INSERT INTO schema_versions(version,applied_at) VALUES(1,?) ON CONFLICT(version) DO NOTHING")).bind(now()).execute(&self.pool).await?;
        Ok(())
    }
    pub fn data_dir(&self) -> &Path {
        &self.root
    }
    /// Read the committed cursor without constructing a company snapshot.
    /// The database is queried so writes from a separate CLI process are visible.
    pub async fn company_revision(&self, company_id: &str) -> Result<i64> {
        identifier(company_id)?;
        let row = query!(&sql(self.pg, "SELECT revision FROM companies WHERE id=?"))
            .bind(company_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| err("not_found", "公司不存在"))?;
        Ok(row.try_get("revision")?)
    }
    pub async fn discovery_localized(
        &self,
        view: Option<&str>,
        service: Option<&str>,
        operation: Option<&str>,
        locale: company_core::Locale,
    ) -> Result<Value> {
        let mut response = self.discovery(view, service, operation).await?;
        if operation.is_some_and(|id| id.starts_with("plugin.")) {
            if let Some(rules) = response["operation"].get_mut("rules") {
                company_core::locale::localize_metadata(rules, locale);
            }
        } else if let Some(items) = response.get_mut("items").and_then(Value::as_array_mut) {
            for item in items {
                if !item["id"].as_str().unwrap_or("").starts_with("plugin.") {
                    company_core::locale::localize_metadata(item, locale);
                }
            }
        } else {
            company_core::locale::localize_metadata(&mut response, locale);
        }
        let locale_key = locale.as_str();
        let plugins = self.plugins().await?;
        for installed in plugins["installed"].as_array().into_iter().flatten() {
            let manifest = &installed["manifest"];
            let id = manifest["id"].as_str().unwrap_or("");
            if let Some(copy) = manifest.get("i18n").and_then(|v| v.get(locale_key)) {
                if let Some(items) = response.get_mut("items").and_then(Value::as_array_mut) {
                    for item in items {
                        if item["id"] == format!("plugin.{id}") {
                            for field in ["name", "description"] {
                                if let Some(text) = copy.get(field).and_then(Value::as_str) {
                                    item[field] = json!(text);
                                }
                            }
                        }
                    }
                }
                if response["operation"]["service"] == format!("plugin.{id}")
                    && let Some(description) =
                        copy.get("operation_description").and_then(Value::as_str)
                {
                    response["operation"]["description"] = json!(description);
                }
            }
        }
        response["lang"] = json!(locale_key);
        Ok(response)
    }
    pub async fn agent_guide_localized(
        &self,
        topic: &str,
        locale: company_core::Locale,
    ) -> Result<Value> {
        let mut response = self.agent_guide(topic).await?;
        if topic.starts_with("plugin.") {
            if let Some(maintenance) = response.get_mut("maintenance") {
                company_core::locale::localize_metadata(maintenance, locale);
            }
        } else {
            company_core::locale::localize_metadata(&mut response, locale);
        }
        if let Some(id) = topic.strip_prefix("plugin.") {
            let plugins = self.plugins().await?;
            if let Some(manifest) = plugins["installed"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|p| p["id"] == id)
                .map(|p| &p["manifest"])
                && let Some(copy) = manifest.get("i18n").and_then(|v| v.get(locale.as_str()))
            {
                if let Some(description) = copy.get("description").and_then(Value::as_str) {
                    response["purpose"] = json!(description);
                }
                if let Some(guidance) = copy.get("guidance").filter(|v| v.is_array()) {
                    response["steps"] = guidance.clone();
                }
            }
        }
        response["lang"] = json!(locale.as_str());
        Ok(response)
    }
    pub async fn plugin_catalog_localized(&self, locale: company_core::Locale) -> Result<Value> {
        let mut response = self.plugin_catalog().await?;
        for group in ["catalog", "installed"] {
            for item in response[group].as_array_mut().into_iter().flatten() {
                let source = item.get("manifest").unwrap_or(item).clone();
                item["manifest"] = source.clone();
                if let Some(copy) = source.get("i18n").and_then(|v| v.get(locale.as_str())) {
                    for field in ["name", "description"] {
                        if let Some(text) = copy.get(field).and_then(Value::as_str) {
                            item[field] = json!(text);
                        }
                    }
                } else if source["id"] == "local-backup" && locale == company_core::Locale::En {
                    for field in ["name", "description"] {
                        let translated = source[field]
                            .as_str()
                            .map(|s| company_core::locale::translate(s, locale));
                        if let Some(text) = translated {
                            item[field] = json!(text);
                        }
                    }
                }
            }
        }
        Ok(response)
    }
    pub async fn snapshot_localized(
        &self,
        company_id: &str,
        locale: company_core::Locale,
    ) -> Result<Value> {
        let mut response = self.snapshot(company_id).await?;
        let plugins = self.plugins().await?;
        for extension in response["extensions"].as_array_mut().into_iter().flatten() {
            if let Some(manifest) = plugins["installed"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|p| p["id"] == extension["id"])
                .map(|p| &p["manifest"])
                && let Some(copy) = manifest.get("i18n").and_then(|v| v.get(locale.as_str()))
            {
                if let Some(name) = copy.get("name").and_then(Value::as_str) {
                    extension["name"] = json!(name);
                }
                for metric in extension["metrics"].as_array_mut().into_iter().flatten() {
                    if let Some(label) = copy
                        .get("metric_labels")
                        .and_then(|v| v.get(metric["id"].as_str().unwrap_or("")))
                        .and_then(Value::as_str)
                    {
                        metric["label"] = json!(label);
                    }
                }
                for view in extension["views"].as_array_mut().into_iter().flatten() {
                    if let Some(title) = copy
                        .get("view_titles")
                        .and_then(|v| v.get(view["id"].as_str().unwrap_or("")))
                        .and_then(Value::as_str)
                    {
                        view["title"] = json!(title);
                    }
                    for field in view["fields"].as_array_mut().into_iter().flatten() {
                        if let Some(label) = copy
                            .get("field_labels")
                            .and_then(|v| v.get(field["label"].as_str().unwrap_or("")))
                            .and_then(Value::as_str)
                        {
                            field["label"] = json!(label);
                        }
                    }
                }
            }
        }
        Ok(response)
    }
    pub async fn snapshot_revision(&self, company_id: &str, revision: i64) -> Result<Value> {
        let mut tx = self.pool.begin().await?;
        let snapshot = self
            .snapshot_revision_tx(&mut tx, company_id, revision, None)
            .await?;
        tx.commit().await?;
        Ok(snapshot)
    }
    async fn snapshot_revision_tx(
        &self,
        tx: &mut Transaction<'_, Any>,
        company_id: &str,
        revision: i64,
        as_of: Option<&str>,
    ) -> Result<Value> {
        let row = query!(&sql(
            self.pg,
            "SELECT data FROM company_versions WHERE id=? AND revision=?"
        ))
        .bind(company_id)
        .bind(revision)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| err("not_found", "公司历史版本不存在"))?;
        let company: Company = serde_json::from_str(&row.try_get::<String, _>("data")?)?;
        let rows = query!(&sql(
            self.pg,
            "SELECT data FROM record_versions WHERE company_id=? ORDER BY kind,id,revision"
        ))
        .bind(company_id)
        .fetch_all(&mut **tx)
        .await?;
        let mut latest: BTreeMap<(String, String), Record> = BTreeMap::new();
        for row in rows {
            let r: Record = serde_json::from_str(&row.try_get::<String, _>("data")?)?;
            if r.company_revision <= revision {
                latest.insert((r.kind.clone(), r.id.clone()), r);
            }
        }
        let date = if let Some(d) = as_of {
            company_core::date(d)?
        } else if company.mode == "simulation" {
            company_core::date(
                company
                    .simulation_date
                    .as_deref()
                    .unwrap_or(&company.opening_date),
            )?
        } else {
            chrono::DateTime::parse_from_rfc3339(&company.updated_at)?
                .with_timezone(&company.timezone.parse::<chrono_tz::Tz>()?)
                .date_naive()
        };
        let flat: Vec<_> = latest.into_values().collect();
        let metrics = company_core::compute_metrics(&company, &flat, date)?;
        let mut records: BTreeMap<String, Vec<Record>> = BTreeMap::new();
        let mut skill_refs = Vec::new();
        for r in flat {
            if r.kind == "skill" {
                skill_refs.push(json!({"id":r.id,"revision":r.revision}));
                continue;
            }
            records.entry(r.kind.clone()).or_default().push(r);
        }
        Ok(
            json!({"company":company,"metrics":metrics,"records":records,"events":[],"historical":true,"skills":{"catalog":format!("/api/v1/companies/{company_id}/skills"),"catalog_state":"current","references":skill_refs}}),
        )
    }
    pub async fn backend_info(&self) -> Result<Value> {
        let sqlite_version = if self.pg {
            None
        } else {
            Some(
                query!("SELECT sqlite_version() AS v")
                    .fetch_one(&self.pool)
                    .await?
                    .try_get::<String, _>("v")?,
            )
        };
        Ok(
            json!({"backend":if self.pg{"postgresql"}else{"sqlite"},"sqlite_version":sqlite_version,"schema_version":1}),
        )
    }
    async fn company_tx(&self, tx: &mut Transaction<'_, Any>, id: &str) -> Result<Company> {
        let row = query!(&sql(self.pg, "SELECT data FROM companies WHERE id=?"))
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or_else(|| err("not_found", "公司不存在"))?;
        Ok(serde_json::from_str(&row.try_get::<String, _>("data")?)?)
    }
    pub async fn company(&self, id: &str) -> Result<Company> {
        let mut tx = self.pool.begin().await?;
        let c = self.company_tx(&mut tx, id).await?;
        tx.commit().await?;
        Ok(c)
    }
    async fn record_tx(
        &self,
        tx: &mut Transaction<'_, Any>,
        company: &str,
        kind: &str,
        id: &str,
    ) -> Result<Option<Record>> {
        let row = query!(&sql(
            self.pg,
            "SELECT data FROM records WHERE company_id=? AND kind=? AND id=?"
        ))
        .bind(company)
        .bind(kind)
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?;
        row.map(|r| serde_json::from_str(&r.try_get::<String, _>("data")?).map_err(Into::into))
            .transpose()
    }
    async fn noncash_tx(
        &self,
        tx: &mut Transaction<'_, Any>,
        company: &str,
    ) -> Result<Vec<Record>> {
        let rows = query!(&sql(
            self.pg,
            "SELECT data FROM records WHERE company_id=? AND kind<>'transaction' ORDER BY kind,id"
        ))
        .bind(company)
        .fetch_all(&mut **tx)
        .await?;
        rows.into_iter()
            .map(|r| Ok(serde_json::from_str(&r.try_get::<String, _>("data")?)?))
            .collect()
    }
    pub async fn list_companies(&self, principal: &Principal) -> Result<Value> {
        let rows = if principal.kind == "owner" {
            query!("SELECT data FROM companies ORDER BY id")
                .fetch_all(&self.pool)
                .await?
        } else {
            query!(&sql(self.pg, "SELECT data FROM companies WHERE id=?"))
                .bind(principal.company_id.as_deref().unwrap_or(""))
                .fetch_all(&self.pool)
                .await?
        };
        let companies: Vec<Company> = rows
            .into_iter()
            .map(|r| Ok(serde_json::from_str(&r.try_get::<String, _>("data")?)?))
            .collect::<Result<_>>()?;
        Ok(json!({"companies":companies}))
    }
    pub async fn records(
        &self,
        company_id: &str,
        kind: Option<&str>,
        history: bool,
    ) -> Result<Value> {
        self.records_page(company_id, kind, history, 0).await
    }
    pub async fn records_page(
        &self,
        company_id: &str,
        kind: Option<&str>,
        history: bool,
        offset: i64,
    ) -> Result<Value> {
        self.records_filtered_page(company_id, kind, None, history, offset)
            .await
    }
    pub async fn records_filtered_page(
        &self,
        company_id: &str,
        kind: Option<&str>,
        record_id: Option<&str>,
        history: bool,
        offset: i64,
    ) -> Result<Value> {
        if offset < 0 {
            return Err(err("invalid_input", "offset 不能为负数"));
        }
        self.company(company_id).await?;
        let table = if history {
            "record_versions"
        } else {
            "records"
        };
        let filter = format!(
            "{}{}",
            if kind.is_some() { " AND kind=?" } else { "" },
            if record_id.is_some() { " AND id=?" } else { "" }
        );
        let count_q = sql(
            self.pg,
            &format!("SELECT COUNT(*) AS n FROM {table} WHERE company_id=?{filter}"),
        );
        let mut query = query!(&count_q).bind(company_id);
        if let Some(kind) = kind {
            query = query.bind(kind);
        }
        if let Some(id) = record_id {
            query = query.bind(id);
        }
        let total: i64 = query.fetch_one(&self.pool).await?.try_get("n")?;
        let q = sql(
            self.pg,
            &format!(
                "SELECT data FROM {table} WHERE company_id=?{filter} ORDER BY kind,id,revision LIMIT 500 OFFSET ?"
            ),
        );
        let mut query = query!(&q).bind(company_id);
        if let Some(kind) = kind {
            query = query.bind(kind);
        }
        if let Some(id) = record_id {
            query = query.bind(id);
        }
        let rows = query.bind(offset).fetch_all(&self.pool).await?;
        let records: Vec<Record> = rows
            .into_iter()
            .map(|r| Ok(serde_json::from_str(&r.try_get::<String, _>("data")?)?))
            .collect::<Result<_>>()?;
        let next = offset + records.len() as i64;
        Ok(
            json!({"records":records,"limit":500,"offset":offset,"total":total,"has_more":next<total,"next_offset":if next<total{Some(next)}else{None}}),
        )
    }
    async fn metrics_tx(
        &self,
        tx: &mut Transaction<'_, Any>,
        company: &Company,
        as_of: Option<&str>,
    ) -> Result<Value> {
        let as_of = match as_of {
            Some(d) => date(d)?,
            None => {
                if company.mode == "simulation" {
                    date(
                        company
                            .simulation_date
                            .as_deref()
                            .unwrap_or(&company.opening_date),
                    )?
                } else {
                    Utc::now()
                        .with_timezone(&company.timezone.parse::<chrono_tz::Tz>()?)
                        .date_naive()
                }
            }
        };
        let mut records = self.noncash_tx(tx, &company.id).await?;
        let rows=query!(&sql(self.pg,"SELECT category,direction,SUBSTR(occurred_on,1,7) AS month,CAST(SUM(amount_minor) AS BIGINT) AS total FROM cash_entries WHERE company_id=? AND occurred_on<=? GROUP BY category,direction,SUBSTR(occurred_on,1,7)" )).bind(&company.id).bind(as_of.to_string()).fetch_all(&mut **tx).await?;
        for (i, row) in rows.into_iter().enumerate() {
            records.push(Record{id:format!("aggregate-{i}"),company_id:company.id.clone(),kind:"transaction".into(),revision:1,company_revision:company.revision,created_at:String::new(),updated_at:String::new(),data:json!({"category":row.try_get::<String,_>("category")?,"direction":row.try_get::<String,_>("direction")?,"occurred_on":format!("{}-01",row.try_get::<String,_>("month")?),"amount_minor":row.try_get::<i64,_>("total")?})});
        }
        Ok(company_core::compute_metrics(company, &records, as_of)?)
    }
    pub async fn metrics(&self, company_id: &str, as_of: Option<&str>) -> Result<Value> {
        let mut tx = self.pool.begin().await?;
        if self.pg {
            query!("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
                .execute(&mut *tx)
                .await?;
        }
        let company = self.company_tx(&mut tx, company_id).await?;
        let metrics = self.metrics_tx(&mut tx, &company, as_of).await?;
        tx.commit().await?;
        Ok(metrics)
    }
    pub async fn snapshot(&self, company_id: &str) -> Result<Value> {
        let mut tx = self.pool.begin().await?;
        if self.pg {
            query!("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
                .execute(&mut *tx)
                .await?;
        }
        let company = self.company_tx(&mut tx, company_id).await?;
        let metrics = self.metrics_tx(&mut tx, &company, None).await?;
        let mut records: BTreeMap<String, Vec<Record>> = BTreeMap::new();
        for r in self.noncash_tx(&mut tx, company_id).await? {
            if r.kind == "skill" {
                continue;
            }
            records.entry(r.kind.clone()).or_default().push(r);
        }
        let rows=query!(&sql(self.pg,"SELECT r.data FROM records r JOIN cash_entries t ON r.company_id=t.company_id AND r.id=t.id WHERE r.company_id=? AND r.kind='transaction' ORDER BY t.occurred_on DESC,r.id DESC LIMIT 200")).bind(company_id).fetch_all(&mut *tx).await?;
        for row in rows {
            records
                .entry("transaction".into())
                .or_default()
                .push(serde_json::from_str(&row.try_get::<String, _>("data")?)?);
        }
        let rows = query!(&sql(
            self.pg,
            "SELECT data FROM events WHERE company_id=? ORDER BY revision DESC LIMIT 30"
        ))
        .bind(company_id)
        .fetch_all(&mut *tx)
        .await?;
        let events: Vec<Value> = rows
            .into_iter()
            .map(|r| Ok(serde_json::from_str(&r.try_get::<String, _>("data")?)?))
            .collect::<Result<_>>()?;
        tx.commit().await?;
        let extensions = self.plugin_extensions(company_id).await?;
        Ok(
            json!({"company":company,"metrics":metrics,"records":records,"events":events,"extensions":extensions,"skills_catalog":format!("/api/v1/companies/{company_id}/skills")}),
        )
    }

    async fn save_company(
        &self,
        tx: &mut Transaction<'_, Any>,
        company: &Company,
        previous: Option<i64>,
    ) -> Result<()> {
        let data = serde_json::to_string(company)?;
        if let Some(rev) = previous {
            let updated = query!(&sql(
                self.pg,
                "UPDATE companies SET revision=?,data=? WHERE id=? AND revision=?"
            ))
            .bind(company.revision)
            .bind(&data)
            .bind(&company.id)
            .bind(rev)
            .execute(&mut **tx)
            .await?;
            if updated.rows_affected() != 1 {
                return Err(err("revision_conflict", "公司版本已变化，请重新读取"));
            }
        } else {
            let updated = query!(&sql(
                self.pg,
                "INSERT INTO companies(id,revision,data) VALUES(?,?,?) ON CONFLICT(id) DO NOTHING"
            ))
            .bind(&company.id)
            .bind(company.revision)
            .bind(&data)
            .execute(&mut **tx)
            .await?;
            if updated.rows_affected() != 1 {
                return Err(err("already_exists", "公司标识已存在"));
            }
        }
        query!(&sql(
            self.pg,
            "INSERT INTO company_versions(id,revision,data) VALUES(?,?,?)"
        ))
        .bind(&company.id)
        .bind(company.revision)
        .bind(data)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }
    // Persistence separates resource identity, record CAS and company snapshot revision.
    #[allow(clippy::too_many_arguments)]
    async fn save_record(
        &self,
        tx: &mut Transaction<'_, Any>,
        company: &str,
        kind: &str,
        id: &str,
        data: Value,
        expected: Option<i64>,
        company_revision: i64,
    ) -> Result<Record> {
        identifier(id)?;
        let old = self.record_tx(tx, company, kind, id).await?;
        if let Some(old) = &old {
            if expected != Some(old.revision) {
                return Err(err(
                    "revision_conflict",
                    "更新已有记录需要当前 expected_revision",
                ));
            }
            if kind == "transaction" {
                return Err(err("immutable_record", "资金记录必须通过冲销更正"));
            }
        } else if expected.is_some_and(|r| r != 0) {
            return Err(err(
                "revision_conflict",
                "新记录的 expected_revision 必须为空或零",
            ));
        }
        let record = Record {
            id: id.into(),
            company_id: company.into(),
            kind: kind.into(),
            company_revision,
            revision: old.as_ref().map(|r| r.revision + 1).unwrap_or(1),
            data,
            created_at: old.map(|r| r.created_at).unwrap_or_else(now),
            updated_at: now(),
        };
        let serialized = serde_json::to_string(&record)?;
        if record.revision == 1 {
            let result=query!(&sql(self.pg,"INSERT INTO records(company_id,kind,id,revision,data) VALUES(?,?,?,?,?) ON CONFLICT(company_id,kind,id) DO NOTHING")).bind(company).bind(kind).bind(id).bind(record.revision).bind(&serialized).execute(&mut **tx).await?;
            if result.rows_affected() != 1 {
                return Err(err("revision_conflict", "记录已被其他请求建立"));
            }
        } else {
            let result=query!(&sql(self.pg,"UPDATE records SET revision=?,data=? WHERE company_id=? AND kind=? AND id=? AND revision=?")).bind(record.revision).bind(&serialized).bind(company).bind(kind).bind(id).bind(record.revision-1).execute(&mut **tx).await?;
            if result.rows_affected() != 1 {
                return Err(err("revision_conflict", "记录已被其他请求修改"));
            }
        }
        query!(&sql(
            self.pg,
            "INSERT INTO record_versions(company_id,kind,id,revision,data) VALUES(?,?,?,?,?)"
        ))
        .bind(company)
        .bind(kind)
        .bind(id)
        .bind(record.revision)
        .bind(serialized)
        .execute(&mut **tx)
        .await?;
        if kind == "transaction" {
            query!(&sql(self.pg,"INSERT INTO cash_entries(company_id,id,account_id,amount_minor,direction,category,occurred_on) VALUES(?,?,?,?,?,?,?)")).bind(company).bind(id).bind(text(&record.data,"account_id")?).bind(integer(&record.data,"amount_minor")?).bind(text(&record.data,"direction")?).bind(text(&record.data,"category")?).bind(text(&record.data,"occurred_on")?).execute(&mut **tx).await?;
        }
        Ok(record)
    }
    pub(crate) async fn writer_transaction(&self) -> Result<Transaction<'static, Any>> {
        if self.pg {
            return Ok(self.write_pool.begin().await?);
        }
        for attempt in 0..3 {
            match self.write_pool.begin_with("BEGIN IMMEDIATE").await {
                Ok(tx) => return Ok(tx),
                Err(error) if sqlite_busy(&error) => {
                    if attempt == 2 {
                        return Err(err(
                            "storage_busy",
                            "数据库仍在写入，请保留原请求标识后重试",
                        ));
                    }
                    tokio::time::sleep(Duration::from_millis(60 * (attempt + 1))).await;
                }
                Err(error) => return Err(error.into()),
            }
        }
        unreachable!()
    }
    pub async fn execute(
        &self,
        company_id: Option<&str>,
        operation: &str,
        request: OperationRequest,
        actor: &str,
    ) -> Result<Value> {
        if operation.starts_with("plugin.") {
            return self
                .call_plugin(
                    company_id.ok_or_else(|| err("invalid_input", "插件调用需要公司"))?,
                    operation,
                    request,
                    actor,
                )
                .await;
        }
        let _guard = self.write_lock.lock().await;
        let mut tx = self.writer_transaction().await?;
        let result = self
            .execute_tx(&mut tx, company_id, operation, request, actor)
            .await?;
        tx.commit().await?;
        self.materialize_result(&result)?;
        if operation == "simulation.fork" {
            let company = result["company"]["id"].as_str().unwrap();
            self.materialize_fork_files(company).await?;
        }
        Ok(result)
    }
    pub async fn import_batch(
        &self,
        company_id: &str,
        operations: Vec<BatchOperation>,
        actor: &str,
    ) -> Result<Value> {
        if operations.is_empty() || operations.len() > 2000 {
            return Err(err("invalid_input", "导入批次需要 1 至 2000 个操作"));
        }
        let _guard = self.write_lock.lock().await;
        let mut tx = self.writer_transaction().await?;
        let mut results = Vec::new();
        for op in operations {
            if ["company.create", "simulation.fork"].contains(&op.operation.as_str())
                || op.operation.starts_with("plugin.")
            {
                return Err(err("invalid_input", "批次仅允许公司内经营操作"));
            }
            results.push(
                self.execute_tx(&mut tx, Some(company_id), &op.operation, op.request, actor)
                    .await?,
            );
        }
        tx.commit().await?;
        for r in &results {
            self.materialize_result(r)?;
        }
        Ok(json!({"status":"committed","results":results}))
    }
    async fn execute_tx(
        &self,
        tx: &mut Transaction<'_, Any>,
        company_id: Option<&str>,
        operation: &str,
        request: OperationRequest,
        actor: &str,
    ) -> Result<Value> {
        identifier(&request.request_id)?;
        if company_core::operation_scope(operation).is_none() {
            return Err(err("unknown_operation", "操作不存在"));
        }
        let input_hash = hash(&serde_json::to_vec(
            &json!({"operation":operation,"company":company_id,"request":request}),
        )?);
        if let Some(row) = query!(&sql(
            self.pg,
            "SELECT input_hash,result FROM receipts WHERE company_key=? AND request_id=?"
        ))
        .bind(key(company_id))
        .bind(&request.request_id)
        .fetch_optional(&mut **tx)
        .await?
        {
            if row.try_get::<String, _>("input_hash")? != input_hash {
                return Err(err("request_id_conflict", "同一 request_id 的内容已变化"));
            }
            let mut result: Value = serde_json::from_str(&row.try_get::<String, _>("result")?)?;
            result["replayed"] = json!(true);
            return Ok(result);
        }
        let mut result;
        let result_company;
        if operation == "company.create" {
            if company_id.is_some() {
                return Err(err("invalid_input", "建立公司必须使用独立创建入口"));
            }
            company_core::validate_company(&request.input)?;
            let id = request
                .input
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(new_id);
            identifier(&id)?;
            let c = Company {
                id,
                name: text(&request.input, "name")?.into(),
                currency: text(&request.input, "currency")?.into(),
                timezone: text(&request.input, "timezone")?.into(),
                description: request
                    .input
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
                opening_date: text(&request.input, "opening_date")?.into(),
                mode: request
                    .input
                    .get("mode")
                    .and_then(Value::as_str)
                    .unwrap_or("real")
                    .into(),
                subscription_tracking: request
                    .input
                    .get("subscription_tracking")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                simulation_date: if request.input.get("mode").and_then(Value::as_str)
                    == Some("simulation")
                {
                    Some(
                        request
                            .input
                            .get("simulation_date")
                            .and_then(Value::as_str)
                            .unwrap_or(text(&request.input, "opening_date")?)
                            .into(),
                    )
                } else {
                    None
                },
                baseline_company_id: None,
                baseline_revision: None,
                revision: 1,
                created_at: now(),
                updated_at: now(),
            };
            self.save_company(tx, &c, None).await?;
            let index_bytes="# Company files\n\n- [context](context/)\n- [documents](documents/)\n- [reports](reports/)\n".as_bytes();
            let digest = self.write_object(index_bytes)?;
            self.save_record(tx,&c.id,"file","company-index",json!({"id":"company-index","relative_path":"INDEX.md","title":"INDEX.md","category":"context","sha256":digest,"size_bytes":index_bytes.len()}),None,1).await?;
            result = json!({"company":c});
            result_company = c;
        } else {
            let id = company_id.ok_or_else(|| err("invalid_input", "此操作需要 company_id"))?;
            identifier(id)?;
            let mut c = self.company_tx(tx, id).await?;
            let previous = c.revision;
            if operation == "simulation.fork" {
                let mut fork = c.clone();
                fork.id = new_id();
                fork.name = text(&request.input, "name")?.into();
                fork.mode = "simulation".into();
                fork.simulation_date = if c.mode == "simulation" {
                    Some(
                        c.simulation_date
                            .clone()
                            .unwrap_or_else(|| c.opening_date.clone()),
                    )
                } else {
                    Some(
                        Utc::now()
                            .with_timezone(&c.timezone.parse::<chrono_tz::Tz>()?)
                            .date_naive()
                            .to_string(),
                    )
                };
                fork.baseline_company_id = Some(c.id.clone());
                fork.baseline_revision = Some(c.revision);
                fork.revision = 1;
                fork.created_at = now();
                fork.updated_at = now();
                self.save_company(tx, &fork, None).await?;
                self.copy_fork_records_tx(tx, &c, &fork).await?;
                result = json!({"company":fork});
                result_company = fork;
            } else if operation == "simulation.advance" {
                if c.mode != "simulation" {
                    return Err(err("simulation_required", "只有模拟公司可以推进经营日期"));
                }
                if request.expected_revision != Some(c.revision) {
                    return Err(err("revision_conflict", "推进日期需要当前公司版本"));
                }
                let next = text(&request.input, "as_of")?;
                if date(next)? < date(c.simulation_date.as_deref().unwrap_or(&c.opening_date))? {
                    return Err(err("invalid_input", "模拟日期不能倒退"));
                }
                c.simulation_date = Some(next.into());
                c.revision += 1;
                c.updated_at = now();
                self.save_company(tx, &c, Some(previous)).await?;
                result = json!({"company":c});
                result_company = c;
            } else if operation == "company.update" {
                if request.expected_revision != Some(c.revision) {
                    return Err(err(
                        "revision_conflict",
                        "公司资料更新需要当前 expected_revision",
                    ));
                }
                c.name = text(&request.input, "name")?.into();
                if let Some(d) = request.input.get("description").and_then(Value::as_str) {
                    c.description = d.into();
                }
                if let Some(track) = request
                    .input
                    .get("subscription_tracking")
                    .and_then(Value::as_bool)
                {
                    c.subscription_tracking = track;
                }
                c.revision += 1;
                c.updated_at = now();
                self.save_company(tx, &c, Some(previous)).await?;
                result = json!({"company":c});
                result_company = c;
            } else {
                let mut input = request.input.clone();
                let mut existing = self.noncash_tx(tx, id).await?;
                if ["network_entity.put", "relationship.put", "profit_entry.put"]
                    .contains(&operation)
                {
                    let record = self
                        .network_operation_tx(tx, &c, &existing, operation, &request)
                        .await?;
                    result = json!({"record":record});
                } else if operation.starts_with("quest.")
                    || operation.starts_with("alert.")
                    || operation == "simulation.draw_event"
                {
                    result = self
                        .game_operation_tx(tx, &c, &existing, operation, &request, actor)
                        .await?;
                } else if operation.starts_with("skill.") {
                    let record = self.skill_operation_tx(tx, &c, operation, &request).await?;
                    result = json!({"record":record});
                } else if operation.starts_with("decision.") {
                    let record = self
                        .decision_operation_tx(tx, &c, &existing, operation, &request, actor)
                        .await?;
                    result = json!({"record":record});
                } else if operation == "transaction.reverse" {
                    let original_id = text(&input, "transaction_id")?;
                    let original = self
                        .record_tx(tx, id, "transaction", original_id)
                        .await?
                        .ok_or_else(|| err("not_found", "资金记录不存在"))?;
                    let reverse_id =
                        format!("rev-{}", hash(format!("{id}:{original_id}").as_bytes()));
                    if self
                        .record_tx(tx, id, "transaction", &reverse_id)
                        .await?
                        .is_some()
                    {
                        return Err(err("already_reversed", "原资金记录已经冲销"));
                    }
                    if original.data.get("reverses").is_some() {
                        return Err(err("invalid_input", "不能对冲销记录再次冲销"));
                    }
                    let reason = text(&input, "reason")?.to_owned();
                    let occurred_on = text(&input, "occurred_on")?.to_owned();
                    date(&occurred_on)?;
                    if date(&occurred_on)? < date(text(&original.data, "occurred_on")?)? {
                        return Err(err("invalid_input", "冲销日期不能早于原资金记录"));
                    }
                    let originals = if original.data["category"] == "transfer" {
                        let group = text(&original.data, "transfer_group")?;
                        let rows = query!(&sql(
                            self.pg,
                            "SELECT data FROM records WHERE company_id=? AND kind='transaction'"
                        ))
                        .bind(id)
                        .fetch_all(&mut **tx)
                        .await?;
                        rows.into_iter()
                            .map(|row| {
                                Ok(serde_json::from_str::<Record>(
                                    &row.try_get::<String, _>("data")?,
                                )?)
                            })
                            .collect::<Result<Vec<_>>>()?
                            .into_iter()
                            .filter(|r| {
                                r.data["transfer_group"] == group
                                    && r.data.get("reverses").is_none()
                            })
                            .collect()
                    } else {
                        vec![original]
                    };
                    let mut reversed = Vec::new();
                    for original in originals {
                        let reverse_id =
                            format!("rev-{}", hash(format!("{id}:{}", original.id).as_bytes()));
                        let mut d = original.data.clone();
                        d["direction"] = json!(if d["direction"] == "inflow" {
                            "outflow"
                        } else {
                            "inflow"
                        });
                        d["occurred_on"] = json!(occurred_on);
                        d["reverses"] = json!(original.id);
                        d["source"] = json!({"kind":"correction","reference":original.id});
                        d["description"] = json!(reason);
                        company_core::validate_record("transaction", &d, &c, &existing)?;
                        reversed.push(
                            self.save_record(
                                tx,
                                id,
                                "transaction",
                                &reverse_id,
                                d,
                                None,
                                c.revision + 1,
                            )
                            .await?,
                        );
                    }
                    result = json!({"records":reversed});
                } else if operation == "transaction.transfer" {
                    let from = text(&input, "from_account_id")?;
                    let to = text(&input, "to_account_id")?;
                    if from == to {
                        return Err(err("invalid_input", "转出和转入账户必须不同"));
                    }
                    let group = new_id();
                    let mut records = Vec::new();
                    for (account, direction, suffix) in
                        [(from, "outflow", "out"), (to, "inflow", "in")]
                    {
                        let d = json!({"account_id":account,"amount_minor":integer(&input,"amount_minor")?,"direction":direction,"category":"transfer","occurred_on":text(&input,"occurred_on")?,"source":input["source"],"description":input.get("description"),"transfer_group":group});
                        company_core::validate_record("transaction", &d, &c, &existing)?;
                        records.push(
                            self.save_record(
                                tx,
                                id,
                                "transaction",
                                &format!("{group}-{suffix}"),
                                d,
                                None,
                                c.revision + 1,
                            )
                            .await?,
                        );
                    }
                    result = json!({"records":records});
                } else {
                    let kind = operation.split('.').next().unwrap();
                    if ["forecast", "report"].contains(&kind)
                        && input.get("baseline_revision").is_none()
                    {
                        input["baseline_revision"] = json!(c.revision);
                    }
                    if kind == "simulation_turn" {
                        if input.get("after_revision").is_none() {
                            input["after_revision"] = json!(c.revision);
                        }
                        if let Some(decision_id) = input.get("decision_id") {
                            let decision_id = decision_id
                                .as_str()
                                .ok_or_else(|| err("invalid_input", "decision_id 必须是文字"))?;
                            let decision = existing
                                .iter()
                                .find(|r| r.kind == "decision" && r.id == decision_id)
                                .ok_or_else(|| {
                                    err("reference_not_found", "回合关联的经营事件不存在")
                                })?;
                            if !matches!(
                                decision.data["status"].as_str(),
                                Some("selected" | "processing")
                            ) {
                                return Err(err(
                                    "invalid_transition",
                                    "回合只能关联已选择的经营事件",
                                ));
                            }
                            let selection_revision = decision
                                .data
                                .get("claim")
                                .and_then(|claim| claim.get("selection_company_revision"))
                                .and_then(Value::as_i64)
                                .unwrap_or(decision.company_revision);
                            if input.get("before_revision").is_none() {
                                input["before_revision"] = json!(selection_revision);
                            }
                            if integer(&input, "before_revision")? != selection_revision {
                                return Err(err(
                                    "revision_conflict",
                                    "回合起点必须对应所关联的选择版本",
                                ));
                            }
                        }
                    }
                    let record_id = if kind == "file" {
                        self.prepare_file(id, &mut input, operation).await?
                    } else {
                        text(&input, "id")?.into()
                    };
                    identifier(&record_id)?;
                    if kind == "file"
                        && existing.iter().any(|r| {
                            r.kind == "file"
                                && r.id != record_id
                                && r.data["relative_path"] == input["relative_path"]
                        })
                    {
                        return Err(err(
                            "file_path_conflict",
                            "此文件路径已绑定其他标识，请读取现有文件记录后更新",
                        ));
                    }
                    if kind == "transaction" && input["category"] == "transfer" {
                        return Err(err(
                            "invalid_input",
                            "内部转账必须使用 transaction.transfer",
                        ));
                    }
                    if kind == "account"
                        && let Some(old) = self.record_tx(tx, id, kind, &record_id).await?
                        && (old.data["opening_balance_minor"] != input["opening_balance_minor"]
                            || old.data.get("opening_on") != input.get("opening_on"))
                    {
                        return Err(err(
                            "immutable_record",
                            "期初余额与日期固定；更正使用资金记录",
                        ));
                    }
                    company_core::validate_record(kind, &input, &c, &existing)?;
                    if ["forecast", "report"].contains(&kind) {
                        let baseline = integer(&input, "baseline_revision")?;
                        let snapshot = self
                            .snapshot_revision_tx(
                                tx,
                                id,
                                baseline,
                                input.get("as_of").and_then(Value::as_str),
                            )
                            .await?;
                        input["metrics_snapshot"] = snapshot["metrics"].clone();
                        input["baseline_mode"] = json!(c.mode);
                    }
                    if kind == "simulation_turn" {
                        if integer(&input, "after_revision")? != c.revision {
                            return Err(err(
                                "revision_conflict",
                                "推演结果需要当前公司版本作为 after_revision",
                            ));
                        }
                        let computed = self
                            .metrics_tx(tx, &c, Some(text(&input, "occurred_on")?))
                            .await?;
                        for outcome in input["outcomes"].as_array().unwrap() {
                            if outcome["status"] == "success"
                                && let Some(goal_id) =
                                    outcome.get("goal_id").and_then(Value::as_str)
                                && existing.iter().any(|r| r.kind == "goal" && r.id == goal_id)
                                && !computed["goal_progress"]
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .any(|g| g["id"] == goal_id && g["status"] == "achieved")
                            {
                                return Err(err(
                                    "goal_not_achieved",
                                    "量化目标尚未达成，不能宣告成功",
                                ));
                            }
                        }
                        input["metrics_snapshot"] = computed;
                    }

                    if kind == "transaction" {
                        let account = existing
                            .iter()
                            .find(|r| {
                                r.kind == "account" && r.id == input["account_id"].as_str().unwrap()
                            })
                            .unwrap();
                        let open = account
                            .data
                            .get("opening_on")
                            .and_then(Value::as_str)
                            .unwrap_or(&c.opening_date);
                        if date(text(&input, "occurred_on")?)? < date(open)? {
                            return Err(err("invalid_input", "资金记录早于账户期初日期"));
                        }
                    }
                    let record = self
                        .save_record(
                            tx,
                            id,
                            kind,
                            &record_id,
                            input,
                            request.expected_revision,
                            c.revision + 1,
                        )
                        .await?;
                    if kind == "goal" {
                        existing.retain(|r| r.kind != "goal" || r.id != record.id);
                        existing.push(record.clone());
                        company_core::validate_goal_cycles(&existing)?;
                    }
                    result = json!({"record":record});
                }
                c.revision += 1;
                c.updated_at = now();
                self.save_company(tx, &c, Some(previous)).await?;
                result_company = c;
            }
        }
        result["operation"] = json!(operation);
        result["status"] = json!("committed");
        result["replayed"] = json!(false);
        result["company_revision"] = json!(result_company.revision);
        result["request_id"] = json!(request.request_id);
        let event = json!({"schema_version":1,"source":"transplane-company","visibility":"private","correlation_id":request.request_id,"id":new_id(),"company_id":result_company.id,"kind":operation,"revision":result_company.revision,"occurred_at":now(),"actor":{"issuer":"local","subject":actor},"resource_id":result.get("record").and_then(|r|r.get("id")),"mode":result_company.mode});
        query!(&sql(
            self.pg,
            "INSERT INTO events(id,company_id,kind,revision,data) VALUES(?,?,?,?,?)"
        ))
        .bind(text(&event, "id")?)
        .bind(&result_company.id)
        .bind(operation)
        .bind(result_company.revision)
        .bind(serde_json::to_string(&event)?)
        .execute(&mut **tx)
        .await?;
        query!(&sql(self.pg,"INSERT INTO receipts(company_key,request_id,operation,input_hash,result,actor,created_at) VALUES(?,?,?,?,?,?,?)")).bind(key(company_id)).bind(&request.request_id).bind(operation).bind(input_hash).bind(serde_json::to_string(&result)?).bind(actor).bind(now()).execute(&mut **tx).await?;
        Ok(result)
    }

    async fn decision_operation_tx(
        &self,
        tx: &mut Transaction<'_, Any>,
        company: &Company,
        existing: &[Record],
        operation: &str,
        request: &OperationRequest,
        actor: &str,
    ) -> Result<Record> {
        let id = text(&request.input, "id")?;
        identifier(id)?;
        let old = self.record_tx(tx, &company.id, "decision", id).await?;
        let mut data = match operation {
            "decision.put" => {
                if existing.iter().any(|r| {
                    r.kind == "decision"
                        && r.id != id
                        && matches!(
                            r.data["status"].as_str(),
                            Some("open" | "selected" | "processing")
                        )
                }) {
                    return Err(err(
                        "active_decision_exists",
                        "这家公司已有未结束的经营事件",
                    ));
                }
                if old.as_ref().is_some_and(|r| r.data["status"] != "open") {
                    return Err(err("invalid_transition", "只能修订尚未选择的经营事件"));
                }
                if request.input.get("selection").is_some()
                    || request.input.get("claim").is_some()
                    || request.input.get("resolution").is_some()
                    || request.input.get("cancellation").is_some()
                {
                    return Err(err("invalid_input", "事件创建不能提交选择或结果"));
                }
                if request
                    .input
                    .get("status")
                    .is_some_and(|status| status != "open")
                {
                    return Err(err("invalid_input", "新事件状态必须是 open"));
                }
                let mut data = request.input.clone();
                if data.get("baseline_revision").is_none() {
                    data["baseline_revision"] = json!(company.revision);
                }
                if integer(&data, "baseline_revision")? != company.revision {
                    return Err(err("revision_conflict", "事件基线需要当前公司版本"));
                }
                data["status"] = json!("open");
                data
            }
            "decision.select" | "decision.claim" | "decision.resolve" | "decision.cancel" => {
                let old_record = old
                    .as_ref()
                    .ok_or_else(|| err("not_found", "经营事件不存在"))?;
                if request.expected_revision != Some(old_record.revision) {
                    return Err(err("revision_conflict", "事件版本已变化，请重新读取"));
                }
                let mut data = old_record.data.clone();
                match operation {
                    "decision.select" => {
                        if !matches!(data["status"].as_str(), Some("open" | "selected")) {
                            return Err(err("invalid_transition", "此事件不能再次选择"));
                        }
                        if company.revision != old_record.company_revision {
                            let changes = query!(&sql(self.pg,
                                "SELECT kind,data FROM events WHERE company_id=? AND revision>? ORDER BY revision"))
                                .bind(&company.id).bind(old_record.company_revision)
                                .fetch_all(&mut **tx).await?;
                            for row in changes {
                                let event: Value =
                                    serde_json::from_str(&row.try_get::<String, _>("data")?)?;
                                if company_core::decision_change_affects_context(
                                    &data,
                                    &row.try_get::<String, _>("kind")?,
                                    event.get("resource_id").and_then(Value::as_str),
                                ) {
                                    return Err(err(
                                        "revision_conflict",
                                        if data["status"] == "selected" {
                                            "已有相关经营操作，不能直接改选；请让 Agent 核对后取消或结算事件"
                                        } else {
                                            "公司状态已变化，请先修订事件"
                                        },
                                    ));
                                }
                            }
                        }
                        let option = request.input.get("option_id").and_then(Value::as_str);
                        let freeform = request.input.get("text").and_then(Value::as_str);
                        let choice = request
                            .input
                            .get("choice")
                            .map(|value| {
                                value
                                    .as_str()
                                    .ok_or_else(|| err("invalid_input", "choice 必须是文字"))
                            })
                            .transpose()?;
                        if [option.is_some(), freeform.is_some(), choice.is_some()]
                            .into_iter()
                            .filter(|present| *present)
                            .count()
                            != 1
                        {
                            return Err(err("invalid_input", "请提供一个方案或自由选择"));
                        }
                        let resolved_choice = if let Some(choice) = choice {
                            let normalize = |value: &str| {
                                value
                                    .split_whitespace()
                                    .collect::<Vec<_>>()
                                    .join(" ")
                                    .to_lowercase()
                            };
                            let title = normalize(choice);
                            let matches: Vec<_> = data["options"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .filter(|item| {
                                    item["id"].as_str() == Some(choice.trim())
                                        || item["title"]
                                            .as_str()
                                            .is_some_and(|value| normalize(value) == title)
                                })
                                .collect();
                            if matches.len() != 1 {
                                return Err(err(
                                    "invalid_input",
                                    "选择文字未匹配唯一方案，请使用方案标识或完整名称",
                                ));
                            }
                            Some(text(matches[0], "id")?.to_owned())
                        } else {
                            None
                        };
                        let (selected_id, selected_text) =
                            if let Some(option_id) = option.or(resolved_choice.as_deref()) {
                                let item = data["options"]
                                    .as_array()
                                    .and_then(|options| {
                                        options
                                            .iter()
                                            .find(|option| option["id"].as_str() == Some(option_id))
                                    })
                                    .ok_or_else(|| err("invalid_input", "所选方案不存在"))?;
                                (Some(option_id.to_owned()), text(item, "title")?.to_owned())
                            } else {
                                if data["allow_freeform"] != true {
                                    return Err(err("invalid_input", "此事件没有开放自由选择"));
                                }
                                let choice = freeform.unwrap().trim();
                                if choice.is_empty() || choice.chars().count() > 4000 {
                                    return Err(err(
                                        "invalid_input",
                                        "自由选择必须是 1 至 4000 字的文字",
                                    ));
                                }
                                (None, choice.to_owned())
                            };
                        data["status"] = json!("selected");
                        data["selection"] = json!({"option_id":selected_id,"text":selected_text,"selected_at":now(),"actor":actor,"company_revision":company.revision});
                        if selected_id.is_none() {
                            data["selection"]
                                .as_object_mut()
                                .unwrap()
                                .remove("option_id");
                        }
                    }
                    "decision.claim" => {
                        if data["status"] != "selected" {
                            return Err(err(
                                "invalid_transition",
                                "只能接手已选择且尚未处理的事件",
                            ));
                        }
                        data["status"] = json!("processing");
                        data["claim"] = json!({"claimed_at":now(),"actor":actor,"company_revision":company.revision,"selection_company_revision":old_record.company_revision});
                    }
                    "decision.resolve" => {
                        if !matches!(data["status"].as_str(), Some("selected" | "processing")) {
                            return Err(err("invalid_transition", "只能结算已选择的事件"));
                        }
                        if request.input.get("company_revision").is_some()
                            && integer(&request.input, "company_revision")? != company.revision
                        {
                            return Err(err("revision_conflict", "结算需要当前公司版本"));
                        }
                        let refs = request
                            .input
                            .get("result_refs")
                            .and_then(Value::as_array)
                            .ok_or_else(|| err("invalid_input", "结算需要 result_refs 数组"))?;
                        if refs.is_empty() || refs.len() > 100 {
                            return Err(err("invalid_input", "结算需要 1 至 100 个结果引用"));
                        }
                        let mut has_required = false;
                        let mut unique = std::collections::BTreeSet::new();
                        for item in refs {
                            let kind = text(item, "kind")?;
                            let reference_id = text(item, "id")?;
                            let revision = integer(item, "revision")?;
                            identifier(reference_id)?;
                            if kind == "decision"
                                || revision < 1
                                || !unique.insert((kind, reference_id, revision))
                            {
                                return Err(err("invalid_result_reference", "结果引用无效或重复"));
                            }
                            if (company.mode == "simulation" && kind == "simulation_turn")
                                || (company.mode == "real"
                                    && company_core::is_real_decision_result(kind))
                            {
                                has_required = true;
                            }
                            if company.mode == "real" && kind == "simulation_turn" {
                                return Err(err(
                                    "invalid_result_reference",
                                    "真实公司不能引用模拟回合",
                                ));
                            }
                            let row = query!(&sql(self.pg,
                                "SELECT data FROM record_versions WHERE company_id=? AND kind=? AND id=? AND revision=?"))
                                .bind(&company.id).bind(kind).bind(reference_id).bind(revision)
                                .fetch_optional(&mut **tx).await?
                                .ok_or_else(|| err("invalid_result_reference", "结果版本在本公司不存在"))?;
                            let referenced: Record =
                                serde_json::from_str(&row.try_get::<String, _>("data")?)?;
                            let selected_revision = old_record
                                .data
                                .get("claim")
                                .and_then(|claim| claim.get("selection_company_revision"))
                                .and_then(Value::as_i64)
                                .unwrap_or(old_record.company_revision);
                            if referenced.company_revision <= selected_revision
                                || referenced.company_revision > company.revision
                            {
                                return Err(err(
                                    "invalid_result_reference",
                                    "结果必须在选择之后提交",
                                ));
                            }
                        }
                        if !has_required {
                            return Err(err(
                                "invalid_result_reference",
                                "真实结算需要实际经营记录或报告，模拟结算需要推演回合",
                            ));
                        }
                        data["status"] = json!("resolved");
                        data["resolution"] = json!({"summary":text(&request.input,"summary")?,"result_refs":refs,"company_revision":company.revision,"resolved_at":now()});
                    }
                    "decision.cancel" => {
                        if !matches!(
                            data["status"].as_str(),
                            Some("open" | "selected" | "processing")
                        ) {
                            return Err(err("invalid_transition", "只能取消未结束的事件"));
                        }
                        data["status"] = json!("cancelled");
                        data["cancellation"] = json!({"reason":text(&request.input,"reason")?,"cancelled_at":now(),"actor":actor});
                    }
                    _ => unreachable!(),
                }
                data
            }
            _ => return Err(err("unknown_operation", "操作不存在")),
        };
        company_core::validate_record("decision", &data, company, existing)?;
        self.save_record(
            tx,
            &company.id,
            "decision",
            id,
            std::mem::take(&mut data),
            request.expected_revision,
            company.revision + 1,
        )
        .await
    }

    fn ensure_company_dirs(&self, id: &str) -> Result<()> {
        identifier(id)?;
        let dir = self.root.join("companies").join(id);
        for name in ["context", "documents", "reports", "imports", "extensions"] {
            std::fs::create_dir_all(dir.join(name))?;
        }
        let index = dir.join("INDEX.md");
        if !index.exists() {
            std::fs::write(
                index,
                "# Company files\n\n- [context](context/)\n- [documents](documents/)\n- [reports](reports/)\n",
            )?;
        }
        Ok(())
    }
    async fn materialize_fork_files(&self, id: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let records = self.noncash_tx(&mut tx, id).await?;
        tx.commit().await?;
        for record in records.into_iter().filter(|r| r.kind == "file") {
            self.materialize_result(
                &json!({"record":record,"operation":"file.put","replayed":true}),
            )?;
        }
        Ok(())
    }
    fn materialize_result(&self, result: &Value) -> Result<()> {
        if let Some(c) = result.get("company")
            && let Ok(c) = serde_json::from_value::<Company>(c.clone())
        {
            self.ensure_company_dirs(&c.id)?;
        }
        if let Some(r) = result.get("record") {
            let record: Record = serde_json::from_value(r.clone())?;
            if record.kind == "file"
                && result.get("operation").and_then(Value::as_str) != Some("file.sync")
            {
                let path = self
                    .safe_company_path(&record.company_id, text(&record.data, "relative_path")?)?;
                if result["replayed"] == true && path.exists() {
                    return Ok(());
                }
                let bytes = std::fs::read(
                    self.root
                        .join("objects")
                        .join(text(&record.data, "sha256")?),
                )?;
                std::fs::create_dir_all(path.parent().unwrap())?;
                let tmp = path.with_file_name(format!(".pending-{}", new_id()));
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&tmp)?;
                file.write_all(&bytes)?;
                file.sync_all()?;
                drop(file);
                std::fs::rename(&tmp, &path)?;
            }
        }
        Ok(())
    }
    fn safe_company_path(&self, id: &str, relative: &str) -> Result<PathBuf> {
        identifier(id)?;
        company_core::relative_path(relative)?;
        self.ensure_company_dirs(id)?;
        let base = self.root.join("companies").join(id);
        let path = base.join(relative);
        let mut p = path.as_path();
        while p.starts_with(&base) {
            if let Ok(meta) = std::fs::symlink_metadata(p) {
                if meta.file_type().is_symlink() {
                    return Err(err("invalid_path", "公司资料路径不能使用符号链接"));
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if meta.file_attributes() & 0x400 != 0 {
                        return Err(err("invalid_path", "公司资料路径不能使用重解析点"));
                    }
                }
            }
            if let Some(parent) = p.parent() {
                p = parent;
            } else {
                break;
            }
        }
        Ok(path)
    }
    fn write_object(&self, data: &[u8]) -> Result<String> {
        if data.len() > 16 * 1024 * 1024 {
            return Err(err("payload_too_large", "文件超过 16 MiB"));
        }
        let digest = hash(data);
        let path = self.root.join("objects").join(&digest);
        if path.exists() {
            if hash(&std::fs::read(&path)?) != digest {
                return Err(err("integrity_error", "已保存文件摘要不一致"));
            }
        } else {
            atomic_write_new(&path, data)?;
        }
        Ok(digest)
    }
    async fn prepare_file(
        &self,
        company: &str,
        input: &mut Value,
        operation: &str,
    ) -> Result<String> {
        let relative = text(input, "relative_path")?.to_owned();
        if input.get("fork_evidence").is_some()
            || input.get("fork_evidence_origin").is_some()
            || input.get("fork_source_content").is_some()
            || input
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| id.to_ascii_lowercase().starts_with("fork-proof-"))
            || input
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| id.to_ascii_lowercase().starts_with("fork-source-"))
            || relative
                .to_ascii_lowercase()
                .starts_with("extensions/provenance/")
        {
            return Err(err("invalid_input", "推演基线依据由系统保存，不能直接修改"));
        }
        let path = self.safe_company_path(company, &relative)?;
        let bytes = if operation == "file.sync" {
            if std::fs::metadata(&path)
                .map_err(|_| err("not_found", "待同步文件不存在"))?
                .len()
                > 16 * 1024 * 1024
            {
                return Err(err("payload_too_large", "文件超过 16 MiB"));
            }
            std::fs::read(&path).map_err(|_| err("not_found", "待同步文件不存在"))?
        } else {
            let encoded = input
                .get("content_base64")
                .and_then(Value::as_str)
                .ok_or_else(|| err("invalid_input", "content_base64 必须是字符串"))?;
            if encoded.len() > (16_usize * 1024 * 1024).div_ceil(3) * 4 {
                return Err(err("payload_too_large", "文件超过 16 MiB"));
            }
            STANDARD
                .decode(encoded)
                .map_err(|_| err("invalid_input", "content_base64 无效"))?
        };
        let sha = self.write_object(&bytes)?;
        let id = input
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| format!("file-{}", hash(relative.as_bytes())));
        input
            .as_object_mut()
            .ok_or_else(|| err("invalid_input", "文件输入必须是对象"))?
            .remove("content_base64");
        input["sha256"] = json!(sha);
        input["size_bytes"] = json!(bytes.len());
        input["relative_path"] = json!(relative);
        input["id"] = json!(id);
        if input.get("title").is_none() {
            input["title"] = json!(path.file_name().unwrap_or_default().to_string_lossy());
        }
        Ok(id)
    }
    pub async fn sync_file(
        &self,
        company: &str,
        relative: &str,
        request_id: &str,
    ) -> Result<Value> {
        self.sync_file_as(company, relative, request_id, "local-cli")
            .await
    }
    pub async fn sync_file_as(
        &self,
        company: &str,
        relative: &str,
        request_id: &str,
        actor: &str,
    ) -> Result<Value> {
        identifier(request_id)?;
        company_core::relative_path(relative)?;
        if let Some(row) = query!(&sql(
            self.pg,
            "SELECT operation,result FROM receipts WHERE company_key=? AND request_id=?"
        ))
        .bind(company)
        .bind(request_id)
        .fetch_optional(&self.pool)
        .await?
        {
            let mut result: Value = serde_json::from_str(&row.try_get::<String, _>("result")?)?;
            if row.try_get::<String, _>("operation")? != "file.sync"
                || result["record"]["data"]["relative_path"] != relative
            {
                return Err(err("request_id_conflict", "同步请求键已用于其他内容"));
            }
            result["replayed"] = json!(true);
            return Ok(result);
        }
        let mut tx = self.pool.begin().await?;
        let candidates = self.noncash_tx(&mut tx, company).await?;
        let current = candidates
            .into_iter()
            .find(|r| r.kind == "file" && r.data["relative_path"] == relative);
        let file_id = current
            .as_ref()
            .map(|r| r.id.clone())
            .unwrap_or_else(|| format!("file-{}", hash(relative.as_bytes())));
        tx.commit().await?;
        self.execute(
            Some(company),
            "file.sync",
            OperationRequest {
                request_id: request_id.into(),
                expected_revision: current.map(|r| r.revision),
                input: json!({"relative_path":relative,"id":file_id}),
            },
            actor,
        )
        .await
    }
    pub async fn download_file(
        &self,
        company: &str,
        id: &str,
        revision: Option<i64>,
    ) -> Result<(Vec<u8>, String)> {
        let q = sql(
            self.pg,
            if revision.is_some() {
                "SELECT data FROM record_versions WHERE company_id=? AND kind='file' AND id=? AND revision=?"
            } else {
                "SELECT data FROM records WHERE company_id=? AND kind='file' AND id=?"
            },
        );
        let mut query = query!(&q).bind(company).bind(id);
        if let Some(rev) = revision {
            query = query.bind(rev);
        }
        let row = query
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| err("not_found", "文件版本不存在"))?;
        let r: Record = serde_json::from_str(&row.try_get::<String, _>("data")?)?;
        let sha = text(&r.data, "sha256")?;
        let bytes = std::fs::read(self.root.join("objects").join(sha))
            .map_err(|_| err("integrity_error", "文件内容缺失"))?;
        if hash(&bytes) != sha {
            return Err(err("integrity_error", "文件摘要校验失败"));
        }
        Ok((
            bytes,
            Path::new(text(&r.data, "relative_path")?)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "document".into()),
        ))
    }

    pub async fn ensure_owner_token(&self) -> Result<Option<String>> {
        let _guard = self.write_lock.lock().await;
        let count: i64 =
            query!("SELECT COUNT(*) AS n FROM credentials WHERE kind='owner' AND revoked=0")
                .fetch_one(&self.pool)
                .await?
                .try_get("n")?;
        if count > 0 {
            return Ok(None);
        }
        let granted = self
            .grant_token_unlocked(None, vec!["admin".into()], true)
            .await?;
        Ok(granted["token"].as_str().map(str::to_owned))
    }
    pub async fn grant_token(
        &self,
        company: Option<&str>,
        scopes: Vec<String>,
        owner: bool,
    ) -> Result<Value> {
        let _guard = self.write_lock.lock().await;
        self.grant_token_unlocked(company, scopes, owner).await
    }
    async fn grant_token_unlocked(
        &self,
        company: Option<&str>,
        scopes: Vec<String>,
        owner: bool,
    ) -> Result<Value> {
        if !owner {
            let company =
                company.ok_or_else(|| err("invalid_input", "Agent 凭据需要 company_id"))?;
            self.company(company).await?;
            if scopes.is_empty()
                || scopes.iter().any(|s| {
                    ![
                        "company.read",
                        "company.write",
                        "files.read",
                        "files.write",
                        "finance.write",
                        "goals.write",
                        "reports.write",
                        "skills.write",
                    ]
                    .contains(&s.as_str())
                })
            {
                return Err(err("invalid_input", "Agent scope 无效"));
            }
        }
        let id = new_id();
        let token = format!("tpc_{}{}", new_id(), new_id());
        let kind = if owner { "owner" } else { "agent" };
        query!(&sql(self.pg,"INSERT INTO credentials(id,token_hash,kind,subject,company_id,scopes,revoked,created_at) VALUES(?,?,?,?,?,?,0,?)")).bind(&id).bind(hash(token.as_bytes())).bind(kind).bind(&id).bind(company.unwrap_or("")).bind(serde_json::to_string(&scopes)?).bind(now()).execute(&self.pool).await?;
        Ok(json!({"id":id,"token":token,"kind":kind,"company_id":company,"scopes":scopes}))
    }
    pub async fn authenticate(&self, token: &str) -> Result<Principal> {
        if let Some(rest) = token.strip_prefix("tpc_exp_") {
            let deadline = rest
                .split('_')
                .next()
                .and_then(|s| s.parse::<i64>().ok())
                .ok_or_else(|| err("unauthorized", "临时凭据无效"))?;
            if Utc::now().timestamp() >= deadline {
                return Err(err("unauthorized", "临时凭据已到期"));
            }
        }
        if token.len() > 256 {
            return Err(err("unauthorized", "凭据无效"));
        }
        let row=query!(&sql(self.pg,"SELECT kind,subject,company_id,scopes FROM credentials WHERE token_hash=? AND revoked=0")).bind(hash(token.as_bytes())).fetch_optional(&self.pool).await?.ok_or_else(||err("unauthorized","凭据无效或已撤销"))?;
        let company: String = row.try_get("company_id")?;
        Ok(Principal {
            kind: row.try_get("kind")?,
            subject: row.try_get("subject")?,
            company_id: if company.is_empty() {
                None
            } else {
                Some(company)
            },
            scopes: serde_json::from_str(&row.try_get::<String, _>("scopes")?)?,
        })
    }
    pub async fn list_tokens(&self) -> Result<Value> {
        let rows=query!("SELECT id,kind,company_id,scopes,revoked,created_at FROM credentials ORDER BY created_at").fetch_all(&self.pool).await?;
        let items:Vec<Value>=rows.into_iter().map(|r|Ok(json!({"id":r.try_get::<String,_>("id")?,"kind":r.try_get::<String,_>("kind")?,"company_id":r.try_get::<String,_>("company_id")?,"scopes":serde_json::from_str::<Value>(&r.try_get::<String,_>("scopes")?)?,"revoked":r.try_get::<i64,_>("revoked")?!=0,"created_at":r.try_get::<String,_>("created_at")?}))).collect::<Result<_>>()?;
        Ok(json!({"tokens":items}))
    }
    pub async fn revoke_token(&self, id: &str) -> Result<Value> {
        let _guard = self.write_lock.lock().await;
        let mut tx = self.pool.begin().await?;
        let row = query!(&sql(
            self.pg,
            "SELECT kind FROM credentials WHERE id=? AND revoked=0"
        ))
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| err("not_found", "有效凭据不存在"))?;
        if row.try_get::<String, _>("kind")? == "owner" {
            let count: i64 =
                query!("SELECT COUNT(*) AS n FROM credentials WHERE kind='owner' AND revoked=0")
                    .fetch_one(&mut *tx)
                    .await?
                    .try_get("n")?;
            if count <= 1 {
                return Err(err("last_owner", "请先建立新的主人凭据，再撤销旧凭据"));
            }
        }
        query!(&sql(self.pg, "UPDATE credentials SET revoked=1 WHERE id=?"))
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(json!({"status":"revoked","id":id}))
    }
}

fn atomic_write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let parent = path.parent().context("output parent")?;
    std::fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".pending-{}", new_id()));
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    drop(f);
    if let Err(e) = std::fs::hard_link(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    std::fs::remove_file(tmp)?;
    Ok(())
}
