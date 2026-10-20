//! Text renderers for pinned public GGUF chat templates.

use crate::{GgufPromptTemplateFamily, PromptMessage, PromptMessageRole, PromptRenderError};

fn invalid(message: &str) -> PromptRenderError {
    PromptRenderError::InvalidConversation {
        message: message.into(),
    }
}

pub(crate) fn render(
    family: GgufPromptTemplateFamily,
    bos: &str,
    messages: &[PromptMessage],
    generation: bool,
) -> Result<String, PromptRenderError> {
    if messages.is_empty() {
        return Err(invalid("Send at least one message."));
    }
    if messages.iter().any(|m| m.role == PromptMessageRole::Tool) {
        return Err(invalid(
            "This chat template does not support tool-result messages yet.",
        ));
    }
    match family {
        GgufPromptTemplateFamily::DeepSeekR1 => {
            crate::deepseek_r1::render(bos, messages, generation)
        }
        GgufPromptTemplateFamily::Llama3 => llama3(bos, messages, generation),
        GgufPromptTemplateFamily::MistralV3 => mistral(bos, messages),
        GgufPromptTemplateFamily::Gemma4System => gemma4(bos, messages, generation),
        GgufPromptTemplateFamily::Qwen3Chat => qwen3(messages, generation),
        _ => Err(invalid(
            "This renderer does not support the selected template.",
        )),
    }
}

fn llama3(
    bos: &str,
    messages: &[PromptMessage],
    generation: bool,
) -> Result<String, PromptRenderError> {
    let mut out = format!(
        "{bos}<|start_header_id|>system<|end_header_id|>\n\nCutting Knowledge Date: December 2023\nToday Date: 26 Jul 2024\n\n"
    );
    let start = usize::from(messages[0].role == PromptMessageRole::System);
    if start == 1 {
        out.push_str(messages[0].content.trim());
    }
    out.push_str("<|eot_id|>");
    for message in &messages[start..] {
        out.push_str("<|start_header_id|>");
        out.push_str(message.role.as_str());
        out.push_str("<|end_header_id|>\n\n");
        out.push_str(message.content.trim());
        out.push_str("<|eot_id|>");
    }
    if generation {
        out.push_str("<|start_header_id|>assistant<|end_header_id|>\n\n");
    }
    Ok(out)
}

fn mistral(bos: &str, messages: &[PromptMessage]) -> Result<String, PromptRenderError> {
    let mut out = bos.to_string();
    for (index, message) in messages.iter().enumerate() {
        let expected = if index % 2 == 0 {
            PromptMessageRole::User
        } else {
            PromptMessageRole::Assistant
        };
        if message.role != expected {
            return Err(invalid(
                "Mistral v0.3 requires alternating user and assistant messages.",
            ));
        }
        if expected == PromptMessageRole::User {
            out.push_str("[INST] ");
            out.push_str(&message.content);
            out.push_str(" [/INST]");
        } else {
            out.push_str(&message.content);
            out.push_str("</s>");
        }
    }
    Ok(out)
}

fn gemma4(
    bos: &str,
    messages: &[PromptMessage],
    generation: bool,
) -> Result<String, PromptRenderError> {
    let mut out = bos.to_string();
    let mut previous = None;
    for (index, message) in messages.iter().enumerate() {
        let role = match message.role {
            PromptMessageRole::System | PromptMessageRole::Developer if index == 0 => "system",
            PromptMessageRole::System => "system",
            PromptMessageRole::Developer => "developer",
            PromptMessageRole::User => "user",
            PromptMessageRole::Assistant => "model",
            PromptMessageRole::Tool => return Err(invalid("Tool results are not supported.")),
        };
        let continues = previous == Some(PromptMessageRole::Assistant)
            && message.role == PromptMessageRole::Assistant;
        if !continues {
            out.push_str("<|turn>");
            out.push_str(role);
            out.push('\n');
        }
        if message.role == PromptMessageRole::Assistant {
            let content: String = message
                .content
                .split("<channel|>")
                .map(|part| part.split("<|channel>").next().unwrap_or_default())
                .collect();
            out.push_str(content.trim());
        } else {
            out.push_str(message.content.trim());
        }
        if messages.get(index + 1).is_none_or(|next| {
            message.role != PromptMessageRole::Assistant
                || next.role != PromptMessageRole::Assistant
        }) {
            out.push_str("<turn|>\n");
        }
        previous = Some(message.role);
    }
    if generation {
        out.push_str("<|turn>model\n");
    }
    Ok(out)
}

