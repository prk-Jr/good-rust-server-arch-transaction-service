//! SQLite repository adapter.
#![allow(clippy::collapsible_if)]

use async_trait::async_trait;
use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;
use std::str::FromStr;
use uuid::Uuid;

use payments_types::{
    Account, AccountId, CreateAccountRequest, DepositRequest, DomainError, Money, RepoError,
    Transaction, TransactionRepository, TransferRequest, WebhookEvent, WebhookStatus,
    WithdrawRequest,
};

use crate::types::{DbAccount, DbAccountBalance, DbAccountCurrency, DbBalance, DbTransaction};

// ─────────────────────────────────────────────────────────────────────────────
// SQLite Repository
// ─────────────────────────────────────────────────────────────────────────────

/// SQLite repository implementation.
pub struct SqliteRepo {
    pool: SqlitePool,
}

impl SqliteRepo {
    /// Creates a new SQLite repository with automatic migration.
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        // Ensure on-disk SQLite target directory exists (no-op for in-memory).
        if let Some(path) = database_url.strip_prefix("sqlite://") {
            // Remove query parameters
            let path = path.split('?').next().unwrap_or(path);
            if path != ":memory:" {
                let p = std::path::Path::new(path);
                if let Some(parent) = p.parent() {
                    if !parent.as_os_str().is_empty() {
                        tokio::fs::create_dir_all(parent).await?;
                    }
                }
            }
        }

        let options = SqliteConnectOptions::from_str(database_url)?.create_if_missing(true);
        let pool = SqlitePool::connect_with(options).await?;

        // Run migration from migration file
        let ddl = include_str!("../migrations/0001_create_tables.sql");
        sqlx::query(ddl).execute(&pool).await?;

        let ddl_webhooks = include_str!("../migrations/0002_create_webhook_events.sql");
        sqlx::query(ddl_webhooks).execute(&pool).await?;

        let ddl_api_keys = include_str!("../migrations/0003_create_api_keys.sql");
        sqlx::query(ddl_api_keys).execute(&pool).await?;

        let ddl_webhook_endpoints =
            include_str!("../migrations/0004_create_webhook_endpoints_sqlite.sql");
        sqlx::query(ddl_webhook_endpoints).execute(&pool).await?;

