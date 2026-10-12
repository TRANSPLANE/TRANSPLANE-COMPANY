use super::*;

pub const RECORD_KINDS: [&str; 3] = ["network_entity", "relationship", "profit_entry"];
pub const NODE_KINDS: [&str; 5] = [
    "company",
    "customer",
    "business",
    "resource",
    "network_entity",
];

fn allowed_fields(v: &Value, allowed: &[&str]) -> Result<()> {
    let obj = v
        .as_object()
        .ok_or_else(|| DomainError::new("invalid_input", "关系网记录必须为对象"))?;
    if obj.keys().any(|key| !allowed.contains(&key.as_str())) {
        return fail("invalid_input", "关系网记录包含未知字段");
    }
    Ok(())
}

fn optional_text(v: &Value, key: &str) -> Result<()> {
    if let Some(value) = v.get(key)
        && !value.is_null()
        && value
            .as_str()
            .is_none_or(|s| s.len() > 16384 || s.contains('\0'))
    {
        return fail("invalid_input", format!("{key} 必须是文字"));
    }
    Ok(())
}

pub fn validate_node_reference(
    reference: &Value,
    company: &Company,
    records: &[Record],
) -> Result<()> {
    allowed_fields(reference, &["kind", "id"])?;
    let kind = text(reference, "kind")?;
    let id = text(reference, "id")?;
    identifier(id)?;
    if !NODE_KINDS.contains(&kind) {
        return fail("invalid_input", "关系网节点类型无效");
    }
    if (kind == "company" && id != company.id)
        || (kind != "company"
            && !records
                .iter()
                .any(|r| r.company_id == company.id && r.kind == kind && r.id == id))
    {
        return fail("reference_not_found", "关系网节点不存在于当前公司");
    }
    Ok(())
}

pub fn validate_source_refs(v: &Value, required: bool) -> Result<()> {
    let Some(refs) = v.get("source_refs") else {
        return if required {
            fail("invalid_input", "收益归属需要准确来源")
        } else {
            Ok(())
        };
    };
    let refs = refs
        .as_array()
        .ok_or_else(|| DomainError::new("invalid_input", "source_refs 必须是数组"))?;
    if refs.len() > 100 || (required && refs.is_empty()) {
        return fail("invalid_input", "source_refs 需要一至一百项准确来源");
    }
    let mut unique = BTreeSet::new();
    for reference in refs {
        allowed_fields(reference, &["kind", "id", "revision"])?;
        let kind = text(reference, "kind")?;
        let id = text(reference, "id")?;
        identifier(kind)?;
        identifier(id)?;
        let revision = integer(reference, "revision")?;
        if revision < 1 || !unique.insert((kind, id, revision)) {
            return fail("invalid_input", "来源版本必须为正整数且不可重复");
        }
    }
    Ok(())
}

fn nullable_money(v: &Value, key: &str) -> Result<Option<i64>> {
    let value = v
        .get(key)
        .ok_or_else(|| DomainError::new("invalid_input", format!("缺少 {key}")))?;
    if value.is_null() {
        return Ok(None);
    }
    let amount = value
        .as_i64()
        .filter(|n| (0..=MAX_SAFE_MINOR).contains(n))
        .ok_or_else(|| {
            DomainError::new("invalid_input", "收益金额必须为非负整数最小货币单位或 null")
        })?;
    Ok(Some(amount))
}

