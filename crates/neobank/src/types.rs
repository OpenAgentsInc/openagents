use serde::{Deserialize, Serialize};
use url::Url;

/// Currency denomination
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Currency {
    /// Bitcoin (amounts in sats)
    Btc,
    /// US Dollar (amounts in cents)
    Usd,
}

impl Currency {
    /// Returns the smallest unit name for this currency
    pub fn unit_name(&self) -> &'static str {
        match self {
            Self::Btc => "sats",
            Self::Usd => "cents",
        }
    }

    /// Returns the display symbol
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Btc => "BTC",
            Self::Usd => "USD",
        }
    }
}

/// Specific asset on a specific rail (mint)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AssetId {
    /// BTC-denominated eCash from a specific mint
    BtcCashu { mint_url: Url },
    /// USD-denominated eCash from a specific mint
    UsdCashu { mint_url: Url },
}

impl AssetId {
    /// Get the currency denomination of this asset
    pub fn currency(&self) -> Currency {
        match self {
            Self::BtcCashu { .. } => Currency::Btc,
            Self::UsdCashu { .. } => Currency::Usd,
        }
    }

    /// Get the mint URL for this asset
    pub fn mint_url(&self) -> &Url {
        match self {
            Self::BtcCashu { mint_url } | Self::UsdCashu { mint_url } => mint_url,
        }
    }

    /// Create a BTC asset for a given mint
    pub fn btc_cashu(mint_url: Url) -> Self {
        Self::BtcCashu { mint_url }
    }

    /// Create a USD asset for a given mint
    pub fn usd_cashu(mint_url: Url) -> Self {
        Self::UsdCashu { mint_url }
    }
}

/// Amount in smallest unit (sats for BTC, cents for USD)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Amount {
    /// Value in smallest unit
    pub value: u64,
    /// Currency denomination
    pub currency: Currency,
}

impl Amount {
    /// Create an amount in satoshis
    pub fn sats(value: u64) -> Self {
        Self {
            value,
            currency: Currency::Btc,
        }
    }

    /// Create an amount in cents
    pub fn cents(value: u64) -> Self {
        Self {
            value,
            currency: Currency::Usd,
        }
    }

    /// Create an amount with explicit currency
    pub fn new(value: u64, currency: Currency) -> Self {
        Self { value, currency }
    }

    /// Check if this amount is zero
    pub fn is_zero(&self) -> bool {
        self.value == 0
    }

    /// Add two amounts (panics if currencies don't match)
    pub fn add(&self, other: &Self) -> Self {
        assert_eq!(
            self.currency, other.currency,
            "Cannot add amounts with different currencies"
        );
        Self {
            value: self.value + other.value,
            currency: self.currency,
        }
    }

    /// Subtract two amounts (panics if currencies don't match or would underflow)
    pub fn sub(&self, other: &Self) -> Self {
        assert_eq!(
            self.currency, other.currency,
            "Cannot subtract amounts with different currencies"
        );
        Self {
            value: self.value.checked_sub(other.value).expect("Amount underflow"),
            currency: self.currency,
        }
    }

    /// Checked subtraction
    pub fn checked_sub(&self, other: &Self) -> Option<Self> {
        if self.currency != other.currency {
            return None;
        }
        self.value.checked_sub(other.value).map(|value| Self {
            value,
            currency: self.currency,
        })
    }
}

impl std::fmt::Display for Amount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.currency {
            Currency::Btc => write!(f, "{} sats", self.value),
            Currency::Usd => {
                let dollars = self.value / 100;
                let cents = self.value % 100;
                write!(f, "${}.{:02}", dollars, cents)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amount_sats() {
        let amount = Amount::sats(50000);
        assert_eq!(amount.value, 50000);
        assert_eq!(amount.currency, Currency::Btc);
        assert_eq!(format!("{}", amount), "50000 sats");
    }

    #[test]
    fn test_amount_cents() {
        let amount = Amount::cents(1234);
        assert_eq!(amount.value, 1234);
        assert_eq!(amount.currency, Currency::Usd);
        assert_eq!(format!("{}", amount), "$12.34");
    }

    #[test]
    fn test_amount_add() {
        let a = Amount::sats(100);
        let b = Amount::sats(200);
        let c = a.add(&b);
        assert_eq!(c.value, 300);
    }

    #[test]
    #[should_panic(expected = "Cannot add amounts with different currencies")]
    fn test_amount_add_different_currencies() {
        let a = Amount::sats(100);
        let b = Amount::cents(100);
        let _ = a.add(&b);
    }

    #[test]
    fn test_asset_id() {
        let url = Url::parse("https://mint.example.com").unwrap();
        let btc = AssetId::btc_cashu(url.clone());
        let usd = AssetId::usd_cashu(url.clone());

        assert_eq!(btc.currency(), Currency::Btc);
        assert_eq!(usd.currency(), Currency::Usd);
        assert_eq!(btc.mint_url(), &url);
    }
}
