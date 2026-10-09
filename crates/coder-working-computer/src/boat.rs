//! Boat provider effects for a working computer.
//!
//! Boat retains a stopped sandbox's filesystem as a snapshot and restores it
//! on resume; it does not keep processes or memory. So a turn checkpoint
//! here quiesces by stopping the sandbox (the same stop/wait primitive
//! `coder_cloud::boat_backend` uses for cleanup) and records the snapshot
//! Boat reports; the next prompt resumes it. Resume supplies the selected
//! credentials as per-boot process environment (`noEnv`), never in the
//! filesystem, so no checkpoint captures them; [`Provider::apply_credentials`]
//! then verifies their presence by name without reading values.
//!
//! Named snapshots are reusable templates: a chat or setup computer never
//! saves one; only a dedicated builder's [`Images`] capture does (ENV-04).
//! This provider never downloads or reads snapshot files.
//!
//! Errors: a definite 4xx from a mutation is `Failed`; transport loss,
//! deadlines, and 5xx are `Unknown`, and a 404 on reads means gone.

//!
//! Output images ([`Images`], ENV-04) are Boat named snapshots saved from a
//! dedicated builder sandbox under an owned name. A capture reads the name
//! first and refuses one saved from another sandbox, so a name is never
//! replaced; readiness is Boat's snapshot status plus its snapshot ID.
//!
//! Identified commands ([`Commands`]): Boat commands carry no idempotency
//! key and are never retried, so each command runs inside a wrapper that
//! claims `COMMAND_DIR/<id>` with `mkdir` (at most once per identity),
//! keeps its own stdout, stderr, pid, spec digest, and exit code there, and
//! removes every selected credential the command did not name from its
//! environment. Reads report state first and then output from a byte
//! cursor, so a lost start reply reconciles by identity instead of by
//! running the command again.

use crate::provider::{
    CheckpointEvidence, CommandCursor, CommandProgress, CommandRead, CommandSpec, Commands,
    ImageRecord, ImageState, Images, Inspection, Meter, Outcome, Provider,
};
use crate::{Checkpoint, Computer, Health, ServiceDecl};
use base64::Engine;
use boat::{Client, Nullable, WaitOptions, models::*, shell_quote};
use coder_cloud::runtime::Credentials;
use std::time::Duration;

/// Where identified commands keep their records inside the sandbox.
pub const COMMAND_DIR: &str = "/tmp/oa-commands";
/// The wrapper's exit code when the identity was already claimed.
pub const ALREADY_CLAIMED: i32 = 97;
/// Reads (2 s apart) of a ready sandbox that still reports its creator's
/// logins before the boot is refused.
const CREATOR_LOGIN_READS: u32 = 15;
/// The longest command timeout Boat accepts (`invalid_timeout` above it).
pub const BOAT_COMMAND_SECONDS: i64 = 600;

fn join_dir(workdir: &str, cwd: &str) -> String {
    if cwd == "." {
        workdir.to_owned()
    } else {
        format!("{workdir}/{cwd}")
    }
}

/// The detached wrapper for one identified command. `unset` lists the
/// selected credentials this command did not name.
pub fn command_script(root: &str, workdir: &str, spec: &CommandSpec, unset: &[&str]) -> String {
    let d = shell_quote(&format!("{root}/{}", spec.id));
    let mut env = vec!["env".to_owned()];
    env.extend(unset.iter().map(|n| format!("-u {n}")));
    env.extend(
        spec.env
            .iter()
            .map(|(k, v)| format!("{k}={}", shell_quote(v))),
    );
    format!(
        "mkdir -p {root} && mkdir {d} 2>/dev/null || exit {ALREADY_CLAIMED}\n\
         printf %s {digest} > {d}/spec\n\
         mkdir -p {workdir}\n\
         cd {cwd} || {{ echo 126 > {d}/exit.tmp; mv {d}/exit.tmp {d}/exit; exit 126; }}\n\
         {env} sh -c {command} </dev/null >{d}/stdout 2>{d}/stderr &\n\
         echo $! > {d}/pid\n\
         wait $!\n\
         code=$?\n\
         echo $code > {d}/exit.tmp && mv {d}/exit.tmp {d}/exit\n\
         exit $code",
        root = shell_quote(root),
        digest = shell_quote(&spec.digest),
        workdir = shell_quote(workdir),
        cwd = shell_quote(&join_dir(workdir, &spec.cwd)),
        env = env.join(" "),
        command = shell_quote(&spec.command),
    )
}

