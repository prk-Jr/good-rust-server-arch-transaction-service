//! Type-Safe Exchange Rates Library with Macro-Based Currency Generation
//!
//! This library provides compile-time type-safe currency handling using Rust's
//! type system with PhantomData. Currencies are defined declaratively using a macro
//! that auto-generates all necessary types, traits, and conversions.
//!
//! # Adding a New Currency
//! Simply add a line to the `define_currencies!` macro invocation:
//! ```ignore
//! define_currencies! {
//!     // ... existing currencies ...
//!     JPY => ("JPY", "¥", "sen", 100, 0.0067),
//! }
//! ```
//!
//! # Example
//! ```
//! use exchange_rates::{Money, USD, EUR, INR, CurrencyCode};
//!
//! // Create money in USD (amount in cents)
//! let dollars = Money::<USD>::from_minor(10000); // $100.00
//!
//! // Type-safe conversion to INR
//! let rupees: Money<INR> = dollars.into();
//! println!("{}", rupees); // ₹8,312.00
//!
//! // Runtime conversion
//! let converted = exchange_rates::convert_dynamic(10000, CurrencyCode::USD, CurrencyCode::INR);
//! ```

use std::fmt;
use std::marker::PhantomData;
use std::ops::{Add, Sub};
use std::sync::atomic::{AtomicBool, Ordering};

// ─────────────────────────────────────────────────────────────────────────────
// Global Fluctuation Control
// ─────────────────────────────────────────────────────────────────────────────

static FLUCTUATION_ENABLED: AtomicBool = AtomicBool::new(false);

/// Enable random rate fluctuation for realistic simulation.
pub fn enable_fluctuation() {
    FLUCTUATION_ENABLED.store(true, Ordering::Relaxed);
}

/// Disable rate fluctuation (use base rates only).
pub fn disable_fluctuation() {
    FLUCTUATION_ENABLED.store(false, Ordering::Relaxed);
}

/// Check if fluctuation is enabled.
pub fn is_fluctuation_enabled() -> bool {
    FLUCTUATION_ENABLED.load(Ordering::Relaxed)
}

fn fluctuate(base_rate: f64, max_variance_percent: f64) -> f64 {
    if !is_fluctuation_enabled() {
        return base_rate;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let random_factor = ((nanos % 2001) as f64 / 1000.0) - 1.0;
    let variance = base_rate * (max_variance_percent / 100.0) * random_factor;
    base_rate + variance
}

// ─────────────────────────────────────────────────────────────────────────────
// Currency Trait
// ─────────────────────────────────────────────────────────────────────────────

/// Trait defining currency metadata and behavior.
pub trait Currency: Default + Clone + Copy + Send + Sync + 'static {
    const CODE: &'static str;
    const SYMBOL: &'static str;
    const MINOR_UNIT: &'static str;
    const MINOR_UNITS_PER_MAJOR: i32;
    const BASE_TO_USD_RATE: f64;
    const MAX_VARIANCE_PERCENT: f64;

    fn to_usd_rate() -> f64 {
        fluctuate(Self::BASE_TO_USD_RATE, Self::MAX_VARIANCE_PERCENT)
    }

    fn base_to_usd_rate() -> f64 {
        Self::BASE_TO_USD_RATE
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Type-Safe Money
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Money<C: Currency> {
    amount: i64,
    _currency: PhantomData<C>,
}

impl<C: Currency> Money<C> {
    pub fn from_minor(amount: i64) -> Self {
        Self {
            amount,
            _currency: PhantomData,
        }
    }

    pub fn from_major(major: i64) -> Self {
        Self::from_minor(major * C::MINOR_UNITS_PER_MAJOR as i64)
    }

    pub fn minor_units(&self) -> i64 {
        self.amount
    }
    pub fn major_units(&self) -> i64 {
        self.amount / C::MINOR_UNITS_PER_MAJOR as i64
    }
    pub fn minor_part(&self) -> i64 {
        self.amount.abs() % C::MINOR_UNITS_PER_MAJOR as i64
    }
    pub fn is_zero(&self) -> bool {
        self.amount == 0
    }
    pub fn is_negative(&self) -> bool {
        self.amount < 0
    }
    pub fn currency_code(&self) -> &'static str {
        C::CODE
    }
    pub fn currency_symbol(&self) -> &'static str {
        C::SYMBOL
    }

    pub fn convert<T: Currency>(self) -> Money<T> {
        convert::<C, T>(self)
    }

    pub fn convert_at_base_rate<T: Currency>(self) -> Money<T> {
        convert_at_base_rate::<C, T>(self)
    }
}

impl<C: Currency> Default for Money<C> {
    fn default() -> Self {
        Self::from_minor(0)
    }
}

