use super::*;
use sha2::{Digest, Sha256};

pub const LEVEL_THRESHOLDS: [i64; 8] = [0, 100, 250, 500, 900, 1500, 2300, 3300];
pub const DECK_VERSION: i64 = 1;

fn allowed_fields(v: &Value, allowed: &[&str]) -> Result<()> {
    if !v.is_object()
        || v.as_object()
            .unwrap()
            .keys()
            .any(|key| !allowed.contains(&key.as_str()))
    {
        return fail("invalid_input", "游戏记录包含未定义的字段");
    }
    Ok(())
}
fn timestamp(v: &Value, key: &str) -> Result<()> {
    chrono::DateTime::parse_from_rfc3339(text(v, key)?)
        .map_err(|_| DomainError::new("invalid_input", "游戏记录系统字段无效"))?;
    Ok(())
}

pub fn validate_exact_refs(refs: &Value) -> Result<()> {
    let refs = refs
        .as_array()
        .ok_or_else(|| DomainError::new("invalid_input", "来源需要精确版本引用数组"))?;
    if refs.len() > 100 {
        return fail("invalid_input", "来源最多 100 项");
    }
    let mut unique = BTreeSet::new();
    for reference in refs {
        allowed_fields(reference, &["kind", "id", "revision"])?;
        identifier(text(reference, "kind")?)?;
        identifier(text(reference, "id")?)?;
        if integer(reference, "revision")? < 1 {
            return fail("invalid_input", "来源版本必须为正整数");
        }
        if !unique.insert((
            text(reference, "kind")?,
            text(reference, "id")?,
            integer(reference, "revision")?,
        )) {
            return fail("invalid_input", "来源引用不能重复");
        }
    }
    Ok(())
}

