//! Cheap local classifier for the default Flash lane.
//!
//! Short greetings and chit-chat ride Gemini 3.7 Flash. Anything that looks
//! like coding work stays on GLM 5.3 Flash. Character-trigram cosine against
//! a handful of greeting prototypes; no network, no extra crate.

use std::collections::HashMap;

/// Default Flash model for work that needs a thinking lane.
pub const THOUGHTFUL_FLASH: &str = "glm-5.3-flash";
/// Faster Flash model for greetings and other trivial turns.
pub const SIMPLE_FLASH: &str = "gemini-3.7-flash";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptClass {
    Simple,
    Thoughtful,
}

impl PromptClass {
    pub fn model(self) -> &'static str {
        match self {
            Self::Simple => SIMPLE_FLASH,
            Self::Thoughtful => THOUGHTFUL_FLASH,
        }
    }
}

const SIMPLE_PROTOTYPES: &[&str] = &[
    "hey",
    "hi",
    "hello",
    "hey there",
    "hi there",
    "hello there",
    "yo",
    "sup",
    "what's up",
    "whats up",
    "how are you",
    "who are you",
    "thanks",
    "thank you",
    "ok",
    "okay",
    "yes",
    "no",
    "cool",
    "nice",
    "ping",
    "good morning",
    "good night",
    "gm",
    "lol",
];

const HEAVY_WORDS: &[&str] = &[
    "implement",
    "refactor",
    "debug",
    "compile",
    "traceback",
    "stacktrace",
    "function",
    "module",
    "crate",
    "architecture",
    "codebase",
    "repository",
    "migrate",
    "patch",
    "algorithm",
    "performance",
    "deadlock",
    "schema",
    "typeerror",
    "grep",
    "delegate",
    "fix",
    "search",
    "panic",
];

const COSINE_THRESHOLD: f64 = 0.42;

/// Classify one user prompt. Conservative: doubtful prompts stay thoughtful.
pub fn classify_prompt(text: &str) -> PromptClass {
    let normalized = normalize(text);
    if normalized.is_empty() {
        return PromptClass::Simple;
    }
    if looks_heavy(&normalized, text) {
        return PromptClass::Thoughtful;
    }
    if SIMPLE_PROTOTYPES.contains(&normalized.as_str()) {
        return PromptClass::Simple;
    }
    let query = trigrams(&normalized, 3);
    let mut best = 0.0_f64;
    for prototype in SIMPLE_PROTOTYPES {
        let score = cosine(&query, &trigrams(prototype, 3));
        if score > best {
            best = score;
        }
    }
    if best >= COSINE_THRESHOLD {
        PromptClass::Simple
    } else {
        PromptClass::Thoughtful
    }
}

/// When the grant is the default GLM Flash lane, pick Gemini instead for a
/// simple last user turn. Other grants are left alone (Explore pin, Free).
pub fn maybe_simple_flash(grant_model: &str, last_user: &str) -> Option<&'static str> {
    if grant_model != THOUGHTFUL_FLASH {
        return None;
    }
    match classify_prompt(last_user) {
        PromptClass::Simple => Some(SIMPLE_FLASH),
        PromptClass::Thoughtful => None,
    }
}

/// Last `role=user` text from an OpenAI chat-completions `messages` array.
pub fn last_user_text(messages: &serde_json::Value) -> Option<String> {
    let array = messages.as_array()?;
    for message in array.iter().rev() {
        let role = message.get("role").and_then(|value| value.as_str())?;
        if role != "user" {
            continue;
        }
        return message_text(message.get("content"));
    }
    None
}

fn message_text(content: Option<&serde_json::Value>) -> Option<String> {
    let content = content?;
    if let Some(text) = content.as_str() {
        let text = text.trim();
        return (!text.is_empty()).then(|| text.to_string());
    }
    let mut parts = String::new();
    for part in content.as_array()? {
        if let Some(text) = part.as_str() {
            parts.push_str(text);
        } else if part.get("type").and_then(|value| value.as_str()) == Some("text") {
            if let Some(text) = part.get("text").and_then(|value| value.as_str()) {
                parts.push_str(text);
            }
        }
    }
    let parts = parts.trim();
    (!parts.is_empty()).then(|| parts.to_string())
}

fn looks_heavy(normalized: &str, original: &str) -> bool {
    if original.contains("```") {
        return true;
    }
    if original.contains('/') && original.contains('.') {
        return true;
    }
    if normalized.chars().count() > 160 {
        return true;
    }
    let padded = format!(" {normalized} ");
    HEAVY_WORDS
        .iter()
        .any(|word| padded.contains(&format!(" {word} ")))
}

fn normalize(text: &str) -> String {
    let mut out = String::new();
    let mut last_space = true;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_space = false;
        } else if !last_space {
            out.push(' ');
            last_space = true;
        }
    }
    out.trim().to_string()
}

fn trigrams(text: &str, n: usize) -> HashMap<String, f64> {
    let chars: Vec<char> = text.chars().collect();
    let mut counts = HashMap::new();
    if chars.len() < n {
        if !chars.is_empty() {
            counts.insert(chars.iter().collect(), 1.0);
        }
        return counts;
    }
    for window in chars.windows(n) {
        let gram: String = window.iter().collect();
        *counts.entry(gram).or_insert(0.0) += 1.0;
    }
    counts
}

fn cosine(left: &HashMap<String, f64>, right: &HashMap<String, f64>) -> f64 {
    let mut dot = 0.0;
    let mut left_norm = 0.0;
    let mut right_norm = 0.0;
    for (key, value) in left {
        left_norm += value * value;
        if let Some(other) = right.get(key) {
            dot += value * other;
        }
    }
    for value in right.values() {
        right_norm += value * value;
    }
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm.sqrt() * right_norm.sqrt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hey_is_simple() {
        assert_eq!(classify_prompt("hey"), PromptClass::Simple);
        assert_eq!(classify_prompt("Hey!"), PromptClass::Simple);
        assert_eq!(classify_prompt("hi there"), PromptClass::Simple);
        assert_eq!(classify_prompt("thanks"), PromptClass::Simple);
        assert_eq!(classify_prompt("who are you"), PromptClass::Simple);
        assert_eq!(
            maybe_simple_flash(THOUGHTFUL_FLASH, "hey"),
            Some(SIMPLE_FLASH)
        );
    }

    #[test]
    fn coding_work_stays_thoughtful() {
        assert_eq!(
            classify_prompt("implement a cosine classifier in runtime.rs"),
            PromptClass::Thoughtful
        );
        assert_eq!(
            classify_prompt("fix the failing test in crates/openagents-cli"),
            PromptClass::Thoughtful
        );
        assert_eq!(classify_prompt("write a rust cli"), PromptClass::Thoughtful);
        assert_eq!(classify_prompt("add tests"), PromptClass::Thoughtful);
        assert_eq!(
            maybe_simple_flash(THOUGHTFUL_FLASH, "refactor the proxy"),
            None
        );
        assert_eq!(maybe_simple_flash(SIMPLE_FLASH, "hey"), None);
    }

    #[test]
    fn last_user_text_reads_openai_messages() {
        let messages = json!([
            {"role": "system", "content": "you are coder"},
            {"role": "user", "content": "hey"}
        ]);
        assert_eq!(last_user_text(&messages).as_deref(), Some("hey"));
    }
}
