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
//! Named snapshots are reusable templates; this provider never saves one,
//! and it never downloads or reads snapshot files.
//!
//! Errors: a definite 4xx from a mutation is `Failed`; transport loss,
//! deadlines, and 5xx are `Unknown`, and a 404 on reads means gone.

use crate::provider::{CheckpointEvidence, Inspection, Meter, Outcome, Provider};
use crate::{Checkpoint, Computer, Health, ServiceDecl};
use boat::{Client, Nullable, WaitOptions, models::*, shell_quote};
use coder_cloud::runtime::Credentials;
use std::time::Duration;

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
                    timeout_seconds: Some(seconds),
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
                    from_: self.template.clone(),
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
        let sandbox = match self.sandbox(resource).await {
            Ok(s) => s,
            Err(e) => return Outcome::unknown(format!("read sandbox: {e}")),
        };
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
