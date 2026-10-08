//! Chat selections and inert requests over the current native Cloud authority.

use super::controls::{self, Context};
use super::session::SessionError;
use super::{operator, refused, service};
use crate::App;
use crate::chat_store::{CloudRequest, RuntimeSelection, Selection};
use crate::composer::RuntimeChoice;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use coder_access::cloud;
use coder_access::protocol::{Operation, Outcome};
use sha2::{Digest, Sha256};
use std::future::Future;
use std::time::Duration;

const DEADLINE: Duration = Duration::from_secs(8);
const MAX_CHOICES: usize = 128;

async fn bounded<T>(operation: impl Future<Output = Result<T, Response>>) -> Result<T, Response> {
    tokio::time::timeout(DEADLINE, operation)
        .await
        .map_err(|_| refused(SessionError::Unavailable))?
}

/// Read only profiles admitted by the current account and native host bindings.
pub(crate) async fn choices(
    app: &App,
    headers: &HeaderMap,
) -> Result<Vec<RuntimeChoice>, Response> {
    bounded(async {
        let service = service(app)?;
        let viewer = service.authenticate(headers).await.map_err(refused)?;
        let hosts = app
            .config
            .cloud_hosts
            .as_deref()
            .ok_or_else(|| refused(SessionError::Unavailable))?;
        let mut choices = Vec::new();
        for binding in hosts.current(&viewer) {
            let context = controls::admitted(app, headers, binding.id()).await?;
            let operation = Operation::CloudProjects {
                workspace: context.binding.workspace().into(),
            };
            let outcome = match context
                .binding
                .read(&context.viewer, operation.clone())
                .await
            {
                Ok(outcome) => outcome,
                Err(SessionError::Forbidden | SessionError::Unavailable) => {
                    context.current(headers).await?;
                    continue;
                }
                Err(error) => return Err(refused(error)),
            };
            if outcome.validate().is_err() || !outcome.answers(&operation) {
                return Err(refused(SessionError::Conflict));
            }
            context.current(headers).await?;
            let Outcome::CloudProjects { projects } = outcome else {
                return Err(refused(SessionError::Conflict));
            };
            for project in projects.projects {
                let (_, outcome) = match operator::catalog(&context, headers, &project).await {
                    Ok(value) => value,
                    Err(response)
                        if matches!(
                            response.status(),
                            StatusCode::FORBIDDEN | StatusCode::SERVICE_UNAVAILABLE
                        ) =>
                    {
                        context.current(headers).await?;
                        continue;
                    }
                    Err(response) => return Err(response),
                };
                let Outcome::CloudCatalog { catalog } = outcome else {
                    return Err(refused(SessionError::Conflict));
                };
                for profile in catalog.profiles {
                    if choices.len() == MAX_CHOICES {
                        return Err(refused(SessionError::Conflict));
                    }
                    let runtime = selection(&context, &project, &profile)?;
                    choices.push(RuntimeChoice {
                        available: operator::can_operate(&context)
                            && profile.availability == "configured",
                        runtime,
                        repository: profile.repository,
                        branch: profile.branch,
                        template: profile.template,
                        size: profile.size,
                    });
                }
            }
        }
        Ok(choices)
    })
    .await
}

fn selection(
    context: &Context<'_>,
    project: &str,
    profile: &cloud::Profile,
) -> Result<RuntimeSelection, Response> {
    let workspace = context
        .viewer
        .workspace
        .as_ref()
        .ok_or_else(|| refused(SessionError::Forbidden))?;
    let runtime = RuntimeSelection {
        binding: context.binding.id().into(),
        account: context.viewer.account_id.clone(),
        workspace: context.binding.workspace().into(),
        members_epoch: workspace.members_epoch,
        project: project.into(),
        profile: profile.name.clone(),
        profile_revision: profile.revision.clone(),
        source_revision: profile.source_revision.clone(),
        source_digest: profile.source_digest.clone(),
        placement: profile.placement.clone(),
        executor: profile.executor.clone(),
        model: profile.model.clone(),
        max_timeout_seconds: profile.max_timeout_seconds,
    };
    runtime
        .validate()
        .map_err(|_| refused(SessionError::Conflict))?;
    Ok(runtime)
}

fn authority(context: &Context<'_>, runtime: &RuntimeSelection) -> Result<(), Response> {
    runtime
        .validate()
        .map_err(|_| refused(SessionError::InvalidRequest))?;
    if context.viewer.account_id != runtime.account
        || context.binding.id() != runtime.binding
        || context.binding.workspace() != runtime.workspace
        || context
            .viewer
            .workspace
            .as_ref()
            .is_none_or(|workspace| workspace.members_epoch != runtime.members_epoch)
    {
        return Err(refused(SessionError::Forbidden));
    }
    context.enrolled()
}

