use serde_json::{Value, json};

pub fn operation_ids() -> Vec<&'static str> {
    vec![
        "agent-link.status",
        "agent-link.connect",
        "agent-link.pending-work",
        "agent-link.ack",
        "agent-link.disconnect",
    ]
}

pub fn operation_scope(operation: &str) -> Option<&'static str> {
    match operation {
        "agent-link.status" => Some("company.read"),
        "agent-link.connect"
        | "agent-link.pending-work"
        | "agent-link.ack"
        | "agent-link.disconnect" => Some("company.write"),
        _ => None,
    }
}

pub fn operation_contract(operation: &str) -> Option<Value> {
    let (description, method, suffix, required, properties) = match operation {
        "agent-link.status" => (
            "查看外部 Agent 的连接和交接状态",
            "GET",
            "",
            vec![],
            json!({}),
        ),
        "agent-link.connect" => (
            "建立或续期外部 Agent 连接",
            "POST",
            "/connect",
            vec!["label"],
            json!({
                "label":{"type":"string","minLength":1,"maxLength":120},
                "lease_seconds":{"type":"integer","minimum":1,"maximum":120,"default":60},
                "session_id":{"type":"string","description":"续期时使用当前连接返回的 session_id"}
            }),
        ),
        "agent-link.pending-work" => (
            "拉取当前待办及准确回执",
            "GET",
            "/pending-work",
            vec!["session_id"],
            json!({
                "session_id":{"type":"string"},
                "after_revision":{"type":"integer","minimum":0},
                "after_request_key":{"type":"string","maxLength":512},
                "wait_ms":{"type":"integer","minimum":0,"maximum":25000,"default":0}
            }),
        ),
        "agent-link.ack" => (
            "确认已收到当前准确待办",
            "POST",
            "/ack",
            vec!["session_id", "request_key", "receipt_id"],
            json!({
                "session_id":{"type":"string"},"request_key":{"type":"string"},"receipt_id":{"type":"string"}
            }),
        ),
        "agent-link.disconnect" => (
            "断开当前外部 Agent 连接",
            "POST",
            "/disconnect",
            vec!["session_id"],
            json!({"session_id":{"type":"string"}}),
        ),
        _ => return None,
    };
    let scope = operation_scope(operation)?;
    let mut contract = json!({
        "id":operation,"operation":operation,"service":"agent-link","description":description,
        "kind":"runtime","scope":scope,
        "required_scopes":if operation=="agent-link.status" {vec!["company.read"]}else{vec!["company.read","company.write"]},
        "http":{"method":method,"path":format!("/api/v1/companies/{{company_id}}/agent-link{suffix}")},
        "input_schema":{"type":"object","required":required,"additionalProperties":false,"properties":properties},
        "transport":"http","supports_import":false,
        "rules":[
            "外部客户端主动连接和拉取；平台不启动模型、宿主或定时任务",
            "连接、拉取、回执与断开不改变经营记录、公司版本或现金；连接到期、重启和恢复后失效",
            "连接和拉取不表示接手选择；明确 ACK 只表示收到，decision.claim 才表示接手",
            "每家公司同一时刻只连接一个宿主；续期沿用 session_id，已接手选择不重新派送",
            "回执必须对应当前 request_key 与 receipt_id；改选后的旧回执返回冲突",
            "执行失败、超时或结果不明时先核对准确记录，不自动重新执行外部行动"
        ],
        "errors":["unauthorized","forbidden","not_found","invalid_input","conflict"],
        "guide":"/api/v1/agent-guide?topic=agent-link"
    });
    contract["result"] = match operation {
        "agent-link.status" => {
            json!({"company_id":"string","data_revision":"integer","phase":"company.play phase",
            "state":["awaiting_choice","handoff_required","queued","delivered","processing","interrupted","idle"],
            "connected":"boolean","label":"string or null","session_id":"string or null","lease_expires_at":"datetime or null",
            "request_key":"string or null","pending_work":"current intent or null","current_claim":"saved decision claim or null","received_receipt_id":"string or null"})
        }
        "agent-link.pending-work" => {
            json!({"company_id":"string","data_revision":"integer","changed":"boolean",
            "link":"agent-link.status result","work":"kind, request_key, receipt_id and selection context, or null","play":"authoritative company.play including player_output"})
        }
        "agent-link.connect" => {
            json!({"company_id":"string","session_id":"string","label":"string","connected":true,"lease_seconds":"integer","lease_expires_at":"datetime"})
        }
        "agent-link.ack" => {
            json!({"acknowledged":true,"request_key":"string","receipt_id":"string","replayed":"boolean","link":"agent-link.status result"})
        }
        _ => json!({"company_id":"string","session_id":"string","connected":false}),
    };
    Some(contract)
}

pub fn guide(topic: &str) -> Option<Value> {
    (topic == "agent-link").then(|| json!({"topic":topic,"purpose":"连接能持续接收经营选择的外部 Agent 宿主","steps":[
        "使用获授权公司的读写凭据建立连接；凭据只放在客户端安全配置中",
        "运行 scripts/agent-host.py 或实现这些 HTTP 合同，持续拉取待办并在租期内续期",
        "接收待办后持久记录准确 request_key，再提交 ACK；选择处理前核对最新事件并以 decision.claim 接手",
        "将 player_output、story_context 和当前事件传给自己的 Agent；接手权限不等于外部资金或消息发送授权",
        "结算时保存准确结果引用，尽量通过同一次原子导入结算并准备下一事件；预备草稿不能提前改变主账",
        "短篇收束后提供继续或暂停的方向，不替玩家决定新的目标；失败或结果不明时核对记录后再恢复"
    ],"discovery":"/api/v1/discovery?view=operations&service=agent-link"}))
}
