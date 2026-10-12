use super::*;
use std::collections::BTreeSet;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(super) fn validate_manifest(v: &Value) -> Result<()> {
    if v["schema_version"] != 1 || v["api_version"] != 1 {
        return Err(err(
            "incompatible_plugin",
            "插件需要 schema_version=1、api_version=1",
        ));
    }
    identifier(text(v, "id")?)?;
    text(v, "name")?;
    text(v, "author")?;
    text(v, "description")?;
    let version = text(v, "version")?;
    if version.split('.').count() != 3 || !version.split('.').all(|s| s.parse::<u32>().is_ok()) {
        return Err(err("invalid_input", "插件版本必须是固定的 x.y.z"));
    }
    if !v.get("contributions").is_some_and(Value::is_object) {
        return Err(err("invalid_input", "插件需要 contributions 对象"));
    }
    if let Some(runtime) = v.get("runtime") {
        let command = runtime
            .get("command")
            .and_then(Value::as_array)
            .ok_or_else(|| err("invalid_input", "执行插件需要 command 数组"))?;
        if command.is_empty()
            || command.len() > 32
            || command
                .iter()
                .any(|arg| arg.as_str().is_none_or(|s| s.is_empty() || s.len() > 4096))
        {
            return Err(err("invalid_input", "插件 command 无效"));
        }
    }
    if let Some(permissions) = v.get("permissions") {
        let permissions = permissions
            .as_array()
            .ok_or_else(|| err("invalid_input", "插件权限必须是数组"))?;
        if permissions.iter().any(|p| {
            ![
                "backup.deliver",
                "company.read",
                "company.write",
                "files.read",
                "files.write",
                "finance.write",
                "goals.write",
                "reports.write",
                "skills.write",
            ]
            .contains(&p.as_str().unwrap_or(""))
        }) {
            return Err(err("invalid_input", "未知插件权限"));
        }
    }
    if v["contributions"].get("operation").is_some() && v.get("runtime").is_none() {
        return Err(err("invalid_input", "可执行能力必须有实际 runtime"));
    }
    validate_contributions(v)?;
    Ok(())
}
fn validate_contributions(manifest: &Value) -> Result<()> {
    let contributions = manifest["contributions"]
        .as_object()
        .ok_or_else(|| err("invalid_input", "contributions 必须是对象"))?;
    for key in contributions.keys() {
        if ![
            "operation",
            "backup_destination",
            "guidance",
            "metrics",
            "views",
            "templates",
            "importers",
            "skills",
        ]
        .contains(&key.as_str())
        {
            return Err(err(
                "unsupported_contribution",
                format!("首版不支持 {key} 扩展声明"),
            ));
        }
    }
    if let Some(idempotent) = manifest.get("runtime").and_then(|r| r.get("idempotent"))
        && !idempotent.is_boolean()
    {
        return Err(err("invalid_input", "runtime.idempotent 必须是布尔值"));
    }
    if let Some(operation) = contributions.get("operation") {
        text(operation, "description")?;
        if !operation.get("input_schema").is_some_and(Value::is_object) {
            return Err(err("invalid_input", "插件操作需要 input_schema 对象"));
        }
        validate_schema_definition(&operation["input_schema"], 0)?;
    }
    for key in [
        "metrics",
        "views",
        "templates",
        "importers",
        "guidance",
        "skills",
    ] {
        if contributions.get(key).is_some_and(|v| !v.is_array()) {
            return Err(err("invalid_input", format!("{key} 必须是数组")));
        }
    }
    if let Some(skills) = contributions.get("skills").and_then(Value::as_array) {
        if skills.len() > 64 {
            return Err(err("invalid_input", "插件最多声明 64 个 Skills"));
        }
        let mut ids = BTreeSet::new();
        for skill in skills {
            company_core::skills::validate_skill(skill)?;
            if skill
                .get("source_refs")
                .is_some_and(|refs| refs.as_array().is_none_or(|refs| !refs.is_empty()))
            {
                return Err(err("invalid_input", "插件 Skill 不得引用私人公司记录"));
            }
            if skill.get("deleted").is_some() {
                return Err(err("invalid_input", "插件 Skill 不接受删除标记"));
            }
            if !ids.insert(text(skill, "id")?) {
                return Err(err("invalid_input", "插件 Skill 标识重复"));
            }
        }
    }
    if let Some(metrics) = contributions.get("metrics").and_then(Value::as_array) {
        if !contributions.contains_key("operation") {
            return Err(err("invalid_input", "指标需要实际 operation 的保存结果"));
        }
        let mut ids = BTreeSet::new();
        for metric in metrics {
            let id = text(metric, "id")?;
            identifier(id)?;
            if !ids.insert(id) {
                return Err(err("invalid_input", "指标标识重复"));
            }
            text(metric, "label")?;
            validate_field(metric)?;
        }
    }
    if let Some(views) = contributions.get("views").and_then(Value::as_array) {
        if !contributions.contains_key("operation") {
            return Err(err("invalid_input", "看板需要实际 operation 的保存结果"));
        }
        let mut ids = BTreeSet::new();
        for view in views {
            let id = text(view, "id")?;
            identifier(id)?;
            if !ids.insert(id) {
                return Err(err("invalid_input", "看板标识重复"));
            }
            text(view, "title")?;
            let fields = view["fields"]
                .as_array()
                .ok_or_else(|| err("invalid_input", "看板需要 fields 数组"))?;
            if fields.is_empty() || fields.len() > 30 {
                return Err(err("invalid_input", "看板字段需要 1 至 30 项"));
            }
            for field in fields {
                text(field, "label")?;
                validate_field(field)?;
            }
        }
    }
    if let Some(templates) = contributions.get("templates").and_then(Value::as_array) {
        for template in templates {
            identifier(text(template, "id")?)?;
            text(template, "name")?;
            let operations: Vec<BatchOperation> =
                serde_json::from_value(template["operations"].clone())
                    .map_err(|_| err("invalid_input", "模板需要标准 operations 批次"))?;
            validate_returned_operations(manifest, &operations)?;
        }
    }
    if let Some(importers) = contributions.get("importers").and_then(Value::as_array) {
        if !contributions.contains_key("operation") {
            return Err(err("invalid_input", "导入扩展需要实际 operation"));
        }
        for importer in importers {
            identifier(text(importer, "id")?)?;
            text(importer, "name")?;
            text(importer, "input_format")?;
        }
    }
    if let Some(destination) = contributions.get("backup_destination") {
        if !["local", "process"].contains(&text(destination, "kind")?) {
            return Err(err(
                "invalid_input",
                "备份目的地 kind 必须是 local 或 process",
            ));
        }
        if destination["kind"] == "process" && manifest.get("runtime").is_none() {
            return Err(err("invalid_input", "进程备份目的地需要 runtime"));
        }
    }
    Ok(())
}
fn validate_field(field: &Value) -> Result<()> {
    let path = text(field, "value_path")?;
    if path.len() > 256
        || path.split('.').any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        })
    {
        return Err(err("invalid_input", "value_path 必须是以点分隔的字段名"));
    }
    if field.get("format").is_some_and(|f| {
        !["number", "money", "percent", "text"].contains(&f.as_str().unwrap_or(""))
    }) {
        return Err(err(
            "invalid_input",
            "字段 format 必须是 number、money、percent 或 text",
        ));
    }
    if field
        .get("unit")
        .is_some_and(|u| u.as_str().is_none_or(|s| s.len() > 32))
    {
        return Err(err("invalid_input", "字段 unit 必须是短文本"));
    }
    Ok(())
}
fn manifest_scopes(manifest: &Value) -> Vec<String> {
    manifest["permissions"]
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .filter(|s| *s != "backup.deliver")
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}
fn validate_schema_definition(schema: &Value, depth: usize) -> Result<()> {
    if depth > 16 || !schema.is_object() {
        return Err(err(
            "invalid_input",
            "插件输入 schema 必须是深度不超过 16 的对象",
        ));
    }
    for key in schema.as_object().unwrap().keys() {
        if ![
            "type",
            "required",
            "properties",
            "items",
            "enum",
            "minimum",
            "maximum",
            "minLength",
            "maxLength",
            "minItems",
            "maxItems",
            "additionalProperties",
            "description",
            "title",
            "default",
            "examples",
            "$schema",
        ]
        .contains(&key.as_str())
        {
            return Err(err(
                "unsupported_schema",
                format!("首版输入 schema 不支持 {key}"),
            ));
        }
    }
    if let Some(types) = schema.get("type") {
        let types = match types {
            Value::String(t) => vec![t.as_str()],
            Value::Array(t) => t
                .iter()
                .map(|v| {
                    v.as_str()
                        .ok_or_else(|| err("invalid_input", "schema type 无效"))
                })
                .collect::<Result<Vec<_>>>()?,
            _ => return Err(err("invalid_input", "schema type 无效")),
        };
        if types.is_empty()
            || types.iter().any(|t| {
                ![
                    "object", "array", "string", "number", "integer", "boolean", "null",
                ]
                .contains(t)
            })
        {
            return Err(err("invalid_input", "schema type 无效"));
        }
    }
    if let Some(required) = schema.get("required") {
        let required = required
            .as_array()
            .ok_or_else(|| err("invalid_input", "schema required 必须是字符串数组"))?;
        if required.iter().any(|v| !v.is_string()) {
            return Err(err("invalid_input", "schema required 必须是字符串数组"));
        }
    }
    if let Some(properties) = schema.get("properties") {
        let properties = properties
            .as_object()
            .ok_or_else(|| err("invalid_input", "schema properties 必须是对象"))?;
        for property in properties.values() {
            validate_schema_definition(property, depth + 1)?;
        }
    }
    if let Some(items) = schema.get("items") {
        validate_schema_definition(items, depth + 1)?;
    }
    if schema
        .get("additionalProperties")
        .is_some_and(|v| !v.is_boolean())
    {
        return Err(err(
            "unsupported_schema",
            "首版 additionalProperties 仅接受布尔值",
        ));
    }
    if schema
        .get("enum")
        .is_some_and(|v| v.as_array().is_none_or(|a| a.is_empty()))
    {
        return Err(err("invalid_input", "schema enum 必须是非空数组"));
    }
    for key in ["minimum", "maximum"] {
        if schema
            .get(key)
            .is_some_and(|v| v.as_f64().is_none_or(|n| !n.is_finite()))
        {
            return Err(err("invalid_input", format!("schema {key} 必须是有限数字")));
        }
    }
    for key in ["minLength", "maxLength", "minItems", "maxItems"] {
        if schema.get(key).is_some_and(|v| v.as_u64().is_none()) {
            return Err(err("invalid_input", format!("schema {key} 必须是非负整数")));
        }
    }
    Ok(())
}
fn validate_plugin_input(schema: &Value, input: &Value, path: &str) -> Result<()> {
    let matches_type = |kind: &str| match kind {
        "object" => input.is_object(),
        "array" => input.is_array(),
        "string" => input.is_string(),
        "number" => input.is_number(),
        "integer" => input.as_i64().is_some() || input.as_u64().is_some(),
        "boolean" => input.is_boolean(),
        "null" => input.is_null(),
        _ => false,
    };
    if let Some(types) = schema.get("type") {
        let valid = if let Some(kind) = types.as_str() {
            matches_type(kind)
        } else {
            types
                .as_array()
                .unwrap()
                .iter()
                .filter_map(Value::as_str)
                .any(matches_type)
        };
        if !valid {
            return Err(err("invalid_input", format!("插件输入 {path} 类型不匹配")));
        }
    }
    if schema
        .get("enum")
        .is_some_and(|values| !values.as_array().unwrap().contains(input))
    {
        return Err(err(
            "invalid_input",
            format!("插件输入 {path} 不在 enum 范围内"),
        ));
    }
    if let Some(object) = input.as_object() {
        if let Some(required) = schema["required"].as_array() {
            for key in required.iter().filter_map(Value::as_str) {
                if !object.contains_key(key) {
                    return Err(err("invalid_input", format!("插件输入缺少 {path}.{key}")));
                }
            }
        }
        let properties = schema["properties"].as_object();
        for (key, value) in object {
            if let Some(property) = properties.and_then(|p| p.get(key)) {
                validate_plugin_input(property, value, &format!("{path}.{key}"))?;
            } else if schema["additionalProperties"] == false {
                return Err(err("invalid_input", format!("插件输入不接受 {path}.{key}")));
            }
        }
    }
    if let Some(array) = input.as_array() {
        for key in ["minItems", "maxItems"] {
            if let Some(bound) = schema[key].as_u64()
                && ((key == "minItems" && array.len() < (bound as usize))
                    || (key == "maxItems" && array.len() > (bound as usize)))
            {
                return Err(err(
                    "invalid_input",
                    format!("插件输入 {path} 数组长度越界"),
                ));
            }
        }
        if let Some(items) = schema.get("items") {
            for (i, item) in array.iter().enumerate() {
                validate_plugin_input(items, item, &format!("{path}[{i}]"))?;
            }
        }
    }
    if let Some(text) = input.as_str() {
        let length = text.chars().count() as u64;
        for key in ["minLength", "maxLength"] {
            if let Some(bound) = schema[key].as_u64()
                && ((key == "minLength" && length < bound)
                    || (key == "maxLength" && length > bound))
            {
                return Err(err(
                    "invalid_input",
                    format!("插件输入 {path} 文本长度越界"),
                ));
            }
        }
    }
    if let Some(number) = input.as_f64()
        && (schema["minimum"].as_f64().is_some_and(|min| number < min)
            || schema["maximum"].as_f64().is_some_and(|max| number > max))
    {
        return Err(err("invalid_input", format!("插件输入 {path} 数值越界")));
    }
    Ok(())
}
fn validate_returned_operations(manifest: &Value, operations: &[BatchOperation]) -> Result<()> {
    if operations.len() > 1000 {
        return Err(err("invalid_input", "插件最多返回 1,000 个操作"));
    }
    let principal = Principal {
        kind: "agent".into(),
        subject: "plugin".into(),
        company_id: Some("company".into()),
        scopes: manifest_scopes(manifest),
    };
    for operation in operations {
        let scope = company_core::operation_scope(&operation.operation)
            .ok_or_else(|| err("unknown_operation", "插件返回未知操作"))?;
        if scope == "admin" || operation.operation.starts_with("plugin.") {
            return Err(err("forbidden", "插件批次不能执行管理或递归插件操作"));
        }
        principal.authorize(Some("company"), scope)?;
        if operation.operation == "skill.put" && operation.request.input.get("copy_from").is_some()
        {
            principal.authorize(Some("company"), "company.read")?;
        }
    }
    Ok(())
}
fn public_manifest(v: &Value) -> Value {
    let mut v = v.clone();
    if let Some(map) = v.as_object_mut() {
        map.remove("runtime");
        map.remove("config");
        map.remove("secrets");
    }
    v
}
fn builtin() -> Value {
    json!({"schema_version":1,"api_version":1,"id":"local-backup","name":"本地备份目的地","version":"0.1.0","author":"TransPlane Company","description":"将已生成的备份快照复制到指定本地目录。","permissions":["backup.deliver"],"contributions":{"backup_destination":{"kind":"local"}},"source":"bundled"})
}
impl App {
    pub async fn plugins(&self) -> Result<Value> {
        self.plugin_catalog().await
    }
    pub async fn plugin_catalog(&self) -> Result<Value> {
        let mut catalog = vec![builtin()];
        let mut installed = Vec::new();
        for row in query!("SELECT * FROM plugins ORDER BY id")
            .fetch_all(&self.pool)
            .await?
        {
            let manifest: Value = serde_json::from_str(&row.try_get::<String, _>("manifest")?)?;
            let public = public_manifest(&manifest);
            if public["id"] != "local-backup" {
                catalog.push(public.clone());
            }
            installed.push(json!({"id":row.try_get::<String,_>("id")?,"version":row.try_get::<String,_>("version")?,"enabled":row.try_get::<i64,_>("enabled")?!=0,"manifest":public,"contributions":manifest["contributions"]}));
        }
        Ok(json!({"catalog":catalog,"installed":installed}))
    }
    pub async fn install_plugin(&self, manifest: Value, config: Value) -> Result<Value> {
        let _guard = self.write_lock.lock().await;
        validate_manifest(&manifest)?;
        if !config.is_object() {
            return Err(err("invalid_input", "插件配置必须是对象"));
        }
        let id = text(&manifest, "id")?;
        let version = text(&manifest, "version")?;
        if let Some(row) = query!(&sql(
            self.pg,
            "SELECT version,manifest FROM plugins WHERE id=?"
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        {
            if row.try_get::<String, _>("version")? != version
                || serde_json::from_str::<Value>(&row.try_get::<String, _>("manifest")?)?
                    != manifest
            {
                return Err(err(
                    "plugin_version_conflict",
                    "已安装其他版本或不同内容，请先处理保留数据并卸载",
                ));
            }
            query!(&sql(self.pg, "UPDATE plugins SET config=? WHERE id=?"))
                .bind(serde_json::to_string(&config)?)
                .bind(id)
                .execute(&self.pool)
                .await?;
            return Ok(json!({"status":"configured","id":id,"version":version}));
        }
        query!(&sql(
            self.pg,
            "INSERT INTO plugins(id,version,enabled,manifest,config) VALUES(?,?,0,?,?)"
        ))
        .bind(id)
        .bind(version)
        .bind(serde_json::to_string(&manifest)?)
        .bind(serde_json::to_string(&config)?)
        .execute(&self.pool)
        .await?;
        Ok(json!({"status":"installed","id":id,"version":version,"enabled":false}))
    }
    pub async fn set_plugin_enabled(&self, id: &str, enabled: bool) -> Result<Value> {
        let _guard = self.write_lock.lock().await;
        let row = query!(&sql(self.pg, "SELECT manifest FROM plugins WHERE id=?"))
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| err("not_found", "插件未安装"))?;
        validate_manifest(&serde_json::from_str(
            &row.try_get::<String, _>("manifest")?,
        )?)?;
        query!(&sql(self.pg, "UPDATE plugins SET enabled=? WHERE id=?"))
            .bind(if enabled { 1i64 } else { 0 })
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(json!({"status":if enabled{"enabled"}else{"disabled"},"id":id,"enabled":enabled}))
    }
    pub async fn uninstall_plugin(&self, id: &str, purge: bool) -> Result<Value> {
        let _guard = self.write_lock.lock().await;
        let row = query!(&sql(self.pg, "SELECT enabled FROM plugins WHERE id=?"))
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| err("not_found", "插件未安装"))?;
        if row.try_get::<i64, _>("enabled")? != 0 {
            return Err(err("plugin_enabled", "请先停用插件"));
        }
        let count: i64 = query!(&sql(
            self.pg,
            "SELECT COUNT(*) AS n FROM plugin_data WHERE plugin_id=?"
        ))
        .bind(id)
        .fetch_one(&self.pool)
        .await?
        .try_get("n")?;
        let mut has_files = false;
        for entry in std::fs::read_dir(self.root.join("companies"))? {
            let path = entry?.path().join("extensions").join(id);
            if path.exists() && std::fs::read_dir(path)?.next().is_some() {
                has_files = true;
            }
        }
        if count > 0 || has_files {
            return Err(err(
                "retained_data",
                "插件仍有保留数据，请先导出并制定迁移或删除方案",
            ));
        }
        if purge {
            return Err(err(
                "invalid_input",
                "卸载不会隐式擦除数据，请使用明确的数据退出流程",
            ));
        }
        query!(&sql(self.pg, "DELETE FROM plugins WHERE id=?"))
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(json!({"status":"uninstalled","id":id}))
    }
    async fn installed_plugin(&self, id: &str) -> Result<(Value, Value)> {
        let row = query!(&sql(
            self.pg,
            "SELECT manifest,config,enabled FROM plugins WHERE id=?"
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| err("not_found", "插件未安装"))?;
        if row.try_get::<i64, _>("enabled")? == 0 {
            return Err(err("plugin_disabled", "插件已停用"));
        }
        Ok((
            serde_json::from_str(&row.try_get::<String, _>("manifest")?)?,
            serde_json::from_str(&row.try_get::<String, _>("config")?)?,
        ))
    }
    pub async fn deliver_backup(&self, id: &str, snapshot: &Path) -> Result<Value> {
        let (manifest, config) = self.installed_plugin(id).await?;
        let snapshot = std::fs::canonicalize(snapshot)?;
        let bytes = std::fs::read(&snapshot)?;
        let _: serde_json::Value = serde_json::from_slice(&bytes)?;
        if !manifest["permissions"]
            .as_array()
            .is_some_and(|a| a.iter().any(|p| p == "backup.deliver"))
        {
            return Err(err("forbidden", "插件没有备份传送能力"));
        }
        if manifest["contributions"]["backup_destination"]["kind"] == "local" {
            let dir = PathBuf::from(text(&config, "destination")?);
            std::fs::create_dir_all(&dir)?;
            let filename = snapshot
                .file_name()
                .ok_or_else(|| err("invalid_path", "备份文件名无效"))?;
            let destination = dir.join(filename);
            if destination.exists() {
                if hash(&std::fs::read(&destination)?) == hash(&bytes) {
                    return Ok(
                        json!({"status":"delivered","path":std::fs::canonicalize(destination)?.to_string_lossy(),"sha256":hash(&bytes),"replayed":true}),
                    );
                }
                return Err(err("destination_conflict", "目的地已有不同文件"));
            }
            atomic_write_new(&destination, &bytes)?;
            return Ok(
                json!({"status":"delivered","path":std::fs::canonicalize(destination)?.to_string_lossy(),"sha256":hash(&bytes)}),
            );
        }
        let result = run_process(
            &manifest,
            json!({"snapshot":snapshot.to_string_lossy(),"config":config}),
        )
        .await?;
        if result["status"] != "delivered" || result["sha256"] != hash(&bytes) {
            return Err(err(
                "plugin_delivery_failed",
                "插件未返回可核对的传送结果；本地快照保留",
            ));
        }
        Ok(result)
    }
    pub async fn discovery(
        &self,
        view: Option<&str>,
        service: Option<&str>,
        operation: Option<&str>,
    ) -> Result<Value> {
        let mut base =
            if view == Some("contract") && operation.is_some_and(|o| o.starts_with("plugin.")) {
                json!({"version":1,"view":"contract"})
            } else {
                company_core::discovery(view, service, operation)?
            };
        let plugins = self.plugin_catalog().await?;
        for item in plugins["installed"].as_array().unwrap() {
            if item["enabled"] != true {
                continue;
            }
            let manifest = &item["manifest"];
            let id = text(manifest, "id")?;
            let service_id = format!("plugin.{id}");
            let op_id = format!("plugin.{id}.call");
            let contributes = manifest["contributions"].get("operation");
            match view.unwrap_or("services") {
                "services" => {
                    let mut contributions = manifest["contributions"].clone();
                    for skill in contributions["skills"].as_array_mut().into_iter().flatten() {
                        if let Some(object) = skill.as_object_mut() {
                            object.remove("body");
                        }
                    }
                    base["items"].as_array_mut().unwrap().push(json!({"id":service_id,"name":manifest["name"],"description":manifest["description"],"contributions":contributions}));
                }
                "operations"
                    if service.is_none_or(|s| s == service_id) && contributes.is_some() =>
                {
                    base["items"].as_array_mut().unwrap().push(json!({"id":op_id,"description":contributes.unwrap()["description"],"scope":"company.write","contract":format!("/api/v1/discovery?view=contract&operation={op_id}")}));
                }
                "contract" if operation == Some(op_id.as_str()) && contributes.is_some() => {
                    base["operation"] = json!({"id":op_id,"service":service_id,"description":contributes.unwrap()["description"],"scope":"company.write","additional_scopes":manifest["permissions"],"http":{"method":"POST","path":format!("/api/v1/companies/{{company_id}}/operations/{op_id}")},"input_schema":{"type":"object","required":["request_id","input"],"additionalProperties":false,"properties":{"request_id":{"type":"string"},"expected_revision":{"type":["integer","null"]},"input":contributes.unwrap()["input_schema"]}},"result":{"status":"committed","request_id":"string","result":"插件 JSON","applied":"通过平台校验后提交的经营操作"},"rules":["执行插件按可信本地程序安装","相同 request_id 与输入原样重试；completed 返回保存结果","pending 稍后查询；unknown 仅在插件声明 runtime.idempotent=true 时允许重复执行","非幂等插件结果未知时由主人核对，再以原请求摘要 reconcile 登记","插件返回 operations 时平台生成稳定的子请求键并原子提交，禁止管理与跨公司操作"]});
                }
                _ => {}
            }
        }
        if view == Some("contract") && base.get("operation").is_none() {
            return Err(err("not_found", "插件能力未安装、停用或未提供实际执行能力"));
        }
        Ok(base)
    }
    pub async fn agent_guide(&self, topic: &str) -> Result<Value> {
        if let Some(id) = topic.strip_prefix("plugin.") {
            let (manifest, _) = self.installed_plugin(id).await?;
            return Ok(
                json!({"version":1,"topic":topic,"purpose":manifest["description"],"steps":manifest["contributions"].get("guidance").cloned().unwrap_or(json!([])),"templates":manifest["contributions"].get("templates").cloned().unwrap_or(json!([])),"importers":manifest["contributions"].get("importers").cloned().unwrap_or(json!([])),"maintenance":["按任务读取所需模板；模板 operations 使用正常公司导入接口，由当前授权 Agent 提交","保持原 request_id；未知结果先查询，非幂等外部执行需主人核对后 reconcile"],"discovery":format!("/api/v1/discovery?view=operations&service=plugin.{id}")}),
            );
        }
        Ok(company_core::guide(topic)?)
    }
    pub async fn plugin_permissions(&self, operation: &str) -> Result<Vec<String>> {
        let id = plugin_id(operation)?;
        let (manifest, _) = self.installed_plugin(id).await?;
        if manifest["contributions"].get("operation").is_none() {
            return Err(err("unknown_operation", "插件没有此执行能力"));
        }
        Ok(manifest["permissions"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default())
    }
    pub async fn plugin_request(&self, company: &str, id: &str, request_id: &str) -> Result<Value> {
        self.company(company).await?;
        let row = query!(&sql(
            self.pg,
            "SELECT data FROM plugin_data WHERE company_id=? AND plugin_id=? AND id=?"
        ))
        .bind(company)
        .bind(id)
        .bind(request_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| err("not_found", "插件请求不存在"))?;
        Ok(serde_json::from_str(&row.try_get::<String, _>("data")?)?)
    }
    pub async fn remove_plugin_data(
        &self,
        company: &str,
        id: &str,
        snapshot: &Path,
        expected_sha256: &str,
    ) -> Result<Value> {
        self.company(company).await?;
        if std::fs::metadata(snapshot)?.len() > 512 * 1024 * 1024 {
            return Err(err("capacity_exceeded", "备份包超过 512 MiB"));
        }
        let bytes = std::fs::read(snapshot)?;
        if hash(&bytes) != expected_sha256 {
            return Err(err("integrity_error", "备份文件摘要与已确认的 sha256 不符"));
        }
        let backup = super::portability::verified_backup_content(snapshot)?;
        let backup_rows = backup["plugin_data"]
            .as_array()
            .ok_or_else(|| err("integrity_error", "备份缺少插件数据索引"))?;
        let _guard = self.write_lock.lock().await;
        let mut tx = self.writer_transaction().await?;
        let plugin = query!(&sql(self.pg, "SELECT enabled FROM plugins WHERE id=?"))
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| err("not_found", "插件未安装"))?;
        if plugin.try_get::<i64, _>("enabled")? != 0 {
            return Err(err("plugin_enabled", "请先停用插件再退出数据"));
        }
        let rows = query!(&sql(
            self.pg,
            "SELECT id,data FROM plugin_data WHERE company_id=? AND plugin_id=? ORDER BY id"
        ))
        .bind(company)
        .bind(id)
        .fetch_all(&mut *tx)
        .await?;
        for row in &rows {
            let request: String = row.try_get("id")?;
            let saved: Value = serde_json::from_str(&row.try_get::<String, _>("data")?)?;
            if saved["state"] == "pending" {
                return Err(err(
                    "plugin_pending",
                    "插件仍有待确认请求；先核对执行结果，再退出数据。",
                ));
            }
            if !backup_rows.iter().any(|r| {
                r["company_id"] == company
                    && r["plugin_id"] == id
                    && r["id"] == request
                    && r["data"] == saved
            }) {
                return Err(err(
                    "backup_required",
                    "此插件的最新数据未包含在指定备份中，请重新备份。",
                ));
            }
        }
        let mut removed = 0;
        for row in &rows {
            let deleted = query!(&sql(
                self.pg,
                "DELETE FROM plugin_data WHERE company_id=? AND plugin_id=? AND id=? AND data=?"
            ))
            .bind(company)
            .bind(id)
            .bind(row.try_get::<String, _>("id")?)
            .bind(row.try_get::<String, _>("data")?)
            .execute(&mut *tx)
            .await?;
            if deleted.rows_affected() != 1 {
                return Err(err("revision_conflict", "插件数据在退出时发生变化"));
            }
            removed += 1;
        }
        tx.commit().await?;
        let files = self
            .root
            .join("companies")
            .join(company)
            .join("extensions")
            .join(id);
        let retained_files = if files.exists() && std::fs::read_dir(&files)?.next().is_some() {
            vec![files.to_string_lossy().into_owned()]
        } else {
            vec![]
        };
        Ok(
            json!({"status":"data_exited","company_id":company,"plugin_id":id,"removed_requests":removed,"backup_sha256":expected_sha256,"retained_files":retained_files}),
        )
    }
    pub(super) async fn call_plugin(
        &self,
        company: &str,
        operation: &str,
        request: OperationRequest,
        actor: &str,
    ) -> Result<Value> {
        let id = plugin_id(operation)?;
        self.company(company).await?;
        identifier(&request.request_id)?;
        let (manifest, config) = self.installed_plugin(id).await?;
        if manifest["contributions"].get("operation").is_none() {
            return Err(err("unknown_operation", "插件没有此执行能力"));
        }
        validate_plugin_input(
            &manifest["contributions"]["operation"]["input_schema"],
            &request.input,
            "input",
        )?;
        let key_hash = hash(&serde_json::to_vec(
            &json!({"operation":operation,"request":request}),
        )?);
        let attempt = new_id();
        {
            let _guard = self.write_lock.lock().await;
            let mut tx = self.writer_transaction().await?;
            if let Some(row) = query!(&sql(
                self.pg,
                "SELECT data FROM plugin_data WHERE company_id=? AND plugin_id=? AND id=?"
            ))
            .bind(company)
            .bind(id)
            .bind(&request.request_id)
            .fetch_optional(&mut *tx)
            .await?
            {
                let saved: Value = serde_json::from_str(&row.try_get::<String, _>("data")?)?;
                if saved["input_hash"] != key_hash {
                    return Err(err("request_id_conflict", "插件请求键对应不同输入"));
                }
                if saved["state"] == "completed"
                    || (saved.get("state").is_none() && saved.get("result").is_some())
                {
                    let mut result = saved["result"].clone();
                    result["replayed"] = json!(true);
                    return Ok(result);
                }
                let age = saved["started_at"]
                    .as_str()
                    .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                    .map(|t| Utc::now().signed_duration_since(t).num_seconds())
                    .unwrap_or(120);
                if saved["state"] == "pending" && age < 120 {
                    return Err(err(
                        "plugin_pending",
                        "此请求正在执行；保持原 request_id，稍后查询结果。",
                    ));
                }
                if manifest["runtime"]["idempotent"] != true {
                    return Err(err(
                        "plugin_result_unknown",
                        "此前执行结果未确认；保持原 request_id，由主人核对插件结果后通过 reconcile 登记。",
                    ));
                }
                let pending = json!({"state":"pending","input_hash":key_hash,"attempt":attempt,"started_at":now(),"actor":actor});
                query!(&sql(
                    self.pg,
                    "UPDATE plugin_data SET data=? WHERE company_id=? AND plugin_id=? AND id=?"
                ))
                .bind(serde_json::to_string(&pending)?)
                .bind(company)
                .bind(id)
                .bind(&request.request_id)
                .execute(&mut *tx)
                .await?;
            } else {
                let pending = json!({"state":"pending","input_hash":key_hash,"attempt":attempt,"started_at":now(),"actor":actor});
                let inserted=query!(&sql(self.pg,"INSERT INTO plugin_data(company_id,plugin_id,id,data) VALUES(?,?,?,?) ON CONFLICT(company_id,plugin_id,id) DO NOTHING")).bind(company).bind(id).bind(&request.request_id).bind(serde_json::to_string(&pending)?).execute(&mut *tx).await?;
                if inserted.rows_affected() != 1 {
                    return Err(err(
                        "plugin_pending",
                        "此请求已由另一个进程接受；保持原 request_id 后重试。",
                    ));
                }
            }
            tx.commit().await?;
        }
        let scopes = manifest_scopes(&manifest);
        let grant = if scopes.is_empty() {
            None
        } else {
            match self.callback_token(company, scopes.clone()).await {
                Ok(grant) => Some(grant),
                Err(error) => {
                    self.mark_plugin_unknown(company, id, &request.request_id, &attempt)
                        .await?;
                    return Err(error);
                }
            }
        };
        let base = config
            .get("api_base")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| std::env::var("COMPANY_PUBLIC_ORIGIN").ok())
            .unwrap_or_else(|| "http://127.0.0.1:4177".into());
        let api = json!({"base_url":base,"company_id":company,"scopes":scopes,"token":grant.as_ref().and_then(|g|g.get("token"))});
        // Only the process phase runs outside the writer lock. Callback API writes use normal transactions.
        let execution=run_process(&manifest,json!({"company_id":company,"request_id":request.request_id,"input":request.input,"config":config,"actor":actor,"api":api})).await;
        if let Some(grant) = grant {
            let token_id = text(&grant, "id")?;
            if let Err(error) = self.revoke_token(token_id).await {
                self.mark_plugin_unknown(company, id, &request.request_id, &attempt)
                    .await?;
                return Err(error);
            }
        }
        match execution {
            Ok(output) => match self
                .finish_plugin(
                    company,
                    id,
                    &request.request_id,
                    &key_hash,
                    &attempt,
                    output,
                    actor,
                    false,
                )
                .await
            {
                Ok(value) => Ok(value),
                Err(error) => {
                    self.mark_plugin_unknown(company, id, &request.request_id, &attempt)
                        .await?;
                    Err(error)
                }
            },
            Err(error) => {
                self.mark_plugin_unknown(company, id, &request.request_id, &attempt)
                    .await?;
                Err(error)
            }
        }
    }
    async fn callback_token(&self, company: &str, scopes: Vec<String>) -> Result<Value> {
        let mut grant = self.grant_token(Some(company), scopes, false).await?;
        let old = text(&grant, "token")?.to_owned();
        let expiry = Utc::now().timestamp() + 120;
        let token = format!(
            "tpc_exp_{expiry}_{}",
            old.strip_prefix("tpc_").unwrap_or(&old)
        );
        let changed = {
            let _guard = self.write_lock.lock().await;
            query!(&sql(
                self.pg,
                "UPDATE credentials SET token_hash=? WHERE id=? AND token_hash=?"
            ))
            .bind(hash(token.as_bytes()))
            .bind(text(&grant, "id")?)
            .bind(hash(old.as_bytes()))
            .execute(&self.pool)
            .await
        };
        match changed {
            Ok(changed) if changed.rows_affected() == 1 => {
                grant["token"] = json!(token);
                grant["expires_at"] = json!(expiry);
                Ok(grant)
            }
            Ok(_) => {
                self.revoke_token(text(&grant, "id")?).await?;
                Err(err("unauthorized", "插件回调凭据在建立时已变化"))
            }
            Err(error) => {
                self.revoke_token(text(&grant, "id")?).await?;
                Err(error.into())
            }
        }
    }
    async fn mark_plugin_unknown(
        &self,
        company: &str,
        id: &str,
        request_id: &str,
        attempt: &str,
    ) -> Result<()> {
        let _guard = self.write_lock.lock().await;
        let mut tx = self.writer_transaction().await?;
        if let Some(row) = query!(&sql(
            self.pg,
            "SELECT data FROM plugin_data WHERE company_id=? AND plugin_id=? AND id=?"
        ))
        .bind(company)
        .bind(id)
        .bind(request_id)
        .fetch_optional(&mut *tx)
        .await?
        {
            let mut value: Value = serde_json::from_str(&row.try_get::<String, _>("data")?)?;
            if value["attempt"] == attempt && value["state"] == "pending" {
                value["state"] = json!("unknown");
                value["updated_at"] = json!(now());
                query!(&sql(
                    self.pg,
                    "UPDATE plugin_data SET data=? WHERE company_id=? AND plugin_id=? AND id=?"
                ))
                .bind(serde_json::to_string(&value)?)
                .bind(company)
                .bind(id)
                .bind(request_id)
                .execute(&mut *tx)
                .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }
    pub async fn reconcile_plugin(
        &self,
        company: &str,
        id: &str,
        request_id: &str,
        result: Value,
    ) -> Result<Value> {
        // Owner acknowledgement includes the hash displayed by plugin_request; it never re-executes external code.
        let input_hash = text(&result, "input_hash")?;
        let output = result
            .get("output")
            .ok_or_else(|| err("invalid_input", "核对登记需要 output"))?
            .clone();
        let saved = self.plugin_request(company, id, request_id).await?;
        if saved["input_hash"] != input_hash {
            return Err(err("request_id_conflict", "请求摘要已变化"));
        }
        if saved["state"] == "completed" {
            let mut result = saved["result"].clone();
            result["replayed"] = json!(true);
            return Ok(result);
        }
        if saved["state"] == "pending" {
            let age = saved["started_at"]
                .as_str()
                .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                .map(|t| Utc::now().signed_duration_since(t).num_seconds())
                .unwrap_or(0);
            if age < 120 {
                return Err(err(
                    "plugin_pending",
                    "请求仍在执行或等待完成，请稍后再核对登记。",
                ));
            }
        }
        let attempt = text(&saved, "attempt")?;
        self.finish_plugin(
            company,
            id,
            request_id,
            input_hash,
            attempt,
            output,
            "owner:reconciliation",
            true,
        )
        .await
    }
    // Completion checks the exact company, request hash and reserved attempt together.
    #[allow(clippy::too_many_arguments)]
    async fn finish_plugin(
        &self,
        company: &str,
        id: &str,
        request_id: &str,
        input_hash: &str,
        attempt: &str,
        output: Value,
        actor: &str,
        reconciled: bool,
    ) -> Result<Value> {
        if serde_json::to_vec(&output)?.len() > 1024 * 1024 {
            return Err(err("plugin_output_too_large", "插件结果超过 1 MiB"));
        }
        let manifest = if reconciled {
            let row = query!(&sql(self.pg, "SELECT manifest FROM plugins WHERE id=?"))
                .bind(id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or_else(|| err("not_found", "插件未安装"))?;
            serde_json::from_str(&row.try_get::<String, _>("manifest")?)?
        } else {
            self.installed_plugin(id).await?.0
        };
        let mut operations = match output.get("operations") {
            Some(value) => serde_json::from_value::<Vec<BatchOperation>>(value.clone())
                .map_err(|_| err("invalid_input", "插件 operations 必须符合标准批次合同"))?,
            None => vec![],
        };
        validate_returned_operations(&manifest, &operations)?;
        for (index, operation) in operations.iter_mut().enumerate() {
            operation.request.request_id = format!(
                "plugin-{}-{}-{index}",
                hash(id.as_bytes())[..12].to_owned(),
                hash(request_id.as_bytes())[..24].to_owned()
            );
        }
        let _guard = self.write_lock.lock().await;
        let mut tx = self.writer_transaction().await?;
        let row = query!(&sql(
            self.pg,
            "SELECT data FROM plugin_data WHERE company_id=? AND plugin_id=? AND id=?"
        ))
        .bind(company)
        .bind(id)
        .bind(request_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| err("not_found", "插件请求不存在"))?;
        let saved: Value = serde_json::from_str(&row.try_get::<String, _>("data")?)?;
        if saved["input_hash"] != input_hash || saved["attempt"] != attempt {
            return Err(err("request_id_conflict", "插件请求的执行状态已变化"));
        }
        if saved["state"] == "completed" {
            let mut value = saved["result"].clone();
            value["replayed"] = json!(true);
            return Ok(value);
        }
        if !["pending", "unknown"].contains(&saved["state"].as_str().unwrap_or("")) {
            return Err(err("plugin_result_unknown", "插件请求需要核对"));
        }
        let mut applied = Vec::new();
        for operation in operations {
            applied.push(
                self.execute_tx(
                    &mut tx,
                    Some(company),
                    &operation.operation,
                    operation.request,
                    &format!("plugin:{id}:for:{actor}"),
                )
                .await?,
            );
        }
        let result = json!({"status":if reconciled{"reconciled"}else{"committed"},"replayed":false,"request_id":request_id,"result":output,"applied":applied});
        let record = json!({"state":"completed","input_hash":input_hash,"attempt":attempt,"started_at":saved["started_at"],"completed_at":now(),"actor":actor,"reconciled":reconciled,"result":result});
        query!(&sql(
            self.pg,
            "UPDATE plugin_data SET data=? WHERE company_id=? AND plugin_id=? AND id=?"
        ))
        .bind(serde_json::to_string(&record)?)
        .bind(company)
        .bind(id)
        .bind(request_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        for result in &applied {
            self.materialize_result(result)?;
        }
        Ok(result)
    }
    pub async fn plugin_extensions(&self, company: &str) -> Result<Vec<Value>> {
        let installed = self.plugin_catalog().await?;
        let mut latest: BTreeMap<String, (String, String, Value)> = BTreeMap::new();
        for row in query!(&sql(
            self.pg,
            "SELECT plugin_id,id,data FROM plugin_data WHERE company_id=?"
        ))
        .bind(company)
        .fetch_all(&self.pool)
        .await?
        {
            let data: Value = serde_json::from_str(&row.try_get::<String, _>("data")?)?;
            if data["state"] != "completed" {
                continue;
            }
            let plugin: String = row.try_get("plugin_id")?;
            let request: String = row.try_get("id")?;
            let timestamp = data["completed_at"].as_str().unwrap_or("").to_owned();
            if latest.get(&plugin).is_none_or(|old| timestamp > old.0) {
                latest.insert(
                    plugin,
                    (timestamp, request, data["result"]["result"].clone()),
                );
            }
        }
        let mut extensions = Vec::new();
        for plugin in installed["installed"].as_array().unwrap() {
            let id = text(plugin, "id")?;
            let Some((timestamp, request, output)) = latest.get(id) else {
                continue;
            };
            let manifest = &plugin["manifest"];
            let contribution = &manifest["contributions"];
            let metrics = contribution["metrics"]
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .map(|metric| render_field(metric, output))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let views=contribution["views"].as_array().map(|items|items.iter().map(|view|json!({"id":view["id"],"title":view["title"],"fields":view["fields"].as_array().unwrap().iter().map(|field|render_field(field,output)).collect::<Vec<_>>()})).collect::<Vec<_>>()).unwrap_or_default();
            if !metrics.is_empty() || !views.is_empty() {
                extensions.push(json!({"id":id,"name":manifest["name"],"enabled":plugin["enabled"],"request_id":request,"updated_at":timestamp,"metrics":metrics,"views":views}));
            }
        }
        Ok(extensions)
    }
}
fn plugin_id(operation: &str) -> Result<&str> {
    operation
        .strip_prefix("plugin.")
        .and_then(|s| s.strip_suffix(".call"))
        .filter(|id| !id.is_empty())
        .ok_or_else(|| err("unknown_operation", "未知插件操作"))
}
fn render_field(field: &Value, output: &Value) -> Value {
    let mut value = output;
    for key in field["value_path"].as_str().unwrap_or("").split('.') {
        value = value.get(key).unwrap_or(&Value::Null);
    }
    let value = if value.is_number() || value.is_string() || value.is_boolean() {
        value.clone()
    } else {
        Value::Null
    };
    json!({"id":field.get("id"),"label":field["label"],"value":value,"format":field.get("format").cloned().unwrap_or(json!("number")),"unit":field.get("unit")})
}
async fn run_process(manifest: &Value, input: Value) -> Result<Value> {
    run_process_timeout(manifest, input, Duration::from_secs(30)).await
}
async fn run_process_timeout(manifest: &Value, input: Value, deadline: Duration) -> Result<Value> {
    let args = manifest["runtime"]["command"]
        .as_array()
        .ok_or_else(|| err("plugin_runtime_missing", "插件没有执行入口"))?;
    let program = args
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| err("invalid_input", "插件执行入口无效"))?;
    let bytes = serde_json::to_vec(&input)?;
    if bytes.len() > 24 * 1024 * 1024 {
        return Err(err("payload_too_large", "插件输入超过 24 MiB"));
    }
    tokio::time::timeout(deadline, async {
        let mut command = tokio::process::Command::new(program);
        for arg in &args[1..] {
            command.arg(
                arg.as_str()
                    .ok_or_else(|| err("invalid_input", "插件参数无效"))?,
            );
        }
        command
            .kill_on_drop(true)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        let mut child = command
            .spawn()
            .map_err(|_| err("plugin_start_failed", "无法启动插件程序"))?;
        let mut stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let write = async move {
            stdin.write_all(&bytes).await?;
            stdin.write_all(b"\n").await?;
            stdin.shutdown().await?;
            drop(stdin);
            Ok::<_, anyhow::Error>(())
        };
        let read = async {
            let mut output = Vec::new();
            stdout
                .take(1024 * 1024 + 1)
                .read_to_end(&mut output)
                .await?;
            if output.len() > 1024 * 1024 {
                return Err(err("plugin_output_too_large", "插件输出超过 1 MiB"));
            }
            Ok::<_, anyhow::Error>(output)
        };
        let wait = async {
            let exit = child.wait().await?;
            if !exit.success() {
                return Err(err("plugin_failed", "插件执行失败，需核对外部结果。"));
            }
            Ok::<_, anyhow::Error>(())
        };
        let (_, output, _) = tokio::try_join!(write, read, wait)?;
        serde_json::from_slice::<Value>(&output)
            .map_err(|_| err("invalid_plugin_output", "插件必须返回一个有效 JSON 结果。"))
    })
    .await
    .map_err(|_| err("plugin_timeout", "插件执行超过 30 秒，需核对外部结果。"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    fn python() -> String {
        ["python", "python3"]
            .into_iter()
            .find(|command| {
                std::process::Command::new(command)
                    .arg("--version")
                    .output()
                    .is_ok_and(|r| r.status.success())
            })
            .expect("Python 3 is required for executable plugin tests")
            .into()
    }
    #[tokio::test]
    async fn a_plugin_that_never_reads_stdin_is_bounded_by_the_whole_process_deadline() {
        let manifest = json!({"runtime":{"command":[python(),"-c","import time; time.sleep(5)"]}});
        let started = std::time::Instant::now();
        let error = run_process_timeout(
            &manifest,
            json!({"padding":"x".repeat(2*1024*1024)}),
            Duration::from_millis(200),
        )
        .await
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<DomainError>().unwrap().code,
            "plugin_timeout"
        );
        assert!(started.elapsed() < Duration::from_secs(3));
    }
    #[tokio::test]
    async fn ephemeral_tokens_expire_even_if_a_process_crashes_before_revocation() {
        let dir = tempfile::TempDir::new().unwrap();
        let app = App::open(AppConfig {
            data_dir: dir.path().to_owned(),
            database_url: None,
        })
        .await
        .unwrap();
        app.execute(None,"company.create",OperationRequest{request_id:"create-expiry".into(),expected_revision:None,input:json!({"id":"alpha","name":"Alpha","currency":"CNY","timezone":"Asia/Shanghai","opening_date":"2026-07-01"})},"local:test").await.unwrap();
        let issued = app
            .callback_token("alpha", vec!["company.read".into()])
            .await
            .unwrap();
        let token = text(&issued, "token").unwrap();
        assert!(app.authenticate(token).await.is_ok());
        let random = token
            .strip_prefix("tpc_exp_")
            .unwrap()
            .split_once('_')
            .unwrap()
            .1;
        let expired = format!("tpc_exp_{}_{random}", Utc::now().timestamp() - 1);
        query!(&sql(
            app.pg,
            "UPDATE credentials SET token_hash=? WHERE id=?"
        ))
        .bind(hash(expired.as_bytes()))
        .bind(text(&issued, "id").unwrap())
        .execute(&app.pool)
        .await
        .unwrap();
        assert!(app.authenticate(&expired).await.is_err());
    }
}
