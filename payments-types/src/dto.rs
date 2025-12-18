//! Data Transfer Objects (DTOs) for requests and responses.

use serde::{Deserialize, Serialize};

use crate::domain::{AccountId, Currency, TransactionId};

// ─────────────────────────────────────────────────────────────────────────────
// Account DTOs
// ─────────────────────────────────────────────────────────────────────────────

/// Request to create a new account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAccountRequest {
    pub name: String,
    #[serde(default = "default_currency")]
    pub currency: Currency,
}

fn default_currency() -> Currency {
    Currency::USD
}

/// Response after creating an account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountResponse {
    pub id: AccountId,
    pub name: String,
    pub balance: i64,
    pub currency: Currency,
}

// ─────────────────────────────────────────────────────────────────────────────
// Transaction DTOs
// ─────────────────────────────────────────────────────────────────────────────

/// Request to deposit money into an account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositRequest {
    pub account_id: AccountId,
    pub amount: i64,
    pub currency: Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

/// Request to withdraw money from an account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawRequest {
    pub account_id: AccountId,
    pub amount: i64,
    pub currency: Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

/// Request to transfer money between accounts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRequest {
    pub from_account_id: AccountId,
    pub to_account_id: AccountId,
    pub amount: i64,
    pub currency: Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

/// Response after a successful transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResponse {
    pub transaction_id: TransactionId,
    pub status: TransactionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_balance_source: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_balance_destination: Option<i64>,
}

/// Status of a transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransactionStatus {
    Success,
    Pending,
    Failed,
}
