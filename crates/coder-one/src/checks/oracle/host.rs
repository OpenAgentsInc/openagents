//! The oracle, written on the host before a trial starts.
//!
//! An oracle depends only on the task's words and its image, never on a
//! candidate. So in a Harbor trial, where the lean loop runs inside the
//! task's container and can't start another one, the host writes the
//! oracle before the trial starts, and the trial receives only the
//! finished files ([`super::deliver`]).
//!
//! [`write_for_trial`] reads the task's instruction, starts a fresh
//! container of the task's image with no network to read the files the
//! instruction names, looks for a provided checker ([`super::find`]), and,
//! when there's none, has Jev pick the spec ([`super::define`]) and a Luna
//! session write the oracle in a container of its own
//! ([`super::contain`]). It writes `oracle.json`, `spec.json`,
//! `writer.json`, `find.json`, and `record.json` to the output directory.
//! The record says whether an oracle came out, its digest, and what the
//! work cost, with the writer's cost marked unknown when a request was
//! still open when the session ended.
//!
//! A task image that isn't on this machine is refused before anything
//! runs: the step never pulls or builds one.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};

use super::{Oracle, Source, Spec, contain, define, find, write};
use crate::accept::runner::{Docker, docker};
use crate::checks::contract::extract;
use crate::checks::contract::host::Container;
use crate::component::jev::JevMode;
use crate::record::Recorder;

/// The schema of the host step's `record.json`.
pub const RECORD_SCHEMA: &str = "openagents.coder-one.oracle-host.v1";

/// What the host step needs.
#[derive(Clone, Debug)]
pub struct Options {
    /// The task's directory: `instruction.md` is read from it, and nothing
    /// else.
    pub task_dir: PathBuf,
    /// The task's image, already on this machine.
    pub image: String,
    /// Where the files go.
    pub out: PathBuf,
    /// The task's working directory, when the image's own isn't it.
    pub workdir: Option<String>,
    /// The writer session's bounds. [`write::Bounds::container`] is set
    /// here, from [`Options::image`].
    pub bounds: write::Bounds,
}

/// Why a writer's cost isn't fully known: a session that ended on its
/// time bound or a transport failure may have had a request open, whose
/// charge was never reported.
fn cost_known(record: &Value) -> bool {
    let ending = record["ending"].as_str().unwrap_or_default();
    !(ending.starts_with("deadline") || ending.starts_with("transport"))
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    crate::record::write_atomic(path, format!("{text}\n").as_bytes())
}

/// The image's ID and working directory, or why the image can't be used.
///
/// # Errors
///
/// A plain sentence when the image isn't on this machine or Docker can't
/// be reached.
pub fn inspect_image(
    docker: &dyn contain::Docker,
    image: &str,
) -> Result<(String, String), String> {
    let image = image.trim();
    if image.is_empty() {
        return Err("the task's image isn't known.".to_string());
    }
    let args: Vec<String> = [
        "image",
        "inspect",
        "--format",
        "{{.Id}}|{{.Config.WorkingDir}}",
        image,
    ]
    .iter()
    .map(|a| (*a).to_string())
    .collect();
    let out = docker.call(&args, None).map_err(|error| {
        if error.starts_with(contain::UNREACHABLE) {
            format!("{error}.")
        } else {
            format!(
                "the task's image {image} isn't on this machine, and the host step never pulls \
                 or builds one ({error})."
            )
        }
    })?;
    let text = String::from_utf8_lossy(&out).trim().to_string();
    let (id, workdir) = text.split_once('|').unwrap_or((text.as_str(), ""));
    Ok((id.trim().to_string(), workdir.trim().to_string()))
}

/// The record of a host step that produced no oracle.
fn unavailable(task: &str, image: &str, why: &str) -> Value {
    json!({
        "schema": RECORD_SCHEMA,
        "task": task,
        "image": image,
        "status": "unavailable",
        "why": why,
        "digest": null,
        "cost": { "luna_usd": 0.0, "luna_known": true, "luna_bound_usd": 0.0, "jev_usd": 0.0 },
    })
}

