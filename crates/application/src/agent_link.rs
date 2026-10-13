use super::*;
use company_core::Locale;
use tokio::time::Instant;

#[derive(Clone)]
pub(super) struct AgentLinkSession {
    id: String,
    credential_hash: String,
    subject: String,
    label: String,
    deadline: Instant,
    expires_at: String,
    receipt: Option<WorkReceipt>,
    observed_revision: i64,
}

#[derive(Clone)]
struct WorkReceipt {
    request_key: String,
    id: String,
    acknowledged: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConnectInput {
    label: String,
    #[serde(default = "default_lease")]
    lease_seconds: u64,
    session_id: Option<String>,
}

fn default_lease() -> u64 {
    60
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AckInput {
    session_id: String,
    request_key: String,
    receipt_id: String,
}

fn input<T: serde::de::DeserializeOwned>(value: &Value) -> Result<T> {
    serde_json::from_value(value.clone())
        .map_err(|_| err("invalid_input", "Agent 连接请求结构无效"))
}

fn pending_work(play: &Value) -> Value {
    if play["cadence"]["next_beat"] == "chapter_review" {
        return Value::Null;
    }
    let company = play["company_id"].as_str().unwrap_or("");
    let event = &play["current_event"];
    match play["phase"].as_str() {
        Some("awaiting_agent") => json!({
            "kind":"process_selection",
            "request_key":format!("{company}:decision:{}:{}",event["id"].as_str().unwrap_or(""),event["revision"]),
            "decision_id":event["id"],"decision_revision":event["revision"],
            "selection":event["data"]["selection"],"claim_command":event["claim_command"]
        }),
        Some("awaiting_event") => json!({
            "kind":"prepare_event",
            "request_key":format!("{company}:prepare-event:{}",play["data_revision"]),
            "baseline_revision":play["data_revision"]
        }),
        _ => Value::Null,
    }
}

fn link_state(play: &Value, lease: Option<&AgentLinkSession>) -> Value {
    let work = pending_work(play);
    let request_key = work["request_key"].as_str();
    let receipt = lease
        .and_then(|lease| lease.receipt.as_ref())
        .filter(|receipt| {
            Some(receipt.request_key.as_str()) == request_key && receipt.acknowledged
        });
    let state = match play["phase"].as_str() {
        Some("agent_processing")
            if lease.is_some_and(|lease| {
                play["current_event"]["data"]["claim"]["actor"] == lease.subject
            }) =>
        {
            "processing"
        }
        Some("agent_processing") => "interrupted",
        Some("awaiting_choice") => "awaiting_choice",
        _ if request_key.is_some() && lease.is_none() => "handoff_required",
        _ if receipt.is_some() => "delivered",
        _ if request_key.is_some() => "queued",
        _ => "idle",
    };
    json!({"company_id":play["company_id"],"data_revision":play["data_revision"],"phase":play["phase"],"state":state,"connected":lease.is_some(),
        "label":lease.map(|lease|lease.label.as_str()),"session_id":lease.map(|lease|lease.id.as_str()),
        "lease_expires_at":lease.map(|lease|lease.expires_at.as_str()),
        "request_key":request_key,"pending_work":work,
        "current_claim":if play["phase"]=="agent_processing" {json!({"decision_id":play["current_event"]["id"],
            "decision_revision":play["current_event"]["revision"],"claim":play["current_event"]["data"]["claim"]})}else{Value::Null},
        "received_receipt_id":receipt.map(|receipt|receipt.id.as_str())})
}

fn issue_work(play: &Value, lease: &mut AgentLinkSession) -> Result<Value> {
    let revision = integer(play, "data_revision")?;
    if revision < lease.observed_revision {
        return Err(err("revision_conflict", "经营快照已更新，请重新读取待办"));
    }
    lease.observed_revision = revision;
    let mut work = pending_work(play);
    if let Some(request_key) = work["request_key"].as_str().map(str::to_owned) {
        if lease
            .receipt
            .as_ref()
            .is_none_or(|receipt| receipt.request_key != request_key)
        {
            lease.receipt = Some(WorkReceipt {
                request_key,
                id: new_id(),
                acknowledged: false,
            });
        }
        let receipt = lease.receipt.as_ref().unwrap();
        if receipt.acknowledged {
            work = Value::Null;
        } else {
            work["receipt_id"] = json!(receipt.id);
        }
    }
    Ok(work)
}

impl App {
    async fn agent_link_authorize(
        &self,
        company_id: &str,
        token: &str,
        write: bool,
    ) -> Result<Principal> {
        identifier(company_id)?;
        let principal = self.authenticate(token).await?;
        principal.authorize(Some(company_id), "company.read")?;
        if write {
            principal.authorize(Some(company_id), "company.write")?;
        }
        Ok(principal)
    }

    async fn agent_link_live(&self, company_id: &str) -> Result<Option<AgentLinkSession>> {
        let lease = self.agent_links.lock().await.get(company_id).cloned();
        let Some(lease) = lease.filter(|lease| lease.deadline > Instant::now()) else {
            return Ok(None);
        };
        let row = query!(&sql(self.pg,
            "SELECT kind,subject,company_id,scopes FROM credentials WHERE token_hash=? AND revoked=0"))
            .bind(&lease.credential_hash).fetch_optional(&self.pool).await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let scope: String = row.try_get("company_id")?;
        let principal = Principal {
            kind: row.try_get("kind")?,
            subject: row.try_get("subject")?,
            company_id: (!scope.is_empty()).then_some(scope),
            scopes: serde_json::from_str(&row.try_get::<String, _>("scopes")?)?,
        };
        if principal.subject != lease.subject
            || principal
                .authorize(Some(company_id), "company.read")
                .is_err()
            || principal
                .authorize(Some(company_id), "company.write")
                .is_err()
        {
            return Ok(None);
        }
        // A disconnect or renewal during credential lookup must not return an
        // old runtime connection or discard its newer lease and receipt.
        Ok(self
            .agent_links
            .lock()
            .await
            .get(company_id)
            .filter(|current| {
                current.id == lease.id
                    && current.credential_hash == lease.credential_hash
                    && current.deadline > Instant::now()
            })
            .cloned())
    }

    async fn agent_link_session(
        &self,
        company_id: &str,
        token: &str,
        session_id: &str,
    ) -> Result<AgentLinkSession> {
        self.agent_link_authorize(company_id, token, true).await?;
        identifier(session_id)?;
        let lease = self
            .agent_link_live(company_id)
            .await?
            .ok_or_else(|| err("conflict", "Agent 连接已失效，请重新连接"))?;
        if lease.id != session_id || lease.credential_hash != hash(token.as_bytes()) {
            return Err(err("forbidden", "Agent 连接未绑定此凭据或会话"));
        }
        Ok(lease)
    }

    pub async fn agent_link_connect(
        &self,
        company_id: &str,
        token: &str,
        value: &Value,
    ) -> Result<Value> {
        let principal = self.agent_link_authorize(company_id, token, true).await?;
        self.company_revision(company_id).await?;
        let input: ConnectInput = input(value)?;
        let label = input.label.trim();
        if label.is_empty()
            || label.chars().count() > 120
            || label.chars().any(char::is_control)
            || !(1..=120).contains(&input.lease_seconds)
        {
            return Err(err(
                "invalid_input",
                "Agent 名称须为 1 至 120 字，连接时限须为 1 至 120 秒",
            ));
        }
        if let Some(id) = &input.session_id {
            identifier(id)?;
        }
        let observed_session_id = self
            .agent_links
            .lock()
            .await
            .get(company_id)
            .map(|lease| lease.id.clone());
        let previous = self.agent_link_live(company_id).await?;
        let credential_hash = hash(token.as_bytes());
        if previous
            .as_ref()
            .is_some_and(|lease| lease.credential_hash != credential_hash)
        {
            return Err(err("conflict", "这家公司已有活跃的 Agent 连接"));
        }
        if let Some(id) = &input.session_id
            && previous
                .as_ref()
                .is_none_or(|lease| lease.id != *id || lease.credential_hash != credential_hash)
        {
            return Err(err("conflict", "Agent 连接已失效，请建立新连接"));
        }
        let expires_at =
            (Utc::now() + chrono::Duration::seconds(input.lease_seconds as i64)).to_rfc3339();
        let mut links = self.agent_links.lock().await;
        // The map may have changed while the credential was being rechecked.
        if links.get(company_id).is_some_and(|lease| {
            lease.deadline > Instant::now()
                && observed_session_id
                    .as_ref()
                    .is_none_or(|old| *old != lease.id)
        }) {
            return Err(err("conflict", "Agent 连接已变化，请重新读取"));
        }
        if input.session_id.is_some()
            && links
                .get(company_id)
                .is_none_or(|lease| lease.deadline <= Instant::now())
        {
            return Err(err("conflict", "Agent 连接已失效，请建立新连接"));
        }
        let receipt = links
            .get(company_id)
            .filter(|lease| previous.as_ref().is_some_and(|old| old.id == lease.id))
            .and_then(|lease| lease.receipt.clone());
        let observed_revision = links
            .get(company_id)
            .filter(|lease| previous.as_ref().is_some_and(|old| old.id == lease.id))
            .map_or(0, |lease| lease.observed_revision);
        let lease = AgentLinkSession {
            id: previous
                .as_ref()
                .map(|lease| lease.id.clone())
                .unwrap_or_else(new_id),
            credential_hash,
            subject: principal.subject,
            label: label.into(),
            deadline: Instant::now() + Duration::from_secs(input.lease_seconds),
            expires_at,
            receipt,
            observed_revision,
        };
        let result = json!({"company_id":company_id,"session_id":lease.id,"label":lease.label,
            "connected":true,"lease_seconds":input.lease_seconds,"lease_expires_at":lease.expires_at});
        links.insert(company_id.into(), lease);
        Ok(result)
    }

    pub async fn agent_link_status(
        &self,
        company_id: &str,
        token: &str,
        locale: Locale,
    ) -> Result<Value> {
        self.agent_link_authorize(company_id, token, false).await?;
        let play = self.play_state_localized(company_id, locale).await?;
        let lease = self.agent_link_live(company_id).await?;
        Ok(link_state(&play, lease.as_ref()))
    }

    pub async fn agent_link_poll_revision(
        &self,
        company_id: &str,
        token: &str,
        session_id: &str,
    ) -> Result<i64> {
        self.agent_link_session(company_id, token, session_id)
            .await?;
        self.company_revision(company_id).await
    }

    pub async fn agent_link_pending_work(
        &self,
        company_id: &str,
        token: &str,
        session_id: &str,
        locale: Locale,
    ) -> Result<Value> {
        // Serialize phase reads and receipt-cache changes with local choices,
        // claims and acknowledgements; the lease map never writes the ledger.
        let _guard = self.write_lock.lock().await;
        let lease = self
            .agent_link_session(company_id, token, session_id)
            .await?;
        let play = self.play_state_localized(company_id, locale).await?;
        let mut links = self.agent_links.lock().await;
        let current = links
            .get_mut(company_id)
            .filter(|current| {
                current.id == lease.id
                    && current.deadline > Instant::now()
                    && current.credential_hash == lease.credential_hash
            })
            .ok_or_else(|| err("conflict", "Agent 连接已失效，请重新连接"))?;
        let work = issue_work(&play, current)?;
        let link = link_state(&play, Some(current));
        Ok(
            json!({"company_id":company_id,"data_revision":play["data_revision"],"link":link,"work":work,"play":play}),
        )
    }

    pub async fn agent_link_ack(
        &self,
        company_id: &str,
        token: &str,
        value: &Value,
        locale: Locale,
    ) -> Result<Value> {
        let input: AckInput = input(value)?;
        identifier(&input.receipt_id)?;
        if input.request_key.is_empty() || input.request_key.len() > 512 {
            return Err(err("invalid_input", "待办标识无效"));
        }
        // A local reselection or credential revocation cannot race the current
        // selection check and its runtime receipt acknowledgement.
        let _guard = self.write_lock.lock().await;
        let lease = self
            .agent_link_session(company_id, token, &input.session_id)
            .await?;
        let play = self.play_state_localized(company_id, locale).await?;
        let work = pending_work(&play);
        if work["request_key"] != input.request_key {
            return Err(err("conflict", "玩家选择或经营状态已变化，请读取当前待办"));
        }
        let mut links = self.agent_links.lock().await;
        let current = links
            .get_mut(company_id)
            .filter(|current| {
                current.id == lease.id
                    && current.deadline > Instant::now()
                    && current.credential_hash == lease.credential_hash
            })
            .ok_or_else(|| err("conflict", "Agent 连接已失效，请重新连接"))?;
        let revision = integer(&play, "data_revision")?;
        if revision < current.observed_revision {
            return Err(err("revision_conflict", "经营快照已更新，请重新读取待办"));
        }
        let receipt = current
            .receipt
            .as_mut()
            .filter(|receipt| {
                receipt.request_key == input.request_key && receipt.id == input.receipt_id
            })
            .ok_or_else(|| err("conflict", "接收回执不匹配当前已发送待办"))?;
        let replayed = receipt.acknowledged;
        receipt.acknowledged = true;
        current.observed_revision = revision;
        Ok(
            json!({"acknowledged":true,"request_key":input.request_key,"receipt_id":input.receipt_id,
            "replayed":replayed,"link":link_state(&play,Some(current))}),
        )
    }

    pub async fn agent_link_disconnect(
        &self,
        company_id: &str,
        token: &str,
        session_id: &str,
    ) -> Result<Value> {
        let principal = self.agent_link_authorize(company_id, token, true).await?;
        identifier(session_id)?;
        let credential_hash = hash(token.as_bytes());
        let mut links = self.agent_links.lock().await;
        if let Some(lease) = links.get(company_id) {
            if lease.id != session_id {
                return Err(err("conflict", "Agent 连接已变化"));
            }
            if principal.kind != "owner" && lease.credential_hash != credential_hash {
                return Err(err("forbidden", "Agent 连接未绑定此凭据"));
            }
            links.remove(company_id);
        }
        Ok(json!({"company_id":company_id,"session_id":session_id,"connected":false}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_late_snapshot_cannot_replace_a_newer_received_selection() {
        let mut lease = AgentLinkSession {
            id: "session".into(),
            credential_hash: "digest".into(),
            subject: "agent".into(),
            label: "Host".into(),
            deadline: Instant::now() + Duration::from_secs(60),
            expires_at: now(),
            receipt: None,
            observed_revision: 0,
        };
        let current = json!({"company_id":"home","data_revision":12,"phase":"awaiting_agent",
            "current_event":{"id":"trial","revision":3,"data":{"selection":{"option_id":"expanded"}}}});
        let work = issue_work(&current, &mut lease).unwrap();
        lease.receipt.as_mut().unwrap().acknowledged = true;
        let receipt = work["receipt_id"].clone();
        let older = json!({"company_id":"home","data_revision":11,"phase":"awaiting_agent",
            "current_event":{"id":"trial","revision":2,"data":{"selection":{"option_id":"focused"}}}});
        let error = issue_work(&older, &mut lease).unwrap_err().to_string();
        assert!(error.contains("revision_conflict"), "{error}");
        assert_eq!(link_state(&current, Some(&lease))["state"], "delivered");
        assert_eq!(
            link_state(&current, Some(&lease))["received_receipt_id"],
            receipt
        );
        assert_eq!(issue_work(&current, &mut lease).unwrap(), Value::Null);
    }
}
