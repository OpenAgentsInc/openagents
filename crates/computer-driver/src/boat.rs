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

// Keep the existing saved-image consumers' repo path while the public file
// API and commands use a physical workspace directory. Refuse two different
// nonempty trees or a workspace symlink rather than merging private content.
const ENVIRONMENT_WORKSPACE: &str = r#"workspace="$HOME/workspace"
legacy="$HOME/repo"
[ ! -L "$workspace" ] || exit 125
if [ -L "$legacy" ]; then
  [ "$(realpath -e "$legacy")" = "$(realpath -e "$workspace")" ] || exit 125
elif [ -e "$legacy" ]; then
  [ -d "$legacy" ] || exit 125
  if [ -e "$workspace" ]; then
    [ -d "$workspace" ] && [ -z "$(ls -A "$workspace")" ] || exit 125
    rmdir "$workspace" || exit 125
  fi
  mv "$legacy" "$workspace" || exit 125
  ln -s "$workspace" "$legacy" || exit 125
else
  mkdir -p "$workspace" || exit 125
  ln -s "$workspace" "$legacy" || exit 125
fi
"#;

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
        let mut caps: Capabilities =
            serde_json::from_value(value).map_err(|e| format!("capabilities: {e}"))?;
        // Identified commands are implemented by this adapter over Core exec,
        // including services that predate the additive declaration field.
        caps.command_sessions = true;
        caps.environment_artifacts = caps.supports(Profile::Snapshot);
        caps.validate()?;
        Ok(Self { client, caps })
    }
    /// Drive a Boat service that does not serve `/capabilities` (the
    /// hosted boat.dev), declaring its profiles by hand.
    pub fn with_capabilities(client: Client, mut caps: Capabilities) -> Self {
        caps.environment_artifacts = caps.supports(Profile::Snapshot);
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
        "stopping" | "deleting" => State::Stopping,
        "stopped" => State::Stopped,
        "deleted" => State::Gone,
        "failed" | "error" => State::Failed,
        _ => State::Starting,
    }
}

