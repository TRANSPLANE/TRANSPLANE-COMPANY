use axum::{
    extract::{rejection::JsonRejection, DefaultBodyLimit, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode, Uri},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Extension, Json, Router,
};
use company_application::{App, BatchOperation, OperationRequest, Principal};
use company_core::{DomainError, Locale};
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    net::SocketAddr,
    path::{Component, PathBuf},
};
mod agent_link;
mod game_systems;
mod network;
mod presentation;
mod skills;
mod studio;

#[derive(RustEmbed)]
#[folder = "../../web/dist/"]
struct WebAssets;

#[derive(Clone)]
struct ServerState {
    app: App,
    web_dir: Option<PathBuf>,
}

pub fn router(app: App, web_dir: Option<PathBuf>) -> Router {
    Router::new()
        .merge(agent_link::routes())
        .merge(presentation::routes())
        .merge(skills::routes())
        .merge(network::routes())
        .merge(game_systems::routes())
        .merge(studio::routes())
        .route("/api/v1/health", get(health))
        .route("/api/v1/agent-entry", get(agent_entry))
        .route("/api/v1/discovery", get(discovery))
        .route("/api/v1/agent-guide", get(guide))
        .route("/api/v1/session", get(session))
        .route("/api/v1/companies", get(companies).post(create_company))
        .route("/api/v1/companies/{company_id}", get(snapshot))
        .route("/api/v1/companies/{company_id}/snapshot", get(snapshot))
        .route("/api/v1/companies/{company_id}/changes", get(changes))
        .route(
            "/api/v1/companies/{company_id}/history/{revision}",
            get(snapshot_revision),
        )
        .route("/api/v1/companies/{company_id}/records", get(records))
        .route("/api/v1/companies/{company_id}/metrics", get(metrics))
        .route(
            "/api/v1/companies/{company_id}/plugins",
            get(company_plugins),
        )
        .route(
            "/api/v1/companies/{company_id}/operations/{operation}",
            post(execute),
        )
        .route(
            "/api/v1/companies/{company_id}/plugins/{id}/requests/{request_id}",
            get(company_plugin_request),
        )
        .route("/api/v1/operations/{operation}", post(execute_global))
        .route("/api/v1/companies/{company_id}/imports", post(import))
        .route(
            "/api/v1/companies/{company_id}/files/{file_id}",
            get(download),
        )
        .route("/api/v1/companies/{company_id}/files/sync", post(sync))
        .route("/api/v1/admin/tokens", get(tokens).post(grant_token))
        .route("/api/v1/admin/tokens/{id}", delete(revoke_token))
        .route("/api/v1/admin/plugins", get(plugins))
        .route("/api/v1/admin/plugins/catalog", get(plugin_catalog))
        .route("/api/v1/admin/plugins/install", post(install_plugin))
        .route("/api/v1/admin/plugins/{id}/enabled", post(enable_plugin))
        .route(
            "/api/v1/admin/plugins/{id}/requests/{request_id}",
            get(plugin_request),
        )
        .route(
            "/api/v1/admin/plugins/{id}/reconcile",
            post(reconcile_plugin),
        )
        .route(
            "/api/v1/admin/plugins/{id}/data-exit",
            post(plugin_data_exit),
        )
        .route("/api/v1/admin/plugins/{id}", delete(uninstall_plugin))
        .route("/api/v1/admin/backups", post(backup))
        .route("/api/v1/admin/backups/deliver", post(deliver_backup))
        .route("/api/v1/admin/restore", post(restore))
        .fallback(static_asset)
        .layer(DefaultBodyLimit::max(24 * 1024 * 1024))
        .layer(middleware::from_fn(response_headers))
        .with_state(ServerState { app, web_dir })
}
fn request_locale(request: &axum::extract::Request) -> Locale {
    let explicit = request.uri().query().and_then(|query| {
        form_urlencoded::parse(query.as_bytes())
            .find(|(key, _)| key == "lang")
            .map(|(_, value)| value.into_owned())
    });
    let language = explicit.or_else(|| {
        request
            .headers()
            .get(header::ACCEPT_LANGUAGE)
            .and_then(|h| h.to_str().ok())
            .map(|value| {
                value
                    .split(',')
                    .next()
                    .unwrap_or(value)
                    .split(';')
                    .next()
                    .unwrap_or(value)
                    .trim()
                    .to_owned()
            })
    });
    match language {
        None => Locale::ZhCn,
        Some(language) if language.to_ascii_lowercase().starts_with("zh") => Locale::ZhCn,
        Some(_) => Locale::En,
    }
}
fn locale_code(locale: Locale) -> &'static str {
    match locale {
        Locale::ZhCn => "zh-CN",
        Locale::En => "en",
    }
}
async fn response_headers(mut request: axum::extract::Request, next: Next) -> Response {
    let locale = request_locale(&request);
    request.extensions_mut().insert(locale);
    let private = request.uri().path().starts_with("/api/");
    let mut response = next.run(request).await;
    let structured = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|t| t.starts_with("application/json"));
    if private && response.status().is_client_error() && !structured {
        let (code, message) = match response.status() {
            StatusCode::METHOD_NOT_ALLOWED => {
                ("method_not_allowed", "请求方法不匹配，请读取操作合同。")
            }
            StatusCode::PAYLOAD_TOO_LARGE => ("payload_too_large", "请求内容超过接口限额。"),
            StatusCode::UNSUPPORTED_MEDIA_TYPE => (
                "unsupported_media_type",
                "请使用 application/json 提交请求。",
            ),
            _ => ("invalid_input", "请求参数格式不正确，请读取操作合同。"),
        };
        let allow = response.headers().get(header::ALLOW).cloned();
        response = ApiError::new(response.status(), code, message).into_response();
        if let Some(allow) = allow {
            response.headers_mut().insert(header::ALLOW, allow);
        }
    }
    if private && (response.status().is_client_error() || response.status().is_server_error()) {
        let (mut parts, body) = response.into_parts();
        response = match axum::body::to_bytes(body, 64 * 1024).await {
            Ok(bytes) => {
                if let Ok(mut value) = serde_json::from_slice::<Value>(&bytes) {
                    if let (Some(code), Some(message)) = (
                        value["error"]["code"].as_str(),
                        value["error"]["message"].as_str(),
                    ) {
                        let message =
                            company_core::localize_error(&DomainError::new(code, message), locale);
                        value["error"]["message"] = json!(message);
                    }
                    parts.headers.remove(header::CONTENT_LENGTH);
                    Response::from_parts(
                        parts,
                        axum::body::Body::from(
                            serde_json::to_vec(&value).unwrap_or_else(|_| bytes.to_vec()),
                        ),
                    )
                } else {
                    Response::from_parts(parts, axum::body::Body::from(bytes))
                }
            }
            Err(_) => ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                &company_core::localize_error(
                    &DomainError::new("internal_error", "操作失败，请检查服务器运行状态。"),
                    locale,
                ),
            )
            .into_response(),
        };
    }
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    if private {
        response.headers_mut().insert(
            header::CONTENT_LANGUAGE,
            HeaderValue::from_static(locale_code(locale)),
        );
        response
            .headers_mut()
            .insert(header::VARY, HeaderValue::from_static("Accept-Language"));
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response
}