/// A read-only probe: state line, spec digest line, then base64 stdout
/// and stderr from the cursor. State is read before output, so an `exit`
/// state means the output that follows is final.
pub fn read_script(root: &str, id: &str, cursor: CommandCursor, max_bytes: u64) -> String {
    let d = shell_quote(&format!("{root}/{id}"));
    format!(
        "d={d}\n\
         if [ ! -d \"$d\" ]; then echo absent; echo; echo; echo; exit 0; fi\n\
         if [ -f \"$d/exit\" ]; then echo \"exit $(cat \"$d/exit\")\"\n\
         elif [ ! -f \"$d/pid\" ]; then echo running\n\
         elif kill -0 \"$(cat \"$d/pid\")\" 2>/dev/null; then echo running\n\
         else sleep 1; if [ -f \"$d/exit\" ]; then echo \"exit $(cat \"$d/exit\")\"; else echo lost; fi; fi\n\
         cat \"$d/spec\" 2>/dev/null; echo\n\
         for s in stdout:{o} stderr:{e}; do f=\"$d/${{s%%:*}}\"; n=${{s##*:}}; \
         if [ -f \"$f\" ]; then tail -c +$((n+1)) \"$f\" | head -c {max_bytes} | base64 | tr -d '\\n'; fi; echo; done",
        o = cursor.stdout,
        e = cursor.stderr,
    )
}

/// Parse [`read_script`] output.
pub fn parse_read(stdout: &str) -> Result<CommandRead, &'static str> {
    let mut lines = stdout.split('\n');
    let state = lines.next().unwrap_or_default().trim();
    let digest = lines.next().unwrap_or_default().trim();
    let decode = |line: Option<&str>| {
        base64::engine::general_purpose::STANDARD
            .decode(line.unwrap_or_default().trim())
            .map_err(|_| "The command output did not decode.")
    };
    let stdout = decode(lines.next())?;
    let stderr = decode(lines.next())?;
    let progress = match state {
        "absent" => CommandProgress::Absent,
        "running" => CommandProgress::Running,
        "lost" => CommandProgress::Lost,
        _ => match state.strip_prefix("exit ").map(|c| c.trim().parse::<i64>()) {
            Some(Ok(code)) => CommandProgress::Exited { code },
            _ => return Err("The command state is not recognized."),
        },
    };
    Ok(CommandRead {
        progress,
        digest: (!digest.is_empty()).then(|| digest.to_owned()),
        stdout,
        stderr,
    })
}

/// Signal a command's process tree; the wrapper then records exit 143.
pub fn stop_script(root: &str, id: &str) -> String {
    let d = shell_quote(&format!("{root}/{id}"));
    format!(
        "d={d}\n\
         if [ -f \"$d/exit\" ]; then echo exited; exit 0; fi\n\
         if [ ! -f \"$d/pid\" ]; then echo absent; exit 0; fi\n\
         t() {{ echo \"$1\"; for c in $(pgrep -P \"$1\"); do t \"$c\"; done; }}\n\
         p=$(cat \"$d/pid\"); kill -s TERM $(t \"$p\") 2>/dev/null; echo stopped"
    )
}

pub struct BoatProvider {
    pub client: Client,
    pub credentials: Credentials,
    /// The interactive runtime template a fresh computer starts from.
    pub template: Option<String>,
    /// The working checkout inside the sandbox; service `cwd`s are relative.
    pub workdir: String,
}

const SERVICE_DIR: &str = "/tmp/oa-services";

