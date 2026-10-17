//! A driver for agents that speak OpenAgents events as JSON lines, always
//! under a keeper, so every run it starts can be adopted.
//!
//! The agent writes one [`AgentEvent`] per stdout line, or
//! `{"ask":{"id":ID,"questions":[...]}}` to ask the person something and wait.
//! The driver writes `{"prompt":TEXT}` for the first prompt and each steer, and
//! `{"answer":{"id":ID,"answers":[...]}}` once the person answers.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};

use openagents_proto::{
    AgentEvent, DoneStatus, HarnessId, Model, ReasoningLevel, RunRequest, SteeringMode,
    UserInputAnswer, UserInputQuestion,
};

use super::{Detach, FrameBody, KeeperSignal, Link, LinkSender, Spawn};
use crate::{Harness, HarnessError, RunControls};

/// An agent program that speaks the line protocol above.
pub struct LineHarness {
    pub id: HarnessId,
    pub name: String,
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub env: Vec<(OsString, OsString)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ask {
    pub id: String,
    pub questions: Vec<UserInputQuestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Answer {
    pub id: String,
    pub answers: Vec<UserInputAnswer>,
}

/// One line the driver writes to the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToAgent {
    Prompt(String),
    Answer(Answer),
}

/// The line the agent writes to ask a question.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FromAgent {
    Ask(Ask),
}

type EventStream = BoxStream<'static, Result<AgentEvent, HarnessError>>;

#[async_trait]
impl Harness for LineHarness {
    fn id(&self) -> HarnessId {
        self.id
    }

    fn display_name(&self) -> &str {
        &self.name
    }

    fn supports_steering(&self) -> bool {
        true
    }

    fn steering_mode(&self) -> SteeringMode {
        SteeringMode::TurnBoundary
    }

    fn reasoning_levels(&self) -> &[ReasoningLevel] {
        &[]
    }

    fn installed(&self) -> bool {
        self.program.exists()
    }

    async fn models(&self) -> Result<Vec<Model>, HarnessError> {
        Ok(Vec::new())
    }

    async fn run(
        &self,
        request: RunRequest,
        controls: RunControls,
    ) -> Result<EventStream, HarnessError> {
        let scope = super::current().ok_or_else(|| {
            HarnessError::Protocol("this agent runs only where agents can outlive the app".into())
        })?;
        let link = super::client::spawn(Spawn {
            keeper_exe: scope.keeper_exe,
            keeper_args: scope.keeper_args,
            root: scope.root,
            program: self.program.clone(),
            args: self.args.clone(),
            cwd: Some(PathBuf::from(&request.cwd)),
            env: self
                .env
                .iter()
                .map(|(key, value)| (key.clone(), Some(value.clone())))
                .collect(),
            meta: scope.meta,
            orphan_after: scope.orphan_after,
        })
        .await?;
        write(&link.sender(), &ToAgent::Prompt(request.prompt))?;
        Ok(drive(link, controls, scope.detach, false))
    }

    async fn attach(
        &self,
        _request: RunRequest,
        controls: RunControls,
        link: Link,
    ) -> Option<Result<EventStream, HarnessError>> {
        let detach = super::current()
            .map(|scope| scope.detach)
            .unwrap_or_default();
        Some(Ok(drive(link, controls, detach, true)))
    }
}

fn write(sender: &LinkSender, message: &ToAgent) -> std::io::Result<()> {
    sender.stdin(serde_json::to_string(message)?)
}

enum Item {
    /// An event from frame `seq`.
    Event(u64, AgentEvent),
    /// Frame `seq` produced no event; acknowledge it in order.
    Ack(u64),
    Fail(String),
}