pub async fn serve(app: App, bind: SocketAddr, web_dir: Option<PathBuf>) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!(address = %listener.local_addr()?, "Company console started");
    axum::serve(listener, router(app, web_dir))
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

async fn shutdown() {
    #[cfg(unix)]
    {
        if let Ok(mut terminate) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: String,
    message: String,
}
impl ApiError {
    fn new(status: StatusCode, code: &str, message: &str) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
        }
    }
    fn from_error(error: anyhow::Error) -> Self {
        if let Some(domain) = error.downcast_ref::<DomainError>() {
            let status = match domain.code.as_str() {
                "unauthorized" | "invalid_token" | "revoked_token" => StatusCode::UNAUTHORIZED,
                "forbidden" | "scope_denied" => StatusCode::FORBIDDEN,
                "not_found" | "company_not_found" | "file_not_found" => StatusCode::NOT_FOUND,
                "conflict"
                | "revision_conflict"
                | "active_decision_exists"
                | "idempotency_conflict"
                | "request_id_conflict"
                | "plugin_result_unknown"
                | "plugin_version_conflict"
                | "already_exists"
                | "target_not_empty" => StatusCode::CONFLICT,
                "busy" | "storage_busy" | "plugin_pending" => StatusCode::SERVICE_UNAVAILABLE,
                "payload_too_large" => StatusCode::PAYLOAD_TOO_LARGE,
                _ => StatusCode::BAD_REQUEST,
            };
            return Self {
                status,
                code: domain.code.clone(),
                message: domain.message.clone(),
            };
        }
        tracing::error!("请求未完成：内部存储或文件操作失败");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "操作失败，请检查服务器运行状态。",
        )
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({"error":{"code":self.code,"message":self.message}})),
        )
            .into_response()
    }
}
type ApiResult = Result<Json<Value>, ApiError>;
fn result(value: anyhow::Result<Value>) -> ApiResult {
    value.map(Json).map_err(ApiError::from_error)
}
fn body<T>(payload: Result<Json<T>, JsonRejection>) -> Result<T, ApiError> {
    payload.map(|Json(value)| value).map_err(|rejection| {
        if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
            return ApiError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload_too_large",
                "请求 JSON 超过 24 MiB。",
            );
        }
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_json",
            "请求必须包含符合接口约定的 JSON。",
        )
    })
}