/// Cash records retain their identifier when copied to an independent branch.
/// A reserved proof can wrap that immutable evidence without making it a new receipt.
fn cash_evidence_keys(data: &Value, company: &Company, existing: &[Record]) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    let mut current = Some(data);
    while let Some(value) = current {
        let mut unresolved_proof = false;
        for reference in value["source_refs"].as_array().into_iter().flatten() {
            if reference["kind"] == "transaction"
                && let Some(id) = reference["id"].as_str()
            {
                keys.insert(id.to_owned());
            } else if reference["kind"] == "file"
                && let Some(file) = existing.iter().find(|file| {
                    file.company_id == company.id
                        && file.kind == "file"
                        && file.id == reference["id"]
                        && file.revision == reference["revision"].as_i64().unwrap_or(0)
                        && file.data["fork_evidence"] == true
                })
            {
                let origin = &file.data["fork_evidence_origin"];
                if origin["kind"] == "transaction"
                    && let Some(id) = origin["id"].as_str()
                {
                    keys.insert(id.to_owned());
                } else if origin.is_null() {
                    unresolved_proof = true;
                }
            } else if reference["kind"] == "file"
                && reference["id"]
                    .as_str()
                    .is_some_and(|id| id.starts_with("fork-proof-"))
            {
                unresolved_proof = true;
            }
        }
        // Retained provenance is system-managed. Older branches may have no
        // canonical file metadata, but their source record still names the receipt.
        current = if unresolved_proof {
            value.pointer("/fork_provenance/source_record/data")
        } else {
            None
        };
    }
    keys
}

pub fn validate_record(
    kind: &str,
    v: &Value,
    company: &Company,
    existing: &[Record],
) -> Result<()> {
    identifier(text(v, "id")?)?;
    if serde_json::to_vec(v).unwrap_or_default().len() > 524288 {
        return fail("invalid_input", "关系网记录超过大小限制");
    }
    if let Some(currency) = v.get("currency")
        && currency.as_str() != Some(company.currency.as_str())
    {
        return fail("currency_mismatch", "金额币种与公司记账币种不一致");
    }
    match kind {
        "network_entity" => {
            allowed_fields(
                v,
                &[
                    "id",
                    "name",
                    "type",
                    "role",
                    "description",
                    "status",
                    "source_refs",
                    "fork_provenance",
                ],
            )?;
            text(v, "name")?;
            text(v, "role")?;
            one_of(v, "type", &["person", "organization"])?;
            one_of(v, "status", &["active", "inactive", "archived"])?;
            optional_text(v, "description")?;
            validate_source_refs(v, false)?;
        }
        "relationship" => {
            allowed_fields(
                v,
                &[
                    "id",
                    "source",
                    "target",
                    "type",
                    "status",
                    "label",
                    "description",
                    "since",
                    "until",
                    "source_refs",
                    "fork_provenance",
                ],
            )?;
            validate_node_reference(&v["source"], company, existing)?;
            validate_node_reference(&v["target"], company, existing)?;
            if v["source"] == v["target"] {
                return fail("invalid_input", "关系的两端必须不同");
            }
            one_of(
                v,
                "type",
                &[
                    "customer",
                    "partner",
                    "supplier",
                    "competitor",
                    "ownership",
                    "team",
                    "influence",
                    "depends_on",
                ],
            )?;
            one_of(v, "status", &["active", "prospective", "ended"])?;
            for field in ["label", "description"] {
                optional_text(v, field)?;
            }
            for field in ["since", "until"] {
                if let Some(value) = v.get(field).filter(|value| !value.is_null()) {
                    date(
                        value
                            .as_str()
                            .ok_or_else(|| DomainError::new("invalid_input", "关系日期无效"))?,
                    )?;
                }
            }
            if let (Some(start), Some(end)) = (v["since"].as_str(), v["until"].as_str())
                && date(end)? < date(start)?
            {
                return fail("invalid_input", "关系结束日期早于开始日期");
            }
            validate_source_refs(v, false)?;
        }
        "profit_entry" => {
            allowed_fields(
                v,
                &[
                    "id",
                    "subject",
                    "period_start",
                    "period_end",
                    "basis",
                    "revenue_minor",
                    "cost_minor",
                    "cost_coverage",
                    "currency",
                    "source_refs",
                    "note",
                    "assumptions",
                    "status",
                    "fork_provenance",
                ],
            )?;
            validate_node_reference(&v["subject"], company, existing)?;
            let start = date(text(v, "period_start")?)?;
            let end = date(text(v, "period_end")?)?;
            if end < start {
                return fail("invalid_input", "收益期间结束日早于开始日");
            }
            one_of(v, "basis", &["cash", "recognized"])?;
            one_of(v, "cost_coverage", &["complete", "partial"])?;
            if v.get("status").is_some() {
                one_of(v, "status", &["active", "void"])?;
            }
            nullable_money(v, "revenue_minor")?;
            nullable_money(v, "cost_minor")?;
            if v["cost_coverage"] == "complete" && v["cost_minor"].is_null() {
                return fail("invalid_input", "成本完整时必须提供已归属成本");
            }
            for field in ["note", "assumptions"] {
                optional_text(v, field)?;
            }
            if company.mode == "simulation" {
                text(v, "assumptions")?;
            }
            validate_source_refs(v, true)?;
            if v["status"] != "void" {
                let evidence = cash_evidence_keys(v, company, existing);
                for old in existing.iter().filter(|r| {
                    r.company_id == company.id
                        && r.kind == "profit_entry"
                        && r.id != v["id"].as_str().unwrap()
                        && r.data["status"] != "void"
                        && r.data["basis"] == v["basis"]
                }) {
                    if old.data["subject"] == v["subject"]
                        && date(text(&old.data, "period_start")?)? <= end
                        && date(text(&old.data, "period_end")?)? >= start
                    {
                        return fail("profit_period_overlap", "同一对象同口径的收益期间不能重叠");
                    }
                    if !evidence.is_disjoint(&cash_evidence_keys(&old.data, company, existing)) {
                        return fail(
                            "profit_source_conflict",
                            "同一资金来源同口径不可重复归属收益",
                        );
                    }
                }
            }
        }
        _ => return fail("invalid_input", "关系网记录类型无效"),
    }
    Ok(())
}

