//! Agent Lab uses the shared workbench and the account's existing Lab host.
use agent_lab::{
    Content, Document, Run,
    api::*,
    view::{Command, Workbench},
};
use rust_native::{Activation, FieldChange};
use serde_json::{Value, json};

#[cfg(target_family = "wasm")]
#[path = "agent_lab/browser.rs"]
mod browser;
#[cfg(target_family = "wasm")]
pub use browser::AgentLab;
#[cfg(target_family = "wasm")]
pub use browser::OpenTask;

/// A write keeps its body and request ID until the server answers or refuses it.
#[derive(Clone)]
pub struct Pending {
    pub command: Command,
    pub method: &'static str,
    pub path: String,
    pub body: Value,
}

#[derive(Clone)]
pub struct State {
    pub workbench: Workbench,
    pub pending: Option<Pending>,
}

impl State {
    pub fn new(instance: String) -> Self {
        Self {
            workbench: Workbench::new(instance),
            pending: None,
        }
    }

    pub fn edit(&mut self, event: FieldChange) -> Result<(), String> {
        let view = self
            .workbench
            .view()
            .validate()
            .map_err(|_| "Reload Agent Lab.")?;
        let action = view
            .change_field(&event)
            .map_err(|_| "Check this field and try again.")?;
        self.workbench.field(&action.0, event.value)
    }

    pub fn activate(&mut self, event: &Activation) -> Result<Option<Command>, String> {
        let view = self
            .workbench
            .view()
            .validate()
            .map_err(|_| "Reload Agent Lab.")?;
        let action = view
            .activate(event)
            .map_err(|_| "Choose an available action.")?;
        self.workbench.action(&action.0)
    }

    pub fn prepare(&mut self, command: Command, request_id: String) -> Result<(), String> {
        if matches!(command, Command::Retry) {
            return self
                .pending
                .as_ref()
                .map(|_| ())
                .ok_or_else(|| "There is no saved change to retry.".into());
        }
        if self.pending.is_some() {
            return Err("Retry your saved change first.".into());
        }
        let s = &self.workbench;
        let document = || {
            s.document
                .as_deref()
                .ok_or_else(|| "Save this experiment first.".to_owned())
        };
        let (method, path, body) = match &command {
            Command::Save => {
                s.draft.validate().map_err(|e| e.message)?;
                let body = SaveDocument {
                    request_id,
                    expected_revision: s
                        .documents
                        .iter()
                        .find(|d| Some(&d.id) == s.document.as_ref())
                        .and_then(|d| d.versions.last())
                        .map_or(0, |v| v.number),
                    content: Content::Experiment {
                        experiment: s.draft.clone(),
                    },
                    summary: "Saved experiment".into(),
                };
                match &s.document {
                    Some(id) => ("PUT", format!("{API}/documents/{id}"), json!(body)),
                    None => ("POST", format!("{API}/documents"), json!(body)),
                }
            }
            Command::Start | Command::Promote => {
                if s.dirty {
                    return Err("Save your changes before using this version.".into());
                }
                let suffix = if matches!(command, Command::Start) {
                    "jobs"
                } else {
                    "promote"
                };
                (
                    "POST",
                    format!("{API}/documents/{}/{suffix}", document()?),
                    json!(StartRun {
                        request_id,
                        version: s.base
                    }),
                )
            }
            Command::Stop(run) | Command::Archive(run) | Command::Review { run, .. } => {
                let r = s
                    .runs
                    .iter()
                    .find(|r| &r.id == run)
                    .ok_or("Choose a run.")?;
                let action = match &command {
                    Command::Stop(_) => RunAction::Stop,
                    Command::Archive(_) => RunAction::Archive,
                    Command::Review { trial, passed, .. } => RunAction::Review {
                        trial: *trial,
                        passed: *passed,
                        note: s
                            .review_notes
                            .get(&format!("review-note:{run}:{trial}"))
                            .cloned()
                            .unwrap_or_default(),
                    },
                    _ => unreachable!(),
                };
                (
                    "POST",
                    format!("{API}/jobs/{run}/actions"),
                    json!(RunChange {
                        request_id,
                        expected_revision: r.revision,
                        action
                    }),
                )
            }
            Command::Fork(id) => {
                let name = s
                    .templates()
                    .into_iter()
                    .find(|(t, _)| t == id)
                    .map(|(_, e)| e.draft.spec.name)
                    .ok_or("Choose an agent to fork.")?;
                (
                    "POST",
                    format!("/v1/published-agents/{id}/fork"),
                    json!(ForkAgent { request_id, name }),
                )
            }
            _ => return Err("Choose an available action.".into()),
        };
        self.pending = Some(Pending {
            command,
            method,
            path,
            body,
        });
        self.workbench.pending = true;
        Ok(())
    }

