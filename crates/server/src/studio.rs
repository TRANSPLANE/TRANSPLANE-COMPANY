use super::*;

pub(super) fn routes() -> Router<ServerState> {
    Router::new()
        .route("/api/v1/companies/{company_id}/studio", get(state))
        .route("/api/v1/companies/{company_id}/studio/start", post(start))
        .route("/api/v1/companies/{company_id}/studio/choose", post(choose))
        .route(
            "/api/v1/companies/{company_id}/studio/preview",
            post(preview),
        )
}

pub(super) fn authorize_write(actor: &Principal, company: &str) -> Result<(), ApiError> {
    for scope in [
        "company.write",
        "finance.write",
        "reports.write",
        "goals.write",
    ] {
        actor
            .authorize(Some(company), scope)
            .map_err(ApiError::from_error)?;
    }
    Ok(())
}

async fn state(
    State(server): State<ServerState>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult {
    authorize(&server, &headers, Some(&id), "company.read").await?;
    result(server.app.studio_state(&id, locale).await)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StartInput {
    request_id: String,
    expected_company_revision: i64,
}

async fn start(
    State(server): State<ServerState>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    Path(id): Path<String>,
    payload: Result<Json<StartInput>, JsonRejection>,
) -> ApiResult {
    let actor = principal(&server, &headers).await?;
    authorize_write(&actor, &id)?;
    let input = body(payload)?;
    result(
        server
            .app
            .execute(
                Some(&id),
                "studio.start",
                OperationRequest {
                    request_id: input.request_id,
                    expected_revision: Some(input.expected_company_revision),
                    input: json!({}),
                },
                &actor.subject,
            )
            .await
            .map(|value| App::localize_studio_receipt(value, locale)),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChooseInput {
    request_id: String,
    run_id: String,
    expected_run_revision: i64,
    option_id: Option<String>,
    allocations: Option<Value>,
    selection_text: Option<String>,
}

async fn choose(
    State(server): State<ServerState>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    Path(id): Path<String>,
    payload: Result<Json<ChooseInput>, JsonRejection>,
) -> ApiResult {
    let actor = principal(&server, &headers).await?;
    authorize_write(&actor, &id)?;
    let input = body(payload)?;
    if input.option_id.is_some() == input.allocations.is_some()
        || (input.option_id.is_some() && input.selection_text.is_some())
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_input",
            "请选择一个方案或提交时间分配。",
        ));
    }
    let mut choice = json!({"run_id":input.run_id});
    if let Some(option) = input.option_id {
        choice["option_id"] = json!(option);
    }
    if let Some(allocations) = input.allocations {
        choice["allocations"] = allocations;
    }
    if let Some(selection_text) = input.selection_text {
        choice["selection_text"] = json!(selection_text);
    }
    result(
        server
            .app
            .execute(
                Some(&id),
                "studio.choose",
                OperationRequest {
                    request_id: input.request_id,
                    expected_revision: Some(input.expected_run_revision),
                    input: choice,
                },
                &actor.subject,
            )
            .await
            .map(|value| App::localize_studio_receipt(value, locale)),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreviewInput {
    run_id: String,
    expected_run_revision: i64,
    allocations: Value,
}

async fn preview(
    State(server): State<ServerState>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    Path(id): Path<String>,
    payload: Result<Json<PreviewInput>, JsonRejection>,
) -> ApiResult {
    authorize(&server, &headers, Some(&id), "company.read").await?;
    let input = body(payload)?;
    result(server.app.studio_preview(&id,&json!({"run_id":input.run_id,"expected_run_revision":input.expected_run_revision,"allocations":input.allocations}),locale).await)
}
