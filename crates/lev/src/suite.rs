//! A frozen, labelled suite.
//!
//! The items are authored in this repository and carry a content digest, so
//! a calibration record can name exactly what it was fitted on. Splits are
//! fixed in the file rather than drawn at run time: fitting a map and scoring
//! it on the same items produces a number that means nothing.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::api::Question;

/// One labelled item.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Item {
    /// A stable id.
    pub id: String,
    /// The question family, which a calibration map covers.
    pub family: String,
    /// The question type.
    pub kind: String,
    /// The document to judge.
    pub state: Value,
    /// The question to ask about it.
    pub question: Question,
    /// The option key a knowledgeable person picks.
    pub truth: String,
    /// `calibration` or `evaluation`.
    pub split: String,
}

/// A suite of labelled items.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Suite {
    /// The suite's name.
    pub name: String,
    /// What it covers and where the labels come from.
    pub description: String,
    /// When it was authored.
    pub created: String,
    /// The digest recorded in the file.
    pub digest: String,
    /// The items.
    pub items: Vec<Item>,
}

impl Suite {
    /// Loads a suite from JSON and checks its digest.
    pub fn load(text: &str) -> Result<Self, String> {
        let suite: Self = serde_json::from_str(text).map_err(|error| error.to_string())?;
        let computed = suite.compute_digest()?;
        if computed != suite.digest {
            return Err(format!(
                "the suite's digest does not match its items: recorded {}, computed {computed}",
                suite.digest
            ));
        }
        Ok(suite)
    }

    /// The digest over the items, as the generator writes it.
    pub fn compute_digest(&self) -> Result<String, String> {
        let canonical =
            serde_json::to_string(&self.items).map_err(|error| error.to_string())?;
        // The generator writes compact, key-sorted JSON.
        let value: Value = serde_json::from_str(&canonical).map_err(|error| error.to_string())?;
        let sorted = canonicalize(&value);
        let mut hasher = Sha256::new();
        hasher.update(sorted.as_bytes());
        Ok(format!("{:x}", hasher.finalize()))
    }

    /// The items in one split.
    pub fn split(&self, split: &str) -> impl Iterator<Item = &Item> {
        self.items.iter().filter(move |item| item.split == split)
    }

    /// The families this suite covers, in first-seen order.
    #[must_use]
    pub fn families(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for item in &self.items {
            if !out.contains(&item.family) {
                out.push(item.family.clone());
            }
        }
        out
    }
}

/// Serializes with object keys sorted, which is what the digest is over.
fn canonicalize(value: &Value) -> String {
    match value {
        Value::Object(fields) => {
            let mut keys: Vec<&String> = fields.keys().collect();
            keys.sort();
            let inner: Vec<String> = keys
                .iter()
                .map(|key| format!("{}:{}", serde_json::to_string(key).unwrap_or_default(), canonicalize(&fields[*key])))
                .collect();
            format!("{{{}}}", inner.join(","))
        }
        Value::Array(items) => {
            let inner: Vec<String> = items.iter().map(canonicalize).collect();
            format!("[{}]", inner.join(","))
        }
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUITE: &str = include_str!("../suites/support-v2.json");

    #[test]
    fn the_shipped_suite_loads_and_its_digest_matches() {
        let suite = Suite::load(SUITE).expect("the suite loads");
        assert_eq!(suite.name, "support-v2");
        assert_eq!(suite.items.len(), 196);
    }

    #[test]
    fn the_splits_are_disjoint_and_both_populated() {
        let suite = Suite::load(SUITE).expect("the suite loads");
        let calibration = suite.split("calibration").count();
        let evaluation = suite.split("evaluation").count();
        // Enough per family to fit a table that is worth fitting.
        assert!(calibration >= 90, "calibration split holds {calibration}");
        assert!(evaluation >= 90, "evaluation split holds {evaluation}");
        assert_eq!(calibration + evaluation, suite.items.len());
    }

    #[test]
    fn every_item_names_a_truth_its_question_admits() {
        let suite = Suite::load(SUITE).expect("the suite loads");
        for item in &suite.items {
            let compiled = crate::schema::compile(&crate::api::SystemOneRequest {
                state: item.state.clone(),
                model: None,
                questions: [(item.id.clone(), item.question.clone())].into_iter().collect(),
                extensions: crate::api::Extensions::default(),
            })
            .expect("the item compiles");
            let options = &compiled[&item.id].options;
            assert!(
                options.contains(&item.truth),
                "{} names truth '{}' outside its options {options:?}",
                item.id,
                item.truth
            );
        }
    }

    #[test]
    fn a_tampered_suite_is_refused() {
        let tampered = SUITE.replace("support-v2", "support-v2 ").replacen(
            "\"truth\": \"billing\"",
            "\"truth\": \"sales\"",
            1,
        );
        assert!(Suite::load(&tampered).is_err());
    }
}
