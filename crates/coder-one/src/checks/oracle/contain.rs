//! The oracle writer's own container, for tasks whose boundary is a task
//! container.
//!
//! A task container can't hide the candidate workspace or the retained
//! records from a command, so the writer doesn't run there. It runs in a
//! separate, fresh container started from the task's image: the task's
//! tools and its untouched files as the image ships them, with no network
//! and nothing mounted from the host. The writer's directory, [`ROOT`],
//! exists only inside that container. Code writes `spec.json` and
//! `cases.json` into it through `docker exec`, the session's tools run
//! there ([`microluna::Remote`]), and the one thing that comes out is
//! `oracle.py`, copied with `docker cp`. The host then runs that oracle on
//! the candidate in the task's container, as it runs any oracle.
//!
//! [`WriterContainer::start`] refuses, and starts no session, when the
//! image isn't known or isn't on this machine, when Docker can't be
//! reached, or when the container it made has a network or a mount from
//! the host. The writer never falls back to a less confined place.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::{Value, json};

/// The writer's directory inside its container.
pub const ROOT: &str = "/opt/oracle-writer";

/// The largest `oracle.py` that comes out of the container.
pub const ORACLE_MAX: u64 = 1024 * 1024;

/// The word the writer's record and its commands' records carry.
pub const ISOLATION: &str = "writer-container";

/// The label every writer container carries, so a stray one can be found.
pub const LABEL: &str = "openagents.oracle-writer=9656";

/// How a refusal starts when there's no Docker client to run, as inside a
/// Harbor trial's task container.
pub const UNREACHABLE: &str = "Docker can't be reached from here";

/// A bounded `docker` run's result.
pub type Running<'a> = Pin<Box<dyn Future<Output = supervise::Ended> + Send + 'a>>;

/// How code reaches Docker: the command-line client, or a fake in tests.
pub trait Docker: Send + Sync + std::fmt::Debug {
    /// Runs `docker` with `args`, and `input` on standard input when there
    /// is some, and returns its standard output.
    ///
    /// # Errors
    ///
    /// Docker's message when it fails or can't be run.
    fn call(&self, args: &[String], input: Option<&[u8]>) -> Result<Vec<u8>, String>;

    /// Runs `docker` with `args` under `supervise`, bounded by `wall`.
    fn bounded(&self, args: Vec<String>, wall: Duration) -> Running<'_>;
}

/// The `docker` command-line client.
#[derive(Clone, Copy, Debug, Default)]
pub struct Cli;

impl Docker for Cli {
    fn call(&self, args: &[String], input: Option<&[u8]>) -> Result<Vec<u8>, String> {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new("docker")
            .args(args)
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("{UNREACHABLE}: {error}"))?;
        if let (Some(bytes), Some(mut stdin)) = (input, child.stdin.take()) {
            stdin.write_all(bytes).map_err(|error| error.to_string())?;
        }
        let output = child.wait_with_output().map_err(|e| e.to_string())?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(format!(
                "docker {} failed: {}",
                args.first().map_or("", String::as_str),
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }

    fn bounded(&self, args: Vec<String>, wall: Duration) -> Running<'_> {
        Box::pin(async move {
            let mut command = std::process::Command::new("docker");
            command.args(&args);
            supervise::Job::from_command(command)
                .bounded(supervise::Limits::within(wall).keeping(microluna::tools::COMMAND_KEEP))
                .run()
                .await
        })
    }
}

/// The image the writer's container starts from, and what it must never
/// see.
#[derive(Clone, Debug)]
pub struct Image {
    /// The task's image reference. Empty when the caller couldn't identify
    /// it, which refuses the writer.
    pub reference: String,
    /// The host paths no mount may come from: the candidate workspace and
    /// the run's records.
    pub withheld: Vec<PathBuf>,
    pub docker: Arc<dyn Docker>,
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_string()).collect()
}

/// A fresh container of the task's image that the writer's tools run in.
/// Dropping it removes the container.
#[derive(Debug)]
pub struct WriterContainer {
    docker: Arc<dyn Docker>,
    id: String,
    image: String,
    image_id: String,
    mounts: Value,
    removed: AtomicBool,
}

