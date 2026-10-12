use super::*;
use company_core::{Locale, game_systems as game};
use std::collections::BTreeSet;

fn localized(zh: &str, en: &str, locale: Locale) -> String {
    if locale == Locale::ZhCn {
        zh.into()
    } else {
        en.into()
    }
}
fn game_day(company: &Company) -> Result<String> {
    Ok(if company.mode == "simulation" {
        company
            .simulation_date
            .clone()
            .unwrap_or_else(|| company.opening_date.clone())
    } else {
        Utc::now()
            .with_timezone(&company.timezone.parse::<chrono_tz::Tz>()?)
            .date_naive()
            .to_string()
    })
}
fn reference(record: &Record) -> Value {
    json!({"kind":record.kind,"id":record.id,"revision":record.revision})
}
fn command(operation: &str, input: Value, revision: Option<i64>) -> Value {
    json!({"operation":operation,"input":input,"expected_revision":revision})
}
fn report_evidence_key(record: &Record) -> Result<String> {
    use chrono::Datelike;
    let end = date(text(&record.data, "period_end")?)?;
    let period = if record.data["type"] == "monthly" {
        format!("{}-{:02}", end.year(), end.month())
    } else {
        let week = end.iso_week();
        format!("{}-W{:02}", week.year(), week.week())
    };
    Ok(format!("report:{}:{period}", text(&record.data, "type")?))
}

