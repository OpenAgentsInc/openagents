//! An agent's whole definition as one JSON document
//! (`openagents.agent-definition-export`, version [`VERSION`]).
//!
//! The document carries the Agent Lab experiment exactly as Agent Lab keeps
//! it (name, description, full instructions, every skill's full text,
//! runtime, model, access, autonomy, and the test suite), and beside it what
//! the graph reads out of that text: the app tools, commands and places it
//! names, its grants, its stall ladder, and any notes such as lessons and
//! measures. `/agent/{id}/definition.json` serves it, and `/agent/{id}`'s
//! Copy JSON and Download JSON controls hand it out.

use agent_contract::{GrantScope, NEVER_GRANTED, NEVER_GRANTED_REFUSAL};
use agent_lab::Experiment;
use serde::{Deserialize, Serialize};

use crate::catalog::{self, MACHINE_SCOPES};
use crate::lab::{self, HOST_STALL_MINUTES, Notes, grant_words};
use crate::{Detail, Unit};

/// The schema a [`Definition`] names.
pub const SCHEMA: &str = "openagents.agent-definition-export";
/// The schema's version. A change that renames or removes a field raises it.
pub const VERSION: u32 = 1;

/// One agent's whole definition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Definition {
    pub schema: String,
    pub version: u32,
    /// The agent's page name, such as `foreman`.
    pub id: String,
    /// Its published Agent Lab ID.
    pub published_id: String,
    /// The Agent Lab experiment, as Agent Lab stores it.
    pub experiment: Experiment,
    /// App tools its text names, by group.
    pub tools: Vec<ToolGroup>,
    /// Commands its text names.
    pub commands: Vec<String>,
    /// Places it reads, with the tools or commands that read each.
    pub sources: Vec<Source>,
    pub authority: Authority,
    /// The stall ladder, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stall_ladder: Option<StallLadder>,
    /// What else we say about it: lessons, replays, measures.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<Note>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolGroup {
    pub group: String,
    pub tools: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Source {
    pub place: String,
    pub what: String,
    pub via: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Authority {
    /// Whether it asks for standing authority.
    pub standing: bool,
    pub grants: Vec<Grant>,
    /// Machine actions its text names, each needing its own grant.
    pub machine_scopes: Vec<String>,
    /// Powers no agent can be granted.
    pub never_granted: Vec<String>,
    pub never_granted_note: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Grant {
    pub scope: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StallLadder {
    pub stall_minutes: u32,
    pub rungs: Vec<Rung>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rung {
    pub title: String,
    pub what: String,
}

/// A unit the notes add or tag, with its details.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub part: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<Detail>,
}

/// `experiment`'s whole definition under `id`.
pub fn of(experiment: &Experiment, id: &str, published_id: &str, notes: &Notes) -> Definition {
    let text = lab::text(experiment);
    let autonomy = experiment.draft.spec.autonomy.clone().unwrap_or_default();
    let ladder = experiment.draft.components.iter().find_map(|c| match c {
        agent_lab::Component::Skill { name, text, .. } if name.eq_ignore_ascii_case("stalls") => {
            Some(lab::rungs(text))
        }
        _ => None,
    });
    let mut notes_out: Vec<Note> = notes
        .tags
        .iter()
        .map(|(part, tags)| Note {
            part: part.clone(),
            title: String::new(),
            tags: tags.clone(),
            details: vec![],
        })
        .collect();
    notes_out.extend(notes.units.iter().map(|u: &Unit| Note {
        part: u.id.clone(),
        title: u.title.clone(),
        tags: u.tags.clone(),
        details: u.details.clone(),
    }));
    Definition {
        schema: SCHEMA.into(),
        version: VERSION,
        id: id.into(),
        published_id: published_id.into(),
        experiment: experiment.clone(),
        tools: catalog::tools(&text)
            .into_iter()
            .map(|(group, tools)| ToolGroup {
                group: group.into(),
                tools: tools.into_iter().map(str::to_owned).collect(),
            })
            .collect(),
        commands: catalog::commands(&text)
            .into_iter()
            .map(str::to_owned)
            .collect(),
        sources: catalog::sources(&text)
            .into_iter()
            .map(|(place, what, via)| Source {
                place: place.into(),
                what: what.into(),
                via: via.into_iter().map(str::to_owned).collect(),
            })
            .collect(),
        authority: Authority {
            standing: autonomy.authority,
            grants: GrantScope::ALL
                .iter()
                .map(|s| Grant {
                    scope: s.word().into(),
                    label: grant_words(*s).into(),
                })
                .collect(),
            machine_scopes: MACHINE_SCOPES
                .iter()
                .filter(|s| text.contains(*s))
                .map(|s| (*s).to_owned())
                .collect(),
            never_granted: NEVER_GRANTED.iter().map(|s| (*s).to_owned()).collect(),
            never_granted_note: NEVER_GRANTED_REFUSAL.into(),
        },
        stall_ladder: ladder.filter(|r| !r.is_empty()).map(|rungs| StallLadder {
            stall_minutes: autonomy.stall_minutes.unwrap_or(HOST_STALL_MINUTES),
            rungs: rungs
                .into_iter()
                .map(|(title, what)| Rung { title, what })
                .collect(),
        }),
        notes: notes_out,
    }
}

/// The file name a download suggests, such as `foreman.agent.json`.
pub fn file_name(id: &str) -> String {
    let safe: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!("{safe}.agent.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_lab::templates;

    #[test]
    fn foremans_definition_round_trips_and_carries_everything() {
        let definition = crate::foreman::definition();
        let json = serde_json::to_string_pretty(&definition).unwrap();
        let back: Definition = serde_json::from_str(&json).unwrap();
        assert_eq!(back, definition);
        assert_eq!(back.schema, SCHEMA);
        assert_eq!(back.version, VERSION);

        // Every skill's full text and the full instructions.
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let text = value.to_string();
        for (name, skill) in templates::FOREMAN_SKILLS {
            let skill = skill.trim_end();
            assert!(
                back.experiment.draft.components.iter().any(|c| matches!(
                    c,
                    agent_lab::Component::Skill { name: n, text, .. } if n == name && text == skill
                )),
                "{name}"
            );
        }
        assert_eq!(
            back.experiment.draft.spec.instructions,
            templates::FOREMAN_INSTRUCTIONS.trim_end()
        );
        // Every tool and command its text names.
        let tools: Vec<&str> = back
            .tools
            .iter()
            .flat_map(|g| g.tools.iter().map(String::as_str))
            .collect();
        for (_, names) in catalog::TOOLS {
            for name in names {
                assert!(tools.contains(name), "{name}");
            }
        }
        for command in catalog::COMMANDS {
            assert!(back.commands.iter().any(|c| c == command), "{command}");
        }
        assert_eq!(back.sources.len(), catalog::SOURCES.len());
        assert_eq!(back.authority.grants.len(), GrantScope::ALL.len());
        assert_eq!(back.stall_ladder.as_ref().unwrap().rungs.len(), 3);
        assert!(
            back.notes
                .iter()
                .any(|n| n.part == "measures" && !n.details.is_empty())
        );
        assert!(text.contains("claude-opus-5-5"));
        assert_eq!(
            back.experiment.suite.cases.len(),
            templates::foreman().suite.cases.len()
        );
    }

    #[test]
    fn download_names_are_plain_files() {
        assert_eq!(file_name("foreman"), "foreman.agent.json");
        assert_eq!(file_name("a/b c"), "a-b-c.agent.json");
    }
}
