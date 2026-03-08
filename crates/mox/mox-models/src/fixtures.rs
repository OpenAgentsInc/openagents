use std::borrow::Cow;

use crate::{
    GgufTokenizerMetadata, GgufTokenizerModel, GgufTokenizerPretokenizer, PromptReasoningEffort,
    TokenId, TokenSequence,
};

/// Stable tokenizer sample used by the golden corpus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GoldenTokenizerSample {
    /// Stable token ID.
    pub token_id: TokenId,
    /// Exact token string from the real source artifact.
    pub token: &'static str,
}

/// Golden tokenizer facts sourced from a real GGUF artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoldenTokenizerFixture {
    /// Stable fixture identifier.
    pub id: &'static str,
    /// Human-readable model family label.
    pub family: &'static str,
    /// Source GGUF path used when refreshing the fixture.
    pub source_path: &'static str,
    /// Source GGUF SHA-256 when the source artifact is intentionally small.
    pub source_sha256: Option<&'static str>,
    /// Total vocabulary length from the real artifact.
    pub vocabulary_len: usize,
    /// GGUF tokenizer model family.
    pub model: GgufTokenizerModel,
    /// GGUF pretokenizer label when applicable.
    pub pretokenizer: Option<&'static str>,
    /// BOS token ID when present.
    pub bos_token_id: Option<TokenId>,
    /// EOS token IDs in stable order.
    pub eos_token_ids: &'static [TokenId],
    /// Padding token ID when present.
    pub pad_token_id: Option<TokenId>,
    /// Unknown token ID when present.
    pub unknown_token_id: Option<TokenId>,
    /// Whether prompt callers should prepend BOS by default.
    pub add_bos: bool,
    /// Whether prompt callers should append EOS by default.
    pub add_eos: bool,
    /// Small sampled token slice for review and assertions.
    pub sample_tokens: &'static [GoldenTokenizerSample],
    /// Review note about why this source is in the corpus.
    pub notes: &'static str,
}

/// Supported prompt-rendering message roles in the golden corpus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoldenPromptRole {
    /// System/developer-style instruction.
    System,
    /// Developer instruction.
    Developer,
    /// End-user message.
    User,
    /// Assistant response.
    Assistant,
    /// Tool response.
    Tool,
}

/// Prompt message fixture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GoldenPromptMessage {
    /// Message role.
    pub role: GoldenPromptRole,
    /// Message content.
    pub content: &'static str,
}

/// GPT-OSS / Harmony context captured for one golden render case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GoldenPromptHarmonyContext {
    /// Conversation date rendered into the Harmony system message.
    pub conversation_start_date: Option<&'static str>,
    /// Knowledge cutoff rendered into the Harmony system message.
    pub knowledge_cutoff: Option<&'static str>,
    /// Reasoning-effort label rendered into the Harmony system message.
    pub reasoning_effort: Option<PromptReasoningEffort>,
    /// Explicit valid channels carried by the Harmony system message.
    pub valid_channels: &'static [&'static str],
}

/// Exact rendered prompt case captured from a real template family.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoldenPromptRenderCase {
    /// Stable render-case identifier.
    pub id: &'static str,
    /// Input message history.
    pub messages: &'static [GoldenPromptMessage],
    /// Harmony render context when the family requires it.
    pub harmony_context: Option<GoldenPromptHarmonyContext>,
    /// Whether generation prompt emission is requested.
    pub add_generation_prompt: bool,
    /// Exact rendered prompt bytes as a UTF-8 string.
    pub expected_rendered: &'static str,
}

/// Small window-pressure scenario for later conformance work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoldenPromptSurface {
    /// Text generation prompt handling.
    Generate,
    /// Embeddings input handling.
    Embed,
}

/// Render-budget scenario derived from a real prompt fixture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoldenPromptWindowCase {
    /// Stable scenario identifier.
    pub id: &'static str,
    /// Surface that should consume the case later.
    pub surface: GoldenPromptSurface,
    /// Template-variant identifier referenced by the scenario.
    pub template_variant_id: &'static str,
    /// Render-case identifier referenced by the scenario.
    pub render_case_id: &'static str,
    /// Maximum rendered byte budget for the scenario.
    pub max_rendered_bytes: usize,
    /// Whether the referenced render case should overflow the budget.
    pub expected_over_budget: bool,
    /// Human-readable note for future harness work.
    pub note: &'static str,
}

/// Chat-template variant captured from a real GGUF artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoldenPromptTemplateVariant {
    /// Stable variant identifier.
    pub id: &'static str,
    /// GGUF metadata key for the variant.
    pub gguf_key: &'static str,
    /// Ollama template name when the variant maps cleanly to one.
    pub ollama_template_name: Option<&'static str>,
    /// Ollama stop-default source when present.
    pub ollama_stop_source: Option<&'static str>,
    /// SHA-256 digest over the full raw template.
    pub template_digest: &'static str,
    /// Full raw template when it is intentionally small and reviewable.
    pub raw_template: Option<&'static str>,
    /// Reviewable excerpt when the raw template is intentionally omitted.
    pub template_excerpt: &'static str,
    /// Stop defaults sourced from Ollama when present.
    pub stop_sequences: &'static [&'static str],
    /// Exact rendered prompt cases.
    pub render_cases: &'static [GoldenPromptRenderCase],
    /// Review note about the variant.
    pub notes: &'static str,
}

