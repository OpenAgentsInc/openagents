//! Prompt rendering for DeepSeek V4 and V4.1.

use std::sync::Arc;

use aho_corasick::AhoCorasick;

use deepseek_recipe_core::conversation::Conversation;
use deepseek_recipe_core::messages::{InputMessage, ToolCall};
use deepseek_recipe_core::multimodal::ImageSource;
use deepseek_recipe_core::prompt;
use deepseek_recipe_core::prompt::{Prompt, Special};
use deepseek_recipe_core::special_tokens::{SpecialToken, SpecialTokens, default_special_tokens};
use deepseek_recipe_core::tools::{ToolChoice, ToolDefinition};

use crate::{EncodingError, RenderedPrompt, TokenizerEncoder};

pub mod dsv4;
pub mod dsv41;

/// The special tokens that assistant content and reasoning content can
/// contain: the grounding tokens. Rendering inserts each one found there as an
/// explicit special token; every other special-token spelling in assistant
/// content or reasoning content is plain text.
const ASSISTANT_SPECIAL_TOKENS: &[SpecialToken] = &[
    SpecialToken::Ref,
    SpecialToken::RefEnd,
    SpecialToken::Box,
    SpecialToken::BoxEnd,
    SpecialToken::Point,
    SpecialToken::PointEnd,
    SpecialToken::Polygon,
    SpecialToken::PolygonEnd,
];

/// Parse tool call arguments into the object whose entries become DSML
/// parameters. Arguments that are not a JSON object become one `arguments`
/// string parameter holding the original text.
fn parse_tool_call_arguments(tool_call: &ToolCall) -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_str(&tool_call.arguments).unwrap_or_else(|err| {
        tracing::warn!(?err, "invalid tool call arguments");
        serde_json::Map::from_iter([("arguments".to_owned(), tool_call.arguments.clone().into())])
    })
}

/// Return the tool definition object that a tool prompt lists.
fn tool_definition_json(tool: &ToolDefinition) -> serde_json::Value {
    serde_json::json!({
        "name": tool.name,
        "description": tool.description.as_deref().unwrap_or_default(),
        "parameters": tool.parameters,
    })
}

/// Return the instructions for the DSML tool call format, which precede the
/// tool schemas in a tool prompt.
fn tool_call_instructions(
    tokens: &dyn SpecialTokens,
    tool_calls_block_name: &str,
    tool_call_tag_name: &str,
    tool_parameter_tag_name: &str,
) -> Prompt {
    let dsml = Special(tokens.token(SpecialToken::Dsml));
    let thinking_start = Special(tokens.token(SpecialToken::ThinkingStart));
    let thinking_end = Special(tokens.token(SpecialToken::ThinkingEnd));
    prompt!(
        r#"## Tools

You have access to a set of tools to help answer the user's question. You can invoke tools by writing a "<{dsml}{tool_calls_block_name}>" block like the following:

<{dsml}{tool_calls_block_name}>
<{dsml}{tool_call_tag_name} name="$TOOL_NAME">
<{dsml}{tool_parameter_tag_name} name="$PARAMETER_NAME" string="true|false">$PARAMETER_VALUE</{dsml}{tool_parameter_tag_name}>
...
</{dsml}{tool_call_tag_name}>
<{dsml}{tool_call_tag_name} name="$TOOL_NAME2">
...
</{dsml}{tool_call_tag_name}>
</{dsml}{tool_calls_block_name}>

String parameters should be specified as is and set `string="true"`. For all other types (numbers, booleans, arrays, objects), pass the value in JSON format and set `string="false"`.

If thinking_mode is enabled (triggered by {thinking_start}), you MUST output your complete reasoning inside {thinking_start}...{thinking_end} BEFORE any tool calls or final response.

Otherwise, output directly after {thinking_end} with tool calls or final response.

### Available Tool Schemas

"#
    )
}

/// Return whether the conversation renders a tool prompt: tool choice is not
/// `None` and at least one tool is defined.
fn renders_tool_prompt(conversation: &Conversation) -> bool {
    conversation.tool_choice != ToolChoice::None && !conversation.tools.is_empty()
}

