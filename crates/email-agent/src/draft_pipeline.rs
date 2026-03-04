use crate::{
    GroundingReference, NormalizedConversationItem, RetrievedContextChunk, StyleProfile, StyleTone,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DraftPolicy {
    pub max_output_chars: usize,
    pub minimum_grounding_refs: usize,
}

impl Default for DraftPolicy {
    fn default() -> Self {
        Self {
            max_output_chars: 1600,
            minimum_grounding_refs: 1,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DraftGenerationInput {
    pub inbound_message: NormalizedConversationItem,
    pub style_profile: StyleProfile,
    pub retrieval_context: Vec<RetrievedContextChunk>,
    pub grounding_references: Vec<GroundingReference>,
    pub policy: DraftPolicy,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DraftArtifact {
    pub draft_id: String,
    pub source_message_id: String,
    pub profile_id: String,
    pub retrieval_context_ids: Vec<String>,
    pub grounding_chunk_ids: Vec<String>,
    pub body: String,
    pub confidence_milli: u32,
    pub rationale: String,
}

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum DraftGenerationError {
    #[error("policy violation: {0}")]
    PolicyViolation(String),
    #[error("invalid draft input: {0}")]
    InvalidInput(String),
}

pub fn generate_draft(input: &DraftGenerationInput) -> Result<DraftArtifact, DraftGenerationError> {
    if input.policy.max_output_chars == 0 {
        return Err(DraftGenerationError::InvalidInput(
            "max_output_chars must be greater than zero".to_string(),
        ));
    }
    if input.grounding_references.len() < input.policy.minimum_grounding_refs {
        return Err(DraftGenerationError::PolicyViolation(format!(
            "requires at least {} grounding references",
            input.policy.minimum_grounding_refs
        )));
    }

    let greeting = greeting_line(
        input.style_profile.preferred_tone,
        input.inbound_message.sender_email.as_str(),
    );
    let core_response = build_core_response(
        input.inbound_message.body_summary.as_str(),
        input.grounding_references.as_slice(),
    );
    let closure = closure_line(input.style_profile.preferred_tone);

    let mut body = format!("{greeting}\n\n{core_response}\n\n{closure}");
    if body.chars().count() > input.policy.max_output_chars {
        body = body.chars().take(input.policy.max_output_chars).collect();
    }

    let retrieval_context_ids = input
        .retrieval_context
        .iter()
        .map(|chunk| chunk.normalized_id.clone())
        .collect::<Vec<String>>();
    let grounding_chunk_ids = input
        .grounding_references
        .iter()
        .map(|reference| reference.chunk_id.clone())
        .collect::<Vec<String>>();

    let confidence_milli = compute_confidence(
        input.retrieval_context.as_slice(),
        input.grounding_references.as_slice(),
        input.style_profile.sample_count,
    );

    let rationale = format!(
        "draft generated with profile={} context={} grounding={} confidence={}m",
        input.style_profile.profile_id,
        retrieval_context_ids.len(),
        grounding_chunk_ids.len(),
        confidence_milli
    );

    Ok(DraftArtifact {
        draft_id: format!(
            "draft:{}:{}",
            input.inbound_message.source_message_id, input.style_profile.profile_id
        ),
        source_message_id: input.inbound_message.source_message_id.clone(),
        profile_id: input.style_profile.profile_id.clone(),
        retrieval_context_ids,
        grounding_chunk_ids,
        body,
        confidence_milli,
        rationale,
    })
}

fn greeting_line(tone: StyleTone, sender_email: &str) -> String {
    match tone {
        StyleTone::Formal => format!("Hello {sender_email},"),
        StyleTone::Neutral => format!("Hi {sender_email},"),
        StyleTone::Friendly => format!("Hey {sender_email},"),
    }
}

fn build_core_response(inbound_summary: &str, grounding_refs: &[GroundingReference]) -> String {
    let top_ref = grounding_refs.first();
    match top_ref {
        Some(reference) => format!(
            "Thanks for your note. Based on our reference ({}) we can proceed with: {}",
            reference.source_uri, inbound_summary
        ),
        None => format!("Thanks for your note. We can proceed with: {inbound_summary}"),
    }
}

fn closure_line(tone: StyleTone) -> &'static str {
    match tone {
        StyleTone::Formal => "Regards,\nAutopilot",
        StyleTone::Neutral => "Thanks,\nAutopilot",
        StyleTone::Friendly => "Thanks so much,\nAutopilot",
    }
}

fn compute_confidence(
    retrieval_context: &[RetrievedContextChunk],
    grounding_references: &[GroundingReference],
    style_samples: usize,
) -> u32 {
    let retrieval_component = (retrieval_context.len() as u32)
        .saturating_mul(140)
        .min(350);
    let grounding_component = (grounding_references.len() as u32)
        .saturating_mul(220)
        .min(440);
    let style_component = (style_samples as u32).saturating_mul(50).min(200);
    retrieval_component
        .saturating_add(grounding_component)
        .saturating_add(style_component)
        .min(1000)
}

#[cfg(test)]
mod tests {
    use super::{DraftGenerationError, DraftGenerationInput, DraftPolicy, generate_draft};
    use crate::{
        GroundingReference, NormalizedConversationItem, RetrievedContextChunk, StyleProfile,
        StyleTone,
    };

    fn inbound_item() -> NormalizedConversationItem {
        NormalizedConversationItem {
            normalized_id: "gmail:m1".to_string(),
            source_message_id: "m1".to_string(),
            thread_id: "thread-1".to_string(),
            subject: "Question".to_string(),
            sender_email: "sender@example.com".to_string(),
            recipient_emails: vec!["team@example.com".to_string()],
            body_summary: "invoice timing and payment steps".to_string(),
            quoted_blocks: Vec::new(),
            signature_block: None,
            timestamp_ms: 1,
            labels: vec!["INBOX".to_string()],
        }
    }

    fn style_profile() -> StyleProfile {
        StyleProfile {
            profile_id: "profile-1".to_string(),
            sample_count: 5,
            average_sentence_words_milli: 8000,
            question_rate_milli: 100,
            exclamation_rate_milli: 50,
            greeting_markers: vec!["hello".to_string()],
            signoff_markers: vec!["thanks".to_string()],
            preferred_tone: StyleTone::Neutral,
        }
    }

    #[test]
    fn draft_pipeline_returns_traceable_artifact() {
        let draft = generate_draft(&DraftGenerationInput {
            inbound_message: inbound_item(),
            style_profile: style_profile(),
            retrieval_context: vec![RetrievedContextChunk {
                source_message_id: "m-hist".to_string(),
                normalized_id: "gmail:m-hist".to_string(),
                thread_id: "thread-hist".to_string(),
                snippet: "historical phrasing".to_string(),
                score_milli: 910,
            }],
            grounding_references: vec![GroundingReference {
                chunk_id: "doc-1:chunk:0000".to_string(),
                document_id: "doc-1".to_string(),
                source_uri: "kb://ops".to_string(),
                snippet: "payment policy".to_string(),
                score_milli: 900,
            }],
            policy: DraftPolicy::default(),
        })
        .expect("draft generation should succeed");

        assert_eq!(draft.draft_id, "draft:m1:profile-1");
        assert_eq!(
            draft.grounding_chunk_ids,
            vec!["doc-1:chunk:0000".to_string()]
        );
        assert!(draft.body.contains("sender@example.com"));
        assert!(draft.rationale.contains("profile=profile-1"));
    }

    #[test]
    fn draft_pipeline_enforces_minimum_grounding_refs() {
        let error = generate_draft(&DraftGenerationInput {
            inbound_message: inbound_item(),
            style_profile: style_profile(),
            retrieval_context: Vec::new(),
            grounding_references: Vec::new(),
            policy: DraftPolicy {
                max_output_chars: 200,
                minimum_grounding_refs: 1,
            },
        })
        .expect_err("missing grounding refs should fail");

        assert_eq!(
            error,
            DraftGenerationError::PolicyViolation(
                "requires at least 1 grounding references".to_string()
            )
        );
    }
}