fn operation_scope(operation: &str) -> Option<&'static str> {
    company_core::operation_scope(operation).or_else(|| {
        if operation.starts_with("plugin.") && operation.ends_with(".call") {
            Some("company.write")
        } else {
            None
        }
    })
}

async fn principal(state: &ServerState, headers: &HeaderMap) -> Result<Principal, ApiError> {
    let value = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
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
    state.app.authenticate(token).await.map_err(|_| {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "登录凭据无效或已撤销。",
        )
    })
}
async fn authorize(
    state: &ServerState,
    headers: &HeaderMap,
    company: Option<&str>,
    scope: &str,
) -> Result<Principal, ApiError> {
    let actor = principal(state, headers).await?;
    actor
        .authorize(company, scope)
        .map_err(ApiError::from_error)?;
    Ok(actor)
}
async fn owner(state: &ServerState, headers: &HeaderMap) -> Result<Principal, ApiError> {
    let actor = principal(state, headers).await?;
    if actor.kind != "owner" {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "此操作需要主人权限。",
        ));
    }
    Ok(actor)
}

async fn health() -> Json<Value> {
    Json(json!({"status":"ok","version":env!("CARGO_PKG_VERSION")}))
}
async fn agent_entry(Extension(locale): Extension<Locale>) -> Json<Value> {
    Json(json!({
        "product":"TransPlane Company","protocol_version":"1.0",
        "language":locale_code(locale),"languages":["en","zh-CN"],
        "discovery":format!("/api/v1/discovery?lang={}",locale_code(locale)),
        "guide":format!("/api/v1/agent-guide?topic=connect&lang={}",locale_code(locale)),
        "player_output":{
            "guide":format!("/api/v1/agent-guide?topic=player-output&lang={}",locale_code(locale)),
            "read_before_play":true,
            "instruction":company_core::locale::translate(company_core::PLAYER_OUTPUT_HINT, locale),
            "policy":company_core::player_output_policy_localized(locale),
            "applies_to":["opening","events","settlements","choices","status","continue","decision-handoff"]
        },
        "skills":{"catalog":"/api/v1/skills","company_catalog":"/api/v1/companies/{company_id}/skills","guide":"/api/v1/agent-guide?topic=skills","installation":"not_required","write_scope":"skills.write"},
        "game":{"state":"/api/v1/companies/{company_id}/game-state","guide":format!("/api/v1/agent-guide?topic=game&lang={}",locale_code(locale)),"write_scope":"company.write"},
        "network":{"state":"/api/v1/companies/{company_id}/network","guide":format!("/api/v1/agent-guide?topic=network&lang={}",locale_code(locale)),"write_scopes":["company.write","finance.write"]},
        "auth":{"scheme":"Bearer","management":"owner"},
        "agent_link":{"status":"/api/v1/companies/{company_id}/agent-link","discovery":"/api/v1/discovery?view=operations&service=agent-link","guide":format!("/api/v1/agent-guide?topic=agent-link&lang={}",locale_code(locale)),"receiver":"external_client","installed_helper":"scripts/agent-host.py"},
        "prediction":{"guide":format!("/api/v1/agent-guide?topic=prediction&lang={}",locale_code(locale)),"standalone":true,"world_signals":"optional","external_information":"provided_or_retrieved_by_owners_agent"},
        "play":{"state":"/api/v1/companies/{company_id}/play","guide":format!("/api/v1/agent-guide?topic=play&lang={}",locale_code(locale)),"presentation":"/api/v1/companies/{company_id}/presentation?view={view}","capture":"/api/v1/companies/{company_id}/presentation/capture"},
        "maintenance":{"cli":"company","writes":{"request_id":"required","expected_revision":"required when updating an existing record","current_company_snapshot":"defaulted atomically when omitted; explicit historical values are validated"},"changes":"/api/v1/companies/{company_id}/changes?after_revision={revision}&wait_ms=25000","scheduling":"external_agent"}
    }))
}
#[derive(Default, Deserialize)]
struct DiscoveryQuery {
    view: Option<String>,
    service: Option<String>,
    operation: Option<String>,
}
async fn discovery(
    State(state): State<ServerState>,
    Extension(locale): Extension<Locale>,
    Query(q): Query<DiscoveryQuery>,
) -> ApiResult {
    result(
        state
            .app
            .discovery_localized(
                q.view.as_deref(),
                q.service.as_deref(),
                q.operation.as_deref(),
                locale,
            )
            .await,
    )
}
#[derive(Deserialize)]
struct GuideQuery {
    #[serde(default = "default_topic")]
    topic: String,
}
fn default_topic() -> String {
    "connect".into()
}
async fn guide(
    State(state): State<ServerState>,
    Extension(locale): Extension<Locale>,
    Query(q): Query<GuideQuery>,
) -> ApiResult {
    result(state.app.agent_guide_localized(&q.topic, locale).await)
}
async fn session(State(state): State<ServerState>, headers: HeaderMap) -> ApiResult {
    let p = principal(&state, &headers).await?;
    Ok(Json(
        json!({"kind":p.kind,"subject":p.subject,"company_id":p.company_id,"scopes":p.scopes}),
    ))
}
async fn companies(State(state): State<ServerState>, headers: HeaderMap) -> ApiResult {
    let p = principal(&state, &headers).await?;
    p.authorize(p.company_id.as_deref(), "company.read")
        .map_err(ApiError::from_error)?;
    result(state.app.list_companies(&p).await)
}
async fn create_company(
    State(state): State<ServerState>,
    headers: HeaderMap,
    payload: Result<Json<OperationRequest>, JsonRejection>,
) -> ApiResult {
    let p = owner(&state, &headers).await?;
    result(
        state
            .app
            .execute(None, "company.create", body(payload)?, &p.subject)
            .await,
    )
}
async fn snapshot(
    State(state): State<ServerState>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult {
    authorize(&state, &headers, Some(&id), "company.read").await?;
    result(state.app.snapshot_localized(&id, locale).await)
}
#[derive(Deserialize)]
struct ChangesQuery {
    after_revision: i64,
    #[serde(default = "default_change_wait")]
    wait_ms: u64,
}
fn default_change_wait() -> u64 {
    25_000
}
async fn changes(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<ChangesQuery>,
) -> ApiResult {
    if query.after_revision < 0 || query.wait_ms > 25_000 {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_input",
            "after_revision 必须非负，wait_ms 必须在 0 至 25000 之间。",
        ));
    }
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(query.wait_ms);
    loop {
        // Recheck revocation while waiting; a read permission cannot survive a
        // revoked credential merely because this request began earlier.
        authorize(&state, &headers, Some(&id), "company.read").await?;
        let revision = state
            .app
            .company_revision(&id)
            .await
            .map_err(ApiError::from_error)?;
        let changed = revision != query.after_revision;
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if changed || remaining.is_zero() {
            return Ok(Json(
                json!({"company_id":id,"changed":changed,"revision":revision}),
            ));
        }
        tokio::time::sleep(remaining.min(std::time::Duration::from_secs(1))).await;
    }
}
async fn snapshot_revision(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Path((id, revision)): Path<(String, i64)>,
) -> ApiResult {
    authorize(&state, &headers, Some(&id), "company.read").await?;
    result(state.app.snapshot_revision(&id, revision).await)
}
#[derive(Default, Deserialize)]
struct RecordsQuery {
    kind: Option<String>,
    id: Option<String>,
    #[serde(default)]
    history: bool,
    #[serde(default)]
    offset: i64,
}
async fn records(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<RecordsQuery>,
) -> ApiResult {
    authorize(&state, &headers, Some(&id), "company.read").await?;
    result(
        state
            .app
            .records_filtered_page(&id, q.kind.as_deref(), q.id.as_deref(), q.history, q.offset)
            .await,
    )
}
#[derive(Default, Deserialize)]
struct MetricsQuery {
    as_of: Option<String>,
}
async fn metrics(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<MetricsQuery>,
) -> ApiResult {
    authorize(&state, &headers, Some(&id), "company.read").await?;
    result(state.app.metrics(&id, q.as_of.as_deref()).await)
}
async fn execute(
    State(state): State<ServerState>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    Path((id, op)): Path<(String, String)>,
    payload: Result<Json<OperationRequest>, JsonRejection>,
) -> ApiResult {
    let scope = operation_scope(&op)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_found", "操作不存在。"))?;
    let p = authorize(&state, &headers, Some(&id), scope).await?;
    if ["studio.start", "studio.choose"].contains(&op.as_str()) {
        studio::authorize_write(&p, &id)?;
    }
    if op.starts_with("plugin.") {
        for permission in state
            .app
            .plugin_permissions(&op)
            .await
            .map_err(ApiError::from_error)?
        {
            p.authorize(
                Some(&id),
                if permission == "backup.deliver" {
                    "admin"
                } else {
                    &permission
                },
            )
            .map_err(ApiError::from_error)?;
        }
    }
    let request = body(payload)?;
    if op == "skill.put" && request.input.get("copy_from").is_some() {
        p.authorize(Some(&id), "company.read")
            .map_err(ApiError::from_error)?;
    }
    result(
        state
            .app
            .execute(Some(&id), &op, request, &p.subject)
            .await
            .map(|value| {
                if ["studio.start", "studio.choose"].contains(&op.as_str()) {
                    App::localize_studio_receipt(value, locale)
                } else {
                    value
                }
            }),
    )
}
async fn execute_global(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Path(op): Path<String>,
    payload: Result<Json<OperationRequest>, JsonRejection>,
) -> ApiResult {
    let p = owner(&state, &headers).await?;
    result(
        state
            .app
            .execute(None, &op, body(payload)?, &p.subject)
            .await,
    )
}
#[derive(Deserialize)]
struct ImportBody {
    operations: Vec<BatchOperation>,
}
async fn import(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    payload: Result<Json<ImportBody>, JsonRejection>,
) -> ApiResult {
    let p = principal(&state, &headers).await?;
    let input = body(payload)?;
    if input.operations.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_input",
            "导入至少需要一个操作。",
        ));
    }
    if input.operations.len() > 1000 {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_input",
            "一次导入最多 1,000 个操作。",
        ));
    }
    for op in &input.operations {
        let scope = operation_scope(&op.operation)
            .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_found", "操作不存在。"))?;
        p.authorize(Some(&id), scope)
            .map_err(ApiError::from_error)?;
        if ["studio.start", "studio.choose"].contains(&op.operation.as_str()) {
            studio::authorize_write(&p, &id)?;
        }
        if op.operation == "skill.put" && op.request.input.get("copy_from").is_some() {
            p.authorize(Some(&id), "company.read")
                .map_err(ApiError::from_error)?;
        }
    }
    result(
        state
            .app
            .import_batch(&id, input.operations, &p.subject)
            .await,
    )
}
#[derive(Default, Deserialize)]
struct FileQuery {
    revision: Option<i64>,
}
async fn download(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Path((id, file)): Path<(String, String)>,
    Query(q): Query<FileQuery>,
) -> Result<Response, ApiError> {
    authorize(&state, &headers, Some(&id), "files.read").await?;
    let (bytes, name) = state
        .app
        .download_file(&id, &file, q.revision)
        .await
        .map_err(ApiError::from_error)?;
    let mime = mime_guess::from_path(&name).first_or_octet_stream();
    // Attachments cannot execute user-owned HTML in the application's origin.
    let filename = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("download")
        .chars()
        .filter(|c| !c.is_control())
        .take(160)
        .collect::<String>();
    let mut safe_name = filename
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        .take(160)
        .collect::<String>();
    if safe_name.is_empty() || safe_name.starts_with('.') {
        safe_name = format!("download{safe_name}");
    }
    let encoded = filename
        .as_bytes()
        .iter()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(*b, b'.' | b'_' | b'-' | b'~') {
                (*b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect::<String>();
    Ok((
        [
            (header::CONTENT_TYPE, mime.to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{safe_name}\"; filename*=UTF-8''{encoded}"),
            ),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
        ],
        bytes,
    )
        .into_response())
}
#[derive(Deserialize)]
struct SyncBody {
    relative_path: String,
    request_id: String,
}
async fn sync(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    payload: Result<Json<SyncBody>, JsonRejection>,
) -> ApiResult {
    let p = authorize(&state, &headers, Some(&id), "files.write").await?;
    let input = body(payload)?;
    result(
        state
            .app
            .sync_file_as(&id, &input.relative_path, &input.request_id, &p.subject)
            .await,
    )
}
async fn tokens(State(state): State<ServerState>, headers: HeaderMap) -> ApiResult {
    owner(&state, &headers).await?;
    result(state.app.list_tokens().await)
}
#[derive(Deserialize)]
struct TokenBody {
    company_id: Option<String>,
    scopes: Vec<String>,
    #[serde(default)]
    owner: bool,
}
async fn grant_token(
    State(state): State<ServerState>,
    headers: HeaderMap,
    payload: Result<Json<TokenBody>, JsonRejection>,
) -> ApiResult {
    owner(&state, &headers).await?;
    let input = body(payload)?;
    result(
        state
            .app
            .grant_token(input.company_id.as_deref(), input.scopes, input.owner)
            .await,
    )
}
async fn revoke_token(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult {
    owner(&state, &headers).await?;
    result(state.app.revoke_token(&id).await)
}
async fn plugins(
    State(state): State<ServerState>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
) -> ApiResult {
    owner(&state, &headers).await?;
    result(state.app.plugin_catalog_localized(locale).await)
}
async fn plugin_catalog(
    State(state): State<ServerState>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
) -> ApiResult {
    owner(&state, &headers).await?;
    result(state.app.plugin_catalog_localized(locale).await)
}
async fn company_plugins(
    State(state): State<ServerState>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult {
    authorize(&state, &headers, Some(&id), "company.read").await?;
    result(state.app.plugin_catalog_localized(locale).await)
}
#[derive(Deserialize)]
struct PluginRequestQuery {
    company_id: String,
}
async fn company_plugin_request(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Path((company, id, request_id)): Path<(String, String, String)>,
) -> ApiResult {
    authorize(&state, &headers, Some(&company), "company.read").await?;
    result(state.app.plugin_request(&company, &id, &request_id).await)
}
async fn plugin_request(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Path((id, request_id)): Path<(String, String)>,
    Query(q): Query<PluginRequestQuery>,
) -> ApiResult {
    owner(&state, &headers).await?;
    result(
        state
            .app
            .plugin_request(&q.company_id, &id, &request_id)
            .await,
    )
}
#[derive(Deserialize)]
struct ReconcileBody {
    company_id: String,
    request_id: String,
    result: Value,
}
async fn reconcile_plugin(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    payload: Result<Json<ReconcileBody>, JsonRejection>,
) -> ApiResult {
    owner(&state, &headers).await?;
    let input = body(payload)?;
    result(
        state
            .app
            .reconcile_plugin(&input.company_id, &id, &input.request_id, input.result)
            .await,
    )
}
#[derive(Deserialize)]
struct DataExitBody {
    company_id: String,
    snapshot: PathBuf,
    expected_sha256: String,
}
async fn plugin_data_exit(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    payload: Result<Json<DataExitBody>, JsonRejection>,
) -> ApiResult {
    owner(&state, &headers).await?;
    let input = body(payload)?;
    result(
        state
            .app
            .remove_plugin_data(
                &input.company_id,
                &id,
                &input.snapshot,
                &input.expected_sha256,
            )
            .await,
    )
}
#[derive(Deserialize)]
struct PluginBody {
    manifest: Value,
    #[serde(default = "default_plugin_config")]
    config: Value,
}
fn default_plugin_config() -> Value {
    json!({})
}
async fn install_plugin(
    State(state): State<ServerState>,
    headers: HeaderMap,
    payload: Result<Json<PluginBody>, JsonRejection>,
) -> ApiResult {
    owner(&state, &headers).await?;
    let input = body(payload)?;
    result(state.app.install_plugin(input.manifest, input.config).await)
}
#[derive(Deserialize)]
struct EnabledBody {
    enabled: bool,
}
async fn enable_plugin(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    payload: Result<Json<EnabledBody>, JsonRejection>,
) -> ApiResult {
    owner(&state, &headers).await?;
    let input = body(payload)?;
    result(state.app.set_plugin_enabled(&id, input.enabled).await)
}
#[derive(Default, Deserialize)]
struct UninstallQuery {
    #[serde(default)]
    purge: bool,
}
async fn uninstall_plugin(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<UninstallQuery>,
) -> ApiResult {
    owner(&state, &headers).await?;
    result(state.app.uninstall_plugin(&id, q.purge).await)
}
#[derive(Deserialize)]
struct BackupBody {
    company_id: Option<String>,
    destination: Option<PathBuf>,
}
async fn backup(
    State(state): State<ServerState>,
    headers: HeaderMap,
    payload: Result<Json<BackupBody>, JsonRejection>,
) -> ApiResult {
    owner(&state, &headers).await?;
    let input = body(payload)?;
    result(
        state
            .app
            .backup(input.company_id.as_deref(), input.destination.as_deref())
            .await,
    )
}
#[derive(Deserialize)]
struct DeliverBody {
    plugin_id: String,
    snapshot: PathBuf,
}
async fn deliver_backup(
    State(state): State<ServerState>,
    headers: HeaderMap,
    payload: Result<Json<DeliverBody>, JsonRejection>,
) -> ApiResult {
    owner(&state, &headers).await?;
    let input = body(payload)?;
    result(
        state
            .app
            .deliver_backup(&input.plugin_id, &input.snapshot)
            .await,
    )
}
#[derive(Deserialize)]
struct RestoreBody {
    snapshot: PathBuf,
}
async fn restore(
    State(state): State<ServerState>,
    headers: HeaderMap,
    payload: Result<Json<RestoreBody>, JsonRejection>,
) -> ApiResult {
    owner(&state, &headers).await?;
    let input = body(payload)?;
    result(state.app.restore(&input.snapshot).await)
}

async fn static_asset(State(state): State<ServerState>, uri: Uri) -> Response {
    let request_path = uri.path().trim_start_matches('/');
    if request_path.starts_with("api/") {
        return ApiError::new(StatusCode::NOT_FOUND, "not_found", "接口不存在。").into_response();
    }
    if request_path.contains('\\')
        || std::path::Path::new(request_path)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    let asset_path = if request_path.is_empty() {
        "index.html"
    } else {
        request_path
    };
    let bytes = if let Some(dir) = state.web_dir {
        let root = match tokio::fs::canonicalize(dir).await {
            Ok(p) => p,
            Err(_) => return StatusCode::NOT_FOUND.into_response(),
        };
        match tokio::fs::canonicalize(root.join(asset_path)).await {
            Ok(p) if p.starts_with(&root) && p.is_file() => tokio::fs::read(p)
                .await
                .ok()
                .map(|b| (b, asset_path.to_owned())),
            _ if !asset_path.contains('.') => tokio::fs::read(root.join("index.html"))
                .await
                .ok()
                .map(|b| (b, "index.html".into())),
            _ => None,
        }
    } else {
        WebAssets::get(asset_path)
            .map(|a| (a.data.into_owned(), asset_path.to_owned()))
            .or_else(|| {
                if !asset_path.contains('.') {
                    WebAssets::get("index.html").map(|a| (a.data.into_owned(), "index.html".into()))
                } else {
                    None
                }
            })
    };
    match bytes {
        Some((bytes, path)) => {
            let cache = if path == "index.html" {
                "no-store"
            } else {
                "public, max-age=3600"
            };
            (
                [
                    (
                        header::CONTENT_TYPE,
                        mime_guess::from_path(path)
                            .first_or_octet_stream()
                            .to_string(),
                    ),
                    (header::CACHE_CONTROL, cache.into()),
                    (header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
                ],
                bytes,
            )
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