/// Validate references against retained history, including snapshots with old evidence revisions.
pub fn validate_graph_records(
    company: &Company,
    current: &[Record],
    history: &[Record],
) -> Result<()> {
    for record in current
        .iter()
        .filter(|r| RECORD_KINDS.contains(&r.kind.as_str()))
    {
        validate_record(&record.kind, &record.data, company, current)?;
        for reference in record.data["source_refs"].as_array().into_iter().flatten() {
            if !history.iter().any(|r| {
                r.company_id == company.id
                    && r.kind == reference["kind"]
                    && r.id == reference["id"]
                    && r.revision == reference["revision"].as_i64().unwrap()
                    && r.company_revision <= record.company_revision
            }) {
                return fail(
                    "reference_not_found",
                    "关系网来源的准确版本不存在于当前公司",
                );
            }
        }
    }
    Ok(())
}

fn node_key(kind: &str, id: &str) -> String {
    format!("{kind}:{id}")
}
fn reference_key(reference: &Value) -> String {
    node_key(
        reference["kind"].as_str().unwrap_or(""),
        reference["id"].as_str().unwrap_or(""),
    )
}
fn unknown_profit(period: &Value) -> Value {
    json!({"status":"unknown","revenue_minor":null,"cost_minor":null,"profit_minor":null,"cost_coverage":null,"basis":period["basis"],"period_start":period["period_start"],"period_end":period["period_end"],"entry_id":null,"entry_revision":null})
}

