use super::*;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Archive {
    format: String,
    schema_version: i64,
    created_at: String,
    sha256: String,
    content: Contents,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Contents {
    companies: Vec<Company>,
    company_versions: Vec<Company>,
    versions: Vec<Record>,
    receipts: Vec<Receipt>,
    events: Vec<Value>,
    plugins: Vec<Value>,
    plugin_data: Vec<Value>,
    blobs: BTreeMap<String, String>,
    workspace_files: BTreeMap<String, String>,
}
#[derive(Serialize, Deserialize)]
struct Receipt {
    company_key: String,
    request_id: String,
    operation: String,
    input_hash: String,
    result: String,
    actor: String,
    created_at: String,
}

pub(crate) fn verified_backup_content(path: &Path) -> Result<Value> {
    use std::io::Read;
    const LIMIT: u64 = 512 * 1024 * 1024;
    if std::fs::metadata(path)?.len() > LIMIT {
        return Err(err("capacity_exceeded", "备份包超过 512 MiB"));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > LIMIT {
        return Err(err("capacity_exceeded", "备份包超过 512 MiB"));
    }
    let archive: Archive = serde_json::from_slice(&bytes)?;
    if archive.format != "transplane-company"
        || archive.schema_version != 1
        || hash(&serde_json::to_vec(&archive.content)?) != archive.sha256
    {
        return Err(err("integrity_error", "备份格式、版本或摘要不匹配"));
    }
    Ok(serde_json::to_value(archive.content)?)
}

impl App {
    pub async fn backup(&self, company: Option<&str>, destination: Option<&Path>) -> Result<Value> {
        let _guard = self.write_lock.lock().await;
        let mut tx = self.pool.begin().await?;
        if self.pg {
            query!("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
                .execute(&mut *tx)
                .await?;
        }
        let filter = if company.is_some() { " WHERE id=?" } else { "" };
        let query = sql(
            self.pg,
            &format!("SELECT data FROM companies{filter} ORDER BY id"),
        );
        let mut q = query!(&query);
        if let Some(id) = company {
            q = q.bind(id);
        }
        let companies: Vec<Company> = q
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .map(|r| Ok(serde_json::from_str(&r.try_get::<String, _>("data")?)?))
            .collect::<Result<_>>()?;
        if company.is_some() && companies.is_empty() {
            return Err(err("not_found", "公司不存在"));
        }
        let mut content = Contents {
            companies: companies.clone(),
            company_versions: vec![],
            versions: vec![],
            receipts: vec![],
            events: vec![],
            plugins: vec![],
            plugin_data: vec![],
            blobs: BTreeMap::new(),
            workspace_files: BTreeMap::new(),
        };
        for c in &companies {
            for row in query!(&sql(
                self.pg,
                "SELECT data FROM company_versions WHERE id=? ORDER BY revision"
            ))
            .bind(&c.id)
            .fetch_all(&mut *tx)
            .await?
            {
                content
                    .company_versions
                    .push(serde_json::from_str(&row.try_get::<String, _>("data")?)?);
            }
            for row in query!(&sql(
                self.pg,
                "SELECT data FROM record_versions WHERE company_id=? ORDER BY kind,id,revision"
            ))
            .bind(&c.id)
            .fetch_all(&mut *tx)
            .await?
            {
                let record: Record = serde_json::from_str(&row.try_get::<String, _>("data")?)?;
                if record.kind == "file" {
                    let digest = text(&record.data, "sha256")?;
                    let bytes = std::fs::read(self.root.join("objects").join(digest))
                        .map_err(|_| err("integrity_error", "备份所需文件缺失"))?;
                    if hash(&bytes) != digest {
                        return Err(err("integrity_error", "备份所需文件已损坏"));
                    }
                    content.blobs.insert(digest.into(), STANDARD.encode(bytes));
                }
                content.versions.push(record);
            }
            for row in query!(&sql(
                self.pg,
                "SELECT * FROM receipts WHERE company_key=? ORDER BY request_id"
            ))
            .bind(&c.id)
            .fetch_all(&mut *tx)
            .await?
            {
                content.receipts.push(Receipt {
                    company_key: row.try_get("company_key")?,
                    request_id: row.try_get("request_id")?,
                    operation: row.try_get("operation")?,
                    input_hash: row.try_get("input_hash")?,
                    result: row.try_get("result")?,
                    actor: row.try_get("actor")?,
                    created_at: row.try_get("created_at")?,
                });
            }
            for row in query!(&sql(
                self.pg,
                "SELECT data FROM events WHERE company_id=? ORDER BY revision,id"
            ))
            .bind(&c.id)
            .fetch_all(&mut *tx)
            .await?
            {
                content
                    .events
                    .push(serde_json::from_str(&row.try_get::<String, _>("data")?)?);
            }
            for row in query!(&sql(
                self.pg,
                "SELECT plugin_id,id,data FROM plugin_data WHERE company_id=? ORDER BY plugin_id,id"
            ))
            .bind(&c.id)
            .fetch_all(&mut *tx)
            .await?
            {
                content.plugin_data.push(json!({"company_id":c.id,"plugin_id":row.try_get::<String,_>("plugin_id")?,"id":row.try_get::<String,_>("id")?,"data":serde_json::from_str::<Value>(&row.try_get::<String,_>("data")?)?}));
            }
            let base = self.root.join("companies").join(&c.id);
            if base.exists() {
                collect_files(&base, &base, &c.id, &mut content.workspace_files)?;
            }
        }
        if company.is_none() {
            for row in query!("SELECT * FROM receipts WHERE company_key='_' ORDER BY request_id")
                .fetch_all(&mut *tx)
                .await?
            {
                content.receipts.push(Receipt {
                    company_key: row.try_get("company_key")?,
                    request_id: row.try_get("request_id")?,
                    operation: row.try_get("operation")?,
                    input_hash: row.try_get("input_hash")?,
                    result: row.try_get("result")?,
                    actor: row.try_get("actor")?,
                    created_at: row.try_get("created_at")?,
                });
            }
        }
        for row in query!("SELECT manifest FROM plugins ORDER BY id")
            .fetch_all(&mut *tx)
            .await?
        {
            content.plugins.push(serde_json::from_str(
                &row.try_get::<String, _>("manifest")?,
            )?);
        }
        tx.commit().await?;
        let sha = hash(&serde_json::to_vec(&content)?);
        let archive = Archive {
            format: "transplane-company".into(),
            schema_version: 1,
            created_at: now(),
            sha256: sha.clone(),
            content,
        };
        let bytes = serde_json::to_vec(&archive)?;
        if bytes.len() > 512 * 1024 * 1024 {
            return Err(err("capacity_exceeded", "备份包超过首版 512 MiB 限额"));
        }
        let path = destination.map(Path::to_path_buf).unwrap_or_else(|| {
            self.root.join("backups").join(format!(
                "{}-{}.tpcompany.json",
                Utc::now().format("%Y%m%d-%H%M%S"),
                new_id()
            ))
        });
        atomic_write_new(&path, &bytes)?;
        Ok(
            json!({"status":"created","path":std::fs::canonicalize(&path)?.to_string_lossy(),"sha256":hash(&bytes),"content_sha256":sha,"size_bytes":bytes.len(),"companies":companies.len(),"credentials_included":false}),
        )
    }
    pub async fn restore(&self, path: &Path) -> Result<Value> {
        let _guard = self.write_lock.lock().await;
        if std::fs::metadata(path)?.len() > 512 * 1024 * 1024 {
            return Err(err("capacity_exceeded", "导入包超过 512 MiB"));
        }
        let archive: Archive = serde_json::from_slice(&std::fs::read(path)?)?;
        if archive.format != "transplane-company"
            || archive.schema_version != 1
            || hash(&serde_json::to_vec(&archive.content)?) != archive.sha256
        {
            return Err(err("integrity_error", "备份格式、版本或摘要不匹配"));
        }
        let content = archive.content;
        let count: i64 = query!("SELECT (SELECT COUNT(*) FROM companies) + (SELECT COUNT(*) FROM records) + (SELECT COUNT(*) FROM record_versions) + (SELECT COUNT(*) FROM company_versions) + (SELECT COUNT(*) FROM cash_entries) + (SELECT COUNT(*) FROM receipts) + (SELECT COUNT(*) FROM events) + (SELECT COUNT(*) FROM plugins) + (SELECT COUNT(*) FROM plugin_data) AS n")
            .fetch_one(&self.pool).await?.try_get("n")?;
        if count != 0
            || std::fs::read_dir(self.root.join("companies"))?
                .next()
                .is_some()
        {
            return Err(err(
                "target_not_empty",
                "恢复目标必须没有经营记录、插件及公司文件",
            ));
        }
        // Validate the entire archive before creating company paths or opening a write transaction.
        let (companies, currents, blobs, workspace_files) = validate_contents(&content)?;
        for bytes in blobs.values() {
            self.write_object(bytes)?;
        }
        let mut staged = RestoreDirectories::new(self.root.join("companies"));
        for company in companies.values() {
            staged.register(&company.id)?;
            self.ensure_company_dirs(&company.id)?;
        }
        for (relative, bytes) in &workspace_files {
            let (id, rel) = relative.split_once('/').unwrap();
            let target = self.safe_company_path(id, rel)?;
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            if target.exists() {
                std::fs::remove_file(&target)?;
            }
            atomic_write_new(&target, bytes)?;
        }
        // A registered file may be absent from the editable workspace, but remains recoverable.
        for record in currents.values().filter(|r| r.kind == "file") {
            let relative = text(&record.data, "relative_path")?;
            let key = format!("{}/{}", record.company_id, relative);
            if !workspace_files.contains_key(&key) {
                let target = self.safe_company_path(&record.company_id, relative)?;
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                if target.exists() {
                    std::fs::remove_file(&target)?;
                }
                atomic_write_new(&target, &blobs[text(&record.data, "sha256")?])?;
            }
        }
        let mut tx = self.writer_transaction().await?;
        for c in companies.values() {
            query!(&sql(
                self.pg,
                "INSERT INTO companies(id,revision,data) VALUES(?,?,?)"
            ))
            .bind(&c.id)
            .bind(c.revision)
            .bind(serde_json::to_string(c)?)
            .execute(&mut *tx)
            .await?;
        }
        for c in &content.company_versions {
            query!(&sql(
                self.pg,
                "INSERT INTO company_versions(id,revision,data) VALUES(?,?,?)"
            ))
            .bind(&c.id)
            .bind(c.revision)
            .bind(serde_json::to_string(c)?)
            .execute(&mut *tx)
            .await?;
        }
        for r in &content.versions {
            query!(&sql(
                self.pg,
                "INSERT INTO record_versions(company_id,kind,id,revision,data) VALUES(?,?,?,?,?)"
            ))
            .bind(&r.company_id)
            .bind(&r.kind)
            .bind(&r.id)
            .bind(r.revision)
            .bind(serde_json::to_string(r)?)
            .execute(&mut *tx)
            .await?;
        }
        for r in currents.values() {
            query!(&sql(
                self.pg,
                "INSERT INTO records(company_id,kind,id,revision,data) VALUES(?,?,?,?,?)"
            ))
            .bind(&r.company_id)
            .bind(&r.kind)
            .bind(&r.id)
            .bind(r.revision)
            .bind(serde_json::to_string(r)?)
            .execute(&mut *tx)
            .await?;
            if r.kind == "transaction" {
                query!(&sql(self.pg,"INSERT INTO cash_entries(company_id,id,account_id,amount_minor,direction,category,occurred_on) VALUES(?,?,?,?,?,?,?)"))
                    .bind(&r.company_id).bind(&r.id).bind(text(&r.data,"account_id")?).bind(integer(&r.data,"amount_minor")?)
                    .bind(text(&r.data,"direction")?).bind(text(&r.data,"category")?).bind(text(&r.data,"occurred_on")?).execute(&mut *tx).await?;
            }
        }
        for r in &content.receipts {
            query!(&sql(self.pg,"INSERT INTO receipts(company_key,request_id,operation,input_hash,result,actor,created_at) VALUES(?,?,?,?,?,?,?)"))
                .bind(&r.company_key).bind(&r.request_id).bind(&r.operation).bind(&r.input_hash).bind(&r.result)
                .bind(&r.actor).bind(&r.created_at).execute(&mut *tx).await?;
        }
        for e in &content.events {
            query!(&sql(
                self.pg,
                "INSERT INTO events(id,company_id,kind,revision,data) VALUES(?,?,?,?,?)"
            ))
            .bind(text(e, "id")?)
            .bind(text(e, "company_id")?)
            .bind(text(e, "kind")?)
            .bind(integer(e, "revision")?)
            .bind(serde_json::to_string(e)?)
            .execute(&mut *tx)
            .await?;
        }
        for manifest in &content.plugins {
            query!(&sql(
                self.pg,
                "INSERT INTO plugins(id,version,enabled,manifest,config) VALUES(?,?,0,?,'{}')"
            ))
            .bind(text(manifest, "id")?)
            .bind(text(manifest, "version")?)
            .bind(serde_json::to_string(manifest)?)
            .execute(&mut *tx)
            .await?;
        }
        for d in &content.plugin_data {
            query!(&sql(
                self.pg,
                "INSERT INTO plugin_data(company_id,plugin_id,id,data) VALUES(?,?,?,?)"
            ))
            .bind(text(d, "company_id")?)
            .bind(text(d, "plugin_id")?)
            .bind(text(d, "id")?)
            .bind(serde_json::to_string(&d["data"])?)
            .execute(&mut *tx)
            .await?;
        }
        query!("UPDATE credentials SET revoked=1")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        staged.committed = true;
        Ok(
            json!({"status":"restored","companies":companies.len(),"records":currents.len(),
            "revisions":content.versions.len(),"remote_authorization":"required","plugins_enabled":false}),
        )
    }
}
fn collect_files(
    base: &Path,
    dir: &Path,
    company: &str,
    files: &mut BTreeMap<String, String>,
) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let meta = std::fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() {
            return Err(err("invalid_path", "备份不接受公司目录符号链接"));
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 {
                return Err(err("invalid_path", "备份不接受公司目录重解析点"));
            }
        }
        if meta.is_dir() {
            collect_files(base, &path, company, files)?;
        } else if meta.is_file() {
            let rel = path
                .strip_prefix(base)?
                .to_string_lossy()
                .replace('\\', "/");
            if rel.starts_with("imports/") || rel.split('/').any(|s| s.starts_with(".pending-")) {
                continue;
            }
            company_core::relative_path(&rel)?;
            if meta.len() > 16 * 1024 * 1024 {
                return Err(err("capacity_exceeded", "公司资料文件超过 16 MiB"));
            }
            let first = std::fs::read(&path)?;
            if hash(&first) != hash(&std::fs::read(&path)?) {
                return Err(err("revision_conflict", "公司文件在备份时被修改，请重试"));
            }
            files.insert(format!("{company}/{rel}"), STANDARD.encode(first));
        }
    }
    Ok(())
}

type RestoreValidation = (
    BTreeMap<String, Company>,
    BTreeMap<(String, String, String), Record>,
    BTreeMap<String, Vec<u8>>,
    BTreeMap<String, Vec<u8>>,
);

fn timestamp(value: &str) -> Result<()> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map_err(|_| err("integrity_error", "历史时间必须为有效 RFC3339"))?;
    Ok(())
}

