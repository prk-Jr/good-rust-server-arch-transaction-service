//! Payments CLI
//!
//! Command-line interface for the Payments API.

use anyhow::Result;
use clap::{Parser, Subcommand};

use payments_client::PaymentsClient;
use payments_types::{AccountId, CurrencyCode};

#[derive(Parser)]
#[command(name = "payments")]
#[command(author, version, about = "Payments API CLI client", long_about = None)]
struct Cli {
    /// Base URL of the Payments API
    #[arg(
        long,
        env = "PAYMENTS_API_URL",
        default_value = "http://localhost:3000"
    )]
    api_url: String,

    /// API key for authentication
    #[arg(long, env = "PAYMENTS_API_KEY")]
    api_key: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Account operations
    Account {
        #[command(subcommand)]
        action: AccountCommands,
    },
    /// Transaction operations  
    Transaction {
        #[command(subcommand)]
        action: TransactionCommands,
    },
    /// Webhook operations
    Webhook {
        #[command(subcommand)]
        action: WebhookCommands,
    },
    /// API key management
    Key {
        #[command(subcommand)]
        action: KeyCommands,
    },
    /// Bootstrap the first API key
    Bootstrap {
        /// Name for the new API key
        #[arg(long, default_value = "bootstrap-key")]
        name: String,
    },
    /// Check API health
    Health,
}

#[derive(Subcommand)]
enum AccountCommands {
    /// Create a new account
    Create {
        /// Account name
        name: String,
        /// Currency (USD, EUR, GBP, INR)
        #[arg(long, default_value = "USD")]
        currency: String,
    },
    /// Get account details
    Get {
        /// Account ID (UUID)
        id: String,
    },
    /// List all accounts
    List,
}

#[derive(Subcommand)]
enum TransactionCommands {
    /// Deposit funds into an account
    Deposit {
        #[arg(long)]
        account: String,
        #[arg(long)]
        amount: i64,
        #[arg(long, default_value = "USD")]
        currency: String,
        #[arg(long)]
        idempotency_key: Option<String>,
        #[arg(long)]
        reference: Option<String>,
    },
    /// Withdraw funds from an account
    Withdraw {
        #[arg(long)]
        account: String,
        #[arg(long)]
        amount: i64,
        #[arg(long, default_value = "USD")]
        currency: String,
        #[arg(long)]
        idempotency_key: Option<String>,
        #[arg(long)]
        reference: Option<String>,
    },
    /// Transfer funds between accounts
    Transfer {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        amount: i64,
        #[arg(long, default_value = "USD")]
        currency: String,
        #[arg(long)]
        idempotency_key: Option<String>,
        #[arg(long)]
        reference: Option<String>,
    },
}

#[derive(Subcommand)]
enum WebhookCommands {
    /// Register a new webhook endpoint
    Register {
        /// URL to receive webhooks
        #[arg(long)]
        url: String,
        /// Event types to subscribe to (comma-separated)
        #[arg(long, value_delimiter = ',', default_value = "")]
        events: Vec<String>,
    },
    /// List registered webhook endpoints
    List,
    /// Start a local webhook listener
    Listen {
        /// Port to listen on
        #[arg(long, default_value = "3000")]
        port: u16,
    },
}

#[derive(Subcommand)]
enum KeyCommands {
    /// Create a new API key
    Create {
        /// Name for the new key
        #[arg(long)]
        name: String,
    },
    /// List all API keys
    List,
    /// Delete (deactivate) an API key
    Delete {
        /// API key ID (UUID)
        #[arg(long)]
        id: String,
    },
}

fn parse_currency(s: &str) -> Result<CurrencyCode> {
    match s.to_uppercase().as_str() {
        "USD" => Ok(CurrencyCode::USD),
        "EUR" => Ok(CurrencyCode::EUR),
        "GBP" => Ok(CurrencyCode::GBP),
        "INR" => Ok(CurrencyCode::INR),
        _ => anyhow::bail!("Unknown currency: {}. Supported: USD, EUR, GBP, INR", s),
    }
}

