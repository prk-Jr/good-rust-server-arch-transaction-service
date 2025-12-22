//! PostgreSQL repository adapter.
#![allow(clippy::collapsible_if)]

use async_trait::async_trait;
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use payments_types::{
    Account, AccountId, CreateAccountRequest, DepositRequest, DomainError, Money, RepoError,
    Transaction, TransactionId, TransactionRepository, TransferRequest, WebhookEvent,
    WebhookStatus, WithdrawRequest,
};

use crate::types::{DbAccount, DbAccountBalance, DbAccountCurrency, DbTransaction};

// ─────────────────────────────────────────────────────────────────────────────
// PostgreSQL Repository
// ─────────────────────────────────────────────────────────────────────────────

/// PostgreSQL repository with row-level locking.
pub struct PostgresRepo {
    pool: PgPool,
}

/// Executes SQL statements from a migration file, splitting by semicolons.
async fn execute_migration(pool: &PgPool, sql: &str, name: &str) -> Result<(), anyhow::Error> {
    for statement in sql.split(';') {
        let stmt = statement.trim();
        if !stmt.is_empty() {
            sqlx::query(stmt)
                .execute(pool)
                .await
                .map_err(|e| anyhow::anyhow!("Migration {} failed: {}", name, e))?;
        }
    }
    Ok(())
}

/// Runs all database migrations.
async fn run_migrations(pool: &PgPool) -> Result<(), anyhow::Error> {
    execute_migration(
        pool,
        include_str!("../migrations/0001_create_tables_pg.sql"),
        "0001",
    )
    .await?;

    execute_migration(
        pool,
        include_str!("../migrations/0002_create_webhook_events_pg.sql"),
        "0002",
    )
    .await?;

    execute_migration(
        pool,
        include_str!("../migrations/0003_create_api_keys_pg.sql"),
        "0003",
    )
    .await?;

    execute_migration(
        pool,
        include_str!("../migrations/0004_create_webhook_endpoints_pg.sql"),
        "0004",
    )
    .await?;

    Ok(())
}

