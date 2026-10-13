use super::*;
use company_core::{
    Locale,
    studio::{self, Allocation, State},
};

const RUN: &str = "studio-v1";
const ACCOUNT: &str = "studio-cash";
const GOAL: &str = "studio-launch";

pub(super) fn validate_archive(
    company: &Company,
    versions: &[Record],
    company_versions: &[Company],
) -> Result<()> {
    let runs: Vec<&Record> = versions
        .iter()
        .filter(|r| r.company_id == company.id && r.kind == "studio_run")
        .collect();
    let mut started = None;
    for run in runs {
        let state: State = serde_json::from_value(run.data["state"].clone())?;
        studio::validate_state(&state)?;
        let start = text(&run.data, "started_on")?;
        if run.id != RUN
            || run.revision != i64::from(state.week)
            || started.is_some_and(|old| old != start)
        {
            return Err(err(
                "integrity_error",
                "Studio history or start date changed.",
            ));
        }
        started = Some(start);
        let date_at = date(start)?
            .checked_add_signed(chrono::Duration::days(7 * i64::from(state.week - 1)))
            .ok_or_else(|| err("integrity_error", "Studio date overflow."))?;
        let historical_company = company_versions
            .iter()
            .find(|c| c.id == company.id && c.revision == run.company_revision)
            .ok_or_else(|| err("integrity_error", "Studio company version is missing."))?;
        let mut latest: BTreeMap<(&str, &str), Record> = BTreeMap::new();
        for record in versions
            .iter()
            .filter(|r| r.company_id == company.id && r.company_revision <= run.company_revision)
        {
            let key = (record.kind.as_str(), record.id.as_str());
            if latest
                .get(&key)
                .is_none_or(|old| old.revision < record.revision)
            {
                latest.insert(key, record.clone());
            }
        }
        let metrics = company_core::compute_metrics(
            historical_company,
            &latest.into_values().collect::<Vec<_>>(),
            date_at,
        )?;
        if historical_company.simulation_date.as_deref() != Some(date_at.to_string().as_str())
            || metrics["cash_balance_minor"] != state.cash_minor
        {
            return Err(err(
                "integrity_error",
                "Studio cash or date differs from its saved rules.",
            ));
        }
        if let Some(result) = state.history.last() {
            let reference = &run.data["last_result_ref"];
            let turn = versions
                .iter()
                .find(|r| {
                    r.company_id == company.id
                        && r.kind == "simulation_turn"
                        && r.id == format!("studio-result-{}", result.week)
                        && r.revision == 1
                })
                .ok_or_else(|| err("integrity_error", "Studio result version is missing."))?;
            if reference["kind"] != "simulation_turn"
                || reference["id"] != turn.id
                || reference["revision"] != turn.revision
                || turn.company_revision >= run.company_revision
                || turn.data["studio_result"] != serde_json::to_value(result)?
            {
                return Err(err(
                    "integrity_error",
                    "Studio result differs from its fixed history.",
                ));
            }
        } else if !run.data["last_result_ref"].is_null() {
            return Err(err(
                "integrity_error",
                "Unplayed studio has a result reference.",
            ));
        }
    }
    Ok(())
}

fn scene(week: u8, locale: Locale) -> &'static str {
    match (week, locale) {
        (1, Locale::ZhCn) => "第一份委托摆在桌上。接单能补充现金，但你的产品也需要这二十个小时。",
        (2, Locale::ZhCn) => {
            "新的委托来了。上周答应的交付不会因为新机会而消失，先看看还有多少工时。"
        }
        (3, Locale::ZhCn) => "最后一份委托开放了。只剩两周，接下它就要在产品与交付之间重新排期。",
        (4, Locale::ZhCn) => "最后一周。兑现承诺、完成产品、守住现金：剩下的二十小时由你安排。",
        (_, Locale::ZhCn) => "四周结束。哪些取舍留下了现金，哪些承诺占住了下一步？",
        (1, _) => {
            "Your first commission is on the desk. Paid work can fund the studio, but your own product needs the same twenty hours."
        }
        (2, _) => {
            "A new commission arrives. Last week's promises still need delivering; check your remaining work before accepting it."
        }
        (3, _) => {
            "The final commission opens. Two weeks remain. Taking it means scheduling around your product and existing promises."
        }
        (4, _) => {
            "The final week: deliver your promises, finish your product and preserve cash. You have twenty hours to allocate."
        }
        (_, _) => {
            "Four weeks are over. Which tradeoffs funded the studio, and which commitments remain?"
        }
    }
}

