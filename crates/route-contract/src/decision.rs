//! Readings retained at a decision site, joined by request or task identity.

use serde::{Deserialize, Serialize};
use serde_json::Number;

/// A single evaluated probability gate. Numbers retain their JSON precision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionReading {
    pub question: String,
    pub site: String,
    pub question_set: String,
    pub model: String,
    pub option: Option<String>,
    pub raw_probability: Number,
    pub calibrated_probability: Option<Number>,
    /// The actual scalar compared by policy (Choice confidence can differ
    /// from the selected option's probability).
    pub policy_probability: Number,
    pub threshold: Number,
    pub comparison: String,
    pub decision: bool,
    /// The action selected after the gates, when known.
    pub action: Option<String>,
    /// Route request ID, or the task trace's session ID.
    pub outcome_key: String,
}

impl DecisionReading {
    /// A finite probability gate; missing answers are not fabricated readings.
    pub fn new(
        question: &str,
        site: &str,
        model: &str,
        p: f64,
        threshold: f64,
        decision: bool,
    ) -> Option<Self> {
        Some(Self {
            question: question.into(),
            site: site.into(),
            question_set: String::new(),
            model: model.into(),
            option: None,
            raw_probability: Number::from_f64(p)?,
            calibrated_probability: None,
            policy_probability: Number::from_f64(p)?,
            threshold: Number::from_f64(threshold)?,
            comparison: "ge".into(),
            decision,
            action: None,
            outcome_key: String::new(),
        })
    }
}