impl GoldenPromptTemplateVariant {
    /// Finds a render case by stable identifier.
    #[must_use]
    pub fn render_case(&self, id: &str) -> Option<&GoldenPromptRenderCase> {
        self.render_cases.iter().find(|case| case.id == id)
    }
}

/// Golden prompt fixture sourced from a real GGUF artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoldenPromptFixture {
    /// Stable fixture identifier.
    pub id: &'static str,
    /// Human-readable model family label.
    pub family: &'static str,
    /// Primary source GGUF path.
    pub source_path: &'static str,
    /// Source GGUF SHA-256 when the source artifact is intentionally small.
    pub source_sha256: Option<&'static str>,
    /// Template variants captured from the source artifact.
    pub template_variants: &'static [GoldenPromptTemplateVariant],
    /// Small render-budget scenarios for conformance work.
    pub window_cases: &'static [GoldenPromptWindowCase],
    /// Review note about why this source is in the corpus.
    pub notes: &'static str,
}

impl GoldenPromptFixture {
    /// Finds a template variant by stable identifier.
    #[must_use]
    pub fn template_variant(&self, id: &str) -> Option<&GoldenPromptTemplateVariant> {
        self.template_variants
            .iter()
            .find(|variant| variant.id == id)
    }
}

const LLAMA_SPM_SAMPLES: [GoldenTokenizerSample; 6] = [
    GoldenTokenizerSample {
        token_id: TokenId(0),
        token: "<unk>",
    },
    GoldenTokenizerSample {
        token_id: TokenId(1),
        token: "<s>",
    },
    GoldenTokenizerSample {
        token_id: TokenId(2),
        token: "</s>",
    },
    GoldenTokenizerSample {
        token_id: TokenId(3),
        token: "<0x00>",
    },
    GoldenTokenizerSample {
        token_id: TokenId(4),
        token: "<0x01>",
    },
    GoldenTokenizerSample {
        token_id: TokenId(5),
        token: "<0x02>",
    },
];

const QWEN2_SAMPLES: [GoldenTokenizerSample; 4] = [
    GoldenTokenizerSample {
        token_id: TokenId(0),
        token: "!",
    },
    GoldenTokenizerSample {
        token_id: TokenId(1),
        token: "\"",
    },
    GoldenTokenizerSample {
        token_id: TokenId(2),
        token: "#",
    },
    GoldenTokenizerSample {
        token_id: TokenId(151643),
        token: "<|endoftext|>",
    },
];

const GPT_OSS_SAMPLES: [GoldenTokenizerSample; 3] = [
    GoldenTokenizerSample {
        token_id: TokenId(199998),
        token: "<|startoftext|>",
    },
    GoldenTokenizerSample {
        token_id: TokenId(199999),
        token: "<|endoftext|>",
    },
    GoldenTokenizerSample {
        token_id: TokenId(200002),
        token: "<|return|>",
    },
];

/// Golden tokenizer fixtures sourced from local real GGUF artifacts.
pub const GOLDEN_TOKENIZER_FIXTURES: [GoldenTokenizerFixture; 3] = [
    GoldenTokenizerFixture {
        id: "llama_spm",
        family: "llama_spm",
        source_path: "/home/christopherdavid/code/llama.cpp/models/ggml-vocab-llama-spm.gguf",
        source_sha256: Some("16c3724582d59aa8bf84711894e833f916ee46a31d80e21312759c48bf8d0e69"),
        vocabulary_len: 32000,
        model: GgufTokenizerModel::SentencePiece,
        pretokenizer: None,
        bos_token_id: Some(TokenId(1)),
        eos_token_ids: &[TokenId(2)],
        pad_token_id: None,
        unknown_token_id: Some(TokenId(0)),
        add_bos: true,
        add_eos: false,
        sample_tokens: &LLAMA_SPM_SAMPLES,
        notes: "SentencePiece baseline from llama.cpp's redistributable vocab-only GGUF.",
    },
    GoldenTokenizerFixture {
        id: "qwen2",
        family: "qwen2",
        source_path: "/home/christopherdavid/code/llama.cpp/models/ggml-vocab-qwen2.gguf",
        source_sha256: Some("44c2f46b715f585c6ab513970e8a006bfa5badd6108560054921cf598d154d8c"),
        vocabulary_len: 151936,
        model: GgufTokenizerModel::Gpt2Bpe,
        pretokenizer: Some("qwen2"),
        bos_token_id: Some(TokenId(151643)),
        eos_token_ids: &[TokenId(151643)],
        pad_token_id: Some(TokenId(151643)),
        unknown_token_id: None,
        add_bos: true,
        add_eos: false,
        sample_tokens: &QWEN2_SAMPLES,
        notes: "GPT-style BPE baseline with real qwen2 special-token defaults and prompt family.",
    },
    GoldenTokenizerFixture {
        id: "gpt_oss_20b",
        family: "gpt_oss",
        source_path: "/home/christopherdavid/models/gpt-oss/gpt-oss-20b-mxfp4.gguf",
        source_sha256: None,
        vocabulary_len: 201088,
        model: GgufTokenizerModel::Gpt2Bpe,
        pretokenizer: Some("gpt-4o"),
        bos_token_id: Some(TokenId(199998)),
        eos_token_ids: &[TokenId(200002)],
        pad_token_id: Some(TokenId(199999)),
        unknown_token_id: None,
        add_bos: true,
        add_eos: false,
        sample_tokens: &GPT_OSS_SAMPLES,
        notes: "Real GPT-OSS tokenizer facts from the local 20B MXFP4 GGUF the user provided for fixture work.",
    },
];

