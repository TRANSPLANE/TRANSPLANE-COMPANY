use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use std::{process::Stdio, sync::LazyLock, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
    sync::Semaphore,
};

const VIEWS: [&str; 8] = [
    "overview",
    "operations",
    "growth",
    "simulation",
    "reports",
    "files",
    "chronicle",
    "plugins",
];
const MAX_OUTPUT: u64 = 12 * 1024 * 1024;
static CAPTURES: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(2));

pub(super) fn routes() -> Router<ServerState> {
    Router::new()
        .route("/api/v1/companies/{company_id}/play", get(play))
        .route(
            "/api/v1/companies/{company_id}/presentation",
            get(presentation),
        )
        .route(
            "/api/v1/companies/{company_id}/presentation/capture",
            post(capture),
        )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RendererConfig {
    command: Vec<String>,
    base_url: String,
    #[serde(default = "default_timeout")]
    timeout_ms: u64,
}
fn default_timeout() -> u64 {
    40_000
}
impl RendererConfig {
    fn validate(&self) -> anyhow::Result<()> {
        let uri: axum::http::Uri = self.base_url.parse()?;
        let scheme = uri.scheme_str().unwrap_or("");
        let authority = uri.authority().map(|value| value.as_str()).unwrap_or("");
        if !["http", "https"].contains(&scheme)
            || authority.is_empty()
            || authority.contains('@')
            || self.base_url.contains('#')
            || uri.path() != "/"
            || uri.query().is_some()
            || self.command.is_empty()
            || self.command[0].is_empty()
            || !(1_000..=45_000).contains(&self.timeout_ms)
        {
            anyhow::bail!("invalid presentation configuration");
        }
        Ok(())
    }
}
async fn renderer_config(app: &App) -> Result<Option<RendererConfig>, ()> {
    let path = app.data_dir().join("config/presentation.json");
    let bytes = match tokio::fs::read(path).await {
        Ok(bytes) if bytes.len() <= 16 * 1024 => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        _ => return Err(()),
    };
    let config: RendererConfig = serde_json::from_slice(&bytes).map_err(|_| ())?;
    config.validate().map_err(|_| ())?;
    Ok(Some(config))
}
#[derive(Deserialize)]
struct ViewQuery {
    view: Option<String>,
}
fn view(value: Option<&str>) -> Result<&str, ApiError> {
    let view = value.unwrap_or("overview");
    if !VIEWS.contains(&view) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_input",
            "view 必须是已开放的公司页面。",
        ));
    }
    Ok(view)
}
async fn play(
    State(state): State<ServerState>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult {
    authorize(&state, &headers, Some(&id), "company.read").await?;
    result(state.app.play_state_localized(&id, locale).await)
}
fn capture_status(config: &Result<Option<RendererConfig>, ()>) -> Value {
    match config {
        Ok(Some(_)) => {
            json!({"configured":true,"operation":"company.capture","method":"POST","views":VIEWS})
        }
        Ok(None) => json!({"configured":false,"reason":"not_configured","views":VIEWS}),
        Err(_) => json!({"configured":false,"reason":"invalid_configuration","views":VIEWS}),
    }
}
async fn presentation(
    State(state): State<ServerState>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<ViewQuery>,
) -> ApiResult {
    authorize(&state, &headers, Some(&id), "company.read").await?;
    let selected_view = view(query.view.as_deref())?;
    let play = state
        .app
        .play_state_localized(&id, locale)
        .await
        .map_err(ApiError::from_error)?;
    let config = renderer_config(&state.app).await;
    Ok(Json(
        json!({"view":selected_view,"href":format!("/?company={id}#{selected_view}"),"capture":capture_status(&config),"play":play}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CaptureInput {
    view: Option<String>,
}
fn unavailable(
    status: StatusCode,
    code: &str,
    locale: Locale,
    play: Value,
    selected_view: &str,
) -> Response {
    let message = match (code, locale) {
        ("screenshot_busy", Locale::ZhCn) => "截图正在处理，请稍后重试。",
        ("screenshot_busy", Locale::En) => "Captures are busy. Try again shortly.",
        ("screenshot_failed", Locale::ZhCn) => "截图未完成，可查看当前状态或打开公司页面。",
        ("screenshot_failed", Locale::En) => "Capture failed. Read the current state or open the company page.",
        (_, Locale::ZhCn) => "此部署尚未配置可用的截图服务，可查看当前状态或打开公司页面。",
        (_, Locale::En) => "This deployment has no available screenshot renderer. Read the current state or open the company page.",
    };
    (
        status,
        Json(json!({"error":{"code":code,"message":message},"view":selected_view,"play":play})),
    )
        .into_response()
}
async fn capture(
    State(state): State<ServerState>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    Path(id): Path<String>,
    payload: Result<Json<CaptureInput>, JsonRejection>,
) -> Result<Response, ApiError> {
    authorize(&state, &headers, Some(&id), "company.read").await?;
    let input = body(payload)?;
    let selected_view = view(input.view.as_deref())?;
    let play = state
        .app
        .play_state_localized(&id, locale)
        .await
        .map_err(ApiError::from_error)?;
    let config = match renderer_config(&state.app).await {
        Ok(Some(config)) => config,
        _ => {
            return Ok(unavailable(
                StatusCode::SERVICE_UNAVAILABLE,
                "screenshot_unavailable",
                locale,
                play,
                selected_view,
            ))
        }
    };
    let _permit = match CAPTURES.try_acquire() {
        Ok(permit) => permit,
        Err(_) => {
            return Ok(unavailable(
                StatusCode::SERVICE_UNAVAILABLE,
                "screenshot_busy",
                locale,
                play,
                selected_view,
            ))
        }
    };
    let grant = state
        .app
        .presentation_token(&id)
        .await
        .map_err(ApiError::from_error)?;
    let request = json!({"base_url":config.base_url,"company_id":id,"view":selected_view,"language":locale_code(locale),"token":grant["token"]});
    let rendered = render(&config, request).await;
    // Revocation does not change a company data revision. Expiry is the fallback
    // if this request is cancelled or the process stops before cleanup.
    state
        .app
        .revoke_token(grant["id"].as_str().unwrap_or(""))
        .await
        .map_err(ApiError::from_error)?;
    authorize(&state, &headers, Some(&id), "company.read").await?;
    let rendered = match rendered.and_then(|value| verify_image(value, &id, selected_view)) {
        Ok(value) => value,
        Err(_) => {
            return Ok(unavailable(
                StatusCode::BAD_GATEWAY,
                "screenshot_failed",
                locale,
                play,
                selected_view,
            ))
        }
    };
    let revision = rendered["data_revision"].as_i64().unwrap_or(0);
    let captured = match state.app.snapshot_revision(&id, revision).await {
        Ok(captured) => captured,
        Err(_) => {
            return Ok(unavailable(
                StatusCode::BAD_GATEWAY,
                "screenshot_failed",
                locale,
                play,
                selected_view,
            ))
        }
    };
    let current = state
        .app
        .play_state_localized(&id, locale)
        .await
        .map_err(ApiError::from_error)?;
    Ok(Json(json!({"view":selected_view,"href":format!("/?company={id}#{selected_view}"),
        "image":rendered["image"],"data_revision":revision,"captured_state":captured["metrics"],
        "current_data_revision":current["data_revision"],"changed_since_capture":current["data_revision"]!=rendered["data_revision"],"play":current})).into_response())
}
async fn render(config: &RendererConfig, request: Value) -> anyhow::Result<Value> {
    let mut child = Command::new(&config.command[0])
        .args(&config.command[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("stdin unavailable"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("stdout unavailable"))?;
    let execute = async {
        let write_input = async {
            stdin.write_all(&serde_json::to_vec(&request)?).await?;
            stdin.shutdown().await?;
            drop(stdin);
            Ok::<(), anyhow::Error>(())
        };
        let read_output = async {
            let mut bytes = Vec::new();
            stdout.take(MAX_OUTPUT + 1).read_to_end(&mut bytes).await?;
            if bytes.len() as u64 > MAX_OUTPUT {
                anyhow::bail!("renderer output too large");
            }
            Ok::<_, anyhow::Error>(bytes)
        };
        let (_, bytes) = tokio::try_join!(write_input, read_output)?;
        if !child.wait().await?.success() {
            anyhow::bail!("renderer failed");
        }
        Ok::<Value, anyhow::Error>(serde_json::from_slice(&bytes)?)
    };
    tokio::time::timeout(Duration::from_millis(config.timeout_ms), execute).await?
}
fn verify_image(value: Value, company_id: &str, selected_view: &str) -> anyhow::Result<Value> {
    if value["company_id"] != company_id
        || value["view"] != selected_view
        || value["data_revision"].as_i64().is_none_or(|n| n < 1)
        || value["image"]["mime_type"] != "image/png"
    {
        anyhow::bail!("renderer metadata mismatch");
    }
    let bytes = STANDARD.decode(value["image"]["base64"].as_str().unwrap_or(""))?;
    if bytes.len() < 33
        || bytes.len() > 8 * 1024 * 1024
        || &bytes[..8] != b"\x89PNG\r\n\x1a\n"
        || &bytes[12..16] != b"IHDR"
    {
        anyhow::bail!("invalid PNG");
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into()?);
    if width == 0 || width > 2048 || height == 0 || height > 8192 {
        anyhow::bail!("invalid PNG dimensions");
    }
    Ok(
        json!({"company_id":company_id,"view":selected_view,"data_revision":value["data_revision"],
        "image":{"mime_type":"image/png","base64":value["image"]["base64"],"width":width,"height":height}}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unsafe_configuration_and_capture_metadata() {
        for base_url in [
            "file:///tmp/",
            "http://user:pass@host/",
            "http://host/else",
            "http://host/?token=secret",
            "http://host/#overview",
        ] {
            assert!(RendererConfig {
                command: vec!["node".into()],
                base_url: base_url.into(),
                timeout_ms: 1000
            }
            .validate()
            .is_err());
        }
        assert!(view(Some("admin")).is_err());
        assert!(verify_image(
            json!({"company_id":"other","view":"overview","data_revision":1}),
            "play",
            "overview"
        )
        .is_err());
    }
}