fn matches_profile(runtime: &RuntimeSelection, profile: &cloud::Profile) -> bool {
    profile.name == runtime.profile
        && profile.revision == runtime.profile_revision
        && profile.source_revision == runtime.source_revision
        && profile.source_digest == runtime.source_digest
        && profile.placement == runtime.placement
        && profile.executor == runtime.executor
        && profile.model == runtime.model
        && profile.max_timeout_seconds == runtime.max_timeout_seconds
}

async fn chosen<'a>(
    app: &'a App,
    headers: &HeaderMap,
    runtime: &RuntimeSelection,
) -> Result<(Context<'a>, cloud::Profile), Response> {
    let context = controls::admitted(app, headers, &runtime.binding).await?;
    authority(&context, runtime)?;
    if !operator::can_operate(&context) {
        return Err(refused(SessionError::Forbidden));
    }
    let (_, outcome) = operator::catalog(&context, headers, &runtime.project).await?;
    let Outcome::CloudCatalog { catalog } = outcome else {
        return Err(refused(SessionError::Conflict));
    };
    let profile = catalog
        .profiles
        .into_iter()
        .find(|profile| matches_profile(runtime, profile))
        .ok_or_else(|| refused(SessionError::Conflict))?;
    if profile.availability != "configured" {
        return Err(refused(SessionError::Unavailable));
    }
    Ok((context, profile))
}

/// Fresh source and profile pins are required before staging another command.
pub(crate) async fn validate(
    app: &App,
    headers: &HeaderMap,
    runtime: &RuntimeSelection,
) -> Result<(), Response> {
    bounded(async { chosen(app, headers, runtime).await.map(|_| ()) }).await
}

/// Retained text remains readable under current authority despite source drift.
pub(crate) async fn authorize(
    app: &App,
    headers: &HeaderMap,
    runtime: &RuntimeSelection,
) -> Result<(), Response> {
    bounded(async {
        let context = controls::admitted(app, headers, &runtime.binding).await?;
        authority(&context, runtime)?;
        context.probe(headers).await?;
        let operation = Operation::CloudProjects {
            workspace: runtime.workspace.clone(),
        };
        let outcome = context
            .binding
            .read(&context.viewer, operation.clone())
            .await
            .map_err(refused)?;
        if outcome.validate().is_err() || !outcome.answers(&operation) {
            return Err(refused(SessionError::Conflict));
        }
        let Outcome::CloudProjects { projects } = outcome else {
            return Err(refused(SessionError::Conflict));
        };
        if !projects.projects.contains(&runtime.project) {
            return Err(refused(SessionError::Forbidden));
        }
        context.current(headers).await
    })
    .await
}

fn source_compatible(selected: &Selection, profile: &cloud::Profile) -> bool {
    selected.repository.as_ref().is_none_or(|repository| {
        profile
            .repository
            .as_ref()
            .is_some_and(|label| label.eq_ignore_ascii_case(&repository.repository))
            && profile
                .branch
                .as_ref()
                .is_none_or(|label| label == &repository.branch)
            && profile.source_revision == repository.revision
    })
}

fn request_identity(owner: &str, request: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(b"openagents.web.chat.cloud-request.v1\0");
    for value in [owner, request] {
        hash.update((value.len() as u64).to_be_bytes());
        hash.update(value.as_bytes());
    }
    format!("{:x}", hash.finalize())
}

fn scope_matches(runtime: &RuntimeSelection, scope: &cloud::Scope) -> bool {
    scope.workspace == runtime.workspace
        && scope.project == runtime.project
        && scope.profile == runtime.profile
        && scope.profile_revision == runtime.profile_revision
        && scope.source_digest == runtime.source_digest
}

fn same_command(
    action: &Operation,
    runtime: &RuntimeSelection,
    prompt: &str,
    previous_job: Option<&str>,
) -> bool {
    match action {
        Operation::CloudSubmit { intent } if previous_job.is_none() => {
            intent.workspace == runtime.workspace
                && intent.project == runtime.project
                && intent.profile == runtime.profile
                && intent.profile_revision == runtime.profile_revision
                && intent.source_digest == runtime.source_digest
                && intent.timeout_seconds == runtime.max_timeout_seconds
                && intent.prompt == prompt
        }
        Operation::CloudContinue { intent } => {
            scope_matches(runtime, &intent.scope)
                && previous_job == Some(intent.scope.job.as_str())
                && intent.prompt == prompt
        }
        _ => false,
    }
}

