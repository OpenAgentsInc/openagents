//! The normal Run command claims its source before the harness receives any context.
use super::*;
use chat_home::{Claim, Home, PartRef, Source};

fn claim_error(error: openagents_account::openagents::Error) -> EngineError {
    use openagents_account::openagents::Error;
    let message = match error {
        Error::Refused {
            code: Some("continued"),
            ..
        } => "This chat already continues elsewhere. Open its latest part.",
        Error::Refused {
            code: Some("changed"),
            ..
        } => "This chat changed. Open it again before continuing.",
        Error::Refused {
            code: Some("busy"), ..
        } => "Stop the current answer before continuing elsewhere.",
        Error::Refused {
            code: Some("context_required"),
            ..
        } => "Open this chat on its computer before continuing elsewhere.",
        Error::Refused { status: 401, .. } => "We couldn't connect to your account. Try again.",
        Error::WrongAccount => "This chat belongs to a different account.",
        Error::Refused { status: 404, .. } | Error::NotFound => {
            "The earlier chat is no longer available to this account."
        }
        _ => "Couldn't continue this chat. Try again when OpenAgents is available.",
    };
    EngineError::Other(message.into())
}
impl DocHost {
    pub(super) async fn recover_continuations(
        &self,
        lease: &openagents_account::Lease,
    ) -> Result<(), EngineError> {
        let Some(workspace) = self.workspace() else {
            return Ok(());
        };
        let pending = workspace
            .read_chats()?
            .into_iter()
            .filter(|c| {
                c.device_id == self.inner.config.device_id
                    && !c.archived
                    && c.config
                        .as_ref()
                        .is_some_and(|c| c.model_options.contains_key(chat_home::REQUEST_OPTION))
            })
            .collect::<Vec<_>>();
        if pending.is_empty() {
            return Ok(());
        }
        let start = (now_ms().max(0) as usize / 3000) % pending.len();
        for chat in pending
            .iter()
            .cycle()
            .skip(start)
            .take(pending.len().min(8))
        {
            if !lease.is_current() {
                break;
            }
            let config = chat.config.as_ref().unwrap();
            let source: Source =
                serde_json::from_value(config.model_options[chat_home::REQUEST_OPTION].clone())
                    .map_err(|_| EngineError::Other("Open the earlier chat again.".into()))?;
            if source.account != lease.session().account().id {
                continue;
            }
            let source_ref = lease
                .client()
                .chat(lease.session(), &source.chat)
                .map_err(EngineError::AgentService)?;
            let destination = openagents_account::openagents::continuations::destination(
                lease.client(),
                lease.session(),
                &source_ref,
                &lease.cancel,
            )
            .await
            .map_err(claim_error)?;
            let expected = PartRef {
                id: chat.id.clone(),
                home: Home::Computer(self.inner.config.device_id.clone()),
            };
            if destination.as_ref() != Some(&expected) {
                continue;
            }
            // Repair metadata only after the server already linked this exact destination.
            // This never starts a harness or changes an unclaimed source.
            let request = openagents_proto::RunRequest {
                prompt: String::new(),
                harness: Some(config.harness),
                model: config.model.clone(),
                reasoning: config.reasoning,
                model_options: config.model_options.clone(),
                cwd: chat.cwd.clone().unwrap_or_default(),
                sandbox: config.sandbox,
                auto_approve: false,
                resume: None,
                attachments: vec![],
                worktree: None,
                mcp: None,
            };
            self.prepare_continuation(&chat.id, config.harness, request)
                .await?;
        }
        Ok(())
    }

