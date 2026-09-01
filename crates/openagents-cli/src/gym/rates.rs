//! The pinned per-token rate catalog (`bench/rates.json`) and the one rule
//! that makes pricing worth doing: an unpriced trial reports `unknown`, never
//! zero. This is the Rust port of the deleted
//! `packages/coder-effectiveness/src/pricing.ts`, with the catalog moved out
//! of source and into a checked-in JSON file so a rate change is a data
//! change, reviewed next to the suites it prices.

use crate::errors::CliError;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

pub const RATE_CATALOG_SCHEMA: &str = "openagents.coder_rate_catalog.v1";

/// A per-model rate row, in USD per 1,000,000 tokens.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRate {
    pub input_usd_per_mtok: f64,
    /// Rate for cached-read input tokens. Absent means cached reads bill at
    /// the full input rate, because no cheaper rate was declared.
    #[serde(default)]
    pub cached_input_usd_per_mtok: Option<f64>,
    pub output_usd_per_mtok: f64,
    /// `operator_placeholder` or `operator_confirmed`. A placeholder rate is
    /// arithmetically usable and economically provisional, and every figure
    /// derived from one says so.
    pub rate_basis: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RateCatalogFile {
    schema: String,
    version: String,
    #[serde(default)]
    models: BTreeMap<String, ModelRate>,
    #[serde(default)]
    deliberately_unpriced: BTreeMap<String, String>,
}

/// The loaded catalog. `deliberately_unpriced` separates "the catalog refuses
/// to price this id, and says why" from "this id is unknown here" — a
/// different and less confident finding.
#[derive(Debug, Clone)]
pub struct RateCatalog {
    pub version: String,
    pub models: BTreeMap<String, ModelRate>,
    pub deliberately_unpriced: BTreeMap<String, String>,
}

/// Read `bench/rates.json` under the repo root. A missing or malformed
/// catalog is a configuration error, not an empty catalog: scoring against a
/// silently empty catalog would report every lane `cost_unknown` and look
/// exactly like the honest answer.
pub fn load_rate_catalog(repo_root: &Path) -> Result<RateCatalog, CliError> {
    let path = repo_root.join("bench").join("rates.json");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| CliError::Configuration(format!("could not read {}: {e}", path.display())))?;
    let file: RateCatalogFile = serde_json::from_str(&text).map_err(|e| {
        CliError::Configuration(format!("{} is not a rate catalog: {e}", path.display()))
    })?;
    if file.schema != RATE_CATALOG_SCHEMA {
        return Err(CliError::Configuration(format!(
            "{} has schema '{}', expected '{RATE_CATALOG_SCHEMA}'",
            path.display(),
            file.schema
        )));
    }
    for (id, rate) in &file.models {
        if !(rate.input_usd_per_mtok >= 0.0)
            || !(rate.output_usd_per_mtok >= 0.0)
            || rate.cached_input_usd_per_mtok.is_some_and(|c| !(c >= 0.0))
        {
            return Err(CliError::Configuration(format!(
                "{} rate for {id} holds a negative or non-finite figure",
                path.display()
            )));
        }
        if rate.rate_basis != "operator_placeholder" && rate.rate_basis != "operator_confirmed" {
            return Err(CliError::Configuration(format!(
                "{} rate for {id} has basis '{}', expected operator_placeholder or operator_confirmed",
                path.display(),
                rate.rate_basis
            )));
        }
    }
    Ok(RateCatalog {
        version: file.version,
        models: file.models,
        deliberately_unpriced: file.deliberately_unpriced,
    })
}

/// Token usage offered for pricing. A `None` count is unknown, not zero.
#[derive(Debug, Clone, Copy)]
pub struct UsageForCost {
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    /// Cached-read input tokens, already included in `prompt_tokens`.
    pub cached_input_tokens: u64,
}

/// Why a cost figure is or is not known. Every disposition other than
/// `known` carries `usd: None`, so an aggregate excludes or flags it rather
/// than silently adding zero to a total.
#[derive(Debug, Clone)]
pub struct CostResult {
    pub usd: Option<f64>,
    /// `known`, `unpriced_model`, `unmetered_local_lane`, `unknown_model`,
    /// or `unknown_usage`.
    pub disposition: &'static str,
    pub rate_basis: Option<String>,
    /// One sentence a report can print next to an unknown.
    pub reason: String,
}

fn is_unmetered(model_id: &str, lane: &str) -> bool {
    lane == "local" || model_id.starts_with("ollama:") || model_id.starts_with("ollama/")
}

