//! # Payments Types
//!
//! Domain types and port traits for the payment transaction service.
//! This crate has ZERO external IO dependencies - only data structures,
//! business rules, and trait definitions.
//!
//! ## Architecture
//!
//! This crate represents the **innermost core** of the hexagonal architecture:
//! - `domain/` - Pure domain types (Money, Account, Transaction)
//! - `ports/` - Trait definitions that adapters must implement
//! - `dto/` - Data Transfer Objects for API boundaries
//! - `error/` - Domain and application error types

pub mod domain;
pub mod dto;
pub mod error;
pub mod ports;

// Re-export commonly used types
pub use domain::{
    Account, AccountId, ApiKey, ApiKeyId, CurrencyCode, DynMoney, Transaction, TransactionId,
    TransactionType, WebhookEndpoint, WebhookEndpointId, WebhookEvent, WebhookStatus,
};
pub use dto::*;
pub use error::{AppError, DomainError, RepoError};
pub use ports::{ExchangeError, ExchangeRateProvider, TransactionRepository};

// Re-export type-safe currency types from exchange-rates for internal use
pub use exchange_rates::{Currency, EUR, GBP, INR, Money, USD};
