use crate::{DomainError, Result, date, fail, text};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum SourceKind {
    Company,
    Public,
    World,
    Assumption,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextSource {
    #[serde(rename = "kind")]
    _kind: SourceKind,
    title: String,
    reference: String,
    observed_on: String,
    retrieved_on: String,
    expires_on: Option<String>,
    summary: Option<String>,
}

fn prose(value: &str, field: &str, max: usize, nonempty: bool) -> Result<()> {
    if (nonempty && value.trim().is_empty()) || value.chars().count() > max {
        return fail("invalid_input", format!("{field} 的文字长度无效"));
    }
    Ok(())
}

fn iso_date(value: &str, field: &str) -> Result<chrono::NaiveDate> {
    let parsed = date(value)?;
    if parsed.format("%Y-%m-%d").to_string() != value {
        return fail("invalid_input", format!("{field} 日期必须为 YYYY-MM-DD"));
    }
    Ok(parsed)
}

/// Source annotations are supplied by the author; validation neither fetches
/// the references nor converts a signal or assumption into operating evidence.
pub fn validate(value: &Value) -> Result<()> {
    if let Some(method) = value.get("analysis_method") {
        prose(
            method
                .as_str()
                .ok_or_else(|| DomainError::new("invalid_input", "analysis_method 必须是文字"))?,
            "analysis_method",
            2000,
            true,
        )?;
    }
    if let Some(raw) = value.get("limitations") {
        let limitations = raw
            .as_array()
            .ok_or_else(|| DomainError::new("invalid_input", "limitations 必须是文字数组"))?;
        if limitations.len() > 8 {
            return fail("invalid_input", "预测限制不超过八项");
        }
        for limitation in limitations {
            prose(
                limitation
                    .as_str()
                    .ok_or_else(|| DomainError::new("invalid_input", "预测限制必须是文字"))?,
                "limitations",
                1000,
                false,
            )?;
        }
    }
    let Some(raw) = value.get("context_sources") else {
        return Ok(());
    };
    let sources: Vec<ContextSource> = serde_json::from_value(raw.clone())
        .map_err(|_| DomainError::new("invalid_input", "context_sources 结构无效"))?;
    if sources.len() > 16 {
        return fail("invalid_input", "预测上下文来源不超过十六项");
    }
    if sources.is_empty() {
        return Ok(());
    }
    let cutoff = iso_date(text(value, "as_of")?, "as_of")?;
    for (source, raw) in sources.iter().zip(raw.as_array().unwrap()) {
        // Optional source fields have string values when present; null is not
        // a dated expiry or a source summary.
        if ["expires_on", "summary"]
            .iter()
            .any(|field| raw.get(*field).is_some_and(|value| !value.is_string()))
        {
            return fail("invalid_input", "来源的期限与摘要必须是文字");
        }
        prose(&source.title, "context_sources.title", 240, true)?;
        prose(&source.reference, "context_sources.reference", 2000, true)?;
        if let Some(summary) = &source.summary {
            prose(summary, "context_sources.summary", 2000, false)?;
        }
        let observed = iso_date(&source.observed_on, "observed_on")?;
        let retrieved = iso_date(&source.retrieved_on, "retrieved_on")?;
        if observed > retrieved || retrieved > cutoff {
            return fail(
                "invalid_input",
                "来源日期须满足观察日期不晚于获取日期，获取日期不晚于预测截止日期",
            );
        }
        if let Some(expires) = &source.expires_on
            && iso_date(expires, "expires_on")? < observed
        {
            return fail("invalid_input", "来源失效日期不能早于观察日期");
        }
    }
    Ok(())
}

pub fn sources_schema() -> Value {
    json!({"type":"array","maxItems":16,"items":{"type":"object","additionalProperties":false,
    "required":["kind","title","reference","observed_on","retrieved_on"],"properties":{
        "kind":{"enum":["company","public","world","assumption"]},
        "title":{"type":"string","minLength":1,"maxLength":240},
        "reference":{"type":"string","minLength":1,"maxLength":2000},
        "observed_on":{"type":"string","format":"date"},
        "retrieved_on":{"type":"string","format":"date"},
        "expires_on":{"type":"string","format":"date"},
        "summary":{"type":"string","maxLength":2000}
    }}})
}