fn parse_account_id(s: &str) -> Result<AccountId> {
    s.parse()
        .map_err(|_| anyhow::anyhow!("Invalid account ID: {}", s))
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    let mut client = PaymentsClient::new(&cli.api_url);
    if let Some(key) = cli.api_key {
        client = client.with_api_key(key);
    }

    match cli.command {
        Commands::Health => {
            let healthy = client.health().await?;
            if healthy {
                println!("✓ API is healthy");
            } else {
                println!("✗ API is not healthy");
                std::process::exit(1);
            }
        }

        Commands::Account { action } => match action {
            AccountCommands::Create { name, currency } => {
                let currency = parse_currency(&currency)?;
                let account = client.create_account(&name, currency).await?;
                println!("{}", serde_json::to_string_pretty(&account)?);
            }
            AccountCommands::Get { id } => {
                let account_id = parse_account_id(&id)?;
                let account = client.get_account(account_id).await?;
                println!("{}", serde_json::to_string_pretty(&account)?);
            }
            AccountCommands::List => {
                let accounts = client.list_accounts().await?;
                println!("{}", serde_json::to_string_pretty(&accounts)?);
            }
        },

        Commands::Transaction { action } => match action {
            TransactionCommands::Deposit {
                account,
                amount,
                currency,
                idempotency_key,
                reference,
            } => {
                let account_id = parse_account_id(&account)?;
                let currency = parse_currency(&currency)?;
                let tx = client
                    .deposit(account_id, amount, currency, idempotency_key, reference)
                    .await?;
                println!("{}", serde_json::to_string_pretty(&tx)?);
            }
            TransactionCommands::Withdraw {
                account,
                amount,
                currency,
                idempotency_key,
                reference,
            } => {
                let account_id = parse_account_id(&account)?;
                let currency = parse_currency(&currency)?;
                let tx = client
                    .withdraw(account_id, amount, currency, idempotency_key, reference)
                    .await?;
                println!("{}", serde_json::to_string_pretty(&tx)?);
            }
            TransactionCommands::Transfer {
                from,
                to,
                amount,
                currency,
                idempotency_key,
                reference,
            } => {
                let from_id = parse_account_id(&from)?;
                let to_id = parse_account_id(&to)?;
                let currency = parse_currency(&currency)?;
                let tx = client
                    .transfer(from_id, to_id, amount, currency, idempotency_key, reference)
                    .await?;
                println!("{}", serde_json::to_string_pretty(&tx)?);
            }
        },

        Commands::Webhook { action } => match action {
            WebhookCommands::Register { url, events } => {
                // Filter out empty strings from events
                let events: Vec<String> = events.into_iter().filter(|e| !e.is_empty()).collect();
                let webhook = client.register_webhook(&url, events).await?;
                println!("{}", serde_json::to_string_pretty(&webhook)?);
            }
            WebhookCommands::List => {
                let webhooks = client.list_webhooks().await?;
                println!("{}", serde_json::to_string_pretty(&webhooks)?);
            }
            WebhookCommands::Listen { port } => {
                let app =
                    axum::Router::new().route("/webhook", axum::routing::post(handle_webhook));
                let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
                println!("Listening for webhooks on {}", addr);
                let listener = tokio::net::TcpListener::bind(&addr).await?;
                axum::serve(listener, app).await?;
            }
        },

        Commands::Key { action } => match action {
            KeyCommands::Create { name } => {
                let api_key = client.create_api_key(&name).await?;
                println!("{}", api_key);
            }
            KeyCommands::List => {
                let keys = client.list_api_keys().await?;
                println!("{}", serde_json::to_string_pretty(&keys)?);
            }
            KeyCommands::Delete { id } => {
                client.delete_api_key(&id).await?;
                println!("✓ API key deleted");
            }
        },

        Commands::Bootstrap { name } => {
            let api_key = client.bootstrap(&name).await?;
            println!("{}", api_key);
        }
    }

    Ok(())
}

async fn handle_webhook(
    headers: axum::http::HeaderMap,
    body: String,
) -> impl axum::response::IntoResponse {
    println!("POST /webhook HTTP/1.1");
    for (name, value) in &headers {
        println!("{}: {:?}", name, value);
    }
    println!();
    println!("{}", body);
    println!("----------------------------------------");
    axum::http::StatusCode::OK
}
