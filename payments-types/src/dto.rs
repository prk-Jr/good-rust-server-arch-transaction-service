//! Data Transfer Objects (DTOs) for requests and responses.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::domain::{AccountId, Currency, TransactionId};

// ─────────────────────────────────────────────────────────────────────────────
// Account DTOs
// ─────────────────────────────────────────────────────────────────────────────

/// Request to create a new account.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateAccountRequest {
    /// Name of the account holder
    #[schema(example = "Alice")]
    pub name: String,
    #[serde(default = "default_currency")]
    pub currency: Currency,
}

fn default_currency() -> Currency {
    Currency::USD
}

/// Response after creating an account.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AccountResponse {
    /// Unique account identifier
    pub id: AccountId,
    /// Name of the account holder
    #[schema(example = "Alice")]
    pub name: String,
    /// Current balance in smallest currency unit (e.g., cents)
    #[schema(example = 10000)]
    pub balance: i64,
    pub currency: Currency,
}

// ─────────────────────────────────────────────────────────────────────────────
// Transaction DTOs
// ─────────────────────────────────────────────────────────────────────────────

/// Request to deposit money into an account.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DepositRequest {
    /// Target account ID
    pub account_id: AccountId,
    /// Amount to deposit in smallest currency unit
    #[schema(example = 1000)]
    pub amount: i64,
    pub currency: Currency,
    /// Optional idempotency key to prevent duplicate transactions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    /// Optional reference for the transaction
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

/// Request to withdraw money from an account.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WithdrawRequest {
    /// Source account ID
    pub account_id: AccountId,
    /// Amount to withdraw in smallest currency unit
    #[schema(example = 500)]
    pub amount: i64,
    pub currency: Currency,
    /// Optional idempotency key to prevent duplicate transactions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    /// Optional reference for the transaction
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

/// Request to transfer money between accounts.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TransferRequest {
    /// Source account ID
    pub from_account_id: AccountId,
    /// Destination account ID
    pub to_account_id: AccountId,
    /// Amount to transfer in smallest currency unit
    #[schema(example = 500)]
    pub amount: i64,
    pub currency: Currency,
    /// Optional idempotency key to prevent duplicate transactions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    /// Optional reference for the transaction
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

/// Response after a successful transaction.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TransactionResponse {
    /// Unique transaction identifier
    pub transaction_id: TransactionId,
    pub status: TransactionStatus,
    /// New balance of source account (for withdrawals/transfers)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_balance_source: Option<i64>,
    /// New balance of destination account (for deposits/transfers)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_balance_destination: Option<i64>,
}

/// Status of a transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransactionStatus {
    Success,
    Pending,
    Failed,
}

// ─────────────────────────────────────────────────────────────────────────────
// Webhook DTOs
// ─────────────────────────────────────────────────────────────────────────────

/// Request to register a webhook endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RegisterWebhookRequest {
    /// The URL to receive webhook notifications
    #[schema(example = "https://example.com/webhook")]
    pub url: String,
    /// Event types to subscribe to. If empty, subscribes to all events.
    #[serde(default)]
    #[schema(example = json!(["deposit.success", "withdraw.success"]))]
    pub events: Vec<String>,
}

/// Response after registering a webhook.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WebhookResponse {
    /// Unique webhook endpoint identifier
    pub id: crate::WebhookEndpointId,
    /// The registered webhook URL
    #[schema(example = "https://example.com/webhook")]
    pub url: String,
    /// Secret key for verifying webhook signatures (HMAC-SHA256)
    pub secret: String,
    /// List of subscribed event types
    pub events: Vec<String>,
    /// Whether the webhook is active
    pub is_active: bool,
}
