//! Claude Code as a Cloud computer engine, under the bring-your-own-Claude
//! policy (`docs/cloud/claude-code-byo.md`).
//!
//! The runtime image installs the published Claude Code release, pinned to
//! [`VERSION`] and unmodified, at [`PROGRAM`]. The user signs in by running
//! that program in their own computer's granted terminal and completing
//! Anthropic's flow there; the login stays in that computer's isolated home.
//! No OpenAgents field, API, operator profile, or Coder credential path
//! accepts a claude.ai login, and evidence never carries one.

/// The executor name an operator profile and the Coder runtime use.
pub const ENGINE: &str = "claude";

/// The published npm package the runtime image installs, unmodified.
pub const PACKAGE: &str = "@anthropic-ai/claude-code";

/// The pinned Claude Code release. `scripts/cloud/coder-host-setup.sh`
/// installs exactly this version; a test keeps the two equal.
pub const VERSION: &str = "2.1.295";

/// Where the runtime image installs the binary (`npm install -g --prefix
/// /usr/local`). The sign-in terminal runs this exact program, with no
/// arguments, so every sign-in method the binary offers stays available.
pub const PROGRAM: &str = coder_engine_status::claude::PROGRAM;

/// The only engine credentials a profile may inject for Claude Code: the
/// user's own Anthropic API key (a separate custody class, rule 8). A
/// profile with none uses the login inside the user's computer.
pub const API_KEY: &str = "ANTHROPIC_API_KEY";

/// The plain-text engine label. No Anthropic or Claude Code logo is used.
pub const LABEL: &str = "This computer runs Claude Code.";

/// Refuse a credential name that would carry a claude.ai login.
///
/// # Errors
/// For `CLAUDE_CODE_OAUTH_TOKEN`, which holds a claude.ai OAuth or
/// `claude setup-token` value that OpenAgents may not collect or inject.
pub fn admit_name(name: &str) -> crate::Result<()> {
    if name.eq_ignore_ascii_case(secret_screen::CLAUDE_CODE_OAUTH_TOKEN) {
        return Err(REFUSAL.into());
    }
    Ok(())
}

/// Refuse a credential value that is a claude.ai login.
///
/// # Errors
/// When the value is a claude.ai OAuth token, a `claude setup-token`
/// value, or Claude Code's credentials document.
pub fn admit_value(value: &str) -> crate::Result<()> {
    if secret_screen::claude_login_in(value) {
        return Err(REFUSAL.into());
    }
    Ok(())
}

/// Why a claude.ai login was refused.
pub const REFUSAL: &str = "OpenAgents does not accept a Claude.ai login or token. Sign in to Claude inside your computer's terminal instead.";

/// Whether an artifact path is Claude Code's login file.
#[must_use]
pub fn login_path(path: &str) -> bool {
    secret_screen::claude_login_path(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_image_installs_the_pinned_unmodified_release() {
        let script = include_str!("../../../scripts/cloud/coder-host-setup.sh");
        assert!(
            script.contains(&format!("CLAUDE_CODE_VERSION=\"{VERSION}\"")),
            "the image pin and the engine pin differ"
        );
        assert!(script.contains(&format!("\"{PACKAGE}@${{CLAUDE_CODE_VERSION}}\"")));
        assert!(script.contains("--prefix /usr/local"));
        assert!(PROGRAM.starts_with('/'));
    }

    #[test]
    fn claude_logins_are_refused_as_injectable_credentials() {
        assert!(admit_name("CLAUDE_CODE_OAUTH_TOKEN").is_err());
        assert!(admit_name("claude_code_oauth_token").is_err());
        assert!(admit_name(API_KEY).is_ok());
        let setup_token = format!("sk-ant-oat01-{}", "x1".repeat(40));
        assert_eq!(admit_value(&setup_token), Err(REFUSAL.into()));
        assert!(admit_value(r#"{"claudeAiOauth":{"accessToken":"a"}}"#).is_err());
        assert!(admit_value(&format!("sk-ant-api03-{}", "k2".repeat(40))).is_ok());
        assert!(login_path(".claude/.credentials.json"));
        assert!(!LABEL.contains("logo"));
    }
}
