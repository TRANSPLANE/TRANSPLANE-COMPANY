use super::*;
use std::time::Duration;
use tokio::time::Instant;

pub(super) fn routes() -> Router<ServerState> {
    Router::new()
        .route("/api/v1/companies/{company_id}/agent-link", get(status))
        .route(
            "/api/v1/companies/{company_id}/agent-link/connect",
            post(connect),
        )
        .route(
            "/api/v1/companies/{company_id}/agent-link/pending-work",
            get(pending_work),
        )
        .route("/api/v1/companies/{company_id}/agent-link/ack", post(ack))
        .route(
            "/api/v1/companies/{company_id}/agent-link/disconnect",
            post(disconnect),
        )
}

fn token(headers: &HeaderMap) -> Result<&str, ApiError> {
    let value = headers
        .get(header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .ok_or_else(|| {
            ApiError::new(StatusCode::UNAUTHORIZED, "unauthorized", "请提供登录凭据。")
        })?;
    let (scheme, token) = value.split_once(' ').ok_or_else(|| {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "登录凭据格式错误。",
        )
    })?;
    if !scheme.eq_ignore_ascii_case("bearer")
        || token.is_empty()
        || token.contains(char::is_whitespace)
    {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "登录凭据格式错误。",
        ));
    }
    Ok(token)
}

async fn status(
    State(state): State<ServerState>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult {
    result(
        state
            .app
            .agent_link_status(&id, token(&headers)?, locale)
            .await,
    )
}

async fn connect(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    payload: Result<Json<Value>, JsonRejection>,
) -> ApiResult {
    let input = body(payload)?;
    result(
        state
            .app
            .agent_link_connect(&id, token(&headers)?, &input)
            .await,
    )
}

#[derive(Deserialize)]
struct PendingQuery {
    session_id: String,
    after_revision: Option<i64>,
    after_request_key: Option<String>,
    #[serde(default)]
    wait_ms: u64,
}

fn changed(response: &Value, query: &PendingQuery) -> bool {
    query
        .after_revision
        .is_some_and(|revision| response["data_revision"] != revision)
        || query
            .after_request_key
            .as_ref()
            .is_some_and(|key| response["link"]["request_key"] != *key)
        || (query.after_revision.is_none() && query.after_request_key.is_none())
}

async fn pending_work(
    State(state): State<ServerState>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<PendingQuery>,
) -> ApiResult {
    if query.after_revision.is_some_and(|revision| revision < 0)
        || query.wait_ms > 25_000
        || query
            .after_request_key
            .as_ref()
            .is_some_and(|key| key.len() > 512)
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_input",
            "after_revision 必须非负，wait_ms 必须在 0 至 25000 之间。",
        ));
    }
    let token = token(&headers)?;
    let mut response = state
        .app
        .agent_link_pending_work(&id, token, &query.session_id, locale)
        .await
        .map_err(ApiError::from_error)?;
    let work_is_new = response["work"].is_object()
        && query
            .after_request_key
            .as_ref()
            .is_none_or(|key| response["work"]["request_key"] != *key);
    if !changed(&response, &query) && !work_is_new && query.wait_ms > 0 {
        let revision = response["data_revision"].as_i64().unwrap_or(0);
        let deadline = Instant::now() + Duration::from_millis(query.wait_ms);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            tokio::time::sleep(remaining.min(Duration::from_secs(1))).await;
            // The lease and credential are checked again even when no ledger
            // change occurs. Polling cannot retain a revoked authorization.
            let current = state
                .app
                .agent_link_poll_revision(&id, token, &query.session_id)
                .await
                .map_err(ApiError::from_error)?;
            if current != revision {
                break;
            }
        }
        response = state
            .app
            .agent_link_pending_work(&id, token, &query.session_id, locale)
            .await
            .map_err(ApiError::from_error)?;
    }
    response["changed"] = json!(changed(&response, &query));
    Ok(Json(response))
}

async fn ack(
    State(state): State<ServerState>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    Path(id): Path<String>,
    payload: Result<Json<Value>, JsonRejection>,
) -> ApiResult {
    let input = body(payload)?;
    result(
        state
            .app
            .agent_link_ack(&id, token(&headers)?, &input, locale)
            .await,
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DisconnectInput {
    session_id: String,
}

async fn disconnect(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    payload: Result<Json<DisconnectInput>, JsonRejection>,
) -> ApiResult {
    let input = body(payload)?;
    result(
        state
            .app
            .agent_link_disconnect(&id, token(&headers)?, &input.session_id)
            .await,
    )
}