/// Decode a keeper's frames into events. `adopted` is true when this engine
/// picks up a run another engine started. Dropping the stream stops the
/// agent unless `detach` is raised; an interrupt while `detach` is raised
/// leaves the agent running.
pub fn drive(link: Link, controls: RunControls, detach: Detach, adopted: bool) -> EventStream {
    let (sender, mut frames) = link.into_parts();
    let RunControls {
        request_input,
        mut steering,
        interrupt,
        ..
    } = controls;
    let request_input = Arc::new(request_input);
    let (items, rx) = mpsc::channel::<Item>(256);
    let task_sender = sender.clone();
    tokio::spawn(async move {
        let sender = task_sender;
        let mut answered: HashSet<String> = HashSet::new();
        // Questions this engine is asking, by id: dropping the sender
        // withdraws the question.
        let mut asking: HashMap<String, oneshot::Sender<()>> = HashMap::new();
        let mut steering_open = true;
        let mut interrupted = false;
        // A prompt or output with no Done after it yet. A fresh run starts
        // with its prompt; an adopted one learns from the frames it replays.
        let mut open_turn = !adopted;
        loop {
            let item = tokio::select! {
                frame = frames.recv() => {
                    let Some(frame) = frame else {
                        let _ = items.send(Item::Fail("lost the connection to the agent".into())).await;
                        return;
                    };
                    let seq = frame.seq;
                    match frame.body {
                        FrameBody::Stdout { line } => {
                            if let Ok(FromAgent::Ask(ask)) = serde_json::from_str(&line) {
                                if !answered.contains(&ask.id) && !asking.contains_key(&ask.id) {
                                    let _ = sender.hold(seq);
                                    let (withdraw, withdrawn) = oneshot::channel();
                                    asking.insert(ask.id.clone(), withdraw);
                                    let answer = request_input(ask.questions);
                                    let sender = sender.clone();
                                    let id = ask.id;
                                    tokio::spawn(async move {
                                        tokio::select! {
                                            answer = answer => if let Ok(answers) = answer {
                                                let _ = write(&sender, &ToAgent::Answer(Answer { id, answers }));
                                                let _ = sender.release(seq);
                                            },
                                            _ = withdrawn => {}
                                        }
                                    });
                                }
                                Item::Ack(seq)
                            } else if let Ok(event) = serde_json::from_str::<AgentEvent>(&line) {
                                open_turn = !matches!(event, AgentEvent::Done { .. });
                                Item::Event(seq, event)
                            } else {
                                Item::Ack(seq)
                            }
                        }
                        FrameBody::Stdin { line } => {
                            match serde_json::from_str::<ToAgent>(&line) {
                                Ok(ToAgent::Answer(answer)) => {
                                    // Answered, here or by an earlier engine.
                                    asking.remove(&answer.id);
                                    answered.insert(answer.id);
                                }
                                Ok(ToAgent::Prompt(_)) => open_turn = true,
                                Err(_) => {}
                            }
                            Item::Ack(seq)
                        }
                        FrameBody::Stderr { line } => {
                            tracing::debug!(target: "openagents_harness::keeper", "agent stderr: {line}");
                            Item::Ack(seq)
                        }
                        FrameBody::Exit { code, signal } => {
                            let item = if !open_turn {
                                Item::Ack(seq)
                            } else {
                                Item::Event(seq, AgentEvent::Done {
                                    status: if interrupted { DoneStatus::Interrupted } else { DoneStatus::Errored },
                                    result: None,
                                    error: (!interrupted).then(|| exit_message(code, signal)),
                                    session_id: None,
                                })
                            };
                            let _ = items.send(item).await;
                            return;
                        }
                    }
                }
                steer = steering.recv(), if steering_open => {
                    match steer {
                        Some(steer) => { let _ = write(&sender, &ToAgent::Prompt(steer.prompt)); }
                        None => steering_open = false,
                    }
                    continue;
                }
                _ = interrupt.cancelled(), if !interrupted => {
                    interrupted = true;
                    if detach.raised() {
                        return; // the engine is leaving; the agent keeps running
                    }
                    let _ = sender.signal(KeeperSignal::Interrupt);
                    continue;
                }
                // The engine let the run go while it stays up: end the agent.
                // An engine that dies never gets here, so its agent keeps running.
                _ = items.closed() => {
                    if !detach.raised() {
                        let _ = sender.finish();
                    }
                    return;
                }
            };
            if items.send(item).await.is_err() {
                if !detach.raised() {
                    let _ = sender.finish();
                }
                return;
            }
        }
    });
    // An event is acknowledged as it is handed over. The engine journals
    // what it receives before it next yields, so an engine that dies has
    // journaled every event this stream acknowledged.
    futures::stream::unfold((rx, sender, false), |(mut rx, sender, ended)| async move {
        if ended {
            return None;
        }
        loop {
            match rx.recv().await? {
                Item::Event(seq, event) => {
                    let _ = sender.ack(seq);
                    return Some((Ok(event), (rx, sender, false)));
                }
                Item::Ack(seq) => {
                    let _ = sender.ack(seq);
                }
                Item::Fail(message) => {
                    return Some((Err(HarnessError::Protocol(message)), (rx, sender, true)));
                }
            }
        }
    })
    .boxed()
}

fn exit_message(code: Option<i32>, signal: Option<i32>) -> String {
    match (code, signal) {
        (_, Some(signal)) => format!("The agent stopped (signal {signal})."),
        (Some(code), None) => format!("The agent exited with code {code}."),
        (None, None) => "The agent could not start.".into(),
    }
}