/// Return whether the last message is an assistant message with `prefix` set,
/// which the prompt continues.
fn is_prefix_continuation(messages: &[InputMessage]) -> bool {
    matches!(
        messages.last(),
        Some(InputMessage::Assistant { prefix: true, .. })
    )
}

/// Collect the image sources of system, user, and tool messages in prompt
/// order.
fn collect_image_sources(messages: &[InputMessage]) -> Vec<ImageSource> {
    messages
        .iter()
        .filter_map(|message| match message {
            InputMessage::System { image_sources, .. }
            | InputMessage::User { image_sources, .. }
            | InputMessage::Tool { image_sources, .. } => Some(image_sources.as_slice()),
            _ => None,
        })
        .flatten()
        .cloned()
        .collect()
}

/// Build a [`RenderedPrompt`] from `prompt` and the image sources of
/// `messages`.
///
/// The caller must pass the normalized messages that produced `prompt`, so
/// that the image sources follow placeholder order.
fn into_rendered_prompt(prompt: Prompt, messages: &[InputMessage]) -> RenderedPrompt {
    let image_sources = collect_image_sources(messages);
    let (prompt, special_ranges) = prompt.into_parts();
    RenderedPrompt {
        prompt,
        special_ranges,
        image_sources,
    }
}

/// The components of a prompt encoding that the caller attaches or replaces:
/// the tokenizer for token encoding and the special token mapping for prompt
/// rendering. [`DeepseekV4Encoding`] and [`DeepseekV41Encoding`] delegate
/// their constructors and token encoding to it.
///
/// [`DeepseekV4Encoding`]: dsv4::DeepseekV4Encoding
/// [`DeepseekV41Encoding`]: dsv41::DeepseekV41Encoding
#[derive(Clone)]
struct EncodingComponents {
    tokenizer: Option<Arc<dyn TokenizerEncoder>>,
    special_tokens: Arc<dyn SpecialTokens>,
    /// Matches the spellings of [`ASSISTANT_SPECIAL_TOKENS`], in that order,
    /// in assistant content and reasoning content.
    assistant_special_tokens: Arc<AhoCorasick>,
}

impl EncodingComponents {
    /// Create the components with the default special tokens and no attached
    /// tokenizer.
    fn new() -> Self {
        Self::with_mapping(None, default_special_tokens())
    }

    fn with_mapping(
        tokenizer: Option<Arc<dyn TokenizerEncoder>>,
        special_tokens: Arc<dyn SpecialTokens>,
    ) -> Self {
        let spellings = ASSISTANT_SPECIAL_TOKENS
            .iter()
            .map(|&token| special_tokens.token(token));
        let assistant_special_tokens =
            AhoCorasick::new(spellings).expect("the special token spellings build a matcher");
        Self {
            tokenizer,
            special_tokens,
            assistant_special_tokens: Arc::new(assistant_special_tokens),
        }
    }

    /// Attach the tokenizer that [`Self::encode`] uses.
    fn with_tokenizer(mut self, tokenizer: impl TokenizerEncoder + 'static) -> Self {
        self.tokenizer = Some(Arc::new(tokenizer));
        self
    }

    /// Replace the special token mapping.
    fn with_special_tokens(self, tokens: impl SpecialTokens + 'static) -> Self {
        Self::with_mapping(self.tokenizer, Arc::new(tokens))
    }

    /// Return the explicit special token fragment for `token`, spelled as the
    /// mapping defines it.
    fn special(&self, token: SpecialToken) -> Special<&str> {
        Special(self.special_tokens.token(token))
    }

    /// Append assistant content or reasoning content to `prompt`: each
    /// assistant special token in it as an explicit special token, the rest as
    /// plain text.
    fn push_assistant_content(&self, prompt: &mut Prompt, content: &str) {
        let mut cursor = 0;
        for matched in self.assistant_special_tokens.find_iter(content) {
            prompt.push_plain(&content[cursor..matched.start()]);
            prompt.push_special(
                self.special_tokens
                    .token(ASSISTANT_SPECIAL_TOKENS[matched.pattern()]),
            );
            cursor = matched.end();
        }
        prompt.push_plain(&content[cursor..]);
    }

