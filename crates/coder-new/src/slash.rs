//! Slash commands and their suggestions above the composer.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

use crate::theme as t;

/// A terminal command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    Demo,
    Plugins,
    Appearance,
    Models,
    Export,
    Resume,
    Brainstorm,
    Login,
    Logout,
    Sync,
    Memory,
    Agents,
    Help,
}

/// Commands in the order shown by the picker.
pub const ALL: &[Command] = if crate::DEMO_AVAILABLE {
    &[
        Command::Demo,
        Command::Plugins,
        Command::Appearance,
        Command::Models,
        Command::Export,
        Command::Resume,
        Command::Brainstorm,
        Command::Login,
        Command::Logout,
        Command::Sync,
        Command::Memory,
        Command::Agents,
        Command::Help,
    ]
} else {
    &[
        Command::Plugins,
        Command::Appearance,
        Command::Models,
        Command::Export,
        Command::Resume,
        Command::Brainstorm,
        Command::Login,
        Command::Logout,
        Command::Sync,
        Command::Memory,
        Command::Agents,
        Command::Help,
    ]
};

impl Command {
    pub const ALL: &'static [Self] = ALL;

    /// The command word without its leading slash.
    pub const fn word(self) -> &'static str {
        match self {
            Self::Demo => "demo",
            Self::Plugins => "plugins",
            Self::Appearance => "appearance",
            Self::Models => "models",
            Self::Export => "export",
            Self::Resume => "resume",
            Self::Brainstorm => "brainstorm",
            Self::Login => "login",
            Self::Logout => "logout",
            Self::Sync => "sync",
            Self::Memory => "memory",
            Self::Agents => "agents",
            Self::Help => "help",
        }
    }

    /// The action described by the picker in the current demo mode.
    pub const fn about(self, demo: bool) -> &'static str {
        match self {
            Self::Demo if demo => "Turn demo off",
            Self::Demo => "Turn demo on",
            Self::Plugins => "Manage plugins",
            Self::Appearance => "Configure terminal appearance",
            Self::Models => "Choose model and reasoning level",
            Self::Export => "Export this conversation as ATIF",
            Self::Resume => "Resume a saved conversation",
            Self::Brainstorm => "Explicit public profile or reputation lookup",
            Self::Login => "Sign in to your openagents.com account",
            Self::Logout => "Sign out of your openagents.com account",
            Self::Sync => "Save chats to your account",
            Self::Memory => "Show what Coder remembers",
            Self::Agents => "Background agents: list, stop, message, resume",
            Self::Help => "Show commands and keys",
        }
    }
}

/// Commands matching a leading slash and an unfinished lowercase word.
pub fn matches(text: &str) -> Vec<Command> {
    let Some(word) = command_prefix(text) else {
        return Vec::new();
    };
    ALL.iter()
        .copied()
        .filter(|command| command.word().starts_with(word))
        .collect()
}

/// A known command written as an exact `/word`.
pub fn parse(text: &str) -> Option<Command> {
    if !is_command_word(text) {
        return None;
    }
    let word = &text[1..];
    ALL.iter().copied().find(|command| command.word() == word)
}

/// Help for the commands available in this build.
pub fn help() -> String {
    let mut text = String::new();
    if crate::DEMO_AVAILABLE {
        text.push_str("/demo  Toggle demo/live\n");
    }
    text.push_str("/plugins  Manage plugins\n/appearance  Configure terminal appearance\n/models  Choose a model for an enabled provider\n/export [path]  Save this conversation to a file (ATIF format)\n/resume [number|id]  Resume a saved conversation\n/login  Sign in to your openagents.com account\n/logout  Sign out of it\n/sync on|all|off|delete  Save chats to your account\n/memory [forget NAME]  Show or delete what Coder remembers\n/agents  Background agents: list, stop, message, resume\n/agent ENGINE TASK  Start a background agent in its own worktree\n/help  Show commands\nTab  Complete a command\nEsc  Close suggestions or stop a reply\nCtrl+C  Quit");
    text.push_str("\n/brainstorm search <public query>  Search public profiles\n/brainstorm rank <hex-or-npub>  Look up a profile's influence score\nBrainstorm sends only what you type after the command to its website.");
    text
}

