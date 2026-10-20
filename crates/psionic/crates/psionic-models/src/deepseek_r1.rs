//! Text support for the pinned DeepSeek R1 distill template.

use crate::{
    GgufContent, GgufPromptTemplateRenderer, GgufRuntimeTokenizer, GgufTokenizerMetadata,
    GgufTokenizerModel, GgufTokenizerPretokenizer, ParsedReasoningResponse, PromptMessage,
    PromptMessageRole, PromptRenderError, ReasoningParser, TokenId, TokenizerBoundary,
};
use serde::{Deserialize, Serialize};

pub const DEEPSEEK_R1_DISTILL_QWEN_1_5B_REVISION: &str = "ad9f0ae0864d7fbcd1cd905e3c6c5b069cc8b562";
pub const DEEPSEEK_R1_TEMPLATE_SHA256: &str =
    "56a1447ad31926fdc21fb07e56e5642bd9c850c4f52d8c8af7bbe5f079a84f5f";
pub const DEEPSEEK_R1_DISTILL_QWEN_1_5B_GGUF_SHA256: &str =
    "f3bdf9cf31dee4b57ae4e455a1cb0d01b5c2c1b50d72d3112141c195506c2840";

#[derive(Deserialize)]
struct GoldenCase {
    text: String,
    token_ids: Vec<u32>,
    decoded: String,
}

#[derive(Deserialize)]
struct GoldenPrompt {
    name: String,
    messages: Vec<PromptMessage>,
    generation: bool,
    rendered: String,
    token_ids: Vec<u32>,
}

#[derive(Deserialize)]
struct GoldenFixture {
    prompt_cases: Vec<GoldenPrompt>,
    token_cases: Vec<GoldenCase>,
}

fn fixture() -> Result<GoldenFixture, String> {
    serde_json::from_str(include_str!("testdata/deepseek_r1/golden.json"))
        .map_err(|error| error.to_string())
}

/// Token and prompt checks only; this report does not qualify numerical execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeepSeekR1TextQualification {
    pub upstream_revision: String,
    pub template_sha256: String,
    pub token_cases_passed: usize,
    pub prompt_cases_passed: usize,
    pub tool_calling: bool,
}

/// Checks an R1 1.5B GGUF's shape, special tokens, and independent golden prompts.
/// Artifact byte identity must be checked separately before claiming the pinned quantization.
pub fn qualify_deepseek_r1_distill_qwen_1_5b_text(
    content: &GgufContent,
) -> Result<DeepSeekR1TextQualification, String> {
    let metadata = content.metadata();
    if metadata
        .get("general.architecture")
        .and_then(crate::GgufMetadataValue::as_str)
        != Some("qwen2")
    {
        return Err("Expected the R1 distill's Qwen2 architecture.".into());
    }
    for (key, expected) in [
        ("qwen2.block_count", 28),
        ("qwen2.embedding_length", 1536),
        ("qwen2.feed_forward_length", 8960),
        ("qwen2.attention.head_count", 12),
        ("qwen2.attention.head_count_kv", 2),
        ("qwen2.context_length", 131072),
    ] {
        if metadata.get(key).and_then(crate::GgufMetadataValue::as_u64) != Some(expected) {
            return Err(format!("Unexpected R1 distill metadata: {key}."));
        }
    }
    if metadata
        .get("qwen2.rope.freq_base")
        .and_then(crate::GgufMetadataValue::as_f32)
        != Some(10000.0)
    {
        return Err("Unexpected R1 distill RoPE frequency base.".into());
    }
    let tokenizer = content
        .load_tokenizer()
        .map_err(|error| error.to_string())?;
    let templates = content
        .load_chat_templates()
        .map_err(|error| error.to_string())?;
    let renderer = GgufPromptTemplateRenderer::new(tokenizer.clone(), templates);
    check_tokenizer(&tokenizer)?;
    check_goldens(&tokenizer, &renderer)
}

