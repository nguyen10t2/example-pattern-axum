use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use uuid::Uuid;

use crate::{
    domain::{
        settlements::{request::CreateSettlementRequest, response::SettlementResponse},
        shared::{PaginatedResponse, PaginationQuery},
    },
    errors::AppError,
    middleware::{AuthUser, ValidatedJson, ValidatedPath, ValidatedQuery},
    responses::SuccessResponse,
    state::AppState,
};

pub fn settlement_router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(handle_create_settlement))
        .route("/{id}", get(handle_get_settlement_by_id).delete(handle_cancel_settlement))
        .route("/group/{id}", get(handle_get_settlements_by_group))
        .route_layer(axum::middleware::from_fn_with_state(state, crate::middleware::require_auth))
}

pub async fn handle_create_settlement(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    ValidatedJson(body): ValidatedJson<CreateSettlementRequest>,
) -> Result<(StatusCode, SuccessResponse<SettlementResponse>), AppError> {
    let settlement = state.settlement_service.create(body, user_id).await?;
    Ok(SuccessResponse::created(settlement, "Settlement recorded successfully"))
}

pub async fn handle_get_settlement_by_id(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    ValidatedPath(id): ValidatedPath<Uuid>,
) -> Result<SuccessResponse<SettlementResponse>, AppError> {
    let settlement = state.settlement_service.find_by_id(id, user_id).await?;
    Ok(SuccessResponse::with_message(settlement, "Settlement found successfully"))
}

pub async fn handle_get_settlements_by_group(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    ValidatedPath(group_id): ValidatedPath<Uuid>,
    ValidatedQuery(query): ValidatedQuery<PaginationQuery>,
) -> Result<SuccessResponse<PaginatedResponse<SettlementResponse>>, AppError> {
    let settlements = state.settlement_service.find_by_group(group_id, user_id, query).await?;
    Ok(SuccessResponse::with_message(settlements, "Settlements for group retrieved successfully"))
}

pub async fn handle_cancel_settlement(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    ValidatedPath(id): ValidatedPath<Uuid>,
) -> Result<SuccessResponse<()>, AppError> {
    state.settlement_service.cancel_settlement(id, user_id).await?;
    Ok(SuccessResponse::message_only("Settlement cancelled successfully"))
}