const PHI3_TEMPLATE: &str = "{{ bos_token }}{% for message in messages %}{% if (message['role'] == 'user') %}{{'<|user|>' + '\n' + message['content'] + '<|end|>' + '\n' + '<|assistant|>' + '\n'}}{% elif (message['role'] == 'assistant') %}{{message['content'] + '<|end|>' + '\n'}}{% endif %}{% endfor %}";

const QWEN2_TEMPLATE: &str = "{% for message in messages %}{% if loop.first and messages[0]['role'] != 'system' %}{{ '<|im_start|>system\nYou are a helpful assistant<|im_end|>\n' }}{% endif %}{{'<|im_start|>' + message['role'] + '\n' + message['content'] + '<|im_end|>' + '\n'}}{% endfor %}{% if add_generation_prompt %}{{ '<|im_start|>assistant\n' }}{% endif %}";

const COMMAND_R_TEMPLATE: &str = "{{ bos_token }}{% if messages[0]['role'] == 'system' %}{% set loop_messages = messages[1:] %}{% set system_message = messages[0]['content'] %}{% elif false == true %}{% set loop_messages = messages %}{% set system_message = 'You are Command-R, a brilliant, sophisticated, AI-assistant trained to assist human users by providing thorough responses. You are trained by Cohere.' %}{% else %}{% set loop_messages = messages %}{% set system_message = false %}{% endif %}{% if system_message != false %}{{ '<|START_OF_TURN_TOKEN|><|SYSTEM_TOKEN|>' + system_message + '<|END_OF_TURN_TOKEN|>' }}{% endif %}{% for message in loop_messages %}{% if (message['role'] == 'user') != (loop.index0 % 2 == 0) %}{{ raise_exception('Conversation roles must alternate user/assistant/user/assistant/...') }}{% endif %}{% set content = message['content'] %}{% if message['role'] == 'user' %}{{ '<|START_OF_TURN_TOKEN|><|USER_TOKEN|>' + content.strip() + '<|END_OF_TURN_TOKEN|>' }}{% elif message['role'] == 'assistant' %}{{ '<|START_OF_TURN_TOKEN|><|CHATBOT_TOKEN|>'  + content.strip() + '<|END_OF_TURN_TOKEN|>' }}{% endif %}{% endfor %}{% if add_generation_prompt %}{{ '<|START_OF_TURN_TOKEN|><|CHATBOT_TOKEN|>' }}{% endif %}";

const PHI3_MESSAGES_USER_ONLY: [GoldenPromptMessage; 1] = [GoldenPromptMessage {
    role: GoldenPromptRole::User,
    content: "Explain rust ownership.",
}];

const PHI3_MESSAGES_MULTI_TURN: [GoldenPromptMessage; 3] = [
    GoldenPromptMessage {
        role: GoldenPromptRole::User,
        content: "Say hello.",
    },
    GoldenPromptMessage {
        role: GoldenPromptRole::Assistant,
        content: "Hello there.",
    },
    GoldenPromptMessage {
        role: GoldenPromptRole::User,
        content: "Now say goodbye.",
    },
];

const QWEN2_MESSAGES_DEFAULT_SYSTEM: [GoldenPromptMessage; 1] = [GoldenPromptMessage {
    role: GoldenPromptRole::User,
    content: "Summarize the roadmap.",
}];

const QWEN2_MESSAGES_WITH_SYSTEM: [GoldenPromptMessage; 4] = [
    GoldenPromptMessage {
        role: GoldenPromptRole::System,
        content: "Be terse.",
    },
    GoldenPromptMessage {
        role: GoldenPromptRole::User,
        content: "Hi",
    },
    GoldenPromptMessage {
        role: GoldenPromptRole::Assistant,
        content: "Hello.",
    },
    GoldenPromptMessage {
        role: GoldenPromptRole::User,
        content: "Summarize.",
    },
];

