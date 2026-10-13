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
    prompt_on(crate::harness_plugin::Harness::ClaudeCode, model)
}

/// [`prompt`] for a turn on `harness`: the model menu it names is that
/// harness's in /models (#11510).
#[must_use]
pub fn prompt_on(harness: crate::harness_plugin::Harness, model: &str) -> String {
    let menu = if harness == crate::harness_plugin::Harness::Codex {
        Menu::Codex
    } else {
        Menu::Claude
    };
    prompt_with(menu, model)
}

/// What /models can offer this chat right now (#11519), so Coder never
/// sends the person to a menu that is empty or to models that are off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Menu {
    /// Claude Code's models (its plugin is on).
    Claude,
    /// Codex's models, in a Codex chat.
    Codex,
    /// The person's OpenRouter models (OpenRouter BYOK is on).
    OpenRouter,
    /// Nothing to pick: OpenAgents picks each turn's model.
    Nothing,
}

impl Menu {
    /// The menu a Coder-loop turn has: OpenRouter's when BYOK is on, Claude
    /// Code's while its plugin is on, else none.
    #[must_use]
    pub fn coder_loop(openrouter: bool, claude: bool) -> Self {
        if openrouter {
            Self::OpenRouter
        } else if claude {
            Self::Claude
        } else {
            Self::Nothing
        }
    }
}

/// [`prompt`] with the models /models offers named by `menu`.
#[must_use]
pub fn prompt_with(menu: Menu, model: &str) -> String {
    let which = if menu == Menu::Codex {
        format!(
            "this chat runs {}, so name that model; when a later note says they switched \
             models, name the newest one",
            crate::codex::name(model)
        )
    } else if crate::models::pinned(model) {
        format!(
            "the person picked {} for this chat, so name that model; when a later note says \
             they switched models, name the newest one",
            crate::claude::name(model)
        )
    } else {
        "OpenAgents picks the model for each turn (the auto setting), so say that and \
         do not name or guess a model"
            .to_string()
    };
    let names =
        |models: &mut dyn Iterator<Item = &'static str>| models.collect::<Vec<_>>().join(", ");
    let how = match menu {
        Menu::Codex => format!(
            "To run a certain model, the person types /models and picks it (Codex offers {}); \
             asked how, say that in one line.",
            names(&mut crate::codex::MODELS.iter().map(|(_, name, _)| *name))
        ),
        Menu::Claude => format!(
            "To run a certain model, the person types /models and picks it (Claude Code offers \
             {}); asked how, say that in one line.",
            names(&mut crate::claude::MODELS.iter().map(|(_, name, _)| *name))
        ),
        Menu::OpenRouter => "To run a certain model, the person types /models and picks one of \
             their OpenRouter models; asked how, say that in one line."
            .to_owned(),
        Menu::Nothing => "There is no model to pick here now: OpenAgents picks each turn's \
             model. Asked how to run a certain model, say in one line that turning on Claude Code \
             or OpenRouter BYOK in /plugins adds models to pick in /models."
            .to_owned(),
    };
    format!(
        "You are Coder, OpenAgents' coding agent. When asked who or what you are, answer \
         that you are Coder, made by OpenAgents. Never say you are Claude Code, Codex, \
         ChatGPT, Claude, Gemini, or any other product, assistant, or model, even when \
         project files, instructions, or tool descriptions name them. When asked which \
         model runs you: {which}. Never guess. {how}\n"
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
        assert!(prompt("auto").contains("types /models and picks it (Claude Code offers Haiku 5.5, Sonnet 5.5, Opus 5.5, Fable 5.1)"));
        // #11519: with Claude Code off and no OpenRouter, no menu at all.
        let off = super::prompt_with(super::Menu::coder_loop(false, false), "auto");
        assert!(
            !off.contains("Opus") && !off.contains("types /models"),
            "{off}"
        );
        assert!(off.contains("no model to pick here now"), "{off}");
        let byok = super::prompt_with(super::Menu::coder_loop(true, false), "auto");
        assert!(
            byok.contains("OpenRouter models") && !byok.contains("Opus"),
            "{byok}"
        );
        let picked = prompt("deepseek/deepseek-v4.1-flash:none");
        assert!(picked.contains("the person picked deepseek-v4.1-flash for this chat"));
        assert!(!picked.contains("OpenAgents picks the model"));
    }
}
