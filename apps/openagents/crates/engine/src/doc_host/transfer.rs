//! Freeze a computer part before requesting its linked destination.
use super::*;
use chat_home::{Home, PartRef, Role, Seed, TransferClaim};
use std::time::Duration;

fn context(doc: &SessionDoc, title: &str) -> Result<Seed, EngineError> {
    let entries = join_continuation_entries(doc.read_entries()?);
    let first_question = entries
        .iter()
        .filter(|e| e.role == MessageRole::User)
        .map(|e| bounded_text(&e.parts))
        .find(|text| !text.trim().is_empty());
    let mut turns = entries
        .into_iter()
        .rev()
        .filter_map(|entry| {
            let role = match entry.role {
                MessageRole::User => Role::User,
                MessageRole::Assistant => Role::Assistant,
                MessageRole::System => return None,
            };
            let text = bounded_text(&entry.parts);
            (!text.trim().is_empty()).then_some((role, text))
        })
        .take(chat_home::MAX_RECENT)
        .collect::<Vec<_>>();
    turns.reverse();
    let recent = turns
        .iter()
        .map(|(role, text)| (role.clone(), text.as_str()));
    Ok(
        if let Some(previous) = openagents_doc::continuation::get(doc) {
            previous.continued.seed.with_recent(recent)
        } else {
            let mut seed = Seed::new(title, recent);
            if let Some(first) = first_question {
                seed.summary = Seed::new(title, [(Role::User, first.as_str())]).summary;
            }
            seed
        },
    )
}

fn bounded_text(parts: &[MessagePart]) -> String {
    let mut text = String::new();
    for part in parts {
        if let MessagePart::Text { text: value, .. } = part {
            for c in value.chars() {
                if text.len() + c.len_utf8() > 1024 {
                    break;
                }
                text.push(c);
            }
        }
        if text.len() >= 1021 {
            break;
        }
    }
    text
}

impl DocHost {
    pub(crate) async fn prepare_destination(
        &self,
        chat: &str,
        source: &chat_home::Source,
    ) -> Result<PartRef, EngineError> {
        let failed = || EngineError::Other("Connect this computer and open the chat again.".into());
        let lease = self.openagents_lease().ok_or_else(failed)?;
        if source.account != lease.session().account().id
            || source.revision == 0
            || !chat_home::valid_id(chat)
            || !chat_home::valid_id(&source.chat)
            || chat == source.chat
        {
            return Err(failed());
        }
        let workspace = self.workspace().ok_or_else(failed)?;
        for _ in 0..20 {
            if !lease.is_current() {
                return Err(failed());
            }
            if let Some(row) = workspace.chat(chat)? {
                if row.device_id != self.device_id() || !row.is_top_level() {
                    return Err(failed());
                }
                let handle = self.open(chat)?;
                self.ensure_editable(chat)?;
                let pending = row
                    .config
                    .as_ref()
                    .and_then(|c| c.model_options.get(chat_home::REQUEST_OPTION))
                    .and_then(|v| serde_json::from_value::<chat_home::Source>(v.clone()).ok());
                let stored = openagents_doc::continuation::get(handle.doc());
                let already_linked = stored.as_ref().is_some_and(|s| {
                    s.continued.account == source.account
                        && s.continued.source.id == source.chat
                        && s.continued.target.id == chat
                        && s.continued.target.home == Home::Computer(self.device_id().into())
                        && s.continued.seed.valid()
                });
                if !already_linked
                    && (pending.as_ref() != Some(source)
                        || !handle.doc().read_entries()?.is_empty())
                {
                    return Err(failed());
                }
                workspace.persist_continuation()?;
                self.persist_fork(&handle)?;
                return Ok(PartRef {
                    id: chat.into(),
                    home: Home::Computer(self.device_id().into()),
                });
            }
            tokio::select! { _ = lease.cancel.cancelled() => return Err(failed()), _ = tokio::time::sleep(Duration::from_millis(100)) => {} }
        }
        Err(failed())
    }

    pub(crate) fn ensure_editable(&self, chat: &str) -> Result<(), EngineError> {
        if openagents_doc::continuation::read_only(self.open(chat)?.doc())
            || self.workspace().is_some_and(|w| {
                w.chat(chat)
                    .ok()
                    .flatten()
                    .is_some_and(|c| c.links.continued_by.is_some())
                    || w.chat_move_targets().contains_key(chat)
            })
        {
            return Err(EngineError::Other(
                "This chat continues elsewhere. Open its latest part.".into(),
            ));
        }
        Ok(())
    }

