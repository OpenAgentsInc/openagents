//! Statistical utilities for significance testing.
//!
//! Provides functions for confidence intervals and hypothesis testing.

/// Result of a t-test.
#[derive(Debug, Clone)]
pub struct TTestResult {
    /// T-statistic.
    pub t_statistic: f64,
    /// P-value (two-tailed).
    pub p_value: f64,
    /// Degrees of freedom.
    pub degrees_of_freedom: f64,
    /// Whether the result is significant at 0.05 level.
    pub significant_05: bool,
    /// Whether the result is significant at 0.01 level.
    pub significant_01: bool,
}

/// Compute the mean of a slice of values.
pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

/// Compute the variance of a slice of values (sample variance).
pub fn variance(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let m = mean(values);
    let sum_sq = values.iter().map(|x| (x - m).powi(2)).sum::<f64>();
    sum_sq / (values.len() - 1) as f64
}

/// Compute the standard deviation of a slice of values.
pub fn std_dev(values: &[f64]) -> f64 {
    variance(values).sqrt()
}

/// Compute the standard error of the mean.
pub fn standard_error(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    std_dev(values) / (values.len() as f64).sqrt()
}

/// Compute a 95% confidence interval for the mean.
///
/// Returns (lower_bound, upper_bound).
pub fn confidence_interval_95(values: &[f64]) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0);
    }

    let m = mean(values);
    let se = standard_error(values);

    // Use t-distribution critical value approximation
    // For n > 30, t ≈ 1.96; for smaller n, use larger values
    let t = if values.len() >= 120 {
        1.96
    } else if values.len() >= 60 {
        2.0
    } else if values.len() >= 30 {
        2.04
    } else if values.len() >= 20 {
        2.09
    } else if values.len() >= 10 {
        2.26
    } else if values.len() >= 5 {
        2.78
    } else {
        3.18 // Very small sample
    };

    (m - t * se, m + t * se)
}

/// Compute a bootstrap confidence interval.
///
/// Uses the percentile method with the specified number of bootstrap samples.
pub fn bootstrap_ci(values: &[f64], n_bootstrap: usize, confidence: f64) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0);
    }

    // Simple pseudo-random number generator (LCG)
    let mut rng_state: u64 = 12345;
    let lcg_next = |state: &mut u64| -> usize {
        *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (*state >> 33) as usize
    };

    let mut bootstrap_means = Vec::with_capacity(n_bootstrap);

    for _ in 0..n_bootstrap {
        // Resample with replacement
        let mut sample_sum = 0.0;
        for _ in 0..values.len() {
            let idx = lcg_next(&mut rng_state) % values.len();
            sample_sum += values[idx];
        }
        bootstrap_means.push(sample_sum / values.len() as f64);
    }

    // Sort and get percentiles
    bootstrap_means.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let alpha = 1.0 - confidence;
    let lower_idx = ((alpha / 2.0) * n_bootstrap as f64).floor() as usize;
    let upper_idx = ((1.0 - alpha / 2.0) * n_bootstrap as f64).ceil() as usize;

    let lower = bootstrap_means.get(lower_idx).copied().unwrap_or(0.0);
    let upper = bootstrap_means.get(upper_idx.min(bootstrap_means.len() - 1)).copied().unwrap_or(0.0);

    (lower, upper)
}

/// Perform a paired t-test between two sets of scores.
///
/// Assumes the scores are paired (same tasks, different methods).
pub fn paired_t_test(scores_a: &[f64], scores_b: &[f64]) -> TTestResult {
    assert_eq!(
        scores_a.len(),
        scores_b.len(),
        "Paired t-test requires equal length arrays"
    );

    if scores_a.is_empty() {
        return TTestResult {
            t_statistic: 0.0,
            p_value: 1.0,
            degrees_of_freedom: 0.0,
            significant_05: false,
            significant_01: false,
        };
    }

    // Compute differences
    let differences: Vec<f64> = scores_a
        .iter()
        .zip(scores_b.iter())
        .map(|(a, b)| a - b)
        .collect();

    let n = differences.len() as f64;
    let d_mean = mean(&differences);
    let d_std = std_dev(&differences);

    if d_std == 0.0 {
        return TTestResult {
            t_statistic: if d_mean == 0.0 { 0.0 } else { f64::INFINITY },
            p_value: if d_mean == 0.0 { 1.0 } else { 0.0 },
            degrees_of_freedom: n - 1.0,
            significant_05: d_mean != 0.0,
            significant_01: d_mean != 0.0,
        };
    }

    // T-statistic
    let t = d_mean / (d_std / n.sqrt());
    let df = n - 1.0;

    // Approximate p-value using normal approximation for large df
    // For small df, this is an approximation
    let p_value = 2.0 * (1.0 - normal_cdf(t.abs()));

    TTestResult {
        t_statistic: t,
        p_value,
        degrees_of_freedom: df,
        significant_05: p_value < 0.05,
        significant_01: p_value < 0.01,
    }
}