fn status(e: &boat::Error) -> Option<u16> {
    match e {
        boat::Error::Api(api) => Some(api.status.as_u16()),
        _ => None,
    }
}
/// A mutation error: definite only for a non-transient API refusal.
fn mutation<T>(context: &str, e: boat::Error) -> Outcome<T> {
    match &e {
        boat::Error::Api(api) if !api.is_transient() && !api.may_be_running() => {
            Outcome::failed(format!("{context}: {e}"))
        }
        boat::Error::Configuration(_) | boat::Error::Encode => {
            Outcome::failed(format!("{context}: {e}"))
        }
        _ => Outcome::unknown(format!("{context}: {e}")),
    }
}
fn wait(seconds: u64) -> WaitOptions {
    WaitOptions {
        timeout: Duration::from_secs(seconds),
        ..Default::default()
    }
}
fn stopped(state: &str) -> bool {
    matches!(state, "stopped" | "archived")
}

impl BoatProvider {
    pub async fn from_env(computer: &Computer, workdir: String) -> Result<Self, String> {
        let client = Client::from_env().await.map_err(|e| e.to_string())?;
        let names: Vec<String> = computer.credential_names.iter().cloned().collect();
        let credentials = Credentials::from_names(&names, |n| std::env::var(n).ok())?;
        Ok(Self {
            client,
            credentials,
            template: None,
            workdir,
        })
    }
    async fn sandbox(&self, id: &str) -> Result<Sandbox, boat::Error> {
        Ok(self
            .client
            .get(&GetParams {
                sandbox_id: id.into(),
                ..Default::default()
            })
            .await?
            .sandbox)
    }
    /// Run one bounded command; `Ok(true)` when it exits 0.
    async fn run(&self, id: &str, command: String, seconds: i64) -> Result<bool, boat::Error> {
        let reply = self
            .client
            .command(&CommandParams {
                sandbox_id: id.into(),
                body: CommandRequest {
                    command,
                    timeout_seconds: Some(seconds.clamp(1, BOAT_COMMAND_SECONDS)),
                    ..Default::default()
                },
                ..Default::default()
            })
            .await?;
        Ok(matches!(reply, CommandResponseBody::Finished(r) if r.success && r.exit_code == Some(0)))
    }
    /// Stop and wait for Boat's stop operation; returns its identity.
    async fn stop_and_wait(&self, id: &str) -> Outcome<String> {
        let sandbox = match self.sandbox(id).await {
            Ok(s) => s,
            Err(e) => return Outcome::unknown(format!("read before stop: {e}")),
        };
        if stopped(&sandbox.state) {
            return match sandbox.stop {
                Nullable::Value(op) if op.status == "completed" => Outcome::done(op.id),
                _ => Outcome::done(format!("already {}", sandbox.state)),
            };
        }
        let reply = match self
            .client
            .stop(&StopParams {
                sandbox_id: id.into(),
                ..Default::default()
            })
            .await
        {
            Ok(r) => r,
            Err(e) => return mutation("stop", e),
        };
        let Nullable::Value(sandbox) = reply.sandbox else {
            return Outcome::unknown("Boat returned no stop evidence");
        };
        let Nullable::Value(op) = sandbox.stop else {
            return Outcome::unknown("Boat returned no stop operation");
        };
        match self.client.wait_for_stop(id, &op.id, &wait(600)).await {
            Ok(done) => Outcome::done(done.id),
            Err(boat::Error::StopIncomplete(op)) => {
                Outcome::unknown(format!("stop {} did not complete", op.id))
            }
            Err(e) => Outcome::unknown(format!("waiting for stop: {e}")),
        }
    }
    async fn latest_snapshot(&self, id: &str) -> Result<Option<SnapshotSummary>, boat::Error> {
        Ok(self
            .client
            .get_latest_sandbox_snapshot(&GetLatestSandboxSnapshotParams {
                sandbox_id: id.into(),
                ..Default::default()
            })
            .await?
            .snapshot)
    }
}