impl App {
    async fn game_require_dependencies_tx(
        &self,
        tx: &mut Transaction<'_, Any>,
        company: &Company,
        data: &Value,
    ) -> Result<()> {
        let deps = data["depends_on"].as_array().cloned().unwrap_or_default();
        if deps.is_empty() {
            return Ok(());
        }
        let metrics = self.metrics_tx(tx, company, None).await?;
        let state = self
            .game_state_tx(tx, company, &metrics, Locale::ZhCn)
            .await?;
        for dep in deps.iter().filter_map(Value::as_str) {
            if !state["quests"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|q| q["kind"] == "quest" && q["id"] == dep && q["completion_valid"] == true)
            {
                return Err(err("dependency_incomplete", "前置任务尚未完成"));
            }
        }
        Ok(())
    }
    async fn game_fork_evidence_tx(
        &self,
        tx: &mut Transaction<'_, Any>,
        company: &Company,
        file: &Record,
    ) -> Result<Option<game::ResolvedForkEvidence>> {
        if file.kind != "file" || file.data["fork_evidence"] != true {
            return Ok(None);
        }
        let rows = query!(&sql(
            self.pg,
            "SELECT data FROM records WHERE company_id=? AND kind='file'"
        ))
        .bind(&company.id)
        .fetch_all(&mut **tx)
        .await?;
        let files = rows
            .into_iter()
            .map(|row| {
                Ok(serde_json::from_str::<Record>(
                    &row.try_get::<String, _>("data")?,
                )?)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(game::resolve_fork_evidence(
            company,
            file,
            &files,
            &mut |digest| {
                std::fs::read(self.root.join("objects").join(digest))
                    .map_err(|_| DomainError::new("integrity_error", "推演基线依据内容缺失"))
            },
        )?)
    }
    async fn game_transaction_reversed_tx(
        &self,
        tx: &mut Transaction<'_, Any>,
        company: &Company,
        id: &str,
    ) -> Result<bool> {
        let rows = query!(&sql(
            self.pg,
            "SELECT data FROM records WHERE company_id=? AND kind='transaction' AND id LIKE 'rev-%'"
        ))
        .bind(&company.id)
        .fetch_all(&mut **tx)
        .await?;
        for row in rows {
            let r: Record = serde_json::from_str(&row.try_get::<String, _>("data")?)?;
            if r.data["reverses"] == id {
                return Ok(true);
            }
        }
        Ok(false)
    }
    async fn game_ref_tx(
        &self,
        tx: &mut Transaction<'_, Any>,
        company: &str,
        reference: &Value,
    ) -> Result<Record> {
        let row = query!(&sql(
            self.pg,
            "SELECT data FROM record_versions WHERE company_id=? AND kind=? AND id=? AND revision=?"
        ))
        .bind(company)
        .bind(text(reference, "kind")?)
        .bind(text(reference, "id")?)
        .bind(integer(reference, "revision")?)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| err("reference_not_found", "游戏依据的准确版本在本公司不存在"))?;
        Ok(serde_json::from_str(&row.try_get::<String, _>("data")?)?)
    }
    async fn game_validate_refs_tx(
        &self,
        tx: &mut Transaction<'_, Any>,
        company: &str,
        refs: &Value,
    ) -> Result<()> {
        game::validate_exact_refs(refs)?;
        for reference in refs.as_array().unwrap() {
            self.game_ref_tx(tx, company, reference).await?;
        }
        Ok(())
    }
    async fn game_result_valid_tx(
        &self,
        tx: &mut Transaction<'_, Any>,
        company: &Company,
        reference: &Value,
        metrics: &Value,
        day: &str,
    ) -> Result<bool> {
        self.game_ref_tx(tx, &company.id, reference).await?;
        let Some(current) = self
            .record_tx(
                tx,
                &company.id,
                text(reference, "kind")?,
                text(reference, "id")?,
            )
            .await?
        else {
            return Ok(false);
        };
        if let Some(proof) = self.game_fork_evidence_tx(tx, company, &current).await? {
            let source = proof.source;
            let current_copy = self
                .record_tx(tx, &company.id, &source.kind, &source.id)
                .await?;
            let effective_metrics = if current_copy.is_some() {
                metrics
            } else {
                &proof.baseline_metrics
            };
            let effective = current_copy.as_ref().unwrap_or(&source);
            return Ok(match source.kind.as_str() {
                "goal" => effective_metrics["goal_progress"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|g| g["id"] == source.id && g["status"] == "achieved"),
                "decision" => effective.data["status"] == "resolved",
                "report" => {
                    !effective.data["body"]
                        .as_str()
                        .unwrap_or("")
                        .trim()
                        .is_empty()
                        && effective.data["period_end"]
                            .as_str()
                            .is_some_and(|d| d <= day)
                        && effective.data["as_of"].as_str().is_some_and(|d| d <= day)
                }
                "simulation_turn" => effective.data["outcomes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|o| {
                        o["status"] == "success"
                            && (o.get("goal_id").is_none()
                                || effective_metrics["goal_progress"]
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .any(|g| g["id"] == o["goal_id"] && g["status"] == "achieved"))
                    }),
                "business" => matches!(
                    effective.data["status"].as_str(),
                    Some("released" | "completed")
                ),
                "transaction" => {
                    effective.data.get("reverses").is_none()
                        && effective.data["occurred_on"]
                            .as_str()
                            .is_some_and(|d| d <= day)
                        && !self
                            .game_transaction_reversed_tx(tx, company, &source.id)
                            .await?
                }
                "file" => true,
                _ => false,
            });
        }
        Ok(match current.kind.as_str() {
            "goal" => metrics["goal_progress"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|g| g["id"] == current.id && g["status"] == "achieved"),
            "decision" => current.data["status"] == "resolved",
            "report" => {
                !current.data["body"]
                    .as_str()
                    .unwrap_or("")
                    .trim()
                    .is_empty()
                    && current.data["period_end"]
                        .as_str()
                        .is_some_and(|d| d <= day)
                    && current.data["as_of"].as_str().is_some_and(|d| d <= day)
            }
            "simulation_turn" => {
                company.mode == "simulation"
                    && current.data["occurred_on"]
                        .as_str()
                        .is_some_and(|d| d <= day)
                    && current.data["outcomes"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|o| {
                            o["status"] == "success"
                                && (o.get("goal_id").is_none()
                                    || metrics["goal_progress"]
                                        .as_array()
                                        .into_iter()
                                        .flatten()
                                        .any(|g| {
                                            g["id"] == o["goal_id"] && g["status"] == "achieved"
                                        }))
                        })
            }
            "business" => matches!(
                current.data["status"].as_str(),
                Some("released" | "completed")
            ),
            "transaction" => {
                current.data.get("reverses").is_none()
                    && current.data["occurred_on"]
                        .as_str()
                        .is_some_and(|d| d <= day)
                    && !self
                        .game_transaction_reversed_tx(tx, company, &current.id)
                        .await?
            }
            "file" => true,
            _ => false,
        })
    }
    async fn game_evidence_keys_tx(
        &self,
        tx: &mut Transaction<'_, Any>,
        company: &Company,
        refs: &[Value],
    ) -> Result<Vec<String>> {
        let mut keys = BTreeSet::new();
        for reference in refs {
            let raw = self.game_ref_tx(tx, &company.id, reference).await?;
            let record = if let Some(proof) = self.game_fork_evidence_tx(tx, company, &raw).await? {
                proof.source
            } else {
                raw
            };
            keys.insert(format!("{}:{}", record.kind, record.id));
            match record.kind.as_str() {
                "report" => {
                    keys.insert(report_evidence_key(&record)?);
                }
                "decision" => {
                    keys.insert(format!("decision:{}", record.id));
                    for nested in record.data["resolution"]["result_refs"]
                        .as_array()
                        .into_iter()
                        .flatten()
                    {
                        if record.company_id != company.id {
                            keys.insert(format!(
                                "{}:{}",
                                text(nested, "kind")?,
                                text(nested, "id")?
                            ));
                            continue;
                        }
                        let nested_record = self.game_ref_tx(tx, &company.id, nested).await?;
                        if nested_record.kind == "report" {
                            keys.insert(report_evidence_key(&nested_record)?);
                        } else {
                            keys.insert(format!("{}:{}", nested_record.kind, nested_record.id));
                        }
                        if nested_record.kind == "simulation_turn" {
                            for outcome in nested_record.data["outcomes"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter(|o| o["status"] == "success")
                            {
                                if let Some(id) = outcome["goal_id"].as_str() {
                                    keys.insert(format!("goal:{id}"));
                                }
                            }
                        }
                    }
                }
                _ => {
                    keys.insert(format!("{}:{}", record.kind, record.id));
                }
            }
            if record.kind == "simulation_turn" {
                for outcome in record.data["outcomes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|o| o["status"] == "success")
                {
                    if let Some(id) = outcome["goal_id"].as_str() {
                        keys.insert(format!("goal:{id}"));
                    }
                }
            }
        }
        Ok(keys.into_iter().collect())
    }

    pub(crate) async fn game_operation_tx(
        &self,
        tx: &mut Transaction<'_, Any>,
        company: &Company,
        existing: &[Record],
        operation: &str,
        request: &OperationRequest,
        actor: &str,
    ) -> Result<Value> {
        if operation == "simulation.draw_event" {
            if company.mode != "simulation" {
                return Err(err("simulation_required", "只有模拟公司可以抽取经营事件"));
            }
            if request.input.as_object().is_none_or(|o| !o.is_empty())
                || request.expected_revision.is_some()
            {
                return Err(err("invalid_input", "抽取事件无需输入字段或公司版本"));
            }
            let day = game_day(company)?;
            if existing
                .iter()
                .any(|r| r.kind == "alert" && r.data["draw"]["draw_date"] == day)
            {
                return Err(err(
                    "event_cooldown",
                    "本经营日已经发现新事件，推进日期后再探索",
                ));
            }
            let seed = hash(
                format!(
                    "company-deck-v1\0{}\0{}\0{}",
                    company.id, day, request.request_id
                )
                .as_bytes(),
            );
            let mut data = game::draw_card(&seed, &day, company.revision, Locale::ZhCn)?;
            let id = format!(
                "event-{}",
                &hash(format!("{}:{day}", company.id).as_bytes())[..32]
            );
            data["id"] = json!(id);
            game::validate_game_record("alert", &data, company, existing)?;
            let record = self
                .save_record(
                    tx,
                    &company.id,
                    "alert",
                    &id,
                    data,
                    None,
                    company.revision + 1,
                )
                .await?;
            return Ok(json!({"record":record}));
        }
        let id = text(&request.input, "id")?;
        identifier(id)?;
        let kind = if operation.starts_with("quest.") {
            "quest"
        } else {
            "alert"
        };
        let old = self.record_tx(tx, &company.id, kind, id).await?;
        let strict = |fields: &[&str]| -> Result<()> {
            if request
                .input
                .as_object()
                .is_none_or(|o| o.keys().any(|k| !fields.contains(&k.as_str())))
            {
                Err(err("invalid_input", "游戏操作包含未定义的字段"))
            } else {
                Ok(())
            }
        };
        if operation == "alert.ack" && id.starts_with("system-") {
            strict(&["id", "fingerprint"])?;
            let metrics = self.metrics_tx(tx, company, None).await?;
            let state = self
                .game_state_tx(tx, company, &metrics, Locale::ZhCn)
                .await?;
            let alert = state["alerts"]
                .as_array()
                .unwrap()
                .iter()
                .find(|a| a["id"] == id && a["derived"] == true)
                .ok_or_else(|| err("not_found", "当前系统提醒不存在"))?;
            if alert["fingerprint"] != request.input["fingerprint"] {
                return Err(err("revision_conflict", "提醒依据已变化，请重新读取"));
            }
            let data = json!({"id":id,"ack_only":true,"fingerprint":alert["fingerprint"],"acknowledgement":{"actor":actor,"acknowledged_at":now()}});
            game::validate_game_record(kind, &data, company, existing)?;
            let record = self
                .save_record(
                    tx,
                    &company.id,
                    kind,
                    id,
                    data,
                    request.expected_revision,
                    company.revision + 1,
                )
                .await?;
            return Ok(json!({"record":record}));
        }
        let mut data = if operation.ends_with(".put") {
            if request.input.get("completion").is_some()
                || request.input.get("failure").is_some()
                || request.input.get("accepted_at").is_some()
                || request.input.get("accepted_by").is_some()
                || request.input.get("acknowledgement").is_some()
                || request.input.get("resolution").is_some()
                || request.input.get("draw").is_some()
                || request.input.get("fork_provenance").is_some()
                || request.input.get("ack_only").is_some()
            {
                return Err(err("invalid_input", "结果和系统字段需通过专用游戏操作维护"));
            }
            let mut data = request.input.clone();
            if let Some(origin) = old.as_ref().and_then(|r| r.data.get("fork_provenance")) {
                data["fork_provenance"] = origin.clone();
            }
            if kind == "quest" {
                if old.as_ref().is_some_and(|r| {
                    !matches!(r.data["status"].as_str(), Some("open" | "accepted"))
                }) {
                    return Err(err(
                        "invalid_transition",
                        "已结束任务不能改写，另建后续任务",
                    ));
                }
                let status = old
                    .as_ref()
                    .map(|r| r.data["status"].clone())
                    .unwrap_or(json!("open"));
                if data.get("status").is_some_and(|s| *s != status) {
                    return Err(err("invalid_input", "任务状态使用专用操作更新"));
                }
                data["status"] = status;
                if let Some(old) = &old {
                    for field in ["accepted_at", "accepted_by"] {
                        if let Some(value) = old.data.get(field) {
                            data[field] = value.clone();
                        }
                    }
                }
            } else {
                if old
                    .as_ref()
                    .is_some_and(|r| r.data.get("draw").is_some() || r.data["ack_only"] == true)
                {
                    return Err(err("immutable_record", "系统事件的抽取依据不可改写"));
                }
                if data.get("status").is_none() {
                    data["status"] = json!("active");
                }
                if !matches!(data["status"].as_str(), Some("active" | "upcoming")) {
                    return Err(err("invalid_input", "提醒结案使用专用操作"));
                }
            }
            data
        } else {
            let old = old
                .as_ref()
                .ok_or_else(|| err("not_found", "任务或提醒不存在"))?;
            if request.expected_revision != Some(old.revision) {
                return Err(err("revision_conflict", "任务或提醒版本已变化，请重新读取"));
            }
            let mut data = old.data.clone();
            match operation {
                "quest.accept" => {
                    strict(&["id"])?;
                    if data["status"] != "open" {
                        return Err(err("invalid_transition", "只能接受尚未开始的任务"));
                    }
                    self.game_require_dependencies_tx(tx, company, &data)
                        .await?;
                    data["status"] = json!("accepted");
                    data["accepted_at"] = json!(now());
                    data["accepted_by"] = json!(actor);
                }
                "quest.complete" => {
                    strict(&["id", "summary", "result_refs"])?;
                    if data["status"] != "accepted" {
                        return Err(err("invalid_transition", "只能完成已接受的任务"));
                    }
                    self.game_validate_refs_tx(tx, &company.id, &request.input["result_refs"])
                        .await?;
                    let metrics = self.metrics_tx(tx, company, None).await?;
                    let day = game_day(company)?;
                    for reference in request.input["result_refs"].as_array().unwrap() {
                        if !self
                            .game_result_valid_tx(tx, company, reference, &metrics, &day)
                            .await?
                        {
                            return Err(err(
                                "result_not_achieved",
                                "任务结果尚未成立，不能宣告完成",
                            ));
                        }
                    }
                    self.game_require_dependencies_tx(tx, company, &data)
                        .await?;
                    data["status"] = json!("completed");
                    data["completion"] = json!({"summary":text(&request.input,"summary")?,"result_refs":request.input["result_refs"],"completed_at":now(),"actor":actor});
                }
                "quest.set_status" => {
                    strict(&["id", "status", "reason"])?;
                    if !matches!(data["status"].as_str(), Some("open" | "accepted"))
                        || !matches!(
                            request.input["status"].as_str(),
                            Some("failed" | "cancelled")
                        )
                    {
                        return Err(err(
                            "invalid_transition",
                            "任务只能从未结束状态转为失败或取消",
                        ));
                    }
                    data["status"] = request.input["status"].clone();
                    data["failure"] = json!({"reason":text(&request.input,"reason")?,"actor":actor,"recorded_at":now()});
                }
                "alert.ack" => {
                    strict(&["id"])?;
                    data["acknowledgement"] = json!({"actor":actor,"acknowledged_at":now()});
                }
                "alert.set_status" => {
                    strict(&["id", "status", "summary", "source_refs"])?;
                    if !matches!(data["status"].as_str(), Some("active" | "upcoming"))
                        || !matches!(
                            request.input["status"].as_str(),
                            Some("resolved" | "dismissed")
                        )
                    {
                        return Err(err("invalid_transition", "只能将当前提醒结案或收起"));
                    }
                    if let Some(refs) = request.input.get("source_refs") {
                        self.game_validate_refs_tx(tx, &company.id, refs).await?;
                    }
                    data["status"] = request.input["status"].clone();
                    data["resolution"] = json!({"summary":text(&request.input,"summary")?,"source_refs":request.input.get("source_refs").cloned().unwrap_or(json!([])),"actor":actor,"resolved_at":now()});
                    if data["basis"] == "observed"
                        && data["status"] == "resolved"
                        && data["resolution"]["source_refs"]
                            .as_array()
                            .unwrap()
                            .is_empty()
                    {
                        return Err(err("invalid_input", "实际风险解除需要新记录依据"));
                    }
                }
                _ => return Err(err("unknown_operation", "操作不存在")),
            }
            data
        };
        game::validate_game_record(kind, &data, company, existing)?;
        if let Some(refs) = data.get("source_refs") {
            self.game_validate_refs_tx(tx, &company.id, refs).await?;
        }
        if kind == "quest" {
            let mut combined = existing.to_vec();
            combined.retain(|r| r.kind != "quest" || r.id != id);
            combined.push(Record {
                id: id.into(),
                company_id: company.id.clone(),
                kind: kind.into(),
                revision: 1,
                company_revision: company.revision + 1,
                data: data.clone(),
                created_at: String::new(),
                updated_at: String::new(),
            });
            game::validate_quest_cycles(&combined)?;
        }
        let record = self
            .save_record(
                tx,
                &company.id,
                kind,
                id,
                std::mem::take(&mut data),
                request.expected_revision,
                company.revision + 1,
            )
            .await?;
        Ok(json!({"record":record}))
    }

    pub async fn game_state_localized(&self, company_id: &str, locale: Locale) -> Result<Value> {
        identifier(company_id)?;
        let mut tx = self.pool.begin().await?;
        if self.pg {
            query!("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
                .execute(&mut *tx)
                .await?;
        }
        let company = self.company_tx(&mut tx, company_id).await?;
        let metrics = self.metrics_tx(&mut tx, &company, None).await?;
        let result = self
            .game_state_tx(&mut tx, &company, &metrics, locale)
            .await?;
        tx.commit().await?;
        Ok(result)
    }
    pub(crate) async fn game_state_tx(
        &self,
        tx: &mut Transaction<'_, Any>,
        company: &Company,
        metrics: &Value,
        locale: Locale,
    ) -> Result<Value> {
        let day = metrics["as_of"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or(game_day(company)?);
        let records = self.noncash_tx(tx, &company.id).await?;
        let mut candidates = Vec::new();
        let mut quests = Vec::new();
        let mut alerts = Vec::new();
        let mut checks = Vec::new();
        let mut valid_quests: BTreeMap<String, bool> = BTreeMap::new();
        for record in records.iter().filter(|r| r.kind == "quest") {
            let mut valid = record.data["status"] == "completed";
            if valid {
                for reference in record.data["completion"]["result_refs"]
                    .as_array()
                    .into_iter()
                    .flatten()
                {
                    if !self
                        .game_result_valid_tx(tx, company, reference, metrics, &day)
                        .await?
                    {
                        valid = false;
                        break;
                    }
                }
            }
            valid_quests.insert(record.id.clone(), valid);
        }
        for _ in 0..valid_quests.len() {
            let mut changed = false;
            for record in records.iter().filter(|r| r.kind == "quest") {
                if valid_quests.get(&record.id) == Some(&true)
                    && record.data["depends_on"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .any(|id| valid_quests.get(id) != Some(&true))
                {
                    valid_quests.insert(record.id.clone(), false);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        for record in &records {
            if record.kind == "goal" {
                let progress = metrics["goal_progress"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|g| g["id"] == record.id);
                let status = progress
                    .and_then(|g| g["status"].as_str())
                    .unwrap_or("planned");
                let mapped = match status {
                    "achieved" => "completed",
                    "active" | "blocked" => "accepted",
                    "archived" => "cancelled",
                    _ => "open",
                };
                let overdue = matches!(status, "active" | "blocked")
                    && record.data["deadline"]
                        .as_str()
                        .is_some_and(|d| d < day.as_str());
                quests.push(json!({"id":record.id,"kind":"goal","company_id":company.id,"revision":record.revision,"company_revision":record.company_revision,"created_at":record.created_at,"updated_at":record.updated_at,"derived":true,"read_only":true,"actions":[],"blocked":status=="blocked","overdue":overdue,"progress":progress.and_then(|p|p.get("progress")),"current":progress.and_then(|p|p.get("current")),"target":progress.and_then(|p|p.get("target")),"completion_valid":status=="achieved","data":{"title":record.data["title"],"description":record.data["outcome"],"outcome":record.data["outcome"],"category":"main","status":mapped,"deadline":record.data.get("deadline"),"source_refs":[reference(record)]}}));
                if status == "achieved" {
                    candidates.push(json!({"id":format!("goal:{}",record.id),"xp":100,"rule":"goal","title":record.data["title"],"source_refs":[reference(record)],"evidence_keys":[format!("goal:{}",record.id)],"valid":true}));
                }
            } else if record.kind == "quest" {
                let mut item = serde_json::to_value(record)?;
                item["derived"] = json!(false);
                item["read_only"] = json!(false);
                let blocked = record.data["depends_on"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .any(|id| valid_quests.get(id) != Some(&true));
                item["blocked"] = json!(blocked);
                item["overdue"] = json!(
                    record.data["status"] == "accepted"
                        && record.data["deadline"]
                            .as_str()
                            .is_some_and(|d| d < day.as_str())
                );
                item["actions"] = if record.data["status"] == "open" && !blocked {
                    json!([command(
                        "quest.accept",
                        json!({"id":record.id}),
                        Some(record.revision)
                    )])
                } else {
                    json!([])
                };
                let valid = valid_quests.get(&record.id) == Some(&true);
                item["completion_valid"] = json!(valid);
                if valid {
                    let refs = record.data["completion"]["result_refs"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default();
                    candidates.push(json!({"id":format!("quest:{}",record.id),"xp":game::quest_reward(record.data["category"].as_str().unwrap_or("")),"rule":"quest","title":record.data["title"],"source_refs":std::iter::once(reference(record)).chain(refs.iter().cloned()).collect::<Vec<_>>(),"evidence_keys":self.game_evidence_keys_tx(tx,company,&refs).await?,"valid":true}));
                }
                quests.push(item);
            } else if record.kind == "decision" && record.data["status"] == "resolved" {
                let mut refs = vec![reference(record)];
                refs.extend(
                    record.data["resolution"]["result_refs"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .cloned(),
                );
                candidates.push(json!({"id":format!("decision:{}",record.id),"xp":25,"rule":"decision","title":record.data["title"],"source_refs":refs,"evidence_keys":self.game_evidence_keys_tx(tx,company,&[reference(record)]).await?,"valid":true}));
            } else if record.kind == "report"
                && record.data["period_end"]
                    .as_str()
                    .is_some_and(|d| d <= day.as_str())
                && record.data["as_of"]
                    .as_str()
                    .is_some_and(|d| d <= day.as_str())
            {
                candidates.push(json!({"id":format!("report:{}",record.id),"xp":if record.data["type"]=="monthly"{20}else{10},"rule":"report","title":record.data["title"],"source_refs":[reference(record)],"evidence_keys":self.game_evidence_keys_tx(tx,company,&[reference(record)]).await?,"valid":true}));
            } else if record.kind == "alert" && record.data["ack_only"] != true {
                let mut item = serde_json::to_value(record)?;
                if let Some(draw) = record.data.get("draw") {
                    let localized = game::draw_card(
                        text(draw, "seed_sha256")?,
                        text(draw, "draw_date")?,
                        integer(draw, "company_revision")?,
                        locale,
                    )?;
                    for field in ["title", "body", "assumptions"] {
                        item["data"][field] = localized[field].clone();
                    }
                }
                let unread = record.data.get("acknowledgement").is_none();
                item["derived"] = json!(false);
                item["read_only"] = json!(false);
                item["unread"] = json!(unread);
                item["actions"] = if unread {
                    json!([command(
                        "alert.ack",
                        json!({"id":record.id}),
                        Some(record.revision)
                    )])
                } else {
                    json!([])
                };
                item["related_entities"]=json!(record.data["entity_refs"].as_array().into_iter().flatten().filter_map(|entity|records.iter().find(|r|r.kind==entity["kind"]&&r.id==entity["id"]).map(|r|json!({"kind":r.kind,"id":r.id,"revision":r.revision,"label":r.data.get("name").or_else(||r.data.get("title")).cloned().unwrap_or(Value::Null)}))).collect::<Vec<_>>());
                item["applicable"] = json!(!matches!(
                    record.data["status"].as_str(),
                    Some("resolved" | "dismissed")
                ));
                alerts.push(item);
            }
        }
        let mut risk_specs: Vec<(String, Value)> = Vec::new();
        match metrics["cash_balance_minor"].as_i64() {
            Some(cash) if cash <= 0 => {
                checks.push(json!({"id":"cash","status":"triggered"}));
                risk_specs.push(("system-cash".into(),json!({"title":localized("现金警报","Cash Alert",locale),"body":localized("可用现金已到零线，先核对付款安排与资金来源。","Available cash has reached zero. Review payment commitments and funding sources.",locale),"type":"risk","severity":"critical","status":"active","basis":"observed","metric_ref":{"metric":"cash_balance_minor","value":cash,"as_of":day,"data_revision":company.revision},"source_refs":records.iter().filter(|r|r.kind=="account").map(reference).collect::<Vec<_>>()})));
            }
            Some(_) => checks.push(json!({"id":"cash","status":"clear"})),
            None => {
                checks.push(json!({"id":"cash","status":"unknown","reason":"missing_accounts"}))
            }
        }
        match metrics["runway_months"].as_f64() {
            Some(months) if months<3.0=>{checks.push(json!({"id":"runway","status":"triggered"}));risk_specs.push(("system-runway".into(),json!({"title":localized("现金缓冲变薄","Cash Buffer Is Thinning",locale),"body":localized("按最近三个完整月的净消耗，现金可维持不足三个月。评估节流或收入安排。","At the average burn of the last three complete months, cash covers less than three months. Review spending and income plans.",locale),"type":"risk","severity":"warning","status":"active","basis":"observed","metric_ref":{"metric":"runway_months","value":months,"as_of":day,"data_revision":company.revision},"source_refs":[]})));},
            Some(_)=>checks.push(json!({"id":"runway","status":"clear"})),None=>checks.push(json!({"id":"runway","status":if metrics["runway_status"]=="not_depleting"{"clear"}else{"unknown"},"reason":metrics["runway_status"]}))
        }
        let mut customer_mrr: BTreeMap<String, i64> = BTreeMap::new();
        let mut subscription_refs = Vec::new();
        if company.subscription_tracking {
            for record in records.iter().filter(|r| {
                r.kind == "subscription"
                    && r.data["status"] == "active"
                    && r.data["starts_on"]
                        .as_str()
                        .is_some_and(|d| d <= day.as_str())
                    && r.data["ends_on"].as_str().is_none_or(|d| d > day.as_str())
            }) {
                let amount = (integer(&record.data, "amount_minor")?
                    - record.data["tax_minor"].as_i64().unwrap_or(0)
                    - record.data["recurring_discount_minor"]
                        .as_i64()
                        .unwrap_or(0))
                    / integer(&record.data, "interval_months")?;
                if amount > 0 {
                    *customer_mrr
                        .entry(text(&record.data, "customer_id")?.into())
                        .or_default() += amount;
                    subscription_refs.push(reference(record));
                }
            }
        }
        let total: i64 = customer_mrr.values().sum();
        if let Some((customer, amount)) = customer_mrr
            .iter()
            .max_by_key(|(_, v)| *v)
            .filter(|_| total > 0)
        {
            let share = *amount as f64 / total as f64;
            checks.push(json!({"id":"concentration","status":if share>=0.8{"triggered"}else{"clear"},"share":share}));
            if share >= 0.8 {
                let name = records
                    .iter()
                    .find(|r| r.kind == "customer" && r.id == *customer)
                    .and_then(|r| r.data["name"].as_str())
                    .unwrap_or(customer);
                risk_specs.push(("system-concentration".into(),json!({"title":localized("收入系在一位客户身上","Revenue Depends on One Customer",locale),"body":if locale==Locale::ZhCn{format!("{name} 占当前有效订阅规模的 {:.0}%。评估客户流失时的缓冲与替代收入。",share*100.0)}else{format!("{name} accounts for {:.0}% of current active subscription value. Review the buffer and replacement revenue if this customer leaves.",share*100.0)},"type":"risk","severity":"warning","status":"active","basis":"observed","metric_ref":{"metric":"subscription_concentration","value":share,"customer_id":customer,"as_of":day,"data_revision":company.revision},"source_refs":subscription_refs})));
            }
        } else {
            checks.push(json!({"id":"concentration","status":"unknown","reason":if company.subscription_tracking{"no_active_subscription_value"}else{"subscription_tracking_unavailable"}}));
        }
        for task in quests.iter().filter(|q| q["overdue"] == true) {
            let id = format!(
                "system-due-{}",
                &hash(format!("{}:{}", task["kind"], task["id"]).as_bytes())[..24]
            );
            risk_specs.push((id,json!({"title":localized("承诺已到期","A Commitment Is Due",locale),"body":task["data"]["title"],"type":"risk","severity":"warning","status":"active","basis":"observed","deadline":task["data"]["deadline"],"source_refs":[{"kind":task["kind"],"id":task["id"],"revision":task["revision"]}]})));
        }
        for (id, data) in risk_specs {
            let mut material = data.clone();
            if let Some(obj) = material
                .get_mut("metric_ref")
                .and_then(Value::as_object_mut)
            {
                obj.remove("as_of");
                obj.remove("data_revision");
            }
            // A language switch must not turn an acknowledged risk unread.
            material.as_object_mut().unwrap().remove("title");
            material.as_object_mut().unwrap().remove("body");
            let fingerprint = game::fingerprint(&material);
            let ack = records
                .iter()
                .find(|r| r.kind == "alert" && r.id == id && r.data["ack_only"] == true);
            let unread = ack.is_none_or(|a| a.data["fingerprint"] != fingerprint);
            alerts.push(json!({"id":id,"kind":"alert","company_id":company.id,"revision":ack.map(|r|r.revision),"company_revision":company.revision,"created_at":ack.map(|r|r.created_at.clone()).unwrap_or_default(),"updated_at":ack.map(|r|r.updated_at.clone()).unwrap_or_default(),"derived":true,"read_only":true,"data":data,"fingerprint":fingerprint,"unread":unread,"applicable":true,"actions":if unread{vec![command("alert.ack",json!({"id":id,"fingerprint":fingerprint}),ack.map(|r|r.revision))]}else{vec![]}}));
        }
        let last_draw = records
            .iter()
            .filter(|r| r.kind == "alert" && r.data.get("draw").is_some())
            .max_by_key(|r| r.data["draw"]["draw_date"].as_str().unwrap_or(""));
        let drawn_today = last_draw.is_some_and(|r| r.data["draw"]["draw_date"] == day);
        let available = company.mode == "simulation" && !drawn_today;
        quests.sort_by_key(|q| {
            (
                q["data"]["status"].as_str().unwrap_or("").to_owned(),
                q["id"].as_str().unwrap_or("").to_owned(),
            )
        });
        alerts.sort_by_key(|a| {
            (
                std::cmp::Reverse(a["unread"] == true),
                std::cmp::Reverse(a["data"]["severity"] == "critical"),
                a["id"].as_str().unwrap_or("").to_owned(),
            )
        });
        Ok(
            json!({"schema_version":1,"company_id":company.id,"company_revision":company.revision,"mode":company.mode,"as_of":day,"progression":game::progression(candidates,locale),"quests":quests,"alerts":alerts,"checks":checks,"unread_count":alerts.iter().filter(|a|a["unread"]==true&&a["applicable"]==true).count(),"draw":{"available":available,"deck_version":game::DECK_VERSION,"cooldown_until":if drawn_today{date(&day)?.succ_opt().map(|d|d.to_string())}else{None},"reason":if company.mode!="simulation"{Some("real_mode")}else if drawn_today{Some("daily_limit")}else{None},"last_event":last_draw.map(|r|json!({"kind":"alert","id":r.id,"revision":r.revision,"draw_date":r.data["draw"]["draw_date"],"card_id":r.data["draw"]["card_id"]})),"action":if available{Some(command("simulation.draw_event",json!({}),None))}else{None}}}),
        )
    }
}

#[cfg(test)]
mod proof_tests {
    use super::*;
    async fn apply(
        app: &App,
        company: Option<&str>,
        operation: &str,
        key: &str,
        input: Value,
        revision: Option<i64>,
    ) -> Value {
        app.execute(
            company,
            operation,
            OperationRequest {
                request_id: key.into(),
                expected_revision: revision,
                input,
            },
            "proof-test",
        )
        .await
        .unwrap()
    }
    #[tokio::test]
    async fn nested_branches_use_current_goal_state_and_ancestor_reversal_evidence() {
        let directory = tempfile::tempdir().unwrap();
        let app = App::open(AppConfig {
            data_dir: directory.path().into(),
            database_url: None,
        })
        .await
        .unwrap();
        apply(&app,None,"company.create","create",json!({"id":"real","name":"Real","currency":"CNY","timezone":"Asia/Shanghai","opening_date":"2026-01-01","mode":"real"}),None).await;
        apply(
            &app,
            Some("real"),
            "account.put",
            "account",
            json!({"id":"cash","name":"Cash","opening_balance_minor":0}),
            None,
        )
        .await;
        apply(&app,Some("real"),"transaction.record","receipt",json!({"id":"receipt","account_id":"cash","amount_minor":1000,"direction":"inflow","category":"operating","occurred_on":"2026-01-01","source":{"kind":"test","reference":"explicit QA receipt"}}),None).await;
        apply(&app,Some("real"),"goal.put","goal",json!({"id":"result","title":"Review","outcome":"A sourced review","metric":"manual","status":"achieved","evidence":"Original result"}),None).await;
        for (id, reference) in [
            ("review", json!({"kind":"goal","id":"result","revision":1})),
            (
                "payment",
                json!({"kind":"transaction","id":"receipt","revision":1}),
            ),
        ] {
            apply(&app,Some("real"),"quest.put",&format!("put-{id}"),json!({"id":id,"title":id,"description":"Verify the result","outcome":"A valid result","category":"side","source_refs":[]}),None).await;
            apply(
                &app,
                Some("real"),
                "quest.accept",
                &format!("accept-{id}"),
                json!({"id":id}),
                Some(1),
            )
            .await;
            apply(
                &app,
                Some("real"),
                "quest.complete",
                &format!("complete-{id}"),
                json!({"id":id,"summary":"Result verified","result_refs":[reference]}),
                Some(2),
            )
            .await;
        }
        assert_eq!(
            app.game_state_localized("real", Locale::En).await.unwrap()["progression"]["xp"],
            125
        );
        apply(&app,Some("real"),"transaction.reverse","reverse",json!({"transaction_id":"receipt","reason":"Correct the QA receipt","occurred_on":"2026-01-02"}),None).await;
        let first = apply(
            &app,
            Some("real"),
            "simulation.fork",
            "fork-one",
            json!({"name":"First route"}),
            None,
        )
        .await;
        let first_id = first["company"]["id"].as_str().unwrap();
        assert_eq!(
            app.game_state_localized(first_id, Locale::En)
                .await
                .unwrap()["progression"]["xp"],
            100
        );
        apply(&app,Some(first_id),"goal.put","correct",json!({"id":"result","title":"Review","outcome":"A sourced review","metric":"manual","status":"active"}),Some(1)).await;
        assert_eq!(
            app.game_state_localized(first_id, Locale::En)
                .await
                .unwrap()["progression"]["xp"],
            0
        );
        let second = apply(
            &app,
            Some(first_id),
            "simulation.fork",
            "fork-two",
            json!({"name":"Second route"}),
            None,
        )
        .await;
        let second_id = second["company"]["id"].as_str().unwrap();
        let state = app
            .game_state_localized(second_id, Locale::En)
            .await
            .unwrap();
        assert_eq!(state["progression"]["xp"], 0);
        assert!(
            state["quests"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|q| q["kind"] == "quest")
                .all(|q| q["completion_valid"] == false)
        );
        let path = directory.path().join("second-only.json");
        app.backup(Some(second_id), Some(&path)).await.unwrap();
        let restored_directory = tempfile::tempdir().unwrap();
        let restored = App::open(AppConfig {
            data_dir: restored_directory.path().into(),
            database_url: None,
        })
        .await
        .unwrap();
        restored.restore(&path).await.unwrap();
        assert_eq!(
            restored
                .game_state_localized(second_id, Locale::En)
                .await
                .unwrap()["progression"]["xp"],
            0
        );
    }
}
