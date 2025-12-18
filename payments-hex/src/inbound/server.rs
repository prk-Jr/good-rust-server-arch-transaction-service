//! HTTP Server configuration and startup.

use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};
use tower_http::trace::TraceLayer;

use payments_types::TransactionRepository;

use super::handlers::{self, AppState};
use crate::PaymentService;

/// HTTP Server for the Payments API.
pub struct HttpServer<R: TransactionRepository> {
    state: Arc<AppState<R>>,
}

impl<R: TransactionRepository> HttpServer<R> {
    /// Creates a new HTTP server with the given service.
    pub fn new(service: PaymentService<R>) -> Self {
        Self {
            state: Arc::new(AppState { service }),
        }
    }

    /// Builds the Axum router with all routes.
    pub fn router(&self) -> Router {
        Router::new()
            .route("/health", get(handlers::health))
            .route("/api/accounts", post(handlers::create_account::<R>))
            .route("/api/accounts", get(handlers::list_accounts::<R>))
            .route("/api/accounts/{id}", get(handlers::get_account::<R>))
            .route(
                "/api/accounts/{id}/transactions",
                get(handlers::list_transactions::<R>),
            )
            .route("/api/transactions/deposit", post(handlers::deposit::<R>))
            .route("/api/transactions/withdraw", post(handlers::withdraw::<R>))
            .route("/api/transactions/transfer", post(handlers::transfer::<R>))
            .layer(TraceLayer::new_for_http())
            .with_state(self.state.clone())
    }

    /// Runs the server on the given address.
    pub async fn run(self, addr: &str) -> anyhow::Result<()> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        tracing::info!("Server listening on {}", listener.local_addr()?);
        axum::serve(listener, self.router()).await?;
        Ok(())
    }
}