const COMMAND_R_MESSAGES_USER_ONLY: [GoldenPromptMessage; 1] = [GoldenPromptMessage {
    role: GoldenPromptRole::User,
    content: "Hello",
}];

const COMMAND_R_MESSAGES_WITH_SYSTEM: [GoldenPromptMessage; 4] = [
    GoldenPromptMessage {
        role: GoldenPromptRole::System,
        content: "Keep it short.",
    },
    GoldenPromptMessage {
        role: GoldenPromptRole::User,
        content: "Hello",
    },
    GoldenPromptMessage {
        role: GoldenPromptRole::Assistant,
        content: "Hi",
    },
    GoldenPromptMessage {
        role: GoldenPromptRole::User,
        content: "More",
    },
];

const GPT_OSS_MESSAGES_REASONING_WITH_DEVELOPER: [GoldenPromptMessage; 2] = [
    GoldenPromptMessage {
        role: GoldenPromptRole::Developer,
        content: "Be concise.",
    },
    GoldenPromptMessage {
        role: GoldenPromptRole::User,
        content: "What is 17 times 19? Answer with just the number.",
    },
];

const GPT_OSS_MESSAGES_USER_ONLY: [GoldenPromptMessage; 1] = [GoldenPromptMessage {
    role: GoldenPromptRole::User,
    content: "What is 42 * pi?",
}];

const GPT_OSS_REASONING_CONTEXT: GoldenPromptHarmonyContext = GoldenPromptHarmonyContext {
    conversation_start_date: Some("2026-03-08"),
    knowledge_cutoff: Some("2024-06"),
    reasoning_effort: Some(PromptReasoningEffort::Low),
    valid_channels: &["analysis", "commentary", "final"],
};

const GPT_OSS_USER_ONLY_CONTEXT: GoldenPromptHarmonyContext = GoldenPromptHarmonyContext {
    conversation_start_date: Some("2026-03-08"),
    knowledge_cutoff: Some("2024-06"),
    reasoning_effort: Some(PromptReasoningEffort::Medium),
    valid_channels: &["analysis", "final"],
};

const PHI3_RENDER_CASES: [GoldenPromptRenderCase; 2] = [
    GoldenPromptRenderCase {
        id: "phi3.user_only",
        messages: &PHI3_MESSAGES_USER_ONLY,
        harmony_context: None,
        add_generation_prompt: true,
        expected_rendered: "<s><|user|>\nExplain rust ownership.<|end|>\n<|assistant|>\n",
    },
    GoldenPromptRenderCase {
        id: "phi3.multi_turn",
        messages: &PHI3_MESSAGES_MULTI_TURN,
        harmony_context: None,
        add_generation_prompt: true,
        expected_rendered: "<s><|user|>\nSay hello.<|end|>\n<|assistant|>\nHello there.<|end|>\n<|user|>\nNow say goodbye.<|end|>\n<|assistant|>\n",
    },
];

const QWEN2_RENDER_CASES: [GoldenPromptRenderCase; 3] = [
    GoldenPromptRenderCase {
        id: "qwen2.default_system",
        messages: &QWEN2_MESSAGES_DEFAULT_SYSTEM,
        harmony_context: None,
        add_generation_prompt: true,
        expected_rendered: "<|im_start|>system\nYou are a helpful assistant<|im_end|>\n<|im_start|>user\nSummarize the roadmap.<|im_end|>\n<|im_start|>assistant\n",
    },
    GoldenPromptRenderCase {
        id: "qwen2.with_system_history",
        messages: &QWEN2_MESSAGES_WITH_SYSTEM,
        harmony_context: None,
        add_generation_prompt: true,
        expected_rendered: "<|im_start|>system\nBe terse.<|im_end|>\n<|im_start|>user\nHi<|im_end|>\n<|im_start|>assistant\nHello.<|im_end|>\n<|im_start|>user\nSummarize.<|im_end|>\n<|im_start|>assistant\n",
    },
    GoldenPromptRenderCase {
        id: "qwen2.without_generation_prompt",
        messages: &QWEN2_MESSAGES_DEFAULT_SYSTEM,
        harmony_context: None,
        add_generation_prompt: false,
        expected_rendered: "<|im_start|>system\nYou are a helpful assistant<|im_end|>\n<|im_start|>user\nSummarize the roadmap.<|im_end|>\n",
    },
];

