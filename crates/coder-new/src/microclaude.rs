//! Microclaude: the mode that runs turns on the Claude Code login this
//! computer already holds, offered only when it has that account login.
//!
//! The mode is a placeholder: it answers one sentence and runs nothing.
//! Whether it may be offered is read through
//! [`crate::bundled_runtime::claude_print`] and [`coder_engine_status`],
//! which report the credential's kind and never the credential.

use std::sync::OnceLock;

/// The mode's name, in the rail and in the conversation.
pub const LABEL: &str = "microclaude";

/// What Shift+Tab says on a computer with no Claude account login.
pub const NEEDS_LOGIN: &str = "microclaude needs Claude Code signed in on this computer.";

/// What the conversation answers until the mode runs turns of its own.
pub const ANSWER: &str = "microclaude coming soon";

static AVAILABLE: OnceLock<bool> = OnceLock::new();

/// Whether this computer has Claude Code on a Claude account login: the
/// `claude` program is present, and it is signed in to a Claude account
/// rather than with an API key or a cloud credential.
///
/// The answer is read once per process, because it runs `claude auth
/// status`, which is the command that names the credential's kind. A chat
/// here runs interactively on the person's plan, not on the plan's
/// automated-run book, so it never takes a plan slot.
#[must_use]
pub fn available() -> bool {
    *AVAILABLE.get_or_init(|| {
        let Some(agent) = crate::bundled_runtime::claude_print::agent() else {
            return false;
        };
        let status =
            coder_engine_status::claude::check(&agent.program, None, atif::now_ms() / 1_000);
        matches!(
            status.state,
            coder_engine_status::State::SignedIn | coder_engine_status::State::Expiring
        ) && matches!(
            status.method,
            Some(coder_engine_status::Method::ClaudeAi | coder_engine_status::Method::OauthToken)
        )
    })
}

#[cfg(test)]
mod tests {
    use coder_engine_status::{Method, State, claude};

    /// The decision [`super::available`] makes, over a status this test
    /// builds instead of one `claude auth status` answered.
    fn offered(state: State, method: Option<Method>) -> bool {
        matches!(state, State::SignedIn | State::Expiring)
            && matches!(method, Some(Method::ClaudeAi | Method::OauthToken))
    }

    fn classify(method: &str) -> coder_engine_status::Status {
        claude::classify(
            Some(&claude::Output {
                exit: Some(0),
                stdout: format!(r#"{{"loggedIn":true,"authMethod":"{method}"}}"#),
            }),
            None,
            1_000,
        )
    }

    #[test]
    fn only_a_claude_account_login_offers_the_mode() {
        for (method, wanted) in [
            ("claude.ai", true),
            ("oauth_token", true),
            ("api_key", false),
            ("api_key_helper", false),
            ("third_party", false),
        ] {
            let status = classify(method);
            assert_eq!(offered(status.state, status.method), wanted, "{method}");
        }
        // Signing out with nothing else configured offers nothing.
        let signed_out = claude::classify(
            Some(&claude::Output {
                exit: Some(1),
                stdout: r#"{"loggedIn":false,"authMethod":"none"}"#.into(),
            }),
            None,
            1_000,
        );
        assert_eq!(signed_out.state, State::SignedOut);
        assert!(!offered(signed_out.state, signed_out.method));
    }
}
