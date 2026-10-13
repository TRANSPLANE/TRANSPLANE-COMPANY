use crate::{Company, DomainError, Record, Result, date, fail, identifier, text};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CastMember {
    id: String,
    name: String,
    role: String,
    concern: Option<String>,
    line: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Callback {
    decision_id: String,
    summary: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Material {
    id: String,
    title: String,
    body: String,
    kind: String,
    cast_id: Option<String>,
    #[serde(default)]
    after: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Chapter {
    id: String,
    title: String,
    turn: i64,
    planned_turns: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Experience {
    scene: String,
    why_now: String,
    stakes: String,
    cast: Vec<CastMember>,
    #[serde(default)]
    callbacks: Vec<Callback>,
    #[serde(default)]
    materials: Vec<Material>,
    chapter: Option<Chapter>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reaction {
    cast_id: String,
    line: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RelationshipChange {
    cast_id: String,
    change: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Commitment {
    id: String,
    title: String,
    cast_id: Option<String>,
    due_on: Option<String>,
    status: String,
    source_decision_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChapterResult {
    id: String,
    status: String,
    summary: Option<String>,
    takeaway: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Aftermath {
    #[serde(default)]
    immediate: Vec<String>,
    #[serde(default)]
    reactions: Vec<Reaction>,
    #[serde(default)]
    relationships: Vec<RelationshipChange>,
    #[serde(default)]
    commitments: Vec<Commitment>,
    chapter: Option<ChapterResult>,
}

fn parse<T: serde::de::DeserializeOwned>(value: &Value, field: &str) -> Result<T> {
    serde_json::from_value(value.clone())
        .map_err(|_| DomainError::new("invalid_input", format!("{field} 结构无效")))
}

fn prose(value: &str, field: &str, max: usize) -> Result<()> {
    if value.trim().is_empty() || value.chars().count() > max {
        return fail(
            "invalid_input",
            format!("{field} 必须是非空文字，且不超过 {max} 个字符"),
        );
    }
    Ok(())
}

fn optional_prose(value: Option<&str>, field: &str, max: usize) -> Result<()> {
    if let Some(value) = value {
        prose(value, field, max)?;
    }
    Ok(())
}

fn local_decision<'a>(company: &Company, records: &'a [Record], id: &str) -> Result<&'a Record> {
    identifier(id)?;
    records
        .iter()
        .find(|record| {
            record.company_id == company.id && record.kind == "decision" && record.id == id
        })
        .ok_or_else(|| DomainError::new("reference_not_found", "剧情引用的本公司经营事件不存在"))
}

pub fn validate_experience(value: &Value, company: &Company, records: &[Record]) -> Result<()> {
    let Some(raw) = value.get("experience") else {
        return Ok(());
    };
    let experience: Experience = parse(raw, "experience")?;
    prose(&experience.scene, "experience.scene", 2000)?;
    prose(&experience.why_now, "experience.why_now", 1000)?;
    prose(&experience.stakes, "experience.stakes", 1000)?;
    if experience.cast.len() > 8 || experience.callbacks.len() > 8 {
        return fail("invalid_input", "剧情人物与回访各不超过八项");
    }
    let mut cast_ids = BTreeSet::new();
    for member in &experience.cast {
        identifier(&member.id)?;
        if !cast_ids.insert(&member.id) {
            return fail("invalid_input", "剧情人物标识不能重复");
        }
        prose(&member.name, "cast.name", 120)?;
        prose(&member.role, "cast.role", 240)?;
        optional_prose(member.concern.as_deref(), "cast.concern", 600)?;
        optional_prose(member.line.as_deref(), "cast.line", 600)?;
    }
    if experience.materials.len() > 12 {
        return fail("invalid_input", "一个场景最多准备十二份相关资料");
    }
    let mut material_ids = BTreeSet::new();
    for material in &experience.materials {
        identifier(&material.id)?;
        if !material_ids.insert(material.id.as_str()) {
            return fail("invalid_input", "场景资料标识不能重复");
        }
        prose(&material.title, "materials.title", 240)?;
        prose(&material.body, "materials.body", 2000)?;
        if !["letter", "person", "note"].contains(&material.kind.as_str()) {
            return fail("invalid_input", "场景资料类型不在允许范围内");
        }
        if material
            .cast_id
            .as_ref()
            .is_some_and(|id| !cast_ids.contains(id))
        {
            return fail("reference_not_found", "场景资料引用的人物不存在");
        }
    }
    let mut dependencies = BTreeMap::new();
    for material in &experience.materials {
        if material.after.len() > 12 {
            return fail("invalid_input", "场景资料依赖超过范围");
        }
        let mut seen = BTreeSet::new();
        for id in &material.after {
            identifier(id)?;
            if id == &material.id
                || !material_ids.contains(id.as_str())
                || !seen.insert(id.as_str())
            {
                return fail("invalid_input", "场景资料依赖不存在或重复");
            }
        }
        dependencies.insert(
            material.id.as_str(),
            material
                .after
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        );
    }
    fn visit<'a>(
        id: &'a str,
        dependencies: &BTreeMap<&'a str, Vec<&'a str>>,
        visiting: &mut BTreeSet<&'a str>,
        done: &mut BTreeSet<&'a str>,
    ) -> Result<()> {
        if done.contains(id) {
            return Ok(());
        }
        if !visiting.insert(id) {
            return fail("invalid_input", "场景资料依赖不能循环");
        }
        for dependency in &dependencies[id] {
            visit(dependency, dependencies, visiting, done)?;
        }
        visiting.remove(id);
        done.insert(id);
        Ok(())
    }
    let mut visiting = BTreeSet::new();
    let mut done = BTreeSet::new();
    for id in dependencies.keys() {
        visit(id, &dependencies, &mut visiting, &mut done)?;
    }
    let baseline = value["baseline_revision"]
        .as_i64()
        .unwrap_or(company.revision);
    let mut callbacks = BTreeSet::new();
    for callback in &experience.callbacks {
        if !callbacks.insert(&callback.decision_id) {
            return fail("invalid_input", "剧情回访不能重复引用同一事件");
        }
        prose(&callback.summary, "callbacks.summary", 1000)?;
        let source = local_decision(company, records, &callback.decision_id)?;
        if source.data["status"] != "resolved" || source.company_revision > baseline {
            return fail(
                "invalid_result_reference",
                "剧情回访只能引用当时已结算的本公司事件",
            );
        }
    }
    if let Some(chapter) = &experience.chapter {
        identifier(&chapter.id)?;
        prose(&chapter.title, "chapter.title", 240)?;
        if !(3..=5).contains(&chapter.planned_turns)
            || !(1..=chapter.planned_turns).contains(&chapter.turn)
        {
            return fail(
                "invalid_input",
                "短篇规划为三至五回合，当前回合须在规划范围内",
            );
        }
    }
    let options = value["options"]
        .as_array()
        .ok_or_else(|| DomainError::new("invalid_input", "经营事件需要 options 数组"))?;
    if !(2..=4).contains(&options.len()) {
        return fail("invalid_input", "叙事事件需要二至四个有取舍的方案");
    }
    for option in options {
        prose(text(option, "benefit")?, "option.benefit", 1000)?;
        prose(text(option, "tradeoff")?, "option.tradeoff", 1000)?;
    }
    Ok(())
}

pub fn validate_option(option: &Value) -> Result<()> {
    for field in ["benefit", "tradeoff", "commitment"] {
        if let Some(value) = option.get(field) {
            prose(
                value.as_str().ok_or_else(|| {
                    DomainError::new("invalid_input", format!("{field} 必须是文字"))
                })?,
                field,
                1000,
            )?;
        }
    }
    if let Some(day) = option.get("feedback_on") {
        date(
            day.as_str()
                .ok_or_else(|| DomainError::new("invalid_input", "feedback_on 必须是文字"))?,
        )?;
    }
    Ok(())
}

pub fn validate_aftermath(value: &Value, company: &Company, records: &[Record]) -> Result<()> {
    let Some(raw) = value.get("aftermath") else {
        return Ok(());
    };
    let aftermath: Aftermath = parse(raw, "aftermath")?;
    let parent_id = text(value, "decision_id")?;
    let parent = local_decision(company, records, parent_id)?;
    if !matches!(
        parent.data["status"].as_str(),
        Some("selected" | "processing" | "resolved")
    ) {
        return fail("invalid_transition", "经营后果需要关联已选择的本公司事件");
    }
    if aftermath.immediate.len() > 8
        || aftermath.reactions.len() > 8
        || aftermath.relationships.len() > 8
        || aftermath.commitments.len() > 16
    {
        return fail("invalid_input", "经营后果条目超过单回合范围");
    }
    if aftermath.immediate.is_empty()
        && aftermath.reactions.is_empty()
        && aftermath.relationships.is_empty()
        && aftermath.commitments.is_empty()
        && aftermath.chapter.is_none()
    {
        return fail("invalid_input", "经营后果需要已发生的变化或阶段反馈");
    }
    let baseline = parent.data["baseline_revision"]
        .as_i64()
        .unwrap_or(company.revision);
    let known_cast: BTreeSet<&str> = records
        .iter()
        .filter(|record| {
            record.company_id == company.id
                && record.kind == "decision"
                && (record.id == parent_id
                    || (record.data["status"] == "resolved" && record.company_revision <= baseline))
        })
        .flat_map(|record| {
            record.data["experience"]["cast"]
                .as_array()
                .into_iter()
                .flatten()
        })
        .filter_map(|member| member["id"].as_str())
        .collect();
    let check_cast = |id: &str| -> Result<()> {
        identifier(id)?;
        if !known_cast.contains(id) {
            return fail("reference_not_found", "经营后果引用的已保存人物不存在");
        }
        Ok(())
    };
    for change in &aftermath.immediate {
        prose(change, "aftermath.immediate", 1200)?;
    }
    for reaction in &aftermath.reactions {
        check_cast(&reaction.cast_id)?;
        prose(&reaction.line, "reactions.line", 600)?;
    }
    for relationship in &aftermath.relationships {
        check_cast(&relationship.cast_id)?;
        prose(&relationship.change, "relationships.change", 1000)?;
    }
    let mut promise_ids = BTreeSet::new();
    for promise in &aftermath.commitments {
        identifier(&promise.id)?;
        if !promise_ids.insert(&promise.id) {
            return fail("invalid_input", "同一回合的承诺标识不能重复");
        }
        prose(&promise.title, "commitments.title", 1000)?;
        if !["pending", "fulfilled", "broken", "cancelled"].contains(&promise.status.as_str()) {
            return fail("invalid_input", "承诺状态不在允许范围内");
        }
        if let Some(id) = &promise.cast_id {
            check_cast(id)?;
        }
        if let Some(day) = &promise.due_on {
            date(day)?;
        }
        let origin = local_decision(company, records, &promise.source_decision_id)?;
        if origin.id != parent_id
            && (origin.data["status"] != "resolved" || origin.company_revision > baseline)
        {
            return fail(
                "invalid_result_reference",
                "承诺来源须为当前选择或先前已结算的本公司事件",
            );
        }
    }
    if let Some(chapter) = &aftermath.chapter {
        identifier(&chapter.id)?;
        if parent.data["experience"]["chapter"]["id"] != chapter.id {
            return fail("invalid_result_reference", "阶段结算必须对应所选事件的短篇");
        }
        if !["ongoing", "closed"].contains(&chapter.status.as_str()) {
            return fail("invalid_input", "短篇状态不在允许范围内");
        }
        optional_prose(chapter.summary.as_deref(), "chapter.summary", 2000)?;
        optional_prose(chapter.takeaway.as_deref(), "chapter.takeaway", 1200)?;
        if chapter.status == "closed" && (chapter.summary.is_none() || chapter.takeaway.is_none()) {
            return fail("invalid_input", "短篇收束需要结果总结与经营所得");
        }
    }
    Ok(())
}

/// Fold only exact, settled records. A corrected source version cannot replace
/// the version the player actually saw at an earlier settlement.
pub fn validate_story_history(company: &Company, versions: &[Record]) -> Result<()> {
    let mut latest = BTreeMap::<&str, &Record>::new();
    let mut exact = BTreeMap::new();
    for record in versions
        .iter()
        .filter(|record| record.company_id == company.id)
    {
        exact.insert(
            (record.kind.as_str(), record.id.as_str(), record.revision),
            record,
        );
        if record.kind == "decision"
            && latest
                .get(record.id.as_str())
                .is_none_or(|old| old.revision < record.revision)
        {
            latest.insert(record.id.as_str(), record);
        }
    }
    let mut resolved = latest
        .into_values()
        .filter(|record| record.data["status"] == "resolved")
        .collect::<Vec<_>>();
    resolved.sort_by_key(|record| record.company_revision);
    let mut origins = BTreeMap::<String, (String, Option<String>)>::new();
    for decision in resolved {
        let mut result_records = Vec::new();
        for reference in decision.data["resolution"]["result_refs"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let kind = text(reference, "kind")?;
            if !["simulation_turn", "report"].contains(&kind) {
                continue;
            }
            let Some(record) = exact.get(&(
                kind,
                text(reference, "id")?,
                reference["revision"].as_i64().unwrap_or(0),
            )) else {
                return fail("integrity_error", "剧情结算引用的准确版本不存在");
            };
            result_records.push(*record);
        }
        result_records.sort_by(|left, right| {
            (
                left.company_revision,
                left.kind.as_str(),
                left.id.as_str(),
                left.revision,
            )
                .cmp(&(
                    right.company_revision,
                    right.kind.as_str(),
                    right.id.as_str(),
                    right.revision,
                ))
        });
        for record in result_records {
            let Some(raw) = record.data.get("aftermath") else {
                continue;
            };
            if record.data["decision_id"] != decision.id {
                return fail("invalid_result_reference", "经营后果必须结算到其关联事件");
            }
            let aftermath: Aftermath = parse(raw, "aftermath")?;
            for promise in aftermath.commitments {
                if let Some((source, cast)) = origins.get(&promise.id)
                    && (source != &promise.source_decision_id
                        || cast
                            .as_ref()
                            .is_some_and(|id| Some(id) != promise.cast_id.as_ref()))
                {
                    return fail("integrity_error", "同一承诺的来源事件与已知人物不能替换");
                }
                origins.insert(promise.id, (promise.source_decision_id, promise.cast_id));
            }
        }
    }
    Ok(())
}

pub fn experience_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["scene","why_now","stakes","cast"],"properties":{
        "scene":{"type":"string","maxLength":2000},"why_now":{"type":"string","maxLength":1000},"stakes":{"type":"string","maxLength":1000},
        "cast":{"type":"array","maxItems":8,"items":{"type":"object","additionalProperties":false,"required":["id","name","role"],"properties":{"id":{"type":"string"},"name":{"type":"string","maxLength":120},"role":{"type":"string","maxLength":240},"concern":{"type":"string","maxLength":600},"line":{"type":"string","maxLength":600}}}},
        "callbacks":{"type":"array","maxItems":8,"items":{"type":"object","additionalProperties":false,"required":["decision_id","summary"],"properties":{"decision_id":{"type":"string"},"summary":{"type":"string","maxLength":1000}}}},
        "materials":{"type":"array","maxItems":12,"items":{"type":"object","additionalProperties":false,"required":["id","title","body","kind"],"properties":{"id":{"type":"string"},"title":{"type":"string","maxLength":240},"body":{"type":"string","maxLength":2000},"kind":{"enum":["letter","person","note"]},"cast_id":{"type":"string"},"after":{"type":"array","maxItems":12,"uniqueItems":true,"items":{"type":"string"}}}}},
        "chapter":{"type":"object","additionalProperties":false,"required":["id","title","turn","planned_turns"],"properties":{"id":{"type":"string"},"title":{"type":"string","maxLength":240},"turn":{"type":"integer","minimum":1,"maximum":5},"planned_turns":{"type":"integer","minimum":3,"maximum":5}}}
    }})
}

pub fn aftermath_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"properties":{
        "immediate":{"type":"array","maxItems":8,"items":{"type":"string","maxLength":1200}},
        "reactions":{"type":"array","maxItems":8,"items":{"type":"object","additionalProperties":false,"required":["cast_id","line"],"properties":{"cast_id":{"type":"string"},"line":{"type":"string","maxLength":600}}}},
        "relationships":{"type":"array","maxItems":8,"items":{"type":"object","additionalProperties":false,"required":["cast_id","change"],"properties":{"cast_id":{"type":"string"},"change":{"type":"string","maxLength":1000}}}},
        "commitments":{"type":"array","maxItems":16,"items":{"type":"object","additionalProperties":false,"required":["id","title","status","source_decision_id"],"properties":{"id":{"type":"string"},"title":{"type":"string","maxLength":1000},"cast_id":{"type":"string"},"due_on":{"type":"string","format":"date"},"status":{"enum":["pending","fulfilled","broken","cancelled"]},"source_decision_id":{"type":"string"}}}},
        "chapter":{"type":"object","additionalProperties":false,"required":["id","status"],"properties":{"id":{"type":"string"},"status":{"enum":["ongoing","closed"]},"summary":{"type":"string","maxLength":2000},"takeaway":{"type":"string","maxLength":1200}}}
    }})
}