impl Provider for BoatProvider {
    async fn create(&self, c: &Computer, operation: &str) -> Outcome<String> {
        let reply = self
            .client
            .create(&CreateParams {
                idempotency_key: Some(operation.into()),
                body: Some(CreateSandboxRequest {
                    type_: Some(c.size.clone()),
                    ttl_seconds: Nullable::Value((c.bounds.absolute_ms / 1000).max(1) as i64),
                    no_env: Some(true),
                    env: Some(self.credentials.environment()),
                    snapshots: Some(true),
                    // A verifier boots from exactly its sealed output image.
                    from_: c
                        .purpose
                        .verify_image()
                        .map(str::to_owned)
                        .or_else(|| self.template.clone()),
                    setup_script: Some(format!(
                        "mkdir -p {} {SERVICE_DIR}",
                        shell_quote(&self.workdir)
                    )),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .await;
        match reply {
            Ok(r) => Outcome::done(r.sandbox.id),
            Err(e) => mutation("create", e),
        }
    }

    async fn restore(
        &self,
        c: &Computer,
        resource: &str,
        checkpoint: Option<&Checkpoint>,
    ) -> Outcome<String> {
        let sandbox = match self.sandbox(resource).await {
            Ok(s) => s,
            Err(e) if status(&e) == Some(404) => return Outcome::failed("the sandbox is gone"),
            Err(e) => return Outcome::unknown(format!("read before resume: {e}")),
        };
        if stopped(&sandbox.state) {
            // Resume restores Boat's latest snapshot; refuse drift from the
            // retained turn checkpoint.
            if let Some(expected) = checkpoint.and_then(|k| k.fact.evidence()) {
                match self.latest_snapshot(resource).await {
                    Ok(Some(s)) if s.id == expected => {}
                    Ok(_) => {
                        return Outcome::failed(
                            "Boat's latest snapshot differs from the retained checkpoint",
                        );
                    }
                    Err(e) => return Outcome::unknown(format!("read snapshot: {e}")),
                }
            }
            if let Err(e) = self
                .client
                .resume(&ResumeParams {
                    sandbox_id: resource.into(),
                    body: Some(ResumeRequest {
                        env: Some(self.credentials.environment()),
                        no_env: Some(true),
                        ttl_seconds: Nullable::Value((c.bounds.absolute_ms / 1000).max(1) as i64),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .await
            {
                return mutation("resume", e);
            }
        }
        match self.client.wait_until_ready(resource, &wait(900)).await {
            Ok(_) => Outcome::done(format!("resumed:{resource}")),
            Err(boat::Error::TerminalState) => Outcome::failed("the sandbox did not resume"),
            Err(e) => Outcome::unknown(format!("waiting for resume: {e}")),
        }
    }

    async fn apply_credentials(&self, c: &Computer, resource: &str) -> Outcome<String> {
        // Boat reports a no-env sandbox made from a template as holding the
        // creator's logins until it has finished provisioning and scrubbed
        // them, so judge it only once it is ready.
        let mut sandbox = match self.client.wait_until_ready(resource, &wait(900)).await {
            Ok(s) => s,
            Err(boat::Error::TerminalState) => return Outcome::failed("the sandbox did not start"),
            Err(e) => return Outcome::unknown(format!("waiting for the sandbox: {e}")),
        };
        for _ in 0..CREATOR_LOGIN_READS {
            if sandbox.holds_creator_logins != Some(true) {
                break;
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
            sandbox = match self.sandbox(resource).await {
                Ok(s) => s,
                Err(e) => return Outcome::unknown(format!("read sandbox: {e}")),
            };
        }
        // A Boat dashboard login copied in by the provider would carry
        // someone else's credentials into this user's computer.
        if sandbox.holds_creator_logins == Some(true) {
            return Outcome::failed("the sandbox carries the creator's provider logins");
        }
        let names: Vec<&str> = c.credential_names.iter().map(String::as_str).collect();
        if names.is_empty() {
            return Outcome::done("applied:".into());
        }
        // Presence by name only; values are never printed or read back.
        let check = names
            .iter()
            .map(|n| format!("[ -n \"${{{n}+x}}\" ] || exit 3"))
            .collect::<Vec<_>>()
            .join("; ");
        match self.run(resource, check, 30).await {
            Ok(true) => Outcome::done(format!("applied:{}", names.join(","))),
            Ok(false) => Outcome::failed("a selected credential is absent after boot"),
            Err(e) => Outcome::unknown(format!("credential check: {e}")),
        }
    }

    async fn start_service(
        &self,
        _c: &Computer,
        resource: &str,
        s: &ServiceDecl,
    ) -> Outcome<String> {
        let dir = if s.cwd == "." {
            self.workdir.clone()
        } else {
            format!("{}/{}", self.workdir, s.cwd)
        };
        let name = &s.name;
        let start = format!(
            "mkdir -p {SERVICE_DIR} && cd {} && (nohup sh -c {} >{SERVICE_DIR}/{name}.log 2>&1 & echo $! >{SERVICE_DIR}/{name}.pid)",
            shell_quote(&dir),
            shell_quote(&s.command),
        );
        match self.run(resource, start, 60).await {
            Ok(true) => {}
            Ok(false) => return Outcome::failed("the service did not start"),
            Err(e) => return Outcome::unknown(format!("start service: {e}")),
        }
        let probe = match &s.health {
            Health::Http { port, path } => format!(
                "curl -fsS -o /dev/null {}",
                shell_quote(&format!("http://127.0.0.1:{port}{path}"))
            ),
            Health::Command { command } => format!("sh -c {}", shell_quote(command)),
        };
        let seconds = s.ready_within_seconds;
        let check = format!(
            "cd {} && i=0; while [ $i -lt {seconds} ]; do {probe} && exit 0; i=$((i+1)); sleep 1; done; exit 1",
            shell_quote(&dir)
        );
        match self.run(resource, check, i64::from(seconds) + 30).await {
            Ok(true) => Outcome::done(format!("ready:{name}")),
            Ok(false) => Outcome::failed(format!("{name} did not pass its health rule")),
            Err(e) => Outcome::unknown(format!("readiness: {e}")),
        }
    }

    async fn checkpoint(
        &self,
        _c: &Computer,
        resource: &str,
        _generation: u64,
    ) -> Outcome<CheckpointEvidence> {
        let stop = match self.stop_and_wait(resource).await {
            Outcome::Done { value } => value,
            Outcome::Failed { reason } => return Outcome::failed(reason),
            Outcome::Unknown { reason } => return Outcome::unknown(reason),
        };
        match self.latest_snapshot(resource).await {
            Ok(Some(s)) if s.status == "completed" => Outcome::done(CheckpointEvidence {
                snapshot: s.id,
                stopped: Some(stop),
            }),
            Ok(_) => Outcome::unknown("stopped, but Boat reports no completed snapshot yet"),
            Err(e) => Outcome::unknown(format!("read snapshot: {e}")),
        }
    }

    async fn shutdown_processes(&self, _c: &Computer, resource: &str) -> Outcome<String> {
        let kill = format!(
            "for f in {SERVICE_DIR}/*.pid; do [ -f \"$f\" ] && kill \"$(cat \"$f\")\" 2>/dev/null; rm -f \"$f\"; done; true"
        );
        match self.run(resource, kill, 60).await {
            Ok(true) => Outcome::done("declared services stopped".into()),
            Ok(false) => Outcome::failed("the shutdown command failed"),
            Err(e) => Outcome::unknown(format!("shutdown: {e}")),
        }
    }

    async fn stop(&self, _c: &Computer, resource: &str) -> Outcome<String> {
        self.stop_and_wait(resource).await
    }

    async fn meter(&self, _c: &Computer, resource: &str) -> Outcome<Meter> {
        match self
            .client
            .usage(&UsageParams {
                sandbox_id: resource.into(),
                ..Default::default()
            })
            .await
        {
            Ok(u) => Outcome::done(Meter {
                running: u.running,
                evidence: format!("seconds={} dollars={}", u.seconds, u.dollars),
            }),
            Err(e) if status(&e) == Some(404) => Outcome::done(Meter {
                running: false,
                evidence: "sandbox gone".into(),
            }),
            Err(e) => Outcome::unknown(format!("usage: {e}")),
        }
    }

    async fn delete(&self, _c: &Computer, resource: &str) -> Outcome<String> {
        let accepted = match self
            .client
            .delete_sandbox(&DeleteSandboxParams {
                sandbox_id: resource.into(),
                x_ascii_confirm_delete: resource.into(),
                ..Default::default()
            })
            .await
        {
            Ok(a) => a.operation,
            Err(e) if status(&e) == Some(404) => return Outcome::done("already gone".into()),
            Err(e) => return mutation("delete", e),
        };
        if accepted.status == "completed" {
            return Outcome::done(accepted.id);
        }
        match self
            .client
            .wait_for_deletion(&accepted.id, &wait(600))
            .await
        {
            Ok(op) => Outcome::done(op.id),
            // Boat parks a deletion as `blocked` while the sandbox's last
            // snapshot uploads finish (or while a named snapshot still reads
            // its chain); the machine itself is already gone, which is what
            // this deletion is for.
            Err(boat::Error::DeletionBlocked(op)) => match self.sandbox(resource).await {
                Err(e) if status(&e) == Some(404) => Outcome::done(format!(
                    "{} (machine gone; {})",
                    op.id,
                    op.stage.as_deref().unwrap_or("blocked")
                )),
                Ok(s) => Outcome::unknown(format!(
                    "deletion {} is blocked ({}) and the sandbox is {}",
                    op.id,
                    op.stage.as_deref().unwrap_or("no stage"),
                    s.state
                )),
                Err(e) => Outcome::unknown(format!("deletion {}: read sandbox: {e}", op.id)),
            },
            Err(e) => Outcome::unknown(format!("deletion {}: {e}", accepted.id)),
        }
    }

    async fn inspect(&self, _c: &Computer, resource: &str) -> Outcome<Inspection> {
        let sandbox = match self.sandbox(resource).await {
            Ok(s) => s,
            Err(e) if status(&e) == Some(404) => {
                return Outcome::done(Inspection {
                    running: None,
                    stop: None,
                    latest_snapshot: None,
                });
            }
            Err(e) => return Outcome::unknown(format!("inspect: {e}")),
        };
        let latest = match self.latest_snapshot(resource).await {
            Ok(s) => s.filter(|s| s.status == "completed").map(|s| s.id),
            Err(e) => return Outcome::unknown(format!("read snapshot: {e}")),
        };
        let is_stopped = stopped(&sandbox.state);
        let stop = match sandbox.stop {
            Nullable::Value(op) if is_stopped && op.status == "completed" => Some(op.id),
            _ => None,
        };
        let running = match sandbox.state.as_str() {
            "cancelled" => None,
            _ if is_stopped => Some(false),
            _ => Some(true),
        };
        Outcome::done(Inspection {
            running,
            stop,
            latest_snapshot: latest,
        })
    }
}

impl BoatProvider {
    /// One short synchronous read-only command; its stdout when it exits 0.
    async fn probe(&self, id: &str, command: String) -> Result<Option<String>, boat::Error> {
        let reply = self
            .client
            .command(&CommandParams {
                sandbox_id: id.into(),
                body: CommandRequest {
                    command,
                    timeout_seconds: Some(30),
                    ..Default::default()
                },
                ..Default::default()
            })
            .await?;
        Ok(match reply {
            CommandResponseBody::Finished(r) if r.success && r.exit_code == Some(0) => {
                Some(r.stdout)
            }
            _ => None,
        })
    }
}

impl Commands for BoatProvider {
    async fn start_command(
        &self,
        c: &Computer,
        resource: &str,
        spec: &CommandSpec,
    ) -> Outcome<String> {
        let unset: Vec<&str> = c
            .credential_names
            .iter()
            .filter(|n| !spec.credential_names.contains(*n))
            .map(String::as_str)
            .collect();
        let script = command_script(COMMAND_DIR, &self.workdir, spec, &unset);
        match self
            .client
            .exec_detached(
                resource,
                CommandRequest {
                    command: script,
                    // Boat refuses a timeout over 600 s and does not end a
                    // detached command at it; the command's own deadline is
                    // kept by its owner, which stops it by identity.
                    timeout_seconds: Some(
                        (spec.timeout_seconds as i64).clamp(1, BOAT_COMMAND_SECONDS),
                    ),
                    ..Default::default()
                },
            )
            .await
        {
            Ok(started) => Outcome::done(format!("boat-process:{}", started.process_id)),
            Err(e) => mutation("start command", e),
        }
    }

    async fn read_command(
        &self,
        _c: &Computer,
        resource: &str,
        id: &str,
        cursor: CommandCursor,
        max_bytes: u64,
    ) -> Outcome<CommandRead> {
        let max = max_bytes.clamp(1, boat::follow::FOLLOW_CHUNK_BYTES);
        match self
            .probe(resource, read_script(COMMAND_DIR, id, cursor, max))
            .await
        {
            Ok(Some(text)) => match parse_read(&text) {
                Ok(read) => Outcome::done(read),
                Err(m) => Outcome::unknown(m),
            },
            Ok(None) => Outcome::unknown("the command read did not finish"),
            Err(e) if status(&e) == Some(404) => Outcome::failed("the sandbox is gone"),
            Err(e) => Outcome::unknown(format!("read command: {e}")),
        }
    }

    async fn stop_command(&self, _c: &Computer, resource: &str, id: &str) -> Outcome<String> {
        match self.probe(resource, stop_script(COMMAND_DIR, id)).await {
            Ok(Some(text)) => Outcome::done(text.trim().to_owned()),
            Ok(None) => Outcome::unknown("the stop command did not finish"),
            Err(e) => mutation("stop command", e),
        }
    }
}

/// Map a Boat named snapshot to an image record.
pub fn image_record(s: &NamedSnapshot) -> ImageRecord {
    let state = match s.status.as_str() {
        "completed" | "ready" | "saved" => ImageState::Ready,
        "failed" | "error" => ImageState::Failed {
            reason: s.error.clone().unwrap_or_else(|| s.status.clone()),
        },
        _ => ImageState::Pending,
    };
    ImageRecord {
        name: s.name.clone(),
        source: s.source_sandbox_id.clone(),
        // Without a snapshot identity the image is not ready.
        state: match (&state, &s.snapshot_id) {
            (ImageState::Ready, None) => ImageState::Pending,
            _ => state,
        },
        snapshot: s.snapshot_id.clone(),
        size_bytes: s.size_bytes.and_then(|b| u64::try_from(b).ok()),
    }
}

impl Images for BoatProvider {
    async fn capture_image(
        &self,
        _c: &Computer,
        resource: &str,
        name: &str,
    ) -> Outcome<ImageRecord> {
        match self.read_image(name).await {
            Outcome::Done { value: Some(r) } if r.source == resource => return Outcome::done(r),
            Outcome::Done { value: Some(_) } => {
                return Outcome::failed("the image name belongs to another sandbox");
            }
            Outcome::Done { value: None } => {}
            Outcome::Failed { reason } | Outcome::Unknown { reason } => {
                return Outcome::unknown(format!("read before capture: {reason}"));
            }
        }
        match self
            .client
            .save_named_snapshot(&SaveNamedSnapshotParams {
                body: NamedSnapshotSaveRequest {
                    sandbox_id: resource.into(),
                    name: name.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .await
        {
            Ok(r) => Outcome::done(image_record(&r.snapshot)),
            Err(e) => mutation("save named snapshot", e),
        }
    }

    async fn hydration(&self, _c: &Computer, resource: &str) -> Outcome<bool> {
        match self.sandbox(resource).await {
            Ok(s) if matches!(s.state.as_str(), "error" | "cancelled" | "archived") => {
                Outcome::failed(format!("the sandbox is {}", s.state))
            }
            Ok(s) => Outcome::done(s.hydrated == Some(true)),
            Err(e) if status(&e) == Some(404) => Outcome::failed("the sandbox is gone"),
            Err(e) => Outcome::unknown(format!("read hydration: {e}")),
        }
    }

    async fn read_image(&self, name: &str) -> Outcome<Option<ImageRecord>> {
        match self
            .client
            .get_named_snapshot(&GetNamedSnapshotParams {
                name: name.into(),
                ..Default::default()
            })
            .await
        {
            Ok(r) => Outcome::done(Some(image_record(&r.snapshot))),
            Err(e) if status(&e) == Some(404) => Outcome::done(None),
            Err(e) => Outcome::unknown(format!("read named snapshot: {e}")),
        }
    }

    async fn delete_image(&self, name: &str) -> Outcome<bool> {
        match self
            .client
            .delete_named_snapshot(&DeleteNamedSnapshotParams {
                name: name.into(),
                ..Default::default()
            })
            .await
        {
            Ok(_) => Outcome::done(true),
            Err(e) if status(&e) == Some(404) => Outcome::done(false),
            Err(e) => mutation("delete named snapshot", e),
        }
    }
}
