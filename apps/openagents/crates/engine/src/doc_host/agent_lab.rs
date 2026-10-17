//! The Lab job provider prepares isolated inputs; the normal host still executes every turn.
use super::*;
use crate::registry::HarnessRegistry;
use openagents_account::{
    Lease,
    openagents::{
        self,
        agent_lab::*,
        agents::assignments::{Assignment, AssignmentAction, AssignmentState},
    },
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};
fn error(message: &str) -> EngineError {
    EngineError::Other(message.into())
}
#[derive(Clone, Serialize, Deserialize)]
struct Marker {
    run: String,
    assignment: String,
    agent: String,
    revision: u64,
    task_sha256: String,
    evidence: Evidence,
    deadline_ms: u64,
}
fn hash(value: &impl Serialize) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    format!("{:x}", Sha256::digest(bytes))
}
fn location(root: &Path, assignment: &str) -> PathBuf {
    root.join("trials").join(assignment)
}
pub(super) fn trial_cwd(root: &Path, a: &Assignment) -> Result<Option<(String, u64)>, EngineError> {
    let dir = location(root, &a.id);
    let path = dir.join("ready.json");
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(path)?;
    if bytes.len() > 32768 {
        return Err(error("The test workspace could not be checked."));
    }
    let m: Marker = serde_json::from_slice(&bytes)
        .map_err(|_| error("The test workspace could not be checked."))?;
    if m.assignment != a.id
        || m.agent != a.spec.agent
        || m.revision != a.spec.agent_revision
        || m.task_sha256 != hash(&a.spec.task)
        || m.evidence.project != a.spec.project.clone().unwrap_or_default()
    {
        return Err(error("This assignment does not match the saved test."));
    }
    let work = dir.join("work");
    if !work.is_dir() {
        return Err(error("The test workspace is no longer available."));
    }
    Ok(Some((work.to_string_lossy().into(), m.deadline_ms)))
}
impl DocHost {
    pub(super) async fn agent_lab_tick(
        &self,
        lease: &Lease,
        root: &Path,
        registry: &HarnessRegistry,
    ) -> Result<(), EngineError> {
        let labs = lease
            .client()
            .agent_lab(lease.session())
            .map_err(EngineError::AgentService)?;
        let ws = self
            .workspace()
            .ok_or_else(|| error("This computer is not ready."))?;
        let name = ws
            .read_devices()?
            .into_iter()
            .find(|d| d.id == self.inner.config.device_id)
            .map(|d| d.name)
            .unwrap_or_else(|| "My computer".into());
        let projects = ws
            .read_spaces()?
            .into_iter()
            .filter(|p| p.device_id == self.inner.config.device_id)
            .take(256)
            .map(|p| Project {
                id: p.id.clone(),
                name: p.display_name().to_string(),
            })
            .collect();
        let runtimes = registry
            .descriptors()
            .into_iter()
            .filter(|d| d.installed && crate::registry::descriptor_enabled(d))
            .map(|d| {
                serde_json::to_value(d.id)
                    .unwrap_or_default()
                    .as_str()
                    .unwrap_or_default()
                    .to_owned()
            })
            .collect();
        let runners = registry
            .discover_models(HarnessId::Sdk)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|m| m.id)
            .collect();
        labs.heartbeat(
            &Host {
                id: self.inner.config.device_id.clone(),
                name,
                projects,
                runtimes,
                runners,
                tools: bindings(root)
                    .into_iter()
                    .filter(|b| {
                        b.validate(&lease.session().account().id, &b.project) && loopback(&b.origin)
                    })
                    .map(|b| ToolOffer {
                        id: b.id.clone(),
                        name: b.name.clone(),
                        project: b.project.clone(),
                        sha256: b.digest(),
                    })
                    .collect(),
                seen_ms: 0,
            },
            &lease.cancel,
        )
        .await
        .map_err(EngineError::AgentService)?;
        let mut after = None;
        loop {
            let page = labs
                .runs(
                    Some(&self.inner.config.device_id),
                    after.as_deref(),
                    &lease.cancel,
                )
                .await
                .map_err(EngineError::AgentService)?;
            for run in page.data {
                if !lease.is_current() {
                    return Ok(());
                }
                let request = labs.bind(HostUpdate {
                    request_id: openagents::agents::request_id()
                        .map_err(EngineError::AgentService)?,
                    host: self.inner.config.device_id.clone(),
                    action: HostAction::Advance,
                });
                let run = labs
                    .host_update(&run.id, &request, &lease.cancel)
                    .await
                    .map_err(EngineError::AgentService)?;
                if run.complete() {
                    let agents = lease
                        .client()
                        .agents(lease.session())
                        .map_err(EngineError::AgentService)?;
                    for t in &run.trials {
                        if t.outcome == Outcome::Unknown {
                            continue;
                        }
                        if let Some(chat) = &t.chat {
                            if let Some(ws) = self.workspace() {
                                let _ = ws.set_chat_archived(chat, true);
                            }
                        }
                        if let Ok(a) = agents.assignment(&t.assignment, &lease.cancel).await {
                            if !a.archived
                                && !matches!(
                                    a.state,
                                    AssignmentState::Queued | AssignmentState::Running
                                )
                            {
                                let req = agents
                                    .command_request(AssignmentAction::Archive)
                                    .map_err(EngineError::AgentService)?;
                                let _ = agents.command(&a.id, &req, &lease.cancel).await;
                            }
                        }
                        if run.archived && t.outcome != Outcome::Unknown {
                            let folder = location(root, &t.assignment);
                            if folder.exists() {
                                std::fs::remove_dir_all(folder)?;
                            }
                        }
                    }
                    if run.archived {
                        if let Some(candidate) = run.candidates.first() {
                            let request = agents
                                .delete_request(candidate.revision)
                                .map_err(EngineError::AgentService)?;
                            let _ = agents.delete(&candidate.id, &request, &lease.cancel).await;
                        }
                    }
                    let req = labs.bind(HostUpdate {
                        request_id: openagents::agents::request_id()
                            .map_err(EngineError::AgentService)?,
                        host: self.inner.config.device_id.clone(),
                        action: HostAction::Settled,
                    });
                    labs.host_update(&run.id, &req, &lease.cancel)
                        .await
                        .map_err(EngineError::AgentService)?;
                    continue;
                }
                let Some(trial) = run.trials.iter().find(|t| !t.outcome.finished()) else {
                    continue;
                };
                if trial.outcome != Outcome::Pending || trial.evidence.is_some() {
                    continue;
                }
                let candidate = &run.candidates[trial.candidate];
                let harness: HarnessId =
                    match serde_json::from_value(candidate.spec.runtime.clone().into()) {
                        Ok(h) => h,
                        Err(_) => {
                            self.lab_unavailable(
                                lease,
                                &run,
                                trial.index,
                                "This runtime is not supported on this computer.",
                            )
                            .await?;
                            continue;
                        }
                    };
                if let Err(e) =
                    super::custom_agents::admit_runtime(registry, harness, &candidate.spec).await
                {
                    self.lab_unavailable(lease, &run, trial.index, &e.to_string())
                        .await?;
                    continue;
                }
                let ws = self
                    .workspace()
                    .ok_or_else(|| error("This computer is not ready."))?;
                let Some(project) = ws.space(&run.experiment.project)? else {
                    self.lab_unavailable(
                        lease,
                        &run,
                        trial.index,
                        "The source project is no longer available.",
                    )
                    .await?;
                    continue;
                };
                if project.device_id != self.inner.config.device_id {
                    self.lab_unavailable(
                        lease,
                        &run,
                        trial.index,
                        "Choose a project on this computer.",
                    )
                    .await?;
                    continue;
                }
                let source = PathBuf::from(&project.path);
                let folder = location(root, &trial.assignment);
                let context =
                    match context(root, &run.experiment, lease.session().account().id.as_str())
                        .await
                    {
                        Ok(c) => c,
                        Err(e) => {
                            self.lab_unavailable(lease, &run, trial.index, &e.to_string())
                                .await?;
                            continue;
                        }
                    };
                let task = run.experiment.task(trial.case);
                let mut full_task = task.clone();
                if !context.is_empty() {
                    full_task = format!("{context}\n\n{task}");
                }
                // Tool context becomes explicit task data, not a tool grant or an executable command.
                let assignment = trial.assignment.clone();
                let run_id = run.id.clone();
                let agent = candidate.id.clone();
                let revision = candidate.revision;
                let project_id = run.experiment.project.clone();
                let deadline_ms = run.deadline_ms;
                let Some(version) = registry
                    .resolve(harness)
                    .ok()
                    .and_then(|h| h.binding_fingerprint(candidate.spec.model.as_deref()))
                else {
                    self.lab_unavailable(
                        lease,
                        &run,
                        trial.index,
                        "The installed runtime could not be checked.",
                    )
                    .await?;
                    continue;
                };
                let prepared=tokio::task::spawn_blocking(move||->Result<Marker,EngineError>{
     let path=folder.join("ready.json");if path.exists(){return serde_json::from_slice(&std::fs::read(path)?).map_err(|_|error("The saved test could not be opened."))}
     std::fs::create_dir_all(&folder)?;
     #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(&folder,std::fs::Permissions::from_mode(0o700))?;}
     let work=folder.join("work");if work.exists(){std::fs::remove_dir_all(&work)?}
     let source_sha256=copy_project(&source,&work).map_err(|_|error("Use a project under 64 MB with no links or restricted training data."))?;
     let evidence=Evidence{project:project_id,source_sha256,context_sha256:hash(&context),runtime_version:version,context:(!context.is_empty()).then_some(context)};
     let marker=Marker{run:run_id,assignment,agent,revision,task_sha256:hash(&full_task),evidence,deadline_ms};
     let bytes=serde_json::to_vec(&marker).map_err(|_|error("Couldn't save the test inputs."))?;
     let pending=folder.join("ready.new");std::fs::write(&pending,bytes)?;std::fs::rename(pending,path)?;Ok(marker)
    }).await.map_err(|_|error("Couldn't prepare the test workspace."))?;
                match prepared {
                    Ok(marker) => {
                        if !lease.is_current() {
                            return Ok(());
                        }
                        let req = labs.bind(HostUpdate {
                            request_id: openagents::agents::request_id()
                                .map_err(EngineError::AgentService)?,
                            host: self.inner.config.device_id.clone(),
                            action: HostAction::Ready {
                                trial: trial.index,
                                evidence: marker.evidence,
                            },
                        });
                        if let Err(e) = labs.host_update(&run.id, &req, &lease.cancel).await {
                            if matches!(e, openagents::Error::Refused { .. }) {
                                self.lab_unavailable(
                                    lease,
                                    &run,
                                    trial.index,
                                    "The project or references changed. Start a new experiment.",
                                )
                                .await?;
                            } else {
                                return Err(EngineError::AgentService(e));
                            }
                        }
                    }
                    Err(e) => {
                        self.lab_unavailable(lease, &run, trial.index, &e.to_string())
                            .await?
                    }
                }
            }
            after = page.next;
            if after.is_none() {
                break;
            }
        }
        Ok(())
    }
    async fn lab_unavailable(
        &self,
        lease: &Lease,
        run: &Run,
        trial: usize,
        reason: &str,
    ) -> Result<(), EngineError> {
        let labs = lease
            .client()
            .agent_lab(lease.session())
            .map_err(EngineError::AgentService)?;
        let req = labs.bind(HostUpdate {
            request_id: openagents::agents::request_id().map_err(EngineError::AgentService)?,
            host: self.inner.config.device_id.clone(),
            action: HostAction::Unavailable {
                trial,
                reason: reason.chars().take(512).collect(),
            },
        });
        labs.host_update(&run.id, &req, &lease.cancel)
            .await
            .map_err(EngineError::AgentService)?;
        Ok(())
    }
}
/// Copy regular project files only, with deterministic evidence and finite traversal.
fn copy_project(source: &Path, target: &Path) -> io::Result<String> {
    let source = source.canonicalize()?;
    if target.starts_with(&source) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    if !source.is_dir() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut entries = BTreeMap::new();
    let mut bytes = 0u64;
    fn walk(
        root: &Path,
        rel: &Path,
        dest: &Path,
        entries: &mut BTreeMap<String, String>,
        bytes: &mut u64,
        depth: usize,
    ) -> io::Result<()> {
        if depth > 32 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        std::fs::create_dir_all(dest.join(rel))?;
        for entry in std::fs::read_dir(root.join(rel))? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_str().ok_or(io::ErrorKind::InvalidData)?;
            if name == ".no-training" {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
            if [".git", "target", "node_modules", ".ssh", ".aws"].contains(&name)
                || name.starts_with(".env")
                || name.ends_with(".pem")
                || name.ends_with(".key")
            {
                continue;
            }
            let next = rel.join(name);
            let meta = std::fs::symlink_metadata(entry.path())?;
            if meta.file_type().is_symlink() {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
            if meta.is_dir() {
                walk(root, &next, dest, entries, bytes, depth + 1)?;
            } else if meta.is_file() {
                *bytes += meta.len();
                if *bytes > 64 * 1024 * 1024 || entries.len() >= 5000 {
                    return Err(io::ErrorKind::InvalidData.into());
                }
                let data = std::fs::read(entry.path())?;
                if data.len() as u64 != meta.len() {
                    return Err(io::ErrorKind::InvalidData.into());
                }
                let mode = meta.permissions();
                std::fs::write(dest.join(&next), &data)?;
                std::fs::set_permissions(dest.join(&next), mode)?;
                entries.insert(
                    next.to_string_lossy().into(),
                    format!("{:x}", Sha256::digest(data)),
                );
            } else {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
        }
        Ok(())
    }
    walk(&source, Path::new(""), target, &mut entries, &mut bytes, 0)?;
    Ok(hash(&entries))
}
fn bindings(root: &Path) -> Vec<openagents::agent_lab::components::ToolBinding> {
    let Ok(bytes) = std::fs::read(root.join("tools.json")) else {
        return vec![];
    };
    if bytes.len() > 65536 {
        return vec![];
    }
    serde_json::from_slice::<Vec<openagents::agent_lab::components::ToolBinding>>(&bytes)
        .ok()
        .filter(|v| v.len() <= 32)
        .unwrap_or_default()
}
fn loopback(origin: &str) -> bool {
    reqwest::Url::parse(origin).is_ok_and(|u| {
        u.scheme() == "http"
            && matches!(u.host_str(), Some("127.0.0.1" | "[::1]"))
            && u.port().is_some()
            && u.path() == "/"
            && u.username().is_empty()
            && u.password().is_none()
            && u.query().is_none()
            && u.fragment().is_none()
    })
}
async fn context(root: &Path, e: &Experiment, account: &str) -> Result<String, EngineError> {
    let installed = bindings(root);
    let mut context = String::new();
    for component in &e.draft.components {
        if let Component::ToolContext { binding, sha256 } = component {
            let b = installed
                .iter()
                .find(|b| {
                    &b.id == binding
                        && b.digest() == *sha256
                        && b.validate(account, &e.project)
                        && loopback(&b.origin)
                })
                .ok_or_else(|| error("Remove the Lab reference before running this experiment."))?;
            // Legacy read tools cannot supply authoritative source policy. Local flags and
            // arbitrary response JSON cannot authorize model context; do not call the tool.
            let source = b
                .admit_model_context(account, &e.project, None)
                .map_err(|e| error(&e.message))?;
            context.push_str(&format!(
                "{} (read-only reference):\n{}\n",
                b.name, source.version.content
            ));
            if context.len() > 16384 {
                return Err(error("Use fewer reference documents."));
            }
        }
    }
    Ok(context)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trial_copy_isolated_and_stable() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("main.rs"), "original").unwrap();
        std::fs::write(src.path().join(".env"), "secret").unwrap();
        let hash1 = copy_project(src.path(), &dst.path().join("one")).unwrap();
        std::fs::write(dst.path().join("one/main.rs"), "changed").unwrap();
        let hash2 = copy_project(src.path(), &dst.path().join("two")).unwrap();
        assert_eq!(hash1, hash2);
        assert_eq!(
            std::fs::read_to_string(src.path().join("main.rs")).unwrap(),
            "original"
        );
        assert!(!dst.path().join("one/.env").exists());
    }
    #[test]
    fn restricted_inputs_refused() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join(".no-training"), "").unwrap();
        assert!(copy_project(src.path(), dst.path()).is_err());
    }
    #[tokio::test]
    async fn legacy_lab_references_are_denied_before_reading() {
        use axum::{Json, Router, routing::post};
        use openagents::agent_lab::components::{ReadTool, ToolBinding};
        use serde_json::json;
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}/", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route(
                    "/api/tool/{tool}",
                    post(move || {
                        let calls = observed.clone();
                        async move {
                            calls.fetch_add(1, Ordering::SeqCst);
                            Json(json!({"policy":{"training_allowed":true},"text":"untrusted"}))
                        }
                    }),
                ),
            )
            .await
            .unwrap()
        });
        let root = tempfile::tempdir().unwrap();
        let mut e = starter();
        e.project = "project".into();
        let mut installed = vec![];
        for (i, tool, args) in [
            (
                0,
                ReadTool::Video,
                json!({"document":"film","version":3,"render":false}),
            ),
            (1, ReadTool::Cad, json!({"doc":"part","version":1})),
            (
                2,
                ReadTool::Robot,
                json!({"document":"arm","version":"a".repeat(64),"frame":0}),
            ),
        ] {
            let b = ToolBinding {
                id: format!("reference-{i}"),
                name: format!("Reference {i}"),
                account: "alice".into(),
                project: "project".into(),
                origin: origin.clone(),
                tool,
                arguments: args,
                training_allowed: true,
                private_local: false,
            };
            e.draft.components.push(Component::ToolContext {
                binding: b.id.clone(),
                sha256: b.digest(),
            });
            installed.push(b);
        }
        std::fs::write(
            root.path().join("tools.json"),
            serde_json::to_vec(&installed).unwrap(),
        )
        .unwrap();
        for component in e.draft.components.clone() {
            e.draft.components = vec![component];
            assert_eq!(
                context(root.path(), &e, "alice")
                    .await
                    .unwrap_err()
                    .to_string(),
                "Remove the Lab reference before running this experiment."
            );
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(context(root.path(), &e, "bob").await.is_err());
        e.draft.components.clear();
        assert_eq!(context(root.path(), &e, "alice").await.unwrap(), "");
        assert!(!loopback("http://example.com/"));
        assert!(!loopback("http://127.0.0.1:8792/private"));
        server.abort();
    }
}
