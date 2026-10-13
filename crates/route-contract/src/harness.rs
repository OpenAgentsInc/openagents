//! The harness contract (#11451, #11452): the engines a Coder turn can run
//! on, each declared the same way so every client of the router (Coder in
//! the terminal, the phone, the web) reads one table.
//!
//! A harness is what runs a piece of work: Claude Code on the person's
//! Claude plan, Coder's own loop on the OpenAgents gateway, Codex on the
//! person's ChatGPT login. Each one declares where it runs, what it can
//! do, and how it takes a chat's turn. The health checks and the candidate
//! filter that read a computer's own state live with Coder
//! (`coder_new::harness_plugin`); where a turn goes when the chat is off
//! the computer is [`crate::placement`].

use serde::{Deserialize, Serialize};

/// A harness's identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Harness {
    /// Claude Code through the Claude Agent SDK: one long-lived process per
    /// chat, every call approved, the person's settings loaded, Coder's
    /// tools over MCP.
    ClaudeCode,
    /// OpenCode on this computer (#11507): its free `opencode/big-pickle`
    /// model or another model the person's OpenCode reaches, one OpenCode
    /// session per chat, every tool checked by Coder before it runs.
    #[serde(rename = "opencode")]
    OpenCode,
    /// Coder's own loop on the OpenAgents gateway, or on the person's
    /// OpenRouter key, with Coder's tools.
    CoderLoop,
    /// Codex on the person's ChatGPT login, through `codex app-server`:
    /// one long-lived thread per chat, Codex's approvals answered by
    /// Coder's hooks, Coder's tools in process (#11510).
    Codex,
}

/// Where a harness runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Place {
    /// The computer the chat itself is on (the terminal, the desktop app).
    ThisComputer,
    /// The person's computer, reached from a phone or the web as a task
    /// on it (NIP-HOST).
    LinkedComputer,
    /// OpenAgents' own servers.
    Gateway,
}

/// What a harness can do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Can {
    Answer,
    RepoEdits,
    Shell,
    IssueToPullRequest,
    BackgroundWork,
    Images,
}

/// How a harness takes a chat's turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Turn {
    /// A long-lived session per chat, which later turns continue.
    Session,
    /// One request per turn over the chat's transcript.
    Transcript,
    /// No chat turn yet; the harness runs only as background work.
    NotYet,
}

/// One harness's declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Declaration {
    pub harness: Harness,
    pub name: &'static str,
    /// The plugin in /plugins that turns it on, when one does.
    pub plugin: Option<&'static str>,
    pub place: Place,
    pub can: &'static [Can],
    pub turn: Turn,
}

/// Every harness Coder knows, in the order a fixed rule prefers them:
/// Claude Code, then OpenCode, then Coder's own loop (#11507).
pub const DECLARATIONS: [Declaration; 4] = [
    Declaration {
        harness: Harness::ClaudeCode,
        name: "Claude Code",
        plugin: Some("claude-code"),
        place: Place::ThisComputer,
        can: &[
            Can::Answer,
            Can::RepoEdits,
            Can::Shell,
            Can::IssueToPullRequest,
            Can::BackgroundWork,
            Can::Images,
        ],
        turn: Turn::Session,
    },
    Declaration {
        harness: Harness::OpenCode,
        name: "OpenCode",
        plugin: Some("opencode"),
        place: Place::ThisComputer,
        can: &[
            Can::Answer,
            Can::RepoEdits,
            Can::Shell,
            Can::IssueToPullRequest,
            Can::BackgroundWork,
        ],
        turn: Turn::Session,
    },
    Declaration {
        harness: Harness::CoderLoop,
        name: "Coder",
        plugin: None,
        place: Place::Gateway,
        can: &[
            Can::Answer,
            Can::RepoEdits,
            Can::Shell,
            Can::IssueToPullRequest,
            Can::BackgroundWork,
        ],
        turn: Turn::Transcript,
    },
    Declaration {
        harness: Harness::Codex,
        name: "Codex",
        plugin: Some("codex"),
        place: Place::ThisComputer,
        can: &[
            Can::Answer,
            Can::RepoEdits,
            Can::Shell,
            Can::IssueToPullRequest,
            Can::BackgroundWork,
        ],
        turn: Turn::Session,
    },
];

impl Harness {
    /// This harness's declaration.
    #[must_use]
    pub fn declaration(self) -> &'static Declaration {
        DECLARATIONS
            .iter()
            .find(|declaration| declaration.harness == self)
            .expect("every harness is declared")
    }

    /// The coding agent's word in a computer's presence (NIP-REACH engine
    /// flags) for a harness that runs on the person's computer: `claude`
    /// or `codex`. `None` for the Coder loop, which every computer runs,
    /// and for OpenCode, which a chat reaches only on the computer it runs
    /// on (a phone's turn on a linked computer's OpenCode is not built).
    #[must_use]
    pub fn engine(self) -> Option<&'static str> {
        match self {
            Self::ClaudeCode => Some("claude"),
            Self::Codex => Some("codex"),
            Self::CoderLoop | Self::OpenCode => None,
        }
    }

    /// The harness a presence's exact engine word names, or `None` for an
    /// agent that is not a harness (such as Grok Build) or a word this
    /// build does not know.
    #[must_use]
    pub fn of_engine(word: &str) -> Option<Self> {
        DECLARATIONS
            .iter()
            .map(|declaration| declaration.harness)
            .find(|harness| harness.engine() == Some(word))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_harness_is_declared_once_and_engines_round_trip() {
        for harness in [
            Harness::ClaudeCode,
            Harness::OpenCode,
            Harness::CoderLoop,
            Harness::Codex,
        ] {
            assert_eq!(harness.declaration().harness, harness);
            if let Some(word) = harness.engine() {
                assert_eq!(Harness::of_engine(word), Some(harness));
            }
        }
        assert_eq!(Harness::of_engine("grok"), None);
        assert_eq!(Harness::of_engine("Claude"), None);
        assert_eq!(Harness::of_engine("opencode"), None);
        assert_eq!(
            serde_json::to_value(Harness::OpenCode).unwrap(),
            serde_json::json!("opencode")
        );
    }

    /// The fixed rule's order (#11507): Claude Code, OpenCode, Coder.
    #[test]
    fn claude_code_comes_first_then_opencode_then_the_coder_loop() {
        let order: Vec<Harness> = DECLARATIONS.iter().map(|d| d.harness).collect();
        assert_eq!(
            order,
            [
                Harness::ClaudeCode,
                Harness::OpenCode,
                Harness::CoderLoop,
                Harness::Codex
            ]
        );
    }
}
