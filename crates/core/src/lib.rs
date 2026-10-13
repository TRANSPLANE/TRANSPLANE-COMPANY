use chrono::{Datelike, Months, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
pub mod agent_link;
pub mod forecast_context;
pub mod game_systems;
pub mod gameplay;
pub mod integration;
pub mod locale;
pub mod network;
pub mod skills;
pub mod studio;
pub mod studio_contract;
pub use locale::{Locale, localize_error};
pub fn discovery_localized(
    view: Option<&str>,
    service: Option<&str>,
    operation: Option<&str>,
    locale: Locale,
) -> Result<Value> {
    let mut result = discovery(view, service, operation)?;
    locale::localize_metadata(&mut result, locale);
    Ok(result)
}
pub fn guide_localized(topic: &str, locale: Locale) -> Result<Value> {
    let mut result = guide(topic)?;
    locale::localize_metadata(&mut result, locale);
    Ok(result)
}

#[derive(Debug, thiserror::Error)]
#[error("{code}: {message}")]
pub struct DomainError {
    pub code: String,
    pub message: String,
}
impl DomainError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}
pub type Result<T> = std::result::Result<T, DomainError>;
pub const MAX_SAFE_MINOR: i64 = 9_007_199_254_740_991;
pub fn fail<T>(code: &str, message: impl Into<String>) -> Result<T> {
    Err(DomainError::new(code, message))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Company {
    pub id: String,
    pub name: String,
    pub currency: String,
    pub timezone: String,
    #[serde(default)]
    pub description: String,
    pub opening_date: String,
    #[serde(default = "real_mode")]
    pub mode: String,
    #[serde(default)]
    pub subscription_tracking: bool,
    #[serde(default)]
    pub simulation_date: Option<String>,
    #[serde(default)]
    pub baseline_company_id: Option<String>,
    #[serde(default)]
    pub baseline_revision: Option<i64>,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}
fn real_mode() -> String {
    "real".into()
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    pub company_id: String,
    pub kind: String,
    pub revision: i64,
    #[serde(default)]
    pub company_revision: i64,
    pub data: Value,
    pub created_at: String,
    pub updated_at: String,
}

pub fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty() && s.len() <= 16384)
        .ok_or_else(|| DomainError::new("invalid_input", format!("{key} 必须是非空文字")))
}
pub fn integer(value: &Value, key: &str) -> Result<i64> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| DomainError::new("invalid_input", format!("{key} 必须是整数")))
}
pub fn date(value: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| DomainError::new("invalid_input", "日期格式必须为 YYYY-MM-DD"))
}
pub fn identifier(s: &str) -> Result<()> {
    if s.is_empty()
        || s.len() > 128
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return fail("invalid_input", "标识只能包含字母、数字、横线和下划线");
    }
    Ok(())
}
fn one_of(v: &Value, key: &str, values: &[&str]) -> Result<()> {
    if !values.contains(&text(v, key)?) {
        return fail("invalid_input", format!("{key} 不在允许范围内"));
    }
    Ok(())
}
pub fn relative_path(p: &str) -> Result<()> {
    if p.len() > 1024
        || p.contains('\\')
        || p.contains(':')
        || p.starts_with('/')
        || p.split('/').any(|c| c.is_empty() || c == "." || c == "..")
    {
        return fail("invalid_path", "仅接受公司目录内的相对路径");
    }
    if p != "INDEX.md"
        && !["context/", "documents/", "reports/", "extensions/"]
            .iter()
            .any(|pre| p.starts_with(pre))
    {
        return fail(
            "invalid_path",
            "文件必须位于 context、documents、reports 或 extensions 目录",
        );
    }
    Ok(())
}

pub fn validate_company(v: &Value) -> Result<()> {
    text(v, "name")?;
    let currency = text(v, "currency")?;
    if currency.len() != 3 || !currency.bytes().all(|c| c.is_ascii_uppercase()) {
        return fail("invalid_input", "币种必须是三个大写字母");
    }
    text(v, "timezone")?
        .parse::<chrono_tz::Tz>()
        .map_err(|_| DomainError::new("invalid_input", "未知时区"))?;
    date(text(v, "opening_date")?)?;
    if v.get("mode").is_some() {
        one_of(v, "mode", &["real", "simulation"])?;
    }
    if let Some(d) = v.get("simulation_date").and_then(Value::as_str) {
        date(d)?;
    }
    Ok(())
}