pub fn projection(
    company: &Company,
    records: &[Record],
    as_of: &str,
    start: Option<&str>,
    end: Option<&str>,
    basis: Option<&str>,
) -> Result<Value> {
    date(as_of)?;
    if start.is_some() != end.is_some() {
        return fail("invalid_input", "收益筛选必须同时提供起止日期");
    }
    if basis.is_some_and(|basis| !["cash", "recognized"].contains(&basis)) {
        return fail("invalid_input", "收益口径必须为 cash 或 recognized");
    }
    if let (Some(start), Some(end)) = (start, end)
        && date(end)? < date(start)?
    {
        return fail("invalid_input", "收益期间结束日早于开始日");
    }
    let own: Vec<_> = records
        .iter()
        .filter(|r| r.company_id == company.id)
        .collect();
    let mut periods: BTreeSet<(String, String, String)> = BTreeSet::new();
    for r in own
        .iter()
        .filter(|r| r.kind == "profit_entry" && r.data["status"] != "void")
    {
        periods.insert((
            text(&r.data, "period_end")?.into(),
            text(&r.data, "period_start")?.into(),
            text(&r.data, "basis")?.into(),
        ));
    }
    let selected = if let (Some(start), Some(end)) = (start, end) {
        Some((
            end.to_owned(),
            start.to_owned(),
            basis.unwrap_or("cash").to_owned(),
        ))
    } else {
        periods
            .iter()
            .rev()
            .find(|(_, _, b)| basis.is_none_or(|basis| b == basis))
            .cloned()
    };
    let period = selected
        .as_ref()
        .map(|(end, start, b)| json!({"period_start":start,"period_end":end,"basis":b}))
        .unwrap_or(Value::Null);
    let mut nodes = BTreeMap::new();
    let root = node_key("company", &company.id);
    nodes.insert(root.clone(),json!({"id":root,"kind":"company","record_id":company.id,"label":company.name,"type":"company","role":"company","description":company.description,"status":"active","revision":company.revision,"source_refs":[],"profit":unknown_profit(&period)}));
    for r in own.iter().filter(|r| {
        ["customer", "business", "resource", "network_entity"].contains(&r.kind.as_str())
    }) {
        let key = node_key(&r.kind, &r.id);
        nodes.insert(key.clone(), json!({"id":key,"kind":r.kind,"record_id":r.id,"label":r.data.get("display_name").or_else(||r.data.get("name")).cloned().unwrap_or(Value::Null),"type":r.data.get("type").cloned().unwrap_or(json!(r.kind)),"role":r.data.get("role").cloned().unwrap_or(json!(r.kind)),"description":r.data.get("description").cloned().unwrap_or(json!("")),"status":r.data["status"],"revision":r.revision,"source_refs":r.data.get("source_refs").cloned().unwrap_or(json!([])),"profit":unknown_profit(&period)}));
    }
    for r in own
        .iter()
        .filter(|r| r.kind == "profit_entry" && r.data["status"] != "void")
    {
        if selected.as_ref().is_some_and(|(end, start, b)| {
            r.data["period_end"] == end.as_str()
                && r.data["period_start"] == start.as_str()
                && r.data["basis"] == b.as_str()
        }) {
            let revenue = nullable_money(&r.data, "revenue_minor")?;
            let cost = nullable_money(&r.data, "cost_minor")?;
            let complete =
                r.data["cost_coverage"] == "complete" && revenue.is_some() && cost.is_some();
            let profit = if complete {
                Some(revenue.unwrap() - cost.unwrap())
            } else {
                None
            };
            if let Some(node) = nodes.get_mut(&reference_key(&r.data["subject"])) {
                node["profit"] = json!({"status":if complete {"complete"} else if revenue.is_some() || cost.is_some() {"partial"} else {"unknown"},"revenue_minor":revenue,"cost_minor":cost,"profit_minor":profit,"cost_coverage":r.data["cost_coverage"],"basis":r.data["basis"],"period_start":r.data["period_start"],"period_end":r.data["period_end"],"entry_id":r.id,"entry_revision":r.revision,"source_refs":r.data["source_refs"],"note":r.data.get("note"),"assumptions":r.data.get("assumptions")});
            }
        }
    }
    let mut edges = Vec::new();
    for r in own.iter().filter(|r| r.kind == "relationship") {
        let source = reference_key(&r.data["source"]);
        let target = reference_key(&r.data["target"]);
        if nodes.contains_key(&source) && nodes.contains_key(&target) {
            edges.push(json!({"id":format!("relationship:{}",r.id),"source":source,"target":target,"type":r.data["type"],"status":r.data["status"],"label":r.data.get("label").cloned().unwrap_or(json!("")),"description":r.data.get("description"),"since":r.data.get("since"),"until":r.data.get("until"),"revision":r.revision,"derived":false,"source_refs":r.data.get("source_refs").cloned().unwrap_or(json!([]))}));
        }
    }
    for r in own
        .iter()
        .filter(|r| ["customer", "business", "resource"].contains(&r.kind.as_str()))
    {
        let target = node_key(&r.kind, &r.id);
        let relation = match r.kind.as_str() {
            "customer" => "customer",
            "business" => "ownership",
            _ => "team",
        };
        if !edges.iter().any(|e| {
            e["source"] == root && e["target"] == target && e["type"] == relation
                || e["source"] == target && e["target"] == root && e["type"] == relation
        }) {
            edges.push(json!({"id":format!("derived:{}:{}",r.kind,r.id),"source":root,"target":target,"type":relation,"status":r.data["status"],"label":"","revision":r.revision,"derived":true,"source_refs":[{"kind":r.kind,"id":r.id,"revision":r.revision}]}));
        }
    }
    for r in own.iter().filter(|r| r.kind == "subscription") {
        let source = node_key("customer", r.data["customer_id"].as_str().unwrap_or(""));
        let target = r.data["business_id"]
            .as_str()
            .map(|id| node_key("business", id))
            .filter(|key| nodes.contains_key(key))
            .unwrap_or(root.clone());
        if nodes.contains_key(&source) {
            edges.push(json!({"id":format!("subscription:{}",r.id),"source":source,"target":target,"type":"subscribes_to","status":r.data["status"],"label":"","revision":r.revision,"derived":true,"source_refs":[{"kind":"subscription","id":r.id,"revision":r.revision}]}));
        }
    }
    Ok(
        json!({"version":1,"company_id":company.id,"company_revision":company.revision,"as_of":as_of,"currency":company.currency,"mode":company.mode,"period":period,"profit_periods":periods.iter().rev().map(|(end,start,b)|json!({"period_start":start,"period_end":end,"basis":b})).collect::<Vec<_>>(),"nodes":nodes.into_values().collect::<Vec<_>>(),"edges":edges}),
    )
}

