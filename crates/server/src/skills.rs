use super::*;

pub(super) fn routes() -> Router<ServerState> {
    Router::new()
        .route("/api/v1/skills", get(builtins))
        .route("/api/v1/skills/{skill_id}", get(builtin))
        .route("/api/v1/skills/{skill_id}/document", get(builtin_document))
        .route("/api/v1/companies/{company_id}/skills", get(catalog))
        .route(
            "/api/v1/companies/{company_id}/skills/{skill_id}",
            get(read),
        )
        .route(
            "/api/v1/companies/{company_id}/skills/{skill_id}/document",
            get(document),
        )
        .route(
            "/api/v1/companies/{company_id}/skills/{skill_id}/history",
            get(history),
        )
        .route(
            "/api/v1/companies/{company_id}/files/{file_id}/preview",
            get(preview),
        )
        .route(
            "/api/v1/companies/{company_id}/files/{file_id}/preview/content",
            get(preview_content),
        )
}

#[derive(Default, Deserialize)]
struct SkillQuery {
    #[serde(default)]
    include_deleted: bool,
    revision: Option<i64>,
}

async fn builtins(
    State(state): State<ServerState>,
    Extension(locale): Extension<Locale>,
) -> ApiResult {
    result(state.app.builtin_skills(locale).await)
}
async fn builtin(
    State(state): State<ServerState>,
    Extension(locale): Extension<Locale>,
    Path(id): Path<String>,
    Query(query): Query<SkillQuery>,
) -> ApiResult {
    if query.revision.is_some_and(|r| r != 1) {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "Skill 版本不存在",
        ));
    }
    result(state.app.builtin_skill(&id, locale).await)
}
async fn catalog(
    State(state): State<ServerState>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    Path(company): Path<String>,
    Query(query): Query<SkillQuery>,
) -> ApiResult {
    authorize(&state, &headers, Some(&company), "company.read").await?;
    result(
        state
            .app
            .company_skills(&company, query.include_deleted, locale)
            .await,
    )
}
async fn read(
    State(state): State<ServerState>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    Path((company, id)): Path<(String, String)>,
    Query(query): Query<SkillQuery>,
) -> ApiResult {
    authorize(&state, &headers, Some(&company), "company.read").await?;
    result(
        state
            .app
            .company_skill(&company, &id, query.revision, locale)
            .await,
    )
}
async fn history(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Path((company, id)): Path<(String, String)>,
) -> ApiResult {
    authorize(&state, &headers, Some(&company), "company.read").await?;
    result(state.app.skill_history(&company, &id).await)
}
fn markdown_response(document: &Value) -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/markdown; charset=utf-8"),
            (
                header::CONTENT_SECURITY_POLICY,
                "sandbox; default-src 'none'",
            ),
        ],
        document["document"].as_str().unwrap_or("").to_owned(),
    )
        .into_response()
}
async fn builtin_document(
    State(state): State<ServerState>,
    Extension(locale): Extension<Locale>,
    Path(id): Path<String>,
    Query(query): Query<SkillQuery>,
) -> Result<Response, ApiError> {
    if query.revision.is_some_and(|r| r != 1) {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "Skill 版本不存在",
        ));
    }
    let doc = state
        .app
        .builtin_skill(&id, locale)
        .await
        .map_err(ApiError::from_error)?;
    Ok(markdown_response(&doc))
}
async fn document(
    State(state): State<ServerState>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    Path((company, id)): Path<(String, String)>,
    Query(query): Query<SkillQuery>,
) -> Result<Response, ApiError> {
    authorize(&state, &headers, Some(&company), "company.read").await?;
    let doc = state
        .app
        .company_skill(&company, &id, query.revision, locale)
        .await
        .map_err(ApiError::from_error)?;
    Ok(markdown_response(&doc))
}
async fn preview(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Path((company, id)): Path<(String, String)>,
    Query(query): Query<FileQuery>,
) -> ApiResult {
    authorize(&state, &headers, Some(&company), "files.read").await?;
    result(state.app.file_preview(&company, &id, query.revision).await)
}
async fn preview_content(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Path((company, id)): Path<(String, String)>,
    Query(query): Query<FileQuery>,
) -> Result<Response, ApiError> {
    authorize(&state, &headers, Some(&company), "files.read").await?;
    let (bytes, mime) = state
        .app
        .file_preview_content(&company, &id, query.revision)
        .await
        .map_err(ApiError::from_error)?;
    let mut response = bytes.into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_str(&mime).unwrap());
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("inline"),
    );
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("sandbox; default-src 'none'"),
    );
    response.headers_mut().insert(
        "X-Preview-Format",
        HeaderValue::from_static(if mime == "application/pdf" {
            "pdf"
        } else {
            "image"
        }),
    );
    Ok(response)
}