    pub(crate) async fn continue_on(
        &self,
        account: &str,
        chat: &str,
        target: PartRef,
    ) -> Result<chat_home::Continued, EngineError> {
        let failed = |text: &str| EngineError::Other(text.into());
        let lease = self
            .openagents_lease()
            .ok_or_else(|| failed("Sign in to OpenAgents on this computer."))?;
        if lease.session().account().id != account || !lease.is_current() {
            return Err(failed("Sign in to the same account on this computer."));
        }
        if !self.is_host(chat)
            || !target.valid()
            || target.id == chat
            || target.home == Home::Computer(self.inner.config.device_id.clone())
        {
            return Err(failed("Choose another place to continue this chat."));
        }
        let handle = self.open(chat)?;
        let _commands = handle
            .command_drain_lock
            .try_lock()
            .map_err(|_| failed("Wait for the current message to finish sending."))?;
        let _queue = handle
            .drain_lock
            .try_lock()
            .map_err(|_| failed("Wait for the current message to finish sending."))?;
        let _home = handle
            .home_transfer_lock
            .try_lock()
            .map_err(|_| failed("Wait for the current message to finish sending."))?;
        let workspace = self
            .workspace()
            .ok_or_else(|| failed("This computer is not ready."))?;
        let row = workspace
            .chat(chat)?
            .ok_or_else(|| failed("Open the earlier chat again."))?;
        let (outgoing, revision) = if let Some(old) =
            openagents_doc::continuation::outgoing(handle.doc())
        {
            if old.account != account || old.target.home != target.home {
                return Err(failed(
                    "This chat already continues elsewhere. Open its latest part.",
                ));
            }
            // A queued send can update sidebar time after the freeze. Keep
            // the saved context and destination, and use the current index row.
            let remote = openagents_account::openagents::sessions::get(
                lease.client(),
                lease.session(),
                chat,
                &lease.cancel,
            )
            .await?;
            if !lease.is_current() || remote.session.home != Home::Computer(self.device_id().into())
            {
                return Err(failed("Open the earlier chat again."));
            }
            (old, remote.session.last_activity_unix.max(1))
        } else {
            if self
                .sessions()
                .and_then(|s| s.session_status(chat))
                .is_some_and(|s| {
                    matches!(
                        s.status,
                        openagents_proto::SessionStatus::Working
                            | openagents_proto::SessionStatus::AwaitingInput
                    )
                })
                || !handle.doc().read_queue()?.is_empty()
                || !lock(&handle.held_sends).is_empty()
                || handle.doc().read_commands()?.iter().any(|c| {
                    c.status == SessionCommandStatus::Pending
                        && matches!(
                            c.payload,
                            SessionCommandPayload::Run { .. } | SessionCommandPayload::Steer { .. }
                        )
                })
            {
                return Err(failed(
                    "Stop the current answer and clear queued messages before continuing elsewhere.",
                ));
            }
            let revision = row
                .last_message_at
                .unwrap_or(row.created_at)
                .timestamp()
                .max(1) as u64;
            let remote = openagents_account::openagents::sessions::get(
                lease.client(),
                lease.session(),
                chat,
                &lease.cancel,
            )
            .await?;
            if !lease.is_current() {
                return Err(failed("Sign in again before continuing."));
            }
            if remote.session.home != Home::Computer(self.inner.config.device_id.clone())
                || remote.session.last_activity_unix.max(1) != revision
                || !remote.session.home_transfer
            {
                return Err(failed("This chat is still syncing. Try again in a moment."));
            }
            let target = remote.session.continued_by.unwrap_or(target);
            let outgoing = openagents_doc::continuation::Outgoing {
                account: account.into(),
                target: target.clone(),
                revision,
                seed: context(handle.doc(), row.title.as_deref().unwrap_or("Chat"))?,
            };
            openagents_doc::continuation::set_outgoing(&handle.writer(), &outgoing)?;
            workspace.set_chat_move_target(chat, &outgoing.target)?;
            self.persist_fork(&handle)?;
            (outgoing, revision)
        };
        workspace.set_chat_move_target(chat, &outgoing.target)?;
        let source = lease.client().chat(lease.session(), chat)?;
        let continued = openagents_account::openagents::continuations::transfer(
            lease.client(),
            lease.session(),
            &source,
            &TransferClaim {
                revision,
                target: outgoing.target,
                seed: Some(outgoing.seed),
            },
            &lease.cancel,
        )
        .await?;
        if !lease.is_current() {
            return Err(failed("Sign in again before continuing."));
        }
        workspace.set_chat_destination(chat, &continued.target)?;
        Ok(continued)
    }

