//! Changing an agent from its graph.
//!
//! A view asks for an [`Edit`]; with the `lab` feature, [`plan`] turns it
//! into the same field changes and actions the Agent Lab form sends
//! ([`Op`]), so every change goes through Agent Lab's own store and checks
//! and the form and the graph show the same draft.
//!
//! Tools and places are read from the agent's text: an agent uses a tool
//! when its instructions or skills name it in code formatting. Adding one
//! names it on a "Tools I use:" line at the end of the instructions;
//! removing one drops it from that line and takes the code formatting off
//! every other mention, so the agent no longer names it.

use serde::{Deserialize, Serialize};

/// A change asked for from the graph.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "edit", rename_all = "snake_case")]
pub enum Edit {
    AddTool {
        name: String,
    },
    RemoveTool {
        name: String,
    },
    AddSource {
        place: String,
    },
    RemoveSource {
        place: String,
    },
    AddSkill,
    /// The skill by its number on the graph (`skill-N`), from 1.
    RemoveSkill {
        number: usize,
    },
    ToggleCheckIns,
    ToggleAuthority,
    SetEveryMinutes {
        minutes: Option<u32>,
    },
    SetStallMinutes {
        minutes: Option<u32>,
    },
}

/// One Agent Lab form step: a field's new value, or an action by its key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Op {
    Field { key: String, value: String },
    Action { key: String },
}

/// What a view needs to offer changes: the agent's settings and what it
/// names now.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Editable {
    pub check_ins: bool,
    pub every_minutes: Option<u32>,
    pub authority: bool,
    pub stall_minutes: Option<u32>,
    /// App tools and commands the agent names.
    pub named: Vec<String>,
    /// Places it reads.
    pub places: Vec<String>,
    /// Whether another skill fits (Agent Lab keeps at most eight).
    pub room_for_skill: bool,
}

/// The line adding a tool writes to.
pub const TOOLS_LINE: &str = "Tools I use: ";

/// Every tool and command a graph can add.
pub fn addable() -> Vec<&'static str> {
    crate::catalog::TOOLS
        .iter()
        .flat_map(|(_, names)| names.iter().copied())
        .chain(crate::catalog::COMMANDS)
        .collect()
}

/// `text` with `name` added to its "Tools I use:" line.
pub fn with_tool(text: &str, name: &str) -> String {
    let item = format!("`{name}`");
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    if let Some(line) = lines.iter_mut().find(|l| l.starts_with(TOOLS_LINE)) {
        let body = line[TOOLS_LINE.len()..].trim_end_matches('.').to_owned();
        *line = if body.trim().is_empty() {
            format!("{TOOLS_LINE}{item}.")
        } else {
            format!("{TOOLS_LINE}{body}, {item}.")
        };
        return lines.join("\n");
    }
    let mut out = text.trim_end().to_owned();
    out.push_str(&format!("\n\n{TOOLS_LINE}{item}."));
    out
}