    /// Encode a rendered prompt with the attached tokenizer.
    ///
    /// # Errors
    ///
    /// - [`EncodingError::MissingTokenizer`] when no tokenizer is attached.
    /// - [`EncodingError::Encode`] when the tokenizer returns an error.
    fn encode(&self, rendered: &RenderedPrompt) -> Result<Vec<u32>, EncodingError> {
        let tokenizer = self
            .tokenizer
            .as_ref()
            .ok_or(EncodingError::MissingTokenizer)?;
        tokenizer
            .encode_ids(&rendered.prompt, &rendered.special_ranges)
            .map_err(EncodingError::Encode)
    }
}

#[cfg(test)]
mod tests {
    use deepseek_recipe_core::conversation::ResponseFormat;
    use deepseek_recipe_core::multimodal::{IMAGE_SPECIAL_TOKEN_ID, ImageDetail};
    use deepseek_recipe_core::special_tokens::SpecialTokensMap;
    use tokenizers::{AddedVocabulary, Tokenizer};

    use super::dsv4::DeepseekV4Encoding;
    use super::dsv41::DeepseekV41Encoding;
    use super::*;
    use crate::{PromptEncoding, PromptTokenizer};

    const IMAGE: &str = "<｜image｜>";

    fn v41_tokenizer() -> Tokenizer {
        // Downstream change: the 6 MB tokenizer is not vendored. Point
        // DEEPSEEK_RECIPE_V41_TOKENIZER at the checkpoint's tokenizer.json.
        let path = std::env::var("DEEPSEEK_RECIPE_V41_TOKENIZER")
            .expect("DEEPSEEK_RECIPE_V41_TOKENIZER names the V4.1 tokenizer.json");
        Tokenizer::from_file(path).expect("the V4.1 tokenizer loads")
    }

    fn image_source() -> ImageSource {
        ImageSource::Url {
            url: "http://x/y.png".to_string(),
            detail: ImageDetail::High,
            path: String::new(),
        }
    }

    fn system(content: &str) -> InputMessage {
        InputMessage::System {
            content: Prompt::plain(content),
            image_sources: Vec::new(),
            tools: None,
        }
    }

    fn user(content: &str) -> InputMessage {
        InputMessage::User {
            content: Prompt::plain(content),
            image_sources: Vec::new(),
        }
    }

    fn assistant(content: &str, reasoning_content: &str) -> InputMessage {
        InputMessage::Assistant {
            content: content.to_string(),
            reasoning_content: Some(reasoning_content.to_string()),
            tool_calls: None,
            prefix: false,
        }
    }

    fn tool() -> ToolDefinition {
        ToolDefinition {
            name: "f".to_string(),
            description: None,
            parameters: serde_json::json!({"type": "object"}),
            strict: None,
            defer_loading: false,
        }
    }

    fn render(conversation: &Conversation) -> String {
        DeepseekV41Encoding::new()
            .render_conversation(conversation)
            .prompt
    }

    fn render_v4(conversation: &Conversation) -> String {
        DeepseekV4Encoding::new()
            .render_conversation(conversation)
            .prompt
    }

    fn conversation_with_system() -> Conversation {
        Conversation {
            messages: vec![system("sys"), user("hi")],
            thinking_mode: false,
            ..Conversation::default()
        }
    }

    fn custom_system_tokens() -> SpecialTokensMap {
        SpecialTokensMap {
            system: "<|sys|>".to_string(),
            ..SpecialTokensMap::default()
        }
    }

    #[test]
    fn assistant_special_tokens_in_assistant_content_are_special() {
        let mut prompt = Prompt::new();
        EncodingComponents::new()
            .push_assistant_content(&mut prompt, "a<｜ref｜>b<｜User｜>c<｜/ref｜>");
        let specials: Vec<&str> = prompt
            .special_ranges()
            .iter()
            .map(|range| &prompt.as_str()[range.clone()])
            .collect();
        assert_eq!(specials, ["<｜ref｜>", "<｜/ref｜>"]);
    }

