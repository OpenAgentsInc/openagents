//! `evidence.departures` as a component: mine a workspace with the three
//! departure miners and have Jev rank every candidate
//! ([`crate::departures`], issue #9634).
//!
//! The fixture's input names the task and either a workspace directory
//! (`workspace`, where a leading `~/` is the home directory) or the
//! workspace's files inline (`files`, path to text). `sources` limits the
//! miners; absent, all three run. `comments` is how `rationale` finds its
//! comments: `keywords`, the default, or `lexicon-free` (issue #9652). The output holds every row with its
//! probability and the rows the briefing would list, so a measurement can
//! choose its own threshold from one run.

use std::collections::BTreeMap;
use std::path::PathBuf;

use futures_util::future::LocalBoxFuture;
use serde::Deserialize;
use serde_json::{Map, json};

use super::jev::JevMode;
use super::{Component, Fixture, Ran, input};
use crate::departures::{self, CommentMode, Source};
use crate::record::{Implementation, Recorder};

/// The `evidence.departures` component.
pub struct Departures;

#[derive(Deserialize)]
struct DeparturesInput {
    task: String,
    #[serde(default)]
    workspace: Option<String>,
    #[serde(default)]
    files: BTreeMap<String, String>,
    #[serde(default)]
    sources: Option<Vec<Source>>,
    /// How `rationale` finds its comments: `keywords`, the default, or
    /// `lexicon-free` (issue #9652).
    #[serde(default)]
    comments: CommentMode,
}

/// A directory that removes itself.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn expand(path: &str) -> PathBuf {
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => PathBuf::from(path),
    }
}

impl Component for Departures {
    fn id(&self) -> &'static str {
        departures::COMPONENT
    }
    fn implementation(&self) -> Implementation {
        departures::implementation(&Source::ALL)
    }
    fn about(&self) -> &'static str {
        "Code mines justifying comments, documented functions, and named standard methods; Jev \
         ranks each as a likely defect."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        jev: &'a JevMode,
        recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let input: DeparturesInput = input(fixture)?;
            let mut _scratch = None;
            let workspace = match (&input.workspace, input.files.is_empty()) {
                (Some(dir), true) => expand(dir),
                (None, false) => {
                    let dir = std::env::temp_dir().join(format!(
                        "departures-{}-{}",
                        std::process::id(),
                        atif::digest(&json!(input.files))
                            .get(..12)
                            .unwrap_or_default()
                    ));
                    for (path, text) in &input.files {
                        let at = dir.join(path);
                        if let Some(parent) = at.parent() {
                            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                        }
                        std::fs::write(&at, text).map_err(|e| e.to_string())?;
                    }
                    _scratch = Some(Scratch(dir.clone()));
                    dir
                }
                _ => {
                    return Err(
                        "the evidence.departures input needs a workspace or files, not both"
                            .to_string(),
                    );
                }
            };
            if !workspace.is_dir() {
                return Err(format!("no workspace at {}", workspace.display()));
            }
            let sources = input.sources.unwrap_or_else(|| Source::ALL.to_vec());
            let ranked = departures::rank_with(
                jev,
                recorder,
                &departures::Context {
                    component: self.id(),
                    id: "departures".to_string(),
                    deadline: None,
                },
                &input.task,
                &workspace,
                &sources,
                input.comments,
            )
            .await;
            let listed = departures::listed(&ranked.rows);
            let mut metrics = Map::new();
            for source in &sources {
                let count =
                    |rows: &[departures::Row]| rows.iter().filter(|r| r.kind == *source).count();
                metrics.insert(
                    format!("rows_{}", source.word()),
                    json!(count(&ranked.rows)),
                );
                metrics.insert(format!("listed_{}", source.word()), json!(count(&listed)));
            }
            metrics.insert(
                "answered".to_string(),
                json!(ranked.rows.iter().filter(|r| r.p.is_some()).count()),
            );
            metrics.insert("jev_usd".to_string(), json!(ranked.usd));
            let sets: BTreeMap<&str, serde_json::Value> = sources
                .iter()
                .map(|s| {
                    let set = departures::question_set(*s);
                    (s.word(), json!({ "id": set.id, "digest": set.digest }))
                })
                .collect();
            let (list, digest) = departures::methods();
            Ok(Ran {
                output: json!({
                    "rows": ranked.rows,
                    "listed": listed,
                    "calls": ranked.calls,
                    "sets": sets,
                    "methods": { "version": list.version, "digest": digest },
                    "comments": input.comments.word(),
                }),
                metrics,
            })
        })
    }
}
