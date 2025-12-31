use url::Url;

use crate::types::Currency;

/// Configuration for a Cashu mint
#[derive(Debug, Clone)]
pub struct MintConfig {
    /// Mint URL
    pub url: Url,
    /// Currency this mint issues
    pub currency: Currency,
    /// Human-readable name
    pub name: String,
}

impl MintConfig {
    /// Create a new mint configuration
    pub fn new(url: Url, currency: Currency, name: impl Into<String>) -> Self {
        Self {
            url,
            currency,
            name: name.into(),
        }
    }

    /// Default BTC mint (Minibits)
    pub fn default_btc_mint() -> Self {
        Self {
            url: Url::parse("https://mint.minibits.cash").expect("valid URL"),
            currency: Currency::Btc,
            name: "Minibits".to_string(),
        }
    }

    /// Default USD mint (Stablenut)
    pub fn default_usd_mint() -> Self {
        Self {
            url: Url::parse("https://stablenut.umint.cash").expect("valid URL"),
            currency: Currency::Usd,
            name: "Stablenut".to_string(),
        }
    }

    /// Alternative BTC mint (8333.space)
    pub fn alt_btc_mint() -> Self {
        Self {
            url: Url::parse("https://8333.space").expect("valid URL"),
            currency: Currency::Btc,
            name: "8333".to_string(),
        }
    }
}

/// List of known mints for convenience
pub struct KnownMints;

impl KnownMints {
    /// Get all default mints
    pub fn defaults() -> Vec<MintConfig> {
        vec![
            MintConfig::default_btc_mint(),
            MintConfig::default_usd_mint(),
        ]
    }

    /// Get all BTC mints
    pub fn btc_mints() -> Vec<MintConfig> {
        vec![MintConfig::default_btc_mint(), MintConfig::alt_btc_mint()]
    }

    /// Get all USD mints
    pub fn usd_mints() -> Vec<MintConfig> {
        vec![MintConfig::default_usd_mint()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_btc_mint() {
        let mint = MintConfig::default_btc_mint();
        assert_eq!(mint.currency, Currency::Btc);
        assert!(mint.url.as_str().contains("minibits"));
    }

    #[test]
    fn test_default_usd_mint() {
        let mint = MintConfig::default_usd_mint();
        assert_eq!(mint.currency, Currency::Usd);
        assert!(mint.url.as_str().contains("stablenut"));
    }

    #[test]
    fn test_known_mints() {
        let defaults = KnownMints::defaults();
        assert_eq!(defaults.len(), 2);

        let btc = KnownMints::btc_mints();
        assert!(btc.iter().all(|m| m.currency == Currency::Btc));

        let usd = KnownMints::usd_mints();
        assert!(usd.iter().all(|m| m.currency == Currency::Usd));
    }
}
