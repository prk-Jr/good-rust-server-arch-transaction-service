//! HTTP request handlers.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use payments_types::{
    AccountId, AppError, CreateAccountRequest, DepositRequest, TransactionRepository,
    TransferRequest, WithdrawRequest,
};

use crate::PaymentService;

/// Application state shared across handlers.
pub struct AppState<R: TransactionRepository> {
    pub service: PaymentService<R>,
}

/// Wrapper to implement IntoResponse for AppError (orphan rule workaround).
pub struct ApiError(pub AppError);

impl From<AppError> for ApiError {
    fn from(err: AppError) -> Self {
        ApiError(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self.0 {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::InsufficientFunds {
                available,
                requested,
            } => (
                StatusCode::BAD_REQUEST,
                format!(
                    "Insufficient funds: available {}, requested {}",
                    available, requested
                ),
            ),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };

        let body = serde_json::json!({
            "error": message,
            "code": status.as_u16()
        });

        (status, Json(body)).into_response()
    }
}

/// Health check endpoint.
pub async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "healthy" }))
}

/// Create a new account.
pub async fn create_account<R: TransactionRepository>(
    State(state): State<Arc<AppState<R>>>,
    Json(req): Json<CreateAccountRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let account = state.service.create_account(req).await?;
    Ok((StatusCode::CREATED, Json(account)))
}

/// List all accounts.
pub async fn list_accounts<R: TransactionRepository>(
    State(state): State<Arc<AppState<R>>>,
) -> Result<impl IntoResponse, ApiError> {
    let accounts = state.service.list_accounts().await?;
    Ok(Json(accounts))
}

/// Get account by ID.
pub async fn get_account<R: TransactionRepository>(
    State(state): State<Arc<AppState<R>>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let account_id: AccountId = id
        .parse()
        .map_err(|_| AppError::BadRequest("Invalid account ID".into()))?;

    let account = state.service.get_account(account_id).await?;
    Ok(Json(account))
}

/// Deposit money into an account.
pub async fn deposit<R: TransactionRepository>(
    State(state): State<Arc<AppState<R>>>,
    Json(req): Json<DepositRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let tx = state.service.deposit(req).await?;
    Ok(Json(tx))
}

/// Withdraw money from an account.
pub async fn withdraw<R: TransactionRepository>(
    State(state): State<Arc<AppState<R>>>,
    Json(req): Json<WithdrawRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let tx = state.service.withdraw(req).await?;
    Ok(Json(tx))
}

/// Transfer money between accounts.
pub async fn transfer<R: TransactionRepository>(
    State(state): State<Arc<AppState<R>>>,
    Json(req): Json<TransferRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let tx = state.service.transfer(req).await?;
    Ok(Json(tx))
}

/// List transactions for an account.
pub async fn list_transactions<R: TransactionRepository>(
    State(state): State<Arc<AppState<R>>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let account_id: AccountId = id
        .parse()
        .map_err(|_| AppError::BadRequest("Invalid account ID".into()))?;

    let transactions = state.service.list_transactions(account_id).await?;
    Ok(Json(transactions))
}