const COMMAND_R_RENDER_CASES: [GoldenPromptRenderCase; 2] = [
    GoldenPromptRenderCase {
        id: "command_r.user_only",
        messages: &COMMAND_R_MESSAGES_USER_ONLY,
        harmony_context: None,
        add_generation_prompt: true,
        expected_rendered: "<BOS_TOKEN><|START_OF_TURN_TOKEN|><|USER_TOKEN|>Hello<|END_OF_TURN_TOKEN|><|START_OF_TURN_TOKEN|><|CHATBOT_TOKEN|>",
    },
    GoldenPromptRenderCase {
        id: "command_r.with_system_history",
        messages: &COMMAND_R_MESSAGES_WITH_SYSTEM,
        harmony_context: None,
        add_generation_prompt: true,
        expected_rendered: "<BOS_TOKEN><|START_OF_TURN_TOKEN|><|SYSTEM_TOKEN|>Keep it short.<|END_OF_TURN_TOKEN|><|START_OF_TURN_TOKEN|><|USER_TOKEN|>Hello<|END_OF_TURN_TOKEN|><|START_OF_TURN_TOKEN|><|CHATBOT_TOKEN|>Hi<|END_OF_TURN_TOKEN|><|START_OF_TURN_TOKEN|><|USER_TOKEN|>More<|END_OF_TURN_TOKEN|><|START_OF_TURN_TOKEN|><|CHATBOT_TOKEN|>",
    },
];

const EMPTY_RENDER_CASES: [GoldenPromptRenderCase; 0] = [];

const GPT_OSS_RENDER_CASES: [GoldenPromptRenderCase; 2] = [
    GoldenPromptRenderCase {
        id: "gpt_oss.reasoning_with_developer",
        messages: &GPT_OSS_MESSAGES_REASONING_WITH_DEVELOPER,
        harmony_context: Some(GPT_OSS_REASONING_CONTEXT),
        add_generation_prompt: true,
        expected_rendered: "<|start|>system<|message|>You are ChatGPT, a large language model trained by OpenAI.\nKnowledge cutoff: 2024-06\nCurrent date: 2026-03-08\n\nReasoning: low\n\n# Valid channels: analysis, commentary, final. Channel must be included for every message.<|end|><|start|>developer<|message|># Instructions\n\nBe concise.<|end|><|start|>user<|message|>What is 17 times 19? Answer with just the number.<|end|><|start|>assistant",
    },
    GoldenPromptRenderCase {
        id: "gpt_oss.user_only_with_date",
        messages: &GPT_OSS_MESSAGES_USER_ONLY,
        harmony_context: Some(GPT_OSS_USER_ONLY_CONTEXT),
        add_generation_prompt: true,
        expected_rendered: "<|start|>system<|message|>You are ChatGPT, a large language model trained by OpenAI.\nKnowledge cutoff: 2024-06\nCurrent date: 2026-03-08\n\nReasoning: medium\n\n# Valid channels: analysis, final. Channel must be included for every message.<|end|><|start|>user<|message|>What is 42 * pi?<|end|><|start|>assistant",
    },
];

const PHI3_VARIANTS: [GoldenPromptTemplateVariant; 1] = [GoldenPromptTemplateVariant {
    id: "phi3.default",
    gguf_key: "tokenizer.chat_template",
    ollama_template_name: Some("phi-3"),
    ollama_stop_source: Some("/home/christopherdavid/code/ollama/template/phi-3.json"),
    template_digest: "268b6082ceb7176dc6ed80557a2f7837f9f0339592fbee677d405a553af15f88",
    raw_template: Some(PHI3_TEMPLATE),
    template_excerpt: "{{ bos_token }}{% for message in messages %}{% if (message['role'] == 'user') %}...",
    stop_sequences: &["<|end|>", "<|system|>", "<|user|>", "<|assistant|>"],
    render_cases: &PHI3_RENDER_CASES,
    notes: "Small redistributable prompt family with real BOS insertion and Ollama stop defaults.",
}];

const QWEN2_VARIANTS: [GoldenPromptTemplateVariant; 1] = [GoldenPromptTemplateVariant {
    id: "qwen2.default",
    gguf_key: "tokenizer.chat_template",
    ollama_template_name: None,
    ollama_stop_source: None,
    template_digest: "af9c0233881b083b52ff773580215222b5440ac3d0beeeca99b76329b048f8db",
    raw_template: Some(QWEN2_TEMPLATE),
    template_excerpt: "{% for message in messages %}{% if loop.first and messages[0]['role'] != 'system' %}...",
    stop_sequences: &[],
    render_cases: &QWEN2_RENDER_CASES,
    notes: "Real qwen2 chat template with injected default system behavior when the first turn is not system.",
}];

