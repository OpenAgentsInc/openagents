//! The Boat API driver: our `oa-boat` on GCE (or any service that serves
//! the Boat subset its declared profiles need, plus `GET /capabilities`).
//!
//! Errors: a definite 4xx from a mutation is `Failed`; transport loss,
//! deadlines and 5xx are `Unknown`; a 404 on a read means gone.
//!
//! Per-command secrets go in the command request's `env` object. `oa-boat`
//! hands them to that one command over the SSH channel's standard input
//! and exports them for that process only; they are never written to the
//! VM's disk or to the per-boot `/run/oa-boat/env`.

use crate::*;
use ::boat::models::*;
use ::boat::{Client, Error, Nullable};

/// One Boat service as a computer driver.
#[derive(Clone)]
pub struct BoatDriver {
    client: Client,
    caps: Capabilities,
}

fn definite(status: u16) -> bool {
    (400..500).contains(&status) && status != 408 && status != 429
}

fn status_of(e: &Error) -> Option<u16> {
    match e {
        Error::Api(api) => Some(api.status.as_u16()),
        _ => None,
    }
}

fn mutation<T>(e: Error) -> Outcome<T> {
    match status_of(&e) {
        Some(s) if definite(s) => Outcome::failed(e.to_string()),
        _ => Outcome::unknown(e.to_string()),
    }
}

impl BoatDriver {
    /// Read the service's declared capabilities, then drive it.
    pub async fn connect(client: Client) -> Result<Self, String> {
        let value = client
            .capabilities()
            .await
            .map_err(|e| format!("GET /capabilities: {e}"))?;
        let caps: Capabilities =
            serde_json::from_value(value).map_err(|e| format!("capabilities: {e}"))?;
        caps.validate()?;
        Ok(Self { client, caps })
    }
    /// Drive a Boat service that does not serve `/capabilities` (the
    /// hosted boat.dev), declaring its profiles by hand.
    pub fn with_capabilities(client: Client, caps: Capabilities) -> Self {
        Self { client, caps }
    }
    pub fn client(&self) -> &Client {
        &self.client
    }
    fn declared(&self, p: Profile) -> bool {
        self.caps.supports(p)
    }
}

fn state_of(s: &str) -> State {
    match s {
        "ready" => State::Running,
        "provisioned" | "creating" | "starting" | "provisioning" | "resuming" => State::Starting,
        "stopping" => State::Stopping,
        "stopped" => State::Stopped,
        "deleted" | "deleting" => State::Gone,
        "failed" | "error" => State::Failed,
        _ => State::Starting,
    }
}

impl Driver for BoatDriver {
    fn capabilities(&self) -> Capabilities {
        self.caps.clone()
    }

    async fn create(&self, r: &CreateRequest) -> Outcome<String> {
        if r.operation.is_empty() {
            return Outcome::failed("a create needs an operation identity");
        }
        let mut extra = BTreeMap::new();
        if !r.labels.is_empty() {
            extra.insert(
                "labels".to_owned(),
                serde_json::to_value(&r.labels).unwrap_or_default(),
            );
        }
        let body = CreateSandboxRequest {
            type_: r.size.clone(),
            ttl_seconds: r
                .ttl_seconds
                .map_or(Nullable::Unset, |t| Nullable::Value(t as i64)),
            no_env: Some(true),
            from_: r.image.clone(),
            extra,
            ..Default::default()
        };
        match self
            .client
            .create(&CreateParams {
                idempotency_key: Some(r.operation.clone()),
                body: Some(body),
                ..Default::default()
            })
            .await
        {
            Ok(reply) => Outcome::done(reply.sandbox.id),
            Err(e) => mutation(e),
        }
    }

    async fn observe(&self, computer: &str) -> Outcome<Observation> {
        match self
            .client
            .get(&GetParams {
                sandbox_id: computer.into(),
                ..Default::default()
            })
            .await
        {
            Ok(r) => Outcome::done(Observation {
                state: state_of(&r.sandbox.state),
                detail: Some(r.sandbox.state),
            }),
            Err(e) if status_of(&e) == Some(404) => Outcome::done(Observation {
                state: State::Gone,
                detail: None,
            }),
            Err(e) => Outcome::unknown(e.to_string()),
        }
    }