pub fn validate_game_record(
    kind: &str,
    v: &Value,
    company: &Company,
    existing: &[Record],
) -> Result<()> {
    identifier(text(v, "id")?)?;
    if let Some(refs) = v.get("source_refs") {
        validate_exact_refs(refs)?;
    }
    if kind == "quest" {
        allowed_fields(
            v,
            &[
                "id",
                "title",
                "description",
                "outcome",
                "category",
                "status",
                "deadline",
                "depends_on",
                "source_refs",
                "completion",
                "failure",
                "accepted_at",
                "accepted_by",
                "fork_provenance",
            ],
        )?;
        text(v, "title")?;
        text(v, "description")?;
        text(v, "outcome")?;
        one_of(v, "category", &["main", "side", "daily"])?;
        one_of(
            v,
            "status",
            &["open", "accepted", "completed", "failed", "cancelled"],
        )?;
        if let Some(deadline) = v.get("deadline") {
            date(
                deadline
                    .as_str()
                    .ok_or_else(|| DomainError::new("invalid_input", "任务期限必须为日期"))?,
            )?;
        }
        if let Some(deps) = v.get("depends_on") {
            let deps = deps
                .as_array()
                .ok_or_else(|| DomainError::new("invalid_input", "任务依赖必须为数组"))?;
            let mut unique = BTreeSet::new();
            if deps.len() > 100 {
                return fail("invalid_input", "任务依赖最多 100 项");
            }
            for dep in deps {
                let id = dep
                    .as_str()
                    .ok_or_else(|| DomainError::new("invalid_input", "任务依赖必须为标识"))?;
                identifier(id)?;
                if id == text(v, "id")? || !unique.insert(id) {
                    return fail("invalid_input", "任务依赖不能重复或指向自身");
                }
                if !existing
                    .iter()
                    .any(|r| r.company_id == company.id && r.kind == "quest" && r.id == id)
                {
                    return fail("reference_not_found", "依赖任务在本公司不存在");
                }
            }
        }
        if v["status"] == "completed" {
            let completion = v
                .get("completion")
                .ok_or_else(|| DomainError::new("invalid_input", "完成任务需要结果依据"))?;
            allowed_fields(
                completion,
                &["summary", "result_refs", "completed_at", "actor"],
            )?;
            text(completion, "summary")?;
            text(completion, "actor")?;
            timestamp(completion, "completed_at")?;
            validate_exact_refs(&completion["result_refs"])?;
            if completion["result_refs"].as_array().unwrap().is_empty() {
                return fail("invalid_input", "完成任务需要至少一项结果依据");
            }
            for reference in completion["result_refs"].as_array().unwrap() {
                if ![
                    "goal",
                    "decision",
                    "report",
                    "simulation_turn",
                    "business",
                    "transaction",
                    "file",
                ]
                .contains(&text(reference, "kind")?)
                    || (company.mode == "real" && reference["kind"] == "simulation_turn")
                {
                    return fail("invalid_result_reference", "任务需要可核对的经营结果");
                }
            }
        } else if v.get("completion").is_some() {
            return fail("invalid_input", "未完成任务不能携带完成结果");
        }
        if v["status"] == "failed" {
            text(&v["failure"], "reason")?;
        }
        if let Some(failure) = v.get("failure") {
            allowed_fields(failure, &["reason", "actor", "recorded_at"])?;
            if !matches!(v["status"].as_str(), Some("failed" | "cancelled")) {
                return fail("invalid_input", "游戏记录系统字段无效");
            }
            text(failure, "reason")?;
            text(failure, "actor")?;
            timestamp(failure, "recorded_at")?;
        }
        if let Some(accepted_at) = v.get("accepted_at") {
            if accepted_at.is_null() {
                return fail("invalid_input", "游戏记录系统字段无效");
            }
            timestamp(v, "accepted_at")?;
            text(v, "accepted_by")?;
        }
    } else if kind == "alert" {
        if v["ack_only"] == true {
            allowed_fields(v, &["id", "ack_only", "fingerprint", "acknowledgement"])?;
            if !text(v, "id")?.starts_with("system-")
                || text(v, "fingerprint")?.len() != 64
                || !text(v, "fingerprint")?
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            {
                return fail("invalid_input", "系统提醒确认无效");
            }
            text(&v["acknowledgement"], "actor")?;
            allowed_fields(&v["acknowledgement"], &["actor", "acknowledged_at"])?;
            timestamp(&v["acknowledgement"], "acknowledged_at")?;
            return Ok(());
        }
        allowed_fields(
            v,
            &[
                "id",
                "title",
                "body",
                "type",
                "severity",
                "status",
                "basis",
                "assumptions",
                "source_refs",
                "window_start",
                "window_end",
                "entity_refs",
                "quest_id",
                "acknowledgement",
                "resolution",
                "draw",
                "fork_provenance",
            ],
        )?;
        if text(v, "id")?.starts_with("system-") {
            return fail("invalid_input", "system- 标识仅供系统提醒使用");
        }
        text(v, "title")?;
        text(v, "body")?;
        one_of(v, "type", &["opportunity", "risk", "event"])?;
        one_of(v, "severity", &["info", "warning", "critical"])?;
        one_of(
            v,
            "status",
            &["upcoming", "active", "resolved", "dismissed"],
        )?;
        one_of(v, "basis", &["observed", "forecast", "simulation"])?;
        if company.mode == "real" && v["basis"] == "simulation" {
            return fail("simulation_required", "虚构事件只用于模拟公司");
        }
        if v["basis"] != "observed" {
            let assumptions = v["assumptions"]
                .as_array()
                .ok_or_else(|| DomainError::new("invalid_input", "预测或模拟提醒需要假设"))?;
            if assumptions.is_empty()
                || assumptions.len() > 20
                || assumptions.iter().any(|a| {
                    a.as_str()
                        .is_none_or(|s| s.trim().is_empty() || s.len() > 4000)
                })
            {
                return fail("invalid_input", "预测或模拟提醒需要 1 至 20 项假设");
            }
        } else if v["source_refs"].as_array().is_none_or(Vec::is_empty) {
            return fail("invalid_input", "已观察提醒需要记录来源");
        }
        let start = v
            .get("window_start")
            .map(|x| {
                x.as_str()
                    .ok_or_else(|| DomainError::new("invalid_input", "事件窗口必须为日期"))
                    .and_then(date)
            })
            .transpose()?;
        let end = v
            .get("window_end")
            .map(|x| {
                x.as_str()
                    .ok_or_else(|| DomainError::new("invalid_input", "事件窗口必须为日期"))
                    .and_then(date)
            })
            .transpose()?;
        if start.zip(end).is_some_and(|(s, e)| s > e) {
            return fail("invalid_input", "事件窗口不能倒置");
        }
        if let Some(quest) = v.get("quest_id") {
            let id = quest
                .as_str()
                .ok_or_else(|| DomainError::new("invalid_input", "任务关联必须为标识"))?;
            if !existing
                .iter()
                .any(|r| r.kind == "quest" && r.id == id && r.company_id == company.id)
            {
                return fail("reference_not_found", "提醒关联的任务不存在");
            }
        }
        if let Some(refs) = v.get("entity_refs") {
            let refs = refs
                .as_array()
                .ok_or_else(|| DomainError::new("invalid_input", "实体关联必须为数组"))?;
            let mut unique = BTreeSet::new();
            if refs.len() > 100 {
                return fail("invalid_input", "实体关联最多 100 项");
            }
            for reference in refs {
                allowed_fields(reference, &["kind", "id"])?;
                let kind = text(reference, "kind")?;
                let id = text(reference, "id")?;
                if !unique.insert((kind, id))
                    || !existing
                        .iter()
                        .any(|r| r.company_id == company.id && r.kind == kind && r.id == id)
                {
                    return fail("reference_not_found", "提醒关联的实体不存在或重复");
                }
            }
        }
        if let Some(draw) = v.get("draw") {
            allowed_fields(
                draw,
                &[
                    "deck_version",
                    "draw_date",
                    "seed_sha256",
                    "card_id",
                    "weight",
                    "total_weight",
                    "index",
                    "company_revision",
                ],
            )?;
            if company.mode != "simulation" || integer(draw, "deck_version")? != DECK_VERSION {
                return fail("invalid_input", "事件牌组版本无效");
            }
            date(text(draw, "draw_date")?)?;
            let seed = text(draw, "seed_sha256")?;
            if seed.len() != 64
                || !seed
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            {
                return fail("invalid_input", "事件抽取依据无效");
            }
            let generated = draw_card(
                seed,
                text(draw, "draw_date")?,
                integer(draw, "company_revision")?,
                Locale::ZhCn,
            )?;
            if generated["draw"] != *draw || v["basis"] != "simulation" {
                return fail("invalid_input", "事件抽取依据与牌组不一致");
            }
            for field in [
                "title",
                "body",
                "type",
                "severity",
                "assumptions",
                "window_start",
                "source_refs",
            ] {
                if v[field] != generated[field] {
                    return fail("invalid_input", "事件抽取依据与牌组不一致");
                }
            }
        }
        if let Some(ack) = v.get("acknowledgement") {
            allowed_fields(ack, &["actor", "acknowledged_at"])?;
            text(ack, "actor")?;
            timestamp(ack, "acknowledged_at")?;
        }
        if let Some(resolution) = v.get("resolution") {
            allowed_fields(
                resolution,
                &["summary", "source_refs", "actor", "resolved_at"],
            )?;
            if !matches!(v["status"].as_str(), Some("resolved" | "dismissed")) {
                return fail("invalid_input", "游戏记录系统字段无效");
            }
            text(resolution, "summary")?;
            text(resolution, "actor")?;
            timestamp(resolution, "resolved_at")?;
            validate_exact_refs(&resolution["source_refs"])?;
            if v["status"] == "resolved"
                && v["basis"] == "observed"
                && resolution["source_refs"].as_array().unwrap().is_empty()
            {
                return fail("invalid_input", "实际风险解除需要新记录依据");
            }
        } else if matches!(v["status"].as_str(), Some("resolved" | "dismissed")) {
            return fail("invalid_input", "游戏记录系统字段无效");
        }
    } else {
        return fail("unknown_kind", "未知游戏记录类型");
    }
    Ok(())
}

pub fn validate_quest_cycles(records: &[Record]) -> Result<()> {
    fn visit<'a>(
        id: &'a str,
        records: &'a [Record],
        visiting: &mut BTreeSet<&'a str>,
        done: &mut BTreeSet<&'a str>,
    ) -> Result<()> {
        if done.contains(id) {
            return Ok(());
        }
        if !visiting.insert(id) {
            return fail("invalid_input", "任务依赖不能形成循环");
        }
        if let Some(record) = records.iter().find(|r| r.kind == "quest" && r.id == id) {
            for dep in record.data["depends_on"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                visit(dep, records, visiting, done)?;
            }
        }
        visiting.remove(id);
        done.insert(id);
        Ok(())
    }
    let mut visiting = BTreeSet::new();
    let mut done = BTreeSet::new();
    for record in records.iter().filter(|r| r.kind == "quest") {
        visit(&record.id, records, &mut visiting, &mut done)?;
    }
    Ok(())
}

