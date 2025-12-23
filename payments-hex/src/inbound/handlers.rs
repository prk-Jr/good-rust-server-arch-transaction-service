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

// #[tracing::instrument(skip(state), fields(owner = %req.name))]
#[tracing::instrument(skip(state))]
pub async fn create_account<R: TransactionRepository>(
    State(state): State<Arc<AppState<R>>>,
    Json(req): Json<CreateAccountRequest>,
) -> Result<impl IntoResponse, ApiError> {
    tracing::info!("👉 ENTERING create_account handler for {}", req.name);
    let account = state.service.create_account(req).await?;
    Ok((StatusCode::CREATED, Json(account)))
}

/// List all accounts.
#[tracing::instrument(skip(state))]
pub async fn list_accounts<R: TransactionRepository>(
    State(state): State<Arc<AppState<R>>>,
) -> Result<impl IntoResponse, ApiError> {
    let accounts = state.service.list_accounts().await?;
    Ok(Json(accounts))
}

/// Get account by ID.
#[tracing::instrument(skip(state), fields(account_id = %id))]
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
#[tracing::instrument(skip(state), fields(account_id = %req.account_id, amount = req.amount))]
pub async fn deposit<R: TransactionRepository>(
    State(state): State<Arc<AppState<R>>>,
    Json(req): Json<DepositRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let tx = state.service.deposit(req).await?;
    Ok(Json(tx))
}

/// Withdraw money from an account.
#[tracing::instrument(skip(state), fields(account_id = %req.account_id, amount = req.amount))]
pub async fn withdraw<R: TransactionRepository>(
    State(state): State<Arc<AppState<R>>>,
    Json(req): Json<WithdrawRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let tx = state.service.withdraw(req).await?;
    Ok(Json(tx))
}

/// Transfer money between accounts.
#[tracing::instrument(skip(state), fields(from = %req.from_account_id, to = %req.to_account_id, amount = req.amount))]
pub async fn transfer<R: TransactionRepository>(
    State(state): State<Arc<AppState<R>>>,
    Json(req): Json<TransferRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let tx = state.service.transfer(req).await?;
    Ok(Json(tx))
}

/// List transactions for an account.
#[tracing::instrument(skip(state), fields(account_id = %id))]
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

/// Bootstrap endpoint - creates the first API key.
///
/// This endpoint only works when there are NO existing API keys in the system.
/// It returns the raw API key (only shown once) that should be saved securely.
#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct BootstrapRequest {
    /// Name for the API key
    #[schema(example = "my-api-key")]
    pub name: String,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct BootstrapResponse {
    /// The generated API key (shown only once)
    #[schema(example = "sk_abc123xyz...")]
    pub api_key: String,
    /// Informational message
    pub message: String,
}

#[tracing::instrument(skip(state), fields(key_name = %req.name))]
pub async fn bootstrap<R: TransactionRepository>(
    State(state): State<Arc<AppState<R>>>,
    Json(req): Json<BootstrapRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Check if there are any existing API keys
    let key_count = state
        .service
        .repo()
        .count_api_keys()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    if key_count > 0 {
        return Err(AppError::BadRequest(
            "Bootstrap not allowed: API keys already exist. Use an existing key to create new ones.".into()
        ).into());
    }

    // Create the first API key
    let (_api_key, raw_key) = state
        .service
        .repo()
        .create_api_key(&req.name)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(BootstrapResponse {
            api_key: raw_key,
            message: "First API key created. Save this key securely - it won't be shown again!"
                .into(),
        }),
    ))
}

// ─────────────────────────────────────────────────────────────────────────────
// API Key Management
// ─────────────────────────────────────────────────────────────────────────────

/// Request to create a new API key.
#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct CreateApiKeyRequest {
    /// Name for the API key
    #[schema(example = "production-key")]
    pub name: String,
}

