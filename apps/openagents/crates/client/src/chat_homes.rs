//! Continuations use the same registry and admitted host RPC as normal sessions.
use super::*;
use chat_home::{Continued, PartRef, Source};

impl Client {
    fn continuation_row_ready(&self, id: String) -> Result<String> {
        if let Some(live) = self.inner.live() {
            live.persist_registry(&self.inner)?;
        }
        Ok(id)
    }
    /// Bind account actions to this login, including its replacement token.
    pub fn uses_account(&self, account: &str, token: &str) -> bool {
        matches!(&self.inner.credentials, Credentials::OpenAgents { account_id, bearer, .. }
            if account_id == account && bearer == token)
    }

    pub fn session_target_device(&self, target: &SessionTarget) -> Option<String> {
        match target {
            SessionTarget::Projectless { device_id } => Some(device_id.clone()),
            SessionTarget::Project { space_id } => self
                .inner
                .workspace
                .state()
                .0
                .spaces
                .iter()
                .find(|s| &s.id == space_id)
                .map(|s| s.device_id.clone()),
        }
    }

    pub fn chat_record(&self, id: &str) -> Option<Chat> {
        self.inner
            .workspace
            .state()
            .0
            .chats
            .iter()
            .find(|c| c.id == id)
            .cloned()
    }

    pub fn continuation_hosts(&self) -> Vec<openagents_proto::Device> {
        self.inner
            .workspace
            .state()
            .0
            .devices
            .iter()
            .filter(|d| !matches!(d.platform.as_str(), "ios" | "android" | "ipados"))
            .filter(|d| d.supports(openagents_proto::capabilities::OPENAGENTS_CONTINUATIONS_V1))
            .cloned()
            .collect()
    }

    /// Only a reference enters the command. The destination host fetches
    /// context through its own current account before running a harness.
    pub fn create_continuation_session(
        &self,
        source: &Source,
        id: String,
        mut new: NewSession,
    ) -> Result<String> {
        if source.account.is_empty()
            || source.account.len() > 128
            || !chat_home::valid_id(&source.chat)
            || source.revision == 0
        {
            return Err(ClientError::InvalidArgument(
                "Open the earlier chat again.".into(),
            ));
        }
        if let Some(previous) = self.chat_record(&id) {
            let same_place = match &new.target {
                SessionTarget::Project { space_id } => previous.space_id.as_ref() == Some(space_id),
                SessionTarget::Projectless { device_id } => {
                    previous.space_id.is_none() && previous.device_id == *device_id
                }
            };
            if previous
                .links
                .continues
                .as_ref()
                .is_some_and(|p| p.id == source.chat)
            {
                return if same_place {
                    self.continuation_row_ready(id)
                } else {
                    Err(ClientError::InvalidArgument(
                        "Open the latest part of this chat.".into(),
                    ))
                };
            }
        }
        let config = new.config.as_mut().ok_or_else(|| {
            ClientError::InvalidArgument("Choose an agent before continuing.".into())
        })?;
        config.model_options.insert(
            chat_home::REQUEST_OPTION.into(),
            serde_json::to_value(source)
                .map_err(|_| ClientError::InvalidArgument("Open the earlier chat again.".into()))?,
        );
        self.continuation_row_ready(self.create_session_at(id, new)?)
    }

    pub async fn continue_session(
        &self,
        account: &str,
        id: &str,
        target: PartRef,
    ) -> Result<Continued> {
        let source = self
            .chat_record(id)
            .ok_or_else(|| ClientError::NotFound("Open the earlier chat again.".into()))?;
        if !self.inner.workspace.state().0.devices.iter().any(|d| {
            d.id == source.device_id
                && d.supports(openagents_proto::capabilities::CHAT_HOME_TRANSFER_V1)
        }) {
            return Err(ClientError::Unsupported(
                "Update OpenAgents on this computer before moving the chat.".into(),
            ));
        }
        if !target.valid() || target.id == id || account.is_empty() || account.len() > 128 {
            return Err(ClientError::InvalidArgument(
                "Choose another place to continue this chat.".into(),
            ));
        }
        let value = self
            .inner
            .host_rpc(
                &source.device_id,
                "ContinueChat",
                serde_json::json!({
                    "account":account,"chatId":id,"target":target,
                }),
            )
            .await?;
        let continued: Continued = serde_json::from_value(value).map_err(|_| {
            ClientError::HostError("Couldn't confirm where this chat continues.".into())
        })?;
        if continued.account != account
            || continued.source.id != id
            || continued.source.home != chat_home::Home::Computer(source.device_id)
            || continued.target.home != target.home
            || !continued.target.valid()
            || !continued.seed.valid()
        {
            return Err(ClientError::HostError(
                "Couldn't confirm where this chat continues.".into(),
            ));
        }
        Ok(continued)
    }