fn rules() -> Value {
    json!({"weekly_capacity_hours":20,"weekly_overhead_minor":80000,"product_target_hours":24,
        "cash_target_minor":600000,"total_weeks":4,"late_payment_percent":75})
}

fn projected(company: &Company, records: &[Record], locale: Locale) -> Result<Value> {
    let run = records.iter().find(|r| r.kind == "studio_run");
    let can_start = run.is_none()
        && company.mode == "simulation"
        && company.currency == "CNY"
        && records
            .iter()
            .all(|r| r.kind == "file" && r.id == "company-index");
    let reason = if run.is_none() && !can_start {
        Some(match locale {
            Locale::ZhCn => "新建一个以人民币记账的空白模拟公司，即可进入四周工作室。",
            Locale::En => "Create an empty CNY simulation company to start the four-week studio.",
        })
    } else {
        None
    };
    let Some(run) = run else {
        return Ok(
            json!({"company_id":company.id,"active":false,"can_start":can_start,"reason":reason,
            "run":null,"rules":rules(),"event":null,"choices":[]}),
        );
    };
    let state: State = serde_json::from_value(run.data["state"].clone())?;
    studio::validate_state(&state)?;
    let mut run_view = serde_json::to_value(&state)?;
    run_view["id"] = json!(run.id);
    run_view["revision"] = json!(run.revision);
    run_view["started_on"] = run.data["started_on"].clone();
    let mut choices = serde_json::to_value(studio::choices(&state))?;
    if locale == Locale::ZhCn {
        for choice in choices.as_array_mut().into_iter().flatten() {
            let (title, benefit, tradeoff) = match choice["id"].as_str() {
                Some("deliver") => (
                    "先兑现交付",
                    "把工时优先留给委托，完成后收款",
                    "自己的产品可能需要等到下周",
                ),
                Some("product") => (
                    "先做自己的产品",
                    "优先积累产品进度",
                    "现金要靠现有储备，新委托暂不接",
                ),
                _ => (
                    "两边都推进",
                    "产品先安排最多八小时，再处理委托",
                    "交付被拆到多周，期限可能更紧",
                ),
            };
            choice["title"] = json!(title);
            choice["benefit"] = json!(benefit);
            choice["tradeoff"] = json!(tradeoff);
        }
    }
    Ok(
        json!({"company_id":company.id,"active":true,"can_start":false,"reason":null,
        "run":run_view,"rules":rules(),"event":{"scene":scene(state.week, locale)},"choices":choices}),
    )
}

fn allocations(input: &Value, state: &State) -> Result<Vec<Allocation>> {
    match (input.get("option_id"), input.get("allocations")) {
        (Some(id), None) => studio::choices(state)
            .into_iter()
            .find(|choice| Some(choice.id.as_str()) == id.as_str())
            .map(|choice| choice.allocations)
            .ok_or_else(|| err("invalid_input", "Unknown studio choice.")),
        (None, Some(values)) => serde_json::from_value(values.clone())
            .map_err(|_| err("invalid_input", "Invalid work allocation.")),
        _ => Err(err(
            "invalid_input",
            "Provide exactly one option_id or allocations.",
        )),
    }
}