const COMMAND_R_VARIANTS: [GoldenPromptTemplateVariant; 3] = [
    GoldenPromptTemplateVariant {
        id: "command_r.default",
        gguf_key: "tokenizer.chat_template",
        ollama_template_name: Some("command-r"),
        ollama_stop_source: Some("/home/christopherdavid/code/ollama/template/command-r.json"),
        template_digest: "9db2cf47ce03bfd0aab6ec59942503714fa0372f09f7e1d54cbcd71a1110b863",
        raw_template: Some(COMMAND_R_TEMPLATE),
        template_excerpt: "{{ bos_token }}{% if messages[0]['role'] == 'system' %}{% set loop_messages = messages[1:] %}...",
        stop_sequences: &["<|START_OF_TURN_TOKEN|>", "<|END_OF_TURN_TOKEN|>"],
        render_cases: &COMMAND_R_RENDER_CASES,
        notes: "Default Command-R chat template plus Ollama stop defaults.",
    },
    GoldenPromptTemplateVariant {
        id: "command_r.tool_use",
        gguf_key: "tokenizer.chat_template.tool_use",
        ollama_template_name: None,
        ollama_stop_source: Some("/home/christopherdavid/code/ollama/template/command-r.json"),
        template_digest: "3a06dd2315c5fad58fb45ffe6bf91e012519104eb1817cb97b9924a6d9767702",
        raw_template: None,
        template_excerpt: "{{ bos_token }}{% if messages[0]['role'] == 'system' %}{% set loop_messages = messages[1:] %}{% else %}...",
        stop_sequences: &["<|START_OF_TURN_TOKEN|>", "<|END_OF_TURN_TOKEN|>"],
        render_cases: &EMPTY_RENDER_CASES,
        notes: "Named variant kept digest-only because the full tool schema block is large and review-hostile.",
    },
    GoldenPromptTemplateVariant {
        id: "command_r.rag",
        gguf_key: "tokenizer.chat_template.rag",
        ollama_template_name: None,
        ollama_stop_source: Some("/home/christopherdavid/code/ollama/template/command-r.json"),
        template_digest: "2b5c6d6b33d8162aaa71cd8bab0c0e1f1df464b996e795a16d592e8bc2a3bdbc",
        raw_template: None,
        template_excerpt: "{{ bos_token }}{% if messages[0]['role'] == 'system' %}{% set loop_messages = messages[1:] %}{% else %}...",
        stop_sequences: &["<|START_OF_TURN_TOKEN|>", "<|END_OF_TURN_TOKEN|>"],
        render_cases: &EMPTY_RENDER_CASES,
        notes: "Named variant kept digest-only because the full retrieved-document scaffolding is large and review-hostile.",
    },
];

const GPT_OSS_VARIANTS: [GoldenPromptTemplateVariant; 1] = [GoldenPromptTemplateVariant {
    id: "gpt_oss.default",
    gguf_key: "tokenizer.chat_template",
    ollama_template_name: None,
    ollama_stop_source: None,
    template_digest: "a4c9919cbbd4acdd51ccffe22da049264b1b73e59055fa58811a99efbd7c8146",
    raw_template: None,
    template_excerpt: "{#- In addition to the normal inputs of `messages` and `tools`, this template also accepts the following kwargs: ...",
    stop_sequences: &[],
    render_cases: &GPT_OSS_RENDER_CASES,
    notes: "Real GPT-OSS template digest plus stable Harmony render cases with explicit date/channel context.",
}];

const PHI3_WINDOW_CASES: [GoldenPromptWindowCase; 1] = [GoldenPromptWindowCase {
    id: "phi3.user_only_over_small_window",
    surface: GoldenPromptSurface::Generate,
    template_variant_id: "phi3.default",
    render_case_id: "phi3.user_only",
    max_rendered_bytes: 32,
    expected_over_budget: true,
    note: "Small rendered-byte budget for later context-window conformance checks.",
}];

const QWEN2_WINDOW_CASES: [GoldenPromptWindowCase; 2] = [
    GoldenPromptWindowCase {
        id: "qwen2.default_system_within_medium_window",
        surface: GoldenPromptSurface::Generate,
        template_variant_id: "qwen2.default",
        render_case_id: "qwen2.default_system",
        max_rendered_bytes: 192,
        expected_over_budget: false,
        note: "Keeps the single-turn prompt inside a moderate window.",
    },
    GoldenPromptWindowCase {
        id: "qwen2.history_over_small_window",
        surface: GoldenPromptSurface::Generate,
        template_variant_id: "qwen2.default",
        render_case_id: "qwen2.with_system_history",
        max_rendered_bytes: 128,
        expected_over_budget: true,
        note: "Forces a history-shaped prompt over a small render budget without dropping system retention from the corpus.",
    },
];

const COMMAND_R_WINDOW_CASES: [GoldenPromptWindowCase; 2] = [
    GoldenPromptWindowCase {
        id: "command_r.user_only_within_medium_window",
        surface: GoldenPromptSurface::Generate,
        template_variant_id: "command_r.default",
        render_case_id: "command_r.user_only",
        max_rendered_bytes: 192,
        expected_over_budget: false,
        note: "Fits a minimal Command-R turn inside a medium render budget.",
    },
    GoldenPromptWindowCase {
        id: "command_r.system_history_over_small_window",
        surface: GoldenPromptSurface::Generate,
        template_variant_id: "command_r.default",
        render_case_id: "command_r.with_system_history",
        max_rendered_bytes: 160,
        expected_over_budget: true,
        note: "Forces a system-plus-history prompt over a small render budget for later conformance work.",
    },
];

const GPT_OSS_WINDOW_CASES: [GoldenPromptWindowCase; 0] = [];

