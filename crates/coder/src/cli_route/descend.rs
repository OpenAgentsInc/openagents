//! Descending the command tree, one typed choice per level.
//!
//! Level 0 picks the group (`computer`, `verse`, `kb`); each later level
//! picks among the children of the node chosen above it (`list`, `show`,
//! or `channel` and then `open`). Every level is one Jev Choice whose
//! options are exactly that node's children, each described by its usage
//! and summary from the help, plus `none`. `none`, or an argmax below the
//! level's confidence floor, ends the descent: it never guesses past a
//! level it is unsure of.

use indexmap::IndexMap;
use jev::{Answer, Choice, Entry, Questions, SystemOneResponse};

use super::tree::{CommandTree, Node};

/// The question set's identity, for evidence and for the wire.
pub const SET: &str = "chat-router-cli-v1";

/// The id of the level-0 question, as the core router asks it
/// (`cli_group`).
pub const GROUP_QUESTION: &str = "cli_group";

/// The id of a later level's question.
pub const LEVEL_QUESTION: &str = "cli_command";

/// The option id that picks a node's own command when it also has
/// children (`verse xp` beside `verse xp verify-card`).
pub const SELF: &str = "self";

/// The least probability at which a group is taken, from the design's
/// threshold table.
pub const GROUP_CONFIDENCE: f64 = 0.60;

/// The least probability at which a later level's command is taken.
pub const LEVEL_CONFIDENCE: f64 = 0.50;

const PREMISE: &str = "We are OpenAgents. The `openagents` program reaches every OpenAgents \
surface for the user: their connected computers, the Verse and its quests and XP, the \
knowledge base, relays, the wallet, and more. The user does not need to name the program: \
asking for something it does is asking for it.";

fn describe(node: &Node) -> String {
    match (&node.leaf, node.children.is_empty()) {
        (Some(leaf), true) => {
            let usage = leaf.usage.first().map_or_else(
                || leaf.command(),
                |usage| format!("openagents {} {usage}", leaf.path[0]),
            );
            if leaf.summary.is_empty() {
                format!("`{usage}`")
            } else {
                format!("`{usage}`: {}", leaf.summary)
            }
        }
        _ => {
            let names: Vec<&str> = node.children.iter().map(|c| c.name.as_str()).collect();
            let own = node
                .leaf
                .as_ref()
                .map_or(String::new(), |leaf| format!("{}; ", leaf.summary));
            format!("`{}`: {own}commands {}", node.name, names.join(", "))
        }
    }
}

/// The longest part of a command's summary a group's description quotes.
const BRIEF_CHARS: usize = 70;

/// A command's summary cut to its first sentence and [`BRIEF_CHARS`].
fn brief(summary: &str) -> String {
    let sentence = summary
        .split(". ")
        .next()
        .unwrap_or(summary)
        .trim_end_matches('.');
    if sentence.chars().count() <= BRIEF_CHARS {
        sentence.to_string()
    } else {
        let cut: String = sentence.chars().take(BRIEF_CHARS).collect();
        format!("{}…", cut.trim_end())
    }
}

/// A group as the level-0 question describes it: the help table's
/// summary, then each command under it with the first words of its own
/// summary, so the judge reads what the group can do, not only its name.
#[must_use]
pub fn group_summary(group: &Node) -> String {
    let mut commands = Vec::new();
    for leaf in group.leaves() {
        let words = leaf.path[1..].join(" ");
        if words.is_empty() {
            continue;
        }
        if leaf.summary.is_empty() {
            commands.push(words);
        } else {
            commands.push(format!("{words} ({})", brief(&leaf.summary)));
        }
    }
    if commands.is_empty() {
        group.summary.clone()
    } else {
        format!("{} Commands: {}.", group.summary, commands.join("; "))
    }
}

/// The level-0 question: which group, if any, does what the user asks.
#[must_use]
pub fn group_question(tree: &CommandTree) -> Choice {
    let mut criteria: IndexMap<String, Option<Entry>> = tree
        .groups
        .iter()
        .map(|group| {
            (
                group.name.clone(),
                Some(Entry::from(format!(
                    "`openagents {}`: {}",
                    group.name,
                    group_summary(group)
                ))),
            )
        })
        .collect();
    criteria.insert(
        "none".to_string(),
        Some(Entry::from(
            "No `openagents` command does what the user asks, or the user is not asking for \
             one",
        )),
    );
    Choice::new(
        format!("{PREMISE} Which command group does what the user's latest message asks for?"),
        criteria,
    )
}

/// The question for the level under `node` (whose full words are
/// `path`): which of its children does what the user asks.
#[must_use]
pub fn level_question(path: &[String], node: &Node) -> Choice {
    let mut criteria: IndexMap<String, Option<Entry>> = IndexMap::new();
    if let Some(leaf) = node.leaf.as_ref().filter(|_| !node.children.is_empty()) {
        criteria.insert(
            SELF.to_string(),
            Some(Entry::from(format!(
                "`{}` itself: {}",
                leaf.command(),
                leaf.summary
            ))),
        );
    }
    for child in &node.children {
        criteria.insert(child.name.clone(), Some(Entry::from(describe(child))));
    }
    criteria.insert(
        "none".to_string(),
        Some(Entry::from(format!(
            "None of these `openagents {}` commands does what the user asks",
            path.join(" ")
        ))),
    );
    Choice::new(
        format!(
            "{PREMISE} The user wants something done with `openagents {}`. Which of its \
             commands does what the user's latest message asks?",
            path.join(" ")
        ),
        criteria,
    )
}

/// One level's request questions.
#[must_use]
pub fn level_questions(path: &[String], node: &Node) -> Questions {
    Questions::new().with(LEVEL_QUESTION, level_question(path, node))
}

/// A Choice answer's argmax and its probability; `None` when absent.
#[must_use]
pub fn pick(response: &SystemOneResponse, id: &str) -> Option<(String, f64)> {
    match response.answers.get(id) {
        Some(Answer::Choice(answer)) => {
            let p = answer
                .probabilities
                .get(&answer.choice)
                .copied()
                .unwrap_or(answer.confidence);
            Some((answer.choice.clone(), p))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli_route::tree::bundled;

    #[test]
    fn each_level_lists_exactly_its_children_and_none() {
        let tree = bundled();
        let group = group_question(tree);
        assert_eq!(group.criteria.len(), tree.groups.len() + 1);
        assert!(
            Questions::new()
                .with(GROUP_QUESTION, group)
                .validate()
                .is_ok()
        );
        let verse = tree.group("verse").unwrap();
        let level = level_question(&["verse".to_string()], verse);
        assert!(level.criteria.contains_key("who"));
        assert!(level.criteria.contains_key("control"));
        assert!(level.criteria.contains_key("none"));
        assert!(!level.criteria.contains_key(SELF));
        let xp = verse.child("xp").unwrap();
        let level = level_question(&["verse".to_string(), "xp".to_string()], xp);
        assert!(level.criteria.contains_key(SELF));
        assert!(level.criteria.contains_key("verify-card"));
    }

    #[test]
    fn every_level_fits_one_choice() {
        fn walk(path: &mut Vec<String>, node: &Node) {
            if !node.children.is_empty() {
                assert!(level_questions(path, node).validate().is_ok(), "{path:?}");
            }
            for child in &node.children {
                path.push(child.name.clone());
                walk(path, child);
                path.pop();
            }
        }
        for group in &bundled().groups {
            walk(&mut vec![group.name.clone()], group);
        }
    }
}
