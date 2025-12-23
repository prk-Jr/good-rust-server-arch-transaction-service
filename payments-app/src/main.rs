//! # Payments Application
//!
//! Binary that wires together all the components:
//! - Load configuration from environment
//! - Initialize the repository adapter
//! - Create the payment service
//! - Start the HTTP server

mod config;

use opentelemetry::global;
use opentelemetry_sdk::{propagation::TraceContextPropagator, trace as sdktrace};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use payments_hex::{PaymentService, inbound::HttpServer};
use payments_repo::build_repo;

fn init_tracer() -> (sdktrace::Tracer, sdktrace::SdkTracerProvider) {
    global::set_text_map_propagator(TraceContextPropagator::new());

    // Use gRPC exporter with batch processing (non-blocking)
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .build()
        .expect("failed to create OTLP span exporter");

    let provider = sdktrace::SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .build();

    global::set_tracer_provider(provider.clone());

    use opentelemetry::trace::TracerProvider as _;
    (provider.tracer("payments-service"), provider)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Initialize OpenTelemetry tracing
    let (otel_tracer, otel_provider) = init_tracer();
    let telemetry = tracing_opentelemetry::layer().with_tracer(otel_tracer);

    // Initialize tracing subscriber
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,payments_app=debug,payments_hex=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .with(telemetry)
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

    // Ensure traces are flushed before exit
    let _ = otel_provider.shutdown();
    Ok(())
}