    #[test]
    fn v41_uses_the_custom_system_token() {
        let encoding = DeepseekV41Encoding::new().with_special_tokens(custom_system_tokens());
        let rendered = encoding.render_conversation(&conversation_with_system());
        assert!(rendered.prompt.contains("<|sys|>sys"));
    }

    #[test]
    fn v4_never_emits_a_system_token() {
        let encoding = DeepseekV4Encoding::new().with_special_tokens(custom_system_tokens());
        let rendered = encoding.render_conversation(&conversation_with_system());
        assert!(!rendered.prompt.contains("<|sys|>"));
    }

    #[test]
    fn earlier_reasoning_is_rendered_only_with_tools() {
        let mut conversation = Conversation {
            messages: vec![user("a"), assistant("b", "r"), user("c")],
            ..Conversation::default()
        };
        assert!(render(&conversation).contains("<｜Assistant｜></think>b<｜end▁of▁sentence｜>"));
        assert!(render_v4(&conversation).contains("<｜Assistant｜></think>b<｜end▁of▁sentence｜>"));

        conversation.tools = vec![tool()];
        assert!(
            render(&conversation).contains("<｜Assistant｜><think>r</think>b<｜end▁of▁sentence｜>")
        );
        assert!(
            render_v4(&conversation)
                .contains("<｜Assistant｜><think>r</think>b<｜end▁of▁sentence｜>")
        );

        conversation.tool_choice = ToolChoice::None;
        assert!(render(&conversation).contains("<｜Assistant｜></think>b<｜end▁of▁sentence｜>"));
    }

    #[test]
    fn consecutive_latest_reminders_keep_the_first() {
        let conversation = Conversation {
            messages: vec![
                user("a"),
                InputMessage::LatestReminder {
                    content: "first".to_string(),
                },
                InputMessage::LatestReminder {
                    content: "second".to_string(),
                },
            ],
            ..Conversation::default()
        };
        let prompt = render(&conversation);
        assert!(prompt.contains("<｜latest_reminder｜>first<｜Assistant｜>"));
        assert!(!prompt.contains("second"));
    }

    #[test]
    fn json_schema_is_rendered_into_the_first_system_message() {
        let conversation = Conversation {
            messages: vec![user("a")],
            thinking_mode: false,
            response_format: ResponseFormat::JsonSchema(serde_json::json!({"type": "object"})),
            ..Conversation::default()
        };
        assert_eq!(
            render(&conversation),
            "<｜begin▁of▁sentence｜><｜System｜>\n\n## Response Format:\n\nYou MUST strictly \
             adhere to the following schema to reply:\n{\"type\": \"object\"}<｜User｜>a\
             <｜Assistant｜></think>"
        );
    }

    #[test]
    fn a_regex_response_format_is_not_rendered() {
        let conversation = Conversation {
            messages: vec![user("a")],
            thinking_mode: false,
            response_format: ResponseFormat::Regex("[0-9]+".to_string()),
            ..Conversation::default()
        };
        let text = Conversation {
            response_format: ResponseFormat::Text,
            ..conversation.clone()
        };
        assert_eq!(render(&conversation), render(&text));
        assert_eq!(render_v4(&conversation), render_v4(&text));
    }

    #[test]
    fn v4_hoists_system_messages_and_appends_format_messages() {
        let conversation = Conversation {
            messages: vec![system("s1"), user("a"), system(""), system("s2"), user("b")],
            thinking_mode: false,
            response_format: ResponseFormat::JsonObject,
            ..Conversation::default()
        };
        assert_eq!(
            render_v4(&conversation),
            "<｜begin▁of▁sentence｜>s1\n\ns2\n\nYou are a helpful assistant designed to output \
             JSON.\n## Response Format:\n\nReply with JSON object ONLY.<｜User｜>a\n\nb\
             <｜Assistant｜></think>"
        );
    }