/// Writes the oracle for one task on the host, and returns the record it
/// also writes to `out/record.json`. `wire` is the Luna transport, or why
/// there's none. `docker` is used to check that the image is present.
///
/// # Errors
///
/// A message only when the output directory can't be written; every
/// other failure is a record with `status: unavailable`.
#[allow(clippy::too_many_lines)]
pub async fn write_for_trial<T: microluna::Transport>(
    wire: Result<&T, String>,
    docker_client: Arc<dyn contain::Docker>,
    mode: &JevMode,
    options: &Options,
) -> Result<Value, String> {
    std::fs::create_dir_all(&options.out).map_err(|e| e.to_string())?;
    for stale in [
        "oracle.json",
        "spec.json",
        "writer.json",
        "find.json",
        "record.json",
    ] {
        let _ = std::fs::remove_file(options.out.join(stale));
    }
    let task = options
        .task_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let done = |record: Value| -> Result<Value, String> {
        write_json(&options.out.join("record.json"), &record)?;
        Ok(record)
    };
    let instruction = match std::fs::read_to_string(options.task_dir.join("instruction.md")) {
        Ok(text) => text,
        Err(error) => {
            return done(unavailable(
                &task,
                &options.image,
                &format!("the task's instruction can't be read: {error}"),
            ));
        }
    };
    let (image_id, image_workdir) = match inspect_image(docker_client.as_ref(), &options.image) {
        Ok(found) => found,
        Err(why) => return done(unavailable(&task, &options.image, &why)),
    };
    let workdir = options
        .workdir
        .clone()
        .filter(|w| !w.trim().is_empty())
        .or_else(|| (!image_workdir.is_empty()).then(|| image_workdir.clone()))
        .unwrap_or_else(|| "/app".to_string());
    // Read what the instruction names from an untouched container of the
    // image, with no network.
    let runner = Docker {
        image: options.image.clone(),
        workdir: workdir.clone(),
        candidate: None,
        test_sec: 0,
        dev: None,
        setup: None,
    };
    let label = format!(
        "oracle-host-{}-{}-{}",
        task,
        std::process::id(),
        atif::now_ms()
    );
    let scratch = options.out.join("scratch-read");
    let pristine = match runner.start(Some(&label)) {
        Ok(id) => {
            let host = Container {
                id: id.clone(),
                workdir: workdir.clone(),
                scratch: scratch.clone(),
            };
            let pristine = extract::gather(&host, &instruction, &workdir).await;
            let _ = docker(&["rm", "-f", &id]);
            let _ = std::fs::remove_dir_all(&scratch);
            pristine
        }
        Err(error) => {
            return done(unavailable(
                &task,
                &options.image,
                &format!("a container of the task's image couldn't be started: {error}"),
            ));
        }
    };
    let located = find::find(&instruction, &workdir, &pristine);
    write_json(&options.out.join("find.json"), &located)?;
    let mut jev_usd = 0.0;
    let mut luna_usd = 0.0;
    let mut luna_known = true;
    let mut writer = Value::Null;
    let mut spec: Option<Spec> = None;
    let oracle: Option<Oracle> = if let Some(command) = &located.command {
        Some(
            Oracle {
                schema: String::new(),
                task: task.clone(),
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
        let recorder = Recorder::default();
        let (made, asked) = define::spec(
            &task,
            &instruction,
            &workdir,
            &pristine,
            located.references.clone(),
            mode,
            None,
            &recorder,
        )
        .await;
        jev_usd = asked.usd;
        write_json(&options.out.join("spec.json"), &made)?;
        let written = match wire {
            Ok(wire) => {
                let bounds = write::Bounds {
                    isolation: microluna::Isolation::TaskContainer,
                    container: Some(contain::Image {
                        reference: options.image.clone(),
                        withheld: vec![options.out.clone(), options.task_dir.clone()],
                        docker: docker_client.clone(),
                    }),
                    readable: Vec::new(),
                    ..options.bounds.clone()
                };
                let dir = options.out.join("writer");
                let (oracle, record) =
                    write::write(wire, &made, &dir, &bounds, Some(&options.out)).await?;
                let _ = std::fs::remove_dir_all(&dir);
                luna_usd = record["usd"].as_f64().unwrap_or(0.0);
                luna_known = cost_known(&record);
                writer = record;
                oracle
            }
            Err(error) => {
                writer = json!({ "error": error });
                None
            }
        };
        write_json(&options.out.join("writer.json"), &writer)?;
        spec = Some(made);
        written
    };
    let cost = json!({
        "luna_usd": luna_usd,
        "luna_known": luna_known,
        "luna_bound_usd": if located.command.is_some() { 0.0 } else { options.bounds.usd },
        "jev_usd": jev_usd,
    });
    let Some(oracle) = oracle else {
        let why = writer["refused"]
            .as_str()
            .or_else(|| writer["error"].as_str())
            .map_or_else(
                || "the writer left no oracle.py".to_string(),
                str::to_string,
            );
        let mut record = unavailable(&task, &options.image, &why);
        record["image_id"] = json!(image_id);
        record["workdir"] = json!(workdir);
        record["spec"] = json!(spec.as_ref().map(|s| s.digest.clone()));
        record["cost"] = cost;
        return done(record);
    };
    write_json(&options.out.join("oracle.json"), &oracle)?;
    done(json!({
        "schema": RECORD_SCHEMA,
        "task": task,
        "image": options.image,
        "image_id": image_id,
        "workdir": workdir,
        "status": oracle.source.word(),
        "why": null,
        "digest": oracle.digest,
        "spec": spec.as_ref().map(|s| s.digest.clone()),
        "cost": cost,
    }))
}