pub fn validate_record(
    kind: &str,
    v: &Value,
    company: &Company,
    existing: &[Record],
) -> Result<()> {
    if !v.is_object() || serde_json::to_vec(v).unwrap_or_default().len() > 524288 {
        return fail("invalid_input", "记录必须是大小不超过 512 KiB 的对象");
    }
    if network::RECORD_KINDS.contains(&kind) {
        return network::validate_record(kind, v, company, existing);
    }
    if ["quest", "alert"].contains(&kind) {
        return game_systems::validate_game_record(kind, v, company, existing);
    }
    let require_ref = |field: &str, target: &str| -> Result<()> {
        let id = text(v, field)?;
        if !existing.iter().any(|r| r.id == id && r.kind == target) {
            return fail("reference_not_found", format!("{field} 引用不存在"));
        }
        Ok(())
    };
    if v.get("currency").is_some() {
        let currency = text(v, "currency")?;
        if currency != company.currency {
            return fail("currency_mismatch", "金额币种与公司记账币种不一致");
        }
    }
    match kind {
        "account" => {
            text(v, "name")?;
            if integer(v, "opening_balance_minor")?.unsigned_abs() > 1_000_000_000_000_000 {
                return fail("invalid_input", "期初金额超过支持范围");
            }
            if let Some(d) = v.get("opening_on").and_then(Value::as_str) {
                date(d)?;
            }
        }
        "transaction" => {
            require_ref("account_id", "account")?;
            let account = existing
                .iter()
                .find(|r| r.kind == "account" && r.id == v["account_id"].as_str().unwrap())
                .unwrap();
            let opening = account
                .data
                .get("opening_on")
                .and_then(Value::as_str)
                .unwrap_or(&company.opening_date);
            if date(text(v, "occurred_on")?)? < date(opening)? {
                return fail("invalid_input", "资金记录早于账户期初日期");
            }
            if !(1..=1_000_000_000_000_000).contains(&integer(v, "amount_minor")?) {
                return fail("invalid_input", "金额必须是有效的正整数最小货币单位");
            }
            one_of(v, "direction", &["inflow", "outflow"])?;
            one_of(
                v,
                "category",
                &["operating", "capital", "financing", "transfer"],
            )?;
            date(text(v, "occurred_on")?)?;
            let source = v
                .get("source")
                .ok_or_else(|| DomainError::new("invalid_input", "资金记录需要来源"))?;
            text(source, "kind")?;
            text(source, "reference")?;
            if v.get("customer_id").is_some() {
                require_ref("customer_id", "customer")?;
            }
        }
        "business" => {
            text(v, "name")?;
            one_of(v, "type", &["product", "project"])?;
            one_of(
                v,
                "status",
                &[
                    "planned",
                    "active",
                    "released",
                    "completed",
                    "paused",
                    "archived",
                ],
            )?;
        }
        "customer" => {
            text(v, "name")?;
            one_of(
                v,
                "status",
                &["lead", "active", "paying", "inactive", "archived"],
            )?;
        }
        "resource" => {
            text(v, "name")?;
            one_of(v, "type", &["human", "agent", "tool", "service"])?;
            text(v, "status")?;
        }
        "subscription" => {
            require_ref("customer_id", "customer")?;
            let amount = integer(v, "amount_minor")?;
            if !(0..=1_000_000_000_000_000).contains(&amount) {
                return fail("invalid_input", "订阅金额越界");
            }
            if ![1, 12].contains(&integer(v, "interval_months")?) {
                return fail("invalid_input", "首版订阅周期为 1 或 12 个月");
            }
            one_of(
                v,
                "status",
                &["active", "past_due", "trial", "free", "unpaid", "cancelled"],
            )?;
            let start = date(text(v, "starts_on")?)?;
            if let Some(end) = v.get("ends_on").and_then(Value::as_str)
                && date(end)? <= start
            {
                return fail("invalid_input", "订阅结束日必须晚于开始日");
            }
            for field in ["tax_minor", "recurring_discount_minor"] {
                let n = if v.get(field).is_some() {
                    integer(v, field)?
                } else {
                    0
                };
                if n < 0 || n > amount {
                    return fail("invalid_input", format!("{field} 越界"));
                }
            }
            if v.get("tax_minor").and_then(Value::as_i64).unwrap_or(0)
                + v.get("recurring_discount_minor")
                    .and_then(Value::as_i64)
                    .unwrap_or(0)
                > amount
            {
                return fail("invalid_input", "税款与折扣合计超过订阅金额");
            }
        }
        "goal" => {
            text(v, "title")?;
            text(v, "outcome")?;
            one_of(
                v,
                "status",
                &["planned", "active", "achieved", "blocked", "archived"],
            )?;
            let metric = v.get("metric").and_then(Value::as_str).unwrap_or("manual");
            if ![
                "manual",
                "mrr",
                "cash_balance",
                "paid_customers",
                "business_released",
            ]
            .contains(&metric)
            {
                return fail("invalid_input", "未知目标指标");
            }
            if metric != "manual"
                && v.get("target")
                    .and_then(Value::as_f64)
                    .is_none_or(|n| !n.is_finite() || n <= 0.)
            {
                return fail("invalid_input", "量化目标需要大于零的 target");
            }
            if metric == "manual" && text(v, "status")? == "achieved" {
                text(v, "evidence")?;
            }
            if let Some(d) = v.get("deadline").and_then(Value::as_str) {
                date(d)?;
            }
            if let Some(deps) = v.get("depends_on") {
                let deps = deps
                    .as_array()
                    .ok_or_else(|| DomainError::new("invalid_input", "depends_on 必须是数组"))?;
                for dep in deps {
                    let dep = dep
                        .as_str()
                        .ok_or_else(|| DomainError::new("invalid_input", "目标引用必须是字符串"))?;
                    if !existing.iter().any(|r| r.kind == "goal" && r.id == dep) {
                        return fail("reference_not_found", "依赖目标不存在");
                    }
                }
            }
        }
        "route" => {
            text(v, "title")?;
            text(v, "story")?;
            if let Some(stages) = v.get("stages") {
                let stages = stages
                    .as_array()
                    .ok_or_else(|| DomainError::new("invalid_input", "stages 必须是数组"))?;
                for stage in stages {
                    text(stage, "id")?;
                    text(stage, "title")?;
                    validate_goal_refs(stage, existing)?;
                }
            }
            validate_goal_refs(v, existing)?;
        }
        "forecast" => {
            text(v, "title")?;
            date(text(v, "as_of")?)?;
            let rev = integer(v, "baseline_revision")?;
            if rev < 1 || rev > company.revision {
                return fail("invalid_input", "预测基准版本不存在");
            }
            if !v.get("assumptions").is_some_and(Value::is_array)
                || !v.get("scenarios").is_some_and(Value::is_array)
            {
                return fail("invalid_input", "预测需要 assumptions 与 scenarios 数组");
            }
            for scenario in v["scenarios"].as_array().unwrap() {
                text(scenario, "name")?;
                let points = scenario
                    .get("points")
                    .and_then(Value::as_array)
                    .ok_or_else(|| DomainError::new("invalid_input", "情景需要 points"))?;
                for p in points {
                    date(text(p, "date")?)?;
                    if integer(p, "cash_balance_minor")?.unsigned_abs() > MAX_SAFE_MINOR as u64 {
                        return fail("amount_overflow", "预测金额超过精确展示范围");
                    }
                }
            }
            forecast_context::validate(v)?;
        }
        "report" => {
            text(v, "title")?;
            one_of(v, "type", &["weekly", "monthly"])?;
            let from = date(text(v, "period_start")?)?;
            if date(text(v, "period_end")?)? < from {
                return fail("invalid_input", "报告时间范围倒置");
            }
            date(text(v, "as_of")?)?;
            let rev = integer(v, "baseline_revision")?;
            if rev < 1 || rev > company.revision {
                return fail("invalid_input", "报告基准版本不存在");
            }
            text(v, "body")?;
            gameplay::validate_aftermath(v, company, existing)?;
        }
        "file" => {
            relative_path(text(v, "relative_path")?)?;
            text(v, "sha256")?;
            integer(v, "size_bytes")?;
        }
        "skill" => skills::validate_skill(v)?,
        "studio_run" => {
            if company.mode != "simulation" || company.currency != "CNY" {
                return fail(
                    "simulation_required",
                    "The studio chapter requires a CNY simulation company.",
                );
            }
            if text(v, "ruleset")? != studio::RULESET || text(v, "source")? != "builtin:studio-v1" {
                return fail("invalid_input", "Unknown studio ruleset.");
            }
            date(text(v, "started_on")?)?;
            let state: studio::State = serde_json::from_value(v["state"].clone())
                .map_err(|_| DomainError::new("invalid_input", "Invalid studio state."))?;
            studio::validate_state(&state)?;
        }
        "simulation_turn" => {
            if company.mode != "simulation" {
                return fail("simulation_required", "推演回合只能写入模拟公司");
            }
            text(v, "title")?;
            text(v, "action")?;
            text(v, "story")?;
            date(text(v, "occurred_on")?)?;
            let before = integer(v, "before_revision")?;
            let after = integer(v, "after_revision")?;
            if before < 1 || after < before || after > company.revision {
                return fail("invalid_input", "推演回合版本范围无效");
            }
            let outcomes = v
                .get("outcomes")
                .and_then(Value::as_array)
                .ok_or_else(|| DomainError::new("invalid_input", "推演需要 outcomes 数组"))?;
            for outcome in outcomes {
                text(outcome, "title")?;
                one_of(outcome, "status", &["success", "failure", "ongoing"])?;
                text(outcome, "evidence")?;
                if let Some(id) = outcome.get("goal_id").and_then(Value::as_str)
                    && !existing.iter().any(|r| r.kind == "goal" && r.id == id)
                {
                    return fail("reference_not_found", "推演目标引用不存在");
                }
            }
            gameplay::validate_aftermath(v, company, existing)?;
        }
        "decision" => {
            text(v, "title")?;
            text(v, "situation")?;
            one_of(
                v,
                "status",
                &["open", "selected", "processing", "resolved", "cancelled"],
            )?;
            let baseline = integer(v, "baseline_revision")?;
            if baseline < 1 || baseline > company.revision {
                return fail("invalid_input", "经营事件基线版本不存在");
            }
            if let Some(day) = v.get("deadline").and_then(Value::as_str) {
                date(day)?;
            }
            if let Some(day) = v.get("occurred_on").and_then(Value::as_str) {
                date(day)?;
            }
            if let Some(route_id) = v.get("route_id").and_then(Value::as_str) {
                let route = existing
                    .iter()
                    .find(|r| r.kind == "route" && r.id == route_id)
                    .ok_or_else(|| DomainError::new("reference_not_found", "经营路线引用不存在"))?;
                if let Some(stage_id) = v.get("stage_id").and_then(Value::as_str)
                    && !route
                        .data
                        .get("stages")
                        .and_then(Value::as_array)
                        .is_some_and(|stages| stages.iter().any(|s| s["id"] == stage_id))
                {
                    return fail("reference_not_found", "经营阶段引用不存在");
                }
            } else if v.get("stage_id").is_some() {
                return fail("invalid_input", "经营阶段需要关联路线");
            }
            for (field, kind) in [("goal_ids", "goal"), ("file_ids", "file")] {
                if let Some(ids) = v.get(field) {
                    let ids = ids.as_array().ok_or_else(|| {
                        DomainError::new("invalid_input", format!("{field} 必须是数组"))
                    })?;
                    for id in ids {
                        if !id
                            .as_str()
                            .is_some_and(|id| existing.iter().any(|r| r.kind == kind && r.id == id))
                        {
                            return fail("reference_not_found", format!("{field} 引用不存在"));
                        }
                    }
                }
            }
            if v.get("allow_freeform")
                .is_some_and(|value| !value.is_boolean())
            {
                return fail("invalid_input", "allow_freeform 必须是布尔值");
            }
            if let Some(research) = v.get("research") {
                let sources = research
                    .as_array()
                    .ok_or_else(|| DomainError::new("invalid_input", "research 必须是数组"))?;
                for source in sources {
                    text(source, "title")?;
                    let url = text(source, "url")?;
                    if !url.starts_with("https://") && !url.starts_with("http://") {
                        return fail("invalid_input", "信息来源必须是 HTTP(S) URL");
                    }
                    date(text(source, "observed_on")?)?;
                    text(source, "claim")?;
                }
            }
            let options = v
                .get("options")
                .and_then(Value::as_array)
                .ok_or_else(|| DomainError::new("invalid_input", "经营事件需要 options 数组"))?;
            if options.is_empty() || options.len() > 12 {
                return fail("invalid_input", "经营事件需要 1 至 12 个方案");
            }
            let mut option_ids = BTreeSet::new();
            for option in options {
                gameplay::validate_option(option)?;
                let id = text(option, "id")?;
                identifier(id)?;
                if !option_ids.insert(id) {
                    return fail("invalid_input", "经营方案标识不能重复");
                }
                text(option, "title")?;
                for field in ["description", "risk"] {
                    if let Some(value) = option.get(field)
                        && !value.is_string()
                    {
                        return fail("invalid_input", format!("{field} 必须是文字"));
                    }
                }
                if let Some(cost) = option.get("estimated_cost_minor")
                    && cost
                        .as_i64()
                        .is_none_or(|n| !(0..=MAX_SAFE_MINOR).contains(&n))
                {
                    return fail("invalid_input", "estimated_cost_minor 必须是非负整数");
                }
                if let Some(assumptions) = option.get("assumptions") {
                    let assumptions = assumptions.as_array().ok_or_else(|| {
                        DomainError::new("invalid_input", "assumptions 必须是数组")
                    })?;
                    if assumptions
                        .iter()
                        .any(|item| item.as_str().is_none_or(|s| s.trim().is_empty()))
                    {
                        return fail("invalid_input", "方案假设必须是文字");
                    }
                }
            }
            gameplay::validate_experience(v, company, existing)?;
            let status = text(v, "status")?;
            if status == "selected" || status == "processing" || status == "resolved" {
                let selection = v
                    .get("selection")
                    .ok_or_else(|| DomainError::new("invalid_input", "选择内容不存在"))?;
                text(selection, "text")?;
                text(selection, "selected_at")?;
                text(selection, "actor")?;
                if let Some(id) = selection.get("option_id")
                    && !id.as_str().is_some_and(|id| option_ids.contains(id))
                {
                    return fail("invalid_input", "所选方案不存在");
                }
            }
            if status == "processing" || v.get("claim").is_some() {
                let claim = v
                    .get("claim")
                    .ok_or_else(|| DomainError::new("invalid_input", "事件接手记录不存在"))?;
                text(claim, "claimed_at")?;
                text(claim, "actor")?;
                for field in ["company_revision", "selection_company_revision"] {
                    let revision = integer(claim, field)?;
                    if revision < baseline || revision > company.revision {
                        return fail("invalid_input", "事件接手公司版本不存在");
                    }
                }
            }
            if status == "resolved" {
                let resolution = v
                    .get("resolution")
                    .ok_or_else(|| DomainError::new("invalid_input", "结算内容不存在"))?;
                text(resolution, "summary")?;
                text(resolution, "resolved_at")?;
                let revision = integer(resolution, "company_revision")?;
                if revision < baseline || revision > company.revision {
                    return fail("invalid_input", "结算公司版本不存在");
                }
                if !resolution.get("result_refs").is_some_and(Value::is_array) {
                    return fail("invalid_input", "结算需要 result_refs 数组");
                }
            }
            if status == "cancelled" {
                let cancellation = v
                    .get("cancellation")
                    .ok_or_else(|| DomainError::new("invalid_input", "取消依据不存在"))?;
                text(cancellation, "reason")?;
                text(cancellation, "cancelled_at")?;
            }
        }
        _ => return fail("unknown_kind", "未知经营记录类型"),
    }
    Ok(())
}
fn validate_goal_refs(v: &Value, existing: &[Record]) -> Result<()> {
    if let Some(ids) = v.get("goal_ids") {
        let ids = ids
            .as_array()
            .ok_or_else(|| DomainError::new("invalid_input", "goal_ids 必须是数组"))?;
        for id in ids {
            if !id
                .as_str()
                .is_some_and(|id| existing.iter().any(|r| r.kind == "goal" && r.id == id))
            {
                return fail("reference_not_found", "路线目标引用不存在");
            }
        }
    }
    Ok(())
}
pub fn validate_goal_cycles(records: &[Record]) -> Result<()> {
    fn visit(
        id: &str,
        goals: &BTreeMap<String, Vec<String>>,
        path: &mut BTreeSet<String>,
        done: &mut BTreeSet<String>,
    ) -> Result<()> {
        if done.contains(id) {
            return Ok(());
        }
        if !path.insert(id.into()) {
            return fail("invalid_input", "目标依赖存在循环");
        }
        if let Some(deps) = goals.get(id) {
            for dep in deps {
                visit(dep, goals, path, done)?;
            }
        }
        path.remove(id);
        done.insert(id.into());
        Ok(())
    }
    let goals: BTreeMap<_, _> = records
        .iter()
        .filter(|r| r.kind == "goal")
        .map(|r| {
            (
                r.id.clone(),
                r.data
                    .get("depends_on")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default(),
            )
        })
        .collect();
    let mut done = BTreeSet::new();
    for id in goals.keys() {
        visit(id, &goals, &mut BTreeSet::new(), &mut done)?;
    }
    Ok(())
}

