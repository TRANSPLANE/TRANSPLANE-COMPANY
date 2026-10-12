use super::*;
use company_core::Locale;

pub const COMPANY_VIEWS: [&str; 10] = [
    "overview",
    "operations",
    "growth",
    "simulation",
    "reports",
    "files",
    "chronicle",
    "plugins",
    "network",
    "command",
];

pub fn company_view_links(company_id: &str, locale: Locale) -> Value {
    let names = match locale {
        Locale::ZhCn => [
            "经营游戏",
            "经营",
            "成长",
            "推演",
            "报告",
            "资料",
            "经营历程",
            "插件",
            "关系网",
            "任务与信笺",
        ],
        Locale::En => [
            "Play",
            "Operations",
            "Growth",
            "Simulation",
            "Reports",
            "Files",
            "Chronicle",
            "Plugins",
            "Relationships",
            "Quests & Letters",
        ],
    };
    json!(
        COMPANY_VIEWS
            .iter()
            .zip(names)
            .map(|(view, name)| {
                json!({"view":view,"label":name,"href":format!("/?company={company_id}#{view}")})
            })
            .collect::<Vec<_>>()
    )
}

impl App {
    /// A read-only, authoritative projection for playing through an external Agent.
    /// A record's own revision is retained in each command template. The company
    /// revision describes the snapshot; it is not a counter the Agent increments.
    pub async fn play_state_localized(&self, company_id: &str, locale: Locale) -> Result<Value> {
        identifier(company_id)?;
        let snapshot = self.snapshot(company_id).await?;
        let company = &snapshot["company"];
        let metrics = &snapshot["metrics"];
        let mut decisions: Vec<&Value> = snapshot["records"]["decision"]
            .as_array()
            .into_iter()
            .flatten()
            .collect();
        decisions.sort_by_key(|r| std::cmp::Reverse(r["company_revision"].as_i64().unwrap_or(0)));
        let active = decisions.iter().copied().find(|r| {
            matches!(
                r["data"]["status"].as_str(),
                Some("open" | "selected" | "processing")
            )
        });
        let resolved = decisions
            .iter()
            .copied()
            .find(|r| r["data"]["status"] == "resolved");
        let phase = match active.map(|r| r["data"]["status"].as_str().unwrap_or("")) {
            Some("open") => "awaiting_choice",
            Some("selected") => "awaiting_agent",
            Some("processing") => "agent_processing",
            _ => "awaiting_event",
        };
        let mut current_event = Value::Null;
        if let Some(event) = active {
            current_event = event.clone();
            current_event["selection_commands"] =
                if matches!(event["data"]["status"].as_str(), Some("open" | "selected")) {
                    json!(
                        event["data"]["options"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|option| {
                                json!({
                                    "expression":option["title"],
                                    "operation":"decision.select",
                                    "request":{"request_id":"<new-stable-request-id>",
                                        "expected_revision":event["revision"],
                                        "input":{"id":event["id"],"option_id":option["id"]}}
                                })
                            })
                            .collect::<Vec<_>>()
                    )
                } else {
                    json!([])
                };
            current_event["claim_command"] = if event["data"]["status"] == "selected" {
                json!({"operation":"decision.claim","request":{"request_id":"<new-stable-request-id>",
                    "expected_revision":event["revision"],"input":{"id":event["id"]}}})
            } else {
                Value::Null
            };
        }
        let mut latest_result = Value::Null;
        if let Some(event) = resolved {
            let mut references = Vec::new();
            for reference in event["data"]["resolution"]["result_refs"]
                .as_array()
                .into_iter()
                .flatten()
            {
                let row = query!(&sql(self.pg,
                    "SELECT data FROM record_versions WHERE company_id=? AND kind=? AND id=? AND revision=?"))
                    .bind(company_id)
                    .bind(text(reference, "kind")?)
                    .bind(text(reference, "id")?)
                    .bind(integer(reference, "revision")?)
                    .fetch_optional(&self.pool).await?
                    .ok_or_else(|| err("invalid_result_reference", "结算引用的历史记录不存在"))?;
                references.push(serde_json::from_str::<Value>(
                    &row.try_get::<String, _>("data")?,
                )?);
            }
            let revision = integer(&event["data"]["resolution"], "company_revision")?;
            let result_snapshot = self.snapshot_revision(company_id, revision).await?;
            let before_revision = event["data"]["selection"]["company_revision"]
                .as_i64()
                .unwrap_or_else(|| event["data"]["baseline_revision"].as_i64().unwrap_or(1));
            let before_snapshot = self.snapshot_revision(company_id, before_revision).await?;
            latest_result = json!({"decision":event,"records":references,
                "data_revision":revision,"state":result_snapshot["metrics"],
                "before_data_revision":before_revision,"before_state":before_snapshot["metrics"]});
        } else if let Some(turn) = snapshot["records"]["simulation_turn"]
            .as_array()
            .into_iter()
            .flatten()
            .max_by_key(|record| record["company_revision"].as_i64().unwrap_or(0))
        {
            let revision = integer(&turn["data"], "after_revision")?;
            let result_snapshot = self.snapshot_revision(company_id, revision).await?;
            latest_result = json!({"legacy":true,"records":[turn],"data_revision":revision,"state":result_snapshot["metrics"]});
        }
        let goals = snapshot["records"]["goal"].as_array().into_iter().flatten().map(|goal| {
            let progress = metrics["goal_progress"].as_array().into_iter().flatten()
                .find(|progress| progress["id"] == goal["id"]);
            json!({"id":goal["id"],"revision":goal["revision"],"title":goal["data"]["title"],"progress":progress})
        }).collect::<Vec<_>>();
        let labels = match locale {
            Locale::ZhCn => (
                "请选择一个方案，或提出自己的经营方案。",
                "已保存选择，Agent 处理前可更改。",
                "由 Agent 依据当前状态准备下一事件。",
                "Agent 已领取此选择，等待提交结果。",
            ),
            Locale::En => (
                "Choose an option or propose your own business plan.",
                "Your choice is saved and can change before your Agent claims it.",
                "Ask your Agent to prepare the next event from the current state.",
                "Your Agent has claimed this choice and awaits a committed result.",
            ),
        };
        Ok(json!({
            "company_id":company_id,"language":locale.as_str(),"company":company,
            "data_revision":company["revision"],"phase":phase,
            "state":{"as_of":metrics["as_of"],"currency":company["currency"],
                "cash_balance_minor":metrics["cash_balance_minor"],"mrr_minor":metrics["mrr_minor"],
                "arr_minor":metrics["arr_minor"],"runway_months":metrics["runway_months"],
                "runway_status":metrics["runway_status"],"goals":goals},
            "current_event":current_event,"latest_result":latest_result,
            "player_output_guide":format!("/api/v1/agent-guide?topic=player-output&lang={}",locale.as_str()),
            "player_output_instruction":company_core::locale::translate(company_core::PLAYER_OUTPUT_HINT,locale),
            "player_output_policy":company_core::player_output_policy_localized(locale),
            "game":{"state":format!("/api/v1/companies/{company_id}/game-state"),"guide":format!("/api/v1/agent-guide?topic=game&lang={}",locale.as_str())},
            "network":{"state":format!("/api/v1/companies/{company_id}/network"),"guide":format!("/api/v1/agent-guide?topic=network&lang={}",locale.as_str())},
            "next":{"action":match phase {"awaiting_choice"=>"select", "awaiting_agent"=>"claim_saved_selection","agent_processing"=>"await_result",_=>"prepare_event"},
                "message":match phase {"awaiting_choice"=>labels.0,"awaiting_agent"=>labels.1,"agent_processing"=>labels.3,_=>labels.2}},
            "links":company_view_links(company_id, locale)
        }))
    }

    /// Renderer credentials expire even if its process or the server is interrupted.
    /// They have no write, installation or deployment-management capability.
    pub async fn presentation_token(&self, company_id: &str) -> Result<Value> {
        let _guard = self.write_lock.lock().await;
        self.company(company_id).await?;
        let id = new_id();
        let expiry = Utc::now().timestamp() + 60;
        let token = format!("tpc_exp_{expiry}_{}{}", new_id(), new_id());
        query!(&sql(self.pg,
            "INSERT INTO credentials(id,token_hash,kind,subject,company_id,scopes,revoked,created_at) VALUES(?,?,'agent',?,?,?,0,?)"))
            .bind(&id).bind(hash(token.as_bytes())).bind(&id).bind(company_id)
            .bind("[\"company.read\"]").bind(now()).execute(&self.pool).await?;
        Ok(json!({"id":id,"token":token,"expires_at":expiry}))
    }
}