    async fn exec(&self, computer: &str, e: &Exec) -> Outcome<ExecOutput> {
        if e.secret_env.keys().any(|k| !env_name_ok(k)) {
            return Outcome::failed("invalid environment variable name");
        }
        let mut extra = BTreeMap::new();
        if !e.secret_env.is_empty() {
            let env: serde_json::Map<String, serde_json::Value> = e
                .secret_env
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.expose().to_owned())))
                .collect();
            extra.insert("env".to_owned(), serde_json::Value::Object(env));
        }
        let reply = self
            .client
            .command(&CommandParams {
                sandbox_id: computer.into(),
                body: CommandRequest {
                    command: e.command.clone(),
                    cwd: e.cwd.clone(),
                    timeout_seconds: Some(e.timeout_seconds.clamp(1, 3600) as i64),
                    extra,
                    ..Default::default()
                },
                ..Default::default()
            })
            .await;
        match reply {
            Ok(CommandResponseBody::Finished(r)) => Outcome::done(ExecOutput {
                code: r.exit_code.unwrap_or(-1),
                stdout: r.stdout,
                stderr: r.stderr,
            }),
            Ok(CommandResponseBody::Started(_)) => {
                Outcome::unknown("the command was started detached")
            }
            Err(err) => mutation(err),
        }
    }

    async fn start_command(&self, computer: &str, spec: &CommandSpec) -> Outcome<String> {
        let script =
            identified::bounded_command_script("/tmp/oa-public-commands", "/workspace", spec);
        match self
            .client
            .exec_detached(
                computer,
                CommandRequest {
                    command: script,
                    timeout_seconds: Some(600),
                    ..Default::default()
                },
            )
            .await
        {
            Ok(r) => Outcome::done(format!("boat-process:{}", r.process_id)),
            Err(e) => mutation(e),
        }
    }

    async fn read_command(
        &self,
        computer: &str,
        id: &str,
        cursor: CommandCursor,
        max_bytes: u64,
    ) -> Outcome<CommandRead> {
        let script = identified::read_script(
            "/tmp/oa-public-commands",
            id,
            cursor,
            max_bytes.clamp(1, 65536),
        );
        match self.exec(computer, &Exec::new(script)).await {
            Outcome::Done { value } if value.code == 0 => {
                match identified::parse_read(&value.stdout) {
                    Ok(r) => Outcome::done(r),
                    Err(e) => Outcome::unknown(e),
                }
            }
            _ => Outcome::unknown("The command output is unavailable."),
        }
    }

    async fn stop_command(&self, computer: &str, id: &str) -> Outcome<String> {
        self.exec(
            computer,
            &Exec::new(identified::bounded_stop_script(
                "/tmp/oa-public-commands",
                id,
            )),
        )
        .await
        .map(|o| o.stdout)
    }

    async fn put_file(&self, computer: &str, path: &str, bytes: &[u8]) -> Outcome<()> {
        match self.client.write_bytes(computer, path, bytes).await {
            Ok(_) => Outcome::done(()),
            Err(e) => mutation(e),
        }
    }

    async fn get_file(&self, computer: &str, path: &str) -> Outcome<Vec<u8>> {
        match self.client.read_bytes(computer, path).await {
            Ok(b) => Outcome::done(b),
            Err(e) if status_of(&e).is_some_and(definite) => Outcome::failed(e.to_string()),
            Err(e) => Outcome::unknown(e.to_string()),
        }
    }

    async fn meter(&self, computer: &str) -> Outcome<Meter> {
        match self
            .client
            .usage(&UsageParams {
                sandbox_id: computer.into(),
                ..Default::default()
            })
            .await
        {
            Ok(u) => Outcome::done(Meter {
                running: u.running,
                evidence: format!("boat-usage:{}:{}s", u.sandbox_id, u.seconds),
                seconds: u64::try_from(u.seconds).ok(),
            }),
            Err(e) if status_of(&e) == Some(404) => Outcome::done(Meter {
                running: false,
                evidence: format!("boat-gone:{computer}"),
                seconds: None,
            }),
            Err(e) => Outcome::unknown(e.to_string()),
        }
    }

    async fn delete(&self, computer: &str) -> Outcome<()> {
        match self
            .client
            .delete_sandbox(&DeleteSandboxParams {
                sandbox_id: computer.into(),
                x_ascii_confirm_delete: computer.into(),
                ..Default::default()
            })
            .await
        {
            Ok(_) => Outcome::done(()),
            Err(e) if status_of(&e) == Some(404) => Outcome::done(()),
            Err(e) => mutation(e),
        }
    }

    async fn suspend(&self, computer: &str) -> Outcome<String> {
        if !self.declared(Profile::Suspend) {
            return unsupported(Profile::Suspend);
        }
        match self
            .client
            .stop(&StopParams {
                sandbox_id: computer.into(),
                ..Default::default()
            })
            .await
        {
            Ok(r) => Outcome::done(match r.sandbox {
                Nullable::Value(sb) => match sb.stop {
                    Nullable::Value(op) => op.id,
                    _ => format!("stop:{computer}"),
                },
                _ => format!("stop:{computer}"),
            }),
            Err(e) => mutation(e),
        }
    }

    async fn resume(&self, computer: &str) -> Outcome<String> {
        if !self.declared(Profile::Suspend) {
            return unsupported(Profile::Suspend);
        }
        match self
            .client
            .resume(&ResumeParams {
                sandbox_id: computer.into(),
                ..Default::default()
            })
            .await
        {
            Ok(_) => Outcome::done(format!("resume:{computer}")),
            Err(e) => mutation(e),
        }
    }

    async fn snapshot(&self, computer: &str, name: &str) -> Outcome<String> {
        if !self.declared(Profile::Snapshot) {
            return unsupported(Profile::Snapshot);
        }
        match self
            .client
            .save_named_snapshot(&SaveNamedSnapshotParams {
                body: NamedSnapshotSaveRequest {
                    sandbox_id: computer.into(),
                    name: name.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .await
        {
            Ok(r) => Outcome::done(r.snapshot.name),
            Err(e) => mutation(e),
        }
    }

    async fn snapshot_ready(&self, name: &str) -> Outcome<bool> {
        if !self.declared(Profile::Snapshot) {
            return unsupported(Profile::Snapshot);
        }
        match self
            .client
            .get_named_snapshot(&GetNamedSnapshotParams {
                name: name.into(),
                ..Default::default()
            })
            .await
        {
            Ok(r) => Outcome::done(r.snapshot.status == "ready"),
            Err(e) if status_of(&e) == Some(404) => Outcome::done(false),
            Err(e) => Outcome::unknown(e.to_string()),
        }
    }

    async fn delete_snapshot(&self, name: &str) -> Outcome<()> {
        if !self.declared(Profile::Snapshot) {
            return unsupported(Profile::Snapshot);
        }
        match self
            .client
            .delete_named_snapshot(&DeleteNamedSnapshotParams {
                name: name.into(),
                ..Default::default()
            })
            .await
        {
            Ok(_) => Outcome::done(()),
            Err(e) if status_of(&e) == Some(404) => Outcome::done(()),
            Err(e) => mutation(e),
        }
    }

    async fn fork(&self, computer: &str, operation: &str) -> Outcome<String> {
        if !self.declared(Profile::Fork) {
            return unsupported(Profile::Fork);
        }
        match self
            .client
            .fork(&ForkParams {
                sandbox_id: computer.into(),
                idempotency_key: Some(operation.into()),
                ..Default::default()
            })
            .await
        {
            Ok(r) => Outcome::done(r.id),
            Err(e) => mutation(e),
        }
    }
}
