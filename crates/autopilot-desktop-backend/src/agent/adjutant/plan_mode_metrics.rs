//! Metrics for plan mode signature optimization.

use dsrs::evaluate::FeedbackMetric;
use dsrs::{Example, Prediction};
use serde_json::Value;

use super::plan_mode_signatures::PlanModeSignatureKind;

pub fn score_signature(
    signature: PlanModeSignatureKind,
    example: &Example,
    prediction: &Prediction,
) -> f32 {
    match signature {
        PlanModeSignatureKind::TopicDecomposition => score_topic_decomposition(prediction).score,
        PlanModeSignatureKind::ParallelExploration => {
            score_parallel_exploration(example, prediction).score
        }
        PlanModeSignatureKind::PlanSynthesis => score_plan_synthesis(prediction).score,
        PlanModeSignatureKind::ComplexityClassification => {
            score_complexity_classification(prediction).score
        }
        PlanModeSignatureKind::DeepPlanning => score_deep_planning(prediction).score,
        PlanModeSignatureKind::ResultValidation => score_result_validation(prediction).score,
    }
}

pub fn feedback_signature(
    signature: PlanModeSignatureKind,
    example: &Example,
    prediction: &Prediction,
) -> FeedbackMetric {
    let report = match signature {
        PlanModeSignatureKind::TopicDecomposition => score_topic_decomposition(prediction),
        PlanModeSignatureKind::ParallelExploration => {
            score_parallel_exploration(example, prediction)
        }
        PlanModeSignatureKind::PlanSynthesis => score_plan_synthesis(prediction),
        PlanModeSignatureKind::ComplexityClassification => {
            score_complexity_classification(prediction)
        }
        PlanModeSignatureKind::DeepPlanning => score_deep_planning(prediction),
        PlanModeSignatureKind::ResultValidation => score_result_validation(prediction),
    };

    if report.notes.is_empty() {
        FeedbackMetric::new(report.score, "All checks passed")
    } else {
        FeedbackMetric::new(report.score, report.notes.join("; "))
    }
}

struct MetricReport {
    score: f32,
    notes: Vec<String>,
}

fn score_topic_decomposition(prediction: &Prediction) -> MetricReport {
    let mut score = 0.0;
    let mut notes = Vec::new();

    let topics_value = prediction.get("topics", None);
    let topics = parse_json_array(&topics_value);

    if let Some(items) = topics {
        score += 0.4;
        let count = items.len();
        if (2..=4).contains(&count) {
            score += 0.2;
        } else {
            notes.push(format!("topics count {} not in 2-4", count));
        }

        let mut valid_topics = 0usize;
        let mut valid_patterns = 0usize;

        for item in &items {
            let name_ok = item
                .get("name")
                .and_then(Value::as_str)
                .map_or(false, is_substantive);
            let focus_ok = item
                .get("focus")
                .and_then(Value::as_str)
                .map_or(false, is_substantive);
            if name_ok && focus_ok {
                valid_topics += 1;
            }

            let patterns_ok = item
                .get("patterns")
                .and_then(Value::as_array)
                .map(|patterns| {
                    patterns
                        .iter()
                        .any(|p| p.as_str().map_or(false, is_substantive))
                })
                .unwrap_or(false);
            if patterns_ok {
                valid_patterns += 1;
            }
        }

        if valid_topics == items.len() {
            score += 0.2;
        } else {
            notes.push("missing name/focus in topics".to_string());
        }

        if valid_patterns == items.len() {
            score += 0.2;
        } else {
            notes.push("missing patterns in topics".to_string());
        }
    } else {
        notes.push("topics output is not valid JSON array".to_string());
    }

    MetricReport { score, notes }
}

fn score_parallel_exploration(example: &Example, prediction: &Prediction) -> MetricReport {
    let mut score = 0.0;
    let mut notes = Vec::new();

    let findings = field_string(prediction, "findings");
    if findings.len() >= 40 {
        score += 0.4;
    } else {
        notes.push("findings too short".to_string());
    }

    let files_value = prediction.get("files_examined", None);
    let files = parse_json_array(&files_value)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| item.as_str().map(|s| s.to_string()))
        .collect::<Vec<_>>();

    if !files.is_empty() {
        score += 0.3;
    } else {
        notes.push("files_examined empty or invalid".to_string());
    }

    let topic = example.get("topic", None).as_str().unwrap_or("");
    if !topic.is_empty() && findings.to_lowercase().contains(&topic.to_lowercase()) {
        score += 0.3;
    } else {
        notes.push("findings missing topic hint".to_string());
    }

    MetricReport { score, notes }
}

fn score_plan_synthesis(prediction: &Prediction) -> MetricReport {
    let mut score = 0.0;
    let mut notes = Vec::new();

    let plan = field_string(prediction, "implementation_plan");
    if plan.len() >= 120 {
        score += 0.6;
    } else if plan.len() >= 60 {
        score += 0.4;
        notes.push("implementation_plan could be more detailed".to_string());
    } else {
        notes.push("implementation_plan too short".to_string());
    }

    if plan.contains("\n") || plan.contains("-") || plan.contains("1.") {
        score += 0.4;
    } else {
        notes.push("implementation_plan lacks step structure".to_string());
    }

    MetricReport { score, notes }
}

