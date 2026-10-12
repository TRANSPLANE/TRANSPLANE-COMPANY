use super::*;

pub(super) fn routes() -> Router<ServerState> {
    Router::new().route("/api/v1/companies/{company_id}/game-state", get(game_state))
}
async fn game_state(
    State(state): State<ServerState>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    Path(company): Path<String>,
) -> ApiResult {
    authorize(&state, &headers, Some(&company), "company.read").await?;
    result(state.app.game_state_localized(&company, locale).await)
}