pub fn operation_ids() -> Vec<&'static str> {
    vec![
        "company.network",
        "network_entity.put",
        "relationship.put",
        "profit_entry.put",
    ]
}
pub fn operation_scope(operation: &str) -> Option<&'static str> {
    match operation {
        "network_entity.put" | "relationship.put" => Some("company.write"),
        "profit_entry.put" => Some("finance.write"),
        _ => None,
    }
}

pub fn operation_contract(operation: &str) -> Option<Value> {
    if operation == "company.network" {
        return Some(
            json!({"id":operation,"operation":operation,"service":"network","description":"读取公司关系网与有依据的客户收益归属","kind":"read","scope":"company.read","http":{"method":"GET","path":"/api/v1/companies/{company_id}/network?period_start={date}&period_end={date}&basis={cash|recognized}"},"rules":["关系只引用当前公司档案，不访问其他公司的记录","客户收益按明确期间和口径展示；缺少完整成本时利润保持未知"],"guide":"/api/v1/agent-guide?topic=network"}),
        );
    }
    let node_ref = json!({"type":"object","required":["kind","id"],"additionalProperties":false,"properties":{"kind":{"enum":NODE_KINDS},"id":{"type":"string"}}});
    let source_refs = json!({"type":"array","maxItems":100,"items":{"type":"object","required":["kind","id","revision"],"additionalProperties":false,"properties":{"kind":{"type":"string"},"id":{"type":"string"},"revision":{"type":"integer","minimum":1}}}});
    let (description, required, mut properties) = match operation {
        "network_entity.put" => (
            "维护经营人物与关联组织",
            vec!["id", "name", "type", "role"],
            json!({"id":{"type":"string"},"name":{"type":"string"},"type":{"enum":["person","organization"]},"role":{"type":"string"},"description":{"type":"string"},"status":{"enum":["active","inactive","archived"],"default":"active"}}),
        ),
        "relationship.put" => (
            "维护本公司客户、合作与组织关系",
            vec!["id", "source", "target", "type"],
            json!({"id":{"type":"string"},"source":node_ref,"target":node_ref,"type":{"enum":["customer","partner","supplier","competitor","ownership","team","influence","depends_on"]},"status":{"enum":["active","prospective","ended"],"default":"active"},"label":{"type":"string"},"description":{"type":"string"},"since":{"type":"string","format":"date"},"until":{"type":"string","format":"date"}}),
        ),
        "profit_entry.put" => (
            "登记客户与业务的收入成本归属",
            vec![
                "id",
                "subject",
                "period_start",
                "period_end",
                "basis",
                "revenue_minor",
                "cost_minor",
                "cost_coverage",
                "source_refs",
            ],
            json!({"id":{"type":"string"},"subject":node_ref,"period_start":{"type":"string","format":"date"},"period_end":{"type":"string","format":"date"},"basis":{"enum":["cash","recognized"]},"revenue_minor":{"type":["integer","null"],"minimum":0,"maximum":MAX_SAFE_MINOR},"cost_minor":{"type":["integer","null"],"minimum":0,"maximum":MAX_SAFE_MINOR},"cost_coverage":{"enum":["complete","partial"]},"currency":{"type":"string"},"note":{"type":"string"},"assumptions":{"type":"string"},"status":{"enum":["active","void"],"default":"active"}}),
        ),
        _ => return None,
    };
    properties["source_refs"] = source_refs;
    Some(
        json!({"id":operation,"operation":operation,"service":"network","description":description,"kind":"write","scope":operation_scope(operation),"http":{"method":"POST","path":format!("/api/v1/companies/{{company_id}}/operations/{operation}")},"input_schema":{"type":"object","required":["request_id","input"],"properties":{"request_id":{"type":"string"},"expected_revision":{"type":["integer","null"]},"input":{"type":"object","additionalProperties":false,"required":required,"properties":properties}}},"result":{"status":"committed","replayed":"boolean","record":"object","company_revision":"integer"},"errors":["invalid_input","forbidden","reference_not_found","revision_conflict","request_id_conflict","currency_mismatch","profit_period_overlap","profit_source_conflict"],"rules":["仅引用本公司节点和准确来源版本；公司节点只允许当前公司标识","更新复用稳定标识并提供记录 expected_revision，准确重试复用 request_id","同一对象同口径期间不能重叠，同一资金来源同口径不可重复归属","利润为已记录收入减完整已归属成本；成本不完整或收入未知时利润未知，不用现金余额或 ARR 代替利润","模拟收益写明假设；真实收益使用实际依据；结束关系或撤销收益保留历史"],"guide":"/api/v1/agent-guide?topic=network"}),
    )
}