pub fn record_references(data: &Value) -> Vec<&Value> {
    [
        data.get("source_refs"),
        data.get("completion").and_then(|c| c.get("result_refs")),
        data.get("resolution").and_then(|r| r.get("source_refs")),
    ]
    .into_iter()
    .flatten()
    .flat_map(|v| v.as_array().into_iter().flatten())
    .collect()
}
/// Restore checks run against all versions, preserving the exact evidence that
/// existed before each write rather than substituting its latest revision.
pub fn validate_game_history(company: &Company, versions: &[Record]) -> Result<()> {
    let own: Vec<&Record> = versions
        .iter()
        .filter(|r| r.company_id == company.id)
        .collect();
    let mut ordered: Vec<&Record> = own
        .iter()
        .copied()
        .filter(|r| matches!(r.kind.as_str(), "quest" | "alert"))
        .collect();
    ordered.sort_by_key(|r| (&r.kind, &r.id, r.revision));
    let mut previous: BTreeMap<(&str, &str), &Record> = BTreeMap::new();
    for record in ordered {
        for reference in record_references(&record.data) {
            let source = own
                .iter()
                .copied()
                .find(|r| {
                    r.kind == reference["kind"]
                        && r.id == reference["id"]
                        && r.revision == reference["revision"].as_i64().unwrap_or(0)
                })
                .ok_or_else(|| {
                    DomainError::new("integrity_error", "游戏依据的准确版本在本公司不存在")
                })?;
            if source.company_revision > record.company_revision
                || (source.company_revision == record.company_revision
                    && record.company_revision != 1)
            {
                return fail("integrity_error", "游戏依据引用了尚未存在的结果");
            }
        }
        if record.kind == "quest" {
            if let Some(old) = previous.get(&(record.kind.as_str(), record.id.as_str())) {
                let old_status = text(&old.data, "status")?;
                let new_status = text(&record.data, "status")?;
                let allowed = match old_status {
                    "open" => matches!(new_status, "open" | "accepted" | "failed" | "cancelled"),
                    "accepted" => matches!(
                        new_status,
                        "accepted" | "completed" | "failed" | "cancelled"
                    ),
                    _ => false,
                };
                if !allowed {
                    return fail("integrity_error", "任务历史状态转换无效");
                }
            } else if record.data["status"] != "open"
                && record.data.get("fork_provenance").is_none()
            {
                return fail("integrity_error", "任务历史状态转换无效");
            }
        }
        if let Some(old) = previous.get(&(record.kind.as_str(), record.id.as_str()))
            && (old.data.get("draw") != record.data.get("draw")
                || old.data.get("ack_only") != record.data.get("ack_only"))
        {
            return fail("integrity_error", "系统事件的抽取依据不可改写");
        }
        previous.insert((record.kind.as_str(), record.id.as_str()), record);
    }
    let latest: Vec<Record> = previous.into_values().cloned().collect();
    validate_quest_cycles(&latest)?;
    Ok(())
}

pub struct ResolvedForkEvidence {
    pub source: Record,
    pub immediate_source: Record,
    pub baseline_metrics: Value,
}
fn valid_digest(digest: &str) -> Result<()> {
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return fail("integrity_error", "推演基线依据摘要或大小不符");
    }
    Ok(())
}
fn checked_fork_origin(
    metadata: &[Value],
    result: ResolvedForkEvidence,
) -> Result<ResolvedForkEvidence> {
    let origin = &result.source;
    let expected = json!({"company_id":origin.company_id,"kind":origin.kind,"id":origin.id,"revision":origin.revision,"company_revision":origin.company_revision});
    if metadata.iter().any(|entry| *entry != expected) {
        return fail("integrity_error", "推演基线依据来源不符");
    }
    Ok(result)
}

/// The caller supplies immutable object bytes. Core only verifies data and never
/// obtains host paths or performs storage access. Nested branches keep the
/// original business identity rather than turning an archived result into a
/// new document-completion reward.
pub fn resolve_fork_evidence(
    company: &Company,
    file: &Record,
    registered_files: &[Record],
    read_object: &mut impl FnMut(&str) -> Result<Vec<u8>>,
) -> Result<Option<ResolvedForkEvidence>> {
    if file.kind != "file" || file.data["fork_evidence"] != true {
        return Ok(None);
    }
    let mut current = file.clone();
    valid_digest(text(&current.data, "sha256")?)?;
    let mut bytes = read_object(text(&current.data, "sha256")?)?;
    let mut immediate_source = None;
    let mut baseline_metrics = Value::Null;
    let mut seen = BTreeSet::new();
    let mut origin_metadata = Vec::new();
    for depth in 0..128 {
        let digest = text(&current.data, "sha256")?;
        if !seen.insert(digest.to_owned()) {
            return fail("integrity_error", "推演基线依据来源不符");
        }
        if current.kind != "file"
            || current.data["fork_evidence"] != true
            || !current.id.starts_with("fork-proof-")
            || current.revision != 1
            || !text(&current.data, "relative_path")?.starts_with("extensions/provenance/")
        {
            return fail("integrity_error", "推演基线依据来源不符");
        }
        if let Some(metadata) = current.data.get("fork_evidence_origin") {
            origin_metadata.push(metadata.clone());
        }
        if hex::encode(Sha256::digest(&bytes)) != digest
            || integer(&current.data, "size_bytes")? != bytes.len() as i64
        {
            return fail("integrity_error", "推演基线依据摘要或大小不符");
        }
        let proof: Value = serde_json::from_slice(&bytes)
            .map_err(|_| DomainError::new("integrity_error", "推演基线依据来源不符"))?;
        let source: Record = serde_json::from_value(proof["source_record"].clone())
            .map_err(|_| DomainError::new("integrity_error", "推演基线依据来源不符"))?;
        if proof["format"] != "transplane-company-fork-evidence"
            || proof["version"] != 1
            || proof["origin"]["kind"] != source.kind
            || proof["origin"]["id"] != source.id
            || proof["origin"]["revision"] != source.revision
            || proof["origin"]["company_revision"] != source.company_revision
            || proof["origin"]["company_id"] != source.company_id
            || source.revision < 1
            || source.company_revision < 1
            || proof["baseline"]["company_id"] != source.company_id
            || proof["baseline"]["company_revision"]
                .as_i64()
                .is_none_or(|r| source.company_revision > r)
        {
            return fail("integrity_error", "推演基线依据来源不符");
        }
        if depth == 0 {
            if current.company_id != company.id
                || proof["origin"]["company_id"].as_str() != company.baseline_company_id.as_deref()
                || proof["baseline"]["company_revision"].as_i64() != company.baseline_revision
            {
                return fail("integrity_error", "推演基线依据来源不符");
            }
            immediate_source = Some(source.clone());
            baseline_metrics = proof["baseline"]["metrics"].clone();
        }
        if source.kind != "file" {
            return Ok(Some(checked_fork_origin(
                &origin_metadata,
                ResolvedForkEvidence {
                    source,
                    immediate_source: immediate_source.unwrap(),
                    baseline_metrics,
                },
            )?));
        }
        let raw = &proof["source_file"];
        if raw["sha256"] != source.data["sha256"] || raw["size_bytes"] != source.data["size_bytes"]
        {
            return fail("integrity_error", "推演基线依据摘要或大小不符");
        }
        let source_digest = text(&source.data, "sha256")?;
        valid_digest(source_digest)?;
        let next = if let Some(content) = raw["content_base64"].as_str() {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD
                .decode(content)
                .map_err(|_| DomainError::new("integrity_error", "推演基线依据内容缺失"))?
        } else {
            if raw["file_ref"]["kind"] != "file"
                || raw["file_ref"]["id"] != format!("fork-source-{source_digest}")
                || raw["file_ref"]["revision"] != 1
            {
                return fail("integrity_error", "推演基线依据来源不符");
            }
            if !registered_files.iter().any(|r| {
                r.company_id == company.id
                    && r.kind == "file"
                    && r.id == raw["file_ref"]["id"]
                    && r.revision == 1
                    && r.data["fork_source_content"] == true
                    && r.data["sha256"] == raw["sha256"]
                    && r.data["size_bytes"] == raw["size_bytes"]
                    && r.data["relative_path"].as_str().is_some_and(|p| {
                        p == format!("extensions/provenance/fork-source-{source_digest}.bin")
                    })
            }) {
                return fail("integrity_error", "推演基线依据来源不符");
            }
            read_object(source_digest)?
        };
        if hex::encode(Sha256::digest(&next)) != source_digest
            || integer(&source.data, "size_bytes")? != next.len() as i64
        {
            return fail("integrity_error", "推演基线依据摘要或大小不符");
        }
        if source.data["fork_evidence"] != true {
            return Ok(Some(checked_fork_origin(
                &origin_metadata,
                ResolvedForkEvidence {
                    source,
                    immediate_source: immediate_source.unwrap(),
                    baseline_metrics,
                },
            )?));
        }
        current = source;
        bytes = next;
    }
    fail("integrity_error", "推演基线依据来源不符")
}

