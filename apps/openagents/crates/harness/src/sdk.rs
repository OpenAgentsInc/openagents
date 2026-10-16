//! Operator-installed SDK runners. Definitions cannot choose a command.
use crate::{Harness, HarnessError, RunControls};
use async_trait::async_trait;
use futures::{StreamExt, stream::BoxStream};
use openagents_sdk::agents::runner::{self, Event, Turn};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use zeron_proto::{
    AgentEvent, DoneStatus, HarnessId, Model, ReasoningLevel, RunRequest, SteeringMode,
};

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstalledRunner {
    label: String,
    executable: PathBuf,
    sha256: String,
    #[serde(default)]
    args: Vec<String>,
}
pub struct SdkHarness {
    runners: BTreeMap<String, InstalledRunner>,
}
impl SdkHarness {
    pub fn load(path: &Path) -> Result<Self, HarnessError> {
        let bytes = std::fs::read(path)?;
        if bytes.len() > 64 * 1024 {
            return Err(HarnessError::Protocol(
                "The runner list is too large.".into(),
            ));
        }
        let runners: BTreeMap<String, InstalledRunner> =
            serde_json::from_slice(&bytes).map_err(|e| HarnessError::Protocol(e.to_string()))?;
        if runners.is_empty()
            || runners.len() > 32
            || runners.iter().any(|(id, r)| {
                id.is_empty()
                    || id.len() > 128
                    || !id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
                    || r.label.trim().is_empty()
                    || r.label.len() > 80
                    || !r.executable.is_absolute()
                    || !r.executable.is_file()
                    || !openagents_sdk::agents::valid_id(&r.sha256)
                    || executable_digest(&r.executable).ok().as_deref() != Some(r.sha256.as_str())
                    || r.args.len() > 32
                    || r.args.iter().any(|a| a.len() > 4096 || a.contains('\0'))
            })
        {
            return Err(HarnessError::Protocol(
                "Check the installed runner list.".into(),
            ));
        }
        Ok(Self { runners })
    }
}
pub fn executable_digest(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 16384];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn done(status: DoneStatus, result: Option<String>, error: Option<String>) -> AgentEvent {
    AgentEvent::Done {
        status,
        result,
        error,
        session_id: None,
    }
}
#[async_trait]
impl Harness for SdkHarness {
    fn id(&self) -> HarnessId {
        HarnessId::Sdk
    }
    fn display_name(&self) -> &str {
        "SDK runners"
    }
    fn supports_steering(&self) -> bool {
        false
    }
    fn steering_mode(&self) -> SteeringMode {
        SteeringMode::TurnBoundary
    }
    fn reasoning_levels(&self) -> &[ReasoningLevel] {
        &[]
    }
    async fn skills(
        &self,
        _: &Path,
    ) -> Result<Option<Vec<zeron_proto::invocation::Skill>>, HarnessError> {
        Ok(None)
    }
    async fn models(&self) -> Result<Vec<Model>, HarnessError> {
        Ok(self
            .runners
            .iter()
            .map(|(id, r)| Model {
                id: id.clone(),
                label: r.label.clone(),
                description: None,
                reasoning_levels: vec![],
                options: vec![],
            })
            .collect())
    }
    async fn run(
        &self,
        request: RunRequest,
        controls: RunControls,
    ) -> Result<BoxStream<'static, Result<AgentEvent, HarnessError>>, HarnessError> {
        let value = request
            .model_options
            .get("openagentsAgentSpec")
            .cloned()
            .ok_or_else(|| HarnessError::Protocol("Choose a saved SDK agent.".into()))?;
        let turn = Turn {
            schema: runner::VERSION.into(),
            assignment: request
                .model_options
                .get("openagentsAssignment")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .into(),
            conversation: request
                .model_options
                .get("openagentsConversation")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .into(),
            agent: serde_json::from_value(value)
                .map_err(|e| HarnessError::Protocol(e.to_string()))?,
            task: request.prompt.clone(),
            cwd: request.cwd.clone(),
        };
        turn.validate()
            .map_err(|s| HarnessError::Protocol(s.into()))?;
        if turn.agent.model != request.model
            || turn.agent.access != openagents_sdk::agents::Access::HostDefault
        {
            return Err(HarnessError::Protocol(
                "This runner uses this computer's access settings.".into(),
            ));
        }
        let runner = self
            .runners
            .get(request.model.as_deref().unwrap_or_default())
            .ok_or_else(|| {
                HarnessError::Protocol("Install this SDK runner on this computer.".into())
            })?;
        if executable_digest(&runner.executable)? != runner.sha256 {
            return Err(HarnessError::Protocol(
                "This runner changed. Ask the computer's owner to approve its new version.".into(),
            ));
        }
        let mut cmd = tokio::process::Command::new(&runner.executable);
        cmd.args(&runner.args)
            .current_dir(&request.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(unix)]
        cmd.process_group(0);
        let mut child = cmd.spawn()?;
        let mut input = child.stdin.take().unwrap();
        let mut bytes =
            serde_json::to_vec(&turn).map_err(|e| HarnessError::Protocol(e.to_string()))?;
        bytes.push(b'\n');
        if bytes.len() > runner::MAX_FRAME {
            let _ = child.kill().await;
            return Err(HarnessError::Protocol(
                "The runner request is too large.".into(),
            ));
        }
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = tokio::sync::mpsc::channel(32);
        tokio::spawn(async move {
            let _lease = controls.execution_lease;
            let run = async {
                let started = AgentEvent::SessionStarted {
                    harness: HarnessId::Sdk,
                    model: request.model.clone().unwrap_or_default(),
                    tools: vec![],
                    cwd: request.cwd.clone(),
                    session_id: turn.conversation.clone(),
                    assistant_message_id: uuid::Uuid::new_v4().to_string(),
                };
                tx.send(Ok(started))
                    .await
                    .map_err(|_| HarnessError::Protocol("The conversation was closed.".into()))?;
                input.write_all(&bytes).await?;
                input.shutdown().await?;
                drop(input);
                let mut reader = BufReader::new(stdout);
                let mut total = 0;
                let mut terminal = None;
                let mut streamed = false;
                loop {
                    let mut line = vec![];
                    let read = (&mut reader)
                        .take((runner::MAX_FRAME + 1) as u64)
                        .read_until(b'\n', &mut line)
                        .await?;
                    if read == 0 {
                        break;
                    }
                    total += read;
                    if read > runner::MAX_FRAME || total > runner::MAX_OUTPUT || terminal.is_some()
                    {
                        return Err(HarnessError::Protocol(
                            "The runner returned an invalid response.".into(),
                        ));
                    }
                    let event: Event = serde_json::from_slice(&line).map_err(|_| {
                        HarnessError::Protocol("The runner returned an invalid response.".into())
                    })?;
                    match event {
                        Event::Text { text } => {
                            streamed = true;
                            if tx.send(Ok(AgentEvent::TextDelta { text })).await.is_err() {
                                return Err(HarnessError::Protocol(
                                    "The conversation was closed.".into(),
                                ));
                            }
                        }
                        Event::Completed { result } => {
                            terminal = Some(done(DoneStatus::Completed, Some(result), None))
                        }
                        Event::Failed { message } => {
                            terminal = Some(done(DoneStatus::Errored, None, Some(message)))
                        }
                    }
                }
                if !child.wait().await?.success() {
                    return Err(HarnessError::Protocol(
                        "The runner stopped before finishing.".into(),
                    ));
                }
                if !streamed {
                    if let Some(AgentEvent::Done {
                        result: Some(text), ..
                    }) = &terminal
                    {
                        let _ = tx
                            .send(Ok(AgentEvent::TextDelta { text: text.clone() }))
                            .await;
                    }
                }
                terminal.ok_or_else(|| {
                    HarnessError::Protocol("The runner did not return a result.".into())
                })
            };
            let outcome = tokio::select! {
                r=tokio::time::timeout(std::time::Duration::from_secs(300),run) => match r { Ok(Ok(done))=>done, Ok(Err(e))=>done(DoneStatus::Errored,None,Some(e.to_string())), Err(_)=>done(DoneStatus::Errored,None,Some("The runner took too long.".into())) },
                _=controls.interrupt.cancelled()=>done(DoneStatus::Interrupted,None,None),
                _=controls.turn.stop_requested()=>done(DoneStatus::Interrupted,None,None),
                _=tx.closed()=>done(DoneStatus::Interrupted,None,None),
            };
            #[cfg(unix)]
            if let Some(pid) = child.id() {
                let mut pids = crate::process::descendants(pid).await;
                pids.push(pid as i32);
                crate::process::terminate_tree(&pids, std::time::Duration::from_millis(100)).await;
            }
            let _ = child.kill().await;
            let _ = child.wait().await;
            let _ = tx.send(Ok(outcome)).await;
        });
        Ok(futures::stream::unfold(rx, |mut rx| async move {
            rx.recv().await.map(|event| (event, rx))
        })
        .boxed())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use serde_json::json;
    fn controls() -> RunControls {
        let (_tx, rx) = tokio::sync::mpsc::channel(1);
        RunControls {
            realtime: None,
            execution_lease: None,
            request_input: Box::new(|_| tokio::sync::oneshot::channel().1),
            steering: rx,
            interrupt: Default::default(),
            turn: Default::default(),
        }
    }
    fn request() -> RunRequest {
        serde_json::from_value(json!({"prompt":"Task","cwd":"/","model":"fixture","reasoning":null,"resume":null,"sandbox":"danger-full-access","modelOptions":{"openagentsAssignment":"a".repeat(64),"openagentsConversation":"chat","openagentsAgentSpec":{"name":"Fixture","description":"","instructions":"Find defects","runtime":"sdk","model":"fixture","access":"host_default"}}})).unwrap()
    }
    fn install(dir: &Path, script: &str) -> SdkHarness {
        let path = dir.join("runners.json");
        std::fs::write(&path,serde_json::to_vec(&json!({"fixture":{"label":"Fixture","executable":"/bin/sh","sha256":executable_digest(Path::new("/bin/sh")).unwrap(),"args":["-c",script]}})).unwrap()).unwrap();
        SdkHarness::load(&path).unwrap()
    }
    #[tokio::test]
    async fn typed_result_and_malformed_or_missing_result() {
        let dir = tempfile::tempdir().unwrap();
        let harness = install(
            dir.path(),
            "cat >/dev/null; printf '%s\\n' '{\"type\":\"completed\",\"result\":\"Reviewed\"}'",
        );
        let events: Vec<_> = harness
            .run(request(), controls())
            .await
            .unwrap()
            .collect()
            .await;
        assert!(
            matches!(events.last().unwrap().as_ref().unwrap(),AgentEvent::Done{status:DoneStatus::Completed,result:Some(s),..} if s=="Reviewed")
        );
        for script in [
            "cat >/dev/null",
            "cat >/dev/null; echo invalid",
            "cat >/dev/null; printf '%s\\n' '{\"type\":\"completed\",\"result\":\"bad exit\"}'; exit 1",
        ] {
            let harness = install(dir.path(), script);
            let events: Vec<_> = harness
                .run(request(), controls())
                .await
                .unwrap()
                .collect()
                .await;
            assert!(matches!(
                events.last().unwrap().as_ref().unwrap(),
                AgentEvent::Done {
                    status: DoneStatus::Errored,
                    ..
                }
            ));
        }
    }
    #[tokio::test]
    async fn cancellation_reaps_a_long_task_and_changed_binary_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let harness = install(dir.path(), "cat >/dev/null; sleep 60");
        let control = controls();
        let cancel = control.interrupt.clone();
        let mut stream = harness.run(request(), control).await.unwrap();
        cancel.cancel();
        let events: Vec<_> = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            stream.collect::<Vec<_>>().await
        })
        .await
        .unwrap();
        assert!(matches!(
            events.last().unwrap().as_ref().unwrap(),
            AgentEvent::Done {
                status: DoneStatus::Interrupted,
                ..
            }
        ));
        let path = dir.path().join("bad.json");
        std::fs::write(&path,serde_json::to_vec(&json!({"fixture":{"label":"Fixture","executable":"/bin/sh","sha256":"0".repeat(64),"args":[]}})).unwrap()).unwrap();
        assert!(SdkHarness::load(&path).is_err());
    }
}