/// Golden prompt fixtures sourced from local real GGUF artifacts.
pub const GOLDEN_PROMPT_FIXTURES: [GoldenPromptFixture; 4] = [
    GoldenPromptFixture {
        id: "phi3",
        family: "phi3",
        source_path: "/home/christopherdavid/code/llama.cpp/models/ggml-vocab-phi-3.gguf",
        source_sha256: Some("967d7190d11c4842eab697079d98d56c2116e10eb617be355a2733bfc132e326"),
        template_variants: &PHI3_VARIANTS,
        window_cases: &PHI3_WINDOW_CASES,
        notes: "SentencePiece prompt family with real BOS injection and direct Ollama stop defaults.",
    },
    GoldenPromptFixture {
        id: "qwen2",
        family: "qwen2",
        source_path: "/home/christopherdavid/code/llama.cpp/models/ggml-vocab-qwen2.gguf",
        source_sha256: Some("44c2f46b715f585c6ab513970e8a006bfa5badd6108560054921cf598d154d8c"),
        template_variants: &QWEN2_VARIANTS,
        window_cases: &QWEN2_WINDOW_CASES,
        notes: "GPT-style prompt family with default-system injection and explicit generation-prompt coverage.",
    },
    GoldenPromptFixture {
        id: "command_r",
        family: "command_r",
        source_path: "/home/christopherdavid/code/llama.cpp/models/ggml-vocab-command-r.gguf",
        source_sha256: Some("a2f8cfea952ef7c391a6d92a1c309d0bd32e36384d9b9230569a7425732f27d9"),
        template_variants: &COMMAND_R_VARIANTS,
        window_cases: &COMMAND_R_WINDOW_CASES,
        notes: "Named/default chat-template corpus entry; default render cases are small, named variants are digest-only.",
    },
    GoldenPromptFixture {
        id: "gpt_oss",
        family: "gpt_oss",
        source_path: "/home/christopherdavid/models/gpt-oss/gpt-oss-20b-mxfp4.gguf",
        source_sha256: None,
        template_variants: &GPT_OSS_VARIANTS,
        window_cases: &GPT_OSS_WINDOW_CASES,
        notes: "Real GPT-OSS prompt-family anchor from the user-provided local GGUF with stable Harmony render cases pinned by explicit context.",
    },
];

/// Returns all golden tokenizer fixtures.
#[must_use]
pub fn golden_tokenizer_fixtures() -> &'static [GoldenTokenizerFixture] {
    &GOLDEN_TOKENIZER_FIXTURES
}

/// Finds a golden tokenizer fixture by stable identifier.
#[must_use]
pub fn golden_tokenizer_fixture(id: &str) -> Option<&'static GoldenTokenizerFixture> {
    GOLDEN_TOKENIZER_FIXTURES
        .iter()
        .find(|fixture| fixture.id == id)
}

/// Returns all golden prompt fixtures.
#[must_use]
pub fn golden_prompt_fixtures() -> &'static [GoldenPromptFixture] {
    &GOLDEN_PROMPT_FIXTURES
}

/// Finds a golden prompt fixture by stable identifier.
#[must_use]
pub fn golden_prompt_fixture(id: &str) -> Option<&'static GoldenPromptFixture> {
    GOLDEN_PROMPT_FIXTURES
        .iter()
        .find(|fixture| fixture.id == id)
}

/// Applies the GGUF add-BOS/add-EOS defaults to a token sequence.
#[must_use]
pub fn apply_special_token_defaults(
    tokenizer: &GgufTokenizerMetadata,
    tokens: &[TokenId],
) -> TokenSequence {
    let mut output = Vec::with_capacity(tokens.len() + 2);
    if tokenizer.add_bos {
        if let Some(bos) = tokenizer.vocabulary.bos_token_id() {
            output.push(bos);
        }
    }
    output.extend_from_slice(tokens);
    if tokenizer.add_eos {
        if let Some(eos) = tokenizer.vocabulary.eos_token_ids().first() {
            output.push(*eos);
        }
    }
    TokenSequence::new(output)
}

/// Computes the stable digest for a raw chat template.
#[must_use]
pub fn digest_chat_template(template: &str) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(template.as_bytes());
    hex::encode(hasher.finalize())
}

