use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const TARGET_MINIMAL_EDIT_RATE: f32 = 0.60;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskTier {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ThreadCategory {
    Scheduling,
    ReportDelivery,
    FindingsClarification,
    Pricing,
    ComplaintDispute,
    LegalInsurance,
    Other,
}

impl ThreadCategory {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scheduling => "scheduling",
            Self::ReportDelivery => "report_delivery",
            Self::FindingsClarification => "findings_clarification",
            Self::Pricing => "pricing",
            Self::ComplaintDispute => "complaint_dispute",
            Self::LegalInsurance => "legal_insurance",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDecision {
    DraftOnly,
    SendWithApproval,
    Blocked,
}

impl PolicyDecision {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DraftOnly => "draft_only",
            Self::SendWithApproval => "send_with_approval",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DraftStatus {
    Pending,
    Approved,
    Rejected,
    NeedsHuman,
    Sent,
}

impl DraftStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::NeedsHuman => "needs_human",
            Self::Sent => "sent",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClassificationDecision {
    pub category: ThreadCategory,
    pub risk: RiskTier,
    pub policy: PolicyDecision,
    pub reason_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DraftQualitySample {
    pub thread_id: String,
    pub category: ThreadCategory,
    pub generated_draft: String,
    pub sent_reply: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DraftQualitySampleResult {
    pub thread_id: String,
    pub category: ThreadCategory,
    pub edit_ratio: f32,
    pub minimal_edit: bool,
    pub draft_word_count: usize,
    pub sent_word_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DraftQualityCategorySummary {
    pub category: ThreadCategory,
    pub samples: usize,
    pub minimal_edit_count: usize,
    pub minimal_edit_rate: f32,
    pub average_edit_ratio: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DraftQualityReport {
    pub generated_at: DateTime<Utc>,
    pub threshold: f32,
    pub target_rate: f32,
    pub total_samples: usize,
    pub total_minimal_edit: usize,
    pub total_minimal_edit_rate: f32,
    pub target_met: bool,
    pub categories: Vec<DraftQualityCategorySummary>,
    pub samples: Vec<DraftQualitySampleResult>,
}

#[must_use]
pub fn classify_thread(subject: &str, body: &str, snippet: &str) -> ClassificationDecision {
    let text = format!(
        "{}\n{}\n{}",
        subject.to_lowercase(),
        body.to_lowercase(),
        snippet.to_lowercase()
    );
    let mut reason_codes = Vec::new();
    let (category, risk, policy) = if contains_any(
        &text,
        &[
            "lawsuit",
            "attorney",
            "lawyer",
            "insurance",
            "claim",
            "subpoena",
        ],
    ) {
        reason_codes.push("keyword_legal".to_string());
        (
            ThreadCategory::LegalInsurance,
            RiskTier::High,
            PolicyDecision::Blocked,
        )
    } else if contains_any(
        &text,
        &["complaint", "dispute", "angry", "escalate", "unhappy"],
    ) {
        reason_codes.push("keyword_complaint".to_string());
        (
            ThreadCategory::ComplaintDispute,
            RiskTier::High,
            PolicyDecision::DraftOnly,
        )
    } else if contains_any(
        &text,
        &["price", "pricing", "quote", "cost", "invoice", "discount"],
    ) {
        reason_codes.push("keyword_pricing".to_string());
        (
            ThreadCategory::Pricing,
            RiskTier::Medium,
            PolicyDecision::DraftOnly,
        )
    } else if contains_any(
        &text,
        &["schedule", "calendar", "availability", "meet", "reschedule"],
    ) {
        reason_codes.push("keyword_scheduling".to_string());
        (
            ThreadCategory::Scheduling,
            RiskTier::Low,
            PolicyDecision::SendWithApproval,
        )
    } else if contains_any(
        &text,
        &["report", "deliverable", "attached", "results", "summary"],
    ) {
        reason_codes.push("keyword_report_delivery".to_string());
        (
            ThreadCategory::ReportDelivery,
            RiskTier::Low,
            PolicyDecision::SendWithApproval,
        )
    } else if contains_any(
        &text,
        &["clarify", "clarification", "follow up", "question"],
    ) {
        reason_codes.push("keyword_clarification".to_string());
        (
            ThreadCategory::FindingsClarification,
            RiskTier::Medium,
            PolicyDecision::SendWithApproval,
        )
    } else {
        reason_codes.push("fallback_other".to_string());
        (
            ThreadCategory::Other,
            RiskTier::Medium,
            PolicyDecision::DraftOnly,
        )
    };
    ClassificationDecision {
        category,
        risk,
        policy,
        reason_codes,
    }
}

#[must_use]
pub fn compose_local_draft(
    category: ThreadCategory,
    subject: &str,
    inbound_body: &str,
    scheduling_template: Option<&str>,
    report_template: Option<&str>,
    signature: Option<&str>,
    style_signature: &str,
) -> String {
    let base = match category {
        ThreadCategory::Scheduling => scheduling_template
            .map(ToString::to_string)
            .unwrap_or_else(|| {
                "Thanks for reaching out. I can do Tuesday at 10:00 AM PT or Wednesday at 2:00 PM PT. Let me know which works best for you.".to_string()
            }),
        ThreadCategory::ReportDelivery => report_template
            .map(ToString::to_string)
            .unwrap_or_else(|| {
                "Thanks for your note. I have attached the requested report and key findings summary. Please let me know if you want a quick walkthrough call.".to_string()
            }),
        ThreadCategory::FindingsClarification => {
            "Thanks for the follow-up. Happy to clarify. The key point is that the findings reflect the latest data pull and we can review details together on a short call if helpful.".to_string()
        }
        ThreadCategory::Pricing => {
            "Thanks for asking about pricing. I can confirm options once I verify scope and deliverables. I will send a detailed quote shortly.".to_string()
        }
        ThreadCategory::ComplaintDispute => {
            "Thanks for flagging this. I understand the concern and want to resolve it quickly. I’m reviewing details now and will follow up with concrete next steps shortly.".to_string()
        }
        ThreadCategory::LegalInsurance => {
            "Thanks for your message. I have flagged this for manual legal review and will respond after that review is complete.".to_string()
        }
        ThreadCategory::Other => {
            "Thanks for reaching out. I reviewed your message and will get back to you with next steps shortly.".to_string()
        }
    };

    let inbound_excerpt: String = inbound_body.chars().take(220).collect();
    let signature_block = signature
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("\n\n{value}"))
        .unwrap_or_default();

    format!(
        "Subject context: {subject}\n\n{base}\n\nReference: {inbound_excerpt}\n\n{style_signature}{signature_block}"
    )
}

#[must_use]
pub fn infer_style_signature_from_bodies<'a>(
    messages: impl IntoIterator<Item = &'a str>,
) -> String {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    let mut seen = 0usize;
    for body in messages {
        if seen >= 20 {
            break;
        }
        seen += 1;
        let lowercase = body.to_lowercase();
        if lowercase.contains("thanks,") {
            *counts.entry("Thanks,").or_insert(0) += 1;
        }
        if lowercase.contains("best,") {
            *counts.entry("Best,").or_insert(0) += 1;
        }
        if lowercase.contains("regards,") {
            *counts.entry("Regards,").or_insert(0) += 1;
        }
    }
    if seen == 0 {
        return "Best,".to_string();
    }

    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(signoff, _)| signoff.to_string())
        .unwrap_or_else(|| "Best,".to_string())
}

#[must_use]
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

    let mut grouped: HashMap<ThreadCategory, Vec<&DraftQualitySampleResult>> = HashMap::new();
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

#[must_use]
pub fn parse_thread_category(raw: &str) -> Option<ThreadCategory> {
    match raw {
        "scheduling" => Some(ThreadCategory::Scheduling),
        "report_delivery" => Some(ThreadCategory::ReportDelivery),
        "findings_clarification" => Some(ThreadCategory::FindingsClarification),
        "pricing" => Some(ThreadCategory::Pricing),
        "complaint_dispute" => Some(ThreadCategory::ComplaintDispute),
        "legal_insurance" => Some(ThreadCategory::LegalInsurance),
        "other" => Some(ThreadCategory::Other),
        _ => None,
    }
}

#[must_use]
pub fn parse_risk_tier(raw: &str) -> Option<RiskTier> {
    match raw {
        "low" => Some(RiskTier::Low),
        "medium" => Some(RiskTier::Medium),
        "high" => Some(RiskTier::High),
        _ => None,
    }
}

#[must_use]
pub const fn risk_to_str(risk: RiskTier) -> &'static str {
    match risk {
        RiskTier::Low => "low",
        RiskTier::Medium => "medium",
        RiskTier::High => "high",
    }
}

#[must_use]
pub fn parse_policy(raw: &str) -> Option<PolicyDecision> {
    match raw {
        "draft_only" => Some(PolicyDecision::DraftOnly),
        "send_with_approval" => Some(PolicyDecision::SendWithApproval),
        "blocked" => Some(PolicyDecision::Blocked),
        _ => None,
    }
}

#[must_use]
pub fn parse_draft_status(raw: &str) -> Option<DraftStatus> {
    match raw {
        "pending" => Some(DraftStatus::Pending),
        "approved" => Some(DraftStatus::Approved),
        "rejected" => Some(DraftStatus::Rejected),
        "needs_human" => Some(DraftStatus::NeedsHuman),
        "sent" => Some(DraftStatus::Sent),
        _ => None,
    }
}

fn contains_any(text: &str, terms: &[&str]) -> bool {
    terms.iter().any(|term| text.contains(term))
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
            let substitution_cost = usize::from(left_word != right_word);
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
        .map(|value| {
            if value.is_ascii_alphanumeric() || value.is_whitespace() {
                value
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

    #[test]
    fn classification_flags_legal_content_as_blocked() {
        let decision = classify_thread(
            "Need help",
            "Our attorney requested documents for insurance claim",
            "",
        );
        assert_eq!(decision.category, ThreadCategory::LegalInsurance);
        assert_eq!(decision.risk, RiskTier::High);
        assert_eq!(decision.policy, PolicyDecision::Blocked);
    }

    #[test]
    fn classification_flags_scheduling_as_send_with_approval() {
        let decision = classify_thread("Meeting", "Can we schedule a call tomorrow?", "");
        assert_eq!(decision.category, ThreadCategory::Scheduling);
        assert_eq!(decision.risk, RiskTier::Low);
        assert_eq!(decision.policy, PolicyDecision::SendWithApproval);
    }

    #[test]
    fn quality_report_aggregates_categories_and_totals() {
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
    }

    #[test]
    fn style_signature_uses_recent_message_signoff() {
        let bodies = vec!["Thanks,\nChris", "Best,\nAlex", "Thanks,\nTaylor"];
        let signoff = infer_style_signature_from_bodies(bodies);
        assert_eq!(signoff, "Thanks,");
    }
}
