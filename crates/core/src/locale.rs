use crate::{DomainError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Locale {
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "en")]
    En,
}
impl Locale {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ZhCn => "zh-CN",
            Self::En => "en",
        }
    }
    pub fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "zh" | "zh-cn" | "zh-hans" => Ok(Self::ZhCn),
            "en" | "en-us" | "en-gb" => Ok(Self::En),
            _ => Err(DomainError::new(
                "unsupported_language",
                "Supported languages are en and zh-CN.",
            )),
        }
    }
    pub fn from_language(value: &str) -> Self {
        if value
            .split(',')
            .next()
            .unwrap_or(value)
            .trim()
            .to_ascii_lowercase()
            .starts_with("zh")
        {
            Self::ZhCn
        } else {
            Self::En
        }
    }
}
fn catalog() -> &'static BTreeMap<String, String> {
    static CATALOG: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let mut catalog: BTreeMap<String, String> =
            serde_json::from_str(include_str!("locales/en.json"))
                .expect("valid built-in English catalog");
        for module in [
            include_str!("network.en.json"),
            include_str!("game_systems.en.json"),
        ] {
            let contribution: BTreeMap<String, String> =
                serde_json::from_str(module).expect("valid module English catalog");
            catalog.extend(contribution);
        }
        catalog
    })
}
pub fn translate(text: &str, locale: Locale) -> String {
    if locale == Locale::ZhCn {
        return text.into();
    }
    if let Some(translation) = catalog().get(text) {
        return translation.clone();
    }
    for (template, translation) in catalog().iter().filter(|(key, _)| key.contains('{')) {
        let mut rest = text;
        let mut source = template.as_str();
        let mut substitutions = Vec::new();
        let mut matched = true;
        while let Some(start) = source.find('{') {
            if !rest.starts_with(&source[..start]) {
                matched = false;
                break;
            }
            rest = &rest[start..];
            source = &source[start..];
            let Some(end) = source.find('}') else {
                matched = false;
                break;
            };
            let name = &source[..=end];
            source = &source[end + 1..];
            let tail = source.split('{').next().unwrap_or("");
            let position = if tail.is_empty() {
                rest.len()
            } else if let Some(p) = rest.find(tail) {
                p
            } else {
                matched = false;
                break;
            };
            substitutions.push((name.to_owned(), rest[..position].to_owned()));
            rest = &rest[position..];
        }
        if matched && rest == source {
            let mut result = translation.clone();
            for (name, value) in substitutions {
                result = result.replace(&name, &value);
            }
            return result;
        }
    }
    text.into()
}
pub fn localize_metadata(value: &mut Value, locale: Locale) {
    if locale == Locale::ZhCn {
        return;
    }
    match value {
        Value::String(s) => *s = translate(s, locale),
        Value::Array(a) => a.iter_mut().for_each(|v| localize_metadata(v, locale)),
        Value::Object(o) => o.values_mut().for_each(|v| localize_metadata(v, locale)),
        _ => {}
    }
}
pub fn localize_error(error: &DomainError, locale: Locale) -> String {
    let translation = translate(&error.message, locale);
    if locale == Locale::ZhCn
        || translation != error.message
        || !error
            .message
            .chars()
            .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
    {
        return translation;
    }
    match error.code.as_str(){
        "unauthorized"=>"The credential is invalid, expired or revoked.".into(),
        "forbidden"=>"This credential does not permit this company or operation.".into(),
        "not_found"=>"The requested resource does not exist or is unavailable.".into(),
        "revision_conflict"=>"The version has changed. Read the current version, preserve your changes and submit a new request ID.".into(),
        "request_id_conflict"=>"This request ID was already used with different input.".into(),
        "storage_busy"=>"The database is busy. Retry with the same request ID and input.".into(),
        "integrity_error"=>"Integrity verification failed. Preserve the source and inspect its versions and hashes.".into(),
        "internal_error"=>"The operation could not finish. Check the deployment status and retry the original request when appropriate.".into(),
        _=>format!("Operation rejected ({}). Check the current operation contract and input.",error.code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parameters_and_identifiers_are_preserved() {
        assert_eq!(
            translate("amount_minor 必须是整数", Locale::En),
            "amount_minor must be an integer."
        );
        assert_eq!(
            translate("subscription.amount_minor 越界", Locale::En),
            "subscription.amount_minor is out of range."
        );
        assert_eq!(Locale::from_language("en-US,en;q=0.9"), Locale::En);
        assert_eq!(Locale::from_language("zh-CN,zh;q=0.8"), Locale::ZhCn);
    }
    #[test]
    fn both_discovery_and_guidance_have_complete_host_translations() {
        fn check(v: &Value) {
            match v {
                Value::String(s) => assert!(
                    !s.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)),
                    "Untranslated host text: {s}"
                ),
                Value::Array(a) => a.iter().for_each(check),
                Value::Object(o) => o.values().for_each(check),
                _ => {}
            }
        }
        for op in crate::operation_ids() {
            let mut contract = crate::discovery(Some("contract"), None, Some(op)).unwrap();
            localize_metadata(&mut contract, Locale::En);
            check(&contract);
        }
        check(&serde_json::json!(translate(
            crate::PLAYER_OUTPUT_HINT,
            Locale::En
        )));
        for topic in [
            "connect",
            "company",
            "finance",
            "growth",
            "reports",
            "business",
            "files",
            "simulation",
            "decision",
            "play",
            "skills",
            "player-output",
            "network",
            "game",
            "backup",
            "plugins",
            "recover",
        ] {
            let mut guide = crate::guide(topic).unwrap();
            localize_metadata(&mut guide, Locale::En);
            check(&guide);
        }
    }
}