impl WriterContainer {
    /// Starts a container of `image` named `name`, with no network and no
    /// mount from the host, and makes the writer's directory in it.
    ///
    /// # Errors
    ///
    /// A plain sentence when the image isn't known or isn't on this
    /// machine, when Docker can't start the container, or when the
    /// container has a network or a mount from the host. A container that
    /// was made is removed.
    pub fn start(image: &Image, name: &str) -> Result<WriterContainer, String> {
        let refuse = |why: String| format!("The oracle writer did not run: {why}");
        let reference = image.reference.trim();
        if reference.is_empty() {
            return Err(refuse(
                "the task's image isn't known, so no clean container of it can be started."
                    .to_string(),
            ));
        }
        let docker = image.docker.clone();
        let image_id = docker
            .call(
                &strings(&["image", "inspect", "--format", "{{.Id}}", reference]),
                None,
            )
            .map_err(|error| {
                if error.starts_with(UNREACHABLE) {
                    refuse(format!("{error}."))
                } else {
                    refuse(format!(
                        "the task's image {reference} isn't on this machine, and the writer \
                         never pulls one ({error})."
                    ))
                }
            })?;
        let image_id = String::from_utf8_lossy(&image_id).trim().to_string();
        let id = docker
            .call(
                &strings(&[
                    "create",
                    "--network",
                    "none",
                    "--pull",
                    "never",
                    "--name",
                    name,
                    "--label",
                    LABEL,
                    "--entrypoint",
                    "sleep",
                    reference,
                    "infinity",
                ]),
                None,
            )
            .map_err(|error| {
                refuse(format!(
                    "Docker couldn't make a container of {reference}: {error}"
                ))
            })?;
        let id = String::from_utf8_lossy(&id).trim().to_string();
        let mut container = WriterContainer {
            docker,
            id,
            image: reference.to_string(),
            image_id,
            mounts: Value::Null,
            removed: AtomicBool::new(false),
        };
        container.prepare(&image.withheld).map_err(refuse)?;
        Ok(container)
    }

    /// Starts the container, checks that it has no network and no mount
    /// from the host, and makes the writer's directory.
    fn prepare(&mut self, withheld: &[PathBuf]) -> Result<(), String> {
        self.docker
            .call(&strings(&["start", &self.id]), None)
            .map_err(|error| format!("Docker couldn't start the container: {error}"))?;
        let inspected = self
            .docker
            .call(
                &strings(&[
                    "inspect",
                    "--format",
                    "{{json .Mounts}}|{{.HostConfig.NetworkMode}}",
                    &self.id,
                ]),
                None,
            )
            .map_err(|error| format!("Docker couldn't describe the container: {error}"))?;
        let inspected = String::from_utf8_lossy(&inspected).trim().to_string();
        let (mounts, network) = inspected.rsplit_once('|').unwrap_or((&inspected, ""));
        if network.trim() != "none" {
            return Err(format!(
                "the container's network is '{}', not none.",
                network.trim()
            ));
        }
        let mounts: Value = serde_json::from_str(mounts.trim())
            .map_err(|_| "Docker's description of the container's mounts can't be read.")?;
        for mount in mounts.as_array().into_iter().flatten() {
            let kind = mount["Type"].as_str().unwrap_or_default();
            let source = mount["Source"].as_str().unwrap_or_default();
            let destination = mount["Destination"].as_str().unwrap_or_default();
            // An image's own volume starts as a copy of the image's files;
            // anything bound from the host could hold a candidate.
            if kind != "volume" {
                return Err(format!(
                    "the container has a {kind} mount at {destination}, and the writer may \
                     see nothing from the host."
                ));
            }
            if withheld
                .iter()
                .any(|path| !source.is_empty() && Path::new(source).starts_with(path))
            {
                return Err(format!(
                    "the container mounts {source}, which holds the candidate's files."
                ));
            }
        }
        self.mounts = mounts;
        self.docker
            .call(
                &strings(&[
                    "exec",
                    "-u",
                    "0",
                    &self.id,
                    "sh",
                    "-c",
                    &format!("mkdir -p {ROOT} && chmod 1777 {ROOT}"),
                ]),
                None,
            )
            .map_err(|error| format!("the writer's directory couldn't be made: {error}"))?;
        Ok(())
    }

    /// The container's ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// What the writer's record keeps about its container.
    #[must_use]
    pub fn describe(&self) -> Value {
        json!({
            "isolation": ISOLATION,
            "image": self.image,
            "image_id": self.image_id,
            "container": self.id,
            "network": "none",
            "mounts": self.mounts,
            "root": ROOT,
            "copied_out": ["oracle.py"],
        })
    }

