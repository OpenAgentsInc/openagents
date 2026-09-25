//! `evidence.data_profile` as a component: profile every data file in a
//! workspace ([`crate::data_profile`], issue #9654).
//!
//! The fixture's input holds either a workspace directory (`workspace`,
//! where a leading `~/` is the home directory) or the workspace's files
//! inline (`files`, path to text), and optionally `params`. The fixture's
//! `retained` states what must hold: `files`, each with its `path`,
//! `kind`, and optionally `findings`, substrings each of which one of the
//! file's findings must contain; and `skipped`, the paths not profiled.

use std::collections::BTreeMap;
use std::path::PathBuf;

use futures_util::future::LocalBoxFuture;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::jev::JevMode;
use super::{Component, Fixture, Ran, input};
use crate::data_profile::{self, Params, Profile};
use crate::record::{Implementation, Recorder};

/// The `evidence.data_profile` component.
pub struct DataProfileComponent;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileInput {
    #[serde(default)]
    workspace: Option<String>,
    #[serde(default)]
    files: BTreeMap<String, String>,
    #[serde(default)]
    params: Option<Params>,
}

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

/// Whether `profile` holds what `retained` states.
fn matches(profile: &Profile, retained: &Value) -> bool {
    let files = retained["files"].as_array().cloned().unwrap_or_default();
    if files.len() != profile.files.len() {
        return false;
    }
    let files_match = files.iter().zip(&profile.files).all(|(want, file)| {
        want["path"].as_str() == Some(file.path.as_str())
            && want["kind"] == json!(file.kind)
            && want["findings"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .all(|w| file.findings.iter().any(|f| f.contains(w)))
    });
    let skipped: Vec<&str> = profile.skipped.iter().map(|(p, _)| p.as_str()).collect();
    let want_skipped: Vec<&str> = retained["skipped"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    files_match && skipped == want_skipped
}

impl Component for DataProfileComponent {
    fn id(&self) -> &'static str {
        data_profile::COMPONENT
    }
    fn implementation(&self) -> Implementation {
        data_profile::implementation(&Params::default())
    }
    fn about(&self) -> &'static str {
        "Code profiles every data file the task ships, by type, before the first session."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        _jev: &'a JevMode,
        _recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let input: ProfileInput = input(fixture)?;
            let params = input.params.clone().unwrap_or_default();
            params.validate()?;
            let mut _scratch = None;
            let root = match (&input.workspace, input.files.is_empty()) {
                (Some(dir), true) => expand(dir),
                (None, false) => {
                    let dir = std::env::temp_dir().join(format!(
                        "data-profile-fixture-{}-{}",
                        std::process::id(),
                        atif::digest(&json!(input.files))
                            .get(..12)
                            .unwrap_or_default()
                    ));
                    let _ = std::fs::remove_dir_all(&dir);
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
                        "the evidence.data_profile input needs a workspace or files, not both"
                            .to_string(),
                    );
                }
            };
            if !root.is_dir() {
                return Err(format!("no workspace at {}", root.display()));
            }
            let profile = data_profile::profile(&root, &params);
            let mut metrics = Map::new();
            metrics.insert("files".to_string(), json!(profile.files.len()));
            metrics.insert("skipped".to_string(), json!(profile.skipped.len()));
            metrics.insert(
                "findings".to_string(),
                json!(
                    profile
                        .files
                        .iter()
                        .map(|f| f.findings.len())
                        .sum::<usize>()
                ),
            );
            metrics.insert("ms".to_string(), json!(profile.ms));
            if !fixture.retained.is_null() {
                metrics.insert(
                    "matches_retained".to_string(),
                    json!(matches(&profile, &fixture.retained)),
                );
            }
            let items: Vec<Value> = profile
                .items(&params)
                .into_iter()
                .map(|(label, text)| json!({ "label": label, "text": text }))
                .collect();
            Ok(Ran {
                output: json!({
                    "profile": profile.summary(),
                    "items": items,
                }),
                metrics,
            })
        })
    }
}
