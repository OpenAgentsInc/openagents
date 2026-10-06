//! Read-only contribution evidence. Activity, credit, and settlement remain separate.
use serde::{Deserialize, Serialize};

#[cfg(feature = "host")]
pub mod host;

/// A state always states the evidence scope or why that evidence is unavailable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub available: bool,
    pub references: Vec<String>,
    pub scope: String,
}
impl State {
    pub fn unavailable(scope: impl Into<String>) -> Self {
        Self {
            available: false,
            references: Vec::new(),
            scope: scope.into(),
        }
    }
    pub fn observed(references: Vec<String>, scope: impl Into<String>) -> Self {
        Self {
            available: !references.is_empty(),
            references,
            scope: scope.into(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Contribution {
    /// Exact retained source location, separate from the reusable version identity.
    pub source_record: String,
    pub identity: String,
    pub version: String,
    pub digest: String,
    pub author: String,
    pub sources: Vec<String>,
    pub authored: State,
    pub published: State,
    pub installed: State,
    pub invoked: State,
    pub validated: State,
    pub adopted: State,
    pub credited: State,
    pub settled: State,
    pub attempts: Vec<String>,
    pub costs: Vec<String>,
    pub limitations: Vec<String>,
}
impl Contribution {
    /// Bounded pane text contains references and outcomes, never source content.
    pub fn lines(&self) -> String {
        let mut lines = vec![
            format!("{} version {} {}", self.identity, self.version, self.digest),
            format!("Author: {}", self.author),
        ];
        let states = [
            ("Authored", &self.authored),
            ("Published", &self.published),
            ("Installed", &self.installed),
            ("Invoked", &self.invoked),
            ("Externally validated", &self.validated),
            ("Adopted", &self.adopted),
            ("Credited", &self.credited),
            ("Settled", &self.settled),
        ];
        for (label, state) in states {
            lines.push(format!(
                "{label}: {} ({} references)",
                if state.available {
                    "evidence retained"
                } else {
                    "unavailable"
                },
                state.references.len()
            ));
        }
        lines.push(format!(
            "Retained source {}: {} attempts, {} source references",
            self.source_record,
            self.attempts.len(),
            self.sources.len()
        ));
        // Failed or inconclusive evidence and unknown costs precede verbose reference lists.
        for attempt in self
            .attempts
            .iter()
            .filter(|a| {
                a.contains("failed or unknown") || a.contains("Fail") || a.contains("Inconclusive")
            })
            .take(2)
        {
            lines.push(format!("Attempt: {}", brief(attempt, 180)));
        }
        for cost in self.costs.iter().take(4) {
            lines.push(format!("Cost: {}", brief(cost, 120)));
        }
        for limitation in self.limitations.iter().take(2) {
            lines.push(format!("Limit: {}", brief(limitation, 140)));
        }
        for (label, state) in states {
            lines.push(format!("{label} scope: {}", brief(&state.scope, 100)));
            for reference in state.references.iter().take(2) {
                lines.push(format!("Evidence: {reference}"));
            }
        }
        lines.extend(self.sources.iter().map(|s| format!("Source: {s}")));
        lines.extend(self.attempts.iter().map(|s| format!("Attempt: {s}")));
        let text: String = lines
            .join("\n")
            .chars()
            .filter(|c| !c.is_control() || *c == '\n')
            .collect();
        if text.len() <= 2048 {
            return text;
        }
        let mut short = text;
        while short.len() > 1980 {
            short.pop();
        }
        short.push_str("\nEvidence display truncated; retained records remain unchanged.");
        short
    }
}

fn brief(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}
