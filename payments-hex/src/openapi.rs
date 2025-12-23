//! OpenAPI specification and documentation.

#![allow(dead_code)] // Path functions are only used by utoipa for documentation generation

use payments_types::domain::{AccountId, Currency, TransactionId, WebhookEndpointId};
use payments_types::dto::{
    AccountResponse, CreateAccountRequest, DepositRequest, RegisterWebhookRequest,
    TransactionResponse, TransactionStatus, TransferRequest, WebhookResponse, WithdrawRequest,
};
use utoipa::{
    Modify, OpenApi,
    openapi::security::{Http, HttpAuthScheme, SecurityScheme},
};

use crate::inbound::handlers::{
    ApiKeyInfo, BootstrapRequest, BootstrapResponse, CreateApiKeyRequest,
};

// Dummy functions to generate path documentation
// These are not the actual handlers, just for OpenAPI path generation

/// Health check endpoint
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "Service is healthy", body = inline(serde_json::Value), example = json!({"status": "healthy"}))
    )
)]
async fn health() {}

/// Bootstrap first API key
#[utoipa::path(
    post,
    path = "/api/bootstrap",
    tag = "auth",
    request_body = BootstrapRequest,
    responses(
        (status = 201, description = "API key created successfully", body = BootstrapResponse),
        (status = 400, description = "Bootstrap not allowed - API keys already exist")
    )
)]
async fn bootstrap() {}

/// Create a new API key (requires authentication)
#[utoipa::path(
    post,
    path = "/api/keys",
    tag = "auth",
    request_body = CreateApiKeyRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 201, description = "API key created", body = BootstrapResponse),
        (status = 401, description = "Unauthorized")
    )
)]
async fn create_api_key() {}

/// List all API keys (without exposing raw keys)
#[utoipa::path(
    get,
    path = "/api/keys",
    tag = "auth",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List of API keys", body = Vec<ApiKeyInfo>),
        (status = 401, description = "Unauthorized")
    )
)]
async fn list_api_keys() {}

/// Delete (deactivate) an API key
#[utoipa::path(
    delete,
    path = "/api/keys/{id}",
    tag = "auth",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "API key ID (UUID)")
    ),
    responses(
        (status = 204, description = "API key deleted"),
        (status = 404, description = "API key not found"),
        (status = 401, description = "Unauthorized")
    )
)]
async fn delete_api_key() {}

/// Create a new account

#[utoipa::path(
    post,
    path = "/api/accounts",
    tag = "accounts",
    request_body = CreateAccountRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 201, description = "Account created successfully", body = AccountResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized")
    )
)]
async fn create_account() {}

/// List all accounts
#[utoipa::path(
    get,
    path = "/api/accounts",
    tag = "accounts",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List of accounts", body = Vec<AccountResponse>),
        (status = 401, description = "Unauthorized")
    )
)]
async fn list_accounts() {}

/// Get account by ID
#[utoipa::path(
    get,
    path = "/api/accounts/{id}",
    tag = "accounts",
    security(("bearer_auth" = [])),
    params(
        ("id" = AccountId, Path, description = "Account ID (UUID)")
    ),
    responses(
        (status = 200, description = "Account details", body = AccountResponse),
        (status = 404, description = "Account not found"),
        (status = 401, description = "Unauthorized")
    )
)]
async fn get_account() {}

/// Deposit money into an account
#[utoipa::path(
    post,
    path = "/api/transactions/deposit",
    tag = "transactions",
    request_body = DepositRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Deposit successful", body = TransactionResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized")
    )
)]
async fn deposit() {}

/// Withdraw money from an account
#[utoipa::path(
    post,
    path = "/api/transactions/withdraw",
    tag = "transactions",
    request_body = WithdrawRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Withdrawal successful", body = TransactionResponse),
        (status = 400, description = "Insufficient funds or invalid request"),
        (status = 401, description = "Unauthorized")
    )
)]
async fn withdraw() {}

/// Transfer money between accounts
#[utoipa::path(
    post,
    path = "/api/transactions/transfer",
    tag = "transactions",
    request_body = TransferRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Transfer successful", body = TransactionResponse),
        (status = 400, description = "Insufficient funds or invalid accounts"),
        (status = 401, description = "Unauthorized")
    )
)]
async fn transfer() {}

/// Register a webhook endpoint
#[utoipa::path(
    post,
    path = "/api/webhooks",
    tag = "webhooks",
    request_body = RegisterWebhookRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 201, description = "Webhook registered successfully", body = WebhookResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized")
    )
)]
async fn register_webhook() {}

/// List all webhook endpoints
#[utoipa::path(
    get,
    path = "/api/webhooks",
    tag = "webhooks",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List of webhook endpoints", body = Vec<WebhookResponse>),
        (status = 401, description = "Unauthorized")
    )
)]
async fn list_webhooks() {}

/// OpenAPI documentation for the Payments API.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Payments Transaction Service API",
        version = "1.0.0",
        description = "A production-ready payment transaction service with accounts, transactions, and webhooks.\n\n## Authentication\n\nMost endpoints require Bearer token authentication. Use the `/api/bootstrap` endpoint to create your first API key, then include it in the `Authorization` header:\n\n```\nAuthorization: Bearer sk_your_api_key_here\n```",
        license(name = "MIT"),
    ),
    paths(
        health,
        bootstrap,
        create_api_key,
        list_api_keys,
        delete_api_key,
        create_account,
        list_accounts,
        get_account,
        deposit,
        withdraw,
        transfer,
        register_webhook,
        list_webhooks,
    ),
    components(
        schemas(
            CreateAccountRequest,
            AccountResponse,
            DepositRequest,
            WithdrawRequest,
            TransferRequest,
            TransactionResponse,
            TransactionStatus,
            RegisterWebhookRequest,
            WebhookResponse,
            Currency,
            AccountId,
            TransactionId,
            WebhookEndpointId,
            BootstrapRequest,
            BootstrapResponse,
            CreateApiKeyRequest,
            ApiKeyInfo,
        )
    ),

    modifiers(&SecurityAddon),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "auth", description = "API key management"),
        (name = "accounts", description = "Account management operations"),
        (name = "transactions", description = "Deposit, withdraw, and transfer operations"),
        (name = "webhooks", description = "Webhook endpoint management"),
    )
)]
pub struct ApiDoc;

/// Security scheme modifier for Bearer token authentication.
struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
            );
        }
    }
}
