//! Later market qualification contracts. These values grant no dispatch or wallet authority.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

pub mod bids;
pub mod worker;

/// Check a pinned SHA-256 identity rather than accepting a mutable name.
pub fn exact(value: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(Error::Invalid("market artifact digest"));
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deadlines {
    pub delivery: i64,
    pub review: i64,
    pub dispute: i64,
    pub resolution: i64,
    pub payment: i64,
    pub retain_until: i64,
}
impl Deadlines {
    pub fn validate(&self, now: i64) -> Result<()> {
        if !(now < self.delivery
            && self.delivery <= self.review
            && self.review <= self.dispute
            && self.dispute < self.resolution
            && self.resolution < self.payment
            && self.payment < self.retain_until)
        {
            return Err(Error::Invalid("market deadline ordering"));
        }
        Ok(())
    }
}