/// Response containing API key info (without the raw key).
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct ApiKeyInfo {
    /// API key ID
    #[schema(value_type = String, example = "123e4567-e89b-12d3-a456-426614174000")]
    pub id: payments_types::ApiKeyId,
    /// Name of the API key
    pub name: String,
    /// Whether the key is active
    pub is_active: bool,
    /// When the key was created (ISO 8601)
    #[schema(value_type = String, example = "2024-01-01T00:00:00Z")]
    pub created_at: String,
    /// When the key was last used (ISO 8601)
    #[schema(value_type = Option<String>)]
    pub last_used_at: Option<String>,
}

/// Create a new API key (requires authentication).
#[tracing::instrument(skip(state), fields(key_name = %req.name))]
pub async fn create_api_key<R: TransactionRepository>(
    State(state): State<Arc<AppState<R>>>,
    Json(req): Json<CreateApiKeyRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let (_api_key, raw_key) = state
        .service
        .repo()
        .create_api_key(&req.name)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(BootstrapResponse {
            api_key: raw_key,
            message: "API key created. Save this key securely - it won't be shown again!".into(),
        }),
    ))
}

/// List all active API keys (without exposing raw keys).
#[tracing::instrument(skip(state))]
pub async fn list_api_keys<R: TransactionRepository>(
    State(state): State<Arc<AppState<R>>>,
) -> Result<impl IntoResponse, ApiError> {
    let keys = state
        .service
        .repo()
        .list_api_keys()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let response: Vec<ApiKeyInfo> = keys
        .into_iter()
        .map(|k| ApiKeyInfo {
            id: k.id,
            name: k.name,
            is_active: k.is_active,
            created_at: k.created_at.to_rfc3339(),
            last_used_at: k.last_used_at.map(|dt| dt.to_rfc3339()),
        })
        .collect();

    Ok(Json(response))
}

/// Delete (deactivate) an API key.
#[tracing::instrument(skip(state), fields(key_id = %id))]
pub async fn delete_api_key<R: TransactionRepository>(
    State(state): State<Arc<AppState<R>>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let key_id: payments_types::ApiKeyId = id
        .parse()
        .map_err(|_| AppError::BadRequest("Invalid API key ID".into()))?;

    let deleted = state
        .service
        .repo()
        .delete_api_key(key_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    if deleted {
        Ok(StatusCode::NO_CONTENT.into_response())
    } else {
        Err(AppError::NotFound("API key not found".into()).into())
    }
}

// ─────────────────────────────────────────────────────────────────────────────

// Webhooks
// ─────────────────────────────────────────────────────────────────────────────

/// Register a new webhook endpoint.
#[tracing::instrument(skip(state), fields(url = %req.url))]
pub async fn register_webhook<R: TransactionRepository>(
    State(state): State<Arc<AppState<R>>>,
    Json(req): Json<payments_types::RegisterWebhookRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Validate URL
    if req.url.is_empty() {
        return Err(AppError::BadRequest("Webhook URL cannot be empty".into()).into());
    }

    let endpoint = state
        .service
        .repo()
        .register_webhook_endpoint(&req.url, req.events)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(payments_types::WebhookResponse {
            id: payments_types::WebhookEndpointId::from_uuid(endpoint.id),
            url: endpoint.url,
            secret: endpoint.secret,
            events: endpoint.events,
            is_active: endpoint.is_active,
        }),
    ))
}

/// List all active webhook endpoints.
#[tracing::instrument(skip(state))]
pub async fn list_webhooks<R: TransactionRepository>(
    State(state): State<Arc<AppState<R>>>,
) -> Result<impl IntoResponse, ApiError> {
    let endpoints = state
        .service
        .repo()
        .list_webhook_endpoints()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let response: Vec<_> = endpoints
        .into_iter()
        .map(|ep| payments_types::WebhookResponse {
            id: payments_types::WebhookEndpointId::from_uuid(ep.id),
            url: ep.url,
            secret: ep.secret,
            events: ep.events,
            is_active: ep.is_active,
        })
        .collect();

    Ok(Json(response))
}
