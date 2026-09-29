//! Samples of one phase and their summary.

use std::time::Duration;

/// Every sample of one phase.
#[derive(Clone, Debug)]
pub struct Phase {
    /// The table this phase is printed in.
    pub group: &'static str,
    pub name: String,
    pub samples: Vec<Duration>,
    /// Counts that explain the samples, such as pages read.
    pub note: String,
}

impl Phase {
    pub fn new(group: &'static str, name: impl Into<String>) -> Self {
        Self {
            group,
            name: name.into(),
            samples: vec![],
            note: String::new(),
        }
    }

    pub fn median(&self) -> Option<Duration> {
        percentile(&self.samples, 50)
    }

    pub fn p95(&self) -> Option<Duration> {
        percentile(&self.samples, 95)
    }

    pub fn min(&self) -> Option<Duration> {
        self.samples.iter().min().copied()
    }

    pub fn max(&self) -> Option<Duration> {
        self.samples.iter().max().copied()
    }
}

/// The nearest-rank percentile.
pub fn percentile(samples: &[Duration], p: usize) -> Option<Duration> {
    if samples.is_empty() {
        return None;
    }
    let mut sorted = samples.to_vec();
    sorted.sort();
    let rank = (p * sorted.len()).div_ceil(100).max(1);
    sorted.get(rank - 1).copied()
}

/// Milliseconds with one decimal, or `-`.
pub fn ms(value: Option<Duration>) -> String {
    value.map_or_else(
        || "-".into(),
        |d| format!("{:.1}", d.as_secs_f64() * 1000.0),
    )
}

/// The phases as Markdown tables, one per group, in first-seen order.
pub fn markdown(phases: &[Phase]) -> String {
    let mut groups: Vec<&'static str> = vec![];
    for phase in phases {
        if !groups.contains(&phase.group) {
            groups.push(phase.group);
        }
    }
    let mut out = String::new();
    for group in groups {
        out.push_str(&format!("\n### {group}\n\n"));
        out.push_str("| Phase | n | median ms | p95 ms | min ms | max ms | notes |\n");
        out.push_str("| --- | ---: | ---: | ---: | ---: | ---: | --- |\n");
        for phase in phases.iter().filter(|p| p.group == group) {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                phase.name,
                phase.samples.len(),
                ms(phase.median()),
                ms(phase.p95()),
                ms(phase.min()),
                ms(phase.max()),
                phase.note
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_percentiles() {
        let samples: Vec<Duration> = (1..=20).map(Duration::from_millis).collect();
        assert_eq!(percentile(&samples, 50), Some(Duration::from_millis(10)));
        assert_eq!(percentile(&samples, 95), Some(Duration::from_millis(19)));
        assert_eq!(percentile(&[], 50), None);
        let one = [Duration::from_millis(7)];
        assert_eq!(percentile(&one, 95), Some(Duration::from_millis(7)));
    }
}