fn check_tokenizer(tokenizer: &GgufTokenizerMetadata) -> Result<(), String> {
    if tokenizer.model != GgufTokenizerModel::Gpt2Bpe
        || tokenizer.pretokenizer != Some(GgufTokenizerPretokenizer::DeepSeekR1Qwen)
        || tokenizer.vocabulary.len() != 151936
        || tokenizer.vocabulary.bos_token_id() != Some(TokenId(151646))
        || tokenizer.vocabulary.eos_token_ids() != [TokenId(151643)]
        || tokenizer.vocabulary.pad_token_id() != Some(TokenId(151643))
        || !tokenizer.add_bos
        || tokenizer.add_eos
    {
        return Err("Unexpected R1 distill tokenizer metadata.".into());
    }
    for (id, expected) in [
        (151643, "<｜end▁of▁sentence｜>"),
        (151644, "<｜User｜>"),
        (151645, "<｜Assistant｜>"),
        (151646, "<｜begin▁of▁sentence｜>"),
        (151648, "<think>"),
        (151649, "</think>"),
    ] {
        if tokenizer.vocabulary.token(TokenId(id)) != Some(expected) {
            return Err(format!("Unexpected R1 distill token {id}."));
        }
    }
    Ok(())
}

fn check_goldens(
    tokenizer: &GgufTokenizerMetadata,
    renderer: &GgufPromptTemplateRenderer,
) -> Result<DeepSeekR1TextQualification, String> {
    let fixture = fixture()?;
    let runtime = GgufRuntimeTokenizer::from_gguf(tokenizer).map_err(|error| error.to_string())?;
    for case in &fixture.token_cases {
        let expected = case
            .token_ids
            .iter()
            .copied()
            .map(TokenId)
            .collect::<Vec<_>>();
        if runtime.encode(&case.text).as_slice() != expected
            || runtime.decode(&expected) != case.decoded
        {
            return Err(format!(
                "R1 tokenizer differs from the reference for {:?}.",
                case.text
            ));
        }
    }
    for case in &fixture.prompt_cases {
        let rendered = renderer
            .render(None, &case.messages, case.generation)
            .map_err(|error| error.to_string())?;
        let expected = case
            .token_ids
            .iter()
            .copied()
            .map(TokenId)
            .collect::<Vec<_>>();
        if rendered.template_digest != DEEPSEEK_R1_TEMPLATE_SHA256
            || rendered.stop_sequences != ["<｜end▁of▁sentence｜>"]
        {
            return Err(format!(
                "R1 template identity or stops differ: {}.",
                case.name
            ));
        }
        if rendered.text != case.rendered {
            return Err(format!(
                "R1 prompt text differs for {}: expected {:?}, got {:?}.",
                case.name, case.rendered, rendered.text
            ));
        }
        let actual = renderer
            .tokenize_rendered_prompt(&rendered.text)
            .map_err(|error| error.to_string())?;
        if actual.as_slice() != expected {
            return Err(format!(
                "R1 prompt tokens differ for {}: expected {:?}, got {:?}.",
                case.name,
                expected,
                actual.as_slice()
            ));
        }
    }
    if !runtime.is_end_of_sequence(TokenId(151643))
        || runtime.is_end_of_sequence(TokenId(151646))
        || runtime.is_end_of_sequence(TokenId(151649))
        || !runtime.decode(&[TokenId(151643)]).is_empty()
        || runtime.decode(&[TokenId(151649)]) != "</think>"
    {
        return Err("R1 EOS or reasoning-marker handling differs from the reference.".into());
    }
    Ok(DeepSeekR1TextQualification {
        upstream_revision: DEEPSEEK_R1_DISTILL_QWEN_1_5B_REVISION.into(),
        template_sha256: DEEPSEEK_R1_TEMPLATE_SHA256.into(),
        token_cases_passed: fixture.token_cases.len(),
        prompt_cases_passed: fixture.prompt_cases.len(),
        tool_calling: false,
    })
}