pub fn compute_metrics(company: &Company, records: &[Record], as_of: NaiveDate) -> Result<Value> {
    let first = as_of.with_day(1).unwrap();
    let accounts: Vec<_> = records.iter().filter(|r| r.kind == "account").collect();
    let mut cash: i128 = 0;
    let mut inflow: i128 = 0;
    let mut outflow: i128 = 0;
    for account in &accounts {
        let open = date(
            account
                .data
                .get("opening_on")
                .and_then(Value::as_str)
                .unwrap_or(&company.opening_date),
        )?;
        if open <= as_of {
            cash += integer(&account.data, "opening_balance_minor")? as i128;
        }
    }
    let mut monthly: BTreeMap<String, (i128, i128)> = BTreeMap::new();
    for n in 0..6 {
        let month = first.checked_sub_months(Months::new(n)).unwrap();
        monthly.insert(month.format("%Y-%m").to_string(), (0, 0));
    }
    for r in records.iter().filter(|r| r.kind == "transaction") {
        let d = date(text(&r.data, "occurred_on")?)?;
        if d > as_of {
            continue;
        }
        let amount = integer(&r.data, "amount_minor")? as i128;
        let is_in = text(&r.data, "direction")? == "inflow";
        cash += if is_in { amount } else { -amount };
        if text(&r.data, "category")? == "operating" {
            if d >= first {
                if is_in {
                    inflow += amount;
                } else {
                    outflow += amount;
                }
            }
            if let Some(t) = monthly.get_mut(&d.format("%Y-%m").to_string()) {
                if is_in {
                    t.0 += amount;
                } else {
                    t.1 += amount;
                }
            }
        }
    }
    let convert = |n: i128| -> Result<i64> {
        if n.abs() > MAX_SAFE_MINOR as i128 {
            return fail("amount_overflow", "金额合计超过精确展示范围");
        }
        i64::try_from(n).map_err(|_| DomainError::new("amount_overflow", "金额合计超过支持范围"))
    };
    let subscriptions: Vec<_> = records
        .iter()
        .filter(|r| r.kind == "subscription")
        .collect();
    let mrr_known = company.subscription_tracking || !subscriptions.is_empty();
    let mut mrr_units: i128 = 0;
    for r in subscriptions {
        let data = &r.data;
        let start = date(text(data, "starts_on")?)?;
        let end = data
            .get("ends_on")
            .and_then(Value::as_str)
            .map(date)
            .transpose()?;
        if start <= as_of
            && end.is_none_or(|d| d > as_of)
            && ["active", "past_due"].contains(&text(data, "status")?)
        {
            let net = integer(data, "amount_minor")?
                - data.get("tax_minor").and_then(Value::as_i64).unwrap_or(0)
                - data
                    .get("recurring_discount_minor")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
            mrr_units += i128::from(net) * 12 / i128::from(integer(data, "interval_months")?);
        }
    }
    let mrr = if mrr_known {
        Some(convert((mrr_units + 6) / 12)?)
    } else {
        None
    };
    let arr = mrr.map(|m| convert(i128::from(m) * 12)).transpose()?;
    let enough = date(&company.opening_date)? <= first.checked_sub_months(Months::new(3)).unwrap()
        && !accounts.is_empty()
        && accounts.iter().all(|a| {
            date(
                a.data
                    .get("opening_on")
                    .and_then(Value::as_str)
                    .unwrap_or(&company.opening_date),
            )
            .is_ok_and(|d| d <= first.checked_sub_months(Months::new(3)).unwrap())
        });
    let mut burn: i128 = 0;
    for n in 1..=3 {
        let key = first
            .checked_sub_months(Months::new(n))
            .unwrap()
            .format("%Y-%m")
            .to_string();
        if let Some((i, o)) = monthly.get(&key) {
            burn += o - i;
        }
    }
    let (runway, status) = if !enough {
        (None, "insufficient_data")
    } else if burn <= 0 {
        (None, "not_depleting")
    } else {
        (Some((cash.max(0) as f64) / (burn as f64 / 3.)), "available")
    };
    let paid = records
        .iter()
        .filter(|r| r.kind == "customer" && r.data["status"] == "paying")
        .count() as f64;
    let released = records
        .iter()
        .filter(|r| {
            r.kind == "business"
                && ["released", "completed"].contains(&r.data["status"].as_str().unwrap_or(""))
        })
        .count() as f64;
    let mut progress: BTreeMap<String, Value> = BTreeMap::new();
    for r in records.iter().filter(|r| r.kind == "goal") {
        let target = r.data.get("target").and_then(Value::as_f64);
        let metric = r
            .data
            .get("metric")
            .and_then(Value::as_str)
            .unwrap_or("manual");
        let current = match metric {
            "mrr" => mrr.map(|n| n as f64),
            "cash_balance" => {
                if accounts.is_empty() {
                    None
                } else {
                    Some(cash as f64)
                }
            }
            "paid_customers" => Some(paid),
            "business_released" => Some(released),
            _ => None,
        };
        let declared = r.data["status"].as_str().unwrap_or("planned");
        let achieved = match (current, target) {
            (Some(c), Some(t)) => c >= t,
            _ => metric == "manual" && declared == "achieved",
        };
        let status = if ["archived", "blocked", "planned"].contains(&declared) {
            declared
        } else if achieved {
            "achieved"
        } else {
            "active"
        };
        progress.insert(r.id.clone(),json!({"id":r.id,"current":current,"target":target,"progress":current.zip(target).map(|(c,t)|(c/t).clamp(0.,1.)),"status":status}));
    }
    for _ in 0..records.len().min(100) {
        let mut changed = false;
        for r in records.iter().filter(|r| r.kind == "goal") {
            let blocked = r
                .data
                .get("depends_on")
                .and_then(Value::as_array)
                .is_some_and(|deps| {
                    deps.iter()
                        .filter_map(Value::as_str)
                        .any(|dep| progress.get(dep).is_none_or(|p| p["status"] != "achieved"))
                });
            if blocked && progress[&r.id]["status"] == "achieved" {
                progress.get_mut(&r.id).unwrap()["status"] = json!("blocked");
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    Ok(
        json!({"as_of":as_of.to_string(),"currency":company.currency,"mode":company.mode,"cash_balance_minor":if accounts.is_empty(){None}else{Some(convert(cash)?)},"operating_inflow_minor":if accounts.is_empty(){None}else{Some(convert(inflow)?)},"operating_outflow_minor":if accounts.is_empty(){None}else{Some(convert(outflow)?)},"net_operating_cashflow_minor":if accounts.is_empty(){None}else{Some(convert(inflow-outflow)?)},"mrr_minor":mrr,"arr_minor":arr,"runway_months":runway,"runway_status":status,"monthly_cashflow":if accounts.is_empty(){vec![]}else{monthly.into_iter().map(|(month,(i,o))|Ok(json!({"month":month,"inflow_minor":convert(i)?,"outflow_minor":convert(o)?,"net_minor":convert(i-o)?}))).collect::<Result<Vec<_>>>()?},"goal_progress":progress.into_values().collect::<Vec<_>>(),"data_revision":company.revision}),
    )
}

pub fn operation_scope(op: &str) -> Option<&'static str> {
    if let Some(scope) = network::operation_scope(op)
        .or_else(|| game_systems::operation_scope(op))
        .or_else(|| agent_link::operation_scope(op))
        .or_else(|| studio_contract::operation_scope(op))
    {
        return Some(scope);
    }
    match op {
        "company.create" | "simulation.fork" => Some("admin"),
        "company.update" | "simulation.advance" | "decision.put" | "decision.select"
        | "decision.claim" | "decision.resolve" | "decision.cancel" => Some("company.write"),
        "account.put"
        | "transaction.record"
        | "transaction.reverse"
        | "transaction.transfer"
        | "subscription.put" => Some("finance.write"),
        "goal.put" | "route.put" => Some("goals.write"),
        "report.put" | "forecast.put" | "simulation_turn.put" => Some("reports.write"),
        "file.put" | "file.sync" => Some("files.write"),
        "skill.put" | "skill.delete" | "skill.set_enabled" => Some("skills.write"),
        "business.put" | "customer.put" | "resource.put" => Some("company.write"),
        _ => None,
    }
}

pub fn is_real_decision_result(kind: &str) -> bool {
    matches!(
        kind,
        "report"
            | "transaction"
            | "business"
            | "customer"
            | "subscription"
            | "goal"
            | "resource"
            | "account"
    )
}

/// A decision's resource context can survive report writing and unrelated
/// document maintenance. Ledger, business and linked knowledge changes still
/// require the Agent to review its proposed choices.
pub fn decision_change_affects_context(
    decision: &Value,
    operation: &str,
    resource_id: Option<&str>,
) -> bool {
    let kind = operation.split('.').next().unwrap_or(operation);
    match kind {
        "company" => operation == "company.update",
        "simulation" => operation == "simulation.advance",
        "file" => resource_id.is_some_and(|id| {
            decision
                .get("file_ids")
                .and_then(Value::as_array)
                .is_some_and(|files| files.iter().any(|file| file.as_str() == Some(id)))
        }),
        "account" | "transaction" | "business" | "customer" | "resource" | "subscription"
        | "goal" | "route" | "simulation_turn" => true,
        _ => false,
    }
}

fn operation_contract(op: &str) -> Value {
    if let Some(contract) = network::operation_contract(op)
        .or_else(|| game_systems::operation_contract(op))
        .or_else(|| agent_link::operation_contract(op))
        .or_else(|| studio_contract::operation_contract(op))
    {
        return contract;
    }
    if let Some((service, description, path, scope)) = match op {
        "company.snapshot" => Some((
            "company",
            "读取当前公司状态与经营数据",
            "/api/v1/companies/{company_id}/snapshot",
            "company.read",
        )),
        "company.records" => Some((
            "company",
            "分页查询记录或单件历史",
            "/api/v1/companies/{company_id}/records?kind={kind}&id={id}&history={history}&offset={offset}",
            "company.read",
        )),
        "company.history" => Some((
            "company",
            "读取固定公司版本的完整经营状态",
            "/api/v1/companies/{company_id}/history/{revision}",
            "company.read",
        )),
        "company.changes" => Some((
            "company",
            "等待公司已提交数据变化",
            "/api/v1/companies/{company_id}/changes?after_revision={revision}&wait_ms={milliseconds}",
            "company.read",
        )),
        "company.play" => Some((
            "company",
            "读取当前事件、方案、等待状态和最新结果",
            "/api/v1/companies/{company_id}/play",
            "company.read",
        )),
        "company.presentation" => Some((
            "company",
            "读取所选经营页面的状态和快捷链接",
            "/api/v1/companies/{company_id}/presentation?view={view}",
            "company.read",
        )),
        "company.capture" => Some((
            "company",
            "获取经营页面的实际截图和快捷链接",
            "/api/v1/companies/{company_id}/presentation/capture",
            "company.read",
        )),
        "finance.metrics" => Some((
            "finance",
            "计算当前已登记数据的指标",
            "/api/v1/companies/{company_id}/metrics?as_of={YYYY-MM-DD}",
            "company.read",
        )),
        "files.download" => Some((
            "files",
            "取回文件的指定版本原件",
            "/api/v1/companies/{company_id}/files/{file_id}?revision={revision}",
            "files.read",
        )),
        "files.preview" => Some((
            "files",
            "读取资料的安全预览与原件链接",
            "/api/v1/companies/{company_id}/files/{file_id}/preview?revision={revision}",
            "files.read",
        )),
        "skills.list" => Some((
            "skills",
            "按元数据发现经营 Skills",
            "/api/v1/companies/{company_id}/skills",
            "company.read",
        )),
        "skills.read" => Some((
            "skills",
            "按需读取完整 SKILL.md 与固定版本",
            "/api/v1/companies/{company_id}/skills/{skill_id}?revision={revision}",
            "company.read",
        )),
        "skills.history" => Some((
            "skills",
            "读取 Skill 的历史版本",
            "/api/v1/companies/{company_id}/skills/{skill_id}/history",
            "company.read",
        )),
        "plugins.list" => Some((
            "plugins",
            "查看实际插件目录与安装状态",
            "/api/v1/companies/{company_id}/plugins",
            "company.read",
        )),
        _ => None,
    } {
        let mut contract = json!({"id":op,"operation":op,"service":service,"description":description,"kind":"read","scope":scope,"http":{"method":if op=="company.capture"{"POST"}else{"GET"},"path":path},"input_schema":{"type":"object","properties":{"company_id":{"type":"string"},"kind":{"type":"string"},"id":{"type":"string"},"history":{"type":"boolean"},"offset":{"type":"integer","minimum":0},"revision":{"type":"integer","minimum":1},"as_of":{"type":"string"}}},"result":{"format":if op=="files.download"{"binary"}else{"json"}},"rules":["使用获得授权的公司标识","记录分页响应包含 total、next_offset、has_more","历史状态按公司版本读取；日期筛选作用于当前登记的经营生命周期"],"errors":["unauthorized","forbidden","not_found","invalid_input"]});
        if op == "company.changes" {
            contract["input_schema"]["required"] = json!(["after_revision"]);
            contract["input_schema"]["properties"]["after_revision"] =
                json!({"type":"integer","minimum":0});
            contract["input_schema"]["properties"]["wait_ms"] =
                json!({"type":"integer","minimum":0,"maximum":25000,"default":25000});
            contract["result"] =
                json!({"company_id":"string","changed":"boolean","revision":"integer"});
            contract["rules"] = json!([
                "空闲时只读取变化游标；发现 CLI、导入或 API 提交后返回最新版本，等待期间持续检查凭据与公司授权"
            ]);
        }
        if ["company.presentation", "company.capture"].contains(&op) {
            contract["input_schema"]["properties"]["view"] = json!({"enum":["overview","operations","growth","simulation","reports","files","chronicle","plugins"],"default":"overview"});
            contract["rules"] = json!([
                "快捷链接不包含凭据；截图来自获授权页面的实际渲染，不能用生成图片替代",
                "截图能力未配置时返回 screenshot_unavailable，同时提供可读取状态与快捷链接"
            ]);
        }
        if op == "company.play" {
            contract["result"] = json!({"company_id":"string","data_revision":"integer","phase":["awaiting_choice","awaiting_agent","agent_processing","awaiting_event"],"current_event":"decision record with selection and claim request templates, or null","latest_result":"fixed result record versions and before/after metrics, or null","state":"current deterministic metrics and goal progress","links":"company-scoped menu links"});
            contract["result"]["story_context"] = json!({"recent_choices":"at most five newest settled choices with exact source_ref","cast":"saved identities, roles and concerns with source_ref","relationships":"settled relationship changes with exact source_ref","open_commitments":"pending saved commitments with stable origin and exact source_ref","chapter":"current saved chapter and settled closure, or null"});
            contract["result"]["presentation"] = json!(
                "current experience, options and exact source_ref; null for a legacy event without experience"
            );
            contract["result"]["cadence"] = json!({"chapter":"saved chapter or null","next_beat":["choose","await_agent","await_result","chapter_review","prepare_event"]});
        }
        if op == "company.presentation" {
            contract["result"] = json!({"view":"string","href":"credential-free relative company link","capture":"configured boolean, reason and supported views","play":"company.play result"});
        }
        if op == "company.capture" {
            contract["result"] = json!({"view":"string","href":"credential-free relative company link","image":{"mime_type":"image/png","base64":"actual PNG bytes","width":"integer","height":"integer"},"data_revision":"rendered snapshot cursor","captured_state":"deterministic metrics at rendered revision","current_data_revision":"latest snapshot cursor","changed_since_capture":"boolean","play":"latest company.play result"});
            contract["errors"] = json!([
                "unauthorized",
                "forbidden",
                "not_found",
                "invalid_input",
                "screenshot_unavailable",
                "screenshot_busy",
                "screenshot_failed"
            ]);
        }
        if op.starts_with("skills.") {
            contract["rules"] = json!([
                "先读取 Skill 元数据，仅按当前任务展开所需完整文档",
                "内置与插件 Skill 只读；复制为公司的独立 Skill 后可以维护",
                "只应用 enabled、effective 且未 deleted 的规范；Skill 正文不授予数据或外部行动权限"
            ]);
            contract["result"] = json!({"format":"json","document":"完整 SKILL.md 按需返回","metadata":"name, description, source, enabled, revision"});
        }
        if op == "files.preview" {
            contract["result"] = json!({"format":"json","previewable":"boolean","format_values":["markdown","text","image","pdf","unsupported"],"content_text":"可安全呈现的 UTF-8 正文","content_url":"准确版本原始字节接口","download_url":"准确版本原件"});
            contract["rules"] = json!([
                "预览读取已登记的不可变内容版本；Markdown 与文本上限 1 MiB，PNG、JPEG、GIF、WebP、PDF 上限 10 MiB",
                "HTML、SVG 和其他不支持格式只提供下载；Markdown 禁止执行原始 HTML"
            ]);
        }
        return contract;
    }

    let (service, description, required, mut properties) = match op {
        "company.create" => (
            "company",
            "建立真实或模拟公司",
            vec!["name", "currency", "timezone", "opening_date"],
            json!({"name":{"type":"string"},"currency":{"type":"string"},"timezone":{"type":"string"},"opening_date":{"type":"string"},"mode":{"enum":["real","simulation"]},"subscription_tracking":{"type":"boolean"}}),
        ),
        "company.update" => (
            "company",
            "修改公司资料，保留公司身份与模式",
            vec!["name"],
            json!({"name":{"type":"string"},"description":{"type":"string"},"subscription_tracking":{"type":"boolean"}}),
        ),
        "simulation.advance" => (
            "simulation",
            "推进模拟公司的经营日期",
            vec!["as_of"],
            json!({"as_of":{"type":"string"}}),
        ),
        "simulation.fork" => (
            "simulation",
            "从当前经营状态建立独立模拟公司",
            vec!["name"],
            json!({"name":{"type":"string"}}),
        ),
        "decision.put" => (
            "decision",
            "创建或修订尚未选择的经营事件",
            vec!["id", "title", "situation", "options"],
            json!({"id":{"type":"string"},"title":{"type":"string"},"situation":{"type":"string"},"baseline_revision":{"type":"integer","minimum":1},"route_id":{"type":"string"},"stage_id":{"type":"string"},"goal_ids":{"type":"array","items":{"type":"string"}},"file_ids":{"type":"array","items":{"type":"string"}},"options":{"type":"array","minItems":1,"maxItems":12,"items":{"type":"object","required":["id","title"],"properties":{"id":{"type":"string"},"title":{"type":"string"},"description":{"type":"string"},"risk":{"type":"string"},"assumptions":{"type":"array","items":{"type":"string"}},"estimated_cost_minor":{"type":"integer","minimum":0}}}},"allow_freeform":{"type":"boolean"},"status":{"enum":["open"]},"deadline":{"type":"string"},"occurred_on":{"type":"string"}}),
        ),
        "decision.select" => (
            "decision",
            "保存玩家选择并等待 Agent 处理，不执行经营行动",
            vec!["id"],
            json!({"id":{"type":"string"},"option_id":{"type":"string"},"choice":{"type":"string","description":"方案标识或完整名称；忽略名称大小写与空白；只接受唯一准确匹配"},"text":{"type":"string"}}),
        ),
        "decision.resolve" => (
            "decision",
            "用精确结果版本结束已选择事件",
            vec!["id", "summary", "result_refs"],
            json!({"id":{"type":"string"},"summary":{"type":"string"},"company_revision":{"type":"integer","minimum":1},"result_refs":{"type":"array","minItems":1,"items":{"type":"object","required":["kind","id","revision"],"properties":{"kind":{"type":"string"},"id":{"type":"string"},"revision":{"type":"integer","minimum":1}}}}}),
        ),
        "decision.claim" => (
            "decision",
            "接手已保存的玩家选择并锁定方案，不授予外部行动权限",
            vec!["id"],
            json!({"id":{"type":"string"}}),
        ),
        "decision.cancel" => (
            "decision",
            "取消未结束的经营事件并保留历史",
            vec!["id", "reason"],
            json!({"id":{"type":"string"},"reason":{"type":"string"}}),
        ),
        "account.put" => (
            "finance",
            "登记账户及期初余额",
            vec!["id", "name", "opening_balance_minor"],
            json!({"id":{"type":"string"},"name":{"type":"string"},"opening_balance_minor":{"type":"integer"},"opening_on":{"type":"string"}}),
        ),
        "transaction.record" => (
            "finance",
            "追加一笔真实或模拟的资金记录",
            vec![
                "id",
                "account_id",
                "amount_minor",
                "direction",
                "category",
                "occurred_on",
                "source",
            ],
            json!({"id":{"type":"string"},"account_id":{"type":"string"},"amount_minor":{"type":"integer","minimum":1},"direction":{"enum":["inflow","outflow"]},"category":{"enum":["operating","capital","financing"]},"occurred_on":{"type":"string"},"source":{"type":"object","required":["kind","reference"]}}),
        ),
        "transaction.reverse" => (
            "finance",
            "冲销已有资金记录并保留原始依据",
            vec!["transaction_id", "reason", "occurred_on"],
            json!({"transaction_id":{"type":"string"},"reason":{"type":"string"},"occurred_on":{"type":"string"}}),
        ),
        "transaction.transfer" => (
            "finance",
            "在同一事务中记录账户间的两条转账记录",
            vec![
                "from_account_id",
                "to_account_id",
                "amount_minor",
                "occurred_on",
                "source",
            ],
            json!({"from_account_id":{"type":"string"},"to_account_id":{"type":"string"},"amount_minor":{"type":"integer"},"occurred_on":{"type":"string"},"source":{"type":"object"}}),
        ),
        "business.put" => (
            "business",
            "维护产品或项目",
            vec!["id", "name", "type", "status"],
            json!({"id":{"type":"string"},"name":{"type":"string"},"type":{"enum":["product","project"]},"status":{"enum":["planned","active","released","completed","paused","archived"]}}),
        ),
        "customer.put" => (
            "business",
            "维护客户经营档案",
            vec!["id", "name", "status"],
            json!({"id":{"type":"string"},"name":{"type":"string"},"status":{"enum":["lead","active","paying","inactive","archived"]}}),
        ),
        "resource.put" => (
            "business",
            "维护参与者、工具和服务资源",
            vec!["id", "name", "type", "status"],
            json!({"id":{"type":"string"},"name":{"type":"string"},"type":{"enum":["human","agent","tool","service"]},"status":{"type":"string"}}),
        ),
        "subscription.put" => (
            "finance",
            "维护固定周期订阅，收款另行登记",
            vec![
                "id",
                "customer_id",
                "amount_minor",
                "interval_months",
                "status",
                "starts_on",
            ],
            json!({"id":{"type":"string"},"customer_id":{"type":"string"},"amount_minor":{"type":"integer"},"interval_months":{"enum":[1,12]},"status":{"enum":["active","past_due","trial","free","unpaid","cancelled"]},"starts_on":{"type":"string"},"tax_minor":{"type":"integer"},"recurring_discount_minor":{"type":"integer"}}),
        ),
        "goal.put" => (
            "growth",
            "维护公司级经营目标，达成依据可追溯",
            vec!["id", "title", "outcome", "status"],
            json!({"id":{"type":"string"},"title":{"type":"string"},"outcome":{"type":"string"},"status":{"enum":["planned","active","achieved","blocked","archived"]},"metric":{"enum":["manual","mrr","cash_balance","paid_customers","business_released"]},"target":{"type":"number"},"evidence":{"type":"string"},"depends_on":{"type":"array","items":{"type":"string"}}}),
        ),
        "route.put" => (
            "growth",
            "维护版本化经营故事与成长路线",
            vec!["id", "title", "story"],
            json!({"id":{"type":"string"},"title":{"type":"string"},"story":{"type":"string"},"stages":{"type":"array"},"goal_ids":{"type":"array"}}),
        ),
        "forecast.put" => (
            "reports",
            "保存情景预测与其基准和假设",
            vec!["id", "title", "as_of", "assumptions", "scenarios"],
            json!({"id":{"type":"string"},"title":{"type":"string"},"as_of":{"type":"string"},"baseline_revision":{"type":"integer"},"assumptions":{"type":"array"},"scenarios":{"type":"array"}}),
        ),
        "report.put" => (
            "reports",
            "保存有截止点的周报或月报",
            vec![
                "id",
                "title",
                "type",
                "period_start",
                "period_end",
                "as_of",
                "body",
            ],
            json!({"id":{"type":"string"},"title":{"type":"string"},"type":{"enum":["weekly","monthly"]},"period_start":{"type":"string"},"period_end":{"type":"string"},"as_of":{"type":"string"},"baseline_revision":{"type":"integer"},"body":{"type":"string"}}),
        ),
        "simulation_turn.put" => (
            "simulation",
            "展示一次模拟选择的变化与任务结果",
            vec!["id", "title", "action", "story", "outcomes", "occurred_on"],
            json!({"id":{"type":"string"},"title":{"type":"string"},"action":{"type":"string"},"story":{"type":"string"},"before_revision":{"type":"integer"},"after_revision":{"type":"integer"},"outcomes":{"type":"array"},"occurred_on":{"type":"string"},"assumptions":{"type":"array"}}),
        ),
        "file.put" => (
            "files",
            "登记文件内容与可恢复版本",
            vec!["id", "relative_path", "content_base64"],
            json!({"id":{"type":"string"},"relative_path":{"type":"string"},"content_base64":{"type":"string"},"title":{"type":"string"},"category":{"type":"string"}}),
        ),
        "file.sync" => (
            "files",
            "显式登记公司目录中的文件修改",
            vec!["relative_path"],
            json!({"relative_path":{"type":"string"},"id":{"type":"string"}}),
        ),
        "skill.put" => (
            "skills",
            "创建或修订公司的经营 Skill",
            vec!["id", "name", "title", "description", "body"],
            json!({"id":{"type":"string"},"name":{"type":"string"},"title":{"type":"string"},"description":{"type":"string"},"body":{"type":"string","maxLength":131072},"enabled":{"type":"boolean"},"type":{"enum":["procedure","experience"]},"tags":{"type":"array","items":{"type":"string"}},"modes":{"type":"array","items":{"enum":["real","simulation"]}},"source_refs":{"type":"array","items":{"type":"object","required":["kind","id","revision"],"properties":{"kind":{"type":"string"},"id":{"type":"string"},"revision":{"type":"integer","minimum":1}}}}}),
        ),
        "skill.set_enabled" => (
            "skills",
            "启停公司 Skill 并保留正文和历史",
            vec!["id", "enabled"],
            json!({"id":{"type":"string"},"enabled":{"type":"boolean"}}),
        ),
        "skill.delete" => (
            "skills",
            "删除公司 Skill 并保留可恢复历史",
            vec!["id"],
            json!({"id":{"type":"string"},"reason":{"type":"string"}}),
        ),
        _ => ("company", "", vec![], json!({})),
    };
    let ids = json!({"type":"array","items":{"type":"string"}});
    if op == "decision.put" {
        properties["experience"] = gameplay::experience_schema();
        let option_properties = &mut properties["options"]["items"]["properties"];
        for field in ["benefit", "tradeoff", "commitment"] {
            option_properties[field] = json!({"type":"string","maxLength":1000});
        }
        option_properties["feedback_on"] = json!({"type":"string","format":"date"});
    }
    if op == "forecast.put" {
        properties["context_sources"] = forecast_context::sources_schema();
        properties["analysis_method"] = json!({"type":"string","minLength":1,"maxLength":2000});
        properties["limitations"] =
            json!({"type":"array","maxItems":8,"items":{"type":"string","maxLength":1000}});
    }
    if ["report.put", "simulation_turn.put"].contains(&op) {
        properties["aftermath"] = gameplay::aftermath_schema();
        properties["decision_id"] = json!({"type":"string"});
    }
    let source = json!({"type":"object","required":["kind","reference"],"properties":{"kind":{"type":"string"},"reference":{"type":"string"}}});
    match op {
        "company.create" => {
            properties["id"] = json!({"type":"string"});
            properties["description"] = json!({"type":"string"});
            properties["simulation_date"] = json!({"type":"string","description":"模拟模式当前经营日期 YYYY-MM-DD，默认 opening_date"});
        }
        "route.put" => {
            properties["goal_ids"] = ids.clone();
            properties["stages"] = json!({"type":"array","items":{"type":"object","required":["id","title"],"properties":{"id":{"type":"string"},"title":{"type":"string"},"goal_ids":ids}}});
        }
        "forecast.put" => {
            properties["assumptions"] = json!({"type":"array","items":{"type":"string"}});
            properties["scenarios"] = json!({"type":"array","items":{"type":"object","required":["name","points"],"properties":{"name":{"type":"string"},"points":{"type":"array","items":{"type":"object","required":["date","cash_balance_minor"],"properties":{"date":{"type":"string"},"cash_balance_minor":{"type":"integer"}}}}}}});
        }
        "simulation_turn.put" => {
            properties["outcomes"] = json!({"type":"array","items":{"type":"object","required":["title","status","evidence"],"properties":{"goal_id":{"type":"string"},"title":{"type":"string"},"status":{"enum":["success","failure","ongoing"]},"evidence":{"type":"string"}}}});
            properties["assumptions"] = json!({"type":"array","items":{"type":"string"}});
        }
        "transaction.record" | "transaction.transfer" => {
            properties["source"] = source;
            properties["description"] = json!({"type":"string"});
        }
        "subscription.put" => {
            properties["ends_on"] = json!({"type":["string","null"]});
        }
        "goal.put" => {
            properties["deadline"] = json!({"type":["string","null"]});
            properties["stage"] = json!({"type":"string"});
        }
        _ => {}
    }
    if op == "decision.put" {
        properties["research"] = json!({"type":"array","items":{"type":"object","required":["title","url","observed_on","claim"],"properties":{"title":{"type":"string"},"url":{"type":"string"},"observed_on":{"type":"string"},"claim":{"type":"string"}}}});
        properties["calculation"] = json!({"type":"object"});
    }
    for field in ["baseline_revision", "after_revision", "company_revision"] {
        if let Some(schema) = properties.get_mut(field) {
            schema["description"] =
                json!("省略时平台在提交事务中固定当前公司快照；显式值仍按合同校验");
        }
    }
    if op == "simulation_turn.put" {
        properties["decision_id"] = json!({"type":"string","description":"本公司已选择事件；提供时平台自动固定 before_revision 为该选择的公司快照"});
    }
    if ["simulation_turn.put", "forecast.put", "report.put"].contains(&op) {
        properties["research"] = json!({"type":"array","items":{"type":"object","required":["title","url","observed_on","claim"],"properties":{"title":{"type":"string"},"url":{"type":"string","description":"现实信息来源的 HTTP(S) URL"},"observed_on":{"type":"string","description":"采集日期 YYYY-MM-DD"},"claim":{"type":"string","description":"来源能支持的事实；不将假设写成事实"}}}});
        properties["calculation"] = json!({"type":"object","properties":{"inputs":{"type":["object","array","string"]},"method":{"type":"string"},"result":{"type":["string","number"]}}});
    }
    let mut rules = vec![
        "使用稳定 request_id，结果未知时原样重试",
        "更新已有记录必须提供当前 expected_revision",
        "所有金额使用公司币种的整数最小货币单位",
    ];
    if op.starts_with("decision.") {
        rules.extend(["每家公司同时最多一个未结束事件；选择只保存意向，不改变账目或执行外部行动","选择、结算和取消需要当前事件版本；结算引用本公司已提交结果的准确版本","真实公司结算需引用选择后提交的实际经营记录或报告，模拟公司需引用推演回合；预测或文件不能单独作为结果"]);
        rules.push("每次事件写入后用回执 record.revision 作为下一次 expected_revision，包括改选和接手；不要使用 company_revision 代替事件版本");
    }
    if op == "decision.claim" {
        rules.push("接手使用当前事件 expected_revision；接手后不能改选，接手状态不代表外部行动已执行或获得授权");
    }
    if op == "decision.put" {
        rules.push("公司快照版本是历史与变化游标；报告、预测和未关联文件更新不会使选项失效，经营记录、公司日期和关联文件变化需要重新核对事件");
    }
    if op == "decision.select" {
        rules.push("option_id、choice、text 只能提供一个；choice 只准确匹配方案标识或完整名称，含糊表达由 Agent 澄清");
        rules.push("Agent 接手前可用最新事件版本改选；已有相关经营写入时停止改选并让 Agent 核对，历史选择保留");
    }
    if op.starts_with("skill.") {
        rules.push("修改经营 Skills 需要明确 skills.write；company.write 不包含此权限");
        rules.push("正文只存 Markdown，不填写 YAML frontmatter；完整 SKILL.md 从 name、description 和正文生成");
        rules.push("删除保留历史；skill.put 用最新 expected_revision 可恢复；只在用户要求时将经验总结持久化");
    }
    if op == "skill.put" {
        rules.push("copy_from 仅用于新建，只接受准确内置或插件模板引用；读取权限同时需要 company.read；服务核对摘要并保存只读来源快照，改写正文不改变来源");
    }
    let mut contract = json!({"id":op,"operation":op,"service":service,"description":description,"kind":"write","scope":operation_scope(op),"http":{"method":"POST","path":if op=="company.create"{"/api/v1/companies".to_string()}else{format!("/api/v1/companies/{{company_id}}/operations/{op}")}},"input_schema":{"type":"object","required":["request_id","input"],"properties":{"request_id":{"type":"string"},"expected_revision":{"type":["integer","null"]},"input":{"type":"object","required":required,"properties":properties}}},"result":{"status":"committed","replayed":"boolean","record":"optional","company":"optional","company_revision":"integer"},"errors":["invalid_input","forbidden","reference_not_found","revision_conflict","request_id_conflict","active_decision_exists","invalid_result_reference"],"rules":rules,"guide":format!("/api/v1/agent-guide?topic={service}")});
    if op == "forecast.put" {
        contract["context_guide"] = json!("/api/v1/agent-guide?topic=prediction");
        contract["rules"].as_array_mut().unwrap().extend([
            json!("Current external facts need dated references. Source observation and retrieval must be at or before as_of; retain expired sources as dated context."),
            json!("Local reasoning works without WORLD. Optional WORLD interactions add perspectives and response signals; they do not certify real customer outcomes or improved predictive accuracy.")
        ]);
    }
    if op == "decision.put" {
        contract["input_schema"]["properties"]["input"]["allOf"] = json!([{
            "if":{"required":["experience"]},
            "then":{"properties":{"options":{"minItems":2,"maxItems":4,"items":{"required":["benefit","tradeoff"]}}}}
        }]);
        contract["rules"].as_array_mut().unwrap().push(json!("使用 experience 时提供二至四个方案，每项说明 benefit 与 tradeoff；不用兼得一切的最优答案代替经营取舍"));
    }
    if ["report.put", "simulation_turn.put"].contains(&op) {
        contract["input_schema"]["properties"]["input"]["allOf"] = json!([{
            "if":{"required":["aftermath"]},"then":{"required":["decision_id"]}
        }]);
    }
    if op == "simulation_turn.put" {
        contract["input_schema"]["properties"]["input"]["anyOf"] =
            json!([{"required":["before_revision"]},{"required":["decision_id"]}]);
    }
    if op.starts_with("skill.") {
        let input = &mut contract["input_schema"]["properties"]["input"];
        input["additionalProperties"] = json!(false);
        if op == "skill.put" {
            input["properties"]["name"] =
                json!({"type":"string","pattern":"^[a-z0-9][a-z0-9-]*$","maxLength":63});
            input["properties"]["modes"]["uniqueItems"] = json!(true);
            input["properties"]["source_refs"]["uniqueItems"] = json!(true);
            input["properties"]["copy_from"] = json!({"type":"object","additionalProperties":false,"required":["id","revision","content_sha256","language"],"properties":{"id":{"type":"string"},"revision":{"type":"integer","minimum":1},"content_sha256":{"type":"string","pattern":"^[a-f0-9]{64}$"},"language":{"enum":["en","zh-CN"]}}});
        }
        if op == "skill.delete" {
            input["properties"]["reason"] =
                json!({"type":"string","minLength":1,"maxLength":16384});
        }
    }
    contract
}
pub fn operation_ids() -> Vec<&'static str> {
    let mut operations = vec![
        "company.snapshot",
        "company.records",
        "company.history",
        "company.changes",
        "company.play",
        "company.presentation",
        "company.capture",
        "finance.metrics",
        "files.download",
        "files.preview",
        "skills.list",
        "skills.read",
        "skills.history",
        "plugins.list",
        "company.create",
        "company.update",
        "decision.put",
        "decision.select",
        "decision.claim",
        "decision.resolve",
        "decision.cancel",
        "simulation.fork",
        "simulation.advance",
        "account.put",
        "transaction.record",
        "transaction.reverse",
        "transaction.transfer",
        "business.put",
        "customer.put",
        "resource.put",
        "subscription.put",
        "goal.put",
        "route.put",
        "forecast.put",
        "report.put",
        "simulation_turn.put",
        "file.put",
        "file.sync",
        "skill.put",
        "skill.delete",
        "skill.set_enabled",
    ];
    operations.extend(network::operation_ids());
    operations.extend(game_systems::operation_ids());
    operations.extend(agent_link::operation_ids());
    operations.extend(studio_contract::operation_ids());
    operations
}
pub fn discovery(
    view: Option<&str>,
    service: Option<&str>,
    operation: Option<&str>,
) -> Result<Value> {
    let view = view.unwrap_or("services");
    let ops: Vec<Value> = operation_ids()
        .into_iter()
        .map(operation_contract)
        .collect();
    match view {
        "services" => Ok(
            json!({"version":1,"view":"services","items":[{"id":"company","name":"公司档案"},{"id":"decision","name":"经营事件"},{"id":"finance","name":"资金与订阅"},{"id":"business","name":"业务与资源"},{"id":"growth","name":"经营目标与路线"},{"id":"reports","name":"报告与预测"},{"id":"files","name":"公司资料"},{"id":"simulation","name":"经营推演"},{"id":"skills","name":"经营 Skills","description":"按需读取经营方法与公司经验规范","catalog":"/api/v1/skills","company_catalog":"/api/v1/companies/{company_id}/skills"},{"id":"plugins","name":"插件能力"},{"id":"agent-link","name":"Agent 连接","description":"连接外部宿主并确认待办交接"},{"id":"studio","name":"Four-week studio","description":"Fixed capacity, deadlines and deterministic outcomes"},{"id":"network","name":"关系网","query":"/api/v1/companies/{company_id}/network"},{"id":"game","name":"任务与信笺","query":"/api/v1/companies/{company_id}/game-state"}],"next":"/api/v1/discovery?view=operations&service=<id>"}),
        ),
        "operations" => Ok(
            json!({"version":1,"view":view,"items":ops.into_iter().filter(|o|service.is_none_or(|s|o["service"]==s)).map(|o|json!({"id":o["id"],"description":o["description"],"scope":o["scope"],"contract":format!("/api/v1/discovery?view=contract&operation={}",o["id"].as_str().unwrap())})).collect::<Vec<_>>()}),
        ),
        "contract" => {
            let op =
                operation.ok_or_else(|| DomainError::new("invalid_input", "需要 operation"))?;
            if !operation_ids().contains(&op) {
                return fail("not_found", "操作合同不存在");
            }
            Ok(json!({"version":1,"view":view,"operation":operation_contract(op)}))
        }
        _ => fail(
            "invalid_input",
            "view 必须是 services、operations 或 contract",
        ),
    }
}
pub const PLAYER_OUTPUT_HINT: &str = "每次面向玩家回复前，必须先应用本次 play 内嵌的 player_output 合同；旧部署没有内嵌合同时读取玩家呈现指南。用场景、人物台词、结果和下一选择呈现，不照抄存档中的 V5 等样本代号，不附旧代号或宣布别名维护。技术过程留在幕后，关键未知、风险和未完成状态如实呈现；玩家明确询问时再展开依据。";

pub fn player_output_policy() -> Value {
    json!({
        "version": 3,
        "topic": "player-output",
        "purpose": "以经营叙事向玩家呈现事件、结果和下一选择",
        "required": true,
        "apply_to": ["opening", "event", "settlement", "status", "continue", "decision-handoff"],
        "layout": [
            {"part":"scene","instruction":"用当前事件的自然标题开场；日期、资金与当前目标只列影响本次取舍的项，未知值保持未知"},
            {"part":"dialogue","instruction":"用一至两段短现场叙事和必要的自然台词呈现人物行动、诉求与经营取舍；没有人物的状态查询不强加台词"},
            {"part":"result","instruction":"已经结算时简述行动造成的变化、任务结果及待兑现承诺；未结算时如实呈现等待、处理中或尚缺依据，省略不存在的反馈"},
            {"part":"next-choice","instruction":"结尾给出可直接回复的自然方案文字及必要投入、期限或风险；让玩家作决定，不把继续当作代选授权"}
        ],
        "gameplay": {
            "opening":"模拟开局把题材、有限资源与近期目标合成一组建议供玩家确认，确认后当次接首个处境；已有答案不再重复问卷",
            "event":"用此刻发生的人物行动、旧承诺或机会引出取舍，让玩家读出为什么在意；每个方案都有收益与代价，保留自由计划",
            "judgement":"选择前保存约束、风险与反馈条件，结果接续玩家实际方案；不事后改规则，不用明显最优答案、任意奖惩或无休止危机维持剧情",
            "continuity":"从 story_context 读取相关人物、早先选择与未兑现承诺；结算用具体变化和人物回应显示决定的影响，未发生的后果保持待兑现",
            "closure":"模拟短篇围绕一个近期目标组织三至五回合，按选择调整路径；收束时呈现实际结果、关键取舍和经营所得，目标未达成也允许收束；由玩家决定继续下一阶段或先停下",
            "real_mode":"真实经营只呈现实际人物、已知约束与保存结果；不虚构台词、关系变化或经营成果，不为篇章节奏改变真实日期与账目",
            "fixed_rules":"For an event with execution=studio-v1, read the studio contract, preview a structured allocation and submit studio.choose after confirmation. Do not invent payments, time, success or a fifth week; preserve selection_text when translating a saved free plan.",
            "prediction":"Use actual company records and dated public information available to the owner's Agent for local reasoning. Record observation and retrieval dates, assumptions and missing inputs. WORLD interaction signals are optional additional perspectives, not confirmed real customer outcomes. Offer those signals when the host supports them; do not promise improved accuracy without comparing predictions against later actual results."
        },
        "steps": [
            "本合同适用于每次开局、事件、结算、继续、状态和意向交接回复；读取完整合同后再组织玩家文字，不能只看链接或照抄旧报告、事件和工具输出",
            "接续角色先读取已登记的人物资料、报告与回访线索，沿用稳定自然称呼、已知身份、动机和承诺；旧存档中的技术代号作为内部查找键，不直接复述给玩家",
            "V5、V2、P1、customer-xx、样本 A、访谈对象 5 等研究占位号不作默认人物称呼，也不括注在人名旁；玩家明确选定的代号、实际型号和有意义的产品版本可以保留，不按字符模式盲目替换",
            "有显示称呼就复用；没有时用已知角色称谓，必要时为模拟人物建立稳定自然名并保存到获准公司知识；不宣告改名、建立映射或维护过程，不为命名新建客户或补造业务事实",
            "真实人物只用实际或获准展示的称呼，不虚构身份、台词与反馈；模拟人物、关系和结果延续存档，不把虚构反馈变成真实经营证据",
            "玩家方案用自然文字呈现，按当前事件 option_id 映射提交；玩家无须记住技术标识，不能因改写表达改变原方案承诺或替其选择",
            "接口、命令、文件路径、JSON、回执、版本、原始标识与我完成了哪些维护不进入默认回复；不重复仅模拟、未执行外部操作等声明，开局或模式变化时说明一次",
            "来源、观察日期、假设、计算和授权依据保存在经营记录中；在故事里自然保留影响决定的未知、风险与约束，例如尚未收到付款，不用审计表或免责声明挤占剧情",
            "只有已成功保存并结算的结果才称为已结算；实际写入或授权失败时说明哪项结果未完成及最短恢复动作，不能用剧情掩盖失败或编造金额、经验与任务达成",
            "玩家明确要求技术调试、来源核对、样本比较或完整账目时才展开相关标识、版本与依据；只提供当前问题所需内容，随后继续自然呈现"
        ],
        "preflight": [
            "人物是否沿用自然称呼，且没有附旧样本号或宣布别名维护？",
            "场景、台词、结果和下一选择是否有实际内容，且没有空标题、填充解释或维护汇报？",
            "金额、任务反馈和状态是否与保存结果一致，关键未知与风险是否仍清楚？",
            "玩家能否直接回复一个自然方案，且其选择和外部授权仍由玩家决定？"
        ],
        "technical_details": "on_explicit_request"
    })
}

pub fn player_output_policy_localized(locale: Locale) -> Value {
    let mut policy = player_output_policy();
    locale::localize_metadata(&mut policy, locale);
    policy
}

pub fn guide(topic: &str) -> Result<Value> {
    if topic == "prediction" {
        return Ok(
            json!({"topic":"prediction","purpose":"Reason and forecast from current evidence while retaining a reproducible cutoff","steps":[
            "Local reasoning works without WORLD: use company records, owner-provided data and current public information the external Agent can retrieve.",
            "Ask for missing material or minimum source credentials when it changes the decision. Keep credentials outside company knowledge, reports, links and captures.",
            "Before submitting forecast.put, record as_of, company baseline, assumptions, scenarios, analysis_method, context_sources and limitations. Each source states its kind, reference, observation date and retrieval date.",
            "Dates must not introduce knowledge from after the forecast cutoff. Preserve expired sources as dated context; do not label unknown freshness as real-time evidence.",
            "When supported by the owner's host, optional WORLD social interactions may add independent views and response signals. Keep simulated Agent responses separate from real customer evidence.",
            "After later actual records become available, compare the fixed original scenarios with those results. Improved accuracy is a testable outcome, not a connection guarantee."
        ],"discovery":"/api/v1/discovery?view=contract&operation=forecast.put"}),
        );
    }
    if topic == "player-output" {
        let mut policy = player_output_policy();
        policy["discovery"] = json!("/api/v1/discovery?view=operations&service=company");
        return Ok(policy);
    }
    if let Some(guide) = network::guide(topic)
        .or_else(|| game_systems::guide(topic))
        .or_else(|| agent_link::guide(topic))
        .or_else(|| studio_contract::guide(topic))
    {
        return Ok(guide);
    }
    let (purpose, steps) = match topic {
        "connect" => (
            "连接主人选择的公司部署",
            vec![
                "从主人指定入口读取 agent-entry，再读取所需服务",
                "部署完成后提供可访问的前端入口与安全登录方式；凭据单独交付，不写入公司资料、Skills、链接或截图",
                "主动说明可以新建真实业务、从零开始模拟，或在多家公司间切换；没有真实业务的用户可先选模拟，按用户确认的玩法继续",
                "由主人创建公司范围的 Agent 凭据；本地 CLI 依赖操作系统权限",
                "先读取公司模式、资料与当前版本，按当前任务展开操作合同",
                "每次面向玩家回复前必须应用 play 内嵌的 player_output 合同；没有内嵌合同时读取 player-output 引导，维护记录不直接复述为故事",
                "接到界面创建公司的交接内容时，读取其中的公司标识并维护已有公司，不重复建档；没有待处理事件时接续目标路线与首个事件",
                "模拟游玩按需读取 builtin:simulation-gm，依据公司状态与历史选择主动准备有因果的经营事件，不要求玩家先编写剧情",
            ],
        ),
        "company" => (
            "建立并维护公司档案",
            vec![
                "从对话取得起步业务、可用资源、可投入时间和近期目标，核对币种、时区与起始日期；只追问会影响开局的关键未知值，其余保持未知",
                "明确真实公司还是模拟公司；创建后模式固定",
                "没有开局方向时提供少量具体模拟题材，例如小型 AI 工具、独立订阅产品或工作室服务；用户选定后再建档，模拟启动资金与能力建议须确认",
                "创建公司需要主人权限，准确重试复用原 request_id 与输入并核对回执中的公司标识；已由界面创建的公司继续维护其标识",
                "真实公司缺少实际记录时索取必要资料或最小数据来源授权；模拟公司的背景、产品、客户画像和启动资源可由 Agent 主持并明确标为虚构",
                "读取真实来源需要连接凭据时只索取最小必要授权，凭据保存在 Agent 安全存储或专用配置，不进入公司资料、报告、导出或截图",
                "按需读取 INDEX.md 和关联资料，不一次装载全部公司文件",
                "建档后继续建立与资源相称、近期可反馈的结果目标与阶段路线，准备一个能独立理解的首事件；说明处境、取舍和如何表达选择，在需要玩家决定时等待，不以写完档案结束开局",
                "真实公司进入模拟使用主人授权的 simulation.fork 创建当前基线的独立副本；返回真实经营选择原真实公司或新建真实公司，不把模拟成果写回真实主账",
            ],
        ),
        "decision" => (
            "主持经营事件与玩家选择",
            vec![
                "读取当前公司、目标和相关资料；事件方案包含行动、目标对象及承诺范围，提交时可省略当前公司基线版本",
                "开局事件从短期结果目标出发，方案交代能推动什么、要付出什么及何时反馈；难度随经营阶段推进，结果允许失败复盘与换一种办法，不用无依据奖励代替经营后果",
                "每次面向玩家回复前必须应用 play 内嵌的 player_output 合同；没有内嵌合同时读取 player-output 引导，维护记录不直接复述为故事",
                "模拟事件由经营阶段、资源、经营日期、未结事项和过去选择触发；说明触发依据、经营后果、风险与取舍，现实变化保留来源和日期，虚构情景明确标为假设",
                "玩家选择只保存经营意向；处理前读取最新选择并以 decision.claim 接手，核对公司状态及外部授权，再分别提交真实记录或模拟结果",
                "事件每次写入返回新 record.revision；选择、改选和接手后核对回执，下一步 expected_revision 使用该事件新版本，不使用公司快照版本",
                "结果必须引用选择之后提交的本公司记录的精确版本；真实公司以实际记录或报告结算，模拟公司以推演回合结算",
                "接手前可按当前事件版本改选，接手或相关经营操作后须核对并取消或结算；结算公司版本由平台自动固定，结束后准备下一事件",
            ],
        ),
        "play" => (
            "在 Agent 对话中经营公司并查看页面",
            vec![
                "主要通过外部 Agent 对话进行文字叙事和经营选择；前端辅助查看状态、直接选择、按需截图和快捷链接，分享须由用户明确授权",
                "每次面向玩家回复前必须应用 play 内嵌的 player_output 合同；没有内嵌合同时读取 player-output 引导，维护记录不直接复述为故事",
                "用 company.play 读取当前事件、完整方案、可用菜单及等待状态；用户无需先打开网页",
                "初次游玩说明可直接说出方案文字或自己的计划，也可查看公司、开新公司和切换玩法；没有事件时继续准备开局或下一事件，待选择时的继续只展示处境与方案，不代选",
                "用户表达选择后，Agent 对照方案用 option_id 或 choice 准确提交；含糊选择先澄清，自定义计划仅在允许自由选择时用 text",
                "处理前读取最新选择并以 decision.claim 接手；模拟模式可主持明确虚构的背景、产品、客户画像、事件和响应假设，只询问关键意图与约束，现实市场事实仍保留来源",
                "模拟结算后接续历史选择、承诺与延迟后果准备下一事件；按相关性补充现实信息或使用明确模拟假设，不要求每回合联网或制造随机危机",
                "用户请求某个菜单时，读取 company.presentation；需要画面时调用 company.capture 返回实际截图及无凭据快捷链接，再根据已保存结果继续对话",
                "真实经营缺乏计算依据时取得实际资料或最小数据来源授权；连接凭据只保存在 Agent 安全存储或专用配置，不进入公司资料、报告、导出或截图",
            ],
        ),
        "finance" => (
            "维护可复核的资金记录",
            vec![
                "记录来源与实际发生日，金额使用整数最小货币单位",
                "期初账户建立后用资金记录更新；更正使用冲销，转账使用成对操作",
                "收款、订阅、资本投入与预测分别登记；资金知识不足时补齐依据",
                "真实公司缺少实际资金记录时向用户索取资料，或申请相关来源的最小读取授权；未经依据不估算实际余额和收支",
                "读取真实来源需要连接凭据时只索取最小必要授权，凭据保存在 Agent 安全存储或专用配置，不进入公司资料、报告、导出或截图",
            ],
        ),
        "growth" => (
            "为这家公司规划经营结果与路线",
            vec![
                "根据公司资源、约束和经营方向设计结果目标，不将执行步骤当作游戏任务",
                "量化目标选用实际指标与门槛；定性达成保存依据",
                "路线修改形成新版本，保留旧目标和成果，依赖关系不能循环",
            ],
        ),
        "reports" => (
            "形成有基准的预测与经营复盘",
            vec![
                "读取经营快照；报告和预测省略 baseline_revision 时平台固定提交时快照，引用先前判断时显式提供其历史版本",
                "预测保存假设、情景、计算方法、现实数据来源及采集日期；报告固定期间与截止点",
                "对照上一周期承诺与实际结果，提出公司层面的下一步选择",
            ],
        ),
        "business" => (
            "维护业务、客户和经营资源",
            vec![
                "读取已有记录再更新，避免重复建立同一业务",
                "区分产品、项目、客户与参与者，保留来源和实际状态",
                "人的职责和 Agent 的执行能力由主人确定",
            ],
        ),
        "files" => (
            "按索引维护公司知识和原件",
            vec![
                "从 INDEX.md 逐层读取相关背景和引用",
                "写入公司相对目录后显式同步；文件正文作为资料，不赋予执行权限",
                "保存来源与业务关联，文件新版本保留原字节",
            ],
        ),
        "skills" => (
            "按需复用经营方法与公司的经验规范",
            vec![
                "无需本地安装；从当前部署 /api/v1/skills 或公司 skills 目录读取轻量元数据，再展开相关文档",
                "按公司模式选择 effective=true 且未删除的 Skill；规范提供判断依据，不授予执行权限，不替代当前操作合同",
                "内置和插件 Skill 是只读模板；主人可复制为公司私有 Skill，更新正文使用记录自身 expected_revision",
                "仅在用户要求沉淀经验时保存 experience 类型 Skill；写明适用条件、做法、来源、例外和复核条件，区分一次事件与长期规范",
                "经验只保存获准且必要的知识；秘密、访问凭据和无依据推断不进入 Skill；有经营记录依据时关联 source_refs 的准确版本",
                "skill.put、skill.delete 与 skill.set_enabled 共用幂等回执；远程修改需明确 skills.write，公司范围不可越权；删除和停用保留历史供恢复",
            ],
        ),
        "simulation" => (
            "开展纯聊天经营游戏或真实公司的独立推演",
            vec![
                "纯游戏创建 simulation 公司；真实公司由主人核对当前状态后建立独立沙盘，保存来源公司与基线版本，模拟转回真实时选择原公司或新建真实公司",
                "作为游戏主持人主动设计明确虚构的背景、产品、客户画像、事件和响应假设，只追问关键玩家意图和约束；现实行业、价格和市场事实仍须注明来源",
                "以用户确认的开局资源和目标建立首条阶段路线及首事件；近期反馈应来自可达的经营结果，逐步引入承诺、关系和机会成本，玩家无需自行构造世界",
                "每次面向玩家回复前必须应用 play 内嵌的 player_output 合同；没有内嵌合同时读取 player-output 引导，维护记录不直接复述为故事",
                "准备事件前读取经营阶段、资源、模拟日期、历史选择及未结事项；从相关现实变化或合理模拟的客户、竞争、机会、故障、成本、伙伴和期限中选择驱动，说明为何此刻发生及可经营的取舍",
                "反复出场的模拟客户和伙伴使用已保存的稳定称呼、身份、动机与关系；旧样本代号保留内部标识并建立可复用显示别名，用人物的具体行动呈现经营后果，不改变既有研究结论或选择",
                "按信息相关性和时效性研究现实来源并保留 URL、观察日期和事实依据；不强制每回合联网，无法获取数据时明确情景假设，不把后来信息当作模拟日期当时已知的事实",
                "用户在聊天中提出经营选择后，收集公司信息与相关现实数据，保留来源、日期、计算方法和假设；用模拟记录计算结果，再发布回合",
                "成功失败依据已说明的风险、资源约束、模拟响应和保存的指标；区分即时与延迟后果，不用无依据惩罚或赠送成果代替推演",
                "在公司资料或回合叙事中保留未结承诺、期限、延迟后果及来源事件或回合标识；资料修改后显式同步，下一事件读取并接续，不在每回合重置关系或要求玩家编写世界",
                "回合保存选择、故事、依据、成功失败结果和前后版本；缺少证据时展示不确定性；虚拟初始资源明确标识，经营规律依据现实信息，推演不修改真实公司",
                "回合提供 decision_id 时平台从已选择事件固定 before_revision，after_revision 和结算公司版本默认当前提交快照；更新记录仍用该记录的 expected_revision",
            ],
        ),
        "backup" => (
            "备份、恢复与迁移",
            vec![
                "调用内核生成含全部版本及文件的可校验快照",
                "可选插件传送到主人选择的目的地；调度由外部客户端承担",
                "在空目标恢复并核对后再切换，重新建立远程授权",
            ],
        ),
        "plugins" => (
            "安装与维护扩展",
            vec![
                "读取固定版本、来源、兼容与权限说明",
                "执行插件属于可信本地程序，安装和启用需要主人权限",
                "停用保留数据，卸载先处理保留数据与退出责任",
            ],
        ),
        "recover" => (
            "处理未知结果或版本冲突",
            vec![
                "结果未知时保持原 request_id 与输入重试",
                "版本冲突保留自己的修改，读取当前版本后合并并使用新 request_id",
                "凭据撤销、文件损坏或依赖不可用时停止相应写入并提供可恢复路径",
            ],
        ),
        _ => return fail("not_found", "引导主题不存在"),
    };
    Ok(
        json!({"version":1,"topic":topic,"purpose":purpose,"steps":steps,"discovery":if ["connect","backup","plugins","recover"].contains(&topic){"/api/v1/discovery?view=services".to_string()}else if ["play","player-output"].contains(&topic){"/api/v1/discovery?view=operations&service=company".to_string()}else{format!("/api/v1/discovery?view=operations&service={topic}")}}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn player_output_contract_is_required_and_shared_with_localized_guidance() {
        for locale in [Locale::ZhCn, Locale::En] {
            let policy = player_output_policy_localized(locale);
            let mut guide = guide_localized("player-output", locale).unwrap();
            assert_eq!(
                guide["discovery"],
                "/api/v1/discovery?view=operations&service=company"
            );
            guide.as_object_mut().unwrap().remove("discovery");
            assert_eq!(guide, policy);
            assert_eq!(policy["required"], true);
            assert_eq!(policy["technical_details"], "on_explicit_request");
            assert_eq!(
                policy["apply_to"],
                json!([
                    "opening",
                    "event",
                    "settlement",
                    "status",
                    "continue",
                    "decision-handoff"
                ])
            );
            let parts: Vec<&str> = policy["layout"]
                .as_array()
                .unwrap()
                .iter()
                .map(|part| part["part"].as_str().unwrap())
                .collect();
            assert_eq!(parts, ["scene", "dialogue", "result", "next-choice"]);
            assert!(!policy["steps"].as_array().unwrap().is_empty());
            assert!(!policy["preflight"].as_array().unwrap().is_empty());
        }
    }
    fn c() -> Company {
        serde_json::from_value(json!({"id":"c","name":"测试","currency":"CNY","timezone":"Asia/Shanghai","opening_date":"2026-01-01","revision":1,"created_at":"x","updated_at":"x"})).unwrap()
    }
    fn r(kind: &str, id: &str, data: Value) -> Record {
        Record {
            id: id.into(),
            company_id: "c".into(),
            kind: kind.into(),
            revision: 1,
            company_revision: 1,
            data,
            created_at: "x".into(),
            updated_at: "x".into(),
        }
    }
    #[test]
    fn unknown_is_not_zero() {
        let m = compute_metrics(&c(), &[], date("2026-10-11").unwrap()).unwrap();
        assert!(m["cash_balance_minor"].is_null());
        assert!(m["mrr_minor"].is_null());
        assert!(m["net_operating_cashflow_minor"].is_null());
        assert!(m["monthly_cashflow"].as_array().unwrap().is_empty());
    }
    #[test]
    fn subscription_and_cash_are_distinct() {
        let rows = vec![
            r(
                "account",
                "a",
                json!({"name":"账户","opening_balance_minor":20000}),
            ),
            r("customer", "u", json!({"name":"客户","status":"paying"})),
            r(
                "subscription",
                "s",
                json!({"customer_id":"u","amount_minor":120000,"interval_months":12,"status":"active","starts_on":"2026-01-01"}),
            ),
            r(
                "subscription",
                "trial",
                json!({"customer_id":"u","amount_minor":50000,"interval_months":1,"status":"trial","starts_on":"2026-01-01"}),
            ),
        ];
        let m = compute_metrics(&c(), &rows, date("2026-10-11").unwrap()).unwrap();
        assert_eq!(m["mrr_minor"], 10000);
        assert_eq!(m["cash_balance_minor"], 20000);
        assert_eq!(m["operating_inflow_minor"], 0);
    }
    #[test]
    fn month_cashflow_and_capital_separated() {
        let rows = vec![
            r(
                "account",
                "a",
                json!({"name":"账户","opening_balance_minor":1000}),
            ),
            r(
                "transaction",
                "t",
                json!({"account_id":"a","amount_minor":200,"direction":"inflow","category":"capital","occurred_on":"2026-10-10"}),
            ),
            r(
                "transaction",
                "s",
                json!({"account_id":"a","amount_minor":50,"direction":"outflow","category":"operating","occurred_on":"2026-10-11"}),
            ),
        ];
        let m = compute_metrics(&c(), &rows, date("2026-10-11").unwrap()).unwrap();
        assert_eq!(m["cash_balance_minor"], 1150);
        assert_eq!(m["net_operating_cashflow_minor"], -50);
    }
    #[test]
    fn goal_corrections_recompute_and_dependencies_block() {
        let mut company = c();
        company.subscription_tracking = true;
        let rows = vec![
            r(
                "goal",
                "g",
                json!({"title":"订阅","outcome":"订阅规模","status":"achieved","metric":"mrr","target":1000}),
            ),
            r(
                "goal",
                "h",
                json!({"title":"发布","outcome":"发布成果","status":"achieved","metric":"manual","evidence":"x","depends_on":["g"]}),
            ),
        ];
        let m = compute_metrics(&company, &rows, date("2026-10-11").unwrap()).unwrap();
        assert_eq!(m["goal_progress"][0]["status"], "active");
        assert_eq!(m["goal_progress"][1]["status"], "blocked");
    }
}