fn score_complexity_classification(prediction: &Prediction) -> MetricReport {
    let mut score = 0.0;
    let mut notes = Vec::new();

    let complexity = field_string(prediction, "complexity").to_lowercase();
    if matches!(
        complexity.as_str(),
        "low" | "medium" | "high" | "veryhigh" | "very high"
    ) {
        score += 0.4;
    } else {
        notes.push("complexity not in allowed set".to_string());
    }

    let routing = field_string(prediction, "routing_decision");
    if is_substantive(&routing) {
        score += 0.3;
    } else {
        notes.push("routing_decision missing".to_string());
    }

    let reasoning = field_string(prediction, "reasoning");
    if reasoning.len() >= 30 {
        score += 0.3;
    } else {
        notes.push("reasoning too short".to_string());
    }

    MetricReport { score, notes }
}

fn score_deep_planning(prediction: &Prediction) -> MetricReport {
    let mut score = 0.0;
    let mut notes = Vec::new();

    let reasoning = field_string(prediction, "reasoning");
    if reasoning.len() >= 80 {
        score += 0.25;
    } else {
        notes.push("reasoning too short".to_string());
    }

    let strategy = field_string(prediction, "strategy");
    if strategy.len() >= 40 {
        score += 0.25;
    } else {
        notes.push("strategy too short".to_string());
    }

    let plan = field_string(prediction, "implementation_plan");
    if plan.len() >= 120 {
        score += 0.25;
    } else {
        notes.push("implementation_plan too short".to_string());
    }

    let risk = field_string(prediction, "risk_assessment");
    if risk.len() >= 40 {
        score += 0.25;
    } else {
        notes.push("risk_assessment too short".to_string());
    }

    MetricReport { score, notes }
}

fn score_result_validation(prediction: &Prediction) -> MetricReport {
    let mut score = 0.0;
    let mut notes = Vec::new();

    let assessment = field_string(prediction, "quality_assessment");
    if assessment.len() >= 40 {
        score += 0.35;
    } else {
        notes.push("quality_assessment too short".to_string());
    }

    let issues = field_string(prediction, "issues");
    if issues.len() >= 3 {
        score += 0.25;
    } else {
        notes.push("issues missing".to_string());
    }

    let confidence = field_string(prediction, "confidence");
    match confidence.trim().parse::<f32>() {
        Ok(value) if (0.0..=1.0).contains(&value) => {
            score += 0.4;
        }
        _ => notes.push("confidence not in 0..1".to_string()),
    }

    MetricReport { score, notes }
}

fn parse_json_array(value: &Value) -> Option<Vec<Value>> {
    if let Some(arr) = value.as_array() {
        return Some(arr.clone());
    }
    if let Some(text) = value.as_str() {
        serde_json::from_str::<Vec<Value>>(text).ok()
    } else {
        None
    }
}

fn field_string(prediction: &Prediction, field: &str) -> String {
    prediction
        .data
        .get(field)
        .and_then(|value| value.as_str().map(|text| text.to_string()))
        .unwrap_or_else(|| {
            prediction
                .data
                .get(field)
                .map(|value| value.to_string())
                .unwrap_or_default()
        })
}

fn is_substantive(text: &str) -> bool {
    text.trim().len() >= 4
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsrs::{Example, LmUsage, Prediction};
    use std::collections::HashMap;

    #[test]
    fn topic_decomposition_scores_valid_output() {
        let topics = serde_json::json!([
            {"name": "Core", "focus": "Check core flow", "patterns": ["core"]},
            {"name": "UI", "focus": "Inspect UI", "patterns": ["ui"]}
        ]);
        let mut data = HashMap::new();
        data.insert("topics".to_string(), Value::String(topics.to_string()));
        let prediction = Prediction::new(data, LmUsage::default());
        let report = super::score_topic_decomposition(&prediction);
        assert!(report.score >= 0.8);
    }

    #[test]
    fn result_validation_scores_confidence() {
        let mut data = HashMap::new();
        data.insert(
            "quality_assessment".to_string(),
            Value::String("Looks solid.".to_string()),
        );
        data.insert("issues".to_string(), Value::String("None".to_string()));
        data.insert("confidence".to_string(), Value::String("0.72".to_string()));
        let prediction = Prediction::new(data, LmUsage::default());
        let report = super::score_result_validation(&prediction);
        assert!(report.score >= 0.6);
    }

    #[test]
    fn parallel_exploration_scores_topic_hint() {
        let mut example_data = HashMap::new();
        example_data.insert("topic".to_string(), Value::String("Routing".to_string()));
        let example = Example::new(example_data, vec!["topic".to_string()], Vec::new());

        let mut prediction_data = HashMap::new();
        prediction_data.insert(
            "findings".to_string(),
            Value::String("Routing logic lives in planner.rs".to_string()),
        );
        prediction_data.insert(
            "files_examined".to_string(),
            Value::String("[\"planner.rs\"]".to_string()),
        );
        let prediction = Prediction::new(prediction_data, LmUsage::default());

        let report = super::score_parallel_exploration(&example, &prediction);
        assert!(report.score >= 0.6);
    }
}
