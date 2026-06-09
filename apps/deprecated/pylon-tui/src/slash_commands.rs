#![allow(dead_code)]

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
        summary: "run an explicit local model prompt",
    },
    SlashCommandSpec {
        id: SlashCommandId::Download,
        name: "download",
        usage: "/download [model]",
        summary: "download an optional curated local model cache",
    },
    SlashCommandSpec {
        id: SlashCommandId::Model,
        name: "model",
        usage: "/model [model]",
        summary: "select an optional local runtime model",
    },
    SlashCommandSpec {
        id: SlashCommandId::Uninstall,
        name: "uninstall",
        usage: "/uninstall [model]",
        summary: "remove an optional local model cache",
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
        usage: "/wallet [show|receive|withdraw|recovery|status|balance|address|invoice|pay|history]",
        summary: "open the wallet surface, receive sats, withdraw by invoice, or inspect history",
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

pub fn active_slash_query(text: &str, cursor: usize) -> Option<SlashQueryContext> {
    if cursor > text.len() {
        return None;
    }

    let line_start = text[..cursor].rfind('\n').map_or(0, |index| index + 1);
    let current_line = &text[line_start..];
    let leading_ws = current_line
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(index, _)| index)
        .unwrap_or(current_line.len());
    let command_start = line_start + leading_ws;
    if command_start >= text.len() || !text[command_start..].starts_with('/') {
        return None;
    }

    let command_end = text[command_start..]
        .find(char::is_whitespace)
        .map_or(text.len(), |offset| command_start + offset);
    if cursor < command_start + 1 || cursor > command_end {
        return None;
    }

    Some(SlashQueryContext {
        query: text[command_start + 1..cursor].to_string(),
        replace_start: command_start,
        replace_end: command_end,
    })
}

pub fn suggestions_for_query(query: &str) -> Vec<&'static SlashCommandSpec> {
    if query.trim().is_empty() {
        return registry().iter().collect();
    }

    let query = query.trim().to_ascii_lowercase();
    let mut prefix = Vec::new();
    let mut contains = Vec::new();
    for spec in registry() {
        let name = spec.name.to_ascii_lowercase();
        let usage = spec.usage.to_ascii_lowercase();
        let summary = spec.summary.to_ascii_lowercase();
        if name.starts_with(&query) {
            prefix.push(spec);
        } else if usage.contains(&query) || summary.contains(&query) {
            contains.push(spec);
        }
    }
    prefix.into_iter().chain(contains).collect()
}

pub fn insertion_text(spec: &SlashCommandSpec) -> String {
    if spec.usage.contains(' ') {
        format!("/{} ", spec.name)
    } else {
        format!("/{}", spec.name)
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
        "Plain text input is disabled in the default homework dashboard.",
    ));
    lines
}

#[cfg(test)]
mod tests {
    use super::{
        ParsedSubmission, SlashCommandId, active_slash_query, help_lines, insertion_text,
        parse_submission, registry, suggestions_for_query,
    };

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
                .any(|line| line.contains("Plain text input is disabled"))
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

    #[test]
    fn active_slash_query_detects_first_token_only() {
        let query = active_slash_query("/pro", 4).expect("slash query");
        assert_eq!(query.query, "pro");
        assert_eq!(query.replace_start, 0);
        assert_eq!(query.replace_end, 4);

        assert!(active_slash_query("hello /pro", 10).is_none());
        assert!(active_slash_query("/provider run", 12).is_none());
    }

    #[test]
    fn suggestions_rank_prefix_matches_first() {
        let matches = suggestions_for_query("pro");
        assert_eq!(matches.first().map(|spec| spec.name), Some("provider"));
    }

    #[test]
    fn insertion_text_adds_trailing_space_for_argument_commands() {
        assert_eq!(insertion_text(&registry()[0]), "/help");
        assert_eq!(insertion_text(&registry()[6]), "/provider ");
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlashQueryContext {
    pub query: String,
    pub replace_start: usize,
    pub replace_end: usize,
}