    /// Writes a file into the writer's directory, from the host. This is
    /// how `spec.json` and `cases.json` go in.
    ///
    /// # Errors
    ///
    /// Docker's message when it can't.
    pub fn put(&self, path: &str, contents: &[u8]) -> Result<(), String> {
        microluna::Remote::write(self, path, Some(contents))
    }

    /// Copies `oracle.py` out of the writer's directory into `into`, and
    /// returns its text, or `None` when the writer left none. Nothing else
    /// comes out of the container.
    ///
    /// # Errors
    ///
    /// A message when the copy fails, or what came out isn't a plain file
    /// no larger than [`ORACLE_MAX`].
    pub fn copy_out(&self, into: &Path) -> Result<Option<String>, String> {
        let exists = self.docker.call(
            &strings(&["exec", &self.id, "test", "-f", &format!("{ROOT}/oracle.py")]),
            None,
        );
        if exists.is_err() {
            return Ok(None);
        }
        let _ = std::fs::remove_file(into);
        self.docker.call(
            &strings(&[
                "cp",
                "-L",
                &format!("{}:{ROOT}/oracle.py", self.id),
                &into.display().to_string(),
            ]),
            None,
        )?;
        let meta = std::fs::symlink_metadata(into).map_err(|e| e.to_string())?;
        if !meta.is_file() || meta.len() > ORACLE_MAX {
            let _ = std::fs::remove_file(into);
            return Err(format!(
                "the writer's oracle.py isn't a plain file of at most {ORACLE_MAX} bytes"
            ));
        }
        std::fs::read_to_string(into)
            .map(Some)
            .map_err(|e| e.to_string())
    }

    /// Removes the container. Dropping it does the same.
    pub fn remove(&self) {
        if !self.removed.swap(true, Ordering::SeqCst) {
            let _ = self.docker.call(&strings(&["rm", "-f", &self.id]), None);
        }
    }
}

impl Drop for WriterContainer {
    fn drop(&mut self) {
        self.remove();
    }
}

impl microluna::Remote for WriterContainer {
    fn word(&self) -> &'static str {
        ISOLATION
    }

    fn root(&self) -> &str {
        ROOT
    }

    fn offline(&self) -> bool {
        true
    }

    fn run(&self, command: &str, wall: Duration) -> microluna::remote::Running<'_> {
        // The inner `timeout` stops the command inside the container; the
        // outer bound stops the client if the container doesn't answer.
        let seconds = wall.as_secs().max(1);
        let quoted = crate::accept::runner::sh_quote(command);
        let script = format!(
            "if command -v timeout >/dev/null 2>&1; then exec timeout {seconds} sh -c {quoted}; \
             else exec sh -c {quoted}; fi"
        );
        let args = vec![
            "exec".to_string(),
            "-w".to_string(),
            ROOT.to_string(),
            self.id.clone(),
            "sh".to_string(),
            "-c".to_string(),
            script,
        ];
        Box::pin(async move {
            let mut ended = self
                .docker
                .bounded(args, wall + Duration::from_secs(20))
                .await;
            if ended.ending.code() == Some(124) {
                ended.ending = supervise::Ending::TimedOut;
            }
            ended
        })
    }

    fn read(&self, path: &str, max: usize) -> Result<Vec<u8>, String> {
        self.docker.call(
            &strings(&[
                "exec",
                "-w",
                ROOT,
                &self.id,
                "sh",
                "-c",
                &format!("test -f \"$1\" && head -c {max} -- \"$1\""),
                "sh",
                path,
            ]),
            None,
        )
    }

    fn write(&self, path: &str, contents: Option<&[u8]>) -> Result<(), String> {
        match contents {
            Some(bytes) => self
                .docker
                .call(
                    &strings(&[
                        "exec",
                        "-i",
                        "-w",
                        ROOT,
                        &self.id,
                        "sh",
                        "-c",
                        "mkdir -p \"$(dirname \"$1\")\" && cat > \"$1\"",
                        "sh",
                        path,
                    ]),
                    Some(bytes),
                )
                .map(|_| ()),
            None => self
                .docker
                .call(
                    &strings(&["exec", "-w", ROOT, &self.id, "rm", "-f", "--", path]),
                    None,
                )
                .map(|_| ()),
        }
    }
}
