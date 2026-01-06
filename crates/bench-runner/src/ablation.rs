//! Ablation analysis for comparing RLM vs RLM-NoSubcalls.
//!
//! Quantifies the contribution of recursive `llm_query()` sub-calls.

use bench_harness::stats::{cohens_d, confidence_interval_95, mean, paired_t_test, TTestResult};
use std::collections::HashMap;

/// Results from ablation analysis.
#[derive(Debug, Clone)]
pub struct AblationAnalysis {
    /// Dataset name.
    pub dataset: String,
    /// RLM Full method scores.
    pub rlm_scores: Vec<f64>,
    /// RLM-NoSubcalls method scores.
    pub no_subcalls_scores: Vec<f64>,
    /// Mean improvement (RLM - NoSubcalls).
    pub improvement: f64,
    /// Relative improvement ((RLM - NoSubcalls) / NoSubcalls).
    pub relative_improvement: f64,
    /// Statistical significance test result.
    pub significance: TTestResult,
    /// Effect size (Cohen's d).
    pub effect_size: f64,
}

impl AblationAnalysis {
    /// Create ablation analysis from paired scores.
    pub fn new(dataset: &str, rlm_scores: Vec<f64>, no_subcalls_scores: Vec<f64>) -> Self {
        assert_eq!(
            rlm_scores.len(),
            no_subcalls_scores.len(),
            "Scores must be paired"
        );

        let rlm_mean = mean(&rlm_scores);
        let no_subcalls_mean = mean(&no_subcalls_scores);

        let improvement = rlm_mean - no_subcalls_mean;
        let relative_improvement = if no_subcalls_mean > 0.0 {
            improvement / no_subcalls_mean
        } else {
            0.0
        };

        let significance = paired_t_test(&rlm_scores, &no_subcalls_scores);
        let effect_size = cohens_d(&rlm_scores, &no_subcalls_scores);

        Self {
            dataset: dataset.to_string(),
            rlm_scores,
            no_subcalls_scores,
            improvement,
            relative_improvement,
            significance,
            effect_size,
        }
    }

    /// Get RLM mean with 95% CI.
    pub fn rlm_mean_ci(&self) -> (f64, f64, f64) {
        let m = mean(&self.rlm_scores);
        let (lower, upper) = confidence_interval_95(&self.rlm_scores);
        (m, lower, upper)
    }

    /// Get NoSubcalls mean with 95% CI.
    pub fn no_subcalls_mean_ci(&self) -> (f64, f64, f64) {
        let m = mean(&self.no_subcalls_scores);
        let (lower, upper) = confidence_interval_95(&self.no_subcalls_scores);
        (m, lower, upper)
    }

    /// Interpret effect size.
    pub fn effect_interpretation(&self) -> &'static str {
        let d = self.effect_size.abs();
        if d < 0.2 {
            "negligible"
        } else if d < 0.5 {
            "small"
        } else if d < 0.8 {
            "medium"
        } else {
            "large"
        }
    }
}

/// Collection of ablation analyses across datasets.
#[derive(Debug, Default)]
pub struct AblationReport {
    /// Per-dataset analyses.
    pub analyses: Vec<AblationAnalysis>,
}

impl AblationReport {
    /// Create a new empty report.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an analysis for a dataset.
    pub fn add(&mut self, analysis: AblationAnalysis) {
        self.analyses.push(analysis);
    }

    /// Generate markdown table.
    pub fn to_markdown(&self) -> String {
        let mut output = String::new();

        output.push_str("## Ablation Analysis: RLM vs RLM-NoSubcalls\n\n");
        output.push_str("| Dataset | RLM | RLM-NoSubcalls | Δ | Rel. Δ | p-value | Effect |\n");
        output.push_str("|---------|-----|----------------|---|--------|---------|--------|\n");

        for analysis in &self.analyses {
            let rlm = mean(&analysis.rlm_scores) * 100.0;
            let no_sub = mean(&analysis.no_subcalls_scores) * 100.0;
            let delta = analysis.improvement * 100.0;
            let rel_delta = analysis.relative_improvement * 100.0;
            let p = analysis.significance.p_value;
            let effect = analysis.effect_interpretation();

            let p_str = if p < 0.001 {
                "<0.001".to_string()
            } else {
                format!("{:.3}", p)
            };

            let sig_marker = if analysis.significance.significant_01 {
                "**"
            } else if analysis.significance.significant_05 {
                "*"
            } else {
                ""
            };

            output.push_str(&format!(
                "| {} | {:.1} | {:.1} | {:+.1} | {:+.1}% | {}{} | {} |\n",
                analysis.dataset, rlm, no_sub, delta, rel_delta, p_str, sig_marker, effect
            ));
        }

        output.push_str("\n*p < 0.05, **p < 0.01\n");

        // Summary statistics
        if !self.analyses.is_empty() {
            let avg_improvement: f64 =
                self.analyses.iter().map(|a| a.improvement).sum::<f64>() / self.analyses.len() as f64;
            let avg_rel_improvement: f64 = self.analyses
                .iter()
                .map(|a| a.relative_improvement)
                .sum::<f64>()
                / self.analyses.len() as f64;
            let significant_count = self
                .analyses
                .iter()
                .filter(|a| a.significance.significant_05)
                .count();

            output.push_str(&format!(
                "\n### Summary\n- Average improvement: {:.1}%\n- Average relative improvement: {:.1}%\n- Significant differences: {}/{}\n",
                avg_improvement * 100.0,
                avg_rel_improvement * 100.0,
                significant_count,
                self.analyses.len()
            ));
        }

        output
    }

