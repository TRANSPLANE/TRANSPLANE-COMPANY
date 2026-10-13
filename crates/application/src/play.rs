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

fn story_reference(record: &Value) -> Value {
    json!({"kind":record["kind"],"id":record["id"],"revision":record["revision"],
        "company_revision":record["company_revision"]})
}

fn story_cast(event: &Value, cast: &mut BTreeMap<String, Value>) {
    for person in event["data"]["experience"]["cast"]
        .as_array()
        .into_iter()
        .flatten()
    {
        if let Some(id) = person["id"].as_str() {
            let mut saved = person.clone();
            saved["source_ref"] = story_reference(event);
            cast.insert(id.into(), saved);
        }
    }
}

fn story_chapter(event: &Value) -> Option<Value> {
    let mut chapter = event["data"]["experience"].get("chapter")?.clone();
    chapter["source_ref"] = story_reference(event);
    Some(chapter)
}

impl App {
    async fn play_result_records(
        &self,
        company_id: &str,
        event: &Value,
        current_records: &BTreeMap<(String, String, i64), &Value>,
        include_all: bool,
    ) -> Result<Vec<Value>> {
        let mut records = Vec::new();
        for reference in event["data"]["resolution"]["result_refs"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let kind = text(reference, "kind")?;
            let id = text(reference, "id")?;
            let revision = integer(reference, "revision")?;
            if !include_all && !matches!(kind, "simulation_turn" | "report") {
                continue;
            }
            if let Some(record) = current_records.get(&(kind.to_owned(), id.to_owned(), revision)) {
                records.push((**record).clone());
                continue;
            }
            let row = query!(&sql(self.pg,
                "SELECT data FROM record_versions WHERE company_id=? AND kind=? AND id=? AND revision=?"))
                .bind(company_id).bind(kind).bind(id).bind(revision)
                .fetch_optional(&self.pool).await?
                .ok_or_else(|| err("invalid_result_reference", "结算引用的历史记录不存在"))?;
            records.push(serde_json::from_str::<Value>(
                &row.try_get::<String, _>("data")?,
            )?);
        }
        Ok(records)
    }

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
        let mut current_records = BTreeMap::new();
        for records in snapshot["records"]
            .as_object()
            .into_iter()
            .flat_map(|kinds| kinds.values())
        {
            for record in records.as_array().into_iter().flatten() {
                if record["company_id"] == company_id {
                    current_records.insert(
                        (
                            text(record, "kind")?.to_owned(),
                            text(record, "id")?.to_owned(),
                            integer(record, "revision")?,
                        ),
                        record,
                    );
                }
            }
        }
        let mut settled_records = BTreeMap::new();
        for event in decisions
            .iter()
            .copied()
            .filter(|event| event["data"]["status"] == "resolved")
        {
            settled_records.insert(
                text(event, "id")?.to_owned(),
                self.play_result_records(
                    company_id,
                    event,
                    &current_records,
                    resolved.is_some_and(|latest| latest["id"] == event["id"]),
                )
                .await?,
            );
        }
        if let Some(event) = resolved {
            let references = &settled_records[text(event, "id")?];
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
            .filter(|record| record["data"].get("decision_id").is_none())
            .max_by_key(|record| record["company_revision"].as_i64().unwrap_or(0))
        {
            let revision = integer(&turn["data"], "after_revision")?;
            let result_snapshot = self.snapshot_revision(company_id, revision).await?;
            latest_result = json!({"legacy":true,"records":[turn],"data_revision":revision,"state":result_snapshot["metrics"]});
        }
        let mut cast = BTreeMap::new();
        let mut relationships = BTreeMap::new();
        let mut commitments = BTreeMap::new();
        let mut recent_choices = Vec::new();
        let mut chapter = Value::Null;
        for event in decisions.iter().rev().copied() {
            if event["data"]["status"] != "resolved" {
                continue;
            }
            story_cast(event, &mut cast);
            if let Some(saved) = story_chapter(event) {
                chapter = saved;
            }
            recent_choices.push(json!({"decision_id":event["id"],"title":event["data"]["title"],
                "choice":event["data"]["selection"]["text"],"summary":event["data"]["resolution"]["summary"],
                "source_ref":story_reference(event)}));
            let mut result_records = settled_records[text(event, "id")?]
                .iter()
                .collect::<Vec<_>>();
            result_records.sort_by(|left, right| {
                (
                    left["company_revision"].as_i64().unwrap_or(0),
                    left["kind"].as_str().unwrap_or(""),
                    left["id"].as_str().unwrap_or(""),
                    left["revision"].as_i64().unwrap_or(0),
                )
                    .cmp(&(
                        right["company_revision"].as_i64().unwrap_or(0),
                        right["kind"].as_str().unwrap_or(""),
                        right["id"].as_str().unwrap_or(""),
                        right["revision"].as_i64().unwrap_or(0),
                    ))
            });
            for record in result_records {
                if !matches!(record["kind"].as_str(), Some("simulation_turn" | "report")) {
                    continue;
                }
                let aftermath = &record["data"]["aftermath"];
                for relationship in aftermath["relationships"].as_array().into_iter().flatten() {
                    if let Some(id) = relationship["cast_id"].as_str() {
                        let mut saved = relationship.clone();
                        saved["source_ref"] = story_reference(record);
                        relationships.insert(id.to_owned(), saved);
                    }
                }
                for commitment in aftermath["commitments"].as_array().into_iter().flatten() {
                    if let Some(id) = commitment["id"].as_str() {
                        let mut saved = commitment.clone();
                        saved["source_ref"] = story_reference(record);
                        commitments.insert(id.to_owned(), saved);
                    }
                }
                if let Some(saved) = aftermath.get("chapter")
                    && chapter["id"] == saved["id"]
                {
                    for field in ["status", "summary", "takeaway"] {
                        if let Some(value) = saved.get(field) {
                            chapter[field] = value.clone();
                        }
                    }
                    chapter["result_source_ref"] = story_reference(record);
                }
            }
        }
        if let Some(event) = active {
            story_cast(event, &mut cast);
            if let Some(saved) = story_chapter(event) {
                chapter = saved;
            }
        }
        recent_choices.reverse();
        recent_choices.truncate(5);
        let current_cast = current_event["data"]["experience"]["cast"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|person| person["id"].as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let mut cast = cast.into_values().collect::<Vec<_>>();
        cast.sort_by_key(|person| {
            !person["id"]
                .as_str()
                .is_some_and(|id| current_cast.contains(id))
        });
        let story_context = json!({"recent_choices":recent_choices,"cast":cast,
            "relationships":relationships.into_values().collect::<Vec<_>>(),
            "open_commitments":commitments.into_values().filter(|item|item["status"]=="pending").collect::<Vec<_>>(),
            "chapter":chapter});
        let presentation = if current_event["data"]["experience"].is_object() {
            let mut saved = current_event["data"]["experience"].clone();
            saved["options"] = current_event["data"]["options"].clone();
            saved["source_ref"] = story_reference(&current_event);
            saved
        } else {
            Value::Null
        };
        let cadence = json!({"chapter":chapter,"next_beat":match phase {
            "awaiting_choice"=>"choose", "awaiting_agent"=>"await_agent", "agent_processing"=>"await_result",
            _ if chapter["status"]=="closed"=>"chapter_review", _=>"prepare_event"
        }});
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
            "story_context":story_context,"presentation":presentation,"cadence":cadence,
            "player_output_guide":format!("/api/v1/agent-guide?topic=player-output&lang={}",locale.as_str()),
            "player_output_instruction":company_core::locale::translate(company_core::PLAYER_OUTPUT_HINT,locale),
            "player_output_policy":company_core::player_output_policy_localized(locale),
            "game":{"state":format!("/api/v1/companies/{company_id}/game-state"),"guide":format!("/api/v1/agent-guide?topic=game&lang={}",locale.as_str())},
            "network":{"state":format!("/api/v1/companies/{company_id}/network"),"guide":format!("/api/v1/agent-guide?topic=network&lang={}",locale.as_str())},
            "studio":{"state":format!("/api/v1/companies/{company_id}/studio"),"guide":format!("/api/v1/agent-guide?topic=studio&lang={}",locale.as_str()),"execution":current_event["data"]["execution"]},
            "prediction":{"guide":format!("/api/v1/agent-guide?topic=prediction&lang={}",locale.as_str()),"standalone":true,"world_signals":"optional"},
            "next":{"action":if current_event["data"]["execution"]=="studio-v1" {"preview_and_settle_studio_week"}else if cadence["next_beat"]=="chapter_review" {"review_chapter"}else {match phase {"awaiting_choice"=>"select", "awaiting_agent"=>"claim_saved_selection","agent_processing"=>"await_result",_=>"prepare_event"}},
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