/// Asserts that observed tokenizer metadata matches a golden tokenizer fixture.
pub fn assert_tokenizer_fixture_matches(
    fixture: &GoldenTokenizerFixture,
    metadata: &GgufTokenizerMetadata,
) -> Result<(), String> {
    if metadata.model != fixture.model {
        return Err(format!(
            "tokenizer fixture `{}` model mismatch: expected {:?}, got {:?}",
            fixture.id, fixture.model, metadata.model
        ));
    }
    let actual_pretokenizer = metadata
        .pretokenizer
        .as_ref()
        .map(pretokenizer_fixture_label);
    if actual_pretokenizer.as_deref() != fixture.pretokenizer {
        return Err(format!(
            "tokenizer fixture `{}` pretokenizer mismatch: expected {:?}, got {:?}",
            fixture.id,
            fixture.pretokenizer,
            actual_pretokenizer.as_deref()
        ));
    }
    if metadata.vocabulary.len() != fixture.vocabulary_len {
        return Err(format!(
            "tokenizer fixture `{}` vocabulary length mismatch: expected {}, got {}",
            fixture.id,
            fixture.vocabulary_len,
            metadata.vocabulary.len()
        ));
    }
    if metadata.vocabulary.bos_token_id() != fixture.bos_token_id {
        return Err(format!(
            "tokenizer fixture `{}` BOS mismatch: expected {:?}, got {:?}",
            fixture.id,
            fixture.bos_token_id,
            metadata.vocabulary.bos_token_id()
        ));
    }
    if metadata.vocabulary.eos_token_ids() != fixture.eos_token_ids {
        return Err(format!(
            "tokenizer fixture `{}` EOS mismatch: expected {:?}, got {:?}",
            fixture.id,
            fixture.eos_token_ids,
            metadata.vocabulary.eos_token_ids()
        ));
    }
    if metadata.vocabulary.pad_token_id() != fixture.pad_token_id {
        return Err(format!(
            "tokenizer fixture `{}` PAD mismatch: expected {:?}, got {:?}",
            fixture.id,
            fixture.pad_token_id,
            metadata.vocabulary.pad_token_id()
        ));
    }
    if metadata.vocabulary.unknown_token_id() != fixture.unknown_token_id {
        return Err(format!(
            "tokenizer fixture `{}` UNK mismatch: expected {:?}, got {:?}",
            fixture.id,
            fixture.unknown_token_id,
            metadata.vocabulary.unknown_token_id()
        ));
    }
    if metadata.add_bos != fixture.add_bos || metadata.add_eos != fixture.add_eos {
        return Err(format!(
            "tokenizer fixture `{}` special-token defaults mismatch: expected add_bos={} add_eos={}, got add_bos={} add_eos={}",
            fixture.id, fixture.add_bos, fixture.add_eos, metadata.add_bos, metadata.add_eos
        ));
    }
    for sample in fixture.sample_tokens {
        let actual = metadata.vocabulary.token(sample.token_id);
        if actual != Some(sample.token) {
            return Err(format!(
                "tokenizer fixture `{}` sample token mismatch at {:?}: expected {:?}, got {:?}",
                fixture.id, sample.token_id, sample.token, actual
            ));
        }
    }
    Ok(())
}

/// Asserts that an observed raw template matches a golden prompt-template fixture.
pub fn assert_prompt_template_fixture_matches(
    fixture: &GoldenPromptTemplateVariant,
    actual_template: &str,
) -> Result<(), String> {
    let actual_digest = digest_chat_template(actual_template);
    if actual_digest != fixture.template_digest {
        return Err(format!(
            "prompt template fixture `{}` digest mismatch: expected {}, got {}",
            fixture.id, fixture.template_digest, actual_digest
        ));
    }
    if let Some(raw_template) = fixture.raw_template {
        if raw_template != actual_template {
            return Err(format!(
                "prompt template fixture `{}` raw template mismatch",
                fixture.id
            ));
        }
    }
    Ok(())
}

/// Asserts an exact rendered prompt against the golden fixture output.
pub fn assert_rendered_prompt_case(
    fixture: &GoldenPromptRenderCase,
    actual_rendered: &str,
) -> Result<(), String> {
    if actual_rendered != fixture.expected_rendered {
        return Err(format!(
            "render case `{}` mismatch: expected {:?}, got {:?}",
            fixture.id, fixture.expected_rendered, actual_rendered
        ));
    }
    Ok(())
}

/// Asserts that a rendered prompt meets the scenario's configured byte budget.
pub fn assert_prompt_window_case(
    fixture: &GoldenPromptWindowCase,
    actual_rendered: &str,
) -> Result<(), String> {
    let actual_over_budget = actual_rendered.len() > fixture.max_rendered_bytes;
    if actual_over_budget != fixture.expected_over_budget {
        return Err(format!(
            "window case `{}` mismatch: rendered {} bytes with budget {}, expected over_budget={}, got {}",
            fixture.id,
            actual_rendered.len(),
            fixture.max_rendered_bytes,
            fixture.expected_over_budget,
            actual_over_budget
        ));
    }
    Ok(())
}

fn pretokenizer_fixture_label(pretokenizer: &GgufTokenizerPretokenizer) -> Cow<'static, str> {
    match pretokenizer {
        GgufTokenizerPretokenizer::Default => Cow::Borrowed("default"),
        GgufTokenizerPretokenizer::Llama => Cow::Borrowed("llama"),
        GgufTokenizerPretokenizer::Qwen2 => Cow::Borrowed("qwen2"),
        GgufTokenizerPretokenizer::Refact => Cow::Borrowed("refact"),
        GgufTokenizerPretokenizer::Tekken => Cow::Borrowed("tekken"),
        GgufTokenizerPretokenizer::Custom(value) => Cow::Owned(value.clone()),
    }
}
