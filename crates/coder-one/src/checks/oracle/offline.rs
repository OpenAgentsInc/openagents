//! `checks.oracle` on retained workspaces.
//!
//! For one task, [`task`] starts a fresh container of the task's image,
//! the untouched workspace, with no network. There it reads what the
//! instruction names, looks for a provided checker ([`super::find`]), and,
//! when there's none, has Jev pick the spec ([`super::define`]) and a
//! Luna session write the oracle from it ([`super::write`]). It runs the
//! oracle once on the untouched workspace, then restores every retained
//! workspace `accept offline` reads into its own container, runs the
//! oracle there with the network off, and removes the container. The
//! verifier's rewards go to `labels.json`, apart from the results, so the
//! results can be read before the labels are joined.

use std::path::Path;

use futures_util::StreamExt;
use serde_json::{Value, json};

use super::super::acceptance::Acceptance;
use super::super::contract::extract;
use super::super::contract::host::Container;
use super::super::contract::offline::{Options, workspaces};
use super::{Oracle, Source, Spec, define, find, write};
use crate::accept::offline::{self as accept_offline, Trial};
use crate::accept::runner::{Docker, docker};
use crate::component::jev::JevMode;
use crate::record::Recorder;

/// The schema of one task's results.
pub const SCHEMA: &str = "openagents.coder-one.oracle-offline.v1";

/// The schema of one task's labels.
pub const LABELS_SCHEMA: &str = "openagents.coder-one.oracle-labels.v1";

/// Where an oracle's files go inside a container.
pub const STAGE: &str = "/opt/oracle-check";

/// Every container this measurement starts is named with this prefix.
pub const PREFIX: &str = "oracle-9656";

/// How to write oracles, when one has to be written.
#[derive(Clone, Debug)]
pub struct Writing {
    /// Write with Luna when no oracle is retained; `false` leaves the task
    /// without one.
    pub luna: bool,
    pub bounds: write::Bounds,
    /// Stop writing once this much has been spent across tasks.
    pub budget_usd: f64,
}

fn now() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

