//! Port traits (interfaces for adapters).
//!
//! These are the contracts that adapters must implement.
//! The application layer depends on these traits, not concrete implementations.

mod exchange;
mod repository;

pub use exchange::{ExchangeError, ExchangeRateProvider};
pub use repository::TransactionRepository;
