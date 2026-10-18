//! Loopback account and saved agents for fake voice sessions.
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use openagents_account::openagents as sdk;
use openagents_engine::{
    EngineCore,
    auth::{Auth, AuthConfig},
};
use sdk::agents::{
    AgentDefinition, AgentSpec, Access,
    assignments::{Assignment, AssignmentSpec, AssignmentState},
    voice::{VoiceAgent, VoiceTarget},
};
use serde_json::json;
use std::sync::{Arc, Mutex};

pub struct VoiceFixture {
    pub choice: Arc<Mutex<VoiceAgent>>,
    pub targets: Vec<VoiceTarget>,
    pub release: Arc<tokio::sync::Semaphore>,
    server: tokio::task::JoinHandle<()>,
}
impl Drop for VoiceFixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}
#[derive(Clone)]
struct World {
    choice: Arc<Mutex<VoiceAgent>>,
    assignments: Vec<Assignment>,
}
impl VoiceFixture {
    pub async fn attach(
        core: &EngineCore,
        root: &std::path::Path,
        release: Arc<tokio::sync::Semaphore>,
    ) -> Self {
        let mut assignments = Vec::new();
        for (n, chat) in [(10, "voice-agent-chat"), (20, "fork-agent-chat")] {
            let definition = AgentDefinition {
                id: format!("{n:064x}"),
                revision: 1,
                deleted: false,
                spec: AgentSpec {
                    name: "A differently named helper".into(),
                    description: "Fixture fork".into(),
                    instructions: "Answer the owner.".into(),
                    runtime: "opencode".into(),
                    model: None,
                    access: Access::HostDefault,
                    fallbacks: vec![],
                    autonomy: None,
                },
            };
            let assignment = Assignment {
                id: format!("{:064x}", n + 1),
                revision: 1,
                spec: AssignmentSpec {
                    agent: definition.id.clone(),
                    agent_revision: 1,
                    host: core.device_id.clone(),
                    project: None,
                    task: "Fixture chat".into(),
                },
                definition,
                chat: chat.into(),
                state: AssignmentState::Completed,
                commands: vec![],
                result: None,
                archived: false,
                active_command: None,
                after_seq: 0,
            };
            let config =
                serde_json::from_value(json!({"harness":"opencode", "sandbox":"workspace-write",
                "modelOptions":{"openagentsAssignment":assignment.id}}))
                .unwrap();
            core.workspace
                .create_chat(
                    chat,
                    None,
                    Some(&core.device_id),
                    Some(config),
                    Some(root.display().to_string()),
                )
                .unwrap();
            core.workspace
                .rename_chat(chat, "An arbitrary chat title")
                .unwrap();
            assignments.push(assignment);
        }
        let targets: Vec<_> = assignments
            .iter()
            .map(|a| VoiceTarget {
                agent_id: a.spec.agent.clone(),
                chat_id: a.chat.clone(),
                host_id: a.spec.host.clone(),
            })
            .collect();
        let choice = Arc::new(Mutex::new(VoiceAgent {
            revision: 1,
            target: Some(targets[0].clone()),
        }));
        let world = World {
            choice: choice.clone(),
            assignments,
        };
        let router = Router::new()
            .route("/v1/account", get(|| async { Json(json!({"account":{"id":"voice-owner-fixture","label":"Fixture"},"workspaces":[]})) }))
            .route("/v1/voice-agent", get(|State(w): State<World>| async move { Json(w.choice.lock().unwrap().clone()) }))
            .route("/v1/agent-assignments", get(|State(w): State<World>| async move { Json(json!({"data":w.assignments,"next":null})) }))
            .route("/v1/agent-assignments/{id}", get(|State(w): State<World>, Path(id): Path<String>| async move {
                Json(w.assignments.iter().find(|a| a.id == id).cloned())
            }))
            .route("/v1/agent-definitions/{id}", get(|State(w): State<World>, Path(id): Path<String>| async move {
                Json(w.assignments.iter().find(|a| a.spec.agent == id).map(|a| a.definition.clone()))
            }))
            .with_state(world);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let client =
            sdk::Client::new(sdk::Deployment::new(&format!("{origin}/v1"), &origin).unwrap())
                .unwrap();
        let account = openagents_account::Controller::new(
            client,
            Arc::new(openagents_account::desktop::MemoryStore::default()),
            root.join("voice-account-marker"),
        );
        // This synthetic credential is accepted only by the loopback fixture.
        account
            .import_credential(format!("oa_sk_{}", "a".repeat(64)))
            .await;
        assert!(account.lease().is_some());
        core.set_auth(Auth::with_account(
            AuthConfig::new(origin, root.join("voice-auth")),
            account,
        ));
        Self {
            choice,
            targets,
            release,
            server,
        }
    }
}

/// A fake agent that stays busy across several short voice waits.
pub struct AnswerHarness(pub Arc<tokio::sync::Semaphore>);
#[async_trait::async_trait]
impl openagents_harness::Harness for AnswerHarness {
    fn id(&self) -> openagents_proto::HarnessId {
        openagents_proto::HarnessId::Opencode
    }
    fn display_name(&self) -> &str {
        "Fixture agent"
    }
    fn supports_steering(&self) -> bool {
        true
    }
    fn steering_mode(&self) -> openagents_proto::SteeringMode {
        openagents_proto::SteeringMode::StepBoundary
    }
    fn reasoning_levels(&self) -> &[openagents_proto::ReasoningLevel] {
        &[]
    }
    fn deterministic_turn_end(&self) -> bool {
        true
    }
    async fn models(
        &self,
    ) -> Result<Vec<openagents_proto::Model>, openagents_harness::HarnessError> {
        Ok(vec![])
    }
    async fn run(
        &self,
        request: openagents_proto::RunRequest,
        mut controls: openagents_harness::RunControls,
    ) -> Result<
        futures::stream::BoxStream<
            'static,
            Result<openagents_proto::AgentEvent, openagents_harness::HarnessError>,
        >,
        openagents_harness::HarnessError,
    > {
        use futures::StreamExt;
        let release = self.0.clone();
        let response = futures::stream::once(async move {
            use openagents_proto::AgentEvent;
            release.acquire_owned().await.unwrap().forget();
            let mut events = Vec::new();
            let mut text = "The full written answer stays in this chat.\n".repeat(80);
            text.push_str(&request.prompt);
            while let Ok(message) = controls.steering.try_recv() {
                text.push_str(&message.prompt);
                events.push(AgentEvent::Steered {
                    assistant_message_id: None,
                    next_assistant_message_id: None,
                });
            }
            events.push(AgentEvent::TextDelta { text });
            events.push(AgentEvent::Done {
                status: openagents_proto::DoneStatus::Completed,
                result: None,
                error: None,
                session_id: None,
            });
            events
        });
        Ok(response.flat_map(futures::stream::iter).map(Ok).boxed())
    }
}
