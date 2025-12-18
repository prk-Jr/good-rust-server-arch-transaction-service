//! Type-safe monetary value with embedded currency.

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::error::DomainError;

/// Currencies supported by the payment system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Currency {
    USD,
    EUR,
    GBP,
    INR,
}

impl Currency {
    /// Returns the number of decimal places for this currency.
    pub fn decimal_places(&self) -> u8 {
        match self {
            Currency::USD | Currency::EUR | Currency::GBP | Currency::INR => 2,
        }
    }

    /// Returns the currency symbol.
    pub fn symbol(&self) -> &'static str {
        match self {
            Currency::USD => "$",
            Currency::EUR => "€",
            Currency::GBP => "£",
            Currency::INR => "₹",
        }
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Type-safe money representation with embedded currency.
///
/// Amount is stored in the smallest unit of the currency (cents, paise, etc.)
/// to avoid floating-point precision issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    amount: i64,
    currency: Currency,
}

impl Money {
    /// Creates a new Money value.
    pub fn new(amount: i64, currency: Currency) -> Result<Self, DomainError> {
        if amount < 0 {
            return Err(DomainError::NegativeAmount);
        }
        Ok(Self { amount, currency })
    }

    /// Creates a zero-value Money for the given currency.
    pub fn zero(currency: Currency) -> Self {
        Self {
            amount: 0,
            currency,
        }
    }

    /// Returns the amount in smallest currency unit.
    pub fn amount(&self) -> i64 {
        self.amount
    }

    /// Returns the currency.
    pub fn currency(&self) -> Currency {
        self.currency
    }

    /// Checked addition - returns error if currencies don't match.
    pub fn checked_add(&self, other: Money) -> Result<Money, DomainError> {
        if self.currency != other.currency {
            return Err(DomainError::CurrencyMismatch {
                expected: self.currency,
                got: other.currency,
            });
        }
        Ok(Money {
            amount: self.amount.saturating_add(other.amount),
            currency: self.currency,
        })
    }

    /// Checked subtraction - returns error if currencies don't match or result would be negative.
    pub fn checked_sub(&self, other: Money) -> Result<Money, DomainError> {
        if self.currency != other.currency {
            return Err(DomainError::CurrencyMismatch {
                expected: self.currency,
                got: other.currency,
            });
        }
        if self.amount < other.amount {
            return Err(DomainError::InsufficientFunds {
                available: self.amount,
                requested: other.amount,
            });
        }
        Ok(Money {
            amount: self.amount - other.amount,
            currency: self.currency,
        })
    }

    /// Returns true if this Money is greater than or equal to the other.
    pub fn gte(&self, other: &Money) -> bool {
        assert_eq!(
            self.currency, other.currency,
            "Cannot compare Money with different currencies"
        );
        self.amount >= other.amount
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let major = self.amount / 100;
        let minor = (self.amount % 100).abs();
        write!(f, "{}{}.{:02}", self.currency.symbol(), major, minor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_money_creation() {
        let money = Money::new(1000, Currency::USD).unwrap();
        assert_eq!(money.amount(), 1000);
        assert_eq!(money.currency(), Currency::USD);
    }

    #[test]
    fn test_negative_money_fails() {
        let result = Money::new(-100, Currency::USD);
        assert!(matches!(result, Err(DomainError::NegativeAmount)));
    }

    #[test]
    fn test_money_addition() {
        let a = Money::new(100, Currency::USD).unwrap();
        let b = Money::new(50, Currency::USD).unwrap();
        let sum = a.checked_add(b).unwrap();
        assert_eq!(sum.amount(), 150);
    }

    #[test]
    fn test_currency_mismatch() {
        let usd = Money::new(100, Currency::USD).unwrap();
        let eur = Money::new(50, Currency::EUR).unwrap();
        let result = usd.checked_add(eur);
        assert!(matches!(result, Err(DomainError::CurrencyMismatch { .. })));
    }

    #[test]
    fn test_money_display() {
        let money = Money::new(1050, Currency::USD).unwrap();
        assert_eq!(format!("{}", money), "$10.50");
    }
}