fn fork_source(
    company: &Company,
    file: &Record,
    blobs: &BTreeMap<String, Vec<u8>>,
    registered_files: &[Record],
) -> Result<Option<ResolvedForkEvidence>> {
    resolve_fork_evidence(company, file, registered_files, &mut |digest| {
        blobs
            .get(digest)
            .cloned()
            .ok_or_else(|| DomainError::new("integrity_error", "推演基线依据内容缺失"))
    })
}
fn valid_result(record: &Record, metrics: &Value, context: &[Record], day: &str) -> bool {
    match record.kind.as_str() {
        "goal" => metrics["goal_progress"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|g| g["id"] == record.id && g["status"] == "achieved"),
        "decision" => record.data["status"] == "resolved",
        "report" => {
            !record.data["body"].as_str().unwrap_or("").trim().is_empty()
                && record.data["period_end"].as_str().is_some_and(|d| d <= day)
                && record.data["as_of"].as_str().is_some_and(|d| d <= day)
        }
        "simulation_turn" => {
            record.data["occurred_on"]
                .as_str()
                .is_some_and(|d| d <= day)
                && record.data["outcomes"]
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
                                    .any(|g| g["id"] == o["goal_id"] && g["status"] == "achieved"))
                    })
        }
        "business" => matches!(
            record.data["status"].as_str(),
            Some("released" | "completed")
        ),
        "transaction" => {
            record.data.get("reverses").is_none()
                && record.data["occurred_on"]
                    .as_str()
                    .is_some_and(|d| d <= day)
                && !context
                    .iter()
                    .any(|r| r.kind == "transaction" && r.data["reverses"] == record.id)
        }
        "file" => true,
        _ => false,
    }
}

