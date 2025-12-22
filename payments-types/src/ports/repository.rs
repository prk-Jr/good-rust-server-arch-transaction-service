//! Repository port trait.
//!
//! This is the primary port in our hexagonal architecture.
//! Adapters (Postgres, SQLite, InMemory) will implement this trait.

use crate::domain::{Account, AccountId, Transaction, TransactionId};
use crate::dto::{CreateAccountRequest, DepositRequest, TransferRequest, WithdrawRequest};
use crate::error::RepoError;

/// The main repository port for payment operations.
///
/// All operations that modify balances MUST be atomic.
/// Implementations should use database transactions to ensure consistency.
#[async_trait::async_trait]
pub trait TransactionRepository: Send + Sync + 'static {
    // ─────────────────────────────────────────────────────────────────────────────
    // Account Operations
    // ─────────────────────────────────────────────────────────────────────────────

    /// Creates a new account with zero balance.
    async fn create_account(&self, req: CreateAccountRequest) -> Result<Account, RepoError>;

    /// Gets an account by ID.
    async fn get_account(&self, id: AccountId) -> Result<Option<Account>, RepoError>;

    /// Lists all accounts.
    async fn list_accounts(&self) -> Result<Vec<Account>, RepoError>;

    // ─────────────────────────────────────────────────────────────────────────────
    // Transaction Operations (MUST be atomic)
    // ─────────────────────────────────────────────────────────────────────────────

    /// Deposits money into an account.
    async fn deposit(&self, req: DepositRequest) -> Result<Transaction, RepoError>;

    /// Withdraws money from an account.
    async fn withdraw(&self, req: WithdrawRequest) -> Result<Transaction, RepoError>;

    /// Transfers money between two accounts.
    async fn transfer(&self, req: TransferRequest) -> Result<Transaction, RepoError>;

    // ─────────────────────────────────────────────────────────────────────────────
    // Idempotency & History
    // ─────────────────────────────────────────────────────────────────────────────

    /// Finds a transaction by its idempotency key.
    async fn find_by_idempotency_key(&self, key: &str) -> Result<Option<Transaction>, RepoError>;

    /// Gets a transaction by ID.
    async fn get_transaction(&self, id: TransactionId) -> Result<Option<Transaction>, RepoError>;

    /// Lists transactions for an account.
    async fn list_transactions_for_account(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<Transaction>, RepoError>;

    // ─────────────────────────────────────────────────────────────────────────────
    // API Key Verification
    // ─────────────────────────────────────────────────────────────────────────────

    /// Verifies an API key hash and returns the associated ApiKey if valid and active.
    async fn verify_api_key_hash(&self, key_hash: &str)
    -> Result<Option<crate::ApiKey>, RepoError>;

    /// Creates a new API key with the given name and returns the raw key (only shown once).
    /// The key is stored as a hash in the database.
    async fn create_api_key(&self, name: &str) -> Result<(crate::ApiKey, String), RepoError>;

    /// Counts the number of active API keys in the system.
    async fn count_api_keys(&self) -> Result<i64, RepoError>;

    // ─────────────────────────────────────────────────────────────────────────────
    // Webhook Endpoint Management
    // ─────────────────────────────────────────────────────────────────────────────

    /// Registers a new webhook endpoint.
    async fn register_webhook_endpoint(
        &self,
        url: &str,
        events: Vec<String>,
    ) -> Result<crate::WebhookEndpoint, RepoError>;

    /// Lists all active webhook endpoints.
    async fn list_webhook_endpoints(&self) -> Result<Vec<crate::WebhookEndpoint>, RepoError>;

    /// Creates a new webhook event to be sent to a specific endpoint.
    async fn create_webhook_event(
        &self,
        endpoint_id: crate::WebhookEndpointId,
        event_type: &str,
        payload: serde_json::Value,
    ) -> Result<crate::WebhookEvent, RepoError>;
}
