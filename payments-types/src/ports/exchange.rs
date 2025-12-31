//! Exchange rate provider port.
//!
//! This trait defines the interface for exchange rate services.
//! Implementations can be HTTP clients, mock providers, etc.

use crate::CurrencyCode;

/// Error type for exchange rate operations.
#[derive(Debug, thiserror::Error)]
pub enum ExchangeError {
    #[error("Unsupported currency: {0}")]
    UnsupportedCurrency(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("Rate not available for {0} -> {1}")]
    RateNotAvailable(CurrencyCode, CurrencyCode),
}

/// Port trait for exchange rate providers.
#[async_trait::async_trait]
pub trait ExchangeRateProvider: Send + Sync {
    /// Get the exchange rate from one currency to another.
    /// Returns how many units of `to` currency you get for 1 unit of `from` currency.
    async fn get_rate(&self, from: CurrencyCode, to: CurrencyCode) -> Result<f64, ExchangeError>;

    /// Convert an amount from one currency to another.
    /// Amount is in smallest units (cents, pence, paise, etc.)
    async fn convert(
        &self,
        amount: i64,
        from: CurrencyCode,
        to: CurrencyCode,
    ) -> Result<i64, ExchangeError>;
}
