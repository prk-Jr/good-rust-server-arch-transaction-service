//! Account domain model.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::money::{CurrencyCode, DynMoney};
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
    pub balance: DynMoney,
    /// When the account was created
    pub created_at: DateTime<Utc>,
}

impl Account {
    /// Creates a new account with zero balance.
    ///
    /// # Validation
    /// - Name cannot be empty
    pub fn new(name: String, currency: CurrencyCode) -> Result<Self, DomainError> {
        if name.trim().is_empty() {
            return Err(DomainError::ValidationError(
                "Account name cannot be empty".into(),
            ));
        }

        Ok(Self {
            id: AccountId::new(),
            name,
            balance: DynMoney::zero(currency),
            created_at: Utc::now(),
        })
    }

    /// Creates an account with all fields specified (for database reconstruction).
    pub fn from_parts(
        id: AccountId,
        name: String,
        balance: DynMoney,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            name,
            balance,
            created_at,
        }
    }

    /// Returns the account's currency.
    pub fn currency(&self) -> CurrencyCode {
        self.balance.currency()
    }

    /// Deposits money into the account.
    ///
    /// # Validation
    /// - Currency must match
    /// - Amount must be positive
    pub fn deposit(&mut self, amount: DynMoney) -> Result<(), DomainError> {
        self.balance = self.balance.checked_add(amount)?;
        Ok(())
    }

    /// Withdraws money from the account.
    ///
    /// # Validation
    /// - Currency must match
    /// - Sufficient funds required
    pub fn withdraw(&mut self, amount: DynMoney) -> Result<(), DomainError> {
        self.balance = self.balance.checked_sub(amount)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_creation() {
        let account = Account::new("Test Account".into(), CurrencyCode::USD).unwrap();
        assert_eq!(account.balance.amount(), 0);
        assert_eq!(account.currency(), CurrencyCode::USD);
    }

    #[test]
    fn test_empty_name_fails() {
        let result = Account::new("".into(), CurrencyCode::USD);
        assert!(matches!(result, Err(DomainError::ValidationError(_))));
    }

    #[test]
    fn test_deposit() {
        let mut account = Account::new("Test".into(), CurrencyCode::USD).unwrap();
        let deposit = DynMoney::new(100, CurrencyCode::USD).unwrap();
        account.deposit(deposit).unwrap();
        assert_eq!(account.balance.amount(), 100);
    }

    #[test]
    fn test_withdraw() {
        let mut account = Account::new("Test".into(), CurrencyCode::USD).unwrap();
        let deposit = DynMoney::new(100, CurrencyCode::USD).unwrap();
        account.deposit(deposit).unwrap();

        let withdraw = DynMoney::new(30, CurrencyCode::USD).unwrap();
        account.withdraw(withdraw).unwrap();
        assert_eq!(account.balance.amount(), 70);
    }

    #[test]
    fn test_insufficient_funds() {
        let mut account = Account::new("Test".into(), CurrencyCode::USD).unwrap();
        let deposit = DynMoney::new(50, CurrencyCode::USD).unwrap();
        account.deposit(deposit).unwrap();

        let withdraw = DynMoney::new(100, CurrencyCode::USD).unwrap();
        let result = account.withdraw(withdraw);
        assert!(matches!(result, Err(DomainError::InsufficientFunds { .. })));
    }

    #[test]
    fn test_currency_mismatch() {
        let mut account = Account::new("Test".into(), CurrencyCode::USD).unwrap();
        let deposit = DynMoney::new(100, CurrencyCode::EUR).unwrap();
        let result = account.deposit(deposit);
        assert!(matches!(result, Err(DomainError::CurrencyMismatch { .. })));
    }
}
