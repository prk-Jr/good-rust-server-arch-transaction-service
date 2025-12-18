//! # Payments Hex
//!
//! Application service layer and HTTP adapter for the payments service.
//!
//! ## Architecture
//!
//! - `service/` - Application service (orchestrates domain operations)
//! - `inbound/` - HTTP adapter (Axum server)
//!
//! The service is generic over `R: TransactionRepository`, allowing
//! different repository implementations to be injected.

pub mod inbound;
pub mod service;

#[cfg(test)]
mod service_tests;

pub use service::PaymentService;
