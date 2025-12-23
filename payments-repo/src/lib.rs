//! # Payments Repository
//!
//! Concrete repository implementations (adapters) for the payments service.
//! This crate provides database adapters that implement the `TransactionRepository` port.

#[cfg(not(any(feature = "postgres", feature = "sqlite")))]
compile_error!("Enable a repo feature: `postgres` or `sqlite`.");

use async_trait::async_trait;
use payments_types::{
    Account, AccountId, CreateAccountRequest, DepositRequest, RepoError, Transaction,
    TransactionId, TransactionRepository, TransferRequest, WithdrawRequest,
};

#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(feature = "sqlite")]
pub mod sqlite;

#[cfg(any(feature = "postgres", feature = "sqlite"))]
mod types;

pub mod security;
pub mod webhooks;

#[cfg(feature = "sqlite")]
#[cfg(test)]
mod sqlite_tests;

/// Unified repository wrapper that handles both SQLite and PostgreSQL.
pub struct Repo {
    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    inner: sqlite::SqliteRepo,
    #[cfg(feature = "postgres")]
    inner: postgres::PostgresRepo,
}

/// Build and initialize a repository from a database URL.
///
/// This function:
/// 1. Connects to the database
/// 2. Runs migrations to create tables
/// 3. Returns a ready-to-use `Repo`
///
/// # Examples
///
/// ```ignore
/// // SQLite (with `sqlite` feature)
/// let repo = build_repo("sqlite://payments.db?mode=rwc").await?;
///
/// // PostgreSQL (with `postgres` feature)
/// let repo = build_repo("postgres://user:pass@localhost/payments").await?;
/// ```
pub async fn build_repo(database_url: &str) -> anyhow::Result<Repo> {
    Repo::new(database_url).await
}

impl Repo {
    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        let inner = sqlite::SqliteRepo::new(database_url).await?;
        Ok(Self { inner })
    }

    #[cfg(feature = "postgres")]
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        let inner = postgres::PostgresRepo::new(database_url).await?;
        Ok(Self { inner })
    }

    pub async fn get_pending_webhooks(
        &self,
        limit: i64,
    ) -> Result<Vec<payments_types::WebhookEvent>, RepoError> {
        self.inner.get_pending_webhooks(limit).await
    }

    pub async fn update_webhook_status(
        &self,
        id: uuid::Uuid,
        status: payments_types::WebhookStatus,
        last_error: Option<String>,
    ) -> Result<(), RepoError> {
        self.inner
            .update_webhook_status(id, status, last_error)
            .await
    }
}

// Re-export individual repos for direct use if needed
#[cfg(feature = "postgres")]
pub use postgres::PostgresRepo;
#[cfg(feature = "sqlite")]
pub use sqlite::SqliteRepo;

// ─────────────────────────────────────────────────────────────────────────────
// Implement TransactionRepository for Repo (delegation)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(all(feature = "sqlite", not(feature = "postgres")))]
#[async_trait]
impl TransactionRepository for Repo {
    async fn create_account(&self, req: CreateAccountRequest) -> Result<Account, RepoError> {
        self.inner.create_account(req).await
    }

    async fn get_account(&self, id: AccountId) -> Result<Option<Account>, RepoError> {
        self.inner.get_account(id).await
    }

    async fn list_accounts(&self) -> Result<Vec<Account>, RepoError> {
        self.inner.list_accounts().await
    }

    async fn deposit(&self, req: DepositRequest) -> Result<Transaction, RepoError> {
        self.inner.deposit(req).await
    }

    async fn withdraw(&self, req: WithdrawRequest) -> Result<Transaction, RepoError> {
        self.inner.withdraw(req).await
    }

    async fn transfer(&self, req: TransferRequest) -> Result<Transaction, RepoError> {
        self.inner.transfer(req).await
    }

    async fn find_by_idempotency_key(&self, key: &str) -> Result<Option<Transaction>, RepoError> {
        self.inner.find_by_idempotency_key(key).await
    }

    async fn get_transaction(&self, id: TransactionId) -> Result<Option<Transaction>, RepoError> {
        self.inner.get_transaction(id).await
    }