pub fn guide(topic: &str) -> Option<Value> {
    if topic != "network" {
        return None;
    }
    Some(
        json!({"version":1,"topic":"network","purpose":"维护公司关系与可追溯收益归属","steps":["先查询公司关系网，复用已有客户、业务、资源和人物标识，人物用稳定称呼和具体角色","公司根节点只代表当前公司；关联企业建立 organization 档案，不读取或连接未经授权的其他公司","根据真实依据或模拟历史保存合作、供应、竞争、团队及依赖关系；关系变化保留版本和日期","收益按对象、闭区间期间和 cash 或 recognized 口径登记。收入与成本未知时用 null；成本未完整归属时用 partial，不宣称利润","收益来源引用本公司准确记录版本；同一对象同口径禁止期间重叠，同一资金来源不能重复归属","模拟金额附明确假设。真实收益不能从余额、融资、订阅规模或人物好感度推断","对玩家用人物、合作进展和关键收益取舍讲述关系变化；原始标识和接口回执留在维护过程"]}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn company() -> Company {
        serde_json::from_value(json!({"id":"ours","name":"我们的小店","currency":"CNY","timezone":"Asia/Shanghai","description":"","opening_date":"2026-01-01","revision":2,"created_at":"now","updated_at":"now"})).unwrap()
    }
    fn record(company: &str, kind: &str, id: &str, data: Value) -> Record {
        Record {
            company_id: company.into(),
            kind: kind.into(),
            id: id.into(),
            revision: 1,
            company_revision: 1,
            data,
            created_at: "now".into(),
            updated_at: "now".into(),
        }
    }
    #[test]
    fn projection_drops_other_company_records_and_never_derives_profit_from_cash() {
        let c = company();
        let records = vec![
            record(
                "ours",
                "account",
                "bank",
                json!({"name":"bank","opening_balance_minor":500000}),
            ),
            record(
                "ours",
                "customer",
                "known",
                json!({"name":"小林","status":"lead"}),
            ),
            record(
                "other",
                "customer",
                "private",
                json!({"name":"private","status":"paying"}),
            ),
            record(
                "other",
                "network_entity",
                "private-org",
                json!({"name":"private-org","status":"active","type":"organization"}),
            ),
        ];
        let graph = projection(&c, &records, "2026-01-31", None, None, None).unwrap();
        assert_eq!(graph["nodes"].as_array().unwrap().len(), 2);
        assert!(
            graph["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .all(|n| n["profit"]["profit_minor"].is_null())
        );
        assert!(!graph.to_string().contains("private"));
    }
    #[test]
    fn references_reject_company_scope_fields_and_future_evidence() {
        let c = company();
        let other = record(
            "other",
            "customer",
            "private",
            json!({"name":"private","status":"lead"}),
        );
        assert!(
            validate_node_reference(&json!({"kind":"customer","id":"private"}), &c, &[other])
                .is_err()
        );
        assert!(validate_node_reference(&json!({"kind":"company","id":"other"}), &c, &[]).is_err());
        assert!(validate_source_refs(&json!({"source_refs":[{"kind":"file","id":"doc","revision":1,"company_id":"other"}]}),true).is_err());
        let mut entity = record(
            "ours",
            "network_entity",
            "lin",
            json!({"id":"lin","name":"小林","role":"合作方","type":"person","status":"active","source_refs":[{"kind":"file","id":"doc","revision":1}]}),
        );
        entity.company_revision = 1;
        let mut future = record("ours", "file", "doc", json!({"id":"doc"}));
        future.company_revision = 2;
        assert!(validate_graph_records(&c, &[entity.clone()], &[entity, future]).is_err());
    }

    #[test]
    fn wrapped_cash_uses_the_original_identity_but_corrections_release_old_attribution() {
        let c = company();
        let original = json!({"source_refs":[{"kind":"transaction","id":"receipt","revision":1}]});
        let wrapped = json!({"source_refs":[{"kind":"file","id":"fork-proof-one","revision":1}],"fork_provenance":{"source_record":{"data":original}}});
        let legacy_proof = record(
            "ours",
            "file",
            "fork-proof-one",
            json!({"fork_evidence":true}),
        );
        assert_eq!(
            cash_evidence_keys(&wrapped, &c, &[legacy_proof]),
            BTreeSet::from(["receipt".into()])
        );
        let proof = record(
            "ours",
            "file",
            "fork-proof-two",
            json!({"fork_evidence":true,"fork_evidence_origin":{"kind":"transaction","id":"receipt","revision":1,"company_id":"original","company_revision":3}}),
        );
        let nested = json!({"source_refs":[{"kind":"file","id":"fork-proof-two","revision":1}],"fork_provenance":{"source_record":{"data":wrapped}}});
        assert_eq!(
            cash_evidence_keys(&nested, &c, &[proof]),
            BTreeSet::from(["receipt".into()])
        );
        let mut corrected = nested;
        corrected["source_refs"] =
            json!([{"kind":"transaction","id":"another-receipt","revision":1}]);
        assert_eq!(
            cash_evidence_keys(&corrected, &c, &[]),
            BTreeSet::from(["another-receipt".into()])
        );
    }
}