    pub fn complete(&mut self, value: Value) -> Result<(), String> {
        let pending = self.pending.as_ref().ok_or("There is no saved change.")?;
        let s = &mut self.workbench;
        match &pending.command {
            Command::Save => {
                let d: Document = serde_json::from_value(value)
                    .map_err(|_| "Couldn't read the saved experiment. Retry this change.")?;
                s.document = Some(d.id.clone());
                s.base = d.versions.last().map_or(0, |v| v.number);
                s.dirty = false;
                s.documents.retain(|old| old.id != d.id);
                s.documents.push(d);
            }
            Command::Start | Command::Stop(_) | Command::Archive(_) | Command::Review { .. } => {
                let r: Run = serde_json::from_value(value)
                    .map_err(|_| "Couldn't read the result. Retry this change.")?;
                if matches!(pending.command, Command::Start) {
                    s.selected_run = Some(r.id.clone());
                }
                s.runs.retain(|old| old.id != r.id);
                if !r.archived {
                    s.runs.push(r);
                }
            }
            Command::Promote => {
                let agent = serde_json::from_value(value.clone())
                    .map_err(|_| "Couldn't read the saved agent. Retry this change.")?;
                s.agents
                    .retain(|old| value["id"].as_str() != Some(old.id.as_str()));
                s.agents.push(agent);
                s.notice = Some(format!(
                    "{} is ready in your agent picker.",
                    s.agents.last().unwrap().spec.name
                ));
            }
            Command::Fork(_) => {
                let forked = serde_json::from_value(value)
                    .map_err(|_| "Couldn't read the fork. Retry this change.")?;
                s.forked(forked);
            }
            _ => return Err("Choose an available action.".into()),
        }
        self.pending = None;
        s.pending = false;
        s.bump();
        Ok(())
    }
}

pub const API: &str = "/v1/labs/agent";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area {
    Library,
    Agent,
    Tests,
    Results,
    Actions,
    Notice,
}

/// Group the shared controls into the desktop's three workbench tabs.
pub fn area(key: &str) -> Area {
    if matches!(key, "heading" | "refresh" | "new")
        || ["template:", "open:", "agent:"]
            .iter()
            .any(|p| key.starts_with(p))
    {
        Area::Library
    } else if matches!(key, "save" | "start" | "promote" | "retry") {
        Area::Actions
    } else if key == "notice" {
        Area::Notice
    } else if [
        "version:",
        "run:",
        "counts:",
        "stop:",
        "archive:",
        "review",
        "reviewed:",
        "result:",
        "answer:",
        "session:",
    ]
    .iter()
    .any(|p| key.starts_with(p))
        || matches!(key, "cost" | "comparison" | "project-files-report")
    {
        Area::Results
    } else if [
        "check",
        "case-title:",
        "task:",
        "expected:",
        "pointer:",
        "remove:",
        "repeat",
        "compare:",
    ]
    .iter()
    .any(|p| key.starts_with(p))
        || matches!(
            key,
            "add-case"
                | "seconds"
                | "partition"
                | "development"
                | "confirmation"
                | "comparison-choice"
                | "compare-none"
                | "planned"
        )
    {
        Area::Tests
    } else {
        Area::Agent
    }
}

#[cfg(test)]
#[path = "agent_lab/tests.rs"]
mod tests;