/// Whether text is a complete command word, including an unknown one.
pub fn is_command_word(text: &str) -> bool {
    command_prefix(text).is_some_and(|word| !word.is_empty())
}

fn command_prefix(text: &str) -> Option<&str> {
    let word = text.strip_prefix('/')?;
    word.bytes()
        .all(|byte| byte.is_ascii_lowercase())
        .then_some(word)
}

/// Draw suggestions in the rows supplied immediately above the composer.
pub fn render(frame: &mut Frame, area: Rect, hints: &[Command], selected: usize, demo: bool) {
    if area.width == 0 || area.height == 0 || hints.is_empty() {
        return;
    }
    let shown = hints.len().min(usize::from(area.height));
    let selected = selected.min(hints.len() - 1);
    let first = selected.saturating_sub(shown - 1);
    let usage_width = hints
        .iter()
        .map(|command| command.word().len() + 1)
        .max()
        .unwrap_or(0);
    frame.render_widget(
        Block::default().style(Style::default().bg(t::BG_BASE)),
        area,
    );
    let top = area.bottom().saturating_sub(shown as u16);
    for (offset, command) in hints.iter().enumerate().skip(first).take(shown) {
        let active = offset == selected;
        let background = if active { t::BG_LIGHT } else { t::BG_BASE };
        let usage = format!("/{}", command.word());
        let line = Line::from(vec![
            Span::styled(
                if active { " ❯ " } else { "   " },
                Style::default().fg(t::ACCENT_MODEL),
            ),
            Span::styled(
                format!("{usage:<usage_width$}  "),
                Style::default()
                    .fg(if active {
                        t::TEXT_PRIMARY
                    } else {
                        t::TEXT_SECONDARY
                    })
                    .add_modifier(if active {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
            Span::styled(command.about(demo), Style::default().fg(t::GRAY)),
        ])
        .style(Style::default().bg(background));
        let row = Rect {
            y: top + (offset - first) as u16,
            height: 1,
            ..area
        };
        frame.render_widget(Paragraph::new(line), row);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestions_accept_only_a_leading_slash_and_lowercase_prefix() {
        assert_eq!(matches("/").as_slice(), ALL);
        assert_eq!(
            matches("/d").contains(&Command::Demo),
            crate::DEMO_AVAILABLE
        );
        assert_eq!(matches("/plugins"), [Command::Plugins]);
        assert!(matches("/unknown").is_empty());
        for text in ["", "demo", " /d", "/D", "/d ", "/demo on", "/usr/bin"] {
            assert!(matches(text).is_empty(), "{text:?}");
        }
    }

    #[test]
    fn submission_distinguishes_known_commands_unknown_words_and_messages() {
        for &command in ALL {
            let text = format!("/{}", command.word());
            assert_eq!(parse(&text), Some(command));
            assert!(is_command_word(&text));
        }
        assert_eq!(parse("/unknown"), None);
        assert!(is_command_word("/unknown"));
        assert_eq!(parse("/dem"), None);
        assert!(is_command_word("/dem"));
        for text in [
            "", "/", "/Help", "/demo on", "/usr/bin", " /demo", "/demo ", "help",
        ] {
            assert_eq!(parse(text), None, "{text:?}");
            assert!(!is_command_word(text), "{text:?}");
        }
    }

    #[test]
    fn demo_availability_agrees_across_commands_help_and_default_mode() {
        assert_eq!(parse("/demo").is_some(), crate::DEMO_AVAILABLE);
        assert_eq!(help().contains("/demo"), crate::DEMO_AVAILABLE);
        assert_eq!(ALL.contains(&Command::Demo), crate::DEMO_AVAILABLE);
        assert_eq!(
            crate::Mode::default() == crate::Mode::Demo,
            crate::DEMO_AVAILABLE
        );
    }
}