/// Price one trial's usage against the catalog. Unknown stays unknown: a
/// model with no rate, a lane with no per-token rate at all, or usage missing
/// either token dimension all return `usd: None` with the reason named.
/// Nothing here falls back to a conservative default rate — this function
/// measures, it does not charge.
pub fn price_usage(
    model_id: Option<&str>,
    usage: UsageForCost,
    catalog: &RateCatalog,
    lane: &str,
) -> CostResult {
    let Some(model_id) = model_id.filter(|id| !id.is_empty()) else {
        return CostResult {
            usd: None,
            disposition: "unknown_model",
            rate_basis: None,
            reason: "the trial records no model id, so no rate can be selected".into(),
        };
    };
    if is_unmetered(model_id, lane) {
        return CostResult {
            usd: None,
            disposition: "unmetered_local_lane",
            rate_basis: None,
            reason: format!(
                "{model_id} ran on the local lane, which bills no metered tokens, so it has no per-token cost"
            ),
        };
    }
    let Some(row) = catalog.models.get(model_id) else {
        return match catalog.deliberately_unpriced.get(model_id) {
            Some(declared) => CostResult {
                usd: None,
                disposition: "unpriced_model",
                rate_basis: None,
                reason: format!("{model_id} is unpriced: {declared}"),
            },
            None => CostResult {
                usd: None,
                disposition: "unknown_model",
                rate_basis: None,
                reason: format!(
                    "{model_id} is absent from the rate catalog, so its cost is unknown"
                ),
            },
        };
    };
    let (Some(prompt), Some(completion)) = (usage.prompt_tokens, usage.completion_tokens) else {
        return CostResult {
            usd: None,
            disposition: "unknown_usage",
            rate_basis: Some(row.rate_basis.clone()),
            reason: format!(
                "{model_id} has a rate but the trial reports no token counts, so its cost is unknown"
            ),
        };
    };
    if usage.cached_input_tokens > prompt {
        return CostResult {
            usd: None,
            disposition: "unknown_usage",
            rate_basis: Some(row.rate_basis.clone()),
            reason: format!(
                "{model_id} reports {} cached-read tokens inside {prompt} prompt tokens, which cannot both be true",
                usage.cached_input_tokens
            ),
        };
    }
    let cached_rate = row
        .cached_input_usd_per_mtok
        .unwrap_or(row.input_usd_per_mtok);
    let uncached = prompt - usage.cached_input_tokens;
    let usd = (uncached as f64 / 1_000_000.0) * row.input_usd_per_mtok
        + (usage.cached_input_tokens as f64 / 1_000_000.0) * cached_rate
        + (completion as f64 / 1_000_000.0) * row.output_usd_per_mtok;
    CostResult {
        usd: Some(usd),
        disposition: "known",
        rate_basis: Some(row.rate_basis.clone()),
        reason: format!(
            "{model_id} priced from the {} catalog rate",
            if row.rate_basis == "operator_placeholder" {
                "placeholder"
            } else {
                "confirmed"
            }
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> RateCatalog {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        load_rate_catalog(&root).expect("bench/rates.json is checked in")
    }

    #[test]
    fn checked_in_catalog_prices_gemini_and_nothing_it_should_not() {
        let cat = catalog();
        assert_eq!(cat.version, "openagents.coder-rate-catalog.2026-08-25");
        assert!(cat.models.contains_key("gemini-3.7-flash"));
        assert!(cat.models.contains_key("ox-alpha"));
        assert!(!cat.models.contains_key("gpt-5.6-luna"));
        assert!(!cat.models.contains_key("glm-5.3-flash"));
        assert!(cat.deliberately_unpriced.contains_key("gpt-5.6-luna"));
    }

    #[test]
    fn priced_model_reproduces_the_typescript_arithmetic() {
        // 6_000 uncached input at $1.25/Mtok + 14_000 cached at $0.10/Mtok
        // + 600 output at $10.00/Mtok = 0.0075 + 0.0014 + 0.006.
        let cost = price_usage(
            Some("gemini-3.7-flash"),
            UsageForCost {
                prompt_tokens: Some(20_000),
                completion_tokens: Some(600),
                cached_input_tokens: 14_000,
            },
            &catalog(),
            "proxy",
        );
        assert_eq!(cost.disposition, "known");
        assert!((cost.usd.unwrap() - 0.0149).abs() < 1e-12);
        assert_eq!(cost.rate_basis.as_deref(), Some("operator_placeholder"));
    }

    #[test]
    fn deliberately_unpriced_and_unknown_models_stay_unknown() {
        let cat = catalog();
        let usage = UsageForCost {
            prompt_tokens: Some(1_000),
            completion_tokens: Some(100),
            cached_input_tokens: 0,
        };
        let luna = price_usage(Some("gpt-5.6-luna"), usage, &cat, "proxy");
        assert_eq!(luna.usd, None);
        assert_eq!(luna.disposition, "unpriced_model");
        let glm = price_usage(Some("glm-5.3-flash"), usage, &cat, "proxy");
        assert_eq!(glm.usd, None);
        assert_eq!(glm.disposition, "unknown_model");
    }

    #[test]
    fn local_lane_is_unmetered_not_unknown() {
        let cost = price_usage(
            Some("gemini-3.7-flash"),
            UsageForCost {
                prompt_tokens: Some(1_000),
                completion_tokens: Some(100),
                cached_input_tokens: 0,
            },
            &catalog(),
            "local",
        );
        assert_eq!(cost.usd, None);
        assert_eq!(cost.disposition, "unmetered_local_lane");
    }

    #[test]
    fn missing_token_counts_leave_a_priced_model_unknown_usage() {
        let cost = price_usage(
            Some("gemini-3.7-flash"),
            UsageForCost {
                prompt_tokens: None,
                completion_tokens: None,
                cached_input_tokens: 0,
            },
            &catalog(),
            "proxy",
        );
        assert_eq!(cost.usd, None);
        assert_eq!(cost.disposition, "unknown_usage");
    }
}
