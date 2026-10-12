use super::*;

pub(super) fn routes() -> Router<ServerState> {
    Router::new().route("/api/v1/companies/{company_id}/network", get(read))
}

#[derive(Default, Deserialize)]
struct NetworkQuery {
    period_start: Option<String>,
    period_end: Option<String>,
    basis: Option<String>,
}

async fn read(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Path(company): Path<String>,
    Query(query): Query<NetworkQuery>,
) -> ApiResult {
    authorize(&state, &headers, Some(&company), "company.read").await?;
    result(
        state
            .app
            .network(
                &company,
                query.period_start.as_deref(),
                query.period_end.as_deref(),
                query.basis.as_deref(),
            )
            .await,
    )
}
