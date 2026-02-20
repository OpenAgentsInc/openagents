use crate::config::Config;
use crate::db::{ClassificationDecision, Database, NewDraft, privacy_mode_to_str};
use crate::error::ApiError;
use crate::types::{GenerateDraftResponse, PolicyDecision, PrivacyMode, RiskTier, ThreadCategory};
use anyhow::Context;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Clone)]
pub struct DraftPipeline {
    http: reqwest::Client,
    config: Config,
}

impl DraftPipeline {
    pub fn new(config: Config) -> Self {
        Self {
            http: reqwest::Client::new(),
            config,
        }
    }

    pub async fn run_for_thread(
        &self,
        db: &Database,
        thread_id: &str,
    ) -> Result<GenerateDraftResponse, ApiError> {
        let detail = db
            .get_thread_detail(thread_id)
            .map_err(ApiError::internal)?
            .ok_or_else(|| ApiError::NotFound(format!("thread {thread_id} not found")))?;
        let inbound = db
            .latest_inbound_message(thread_id)
            .map_err(ApiError::internal)?
            .ok_or_else(|| ApiError::BadRequest("thread has no inbound message".to_string()))?;

        let decision = classify_thread(&detail.thread.subject, &inbound.body, &inbound.snippet);
        db.apply_classification(thread_id, &decision)
            .map_err(ApiError::internal)?;

        let similar_thread_ids = db
            .similar_threads(thread_id, decision.category)
            .map_err(ApiError::internal)?;

        let settings = db.settings().map_err(ApiError::internal)?;
        let recent_sent = db.recent_sent_messages(40).map_err(ApiError::internal)?;
        let style_signature = infer_style_signature(&recent_sent);

        let local_body = compose_local_draft(
            decision.category,
            &detail.thread.subject,
            &inbound.body,
            settings.template_scheduling.as_deref(),
            settings.template_report_delivery.as_deref(),
            settings.signature.as_deref(),
            &style_signature,
        );

        let (final_body, external_model_used, model_used) = match settings.privacy_mode {
            PrivacyMode::LocalOnly => (local_body, false, None),
            PrivacyMode::Hybrid | PrivacyMode::Cloud => {
                if let Some(token) = db
                    .get_provider_token("chatgpt")
                    .map_err(ApiError::internal)?
                    .and_then(|token| token.access_token)
                {
                    match self
                        .rewrite_with_openai(&token, &local_body, decision.category, decision.risk)
                        .await
                    {
                        Ok(rewrite) => (rewrite, true, Some(self.config.openai_model.clone())),
                        Err(err) => {
                            tracing::warn!(
                                "openai rewrite failed, falling back local draft: {}",
                                err
                            );
                            (local_body, false, None)
                        }
                    }
                } else {
                    (local_body, false, None)
                }
            }
        };

        db.set_thread_draft_metadata(thread_id, &similar_thread_ids, external_model_used)
            .map_err(ApiError::internal)?;

        let draft = db
            .create_draft(NewDraft {
                thread_id: thread_id.to_string(),
                body: final_body,
                source_summary: format!(
                    "category={} risk={} similar_threads={} privacy_mode={}",
                    decision.category.as_str(),
                    risk_to_str(decision.risk),
                    similar_thread_ids.len(),
                    privacy_mode_to_str(settings.privacy_mode),
                ),
                model_used,
            })
            .map_err(ApiError::internal)?;

        Ok(GenerateDraftResponse {
            draft,
            category: decision.category,
            risk: decision.risk,
            policy: decision.policy,
        })
    }

    async fn rewrite_with_openai(
        &self,
        api_key: &str,
        local_draft: &str,
        category: ThreadCategory,
        risk: RiskTier,
    ) -> anyhow::Result<String> {
        let endpoint = format!("{}/chat/completions", self.config.openai_base_url);
        let system_prompt = "You rewrite professional email drafts. Preserve intent, facts, and safety constraints. Keep concise and clear.";
        let user_prompt = format!(
            "Category: {}\nRisk: {}\nRewrite this draft to be polished and concise.\n\n{}",
            category.as_str(),
            risk_to_str(risk),
            local_draft
        );

        let response = self
            .http
            .post(endpoint)
            .header("Authorization", format!("Bearer {api_key}"))
            .json(&json!({
                "model": self.config.openai_model,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_prompt}
                ],
                "temperature": 0.2
            }))
            .send()
            .await
            .context("failed calling OpenAI")?;

        if !response.status().is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<no body>".to_string());
            anyhow::bail!("openai call failed: {body}");
        }

        let body: OpenAiChatResponse = response.json().await.context("invalid openai response")?;
        let content = body
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone())
            .filter(|text| !text.trim().is_empty())
            .context("openai returned empty content")?;
        Ok(content)
    }
}

fn classify_thread(subject: &str, body: &str, snippet: &str) -> ClassificationDecision {
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

fn contains_any(text: &str, terms: &[&str]) -> bool {
    terms.iter().any(|term| text.contains(term))
}

fn compose_local_draft(
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

fn infer_style_signature(messages: &[crate::types::MessageRecord]) -> String {
    if messages.is_empty() {
        return "Best,".to_string();
    }

    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for message in messages.iter().take(20) {
        let lowercase = message.body.to_lowercase();
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

    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(signoff, _)| signoff.to_string())
        .unwrap_or_else(|| "Best,".to_string())
}

fn risk_to_str(risk: RiskTier) -> &'static str {
    match risk {
        RiskTier::Low => "low",
        RiskTier::Medium => "medium",
        RiskTier::High => "high",
    }
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessage {
    content: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_legal_as_high_risk_blocked() {
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
    fn classifies_scheduling_as_low_risk() {
        let decision = classify_thread("Meeting", "Can we schedule a call tomorrow?", "");
        assert_eq!(decision.category, ThreadCategory::Scheduling);
        assert_eq!(decision.risk, RiskTier::Low);
        assert_eq!(decision.policy, PolicyDecision::SendWithApproval);
    }
}