fn qwen3(messages: &[PromptMessage], generation: bool) -> Result<String, PromptRenderError> {
    let mut out = String::new();
    let last_user = messages
        .iter()
        .rposition(|m| m.role == PromptMessageRole::User);
    for (index, message) in messages.iter().enumerate() {
        if message.role == PromptMessageRole::Developer {
            return Err(invalid("Use a system message with this Qwen3 template."));
        }
        out.push_str("<|im_start|>");
        out.push_str(message.role.as_str());
        out.push('\n');
        if message.role == PromptMessageRole::Assistant {
            let content = message.content.split("</think>").last().unwrap_or_default();
            if last_user.is_some_and(|last| index > last) {
                out.push_str("<think>\n");
                if let Some(reasoning) = &message.reasoning_content {
                    out.push_str(reasoning.trim_matches('\n'));
                } else if let Some(reasoning) = crate::extract_qwen35_reasoning(&message.content) {
                    out.push_str(reasoning.trim_matches('\n'));
                }
                out.push_str("\n</think>\n\n");
            }
            out.push_str(content.trim_start_matches('\n'));
        } else {
            out.push_str(&message.content);
        }
        out.push_str("<|im_end|>\n");
    }
    if generation {
        out.push_str("<|im_start|>assistant\n<think>\n\n</think>\n\n");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn released_templates_match_their_jinja_text_prompts() -> Result<(), Box<dyn std::error::Error>>
    {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("testdata/released_chat_templates/golden.json"))?;
        for case in cases.as_array().ok_or("expected test cases")? {
            let template = match case["family"].as_str().ok_or("missing family")? {
                "llama3" => include_str!("testdata/released_chat_templates/llama3.jinja"),
                "mistral" => include_str!("testdata/released_chat_templates/mistral.jinja"),
                "qwen3" => include_str!("testdata/released_chat_templates/qwen3.jinja"),
                "qwen35" => include_str!("testdata/released_chat_templates/qwen35.jinja"),
                "gemma4" => include_str!("testdata/released_chat_templates/gemma4.jinja"),
                _ => return Err("unknown fixture family".into()),
            };
            let digest = crate::digest_chat_template(template);
            let family = crate::supported_prompt_template_family(&digest).ok_or(digest)?;
            let messages: Vec<PromptMessage> = serde_json::from_value(case["messages"].clone())?;
            let actual = if family == GgufPromptTemplateFamily::Qwen35 {
                let renderer = crate::GgufPromptTemplateRenderer::new(
                    crate::tests::qwen35_prompt_tokenizer_metadata(),
                    crate::GgufChatTemplateMetadata::new(Some(template.into()), Default::default()),
                );
                renderer
                    .render(None, &messages, case["generation"] == true)?
                    .text
            } else {
                render(
                    family,
                    case["bos"].as_str().unwrap_or_default(),
                    &messages,
                    case["generation"] == true,
                )?
            };
            assert_eq!(
                actual,
                case["expected"].as_str().ok_or("missing expected prompt")?,
                "{}",
                case["name"]
            );
        }
        Ok(())
    }

    #[test]
    fn mistral_rejects_system_and_non_alternating_messages() {
        for roles in [
            vec![PromptMessageRole::System],
            vec![PromptMessageRole::User, PromptMessageRole::User],
        ] {
            let messages: Vec<_> = roles
                .into_iter()
                .map(|role| PromptMessage::new(role, "hello"))
                .collect();
            assert!(render(GgufPromptTemplateFamily::MistralV3, "<s>", &messages, true).is_err());
        }
    }
}
