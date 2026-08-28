use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use std::time::Duration;
use uuid::Uuid;

use crate::{
    domain::groups::{
        request::{AddMemberRequest, CreateGroupRequest, JoinGroupRequest},
        response::{GroupMemberResponse, GroupResponse, GroupSummaryResponse},
    },
    errors::{AppError, BusinessError},
    middleware::{AuthUser, ValidatedJson, ValidatedPath, extract_client_ip},
    responses::SuccessResponse,
    state::AppState,
};

pub fn group_router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(handle_get_all_groups).post(handle_create_group))
        .route("/join", post(handle_join_group))
        .route("/{id}", get(handle_get_group_by_id).delete(handle_delete_group))
        .route("/{id}/summary", get(handle_get_group_summary))
        .route("/{id}/members", get(handle_get_group_members).post(handle_add_group_member))
        .route_layer(axum::middleware::from_fn_with_state(state, crate::middleware::require_auth))
}

pub async fn handle_get_all_groups(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> Result<SuccessResponse<Vec<GroupResponse>>, AppError> {
    let groups = state.group_service.find_all_by_user(user_id).await?;
    Ok(SuccessResponse::with_message(groups, "Groups retrieved successfully"))
}

pub async fn handle_create_group(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    headers: HeaderMap,
    ValidatedJson(body): ValidatedJson<CreateGroupRequest>,
) -> Result<(StatusCode, SuccessResponse<GroupResponse>), AppError> {
    let ip = extract_client_ip(&headers);
    if !state.rate_limiter.check_mixed_limit("create-group", user_id, &ip, 5, 20, Duration::from_secs(60)).await {
        return Err(AppError::Business(BusinessError::TooManyRequests));
    }

    let group = state.group_service.create(body, user_id).await?;
    Ok(SuccessResponse::created(group, "Group created successfully"))
}

pub async fn handle_join_group(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    headers: HeaderMap,
    ValidatedJson(body): ValidatedJson<JoinGroupRequest>,
) -> Result<SuccessResponse<GroupResponse>, AppError> {
    let ip = extract_client_ip(&headers);
    if !state.rate_limiter.check_mixed_limit("join-group", user_id, &ip, 10, 30, Duration::from_secs(60)).await {
        return Err(AppError::Business(BusinessError::TooManyRequests));
    }

    let group = state.group_service.join_by_invite_code(&body.code, user_id).await?;
    Ok(SuccessResponse::with_message(group, "Joined group successfully"))
}

pub async fn handle_get_group_by_id(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    ValidatedPath(id): ValidatedPath<Uuid>,
) -> Result<SuccessResponse<GroupResponse>, AppError> {
    let group = state.group_service.find_by_id(id, Some(user_id)).await?;
    Ok(SuccessResponse::with_message(group, "Group found successfully"))
}

pub async fn handle_get_group_summary(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    ValidatedPath(id): ValidatedPath<Uuid>,
) -> Result<SuccessResponse<GroupSummaryResponse>, AppError> {
    let summary = state.group_service.get_group_summary(id, user_id).await?;
    Ok(SuccessResponse::with_message(summary, "Group summary retrieved successfully"))
}

pub async fn handle_add_group_member(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    ValidatedPath(id): ValidatedPath<Uuid>,
    ValidatedJson(body): ValidatedJson<AddMemberRequest>,
) -> Result<(StatusCode, SuccessResponse<GroupMemberResponse>), AppError> {
    let member = state.group_service.add_member(id, body, user_id).await?;
    Ok(SuccessResponse::created(member, "Member added successfully"))
}

pub async fn handle_get_group_members(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    ValidatedPath(id): ValidatedPath<Uuid>,
) -> Result<SuccessResponse<Vec<GroupMemberResponse>>, AppError> {
    let members = state.group_service.get_members(id, user_id).await?;
    Ok(SuccessResponse::with_message(members, "Members retrieved successfully"))
}

pub async fn handle_delete_group(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    ValidatedPath(id): ValidatedPath<Uuid>,
) -> Result<SuccessResponse<()>, AppError> {
    state.group_service.delete_group(id, user_id).await?;
    Ok(SuccessResponse::message_only("Group deleted successfully"))
}