    #[test]
    fn v4_renders_the_tool_prompt_as_a_system_message() {
        let conversation = Conversation {
            messages: vec![user("a")],
            tools: vec![tool()],
            tool_choice: ToolChoice::Required,
            thinking_mode: false,
            ..Conversation::default()
        };
        let prompt = render_v4(&conversation);
        assert!(prompt.starts_with("<｜begin▁of▁sentence｜>\n\n## Tools\n\n"));
        assert!(prompt.contains(
            "{\"name\": \"f\", \"description\": \"\", \"parameters\": {\"type\": \"object\"}}\n\
             \nYou MUST strictly follow"
        ));
        assert!(prompt.ends_with("<｜Assistant｜></think>\n\n<｜DSML｜tool_calls>\n"));
    }

    #[test]
    fn v4_sorts_tool_calls_and_results_by_id() {
        let tool_call = |id: &str| ToolCall {
            id: id.to_string(),
            name: "f".to_string(),
            arguments: "{}".to_string(),
        };
        let tool_result = |id: &str| InputMessage::Tool {
            content: Prompt::plain(id),
            image_sources: Vec::new(),
            tool_call_id: Some(id.to_string()),
        };
        let conversation = Conversation {
            messages: vec![
                user("a"),
                InputMessage::Assistant {
                    content: String::new(),
                    reasoning_content: None,
                    tool_calls: Some(vec![tool_call("b"), tool_call("a")]),
                    prefix: false,
                },
                tool_result("b"),
                tool_result("a"),
            ],
            thinking_mode: false,
            ..Conversation::default()
        };
        assert_eq!(
            render_v4(&conversation),
            "<｜begin▁of▁sentence｜><｜User｜>a<｜Assistant｜></think>\n\n<｜DSML｜tool_calls>\n\
             <｜DSML｜invoke name=\"f\">\n</｜DSML｜invoke>\n<｜DSML｜invoke name=\"f\">\n\
             </｜DSML｜invoke>\n</｜DSML｜tool_calls><｜end▁of▁sentence｜><｜User｜>\
             <tool_result>a</tool_result>\n\n<tool_result>b</tool_result><｜Assistant｜></think>"
        );
    }

    #[test]
    fn v4_sorts_a_user_message_in_a_tool_result_range_after_the_results() {
        let tool_call = |id: &str| ToolCall {
            id: id.to_string(),
            name: "f".to_string(),
            arguments: "{}".to_string(),
        };
        let tool_result = |id: &str| InputMessage::Tool {
            content: Prompt::plain(id),
            image_sources: Vec::new(),
            tool_call_id: Some(id.to_string()),
        };
        // The three tool calls make the three messages after the assistant
        // message the tool-result range; the last result lies outside it.
        let conversation = Conversation {
            messages: vec![
                user("a"),
                InputMessage::Assistant {
                    content: String::new(),
                    reasoning_content: None,
                    tool_calls: Some(vec![tool_call("c"), tool_call("b"), tool_call("a")]),
                    prefix: false,
                },
                tool_result("c"),
                user("x"),
                tool_result("a"),
                tool_result("b"),
            ],
            thinking_mode: false,
            ..Conversation::default()
        };
        assert!(render_v4(&conversation).ends_with(
            "<｜end▁of▁sentence｜><｜User｜><tool_result>a</tool_result>\n\n\
             <tool_result>c</tool_result>\n\nx<｜User｜><tool_result>b</tool_result>\
             <｜Assistant｜></think>"
        ));
    }