impl<C: Currency> fmt::Debug for Money<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Money {{ amount: {}, currency: {} }}",
            self.amount,
            C::CODE
        )
    }
}

impl<C: Currency> fmt::Display for Money<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let major = self.major_units();
        let minor = self.minor_part();
        if self.is_negative() && major == 0 {
            write!(f, "-{}{}.{:02}", C::SYMBOL, 0, minor)
        } else {
            write!(f, "{}{}.{:02}", C::SYMBOL, major, minor)
        }
    }
}

impl<C: Currency> Add for Money<C> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Money::from_minor(self.amount + rhs.amount)
    }
}

impl<C: Currency> Sub for Money<C> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Money::from_minor(self.amount - rhs.amount)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Currency Conversion Functions
// ─────────────────────────────────────────────────────────────────────────────

pub fn convert<From: Currency, To: Currency>(money: Money<From>) -> Money<To> {
    let usd_amount = money.amount as f64 * From::to_usd_rate();
    let target_amount = usd_amount / To::to_usd_rate();
    Money::from_minor(target_amount.round() as i64)
}

pub fn convert_at_base_rate<From: Currency, To: Currency>(money: Money<From>) -> Money<To> {
    let usd_amount = money.amount as f64 * From::base_to_usd_rate();
    let target_amount = usd_amount / To::base_to_usd_rate();
    Money::from_minor(target_amount.round() as i64)
}

pub fn get_rate<From: Currency, To: Currency>() -> f64 {
    From::to_usd_rate() / To::to_usd_rate()
}

pub fn get_base_rate<From: Currency, To: Currency>() -> f64 {
    From::base_to_usd_rate() / To::base_to_usd_rate()
}

// ─────────────────────────────────────────────────────────────────────────────
// THE MACRO: Defines all currencies, CurrencyCode enum, and runtime dispatch
// ─────────────────────────────────────────────────────────────────────────────

/// Macro to define currencies with auto-generated types, traits, and conversions.
///
/// # Syntax
/// ```ignore
/// define_currencies! {
///     CurrencyName => ("CODE", "SYMBOL", "minor_unit", minor_per_major, to_usd_rate, variance%),
/// }
/// ```
#[macro_export]
macro_rules! define_currencies {
    (
        $(
            $name:ident => ($code:literal, $symbol:literal, $minor:literal, $minor_per_major:expr, $to_usd:expr, $variance:expr)
        ),* $(,)?
    ) => {
        // ─────────────────────────────────────────────────────────────────────
        // Generate marker types and Currency trait impls
        // ─────────────────────────────────────────────────────────────────────
        $(
            #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
            pub struct $name;

            impl Currency for $name {
                const CODE: &'static str = $code;
                const SYMBOL: &'static str = $symbol;
                const MINOR_UNIT: &'static str = $minor;
                const MINOR_UNITS_PER_MAJOR: i32 = $minor_per_major;
                const BASE_TO_USD_RATE: f64 = $to_usd;
                const MAX_VARIANCE_PERCENT: f64 = $variance;
            }
        )*

        // ─────────────────────────────────────────────────────────────────────
        // Note: From impls are generated separately using impl_from_for_pair! macro
        // ─────────────────────────────────────────────────────────────────────


        // ─────────────────────────────────────────────────────────────────────
        // Generate CurrencyCode enum for runtime operations
        // ─────────────────────────────────────────────────────────────────────
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
        #[serde(rename_all = "UPPERCASE")]
        pub enum CurrencyCode {
            $($name),*
        }


        impl CurrencyCode {
            pub fn code(&self) -> &'static str {
                match self {
                    $(CurrencyCode::$name => $code),*
                }
            }

            pub fn symbol(&self) -> &'static str {
                match self {
                    $(CurrencyCode::$name => $symbol),*
                }
            }

            pub fn base_to_usd_rate(&self) -> f64 {
                match self {
                    $(CurrencyCode::$name => $to_usd),*
                }
            }

            pub fn to_usd_rate(&self) -> f64 {
                match self {
                    $(CurrencyCode::$name => <$name as Currency>::to_usd_rate()),*
                }
            }

            pub fn all() -> &'static [CurrencyCode] {
                &[$(CurrencyCode::$name),*]
            }
        }

        impl std::fmt::Display for CurrencyCode {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.code())
            }
        }

        impl std::str::FromStr for CurrencyCode {
            type Err = String;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s.to_uppercase().as_str() {
                    $($code => Ok(CurrencyCode::$name),)*
                    _ => Err(format!("Unknown currency: {}", s)),
                }
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // Runtime conversion function (dispatches to type-safe converters)
        // ─────────────────────────────────────────────────────────────────────
        pub fn convert_dynamic(amount: i64, from: CurrencyCode, to: CurrencyCode) -> i64 {
            if from == to { return amount; }

            // Use the base rates for conversion
            let usd_amount = amount as f64 * from.to_usd_rate();
            let target_amount = usd_amount / to.to_usd_rate();
            target_amount.round() as i64
        }

        pub fn get_rate_dynamic(from: CurrencyCode, to: CurrencyCode) -> f64 {
            if from == to { return 1.0; }
            from.to_usd_rate() / to.to_usd_rate()
        }

        pub fn get_all_rates(base: CurrencyCode) -> std::collections::HashMap<CurrencyCode, f64> {
            CurrencyCode::all()
                .iter()
                .map(|&c| (c, get_rate_dynamic(base, c)))
                .collect()
        }
    };
}