fn name(task: &str, what: &str) -> String {
    format!("{PREFIX}-{task}-{what}-{}-{}", std::process::id(), now())
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    crate::record::write_atomic(path, format!("{text}\n").as_bytes())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Copies a written oracle's files into the container at [`STAGE`].
fn stage(oracle: &Oracle, id: &str, scratch: &Path) -> Result<(), String> {
    if oracle.files.is_empty() {
        return Ok(());
    }
    let local = scratch.join("oracle-files");
    let _ = std::fs::remove_dir_all(&local);
    std::fs::create_dir_all(&local).map_err(|e| e.to_string())?;
    for (file, text) in &oracle.files {
        std::fs::write(local.join(file), text).map_err(|e| e.to_string())?;
    }
    docker(&["exec", "-u", "0", id, "mkdir", "-p", STAGE])?;
    docker(&[
        "cp",
        &format!("{}/.", local.display()),
        &format!("{id}:{STAGE}/"),
    ])?;
    docker(&["exec", "-u", "0", id, "chmod", "-R", "a+rwX", STAGE])?;
    let _ = std::fs::remove_dir_all(&local);
    Ok(())
}

/// Starts `runner`'s container, takes its network away once any setup
/// ran, stages the oracle, runs it, and removes the container.
async fn in_container(
    runner: Docker,
    label: &str,
    scratch: &Path,
    oracle: &Oracle,
    spec: Option<&Spec>,
    candidate: &str,
    untouched: Option<&Acceptance>,
) -> Result<(Acceptance, &'static str), String> {
    let had_setup = runner.setup.is_some();
    let workdir = runner.workdir.clone();
    let id = runner.start(Some(label))?;
    let result = async {
        let network = if had_setup {
            docker(&["network", "disconnect", "bridge", &id])
                .map(|_| "none after setup")
                .map_err(|e| format!("cannot take the network away after setup: {e}"))?
        } else {
            "none"
        };
        stage(oracle, &id, scratch)?;
        let host = Container {
            id: id.clone(),
            workdir: workdir.clone(),
            scratch: scratch.to_path_buf(),
        };
        let result = super::run(oracle, spec, &host, STAGE, &workdir, candidate, untouched).await;
        Ok((result, network))
    }
    .await;
    let _ = docker(&["rm", "-f", &id]);
    let _ = std::fs::remove_dir_all(scratch);
    result
}

/// Finds or writes `name`'s oracle and runs it on every retained
/// workspace; writes `out/<name>/spec.json`, `oracle.json`, the oracle's
/// files under `oracle/`, `results.json`, and `labels.json`. Returns the
/// Luna spend of this task's writing session.
///
/// # Errors
///
/// A message when the task, its image, or its workspaces are missing.
#[allow(clippy::too_many_lines)]
pub async fn task(
    name_: &str,
    mode: &JevMode,
    replay: Option<&JevMode>,
    options: &Options,
    writing: &Writing,
    spent_before: f64,
) -> Result<f64, String> {
    let found = workspaces(name_, options)?;
    let instruction = std::fs::read_to_string(options.tasks_dir.join(name_).join("instruction.md"))
        .map_err(|e| format!("cannot read {name_}'s instruction: {e}"))?;
    let image = options
        .image
        .clone()
        .or_else(|| accept_offline::image(name_))
        .ok_or_else(|| format!("no image for {name_}; pass --image"))?;
    let workdir = found
        .first()
        .map_or_else(|| "/app".to_string(), |t| t.workdir.clone());
    let out = options.out.join(name_);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let recorder = Recorder::default();
    let pristine_runner = || Docker {
        image: image.clone(),
        workdir: workdir.clone(),
        candidate: None,
        test_sec: 0,
        dev: None,
        setup: None,
    };
    // Read what the instruction names from the untouched workspace.
    let pristine = {
        let id = pristine_runner().start(Some(&name(name_, "read")))?;
        let host = Container {
            id: id.clone(),
            workdir: workdir.clone(),
            scratch: out.join("scratch-read"),
        };
        let pristine = extract::gather(&host, &instruction, &workdir).await;
        let _ = docker(&["rm", "-f", &id]);
        let _ = std::fs::remove_dir_all(out.join("scratch-read"));
        pristine
    };
    let located = find::find(&instruction, &workdir, &pristine);
    write_json(&out.join("find.json"), &located)?;
    let mut luna_usd = 0.0;
    let mut spec: Option<Spec> = None;
    let oracle: Option<Oracle> = if let Some(command) = &located.command {
        Some(
            Oracle {
                schema: String::new(),
                task: name_.to_string(),
                source: Source::Found,
                command: Some(command.clone()),
                origin: located.origin.clone(),
                files: std::collections::BTreeMap::new(),
                spec: None,
                writer: json!({ "from": located.from }),
                digest: String::new(),
            }
            .sealed(),
        )
    } else {
        let retained_spec: Option<Spec> = options
            .reuse_plan
            .then(|| read_json(&out.join("spec.json")))
            .flatten();
        let made = match retained_spec {
            Some(s) => s,
            None => {
                let (s, asked) = define::spec(
                    name_,
                    &instruction,
                    &workdir,
                    &pristine,
                    located.references.clone(),
                    mode,
                    replay,
                    &recorder,
                )
                .await;
                crate::say::line(&format!(
                    "oracle ▸ {name_}: spec with {} definition sentences, {} parameters, {} \
                     boundaries; Jev {} requests, ${:.5}",
                    s.definition.len(),
                    s.parameters.len(),
                    s.boundaries.len(),
                    asked.requests,
                    asked.usd
                ));
                s
            }
        };
        write_json(&out.join("spec.json"), &made)?;
        let retained: Option<Oracle> = read_json(&out.join("oracle.json"));
        let oracle = match retained.filter(|o| o.spec.as_deref() == Some(made.digest.as_str())) {
            Some(o) => Some(o),
            None if writing.luna && spent_before < writing.budget_usd => {
                let wire = crate::micro::codex_wire(&format!("oracle-writer-{name_}-{}", now()))?;
                let bounds = write::Bounds {
                    usd: writing
                        .bounds
                        .usd
                        .min(writing.budget_usd - spent_before)
                        .max(0.0),
                    ..writing.bounds.clone()
                };
                crate::say::line(&format!(
                    "oracle ▸ {name_}: a Luna session writes the oracle from the spec"
                ));
                let dir = out.join("writer");
                let (oracle, record) =
                    write::write(&wire, &made, &dir, &bounds, Some(&out)).await?;
                luna_usd = record["usd"].as_f64().unwrap_or(0.0);
                write_json(&out.join("writer.json"), &record)?;
                let _ = std::fs::remove_dir_all(&dir);
                oracle
            }
            None => None,
        };
        spec = Some(made);
        oracle
    };
    let Some(oracle) = oracle else {
        write_json(
            &out.join("results.json"),
            &json!({
                "schema": SCHEMA,
                "task": name_,
                "image": image,
                "oracle": null,
                "why": if writing.luna { "the writer left no oracle.py" } else { "no checker was found and writing was off" },
                "trials": [],
            }),
        )?;
        write_labels(name_, &found, &out)?;
        return Ok(luna_usd);
    };
    write_json(&out.join("oracle.json"), &oracle)?;
    for (file, text) in &oracle.files {
        let path = out.join("oracle").join(file);
        std::fs::create_dir_all(out.join("oracle")).map_err(|e| e.to_string())?;
        std::fs::write(&path, text).map_err(|e| e.to_string())?;
    }
    let (untouched, _) = in_container(
        pristine_runner(),
        &name(name_, "untouched"),
        &out.join("scratch-untouched"),
        &oracle,
        spec.as_ref(),
        "untouched",
        None,
    )
    .await?;
    crate::say::line(&format!(
        "oracle ▸ {name_}: {} oracle; on the untouched workspace: {}",
        oracle.source.word(),
        untouched.summary()
    ));
    let one = |(i, trial): (usize, &Trial)| {
        let (oracle, spec, image, out, untouched) = (&oracle, &spec, &image, &out, &untouched);
        let trial = trial.clone();
        async move {
            let started = now();
            let workspace = out.join(format!("ws-{i}"));
            if let Err(error) = accept_offline::materialize(&trial, image, &workspace) {
                let _ = std::fs::remove_dir_all(&workspace);
                crate::say::line(&format!("oracle ▸ {}: {error}", trial.trial));
                return json!({ "trial": trial.trial, "error": error });
            }
            let requirements = workspace
                .join(trial.workdir.trim_start_matches('/'))
                .join("requirements.txt");
            let runner = Docker {
                image: image.clone(),
                workdir: trial.workdir.clone(),
                candidate: Some(workspace.clone()),
                test_sec: 0,
                dev: None,
                setup: requirements
                    .is_file()
                    .then(|| crate::accept::offline::SETUP.to_string()),
            };
            let ran = in_container(
                runner,
                &name(name_, &i.to_string()),
                &out.join(format!("scratch-{i}")),
                oracle,
                spec.as_ref(),
                &trial.trial,
                Some(untouched),
            )
            .await;
            let _ = std::fs::remove_dir_all(&workspace);
            match ran {
                Ok((result, network)) => {
                    crate::say::line(&format!("oracle ▸ {}: {}", trial.trial, result.summary()));
                    json!({
                        "trial": trial.trial,
                        "job": trial.job,
                        "kind": trial.kind,
                        "network": network,
                        "result": result,
                        "seconds": (now() - started) / 1000,
                    })
                }
                Err(error) => json!({ "trial": trial.trial, "error": error }),
            }
        }
    };
    let results: Vec<Value> = futures_util::stream::iter(found.iter().enumerate().map(one))
        .buffered(options.workers.max(1))
        .collect()
        .await;
    write_json(
        &out.join("results.json"),
        &json!({
            "schema": SCHEMA,
            "task": name_,
            "image": image,
            "oracle": oracle.digest,
            "source": oracle.source,
            "untouched": untouched,
            "trials": results,
        }),
    )?;
    write_labels(name_, &found, &out)?;
    Ok(luna_usd)
}

fn write_labels(name: &str, found: &[Trial], out: &Path) -> Result<(), String> {
    write_json(
        &out.join("labels.json"),
        &json!({
            "schema": LABELS_SCHEMA,
            "task": name,
            "trials": found.iter().map(|t| json!({
                "trial": t.trial,
                "job": t.job,
                "kind": t.kind,
                "reward": t.reward,
                "reward_source": t.reward_source,
                "snapshot_graded": t.snapshot_graded,
                "episode": t.episode,
            })).collect::<Vec<_>>(),
        }),
    )
}

/// The recorded Jev answers every task's spec asked, for replay.
#[must_use]
pub fn recorded_from(out: &Path, tasks: &[String]) -> Vec<(String, Value)> {
    let mut all = Vec::new();
    for task in tasks {
        let Some(spec) = read_json::<Spec>(&out.join(task).join("spec.json")) else {
            continue;
        };
        for call in spec.jev {
            if call["how"] == "live"
                && !call["answers"].is_null()
                && let Some(key) = call["key"].as_str()
            {
                all.push((key.to_string(), call.clone()));
            }
        }
    }
    all
}
