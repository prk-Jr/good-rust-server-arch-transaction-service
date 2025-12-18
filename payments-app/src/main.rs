//! # Payments Application
//!
//! Binary that wires together all the components:
//! - Load configuration from environment
//! - Initialize the repository adapter
//! - Create the payment service
//! - Start the HTTP server

mod config;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use payments_hex::{PaymentService, inbound::HttpServer};
use payments_repo::build_repo;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,payments=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = config::Config::from_env()?;

    tracing::info!("Starting payments server on port {}", config.port);
    tracing::info!("Using database: {}", config.database_url);

    // Build repository (handles connection and migration)
    let repo = build_repo(&config.database_url).await?;

    // Create the payment service
    let service = PaymentService::new(repo);

    // Create and run the HTTP server
    let server = HttpServer::new(service);
    let addr = format!("0.0.0.0:{}", config.port);

    server.run(&addr).await?;

    Ok(())
}
