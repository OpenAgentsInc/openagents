//! The composer's completion lists and drafts as plain values (#12267):
//! which list the token under the caret opens, a computer's commands and
//! skills as rows, and file matches as links. `compose` draws them.

use std::collections::HashSet;

use openagents_chat_view::composer_tokens::{MentionToken, invocation_token, mention_token};
use openagents_proto::invocation::{
    Invocation, Skill, valid_invocation_name, valid_skill_command_name, valid_skill_path,
};
use openagents_proto::{FileSearchMatch, SlashCommand};

/// Which list the token under the caret opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `@`: the computer's files.
    File,
    /// `/`: commands and skills.
    Command,
    /// `$`: skills.
    Skill,
}

/// One row of a completion list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub label: String,
    pub detail: String,
    /// What replaces the token.
    pub insert: String,
}

pub fn draft_key(chat: Option<&str>) -> String {
    chat.map_or_else(|| "new".to_owned(), |chat| format!("chat:{chat}"))
}

/// The completion token under the caret: `@` for files, `$` for skills,
/// `/` for commands and skills.
pub fn token_at(text: &str, cursor: usize) -> Option<(Kind, MentionToken)> {
    if let Some(token) = mention_token(text, cursor) {
        return Some((Kind::File, token));
    }
    if let Some(token) = invocation_token(text, cursor, '$') {
        return Some((Kind::Skill, token));
    }
    invocation_token(text, cursor, '/').map(|token| (Kind::Command, token))
}

/// A computer's commands and skills as rows, in the order its catalog
/// lists them. Only names the host's own reference rules accept are kept.
pub fn catalog(commands: Vec<SlashCommand>, skills: Vec<Skill>) -> Vec<(Kind, Item)> {
    let skills: Vec<Skill> = skills
        .into_iter()
        .filter(|skill| {
            skill.enabled
                && valid_invocation_name(&skill.name)
                && valid_skill_path(&skill.path)
                && skill
                    .command
                    .as_ref()
                    .is_none_or(|command| valid_skill_command_name(&command.name))
        })
        .collect();
    let skill_commands: HashSet<String> = skills
        .iter()
        .filter_map(|skill| skill.command.as_ref().map(|command| command.name.clone()))
        .collect();
    let commands = commands
        .into_iter()
        .filter(|c| valid_invocation_name(&c.name) && !skill_commands.contains(&c.name))
        .map(|c| {
            let detail = match &c.input_hint {
                Some(hint) if !hint.is_empty() && c.description.is_empty() => hint.clone(),
                _ => c.description.clone(),
            };
            (
                Kind::Command,
                Item {
                    label: format!("/{}", c.name),
                    detail,
                    insert: Invocation::Command { name: c.name }.prompt_text(),
                },
            )
        });
    let skills = skills.into_iter().map(|s| {
        (
            Kind::Skill,
            Item {
                label: format!("${}", s.name),
                detail: s.description.clone(),
                insert: Invocation::Skill {
                    name: s.name,
                    path: s.path,
                    command: s.command,
                }
                .prompt_text(),
            },
        )
    });
    commands.chain(skills).collect()
}

/// The catalog's rows for `kind` that match `query`: names that start with
/// it first, then names that contain it.
pub fn filter_catalog(catalog: &[(Kind, Item)], kind: Kind, query: &str) -> Vec<Item> {
    let query = query.to_lowercase();
    let wanted = |row: &Kind| kind == Kind::Command || *row == Kind::Skill;
    let name = |item: &Item| item.label[1..].to_lowercase();
    let mut starts: Vec<Item> = Vec::new();
    let mut contains: Vec<Item> = Vec::new();
    for (row, item) in catalog {
        if !wanted(row) {
            continue;
        }
        let name = name(item);
        if name.starts_with(&query) {
            starts.push(item.clone());
        } else if name.contains(&query) {
            contains.push(item.clone());
        }
    }
    starts.extend(contains);
    starts
}

/// The computer's file matches as rows that insert a file link.
pub fn file_items(matches: Vec<FileSearchMatch>) -> Vec<Item> {
    use openagents_proto::file_mentions::{local_file_link, local_path_is_safe};
    matches
        .into_iter()
        .filter(|file| local_path_is_safe(file.path.trim_end_matches('/')))
        .map(|file| {
            let path = file.path.trim_end_matches('/');
            let name = path.rsplit('/').next().unwrap_or(path).to_owned();
            Item {
                label: if file.is_dir {
                    format!("{name}/")
                } else {
                    name
                },
                detail: path.to_owned(),
                insert: local_file_link(path, file.is_dir),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(name: &str) -> SlashCommand {
        SlashCommand {
            name: name.into(),
            description: format!("{name} it"),
            input_hint: None,
        }
    }

    fn skill(name: &str, enabled: bool) -> Skill {
        Skill {
            name: name.into(),
            path: format!(".claude/skills/{name}/SKILL.md"),
            description: format!("the {name} skill"),
            enabled,
            command: None,
        }
    }

    #[test]
    fn tokens_open_the_right_list() {
        assert_eq!(token_at("see @src/ma", 11).map(|t| t.0), Some(Kind::File));
        assert_eq!(token_at("/rev", 4).map(|t| t.0), Some(Kind::Command));
        assert_eq!(token_at("use $pdf", 8).map(|t| t.0), Some(Kind::Skill));
        assert!(token_at("mail@example.com", 16).is_none());
        assert!(token_at("`@code`", 5).is_none());
        assert!(token_at("try /usr/bin", 12).is_none());
    }

    #[test]
    fn the_catalog_lists_commands_and_enabled_skills_and_filters_by_name() {
        let catalog = catalog(
            vec![command("review"), command("compact"), command("bad name")],
            vec![skill("pdf", true), skill("off", false)],
        );
        let labels: Vec<_> = catalog
            .iter()
            .map(|(_, item)| item.label.as_str())
            .collect();
        assert_eq!(labels, ["/review", "/compact", "$pdf"]);
        let found = filter_catalog(&catalog, Kind::Command, "");
        assert_eq!(found.len(), 3);
        let found = filter_catalog(&catalog, Kind::Skill, "");
        assert_eq!(found.len(), 1);
        let found = filter_catalog(&catalog, Kind::Command, "pa");
        assert_eq!(found[0].label, "/compact");
        assert_eq!(found[0].insert, "/compact");
        let found = filter_catalog(&catalog, Kind::Command, "re");
        assert_eq!(found[0].label, "/review");
    }

    #[test]
    fn files_insert_links_and_unsafe_paths_are_dropped() {
        let items = file_items(vec![
            FileSearchMatch {
                path: "src/main.rs".into(),
                is_dir: false,
            },
            FileSearchMatch {
                path: "src/".into(),
                is_dir: true,
            },
            FileSearchMatch {
                path: "../etc/passwd".into(),
                is_dir: false,
            },
        ]);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].label, "main.rs");
        assert!(items[0].insert.starts_with("[main.rs]("));
        assert_eq!(items[1].label, "src/");
    }

    #[test]
    fn drafts_are_kept_per_chat() {
        assert_eq!(draft_key(None), "new");
        assert_eq!(draft_key(Some("abc")), "chat:abc");
    }
}
