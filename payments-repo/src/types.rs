//! Shared database types with feature-gated fields for SQLite and PostgreSQL.

use sqlx::FromRow;

use payments_types::{
    Account, AccountId, Currency, Money, RepoError, Transaction, TransactionId, TransactionType,
    WebhookEvent, WebhookStatus,
};

// ─────────────────────────────────────────────────────────────────────────────
// Feature-gated imports
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(not(feature = "sqlite"))]
use chrono::{DateTime, Utc};
#[cfg(not(feature = "sqlite"))]
use uuid::Uuid;

// ─────────────────────────────────────────────────────────────────────────────
// Database row structs (derive FromRow for automatic mapping)
// ─────────────────────────────────────────────────────────────────────────────

/// Account row from database.
#[derive(FromRow)]
pub struct DbAccount {
    #[cfg(not(feature = "sqlite"))]
    pub id: Uuid,
    #[cfg(feature = "sqlite")]
    pub id: String,

    pub name: String,
    pub balance: i64,
    pub currency: String,

    #[cfg(not(feature = "sqlite"))]
    pub created_at: DateTime<Utc>,
    #[cfg(feature = "sqlite")]
    pub created_at: String,
}

/// Transaction row from database.
#[derive(FromRow)]
pub struct DbTransaction {
    #[cfg(not(feature = "sqlite"))]
    pub id: Uuid,
    #[cfg(feature = "sqlite")]
    pub id: String,

    pub direction: String,
    pub amount: i64,
    pub currency: String,

    #[cfg(not(feature = "sqlite"))]
    pub source_account_id: Option<Uuid>,
    #[cfg(feature = "sqlite")]
    pub source_account_id: Option<String>,

    #[cfg(not(feature = "sqlite"))]
    pub destination_account_id: Option<Uuid>,
    #[cfg(feature = "sqlite")]
    pub destination_account_id: Option<String>,

    pub idempotency_key: Option<String>,
    pub reference: Option<String>,

    #[cfg(not(feature = "sqlite"))]
    pub created_at: DateTime<Utc>,
    #[cfg(feature = "sqlite")]
    pub created_at: String,
}

/// Webhook event row from database.
#[derive(FromRow)]
pub struct DbWebhookEvent {
    #[cfg(not(feature = "sqlite"))]
    pub id: Uuid,
    #[cfg(feature = "sqlite")]
    pub id: String,

    #[cfg(not(feature = "sqlite"))]
    pub endpoint_id: Uuid,
    #[cfg(feature = "sqlite")]
    pub endpoint_id: String,

    pub event_type: String,

    #[cfg(not(feature = "sqlite"))]
    pub payload: serde_json::Value,
    #[cfg(feature = "sqlite")]
    pub payload: String,

    pub status: String,

    #[cfg(not(feature = "sqlite"))]
    pub created_at: DateTime<Utc>,
    #[cfg(feature = "sqlite")]
    pub created_at: String,

    #[cfg(not(feature = "sqlite"))]
    pub processed_at: Option<DateTime<Utc>>,
    #[cfg(feature = "sqlite")]
    pub processed_at: Option<String>,

    pub attempts: i32,
    pub last_error: Option<String>,
}

