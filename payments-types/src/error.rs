//! Error types for the payment service.

use crate::domain::{AccountId, Currency};

/// Domain-level errors (business logic violations).
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("Amount cannot be negative")]
    NegativeAmount,

    #[error("Currency mismatch: expected {expected}, got {got}")]
    CurrencyMismatch { expected: Currency, got: Currency },

    #[error("Insufficient funds: available {available}, requested {requested}")]
    InsufficientFunds { available: i64, requested: i64 },

    #[error("Account not found: {0}")]
    AccountNotFound(AccountId),

    #[error("Cannot transfer between accounts with different currencies")]
    CrossCurrencyTransfer,

    #[error("Validation error: {0}")]
    ValidationError(String),
}

/// Repository-level errors (data access failures).
#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    #[error(transparent)]
    Domain(#[from] DomainError),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Transaction error: {0}")]
    Transaction(String),

    #[error("Entity not found")]
    NotFound,

    #[error("Conflict: {0}")]
    Conflict(String),
}

/// Application-level errors (for HTTP responses).
///
/// Maps cleanly to HTTP status codes.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Insufficient funds: available {available}, requested {requested}")]
    InsufficientFunds { available: i64, requested: i64 },

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<RepoError> for AppError {
    fn from(err: RepoError) -> Self {
        match err {
            RepoError::Domain(DomainError::InsufficientFunds {
                available,
                requested,
            }) => AppError::InsufficientFunds {
                available,
                requested,
            },
            RepoError::Domain(DomainError::ValidationError(msg)) => AppError::BadRequest(msg),
            RepoError::Domain(DomainError::AccountNotFound(id)) => {
                AppError::NotFound(format!("Account not found: {}", id))
            }
            RepoError::Domain(e) => AppError::BadRequest(e.to_string()),
            RepoError::NotFound => AppError::NotFound("Resource not found".into()),
            RepoError::Database(e) => AppError::Internal(e),
            RepoError::Transaction(e) => AppError::Internal(e),
            RepoError::Conflict(e) => AppError::BadRequest(e),
        }
    }
}
