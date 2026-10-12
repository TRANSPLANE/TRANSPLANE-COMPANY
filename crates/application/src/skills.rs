use super::*;
use company_core::Locale;

const BUILTINS: [(&str, &str, &str); 4] = [
    (
        "customer-interviews",
        include_str!("../../../skills/company-operating/customer-interviews/SKILL.md"),
        include_str!("../../../skills/company-operating/customer-interviews/SKILL.en.md"),
    ),
    (
        "cashflow-review",
        include_str!("../../../skills/company-operating/cashflow-review/SKILL.md"),
        include_str!("../../../skills/company-operating/cashflow-review/SKILL.en.md"),
    ),
    (
        "weekly-retrospective",
        include_str!("../../../skills/company-operating/weekly-retrospective/SKILL.md"),
        include_str!("../../../skills/company-operating/weekly-retrospective/SKILL.en.md"),
    ),
    (
        "simulation-gm",
        include_str!("../../../skills/company-operating/simulation-gm/SKILL.md"),
        include_str!("../../../skills/company-operating/simulation-gm/SKILL.en.md"),
    ),
];

fn builtin_document(id: &str, locale: Locale) -> Result<Value> {
    let (name, zh, en) = BUILTINS
        .iter()
        .find(|(name, _, _)| id == *name || id == format!("builtin:{name}"))
        .ok_or_else(|| err("not_found", "内置 Skill 不存在"))?;
    let document = if locale == Locale::ZhCn { zh } else { en };
    let normalized = document.replace("\r\n", "\n");
    let rest = normalized
        .strip_prefix("---\n")
        .ok_or_else(|| err("integrity_error", "内置 Skill 缺少元数据"))?;
    let (frontmatter, body) = rest
        .split_once("\n---\n")
        .ok_or_else(|| err("integrity_error", "内置 Skill 元数据无效"))?;
    let field = |key: &str| -> Result<String> {
        let value = frontmatter
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{key}:")))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| err("integrity_error", format!("内置 Skill 缺少 {key}")))?;
        if value.starts_with('"') {
            Ok(serde_json::from_str(value)?)
        } else {
            Ok(value.trim_matches('\'').to_owned())
        }
    };
    let title = body
        .lines()
        .find_map(|line| line.strip_prefix("# "))
        .unwrap_or(name);
    let mut skill = json!({"id":format!("builtin:{name}"),"name":field("name")?,"title":title,"description":field("description")?,"body":body.trim(),"enabled":true,"deleted":false,"effective":true,"source":"builtin","revision":1,"read_only":true,"type":"procedure","tags":[],"modes":if *name=="simulation-gm"{vec!["simulation"]}else{vec!["real","simulation"]}});
    skill["content_sha256"] = json!(hash(
        company_core::skills::skill_document(&skill).as_bytes()
    ));
    Ok(skill)
}

fn metadata(mut skill: Value, company: Option<&str>) -> Value {
    if let Some(obj) = skill.as_object_mut() {
        for field in ["body", "source_refs", "fork_provenance", "deleted_reason"] {
            obj.remove(field);
        }
    }
    if let Some(origin) = skill.get_mut("copied_from").and_then(Value::as_object_mut) {
        origin.remove("document");
    }
    let id = skill["id"].as_str().unwrap();
    let base = company
        .map(|c| format!("/api/v1/companies/{c}/skills/{id}"))
        .unwrap_or_else(|| format!("/api/v1/skills/{id}"));
    skill["read_url"] = json!(base);
    skill["document_url"] = json!(format!("{base}/document"));
    if skill["source"] == "company" {
        skill["history_url"] = json!(format!("{base}/history"));
    }
    skill
}
fn with_links(mut skill: Value, company: Option<&str>) -> Value {
    let meta = metadata(skill.clone(), company);
    for key in ["read_url", "document_url", "history_url"] {
        if let Some(value) = meta.get(key) {
            skill[key] = value.clone();
        }
    }
    skill
}

fn company_skill(record: &Record, mode: &str) -> Value {
    let mut skill = record.data.clone();
    skill["id"] = json!(record.id);
    skill["company_id"] = json!(record.company_id);
    skill["revision"] = json!(record.revision);
    skill["created_at"] = json!(record.created_at);
    skill["updated_at"] = json!(record.updated_at);
    skill["source"] = json!("company");
    skill["read_only"] = json!(false);
    skill["content_sha256"] = json!(hash(
        company_core::skills::skill_document(&skill).as_bytes()
    ));
    skill["effective"] = json!(
        skill["enabled"] == true
            && skill["deleted"] != true
            && skill["modes"]
                .as_array()
                .is_some_and(|modes| modes.iter().any(|item| item == mode))
    );
    skill
}