/// Perform an independent two-sample t-test (Welch's t-test).
pub fn independent_t_test(scores_a: &[f64], scores_b: &[f64]) -> TTestResult {
    if scores_a.is_empty() || scores_b.is_empty() {
        return TTestResult {
            t_statistic: 0.0,
            p_value: 1.0,
            degrees_of_freedom: 0.0,
            significant_05: false,
            significant_01: false,
        };
    }

    let n1 = scores_a.len() as f64;
    let n2 = scores_b.len() as f64;

    let m1 = mean(scores_a);
    let m2 = mean(scores_b);

    let v1 = variance(scores_a);
    let v2 = variance(scores_b);

    // Welch's t-test
    let se = ((v1 / n1) + (v2 / n2)).sqrt();

    if se == 0.0 {
        return TTestResult {
            t_statistic: if m1 == m2 { 0.0 } else { f64::INFINITY },
            p_value: if m1 == m2 { 1.0 } else { 0.0 },
            degrees_of_freedom: n1 + n2 - 2.0,
            significant_05: m1 != m2,
            significant_01: m1 != m2,
        };
    }

    let t = (m1 - m2) / se;

    // Welch-Satterthwaite degrees of freedom
    let df_num = ((v1 / n1) + (v2 / n2)).powi(2);
    let df_den = ((v1 / n1).powi(2) / (n1 - 1.0)) + ((v2 / n2).powi(2) / (n2 - 1.0));
    let df = df_num / df_den;

    // Approximate p-value
    let p_value = 2.0 * (1.0 - normal_cdf(t.abs()));

    TTestResult {
        t_statistic: t,
        p_value,
        degrees_of_freedom: df,
        significant_05: p_value < 0.05,
        significant_01: p_value < 0.01,
    }
}

/// Approximate the standard normal CDF.
fn normal_cdf(x: f64) -> f64 {
    // Approximation using error function
    0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
}

/// Approximation of the error function.
fn erf(x: f64) -> f64 {
    // Abramowitz and Stegun approximation
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();

    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();

    sign * y
}

/// Effect size (Cohen's d) for two independent samples.
pub fn cohens_d(scores_a: &[f64], scores_b: &[f64]) -> f64 {
    let m1 = mean(scores_a);
    let m2 = mean(scores_b);

    let n1 = scores_a.len() as f64;
    let n2 = scores_b.len() as f64;

    let v1 = variance(scores_a);
    let v2 = variance(scores_b);

    // Pooled standard deviation
    let pooled_var = ((n1 - 1.0) * v1 + (n2 - 1.0) * v2) / (n1 + n2 - 2.0);
    let pooled_std = pooled_var.sqrt();

    if pooled_std == 0.0 {
        return 0.0;
    }

    (m1 - m2) / pooled_std
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mean() {
        assert_eq!(mean(&[1.0, 2.0, 3.0, 4.0, 5.0]), 3.0);
        assert_eq!(mean(&[]), 0.0);
    }

    #[test]
    fn test_std_dev() {
        let values = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let sd = std_dev(&values);
        assert!((sd - 2.14).abs() < 0.1);
    }

    #[test]
    fn test_confidence_interval() {
        let values: Vec<f64> = (0..100).map(|x| x as f64).collect();
        let (lower, upper) = confidence_interval_95(&values);
        let m = mean(&values);

        assert!(lower < m);
        assert!(upper > m);
        assert!(upper - lower < 20.0); // Reasonable CI width
    }

    #[test]
    fn test_paired_t_test_no_difference() {
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let b = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        let result = paired_t_test(&a, &b);
        assert_eq!(result.t_statistic, 0.0);
        assert!(!result.significant_05);
    }

    #[test]
    fn test_paired_t_test_with_difference() {
        let a = vec![10.0, 12.0, 14.0, 16.0, 18.0];
        let b = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        let result = paired_t_test(&a, &b);
        assert!(result.t_statistic > 0.0);
        assert!(result.significant_05);
    }

    #[test]
    fn test_bootstrap_ci() {
        let values: Vec<f64> = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let (lower, upper) = bootstrap_ci(&values, 1000, 0.95);

        assert!(lower < 5.5);
        assert!(upper > 5.5);
    }

    #[test]
    fn test_cohens_d() {
        // Large effect
        let a = vec![10.0, 11.0, 12.0, 13.0, 14.0];
        let b = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        let d = cohens_d(&a, &b);
        assert!(d > 2.0); // Large effect size
    }
}