pub fn validate_game_results(
    company: &Company,
    game_records: &[Record],
    all_versions: &[Record],
    company_versions: &[Company],
    blobs: &BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    for record in game_records.iter().filter(|r| {
        r.company_id == company.id && r.kind == "quest" && r.data["status"] == "completed"
    }) {
        if record.company_revision == 1 && record.data.get("fork_provenance").is_some() {
            let provenance = &record.data["fork_provenance"];
            let source: Record = serde_json::from_value(provenance["source_record"].clone())
                .map_err(|_| DomainError::new("integrity_error", "推演基线依据来源不符"))?;
            if source.company_id != company.baseline_company_id.clone().unwrap_or_default()
                || source.kind != "quest"
                || source.id != record.id
                || source.revision != provenance["revision"].as_i64().unwrap_or(0)
                || source.company_revision > company.baseline_revision.unwrap_or(0)
                || source.data["status"] != "completed"
            {
                return fail("integrity_error", "推演基线依据来源不符");
            }
            let mut imported = record.data.clone();
            imported.as_object_mut().unwrap().remove("fork_provenance");
            if let Some(previous) = source.data.get("fork_provenance") {
                imported["fork_provenance"] = previous.clone();
            }
            for path in [
                "/source_refs",
                "/completion/result_refs",
                "/resolution/source_refs",
            ] {
                if let Some(original) = source.data.pointer(path).and_then(Value::as_array) {
                    let mapped = record
                        .data
                        .pointer(path)
                        .and_then(Value::as_array)
                        .ok_or_else(|| {
                            DomainError::new("integrity_error", "推演基线依据来源不符")
                        })?;
                    if original.len() != mapped.len() {
                        return fail("integrity_error", "推演基线依据来源不符");
                    }
                    for (origin, reference) in original.iter().zip(mapped) {
                        let proof = all_versions
                            .iter()
                            .find(|r| {
                                r.company_id == company.id
                                    && r.kind == reference["kind"]
                                    && r.id == reference["id"]
                                    && r.revision == reference["revision"].as_i64().unwrap_or(0)
                            })
                            .ok_or_else(|| {
                                DomainError::new("integrity_error", "推演基线依据内容缺失")
                            })?;
                        let resolved = fork_source(company, proof, blobs, all_versions)?
                            .ok_or_else(|| {
                                DomainError::new("integrity_error", "推演基线依据来源不符")
                            })?;
                        let referenced = resolved.immediate_source;
                        if referenced.kind != origin["kind"]
                            || referenced.id != origin["id"]
                            || referenced.revision != origin["revision"].as_i64().unwrap_or(0)
                        {
                            return fail("integrity_error", "推演基线依据来源不符");
                        }
                    }
                    *imported.pointer_mut(path).unwrap() = json!(original);
                }
            }
            if imported != source.data {
                return fail("integrity_error", "推演基线依据来源不符");
            }
            continue;
        }
        let historical = company_versions
            .iter()
            .find(|c| c.id == company.id && c.revision == record.company_revision)
            .ok_or_else(|| DomainError::new("integrity_error", "任务结果的公司历史版本缺失"))?;
        let day = if historical.mode == "simulation" {
            historical
                .simulation_date
                .clone()
                .unwrap_or_else(|| historical.opening_date.clone())
        } else {
            chrono::DateTime::parse_from_rfc3339(&historical.updated_at)
                .map_err(|_| DomainError::new("integrity_error", "游戏记录系统字段无效"))?
                .with_timezone(
                    &historical
                        .timezone
                        .parse::<chrono_tz::Tz>()
                        .map_err(|_| DomainError::new("integrity_error", "游戏记录系统字段无效"))?,
                )
                .date_naive()
                .to_string()
        };
        let mut latest: BTreeMap<(&str, &str), &Record> = BTreeMap::new();
        for version in all_versions
            .iter()
            .filter(|v| v.company_id == company.id && v.company_revision <= record.company_revision)
        {
            let key = (version.kind.as_str(), version.id.as_str());
            if latest
                .get(&key)
                .is_none_or(|old| old.revision < version.revision)
            {
                latest.insert(key, version);
            }
        }
        let context: Vec<Record> = latest.values().copied().cloned().collect();
        let metrics = compute_metrics(historical, &context, date(&day)?)?;
        for reference in record.data["completion"]["result_refs"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let source = latest
                .get(&(text(reference, "kind")?, text(reference, "id")?))
                .ok_or_else(|| {
                    DomainError::new("integrity_error", "游戏依据的准确版本在本公司不存在")
                })?;
            let valid =
                if let Some(resolved) = fork_source(historical, source, blobs, all_versions)? {
                    let origin = &resolved.source;
                    let current = context
                        .iter()
                        .find(|r| r.kind == origin.kind && r.id == origin.id)
                        .unwrap_or(origin);
                    let effective_metrics = if context
                        .iter()
                        .any(|r| r.kind == origin.kind && r.id == origin.id)
                    {
                        &metrics
                    } else {
                        &resolved.baseline_metrics
                    };
                    valid_result(current, effective_metrics, &context, &day)
                } else {
                    valid_result(source, &metrics, &context, &day)
                };
            if !valid {
                return fail("integrity_error", "任务结果尚未成立，不能宣告完成");
            }
        }
    }
    // Every referenced baseline object must remain verifiable, even after a
    // later correction invalidates the quest's current progression reward.
    for file in all_versions.iter().filter(|r| {
        r.company_id == company.id && r.kind == "file" && r.data["fork_evidence"] == true
    }) {
        fork_source(company, file, blobs, all_versions)?;
    }
    Ok(())
}

fn tr(zh: &str, en: &str, locale: Locale) -> String {
    if locale == Locale::ZhCn {
        zh.into()
    } else {
        en.into()
    }
}
pub fn level_name(level: usize, locale: Locale) -> String {
    let zh = [
        "初创掌舵人",
        "市场探索者",
        "经营实干家",
        "增长策划者",
        "资源统筹者",
        "商业建造者",
        "资深经营者",
        "传奇掌舵人",
    ];
    let en = [
        "New Founder",
        "Market Explorer",
        "Business Builder",
        "Growth Strategist",
        "Resource Steward",
        "Venture Architect",
        "Seasoned Operator",
        "Legendary Founder",
    ];
    tr(zh[level - 1], en[level - 1], locale)
}

pub fn quest_reward(category: &str) -> i64 {
    match category {
        "main" => 50,
        "side" => 25,
        "daily" => 10,
        _ => 0,
    }
}
pub fn experience_rules(locale: Locale) -> Value {
    json!({"version":1,"goal_xp":100,"decision_xp":25,"weekly_report_xp":10,"monthly_report_xp":20,"quest_xp":{"main":50,"side":25,"daily":10},"thresholds":LEVEL_THRESHOLDS,"deduplication":tr("同一结果只计一次，关联奖励取最高值；更正后重新核对有效成果。","Shared results count once at the highest applicable reward; corrections re-evaluate valid achievements.",locale),"meaning":tr("经营经验记录档案中的探索与成果，全部基础能力持续可用。","Business experience records exploration and achievements; all core capabilities remain available.",locale)})
}

