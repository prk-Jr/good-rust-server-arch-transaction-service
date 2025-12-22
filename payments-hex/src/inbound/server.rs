//! HTTP Server configuration and startup.

use std::sync::Arc;

use axum::{
    Router, middleware,
    routing::{get, post},
};
use tower_http::trace::TraceLayer;

use payments_types::TransactionRepository;

use super::auth::auth_middleware;
use super::handlers::{self, AppState};
use super::rate_limit::{RateLimiterState, rate_limit_middleware};
use crate::PaymentService;

/// HTTP Server for the Payments API.
pub struct HttpServer<R: TransactionRepository> {
    state: Arc<AppState<R>>,
    rate_limiter: Arc<RateLimiterState>,
}

impl<R: TransactionRepository> HttpServer<R> {
    /// Creates a new HTTP server with the given service.
    pub fn new(service: PaymentService<R>) -> Self {
        Self {
            state: Arc::new(AppState { service }),
            rate_limiter: Arc::new(RateLimiterState::default()), // 100 req/min default
        }
    }

    /// Creates a new HTTP server with custom rate limiting.
    pub fn with_rate_limit(service: PaymentService<R>, requests_per_minute: u32) -> Self {
        use std::time::Duration;
        Self {
            state: Arc::new(AppState { service }),
            rate_limiter: Arc::new(RateLimiterState::new(
                requests_per_minute,
                Duration::from_secs(60),
            )),
        }
    }

    /// Builds the Axum router with all routes.
    pub fn router(&self) -> Router {
        // Build HTTP metrics layer (uses globally set MeterProvider)
        let metrics = axum_otel_metrics::HttpMetricsLayerBuilder::new().build();

        Router::new()
            .route("/health", get(handlers::health))
            .route("/api/bootstrap", post(handlers::bootstrap::<R>))
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
            .route("/api/webhooks", post(handlers::register_webhook::<R>))
            .route("/api/webhooks", get(handlers::list_webhooks::<R>))
            .layer(metrics)
            .layer(middleware::from_fn_with_state(
                self.rate_limiter.clone(),
                rate_limit_middleware,
            ))
            .layer(middleware::from_fn_with_state(
                self.state.clone(),
                auth_middleware::<R>,
            ))
            .layer(TraceLayer::new_for_http())
            .with_state(self.state.clone())
    }

    /// Runs the server on the given address with graceful shutdown.
    pub async fn run(self, addr: &str) -> anyhow::Result<()> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        tracing::info!("Server listening on {}", listener.local_addr()?);

        axum::serve(listener, self.router())
            .with_graceful_shutdown(shutdown_signal())
            .await?;

        Ok(())
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received, starting graceful shutdown...");
}
