//! Exact monetary amounts.
//!
//! Prices are stored as **integer minor units** plus an ISO-4217 currency code, never `f64`,
//! so `19.99 + 0.01 == 20.00` stays true forever and CSV round-trips are lossless.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{DomainError, Result};

/// A monetary amount in minor units of `currency` (e.g. 1999 EGP piastras/cents).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Type)]
pub struct Money {
    /// Amount in minor units. Negative means a refund/credit.
    pub minor: i64,
    /// ISO-4217 alphabetic code, upper-cased (`EGP`, `USD`, `SAR`).
    pub currency: [u8; 3],
}

impl Money {
    /// Create a validated amount.
    ///
    /// # Errors
    ///
    /// * [`DomainError::InvalidEnum`] if `currency` is not a 3-letter alphabetic ISO-4217 code.
    pub fn new(minor: i64, currency: &str) -> Result<Self> {
        let upper = currency.trim().to_ascii_uppercase();
        let bytes = upper.as_bytes();
        if bytes.len() != 3 || !bytes.iter().all(u8::is_ascii_alphabetic) {
            return Err(DomainError::InvalidEnum {
                field: "currency".into(),
                got: currency.to_owned(),
            });
        }
        let mut code = [b'?'; 3];
        code.copy_from_slice(bytes);
        Ok(Self {
            minor,
            currency: code,
        })
    }

    /// The ISO-4217 code as a string.
    #[must_use]
    pub fn currency_str(&self) -> &str {
        // Invariant: only ASCII letters are ever written here.
        std::str::from_utf8(&self.currency).unwrap_or("???")
    }

    /// Amount expressed in major units as an exact `String` (`"19.99"`), never a float.
    ///
    /// Currencies with 0 decimal places (JPY, KWD is 3) are handled by the caller via
    /// [`Money::minor`] directly; the default 2-decimal rendering is what the UI shows.
    #[must_use]
    pub fn to_major_string(&self) -> String {
        let sign = if self.minor < 0 { "-" } else { "" };
        let abs = self.minor.unsigned_abs();
        format!("{sign}{}.{:02}", abs / 100, abs % 100)
    }
}

impl std::fmt::Display for Money {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.to_major_string(), self.currency_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn formats_major_units_exactly() {
        let m = Money::new(1999, "egp").unwrap();
        assert_eq!(m.to_major_string(), "19.99");
        assert_eq!(m.currency_str(), "EGP");
        assert_eq!(m.to_string(), "19.99 EGP");
    }

    #[test]
    fn formats_zero_and_negative() {
        assert_eq!(Money::new(0, "USD").unwrap().to_major_string(), "0.00");
        assert_eq!(Money::new(-250, "USD").unwrap().to_major_string(), "-2.50");
    }

    #[test]
    fn no_float_drift_in_addition() {
        let a = Money::new(1999, "USD").unwrap();
        let b = Money::new(1, "USD").unwrap();
        assert_eq!(a.minor + b.minor, 2000);
        assert_eq!(
            Money::new(a.minor + b.minor, "USD")
                .unwrap()
                .to_major_string(),
            "20.00"
        );
    }

    #[test]
    fn rejects_bad_currency() {
        assert!(Money::new(1, "").is_err());
        assert!(Money::new(1, "US").is_err());
        assert!(Money::new(1, "USDX").is_err());
        assert!(Money::new(1, "123").is_err());
        assert!(Money::new(1, "usd").is_ok());
    }
}