    pub async fn prepare_continuation_session(&self, source: &Source, id: &str) -> Result<PartRef> {
        if source.account != self.user_id() {
            return Err(ClientError::Auth("Log in again to continue.".into()));
        }
        let row = self
            .chat_record(id)
            .ok_or_else(|| ClientError::NotFound("Open the new chat again.".into()))?;
        let target = PartRef {
            id: id.into(),
            home: chat_home::Home::Computer(row.device_id.clone()),
        };
        let value = self
            .inner
            .host_rpc(
                &row.device_id,
                "PrepareContinuation",
                serde_json::json!({"chatId":id,"source":source}),
            )
            .await?;
        let ready: PartRef = serde_json::from_value(value).map_err(|_| {
            ClientError::HostError("Couldn't prepare this chat on that computer.".into())
        })?;
        if ready != target {
            return Err(ClientError::HostError(
                "Couldn't prepare this chat on that computer.".into(),
            ));
        }
        Ok(ready)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn continuation_destination_retries_keep_one_row_and_old_parts_reject_sends() {
        let root = tempfile::tempdir().unwrap();
        let client = Client::new(
            ClientConfig::new("http://127.0.0.1:1", root.path()),
            Credentials::Demo(Default::default()),
            Arc::new(crate::events::NullListener),
        )
        .unwrap();
        let device = client
            .inner
            .workspace
            .state()
            .0
            .devices
            .iter()
            .find(|d| !matches!(d.platform.as_str(), "ios" | "android" | "ipados"))
            .unwrap()
            .id
            .clone();
        let source = Source {
            account: "fixture".into(),
            chat: "a1000000-0000-4000-8000-000000000001".into(),
            revision: 1,
        };
        let id = "a1000000-0000-4000-8000-000000000002";
        let options = NewSession {target: SessionTarget::Projectless {device_id: device},
            config: Some(serde_json::from_value(serde_json::json!({"harness":"mock","sandbox":"workspace-write","modelOptions":{}})).unwrap()),
            branch: None, cwd: None, title: None};
        assert_eq!(
            client
                .create_continuation_session(&source, id.into(), options.clone())
                .unwrap(),
            id
        );
        let first = client.chat_record(id).unwrap();
        assert!(
            !client
                .workspace()
                .front
                .recent
                .iter()
                .any(|row| row.id == id)
        );
        assert_eq!(
            client
                .create_continuation_session(&source, id.into(), options)
                .unwrap(),
            id
        );
        assert_eq!(client.chat_record(id).unwrap(), first);
        let mut config = first.config.unwrap();
        assert_eq!(
            serde_json::from_value::<Source>(
                config.model_options[chat_home::REQUEST_OPTION].clone()
            )
            .unwrap(),
            source
        );
        config.model_options.remove(chat_home::REQUEST_OPTION);
        client
            .inner
            .registry_write(|doc| {
                doc.set_chat_continuation(
                    id,
                    &PartRef {
                        id: source.chat.clone(),
                        home: chat_home::Home::Cloud,
                    },
                )?;
                doc.set_chat_config(id, &config)
            })
            .unwrap();
        assert_eq!(
            client
                .workspace()
                .front
                .recent
                .iter()
                .filter(|row| row.id == id)
                .count(),
            1
        );
        assert_eq!(
            client.workspace().session(id).unwrap().preview.as_deref(),
            Some("Continued chat")
        );
        client
            .inner
            .registry_write(|doc| {
                doc.set_chat_destination(
                    id,
                    &PartRef {
                        id: "a1000000-0000-4000-8000-000000000003".into(),
                        home: chat_home::Home::Cloud,
                    },
                )
            })
            .unwrap();
        assert!(
            !client
                .workspace()
                .front
                .recent
                .iter()
                .any(|row| row.id == id)
        );
        let session = client.open_session(id).unwrap();
        assert!(matches!(
            session.send(crate::SendRequest {
                text: "Keep this draft".into(),
                attachments: vec![],
                worktree: None,
                busy: crate::BusyPolicy::Queue
            }),
            Err(ClientError::InvalidArgument(_))
        ));
        assert!(session.composer().pending_sends.is_empty());
        client.shutdown();
    }
}