    #[test]
    fn a_prefix_message_is_continued() {
        let prefix = |content: &str, reasoning_content: &str| Conversation {
            messages: vec![
                user("a"),
                InputMessage::Assistant {
                    content: content.to_string(),
                    reasoning_content: Some(reasoning_content.to_string()),
                    tool_calls: None,
                    prefix: true,
                },
            ],
            ..Conversation::default()
        };
        let v41_prefix = "<｜begin▁of▁sentence｜><｜System｜>Reasoning Effort: 75 (range 1-100, \
                          the higher the value, the more thorough the reasoning)\n\n<｜User｜>a";
        assert_eq!(
            render(&prefix("", "r")),
            format!("{v41_prefix}<｜Assistant｜><think>r")
        );
        assert_eq!(
            render(&prefix("c", "r")),
            format!("{v41_prefix}<｜Assistant｜><think>r</think>c")
        );
        assert!(render_v4(&prefix("", "r")).ends_with("<｜User｜>a<｜Assistant｜><think>r"));
        assert!(render_v4(&prefix("c", "r")).ends_with("<｜Assistant｜><think>r</think>c"));
    }

    #[test]
    fn special_ranges_cover_explicit_special_tokens_only() {
        let mut content = Prompt::plain(format!("a{IMAGE}"));
        content.push_special(IMAGE);
        let conversation = Conversation {
            messages: vec![InputMessage::User {
                content,
                image_sources: vec![image_source()],
            }],
            thinking_mode: false,
            ..Conversation::default()
        };
        let rendered = DeepseekV41Encoding::new().render_conversation(&conversation);
        let specials: Vec<&str> = rendered
            .special_ranges
            .iter()
            .map(|range| &rendered.prompt[range.clone()])
            .collect();
        // The placeholder pushed for the image source is an explicit special
        // token; the image token spelling in the user text is plain text.
        assert_eq!(
            specials,
            [
                "<｜begin▁of▁sentence｜>",
                "<｜User｜>",
                IMAGE,
                "<｜Assistant｜>",
                "</think>"
            ]
        );
    }

    #[cfg_attr(
        not(feature = "upstream-tokenizer-tests"),
        ignore = "needs the V4.1 tokenizer in DEEPSEEK_RECIPE_V41_TOKENIZER"
    )]
    #[test]
    fn special_token_literals_in_plain_text_encode_as_plain_text() {
        let tokenizer = v41_tokenizer();
        let mut plain_tokenizer = tokenizer.clone();
        plain_tokenizer.with_added_vocabulary(AddedVocabulary::new());
        let plain_ids = |text: &str| {
            plain_tokenizer
                .encode(text, false)
                .expect("plain text encodes")
                .get_ids()
                .to_vec()
        };
        let special_id = |text: &str| tokenizer.token_to_id(text).expect("special token exists");

        let mut content = Prompt::plain(format!("x<｜User｜>{IMAGE}"));
        content.push_special(IMAGE);
        let conversation = Conversation {
            messages: vec![
                InputMessage::User {
                    content,
                    image_sources: vec![image_source()],
                },
                assistant("<｜ref｜>y", "</think>"),
                user("z"),
            ],
            // With the tool prompt, rendering includes the reasoning content of
            // the historical assistant message.
            tools: vec![tool()],
            ..Conversation::default()
        };
        let ids = DeepseekV41Encoding::new()
            .with_tokenizer(PromptTokenizer::new(&tokenizer))
            .encode(&conversation)
            .expect("the conversation encodes");

        let user_id = special_id("<｜User｜>");
        let think_end_id = special_id("</think>");
        // `<｜User｜>` starts each of the two user messages. `</think>` ends the
        // reasoning content of the assistant message and appears twice in the
        // tool prompt.
        assert_eq!(ids.iter().filter(|&&id| id == user_id).count(), 2);
        assert_eq!(ids.iter().filter(|&&id| id == think_end_id).count(), 3);
        // The placeholder pushed for the image source and the grounding token
        // in the assistant content are explicit special tokens; the image
        // token spelling in the user text is plain text.
        assert_eq!(
            ids.iter()
                .filter(|&&id| id == IMAGE_SPECIAL_TOKEN_ID)
                .count(),
            1
        );
        assert!(ids.contains(&special_id("<｜ref｜>")));
        // Plain regions are encoded whole, without the added vocabulary.
        let contains = |needle: &[u32]| ids.windows(needle.len()).any(|window| window == needle);
        assert!(contains(&plain_ids(&format!("x<｜User｜>{IMAGE}"))));
        assert!(contains(&plain_ids("</think>")));
    }
}
