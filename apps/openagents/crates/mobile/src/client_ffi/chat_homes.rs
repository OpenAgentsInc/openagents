//! Shared chat home controls and account-bound continuation drafts.
use super::{CoreAccount, CoreClient, CoreError, CoreResult, CoreSavedHistory, NewSession};
use chat_home::{Home, Links, PartRef, Source, view::Destination};
use openagents_account::{Lease, history::ExistingChat, reader::Reader};
use rust_native::{Activation, ValidatedView};
use std::sync::{Arc, Mutex};

fn failed(message: &str) -> CoreError {
    CoreError::InvalidArgument {
        message: message.into(),
    }
}
fn current(lease: &Lease, client: &openagents_client::Client) -> CoreResult<()> {
    if lease.is_current()
        && client.uses_account(
            &lease.session().account().id,
            lease.session().bearer_token(),
        )
    {
        Ok(())
    } else {
        Err(CoreError::Auth {
            message: "Log in again to continue.".into(),
        })
    }
}

enum Origin {
    Computer(String),
    Saved {
        reader: Arc<Reader>,
        target: ExistingChat,
        turns: Arc<openagents_account::turns::Turns>,
    },
}
struct Controls {
    fingerprint: String,
    revision: u64,
    choices: Option<ValidatedView<Home>>,
    parts: Option<ValidatedView<PartRef>>,
    targets: Vec<(Home, String)>,
}
#[derive(uniffi::Object)]
pub struct CoreChatHomes {
    lease: Lease,
    client: openagents_client::Client,
    origin: Origin,
    instance: String,
    controls: Mutex<Controls>,
}
#[derive(uniffi::Enum)]
pub enum ChatHomeAction {
    OpenComputer { id: String },
    OpenSaved { id: String },
    Draft { draft: Arc<CoreContinuationDraft> },
}
#[derive(uniffi::Object)]
pub struct CoreContinuationDraft {
    lease: Lease,
    source: Source,
    native: Option<String>,
    device: String,
    id: String,
    creating: tokio::sync::Mutex<()>,
}
impl CoreChatHomes {
    fn new(lease: Lease, client: &CoreClient, origin: Origin) -> CoreResult<Arc<Self>> {
        current(&lease, client.client())?;
        Ok(Arc::new(Self {
            lease,
            client: client.client().clone(),
            origin,
            instance: format!("chat-homes-{}", uuid::Uuid::new_v4()),
            controls: Mutex::new(Controls {
                fingerprint: String::new(),
                revision: 0,
                choices: None,
                parts: None,
                targets: vec![],
            }),
        }))
    }
    fn source(&self) -> CoreResult<(Source, Home, Links, bool)> {
        current(&self.lease, &self.client)?;
        let account = self.lease.session().account().id.clone();
        match &self.origin {
            Origin::Computer(id) => {
                let row = self
                    .client
                    .chat_record(id)
                    .ok_or_else(|| failed("Open the chat again."))?;
                let busy = self.client.reported_sessions().iter().any(|s| {
                    s.chat_id == *id
                        && matches!(
                            s.status,
                            openagents_proto::SessionStatus::Working
                                | openagents_proto::SessionStatus::AwaitingInput
                        )
                });
                let supported = self.client.continuation_hosts().iter().any(|d| {
                    d.id == row.device_id
                        && d.supports(openagents_proto::capabilities::CHAT_HOME_TRANSFER_V1)
                });
                Ok((
                    Source {
                        account,
                        chat: id.clone(),
                        revision: row
                            .last_message_at
                            .unwrap_or(row.created_at)
                            .timestamp()
                            .max(1) as u64,
                    },
                    Home::Computer(row.device_id),
                    row.links,
                    busy || !supported,
                ))
            }
            Origin::Saved {
                reader,
                target,
                turns,
            } => {
                if reader.target().is_none_or(|t| t.chat != target.chat) {
                    return Err(failed("Open the chat again."));
                }
                let snapshot = reader.snapshot();
                let row = snapshot.summary.unwrap_or_else(|| target.summary.clone());
                Ok((
                    Source {
                        account,
                        chat: row.chat.id().into(),
                        revision: row.revision,
                    },
                    row.home,
                    row.links,
                    turns.snapshot().busy || snapshot.access_lost || snapshot.login_required,
                ))
            }
        }
    }
    fn update(&self) -> CoreResult<()> {
        let (source, home, links, busy) = self.source()?;
        let native =
            matches!(&home, Home::Computer(device) if device != &format!("legacy-{}", source.chat));
        let hosts = self.client.continuation_hosts();
        let workspace = self.client.workspace();
        let label = match &home {
            Home::Cloud => "OpenAgents".into(),
            Home::Computer(device) => hosts
                .iter()
                .find(|d| &d.id == device)
                .map(|d| d.name.clone())
                .unwrap_or_else(|| "Computer".into()),
        };
        let mut destinations = if home != Home::Cloud {
            vec![Destination {
                home: Home::Cloud,
                label: "OpenAgents".into(),
                available: true,
            }]
        } else {
            vec![]
        };
        destinations.extend(
            hosts
                .into_iter()
                .filter(|d| {
                    !native || d.supports(openagents_proto::capabilities::CHAT_HOME_TRANSFER_V1)
                })
                .map(|d| {
                    let available = workspace.device(&d.id).is_some_and(|row| row.online);
                    Destination {
                        home: Home::Computer(d.id),
                        label: if available {
                            d.name
                        } else {
                            format!("{} (offline)", d.name)
                        },
                        available,
                    }
                }),
        );
        let blocked = if links.continued_by.is_some() {
            Some("Open the latest part to reply.")
        } else if busy {
            Some("Stop the answer and clear queued messages before moving this chat.")
        } else if source.revision == 0 {
            Some("Wait for the chat to finish loading.")
        } else {
            None
        };
        let fingerprint = serde_json::to_string(&(
            &source,
            &home,
            &links,
            blocked,
            destinations
                .iter()
                .map(|d| (&d.home, &d.label, d.available))
                .collect::<Vec<_>>(),
        ))
        .unwrap();
        let mut controls = self.controls.lock().unwrap();
        if controls.fingerprint == fingerprint {
            return Ok(());
        }
        controls.revision += 1;
        controls.choices = Some(
            chat_home::view::choices(
                &self.instance,
                controls.revision,
                &Destination {
                    home,
                    label,
                    available: true,
                },
                &destinations,
                blocked,
            )
            .map_err(|_| failed("Couldn't show where this chat runs."))?,
        );
        controls.parts = Some(
            chat_home::view::parts(
                &format!("{}-parts", self.instance),
                controls.revision,
                &links,
            )
            .map_err(|_| failed("Couldn't show the parts of this chat."))?,
        );
        controls.fingerprint = fingerprint;
        Ok(())
    }
    fn navigation(&self, part: PartRef) -> CoreResult<ChatHomeAction> {
        match &part.home {
            Home::Computer(device) if device != &format!("legacy-{}", part.id) => {
                if self
                    .client
                    .chat_record(&part.id)
                    .is_none_or(|row| row.device_id != *device)
                {
                    return Err(failed(
                        "Connect the computer that has this part of the chat.",
                    ));
                }
                Ok(ChatHomeAction::OpenComputer { id: part.id })
            }
            _ => Ok(ChatHomeAction::OpenSaved { id: part.id }),
        }
    }
}
#[uniffi::export]
impl CoreChatHomes {
    pub fn choices(&self) -> CoreResult<String> {
        self.update()?;
        serde_json::to_string(
            self.controls
                .lock()
                .unwrap()
                .choices
                .as_ref()
                .unwrap()
                .view(),
        )
        .map_err(|_| failed("Couldn't show where this chat runs."))
    }
    pub fn parts(&self) -> CoreResult<String> {
        self.update()?;
        serde_json::to_string(self.controls.lock().unwrap().parts.as_ref().unwrap().view())
            .map_err(|_| failed("Couldn't show the parts of this chat."))
    }
    pub async fn activate(
        &self,
        instance: String,
        revision: u64,
        node: String,
    ) -> CoreResult<ChatHomeAction> {
        self.update()?;
        let activation = Activation {
            instance,
            revision,
            node,
        };
        let (home, id) = {
            let mut controls = self.controls.lock().unwrap();
            if let Ok(part) = controls.parts.as_ref().unwrap().activate(&activation) {
                return self.navigation(part.clone());
            }
            let home = controls
                .choices
                .as_ref()
                .unwrap()
                .activate(&activation)
                .map_err(|_| failed("Open the chat options again."))?
                .clone();
            let id = if let Some((_, id)) = controls.targets.iter().find(|(h, _)| h == &home) {
                id.clone()
            } else {
                let id = uuid::Uuid::new_v4().to_string();
                controls.targets.push((home.clone(), id.clone()));
                id
            };
            (home, id)
        };
        let (source, source_home, _, _) = self.source()?;
        let native = match &source_home {
            Home::Computer(device) if device != &format!("legacy-{}", source.chat) => {
                Some(source.chat.clone())
            }
            _ => None,
        };
        if let Home::Computer(device) = home {
            return Ok(ChatHomeAction::Draft {
                draft: Arc::new(CoreContinuationDraft {
                    lease: self.lease.clone(),
                    source,
                    native,
                    device,
                    id,
                    creating: tokio::sync::Mutex::new(()),
                }),
            });
        }
        let client = self.client.clone();
        let lease = self.lease.clone();
        let receipt = openagents_client::runtime::shared()
            .spawn(async move {
                current(&lease, &client)?;
                let target = PartRef {
                    id,
                    home: Home::Cloud,
                };
                let result = if let Some(native) = native {
                    client
                        .continue_session(&source.account, &native, target)
                        .await?
                } else {
                    let chat = lease
                        .client()
                        .chat(lease.session(), &source.chat)
                        .map_err(|_| failed("Open the earlier chat again."))?;
                    openagents_account::openagents::continuations::transfer(
                        lease.client(),
                        lease.session(),
                        &chat,
                        &chat_home::TransferClaim {
                            revision: source.revision,
                            target,
                            seed: None,
                        },
                        &lease.cancel,
                    )
                    .await
                    .map_err(|_| failed("Couldn't move this chat. Open it again and try again."))?
                };
                current(&lease, &client)?;
                Ok::<_, CoreError>(result)
            })
            .await
            .map_err(|_| failed("Couldn't move this chat."))??;
        self.navigation(receipt.target)
    }
}
#[uniffi::export]
impl CoreContinuationDraft {
    pub fn device_id(&self) -> String {
        self.device.clone()
    }
    pub async fn start(
        &self,
        core: Arc<CoreClient>,
        new_session: NewSession,
        request: super::SendRequest,
    ) -> CoreResult<String> {
        if request.text.trim().is_empty() && request.attachments.is_empty() {
            return Err(failed("Write a message before continuing."));
        }
        let id = self.create(core.clone(), new_session).await?;
        current(&self.lease, core.client())?;
        core.client().open_session(&id)?.send(request.into())?;
        Ok(id)
    }
    /// Prepare a durable destination before freezing the earlier computer part.
    pub async fn create(
        &self,
        core: Arc<CoreClient>,
        new_session: NewSession,
    ) -> CoreResult<String> {
        let _guard = self
            .creating
            .try_lock()
            .map_err(|_| failed("Wait for the message to finish sending."))?;
        current(&self.lease, core.client())?;
        let new: openagents_client::NewSession = new_session.try_into()?;
        if core.client().session_target_device(&new.target).as_deref() != Some(&self.device) {
            return Err(failed("Choose a folder on the selected computer."));
        }
        if !core.client().continuation_hosts().iter().any(|d| {
            d.id == self.device
                && (self.native.is_none()
                    || d.supports(openagents_proto::capabilities::CHAT_HOME_TRANSFER_V1))
        }) {
            return Err(failed("Update OpenAgents on the selected computer."));
        }
        let id = core
            .client()
            .create_continuation_session(&self.source, self.id.clone(), new)?;
        if let Some(source) = &self.native {
            let client = core.client().clone();
            let lease = self.lease.clone();
            let account = self.source.account.clone();
            let reference = self.source.clone();
            let source = source.clone();
            let target = PartRef {
                id: id.clone(),
                home: Home::Computer(self.device.clone()),
            };
            openagents_client::runtime::shared()
                .spawn(async move {
                    current(&lease, &client)?;
                    client
                        .prepare_continuation_session(&reference, &target.id)
                        .await?;
                    current(&lease, &client)?;
                    let receipt = client
                        .continue_session(&account, &source, target.clone())
                        .await?;
                    current(&lease, &client)?;
                    if receipt.target != target {
                        return Err(failed("Open the latest part of this chat."));
                    }
                    Ok::<_, CoreError>(())
                })
                .await
                .map_err(|_| failed("Couldn't move this chat."))??;
        }
        current(&self.lease, core.client())?;
        Ok(id)
    }
}
#[uniffi::export]
impl CoreAccount {
    pub fn chat_homes(
        &self,
        client: Arc<CoreClient>,
        chat: String,
    ) -> CoreResult<Arc<CoreChatHomes>> {
        let lease = self
            .controller()
            .lease()
            .ok_or_else(|| failed("Log in to OpenAgents."))?;
        CoreChatHomes::new(lease, &client, Origin::Computer(chat))
    }
}
#[uniffi::export]
impl CoreClient {
    pub fn chat_read_only(&self, id: String) -> bool {
        self.client()
            .chat_record(&id)
            .is_some_and(|row| row.links.continued_by.is_some())
    }
}
#[uniffi::export]
impl CoreSavedHistory {
    /// Open an account-scoped part that need not be in the current list page.
    pub fn open_chat_part(&self, id: String) -> CoreResult<()> {
        use openagents_account::openagents::{ChatSource, ChatSummary};
        if !self.lease.is_current() || !chat_home::valid_id(&id) {
            return Err(failed("Open the chat again."));
        }
        let chat = self
            .lease
            .client()
            .chat(self.lease.session(), &id)
            .map_err(|_| failed("Open the chat again."))?;
        self.turns.clear();
        self.reader()
            .open_latest(ExistingChat {
                lease: self.lease.clone(),
                chat: chat.clone(),
                summary: ChatSummary {
                    chat,
                    home: Home::Cloud,
                    links: Links::default(),
                    title: "Chat".into(),
                    revision: 0,
                    updated_unix: 0,
                    pinned_unix: None,
                    archived_unix: None,
                    project: None,
                    source: ChatSource::Other("part".into()),
                    messages: 0,
                },
            })
            .map_err(|_| failed("Couldn't open this part of the chat."))?;
        openagents_client::runtime::shared().spawn(self.reader().more());
        Ok(())
    }

    pub fn chat_homes(&self, client: Arc<CoreClient>) -> CoreResult<Arc<CoreChatHomes>> {
        let target = self
            .reader()
            .target()
            .ok_or_else(|| failed("Open a chat first."))?;
        CoreChatHomes::new(
            self.lease.clone(),
            &client,
            Origin::Saved {
                reader: self.reader().clone(),
                target,
                turns: self.turns.clone(),
            },
        )
    }
}