fn digest_identifier(value: &str) -> Result<()> {
    if value.len() != 64 || !value.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(err("integrity_error", "摘要必须为 SHA256 十六进制值"));
    }
    Ok(())
}

fn decode_file(encoded: &str) -> Result<Vec<u8>> {
    if encoded.len() > (16_usize * 1024 * 1024).div_ceil(3) * 4 {
        return Err(err("capacity_exceeded", "公司资料文件超过 16 MiB"));
    }
    let bytes = STANDARD.decode(encoded)?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(err("capacity_exceeded", "公司资料文件超过 16 MiB"));
    }
    Ok(bytes)
}

fn validate_contents(content: &Contents) -> Result<RestoreValidation> {
    use std::collections::BTreeSet;
    let mut companies = BTreeMap::new();
    for c in &content.companies {
        identifier(&c.id)?;
        company_core::validate_company(&serde_json::to_value(c)?)?;
        timestamp(&c.created_at)?;
        timestamp(&c.updated_at)?;
        if c.revision < 1 || companies.insert(c.id.clone(), c.clone()).is_some() {
            return Err(err("integrity_error", "备份公司标识或版本无效"));
        }
    }
    let mut company_revisions = BTreeMap::new();
    let mut last_companies = BTreeMap::new();
    for c in &content.company_versions {
        identifier(&c.id)?;
        company_core::validate_company(&serde_json::to_value(c)?)?;
        timestamp(&c.created_at)?;
        timestamp(&c.updated_at)?;
        let current = companies
            .get(&c.id)
            .ok_or_else(|| err("integrity_error", "公司历史引用未知公司"))?;
        let expected = company_revisions.entry(c.id.clone()).or_insert(0i64);
        *expected += 1;
        if c.revision != *expected
            || c.currency != current.currency
            || c.timezone != current.timezone
            || c.opening_date != current.opening_date
            || c.mode != current.mode
            || c.baseline_company_id != current.baseline_company_id
            || c.baseline_revision != current.baseline_revision
        {
            return Err(err("integrity_error", "公司历史版本不连续或身份发生变化"));
        }
        last_companies.insert(c.id.clone(), c);
    }
    for c in companies.values() {
        if company_revisions.get(&c.id) != Some(&c.revision)
            || last_companies
                .get(&c.id)
                .map(|last| serde_json::to_value(last).unwrap())
                != Some(serde_json::to_value(c)?)
        {
            return Err(err("integrity_error", "公司当前记录与最后历史版本不一致"));
        }
    }
    let mut currents: BTreeMap<(String, String, String), Record> = BTreeMap::new();
    let mut revision_counts = BTreeMap::new();
    let mut global_revisions = BTreeMap::new();
    let mut blobs = BTreeMap::new();
    for (digest, encoded) in &content.blobs {
        digest_identifier(digest)?;
        let bytes = decode_file(encoded)?;
        if hash(&bytes) != *digest {
            return Err(err("integrity_error", "文件摘要校验失败"));
        }
        blobs.insert(digest.clone(), bytes);
    }
    for r in &content.versions {
        identifier(&r.id)?;
        if !companies.contains_key(&r.company_id) {
            return Err(err("integrity_error", "记录引用未知公司"));
        }
        timestamp(&r.created_at)?;
        timestamp(&r.updated_at)?;
        let k = (r.company_id.clone(), r.kind.clone(), r.id.clone());
        let expected = revision_counts.entry(k.clone()).or_insert(0i64);
        *expected += 1;
        if r.revision != *expected {
            return Err(err("integrity_error", "记录历史版本不连续"));
        }
        let prior_global = global_revisions.insert(k.clone(), r.company_revision);
        if r.company_revision < 1
            || r.company_revision > companies[&r.company_id].revision
            || prior_global.is_some_and(|prior| r.company_revision <= prior)
        {
            return Err(err("integrity_error", "记录所属公司版本越界或未随历史递增"));
        }
        if r.kind == "file" {
            let relative = text(&r.data, "relative_path")?.to_ascii_lowercase();
            let proof = r.id.to_ascii_lowercase().starts_with("fork-proof-");
            let source = r.id.to_ascii_lowercase().starts_with("fork-source-");
            let reserved = proof || source || relative.starts_with("extensions/provenance/");
            let expected_path = if proof {
                format!("extensions/provenance/{}.json", r.id)
            } else {
                format!("extensions/provenance/{}.bin", r.id)
            };
            if (reserved
                && (r.revision != 1
                    || (!proof && !source)
                    || text(&r.data, "relative_path")? != expected_path
                    || companies[&r.company_id].mode != "simulation"
                    || companies[&r.company_id].baseline_company_id.is_none()
                    || (proof && r.data["fork_evidence"] != true)
                    || (proof && r.data.get("fork_source_content").is_some())
                    || (source
                        && (r.data["fork_source_content"] != true
                            || r.data.get("fork_evidence").is_some()
                            || r.id != format!("fork-source-{}", text(&r.data, "sha256")?)))))
                || (!reserved
                    && (r.data.get("fork_evidence").is_some()
                        || r.data.get("fork_source_content").is_some()
                        || r.data.get("fork_evidence_origin").is_some()))
            {
                return Err(err("integrity_error", "推演基线依据来源不符"));
            }
            let digest = text(&r.data, "sha256")?;
            let bytes = blobs
                .get(digest)
                .ok_or_else(|| err("integrity_error", "文件版本内容缺失"))?;
            if integer(&r.data, "size_bytes")? != bytes.len() as i64 {
                return Err(err("integrity_error", "文件版本大小不符"));
            }
        }
        if r.kind == "skill"
            && let Some(previous) = currents.get(&k)
            && previous.data.get("copied_from") != r.data.get("copied_from")
        {
            return Err(err("integrity_error", "Skill 模板来源不能在历史版本中改变"));
        }
        currents.insert(k, r.clone());
    }
    let mut by_company: BTreeMap<String, Vec<Record>> = BTreeMap::new();
    for record in currents.values() {
        by_company
            .entry(record.company_id.clone())
            .or_default()
            .push(record.clone());
    }
    let mut by_resource: BTreeMap<(&str, &str, &str), Vec<&Record>> = BTreeMap::new();
    for version in &content.versions {
        by_resource
            .entry((&version.company_id, &version.kind, &version.id))
            .or_default()
            .push(version);
    }
    // Every historical version is validated, not just each record's final revision.
    for record in &content.versions {
        if record.kind == "skill" {
            for reference in record.data["source_refs"].as_array().into_iter().flatten() {
                let referenced = by_resource
                    .get(&(
                        record.company_id.as_str(),
                        text(reference, "kind")?,
                        text(reference, "id")?,
                    ))
                    .and_then(|versions| {
                        versions.iter().find(|version| {
                            version.revision == reference["revision"].as_i64().unwrap_or(0)
                        })
                    })
                    .ok_or_else(|| err("integrity_error", "Skill 来源的准确公司记录版本缺失"))?;
                if referenced.company_revision >= record.company_revision {
                    return Err(err("integrity_error", "Skill 引用了尚未存在的来源版本"));
                }
            }
        }
        let historical_context;
        let records = if record.kind == "decision" {
            historical_context = decision_references_at(record, &by_resource)?;
            &historical_context
        } else if company_core::network::RECORD_KINDS.contains(&record.kind.as_str())
            || ["quest", "alert"].contains(&record.kind.as_str())
        {
            historical_context = by_resource
                .iter()
                .filter(|((company, _, _), _)| *company == record.company_id)
                .filter_map(|(_, versions)| {
                    versions
                        .iter()
                        .filter(|v| v.company_revision <= record.company_revision)
                        .max_by_key(|v| v.revision)
                        .map(|v| (*v).clone())
                })
                .collect::<Vec<_>>();
            &historical_context
        } else {
            by_company.get(&record.company_id).unwrap()
        };
        company_core::validate_record(
            &record.kind,
            &record.data,
            &companies[&record.company_id],
            records,
        )?;
    }
    validate_decision_history(content, &companies)?;
    for records in by_company.values() {
        company_core::validate_goal_cycles(records)?;
        company_core::game_systems::validate_quest_cycles(records)?;
    }
    for company in companies.values() {
        let records = by_company.get(&company.id).cloned().unwrap_or_default();
        company_core::network::validate_graph_records(company, &records, &content.versions)?;
        company_core::game_systems::validate_game_history(company, &content.versions)?;
        company_core::game_systems::validate_game_results(
            company,
            &content.versions,
            &content.versions,
            &content.company_versions,
            &blobs,
        )?;
        for record in content.versions.iter().filter(|r| {
            r.company_id == company.id
                && company_core::network::RECORD_KINDS.contains(&r.kind.as_str())
        }) {
            for reference in record.data["source_refs"].as_array().into_iter().flatten() {
                if !content.versions.iter().any(|source| {
                    source.company_id == company.id
                        && source.kind == reference["kind"]
                        && source.id == reference["id"]
                        && source.revision == reference["revision"].as_i64().unwrap_or(0)
                        && source.company_revision <= record.company_revision
                }) {
                    return Err(err(
                        "integrity_error",
                        "关系网来源的准确版本不存在于当前公司",
                    ));
                }
            }
        }
    }
    let mut event_ids = BTreeSet::new();
    for event in &content.events {
        let id = text(event, "id")?;
        identifier(id)?;
        let company = companies
            .get(text(event, "company_id")?)
            .ok_or_else(|| err("integrity_error", "事件引用未知公司"))?;
        let revision = integer(event, "revision")?;
        if !event_ids.insert(id)
            || revision < 1
            || revision > company.revision
            || company_core::operation_scope(text(event, "kind")?).is_none()
        {
            return Err(err("integrity_error", "事件标识、版本或操作无效"));
        }
        timestamp(text(event, "occurred_at")?)?;
    }
    let mut receipt_keys = BTreeSet::new();
    for receipt in &content.receipts {
        identifier(&receipt.request_id)?;
        digest_identifier(&receipt.input_hash)?;
        timestamp(&receipt.created_at)?;
        if (receipt.company_key != "_" && !companies.contains_key(&receipt.company_key))
            || !receipt_keys.insert((&receipt.company_key, &receipt.request_id))
            || company_core::operation_scope(&receipt.operation).is_none()
            || receipt.actor.trim().is_empty()
        {
            return Err(err("integrity_error", "操作回执标识或归属无效"));
        }
        let result: Value = serde_json::from_str(&receipt.result)
            .map_err(|_| err("integrity_error", "操作回执结果不是有效 JSON"))?;
        if !result.is_object()
            || result["status"] != "committed"
            || result["request_id"] != receipt.request_id
            || result["company_revision"]
                .as_i64()
                .is_none_or(|revision| revision < 1)
        {
            return Err(err("integrity_error", "操作回执结果与标识不符"));
        }
    }
    let mut plugin_ids = BTreeSet::new();
    for manifest in &content.plugins {
        super::plugins::validate_manifest(manifest)?;
        if !plugin_ids.insert(text(manifest, "id")?) {
            return Err(err("integrity_error", "插件标识重复"));
        }
    }
    let mut plugin_keys = BTreeSet::new();
    for data in &content.plugin_data {
        let company = text(data, "company_id")?;
        let plugin = text(data, "plugin_id")?;
        let id = text(data, "id")?;
        identifier(id)?;
        if !companies.contains_key(company)
            || !plugin_ids.contains(plugin)
            || data.get("data").is_none()
            || !plugin_keys.insert((company, plugin, id))
        {
            return Err(err("integrity_error", "插件数据引用或标识无效"));
        }
    }
    let mut workspace_files = BTreeMap::new();
    for (relative, encoded) in &content.workspace_files {
        let (id, rel) = relative
            .split_once('/')
            .ok_or_else(|| err("invalid_path", "公司文件路径无效"))?;
        if !companies.contains_key(id) {
            return Err(err("invalid_path", "文件引用未知公司"));
        }
        company_core::relative_path(rel)?;
        workspace_files.insert(relative.clone(), decode_file(encoded)?);
    }
    Ok((companies, currents, blobs, workspace_files))
}