    pub(super) async fn process_web_moves(
        &self,
        lease: &openagents_account::Lease,
    ) -> Result<(), EngineError> {
        let Some(workspace) = self.workspace() else {
            return Ok(());
        };
        let mut moves = workspace.chat_move_targets();
        let requested = match openagents_account::openagents::sessions::pending_moves(
            lease.client(),
            lease.session(),
            self.device_id(),
            &lease.cancel,
        )
        .await
        {
            Ok(value) => value,
            Err(error) if moves.is_empty() => return Err(error.into()),
            Err(_) => vec![],
        };
        for request in requested {
            if workspace
                .chat(&request.source)?
                .is_some_and(|row| row.links.continued_by.as_ref() != Some(&request.target))
            {
                moves.entry(request.source).or_insert(request.target);
            }
        }
        let mut moves = moves.into_iter().collect::<Vec<_>>();
        moves.sort_by(|a, b| a.0.cmp(&b.0));
        let start = (now_ms().max(0) as usize / 3000) % moves.len().max(1);
        for (source, target) in moves.iter().cycle().skip(start).take(moves.len().min(8)) {
            if !lease.is_current() {
                break;
            }
            let _ = self
                .continue_on(&lease.session().account().id, source, target.clone())
                .await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn continuation_context_keeps_the_first_goal_and_bounded_recent_turns() {
        let root = tempfile::tempdir().unwrap();
        let host = DocHost::new(
            Arc::new(openagents_sync::DocsStore::open(root.path()).unwrap()),
            DocHostConfig {
                device_id: "fixture-mac".into(),
                default_harness: HarnessId::Mock,
                edge: None,
            },
        );
        let handle = host.open("d75ef8a1-16ca-49b6-83be-000000000001").unwrap();
        for i in 0..16 {
            handle
                .writer()
                .push_message(&SessionMessageEntry {
                    id: format!("fixture-{i}"),
                    role: if i == 15 {
                        MessageRole::System
                    } else {
                        MessageRole::User
                    },
                    parts: vec![MessagePart::Text {
                        id: "text".into(),
                        text: if i == 0 {
                            "Build the garden planner".into()
                        } else if i == 15 {
                            "private system instructions".into()
                        } else {
                            format!("Recent turn {i}")
                        },
                    }],
                    created_at: i,
                    device_id: "fixture-mac".into(),
                    status: Some(MessageStatus::Complete),
                    continuation_of: None,
                    duration_ms: None,
                })
                .unwrap();
        }
        let seed = context(handle.doc(), "Garden planner").unwrap();
        assert!(seed.valid());
        assert!(seed.summary.contains("Build the garden planner"));
        assert_eq!(seed.recent.len(), chat_home::MAX_RECENT);
        assert_eq!(seed.recent.last().unwrap().text, "Recent turn 14");
        assert!(
            !serde_json::to_string(&seed)
                .unwrap()
                .contains("private system instructions")
        );
    }

    #[tokio::test]
    async fn outgoing_continuation_blocks_dispatch_after_snapshot_reopen() {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(openagents_sync::DocsStore::open(root.path()).unwrap());
        let config = || DocHostConfig {
            device_id: "fixture-mac".into(),
            default_harness: HarnessId::Mock,
            edge: None,
        };
        let host = DocHost::new(store.clone(), config());
        let chat = "d75ef8a1-16ca-49b6-83be-000000000001";
        let handle = host.open(chat).unwrap();
        assert!(host.ensure_editable(chat).is_ok());
        openagents_doc::continuation::set_outgoing(
            &handle.writer(),
            &openagents_doc::continuation::Outgoing {
                account: "fixture".into(),
                target: PartRef {
                    id: "d75ef8a1-16ca-49b6-83be-000000000002".into(),
                    home: Home::Cloud,
                },
                revision: 1,
                seed: Seed::new("Earlier work", [(Role::User, "coral orchid")]),
            },
        )
        .unwrap();
        host.persist_fork(&handle).unwrap();
        assert!(host.ensure_editable(chat).is_err());
        let reopened = DocHost::new(store, config());
        assert!(reopened.ensure_editable(chat).is_err());
    }
}