impl DbWebhookEvent {
    pub fn into_domain(self) -> Result<WebhookEvent, RepoError> {
        let status = match self.status.as_str() {
            "PENDING" => WebhookStatus::Pending,
            "PROCESSING" => WebhookStatus::Processing,
            "COMPLETED" => WebhookStatus::Completed,
            "FAILED" => WebhookStatus::Failed,
            _ => WebhookStatus::Pending,
        };

        #[cfg(not(feature = "sqlite"))]
        let (id, endpoint_id, payload, created_at, processed_at) = (
            self.id,
            self.endpoint_id,
            self.payload,
            self.created_at,
            self.processed_at,
        );

        #[cfg(feature = "sqlite")]
        let (id, endpoint_id, payload, created_at, processed_at) = {
            let uuid =
                uuid::Uuid::parse_str(&self.id).map_err(|e| RepoError::Database(e.to_string()))?;

            let endpoint_uuid = uuid::Uuid::parse_str(&self.endpoint_id)
                .map_err(|e| RepoError::Database(e.to_string()))?;

            let payload: serde_json::Value = serde_json::from_str(&self.payload)
                .map_err(|e| RepoError::Database(e.to_string()))?;

            let created_at = chrono::DateTime::parse_from_rfc3339(&self.created_at)
                .map_err(|e| RepoError::Database(e.to_string()))?
                .with_timezone(&chrono::Utc);

            let processed_at = match self.processed_at {
                Some(s) => Some(
                    chrono::DateTime::parse_from_rfc3339(&s)
                        .map_err(|e| RepoError::Database(e.to_string()))?
                        .with_timezone(&chrono::Utc),
                ),
                None => None,
            };

            (uuid, endpoint_uuid, payload, created_at, processed_at)
        };

        Ok(WebhookEvent {
            id,
            endpoint_id,
            event_type: self.event_type,
            payload,
            status,
            created_at,
            processed_at,
            attempts: self.attempts,
            last_error: self.last_error,
        })
    }
}

/// Balance-only row for queries.
#[cfg(feature = "sqlite")]
#[derive(FromRow)]
pub struct DbBalance {
    pub balance: i64,
}

/// Balance and currency row for queries.
#[derive(FromRow)]
pub struct DbAccountBalance {
    pub balance: i64,
    pub currency: String,
}

/// Currency-only row for queries.
#[derive(FromRow)]
pub struct DbAccountCurrency {
    pub currency: String,
}

/// API key row from database.
#[derive(FromRow)]
pub struct DbApiKey {
    #[cfg(not(feature = "sqlite"))]
    pub id: Uuid,
    #[cfg(feature = "sqlite")]
    pub id: String,

    pub name: String,
    pub key_hash: String,

    #[cfg(not(feature = "sqlite"))]
    pub account_id: Option<Uuid>,
    #[cfg(feature = "sqlite")]
    pub account_id: Option<String>,

    #[cfg(not(feature = "sqlite"))]
    pub is_active: bool,
    #[cfg(feature = "sqlite")]
    pub is_active: i64,

    #[cfg(not(feature = "sqlite"))]
    pub created_at: DateTime<Utc>,
    #[cfg(feature = "sqlite")]
    pub created_at: String,

