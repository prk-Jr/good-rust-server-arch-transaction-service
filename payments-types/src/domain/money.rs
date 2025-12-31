//! Type-safe monetary value with embedded currency.
//!
//! This module re-exports the type-safe currency system from `exchange-rates`
//! and provides the DynMoney wrapper for runtime API/DB compatibility.

// Re-export type-safe currency types from exchange-rates
pub use exchange_rates::{
    Currency, CurrencyCode, EUR, GBP, INR, Money, USD, convert, convert_at_base_rate,
    convert_dynamic, get_all_rates, get_base_rate, get_rate, get_rate_dynamic,
};

use crate::error::DomainError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Dynamic money for runtime operations (API/DB layer).
/// Uses the exchange-rates library for type-safe conversions internally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynMoney {
    amount: i64,
    currency: CurrencyCode,
}

impl DynMoney {
    /// Creates a new DynMoney value.
    pub fn new(amount: i64, currency: CurrencyCode) -> Result<Self, DomainError> {
        if amount < 0 {
            return Err(DomainError::NegativeAmount);
        }
        Ok(Self { amount, currency })
    }

    /// Creates a zero-value DynMoney for the given currency.
    pub fn zero(currency: CurrencyCode) -> Self {
        Self {
            amount: 0,
            currency,
        }
    }

    /// Returns the amount in smallest currency unit.
    pub fn amount(&self) -> i64 {
        self.amount
    }

    /// Returns the currency code.
    pub fn currency(&self) -> CurrencyCode {
        self.currency
    }

    /// Convert to another currency using the exchange rate system.
    pub fn convert_to(&self, target: CurrencyCode) -> Self {
        if self.currency == target {
            return *self;
        }
        let converted_amount = convert_dynamic(self.amount, self.currency, target);
        Self {
            amount: converted_amount,
            currency: target,
        }
    }

    /// Get the exchange rate to another currency.
    pub fn rate_to(&self, target: CurrencyCode) -> f64 {
        get_rate_dynamic(self.currency, target)
    }

    /// Checked addition - returns error if currencies don't match.
    pub fn checked_add(&self, other: DynMoney) -> Result<DynMoney, DomainError> {
        if self.currency != other.currency {
            return Err(DomainError::CurrencyMismatch {
                expected: self.currency,
                got: other.currency,
            });
        }
        Ok(DynMoney {
            amount: self.amount.saturating_add(other.amount),
            currency: self.currency,
        })
    }

    /// Checked subtraction - returns error if currencies don't match or result would be negative.
    pub fn checked_sub(&self, other: DynMoney) -> Result<DynMoney, DomainError> {
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
        Ok(DynMoney {
            amount: self.amount - other.amount,
            currency: self.currency,
        })
    }

    /// Returns true if this DynMoney is greater than or equal to the other.
    pub fn gte(&self, other: &DynMoney) -> bool {
        assert_eq!(
            self.currency, other.currency,
            "Cannot compare DynMoney with different currencies"
        );
        self.amount >= other.amount
    }
}

impl fmt::Display for DynMoney {
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
    fn test_dyn_money_creation() {
        let money = DynMoney::new(1000, CurrencyCode::USD).unwrap();
        assert_eq!(money.amount(), 1000);
        assert_eq!(money.currency(), CurrencyCode::USD);
    }

    #[test]
    fn test_negative_money_fails() {
        let result = DynMoney::new(-100, CurrencyCode::USD);
        assert!(matches!(result, Err(DomainError::NegativeAmount)));
    }

    #[test]
    fn test_money_addition() {
        let a = DynMoney::new(100, CurrencyCode::USD).unwrap();
        let b = DynMoney::new(50, CurrencyCode::USD).unwrap();
        let sum = a.checked_add(b).unwrap();
        assert_eq!(sum.amount(), 150);
    }

    #[test]
    fn test_currency_mismatch() {
        let usd = DynMoney::new(100, CurrencyCode::USD).unwrap();
        let eur = DynMoney::new(50, CurrencyCode::EUR).unwrap();
        let result = usd.checked_add(eur);
        assert!(matches!(result, Err(DomainError::CurrencyMismatch { .. })));
    }

    #[test]
    fn test_money_display() {
        let money = DynMoney::new(1050, CurrencyCode::USD).unwrap();
        assert_eq!(format!("{}", money), "$10.50");
    }

    #[test]
    fn test_conversion_usd_to_inr() {
        exchange_rates::disable_fluctuation();
        let usd = DynMoney::new(10000, CurrencyCode::USD).unwrap();
        let inr = usd.convert_to(CurrencyCode::INR);
        assert!(inr.amount() > 800000);
        assert_eq!(inr.currency(), CurrencyCode::INR);
    }

    #[test]
    fn test_conversion_same_currency() {
        let usd = DynMoney::new(10000, CurrencyCode::USD).unwrap();
        let usd2 = usd.convert_to(CurrencyCode::USD);
        assert_eq!(usd.amount(), usd2.amount());
    }

    #[test]
    fn test_rate_to() {
        exchange_rates::disable_fluctuation();
        let usd = DynMoney::new(100, CurrencyCode::USD).unwrap();
        let rate = usd.rate_to(CurrencyCode::INR);
        assert!((rate - 83.12).abs() < 1.0);
    }
}