pub(crate) fn render(
    bos: &str,
    messages: &[PromptMessage],
    generation: bool,
) -> Result<String, PromptRenderError> {
    if bos != "<｜begin▁of▁sentence｜>" {
        return Err(PromptRenderError::InvalidConversation {
            message: "DeepSeek R1 requires its begin-of-sentence token.".into(),
        });
    }
    // The upstream template silently ignores developer turns. Refuse them here.
    if messages.iter().any(|message| {
        matches!(
            message.role,
            PromptMessageRole::Developer | PromptMessageRole::Tool
        ) || message.recipient.is_some()
            || message.channel.is_some()
            || message.content_type.is_some()
    }) {
        return Err(PromptRenderError::InvalidConversation {
            message: "DeepSeek R1 supports text system, user, and assistant messages only.".into(),
        });
    }
    let mut out = bos.to_string();
    // Match the pinned Jinja template: the last system message wins, without trimming.
    if let Some(system) = messages
        .iter()
        .rfind(|message| message.role == PromptMessageRole::System)
    {
        out.push_str(&system.content);
    }
    for message in messages {
        match message.role {
            PromptMessageRole::User => {
                out.push_str("<｜User｜>");
                out.push_str(&message.content);
            }
            PromptMessageRole::Assistant => {
                out.push_str("<｜Assistant｜>");
                out.push_str(
                    message
                        .content
                        .rsplit("</think>")
                        .next()
                        .unwrap_or_default(),
                );
                out.push_str("<｜end▁of▁sentence｜>");
            }
            _ => {}
        }
    }
    if generation {
        out.push_str("<｜Assistant｜><think>\n");
    }
    Ok(out)
}