    /// Generate LaTeX table.
    pub fn to_latex(&self) -> String {
        let mut output = String::new();

        output.push_str("\\begin{table}[t]\n");
        output.push_str("\\centering\n");
        output.push_str("\\caption{Ablation Analysis: Impact of Recursive Sub-calls}\n");
        output.push_str("\\begin{tabular}{lcccccc}\n");
        output.push_str("\\toprule\n");
        output.push_str("Dataset & RLM & RLM-NoSubcalls & $\\Delta$ & Rel. $\\Delta$ & p-value & Effect \\\\\n");
        output.push_str("\\midrule\n");

        for analysis in &self.analyses {
            let rlm = mean(&analysis.rlm_scores) * 100.0;
            let no_sub = mean(&analysis.no_subcalls_scores) * 100.0;
            let delta = analysis.improvement * 100.0;
            let rel_delta = analysis.relative_improvement * 100.0;
            let p = analysis.significance.p_value;
            let effect = analysis.effect_interpretation();

            let p_str = if p < 0.001 {
                "$<$0.001".to_string()
            } else {
                format!("{:.3}", p)
            };

            let sig_marker = if analysis.significance.significant_01 {
                "$^{**}$"
            } else if analysis.significance.significant_05 {
                "$^{*}$"
            } else {
                ""
            };

            output.push_str(&format!(
                "{} & {:.1} & {:.1} & {:+.1} & {:+.1}\\% & {}{} & {} \\\\\n",
                analysis.dataset, rlm, no_sub, delta, rel_delta, p_str, sig_marker, effect
            ));
        }

        output.push_str("\\bottomrule\n");
        output.push_str("\\end{tabular}\n");
        output.push_str("\\label{tab:ablation}\n");
        output.push_str("\\end{table}\n");

        output
    }
}

/// Load ablation data from results directory.
pub fn load_ablation_from_results(
    results_dir: &std::path::Path,
) -> Result<AblationReport, Box<dyn std::error::Error>> {
    use std::fs;

    let mut report = AblationReport::new();
    let mut rlm_results: HashMap<String, Vec<f64>> = HashMap::new();
    let mut no_subcalls_results: HashMap<String, Vec<f64>> = HashMap::new();

    // Scan for result directories
    for entry in fs::read_dir(results_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let dir_name = path.file_name().unwrap().to_string_lossy();
            let summary_path = path.join("summary.json");

            if summary_path.exists() {
                let content = fs::read_to_string(&summary_path)?;
                let json: serde_json::Value = serde_json::from_str(&content)?;

                let method = json["method"].as_str().unwrap_or("");
                let dataset = json["dataset"].as_str().unwrap_or("");
                let accuracy = json["accuracy"].as_f64().unwrap_or(0.0);

                if method.contains("Rlm") && !method.contains("NoSubcalls") {
                    rlm_results
                        .entry(dataset.to_string())
                        .or_default()
                        .push(accuracy);
                } else if method.contains("RlmNoSubcalls") || method.contains("NoSubcalls") {
                    no_subcalls_results
                        .entry(dataset.to_string())
                        .or_default()
                        .push(accuracy);
                }
            }
        }
    }

    // Create analyses for datasets with both methods
    for (dataset, rlm_scores) in rlm_results {
        if let Some(no_subcalls_scores) = no_subcalls_results.get(&dataset) {
            // For proper paired analysis, we'd need per-task scores
            // For now, use available data
            let min_len = rlm_scores.len().min(no_subcalls_scores.len());
            if min_len > 0 {
                let rlm = rlm_scores[..min_len].to_vec();
                let no_sub = no_subcalls_scores[..min_len].to_vec();
                report.add(AblationAnalysis::new(&dataset, rlm, no_sub));
            }
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ablation_analysis() {
        let rlm_scores = vec![0.80, 0.82, 0.78, 0.81, 0.79];
        let no_sub_scores = vec![0.65, 0.68, 0.62, 0.66, 0.64];

        let analysis = AblationAnalysis::new("S-NIAH", rlm_scores, no_sub_scores);

        assert!(analysis.improvement > 0.0);
        assert!(analysis.relative_improvement > 0.0);
        assert!(analysis.significance.significant_05);
    }

    #[test]
    fn test_ablation_report() {
        let mut report = AblationReport::new();

        report.add(AblationAnalysis::new(
            "S-NIAH",
            vec![0.80, 0.82, 0.78],
            vec![0.65, 0.68, 0.62],
        ));

        report.add(AblationAnalysis::new(
            "CodeQA",
            vec![0.75, 0.77, 0.73],
            vec![0.68, 0.70, 0.66],
        ));

        let md = report.to_markdown();
        assert!(md.contains("S-NIAH"));
        assert!(md.contains("CodeQA"));
        assert!(md.contains("Summary"));
    }
}