impl App {
    pub fn localize_studio_receipt(mut receipt: Value, locale: Locale) -> Value {
        if locale == Locale::ZhCn && receipt["studio"]["run"].is_object() {
            let view = &mut receipt["studio"];
            view["event"]["scene"] = json!(scene(
                view["run"]["week"].as_u64().unwrap_or(5) as u8,
                locale
            ));
            for choice in view["choices"].as_array_mut().into_iter().flatten() {
                let (title, benefit, tradeoff) = match choice["id"].as_str() {
                    Some("deliver") => (
                        "先兑现交付",
                        "把工时优先留给委托，完成后收款",
                        "自己的产品可能需要等到下周",
                    ),
                    Some("product") => (
                        "先做自己的产品",
                        "优先积累产品进度",
                        "现金要靠现有储备，新委托暂不接",
                    ),
                    _ => (
                        "两边都推进",
                        "产品先安排最多八小时，再处理委托",
                        "交付被拆到多周，期限可能更紧",
                    ),
                };
                choice["title"] = json!(title);
                choice["benefit"] = json!(benefit);
                choice["tradeoff"] = json!(tradeoff);
            }
        }
        receipt
    }

    pub async fn studio_state(&self, company_id: &str, locale: Locale) -> Result<Value> {
        let mut tx = self.pool.begin().await?;
        if self.pg {
            query!("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
                .execute(&mut *tx)
                .await?;
        }
        let company = self.company_tx(&mut tx, company_id).await?;
        let records = self.noncash_tx(&mut tx, company_id).await?;
        let result = projected(&company, &records, locale)?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn studio_preview(
        &self,
        company_id: &str,
        input: &Value,
        _locale: Locale,
    ) -> Result<Value> {
        let view = self.studio_state(company_id, Locale::En).await?;
        if !view["run"].is_object()
            || view["run"]["id"] != input["run_id"]
            || view["run"]["revision"] != input["expected_run_revision"]
        {
            return Err(err(
                "revision_conflict",
                "The studio week has changed. Read its current state.",
            ));
        }
        let mut state_value = view["run"].clone();
        for key in ["id", "revision", "started_on"] {
            state_value.as_object_mut().unwrap().remove(key);
        }
        let state: State = serde_json::from_value(state_value)?;
        let plan = allocations(input, &state)?;
        Ok(
            json!({"run_id":RUN,"run_revision":view["run"]["revision"],"preview":studio::preview(&state, &plan)?}),
        )
    }

    // Keep the parent receipt, transaction and ordinary operation envelope
    // explicit at each internal step of the atomic weekly commit.
    #[allow(clippy::too_many_arguments)]
    async fn studio_step(
        &self,
        tx: &mut Transaction<'_, Any>,
        company: &str,
        parent: &str,
        suffix: &str,
        operation: &str,
        expected_revision: Option<i64>,
        input: Value,
        actor: &str,
    ) -> Result<Value> {
        let request_id = format!("studio-{}", hash(format!("{parent}:{suffix}").as_bytes()));
        Box::pin(self.execute_tx(
            tx,
            Some(company),
            operation,
            OperationRequest {
                request_id,
                expected_revision,
                input,
            },
            actor,
        ))
        .await
    }

    async fn studio_event(
        &self,
        tx: &mut Transaction<'_, Any>,
        company: &str,
        parent: &str,
        state: &State,
        actor: &str,
    ) -> Result<()> {
        if state.week > 4 {
            return Ok(());
        }
        let options: Vec<Value> = studio::choices(state).into_iter().map(|choice| json!({
            "id":choice.id,"title":choice.title,"benefit":choice.benefit,"tradeoff":choice.tradeoff,
            "studio_allocations":choice.allocations
        })).collect();
        let callbacks = if state.week > 1 {
            json!([{"decision_id":format!("studio-week-{}",state.week-1),
            "summary":"Last week's allocation determines the work and promises still on the desk."}])
        } else {
            json!([])
        };
        self.studio_step(tx,company,parent,"event","decision.put",None,json!({
            "id":format!("studio-week-{}",state.week),"title":format!("Studio · week {}",state.week),
            "situation":scene(state.week,Locale::En),"goal_ids":[GOAL],"options":options,"allow_freeform":true,
            "studio_run_id":RUN,"execution":"studio-v1",
            "experience":{"scene":scene(state.week,Locale::En),"why_now":"Twenty hours are available this week. The delivery deadlines remain fixed.",
                "stakes":"Cash, your own product and accepted work compete for the same hours.",
                "cast":[],"callbacks":callbacks,"chapter":{"id":RUN,"title":"Four weeks to launch","turn":state.week,"planned_turns":4}}
        }),actor).await?;
        Ok(())
    }

    async fn studio_save_run(
        &self,
        tx: &mut Transaction<'_, Any>,
        company: &str,
        state: &State,
        started_on: &str,
        expected: Option<i64>,
        last_result_ref: Value,
    ) -> Result<Record> {
        let mut c = self.company_tx(tx, company).await?;
        let previous = c.revision;
        c.revision += 1;
        c.updated_at = now();
        let data = json!({"id":RUN,"ruleset":studio::RULESET,"source":"builtin:studio-v1","state":state,
            "started_on":started_on,"last_result_ref":last_result_ref});
        company_core::validate_record("studio_run", &data, &c, &[])?;
        let record = self
            .save_record(tx, company, "studio_run", RUN, data, expected, c.revision)
            .await?;
        self.save_company(tx, &c, Some(previous)).await?;
        Ok(record)
    }

    pub(super) async fn studio_operation_tx(
        &self,
        tx: &mut Transaction<'_, Any>,
        c: &Company,
        operation: &str,
        request: &OperationRequest,
        actor: &str,
    ) -> Result<Value> {
        if c.mode != "simulation" || c.currency != "CNY" {
            return Err(err(
                "simulation_required",
                "The studio chapter requires a CNY simulation company.",
            ));
        }
        let records = self.noncash_tx(tx, &c.id).await?;
        let run = records.iter().find(|r| r.kind == "studio_run");
        if operation == "studio.start" {
            if request.expected_revision != Some(c.revision) {
                return Err(err(
                    "revision_conflict",
                    "Start requires the current company revision.",
                ));
            }
            if run.is_some()
                || records
                    .iter()
                    .any(|r| r.kind != "file" || r.id != "company-index")
            {
                return Err(err(
                    "invalid_transition",
                    "Start the studio in an empty simulation company.",
                ));
            }
            if !request.input.as_object().is_some_and(|o| o.is_empty()) {
                return Err(err(
                    "invalid_input",
                    "Studio start does not accept extra fields.",
                ));
            }
            let started = c.simulation_date.as_deref().unwrap_or(&c.opening_date);
            self.studio_step(tx,&c.id,&request.request_id,"account","account.put",None,
                json!({"id":ACCOUNT,"name":"Studio cash","opening_balance_minor":600000,"opening_on":started}),actor).await?;
            self.studio_step(tx,&c.id,&request.request_id,"goal","goal.put",None,
                json!({"id":GOAL,"title":"Launch without leaving promises behind","outcome":"Finish 24 product hours, preserve CNY 6,000 and leave no accepted commission unfinished after four weeks.","status":"active","metric":"manual"}),actor).await?;
            let state = studio::initial();
            let record = self
                .studio_save_run(tx, &c.id, &state, started, None, Value::Null)
                .await?;
            self.studio_event(tx, &c.id, &request.request_id, &state, actor)
                .await?;
            let company = self.company_tx(tx, &c.id).await?;
            let records = self.noncash_tx(tx, &c.id).await?;
            return Ok(json!({"record":record,"studio":projected(&company,&records,Locale::En)?}));
        }
        let run = run.ok_or_else(|| err("not_found", "Start a studio chapter first."))?;
        if request.expected_revision != Some(run.revision) || request.input["run_id"] != run.id {
            return Err(err(
                "revision_conflict",
                "The studio week has changed. Read its current state.",
            ));
        }
        if request.input.as_object().is_none_or(|o| {
            o.keys().any(|k| {
                !["run_id", "option_id", "allocations", "selection_text"].contains(&k.as_str())
            })
        }) {
            return Err(err("invalid_input", "Unknown studio choice field."));
        }
        let state: State = serde_json::from_value(run.data["state"].clone())?;
        studio::validate_state(&state)?;
        if request.input.get("selection_text").is_some() && request.input.get("option_id").is_some()
        {
            return Err(err(
                "invalid_input",
                "selection_text is only used with allocations.",
            ));
        }
        let selection_text = request
            .input
            .get("selection_text")
            .map(|_| text(&request.input, "selection_text"))
            .transpose()?;
        if selection_text.is_some_and(|value| value.chars().count() > 2000) {
            return Err(err(
                "invalid_input",
                "selection_text exceeds 2000 characters.",
            ));
        }
        let plan = allocations(&request.input, &state)?;
        let next = studio::advance(&state, &plan)?;
        let result = next.history.last().unwrap();
        let expected_date = date(text(&run.data, "started_on")?)?
            .checked_add_signed(chrono::Duration::days(7 * i64::from(state.week - 1)))
            .ok_or_else(|| err("invalid_input", "Studio date overflow."))?;
        if c.simulation_date.as_deref() != Some(expected_date.to_string().as_str())
            || self.metrics_tx(tx, c, None).await?["cash_balance_minor"] != state.cash_minor
        {
            return Err(err(
                "revision_conflict",
                "The studio ledger or date changed outside its rules. Reconcile it before continuing.",
            ));
        }
        let next_date = expected_date
            .checked_add_signed(chrono::Duration::days(7))
            .ok_or_else(|| err("invalid_input", "Studio date overflow."))?
            .to_string();
        if self.metrics_tx(tx, c, Some(&next_date)).await?["cash_balance_minor"] != state.cash_minor
        {
            return Err(err(
                "revision_conflict",
                "Future cash records conflict with the studio week. Reconcile them before continuing.",
            ));
        }
        let event_id = format!("studio-week-{}", state.week);
        let event = records
            .iter()
            .find(|r| r.kind == "decision" && r.id == event_id)
            .ok_or_else(|| err("not_found", "The current studio event is missing."))?;
        if event.data["studio_run_id"] != RUN
            || !matches!(
                event.data["status"].as_str(),
                Some("open" | "selected" | "processing")
            )
        {
            return Err(err(
                "invalid_transition",
                "The current studio event is unavailable.",
            ));
        }
        let label = if let Some(id) = request.input["option_id"].as_str() {
            studio::choices(&state)
                .into_iter()
                .find(|choice| choice.id == id)
                .unwrap()
                .title
        } else {
            selection_text.map(str::to_owned).unwrap_or_else(|| {
                if plan.is_empty() {
                    return "Leave this week's hours unassigned".into();
                }
                plan.iter()
                    .map(|a| {
                        let target = state
                            .orders
                            .iter()
                            .find(|o| o.id == a.target)
                            .map(|o| o.title.as_str())
                            .unwrap_or("Product");
                        format!("{target}: {} hours", a.hours)
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            })
        };
        if event.data["status"] == "open" {
            let selection = if let Some(option) = request.input.get("option_id") {
                json!({"id":event_id,"option_id":option})
            } else {
                json!({"id":event_id,"text":label})
            };
            self.studio_step(
                tx,
                &c.id,
                &request.request_id,
                "select",
                "decision.select",
                Some(event.revision),
                selection,
                actor,
            )
            .await?;
        } else {
            let matches_selection = if let Some(option) = request.input.get("option_id") {
                event.data["selection"]["option_id"] == *option
            } else {
                event.data["selection"]["option_id"].is_null()
                    && event.data["selection"]["text"] == label
            };
            if !matches_selection
                || (event.data["status"] == "processing" && event.data["claim"]["actor"] != actor)
            {
                return Err(err(
                    "conflict",
                    "The saved selection is different or belongs to another processing Agent.",
                ));
            }
        }
        let current = self.company_tx(tx, &c.id).await?;
        self.studio_step(
            tx,
            &c.id,
            &request.request_id,
            "date",
            "simulation.advance",
            Some(current.revision),
            json!({"as_of":next_date}),
            actor,
        )
        .await?;
        self.studio_step(tx,&c.id,&request.request_id,"overhead","transaction.record",None,
            json!({"id":format!("studio-overhead-{}",state.week),"account_id":ACCOUNT,"amount_minor":80000,"direction":"outflow","category":"operating","occurred_on":next_date,"source":{"kind":"simulation_rule","reference":format!("builtin:studio-v1/week-{}",state.week)}}),actor).await?;
        for order in &result.order_outcomes {
            if order.payment_minor > 0 {
                self.studio_step(tx,&c.id,&request.request_id,&format!("pay-{}",order.id),"transaction.record",None,
                    json!({"id":format!("studio-payment-{}",order.id),"account_id":ACCOUNT,"amount_minor":order.payment_minor,"direction":"inflow","category":"operating","occurred_on":next_date,"source":{"kind":"simulation_rule","reference":format!("builtin:studio-v1/week-{}",state.week)}}),actor).await?;
            }
        }
        let cash_company = self.company_tx(tx, &c.id).await?;
        if self.metrics_tx(tx, &cash_company, Some(&next_date)).await?["cash_balance_minor"]
            != next.cash_minor
        {
            return Err(err(
                "integrity_error",
                "Studio settlement cash differs from its fixed rules.",
            ));
        }
        let end = next.week > 4;
        let narrative = result.explanation.join("\n");
        if end {
            let goal = self.record_tx(tx, &c.id, "goal", GOAL).await?.unwrap();
            let mut data = goal.data;
            data["status"] = json!(if result.goal.success {
                "achieved"
            } else {
                "blocked"
            });
            data["evidence"] = json!(narrative);
            self.studio_step(
                tx,
                &c.id,
                &request.request_id,
                "goal",
                "goal.put",
                Some(goal.revision),
                data,
                actor,
            )
            .await?;
        }
        let turn = self.studio_step(tx,&c.id,&request.request_id,"turn","simulation_turn.put",None,json!({
            "id":format!("studio-result-{}",state.week),"decision_id":event_id,"title":format!("Studio · week {} result",state.week),
            "action":label,"story":narrative,"occurred_on":next_date,"studio_ruleset":studio::RULESET,"studio_result":result,
            "outcomes":[{"goal_id":GOAL,"title":"Four-week launch","status":if end {if result.goal.success {"success"}else{"failure"}}else{"ongoing"},"evidence":narrative}],
            "aftermath":{"immediate":result.explanation.iter().take(8).collect::<Vec<_>>(),
                "chapter":{"id":RUN,"status":if end {"closed"}else{"ongoing"},"summary":if end {"Four weeks of work are complete."}else{"The next week's capacity is available."},"takeaway":"Accepted work and your own product use the same finite hours."}}
        }),actor).await?;
        let selected = self
            .record_tx(tx, &c.id, "decision", &event_id)
            .await?
            .unwrap();
        let reference = json!({"kind":"simulation_turn","id":turn["record"]["id"],"revision":turn["record"]["revision"]});
        self.studio_step(
            tx,
            &c.id,
            &request.request_id,
            "resolve",
            "decision.resolve",
            Some(selected.revision),
            json!({
                "id":event_id,"summary":narrative,"result_refs":[reference.clone()]
            }),
            actor,
        )
        .await?;
        let record = self
            .studio_save_run(
                tx,
                &c.id,
                &next,
                text(&run.data, "started_on")?,
                Some(run.revision),
                reference,
            )
            .await?;
        self.studio_event(tx, &c.id, &request.request_id, &next, actor)
            .await?;
        let company = self.company_tx(tx, &c.id).await?;
        let records = self.noncash_tx(tx, &c.id).await?;
        Ok(
            json!({"record":record,"studio":projected(&company,&records,Locale::En)?,"result":result}),
        )
    }
}
