use crate::db::DraftQualitySample;
use crate::types::{DraftQualityCategorySummary, DraftQualityReport, DraftQualitySampleResult};
use chrono::Utc;
use std::collections::HashMap;

pub const TARGET_MINIMAL_EDIT_RATE: f32 = 0.60;

pub fn build_draft_quality_report(
    samples: Vec<DraftQualitySample>,
    threshold: f32,
) -> DraftQualityReport {
    let threshold = threshold.clamp(0.05, 1.0);
    let mut results = Vec::with_capacity(samples.len());

    for sample in samples {
        let draft_words = normalize_words(&sample.generated_draft);
        let sent_words = normalize_words(&sample.sent_reply);
        let edit_ratio = word_edit_ratio(&draft_words, &sent_words);
        let minimal_edit = edit_ratio <= threshold;

        results.push(DraftQualitySampleResult {
            thread_id: sample.thread_id,
            category: sample.category,
            edit_ratio,
            minimal_edit,
            draft_word_count: draft_words.len(),
            sent_word_count: sent_words.len(),
        });
    }

    let mut grouped: HashMap<crate::types::ThreadCategory, Vec<&DraftQualitySampleResult>> =
        HashMap::new();
    for result in &results {
        grouped.entry(result.category).or_default().push(result);
    }

    let mut categories: Vec<DraftQualityCategorySummary> = grouped
        .into_iter()
        .map(|(category, group)| {
            let samples = group.len();
            let minimal_edit_count = group.iter().filter(|item| item.minimal_edit).count();
            let avg_edit_ratio = if samples == 0 {
                1.0
            } else {
                group.iter().map(|item| item.edit_ratio).sum::<f32>() / samples as f32
            };
            let minimal_edit_rate = if samples == 0 {
                0.0
            } else {
                minimal_edit_count as f32 / samples as f32
            };

            DraftQualityCategorySummary {
                category,
                samples,
                minimal_edit_count,
                minimal_edit_rate,
                average_edit_ratio: avg_edit_ratio,
            }
        })
        .collect();

    categories.sort_by(|a, b| a.category.as_str().cmp(b.category.as_str()));
    results.sort_by(|a, b| a.thread_id.cmp(&b.thread_id));

    let total_samples = results.len();
    let total_minimal_edit = results.iter().filter(|item| item.minimal_edit).count();
    let total_minimal_edit_rate = if total_samples == 0 {
        0.0
    } else {
        total_minimal_edit as f32 / total_samples as f32
    };

    DraftQualityReport {
        generated_at: Utc::now(),
        threshold,
        target_rate: TARGET_MINIMAL_EDIT_RATE,
        total_samples,
        total_minimal_edit,
        total_minimal_edit_rate,
        target_met: total_minimal_edit_rate >= TARGET_MINIMAL_EDIT_RATE,
        categories,
        samples: results,
    }
}

fn word_edit_ratio(left: &[String], right: &[String]) -> f32 {
    let max_len = left.len().max(right.len());
    if max_len == 0 {
        return 0.0;
    }

    let distance = levenshtein_distance(left, right);
    distance as f32 / max_len as f32
}

fn levenshtein_distance(left: &[String], right: &[String]) -> usize {
    if left.is_empty() {
        return right.len();
    }
    if right.is_empty() {
        return left.len();
    }

    let mut prev: Vec<usize> = (0..=right.len()).collect();
    let mut curr = vec![0usize; right.len() + 1];

    for (left_idx, left_word) in left.iter().enumerate() {
        curr[0] = left_idx + 1;
        for (right_idx, right_word) in right.iter().enumerate() {
            let substitution_cost = if left_word == right_word { 0 } else { 1 };
            let deletion = prev[right_idx + 1] + 1;
            let insertion = curr[right_idx] + 1;
            let substitution = prev[right_idx] + substitution_cost;
            curr[right_idx + 1] = deletion.min(insertion).min(substitution);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[right.len()]
}

fn normalize_words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .chars()
        .map(|char| {
            if char.is_ascii_alphanumeric() || char.is_whitespace() {
                char
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .map(ToString::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ThreadCategory;

    #[test]
    fn report_computes_per_category_and_total_rates() {
        let samples = vec![
            DraftQualitySample {
                thread_id: "thread-a".to_string(),
                category: ThreadCategory::Scheduling,
                generated_draft: "Thanks for reaching out. Tuesday works for us.".to_string(),
                sent_reply: "Thanks for reaching out. Tuesday works for us.".to_string(),
            },
            DraftQualitySample {
                thread_id: "thread-b".to_string(),
                category: ThreadCategory::Scheduling,
                generated_draft: "Can we meet Wednesday afternoon?".to_string(),
                sent_reply: "Could we meet next month?".to_string(),
            },
            DraftQualitySample {
                thread_id: "thread-c".to_string(),
                category: ThreadCategory::ReportDelivery,
                generated_draft: "Attached is the requested report.".to_string(),
                sent_reply: "Attached is the requested report and summary.".to_string(),
            },
        ];

        let report = build_draft_quality_report(samples, 0.35);
        assert_eq!(report.total_samples, 3);
        assert_eq!(report.total_minimal_edit, 2);
        assert!((report.total_minimal_edit_rate - 0.6667).abs() < 0.01);
        assert!(report.target_met);

        let scheduling = report
            .categories
            .iter()
            .find(|entry| entry.category == ThreadCategory::Scheduling)
            .expect("scheduling summary should exist");
        assert_eq!(scheduling.samples, 2);
        assert_eq!(scheduling.minimal_edit_count, 1);
        assert!((scheduling.minimal_edit_rate - 0.5).abs() < 0.001);
    }

    #[test]
    fn report_handles_empty_inputs() {
        let report = build_draft_quality_report(Vec::new(), 0.35);
        assert_eq!(report.total_samples, 0);
        assert_eq!(report.total_minimal_edit, 0);
        assert!((report.total_minimal_edit_rate - 0.0).abs() < 0.001);
        assert!(!report.target_met);
    }
}