impl Driver for BoatDriver {
    async fn capture_environment_image(&self, computer: &str, name: &str) -> Outcome<ImageRecord> {
        if !self.caps.environment_artifacts {
            return Outcome::failed("Environment images aren't supported on this computer.");
        }
        match self.read_environment_image(name).await {
            Outcome::Done { value: Some(image) } if image.source == computer => {
                return Outcome::done(image);
            }
            Outcome::Done { value: Some(_) } => {
                return Outcome::failed("This image belongs to another builder.");
            }
            Outcome::Done { value: None } => {}
            Outcome::Unknown { reason } | Outcome::Failed { reason } => {
                return Outcome::unknown(reason);
            }
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
            Ok(reply) => Outcome::done(environment_image(&reply.snapshot)),
            Err(error) => mutation(error),
        }
    }
    async fn read_environment_image(&self, name: &str) -> Outcome<Option<ImageRecord>> {
        match self
            .client
            .get_named_snapshot(&GetNamedSnapshotParams {
                name: name.into(),
                ..Default::default()
            })
            .await
        {
            Ok(reply) => Outcome::done(Some(environment_image(&reply.snapshot))),
            Err(error) if status_of(&error) == Some(404) => Outcome::done(None),
            Err(error) => Outcome::unknown(error.to_string()),
        }
    }
    async fn delete_environment_image(&self, name: &str) -> Outcome<bool> {
        match self
            .client
            .delete_named_snapshot(&DeleteNamedSnapshotParams {
                name: name.into(),
                ..Default::default()
            })
            .await
        {
            Ok(_) => Outcome::done(true),
            Err(error) if status_of(&error) == Some(404) => Outcome::done(false),
            Err(error) => mutation(error),
        }
    }
    async fn environment_image_hydrated(&self, computer: &str) -> Outcome<bool> {
        match self
            .client
            .get(&GetParams {
                sandbox_id: computer.into(),
                ..Default::default()
            })
            .await
        {
            Ok(reply)
                if matches!(
                    reply.sandbox.state.as_str(),
                    "error" | "cancelled" | "archived"
                ) =>
            {
                Outcome::failed("The verification computer is unavailable.")
            }
            Ok(reply) => Outcome::done(reply.sandbox.hydrated == Some(true)),
            Err(error) => Outcome::unknown(error.to_string()),
        }
    }

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
        if spec
            .credential_names
            .iter()
            .any(|name| !env_name_ok(name) || !spec.env.contains_key(name))
        {
            return Outcome::failed("invalid command credentials");
        }
        let mut public = spec.clone();
        let mut credentials = serde_json::Map::new();
        for name in &spec.credential_names {
            if let Some(value) = public.env.remove(name) {
                credentials.insert(name.clone(), serde_json::Value::String(value));
            }
        }
        let volatile = public
            .env
            .remove("OA_ENVIRONMENT_VOLATILE_OUTPUT")
            .as_deref()
            == Some("1");
        let root = if volatile {
            "/run/oa-environment-commands"
        } else {
            "/tmp/oa-public-commands"
        };
        let script = identified::bounded_command_script(root, "workspace", &public);
        let script = if volatile {
            format!(
                "{ENVIRONMENT_WORKSPACE}\nsudo -n install -d -m 700 -o \"$(id -u)\" -g \"$(id -g)\" /run/oa-environment-commands || exit 125\n{script}"
            )
        } else {
            script
        };
        let mut extra = BTreeMap::new();
        if !credentials.is_empty() {
            extra.insert("env".into(), serde_json::Value::Object(credentials));
        }
        match self
            .client
            .exec_detached(
                computer,
                CommandRequest {
                    command: script,
                    timeout_seconds: Some(600),
                    extra,
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
        let public = identified::read_script(
            "/tmp/oa-public-commands",
            id,
            cursor,
            max_bytes.clamp(1, 65536),
        );
        let volatile = identified::read_script(
            "/run/oa-environment-commands",
            id,
            cursor,
            max_bytes.clamp(1, 65536),
        );
        let path = ::boat::shell_quote(&format!("/run/oa-environment-commands/{id}"));
        let script = format!("if [ -d {path} ]; then\n{volatile}\nelse\n{public}\nfi");
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
        let public = identified::bounded_stop_script("/tmp/oa-public-commands", id);
        let volatile = identified::bounded_stop_script("/run/oa-environment-commands", id);
        let path = ::boat::shell_quote(&format!("/run/oa-environment-commands/{id}"));
        self.exec(
            computer,
            &Exec::new(format!(
                "if [ -d {path} ]; then\n{volatile}\nelse\n{public}\nfi"
            )),
        )
        .await
        .map(|o| o.stdout)
    }

    async fn read_workspace_file(
        &self,
        computer: &str,
        path: &str,
    ) -> Result<Vec<u8>, computer_files::FileError> {
        computer_files::validate_path(path)?;
        if self.caps.workspace_file_bytes.is_none() {
            return Err(computer_files::FileError::Unsupported);
        }
        let bytes = self
            .client
            .read_workspace_file(computer, path)
            .await
            .map_err(|e| file_error(&e, false))?;
        if bytes.len() > computer_files::MAX_BYTES {
            return Err(computer_files::FileError::TooLarge);
        }
        Ok(bytes)
    }
    async fn write_workspace_file(
        &self,
        computer: &str,
        path: &str,
        bytes: &[u8],
        overwrite: bool,
    ) -> Result<computer_files::FileInfo, computer_files::FileError> {
        computer_files::validate_path(path)?;
        if bytes.len() > computer_files::MAX_BYTES {
            return Err(computer_files::FileError::TooLarge);
        }
        if self.caps.workspace_file_bytes.is_none() {
            return Err(computer_files::FileError::Unsupported);
        }
        let info = self
            .client
            .write_workspace_file(computer, path, bytes, overwrite)
            .await
            .map_err(|e| file_error(&e, true))?;
        if info.size != bytes.len() || info.sha256 != computer_files::digest(bytes) {
            return Err(computer_files::FileError::WriteUncertain);
        }
        Ok(info)
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

    async fn storage(&self, computer: &str) -> Outcome<StorageObservation> {
        if !valid_storage_id(computer) {
            return Outcome::failed("Invalid computer ID.");
        }
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            self.client.list_named_snapshots(),
        )
        .await;
        let reply = match result {
            Ok(Ok(reply)) => reply,
            _ => return Outcome::unknown("Couldn't read storage information."),
        };
        let mut observation = StorageObservation {
            computer: computer.into(),
            objects: Vec::new(),
            inventories: vec![StorageInventory {
                kind: StorageKind::Snapshot,
                coverage: StorageCoverage::Partial,
                source: StorageSource::GceImage,
            }],
            cleanup: StorageCleanup::Unknown,
        };
        for image in reply.snapshots {
            if image.source_sandbox_id != computer {
                continue;
            }
            // Names can be reused. Only the typed GCE extension establishes
            // an immutable image ID, including while capture is pending.
            let Some(metadata) = image.extra.get("providerImage") else {
                continue;
            };
            if metadata.get("source").and_then(serde_json::Value::as_str) != Some("gce.images.id") {
                continue;
            }
            let Some(id) = metadata.get("id").and_then(serde_json::Value::as_str) else {
                continue;
            };
            if !id
                .parse::<u64>()
                .is_ok_and(|n| n != 0 && n.to_string() == id)
            {
                return Outcome::unknown("The storage response contained an invalid object ID.");
            }
            if observation.objects.len() == MAX_STORAGE_OBJECTS {
                break;
            }
            let bytes = image.extra.get("providerArchiveSize").and_then(|size| {
                (size.get("source").and_then(serde_json::Value::as_str)
                    == Some("gce.images.archiveSizeBytes"))
                .then(|| size.get("bytes").and_then(serde_json::Value::as_u64))
                .flatten()
                .filter(|bytes| *bytes <= i64::MAX as u64)
            });
            let state = match image.status.as_str() {
                "ready" | "saving" => StorageObjectState::Present,
                "deleting" => StorageObjectState::Deleting,
                _ => StorageObjectState::Unknown,
            };
            if state == StorageObjectState::Deleting {
                observation.cleanup = StorageCleanup::Pending;
            }
            observation.objects.push(StorageObject {
                id: id.into(),
                kind: StorageKind::Snapshot,
                state,
                bytes,
                byte_kind: StorageByteKind::ProviderArchive,
                source: StorageSource::GceImage,
            });
        }
        // The list does not prove boot-disk deletion or cover every private
        // artifact. An empty list therefore cannot release storage holds.
        match observation.validate() {
            Ok(()) => Outcome::done(observation),
            Err(_) => Outcome::unknown("The storage response was invalid."),
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

fn file_error(error: &Error, write: bool) -> computer_files::FileError {
    use computer_files::FileError as F;
    let Error::Api(api) = error else {
        return if write {
            F::WriteUncertain
        } else {
            F::Unavailable
        };
    };
    match api.status.as_u16() {
        400 => F::Invalid,
        403 => F::Escape,
        404 => F::Missing,
        409 => F::Changed,
        413 => F::TooLarge,
        415 => F::Special,
        412 => F::Exists,
        422 => F::Incomplete,
        501 => F::Unsupported,
        _ if write => F::WriteUncertain,
        _ => F::Unavailable,
    }
}

#[cfg(test)]
mod storage_tests {
    use super::*;
    use serde_json::{Value, json};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn observe(images: Value) -> Outcome<StorageObservation> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                let mut bytes = [0; 1024];
                let count = stream.read(&mut bytes).await.unwrap();
                assert!(count > 0 && request.len() + count < 16384);
                request.extend_from_slice(&bytes[..count]);
            }
            assert!(request.starts_with(b"GET /named-snapshots HTTP/1.1\r\n"));
            let body = json!({"ok":true,"type":"namedSnapshots","snapshots":images}).to_string();
            stream
                .write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes())
                .await
                .unwrap();
        });
        let client = Client::builder(::boat::ApiKey::new("synthetic-test-key").unwrap())
            .base_url(base)
            .build()
            .unwrap();
        let driver = BoatDriver::with_capabilities(
            client,
            Capabilities::new("oa-boat", Profile::ALL, &["small"]),
        );
        let result = std::sync::Arc::new(driver).storage("bx_test").await;
        server.await.unwrap();
        result
    }

    fn image(id: &str, status: &str, bytes: Value) -> Value {
        json!({
            "name":format!("image-{id}"), "status":status,
            "sourceSandboxId":"bx_test", "createdAt":"2026-10-16T00:00:00Z",
            "sizeBytes":9000, "diskGb":20,
            "providerImage":{"id":id,"source":"gce.images.id"},
            "providerArchiveSize":{"bytes":bytes,"source":"gce.images.archiveSizeBytes"}
        })
    }

    #[test]
    fn deleting_is_pending_until_the_resource_is_gone() {
        assert_eq!(state_of("deleting"), State::Stopping);
        assert_eq!(state_of("deleted"), State::Gone);
    }

    #[tokio::test]
    async fn archive_bytes_keep_their_meaning_and_unknown_is_not_zero() {
        let mut foreign = image("99", "ready", json!(50));
        foreign["sourceSandboxId"] = json!("bx_other");
        let mut shared_base = image("100", "ready", json!(90));
        shared_base["sourceSandboxId"] = json!("");
        let observation = observe(json!([
            image("1", "ready", json!(0)),
            image("2", "saving", Value::Null),
            image("3", "failed", json!(-1)),
            image("18446744073709551615", "deleting", json!(12)),
            foreign,
            shared_base
        ]))
        .await
        .value()
        .unwrap();
        assert_eq!(observation.objects.len(), 4);
        assert_eq!(observation.objects[0].bytes, Some(0));
        assert_eq!(observation.objects[1].bytes, None);
        assert_eq!(observation.objects[2].bytes, None);
        assert_eq!(observation.objects[2].state, StorageObjectState::Unknown);
        assert_eq!(observation.objects[3].state, StorageObjectState::Deleting);
        assert!(observation.objects.iter().all(|object| {
            object.source == StorageSource::GceImage
                && object.byte_kind == StorageByteKind::ProviderArchive
        }));
        assert_eq!(observation.cleanup, StorageCleanup::Pending);
        assert_eq!(
            observation.inventories[0].coverage,
            StorageCoverage::Partial
        );
    }

    #[tokio::test]
    async fn reused_names_missing_ids_and_empty_lists_never_prove_cleanup() {
        let mut missing = image("1", "saving", json!(10));
        missing["providerImage"]["id"] = Value::Null;
        let mut unqualified = image("2", "ready", json!(10));
        unqualified["providerArchiveSize"]["source"] = json!("disk capacity");
        let observation = observe(json!([missing, unqualified]))
            .await
            .value()
            .unwrap();
        assert_eq!(observation.objects.len(), 1);
        assert_eq!(observation.objects[0].bytes, None);
        assert_eq!(observation.cleanup, StorageCleanup::Unknown);
        let empty = observe(json!([])).await.value().unwrap();
        assert!(empty.objects.is_empty());
        assert_eq!(empty.cleanup, StorageCleanup::Unknown);
        assert_eq!(empty.inventories[0].coverage, StorageCoverage::Partial);
        assert!(matches!(
            observe(json!([
                image("1", "ready", json!(10)),
                image("1", "ready", json!(10))
            ]))
            .await,
            Outcome::Unknown { .. }
        ));
    }
}