    pub(super) fn continuation_command(
        &self,
        handle: &ChatDocHandle,
        payload: &SessionCommandPayload,
    ) -> Result<Option<(String, bool)>, EngineError> {
        let SessionCommandPayload::Run {
            request,
            message_id,
        } = payload
        else {
            return Ok(None);
        };
        let Some(source) = request.model_options.get(chat_home::REQUEST_OPTION) else {
            return Ok(None);
        };
        if !chat_home::valid_id(message_id) {
            return Err(EngineError::Other(
                "Send this message again from the saved chat.".into(),
            ));
        }
        for previous in handle.doc().read_commands()?.into_iter().rev() {
            if let SessionCommandPayload::Run {
                request: old,
                message_id: old_id,
            } = &previous.payload
            {
                if old_id == message_id {
                    if old.prompt != request.prompt
                        || old.model_options.get(chat_home::REQUEST_OPTION) != Some(source)
                    {
                        return Err(EngineError::Other(
                            "The first message is still pending. Retry it unchanged.".into(),
                        ));
                    }
                    return Ok(Some(
                        if matches!(
                            previous.status,
                            SessionCommandStatus::Pending | SessionCommandStatus::Applied
                        ) {
                            (previous.id, true)
                        } else {
                            (new_id(), false)
                        },
                    ));
                }
            }
        }
        Ok(Some((message_id.clone(), false)))
    }

    fn publish_continuation(
        &self,
        chat: &str,
        stored: &openagents_doc::continuation::Stored,
    ) -> Result<(), EngineError> {
        let workspace = self
            .workspace()
            .ok_or_else(|| EngineError::Other("This computer is not ready.".into()))?;
        workspace.set_chat_continuation(chat, &stored.continued.source)?;
        workspace.rename_chat(chat, &stored.continued.seed.title)?;
        if let Some(mut config) = workspace.chat_config(chat) {
            if config
                .model_options
                .remove(chat_home::REQUEST_OPTION)
                .is_some()
            {
                workspace.set_chat_config(chat, &config)?;
            }
        }
        Ok(())
    }

