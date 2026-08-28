use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use uuid::Uuid;

use crate::{
    domain::{
        expenses::{request::CreateExpenseRequest, response::ExpenseResponse},
        shared::{PaginatedResponse, PaginationQuery},
    },
    errors::AppError,
    middleware::{AuthUser, ValidatedJson, ValidatedPath, ValidatedQuery},
    responses::SuccessResponse,
    state::AppState,
};

pub fn expense_router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(handle_create_expense))
        .route("/{id}", get(handle_get_expense_by_id).delete(handle_delete_expense))
        .route("/group/{id}", get(handle_get_expenses_by_group))
        .route_layer(axum::middleware::from_fn_with_state(state, crate::middleware::require_auth))
}

pub async fn handle_create_expense(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    ValidatedJson(body): ValidatedJson<CreateExpenseRequest>,
) -> Result<(StatusCode, SuccessResponse<ExpenseResponse>), AppError> {
    state.group_service.ensure_membership(body.group_id, user_id).await?;

    let expense = state.expense_service.create(body, user_id).await?;
    Ok(SuccessResponse::created(expense, "Expense created successfully"))
}

pub async fn handle_get_expense_by_id(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    ValidatedPath(id): ValidatedPath<Uuid>,
) -> Result<SuccessResponse<ExpenseResponse>, AppError> {
    let expense = state.expense_service.find_by_id(id, Some(user_id)).await?;
    Ok(SuccessResponse::with_message(expense, "Expense found successfully"))
}

pub async fn handle_get_expenses_by_group(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    ValidatedPath(group_id): ValidatedPath<Uuid>,
    ValidatedQuery(query): ValidatedQuery<PaginationQuery>,
) -> Result<SuccessResponse<PaginatedResponse<ExpenseResponse>>, AppError> {
    state.group_service.ensure_membership(group_id, user_id).await?;

    let expenses = state.expense_service.find_by_group(group_id, user_id, query).await?;
    Ok(SuccessResponse::with_message(expenses, "Expenses for group retrieved successfully"))
}

pub async fn handle_delete_expense(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    ValidatedPath(id): ValidatedPath<Uuid>,
) -> Result<SuccessResponse<()>, AppError> {
    state.expense_service.delete_expense(id, user_id).await?;
    Ok(SuccessResponse::message_only("Expense deleted successfully"))
}
