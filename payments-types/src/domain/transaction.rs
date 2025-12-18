//! Transaction domain model.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::account::AccountId;
use super::money::Money;

/// Unique identifier for a Transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TransactionId(Uuid);

impl TransactionId {
    /// Creates a new random TransactionId.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates a TransactionId from an existing UUID.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the underlying UUID.
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Returns the UUID value.
    pub fn into_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for TransactionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TransactionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for TransactionId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

/// The type/direction of a transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransactionType {
    /// Money coming into an account from external source
    Deposit,
    /// Money leaving an account to external destination
    Withdrawal,
    /// Money moving between two accounts in the system
    Transfer,
}

impl std::fmt::Display for TransactionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransactionType::Deposit => write!(f, "DEPOSIT"),
            TransactionType::Withdrawal => write!(f, "WITHDRAWAL"),
            TransactionType::Transfer => write!(f, "TRANSFER"),
        }
    }
}

/// A recorded financial transaction.
///
/// Transactions are immutable once created - they represent
/// a historical record of what happened.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Unique identifier
    pub id: TransactionId,
    /// Type of transaction
    pub transaction_type: TransactionType,
    /// Amount transferred
    pub amount: Money,
    /// Source account (None for deposits from external)
    pub source_account_id: Option<AccountId>,
    /// Destination account (None for withdrawals to external)
    pub destination_account_id: Option<AccountId>,
    /// Idempotency key for duplicate detection
    pub idempotency_key: Option<String>,
    /// External reference (e.g., invoice number)
    pub reference: Option<String>,
    /// When the transaction was created
    pub created_at: DateTime<Utc>,
}

impl Transaction {
    /// Creates a new deposit transaction.
    pub fn deposit(
        destination: AccountId,
        amount: Money,
        idempotency_key: Option<String>,
        reference: Option<String>,
    ) -> Self {
        Self {
            id: TransactionId::new(),
            transaction_type: TransactionType::Deposit,
            amount,
            source_account_id: None,
            destination_account_id: Some(destination),
            idempotency_key,
            reference,
            created_at: Utc::now(),
        }
    }

    /// Creates a new withdrawal transaction.
    pub fn withdrawal(
        source: AccountId,
        amount: Money,
        idempotency_key: Option<String>,
        reference: Option<String>,
    ) -> Self {
        Self {
            id: TransactionId::new(),
            transaction_type: TransactionType::Withdrawal,
            amount,
            source_account_id: Some(source),
            destination_account_id: None,
            idempotency_key,
            reference,
            created_at: Utc::now(),
        }
    }

    /// Creates a new transfer transaction.
    pub fn transfer(
        source: AccountId,
        destination: AccountId,
        amount: Money,
        idempotency_key: Option<String>,
        reference: Option<String>,
    ) -> Self {
        Self {
            id: TransactionId::new(),
            transaction_type: TransactionType::Transfer,
            amount,
            source_account_id: Some(source),
            destination_account_id: Some(destination),
            idempotency_key,
            reference,
            created_at: Utc::now(),
        }
    }

    /// Reconstructs a transaction from database fields.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: TransactionId,
        transaction_type: TransactionType,
        amount: Money,
        source_account_id: Option<AccountId>,
        destination_account_id: Option<AccountId>,
        idempotency_key: Option<String>,
        reference: Option<String>,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            transaction_type,
            amount,
            source_account_id,
            destination_account_id,
            idempotency_key,
            reference,
            created_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Currency;

    #[test]
    fn test_deposit_creation() {
        let account = AccountId::new();
        let amount = Money::new(1000, Currency::USD).unwrap();
        let tx = Transaction::deposit(account, amount, None, None);

        assert_eq!(tx.transaction_type, TransactionType::Deposit);
        assert!(tx.source_account_id.is_none());
        assert_eq!(tx.destination_account_id, Some(account));
    }

    #[test]
    fn test_transfer_creation() {
        let alice = AccountId::new();
        let bob = AccountId::new();
        let amount = Money::new(500, Currency::USD).unwrap();
        let tx = Transaction::transfer(alice, bob, amount, Some("key123".to_string()), None);

        assert_eq!(tx.transaction_type, TransactionType::Transfer);
        assert_eq!(tx.source_account_id, Some(alice));
        assert_eq!(tx.destination_account_id, Some(bob));
        assert_eq!(tx.idempotency_key, Some("key123".to_string()));
    }
}
