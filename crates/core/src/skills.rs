use super::*;
use sha2::{Digest, Sha256};

pub const MAX_SKILL_BODY_BYTES: usize = 128 * 1024;

/// Hosted guidance is a document, never an executable plugin or an authorization.
pub fn validate_skill(v: &Value) -> Result<()> {
    identifier(text(v, "id")?)?;
    let name = text(v, "name")?;
    if name.len() >= 64
        || name
            .bytes()
            .next()
            .is_none_or(|b| !b.is_ascii_lowercase() && !b.is_ascii_digit())
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return fail(
            "invalid_input",
            "Skill name 只允许少于 64 字符的小写字母、数字和横线",
        );
    }
    text(v, "title")?;
    text(v, "description")?;
    let body = v
        .get("body")
        .and_then(Value::as_str)
        .filter(|body| !body.trim().is_empty() && body.len() <= MAX_SKILL_BODY_BYTES)
        .ok_or_else(|| {
            DomainError::new(
                "invalid_input",
                "Skill body 必须为不超过 128 KiB 的 Markdown",
            )
        })?;
    if body.contains('\0') || body.trim_start().starts_with("---") {
        return fail(
            "invalid_input",
            "Skill body 只填写正文；name 和 description 由元数据生成",
        );
    }
    for field in ["enabled", "deleted"] {
        if v.get(field).is_some_and(|value| !value.is_boolean()) {
            return fail("invalid_input", format!("{field} 必须是布尔值"));
        }
    }
    if v.get("type").is_some() {
        one_of(v, "type", &["procedure", "experience"])?;
    }
    if let Some(tags) = v.get("tags") {
        let tags = tags
            .as_array()
            .ok_or_else(|| DomainError::new("invalid_input", "tags 必须是数组"))?;
        if tags.len() > 32
            || tags.iter().any(|tag| {
                tag.as_str()
                    .is_none_or(|s| s.trim().is_empty() || s.len() > 128)
            })
        {
            return fail("invalid_input", "tags 需要最多 32 项非空短文字");
        }
    }
    if let Some(modes) = v.get("modes") {
        let modes = modes
            .as_array()
            .ok_or_else(|| DomainError::new("invalid_input", "modes 必须是数组"))?;
        if modes.is_empty()
            || modes.len() > 2
            || modes
                .iter()
                .filter_map(Value::as_str)
                .collect::<BTreeSet<_>>()
                .len()
                != modes.len()
            || modes
                .iter()
                .any(|mode| ![json!("real"), json!("simulation")].contains(mode))
        {
            return fail("invalid_input", "modes 需要 real 或 simulation");
        }
    }
    if let Some(refs) = v.get("source_refs") {
        let refs = refs
            .as_array()
            .ok_or_else(|| DomainError::new("invalid_input", "source_refs 必须是数组"))?;
        if refs.len() > 100 {
            return fail("invalid_input", "source_refs 最多 100 项");
        }
        let mut unique_refs = BTreeSet::new();
        for reference in refs {
            identifier(text(reference, "kind")?)?;
            identifier(text(reference, "id")?)?;
            if integer(reference, "revision")? < 1 {
                return fail("invalid_input", "来源版本必须为正整数");
            }
            if reference.get("company_id").is_some() {
                return fail("invalid_input", "Skill 来源只能引用当前公司");
            }
            if !unique_refs.insert((
                text(reference, "kind")?,
                text(reference, "id")?,
                integer(reference, "revision")?,
            )) {
                return fail("invalid_input", "source_refs 不允许重复引用");
            }
        }
    }
    if let Some(origin) = v.get("copied_from") {
        validate_copied_from(origin)?;
    }
    Ok(())
}

