//! Results table generator for paper-style output.
//!
//! Generates Table 1 style comparisons across methods and datasets.

use std::collections::HashMap;

/// Aggregated results for a single method-dataset combination.
#[derive(Debug, Clone)]
pub struct AggregatedResult {
    /// Method name.
    pub method: String,
    /// Dataset name.
    pub dataset: String,
    /// Mean score.
    pub mean: f64,
    /// Standard deviation.
    pub std_dev: f64,
    /// Number of samples.
    pub n: usize,
    /// 95% confidence interval (lower, upper).
    pub confidence_95: (f64, f64),
    /// Individual scores for statistical tests.
    pub scores: Vec<f64>,
}

impl AggregatedResult {
    /// Create from a list of scores.
    pub fn from_scores(method: &str, dataset: &str, scores: Vec<f64>) -> Self {
        let n = scores.len();
        let mean = if n > 0 {
            scores.iter().sum::<f64>() / n as f64
        } else {
            0.0
        };

        let std_dev = if n > 1 {
            let variance = scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
            variance.sqrt()
        } else {
            0.0
        };

        // 95% CI using t-distribution approximation (t ≈ 1.96 for large n)
        let se = std_dev / (n as f64).sqrt();
        let t = 1.96; // For 95% CI
        let confidence_95 = (mean - t * se, mean + t * se);

        Self {
            method: method.to_string(),
            dataset: dataset.to_string(),
            mean,
            std_dev,
            n,
            confidence_95,
            scores,
        }
    }

    /// Format as "mean ± std_dev" string.
    pub fn format_mean_std(&self, decimals: usize) -> String {
        format!(
            "{:.prec$}±{:.prec$}",
            self.mean * 100.0,
            self.std_dev * 100.0,
            prec = decimals
        )
    }
}

/// Results table containing all method × dataset results.
#[derive(Debug, Default)]
pub struct ResultsTable {
    /// Method names in order.
    pub methods: Vec<String>,
    /// Dataset names in order.
    pub datasets: Vec<String>,
    /// Results keyed by (method, dataset).
    pub results: HashMap<(String, String), AggregatedResult>,
    /// Best scores per dataset for highlighting.
    best_per_dataset: HashMap<String, f64>,
}

impl ResultsTable {
    /// Create a new empty results table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a result to the table.
    pub fn add_result(&mut self, result: AggregatedResult) {
        // Track methods and datasets
        if !self.methods.contains(&result.method) {
            self.methods.push(result.method.clone());
        }
        if !self.datasets.contains(&result.dataset) {
            self.datasets.push(result.dataset.clone());
        }

        // Track best score per dataset
        let current_best = self.best_per_dataset.get(&result.dataset).copied().unwrap_or(0.0);
        if result.mean > current_best {
            self.best_per_dataset.insert(result.dataset.clone(), result.mean);
        }

        self.results
            .insert((result.method.clone(), result.dataset.clone()), result);
    }

    /// Check if a result is the best for its dataset.
    fn is_best(&self, method: &str, dataset: &str) -> bool {
        if let Some(result) = self.results.get(&(method.to_string(), dataset.to_string())) {
            if let Some(&best) = self.best_per_dataset.get(dataset) {
                return (result.mean - best).abs() < 1e-9;
            }
        }
        false
    }

    /// Generate markdown table output.
    pub fn to_markdown(&self) -> String {
        let mut output = String::new();

        // Header
        output.push_str("| Method |");
        for dataset in &self.datasets {
            output.push_str(&format!(" {} |", dataset));
        }
        output.push('\n');

        // Separator
        output.push_str("|--------|");
        for _ in &self.datasets {
            output.push_str("--------|");
        }
        output.push('\n');

        // Data rows
        for method in &self.methods {
            output.push_str(&format!("| {} |", method));
            for dataset in &self.datasets {
                if let Some(result) = self.results.get(&(method.clone(), dataset.clone())) {
                    let formatted = result.format_mean_std(1);
                    if self.is_best(method, dataset) {
                        output.push_str(&format!(" **{}** |", formatted));
                    } else {
                        output.push_str(&format!(" {} |", formatted));
                    }
                } else {
                    output.push_str(" - |");
                }
            }
            output.push('\n');
        }

        output
    }

    /// Generate LaTeX table output.
    pub fn to_latex(&self) -> String {
        let mut output = String::new();

        let ncols = self.datasets.len() + 1;
        output.push_str(&format!(
            "\\begin{{tabular}}{{l{}}}\n",
            "c".repeat(self.datasets.len())
        ));
        output.push_str("\\toprule\n");

        // Header
        output.push_str("Method");
        for dataset in &self.datasets {
            output.push_str(&format!(" & {}", dataset));
        }
        output.push_str(" \\\\\n");
        output.push_str("\\midrule\n");

        // Data rows
        for method in &self.methods {
            output.push_str(method);
            for dataset in &self.datasets {
                if let Some(result) = self.results.get(&(method.clone(), dataset.clone())) {
                    let formatted = result.format_mean_std(1);
                    if self.is_best(method, dataset) {
                        output.push_str(&format!(" & \\textbf{{{}}}", formatted));
                    } else {
                        output.push_str(&format!(" & {}", formatted));
                    }
                } else {
                    output.push_str(" & -");
                }
            }
            output.push_str(" \\\\\n");
        }

        output.push_str("\\bottomrule\n");
        output.push_str("\\end{tabular}\n");

        output
    }

