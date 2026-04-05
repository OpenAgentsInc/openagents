#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlashCommandId {
    Help,
    Chat,
    Download,
    Relay,
    Wallet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SlashCommandSpec {
    pub id: SlashCommandId,
    pub name: &'static str,
    pub usage: &'static str,
    pub summary: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParsedSubmission {
    Prompt(String),
    Command {
        spec: &'static SlashCommandSpec,
        args: String,
        raw: String,
    },
    UnknownCommand {
        name: String,
        raw: String,
    },
}

const COMMANDS: &[SlashCommandSpec] = &[
    SlashCommandSpec {
        id: SlashCommandId::Help,
        name: "help",
        usage: "/help",
        summary: "show the retained Pylon command surface",
    },
    SlashCommandSpec {
        id: SlashCommandId::Chat,
        name: "chat",
        usage: "/chat [prompt]",
        summary: "submit a local Gemma prompt explicitly",
    },
    SlashCommandSpec {
        id: SlashCommandId::Download,
        name: "download",
        usage: "/download [model]",
        summary: "download a curated Gemma GGUF into the local cache",
    },
    SlashCommandSpec {
        id: SlashCommandId::Relay,
        name: "relay",
        usage: "/relay [list|add|remove|refresh]",
        summary: "inspect or update the retained relay set",
    },
    SlashCommandSpec {
        id: SlashCommandId::Wallet,
        name: "wallet",
        usage: "/wallet [status|balance|address|invoice|pay|history]",
        summary: "run retained Spark wallet commands inside the shell",
    },
];

pub fn registry() -> &'static [SlashCommandSpec] {
    COMMANDS
}

pub fn parse_submission(text: &str) -> ParsedSubmission {
    let trimmed = text.trim();
    let Some(remainder) = trimmed.strip_prefix('/') else {
        return ParsedSubmission::Prompt(trimmed.to_string());
    };
    let mut parts = remainder.splitn(2, char::is_whitespace);
    let name = parts.next().unwrap_or_default().trim();
    let args = parts.next().unwrap_or_default().trim().to_string();
    if let Some(spec) = registry().iter().find(|spec| spec.name == name) {
        return ParsedSubmission::Command {
            spec,
            args,
            raw: trimmed.to_string(),
        };
    }
    ParsedSubmission::UnknownCommand {
        name: name.to_string(),
        raw: trimmed.to_string(),
    }
}

pub fn help_lines() -> Vec<String> {
    let mut lines = vec![String::from("Available commands:")];
    lines.extend(
        registry()
            .iter()
            .map(|spec| format!("  {:<24} {}", spec.usage, spec.summary)),
    );
    lines.push(String::from(
        "Plain text without a slash runs a local Gemma prompt.",
    ));
    lines
}

#[cfg(test)]
mod tests {
    use super::{ParsedSubmission, SlashCommandId, help_lines, parse_submission, registry};

    #[test]
    fn registry_includes_retained_commands() {
        let names = registry().iter().map(|spec| spec.name).collect::<Vec<_>>();
        assert_eq!(names, vec!["help", "chat", "download", "relay", "wallet"]);
    }

    #[test]
    fn parse_submission_keeps_plain_prompt_behavior() {
        assert_eq!(
            parse_submission("hello there"),
            ParsedSubmission::Prompt(String::from("hello there"))
        );
    }

    #[test]
    fn parse_submission_resolves_known_command() {
        let parsed = parse_submission("/download gemma-3-4b");
        assert_eq!(
            parsed,
            ParsedSubmission::Command {
                spec: &registry()[2],
                args: String::from("gemma-3-4b"),
                raw: String::from("/download gemma-3-4b"),
            }
        );
    }

    #[test]
    fn parse_submission_reports_unknown_command() {
        assert_eq!(
            parse_submission("/plan this"),
            ParsedSubmission::UnknownCommand {
                name: String::from("plan"),
                raw: String::from("/plan this"),
            }
        );
    }

    #[test]
    fn help_copy_mentions_plain_text_chat() {
        let lines = help_lines();
        assert!(lines.iter().any(|line| line.contains("/help")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("Plain text without a slash"))
        );
        assert_eq!(registry()[1].id, SlashCommandId::Chat);
        assert_eq!(registry()[3].id, SlashCommandId::Relay);
        assert_eq!(registry()[4].id, SlashCommandId::Wallet);
    }
}
