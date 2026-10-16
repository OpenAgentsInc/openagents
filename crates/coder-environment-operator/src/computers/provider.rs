use super::*;
use coder_working_computer::provider::{
    CheckpointEvidence, CommandCursor, CommandProgress, CommandRead, CommandSpec, Commands,
    ImageRecord, Images, Inspection, Meter, Outcome, Provider,
};
use coder_working_computer::{Checkpoint, ServiceDecl};
use openagents_sdk::{
    CancellationToken,
    computers::{CommandOptions, CommandOutcome, Error as ApiError, WaitFor},
};

pub struct Selected<P> {
    pub legacy: P,
    pub shared: bool,
    pub runtime: Option<Arc<Runtime>>,
    pub store: Store,
}
fn unknown<T>(_: impl std::fmt::Display) -> Outcome<T> {
    Outcome::unknown("We couldn't confirm what happened on this computer. Try again shortly.")
}
fn api_error<T>(e: ApiError) -> Outcome<T> {
    match e {
        ApiError::PaymentRequired(_)
        | ApiError::Api {
            status: 400..=499, ..
        } if !matches!(
            e,
            ApiError::Api {
                status: 408 | 429,
                ..
            }
        ) =>
        {
            Outcome::failed("The Environment computer request was refused.")
        }
        ApiError::Unsupported | ApiError::InvalidInput => {
            Outcome::failed("These computer settings aren't supported.")
        }
        _ => unknown(e),
    }
}
impl<P> Selected<P> {
    fn selected(&self, c: &Computer) -> Result<Option<&Runtime>, String> {
        match self.store.binding(&c.id)? {
            Some(b) => {
                if !b.validates(c) {
                    return Err("The computer settings changed after this attempt started.".into());
                }
                self.runtime
                    .as_deref()
                    .map(Some)
                    .ok_or_else(|| "The Environment computer connection is unavailable.".into())
            }
            None => {
                if c.resource().is_some_and(|id| id.starts_with("cmp_")) {
                    return Err("The shared computer record is missing.".into());
                }
                let Some(id) = environment(c) else {
                    return Ok(None);
                };
                let public = self
                    .store
                    .metadata(id)
                    .map(|v| v["public_computers"] == true)
                    .unwrap_or(false);
                if public {
                    self.runtime
                        .as_deref()
                        .map(Some)
                        .ok_or_else(|| "The Environment computer connection is unavailable.".into())
                } else {
                    Ok(None)
                }
            }
        }
    }
    fn binding(&self, c: &Computer, resource: &str) -> Result<Binding, String> {
        let b = self
            .store
            .binding(&c.id)?
            .ok_or("The shared computer record is missing.")?;
        if !b.validates(c) || b.computer.as_ref().is_none_or(|r| r.id() != resource) {
            return Err("This computer belongs to another attempt.".into());
        }
        Ok(b)
    }
    async fn stop_shared(&self, r: &Runtime, b: &Binding) -> Outcome<String> {
        let session = match r.session(b).await {
            Ok(s) => s,
            Err(e) => return unknown(e),
        };
        let api = match r.client.computers(&session) {
            Ok(a) => a,
            Err(e) => return api_error(e),
        };
        let Some(reference) = b.computer.as_ref() else {
            return Outcome::done("No computer was created.".into());
        };
        let cancel = CancellationToken::new();
        if let Err(e) = api.stop(reference, &cancel).await {
            return api_error(e);
        }
        match api
            .wait(
                reference,
                WaitFor::Stopped,
                Duration::from_secs(600),
                &cancel,
            )
            .await
        {
            Ok(_) => Outcome::done(reference.id().into()),
            Err(e) => api_error(e),
        }
    }
}
impl<P: Provider> Provider for Selected<P> {
    fn kind(&self) -> ProviderKind {
        self.legacy.kind()
    }
    fn admits_base(&self, base: &coder_environment::ImagePin) -> Result<(), &'static str> {
        self.legacy.admits_base(base)
    }
    async fn create(&self, c: &Computer, operation: &str) -> Outcome<String> {
        let runtime = match self.selected(c) {
            Ok(Some(r)) => r,
            Ok(None) => return self.legacy.create(c, operation).await,
            Err(e) => return unknown(e),
        };
        let _lock = match self.store.lock(&c.id) {
            Ok(l) => l,
            Err(e) => return unknown(e),
        };
        let mut b = match runtime.prepare(c, operation) {
            Ok(b) => b,
            Err(_) => {
                return Outcome::failed(
                    "The Environment computer settings or access are unavailable.",
                );
            }
        };
        if b.deleted {
            return Outcome::failed("This attempt's computer was deleted.");
        }
        let session = match runtime.session(&b).await {
            Ok(s) => s,
            Err(e) => return unknown(e),
        };
        let api = match runtime.client.computers(&session) {
            Ok(a) => a,
            Err(e) => return api_error(e),
        };
        let cancel = CancellationToken::new();
        if b.request.is_none() {
            b.request = match api.create_request(b.options.clone()) {
                Ok(r) => Some(r),
                Err(e) => return api_error(e),
            };
            if let Err(e) = self.store.save(&b) {
                return unknown(e);
            }
        }
        if b.computer.is_none() {
            b.computer = match api.create(b.request.as_ref().unwrap(), &cancel).await {
                Ok(c) => Some(c.reference().clone()),
                Err(e) => {
                    let found: Option<String> = runtime
                        .private(&b, "computer", &Value::Null)
                        .await
                        .unwrap_or(None);
                    match found {
                        Some(id) => match api.reference(&id) {
                            Ok(reference) => Some(reference),
                            Err(_) => return unknown("The saved computer reference is invalid."),
                        },
                        None => return api_error(e),
                    }
                }
            };
            if let Err(e) = self.store.save(&b) {
                return unknown(e);
            }
        }
        let reference = b.computer.as_ref().unwrap();
        match api
            .wait(reference, WaitFor::Awake, Duration::from_secs(600), &cancel)
            .await
        {
            Ok(_) => Outcome::done(reference.id().into()),
            Err(e) => api_error(e),
        }
    }
    async fn restore(
        &self,
        c: &Computer,
        resource: &str,
        checkpoint: Option<&Checkpoint>,
    ) -> Outcome<String> {
        let r = match self.selected(c) {
            Ok(Some(r)) => r,
            Ok(None) => return self.legacy.restore(c, resource, checkpoint).await,
            Err(e) => return unknown(e),
        };
        if checkpoint.is_some_and(|checkpoint| checkpoint.resource != resource) {
            return Outcome::failed("This checkpoint belongs to another computer.");
        }
        let b = match self.binding(c, resource) {
            Ok(b) => b,
            Err(e) => return unknown(e),
        };
        let session = match r.session(&b).await {
            Ok(s) => s,
            Err(e) => return unknown(e),
        };
        let api = match r.client.computers(&session) {
            Ok(a) => a,
            Err(e) => return api_error(e),
        };
        let cancel = CancellationToken::new();
        if let Err(e) = api.wake(b.computer.as_ref().unwrap(), &cancel).await {
            return api_error(e);
        }
        match api
            .wait(
                b.computer.as_ref().unwrap(),
                WaitFor::Awake,
                Duration::from_secs(600),
                &cancel,
            )
            .await
        {
            Ok(_) => Outcome::done(resource.into()),
            Err(e) => api_error(e),
        }
    }
    async fn apply_credentials(&self, c: &Computer, resource: &str) -> Outcome<String> {
        match self.selected(c) {
            Ok(None) => self.legacy.apply_credentials(c, resource).await,
            Err(e) => unknown(e),
            Ok(Some(r)) => {
                let b = match self.binding(c, resource) {
                    Ok(b) => b,
                    Err(e) => return unknown(e),
                };
                match r
                    .private::<_, Value>(
                        &b,
                        "credentials-check",
                        &json!({"names":c.credential_names}),
                    )
                    .await
                {
                    Ok(_) => Outcome::done(
                        "Credentials are available for this attempt's commands.".into(),
                    ),
                    Err(_) => Outcome::failed("The repository's access is no longer available."),
                }
            }
        }
    }
    async fn start_service(
        &self,
        c: &Computer,
        resource: &str,
        service: &ServiceDecl,
    ) -> Outcome<String> {
        match self.selected(c) {
            Ok(None) => self.legacy.start_service(c, resource, service).await,
            Err(e) => unknown(e),
            Ok(Some(_)) => Outcome::failed(
                "Environment attempts start services through their checked commands.",
            ),
        }
    }
    async fn checkpoint(
        &self,
        c: &Computer,
        resource: &str,
        generation: u64,
    ) -> Outcome<CheckpointEvidence> {
        let r = match self.selected(c) {
            Ok(Some(r)) => r,
            Ok(None) => return self.legacy.checkpoint(c, resource, generation).await,
            Err(e) => return unknown(e),
        };
        let b = match self.binding(c, resource) {
            Ok(b) => b,
            Err(e) => return unknown(e),
        };
        match self.stop_shared(r, &b).await {
            Outcome::Done { value } => Outcome::done(CheckpointEvidence {
                snapshot: resource.into(),
                stopped: Some(value),
            }),
            Outcome::Failed { reason } => Outcome::failed(reason),
            Outcome::Unknown { reason } => Outcome::unknown(reason),
        }
    }
    async fn shutdown_processes(&self, c: &Computer, resource: &str) -> Outcome<String> {
        match self.selected(c) {
            Ok(None) => self.legacy.shutdown_processes(c, resource).await,
            Err(e) => unknown(e),
            Ok(Some(r)) => match self.binding(c, resource) {
                Ok(b) => self.stop_shared(r, &b).await,
                Err(e) => unknown(e),
            },
        }
    }
    async fn stop(&self, c: &Computer, resource: &str) -> Outcome<String> {
        match self.selected(c) {
            Ok(None) => self.legacy.stop(c, resource).await,
            Err(e) => unknown(e),
            Ok(Some(r)) => match self.binding(c, resource) {
                Ok(b) => self.stop_shared(r, &b).await,
                Err(e) => unknown(e),
            },
        }
    }
    async fn meter(&self, c: &Computer, resource: &str) -> Outcome<Meter> {
        let r = match self.selected(c) {
            Ok(Some(r)) => r,
            Ok(None) => return self.legacy.meter(c, resource).await,
            Err(e) => return unknown(e),
        };
        let b = match self.binding(c, resource) {
            Ok(b) => b,
            Err(e) => return unknown(e),
        };
        let session = match r.session(&b).await {
            Ok(s) => s,
            Err(e) => return unknown(e),
        };
        let api = match r.client.computers(&session) {
            Ok(a) => a,
            Err(e) => return api_error(e),
        };
        match api
            .get(b.computer.as_ref().unwrap(), &CancellationToken::new())
            .await
        {
            Ok(value) => Outcome::done(Meter {
                running: !value.stopped() && value.data().state != "deleted",
                evidence: format!("computer:{resource}:usage-unknown"),
                seconds: None,
            }),
            Err(e) => api_error(e),
        }
    }
    async fn delete(&self, c: &Computer, resource: &str) -> Outcome<String> {
        let r = match self.selected(c) {
            Ok(Some(r)) => r,
            Ok(None) => return self.legacy.delete(c, resource).await,
            Err(e) => return unknown(e),
        };
        let _lock = match self.store.lock(&c.id) {
            Ok(l) => l,
            Err(e) => return unknown(e),
        };
        let mut b = match self.binding(c, resource) {
            Ok(b) => b,
            Err(e) => return unknown(e),
        };
        let session = match r.session(&b).await {
            Ok(s) => s,
            Err(e) => return unknown(e),
        };
        let api = match r.client.computers(&session) {
            Ok(a) => a,
            Err(e) => return api_error(e),
        };
        let cancel = CancellationToken::new();
        let reference = b.computer.as_ref().unwrap();
        if let Err(e) = api.delete(reference, &cancel).await {
            // The delete may have succeeded before its reply was lost.
            if !matches!(api.get(reference, &cancel).await, Ok(value) if value.data().state == "deleted")
            {
                return api_error(e);
            }
        }
        match api
            .wait(
                reference,
                WaitFor::Deleted,
                Duration::from_secs(600),
                &cancel,
            )
            .await
        {
            Ok(_) => {
                b.deleted = true;
                if let Err(e) = self.store.save(&b) {
                    return unknown(e);
                }
                Outcome::done(resource.into())
            }
            Err(e) => api_error(e),
        }
    }
    async fn inspect(&self, c: &Computer, resource: &str) -> Outcome<Inspection> {
        let r = match self.selected(c) {
            Ok(Some(r)) => r,
            Ok(None) => return self.legacy.inspect(c, resource).await,
            Err(e) => return unknown(e),
        };
        let b = match self.binding(c, resource) {
            Ok(b) => b,
            Err(e) => return unknown(e),
        };
        let session = match r.session(&b).await {
            Ok(s) => s,
            Err(e) => return unknown(e),
        };
        let api = match r.client.computers(&session) {
            Ok(a) => a,
            Err(e) => return api_error(e),
        };
        match api
            .get(b.computer.as_ref().unwrap(), &CancellationToken::new())
            .await
        {
            Ok(value) => Outcome::done(Inspection {
                running: match value.data().state.as_str() {
                    "deleted" => None,
                    "stopped" => Some(false),
                    _ => Some(true),
                },
                stop: value.stopped().then(|| resource.into()),
                latest_snapshot: value.stopped().then(|| resource.into()),
            }),
            Err(e) => api_error(e),
        }
    }
}
impl<P: Commands> Commands for Selected<P> {
    async fn start_command(
        &self,
        c: &Computer,
        resource: &str,
        spec: &CommandSpec,
    ) -> Outcome<String> {
        let r = match self.selected(c) {
            Ok(Some(r)) => r,
            Ok(None) => return self.legacy.start_command(c, resource, spec).await,
            Err(e) => return unknown(e),
        };
        if spec.timeout_seconds == 0
            || spec.timeout_seconds > 3600
            || !spec.credential_names.is_subset(&c.credential_names)
        {
            return Outcome::failed("The command exceeds this attempt's limits.");
        }
        let _lock = match self.store.lock(&c.id) {
            Ok(l) => l,
            Err(e) => return unknown(e),
        };
        let mut b = match self.binding(c, resource) {
            Ok(b) => b,
            Err(e) => return unknown(e),
        };
        let session = match r.session(&b).await {
            Ok(s) => s,
            Err(e) => return unknown(e),
        };
        let api = match r.client.computers(&session) {
            Ok(a) => a,
            Err(e) => return api_error(e),
        };
        if let Some(saved) = b.commands.get(&spec.id) {
            if saved.spec != *spec {
                return Outcome::failed(
                    "This command identity already has different instructions.",
                );
            }
        } else {
            let key = format!(
                "env-command-{}",
                &digest(format!("{}:{}", c.id, spec.id).as_bytes())[..48]
            );
            let reference = format!(
                "cred_env_{}",
                &digest(format!("{}:{}", c.id, spec.id).as_bytes())[..48]
            );
            // The public credential-reference contract carries no caller env.
            // These are the native owner's non-secret Git settings and cwd;
            // credential values still arrive separately at dispatch.
            let mut command = spec.command.clone();
            if !spec.env.is_empty() {
                if spec.env.keys().any(|name| {
                    !computer_driver::env_name_ok(name) || spec.credential_names.contains(name)
                }) {
                    return Outcome::failed("The command's environment settings are invalid.");
                }
                let assignments = spec
                    .env
                    .iter()
                    .map(|(name, value)| format!("{name}={}", boat::shell_quote(value)))
                    .collect::<Vec<_>>()
                    .join(" ");
                command = format!("env {assignments} sh -c {}", boat::shell_quote(&command));
            }
            if spec.cwd != "." {
                if computer_files_path_invalid(&spec.cwd) {
                    return Outcome::failed("The command's working directory is invalid.");
                }
                command = format!("cd {} && {command}", boat::shell_quote(&spec.cwd));
            }
            let options = CommandOptions {
                command,
                cwd: Some(".".into()),
                timeout_seconds: Some(spec.timeout_seconds),
                env: None,
                idempotency_key: Some(key),
                credential_ref: (!spec.credential_names.is_empty()).then_some(reference),
                extra: Default::default(),
            };
            let request = match api.command_request(b.computer.as_ref().unwrap(), options) {
                Ok(q) => q,
                Err(e) => return api_error(e),
            };
            b.commands.insert(
                spec.id.clone(),
                SavedCommand {
                    digest: spec.digest.clone(),
                    spec: spec.clone(),
                    request,
                    reference: None,
                },
            );
            if let Err(e) = self.store.save(&b) {
                return unknown(e);
            }
        }
        let saved = b.commands.get(&spec.id).unwrap();
        if let Some(command) = &saved.reference {
            return Outcome::done(command.id().into());
        }
        if !spec.credential_names.is_empty() {
            if r.private::<_, Value>(&b, "credentials", &json!({"command":spec.id}))
                .await
                .is_err()
            {
                return Outcome::failed("The repository's access is no longer available.");
            }
        }
        match api.execute(&saved.request, &CancellationToken::new()).await {
            Ok(command) => {
                let reference = command.reference().clone();
                let id = reference.id().to_owned();
                b.commands.get_mut(&spec.id).unwrap().reference = Some(reference);
                if let Err(e) = self.store.save(&b) {
                    return unknown(e);
                }
                Outcome::done(id)
            }
            Err(e) => api_error(e),
        }
    }
    async fn read_command(
        &self,
        c: &Computer,
        resource: &str,
        id: &str,
        cursor: CommandCursor,
        max_bytes: u64,
    ) -> Outcome<CommandRead> {
        let r = match self.selected(c) {
            Ok(Some(r)) => r,
            Ok(None) => {
                return self
                    .legacy
                    .read_command(c, resource, id, cursor, max_bytes)
                    .await;
            }
            Err(e) => return unknown(e),
        };
        let _lock = match self.store.lock(&c.id) {
            Ok(l) => l,
            Err(e) => return unknown(e),
        };
        let mut b = match self.binding(c, resource) {
            Ok(b) => b,
            Err(e) => return unknown(e),
        };
        let Some(saved) = b.commands.get(id) else {
            return Outcome::done(CommandRead {
                timed_out: false,
                progress: CommandProgress::Absent,
                digest: None,
                stdout: vec![],
                stderr: vec![],
            });
        };
        if saved.reference.is_none() {
            let recovered: Option<String> =
                match r.private(&b, "command", &json!({"command":id})).await {
                    Ok(v) => v,
                    Err(e) => return unknown(e),
                };
            if let Some(found) = recovered {
                let session = match r.session(&b).await {
                    Ok(s) => s,
                    Err(e) => return unknown(e),
                };
                let api = match r.client.computers(&session) {
                    Ok(a) => a,
                    Err(e) => return api_error(e),
                };
                let reference = match api.command_reference(b.computer.as_ref().unwrap(), &found) {
                    Ok(c) => c,
                    Err(e) => return api_error(e),
                };
                b.commands.get_mut(id).unwrap().reference = Some(reference);
                if let Err(e) = self.store.save(&b) {
                    return unknown(e);
                }
            } else {
                return Outcome::done(CommandRead {
                    timed_out: false,
                    progress: CommandProgress::Absent,
                    digest: Some(saved.digest.clone()),
                    stdout: vec![],
                    stderr: vec![],
                });
            }
        }
        let saved = b.commands.get(id).unwrap();
        let session = match r.session(&b).await {
            Ok(s) => s,
            Err(e) => return unknown(e),
        };
        let api = match r.client.computers(&session) {
            Ok(a) => a,
            Err(e) => return api_error(e),
        };
        let mut output = match api.output_at(
            saved.reference.as_ref().unwrap(),
            cursor.stdout,
            cursor.stderr,
        ) {
            Ok(o) => o,
            Err(e) => return api_error(e),
        };
        match output.next_page(&api, &CancellationToken::new()).await {
            Ok(Some(mut page)) => {
                if page.stdout_gap || page.stderr_gap {
                    return Outcome::unknown("Some build output is missing.");
                }
                let max = max_bytes.clamp(1, 65536) as usize;
                let more = page.has_more || page.stdout.len() > max || page.stderr.len() > max;
                page.stdout.truncate(max);
                page.stderr.truncate(max);
                let progress = if !page.terminal || more {
                    CommandProgress::Running
                } else {
                    match page.command.outcome() {
                        Some(CommandOutcome::Succeeded | CommandOutcome::Failed) => page
                            .command
                            .data()
                            .exit_code
                            .map(|code| CommandProgress::Exited { code })
                            .unwrap_or(CommandProgress::Lost),
                        Some(CommandOutcome::Cancelled | CommandOutcome::TimedOut) => {
                            CommandProgress::Lost
                        }
                        _ => CommandProgress::Lost,
                    }
                };
                Outcome::done(CommandRead {
                    timed_out: matches!(page.command.outcome(), Some(CommandOutcome::TimedOut)),
                    progress,
                    digest: Some(saved.digest.clone()),
                    stdout: page.stdout,
                    stderr: page.stderr,
                })
            }
            Ok(None) => Outcome::unknown("The command output could not be observed."),
            Err(e) => api_error(e),
        }
    }
    async fn stop_command(&self, c: &Computer, resource: &str, id: &str) -> Outcome<String> {
        let r = match self.selected(c) {
            Ok(Some(r)) => r,
            Ok(None) => return self.legacy.stop_command(c, resource, id).await,
            Err(e) => return unknown(e),
        };
        let b = match self.binding(c, resource) {
            Ok(b) => b,
            Err(e) => return unknown(e),
        };
        let Some(saved) = b.commands.get(id) else {
            return Outcome::done("No command was started.".into());
        };
        let session = match r.session(&b).await {
            Ok(s) => s,
            Err(e) => return unknown(e),
        };
        let api = match r.client.computers(&session) {
            Ok(a) => a,
            Err(e) => return api_error(e),
        };
        let reference = match &saved.reference {
            Some(c) => c.clone(),
            None => {
                let recovered: Option<String> =
                    match r.private(&b, "command", &json!({"command":id})).await {
                        Ok(v) => v,
                        Err(e) => return unknown(e),
                    };
                let Some(id) = recovered else {
                    return Outcome::done("No command was started.".into());
                };
                match api.command_reference(b.computer.as_ref().unwrap(), &id) {
                    Ok(c) => c,
                    Err(e) => return api_error(e),
                }
            }
        };
        match api
            .cancel_command(&reference, &CancellationToken::new())
            .await
        {
            Ok(_) => Outcome::done(reference.id().into()),
            Err(e) => api_error(e),
        }
    }
}
impl<P: Images> Images for Selected<P> {
    async fn capture_image(
        &self,
        c: &Computer,
        resource: &str,
        name: &str,
    ) -> Outcome<ImageRecord> {
        let r = match self.selected(c) {
            Ok(Some(r)) => r,
            Ok(None) => return self.legacy.capture_image(c, resource, name).await,
            Err(e) => return unknown(e),
        };
        let b = match self.binding(c, resource) {
            Ok(b) => b,
            Err(e) => return unknown(e),
        };
        if b.attempt.role != EnvironmentRole::Builder
            || b.attempt.image_name.as_deref() != Some(name)
        {
            return Outcome::failed("Only this clean builder can save its output image.");
        }
        match r
            .private::<_, Outcome<Option<ImageRecord>>>(&b, "image", &json!("capture"))
            .await
        {
            Ok(Outcome::Done { value: Some(image) }) => Outcome::done(image),
            Ok(Outcome::Done { value: None }) => {
                Outcome::unknown("The image has not been saved yet.")
            }
            Ok(Outcome::Failed { reason }) => Outcome::failed(reason),
            Ok(Outcome::Unknown { reason }) => Outcome::unknown(reason),
            Err(e) => unknown(e),
        }
    }
    async fn read_image(&self, name: &str) -> Outcome<Option<ImageRecord>> {
        let id = match self.store.image_owner(name) {
            Ok(Some(id)) => id,
            Ok(None) if !self.shared => return self.legacy.read_image(name).await,
            Ok(None) => return unknown("Shared image record missing"),
            Err(e) => return unknown(e),
        };
        let Some(r) = self.runtime.as_deref() else {
            return unknown("Shared image connection missing");
        };
        let b = match self.store.binding(&id) {
            Ok(Some(b)) => b,
            _ => return unknown("Shared image record missing"),
        };
        match r
            .private::<_, Outcome<Option<ImageRecord>>>(&b, "image", &json!("read"))
            .await
        {
            Ok(o) => o,
            Err(e) => unknown(e),
        }
    }
    async fn hydration(&self, c: &Computer, resource: &str) -> Outcome<bool> {
        let r = match self.selected(c) {
            Ok(Some(r)) => r,
            Ok(None) => return self.legacy.hydration(c, resource).await,
            Err(e) => return unknown(e),
        };
        let b = match self.binding(c, resource) {
            Ok(b) => b,
            Err(e) => return unknown(e),
        };
        match r
            .private::<_, Outcome<bool>>(&b, "image", &json!("hydrated"))
            .await
        {
            Ok(o) => o,
            Err(e) => unknown(e),
        }
    }
    async fn delete_image(&self, name: &str) -> Outcome<bool> {
        let id = match self.store.image_owner(name) {
            Ok(Some(id)) => id,
            Ok(None) if !self.shared => return self.legacy.delete_image(name).await,
            Ok(None) => return unknown("Shared image record missing"),
            Err(e) => return unknown(e),
        };
        let Some(r) = self.runtime.as_deref() else {
            return unknown("Shared image connection missing");
        };
        let _lock = match self.store.lock(&id) {
            Ok(lock) => lock,
            Err(e) => return unknown(e),
        };
        let mut b = match self.store.binding(&id) {
            Ok(Some(b)) => b,
            _ => return unknown("Shared image record missing"),
        };
        match r
            .private::<_, Outcome<bool>>(&b, "image", &json!("delete"))
            .await
        {
            Ok(Outcome::Done { value }) => {
                if !value
                    && crate::now_ms()
                        < b.attempt
                            .deadline_ms
                            .saturating_add(crate::studio::leftovers::DEADLINE_GRACE_MS)
                {
                    return unknown("An earlier capture may still finish.");
                }
                b.image_deleted = true;
                if let Err(e) = self.store.save(&b) {
                    return unknown(e);
                }
                Outcome::done(value)
            }
            Ok(o) => o,
            Err(e) => unknown(e),
        }
    }
}

fn computer_files_path_invalid(path: &str) -> bool {
    path.is_empty()
        || path.starts_with('/')
        || path.chars().any(char::is_control)
        || path.split('/').any(|p| p.is_empty() || p == "..")
}