        Ok(Self { pool })
    }

    /// Returns a reference to the connection pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Creates the database schema (for testing with existing pool).
    pub async fn create_schema(&self) -> Result<(), RepoError> {
        let ddl = include_str!("../migrations/0001_create_tables.sql");
        sqlx::query(ddl)
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

        let ddl_webhooks = include_str!("../migrations/0002_create_webhook_events.sql");
        sqlx::query(ddl_webhooks)
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

        let ddl_api_keys = include_str!("../migrations/0003_create_api_keys.sql");
        sqlx::query(ddl_api_keys)
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Repository implementation
// ─────────────────────────────────────────────────────────────────────────────

#[async_trait]
impl TransactionRepository for SqliteRepo {
    async fn create_account(&self, req: CreateAccountRequest) -> Result<Account, RepoError> {
        // Validate first
        let _ = Account::new(req.name.clone(), req.currency).map_err(RepoError::Domain)?;

        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let id_str = id.to_string();
        let currency_str = req.currency.to_string();
        let created_at_str = now.to_rfc3339();

        sqlx::query(
            r#"INSERT INTO accounts (id, name, balance, currency, created_at) VALUES (?, ?, 0, ?, ?)"#,
        )
        .bind(&id_str)
        .bind(&req.name)
        .bind(&currency_str)
        .bind(&created_at_str)
        .execute(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(Account::from_parts(
            AccountId::from_uuid(id),
            req.name,
            Money::zero(req.currency),
            now,
        ))
    }

    async fn get_account(&self, id: AccountId) -> Result<Option<Account>, RepoError> {
        let id_str = id.to_string();

        let row: Option<DbAccount> = sqlx::query_as(
            r#"SELECT id, name, balance, currency, created_at FROM accounts WHERE id = ?"#,
        )
        .bind(&id_str)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        row.map(DbAccount::into_domain).transpose()
    }

    async fn list_accounts(&self) -> Result<Vec<Account>, RepoError> {
        let rows: Vec<DbAccount> = sqlx::query_as(
            r#"SELECT id, name, balance, currency, created_at FROM accounts ORDER BY created_at DESC"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        rows.into_iter().map(DbAccount::into_domain).collect()
    }

    async fn deposit(&self, req: DepositRequest) -> Result<Transaction, RepoError> {
        // Check idempotency
        if let Some(key) = &req.idempotency_key {
            if let Some(tx) = self.find_by_idempotency_key(key).await? {
                return Ok(tx);
            }
        }

        let money = Money::new(req.amount, req.currency).map_err(RepoError::Domain)?;
        let account_id_str = req.account_id.to_string();

        let mut db_tx = self
            .pool
            .begin()
            .await
            .map_err(|e| RepoError::Transaction(e.to_string()))?;

        let result = sqlx::query(r#"UPDATE accounts SET balance = balance + ? WHERE id = ?"#)
            .bind(money.amount())
            .bind(&account_id_str)
            .execute(&mut *db_tx)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(RepoError::NotFound);
        }

        let tx_id = Uuid::new_v4();
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            r#"INSERT INTO transactions (id, direction, amount, currency, destination_account_id, idempotency_key, reference, created_at)
               VALUES (?, 'DEPOSIT', ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(tx_id.to_string())
        .bind(money.amount())
        .bind(money.currency().to_string())
        .bind(&account_id_str)
        .bind(&req.idempotency_key)
        .bind(&req.reference)
        .bind(&now)
        .execute(&mut *db_tx)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        db_tx
            .commit()
            .await
            .map_err(|e| RepoError::Transaction(e.to_string()))?;

        Ok(Transaction::deposit(
            req.account_id,
            money,
            req.idempotency_key,
            req.reference,
        ))
    }

    async fn withdraw(&self, req: WithdrawRequest) -> Result<Transaction, RepoError> {
        if let Some(key) = &req.idempotency_key {
            if let Some(tx) = self.find_by_idempotency_key(key).await? {
                return Ok(tx);
            }
        }

        let money = Money::new(req.amount, req.currency).map_err(RepoError::Domain)?;
        let account_id_str = req.account_id.to_string();

        let mut db_tx = self
            .pool
            .begin()
            .await
            .map_err(|e| RepoError::Transaction(e.to_string()))?;

        let row: Option<DbBalance> = sqlx::query_as(r#"SELECT balance FROM accounts WHERE id = ?"#)
            .bind(&account_id_str)
            .fetch_optional(&mut *db_tx)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

        let account = row.ok_or(RepoError::NotFound)?;

        if account.balance < money.amount() {
            return Err(RepoError::Domain(DomainError::InsufficientFunds {
                available: account.balance,
                requested: money.amount(),
            }));
        }

        sqlx::query(r#"UPDATE accounts SET balance = balance - ? WHERE id = ?"#)
            .bind(money.amount())
            .bind(&account_id_str)
            .execute(&mut *db_tx)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

        let tx_id = Uuid::new_v4();
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            r#"INSERT INTO transactions (id, direction, amount, currency, source_account_id, idempotency_key, reference, created_at)
               VALUES (?, 'WITHDRAWAL', ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(tx_id.to_string())
        .bind(money.amount())
        .bind(money.currency().to_string())
        .bind(&account_id_str)
        .bind(&req.idempotency_key)
        .bind(&req.reference)
        .bind(&now)
        .execute(&mut *db_tx)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        db_tx
            .commit()
            .await
            .map_err(|e| RepoError::Transaction(e.to_string()))?;

        Ok(Transaction::withdrawal(
            req.account_id,
            money,
            req.idempotency_key,
            req.reference,
        ))
    }

    async fn transfer(&self, req: TransferRequest) -> Result<Transaction, RepoError> {
        if let Some(key) = &req.idempotency_key {
            if let Some(tx) = self.find_by_idempotency_key(key).await? {
                return Ok(tx);
            }
        }

        let money = Money::new(req.amount, req.currency).map_err(RepoError::Domain)?;
        let from_id_str = req.from_account_id.to_string();
        let to_id_str = req.to_account_id.to_string();

        let mut db_tx = self
            .pool
            .begin()
            .await
            .map_err(|e| RepoError::Transaction(e.to_string()))?;

        // Check source
        let source: Option<DbAccountBalance> =
            sqlx::query_as(r#"SELECT balance, currency FROM accounts WHERE id = ?"#)
                .bind(&from_id_str)
                .fetch_optional(&mut *db_tx)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;

        let source = source.ok_or(RepoError::NotFound)?;

        if source.balance < money.amount() {
            return Err(RepoError::Domain(DomainError::InsufficientFunds {
                available: source.balance,
                requested: money.amount(),
            }));
        }

        // Check destination
        let dest: Option<DbAccountCurrency> =
            sqlx::query_as(r#"SELECT currency FROM accounts WHERE id = ?"#)
                .bind(&to_id_str)
                .fetch_optional(&mut *db_tx)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;

        let dest = dest.ok_or(RepoError::NotFound)?;

        if source.currency != dest.currency {
            return Err(RepoError::Domain(DomainError::CrossCurrencyTransfer));
        }

        // Debit source
        sqlx::query(r#"UPDATE accounts SET balance = balance - ? WHERE id = ?"#)
            .bind(money.amount())
            .bind(&from_id_str)
            .execute(&mut *db_tx)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

        // Credit destination
        sqlx::query(r#"UPDATE accounts SET balance = balance + ? WHERE id = ?"#)
            .bind(money.amount())
            .bind(&to_id_str)
            .execute(&mut *db_tx)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

        let tx_id = Uuid::new_v4();
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            r#"INSERT INTO transactions (id, direction, amount, currency, source_account_id, destination_account_id, idempotency_key, reference, created_at)
               VALUES (?, 'TRANSFER', ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(tx_id.to_string())
        .bind(money.amount())
        .bind(money.currency().to_string())
        .bind(&from_id_str)
        .bind(&to_id_str)
        .bind(&req.idempotency_key)
        .bind(&req.reference)
        .bind(&now)
        .execute(&mut *db_tx)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        db_tx
            .commit()
            .await
            .map_err(|e| RepoError::Transaction(e.to_string()))?;

        Ok(Transaction::transfer(
            req.from_account_id,
            req.to_account_id,
            money,
            req.idempotency_key,
            req.reference,
        ))
    }

    async fn find_by_idempotency_key(&self, key: &str) -> Result<Option<Transaction>, RepoError> {
        let row: Option<DbTransaction> = sqlx::query_as(
            r#"SELECT id, direction, amount, currency, source_account_id, destination_account_id, idempotency_key, reference, created_at
               FROM transactions WHERE idempotency_key = ?"#,
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        row.map(DbTransaction::into_domain).transpose()
    }

    async fn get_transaction(
        &self,
        id: payments_types::TransactionId,
    ) -> Result<Option<Transaction>, RepoError> {
        let id_str = id.to_string();

        let row: Option<DbTransaction> = sqlx::query_as(
            r#"SELECT id, direction, amount, currency, source_account_id, destination_account_id, idempotency_key, reference, created_at
               FROM transactions WHERE id = ?"#,
        )
        .bind(&id_str)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        row.map(DbTransaction::into_domain).transpose()
    }

    async fn list_transactions_for_account(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<Transaction>, RepoError> {
        let account_id_str = account_id.to_string();

        let rows: Vec<DbTransaction> = sqlx::query_as(
            r#"SELECT id, direction, amount, currency, source_account_id, destination_account_id, idempotency_key, reference, created_at
               FROM transactions WHERE source_account_id = ? OR destination_account_id = ?
               ORDER BY created_at DESC"#,
        )
        .bind(&account_id_str)
        .bind(&account_id_str)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        rows.into_iter().map(DbTransaction::into_domain).collect()
    }

    async fn verify_api_key_hash(
        &self,
        key_hash: &str,
    ) -> Result<Option<payments_types::ApiKey>, RepoError> {
        let row: Option<crate::types::DbApiKey> = sqlx::query_as(
            r#"
            SELECT id, name, key_hash, account_id, is_active, created_at, last_used_at
            FROM api_keys
            WHERE key_hash = ? AND is_active = 1
            "#,
        )
        .bind(key_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        row.map(|r| r.into_domain()).transpose()
    }

    async fn create_api_key(
        &self,
        name: &str,
    ) -> Result<(payments_types::ApiKey, String), RepoError> {
        use rand::Rng;
        use rand::distr::Alphanumeric;

        // Generate a secure random API key
        let raw_key: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();
        let prefixed_key = format!("sk_{}", raw_key);

        let key_hash = crate::security::hash_api_key(&prefixed_key);
        let id = uuid::Uuid::new_v4();
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO api_keys (id, name, key_hash, is_active, created_at)
            VALUES (?, ?, ?, 1, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(name)
        .bind(&key_hash)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        let created_at = chrono::DateTime::parse_from_rfc3339(&now)
            .map_err(|e| RepoError::Database(e.to_string()))?
            .with_timezone(&chrono::Utc);

        let api_key = payments_types::ApiKey {
            id: payments_types::ApiKeyId::from_uuid(id),
            name: name.to_string(),
            key_hash,
            account_id: None,
            is_active: true,
            created_at,
            last_used_at: None,
        };

        Ok((api_key, prefixed_key))
    }

    async fn count_api_keys(&self) -> Result<i64, RepoError> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM api_keys WHERE is_active = 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(row.0)
    }

    async fn list_api_keys(&self) -> Result<Vec<payments_types::ApiKey>, RepoError> {
        #[derive(sqlx::FromRow)]
        struct DbApiKey {
            id: String,
            name: String,
            key_hash: String,
            account_id: Option<String>,
            is_active: bool,
            created_at: String,
            last_used_at: Option<String>,
        }

        let rows: Vec<DbApiKey> = sqlx::query_as(
            "SELECT id, name, key_hash, account_id, is_active, created_at, last_used_at FROM api_keys WHERE is_active = 1 ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        rows.into_iter()
            .map(|row| {
                let id = uuid::Uuid::parse_str(&row.id)
                    .map_err(|e| RepoError::Database(e.to_string()))?;
                let created_at = chrono::DateTime::parse_from_rfc3339(&row.created_at)
                    .map_err(|e| RepoError::Database(e.to_string()))?
                    .with_timezone(&chrono::Utc);
                let last_used_at = row
                    .last_used_at
                    .map(|s| {
                        chrono::DateTime::parse_from_rfc3339(&s)
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                    })
                    .transpose()
                    .map_err(|e| RepoError::Database(e.to_string()))?;
                let account_id = row
                    .account_id
                    .map(|s| uuid::Uuid::parse_str(&s).map(payments_types::AccountId::from_uuid))
                    .transpose()
                    .map_err(|e| RepoError::Database(e.to_string()))?;

                Ok(payments_types::ApiKey {
                    id: payments_types::ApiKeyId::from_uuid(id),
                    name: row.name,
                    key_hash: row.key_hash,
                    account_id,
                    is_active: row.is_active,
                    created_at,
                    last_used_at,
                })
            })
            .collect()
    }

    async fn delete_api_key(&self, id: payments_types::ApiKeyId) -> Result<bool, RepoError> {
        let result =
            sqlx::query("UPDATE api_keys SET is_active = 0 WHERE id = ? AND is_active = 1")
                .bind(id.to_string())
                .execute(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn register_webhook_endpoint(
        &self,
        url: &str,
        events: Vec<String>,
    ) -> Result<payments_types::WebhookEndpoint, RepoError> {
        use rand::Rng;
        use rand::distr::Alphanumeric;

        let id = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        // Generate a random secret for HMAC signing
        let secret: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();
        let secret = format!("whsec_{}", secret);

        let events_json =
            serde_json::to_string(&events).map_err(|e| RepoError::Database(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO webhook_endpoints (id, url, secret, events, is_active, created_at)
            VALUES (?, ?, ?, ?, 1, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(url)
        .bind(&secret)
        .bind(&events_json)
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(payments_types::WebhookEndpoint {
            id,
            url: url.to_string(),
            secret,
            events,
            is_active: true,
            created_at: now,
        })
    }

    async fn list_webhook_endpoints(
        &self,
    ) -> Result<Vec<payments_types::WebhookEndpoint>, RepoError> {
        let rows: Vec<(String, String, String, String, i32, String)> = sqlx::query_as(
            r#"
            SELECT id, url, secret, events, is_active, created_at
            FROM webhook_endpoints
            WHERE is_active = 1
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        rows.into_iter()
            .map(|(id, url, secret, events, is_active, created_at)| {
                let id =
                    uuid::Uuid::parse_str(&id).map_err(|e| RepoError::Database(e.to_string()))?;
                let events: Vec<String> = serde_json::from_str(&events).unwrap_or_default();
                let created_at = chrono::DateTime::parse_from_rfc3339(&created_at)
                    .map_err(|e| RepoError::Database(e.to_string()))?
                    .with_timezone(&chrono::Utc);
                Ok(payments_types::WebhookEndpoint {
                    id,
                    url,
                    secret,
                    events,
                    is_active: is_active == 1,
                    created_at,
                })
            })
            .collect()
    }

    async fn create_webhook_event(
        &self,
        endpoint_id: payments_types::WebhookEndpointId,
        event_type: &str,
        payload: serde_json::Value,
    ) -> Result<payments_types::WebhookEvent, RepoError> {
        let event_id = uuid::Uuid::new_v4();
        let now_dt = chrono::Utc::now();
        let now = now_dt.to_rfc3339();
        let payload_json =
            serde_json::to_string(&payload).map_err(|e| RepoError::Database(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO webhook_events (id, endpoint_id, event_type, payload, status, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(event_id.to_string())
        .bind(endpoint_id.0.to_string())
        .bind(event_type)
        .bind(payload_json)
        .bind("PENDING")
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(payments_types::WebhookEvent {
            id: event_id,
            endpoint_id: endpoint_id.0,
            event_type: event_type.to_string(),
            payload: serde_json::Value::Null,
            status: payments_types::WebhookStatus::Pending,
            created_at: now_dt,
            processed_at: None,
            attempts: 0,
            last_error: None,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Webhook Extension (Internal)
// ─────────────────────────────────────────────────────────────────────────────
// ─────────────────────────────────────────────────────────────────────────────
// Webhook Extension (Internal)
// ─────────────────────────────────────────────────────────────────────────────
impl SqliteRepo {
    pub async fn get_pending_webhooks(&self, limit: i64) -> Result<Vec<WebhookEvent>, RepoError> {
        let rows = sqlx::query_as::<_, crate::types::DbWebhookEvent>(
            r#"
            SELECT id, endpoint_id, event_type, payload, status, created_at, processed_at, attempts, last_error
            FROM webhook_events
            WHERE status = 'PENDING'
            ORDER BY created_at ASC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        rows.into_iter().map(|row| row.into_domain()).collect()
    }

    pub async fn update_webhook_status(
        &self,
        id: Uuid,
        status: WebhookStatus,
        last_error: Option<String>,
    ) -> Result<(), RepoError> {
        let now = chrono::Utc::now().to_rfc3339();
        let status_str = status.to_string();
        let id_str = id.to_string();

        sqlx::query(
            r#"
            UPDATE webhook_events
            SET status = ?, processed_at = ?, last_error = ?, attempts = attempts + 1
            WHERE id = ?
            "#,
        )
        .bind(status_str)
        .bind(now)
        .bind(last_error)
        .bind(id_str)
        .execute(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(())
    }
}