pub fn validate_copied_from(origin: &Value) -> Result<()> {
    let object = origin
        .as_object()
        .ok_or_else(|| DomainError::new("integrity_error", "Skill 模板来源无效"))?;
    if object.keys().any(|key| {
        ![
            "id",
            "source",
            "revision",
            "plugin_version",
            "language",
            "content_sha256",
            "document",
        ]
        .contains(&key.as_str())
    }) {
        return fail("integrity_error", "Skill 模板来源无效");
    }
    let id = text(origin, "id")?;
    match text(origin, "source")? {
        "builtin" => {
            identifier(
                id.strip_prefix("builtin:")
                    .ok_or_else(|| DomainError::new("integrity_error", "Skill 模板来源无效"))?,
            )?;
            if origin.get("plugin_version").is_some() {
                return fail("integrity_error", "Skill 模板来源无效");
            }
        }
        "plugin" => {
            let (plugin, skill) = id
                .strip_prefix("plugin:")
                .and_then(|qualified| qualified.split_once(':'))
                .ok_or_else(|| DomainError::new("integrity_error", "Skill 模板来源无效"))?;
            identifier(plugin)?;
            identifier(skill)?;
            let version = text(origin, "plugin_version")?;
            if version.split('.').count() != 3
                || !version.split('.').all(|part| part.parse::<u32>().is_ok())
            {
                return fail("integrity_error", "Skill 模板来源无效");
            }
        }
        _ => return fail("integrity_error", "Skill 模板来源无效"),
    }
    if integer(origin, "revision")? < 1 || !["en", "zh-CN"].contains(&text(origin, "language")?) {
        return fail("integrity_error", "Skill 模板来源无效");
    }
    let document = origin
        .get("document")
        .and_then(Value::as_str)
        .filter(|document| document.len() <= 192 * 1024)
        .ok_or_else(|| DomainError::new("integrity_error", "Skill 模板快照无效"))?;
    if hex::encode(Sha256::digest(document.as_bytes())) != text(origin, "content_sha256")? {
        return fail("integrity_error", "Skill 模板快照摘要不符");
    }
    let (metadata, body) = document
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .ok_or_else(|| DomainError::new("integrity_error", "Skill 模板快照无效"))?;
    let mut lines = metadata.lines();
    let name = lines
        .next()
        .and_then(|line| line.strip_prefix("name: "))
        .and_then(|value| serde_json::from_str::<String>(value).ok())
        .ok_or_else(|| DomainError::new("integrity_error", "Skill 模板快照无效"))?;
    let description = lines
        .next()
        .and_then(|line| line.strip_prefix("description: "))
        .and_then(|value| serde_json::from_str::<String>(value).ok())
        .ok_or_else(|| DomainError::new("integrity_error", "Skill 模板快照无效"))?;
    if lines.next().is_some()
        || name.is_empty()
        || name.len() >= 64
        || name
            .bytes()
            .next()
            .is_none_or(|b| !b.is_ascii_lowercase() && !b.is_ascii_digit())
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        || description.trim().is_empty()
        || description.len() > 16384
        || body.trim().is_empty()
        || body.trim().len() > MAX_SKILL_BODY_BYTES
    {
        return fail("integrity_error", "Skill 模板快照无效");
    }
    let canonical_body = body
        .strip_prefix('\n')
        .ok_or_else(|| DomainError::new("integrity_error", "Skill 模板快照无效"))?;
    if canonical_body.trim_end().len() > MAX_SKILL_BODY_BYTES
        || canonical_body.contains('\0')
        || canonical_body.trim_start().starts_with("---")
    {
        return fail("integrity_error", "Skill 模板快照无效");
    }
    if skill_document(&json!({"name":name,"description":description,"body":canonical_body}))
        != document
    {
        return fail("integrity_error", "Skill 模板快照无效");
    }
    Ok(())
}

pub fn skill_document(v: &Value) -> String {
    format!(
        "---\nname: {}\ndescription: {}\n---\n\n{}\n",
        serde_json::to_string(v["name"].as_str().unwrap_or("")).unwrap(),
        serde_json::to_string(v["description"].as_str().unwrap_or("")).unwrap(),
        v["body"].as_str().unwrap_or("").trim_end()
    )
}