    pub(super) async fn prepare_continuation(
        &self,
        chat_id: &str,
        harness: HarnessId,
        mut request: openagents_proto::RunRequest,
    ) -> Result<openagents_proto::RunRequest, EngineError> {
        let handle = self.open(chat_id)?;
        let value = request
            .model_options
            .remove(chat_home::REQUEST_OPTION)
            .or_else(|| {
                openagents_doc::continuation::pending(handle.doc())
                    .and_then(|s| serde_json::to_value(s).ok())
            })
            .or_else(|| {
                self.workspace()
                    .and_then(|w| w.chat_config(chat_id))
                    .and_then(|c| c.model_options.get(chat_home::REQUEST_OPTION).cloned())
            });
        let Some(value) = value else {
            return Ok(request);
        };
        let source: Source = serde_json::from_value(value).map_err(|_| {
            EngineError::Other("Open the saved chat again before continuing.".into())
        })?;
        if let Some(stored) = openagents_doc::continuation::get(handle.doc()) {
            if stored.continued.account == source.account
                && stored.continued.source.id == source.chat
                && stored.continued.target.id == chat_id
            {
                if self
                    .workspace()
                    .and_then(|w| w.chat_config(chat_id))
                    .is_some_and(|c| c.model_options.contains_key(chat_home::REQUEST_OPTION))
                {
                    self.publish_continuation(chat_id, &stored)?;
                }
                return Ok(request);
            }
            return Err(EngineError::Other(
                "This conversation already continues from another chat.".into(),
            ));
        }
        let lease = self
            .openagents_lease()
            .ok_or_else(|| EngineError::Other("Connecting to your account…".into()))?;
        if !handle.doc().read_entries()?.is_empty() {
            return Err(EngineError::Other(
                "Start a new conversation to continue this chat.".into(),
            ));
        }
        if source.account != lease.session().account().id {
            return Err(EngineError::Other(
                "This chat belongs to a different account.".into(),
            ));
        }
        let target = PartRef {
            id: chat_id.into(),
            home: Home::computer(&self.inner.config.device_id)
                .ok_or_else(|| EngineError::Other("This computer is not ready.".into()))?,
        };
        let chat = lease
            .client()
            .chat(lease.session(), &source.chat)
            .map_err(EngineError::AgentService)?;
        let workspace = self
            .workspace()
            .ok_or_else(|| EngineError::Other("This computer is not ready.".into()))?;
        let mut config = workspace
            .chat_config(chat_id)
            .unwrap_or(openagents_proto::ChatConfig {
                harness,
                model: request.model.clone(),
                reasoning: request.reasoning,
                model_options: request.model_options.clone(),
                sandbox: request.sandbox,
            });
        config.model_options.insert(
            chat_home::REQUEST_OPTION.into(),
            serde_json::to_value(&source)
                .map_err(|_| EngineError::Other("Open the earlier chat again.".into()))?,
        );
        workspace.set_chat_config(chat_id, &config)?;
        let writer = handle.writer();
        openagents_doc::continuation::set_pending(&writer, &source)?;
        let continued = openagents_account::openagents::continuations::claim(
            lease.client(),
            lease.session(),
            &chat,
            &Claim {
                revision: source.revision,
                target,
            },
            &lease.cancel,
        )
        .await
        .map_err(claim_error)?;
        if !lease.is_current() {
            return Err(EngineError::Other(
                "We couldn't connect to your account. Open the chat again to continue.".into(),
            ));
        }
        let backlink = lease
            .client()
            .deployment()
            .website()
            .join(&format!("{}/{}", if matches!(&continued.source.home, Home::Computer(device) if !device.starts_with("legacy-")) {"chat/sessions"} else {"chat"}, source.chat))
            .map_err(|_| EngineError::Other("The earlier chat could not be opened.".into()))?
            .to_string();
        let stored = openagents_doc::continuation::Stored {
            continued,
            backlink,
        };
        openagents_doc::continuation::set(&writer, &stored)?;
        self.publish_continuation(chat_id, &stored)?;
        let id = format!("continued-from:{}", source.chat);
        if !writer.read_entries()?.iter().any(|entry| entry.id == id) {
            writer.push_message(&SessionMessageEntry {
                id,
                role: MessageRole::System,
                parts: vec![MessagePart::Text {
                    id: "source".into(),
                    text: format!(
                        "Continued from [{}]({}).",
                        stored.continued.seed.title.replace(['[', ']'], " "),
                        stored.backlink
                    ),
                }],
                created_at: now_ms(),
                device_id: self.inner.config.device_id.clone(),
                status: Some(MessageStatus::Complete),
                continuation_of: None,
                duration_ms: None,
            })?;
        }
        Ok(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn an_uncertain_first_send_reuses_its_command_and_rejects_changed_text() {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(openagents_sync::DocsStore::open(root.path()).unwrap());
        let host = DocHost::new(
            store,
            DocHostConfig {
                device_id: "fixture-mac".into(),
                default_harness: HarnessId::Mock,
                edge: None,
            },
        );
        let chat = "d75ef8a1-16ca-49b6-83be-000000000002";
        let source = Source {
            account: "fixture-account".into(),
            chat: "d75ef8a1-16ca-49b6-83be-000000000001".into(),
            revision: 1,
        };
        let mut options = serde_json::Map::new();
        options.insert(
            chat_home::REQUEST_OPTION.into(),
            serde_json::to_value(source).unwrap(),
        );
        let payload = SessionCommandPayload::Run {
            request: openagents_proto::RunRequest {
                prompt: "Continue the earlier question".into(),
                harness: Some(HarnessId::Mock),
                model: None,
                reasoning: None,
                model_options: options,
                cwd: root.path().to_string_lossy().into(),
                sandbox: openagents_proto::SandboxLevel::WorkspaceWrite,
                auto_approve: false,
                resume: None,
                attachments: vec![],
                worktree: None,
                mcp: None,
            },
            message_id: "d75ef8a1-16ca-49b6-83be-000000000003".into(),
        };
        let first = host.queue_command(chat, payload.clone()).unwrap();
        assert_eq!(host.queue_command(chat, payload.clone()).unwrap(), first);
        assert_eq!(
            host.open(chat)
                .unwrap()
                .doc()
                .read_commands()
                .unwrap()
                .len(),
            1
        );
        let mut changed = payload.clone();
        if let SessionCommandPayload::Run { request, .. } = &mut changed {
            request.prompt = "Different text".into();
        }
        assert!(host.queue_command(chat, changed).is_err());
        if let SessionCommandPayload::Run { request, .. } = payload {
            assert!(
                host.prepare_continuation(chat, HarnessId::Mock, request)
                    .await
                    .is_err()
            );
            assert!(openagents_doc::continuation::get(host.open(chat).unwrap().doc()).is_none());
        }
    }
}
