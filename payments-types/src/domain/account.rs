//! Account domain model.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::money::{Currency, Money};
use crate::error::DomainError;

/// Unique identifier for an Account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(transparent)]
pub struct AccountId(Uuid);

impl AccountId {
    /// Creates a new random AccountId.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates an AccountId from an existing UUID.
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

impl Default for AccountId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for AccountId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for AccountId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

/// A financial account that can hold a balance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// Unique identifier
    pub id: AccountId,
    /// Human-readable account name
    pub name: String,
    /// Current balance (includes currency information)
    pub balance: Money,
    /// When the account was created
    pub created_at: DateTime<Utc>,
}

impl Account {
    /// Creates a new account with zero balance.
    ///
    /// # Validation
    /// - Name cannot be empty
    pub fn new(name: String, currency: Currency) -> Result<Self, DomainError> {
        if name.trim().is_empty() {
            return Err(DomainError::ValidationError(
                "Account name cannot be empty".into(),
            ));
        }

        Ok(Self {
            id: AccountId::new(),
            name,
            balance: Money::zero(currency),
            created_at: Utc::now(),
        })
    }

    /// Creates an account with all fields specified (for database reconstruction).
    pub fn from_parts(
        id: AccountId,
        name: String,
        balance: Money,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            name,
            balance,
            created_at,
        }
    }

    /// Returns the currency of this account.
    pub fn currency(&self) -> Currency {
        self.balance.currency()
    }

    /// Credits (adds) money to the account.
    pub fn credit(&mut self, amount: Money) -> Result<(), DomainError> {
        self.balance = self.balance.checked_add(amount)?;
        Ok(())
    }

    /// Debits (subtracts) money from the account.
    pub fn debit(&mut self, amount: Money) -> Result<(), DomainError> {
        self.balance = self.balance.checked_sub(amount)?;
        Ok(())
    }

    /// Checks if the account has sufficient funds for a debit.
    pub fn has_sufficient_funds(&self, amount: &Money) -> bool {
        self.balance.currency() == amount.currency() && self.balance.gte(amount)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_creation() {
        let account = Account::new("Test Account".to_string(), Currency::USD).unwrap();
        assert_eq!(account.name, "Test Account");
        assert_eq!(account.balance.amount(), 0);
        assert_eq!(account.currency(), Currency::USD);
    }

    #[test]
    fn test_empty_name_fails() {
        let result = Account::new("".to_string(), Currency::USD);
        assert!(matches!(result, Err(DomainError::ValidationError(_))));
    }

    #[test]
    fn test_account_credit() {
        let mut account = Account::new("Test".to_string(), Currency::USD).unwrap();
        let amount = Money::new(1000, Currency::USD).unwrap();
        account.credit(amount).unwrap();
        assert_eq!(account.balance.amount(), 1000);
    }

    #[test]
    fn test_account_debit() {
        let mut account = Account::new("Test".to_string(), Currency::USD).unwrap();
        account
            .credit(Money::new(1000, Currency::USD).unwrap())
            .unwrap();
        account
            .debit(Money::new(300, Currency::USD).unwrap())
            .unwrap();
        assert_eq!(account.balance.amount(), 700);
    }

    #[test]
    fn test_insufficient_funds() {
        let mut account = Account::new("Test".to_string(), Currency::USD).unwrap();
        account
            .credit(Money::new(100, Currency::USD).unwrap())
            .unwrap();
        let result = account.debit(Money::new(200, Currency::USD).unwrap());
        assert!(matches!(result, Err(DomainError::InsufficientFunds { .. })));
    }
}