    /// Generate CSV output.
    pub fn to_csv(&self) -> String {
        let mut output = String::new();

        // Header
        output.push_str("method");
        for dataset in &self.datasets {
            output.push_str(&format!(",{}_mean,{}_std", dataset, dataset));
        }
        output.push('\n');

        // Data rows
        for method in &self.methods {
            output.push_str(method);
            for dataset in &self.datasets {
                if let Some(result) = self.results.get(&(method.clone(), dataset.clone())) {
                    output.push_str(&format!(",{:.4},{:.4}", result.mean, result.std_dev));
                } else {
                    output.push_str(",,");
                }
            }
            output.push('\n');
        }

        output
    }

    /// Generate JSON output for programmatic consumption.
    pub fn to_json(&self) -> String {
        let mut results_json = Vec::new();

        for method in &self.methods {
            let mut method_results = serde_json::Map::new();
            method_results.insert("method".to_string(), serde_json::Value::String(method.clone()));

            for dataset in &self.datasets {
                if let Some(result) = self.results.get(&(method.clone(), dataset.clone())) {
                    let dataset_obj = serde_json::json!({
                        "mean": result.mean,
                        "std_dev": result.std_dev,
                        "n": result.n,
                        "ci_lower": result.confidence_95.0,
                        "ci_upper": result.confidence_95.1,
                    });
                    method_results.insert(dataset.clone(), dataset_obj);
                }
            }

            results_json.push(serde_json::Value::Object(method_results));
        }

        serde_json::to_string_pretty(&results_json).unwrap_or_default()
    }
}

/// Load results from multiple result files and aggregate.
pub fn load_results_from_dir(
    results_dir: &std::path::Path,
) -> Result<ResultsTable, Box<dyn std::error::Error>> {
    use std::fs;

    let mut table = ResultsTable::new();

    for entry in fs::read_dir(results_dir)? {
        let entry = entry?;
        let path = entry.path();

        // Look for summary.json files
        if path.is_file() && path.file_name().map(|n| n == "summary.json").unwrap_or(false) {
            continue; // Skip individual summaries, look in subdirs
        }

        // Look in subdirectories
        if path.is_dir() {
            let summary_path = path.join("summary.json");
            if summary_path.exists() {
                let content = fs::read_to_string(&summary_path)?;
                let json: serde_json::Value = serde_json::from_str(&content)?;

                let method = json["method"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();
                let dataset = json["dataset"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();
                let accuracy = json["accuracy"].as_f64().unwrap_or(0.0);
                let total = json["total"].as_u64().unwrap_or(1) as usize;

                // For single runs, we don't have individual scores
                // In a full implementation, we'd load from trajectories
                let scores = vec![accuracy; total.max(1)];
                table.add_result(AggregatedResult::from_scores(&method, &dataset, scores));
            }
        }
    }

    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregated_result() {
        let scores = vec![0.8, 0.85, 0.78, 0.82, 0.80];
        let result = AggregatedResult::from_scores("RLM", "S-NIAH", scores);

        assert_eq!(result.method, "RLM");
        assert_eq!(result.dataset, "S-NIAH");
        assert_eq!(result.n, 5);
        assert!((result.mean - 0.81).abs() < 0.01);
        assert!(result.std_dev > 0.0);
    }

    #[test]
    fn test_results_table_markdown() {
        let mut table = ResultsTable::new();

        table.add_result(AggregatedResult::from_scores(
            "Base",
            "S-NIAH",
            vec![0.42, 0.45, 0.40],
        ));
        table.add_result(AggregatedResult::from_scores(
            "RLM",
            "S-NIAH",
            vec![0.78, 0.80, 0.76],
        ));
        table.add_result(AggregatedResult::from_scores(
            "Base",
            "CodeQA",
            vec![0.52, 0.55, 0.50],
        ));
        table.add_result(AggregatedResult::from_scores(
            "RLM",
            "CodeQA",
            vec![0.74, 0.76, 0.72],
        ));

        let md = table.to_markdown();
        assert!(md.contains("| Method |"));
        assert!(md.contains("S-NIAH"));
        assert!(md.contains("CodeQA"));
        assert!(md.contains("**")); // Best results should be bold
    }

    #[test]
    fn test_results_table_csv() {
        let mut table = ResultsTable::new();

        table.add_result(AggregatedResult::from_scores(
            "Base",
            "S-NIAH",
            vec![0.42],
        ));

        let csv = table.to_csv();
        assert!(csv.contains("method,S-NIAH_mean,S-NIAH_std"));
        assert!(csv.contains("Base,"));
    }
}