fn decision_references_at(
    decision: &Record,
    by_resource: &BTreeMap<(&str, &str, &str), Vec<&Record>>,
) -> Result<Vec<Record>> {
    let mut context = Vec::new();
    let mut references = Vec::new();
    if let Some(id) = decision.data.get("route_id").and_then(Value::as_str) {
        references.push(("route", id));
    }
    for (field, kind) in [("goal_ids", "goal"), ("file_ids", "file")] {
        if let Some(ids) = decision.data.get(field).and_then(Value::as_array) {
            for id in ids {
                if let Some(id) = id.as_str() {
                    references.push((kind, id));
                }
            }
        }
    }
    for (kind, id) in references {
        if let Some(version) = by_resource
            .get(&(decision.company_id.as_str(), kind, id))
            .and_then(|versions| {
                versions
                    .iter()
                    .rev()
                    .find(|version| version.company_revision <= decision.company_revision)
            })
        {
            context.push((*version).clone());
        }
    }
    Ok(context)
}

fn validate_decision_history(
    content: &Contents,
    companies: &BTreeMap<String, Company>,
) -> Result<()> {
    use std::collections::BTreeSet;
    let mut exact = BTreeMap::new();
    let mut decisions: BTreeMap<(String, String), Vec<&Record>> = BTreeMap::new();
    let mut timeline: BTreeMap<String, Vec<&Record>> = BTreeMap::new();
    for record in &content.versions {
        exact.insert(
            (
                record.company_id.as_str(),
                record.kind.as_str(),
                record.id.as_str(),
                record.revision,
            ),
            record,
        );
        if record.kind == "decision" {
            decisions
                .entry((record.company_id.clone(), record.id.clone()))
                .or_default()
                .push(record);
            timeline
                .entry(record.company_id.clone())
                .or_default()
                .push(record);
        }
    }
    for turn in content
        .versions
        .iter()
        .filter(|record| record.kind == "simulation_turn")
    {
        if let Some(decision_id) = turn.data.get("decision_id") {
            let decision_id = decision_id
                .as_str()
                .ok_or_else(|| err("integrity_error", "回合关联的经营事件无效"))?;
            let selected = content
                .versions
                .iter()
                .filter(|record| {
                    record.company_id == turn.company_id
                        && record.kind == "decision"
                        && record.id == decision_id
                        && record.company_revision < turn.company_revision
                })
                .max_by_key(|record| record.company_revision)
                .ok_or_else(|| err("integrity_error", "回合关联的经营事件不存在"))?;
            let selected_revision = selected
                .data
                .get("claim")
                .and_then(|claim| claim.get("selection_company_revision"))
                .and_then(Value::as_i64)
                .unwrap_or(selected.company_revision);
            if !matches!(
                selected.data["status"].as_str(),
                Some("selected" | "processing")
            ) || integer(&turn.data, "before_revision")? != selected_revision
            {
                return Err(err("integrity_error", "回合起点与经营事件选择不匹配"));
            }
        }
    }
    for ((company_id, _), history) in &decisions {
        let company = &companies[company_id];
        let mut selected_revision = None;
        for (index, record) in history.iter().enumerate() {
            let status = text(&record.data, "status")?;
            let previous = index.checked_sub(1).map(|i| history[i]);
            match status {
                "open" => {
                    if previous.is_some_and(|r| r.data["status"] != "open")
                        || integer(&record.data, "baseline_revision")? + 1
                            != record.company_revision
                        || record.data.get("selection").is_some()
                        || record.data.get("claim").is_some()
                        || record.data.get("resolution").is_some()
                        || record.data.get("cancellation").is_some()
                    {
                        return Err(err("integrity_error", "经营事件开放版本或基线无效"));
                    }
                }
                "selected" => {
                    let previous =
                        previous.ok_or_else(|| err("integrity_error", "经营事件缺少开放版本"))?;
                    if !matches!(previous.data["status"].as_str(), Some("open" | "selected")) {
                        return Err(err("integrity_error", "经营事件选择顺序无效"));
                    }
                    let cursor = record.data["selection"].get("company_revision");
                    if let Some(cursor) = cursor {
                        let cursor = cursor
                            .as_i64()
                            .ok_or_else(|| err("integrity_error", "经营事件选择公司版本无效"))?;
                        if cursor != record.company_revision - 1
                            || cursor < previous.company_revision
                        {
                            return Err(err("integrity_error", "经营事件选择公司版本无效"));
                        }
                        let changed_record = content.versions.iter().any(|changed| {
                            changed.company_id == *company_id
                                && changed.company_revision > previous.company_revision
                                && changed.company_revision <= cursor
                                && company_core::decision_change_affects_context(
                                    &previous.data,
                                    &changed.kind,
                                    Some(&changed.id),
                                )
                        });
                        let changed_event = content.events.iter().any(|event| {
                            event["company_id"] == *company_id
                                && event["revision"].as_i64().is_some_and(|revision| {
                                    revision > previous.company_revision && revision <= cursor
                                })
                                && company_core::decision_change_affects_context(
                                    &previous.data,
                                    event["kind"].as_str().unwrap_or(""),
                                    event["resource_id"].as_str(),
                                )
                        });
                        let baseline = content
                            .company_versions
                            .iter()
                            .find(|version| {
                                version.id == *company_id
                                    && version.revision == previous.company_revision
                            })
                            .ok_or_else(|| err("integrity_error", "经营事件基线公司版本不存在"))?;
                        let changed_company = content.company_versions.iter().any(|version| {
                            version.id == *company_id
                                && version.revision > previous.company_revision
                                && version.revision <= cursor
                                && (version.name != baseline.name
                                    || version.description != baseline.description
                                    || version.simulation_date != baseline.simulation_date
                                    || version.subscription_tracking
                                        != baseline.subscription_tracking)
                        });
                        if changed_record || changed_event || changed_company {
                            return Err(err("integrity_error", "经营事件选择前已有相关经营变化"));
                        }
                    } else if record.company_revision != previous.company_revision + 1 {
                        // Legacy selections did not store a cursor and could
                        // only follow their open version immediately.
                        return Err(err("integrity_error", "经营事件选择顺序无效"));
                    }
                    let mut expected = previous.data.clone();
                    expected["status"] = json!("selected");
                    expected["selection"] = record.data["selection"].clone();
                    if record.data != expected {
                        return Err(err("integrity_error", "经营事件选择改变了事件内容"));
                    }
                    let selection = &record.data["selection"];
                    timestamp(text(selection, "selected_at")?)?;
                    let choice = text(selection, "text")?;
                    if let Some(option_id) = selection.get("option_id") {
                        let option = record.data["options"]
                            .as_array()
                            .and_then(|options| {
                                options.iter().find(|option| option["id"] == *option_id)
                            })
                            .ok_or_else(|| err("integrity_error", "经营事件选择引用未知方案"))?;
                        if text(option, "title")? != choice {
                            return Err(err("integrity_error", "经营事件选择文字与方案不符"));
                        }
                    } else if record.data["allow_freeform"] != true || choice.chars().count() > 4000
                    {
                        return Err(err("integrity_error", "经营事件自由选择无效"));
                    }
                    selected_revision = Some(record.company_revision);
                }
                "processing" => {
                    let previous =
                        previous.ok_or_else(|| err("integrity_error", "经营事件缺少选择版本"))?;
                    if previous.data["status"] != "selected" {
                        return Err(err("integrity_error", "经营事件接手顺序无效"));
                    }
                    let mut expected = previous.data.clone();
                    expected["status"] = json!("processing");
                    expected["claim"] = record.data["claim"].clone();
                    let claim = &record.data["claim"];
                    if expected != record.data
                        || integer(claim, "company_revision")? != record.company_revision - 1
                        || integer(claim, "selection_company_revision")?
                            != previous.company_revision
                    {
                        return Err(err("integrity_error", "经营事件接手改变了选择或版本"));
                    }
                    timestamp(text(claim, "claimed_at")?)?;
                }
                "resolved" => {
                    let previous =
                        previous.ok_or_else(|| err("integrity_error", "经营事件缺少选择版本"))?;
                    if !matches!(
                        previous.data["status"].as_str(),
                        Some("selected" | "processing")
                    ) {
                        return Err(err("integrity_error", "经营事件结算顺序无效"));
                    }
                    let mut expected = previous.data.clone();
                    expected["status"] = json!("resolved");
                    expected["resolution"] = record.data["resolution"].clone();
                    if record.data != expected {
                        return Err(err("integrity_error", "经营事件结算改变了事件内容"));
                    }
                    let resolution = &record.data["resolution"];
                    timestamp(text(resolution, "resolved_at")?)?;
                    let fixed = integer(resolution, "company_revision")?;
                    if fixed != record.company_revision - 1 {
                        return Err(err("integrity_error", "经营事件结算公司版本不匹配"));
                    }
                    let refs = resolution["result_refs"]
                        .as_array()
                        .ok_or_else(|| err("integrity_error", "经营事件结算缺少结果引用"))?;
                    if refs.is_empty() || refs.len() > 100 {
                        return Err(err("integrity_error", "经营事件结果引用数量无效"));
                    }
                    let mut has_required = false;
                    let mut unique = BTreeSet::new();
                    for reference in refs {
                        let kind = text(reference, "kind")?;
                        let id = text(reference, "id")?;
                        let revision = integer(reference, "revision")?;
                        if kind == "decision"
                            || revision < 1
                            || !unique.insert((kind, id, revision))
                            || (company.mode == "real" && kind == "simulation_turn")
                        {
                            return Err(err("integrity_error", "经营事件结果引用类型无效"));
                        }
                        let target = exact
                            .get(&(company_id.as_str(), kind, id, revision))
                            .ok_or_else(|| err("integrity_error", "经营事件结果引用版本不存在"))?;
                        if target.company_revision <= selected_revision.unwrap_or(0)
                            || target.company_revision > fixed
                        {
                            return Err(err("integrity_error", "经营事件结果不在选择与结算之间"));
                        }
                        if (company.mode == "real" && company_core::is_real_decision_result(kind))
                            || (company.mode == "simulation" && kind == "simulation_turn")
                        {
                            has_required = true;
                        }
                    }
                    if !has_required {
                        return Err(err("integrity_error", "经营事件结算缺少对应模式的结果"));
                    }
                }
                "cancelled" => {
                    let previous =
                        previous.ok_or_else(|| err("integrity_error", "经营事件缺少初始版本"))?;
                    if !matches!(
                        previous.data["status"].as_str(),
                        Some("open" | "selected" | "processing")
                    ) {
                        return Err(err("integrity_error", "经营事件取消顺序无效"));
                    }
                    let mut expected = previous.data.clone();
                    expected["status"] = json!("cancelled");
                    expected["cancellation"] = record.data["cancellation"].clone();
                    if record.data != expected {
                        return Err(err("integrity_error", "经营事件取消改变了事件内容"));
                    }
                    timestamp(text(&record.data["cancellation"], "cancelled_at")?)?;
                }
                _ => return Err(err("integrity_error", "经营事件状态无效")),
            }
        }
    }
    for events in timeline.values_mut() {
        events.sort_by_key(|record| record.company_revision);
        let mut active: Option<&str> = None;
        let mut last_revision = 0;
        for record in events {
            if record.company_revision <= last_revision {
                return Err(err("integrity_error", "经营事件公司版本重叠"));
            }
            last_revision = record.company_revision;
            match text(&record.data, "status")? {
                "open" => {
                    if active.is_some_and(|id| id != record.id) {
                        return Err(err("integrity_error", "同一公司存在多个未结束事件"));
                    }
                    active = Some(&record.id);
                }
                "selected" | "processing" => {
                    if active != Some(record.id.as_str()) {
                        return Err(err("integrity_error", "经营事件选择顺序无效"));
                    }
                }
                "resolved" | "cancelled" => {
                    if active != Some(record.id.as_str()) {
                        return Err(err("integrity_error", "经营事件结束顺序无效"));
                    }
                    active = None;
                }
                _ => unreachable!(),
            }
        }
    }
    Ok(())
}

struct RestoreDirectories {
    root: PathBuf,
    created: Vec<PathBuf>,
    committed: bool,
}
impl RestoreDirectories {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            created: vec![],
            committed: false,
        }
    }
    fn register(&mut self, id: &str) -> Result<()> {
        identifier(id)?;
        let path = self.root.join(id);
        if path.parent() != Some(self.root.as_path()) || path.exists() {
            return Err(err("target_not_empty", "恢复目标的公司目录已经存在"));
        }
        // Register before directory creation so partial I/O failures are cleaned up too.
        self.created.push(path);
        Ok(())
    }
}
impl Drop for RestoreDirectories {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        for path in self.created.iter().rev() {
            if path.parent() != Some(self.root.as_path()) {
                continue;
            }
            if let Ok(metadata) = std::fs::symlink_metadata(path) {
                if metadata.file_type().is_symlink() {
                    let _ = std::fs::remove_file(path);
                    continue;
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if metadata.file_attributes() & 0x400 != 0 {
                        let _ = std::fs::remove_dir(path);
                        continue;
                    }
                }
                let _ = std::fs::remove_dir_all(path);
            }
        }
    }
}
