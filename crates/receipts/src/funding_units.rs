//! Pinned monetary units and operator-reviewed conversions.
use serde::{Deserialize, Serialize};

/// Exact monetary denominations. XP and promotional points are not money units.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Unit {
    Satoshis,
    Millisatoshis,
    CurrencyMillionths { currency: String },
}

impl Unit {
    pub fn validate(&self) -> Result<(), String> {
        if let Self::CurrencyMillionths { currency: code } = self {
            currency(code)?;
        }
        Ok(())
    }

    /// Number of integer units per whole named currency unit.
    #[must_use]
    pub const fn scale(&self) -> u64 {
        match self {
            Self::Satoshis => 100_000_000,
            Self::Millisatoshis => 100_000_000_000,
            Self::CurrencyMillionths { .. } => 1_000_000,
        }
    }

    #[must_use]
    pub fn currency(&self) -> &str {
        match self {
            Self::Satoshis | Self::Millisatoshis => "BTC",
            Self::CurrencyMillionths { currency } => currency,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Rounding {
    Exact,
    Down,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FeePayer {
    Customer,
    Operator,
}

/// A conversion quote freezes source units, fees, and the uncredited remainder.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Converted {
    pub gross_units: u64,
    pub fee_units: u64,
    pub convertible_units: u64,
    pub credited_units: u64,
    /// A fraction of one destination integer unit, never spendable credit.
    pub remainder: u64,
    pub denominator: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Conversion {
    pub version: String,
    pub source: Unit,
    pub target: Unit,
    pub numerator: u64,
    pub denominator: u64,
    /// Reference to the configured rate's authority and evidence.
    pub source_ref: String,
    pub valid_from: u64,
    pub valid_until: u64,
    pub rounding: Rounding,
    pub fee_payer: FeePayer,
    /// Maximum verified fee, expressed in the source's integer units.
    pub max_fee_units: u64,
}

impl Conversion {
    pub fn validate(&self) -> Result<(), String> {
        identity(&self.version)?;
        identity(&self.source_ref)?;
        self.source.validate()?;
        self.target.validate()?;
        if self.numerator == 0 || self.denominator == 0 || self.valid_until <= self.valid_from {
            return Err("conversion needs a positive rate and bounded validity".into());
        }
        // Changing scale within one currency must conserve that currency exactly.
        if self.source.currency() == self.target.currency()
            && u128::from(self.numerator) * u128::from(self.source.scale())
                != u128::from(self.denominator) * u128::from(self.target.scale())
        {
            return Err("same-currency conversion changes monetary value".into());
        }
        Ok(())
    }

    /// Convert exact integer units while retaining the fractional remainder.
    pub fn amount(&self, units: u64) -> Result<(u64, u64), String> {
        let numerator = u128::from(units) * u128::from(self.numerator);
        let denominator = u128::from(self.denominator);
        let remainder =
            u64::try_from(numerator % denominator).map_err(|_| "conversion overflow")?;
        if self.rounding == Rounding::Exact && remainder != 0 {
            return Err("conversion precision would discard monetary dust".into());
        }
        let credited = u64::try_from(numerator / denominator).map_err(|_| "conversion overflow")?;
        Ok((credited, remainder))
    }

    pub fn quote(&self, gross: u64, fee: u64, at: u64) -> Result<Converted, String> {
        self.validate()?;
        if at < self.valid_from || at >= self.valid_until {
            return Err("conversion is unavailable at the funding time".into());
        }
        if gross == 0 || fee > self.max_fee_units {
            return Err("funding amount is zero or fee exceeds its terms".into());
        }
        let convertible = match self.fee_payer {
            FeePayer::Customer => gross.checked_sub(fee).ok_or("fee exceeds funding")?,
            FeePayer::Operator => gross,
        };
        let (credited, remainder) = self.amount(convertible)?;
        if credited == 0 {
            return Err("conversion yields no spendable credit".into());
        }
        Ok(Converted {
            gross_units: gross,
            fee_units: fee,
            convertible_units: convertible,
            credited_units: credited,
            remainder,
            denominator: self.denominator,
        })
    }
}

fn identity(value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > 1024 || value.chars().any(char::is_control) {
        return Err("accounting references must be nonempty bounded identifiers".into());
    }
    Ok(())
}
fn currency(value: &str) -> Result<(), String> {
    if value.len() != 3 || !value.bytes().all(|b| b.is_ascii_uppercase()) {
        return Err("currency must be an explicit three-letter uppercase code".into());
    }
    Ok(())
}
