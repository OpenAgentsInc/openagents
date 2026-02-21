use crate::config::Config;
use crate::db::{Database, NewDraft, privacy_mode_to_str};
use crate::error::ApiError;
use crate::types::{GenerateDraftResponse, PrivacyMode};
use anyhow::Context;
use autopilot_inbox_domain::{
    PolicyDecision, RiskTier, ThreadCategory, classify_thread, compose_local_draft,
    infer_style_signature_from_bodies, risk_to_str,
};
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
        let style_signature =
            infer_style_signature_from_bodies(recent_sent.iter().map(|item| item.body.as_str()));

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