/// `text` with `name` dropped from its "Tools I use:" line and the code
/// formatting taken off every other mention.
pub fn without_tool(text: &str, name: &str) -> String {
    let mut lines: Vec<String> = vec![];
    let mut dropped = false;
    for line in text.lines() {
        if let Some(body) = line.strip_prefix(TOOLS_LINE) {
            let items: Vec<&str> = body
                .trim_end_matches('.')
                .split(", ")
                .filter(|i| !i.is_empty() && *i != format!("`{name}`"))
                .collect();
            if items.is_empty() {
                dropped = true;
            } else {
                lines.push(format!("{TOOLS_LINE}{}.", items.join(", ")));
            }
            continue;
        }
        lines.push(uncode(line, name));
    }
    let mut out = lines.join("\n");
    if dropped {
        out = out.trim_end().to_owned();
    } else if text.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Takes the backticks off each code span that is `name` or starts with
/// `name ` (a command with its arguments).
fn uncode(line: &str, name: &str) -> String {
    let parts: Vec<&str> = line.split('`').collect();
    if parts.len() < 3 {
        return line.to_owned();
    }
    let mut out = String::new();
    for (at, part) in parts.iter().enumerate() {
        let code = at % 2 == 1 && at + 1 < parts.len();
        let named = *part == name || part.starts_with(&format!("{name} "));
        if code && !named {
            out.push('`');
            out.push_str(part);
            out.push('`');
        } else {
            out.push_str(part);
        }
    }
    // An unclosed backtick at the end keeps its mark.
    if parts.len() % 2 == 0 {
        let last = parts.len() - 1;
        let tail = parts[last];
        let cut = out.len() - tail.len();
        out.insert(cut, '`');
    }
    out
}

#[cfg(feature = "lab")]
mod lab {
    use super::*;
    use agent_lab::{Component, Experiment};

    use crate::catalog::{self, named};

    /// What the graph may change on `experiment` now.
    pub fn editable(experiment: &Experiment) -> Editable {
        let text = crate::lab::text(experiment);
        let autonomy = experiment.draft.spec.autonomy.clone().unwrap_or_default();
        Editable {
            check_ins: autonomy.check_ins,
            every_minutes: autonomy.every_minutes,
            authority: autonomy.authority,
            stall_minutes: autonomy.stall_minutes,
            named: addable()
                .into_iter()
                .filter(|n| named(&text, n))
                .map(str::to_owned)
                .collect(),
            places: catalog::sources(&text)
                .into_iter()
                .map(|(p, _, _)| p.to_owned())
                .collect(),
            room_for_skill: experiment.draft.components.len() < 8,
        }
    }

    fn add_tool(experiment: &Experiment, name: &str) -> Vec<Op> {
        vec![Op::Field {
            key: "instructions".into(),
            value: with_tool(&experiment.draft.spec.instructions, name),
        }]
    }

    fn remove_tool(experiment: &Experiment, name: &str) -> Vec<Op> {
        let mut ops = vec![];
        let spec = &experiment.draft.spec;
        let instructions = without_tool(&spec.instructions, name);
        if instructions != spec.instructions {
            ops.push(Op::Field {
                key: "instructions".into(),
                value: instructions,
            });
        }
        for (i, c) in experiment.draft.components.iter().enumerate() {
            if let Component::Skill { text, .. } | Component::Memory { text, .. } = c {
                let changed = without_tool(text, name);
                if changed != *text {
                    ops.push(Op::Field {
                        key: format!("component:{i}"),
                        value: changed,
                    });
                }
            }
        }
        ops
    }

    /// The form steps that make `edit` on `experiment`.
    pub fn plan(experiment: &Experiment, edit: &Edit) -> Result<Vec<Op>, String> {
        let text = crate::lab::text(experiment);
        let ops = match edit {
            Edit::AddTool { name } => {
                if !addable().contains(&name.as_str()) {
                    return Err("Choose one of the app's tools.".into());
                }
                if named(&text, name) {
                    return Err("It already uses that tool.".into());
                }
                add_tool(experiment, name)
            }
            Edit::RemoveTool { name } => {
                if !named(&text, name) {
                    return Err("It doesn't use that tool.".into());
                }
                remove_tool(experiment, name)
            }
            Edit::AddSource { place } => {
                let (_, _, via) = catalog::SOURCES
                    .iter()
                    .find(|(p, _, _)| p == place)
                    .ok_or("Choose one of the places the app can read.")?;
                if via.iter().any(|v| named(&text, v)) {
                    return Err("It already reads that place.".into());
                }
                add_tool(experiment, via[0])
            }
            Edit::RemoveSource { place } => {
                let (_, _, via) = catalog::SOURCES
                    .iter()
                    .find(|(p, _, _)| p == place)
                    .ok_or("Choose one of the places the app can read.")?;
                // Remove each tool in turn, from the text the last step left.
                let mut draft = experiment.clone();
                let mut ops = vec![];
                for tool in via.iter().filter(|v| named(&text, v)) {
                    for op in remove_tool(&draft, tool) {
                        apply(&mut draft, &op);
                        ops.retain(|o: &Op| !same_field(o, &op));
                        ops.push(op);
                    }
                }
                if ops.is_empty() {
                    return Err("It doesn't read that place.".into());
                }
                ops
            }
            Edit::AddSkill => {
                if experiment.draft.components.len() >= 8 {
                    return Err("Use up to eight skills.".into());
                }
                vec![Op::Action {
                    key: "add-guidance".into(),
                }]
            }
            Edit::RemoveSkill { number } => {
                let index = experiment
                    .draft
                    .components
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| matches!(c, Component::Skill { .. }))
                    .nth(number.saturating_sub(1))
                    .map(|(i, _)| i)
                    .ok_or("Choose a skill.")?;
                vec![Op::Action {
                    key: format!("remove-reference:{index}"),
                }]
            }
            Edit::ToggleCheckIns => vec![Op::Action {
                key: "check-ins".into(),
            }],
            Edit::ToggleAuthority => vec![Op::Action {
                key: "authority".into(),
            }],
            Edit::SetEveryMinutes { minutes } => {
                if !experiment
                    .draft
                    .spec
                    .autonomy
                    .as_ref()
                    .is_some_and(|a| a.check_ins)
                {
                    return Err("Turn check-ins on first.".into());
                }
                vec![Op::Field {
                    key: "every-minutes".into(),
                    value: minutes.map(|m| m.to_string()).unwrap_or_default(),
                }]
            }
            Edit::SetStallMinutes { minutes } => vec![Op::Field {
                key: "stall-minutes".into(),
                value: minutes.map(|m| m.to_string()).unwrap_or_default(),
            }],
        };
        Ok(ops)
    }

    fn same_field(a: &Op, b: &Op) -> bool {
        matches!((a, b), (Op::Field { key: x, .. }, Op::Field { key: y, .. }) if x == y)
    }

    /// Applies a text field step to a copy of the draft, as Agent Lab would.
    fn apply(draft: &mut Experiment, op: &Op) {
        let Op::Field { key, value } = op else {
            return;
        };
        if key == "instructions" {
            draft.draft.spec.instructions = value.clone();
        } else if let Some(i) = key
            .strip_prefix("component:")
            .and_then(|i| i.parse::<usize>().ok())
            && let Some(Component::Skill { text, .. } | Component::Memory { text, .. }) =
                draft.draft.components.get_mut(i)
        {
            *text = value.clone();
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use agent_lab::templates;

        fn applied(experiment: &Experiment, ops: &[Op]) -> Experiment {
            let mut draft = experiment.clone();
            for op in ops {
                apply(&mut draft, op);
            }
            draft
        }

        #[test]
        fn a_removed_tool_leaves_the_graph_and_an_added_one_returns() {
            let foreman = templates::foreman();
            let ops = plan(
                &foreman,
                &Edit::RemoveTool {
                    name: "read_chat".into(),
                },
            )
            .unwrap();
            assert!(!ops.is_empty());
            let without = applied(&foreman, &ops);
            assert!(!named(&crate::lab::text(&without), "read_chat"));
            assert!(named(&crate::lab::text(&without), "list_chats"));
            assert!(!editable(&without).named.contains(&"read_chat".to_owned()));
            let ops = plan(
                &without,
                &Edit::AddTool {
                    name: "read_chat".into(),
                },
            )
            .unwrap();
            let back = applied(&without, &ops);
            assert!(named(&crate::lab::text(&back), "read_chat"));
            assert!(
                back.draft
                    .spec
                    .instructions
                    .ends_with("Tools I use: `read_chat`.")
            );
            assert!(
                plan(
                    &back,
                    &Edit::AddTool {
                        name: "read_chat".into()
                    }
                )
                .is_err()
            );
            assert!(
                plan(
                    &back,
                    &Edit::AddTool {
                        name: "rm -rf".into()
                    }
                )
                .is_err()
            );
        }

        #[test]
        fn removing_a_place_removes_every_tool_that_reads_it() {
            let foreman = templates::foreman();
            let ops = plan(
                &foreman,
                &Edit::RemoveSource {
                    place: "The app".into(),
                },
            )
            .unwrap();
            let without = applied(&foreman, &ops);
            assert!(!editable(&without).places.contains(&"The app".to_owned()));
            assert!(editable(&without).places.contains(&"GitHub".to_owned()));
            let ops = plan(
                &without,
                &Edit::AddSource {
                    place: "The app".into(),
                },
            )
            .unwrap();
            assert!(
                editable(&applied(&without, &ops))
                    .places
                    .contains(&"The app".to_owned())
            );
        }

        #[test]
        fn skills_and_settings_map_to_the_forms_controls() {
            let foreman = templates::foreman();
            assert!(plan(&foreman, &Edit::AddSkill).is_err());
            assert_eq!(
                plan(&foreman, &Edit::RemoveSkill { number: 2 }).unwrap(),
                vec![Op::Action {
                    key: "remove-reference:1".into()
                }]
            );
            assert_eq!(
                plan(&foreman, &Edit::SetEveryMinutes { minutes: Some(15) }).unwrap(),
                vec![Op::Field {
                    key: "every-minutes".into(),
                    value: "15".into()
                }]
            );
            let e = editable(&foreman);
            assert!(e.check_ins && e.authority && !e.room_for_skill);
            assert_eq!(e.every_minutes, Some(10));
        }
    }
}

#[cfg(feature = "lab")]
pub use lab::{editable, plan};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tools_line_grows_and_shrinks() {
        let one = with_tool("Do the work.", "gh");
        assert_eq!(one, "Do the work.\n\nTools I use: `gh`.");
        let two = with_tool(&one, "read_chat");
        assert_eq!(two, "Do the work.\n\nTools I use: `gh`, `read_chat`.");
        assert_eq!(
            without_tool(&two, "gh"),
            "Do the work.\n\nTools I use: `read_chat`."
        );
        assert_eq!(without_tool(&one, "gh"), "Do the work.");
    }

    #[test]
    fn removing_takes_code_formatting_off_only_that_tool() {
        assert_eq!(
            without_tool("Run `gh issue list` and `read_chat`.", "gh"),
            "Run gh issue list and `read_chat`."
        );
        assert_eq!(
            without_tool("Keep `ghost` as is.", "gh"),
            "Keep `ghost` as is."
        );
        assert_eq!(without_tool("odd `tick", "gh"), "odd `tick");
    }
}