/// Prepare an exact reviewed native packet. This function never confirms it.
pub(crate) async fn stage(
    app: &App,
    headers: &HeaderMap,
    owner: &str,
    chat_request_id: &str,
    selected: &Selection,
    prompt: &str,
    previous: Option<&CloudRequest>,
) -> Result<CloudRequest, Response> {
    bounded(async {
        selected
            .validate()
            .map_err(|_| refused(SessionError::InvalidRequest))?;
        if [owner, chat_request_id]
            .iter()
            .any(|value| value.is_empty() || value.len() > 128 || value.contains('\0'))
        {
            return Err(refused(SessionError::InvalidRequest));
        }
        let runtime = selected
            .runtime
            .as_ref()
            .ok_or_else(|| refused(SessionError::InvalidRequest))?;
        let (context, profile) = chosen(app, headers, runtime).await?;
        if !source_compatible(selected, &profile) {
            return Err(refused(SessionError::Conflict));
        }
        let request = request_identity(owner, chat_request_id);
        let retained = CloudRequest {
            binding: runtime.binding.clone(),
            request: request.clone(),
        };
        let previous_scope = if let Some(previous) = previous {
            previous
                .validate()
                .map_err(|_| refused(SessionError::InvalidRequest))?;
            if previous.binding != runtime.binding || previous.request == request {
                return Err(refused(SessionError::Conflict));
            }
            let snapshot = context
                .book()?
                .lookup(&context.scope, &previous.request)
                .map_err(refused)?;
            let snapshot = controls::recover_request(&context, headers, snapshot).await?;
            let Some(Outcome::CloudAccepted { accepted }) = snapshot.outcome else {
                return Err(refused(SessionError::Conflict));
            };
            if !scope_matches(runtime, &accepted.scope) {
                return Err(refused(SessionError::Conflict));
            }
            Some(accepted.scope)
        } else {
            None
        };
        // Lost replies reuse the signed packet even if its job revision advanced.
        if let Ok(snapshot) = context.book()?.lookup(&context.scope, &request) {
            if same_command(
                &snapshot.action,
                runtime,
                prompt,
                previous_scope.as_ref().map(|scope| scope.job.as_str()),
            ) {
                context.current(headers).await?;
                return Ok(retained);
            }
            return Err(refused(SessionError::Conflict));
        }
        let operation = if let Some(scope) = previous_scope {
            let (_, outcome) =
                operator::job_read(&context, headers, &scope.project, &scope.job).await?;
            let Outcome::CloudRead { job } = outcome else {
                return Err(refused(SessionError::Conflict));
            };
            if !scope_matches(runtime, &job.scope)
                || job.scope.job != scope.job
                || job.executor != runtime.executor
                || job.model != runtime.model
                || job.continuation != "available"
            {
                return Err(refused(SessionError::Conflict));
            }
            Operation::CloudContinue {
                intent: cloud::Continue {
                    scope: job.scope,
                    prompt: prompt.into(),
                },
            }
        } else {
            Operation::CloudSubmit {
                intent: cloud::Submit {
                    workspace: runtime.workspace.clone(),
                    project: runtime.project.clone(),
                    profile: runtime.profile.clone(),
                    profile_revision: runtime.profile_revision.clone(),
                    source_digest: runtime.source_digest.clone(),
                    prompt: prompt.into(),
                    timeout_seconds: runtime.max_timeout_seconds,
                },
            }
        };
        let response = controls::staged(&context, headers, &request, operation).await;
        if response.status() != StatusCode::SEE_OTHER {
            return Err(response);
        }
        Ok(retained)
    })
    .await
}