pub fn progression(mut candidates: Vec<Value>, locale: Locale) -> Value {
    candidates.sort_by_key(|a| a["id"].as_str().unwrap_or("").to_owned());
    // Components, rather than only the first reference, prevent a multi-source
    // quest from laundering an already rewarded decision through a new ID.
    let mut groups: Vec<Vec<Value>> = Vec::new();
    for award in candidates {
        let keys: BTreeSet<String> = award["evidence_keys"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        if keys.is_empty() {
            continue;
        }
        let mut combined = vec![award];
        let mut all_keys = keys;
        let mut i = 0;
        while i < groups.len() {
            let overlaps = groups[i].iter().any(|a| {
                a["evidence_keys"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .any(|k| all_keys.contains(k))
            });
            if overlaps {
                let old = groups.remove(i);
                for a in &old {
                    all_keys.extend(
                        a["evidence_keys"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                            .map(str::to_owned),
                    );
                }
                combined.extend(old);
                i = 0;
            } else {
                i += 1;
            }
        }
        groups.push(combined);
    }
    let mut awards = Vec::new();
    let mut xp = 0i64;
    for mut group in groups {
        group.sort_by_key(|a| {
            (
                std::cmp::Reverse(a["xp"].as_i64().unwrap_or(0)),
                a["id"].as_str().unwrap_or("").to_owned(),
            )
        });
        let mut award = group.remove(0);
        xp += award["xp"].as_i64().unwrap_or(0);
        award["shared_with"] = json!(
            group
                .iter()
                .flat_map(|a| a["source_refs"].as_array().into_iter().flatten().cloned())
                .collect::<Vec<_>>()
        );
        awards.push(award);
    }
    awards.sort_by_key(|a| a["id"].as_str().unwrap_or("").to_owned());
    let level = LEVEL_THRESHOLDS
        .iter()
        .filter(|&&threshold| xp >= threshold)
        .count();
    let current = LEVEL_THRESHOLDS[level - 1];
    let next = LEVEL_THRESHOLDS.get(level).copied();
    json!({"xp":xp,"level":level,"level_name":level_name(level,locale),"current_threshold":current,"next_threshold":next,"progress":next.map(|n|(xp-current) as f64/(n-current) as f64).unwrap_or(1.0),"awards":awards,"rules":experience_rules(locale)})
}

pub fn draw_card(
    seed_sha256: &str,
    day: &str,
    company_revision: i64,
    locale: Locale,
) -> Result<Value> {
    date(day)?;
    let bytes = hex::decode(seed_sha256)
        .map_err(|_| DomainError::new("invalid_input", "事件抽取依据无效"))?;
    if bytes.len() != 32 || company_revision < 1 {
        return fail("invalid_input", "事件抽取依据无效");
    }
    let cards = [
        (
            "customer-referral",
            3,
            "opportunity",
            "熟客带来一位朋友",
            "一位熟客愿意介绍同行，想先确认你的交付时间。安排一场短谈，还是先完成手里的承诺？",
            "A Customer Brings a Friend",
            "A customer offers an introduction and asks about delivery time. Arrange a short conversation or finish your existing commitments first?",
        ),
        (
            "pricing-question",
            2,
            "opportunity",
            "一封询价来信",
            "一位经营者问起小额试单。对方关心的不是功能清单，而是能否解决眼下最费时间的一步。",
            "A Pricing Inquiry",
            "A business owner asks about a small paid trial. Their concern is whether it solves the most time-consuming step in their current workflow.",
        ),
        (
            "competitor-change",
            2,
            "event",
            "同行换了打法",
            "一家同类产品推出了更低价的入门方案。是否调整定位，取决于你的客户究竟在为什么付费。",
            "A Competitor Changes Course",
            "A competing product introduces a cheaper entry plan. Whether to reposition depends on what your customers actually pay for.",
        ),
        (
            "service-disruption",
            1,
            "risk",
            "工具今晚不太稳定",
            "你依赖的一项服务时断时续。备用方案要花时间，继续等待则可能影响交付。先评估影响范围。",
            "An Unsteady Service",
            "A service you rely on becomes intermittent. A fallback takes time; waiting could delay delivery. Assess the affected commitments first.",
        ),
        (
            "cost-review",
            2,
            "risk",
            "续费提醒到了",
            "一项常用工具即将续费。它仍在创造价值吗？保留、降档或迁移，各自占用不同的资金和时间。",
            "A Renewal Is Approaching",
            "A frequently used tool is approaching renewal. Is it still valuable? Keeping, downgrading or moving it uses different amounts of money and time.",
        ),
        (
            "partner-invitation",
            2,
            "opportunity",
            "邻桌递来合作邀请",
            "一位互补业务的经营者提出联合试点，希望先约定客户归属与支持边界。",
            "A Partnership Invitation",
            "An operator in a complementary business proposes a joint pilot and wants to agree on customer ownership and support boundaries.",
        ),
        (
            "customer-feedback",
            3,
            "event",
            "反馈里冒出新线索",
            "一位试用者指出了你没留意的使用障碍。修补细节、进一步访谈或维持路线，需要你决定。",
            "A Clue in Customer Feedback",
            "A trial user identifies a friction point you missed. Fix it, interview further or keep the current course: the choice is yours.",
        ),
        (
            "workload-choice",
            2,
            "risk",
            "待办挤进同一周",
            "几件重要事项撞上了同一段时间。短期增援有成本，重新排序则需要解释新的交付安排。",
            "A Crowded Week",
            "Several important activities compete for the same time. Extra help has a cost; reprioritising requires explaining changed delivery plans.",
        ),
    ];
    let total: i64 = cards.iter().map(|c| c.1).sum();
    let index = (u64::from_be_bytes(bytes[..8].try_into().unwrap()) % total as u64) as i64;
    let mut point = index;
    let mut selected = &cards[0];
    for card in &cards {
        if point < card.1 {
            selected = card;
            break;
        }
        point -= card.1;
    }
    let (id, weight, kind, zh_title, zh_body, en_title, en_body) = selected;
    Ok(
        json!({"title":tr(zh_title,en_title,locale),"body":tr(zh_body,en_body,locale),"type":kind,"severity":if *kind=="risk"{"warning"}else{"info"},"status":"active","basis":"simulation","assumptions":[tr("这是模拟事件牌组提供的经营线索，影响与人物由主持 Agent 结合公司状态设计。","This is a scenario cue from the simulation deck. The hosting Agent adapts its people and consequences to the company state.",locale)],"source_refs":[],"window_start":day,"draw":{"deck_version":DECK_VERSION,"draw_date":day,"seed_sha256":seed_sha256,"card_id":id,"weight":weight,"total_weight":total,"index":index,"company_revision":company_revision}}),
    )
}

pub fn fingerprint(data: &Value) -> String {
    hex::encode(Sha256::digest(serde_json::to_vec(data).unwrap_or_default()))
}

pub fn operation_ids() -> Vec<&'static str> {
    vec![
        "company.game_state",
        "quest.put",
        "quest.accept",
        "quest.complete",
        "quest.set_status",
        "alert.put",
        "alert.ack",
        "alert.set_status",
        "simulation.draw_event",
    ]
}
pub fn operation_scope(operation: &str) -> Option<&'static str> {
    match operation {
        "quest.put"
        | "quest.accept"
        | "quest.complete"
        | "quest.set_status"
        | "alert.put"
        | "alert.ack"
        | "alert.set_status"
        | "simulation.draw_event" => Some("company.write"),
        _ => None,
    }
}
pub fn operation_contract(operation: &str) -> Option<Value> {
    if operation == "company.game_state" {
        return Some(
            json!({"id":operation,"operation":operation,"service":"game","description":"读取任务、事件通知与经营经验","kind":"read","scope":"company.read","http":{"method":"GET","path":"/api/v1/companies/{company_id}/game-state"},"result":{"progression":"经营经验、当前等级、下级阈值、精确奖励依据与规则","quests":"任务和目标投影，包含当前动作","alerts":"事件、预警和已读状态","checks":"明确未知或已知的风险检查","draw":"当前模拟事件抽取权限和冷却"},"rules":["只查询当前公司；真实和模拟进度独立","目标派生任务只读，完成状态由当前指标计算；任务完成只能提交已存在且成立的结果"],"guide":"/api/v1/agent-guide?topic=game"}),
        );
    }
    let refs = json!({"type":"array","maxItems":100,"items":{"type":"object","additionalProperties":false,"required":["kind","id","revision"],"properties":{"kind":{"type":"string"},"id":{"type":"string"},"revision":{"type":"integer","minimum":1}}}});
    let id = json!({"type":"string"});
    let (description, required, mut properties) = match operation {
        "quest.put" => (
            "建立或修订未结束的经营任务",
            vec!["id", "title", "description", "outcome", "category"],
            json!({"id":id,"title":{"type":"string"},"description":{"type":"string"},"outcome":{"type":"string"},"category":{"enum":["main","side","daily"]},"status":{"enum":["open","accepted"]},"deadline":{"type":"string","format":"date"},"depends_on":{"type":"array","uniqueItems":true,"maxItems":100,"items":{"type":"string"}},"source_refs":refs}),
        ),
        "quest.accept" => ("接受已准备的经营任务", vec!["id"], json!({"id":id})),
        "quest.complete" => (
            "用准确经营结果完成已接受任务",
            vec!["id", "summary", "result_refs"],
            json!({"id":id,"summary":{"type":"string"},"result_refs":refs}),
        ),
        "quest.set_status" => (
            "记录任务失败或取消并保留历史",
            vec!["id", "status", "reason"],
            json!({"id":id,"status":{"enum":["failed","cancelled"]},"reason":{"type":"string"}}),
        ),
        "alert.put" => (
            "保存经营事件、机会或风险通知",
            vec!["id", "title", "body", "type", "severity", "basis"],
            json!({"id":id,"title":{"type":"string"},"body":{"type":"string"},"type":{"enum":["opportunity","risk","event"]},"severity":{"enum":["info","warning","critical"]},"status":{"enum":["upcoming","active"]},"basis":{"enum":["observed","forecast","simulation"]},"assumptions":{"type":"array","items":{"type":"string"}},"source_refs":refs,"window_start":{"type":"string","format":"date"},"window_end":{"type":"string","format":"date"},"quest_id":{"type":"string"},"entity_refs":{"type":"array","maxItems":100,"items":{"type":"object","additionalProperties":false,"required":["kind","id"],"properties":{"kind":{"type":"string"},"id":{"type":"string"}}}}}),
        ),
        "alert.ack" => (
            "确认通知已读，保留实际风险",
            vec!["id"],
            json!({"id":id,"fingerprint":{"type":"string","description":"仅派生系统提醒使用查询返回的依据摘要"}}),
        ),
        "alert.set_status" => (
            "为事件结案或收起保留依据",
            vec!["id", "status", "summary"],
            json!({"id":id,"status":{"enum":["resolved","dismissed"]},"summary":{"type":"string"},"source_refs":refs}),
        ),
        "simulation.draw_event" => ("发现当天的一条模拟经营线索", vec![], json!({})),
        _ => return None,
    };
    if operation == "quest.complete" {
        properties["result_refs"]["minItems"] = json!(1);
    }
    Some(
        json!({"id":operation,"operation":operation,"service":"game","description":description,"kind":"write","scope":operation_scope(operation),"http":{"method":"POST","path":format!("/api/v1/companies/{{company_id}}/operations/{operation}")},"input_schema":{"type":"object","required":["request_id","input"],"properties":{"request_id":{"type":"string"},"expected_revision":{"type":["integer","null"]},"input":{"type":"object","additionalProperties":false,"required":required,"properties":properties}}},"result":{"status":"committed","replayed":"boolean","record":"object","company_revision":"integer"},"errors":["invalid_input","forbidden","not_found","reference_not_found","revision_conflict","request_id_conflict","invalid_transition","dependency_incomplete","result_not_achieved","event_cooldown","simulation_required"],"rules":["已有任务或通知修改使用记录 expected_revision；准确重试使用原 request_id","任务结果只能引用本公司已有准确版本；目标未达、业务撤回或现金冲销后重新核对有效成果","经验由记录计算，目标、决策、报告与任务共享结果时取最高奖励，不可直接写入经验","确认已读不解除风险；派生提醒使用查询 actions 中的 fingerprint 和 expected_revision","模拟事件每天最多抽取一次；牌组权重是游戏规则，不代表真实概率，不自动扣款或推进时间"],"guide":"/api/v1/agent-guide?topic=game"}),
    )
}
pub fn guide(topic: &str) -> Option<Value> {
    if !["game", "quests", "alerts"].contains(&topic) {
        return None;
    }
    Some(
        json!({"version":1,"topic":topic,"purpose":"主持任务、经营线索与有依据的成长反馈","steps":["先读取当前公司 game-state，展示当前任务、重要通知和下一项经营取舍，使用稳定人物称呼","任务表达需要取得的结果；目标由指标自动映射为只读主线任务，执行步骤仍由玩家和 Agent 管理","接受任务、确认提醒仅维护游玩状态，不能伪造经营结果；完成任务先提交结果再引用准确版本","模拟可探索每日事件牌组，将线索接入已有关系、承诺和资源；保持因果，真实资料变化与虚构后果分开保存","真实模式只从实际记录产生风险，数据不足按未知处理；预测通知写出假设与触发窗口","等级和经验由固定公开规则与有效结果确定；同一依据不可复用刷分，改正账目后重新核对","面向玩家用场景、投入、后果、任务反馈和选择呈现；版本、抽取种子和接口过程留在维护记录"]}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn award(id: &str, xp: i64, keys: &[&str]) -> Value {
        json!({"id":id,"xp":xp,"evidence_keys":keys,"source_refs":[{"kind":"quest","id":id,"revision":1}]})
    }
    #[test]
    fn experience_is_evidence_based_and_deduplicated() {
        let p = progression(
            vec![
                award("a", 25, &["turn:first"]),
                award("b", 50, &["turn:first", "goal:one"]),
                award("c", 100, &["goal:one"]),
                award("d", 10, &["report:week-one"]),
            ],
            Locale::ZhCn,
        );
        assert_eq!(p["xp"], 110);
        assert_eq!(p["level"], 2);
        assert_eq!(p["awards"].as_array().unwrap().len(), 2);
        assert_eq!(progression(vec![], Locale::En)["xp"], 0);
        assert_eq!(progression(vec![], Locale::En)["level"], 1);
    }
    #[test]
    fn cards_are_repeatable_and_cover_business_tradeoffs() {
        let mut seen = BTreeSet::new();
        for n in 0..200 {
            let seed = hex::encode(Sha256::digest(format!("seed{n}")));
            let a = draw_card(&seed, "2026-10-12", 9, Locale::ZhCn).unwrap();
            assert_eq!(a, draw_card(&seed, "2026-10-12", 9, Locale::ZhCn).unwrap());
            assert_eq!(a["basis"], "simulation");
            seen.insert(a["draw"]["card_id"].as_str().unwrap().to_owned());
        }
        assert_eq!(seen.len(), 8);
    }
    #[test]
    fn exact_refs_reject_foreign_scopes_and_duplicates() {
        assert!(
            validate_exact_refs(
                &json!([{"kind":"goal","id":"x","revision":1,"company_id":"other"}])
            )
            .is_err()
        );
        assert!(validate_exact_refs(&json!([{"kind":"goal","id":"x","revision":1},{"kind":"goal","id":"x","revision":1}])).is_err());
    }
    fn source_record(company: &str, kind: &str, id: &str, data: Value) -> Record {
        Record {
            company_id: company.into(),
            kind: kind.into(),
            id: id.into(),
            revision: 1,
            company_revision: 3,
            data,
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        }
    }
    fn fork_company(id: &str, parent: &str) -> Company {
        serde_json::from_value(json!({"id":id,"name":id,"currency":"CNY","timezone":"Asia/Shanghai","opening_date":"2026-01-01","mode":"simulation","simulation_date":"2026-01-01","baseline_company_id":parent,"baseline_revision":10,"revision":10,"created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z"})).unwrap()
    }
    fn origin(record: &Record) -> Value {
        json!({"company_id":record.company_id,"kind":record.kind,"id":record.id,"revision":record.revision,"company_revision":record.company_revision})
    }
    fn proof_file(
        company: &str,
        source: &Record,
        source_bytes: Option<&[u8]>,
        ultimate: &Record,
        inline: bool,
    ) -> (Record, Vec<u8>, Option<Record>) {
        let mut proof = json!({"format":"transplane-company-fork-evidence","version":1,"origin":origin(source),"source_record":source,"baseline":{"company_id":source.company_id,"company_revision":10,"metrics":{"goal_progress":[{"id":"result","status":"achieved"}]}}});
        let mut registered = None;
        if let Some(content) = source_bytes {
            let digest = text(&source.data, "sha256").unwrap();
            let mut raw = json!({"sha256":digest,"size_bytes":content.len()});
            if inline {
                use base64::Engine;
                raw["content_base64"] =
                    json!(base64::engine::general_purpose::STANDARD.encode(content));
            } else {
                let id = format!("fork-source-{digest}");
                raw["file_ref"] = json!({"kind":"file","id":id,"revision":1});
                let mut file = source_record(
                    company,
                    "file",
                    &id,
                    json!({"sha256":digest,"size_bytes":content.len(),"fork_source_content":true,"relative_path":format!("extensions/provenance/{id}.bin")}),
                );
                file.company_revision = 1;
                registered = Some(file);
            }
            proof["source_file"] = raw;
        }
        let bytes = serde_json::to_vec(&proof).unwrap();
        let digest = hex::encode(Sha256::digest(&bytes));
        let id = format!("fork-proof-{digest}");
        let mut file = source_record(
            company,
            "file",
            &id,
            json!({"sha256":digest,"size_bytes":bytes.len(),"relative_path":format!("extensions/provenance/{id}.json"),"fork_evidence":true,"fork_evidence_origin":origin(ultimate)}),
        );
        file.company_revision = 1;
        (file, bytes, registered)
    }
    #[test]
    fn nested_fork_proof_retains_original_award_identity_and_checks_each_layer() {
        let goal = source_record(
            "real",
            "goal",
            "result",
            json!({"title":"Result","status":"achieved"}),
        );
        let (inner, inner_bytes, _) = proof_file("first", &goal, None, &goal, false);
        let (outer, outer_bytes, raw) =
            proof_file("second", &inner, Some(&inner_bytes), &goal, false);
        let company = fork_company("second", "first");
        let blobs = BTreeMap::from([
            (
                text(&inner.data, "sha256").unwrap().to_owned(),
                inner_bytes.clone(),
            ),
            (text(&outer.data, "sha256").unwrap().to_owned(), outer_bytes),
        ]);
        let registered = vec![outer.clone(), raw.unwrap()];
        let resolved = fork_source(&company, &outer, &blobs, &registered)
            .unwrap()
            .unwrap();
        assert_eq!(resolved.source.kind, "goal");
        assert_eq!(resolved.source.id, "result");
        assert_eq!(resolved.immediate_source.kind, "file");
        assert_eq!(resolved.immediate_source.company_id, "first");
        assert!(fork_source(&company, &outer, &blobs, &[]).is_err());
        let mut forged = outer.clone();
        forged.data["fork_evidence_origin"]["id"] = json!("different");
        assert!(fork_source(&company, &forged, &blobs, &registered).is_err());
        let mut tampered: Value = serde_json::from_slice(&inner_bytes).unwrap();
        tampered["origin"]["revision"] = json!(2);
        let tampered_bytes = serde_json::to_vec(&tampered).unwrap();
        let mut bad_inner = inner.clone();
        bad_inner.data["sha256"] = json!(hex::encode(Sha256::digest(&tampered_bytes)));
        bad_inner.data["size_bytes"] = json!(tampered_bytes.len());
        let (bad_outer, bad_outer_bytes, raw) =
            proof_file("second", &bad_inner, Some(&tampered_bytes), &goal, false);
        let bad_blobs = BTreeMap::from([
            (
                text(&bad_inner.data, "sha256").unwrap().to_owned(),
                tampered_bytes,
            ),
            (
                text(&bad_outer.data, "sha256").unwrap().to_owned(),
                bad_outer_bytes,
            ),
        ]);
        assert!(
            fork_source(
                &company,
                &bad_outer,
                &bad_blobs,
                &[bad_outer.clone(), raw.unwrap()]
            )
            .is_err()
        );
    }
    #[test]
    fn legacy_embedded_proof_bytes_remain_readable_without_new_registration() {
        let goal = source_record(
            "real",
            "goal",
            "result",
            json!({"title":"Result","status":"achieved"}),
        );
        let (inner, inner_bytes, _) = proof_file("first", &goal, None, &goal, false);
        let (outer, outer_bytes, _) = proof_file("second", &inner, Some(&inner_bytes), &goal, true);
        let blobs =
            BTreeMap::from([(text(&outer.data, "sha256").unwrap().to_owned(), outer_bytes)]);
        let resolved = fork_source(&fork_company("second", "first"), &outer, &blobs, &[])
            .unwrap()
            .unwrap();
        assert_eq!(resolved.source.kind, "goal");
        assert_eq!(resolved.source.id, "result");
    }
}
