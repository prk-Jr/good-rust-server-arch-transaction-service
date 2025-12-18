//! Port traits (interfaces for adapters).
//!
//! These are the contracts that adapters must implement.
//! The application layer depends on these traits, not concrete implementations.

mod repository;

pub use repository::TransactionRepository;
