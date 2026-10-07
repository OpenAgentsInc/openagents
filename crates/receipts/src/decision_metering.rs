//! The counters a decision process declares and enforces before inference.
//! This attributable declaration proves neither remote execution nor expense.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA: &str = "openagents.decision-metering.v1";
pub const KEV_PACKED_INPUT: &str = "openagents.kev.packed-input.v1";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Unit {
    Tokens,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Basis {
    PackedInputTokenIds,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Counter {
    pub unit: Unit,
    pub basis: Basis,
    /// An enforced per-request ceiling, not observed usage.
    pub maximum: u64,
    /// Empty for the selected counter: Kev uses no cached-input counter.
    pub overlaps: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Metering {
    pub schema: String,
    pub adapter: String,
    pub counters: BTreeMap<String, Counter>,
}
impl Metering {
    /// The packed System One route reports `enc.ids.len()` as input tokens.
    /// Its token admission bounds the padded forward, which is at least that
    /// length. Serialized-answer tokens are a separate, unpriced counter.
    pub fn kev_packed_input(maximum: u64) -> Self {
        Self {
            schema: SCHEMA.into(),
            adapter: KEV_PACKED_INPUT.into(),
            counters: [(
                "input_tokens".into(),
                Counter {
                    unit: Unit::Tokens,
                    basis: Basis::PackedInputTokenIds,
                    maximum,
                    overlaps: vec![],
                },
            )]
            .into(),
        }
    }
    pub fn validate_kev_input(&self) -> Result<u64, &'static str> {
        let expected =
            Self::kev_packed_input(self.counters.get("input_tokens").map_or(0, |c| c.maximum));
        if *self != expected || expected.counters["input_tokens"].maximum == 0 {
            return Err("The backend does not declare the supported packed input-token counter.");
        }
        Ok(expected.counters["input_tokens"].maximum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_counter_has_exact_units_basis_bound_and_no_overlap() {
        let meter = Metering::kev_packed_input(128);
        assert_eq!(meter.validate_kev_input(), Ok(128));
        assert!(Metering::kev_packed_input(0).validate_kev_input().is_err());
        let mut overlapping = meter.clone();
        overlapping
            .counters
            .get_mut("input_tokens")
            .unwrap()
            .overlaps
            .push("cached_input_tokens".into());
        assert!(overlapping.validate_kev_input().is_err());
        let mut changed = serde_json::to_value(meter).unwrap();
        changed["counters"]["input_tokens"]["unit"] = serde_json::json!("milliseconds");
        assert!(serde_json::from_value::<Metering>(changed).is_err());
    }
}
