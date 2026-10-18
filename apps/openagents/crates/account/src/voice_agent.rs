//! One account-bound voice picker for desktop and mobile.
use crate::{Controller, Lease};
use openagents::agents::{
    AgentDefinition,
    assignments::Assignment,
    voice::{VoiceAgent, VoiceTarget},
};
use rust_native::{Activation, Axis, Element, Node, TextRole, View};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Action {
    Choose(Option<VoiceTarget>),
    Refresh,
}
#[derive(Clone)]
struct State {
    generation: u64,
    revision: u64,
    choice: VoiceAgent,
    definitions: Vec<AgentDefinition>,
    chats: Vec<Assignment>,
    busy: bool,
    notice: Option<String>,
}
pub struct Picker {
    controller: Arc<Controller>,
    instance: String,
    state: Mutex<State>,
    io: tokio::sync::Mutex<()>,
    updates: tokio::sync::watch::Sender<u64>,
}
impl Picker {
    pub fn new(controller: Arc<Controller>) -> Arc<Self> {
        let (updates, _) = tokio::sync::watch::channel(0);
        Arc::new(Self {
            controller: controller.clone(),
            instance: openagents::agents::request_id().unwrap_or_else(|_| "voice-agent".into()),
            state: Mutex::new(State {
                generation: controller.lease().map_or(0, |l| l.generation),
                revision: 1,
                choice: VoiceAgent::default(),
                definitions: vec![],
                chats: vec![],
                busy: false,
                notice: Some("Loading your agents…".into()),
            }),
            io: tokio::sync::Mutex::new(()),
            updates,
        })
    }
    pub fn watch(&self) -> tokio::sync::watch::Receiver<u64> {
        self.updates.subscribe()
    }
    fn lease(&self) -> Result<Lease, String> {
        self.controller
            .lease()
            .ok_or_else(|| "Sign in to choose a voice agent.".into())
    }
    fn publish(&self) {
        self.updates.send_modify(|r| *r = r.wrapping_add(1));
    }
    pub fn view(&self) -> View<Action> {
        let state = self.state.lock().unwrap();
        if self
            .controller
            .lease()
            .is_none_or(|l| l.generation != state.generation)
        {
            return view(
                &self.instance,
                &State {
                    definitions: vec![],
                    chats: vec![],
                    choice: VoiceAgent::default(),
                    notice: Some("Sign in to choose a voice agent.".into()),
                    busy: true,
                    ..state.clone()
                },
            );
        }
        view(&self.instance, &state)
    }
    async fn load(
        &self,
        lease: &Lease,
    ) -> Result<(VoiceAgent, Vec<AgentDefinition>, Vec<Assignment>), String> {
        let agents = lease
            .client()
            .agents(lease.session())
            .map_err(|e| e.to_string())?;
        let choice = agents
            .voice_agent(&lease.cancel)
            .await
            .map_err(|e| e.to_string())?;
        let mut definitions = vec![];
        let mut after = None;
        loop {
            let page = agents
                .list(after.as_deref(), 100, &lease.cancel)
                .await
                .map_err(|e| e.to_string())?;
            definitions.extend(page.data);
            after = page.next;
            if after.is_none() || definitions.len() >= 128 {
                break;
            }
        }
        let mut chats = vec![];
        let mut after = None;
        loop {
            let page = agents
                .assignments(None, after.as_deref(), &lease.cancel)
                .await
                .map_err(|e| e.to_string())?;
            chats.extend(page.data);
            after = page.next;
            if after.is_none() || chats.len() >= 256 {
                break;
            }
        }
        Ok((choice, definitions, chats))
    }
    async fn refresh_inner(&self) -> Result<(), String> {
        let lease = self.lease()?;
        let result = self.load(&lease).await;
        if !lease.is_current() {
            return Err("Your account changed. Open Voice again.".into());
        }
        let mut state = self.state.lock().unwrap();
        if state.generation != lease.generation {
            state.choice = VoiceAgent::default();
            state.definitions.clear();
            state.chats.clear();
        }
        state.generation = lease.generation;
        state.revision += 1;
        state.busy = false;
        match result {
            Ok((choice, definitions, chats)) => {
                state.choice = choice;
                state.definitions = definitions;
                state.chats = chats;
                state.notice = None;
            }
            Err(error) => {
                state.notice = Some(error.clone());
                return Err(error);
            }
        }
        Ok(())
    }
    pub async fn refresh(&self) -> Result<(), String> {
        let _io = self.io.lock().await;
        let result = self.refresh_inner().await;
        self.publish();
        result
    }
    pub async fn activate(&self, event: Activation) -> Result<(), String> {
        let _io = self.io.lock().await;
        let lease = self.lease()?;
        let action = self
            .view()
            .validate()
            .map_err(|_| "Couldn't open Voice.")?
            .activate(&event)
            .map_err(|_| "Your agents changed. Choose again.")?
            .clone();
        if let Action::Choose(target) = action {
            let revision = {
                let mut state = self.state.lock().unwrap();
                state.busy = true;
                state.revision += 1;
                state.choice.revision
            };
            self.publish();
            let result = async {
                let agents = lease
                    .client()
                    .agents(lease.session())
                    .map_err(|e| e.to_string())?;
                let request = agents
                    .voice_agent_request(revision, target)
                    .map_err(|e| e.to_string())?;
                agents
                    .set_voice_agent(&request, &lease.cancel)
                    .await
                    .map_err(|e| e.to_string())
            }
            .await;
            if lease.is_current() {
                let mut state = self.state.lock().unwrap();
                state.busy = false;
                state.revision += 1;
                match result {
                    Ok(choice) => {
                        state.choice = choice;
                        state.notice = None;
                    }
                    Err(error) => {
                        state.notice = Some(error.clone());
                        drop(state);
                        self.publish();
                        return Err(error);
                    }
                }
            }
        } else {
            self.refresh_inner().await?;
        }
        self.publish();
        Ok(())
    }
}
fn node(key: impl Into<String>, element: Element<Action>) -> Node<Action> {
    Node {
        key: key.into(),
        element,
        style: Default::default(),
    }
}
fn text(key: &str, value: impl Into<String>, role: TextRole) -> Node<Action> {
    node(
        key,
        Element::Text {
            value: value.into(),
            role,
        },
    )
}
fn view(instance: &str, state: &State) -> View<Action> {
    let mut children = vec![
        text("heading", "Voice agent", TextRole::Heading),
        text(
            "help",
            "Choose the agent and chat you want to talk to. Changes apply to your next question on every device.",
            TextRole::Body,
        ),
    ];
    if let Some(notice) = &state.notice {
        children.push(text("notice", notice, TextRole::Status));
    }
    children.push(node(
        "refresh",
        Element::Button {
            icon: None,
            shortcut: None,
            label: "Refresh agents".into(),
            enabled: !state.busy,
            intent: Action::Refresh,
        },
    ));
    children.push(node(
        "default",
        Element::Button {
            icon: None,
            shortcut: None,
            label: if state.choice.target.is_none() {
                "✓ Use the default voice agent"
            } else {
                "Use the default voice agent"
            }
            .into(),
            enabled: !state.busy,
            intent: Action::Choose(None),
        },
    ));
    for definition in &state.definitions {
        children.push(text(
            &format!("agent-{}", definition.id),
            &definition.spec.name,
            TextRole::Heading,
        ));
        let mut found = false;
        for chat in state
            .chats
            .iter()
            .filter(|a| !a.archived && a.spec.agent == definition.id)
        {
            found = true;
            let target = VoiceTarget {
                agent_id: definition.id.clone(),
                chat_id: chat.chat.clone(),
                host_id: chat.spec.host.clone(),
            };
            let selected = state.choice.target.as_ref() == Some(&target);
            children.push(node(
                format!("chat-{}", chat.id),
                Element::Button {
                    icon: None,
                    shortcut: None,
                    label: format!(
                        "{}{} · {}",
                        if selected { "✓ " } else { "" },
                        chat.spec
                            .task
                            .lines()
                            .next()
                            .unwrap_or("Agent chat")
                            .chars()
                            .take(80)
                            .collect::<String>(),
                        chat.spec.host
                    ),
                    enabled: !state.busy,
                    intent: Action::Choose(Some(target)),
                },
            ));
        }
        if !found {
            children.push(text(
                &format!("start-{}", definition.id),
                "Start a chat with this agent in Agent Lab to use it for voice.",
                TextRole::Status,
            ));
        }
    }
    View::new(
        instance,
        state.revision,
        node(
            "voice-agent",
            Element::Stack {
                axis: Axis::Vertical,
                children,
            },
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> State {
        let id = format!("{:064x}", 1);
        let definition = AgentDefinition {
            id: id.clone(),
            revision: 1,
            deleted: false,
            spec: openagents::agents::AgentSpec {
                name: "My own helper".into(),
                description: "A saved agent".into(),
                instructions: "Answer questions.".into(),
                runtime: "claude-code".into(),
                model: None,
                access: openagents::agents::Access::ReadOnly,
                fallbacks: vec![],
                autonomy: None,
            },
        };
        let assignment = Assignment {
            id: format!("{:064x}", 2),
            revision: 1,
            spec: openagents::agents::assignments::AssignmentSpec {
                agent: id.clone(),
                agent_revision: 1,
                host: "fixture-host".into(),
                project: None,
                task: "Hello".into(),
            },
            definition: definition.clone(),
            chat: "fixture-chat".into(),
            state: openagents::agents::assignments::AssignmentState::Completed,
            commands: vec![],
            result: None,
            archived: false,
            active_command: None,
            after_seq: 0,
        };
        State {
            generation: 1,
            revision: 1,
            choice: VoiceAgent::default(),
            definitions: vec![definition],
            chats: vec![assignment],
            busy: false,
            notice: None,
        }
    }
    #[test]
    fn choices_use_ids_even_with_duplicate_or_changed_names() {
        let mut state = fixture();
        let id = state.definitions[0].id.clone();
        let first = view("fixture", &state).validate().unwrap();
        let event = Activation {
            instance: "fixture".into(),
            revision: 1,
            node: format!("chat-{:064x}", 2),
        };
        let Action::Choose(Some(target)) = first.activate(&event).unwrap() else {
            panic!("missing choice")
        };
        assert_eq!(target.agent_id, id);
        assert_eq!(target.chat_id, "fixture-chat");
        state.definitions[0].spec.name = "Renamed fork".into();
        state.revision += 1;
        assert!(
            view("fixture", &state)
                .validate()
                .unwrap()
                .activate(&event)
                .is_err()
        );
        let json = serde_json::to_string(&view("fixture", &state)).unwrap();
        assert!(json.contains("Renamed fork"));
        assert!(oa_copy::violations("Choose the agent and chat you want to talk to. Changes apply to your next question on every device.", &[]).is_empty());
    }
    #[tokio::test]
    async fn signed_out_picker_hides_cached_agents_and_refuses_old_actions() {
        let root = tempfile::tempdir().unwrap();
        let deployment =
            openagents::Deployment::new("http://127.0.0.1:9/v1", "http://127.0.0.1:9").unwrap();
        let controller = Controller::new(
            openagents::Client::new(deployment).unwrap(),
            Arc::new(crate::desktop::MemoryStore::default()),
            root.path().join("signed-out"),
        );
        let picker = Picker::new(controller);
        *picker.state.lock().unwrap() = fixture();
        let event = Activation {
            instance: picker.instance.clone(),
            revision: 1,
            node: format!("chat-{:064x}", 2),
        };
        let shown = serde_json::to_string(&picker.view()).unwrap();
        assert!(!shown.contains("My own helper"));
        assert!(!shown.contains("fixture-chat"));
        assert!(picker.activate(event).await.is_err());
    }
}
