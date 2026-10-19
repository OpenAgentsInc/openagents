//! Public run commands enter the existing session command ledger.
use super::*;
use openagents_sdk::runs::{
    CommandOutcome, CommandReceipt, CommandRefusal, RunCommand, SessionCommand,
};
use sha2::{Digest, Sha256};

/// The account grant cache's admission function: definition ID, scope word,
/// and target chat. An absent hook refuses every agent command.
pub type RunCommandAuthority = dyn Fn(&str, &str, &str) -> Result<(), String> + Send + Sync;

fn command_id(account: &str, run: &str, request: &str, step: &str) -> String {
    let hash = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(account, run, request, step)).expect("string tuple"))
    );
    format!(
        "{}-{}-{}-{}-{}",
        &hash[..8],
        &hash[8..12],
        &hash[12..16],
        &hash[16..20],
        &hash[20..32]
    )
}

impl DocHost {
    /// Install the shared account-grant checker. This hook stores no grants.
    pub fn set_run_command_authority(&self, authority: Option<Arc<RunCommandAuthority>>) {
        *lock(&self.inner.run_command_authority) = authority;
    }

    pub(crate) async fn apply_run_command(
        &self,
        account: &str,
        agent: Option<&str>,
        run: &str,
        session: Option<&str>,
        request: &RunCommand,
    ) -> Result<CommandReceipt, EngineError> {
        let refused = || CommandReceipt {
            run: run.into(),
            request_id: request.request_id.clone(),
            outcome: CommandOutcome::Refused {
                reason: CommandRefusal::NotSteerable,
            },
        };
        let Some(workspace) = self.workspace() else {
            return Ok(refused());
        };
        let chats = workspace.read_chats()?;
        let direct = chats
            .iter()
            .find(|c| c.id == run && c.device_id == self.device_id());
        let matches: Vec<_> = chats
            .iter()
            .filter(|c| {
                c.device_id == self.device_id()
                    && session.is_some()
                    && c.harness_session_id.as_deref() == session
            })
            .collect();
        let chat = match direct.or_else(|| (matches.len() == 1).then(|| matches[0])) {
            Some(chat) => chat,
            None => return Ok(refused()),
        };
        if let Some(agent) = agent {
            let authority = lock(&self.inner.run_command_authority).clone();
            let scopes: &[&str] = match &request.command {
                SessionCommand::Message { .. }
                    if self.open_local(&chat.id)?.doc.read_entries()?.is_empty() =>
                {
                    &["chat.start"]
                }
                SessionCommand::Message { .. } => &["chat.message"],
                SessionCommand::Steer { .. } => &["chat.steer"],
                SessionCommand::Stop => &["chat.stop"],
                SessionCommand::Restart { .. }
                    if self.open_local(&chat.id)?.doc.read_entries()?.is_empty() =>
                {
                    &["chat.stop", "chat.message", "chat.start"]
                }
                SessionCommand::Restart { .. } => &["chat.stop", "chat.message"],
            };
            if !openagents_sdk::agents::valid_id(agent)
                || authority.as_ref().is_none_or(|check| {
                    scopes
                        .iter()
                        .any(|scope| check(agent, scope, &chat.id).is_err())
                })
            {
                return Ok(CommandReceipt {
                    run: run.into(),
                    request_id: request.request_id.clone(),
                    outcome: CommandOutcome::Refused {
                        reason: CommandRefusal::NoGrant,
                    },
                });
            }
        }
        let Some(sessions) = self.sessions() else {
            return Ok(refused());
        };
        if matches!(request.command, SessionCommand::Steer { .. })
            && (!sessions.turn_in_flight(&chat.id)
                || !chat
                    .config
                    .as_ref()
                    .is_some_and(|c| sessions.steers_mid_turn(c.harness)))
        {
            return Ok(refused());
        }
        let mut payloads = Vec::new();
        match &request.command {
            SessionCommand::Stop => payloads.push(("stop", SessionCommandPayload::Interrupt {})),
            SessionCommand::Steer { text } => payloads.push((
                "steer",
                SessionCommandPayload::Steer {
                    prompt: text.clone(),
                    message_id: Some(command_id(account, run, &request.request_id, "message")),
                },
            )),
            SessionCommand::Message { text } => {
                let Some(next) = self.request_from_chat_row(&chat.id, text) else {
                    return Ok(refused());
                };
                payloads.push((
                    "message",
                    SessionCommandPayload::Run {
                        request: next,
                        message_id: command_id(account, run, &request.request_id, "message"),
                    },
                ));
            }
            SessionCommand::Restart { brief } => {
                let next = match brief {
                    Some(text) => self.request_from_chat_row(&chat.id, text),
                    None => sessions.last_request(&chat.id),
                };
                let Some(next) = next else {
                    return Ok(refused());
                };
                payloads.push(("stop", SessionCommandPayload::Interrupt {}));
                payloads.push((
                    "restart",
                    SessionCommandPayload::Run {
                        request: next,
                        message_id: command_id(account, run, &request.request_id, "message"),
                    },
                ));
            }
        }
        let handle = self.open_local(&chat.id)?;
        let at = now_ms().max(
            handle
                .doc
                .read_commands()?
                .iter()
                .map(|c| c.issued_at)
                .max()
                .unwrap_or(0)
                .saturating_add(1),
        );
        for (index, (step, payload)) in payloads.into_iter().enumerate() {
            let id = command_id(account, run, &request.request_id, step);
            // Preserve the original timestamp and contents on an exact retry.
            let entry = handle
                .doc
                .read_commands()?
                .into_iter()
                .find(|entry| entry.id == id)
                .unwrap_or(SessionCommandEntry {
                    id,
                    payload,
                    issued_by: self.device_id().into(),
                    issued_at: at + index as i64,
                    based_on: None,
                    expires_at: Some(at + COMMAND_DEFAULT_TTL_MS),
                    status: SessionCommandStatus::Pending,
                    resolution: None,
                });
            handle.doc.queue_command(&entry)?;
            self.ingest_relayed_command(&chat.id, entry).await?;
        }
        Ok(CommandReceipt {
            run: run.into(),
            request_id: request.request_id.clone(),
            outcome: CommandOutcome::Accepted,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const CHAT: &str = "12054000-0000-4000-8000-000000000001";
    const REMOTE: &str = "12054000-0000-4000-8000-000000000002";
    const HOST: &str = "12054000-0000-4000-8000-000000000003";

    struct Harness {
        events: Arc<std::sync::Mutex<Vec<String>>>,
    }
    #[async_trait::async_trait]
    impl openagents_harness::Harness for Harness {
        fn id(&self) -> HarnessId {
            HarnessId::Mock
        }
        fn display_name(&self) -> &str {
            "Run command fixture"
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
        async fn models(
            &self,
        ) -> Result<Vec<openagents_proto::Model>, openagents_harness::HarnessError> {
            Ok(vec![])
        }
        async fn run(
            &self,
            request: openagents_proto::RunRequest,
            controls: openagents_harness::RunControls,
        ) -> Result<
            futures::stream::BoxStream<
                'static,
                Result<openagents_proto::AgentEvent, openagents_harness::HarnessError>,
            >,
            openagents_harness::HarnessError,
        > {
            use futures::StreamExt;
            self.events
                .lock()
                .unwrap()
                .push(format!("run:{}", request.prompt));
            let events = self.events.clone();
            let started = openagents_proto::AgentEvent::SessionStarted {
                harness: HarnessId::Mock,
                model: "fixture".into(),
                tools: vec![],
                cwd: request.cwd,
                session_id: "fixture-native".into(),
                assistant_message_id: "fixture-answer".into(),
            };
            let live = futures::stream::unfold(
                (controls, false, events),
                |(mut controls, done, events)| async move {
                    if done {
                        return None;
                    }
                    let (event, done) = tokio::select! {
                        _=controls.interrupt.cancelled() => (openagents_proto::AgentEvent::Done {status:openagents_proto::DoneStatus::Interrupted,result:None,error:None,session_id:None},true),
                        message=controls.steering.recv() => {
                            let message=message?;events.lock().unwrap().push(format!("steer:{}",message.prompt));
                            (openagents_proto::AgentEvent::TextDelta{text:"Steered".into()},false)
                        }
                    };
                    Some((Ok(event), (controls, done, events)))
                },
            );
            Ok(futures::stream::iter([Ok(started)]).chain(live).boxed())
        }
    }
    async fn observed(events: &Arc<std::sync::Mutex<Vec<String>>>, value: &str) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if events.lock().unwrap().iter().any(|s| s == value) {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn all_commands_use_the_session_ledger_and_exact_retries_do_not_restart_twice() {
        use crate::{
            RunJournal,
            registry::HarnessRegistry,
            workspace_host::{WorkspaceHost, WorkspaceHostConfig},
        };
        let temp = tempfile::tempdir().unwrap();
        let cwd = temp.path().to_str().unwrap().to_string();
        let store = Arc::new(DocsStore::open(temp.path().join("docs")).unwrap());
        let host = DocHost::new(
            store.clone(),
            DocHostConfig {
                device_id: HOST.into(),
                default_harness: HarnessId::Mock,
                edge: None,
            },
        );
        let workspace = WorkspaceHost::open(
            store,
            WorkspaceHostConfig {
                device_id: HOST.into(),
                device_name: "Fixture".into(),
                platform: "fixture".into(),
                device_origin: crate::DeviceOrigin::Unknown,
                org_id: "fixture".into(),
                user_id: "fixture".into(),
                edge: None,
                local_only: true,
            },
        )
        .unwrap();
        let config = openagents_proto::ChatConfig {
            harness: HarnessId::Mock,
            model: None,
            reasoning: None,
            model_options: Default::default(),
            sandbox: openagents_proto::SandboxLevel::WorkspaceWrite,
        };
        workspace
            .create_chat(
                CHAT,
                None,
                Some(HOST),
                Some(config.clone()),
                Some(cwd.clone()),
            )
            .unwrap();
        workspace
            .create_chat(REMOTE, None, Some("other-host"), Some(config), Some(cwd))
            .unwrap();
        host.set_workspace(workspace);
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let registry = Arc::new(HarnessRegistry::new());
        registry.register(Arc::new(Harness {
            events: events.clone(),
        }));
        let sessions = SessionsEngine::new(
            HOST.into(),
            Arc::new(RunJournal::open(temp.path().join("journal")).unwrap()),
            registry,
        );
        sessions.set_doc_host(host.clone());
        host.set_sessions(sessions.clone());
        let definition = "d".repeat(64);
        let probe = RunCommand {
            request_id: "ungranted".into(),
            command: SessionCommand::Stop,
        };
        assert_eq!(
            host.apply_run_command("account", Some(&definition), CHAT, None, &probe)
                .await
                .unwrap()
                .outcome,
            CommandOutcome::Refused {
                reason: CommandRefusal::NoGrant
            }
        );
        let checked = Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = checked.clone();
        let expected = definition.clone();
        host.set_run_command_authority(Some(Arc::new(move |agent, scope, target| {
            if agent != expected || target != CHAT {
                return Err("Denied".into());
            }
            seen.lock().unwrap().push(scope.to_string());
            Ok(())
        })));
        assert_eq!(
            host.apply_run_command("account", Some(&definition), CHAT, None, &probe)
                .await
                .unwrap()
                .outcome,
            CommandOutcome::Accepted
        );
        assert_eq!(*checked.lock().unwrap(), vec!["chat.stop"]);
        host.set_run_command_authority(Some(Arc::new(|_, scope, _| {
            if scope == "chat.start" {
                Err("Not granted".into())
            } else {
                Ok(())
            }
        })));
        let fresh_restart = RunCommand {
            request_id: "fresh-restart".into(),
            command: SessionCommand::Restart {
                brief: Some("Cannot start".into()),
            },
        };
        assert_eq!(
            host.apply_run_command("account", Some(&definition), CHAT, None, &fresh_restart)
                .await
                .unwrap()
                .outcome,
            CommandOutcome::Refused {
                reason: CommandRefusal::NoGrant
            }
        );
        assert!(events.lock().unwrap().is_empty());
        host.set_run_command_authority(None);
        let message = RunCommand {
            request_id: "message".into(),
            command: SessionCommand::Message {
                text: "Start".into(),
            },
        };
        assert_eq!(
            host.apply_run_command("account", None, CHAT, None, &message)
                .await
                .unwrap()
                .outcome,
            CommandOutcome::Accepted
        );
        observed(&events, "run:Start").await;
        host.apply_run_command("account", None, CHAT, None, &message)
            .await
            .unwrap();
        let steer = RunCommand {
            request_id: "steer".into(),
            command: SessionCommand::Steer {
                text: "Review".into(),
            },
        };
        assert_eq!(
            host.apply_run_command("account", None, CHAT, None, &steer)
                .await
                .unwrap()
                .outcome,
            CommandOutcome::Accepted
        );
        observed(&events, "steer:Review").await;
        let stop = RunCommand {
            request_id: "stop".into(),
            command: SessionCommand::Stop,
        };
        host.apply_run_command("account", None, CHAT, None, &stop)
            .await
            .unwrap();
        assert!(!sessions.turn_in_flight(CHAT));
        assert_eq!(
            host.apply_run_command(
                "account",
                None,
                CHAT,
                None,
                &RunCommand {
                    request_id: "idle-steer".into(),
                    command: steer.command.clone()
                }
            )
            .await
            .unwrap()
            .outcome,
            CommandOutcome::Refused {
                reason: CommandRefusal::NotSteerable
            }
        );
        let restart = RunCommand {
            request_id: "restart".into(),
            command: SessionCommand::Restart {
                brief: Some("Again".into()),
            },
        };
        host.apply_run_command("account", None, CHAT, None, &restart)
            .await
            .unwrap();
        observed(&events, "run:Again").await;
        host.apply_run_command("account", None, CHAT, None, &restart)
            .await
            .unwrap();
        assert_eq!(
            events
                .lock()
                .unwrap()
                .iter()
                .filter(|e| e.starts_with("run:"))
                .count(),
            2
        );
        assert_eq!(
            host.apply_run_command("account", None, REMOTE, None, &stop)
                .await
                .unwrap()
                .outcome,
            CommandOutcome::Refused {
                reason: CommandRefusal::NotSteerable
            }
        );
        assert_eq!(
            host.apply_run_command(
                "account",
                None,
                "terminal-run",
                Some("no-app-session"),
                &stop
            )
            .await
            .unwrap()
            .outcome,
            CommandOutcome::Refused {
                reason: CommandRefusal::NotSteerable
            }
        );
        sessions.shutdown().await;
        host.shutdown_workers().await;
    }
}
