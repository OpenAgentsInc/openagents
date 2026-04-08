#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlashCommandId {
    Help,
    Chat,
    Download,
    Model,
    Uninstall,
    Announce,
    Provider,
    Job,
    Jobs,
    Earnings,
    Receipts,
    Activity,
    Payout,
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
        id: SlashCommandId::Model,
        name: "model",
        usage: "/model [model]",
        summary: "target a Gemma model for local runtime use",
    },
    SlashCommandSpec {
        id: SlashCommandId::Uninstall,
        name: "uninstall",
        usage: "/uninstall [model]",
        summary: "remove a Gemma model from local cache and runtime",
    },
    SlashCommandSpec {
        id: SlashCommandId::Announce,
        name: "announce",
        usage: "/announce [show|publish|refresh]",
        summary: "inspect or publish the retained NIP-89 provider announcement",
    },
    SlashCommandSpec {
        id: SlashCommandId::Provider,
        name: "provider",
        usage: "/provider [scan|run] [--seconds <n>]",
        summary: "scan or process retained inbound NIP-90 jobs",
    },
    SlashCommandSpec {
        id: SlashCommandId::Job,
        name: "job",
        usage: "/job [submit|watch|history|replay|approve|deny|policy] ...",
        summary: "submit, inspect, replay, or settle retained NIP-90 buyer jobs",
    },
    SlashCommandSpec {
        id: SlashCommandId::Jobs,
        name: "jobs",
        usage: "/jobs [--limit <n>]",
        summary: "show retained provider job history in the transcript",
    },
    SlashCommandSpec {
        id: SlashCommandId::Earnings,
        name: "earnings",
        usage: "/earnings",
        summary: "show retained provider earnings in the transcript",
    },
    SlashCommandSpec {
        id: SlashCommandId::Receipts,
        name: "receipts",
        usage: "/receipts [--limit <n>]",
        summary: "show retained provider receipts in the transcript",
    },
    SlashCommandSpec {
        id: SlashCommandId::Activity,
        name: "activity",
        usage: "/activity [--limit <n>]",
        summary: "show retained relay and settlement activity in the transcript",
    },
    SlashCommandSpec {
        id: SlashCommandId::Payout,
        name: "payout",
        usage: "/payout [history|withdraw] ...",
        summary: "inspect provider earnings and move retained value out",
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
        assert_eq!(
            names,
            vec![
                "help",
                "chat",
                "download",
                "model",
                "uninstall",
                "announce",
                "provider",
                "job",
                "jobs",
                "earnings",
                "receipts",
                "activity",
                "payout",
                "relay",
                "wallet"
            ]
        );
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
        let parsed = parse_submission("/model gemma-4-e2b");
        assert_eq!(
            parsed,
            ParsedSubmission::Command {
                spec: &registry()[3],
                args: String::from("gemma-4-e2b"),
                raw: String::from("/model gemma-4-e2b"),
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
        assert!(lines.iter().any(|line| line.contains("/model [model]")));
        assert!(lines.iter().any(|line| line.contains("/uninstall [model]")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("Plain text without a slash"))
        );
        assert_eq!(registry()[1].id, SlashCommandId::Chat);
        assert_eq!(registry()[3].id, SlashCommandId::Model);
        assert_eq!(registry()[4].id, SlashCommandId::Uninstall);
        assert_eq!(registry()[5].id, SlashCommandId::Announce);
        assert_eq!(registry()[6].id, SlashCommandId::Provider);
        assert_eq!(registry()[7].id, SlashCommandId::Job);
        assert_eq!(registry()[8].id, SlashCommandId::Jobs);
        assert_eq!(registry()[9].id, SlashCommandId::Earnings);
        assert_eq!(registry()[10].id, SlashCommandId::Receipts);
        assert_eq!(registry()[11].id, SlashCommandId::Activity);
        assert_eq!(registry()[12].id, SlashCommandId::Payout);
        assert_eq!(registry()[13].id, SlashCommandId::Relay);
        assert_eq!(registry()[14].id, SlashCommandId::Wallet);
    }
}