impl App {
    async fn copy_template_tx(
        &self,
        tx: &mut Transaction<'_, Any>,
        reference: &Value,
    ) -> Result<Value> {
        let object = reference
            .as_object()
            .ok_or_else(|| err("invalid_input", "copy_from 必须为模板引用"))?;
        if object
            .keys()
            .any(|key| !["id", "revision", "content_sha256", "language"].contains(&key.as_str()))
        {
            return Err(err("invalid_input", "copy_from 必须为模板引用"));
        }
        let id = text(reference, "id")?;
        let revision = integer(reference, "revision")?;
        let requested_hash = text(reference, "content_sha256")?;
        let locale = match text(reference, "language")? {
            "en" => Locale::En,
            "zh-CN" => Locale::ZhCn,
            _ => {
                return Err(err(
                    "invalid_input",
                    "copy_from language 必须为 en 或 zh-CN",
                ));
            }
        };
        let (source, document, plugin_version) = if id.starts_with("builtin:") {
            let template = builtin_document(id, locale)
                .map_err(|_| err("revision_conflict", "Skill 来源已变化，请重新读取模板"))?;
            (
                "builtin",
                company_core::skills::skill_document(&template),
                None,
            )
        } else if let Some(qualified) = id.strip_prefix("plugin:") {
            let (plugin, skill_id) = qualified
                .split_once(':')
                .ok_or_else(|| err("invalid_input", "copy_from 必须为模板引用"))?;
            identifier(plugin)?;
            identifier(skill_id)?;
            let row = query!(&sql(
                self.pg,
                "SELECT manifest,version FROM plugins WHERE id=?"
            ))
            .bind(plugin)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or_else(|| err("revision_conflict", "Skill 来源已变化，请重新读取模板"))?;
            let manifest: Value = serde_json::from_str(&row.try_get::<String, _>("manifest")?)?;
            let template = manifest["contributions"]["skills"]
                .as_array()
                .and_then(|items| items.iter().find(|item| item["id"] == skill_id))
                .ok_or_else(|| err("revision_conflict", "Skill 来源已变化，请重新读取模板"))?;
            (
                "plugin",
                company_core::skills::skill_document(template),
                Some(row.try_get::<String, _>("version")?),
            )
        } else {
            return Err(err("invalid_input", "copy_from 只接受内置或插件模板"));
        };
        if revision != 1 || hash(document.as_bytes()) != requested_hash {
            return Err(err("revision_conflict", "Skill 来源已变化，请重新读取模板"));
        }
        let mut origin = json!({"id":id,"source":source,"revision":revision,"language":locale.as_str(),"content_sha256":requested_hash,"document":document});
        if let Some(version) = plugin_version {
            origin["plugin_version"] = json!(version);
        }
        company_core::skills::validate_copied_from(&origin)?;
        Ok(origin)
    }
    pub async fn builtin_skills(&self, locale: Locale) -> Result<Value> {
        let items = BUILTINS
            .iter()
            .map(|(id, _, _)| builtin_document(id, locale).map(|doc| metadata(doc, None)))
            .collect::<Result<Vec<_>>>()?;
        Ok(json!({"version":1,"items":items}))
    }
    pub async fn builtin_skill(&self, id: &str, locale: Locale) -> Result<Value> {
        let skill = with_links(builtin_document(id, locale)?, None);
        Ok(
            json!({"version":1,"document":company_core::skills::skill_document(&skill),"skill":skill}),
        )
    }
    pub async fn company_skills(
        &self,
        company: &str,
        include_deleted: bool,
        locale: Locale,
    ) -> Result<Value> {
        let c = self.company(company).await?;
        let mut items = self.builtin_skills(locale).await?["items"]
            .as_array()
            .unwrap()
            .clone();
        for skill in &mut items {
            *skill = metadata(skill.clone(), Some(company));
            skill["effective"] = json!(
                skill["modes"]
                    .as_array()
                    .is_some_and(|modes| modes.iter().any(|mode| mode == &c.mode))
            );
        }
        for row in query!(&sql(
            self.pg,
            "SELECT data FROM records WHERE company_id=? AND kind='skill' ORDER BY id"
        ))
        .bind(company)
        .fetch_all(&self.pool)
        .await?
        {
            let record: Record = serde_json::from_str(&row.try_get::<String, _>("data")?)?;
            if include_deleted || record.data["deleted"] != true {
                items.push(metadata(company_skill(&record, &c.mode), Some(company)));
            }
        }
        for plugin in self.plugins().await?["installed"]
            .as_array()
            .into_iter()
            .flatten()
        {
            for source in plugin["manifest"]["contributions"]["skills"]
                .as_array()
                .into_iter()
                .flatten()
            {
                let mut skill = source.clone();
                skill["id"] = json!(format!(
                    "plugin:{}:{}",
                    plugin["id"].as_str().unwrap(),
                    source["id"].as_str().unwrap()
                ));
                skill["source"] = json!("plugin");
                skill["plugin_id"] = plugin["id"].clone();
                skill["plugin_version"] = plugin["version"].clone();
                skill["revision"] = json!(1);
                skill["read_only"] = json!(true);
                skill["deleted"] = json!(false);
                skill["content_sha256"] = json!(hash(
                    company_core::skills::skill_document(&skill).as_bytes()
                ));
                let enabled =
                    plugin["enabled"] == true && skill.get("enabled").is_none_or(|v| v == true);
                skill["enabled"] = json!(enabled);
                skill["effective"] = json!(
                    enabled
                        && skill
                            .get("modes")
                            .and_then(Value::as_array)
                            .is_none_or(|modes| modes.iter().any(|mode| mode == &c.mode))
                );
                for (field, value) in [
                    ("tags", json!([])),
                    ("modes", json!(["real", "simulation"])),
                    ("type", json!("procedure")),
                ] {
                    if skill.get(field).is_none() {
                        skill[field] = value;
                    }
                }
                items.push(metadata(skill, Some(company)));
            }
        }
        Ok(json!({"version":1,"company_id":company,"items":items}))
    }
    pub async fn company_skill(
        &self,
        company: &str,
        id: &str,
        revision: Option<i64>,
        locale: Locale,
    ) -> Result<Value> {
        let c = self.company(company).await?;
        if id.starts_with("builtin:") {
            if revision.is_some_and(|revision| revision != 1) {
                return Err(err("not_found", "Skill 版本不存在"));
            }
            let mut result = self.builtin_skill(id, locale).await?;
            result["skill"] = with_links(result["skill"].clone(), Some(company));
            result["skill"]["effective"] = json!(
                result["skill"]["modes"]
                    .as_array()
                    .is_some_and(|modes| modes.iter().any(|mode| mode == &c.mode))
            );
            return Ok(result);
        }
        if let Some(qualified) = id.strip_prefix("plugin:") {
            let (plugin_id, skill_id) = qualified
                .split_once(':')
                .ok_or_else(|| err("not_found", "插件 Skill 不存在"))?;
            if revision.is_some_and(|r| r != 1) {
                return Err(err("not_found", "插件 Skill 版本不存在"));
            }
            let catalog = self.company_skills(company, true, locale).await?;
            let mut skill = catalog["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["id"] == id)
                .cloned()
                .ok_or_else(|| err("not_found", "插件 Skill 不存在"))?;
            let plugins = self.plugins().await?;
            let body = plugins["installed"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["id"] == plugin_id)
                .and_then(|p| p["manifest"]["contributions"]["skills"].as_array())
                .and_then(|items| items.iter().find(|s| s["id"] == skill_id))
                .and_then(|s| s.get("body"))
                .cloned()
                .ok_or_else(|| err("not_found", "插件 Skill 正文不存在"))?;
            skill["body"] = body;
            return Ok(
                json!({"version":1,"document":company_core::skills::skill_document(&skill),"skill":skill}),
            );
        }
        identifier(id)?;
        let q = if revision.is_some() {
            "SELECT data FROM record_versions WHERE company_id=? AND kind='skill' AND id=? AND revision=?"
        } else {
            "SELECT data FROM records WHERE company_id=? AND kind='skill' AND id=?"
        };
        let mut query = query!(&sql(self.pg, q)).bind(company).bind(id);
        if let Some(revision) = revision {
            query = query.bind(revision);
        }
        let row = query
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| err("not_found", "Skill 版本不存在"))?;
        let record: Record = serde_json::from_str(&row.try_get::<String, _>("data")?)?;
        let skill = with_links(company_skill(&record, &c.mode), Some(company));
        Ok(
            json!({"version":1,"document":company_core::skills::skill_document(&skill),"skill":skill}),
        )
    }
    pub async fn skill_history(&self, company: &str, id: &str) -> Result<Value> {
        let c = self.company(company).await?;
        identifier(id)?;
        let rows = query!(&sql(self.pg,"SELECT data FROM record_versions WHERE company_id=? AND kind='skill' AND id=? ORDER BY revision DESC")).bind(company).bind(id).fetch_all(&self.pool).await?;
        if rows.is_empty() {
            return Err(err("not_found", "Skill 不存在"));
        }
        let items = rows
            .into_iter()
            .map(|row| {
                let record: Record = serde_json::from_str(&row.try_get::<String, _>("data")?)?;
                Ok(metadata(company_skill(&record, &c.mode), Some(company)))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(json!({"version":1,"company_id":company,"id":id,"items":items}))
    }
    pub(super) async fn skill_operation_tx(
        &self,
        tx: &mut Transaction<'_, Any>,
        company: &Company,
        operation: &str,
        request: &OperationRequest,
    ) -> Result<Record> {
        let id = text(&request.input, "id")?;
        identifier(id)?;
        let old = self.record_tx(tx, &company.id, "skill", id).await?;
        let mut data = match operation {
            "skill.put" => {
                let allowed = [
                    "id",
                    "name",
                    "title",
                    "description",
                    "body",
                    "enabled",
                    "tags",
                    "modes",
                    "type",
                    "source_refs",
                    "copy_from",
                ];
                let object = request
                    .input
                    .as_object()
                    .ok_or_else(|| err("invalid_input", "Skill 输入必须为对象"))?;
                if object.keys().any(|key| !allowed.contains(&key.as_str())) {
                    return Err(err("invalid_input", "Skill 包含未知或系统维护字段"));
                }
                let mut data = request.input.clone();
                if let Some(reference) = request.input.get("copy_from") {
                    if old.is_some() {
                        return Err(err("invalid_input", "copy_from 只用于新建 Skill"));
                    }
                    data["copied_from"] = self.copy_template_tx(tx, reference).await?;
                    data.as_object_mut().unwrap().remove("copy_from");
                } else if let Some(origin) = old
                    .as_ref()
                    .and_then(|record| record.data.get("copied_from"))
                {
                    data["copied_from"] = origin.clone();
                }
                for (field, default) in [
                    ("enabled", json!(true)),
                    ("tags", json!([])),
                    ("modes", json!(["real", "simulation"])),
                    ("type", json!("procedure")),
                    ("source_refs", json!([])),
                ] {
                    if data.get(field).is_none() {
                        data[field] = old
                            .as_ref()
                            .and_then(|r| r.data.get(field))
                            .cloned()
                            .unwrap_or(default);
                    }
                }
                data["deleted"] = json!(false);
                if let Some(provenance) = old.as_ref().and_then(|r| r.data.get("fork_provenance")) {
                    data["fork_provenance"] = provenance.clone();
                }
                data
            }
            "skill.delete" | "skill.set_enabled" => {
                let allowed = if operation == "skill.delete" {
                    vec!["id", "reason"]
                } else {
                    vec!["id", "enabled"]
                };
                let object = request
                    .input
                    .as_object()
                    .ok_or_else(|| err("invalid_input", "Skill 输入必须为对象"))?;
                if object.keys().any(|key| !allowed.contains(&key.as_str())) {
                    return Err(err("invalid_input", "Skill 包含未知或系统维护字段"));
                }
                let old = old
                    .as_ref()
                    .ok_or_else(|| err("not_found", "Skill 不存在"))?;
                let mut data = old.data.clone();
                if operation == "skill.delete" {
                    data["deleted"] = json!(true);
                    data["enabled"] = json!(false);
                    if request.input.get("reason").is_some() {
                        data["deleted_reason"] = json!(text(&request.input, "reason")?);
                    }
                } else {
                    if data["deleted"] == true {
                        return Err(err("invalid_transition", "恢复 Skill 请提交完整正文"));
                    }
                    data["enabled"] = request
                        .input
                        .get("enabled")
                        .cloned()
                        .ok_or_else(|| err("invalid_input", "缺少 enabled"))?;
                }
                data
            }
            _ => return Err(err("unknown_operation", "Skill 操作不存在")),
        };
        company_core::validate_record("skill", &data, company, &[])?;
        for reference in data["source_refs"].as_array().into_iter().flatten() {
            let found = query!(&sql(self.pg,"SELECT revision FROM record_versions WHERE company_id=? AND kind=? AND id=? AND revision=?"))
                .bind(&company.id).bind(text(reference,"kind")?).bind(text(reference,"id")?).bind(integer(reference,"revision")?).fetch_optional(&mut **tx).await?;
            if found.is_none() {
                return Err(err(
                    "reference_not_found",
                    "Skill 来源的准确版本不存在于当前公司",
                ));
            }
        }
        data["id"] = json!(id);
        self.save_record(
            tx,
            &company.id,
            "skill",
            id,
            data,
            request.expected_revision,
            company.revision + 1,
        )
        .await
    }
}