#[cfg(test)]
mod credential_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn detached_credentials_use_stdin_environment_and_never_command_metadata() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let (header_end, size) = loop {
                let mut chunk = [0; 4096];
                let n = stream.read(&mut chunk).await.unwrap();
                assert!(n > 0 && bytes.len() + n < 131072);
                bytes.extend_from_slice(&chunk[..n]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]);
                    let size = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap();
                    break (end + 4, size);
                }
            };
            while bytes.len() < header_end + size {
                let mut chunk = [0; 4096];
                let n = stream.read(&mut chunk).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&chunk[..n]);
            }
            let body: serde_json::Value =
                serde_json::from_slice(&bytes[header_end..header_end + size]).unwrap();
            assert_eq!(body["env"]["ANTHROPIC_API_KEY"], "synthetic-private-value");
            assert!(
                !body["command"]
                    .as_str()
                    .unwrap()
                    .contains("synthetic-private-value")
            );
            assert_eq!(body["detached"], true);
            let reply = serde_json::to_vec(&CommandStartedResponse {
                ok: true,
                success: true,
                type_: "started".into(),
                process_id: 42,
                pid: 42,
                command: body["command"].as_str().unwrap().into(),
                started_at: "2026-10-16T00:00:00Z".into(),
                ..Default::default()
            })
            .unwrap();
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                reply.len()
            );
            stream.write_all(headers.as_bytes()).await.unwrap();
            stream.write_all(&reply).await.unwrap();
        });
        let client =
            ::boat::Client::builder(::boat::ApiKey::new("synthetic-service-credential").unwrap())
                .base_url(base)
                .build()
                .unwrap();
        let driver = BoatDriver::with_capabilities(
            client,
            Capabilities::new("fixture", Profile::ALL, &["small"]),
        );
        let spec = CommandSpec {
            id: "trn_0123456789abcdef0123".into(),
            command: "test -n \"$ANTHROPIC_API_KEY\"".into(),
            cwd: ".".into(),
            credential_names: BTreeSet::from(["ANTHROPIC_API_KEY".into()]),
            env: BTreeMap::from([("ANTHROPIC_API_KEY".into(), "synthetic-private-value".into())]),
            timeout_seconds: 600,
            digest: "a".repeat(64),
        };
        assert!(!format!("{spec:?}").contains("synthetic-private-value"));
        assert!(matches!(
            driver.start_command("computer-fixture", &spec).await,
            Outcome::Done { .. }
        ));
        server.await.unwrap();
    }
}

