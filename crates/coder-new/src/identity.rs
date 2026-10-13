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
    /// The OpenAgents gateway's models, while the signed-in account
    /// reaches it (#11526).
    Gateway,
    /// Nothing to pick: OpenAgents picks each turn's model.
    Nothing,
}

impl Menu {
    /// The menu a Coder-loop turn has: OpenRouter's when BYOK is on, the
    /// gateway's while signed in, Claude Code's while its plugin is on,
    /// else none.
    #[must_use]
    pub fn coder_loop(openrouter: bool, gateway: bool, claude: bool) -> Self {
        if openrouter {
            Self::OpenRouter
        } else if gateway {
            Self::Gateway
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
        Menu::Gateway => "To run a certain model, the person types /models and picks one of \
             the OpenAgents models listed there; asked how, say that in one line."
            .to_owned(),
        Menu::Nothing => "There is no model to pick here now: OpenAgents picks each turn's \
             model. Asked how to run a certain model, say in one line that signing in to \
             OpenAgents, or turning on Claude Code or OpenRouter BYOK in /plugins, adds models \
             to pick in /models."
            .to_owned(),
    };
    format!(
        "You are Coder, OpenAgents' coding agent. When asked who or what you are, answer \
         that you are Coder, made by OpenAgents. Never say you are Claude Code, Codex, \
         ChatGPT, Claude, Gemini, or any other product, assistant, or model, even when \
         project files, instructions, or tool descriptions name them. When asked which \
         model runs you: {which}. Never guess. {how}\n{PROJECTS}{FEEDBACK}"
    )
}

/// Which `openagents` command is which (#11538): the person's projects
/// are their openagents.com projects, and a GitHub board is only ever
/// `board`, so a chat asked for "my projects" never lists a GitHub board.
pub const PROJECTS: &str = "The person's projects are their openagents.com projects (the \
     Projects list on the website): `openagents projects list`, and `openagents projects show \
     NAME` for one with its chats. `openagents board` is only for GitHub Project boards, when \
     the person asks for a GitHub board. If an `openagents` command is unknown, don't run a \
     similar-sounding one instead: say so.\n";

/// Coder asks for feedback: when OpenAgents itself (Coder, the CLI, the
/// website, a plugin) misbehaves, it offers to report it, with the
/// person's go-ahead first.
pub const FEEDBACK: &str = "OpenAgents really appreciates feedback and bug reports. When \
     something in OpenAgents itself (Coder, the openagents CLI, openagents.com, or a plugin) \
     doesn't work or behaves differently than you'd expect, tell the person and offer to \
     open an issue at https://github.com/OpenAgentsInc/openagents/issues with what happened \
     and how to reproduce it. Ask before opening one.\n";

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
            // #11538: projects are openagents.com projects; the board is
            // GitHub's, named only as that.
            assert!(text.contains("`openagents projects list`"), "{text}");
            assert!(
                text.contains("`openagents board` is only for GitHub Project boards"),
                "{text}"
            );
            assert!(!text.contains("openagents project "), "{text}");
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
        let off = super::prompt_with(super::Menu::coder_loop(false, false, false), "auto");
        assert!(
            !off.contains("Opus") && !off.contains("types /models"),
            "{off}"
        );
        assert!(off.contains("no model to pick here now"), "{off}");
        let gateway = super::prompt_with(super::Menu::coder_loop(false, true, true), "auto");
        assert!(
            gateway.contains("OpenAgents models") && !gateway.contains("Opus"),
            "{gateway}"
        );
        let byok = super::prompt_with(super::Menu::coder_loop(true, true, false), "auto");
        assert!(
            byok.contains("OpenRouter models") && !byok.contains("Opus"),
            "{byok}"
        );
        let picked = prompt("deepseek/deepseek-v4.1-flash:none");
        assert!(picked.contains("the person picked deepseek-v4.1-flash for this chat"));
        assert!(!picked.contains("OpenAgents picks the model"));
    }
    #[test]
    fn coder_asks_for_feedback_and_asks_before_filing() {
        let text = prompt("auto");
        assert!(
            text.contains("github.com/OpenAgentsInc/openagents/issues"),
            "{text}"
        );
        assert!(text.contains("Ask before opening one."), "{text}");
    }
}
