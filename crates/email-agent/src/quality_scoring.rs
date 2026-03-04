use crate::StyleTone;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct QualityCase {
    pub case_id: String,
    pub generated_body: String,
    pub expected_tone: StyleTone,
    pub required_grounding_terms: Vec<String>,
    pub required_action_terms: Vec<String>,
    pub forbidden_terms: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct QualityThresholds {
    pub min_case_overall_milli: u32,
    pub min_dimension_milli: u32,
    pub min_corpus_average_milli: u32,
}

impl Default for QualityThresholds {
    fn default() -> Self {
        Self {
            min_case_overall_milli: 740,
            min_dimension_milli: 550,
            min_corpus_average_milli: 780,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct QualityDimensionScores {
    pub tone_match_milli: u32,
    pub factual_grounding_milli: u32,
    pub clarity_milli: u32,
    pub actionability_milli: u32,
    pub safety_milli: u32,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct QualityCaseScore {
    pub case_id: String,
    pub dimensions: QualityDimensionScores,
    pub overall_milli: u32,
    pub passed_gate: bool,
    pub failed_dimensions: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct QualityEvaluationReport {
    pub case_scores: Vec<QualityCaseScore>,
    pub corpus_average_milli: u32,
    pub passed_gate: bool,
    pub failures: Vec<String>,
}

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum QualityGateError {
    #[error("quality gate failed: {0}")]
    Failed(String),
}

pub fn evaluate_quality_corpus(
    cases: &[QualityCase],
    thresholds: &QualityThresholds,
) -> QualityEvaluationReport {
    if cases.is_empty() {
        return QualityEvaluationReport {
            case_scores: Vec::new(),
            corpus_average_milli: 0,
            passed_gate: false,
            failures: vec!["quality corpus is empty".to_string()],
        };
    }

    let mut case_scores = Vec::<QualityCaseScore>::with_capacity(cases.len());
    let mut total_overall = 0u32;
    let mut failures = Vec::<String>::new();

    for case in cases {
        let dimensions = score_case_dimensions(case);
        let overall_milli = dimensions
            .tone_match_milli
            .saturating_add(dimensions.factual_grounding_milli)
            .saturating_add(dimensions.clarity_milli)
            .saturating_add(dimensions.actionability_milli)
            .saturating_add(dimensions.safety_milli)
            / 5;
        total_overall = total_overall.saturating_add(overall_milli);

        let mut failed_dimensions = Vec::<String>::new();
        for (label, value) in [
            ("tone_match", dimensions.tone_match_milli),
            ("factual_grounding", dimensions.factual_grounding_milli),
            ("clarity", dimensions.clarity_milli),
            ("actionability", dimensions.actionability_milli),
            ("safety", dimensions.safety_milli),
        ] {
            if value < thresholds.min_dimension_milli {
                failed_dimensions.push(label.to_string());
            }
        }

        let passed_case = overall_milli >= thresholds.min_case_overall_milli
            && failed_dimensions.is_empty();
        if !passed_case {
            failures.push(format!(
                "case {} failed overall={} failed_dimensions={:?}",
                case.case_id, overall_milli, failed_dimensions
            ));
        }

        case_scores.push(QualityCaseScore {
            case_id: case.case_id.clone(),
            dimensions,
            overall_milli,
            passed_gate: passed_case,
            failed_dimensions,
        });
    }

    let corpus_average_milli = total_overall / (cases.len() as u32);
    if corpus_average_milli < thresholds.min_corpus_average_milli {
        failures.push(format!(
            "corpus average {} below threshold {}",
            corpus_average_milli, thresholds.min_corpus_average_milli
        ));
    }

    QualityEvaluationReport {
        case_scores,
        corpus_average_milli,
        passed_gate: failures.is_empty(),
        failures,
    }
}

pub fn enforce_quality_gate(
    cases: &[QualityCase],
    thresholds: &QualityThresholds,
) -> Result<QualityEvaluationReport, QualityGateError> {
    let report = evaluate_quality_corpus(cases, thresholds);
    if report.passed_gate {
        return Ok(report);
    }

    Err(QualityGateError::Failed(report.failures.join("; ")))
}

fn score_case_dimensions(case: &QualityCase) -> QualityDimensionScores {
    QualityDimensionScores {
        tone_match_milli: score_tone_match(case),
        factual_grounding_milli: score_required_terms(
            case.generated_body.as_str(),
            case.required_grounding_terms.as_slice(),
        ),
        clarity_milli: score_clarity(case.generated_body.as_str()),
        actionability_milli: score_actionability(case),
        safety_milli: score_safety(case),
    }
}

fn score_tone_match(case: &QualityCase) -> u32 {
    let observed = infer_tone(case.generated_body.as_str());
    match (observed, case.expected_tone) {
        (left, right) if left == right => 1000,
        (StyleTone::Neutral, StyleTone::Formal) | (StyleTone::Neutral, StyleTone::Friendly) => 700,
        (StyleTone::Formal, StyleTone::Neutral) | (StyleTone::Friendly, StyleTone::Neutral) => 700,
        _ => 450,
    }
}

fn infer_tone(text: &str) -> StyleTone {
    let lowered = text.to_ascii_lowercase();
    if lowered.contains("regards") || lowered.contains("sincerely") {
        return StyleTone::Formal;
    }
    if lowered.contains("hey") || lowered.contains("thanks so much") || lowered.contains('!') {
        return StyleTone::Friendly;
    }
    StyleTone::Neutral
}

fn score_required_terms(text: &str, required_terms: &[String]) -> u32 {
    if required_terms.is_empty() {
        return 1000;
    }
    let lowered = text.to_ascii_lowercase();
    let matched = required_terms
        .iter()
        .filter(|term| lowered.contains(term.to_ascii_lowercase().as_str()))
        .count();
    ((matched as u32).saturating_mul(1000)) / (required_terms.len() as u32)
}

fn score_clarity(text: &str) -> u32 {
    let sentences = text
        .split_terminator(['.', '?', '!'])
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<&str>>();
    if sentences.is_empty() {
        return 0;
    }

    let total_words = sentences
        .iter()
        .map(|sentence| sentence.split_whitespace().count())
        .sum::<usize>();
    let average_words = (total_words as u32) / (sentences.len() as u32);
    let target = 15u32;
    let distance = average_words.abs_diff(target);
    let readability_penalty = distance.saturating_mul(35).min(650);

    1000u32.saturating_sub(readability_penalty)
}

fn score_actionability(case: &QualityCase) -> u32 {
    if !case.required_action_terms.is_empty() {
        return score_required_terms(
            case.generated_body.as_str(),
            case.required_action_terms.as_slice(),
        );
    }

    let lowered = case.generated_body.to_ascii_lowercase();
    let has_action_signal = ["please", "next step", "can you", "confirm", "share"]
        .iter()
        .any(|marker| lowered.contains(marker));
    if has_action_signal { 1000 } else { 600 }
}

fn score_safety(case: &QualityCase) -> u32 {
    let lowered = case.generated_body.to_ascii_lowercase();
    let mut score = 1000u32;

    for forbidden in &case.forbidden_terms {
        if lowered.contains(forbidden.to_ascii_lowercase().as_str()) {
            score = score.saturating_sub(350);
        }
    }
    for risky in ["guarantee", "always", "never fail"] {
        if lowered.contains(risky) {
            score = score.saturating_sub(120);
        }
    }

    score
}

#[cfg(test)]
mod tests {
    use super::{QualityCase, QualityThresholds, enforce_quality_gate, evaluate_quality_corpus};
    use crate::StyleTone;

    fn golden_corpus() -> Vec<QualityCase> {
        vec![
            QualityCase {
                case_id: "case-ops-1".to_string(),
                generated_body: "Hello ops@example.com,\n\nThanks for your note. Based on payment policy v3 and invoice steps, please confirm the invoice id and expected settlement date.\n\nRegards,\nAutopilot".to_string(),
                expected_tone: StyleTone::Formal,
                required_grounding_terms: vec![
                    "payment policy".to_string(),
                    "invoice".to_string(),
                ],
                required_action_terms: vec!["please confirm".to_string()],
                forbidden_terms: vec!["password".to_string()],
            },
            QualityCase {
                case_id: "case-support-2".to_string(),
                generated_body: "Hi team,\n\nUsing the support runbook and escalation matrix, the next step is to share logs and confirm the affected thread id.\n\nThanks,\nAutopilot".to_string(),
                expected_tone: StyleTone::Neutral,
                required_grounding_terms: vec![
                    "support runbook".to_string(),
                    "escalation matrix".to_string(),
                ],
                required_action_terms: vec!["share logs".to_string()],
                forbidden_terms: vec!["secret key".to_string()],
            },
            QualityCase {
                case_id: "case-followup-3".to_string(),
                generated_body: "Hey there!\n\nFollowing the follow-up rulebook and business-hour policy, can you confirm whether Tuesday 10:00 UTC works for a reminder?\n\nThanks so much,\nAutopilot".to_string(),
                expected_tone: StyleTone::Friendly,
                required_grounding_terms: vec![
                    "follow-up rulebook".to_string(),
                    "business-hour policy".to_string(),
                ],
                required_action_terms: vec!["confirm".to_string()],
                forbidden_terms: vec!["mnemonic".to_string()],
            },
        ]
    }

    #[test]
    fn quality_scores_are_deterministic_for_same_corpus() {
        let thresholds = QualityThresholds::default();
        let corpus = golden_corpus();
        let left = evaluate_quality_corpus(corpus.as_slice(), &thresholds);
        let right = evaluate_quality_corpus(corpus.as_slice(), &thresholds);
        assert_eq!(left, right);
    }

    #[test]
    fn quality_gate_thresholds_hold_for_golden_set() {
        let thresholds = QualityThresholds::default();
        let report = enforce_quality_gate(golden_corpus().as_slice(), &thresholds)
            .expect("golden corpus should pass gate");
        assert!(report.passed_gate);
        assert!(report.corpus_average_milli >= thresholds.min_corpus_average_milli);
    }

    #[test]
    fn quality_gate_fails_when_grounding_or_safety_drop() {
        let mut degraded = golden_corpus();
        degraded[0].generated_body = "Hey!\nNo grounding here. It will always work.".to_string();

        let thresholds = QualityThresholds::default();
        let report = evaluate_quality_corpus(degraded.as_slice(), &thresholds);
        assert!(!report.passed_gate);
        assert!(!report.failures.is_empty());
    }
}