impl PostgresRepo {
    /// Creates a new PostgreSQL repository with automatic migration.
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        let pool = PgPool::connect(database_url).await?;
        run_migrations(&pool).await?;
        Ok(Self { pool })
    }

    /// Returns a reference to the connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Creates the database schema (for testing with existing pool).
    pub async fn create_schema(&self) -> Result<(), RepoError> {
        run_migrations(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Repository implementation
// ─────────────────────────────────────────────────────────────────────────────

#[async_trait]
impl TransactionRepository for PostgresRepo {
    async fn create_account(&self, req: CreateAccountRequest) -> Result<Account, RepoError> {
        // Validate first
        let _ = Account::new(req.name.clone(), req.currency).map_err(RepoError::Domain)?;

        let id = Uuid::new_v4();
        let currency_str = req.currency.to_string();
        let now = Utc::now();

        sqlx::query(
            r#"INSERT INTO accounts (id, name, balance, currency, created_at) VALUES ($1, $2, 0, $3, $4)"#,
        )
        .bind(id)
        .bind(&req.name)
        .bind(&currency_str)
        .bind(now)
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
        let row: Option<DbAccount> = sqlx::query_as(
            r#"SELECT id, name, balance, currency, created_at FROM accounts WHERE id = $1"#,
        )
        .bind(id.into_uuid())
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
        if let Some(key) = &req.idempotency_key {
            if let Some(tx) = self.find_by_idempotency_key(key).await? {
                return Ok(tx);
            }
        }

        let money = Money::new(req.amount, req.currency).map_err(RepoError::Domain)?;

        let mut db_tx = self
            .pool
            .begin()
            .await
            .map_err(|e| RepoError::Transaction(e.to_string()))?;

        let result = sqlx::query(
            r#"UPDATE accounts SET balance = balance + $1 WHERE id = $2 RETURNING balance"#,
        )
        .bind(money.amount())
        .bind(req.account_id.into_uuid())
        .fetch_optional(&mut *db_tx)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        if result.is_none() {
            return Err(RepoError::NotFound);
        }

        let tx_id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            r#"INSERT INTO transactions (id, direction, amount, currency, destination_account_id, idempotency_key, reference, created_at)
               VALUES ($1, 'DEPOSIT', $2, $3, $4, $5, $6, $7)"#,
        )
        .bind(tx_id)
        .bind(money.amount())
        .bind(money.currency().to_string())
        .bind(req.account_id.into_uuid())
        .bind(&req.idempotency_key)
        .bind(&req.reference)
        .bind(now)
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

        let mut db_tx = self
            .pool
            .begin()
            .await
            .map_err(|e| RepoError::Transaction(e.to_string()))?;

        // Lock the account with FOR UPDATE
        let row: Option<DbAccountBalance> =
            sqlx::query_as(r#"SELECT balance, currency FROM accounts WHERE id = $1 FOR UPDATE"#)
                .bind(req.account_id.into_uuid())
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

        sqlx::query(r#"UPDATE accounts SET balance = balance - $1 WHERE id = $2"#)
            .bind(money.amount())
            .bind(req.account_id.into_uuid())
            .execute(&mut *db_tx)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

        let tx_id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            r#"INSERT INTO transactions (id, direction, amount, currency, source_account_id, idempotency_key, reference, created_at)
               VALUES ($1, 'WITHDRAWAL', $2, $3, $4, $5, $6, $7)"#,
        )
        .bind(tx_id)
        .bind(money.amount())
        .bind(money.currency().to_string())
        .bind(req.account_id.into_uuid())
        .bind(&req.idempotency_key)
        .bind(&req.reference)
        .bind(now)
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

        let mut db_tx = self
            .pool
            .begin()
            .await
            .map_err(|e| RepoError::Transaction(e.to_string()))?;

        // Lock accounts in consistent order to prevent deadlocks
        let (first_id, second_id) = if req.from_account_id.as_uuid() < req.to_account_id.as_uuid() {
            (req.from_account_id, req.to_account_id)
        } else {
            (req.to_account_id, req.from_account_id)
        };

        // Lock first account
        let first: Option<DbAccountBalance> =
            sqlx::query_as(r#"SELECT balance, currency FROM accounts WHERE id = $1 FOR UPDATE"#)
                .bind(first_id.into_uuid())
                .fetch_optional(&mut *db_tx)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;

        if first.is_none() {
            return Err(RepoError::NotFound);
        }

        // Lock second account
        let second: Option<DbAccountBalance> =
            sqlx::query_as(r#"SELECT balance, currency FROM accounts WHERE id = $1 FOR UPDATE"#)
                .bind(second_id.into_uuid())
                .fetch_optional(&mut *db_tx)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;

        if second.is_none() {
            return Err(RepoError::NotFound);
        }

        // Get source balance and currency
        let source: DbAccountBalance =
            sqlx::query_as(r#"SELECT balance, currency FROM accounts WHERE id = $1"#)
                .bind(req.from_account_id.into_uuid())
                .fetch_one(&mut *db_tx)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;

        if source.balance < money.amount() {
            return Err(RepoError::Domain(DomainError::InsufficientFunds {
                available: source.balance,
                requested: money.amount(),
            }));
        }

        // Get destination currency
        let dest: DbAccountCurrency =
            sqlx::query_as(r#"SELECT currency FROM accounts WHERE id = $1"#)
                .bind(req.to_account_id.into_uuid())
                .fetch_one(&mut *db_tx)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;

        if source.currency != dest.currency {
            return Err(RepoError::Domain(DomainError::CrossCurrencyTransfer));
        }

        // Debit source
        sqlx::query(r#"UPDATE accounts SET balance = balance - $1 WHERE id = $2"#)
            .bind(money.amount())
            .bind(req.from_account_id.into_uuid())
            .execute(&mut *db_tx)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

        // Credit destination
        sqlx::query(r#"UPDATE accounts SET balance = balance + $1 WHERE id = $2"#)
            .bind(money.amount())
            .bind(req.to_account_id.into_uuid())
            .execute(&mut *db_tx)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

        let tx_id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            r#"INSERT INTO transactions (id, direction, amount, currency, source_account_id, destination_account_id, idempotency_key, reference, created_at)
               VALUES ($1, 'TRANSFER', $2, $3, $4, $5, $6, $7, $8)"#,
        )
        .bind(tx_id)
        .bind(money.amount())
        .bind(money.currency().to_string())
        .bind(req.from_account_id.into_uuid())
        .bind(req.to_account_id.into_uuid())
        .bind(&req.idempotency_key)
        .bind(&req.reference)
        .bind(now)
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
               FROM transactions WHERE idempotency_key = $1"#,
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        row.map(DbTransaction::into_domain).transpose()
    }

    async fn get_transaction(&self, id: TransactionId) -> Result<Option<Transaction>, RepoError> {
        let row: Option<DbTransaction> = sqlx::query_as(
            r#"SELECT id, direction, amount, currency, source_account_id, destination_account_id, idempotency_key, reference, created_at
               FROM transactions WHERE id = $1"#,
        )
        .bind(id.into_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        row.map(DbTransaction::into_domain).transpose()
    }

    async fn list_transactions_for_account(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<Transaction>, RepoError> {
        let rows: Vec<DbTransaction> = sqlx::query_as(
            r#"SELECT id, direction, amount, currency, source_account_id, destination_account_id, idempotency_key, reference, created_at
               FROM transactions WHERE source_account_id = $1 OR destination_account_id = $1
               ORDER BY created_at DESC"#,
        )
        .bind(account_id.into_uuid())
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
            WHERE key_hash = $1 AND is_active = TRUE
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
        let id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO api_keys (id, name, key_hash, is_active, created_at)
            VALUES ($1, $2, $3, TRUE, $4)
            "#,
        )
        .bind(id)
        .bind(name)
        .bind(&key_hash)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        let api_key = payments_types::ApiKey {
            id: payments_types::ApiKeyId::from_uuid(id),
            name: name.to_string(),
            key_hash,
            account_id: None,
            is_active: true,
            created_at: now,
            last_used_at: None,
        };

        Ok((api_key, prefixed_key))
    }

    async fn count_api_keys(&self) -> Result<i64, RepoError> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM api_keys WHERE is_active = TRUE")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(row.0)
    }

    async fn register_webhook_endpoint(
        &self,
        url: &str,
        events: Vec<String>,
    ) -> Result<payments_types::WebhookEndpoint, RepoError> {
        use rand::Rng;
        use rand::distr::Alphanumeric;

        let id = Uuid::new_v4();
        let now = Utc::now();

        // Generate a random secret for HMAC signing
        let secret: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();
        let secret = format!("whsec_{}", secret);

        let events_json =
            serde_json::to_value(&events).map_err(|e| RepoError::Database(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO webhook_endpoints (id, url, secret, events, is_active, created_at)
            VALUES ($1, $2, $3, $4, TRUE, $5)
            "#,
        )
        .bind(id)
        .bind(url)
        .bind(&secret)
        .bind(&events_json)
        .bind(now)
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
        let rows: Vec<(
            Uuid,
            String,
            String,
            serde_json::Value,
            bool,
            chrono::DateTime<Utc>,
        )> = sqlx::query_as(
            r#"
            SELECT id, url, secret, events, is_active, created_at
            FROM webhook_endpoints
            WHERE is_active = TRUE
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        rows.into_iter()
            .map(|(id, url, secret, events, is_active, created_at)| {
                let events: Vec<String> = serde_json::from_value(events).unwrap_or_default();
                Ok(payments_types::WebhookEndpoint {
                    id,
                    url,
                    secret,
                    events,
                    is_active,
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
        let event_id = Uuid::new_v4();
        let now = Utc::now();
        let payload_json =
            serde_json::to_value(payload).map_err(|e| RepoError::Database(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO webhook_events (id, endpoint_id, event_type, payload, status, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(event_id)
        .bind(endpoint_id.0)
        .bind(event_type)
        .bind(payload_json)
        .bind("PENDING")
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(payments_types::WebhookEvent {
            id: event_id,
            endpoint_id: endpoint_id.0,
            event_type: event_type.to_string(),
            payload: serde_json::Value::Null,
            status: payments_types::WebhookStatus::Pending,
            created_at: now,
            processed_at: None,
            attempts: 0,
            last_error: None,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Webhook Extension (Internal)
// ─────────────────────────────────────────────────────────────────────────────
impl PostgresRepo {
    pub async fn get_pending_webhooks(&self, limit: i64) -> Result<Vec<WebhookEvent>, RepoError> {
        // We use SKIP LOCKED to allow multiple workers (Postgres feature)
        let rows = sqlx::query_as::<_, crate::types::DbWebhookEvent>(
            r#"
            SELECT id, endpoint_id, event_type, payload, status, created_at, processed_at, attempts, last_error
            FROM webhook_events
            WHERE status = 'PENDING'
            ORDER BY created_at ASC
            LIMIT $1
            FOR UPDATE SKIP LOCKED
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
        let now = Utc::now();
        let status_str = status.to_string();

        sqlx::query(
            r#"
            UPDATE webhook_events
            SET status = $1, processed_at = $2, last_error = $3, attempts = attempts + 1
            WHERE id = $4
            "#,
        )
        .bind(status_str)
        .bind(now)
        .bind(last_error)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(())
    }
}