/// Mount the actual native review or job page under a retained chat alias.
pub(crate) async fn view(app: &App, headers: &HeaderMap, request: &CloudRequest) -> Response {
    let result = bounded(async {
        request
            .validate()
            .map_err(|_| refused(SessionError::InvalidRequest))?;
        let context = controls::admitted(app, headers, &request.binding).await?;
        context.enrolled()?;
        context.probe(headers).await?;
        let snapshot = context
            .book()?
            .lookup(&context.scope, &request.request)
            .map_err(refused)?;
        if !matches!(
            snapshot.action,
            Operation::CloudSubmit { .. } | Operation::CloudContinue { .. }
        ) {
            return Err(refused(SessionError::Forbidden));
        }
        let snapshot = controls::recover_request(&context, headers, snapshot).await?;
        let page = if let Some(Outcome::CloudAccepted { accepted }) = snapshot.outcome {
            operator::job(
                State(app.clone()),
                headers.clone(),
                Path((
                    request.binding.clone(),
                    accepted.scope.project,
                    accepted.scope.job,
                )),
            )
            .await
        } else {
            controls::request(
                State(app.clone()),
                headers.clone(),
                Path((request.binding.clone(), request.request.clone())),
            )
            .await
        };
        Ok(page)
    })
    .await;
    result.unwrap_or_else(|response| response)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> cloud::Profile {
        cloud::Profile {
            name: "fixture".into(),
            revision: format!("sha256:{}", "a".repeat(64)),
            source_revision: "b".repeat(40),
            source_digest: format!("sha256:{}", "c".repeat(64)),
            repository: Some("OpenAgentsInc/openagents".into()),
            branch: Some("main".into()),
            template: Some("oa-project-fixture-v1".into()),
            size: "small".into(),
            placement: "boat".into(),
            pool: "fixture-pool".into(),
            mode: "coder".into(),
            executor: "codex".into(),
            model: Some("synthetic".into()),
            credential_names: Vec::new(),
            max_timeout_seconds: 600,
            availability: "configured".into(),
        }
    }

    fn runtime() -> RuntimeSelection {
        let profile = profile();
        RuntimeSelection {
            binding: "resident".into(),
            account: "alice".into(),
            workspace: "checkout".into(),
            members_epoch: 1,
            project: "fixture".into(),
            profile: profile.name,
            profile_revision: profile.revision,
            source_revision: profile.source_revision,
            source_digest: profile.source_digest,
            placement: profile.placement,
            executor: profile.executor,
            model: profile.model,
            max_timeout_seconds: profile.max_timeout_seconds,
        }
    }

    #[test]
    fn native_request_identity_is_stable_and_separates_owners_and_messages() {
        let id = request_identity("owner", "message");
        assert_eq!(id, request_identity("owner", "message"));
        assert_eq!(id.len(), 64);
        assert!(
            id.bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        );
        assert_ne!(id, request_identity("another", "message"));
        assert_ne!(id, request_identity("owner", "another"));
        assert_ne!(request_identity("ab", "c"), request_identity("a", "bc"));
    }

    #[test]
    fn selected_source_and_executor_require_exact_native_pins() {
        let runtime = runtime();
        let mut profile = profile();
        assert!(matches_profile(&runtime, &profile));
        profile.executor = "other".into();
        assert!(!matches_profile(&runtime, &profile));
        profile = self::profile();
        let selected = Selection {
            revision: 1,
            repository: Some(crate::chat_store::RepositorySource {
                repository: "OpenAgentsInc/openagents".into(),
                branch: "main".into(),
                revision: runtime.source_revision.clone(),
            }),
            runtime: Some(runtime),
        };
        assert!(source_compatible(&selected, &profile));
        profile.branch = Some("other".into());
        assert!(!source_compatible(&selected, &profile));
        profile.branch = Some("main".into());
        profile.source_revision = "d".repeat(40);
        assert!(!source_compatible(&selected, &profile));
        profile = self::profile();
        profile.branch = None;
        assert!(source_compatible(&selected, &profile));
        profile.repository = Some("openagentsinc/OpenAgents".into());
        assert!(source_compatible(&selected, &profile));
        profile.repository = None;
        assert!(!source_compatible(&selected, &profile));
    }

    #[test]
    fn duplicate_staging_cannot_change_prompt_source_or_timeout() {
        let mut runtime = runtime();
        let action = Operation::CloudSubmit {
            intent: cloud::Submit {
                workspace: runtime.workspace.clone(),
                project: runtime.project.clone(),
                profile: runtime.profile.clone(),
                profile_revision: runtime.profile_revision.clone(),
                source_digest: runtime.source_digest.clone(),
                prompt: "Inspect this source".into(),
                timeout_seconds: runtime.max_timeout_seconds,
            },
        };
        assert!(same_command(&action, &runtime, "Inspect this source", None));
        assert!(!same_command(&action, &runtime, "Change this source", None));
        assert!(!same_command(
            &action,
            &runtime,
            "Inspect this source",
            Some("other")
        ));
        runtime.max_timeout_seconds -= 1;
        assert!(!same_command(
            &action,
            &runtime,
            "Inspect this source",
            None
        ));
    }
}