    #[cfg(not(feature = "sqlite"))]
    pub last_used_at: Option<DateTime<Utc>>,
    #[cfg(feature = "sqlite")]
    pub last_used_at: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Parsing helpers
// ─────────────────────────────────────────────────────────────────────────────

pub fn parse_currency(s: &str) -> Result<Currency, RepoError> {
    match s {
        "USD" => Ok(Currency::USD),
        "EUR" => Ok(Currency::EUR),
        "GBP" => Ok(Currency::GBP),
        "INR" => Ok(Currency::INR),
        _ => Err(RepoError::Database(format!("Unknown currency: {}", s))),
    }
}

pub fn parse_transaction_type(s: &str) -> Result<TransactionType, RepoError> {
    match s {
        "DEPOSIT" => Ok(TransactionType::Deposit),
        "WITHDRAWAL" => Ok(TransactionType::Withdrawal),
        "TRANSFER" => Ok(TransactionType::Transfer),
        _ => Err(RepoError::Database(format!(
            "Unknown transaction type: {}",
            s
        ))),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Domain conversion (feature-gated implementations)
// ─────────────────────────────────────────────────────────────────────────────

impl DbAccount {
    /// Convert database row to domain Account.
    pub fn into_domain(self) -> Result<Account, RepoError> {
        let currency = parse_currency(&self.currency)?;
        let money = Money::new(self.balance, currency).map_err(RepoError::Domain)?;

        #[cfg(not(feature = "sqlite"))]
        let (id, created_at) = (AccountId::from_uuid(self.id), self.created_at);

        #[cfg(feature = "sqlite")]
        let (id, created_at) = {
            let uuid =
                uuid::Uuid::parse_str(&self.id).map_err(|e| RepoError::Database(e.to_string()))?;
            let dt = chrono::DateTime::parse_from_rfc3339(&self.created_at)
                .map_err(|e| RepoError::Database(e.to_string()))?
                .with_timezone(&chrono::Utc);
            (AccountId::from_uuid(uuid), dt)
        };

        Ok(Account::from_parts(id, self.name, money, created_at))
    }
}

impl DbTransaction {
    /// Convert database row to domain Transaction.
    pub fn into_domain(self) -> Result<Transaction, RepoError> {
        let currency = parse_currency(&self.currency)?;
        let tx_type = parse_transaction_type(&self.direction)?;
        let money = Money::new(self.amount, currency).map_err(RepoError::Domain)?;

        #[cfg(not(feature = "sqlite"))]
        let (id, source_id, dest_id, created_at) = (
            TransactionId::from_uuid(self.id),
            self.source_account_id.map(AccountId::from_uuid),
            self.destination_account_id.map(AccountId::from_uuid),
            self.created_at,
        );

        #[cfg(feature = "sqlite")]
        let (id, source_id, dest_id, created_at) = {
            let uuid =
                uuid::Uuid::parse_str(&self.id).map_err(|e| RepoError::Database(e.to_string()))?;

            let source = self
                .source_account_id
                .map(|s| uuid::Uuid::parse_str(&s))
                .transpose()
                .map_err(|e| RepoError::Database(e.to_string()))?
                .map(AccountId::from_uuid);

            let dest = self
                .destination_account_id
                .map(|s| uuid::Uuid::parse_str(&s))
                .transpose()
                .map_err(|e| RepoError::Database(e.to_string()))?
                .map(AccountId::from_uuid);

            let dt = chrono::DateTime::parse_from_rfc3339(&self.created_at)
                .map_err(|e| RepoError::Database(e.to_string()))?
                .with_timezone(&chrono::Utc);

            (TransactionId::from_uuid(uuid), source, dest, dt)
        };

        Ok(Transaction::from_parts(
            id,
            tx_type,
            money,
            source_id,
            dest_id,
            self.idempotency_key,
            self.reference,
            created_at,
        ))
    }
}

impl DbApiKey {
    /// Convert database row to domain ApiKey.
    pub fn into_domain(self) -> Result<payments_types::ApiKey, RepoError> {
        #[cfg(not(feature = "sqlite"))]
        let (id, account_id, is_active, created_at, last_used_at) = (
            payments_types::ApiKeyId::from_uuid(self.id),
            self.account_id.map(payments_types::AccountId::from_uuid),
            self.is_active,
            self.created_at,
            self.last_used_at,
        );

        #[cfg(feature = "sqlite")]
        let (id, account_id, is_active, created_at, last_used_at) = {
            let uuid =
                uuid::Uuid::parse_str(&self.id).map_err(|e| RepoError::Database(e.to_string()))?;

            let account_id = self
                .account_id
                .map(|s| uuid::Uuid::parse_str(&s))
                .transpose()
                .map_err(|e| RepoError::Database(e.to_string()))?
                .map(payments_types::AccountId::from_uuid);

            let is_active = self.is_active != 0;

            let created_at = chrono::DateTime::parse_from_rfc3339(&self.created_at)
                .map_err(|e| RepoError::Database(e.to_string()))?
                .with_timezone(&chrono::Utc);

            let last_used_at = self
                .last_used_at
                .map(|s| chrono::DateTime::parse_from_rfc3339(&s))
                .transpose()
                .map_err(|e| RepoError::Database(e.to_string()))?
                .map(|dt| dt.with_timezone(&chrono::Utc));

            (
                payments_types::ApiKeyId::from_uuid(uuid),
                account_id,
                is_active,
                created_at,
                last_used_at,
            )
        };

        Ok(payments_types::ApiKey {
            id,
            name: self.name,
            key_hash: self.key_hash,
            account_id,
            is_active,
            created_at,
            last_used_at,
        })
    }
}
