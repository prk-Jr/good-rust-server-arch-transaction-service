//! Client example demonstrating full payment flows against a running server.
//!
//! Run with: cargo run -p payments-app --example client_example --no-default-features --features sqlite

use payments_client::PaymentsClient;
use payments_hex::{PaymentService, inbound::HttpServer};
use payments_repo::build_repo;
use payments_types::Currency;
use std::net::SocketAddr;
use tempfile::tempdir;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt().with_env_filter("info").init();

    // Find an available port
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr: SocketAddr = listener.local_addr()?;
    let port = addr.port();
    drop(listener);

    // Use a temp file-backed SQLite DB
    let tmp = tempdir()?;
    let db_path = tmp.path().join("payments.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

    println!("🚀 Starting server on port {port}...");
    println!("   Database: {db_url}");

    // Build repository (handles connection and migration)
    let repo = build_repo(&db_url).await?;

    // Start server in background
    let service = PaymentService::new(repo);
    let server = HttpServer::new(service);
    let router = server.router();

    let server_addr = format!("127.0.0.1:{port}");
    tokio::spawn(async move {
        axum::serve(
            TcpListener::bind(&server_addr).await.unwrap(),
            router.into_make_service(),
        )
        .await
        .unwrap();
    });

    // Wait for server to start
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Create client
    let base_url = format!("http://127.0.0.1:{port}");
    let client = PaymentsClient::new(&base_url);

    // ─────────────────────────────────────────────────────────────────────────
    // Demo: Full payment flow
    // ─────────────────────────────────────────────────────────────────────────

    // Health check
    let health = client.health().await?;
    println!("✅ Server health: {health}");

    //assert response is error unauthorized
    let response = client.create_account("Alice Corp", Currency::USD).await;
    assert!(response.is_err());
    println!("✅ Unauthorized without key: {}", response.unwrap_err());

    // key
    let key = client.bootstrap("test").await?;
    println!("✅ Server key generated: {key}");

    let client = client.with_api_key(key);

    // Create accounts
    let alice = client.create_account("Alice Corp", Currency::USD).await?;
    println!("✅ Created account: {} (id={})", alice.name, alice.id);

    let bob = client.create_account("Bob Inc", Currency::USD).await?;
    println!("✅ Created account: {} (id={})", bob.name, bob.id);

    // Deposit to Alice
    let deposit = client
        .deposit(alice.id, 10000, Currency::USD, None, None)
        .await?;
    println!("✅ Deposited $100.00 to Alice (tx={})", deposit.id);

    let alice = client.get_account(alice.id).await?;
    println!(
        "   Alice balance: ${:.2}",
        alice.balance.amount() as f64 / 100.0
    );

    // Transfer from Alice to Bob
    let transfer = client
        .transfer(alice.id, bob.id, 3500, Currency::USD, None, None)
        .await?;
    println!(
        "✅ Transferred $35.00 from Alice to Bob (tx={})",
        transfer.id
    );

    let alice = client.get_account(alice.id).await?;
    let bob = client.get_account(bob.id).await?;
    println!(
        "   Alice balance: ${:.2}",
        alice.balance.amount() as f64 / 100.0
    );
    println!(
        "   Bob balance: ${:.2}",
        bob.balance.amount() as f64 / 100.0
    );

    // Withdraw from Bob
    let withdraw = client
        .withdraw(bob.id, 1500, Currency::USD, None, None)
        .await?;
    println!("✅ Withdrew $15.00 from Bob (tx={})", withdraw.id);

    let bob = client.get_account(bob.id).await?;
    println!(
        "   Bob balance: ${:.2}",
        bob.balance.amount() as f64 / 100.0
    );

    // List all accounts
    let accounts = client.list_accounts().await?;
    println!("\n📋 All accounts:");
    for acc in accounts {
        println!(
            "   - {} ({}): ${:.2}",
            acc.name,
            acc.id,
            acc.balance.amount() as f64 / 100.0
        );
    }

    println!("\n🎉 Example completed successfully!");

    Ok(())
}