/// Separates an R1 continuation whose opening `<think>` is already in the prompt.
///
/// Without `</think>`, all generated text remains reasoning, including when the
/// token budget runs out. A partial thought must never become a final answer.
#[must_use]
pub fn parse_deepseek_r1_response_text(text: &str) -> ParsedReasoningResponse {
    let text = text.strip_suffix("<｜end▁of▁sentence｜>").unwrap_or(text);
    let with_prefix = if text.trim_start().starts_with("<think>") {
        text.to_string()
    } else {
        format!("<think>{text}")
    };
    let mut parsed = crate::parse_qwen38_thinking_text(&with_prefix);
    parsed.parser = ReasoningParser::DeepSeekR1;
    // R1's template replays assistant content verbatim. Trimming the generated
    // final text changes the next turn's tokens, even for leading newlines.
    if parsed.final_content.is_some()
        && let Some((_, final_content)) = text.split_once("</think>")
    {
        parsed.final_content = Some(final_content.to_string());
        for part in &mut parsed.parts {
            if part.kind == crate::ReasoningResponsePartKind::Final {
                part.content = final_content.to_string();
            }
        }
    }
    parsed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenizer_fixture() -> Result<GgufTokenizerMetadata, Box<dyn std::error::Error>> {
        #[derive(Deserialize)]
        struct SparseVocabulary {
            vocabulary_size: usize,
            vocabulary: Vec<(usize, String, i32)>,
        }
        let sparse: SparseVocabulary =
            serde_json::from_str(include_str!("testdata/deepseek_r1/golden.json"))?;
        // Omitted ordinary entries cannot participate in any fixture's merges.
        let mut tokens = vec![String::new(); sparse.vocabulary_size];
        let mut token_types = vec![1; sparse.vocabulary_size];
        for (id, text, kind) in sparse.vocabulary {
            tokens[id] = text;
            token_types[id] = kind;
        }
        Ok(GgufTokenizerMetadata {
            model: GgufTokenizerModel::Gpt2Bpe,
            vocabulary: crate::GgufTokenizerVocabulary {
                tokens,
                bos_token_id: Some(TokenId(151646)),
                eos_token_ids: vec![TokenId(151643)],
                pad_token_id: Some(TokenId(151643)),
                unknown_token_id: None,
            },
            scores: Vec::new(),
            token_types,
            merges: Vec::new(),
            add_bos: true,
            add_eos: false,
            pretokenizer: Some(GgufTokenizerPretokenizer::from_gguf_name(
                "deepseek-r1-qwen",
            )),
            token_type_count: None,
            digest: "bounded-golden-ranks".into(),
        })
    }

    #[test]
    fn r1_tokenizer_and_template_match_independent_goldens()
    -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            serde_json::to_string(&ReasoningParser::DeepSeekR1)?,
            "\"deepseek_r1\""
        );
        let tokenizer = tokenizer_fixture()?;
        check_tokenizer(&tokenizer)?;
        let renderer = GgufPromptTemplateRenderer::new(
            tokenizer.clone(),
            crate::GgufChatTemplateMetadata::new(
                Some(include_str!("testdata/deepseek_r1/template.jinja").into()),
                Default::default(),
            ),
        );
        let report = check_goldens(&tokenizer, &renderer)?;
        assert_eq!(report.token_cases_passed, 10);
        assert_eq!(report.prompt_cases_passed, 6);
        assert!(!report.tool_calling);
        let continuation = fixture()?
            .prompt_cases
            .into_iter()
            .find(|case| case.name == "continuation")
            .ok_or("missing continuation fixture")?;
        let mut history = continuation.messages;
        let parsed = parse_deepseek_r1_response_text(&history[1].content);
        history[1].content = parsed.final_content.ok_or("missing final answer")?;
        history[1].reasoning_content = parsed.reasoning_content;
        assert_eq!(
            renderer.render(None, &history, true)?.text,
            continuation.rendered
        );
        let mut wrong = tokenizer.clone();
        wrong.add_bos = false;
        assert!(check_tokenizer(&wrong).is_err());
        wrong = tokenizer.clone();
        wrong.vocabulary.bos_token_id = Some(TokenId(151643));
        assert!(check_tokenizer(&wrong).is_err());
        wrong = tokenizer.clone();
        wrong.pretokenizer = Some(GgufTokenizerPretokenizer::Qwen2);
        assert!(check_tokenizer(&wrong).is_err());
        assert!(check_goldens(&wrong, &renderer).is_err());
        let unsupported = GgufPromptTemplateRenderer::new(
            tokenizer,
            crate::GgufChatTemplateMetadata::new(
                Some("changed template".into()),
                Default::default(),
            ),
        );
        assert!(check_goldens(&wrong, &unsupported).is_err());
        Ok(())
    }

    #[test]
    fn r1_refuses_unqualified_message_features() {
        for message in [
            PromptMessage::new(PromptMessageRole::Developer, "instructions"),
            PromptMessage::new(PromptMessageRole::Tool, "result"),
            PromptMessage::new(PromptMessageRole::Assistant, "call").with_recipient("tool"),
        ] {
            assert!(render("<｜begin▁of▁sentence｜>", &[message], true).is_err());
        }
    }

    #[test]
    fn r1_output_keeps_truncated_thinking_out_of_final_content() {
        for text in ["", "Let me work", "<think>Let me work", "Let me work</thi"] {
            let parsed = parse_deepseek_r1_response_text(text);
            assert_eq!(parsed.parser, ReasoningParser::DeepSeekR1);
            assert_eq!(parsed.final_content, None);
        }
        for text in [
            "work\n</think>\n\n2",
            "<think>work\n</think>\n\n2<｜end▁of▁sentence｜>",
        ] {
            let parsed = parse_deepseek_r1_response_text(text);
            assert_eq!(parsed.reasoning_content.as_deref(), Some("work"));
            assert_eq!(parsed.final_content.as_deref(), Some("\n\n2"));
        }
    }
}
