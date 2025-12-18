//! Payments CLI
//!
//! Command-line interface for the Payments API.

use anyhow::Result;
use clap::{Parser, Subcommand};

use payments_client::PaymentsClient;
use payments_types::{AccountId, Currency};

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

fn parse_currency(s: &str) -> Result<Currency> {
    match s.to_uppercase().as_str() {
        "USD" => Ok(Currency::USD),
        "EUR" => Ok(Currency::EUR),
        "GBP" => Ok(Currency::GBP),
        "INR" => Ok(Currency::INR),
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
    }

    Ok(())
}