fn environment_image(s: &NamedSnapshot) -> ImageRecord {
    let state = match s.status.as_str() {
        "completed" | "ready" | "saved" if s.snapshot_id.is_some() => ImageState::Ready,
        "failed" | "error" => ImageState::Failed {
            reason: "The image couldn't be saved.".into(),
        },
        _ => ImageState::Pending,
    };
    ImageRecord {
        name: s.name.clone(),
        source: s.source_sandbox_id.clone(),
        state,
        snapshot: s.snapshot_id.clone(),
        size_bytes: s.size_bytes.and_then(|n| u64::try_from(n).ok()),
    }
}

#[cfg(test)]
mod workspace_tests {
    use super::ENVIRONMENT_WORKSPACE;
    use std::{fs, process::Command};
    fn run(home: &std::path::Path) -> bool {
        Command::new("sh")
            .arg("-c")
            .arg(ENVIRONMENT_WORKSPACE)
            .env("HOME", home)
            .current_dir(home)
            .status()
            .unwrap()
            .success()
    }
    #[test]
    fn saved_images_keep_the_original_repo_path_and_the_public_workspace() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir(home.path().join("repo")).unwrap();
        fs::write(home.path().join("repo/source"), b"original").unwrap();
        assert!(run(home.path()));
        assert!(run(home.path()));
        assert_eq!(
            fs::read(home.path().join("workspace/source")).unwrap(),
            b"original"
        );
        assert_eq!(
            fs::read(home.path().join("repo/source")).unwrap(),
            b"original"
        );
        assert!(
            fs::symlink_metadata(home.path().join("repo"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
    #[test]
    fn conflicting_trees_and_a_workspace_symlink_are_refused() {
        use std::os::unix::fs::symlink;
        let home = tempfile::tempdir().unwrap();
        for directory in ["repo", "workspace"] {
            fs::create_dir(home.path().join(directory)).unwrap();
            fs::write(home.path().join(directory).join("source"), directory).unwrap();
        }
        assert!(!run(home.path()));
        assert_eq!(fs::read(home.path().join("repo/source")).unwrap(), b"repo");
        assert_eq!(
            fs::read(home.path().join("workspace/source")).unwrap(),
            b"workspace"
        );
        let other = tempfile::tempdir().unwrap();
        fs::remove_dir_all(home.path().join("workspace")).unwrap();
        symlink(other.path(), home.path().join("workspace")).unwrap();
        assert!(!run(home.path()));
        assert!(fs::read_dir(other.path()).unwrap().next().is_none());
    }
}
