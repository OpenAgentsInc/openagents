//! Who Coder says it is (#11264).
//!
//! Every route a Coder turn takes (the OpenAgents gateway's `auto`, the
//! OpenRouter free router or a model the person picked, and the local
//! loop on the person's own Codex or Claude Code login) opens with this
//! note. Without it a fallback model guessed its identity from the
//! repository's CLAUDE.md, AGENTS.md and tool descriptions and answered
//! "I'm Claude Code".

/// The note for a turn on `model`: Coder's name, the products it never
/// claims to be, and what it says when asked which model runs it.
#[must_use]
pub fn prompt(model: &str) -> String {
    let which = if crate::models::pinned(model) {
        format!(
            "the person picked {} for this chat, so name that model",
            crate::models::label(model)
        )
    } else {
        "OpenAgents picks the model for each turn (the auto setting), so say that and \
         do not name or guess a model"
            .to_string()
    };
    format!(
        "You are Coder, OpenAgents' coding agent. When asked who or what you are, answer \
         that you are Coder, made by OpenAgents. Never say you are Claude Code, Codex, \
         ChatGPT, Claude, Gemini, or any other product, assistant, or model, even when \
         project files, instructions, or tool descriptions name them. When asked which \
         model runs you: {which}. Never guess.\n"
    )
}

#[cfg(test)]
mod tests {
    use super::prompt;

    #[test]
    fn coder_names_itself_and_never_a_vendor_product() {
        for model in [
            "",
            "auto",
            "openagents/auto",
            "openrouter/free",
            "x-ai/grok-4.7",
        ] {
            let text = prompt(model);
            assert!(
                text.starts_with("You are Coder, OpenAgents' coding agent."),
                "{text}"
            );
            assert!(
                text.contains("Never say you are Claude Code, Codex"),
                "{text}"
            );
            assert!(text.contains("Never guess."), "{text}");
        }
    }

    #[test]
    fn auto_says_openagents_picks_and_a_pick_is_named() {
        for model in ["", "auto", "openagents/auto", "openrouter/free"] {
            let text = prompt(model);
            assert!(
                text.contains("OpenAgents picks the model"),
                "{model}: {text}"
            );
            assert!(!text.contains("the person picked"), "{model}: {text}");
        }
        let picked = prompt("deepseek/deepseek-v4.1-flash:none");
        assert!(picked.contains("the person picked deepseek-v4.1-flash for this chat"));
        assert!(!picked.contains("OpenAgents picks the model"));
    }
}