// We need a different approach for From impls - let's generate them explicitly
// Using a helper macro for cross-product

macro_rules! impl_from_for_pair {
    ($from:ident, $to:ident) => {
        impl From<Money<$from>> for Money<$to> {
            fn from(money: Money<$from>) -> Self {
                convert(money)
            }
        }
    };
}

// ─────────────────────────────────────────────────────────────────────────────
// CURRENCY DEFINITIONS - Add new currencies here!
// ─────────────────────────────────────────────────────────────────────────────

define_currencies! {
    USD => ("USD", "$", "cent", 100, 1.0, 0.0),
    EUR => ("EUR", "€", "cent", 100, 1.087, 0.5),
    GBP => ("GBP", "£", "penny", 100, 1.266, 0.5),
    INR => ("INR", "₹", "paisa", 100, 0.01203, 0.3),
}

// Generate From impls for all pairs (4 currencies = 12 impls)
impl_from_for_pair!(USD, EUR);
impl_from_for_pair!(USD, GBP);
impl_from_for_pair!(USD, INR);
impl_from_for_pair!(EUR, USD);
impl_from_for_pair!(EUR, GBP);
impl_from_for_pair!(EUR, INR);
impl_from_for_pair!(GBP, USD);
impl_from_for_pair!(GBP, EUR);
impl_from_for_pair!(GBP, INR);
impl_from_for_pair!(INR, USD);
impl_from_for_pair!(INR, EUR);
impl_from_for_pair!(INR, GBP);

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() {
        disable_fluctuation();
    }

    #[test]
    fn test_money_creation() {
        setup();
        let usd = Money::<USD>::from_minor(10050);
        assert_eq!(usd.minor_units(), 10050);
        assert_eq!(usd.major_units(), 100);
        assert_eq!(usd.minor_part(), 50);
    }

    #[test]
    fn test_money_display() {
        setup();
        let usd = Money::<USD>::from_minor(10050);
        assert_eq!(format!("{}", usd), "$100.50");
    }

    #[test]
    fn test_same_currency_addition() {
        setup();
        let a = Money::<USD>::from_minor(1000);
        let b = Money::<USD>::from_minor(500);
        assert_eq!((a + b).minor_units(), 1500);
    }

    #[test]
    fn test_usd_to_inr_conversion() {
        setup();
        let usd = Money::<USD>::from_minor(10000);
        let inr: Money<INR> = usd.into();
        assert!((inr.minor_units() - 831200).abs() < 100);
    }

    #[test]
    fn test_inr_to_usd_conversion() {
        setup();
        let inr = Money::<INR>::from_minor(831200);
        let usd: Money<USD> = inr.into();
        assert!((usd.minor_units() - 10000).abs() < 10);
    }

    #[test]
    fn test_currency_code_parse() {
        assert_eq!("USD".parse::<CurrencyCode>().unwrap(), CurrencyCode::USD);
        assert_eq!("eur".parse::<CurrencyCode>().unwrap(), CurrencyCode::EUR);
    }

    #[test]
    fn test_currency_code_display() {
        assert_eq!(CurrencyCode::USD.to_string(), "USD");
    }

    #[test]
    fn test_convert_dynamic() {
        setup();
        let converted = convert_dynamic(10000, CurrencyCode::USD, CurrencyCode::INR);
        assert!((converted - 831200).abs() < 100);
    }

    #[test]
    fn test_get_rate_dynamic() {
        setup();
        let rate = get_rate_dynamic(CurrencyCode::USD, CurrencyCode::INR);
        assert!((rate - 83.12).abs() < 1.0);
    }

    #[test]
    fn test_get_all_rates() {
        setup();
        let rates = get_all_rates(CurrencyCode::USD);
        assert_eq!(rates.get(&CurrencyCode::USD), Some(&1.0));
        assert!(rates.contains_key(&CurrencyCode::EUR));
    }

    #[test]
    fn test_currency_code_all() {
        let all = CurrencyCode::all();
        assert_eq!(all.len(), 4);
    }
}
