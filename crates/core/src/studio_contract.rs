use serde_json::{Value, json};

pub fn operation_ids() -> Vec<&'static str> {
    vec![
        "studio.state",
        "studio.preview",
        "studio.start",
        "studio.choose",
    ]
}
pub fn operation_scope(op: &str) -> Option<&'static str> {
    match op {
        "studio.state" | "studio.preview" => Some("company.read"),
        "studio.start" | "studio.choose" => Some("company.write"),
        _ => None,
    }
}
pub fn operation_contract(op: &str) -> Option<Value> {
    let (description, method, path) = match op {
        "studio.state" => (
            "Read the four-week studio, fixed rules and current options",
            "GET",
            "",
        ),
        "studio.preview" => (
            "Preview a work allocation without changing the company",
            "POST",
            "/preview",
        ),
        "studio.start" => (
            "Confirm and initialize the four-week studio in an empty CNY simulation",
            "POST",
            "/start",
        ),
        "studio.choose" => (
            "Commit one studio week and prepare the next event atomically",
            "POST",
            "/choose",
        ),
        _ => return None,
    };
    let write = ["studio.start", "studio.choose"].contains(&op);
    let mut schema =
        json!({"type":"object","additionalProperties":false,"properties":{},"required":[]});
    if op != "studio.state" {
        schema["properties"] = json!({"run_id":{"type":"string","const":"studio-v1"},
            "expected_run_revision":{"type":"integer","minimum":1},
            "allocations":{"type":"array","maxItems":4,"items":{"type":"object","additionalProperties":false,"required":["target","hours"],
                "properties":{"target":{"enum":["product","atlas","birch","cedar"]},"hours":{"type":"integer","minimum":2,"maximum":20,"multipleOf":2}}}}});
        schema["required"] = json!(["run_id", "expected_run_revision", "allocations"]);
    }
    if op == "studio.start" {
        schema["properties"] = json!({"request_id":{"type":"string"},"expected_company_revision":{"type":"integer","minimum":1}});
        schema["required"] = json!(["request_id", "expected_company_revision"]);
    } else if op == "studio.choose" {
        schema["properties"]["request_id"] = json!({"type":"string"});
        schema["properties"]["option_id"] = json!({"enum":["deliver","product","balance"]});
        schema["properties"]["selection_text"] = json!({"type":"string","minLength":1,"maxLength":2000,"description":"With allocations, retain the exact saved natural-language free selection. Required to settle an existing free selection whose text differs from the generated allocation label."});
        schema["required"] = json!(["request_id", "run_id", "expected_run_revision"]);
        schema["oneOf"] = json!([{"required":["option_id"],"not":{"anyOf":[{"required":["allocations"]},{"required":["selection_text"]}]}},{"required":["allocations"],"not":{"required":["option_id"]}}]);
    }
    Some(
        json!({"id":op,"operation":op,"service":"studio","description":description,
        "kind":if write{"write"}else{"read"},"scope":operation_scope(op),
        "required_scopes":if write {vec!["company.write","finance.write","reports.write","goals.write"]}else{vec!["company.read"]},
        "http":{"method":method,"path":format!("/api/v1/companies/{{company_id}}/studio{path}")},"input_schema":schema,
        "cli":if write {json!({"command":"company operation","mapping":"Use the normal OperationRequest envelope. start: expected_revision is company revision, input is empty. choose: expected_revision is run revision, input contains run_id and exactly one option_id or allocations."})}else{Value::Null},
        "result":if op=="studio.preview" {json!({"run_id":"string","run_revision":"integer","preview":"deterministic WeekResult"})}else if write {json!({"status":"committed","replayed":"boolean","company_revision":"integer","record":"studio_run","studio":"studio.state","result":"WeekResult on choose"})}else {json!({"active":"boolean","can_start":"boolean","reason":"string or null","run":"id, revision, ruleset, next week 1..5, cash, product, orders and exact history","rules":"fixed weekly capacity, overhead, late-payment and goals","event":"current scene","choices":"allocations and deterministic previews"})},
        "rules":[
            "This optional chapter requires explicit confirmation in an empty CNY simulation; real company records cannot enter it.",
            "Each week provides 20 hours in two-hour blocks and costs CNY 800. Unused hours expire; excess work and unavailable orders are rejected.",
            "Starting work on an offer accepts its deadline. Completion earns the fixed fee; late completion earns 75%. Unstarted expired offers create no promise.",
            "The four-week goal is at least CNY 6,000, 24 product hours and no unfinished accepted commissions. Product work alone does not invent sales.",
            "Preview never changes records. Commit atomically selects, accounts for work and cash, settles the event and opens the next week.",
            "Repeat the exact request ID and input after a lost response. Stale run revisions conflict; altered input under the same ID is rejected.",
            "Agent advice and open-plan interpretation are optional. It must use these fixed rules rather than invent payments or revise outcomes."
            ,"For a previously saved free selection, preview the translated allocations, obtain confirmation and submit the exact selection_text. It must still match the saved intent; a processing claim must belong to the same actor."
        ],"errors":["unauthorized","forbidden","not_found","simulation_required","revision_conflict","request_id_conflict","invalid_input","invalid_transition"],
        "guide":"/api/v1/agent-guide?topic=studio"}),
    )
}
pub fn guide(topic: &str) -> Option<Value> {
    (topic=="studio").then(||json!({"topic":"studio","purpose":"Play a finite studio chapter under fixed rules","steps":[
        "Read studio.state. Start only after the player confirms the CNY 6,000 opening cash, twenty weekly hours and four-week goal.",
        "Read available order deadlines, fees and remaining hours. Ask for a choice or translate the player's plan into two-hour allocations.",
        "Use studio.preview to show the exact consequence of a free plan before confirmation; retain the run revision.",
        "Submit studio.choose once authorized. Use its saved WeekResult and new event; no separate Agent settlement is required.",
        "Explain the causal tradeoff using actual allocations, payment conditions and unfinished promises. After four weeks stop and let the player choose whether to start another company."
    ],"discovery":"/api/v1/discovery?view=operations&service=studio"}))
}