    async fn list_transactions_for_account(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<Transaction>, RepoError> {
        self.inner.list_transactions_for_account(account_id).await
    }

    async fn verify_api_key_hash(
        &self,
        key_hash: &str,
    ) -> Result<Option<payments_types::ApiKey>, RepoError> {
        self.inner.verify_api_key_hash(key_hash).await
    }

    async fn create_api_key(
        &self,
        name: &str,
    ) -> Result<(payments_types::ApiKey, String), RepoError> {
        self.inner.create_api_key(name).await
    }

    async fn count_api_keys(&self) -> Result<i64, RepoError> {
        self.inner.count_api_keys().await
    }

    async fn list_api_keys(&self) -> Result<Vec<payments_types::ApiKey>, RepoError> {
        self.inner.list_api_keys().await
    }

    async fn delete_api_key(&self, id: payments_types::ApiKeyId) -> Result<bool, RepoError> {
        self.inner.delete_api_key(id).await
    }

    async fn register_webhook_endpoint(
        &self,
        url: &str,
        events: Vec<String>,
    ) -> Result<payments_types::WebhookEndpoint, RepoError> {
        self.inner.register_webhook_endpoint(url, events).await
    }

    async fn list_webhook_endpoints(
        &self,
    ) -> Result<Vec<payments_types::WebhookEndpoint>, RepoError> {
        self.inner.list_webhook_endpoints().await
    }

    async fn create_webhook_event(
        &self,
        endpoint_id: payments_types::WebhookEndpointId,
        event_type: &str,
        payload: serde_json::Value,
    ) -> Result<payments_types::WebhookEvent, RepoError> {
        self.inner
            .create_webhook_event(endpoint_id, event_type, payload)
            .await
    }
}

#[cfg(feature = "postgres")]
#[async_trait]
impl TransactionRepository for Repo {
    async fn create_account(&self, req: CreateAccountRequest) -> Result<Account, RepoError> {
        self.inner.create_account(req).await
    }

    async fn get_account(&self, id: AccountId) -> Result<Option<Account>, RepoError> {
        self.inner.get_account(id).await
    }

    async fn list_accounts(&self) -> Result<Vec<Account>, RepoError> {
        self.inner.list_accounts().await
    }

    async fn deposit(&self, req: DepositRequest) -> Result<Transaction, RepoError> {
        self.inner.deposit(req).await
    }

    async fn withdraw(&self, req: WithdrawRequest) -> Result<Transaction, RepoError> {
        self.inner.withdraw(req).await
    }

    async fn transfer(&self, req: TransferRequest) -> Result<Transaction, RepoError> {
        self.inner.transfer(req).await
    }

    async fn find_by_idempotency_key(&self, key: &str) -> Result<Option<Transaction>, RepoError> {
        self.inner.find_by_idempotency_key(key).await
    }

    async fn get_transaction(&self, id: TransactionId) -> Result<Option<Transaction>, RepoError> {
        self.inner.get_transaction(id).await
    }

    async fn list_transactions_for_account(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<Transaction>, RepoError> {
        self.inner.list_transactions_for_account(account_id).await
    }

    async fn verify_api_key_hash(
        &self,
        key_hash: &str,
    ) -> Result<Option<payments_types::ApiKey>, RepoError> {
        self.inner.verify_api_key_hash(key_hash).await
    }

    async fn create_api_key(
        &self,
        name: &str,
    ) -> Result<(payments_types::ApiKey, String), RepoError> {
        self.inner.create_api_key(name).await
    }

    async fn count_api_keys(&self) -> Result<i64, RepoError> {
        self.inner.count_api_keys().await
    }

    async fn list_api_keys(&self) -> Result<Vec<payments_types::ApiKey>, RepoError> {
        self.inner.list_api_keys().await
    }

    async fn delete_api_key(&self, id: payments_types::ApiKeyId) -> Result<bool, RepoError> {
        self.inner.delete_api_key(id).await
    }

    async fn register_webhook_endpoint(
        &self,
        url: &str,
        events: Vec<String>,
    ) -> Result<payments_types::WebhookEndpoint, RepoError> {
        self.inner.register_webhook_endpoint(url, events).await
    }

    async fn list_webhook_endpoints(
        &self,
    ) -> Result<Vec<payments_types::WebhookEndpoint>, RepoError> {
        self.inner.list_webhook_endpoints().await
    }

    async fn create_webhook_event(
        &self,
        endpoint_id: payments_types::WebhookEndpointId,
        event_type: &str,
        payload: serde_json::Value,
    ) -> Result<payments_types::WebhookEvent, RepoError> {
        self.inner
            .create_webhook_event(endpoint_id, event_type, payload)
            .await
    }
}
