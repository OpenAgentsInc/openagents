//! Sequential chamber client with committed control and no uncertain command replay.
use std::time::Duration;
#[cfg(feature = "service-net")]
use std::{net::SocketAddr, sync::Arc};

use super::client_runtime::{self, timeout};
#[cfg(feature = "service-net")]
use rustls::{ClientConfig, pki_types::ServerName};
use secp256k1::{Keypair, Secp256k1};
use tokio::io::AsyncWriteExt;
#[cfg(feature = "service-net")]
use tokio::net::TcpStream;
#[cfg(feature = "service-net")]
use tokio_rustls::TlsConnector;

use super::{
    transport::{Transport, read_frame, write_frame, write_frame_batch},
    wire::{
        Body, Control, EventPage, Hello, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, Reply, Request,
        Response, State, VERSION,
    },
};
use crate::{Command, Intent, play::Ability};
const DEADLINE: Duration = Duration::from_secs(10);

/// Owns one verified transport and the last acknowledged player control state.
///
/// Refresh snapshots at the replication cadence before generating input. This
/// client does not predict ticks, retain secret keys, or retry uncertain commands.
/// Convenience methods retry explicit storage refusals for up to ten seconds;
/// `request` exposes each refusal directly.
pub struct Client {
    public_key: [u8; 32],
    stream: Option<Box<dyn Transport>>,
    instance: u64,
    tick: u64,
    control: Option<Control>,
    next_request: u64,
    logged_in: bool,
    player: bool,
    inventory_revision: u64,
    verified_at: Option<web_time::Instant>,
    replication: super::replication::Receiver,
}
impl Client {
    #[cfg(feature = "service-net")]
    pub async fn connect(
        address: SocketAddr,
        server_name: ServerName<'static>,
        tls: Arc<ClientConfig>,
        instance: u64,
        key: &Keypair,
    ) -> Result<Self, String> {
        Self::connect_with_content(address, server_name, tls, instance, None, key).await
    }
    /// Refuses differing configured content before producing an authentication signature.
    #[cfg(feature = "service-net")]
    pub async fn connect_with_content(
        address: SocketAddr,
        server_name: ServerName<'static>,
        tls: Arc<ClientConfig>,
        instance: u64,
        content: Option<[u8; 32]>,
        key: &Keypair,
    ) -> Result<Self, String> {
        let stream = timeout(DEADLINE, async {
            let socket = TcpStream::connect(address)
                .await
                .map_err(|_| "Cannot connect to chamber")?;
            socket
                .set_nodelay(true)
                .map_err(|_| "Cannot configure chamber client socket")?;
            TlsConnector::from(tls)
                .connect(server_name, socket)
                .await
                .map_err(|_| "Chamber TLS identity refused".to_string())
        })
        .await
        .map_err(|_| "Chamber connection timed out")??;
        Self::connect_stream(Box::new(stream), instance, content, key).await
    }
    /// Authenticates over a transport that already proved the host's identity.
    pub async fn connect_stream(
        mut stream: Box<dyn Transport>,
        instance: u64,
        content: Option<[u8; 32]>,
        key: &Keypair,
    ) -> Result<Self, String> {
        let hello = timeout(DEADLINE, async {
            let bytes = read_frame(&mut stream, MAX_RESPONSE_BYTES).await?;
            let hello: Hello = serde_json::from_slice(&bytes)
                .map_err(|_| "Malformed chamber opening challenge")?;
            if hello.version != VERSION || hello.challenge.instance() != instance {
                return Err("Chamber opening version or instance mismatch".to_string());
            }
            if hello.challenge.content() != content {
                return Err("Chamber scene or asset content mismatch".into());
            }
            Ok::<_, String>(hello)
        })
        .await
        .map_err(|_| "Chamber connection timed out")??;
        let public_key = key.x_only_public_key().0.serialize();
        let mut entropy = [0; 32];
        getrandom::fill(&mut entropy).map_err(|_| "Cannot generate client signing entropy")?;
        let signature = Secp256k1::new()
            .sign_schnorr_with_aux_rand(&hello.challenge.signing_digest(public_key), key, &entropy)
            .to_byte_array()
            .to_vec();
        let mut client = Self {
            public_key,
            stream: Some(stream),
            instance,
            tick: 0,
            control: None,
            next_request: 1,
            logged_in: false,
            player: false,
            inventory_revision: 0,
            verified_at: None,
            replication: Default::default(),
        };
        let response = client
            .request_ready(Body::Authenticate {
                public_key,
                signature,
            })
            .await?;
        if !matches!(response.body, Reply::Accepted) {
            return Err("Chamber authentication refused".into());
        }
        client.logged_in = true;
        client.player = client.control.is_some();
        Ok(client)
    }
    pub fn connected(&self) -> bool {
        self.stream.is_some()
    }
    pub fn instance(&self) -> u64 {
        self.instance
    }
    pub fn tick(&self) -> u64 {
        self.tick
    }
    pub fn control(&self) -> Option<&Control> {
        self.stream.as_ref().and(self.control.as_ref())
    }

    /// Returns gameplay refusals as responses; transport/protocol errors close IO.
    /// Cancelling this future drops its taken socket, preventing implicit replay.
    pub async fn request(&mut self, body: Body) -> Result<Response, String> {
        let request_id = self.next_request;
        let next = request_id
            .checked_add(1)
            .ok_or("Client request identities exhausted")?;
        let bytes = serde_json::to_vec(&Request {
            version: VERSION,
            request_id,
            body: body.clone(),
        })
        .map_err(|_| "Cannot encode chamber request")?;
        Request::decode(&bytes)?;
        let mut stream = self.stream.take().ok_or("Chamber client is disconnected")?;
        self.next_request = next;
        let mut response = timeout(DEADLINE, async {
            write_frame(&mut stream, &bytes, MAX_REQUEST_BYTES).await?;
            let bytes = read_frame(&mut stream, MAX_RESPONSE_BYTES).await?;
            serde_json::from_slice::<Response>(&bytes)
                .map_err(|_| "Malformed chamber response".to_string())
        })
        .await
        .map_err(|_| "Chamber request timed out")??;
        self.reconstruct(&body, &mut response)?;
        self.validate(request_id, &body, &response)?;
        self.tick = response.tick;
        self.verified_at = Some(web_time::Instant::now());
        if let Reply::Inventory { inventory } = &response.body {
            self.inventory_revision = inventory.revision;
        }
        if matches!(response.body, Reply::CharacterSelected { .. }) {
            self.player = true;
            self.replication.clear();
        }
        let lost_control = self.player && response.control.is_none();
        self.control = response.control.clone();
        if !lost_control {
            self.stream = Some(stream);
        }
        Ok(response)
    }
    /// Retry only an explicit refusal before admission, keeping every operation field.
    async fn request_ready(&mut self, body: Body) -> Result<Response, String> {
        timeout(DEADLINE, async {
            loop {
                let response = self.request(body.clone()).await?;
                if !matches!(&response.body, Reply::Refused {code, ..} if code == "storage_busy") {
                    return Ok(response);
                }
                client_runtime::sleep(Duration::from_millis(33)).await;
            }
        })
        .await
        .map_err(|_| "Chamber storage admission timed out".to_string())?
    }
    pub async fn inventory(&mut self) -> Result<super::wire::Inventory, String> {
        match self.request_ready(Body::Inventory {}).await?.body {
            Reply::Inventory { inventory } => Ok(inventory),
            Reply::Refused { message, .. } => Err(message),
            _ => Err("Unexpected inventory response".into()),
        }
    }
    pub async fn equip_gear(
        &mut self,
        slot: super::equipment::Slot,
        item: u64,
        operation: [u8; 16],
    ) -> Result<Response, String> {
        let control = self.control().ok_or("Client has no admitted adventurer")?;
        self.request_ready(Body::EquipGear {
            life: control.life,
            epoch: control.epoch,
            slot,
            item,
            operation,
        })
        .await
    }
    pub async fn equip_outfit(
        &mut self,
        outfit: u64,
        operation: [u8; 16],
    ) -> Result<Response, String> {
        let control = self.control().ok_or("Client has no admitted adventurer")?;
        self.request_ready(Body::EquipOutfit {
            life: control.life,
            epoch: control.epoch,
            outfit,
            operation,
        })
        .await
    }
    /// Callers retain the operation ID when an acknowledgment is uncertain.
    pub async fn use_item(&mut self, item: u64, operation: [u8; 16]) -> Result<Response, String> {
        let control = self.control().ok_or("Client has no admitted adventurer")?;
        self.request_ready(Body::UseItem {
            life: control.life,
            epoch: control.epoch,
            item,
            operation,
        })
        .await
    }
    pub async fn accept_quest(
        &mut self,
        quest: u64,
        giver: verse_engine::core::LifeId,
    ) -> Result<Response, String> {
        let control = self.control().ok_or("Client has no admitted adventurer")?;
        self.request_ready(Body::AcceptQuest {
            life: control.life,
            epoch: control.epoch,
            quest,
            giver: giver.into(),
        })
        .await
    }
    pub async fn services(&mut self, character: u64) -> Result<super::game_services::View, String> {
        match self.request_ready(Body::Services { character }).await?.body {
            Reply::Services { view } => Ok(view),
            Reply::Refused { message, .. } => Err(message),
            _ => Err("Realm service view is missing".into()),
        }
    }
    pub async fn safety(&mut self) -> Result<super::safety::View, String> {
        match self.request_ready(Body::Safety {}).await?.body {
            Reply::Safety { view } => Ok(view),
            Reply::Refused { message, .. } => Err(message),
            _ => Err("Account safety view is missing".into()),
        }
    }
    pub async fn safety_action(
        &mut self,
        realm: [u8; 32],
        operation: [u8; 16],
        action: super::safety::Action,
    ) -> Result<super::safety::Receipt, String> {
        match self
            .request_ready(Body::SafetyAction {
                realm,
                operation,
                action,
            })
            .await?
            .body
        {
            Reply::SafetyApplied { receipt } => Ok(receipt),
            Reply::Refused { message, .. } => Err(message),
            _ => Err("Account safety receipt is missing".into()),
        }
    }
    pub async fn service_action(
        &mut self,
        realm: [u8; 32],
        character: u64,
        operation: [u8; 16],
        action: super::game_services::Action,
    ) -> Result<super::game_services::Receipt, String> {
        match self
            .request_ready(Body::ServiceAction {
                realm,
                character,
                operation,
                action,
            })
            .await?
            .body
        {
            Reply::ServiceApplied { receipt } => Ok(receipt),
            Reply::Refused { message, .. } => Err(message),
            _ => Err("Realm service receipt is missing".into()),
        }
    }
    pub async fn quest_cycle(
        &mut self,
        quest: u64,
        cycle: u64,
        action: super::progression::Action,
    ) -> Result<Response, String> {
        let control = self.control().ok_or("Client has no admitted adventurer")?;
        self.request_ready(Body::QuestCycle {
            life: control.life,
            epoch: control.epoch,
            quest,
            cycle,
            action,
        })
        .await
    }
    pub async fn claim_quest(&mut self, quest: u64) -> Result<Response, String> {
        let control = self.control().ok_or("Client has no admitted adventurer")?;
        self.request_ready(Body::ClaimQuest {
            life: control.life,
            epoch: control.epoch,
            quest,
        })
        .await
    }
    /// Binds an input to the current acknowledged control without sending it.
    pub fn prepare_command(&self, intent: Intent<Ability>) -> Result<Command<Ability>, String> {
        let control = self.control().ok_or("Client has no admitted adventurer")?;
        Ok(Command {
            actor: control.life.into(),
            epoch: control.epoch,
            sequence: control
                .accepted_sequence
                .checked_add(1)
                .ok_or("Client command sequence exhausted")?,
            tick: self.tick,
            intent,
        })
    }
    pub async fn begin_movement_frames(&mut self) -> Result<Response, String> {
        let control = self
            .control()
            .ok_or("Client has no admitted adventurer")?
            .clone();
        self.request_ready(Body::BeginMovementFrames {
            life: control.life,
            epoch: control.epoch,
        })
        .await
    }
    pub async fn movement_frame(
        &mut self,
        mut frame: crate::movement::frames::Frame,
    ) -> Result<Response, String> {
        frame.validate_payload()?;
        let command = self.prepare_command(Intent::Jump)?;
        if frame.life != command.actor || frame.epoch != command.epoch {
            return Err("Movement interval control changed".into());
        }
        frame.sequence = command.sequence;
        frame.tick = command.tick;
        self.request_ready(Body::MovementFrame { frame }).await
    }
    pub async fn social(
        &mut self,
        action: crate::play::social::Action,
    ) -> Result<Response, String> {
        timeout(DEADLINE, async {
            for attempt in 0..3 {
                let command = self.prepare_command(Intent::Jump)?;
                let response = self.request_ready(Body::Social { input: crate::play::social::Input { life: command.actor, epoch: command.epoch, sequence: command.sequence, tick: command.tick, action } }).await?;
                let unadmitted = matches!(&response.body, Reply::Refused { code, .. } if code == "stale_tick") && response.control.as_ref().is_some_and(|c| c.life == command.actor.into() && c.epoch == command.epoch && c.accepted_sequence < command.sequence);
                if !unadmitted || attempt == 2 { return Ok(response); }
            }
            unreachable!("Social command attempts are bounded")
        }).await.map_err(|_| "Social command timed out".to_string())?
    }
    pub async fn command(&mut self, intent: Intent<Ability>) -> Result<Response, String> {
        timeout(DEADLINE,async {
            for attempt in 0..3 {
                let command=self.prepare_command(intent.clone())?;
                let response=self.request_ready(Body::Command{command:command.clone().into()}).await?;
                let unadmitted=matches!(&response.body,Reply::Refused{code,..} if code=="stale_tick")
                    && response.control.as_ref().is_some_and(|c|c.life==command.actor.into() && c.epoch==command.epoch && c.accepted_sequence<command.sequence);
                if !unadmitted || attempt==2 {return Ok(response);}
            }
            unreachable!("Command attempts are bounded")
        }).await.map_err(|_|"Chamber command timed out".to_string())?
    }
    /// Reconstructs a validated replication packet and retains its response context.
    pub async fn replicated_snapshot(&mut self) -> Result<Response, String> {
        self.request_ready(Body::Replicate {
            ack: self.replication.ack(),
        })
        .await
    }
    pub async fn snapshot(&mut self) -> Result<State, String> {
        match self.replicated_snapshot().await?.body {
            Reply::Snapshot { state } => Ok(state),
            Reply::Refused { message, .. } => Err(message),
            _ => Err("Unexpected chamber snapshot outcome".into()),
        }
    }
    pub async fn events(&mut self, after: u64, limit: u16) -> Result<EventPage, String> {
        match self
            .request_ready(Body::Events { after, limit })
            .await?
            .body
        {
            Reply::Events { page } => Ok(page),
            Reply::Refused { message, .. } => Err(message),
            _ => Err("Unexpected chamber events outcome".into()),
        }
    }
    /// Delivers unseen committed events using retained reconnect progress.
    pub async fn delivered_events(
        &mut self,
        cursor: &mut super::event_cursor::Cursor,
        limit: u16,
    ) -> Result<super::event_cursor::Delivery, String> {
        if cursor.instance() != self.instance {
            return Err("Event cursor belongs to another chamber".into());
        }
        let after = cursor.after();
        let response = self.request_ready(Body::Events { after, limit }).await?;
        if let Reply::Refused { message, .. } = &response.body {
            return Err(message.clone());
        }
        cursor.admit(&response, after, limit)
    }
    pub async fn respawn(&mut self) -> Result<Response, String> {
        let life = self
            .control()
            .ok_or("Client has no admitted adventurer")?
            .life;
        self.request_ready(Body::Respawn { life }).await
    }
    pub async fn account(&mut self) -> Result<super::accounts::Account, String> {
        match self.request_ready(Body::Account {}).await?.body {
            Reply::Account { account } => Ok(account),
            Reply::Refused { message, .. } => Err(message),
            _ => Err("Unexpected realm account outcome".into()),
        }
    }
    /// Selects an owned dormant character after account authentication.
    pub async fn select_character(&mut self, character: u64) -> Result<Response, String> {
        self.request_ready(Body::SelectCharacter { character })
            .await
    }
    /// Saves the controlled character, frees its resident slot, and ends this session.
    pub async fn logout(&mut self) -> Result<Response, String> {
        let control = self
            .control()
            .ok_or("Client has no admitted adventurer")?
            .clone();
        self.request_ready(Body::Logout {
            life: control.life,
            epoch: control.epoch,
        })
        .await
    }
    pub async fn close(&mut self) -> Result<(), String> {
        let mut stream = self.stream.take().ok_or("Chamber client is disconnected")?;
        self.control = None;
        timeout(DEADLINE, stream.shutdown())
            .await
            .map_err(|_| "Chamber close timed out")?
            .map_err(|_| "Cannot close chamber transport".into())
    }
    /// Requests a full bounded baseline after an application discards cached state.
    pub async fn resync(&mut self) -> Result<State, String> {
        self.replication.clear();
        self.snapshot().await
    }
    fn reconstruct(&mut self, request: &Body, response: &mut Response) -> Result<(), String> {
        if let Reply::Replicated { packet } = &response.body {
            if !matches!(request, Body::Replicate { .. }) {
                return Err("Unrequested replication packet".into());
            }
            let state =
                self.replication
                    .admit(packet, self.instance, response.tick, &response.control)?;
            response.body = Reply::Snapshot { state };
        } else if matches!(request, Body::Replicate { .. })
            && !matches!(response.body, Reply::Refused { .. })
        {
            return Err("Replication request did not return a bounded packet".into());
        }
        Ok(())
    }
    fn validate(&self, request_id: u64, request: &Body, r: &Response) -> Result<(), String> {
        if r.version != VERSION
            || r.instance != self.instance
            || r.request_id != request_id
            || r.tick < self.tick
        {
            return Err("Chamber response context mismatch".into());
        }
        if let Some(control) = &r.control {
            if let Some(baseline) = control.applied_movement {
                baseline.validate()?;
                if baseline.profile != crate::movement::Profile::Frames
                    || baseline.life != control.life.into()
                    || baseline.epoch != control.epoch
                    || baseline.applied_sequence > control.accepted_sequence
                    || baseline.world_step > control.credit_step
                {
                    return Err("Chamber applied movement confirmation is incompatible".into());
                }
            }
            if control.credit_step < control.world_step
                || control.life.instance != self.instance
                || control
                    .credit_step
                    .checked_add(u64::from(crate::movement::frames::MAX_STEPS))
                    .is_none()
                || (self.logged_in
                    && !self.player
                    && !matches!(
                        (request, &r.body),
                        (
                            Body::SelectCharacter { .. },
                            Reply::CharacterSelected { .. }
                        )
                    ))
            {
                return Err("Chamber response control identity mismatch".into());
            }
            if let Some(previous) = &self.control {
                if control.credit_step < previous.credit_step
                    || control.world_step < previous.world_step
                    || control.life.actor != previous.life.actor
                    || control.life.generation < previous.life.generation
                    || control.epoch < previous.epoch
                    || (control.epoch == previous.epoch
                        && (control.accepted_sequence < previous.accepted_sequence
                            || control.life.generation != previous.life.generation))
                {
                    return Err("Chamber response control fence regressed".into());
                }
            }
        } else if self.player && !matches!(r.body, Reply::Refused { .. } | Reply::LoggedOut { .. })
        {
            return Err("Chamber response lost player control".into());
        }
        match (&r.body, request) {
            (Reply::Account { account }, Body::Account {}) => {
                if account.id == 0
                    || account.epoch == 0
                    || account.key != self.public_key
                    || account.characters.is_empty()
                    || account.characters.len() > 8
                    || account.characters.iter().any(|id| *id == 0)
                    || account.characters.windows(2).any(|w| w[0] >= w[1])
                {
                    return Err("Realm account metadata is incompatible".into());
                }
                Ok(())
            }
            (
                Reply::CharacterSelected { character },
                Body::SelectCharacter {
                    character: requested,
                },
            ) if character == requested && *character > 0 && r.control.is_some() => Ok(()),
            (Reply::LoggedOut { character }, Body::Logout { .. })
                if *character > 0 && r.control.is_none() =>
            {
                Ok(())
            }
            (Reply::Refused { .. }, _) => Ok(()),
            (Reply::Accepted, Body::Authenticate { .. }) => Ok(()),
            (Reply::Accepted, Body::Command { command }) => {
                if r.control.as_ref().is_none_or(|c| {
                    let normal =
                        c.epoch == command.epoch && c.accepted_sequence >= command.sequence;
                    let teleport = matches!(
                        command.intent,
                        super::wire::Action::Cast {
                            ability: Ability::MistyStep,
                            ..
                        }
                    ) && command.epoch.checked_add(1) == Some(c.epoch)
                        && c.accepted_sequence == 0;
                    c.life != command.actor || !(normal || teleport)
                }) {
                    return Err("Accepted chamber command has no matching acknowledgment".into());
                }
                Ok(())
            }
            (Reply::Accepted, Body::Social { input }) => {
                if r.control.as_ref().is_none_or(|c| {
                    c.life != input.life.into()
                        || input.epoch.checked_add(1) != Some(c.epoch)
                        || c.accepted_sequence != 0
                }) {
                    return Err("Accepted social command has no matching control fence".into());
                }
                Ok(())
            }
            (Reply::Accepted, Body::MovementCredit {}) => {
                if r.control.is_none() {
                    return Err("Movement credit has no owned control".into());
                }
                Ok(())
            }
            (Reply::Accepted, Body::MovementFrame { frame }) => {
                if r.control.as_ref().is_none_or(|c| {
                    c.life != frame.life.into()
                        || c.epoch != frame.epoch
                        || c.accepted_sequence < frame.sequence
                }) {
                    return Err("Accepted movement interval has no matching acknowledgment".into());
                }
                Ok(())
            }
            (Reply::Snapshot { state }, Body::BeginMovementFrames { life, epoch }) => {
                if r.control.as_ref().is_none_or(|c| {
                    c.life != *life || c.epoch < *epoch || c.epoch > epoch.saturating_add(1)
                }) {
                    return Err("Interval entry has a foreign life".into());
                }
                state.validate_control(self.instance, &r.control)?;
                if state
                    .movement
                    .is_none_or(|b| b.profile != crate::movement::Profile::Frames)
                {
                    return Err("Interval entry has no initial movement baseline".into());
                }
                Ok(())
            }
            (Reply::Accepted, Body::Respawn { life }) => {
                if r.control.as_ref().is_none_or(|c| {
                    c.life.actor != life.actor || c.life.generation <= life.generation
                }) {
                    return Err("Accepted chamber respawn has no new life".into());
                }
                Ok(())
            }
            (Reply::Snapshot { state }, Body::Snapshot {} | Body::Replicate { .. }) => {
                state.validate_control(self.instance, &r.control)
            }
            (Reply::Safety { view }, Body::Safety {}) => {
                view.validate()?;
                if view
                    .contacts
                    .iter()
                    .any(|c| c.life.instance != self.instance)
                {
                    return Err("Public contact belongs to another instance".into());
                }
                Ok(())
            }
            (
                Reply::SafetyApplied { receipt },
                Body::SafetyAction {
                    realm,
                    operation,
                    action,
                },
            ) => receipt.validate(*realm, receipt.account, *operation, action),
            (Reply::Services { view }, Body::Services { character }) => {
                view.validate()?;
                if view.character != *character
                    || view.realm == [0; 32]
                    || view.groups.len() > 2
                    || view.invitations.len() > 16
                    || view.items.len() > 64
                    || view.offers.len() > 16
                    || view
                        .items
                        .iter()
                        .any(|i| i.owner != *character || i.version == 0)
                    || view
                        .offers
                        .iter()
                        .any(|o| o.from != *character && o.to != *character)
                {
                    return Err("Realm service view is incompatible".into());
                }
                Ok(())
            }
            (
                Reply::ServiceApplied { receipt },
                Body::ServiceAction {
                    realm,
                    character,
                    operation,
                    action,
                },
            ) => {
                if receipt.realm != *realm
                    || receipt.character != *character
                    || receipt.operation != *operation
                    || receipt.digest != super::game_services::action_digest(action)?
                {
                    return Err("Realm service acknowledgment is incompatible".into());
                }
                receipt.validate(action)?;
                Ok(())
            }
            (
                Reply::QuestCycleChanged {
                    quest,
                    cycle,
                    action,
                    revision,
                },
                Body::QuestCycle {
                    life,
                    epoch,
                    quest: requested,
                    cycle: expected,
                    action: intent,
                },
            ) => {
                if quest != requested
                    || cycle != expected
                    || action != intent
                    || *revision == 0
                    || *quest == 0
                    || r.control
                        .as_ref()
                        .is_none_or(|c| c.life != *life || c.epoch != *epoch)
                {
                    return Err("Quest cycle acknowledgment is incompatible".into());
                }
                Ok(())
            }
            (
                Reply::QuestAccepted { quest, revision },
                Body::AcceptQuest {
                    life,
                    epoch,
                    quest: requested,
                    giver,
                },
            ) => {
                if quest != requested
                    || *quest == 0
                    || *revision == 0
                    || giver.actor == 0
                    || giver.instance != self.instance
                    || r.control
                        .as_ref()
                        .is_none_or(|c| c.life != *life || c.epoch != *epoch)
                {
                    return Err("Quest acceptance acknowledgment is incompatible".into());
                }
                Ok(())
            }
            (
                Reply::QuestClaimed { quest, revision },
                Body::ClaimQuest {
                    life,
                    epoch,
                    quest: requested,
                },
            ) => {
                if quest != requested
                    || *quest == 0
                    || *revision == 0
                    || r.control
                        .as_ref()
                        .is_none_or(|c| c.life != *life || c.epoch != *epoch)
                {
                    return Err("Quest claim acknowledgment is incompatible".into());
                }
                Ok(())
            }
            (
                Reply::ItemUsed {
                    item,
                    operation,
                    revision,
                },
                Body::UseItem {
                    life,
                    epoch,
                    item: requested,
                    operation: identity,
                },
            ) => {
                if item != requested
                    || operation != identity
                    || *item == 0
                    || *operation == [0; 16]
                    || *revision == 0
                    || r.control
                        .as_ref()
                        .is_none_or(|c| c.life != *life || c.epoch != *epoch)
                {
                    return Err("Item use acknowledgment is incompatible".into());
                }
                Ok(())
            }
            (
                Reply::GearEquipped {
                    slot,
                    item,
                    operation,
                    revision,
                },
                Body::EquipGear {
                    life,
                    epoch,
                    slot: requested_slot,
                    item: requested_item,
                    operation: identity,
                },
            ) => {
                if slot != requested_slot
                    || item != requested_item
                    || operation != identity
                    || *operation == [0; 16]
                    || *revision == 0
                    || r.control
                        .as_ref()
                        .is_none_or(|c| c.life != *life || c.epoch != *epoch)
                {
                    return Err("Equipment acknowledgment is incompatible".into());
                }
                Ok(())
            }
            (
                Reply::OutfitEquipped {
                    outfit,
                    operation,
                    revision,
                },
                Body::EquipOutfit {
                    life,
                    epoch,
                    outfit: requested,
                    operation: identity,
                },
            ) => {
                if outfit != requested
                    || operation != identity
                    || *operation == [0; 16]
                    || *revision == 0
                    || r.control
                        .as_ref()
                        .is_none_or(|c| c.life != *life || c.epoch != *epoch)
                {
                    return Err("Outfit acknowledgment is incompatible".into());
                }
                Ok(())
            }
            (Reply::Inventory { inventory }, Body::Inventory {}) => {
                inventory.validate(&r.control)?;
                if inventory.revision < self.inventory_revision {
                    return Err("Inventory transaction revision regressed".into());
                }
                Ok(())
            }
            (Reply::Events { page }, Body::Events { after, limit }) => {
                page.validate(self.instance, r.tick, *after, *limit)
            }
            _ => Err("Unexpected chamber response outcome".into()),
        }
    }
}

/// Maximum requests awaiting verified responses on one duplex connection.
pub const PIPELINE_CAPACITY: usize = 8;

struct Outgoing {
    bytes: Vec<u8>,
    frame: Option<super::worker::FrameObservation>,
}

struct Pending {
    id: u64,
    body: Body,
    sent: client_runtime::Instant,
}

/// Bounded duplex transport. Dropping it closes uncertain IO without replay.
/// Frame tasks own partial reads and writes independently of caller polling.
pub struct Pipeline {
    last_turnaround: Option<Duration>,
    last_response_bytes: Option<usize>,
    last_request_started: Option<web_time::Instant>,
    client: Client,
    writes: tokio::sync::mpsc::Sender<Outgoing>,
    responses: tokio::sync::mpsc::Receiver<Result<(Response, usize), String>>,
    pending: std::collections::VecDeque<Pending>,
    tasks: Vec<client_runtime::Task>,
    sequence: Option<(super::wire::Life, u64, u64)>,
    failed: bool,
}
impl Drop for Pipeline {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}
impl Client {
    /// Transfers an authenticated connection to bounded duplex IO.
    pub fn pipeline(self) -> Result<Pipeline, String> {
        self.pipeline_observed(None)
    }
    pub(super) fn pipeline_observed(
        mut self,
        observer: Option<super::worker::Observer>,
    ) -> Result<Pipeline, String> {
        let stream = self.stream.take().ok_or("Chamber client is disconnected")?;
        let (mut reader, mut writer) = tokio::io::split(stream);
        let (writes, mut outgoing) = tokio::sync::mpsc::channel::<Outgoing>(PIPELINE_CAPACITY);
        let (incoming, responses) = tokio::sync::mpsc::channel(PIPELINE_CAPACITY);
        let errors = incoming.clone();
        let write = client_runtime::spawn(async move {
            while let Some(first) = outgoing.recv().await {
                let result = timeout(DEADLINE, async {
                    // One millisecond lets a same-wake input and its reads share
                    // a write. The existing deadline includes this collection.
                    client_runtime::sleep(Duration::from_millis(1)).await;
                    let mut frames = vec![first.bytes];
                    let mut traces: Vec<_> = first.frame.into_iter().collect();
                    while frames.len() < PIPELINE_CAPACITY {
                        match outgoing.try_recv() {
                            Ok(outgoing) => {
                                frames.push(outgoing.bytes);
                                traces.extend(outgoing.frame);
                            }
                            Err(_) => break,
                        }
                    }
                    if let Some(observer) = &observer {
                        for mut trace in traces.iter().copied() {
                            trace.at = web_time::Instant::now();
                            trace.phase = "transport_started";
                            observer.frame(trace);
                        }
                    }
                    write_frame_batch(&mut writer, &frames, MAX_REQUEST_BYTES).await?;
                    if let Some(observer) = &observer {
                        for mut trace in traces {
                            trace.at = web_time::Instant::now();
                            trace.phase = "transport_flushed";
                            observer.frame(trace);
                        }
                    }
                    Ok::<_, String>(())
                })
                .await
                .map_err(|_| "Chamber write timed out".to_string())
                .and_then(|r| r);
                if let Err(error) = result {
                    let _ = errors.send(Err(error)).await;
                    return;
                }
            }
        });
        let read = client_runtime::spawn(async move {
            loop {
                let result = match read_frame(&mut reader, MAX_RESPONSE_BYTES).await {
                    Ok(bytes) => serde_json::from_slice::<Response>(&bytes)
                        .map(|response| (response, bytes.len()))
                        .map_err(|_| "Malformed chamber response".to_string()),
                    Err(error) => Err(error),
                };
                let failed = result.is_err();
                if incoming.send(result).await.is_err() || failed {
                    return;
                }
            }
        });
        Ok(Pipeline {
            last_turnaround: None,
            last_response_bytes: None,
            last_request_started: None,
            client: self,
            writes,
            responses,
            pending: Default::default(),
            tasks: vec![read, write],
            sequence: None,
            failed: false,
        })
    }
}
impl Pipeline {
    pub fn available(&self) -> bool {
        !self.failed && self.pending.len() < PIPELINE_CAPACITY
    }
    pub fn send_snapshot(&mut self) -> Result<u64, String> {
        self.send(Body::Replicate {
            ack: self.client.replication.ack(),
        })
    }
    pub fn pending(&self) -> usize {
        self.pending.len()
    }
    /// Enqueue to verified response, including IO queues, transport, and server work.
    /// This is not a one-way network delay or isolated RTT.
    pub fn last_turnaround(&self) -> Option<Duration> {
        self.last_turnaround
    }
    /// Payload bytes of the latest verified reply, excluding the framing prefix.
    pub fn last_response_bytes(&self) -> Option<usize> {
        self.last_response_bytes
    }
    pub fn last_request_started(&self) -> Option<web_time::Instant> {
        self.last_request_started
    }

    pub fn control(&self) -> Option<&Control> {
        if self.failed {
            None
        } else {
            self.client.control.as_ref()
        }
    }
    pub fn tick(&self) -> u64 {
        self.client.tick
    }
    pub fn instance(&self) -> u64 {
        self.client.instance
    }

    pub(crate) fn verified_at(&self) -> Option<web_time::Instant> {
        self.client.verified_at
    }

    /// Allocates commands monotonically within the currently verified life and epoch.
    pub fn prepare_command(&mut self, intent: Intent<Ability>) -> Result<Command<Ability>, String> {
        let control = self
            .control()
            .ok_or("Client has no admitted adventurer")?
            .clone();
        let previous = match self.sequence {
            Some((life, epoch, sequence)) if life == control.life && epoch == control.epoch => {
                sequence.max(control.accepted_sequence)
            }
            _ => control.accepted_sequence,
        };
        let sequence = previous
            .checked_add(1)
            .ok_or("Client command sequence exhausted")?;
        self.sequence = Some((control.life, control.epoch, sequence));
        Ok(Command {
            actor: control.life.into(),
            epoch: control.epoch,
            sequence,
            tick: self.client.tick,
            intent,
        })
    }
    pub fn prepare_movement_frame(
        &mut self,
        mut frame: crate::movement::frames::Frame,
    ) -> Result<crate::movement::frames::Frame, String> {
        frame.validate_payload()?;
        let control = self.control().ok_or("Client has no admitted adventurer")?;
        if frame.life != control.life.into() || frame.epoch != control.epoch {
            return Err("Movement interval control changed before transmission".into());
        }
        let command = self.prepare_command(Intent::Jump)?;
        frame.sequence = command.sequence;
        frame.tick = command.tick;
        Ok(frame)
    }
    /// Enqueues once. A successful enqueue never implies authoritative acceptance.
    pub fn send(&mut self, body: Body) -> Result<u64, String> {
        self.send_observed(body, None)
    }
    pub(super) fn send_observed(
        &mut self,
        body: Body,
        frame: Option<super::worker::FrameObservation>,
    ) -> Result<u64, String> {
        if !self.available() {
            return Err("Chamber pipeline is unavailable or full".into());
        }
        if matches!(body, Body::Snapshot {} | Body::Replicate { .. })
            && self
                .pending
                .iter()
                .any(|p| matches!(p.body, Body::Snapshot {} | Body::Replicate { .. }))
        {
            return Err("Replaceable snapshot request is already in flight".into());
        }
        let id = self.client.next_request;
        let next = id
            .checked_add(1)
            .ok_or("Client request identities exhausted")?;
        let bytes = serde_json::to_vec(&Request {
            version: VERSION,
            request_id: id,
            body: body.clone(),
        })
        .map_err(|_| "Cannot encode chamber request")?;
        Request::decode(&bytes)?;
        self.writes
            .try_send(Outgoing { bytes, frame })
            .map_err(|_| "Chamber writer is unavailable")?;
        self.client.next_request = next;
        self.pending.push_back(Pending {
            id,
            body,
            sent: client_runtime::Instant::now(),
        });
        Ok(id)
    }
    fn fail(&mut self) {
        self.failed = true;
        self.client.control = None;
        self.pending.clear();
        for task in &self.tasks {
            task.abort();
        }
    }
    /// Cancellation before delivery preserves the response and its pending request.
    /// Invalid or overdue responses close both IO tasks without replaying commands.
    pub async fn receive(&mut self) -> Result<(Body, Response), String> {
        let deadline = self
            .pending
            .front()
            .ok_or("Chamber pipeline has no pending request")?
            .sent
            + DEADLINE;
        let received = client_runtime::timeout_at(deadline, self.responses.recv()).await;
        let result = match received {
            Ok(Some(result)) => result,
            Ok(None) => Err("Chamber response reader closed".into()),
            Err(_) => Err("Chamber request timed out".into()),
        };
        let (mut response, response_bytes) = match result {
            Ok(response) => response,
            Err(error) => {
                self.fail();
                return Err(error);
            }
        };
        let pending = self.pending.front().expect("Pending response context");
        let body = pending.body.clone();
        if let Err(error) = self.client.reconstruct(&body, &mut response) {
            self.fail();
            return Err(error);
        }
        let pending = self.pending.front().expect("Pending response context");
        if let Err(error) = self.client.validate(pending.id, &pending.body, &response) {
            self.fail();
            return Err(error);
        }
        let pending = self.pending.pop_front().expect("Verified response context");
        self.last_turnaround = Some(pending.sent.elapsed());
        self.last_response_bytes = Some(response_bytes);
        #[cfg(not(target_arch = "wasm32"))]
        let started = pending.sent.into_std();
        #[cfg(target_arch = "wasm32")]
        let started = pending.sent;
        self.last_request_started = Some(started);
        self.client.tick = response.tick;
        self.client.verified_at = Some(web_time::Instant::now());
        if let Reply::Inventory { inventory } = &response.body {
            self.client.inventory_revision = inventory.revision;
        }
        if matches!(response.body, Reply::CharacterSelected { .. }) {
            self.client.player = true;
            self.client.replication.clear();
        }
        let lost_control = self.client.player && response.control.is_none();
        self.client.control = response.control.clone();
        if lost_control {
            self.fail();
        }
        Ok((pending.body, response))
    }
}

#[cfg(all(test, feature = "service-net"))]
mod tests {
    use super::*;
    use crate::service::{
        net::tests::{gateway, key, start, tls},
        wire::{Action, Input, Life},
    };
    use tokio::{net::TcpListener, sync::oneshot};
    #[cfg(feature = "service-net")]
    use tokio_rustls::TlsAcceptor;
    fn name() -> ServerName<'static> {
        ServerName::try_from("localhost").unwrap()
    }

    #[tokio::test]
    async fn serial_commands_refresh_only_explicit_unconsumed_stale_ticks() {
        use crate::service::net::{read_frame, write_frame};
        let keys = [key(227), key(228), key(229)];
        let mut g = gateway(&keys);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (server, connector) = tls();
        let peer = client_runtime::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut socket = TlsAcceptor::from(server).accept(socket).await.unwrap();
            let (id, hello) = g.open_json(0).unwrap();
            write_frame(&mut socket, &hello, MAX_RESPONSE_BYTES)
                .await
                .unwrap();
            let auth = read_frame(&mut socket, MAX_REQUEST_BYTES).await.unwrap();
            let response = g.dispatch_json(id, 0, &auth).unwrap();
            write_frame(&mut socket, &response, MAX_RESPONSE_BYTES)
                .await
                .unwrap();
            for _ in 0..7 {
                g.tick(1. / 30.).unwrap();
            }
            let mut commands = Vec::new();
            for index in 0..4 {
                let bytes = read_frame(&mut socket, MAX_REQUEST_BYTES).await.unwrap();
                let Body::Command { command } = Request::decode(&bytes).unwrap().body else {
                    panic!()
                };
                commands.push(command);
                let response = g.dispatch_json(id, 0, &bytes).unwrap();
                let reply: Response = serde_json::from_slice(&response).unwrap();
                if index == 0 {
                    assert!(matches!(&reply.body,Reply::Refused{code,..} if code=="stale_tick"));
                    assert_eq!(g.admission(id).unwrap().accepted_sequence(), 0);
                }
                if index == 1 {
                    assert!(matches!(reply.body, Reply::Accepted));
                    g.tick(1. / 30.).unwrap();
                    assert!(
                        g.game()
                            .actor_position(g.game().player_life().actor)
                            .unwrap()
                            .x
                            > 0.
                    );
                }
                write_frame(&mut socket, &response, MAX_RESPONSE_BYTES)
                    .await
                    .unwrap();
            }
            assert_eq!(
                commands.iter().map(|c| c.sequence).collect::<Vec<_>>(),
                vec![1, 1, 2, 3]
            );
            assert!(commands[1].tick > commands[0].tick);
            assert_eq!(g.admission(id).unwrap().accepted_sequence(), 3);
            assert!(
                timeout(
                    Duration::from_millis(50),
                    read_frame(&mut socket, MAX_REQUEST_BYTES)
                )
                .await
                .is_err(),
                "Gameplay refusal must not replay"
            );
        });
        let mut client =
            Client::connect(address, name(), connector.config().clone(), 120, &keys[0])
                .await
                .unwrap();
        assert!(matches!(
            client
                .command(Intent::Move {
                    axes: [1., 0.],
                    yaw: 0.
                })
                .await
                .unwrap()
                .body,
            Reply::Accepted
        ));
        let shield = Intent::Cast {
            ability: Ability::Shield,
            target: None,
            aim: [0., 0., 1.],
        };
        assert!(matches!(
            client.command(shield.clone()).await.unwrap().body,
            Reply::Accepted
        ));
        assert!(
            matches!(client.command(shield).await.unwrap().body,Reply::Refused{code,..} if code=="command")
        );
        assert_eq!(client.control().unwrap().accepted_sequence, 3);
        peer.await.unwrap();
    }
    #[tokio::test]
    async fn frame_write_observation_distinguishes_blocked_flush_from_acceptance() {
        let (stream, mut peer) = tokio::io::duplex(8);
        let life = super::super::wire::Life {
            instance: 120,
            actor: 14,
            generation: 0,
        };
        let client = Client {
            public_key: [1; 32],
            stream: Some(Box::new(stream)),
            instance: 120,
            tick: 1,
            control: Some(Control {
                life,
                epoch: 2,
                accepted_sequence: 0,
                world_step: 0,
                credit_step: 0,
                applied_movement: None,
                dynamic: Vec::new(),
            }),
            next_request: 2,
            logged_in: true,
            player: true,
            inventory_revision: 0,
            verified_at: None,
            replication: Default::default(),
        };
        let observer = super::super::worker::Observer::default();
        let mut pipeline = client.pipeline_observed(Some(observer.clone())).unwrap();
        let trace = super::super::worker::FrameObservation {
            at: web_time::Instant::now(),
            phase: "enqueued",
            actor: 14,
            epoch: 2,
            sequence: 1,
            start: 0,
            end: 4,
            authority_tick: 1,
            control_epoch: Some(2),
            credit_step: Some(0),
            pending_requests: 1,
            queued_inputs: 0,
        };
        let frame = crate::movement::frames::Frame {
            life: life.into(),
            epoch: 2,
            sequence: 1,
            tick: 1,
            start: 0,
            steps: 4,
            segments: vec![crate::movement::frames::Segment {
                offset: 0,
                axes: [0.; 2],
                yaw: 0.,
                until: 4,
                jump: false,
            }],
        };
        pipeline
            .send_observed(Body::MovementFrame { frame }, Some(trace))
            .unwrap();
        let started = timeout(Duration::from_secs(1), async {
            loop {
                let frames = observer.drain().frames;
                if let Some(frame) = frames.first().copied() {
                    assert_eq!(frames.len(), 1);
                    break frame;
                }
                client_runtime::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(started.phase, "transport_started");
        client_runtime::sleep(Duration::from_millis(20)).await;
        assert!(observer.drain().frames.is_empty());
        let bytes = read_frame(&mut peer, MAX_REQUEST_BYTES).await.unwrap();
        assert!(matches!(
            Request::decode(&bytes).unwrap().body,
            Body::MovementFrame { .. }
        ));
        let flushed = timeout(Duration::from_secs(1), async {
            loop {
                if let Some(frame) = observer.drain().frames.first().copied() {
                    break frame;
                }
                client_runtime::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(flushed.phase, "transport_flushed");
        assert!(flushed.at >= started.at && started.at >= trace.at);
        assert_eq!(flushed.sequence, 1);
        assert_eq!(pipeline.pending(), 1);
        assert_eq!(pipeline.control().unwrap().accepted_sequence, 0);
        for _ in 0..300 {
            observer.frame(flushed);
        }
        let overflow = observer.drain();
        assert_eq!(overflow.frames.len(), 256);
        assert_eq!(overflow.omitted_frames, 44);
    }

    #[tokio::test]
    async fn pipeline_sends_bounded_inputs_before_any_reply_and_preserves_partial_reads() {
        let keys = [key(201), key(202), key(203)];
        let mut gateway = gateway(&keys);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (server, connector) = tls();
        let (partial, partial_received) = oneshot::channel();
        let (release, released) = oneshot::channel();
        let peer = client_runtime::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut socket = TlsAcceptor::from(server).accept(socket).await.unwrap();
            let (id, hello) = gateway.open_json(0).unwrap();
            write_frame(&mut socket, &hello, MAX_RESPONSE_BYTES)
                .await
                .unwrap();
            let auth = read_frame(&mut socket, MAX_REQUEST_BYTES).await.unwrap();
            let response = gateway.dispatch_json(id, 0, &auth).unwrap();
            write_frame(&mut socket, &response, MAX_RESPONSE_BYTES)
                .await
                .unwrap();
            let mut responses = Vec::new();
            // The peer withholds every response until all requests arrive.
            for _ in 0..PIPELINE_CAPACITY {
                let bytes = timeout(
                    Duration::from_secs(3),
                    read_frame(&mut socket, MAX_REQUEST_BYTES),
                )
                .await
                .unwrap()
                .unwrap();
                responses.push(gateway.dispatch_json(id, 0, &bytes).unwrap());
            }
            use tokio::io::AsyncWriteExt;
            let first = [b" \n".as_slice(), responses.remove(0).as_slice()].concat();
            socket
                .write_all(&(first.len() as u32).to_be_bytes())
                .await
                .unwrap();
            socket.write_all(&first[..first.len() / 2]).await.unwrap();
            socket.flush().await.unwrap();
            partial.send(first.len()).unwrap();
            released.await.unwrap();
            socket.write_all(&first[first.len() / 2..]).await.unwrap();
            socket.flush().await.unwrap();
            for response in responses {
                write_frame(&mut socket, &response, MAX_RESPONSE_BYTES)
                    .await
                    .unwrap();
            }
        });
        let client = Client::connect(address, name(), connector.config().clone(), 120, &keys[0])
            .await
            .unwrap();
        let mut pipeline = client.pipeline().unwrap();
        for sequence in 1..=PIPELINE_CAPACITY as u64 {
            let command = pipeline
                .prepare_command(Intent::Move {
                    axes: [0., 0.],
                    yaw: 0.,
                })
                .unwrap();
            assert_eq!(command.sequence, sequence);
            pipeline
                .send(Body::Command {
                    command: command.into(),
                })
                .unwrap();
        }
        assert!(!pipeline.available());
        assert!(pipeline.send(Body::Snapshot {}).is_err());
        let first_bytes = partial_received.await.unwrap();
        assert!(
            timeout(Duration::from_millis(20), pipeline.receive())
                .await
                .is_err()
        );
        assert_eq!(pipeline.pending(), PIPELINE_CAPACITY);
        assert_eq!(pipeline.last_response_bytes(), None);
        release.send(()).unwrap();
        for sequence in 1..=PIPELINE_CAPACITY as u64 {
            let (body, response) = timeout(Duration::from_secs(3), pipeline.receive())
                .await
                .unwrap()
                .unwrap();
            assert!(matches!(body, Body::Command { command } if command.sequence == sequence));
            assert!(matches!(response.body, Reply::Accepted));
            if sequence == 1 {
                assert_eq!(pipeline.last_response_bytes(), Some(first_bytes));
                assert!(first_bytes > serde_json::to_vec(&response).unwrap().len());
            }
            assert_eq!(pipeline.control().unwrap().accepted_sequence, sequence);
        }
        assert_eq!(pipeline.pending(), 0);
        assert!(pipeline.available());
        peer.await.unwrap();
    }

    #[tokio::test]
    async fn pipeline_foreign_correlation_closes_io_without_replay() {
        let (client, _, peer) = fake_peer(false).await;
        let mut pipeline = client.pipeline().unwrap();
        pipeline.send(Body::Snapshot {}).unwrap();
        assert!(pipeline.receive().await.is_err());
        assert!(!pipeline.available());
        assert!(pipeline.control().is_none());
        assert!(pipeline.send(Body::Snapshot {}).is_err());
        peer.await.unwrap();
    }

    #[tokio::test]
    async fn pipeline_drop_closes_an_uncertain_request() {
        let (client, received, peer) = fake_peer(true).await;
        let mut pipeline = client.pipeline().unwrap();
        pipeline.send(Body::Snapshot {}).unwrap();
        received.await.unwrap();
        drop(pipeline);
        peer.await.unwrap();
    }

    #[tokio::test]
    async fn npc_quest_acceptance_is_durable_owned_and_withheld_on_storage_failure() {
        use crate::service::{
            net,
            net::tests::gateway_at,
            persistence::Store,
            progression::{Config, Quest},
            rewards::{Entry, Transaction},
        };
        let keys = [key(101), key(102), key(103)];
        let g = gateway_at(&keys, Some(glam::Vec3::new(-1., 0., -22.)))
            .with_content([9; 32])
            .unwrap()
            .with_progression(Config {
                version: 1,
                levels: vec![0, 100],
                quests: vec![Quest {
                    repeatable: false,
                    dialogue: None,
                    giver: Some(2),
                    prerequisites: vec![],
                    id: 1,
                    name: "Disrupt the ritual".into(),
                    objective: 1,
                    goal: 2,
                    experience: 75,
                    items: vec![],
                }],
            })
            .unwrap();
        let actor = g.game().player_life().actor;
        let giver = g.game().actor_life(2).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("state");
        let store = Store::open(&root, [9; 32], 120).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (server_tls, connector) = tls();
        let server = tokio::spawn(net::serve_durable(
            listener,
            server_tls.clone(),
            g,
            store,
            std::future::pending::<()>(),
        ));
        let mut a = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([9; 32]),
            &keys[0],
        )
        .await
        .unwrap();
        let before = a.inventory().await.unwrap();
        assert!(!before.quest_log[0].accepted);
        assert!(before.quest_log[0].interactable);
        std::fs::create_dir(root.join("next.json")).unwrap();
        assert!(a.accept_quest(1, giver).await.is_err());
        let exit = client_runtime::timeout(Duration::from_secs(5), server)
            .await
            .unwrap()
            .unwrap();
        assert!(exit.failure.is_some());
        drop(exit);
        std::fs::remove_dir(root.join("next.json")).unwrap();
        let mut store = Store::open(&root, [9; 32], 120).unwrap();
        let g = store.recover().unwrap();
        assert!(!g.quest_log(actor)[0].accepted);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(net::serve_durable(
            listener,
            server_tls.clone(),
            g,
            store,
            std::future::pending::<()>(),
        ));
        let mut a = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([9; 32]),
            &keys[0],
        )
        .await
        .unwrap();
        let mut b = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([9; 32]),
            &keys[1],
        )
        .await
        .unwrap();
        let mut spectator = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([9; 32]),
            &keys[2],
        )
        .await
        .unwrap();
        assert!(spectator.accept_quest(1, giver).await.is_err());
        let accepted = a.accept_quest(1, giver).await.unwrap();
        assert!(matches!(
            accepted.body,
            Reply::QuestAccepted {
                quest: 1,
                revision: 1
            }
        ));
        assert!(matches!(
            a.accept_quest(1, giver).await.unwrap().body,
            Reply::QuestAccepted {
                quest: 1,
                revision: 1
            }
        ));
        let after = a.inventory().await.unwrap();
        assert!(after.quest_log[0].accepted);
        assert_eq!(after.quest_log[0].progress, 0);
        assert!(!b.inventory().await.unwrap().quest_log[0].accepted);
        server.abort();
        assert!(matches!(server.await, Err(error) if error.is_cancelled()));
        let mut store = Store::open(&root, [9; 32], 120).unwrap();
        let mut g = store.recover().unwrap();
        assert!(g.quest_log(actor)[0].accepted);
        g.grant_reward(Transaction {
            acceptance: None,
            instance: 120,
            actor,
            source: [71; 32],
            experience: 0,
            items: vec![],
            quests: vec![Entry { id: 1, count: 2 }],
            spent: vec![],
            outfit: None,
            equipment: None,
        })
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(net::serve_durable(
            listener,
            server_tls,
            g,
            store,
            std::future::pending::<()>(),
        ));
        let mut a = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([9; 32]),
            &keys[0],
        )
        .await
        .unwrap();
        assert!(matches!(
            a.accept_quest(1, giver).await.unwrap().body,
            Reply::QuestAccepted {
                quest: 1,
                revision: 1
            }
        ));
        assert_eq!(a.inventory().await.unwrap().quest_log[0].progress, 2);
        assert!(matches!(
            a.claim_quest(1).await.unwrap().body,
            Reply::QuestClaimed {
                quest: 1,
                revision: 3
            }
        ));
        assert!(matches!(
            a.claim_quest(1).await.unwrap().body,
            Reply::QuestClaimed {
                quest: 1,
                revision: 3
            }
        ));
        let completed = a.inventory().await.unwrap();
        assert!(completed.quest_log[0].claimed);
        assert_eq!(completed.experience, 75);
        server.abort();
        assert!(matches!(server.await, Err(error) if error.is_cancelled()));
        let mut store = Store::open(&root, [9; 32], 120).unwrap();
        let recovered = store.recover().unwrap();
        assert_eq!(recovered.character_rewards(actor).unwrap().experience, 75);
        assert!(recovered.quest_log(actor)[0].claimed);
        println!(
            "VERSE_QUEST_ENROLLMENT {}",
            serde_json::json!({"schema":"verse.quest-enrollment.fixture.v1",
            "wire_version":VERSION,"before":before,"accepted":after,"completed":completed,
            "failed_storage_ack_withheld":true,"restart":"aborted_host_task","duplicate_acceptance":false,"duplicate_claim":false,
            "other_character_accepted":false,"recovered_experience":75})
        );
    }
    #[tokio::test]
    async fn tls_combat_rewards_are_owned_and_survive_durable_host_restart() {
        use crate::service::{
            net,
            persistence::Store,
            rewards::{Entry, Policy},
        };
        use std::time::Duration;
        let keys = [key(61), key(62), key(63)];
        let mut g = gateway(&keys)
            .with_content([7; 32])
            .unwrap()
            .with_rewards(vec![Policy {
                participation: Default::default(),
                target: 2,
                experience: 45,
                items: vec![Entry { id: 1, count: 1 }],
                quests: vec![Entry { id: 1, count: 1 }],
            }])
            .unwrap()
            .with_progression(super::super::progression::Config {
                version: 1,
                levels: vec![0, 100, 300],
                quests: vec![
                    super::super::progression::Quest {
                        repeatable: false,
                        dialogue: None,
                        giver: None,
                        prerequisites: vec![],
                        id: 1,
                        name: "Disrupt the summoning".into(),
                        objective: 1,
                        goal: 1,
                        experience: 75,
                        items: vec![Entry { id: 1, count: 2 }],
                    },
                    super::super::progression::Quest {
                        repeatable: false,
                        dialogue: None,
                        giver: None,
                        prerequisites: vec![1],
                        id: 2,
                        name: "Secure the chamber".into(),
                        objective: 1,
                        goal: 1,
                        experience: 25,
                        items: vec![],
                    },
                ],
            })
            .unwrap();
        let source = g.game().ids[&2];
        let hp = g
            .game()
            .snapshot()
            .actors
            .iter()
            .find(|a| a.id == source)
            .unwrap()
            .hp;
        g.chamber
            .game
            .simulation
            .bow_impact(source, hp - 1)
            .unwrap();
        g.tick(0.).unwrap();
        let target = g.game().actor_life(2).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("state");
        let store = Store::open(&root, [7; 32], 120).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (server_tls, connector) = tls();
        let server = tokio::spawn(net::serve_durable(
            listener,
            server_tls.clone(),
            g,
            store,
            std::future::pending::<()>(),
        ));
        let mut a = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([7; 32]),
            &keys[0],
        )
        .await
        .unwrap();
        let mut b = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([7; 32]),
            &keys[1],
        )
        .await
        .unwrap();
        let mut spectator = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([7; 32]),
            &keys[2],
        )
        .await
        .unwrap();
        assert_eq!(a.inventory().await.unwrap().experience, 0);
        assert!(matches!(
            a.claim_quest(1).await.unwrap().body,
            Reply::Refused { .. }
        ));
        assert!(spectator.inventory().await.is_err());
        assert!(spectator.connected());
        a.snapshot().await.unwrap();
        assert!(matches!(
            a.command(Intent::Cast {
                ability: Ability::MagicMissile,
                target: Some(target),
                aim: [0., 0., 1.]
            })
            .await
            .unwrap()
            .body,
            Reply::Accepted
        ));
        let left = client_runtime::timeout(Duration::from_secs(4), async {
            loop {
                let inv = a.inventory().await.unwrap();
                if inv.experience == 45 {
                    break inv;
                }
                client_runtime::sleep(Duration::from_millis(33)).await;
            }
        })
        .await
        .unwrap();
        let right = b.inventory().await.unwrap();
        assert_eq!(right.experience, 45);
        assert_ne!(left.life.actor, right.life.actor);
        assert_eq!(left.items, vec![Entry { id: 1, count: 1 }]);
        assert_eq!(left.quests, vec![Entry { id: 1, count: 1 }]);
        assert_eq!(left.revision, 2);
        assert_eq!(right.revision, 2);
        assert_eq!(left.level.level, 1);
        assert_eq!(left.quest_log[0].progress, 1);
        assert!(!left.quest_log[0].claimed);
        assert!(!left.quest_log[1].available);
        assert!(matches!(
            a.claim_quest(2).await.unwrap().body,
            Reply::Refused { .. }
        ));
        let ready = left.clone();
        let own = a.control().unwrap().clone();
        assert!(matches!(
            b.request(Body::ClaimQuest {
                life: own.life,
                epoch: own.epoch,
                quest: 1
            })
            .await
            .unwrap()
            .body,
            Reply::Refused { .. }
        ));
        assert!(spectator.claim_quest(1).await.is_err());
        let claimed = a.claim_quest(1).await.unwrap();
        assert!(matches!(
            claimed.body,
            Reply::QuestClaimed {
                quest: 1,
                revision: 3
            }
        ));
        assert!(matches!(
            a.claim_quest(1).await.unwrap().body,
            Reply::QuestClaimed {
                quest: 1,
                revision: 3
            }
        ));
        assert!(matches!(
            b.claim_quest(1).await.unwrap().body,
            Reply::QuestClaimed {
                quest: 1,
                revision: 4
            }
        ));
        let left = a.inventory().await.unwrap();
        let right = b.inventory().await.unwrap();
        assert_eq!(left.experience, 120);
        assert_eq!(right.experience, 120);
        assert_eq!(left.level.level, 2);
        assert_eq!(left.items, vec![Entry { id: 1, count: 3 }]);
        assert!(left.quest_log[0].claimed);
        assert!(left.quest_log[1].available);
        assert!(right.quest_log[1].available);
        assert_eq!(left.revision, 4);
        server.abort();
        assert!(matches!(server.await,Err(error) if error.is_cancelled()));
        let mut store = Store::open(&root, [7; 32], 120).unwrap();
        let g = store.recover().unwrap();
        assert_eq!(
            g.character_rewards(left.life.actor).unwrap().experience,
            120
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, stopping) = oneshot::channel();
        let server = tokio::spawn(net::serve_durable(listener, server_tls, g, store, async {
            let _ = stopping.await;
        }));
        let mut recovered = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([7; 32]),
            &keys[0],
        )
        .await
        .unwrap();
        assert!(matches!(
            recovered.claim_quest(1).await.unwrap().body,
            Reply::QuestClaimed {
                quest: 1,
                revision: 3
            }
        ));
        let after = recovered.inventory().await.unwrap();
        assert_eq!(after, left);
        client_runtime::sleep(Duration::from_millis(70)).await;
        assert_eq!(recovered.inventory().await.unwrap(), after);
        recovered.close().await.unwrap();
        stop.send(()).unwrap();
        assert!(server.await.unwrap().failure.is_none());
        println!(
            "VERSE_COMBAT_REWARDS {}",
            serde_json::json!({
                "schema":"verse.campaign.fixture.v1","ready":ready,"claim_revisions":[3,4],"wire_version":VERSION,"instance":120,
                "ability":"magic_missile","target":target,"target_fixture_health":1,
                "primary":left,"secondary":right,"recovered_primary":after,
                "spectator_inventory_refused":true,"restart":"aborted_host_task",
                "reward_repeat_after_restart":false
            })
        );
    }

    #[tokio::test]
    async fn durable_claim_failure_withholds_receipt_and_recovers_ready_quest() {
        use crate::service::{
            net,
            persistence::Store,
            progression::{Config, Quest},
            rewards::Transaction,
        };
        let keys = [key(67), key(68), key(69)];
        let mut g = gateway(&keys)
            .with_content([7; 32])
            .unwrap()
            .with_progression(Config {
                version: 1,
                levels: vec![0, 100, 300],
                quests: vec![Quest {
                    repeatable: false,
                    dialogue: None,
                    giver: None,
                    prerequisites: vec![],
                    id: 1,
                    name: "Disrupt the summoning".into(),
                    objective: 1,
                    goal: 1,
                    experience: 75,
                    items: vec![],
                }],
            })
            .unwrap();
        let actor = g.game().player_life().actor;
        g.grant_reward(Transaction {
            acceptance: None,
            outfit: None,
            equipment: None,
            spent: vec![],
            instance: 120,
            actor,
            source: [8; 32],
            experience: 45,
            items: vec![],
            quests: vec![super::super::rewards::Entry { id: 1, count: 1 }],
        })
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("state");
        let store = Store::open(&root, [7; 32], 120).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (server_tls, connector) = tls();
        let server = tokio::spawn(net::serve_durable(
            listener,
            server_tls.clone(),
            g,
            store,
            std::future::pending::<()>(),
        ));
        let mut client = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([7; 32]),
            &keys[0],
        )
        .await
        .unwrap();
        let before = client.inventory().await.unwrap();
        assert!(!before.quest_log[0].claimed);
        std::fs::create_dir(root.join("next.json")).unwrap();
        assert!(client.claim_quest(1).await.is_err());
        let exit = client_runtime::timeout(std::time::Duration::from_secs(3), server)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            exit.failure.as_deref(),
            Some("Chamber storage entry must be a regular file")
        );
        drop(exit);
        std::fs::remove_dir(root.join("next.json")).unwrap();
        let mut store = Store::open(&root, [7; 32], 120).unwrap();
        let g = store.recover().unwrap();
        assert_eq!(g.character_rewards(actor).unwrap().experience, 45);
        assert!(!g.quest_log(actor)[0].claimed);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, stopping) = oneshot::channel();
        let server = tokio::spawn(net::serve_durable(listener, server_tls, g, store, async {
            let _ = stopping.await;
        }));
        let mut client = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([7; 32]),
            &keys[0],
        )
        .await
        .unwrap();
        assert!(matches!(
            client.claim_quest(1).await.unwrap().body,
            Reply::QuestClaimed {
                quest: 1,
                revision: 2
            }
        ));
        assert_eq!(client.inventory().await.unwrap().experience, 120);
        assert!(matches!(
            client.claim_quest(1).await.unwrap().body,
            Reply::QuestClaimed {
                quest: 1,
                revision: 2
            }
        ));
        client.close().await.unwrap();
        stop.send(()).unwrap();
        assert!(server.await.unwrap().failure.is_none());
    }
    #[tokio::test]
    async fn durable_item_use_withholds_failed_ack_and_retries_without_duplicate_recovery() {
        use crate::service::{
            items::{Catalog, Item},
            net,
            persistence::Store,
            rewards::{Entry, Transaction},
        };
        let keys = [key(71), key(72), key(73)];
        let mut g = gateway(&keys)
            .with_content([7; 32])
            .unwrap()
            .with_items(Catalog {
                version: 1,
                items: vec![Item {
                    id: 1,
                    name: "Recovery ember".into(),
                    health: 45,
                    mana: 5,
                }],
            })
            .unwrap();
        let actor = g.game().player_life().actor;
        g.grant_reward(Transaction {
            acceptance: None,
            outfit: None,
            equipment: None,
            instance: 120,
            actor,
            source: [8; 32],
            experience: 1,
            items: vec![Entry { id: 1, count: 2 }],
            quests: vec![],
            spent: vec![],
        })
        .unwrap();
        g.chamber.game.simulation.player_damage_for(0, 100).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("state");
        let store = Store::open(&root, [7; 32], 120).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (server_tls, connector) = tls();
        let server = tokio::spawn(net::serve_durable(
            listener,
            server_tls.clone(),
            g,
            store,
            std::future::pending::<()>(),
        ));
        let mut client = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([7; 32]),
            &keys[0],
        )
        .await
        .unwrap();
        let before = client.inventory().await.unwrap();
        assert_eq!(before.catalog.items[0].health, 45);
        std::fs::create_dir(root.join("next.json")).unwrap();
        assert!(client.use_item(1, [1; 16]).await.is_err());
        let exit = client_runtime::timeout(std::time::Duration::from_secs(3), server)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            exit.failure.as_deref(),
            Some("Chamber storage entry must be a regular file")
        );
        drop(exit);
        std::fs::remove_dir(root.join("next.json")).unwrap();
        let mut store = Store::open(&root, [7; 32], 120).unwrap();
        let g = store.recover().unwrap();
        assert_eq!(g.game().snapshot().player.hp, 100);
        assert_eq!(g.character_rewards(actor).unwrap().items[&1], 2);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(net::serve_durable(
            listener,
            server_tls.clone(),
            g,
            store,
            std::future::pending::<()>(),
        ));
        let mut client = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([7; 32]),
            &keys[0],
        )
        .await
        .unwrap();
        let ack = client.use_item(1, [1; 16]).await.unwrap();
        assert!(matches!(
            ack.body,
            Reply::ItemUsed {
                item: 1,
                revision: 2,
                ..
            }
        ));
        assert_eq!(
            serde_json::to_value(client.use_item(1, [1; 16]).await.unwrap().body).unwrap(),
            serde_json::to_value(&ack.body).unwrap()
        );
        let after = client.inventory().await.unwrap();
        assert_eq!(after.items[0].count, 1);
        server.abort();
        assert!(matches!(server.await,Err(error) if error.is_cancelled()));
        let mut store = Store::open(&root, [7; 32], 120).unwrap();
        let g = store.recover().unwrap();
        assert_eq!(g.game().snapshot().player.hp, 145);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, stopping) = oneshot::channel();
        let server = tokio::spawn(net::serve_durable(listener, server_tls, g, store, async {
            let _ = stopping.await;
        }));
        let mut client = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([7; 32]),
            &keys[0],
        )
        .await
        .unwrap();
        assert_eq!(
            serde_json::to_value(client.use_item(1, [1; 16]).await.unwrap().body).unwrap(),
            serde_json::to_value(&ack.body).unwrap()
        );
        let recovered = client.inventory().await.unwrap();
        assert_eq!(after, recovered);
        client.close().await.unwrap();
        stop.send(()).unwrap();
        let exit = server.await.unwrap();
        assert!(exit.failure.is_none());
        assert_eq!(exit.gateway.game().snapshot().player.hp, 145);
        println!(
            "VERSE_ITEM_USE {}",
            serde_json::json!({"schema":"verse.item-use.fixture.v1","wire_version":VERSION,"before":before,"after":after,"recovered":recovered,"health_before":100,"health_after":145,"restart":"aborted_host_task","failed_storage_ack_withheld":true,"duplicate_spending":false,"duplicate_recovery":false})
        );
    }
    #[tokio::test]
    async fn durable_outfit_change_is_replicated_and_survives_failed_commit_and_restart() {
        use crate::service::{
            net,
            outfits::{Catalog, Outfit},
            persistence::Store,
            rewards::{Entry, Transaction},
        };
        let keys = [key(74), key(75), key(76)];
        let mut g = gateway(&keys)
            .with_content([7; 32])
            .unwrap()
            .with_outfits(Catalog {
                version: 1,
                outfits: vec![
                    Outfit {
                        id: 1,
                        name: "Peasant outfit".into(),
                        model: "universal-male-peasant".into(),
                    },
                    Outfit {
                        id: 2,
                        name: "Ranger outfit".into(),
                        model: "universal-female-ranger".into(),
                    },
                ],
            })
            .unwrap()
            .with_equipment(crate::service::equipment::Catalog {
                version: 1,
                gear: vec![
                    crate::service::equipment::Gear {
                        id: 3,
                        name: "Ritual hat".into(),
                        slot: crate::service::equipment::Slot::Head,
                        model: "gear-hat".into(),
                        offset: [0, 0, 230],
                        health: 100,
                        mana: 0,
                    },
                    crate::service::equipment::Gear {
                        id: 4,
                        name: "Ritual wand".into(),
                        slot: crate::service::equipment::Slot::MainHand,
                        model: "gear-wand".into(),
                        offset: [0; 3],
                        health: 0,
                        mana: 10,
                    },
                ],
            })
            .unwrap();
        let actor = g.game().player_life().actor;
        g.grant_reward(Transaction {
            acceptance: None,
            outfit: None,
            equipment: None,
            instance: 120,
            actor,
            source: [8; 32],
            experience: 1,
            items: vec![
                Entry { id: 1, count: 2 },
                Entry { id: 3, count: 1 },
                Entry { id: 4, count: 1 },
            ],
            quests: vec![],
            spent: vec![],
        })
        .unwrap();
        g.chamber.game.simulation.player_damage_for(0, 100).unwrap();
        let secondary_actor = g
            .game()
            .controlled_effects()
            .map(|(life, _, _)| life.actor)
            .find(|id| *id != actor)
            .unwrap();
        g.grant_reward(Transaction {
            acceptance: None,
            instance: 120,
            actor: secondary_actor,
            source: [9; 32],
            experience: 1,
            items: vec![Entry { id: 2, count: 1 }, Entry { id: 3, count: 1 }],
            quests: vec![],
            spent: vec![],
            outfit: None,
            equipment: None,
        })
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("state");
        let store = Store::open(&root, [7; 32], 120).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (server_tls, connector) = tls();
        let server = tokio::spawn(net::serve_durable(
            listener,
            server_tls.clone(),
            g,
            store,
            std::future::pending::<()>(),
        ));
        let mut client = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([7; 32]),
            &keys[0],
        )
        .await
        .unwrap();
        let before = client.inventory().await.unwrap();
        assert_eq!(before.outfit, 0);
        std::fs::create_dir(root.join("next.json")).unwrap();
        assert!(
            client
                .equip_gear(crate::service::equipment::Slot::Head, 3, [3; 16])
                .await
                .is_err()
        );
        let exit = client_runtime::timeout(std::time::Duration::from_secs(3), server)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            exit.failure.as_deref(),
            Some("Chamber storage entry must be a regular file")
        );
        drop(exit);
        std::fs::remove_dir(root.join("next.json")).unwrap();
        let mut store = Store::open(&root, [7; 32], 120).unwrap();
        let g = store.recover().unwrap();
        assert_eq!(g.game().snapshot().player.hp, 100);
        assert_eq!(g.character_rewards(actor).unwrap().items[&1], 2);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(net::serve_durable(
            listener,
            server_tls.clone(),
            g,
            store,
            std::future::pending::<()>(),
        ));
        let mut client = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([7; 32]),
            &keys[0],
        )
        .await
        .unwrap();
        let ack = client.equip_outfit(1, [1; 16]).await.unwrap();
        assert!(matches!(
            ack.body,
            Reply::OutfitEquipped {
                outfit: 1,
                revision: 3,
                ..
            }
        ));
        assert_eq!(
            serde_json::to_value(client.equip_outfit(1, [1; 16]).await.unwrap().body).unwrap(),
            serde_json::to_value(&ack.body).unwrap()
        );
        let gear_ack = client
            .equip_gear(crate::service::equipment::Slot::Head, 3, [3; 16])
            .await
            .unwrap();
        assert!(matches!(
            gear_ack.body,
            Reply::GearEquipped { revision: 4, .. }
        ));
        client
            .equip_gear(crate::service::equipment::Slot::MainHand, 4, [4; 16])
            .await
            .unwrap();
        assert_eq!(
            serde_json::to_value(
                client
                    .equip_gear(crate::service::equipment::Slot::Head, 3, [3; 16])
                    .await
                    .unwrap()
                    .body
            )
            .unwrap(),
            serde_json::to_value(&gear_ack.body).unwrap()
        );
        let resources = client.snapshot().await.unwrap().snapshot.player;
        assert_eq!(
            (resources.hp, resources.max_hp, resources.max_mana),
            (100, 300, 30)
        );
        let mut second = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([7; 32]),
            &keys[1],
        )
        .await
        .unwrap();
        assert!(matches!(
            second.equip_outfit(2, [2; 16]).await.unwrap().body,
            Reply::OutfitEquipped {
                outfit: 2,
                revision: 6,
                ..
            }
        ));
        second
            .equip_gear(crate::service::equipment::Slot::Head, 3, [5; 16])
            .await
            .unwrap();
        second.close().await.unwrap();
        let after = client.inventory().await.unwrap();
        assert_eq!(after.items[0].count, 2);
        assert_eq!(after.outfit, 1);
        let keyspectator = &keys[2];
        let mut spectator = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([7; 32]),
            keyspectator,
        )
        .await
        .unwrap();
        let state = spectator.snapshot().await.unwrap();
        let pose = state
            .presentation
            .actors
            .iter()
            .find(|p| p.life.actor == actor)
            .unwrap();
        assert_eq!(pose.equipment.len(), 2);
        assert_eq!(pose.actor.health, 300);
        assert_eq!(pose.actor.model, "adventurer");
        assert_eq!(pose.outfit_model.as_deref(), Some("universal-male-peasant"));
        let second_pose = state
            .presentation
            .actors
            .iter()
            .find(|p| p.life.actor == secondary_actor)
            .unwrap();
        assert_eq!(second_pose.equipment.len(), 1);
        assert_eq!(second_pose.actor.health, 300);
        assert_eq!(second_pose.actor.model, "adventurer");
        assert_eq!(
            second_pose.outfit_model.as_deref(),
            Some("universal-female-ranger")
        );
        spectator.close().await.unwrap();
        server.abort();
        assert!(matches!(server.await,Err(error) if error.is_cancelled()));
        let mut store = Store::open(&root, [7; 32], 120).unwrap();
        let g = store.recover().unwrap();
        assert_eq!(g.game().snapshot().player.hp, 100);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, stopping) = oneshot::channel();
        let server = tokio::spawn(net::serve_durable(listener, server_tls, g, store, async {
            let _ = stopping.await;
        }));
        let mut client = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([7; 32]),
            &keys[0],
        )
        .await
        .unwrap();
        assert_eq!(
            serde_json::to_value(client.equip_outfit(1, [1; 16]).await.unwrap().body).unwrap(),
            serde_json::to_value(&ack.body).unwrap()
        );
        assert_eq!(
            serde_json::to_value(
                client
                    .equip_gear(crate::service::equipment::Slot::Head, 3, [3; 16])
                    .await
                    .unwrap()
                    .body
            )
            .unwrap(),
            serde_json::to_value(&gear_ack.body).unwrap()
        );
        let recovered = client.inventory().await.unwrap();
        assert_eq!(after, recovered);
        client.close().await.unwrap();
        stop.send(()).unwrap();
        let exit = server.await.unwrap();
        assert!(exit.failure.is_none());
        assert_eq!(exit.gateway.game().snapshot().player.hp, 100);
        println!(
            "VERSE_EQUIPMENT {}",
            serde_json::json!({"schema":"verse.equipment.fixture.v1","wire_version":VERSION,"before":before,"after":after,"recovered":recovered,"health":100,"max_health":300,"max_mana":30,"secondary_equipped_slots":1,"spectator_equipment_instances":3,"failed_storage_ack_withheld":true,"restart":"aborted_host_task","retry_receipt":gear_ack})
        );
        println!(
            "VERSE_OUTFIT {}",
            serde_json::json!({"schema":"verse.outfit.fixture.v1","wire_version":VERSION,"before":before,"after":after,"recovered":recovered,"health_before":100,"health_after":100,"restart":"aborted_host_task","failed_storage_ack_withheld":true,"duplicate_spending":false,"duplicate_outfit_change":false,"secondary_outfit":2,"spectator_models_verified":2})
        );
    }
    #[tokio::test]
    async fn reward_overflow_stops_durable_host_without_saving_partial_death() {
        use crate::service::{
            net,
            persistence::Store,
            rewards::{Policy, Transaction},
        };
        let keys = [key(64), key(65), key(66)];
        let mut g = gateway(&keys)
            .with_content([7; 32])
            .unwrap()
            .with_rewards(vec![Policy {
                participation: Default::default(),
                target: 2,
                experience: 45,
                items: vec![],
                quests: vec![],
            }])
            .unwrap();
        let actor = g.game().player_life().actor;
        g.grant_reward(Transaction {
            acceptance: None,
            outfit: None,
            equipment: None,
            spent: vec![],
            instance: 120,
            actor,
            source: [90; 32],
            experience: u64::MAX,
            items: vec![],
            quests: vec![],
        })
        .unwrap();
        let source = g.game().ids[&2];
        let hp = g
            .game()
            .snapshot()
            .actors
            .iter()
            .find(|a| a.id == source)
            .unwrap()
            .hp;
        g.chamber
            .game
            .simulation
            .bow_impact(source, hp - 1)
            .unwrap();
        g.tick(0.).unwrap();
        let target = g.game().actor_life(2).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("state");
        let store = Store::open(&root, [7; 32], 120).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (server_tls, connector) = tls();
        let server = tokio::spawn(net::serve_durable(
            listener,
            server_tls,
            g,
            store,
            std::future::pending::<()>(),
        ));
        let mut a = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([7; 32]),
            &keys[0],
        )
        .await
        .unwrap();
        a.snapshot().await.unwrap();
        assert!(matches!(
            a.command(Intent::Cast {
                ability: Ability::MagicMissile,
                target: Some(target),
                aim: [0., 0., 1.]
            })
            .await
            .unwrap()
            .body,
            Reply::Accepted
        ));
        let exit = client_runtime::timeout(std::time::Duration::from_secs(4), server)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            exit.failure.as_deref(),
            Some("Character experience exceeded")
        );
        drop(exit);
        let mut store = Store::open(&root, [7; 32], 120).unwrap();
        let recovered = store.recover().unwrap();
        assert_eq!(
            recovered.character_rewards(actor).unwrap().experience,
            u64::MAX
        );
        let npc = recovered
            .game()
            .snapshot()
            .actors
            .into_iter()
            .find(|a| a.id == source)
            .unwrap();
        assert!(npc.alive && npc.hp > 0);
        assert_eq!(recovered.chamber.rewards.revision(), 1);
    }

    #[tokio::test]
    async fn configured_content_mismatch_sends_no_authentication_request() {
        let keys = [key(1), key(2), key(3)];
        let (server_tls, connector) = tls();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let gateway = gateway(&keys).with_content([9; 32]).unwrap();
        let (stop, stopping) = oneshot::channel();
        let server = tokio::spawn(crate::service::net::serve(
            listener,
            server_tls,
            gateway,
            async {
                let _ = stopping.await;
            },
        ));
        assert!(
            Client::connect_with_content(
                address,
                name(),
                connector.config().clone(),
                120,
                Some([8; 32]),
                &keys[0]
            )
            .await
            .is_err()
        );
        assert!(
            Client::connect(address, name(), connector.config().clone(), 120, &keys[0])
                .await
                .is_err()
        );
        let mut admitted = Client::connect_with_content(
            address,
            name(),
            connector.config().clone(),
            120,
            Some([9; 32]),
            &keys[0],
        )
        .await
        .unwrap();
        assert!(admitted.control().is_some());
        admitted.close().await.unwrap();
        stop.send(()).unwrap();
        let exit = server.await.unwrap();
        assert!(exit.failure.is_none());
        assert_eq!(exit.stats.requests, 1);
    }
    #[tokio::test]
    async fn verified_clients_keep_owned_sequences_and_gameplay_refusals() {
        let keys = [key(31), key(32), key(33)];
        let (address, connector, stop, task) = start(&keys).await;
        let config = connector.config().clone();
        let mut a = Client::connect(address, name(), config.clone(), 120, &keys[0])
            .await
            .unwrap();
        let mut b = Client::connect(address, name(), config.clone(), 120, &keys[1])
            .await
            .unwrap();
        let mut spectator = Client::connect(address, name(), config.clone(), 120, &keys[2])
            .await
            .unwrap();
        assert!(spectator.control().is_none());
        a.snapshot().await.unwrap();
        b.snapshot().await.unwrap();
        let own = a.control().unwrap().clone();
        let foreign = Input {
            actor: own.life,
            epoch: own.epoch,
            sequence: own.accepted_sequence + 1,
            tick: a.tick(),
            intent: Action::Jump {},
        };
        assert!(matches!(
            b.request(Body::Command { command: foreign })
                .await
                .unwrap()
                .body,
            Reply::Refused { .. }
        ));
        assert_eq!(b.control().unwrap().accepted_sequence, 0);
        assert!(spectator.command(Intent::Jump).await.is_err());
        assert!(spectator.connected());
        let shield = Intent::Cast {
            ability: Ability::Shield,
            target: None,
            aim: [0., 0., 1.],
        };
        assert!(matches!(
            a.command(shield.clone()).await.unwrap().body,
            Reply::Accepted
        ));
        assert!(matches!(
            b.command(shield.clone()).await.unwrap().body,
            Reply::Accepted
        ));
        assert!(matches!(
            a.command(shield).await.unwrap().body,
            Reply::Refused { .. }
        ));
        assert_eq!(a.control().unwrap().accepted_sequence, 2);
        assert!(a.connected());
        let state_a = a.snapshot().await.unwrap();
        assert_eq!(
            state_a.hud.as_ref().unwrap().life,
            a.control().unwrap().life.into()
        );
        assert_eq!(state_a.hud.as_ref().unwrap().resources.mana, 19);
        let state_b = b.snapshot().await.unwrap();
        assert_eq!(
            state_b.hud.as_ref().unwrap().life,
            b.control().unwrap().life.into()
        );
        assert_ne!(
            state_a.hud.as_ref().unwrap().life,
            state_b.hud.as_ref().unwrap().life
        );
        assert_eq!(
            state_a
                .presentation
                .effects
                .iter()
                .filter(|e| e.shield > 0)
                .count(),
            2
        );
        let observed = spectator.snapshot().await.unwrap();
        assert!(observed.hud.is_none());
        assert_eq!(
            serde_json::to_vec(&state_a.actors).unwrap(),
            serde_json::to_vec(&observed.actors).unwrap()
        );
        let mut cursor = super::super::event_cursor::Cursor::new(120);
        spectator.delivered_events(&mut cursor, 64).await.unwrap();
        let saved = cursor.checkpoint().unwrap();
        let mut restored = super::super::event_cursor::Cursor::restore(&saved, 120).unwrap();
        assert!(
            spectator
                .delivered_events(&mut restored, 64)
                .await
                .unwrap()
                .events
                .is_empty()
        );
        let mut foreign = super::super::event_cursor::Cursor::new(121);
        assert!(spectator.delivered_events(&mut foreign, 64).await.is_err());
        assert_eq!(saved, cursor.checkpoint().unwrap());
        let mut replacement = Client::connect(address, name(), config.clone(), 120, &keys[0])
            .await
            .unwrap();
        assert!(replacement.control().unwrap().epoch > own.epoch);
        assert!(a.snapshot().await.is_err());
        assert!(!a.connected());
        assert!(a.control().is_none());
        let epoch = b.control().unwrap().epoch;
        b.close().await.unwrap();
        client_runtime::sleep(Duration::from_millis(30)).await;
        let mut b = Client::connect(address, name(), config, 120, &keys[1])
            .await
            .unwrap();
        assert!(b.control().unwrap().epoch >= epoch + 2);
        b.close().await.unwrap();
        replacement.close().await.unwrap();
        spectator.close().await.unwrap();
        stop.send(()).unwrap();
        let exit = task.await.unwrap();
        assert!(exit.failure.is_none());
        assert_eq!(
            exit.stats.accepted_connections,
            exit.stats.completed_connections
        );
    }

    #[tokio::test]
    async fn client_refuses_untrusted_certificates_and_foreign_instance_before_signing() {
        let keys = [key(34), key(35), key(36)];
        let (address, connector, stop, task) = start(&keys).await;
        assert!(
            Client::connect(address, name(), connector.config().clone(), 121, &keys[0])
                .await
                .is_err()
        );
        let empty =
            ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_root_certificates(rustls::RootCertStore::empty())
                .with_no_client_auth();
        assert!(
            Client::connect(address, name(), Arc::new(empty), 120, &keys[0])
                .await
                .is_err()
        );
        let mut valid = Client::connect(address, name(), connector.config().clone(), 120, &keys[0])
            .await
            .unwrap();
        valid.snapshot().await.unwrap();
        valid.close().await.unwrap();
        stop.send(()).unwrap();
        assert!(task.await.unwrap().failure.is_none());
    }

    async fn fake_peer(
        cancel: bool,
    ) -> (Client, oneshot::Receiver<()>, tokio::task::JoinHandle<()>) {
        let keys = [key(37), key(38), key(39)];
        let mut gateway = gateway(&keys);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (server, connector) = tls();
        let (sent, received) = oneshot::channel();
        let task = client_runtime::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            socket.set_nodelay(true).unwrap();
            let mut socket = TlsAcceptor::from(server).accept(socket).await.unwrap();
            let (id, hello) = gateway.open_json(0).unwrap();
            write_frame(&mut socket, &hello, MAX_RESPONSE_BYTES)
                .await
                .unwrap();
            let auth = read_frame(&mut socket, MAX_REQUEST_BYTES).await.unwrap();
            let accepted = gateway.dispatch_json(id, 0, &auth).unwrap();
            write_frame(&mut socket, &accepted, MAX_RESPONSE_BYTES)
                .await
                .unwrap();
            let request = read_frame(&mut socket, MAX_REQUEST_BYTES).await.unwrap();
            let _ = sent.send(());
            if cancel {
                assert!(
                    timeout(
                        Duration::from_secs(3),
                        read_frame(&mut socket, MAX_REQUEST_BYTES)
                    )
                    .await
                    .unwrap()
                    .is_err()
                );
            } else {
                let bytes = gateway.dispatch_json(id, 0, &request).unwrap();
                let mut response: Response = serde_json::from_slice(&bytes).unwrap();
                response.request_id += 1;
                write_frame(
                    &mut socket,
                    &serde_json::to_vec(&response).unwrap(),
                    MAX_RESPONSE_BYTES,
                )
                .await
                .unwrap();
            }
        });
        let client = Client::connect(address, name(), connector.config().clone(), 120, &keys[0])
            .await
            .unwrap();
        (client, received, task)
    }
    #[tokio::test]
    async fn foreign_correlation_invalidates_transport_without_automatic_retry() {
        let (mut client, _, task) = fake_peer(false).await;
        assert!(client.snapshot().await.is_err());
        assert!(!client.connected());
        assert!(client.control().is_none());
        task.await.unwrap();
    }
    #[tokio::test]
    async fn cancelling_an_uncertain_request_drops_transport() {
        let (mut client, received, task) = fake_peer(true).await;
        let mut request = Box::pin(client.snapshot());
        tokio::select! { _ = received => {}, result = &mut request => panic!("Unexpected response: {}", result.is_ok()) }
        drop(request);
        assert!(!client.connected());
        assert!(client.control().is_none());
        task.await.unwrap();
    }
    #[tokio::test]
    async fn historical_snapshot_verifies_credit_without_rewriting_its_state() {
        let keys = [key(230), key(231), key(232)];
        let (address, tls, stop, host) = start(&keys).await;
        let mut client = Client::connect(address, name(), tls.config().clone(), 120, &keys[0])
            .await
            .unwrap();
        let mut snapshot = client.request(Body::Snapshot {}).await.unwrap();
        let body = serde_json::to_vec(&snapshot.body).unwrap();
        let control = snapshot.control.as_mut().unwrap();
        control.credit_step += 8;
        client.control = Some(control.clone());
        assert!(
            client
                .validate(snapshot.request_id, &Body::Snapshot {}, &snapshot)
                .is_ok()
        );
        assert_eq!(serde_json::to_vec(&snapshot.body).unwrap(), body);
        let mut regressed = snapshot.clone();
        regressed.control.as_mut().unwrap().credit_step -= 1;
        assert!(
            client
                .validate(snapshot.request_id, &Body::Snapshot {}, &regressed)
                .is_err()
        );
        let mut missing = serde_json::to_value(&snapshot).unwrap();
        missing["control"]
            .as_object_mut()
            .unwrap()
            .remove("credit_step");
        assert!(serde_json::from_value::<Response>(missing).is_err());
        client.close().await.unwrap();
        stop.send(()).unwrap();
        assert!(host.await.unwrap().failure.is_none());
    }

    #[test]
    fn context_and_control_regressions_are_refused() {
        let life = Life {
            instance: 120,
            actor: 14,
            generation: 0,
        };
        let control = Control {
            credit_step: 4,
            world_step: 4,
            life,
            epoch: 2,
            accepted_sequence: 3,
            applied_movement: None,
            dynamic: Vec::new(),
        };
        let client = Client {
            public_key: [1; 32],
            stream: None,
            instance: 120,
            tick: 10,
            control: Some(control.clone()),
            next_request: 2,
            logged_in: true,
            player: true,
            inventory_revision: 0,
            verified_at: None,
            replication: Default::default(),
        };
        let response = Response {
            version: VERSION,
            request_id: 2,
            instance: 120,
            tick: 10,
            control: Some(control),
            body: Reply::Accepted,
        };
        let request = Body::Command {
            command: Input {
                actor: life,
                epoch: 2,
                sequence: 4,
                tick: 10,
                intent: Action::Jump {},
            },
        };
        assert!(client.validate(2, &request, &response).is_err());
        let mut response = response;
        response.control.as_mut().unwrap().accepted_sequence = 4;
        assert!(client.validate(2, &request, &response).is_ok());
        for field in 0..8 {
            let mut bad = response.clone();
            match field {
                0 => bad.version += 1,
                1 => bad.request_id += 1,
                2 => bad.instance += 1,
                3 => bad.tick -= 1,
                4 => bad.control.as_mut().unwrap().epoch -= 1,
                5 => bad.control.as_mut().unwrap().accepted_sequence -= 2,
                6 => bad.control.as_mut().unwrap().world_step -= 1,
                _ => bad.control.as_mut().unwrap().credit_step -= 1,
            }
            assert!(client.validate(2, &request, &bad).is_err());
        }
        let applied = crate::movement::Baseline {
            profile: crate::movement::Profile::Frames,
            life: life.into(),
            epoch: response.control.as_ref().unwrap().epoch,
            applied_sequence: 2,
            world_step: 4,
            physics_step: 2,
            held: Default::default(),
            policy: Default::default(),
            character: physics::character::Character::new(glam::DVec3::ZERO),
            yaw: 0.,
        };
        let mut confirmed = response.clone();
        confirmed.control.as_mut().unwrap().applied_movement = Some(applied);
        assert!(client.validate(2, &request, &confirmed).is_ok());
        for field in 0..6 {
            let mut invalid = confirmed.clone();
            let proof = invalid
                .control
                .as_mut()
                .unwrap()
                .applied_movement
                .as_mut()
                .unwrap();
            match field {
                0 => proof.life.generation += 1,
                1 => proof.epoch += 1,
                2 => proof.applied_sequence = 5,
                3 => proof.world_step += 1,
                4 => proof.profile = crate::movement::Profile::Arrival,
                _ => proof.character.feet.x = f64::NAN,
            }
            assert!(client.validate(2, &request, &invalid).is_err());
        }
        let mut revived = response;
        revived.control.as_mut().unwrap().life.generation += 1;
        assert!(client.validate(2, &request, &revived).is_err());
        revived.control.as_mut().unwrap().epoch += 1;
        revived.control.as_mut().unwrap().accepted_sequence = 0;
        assert!(
            client
                .validate(2, &Body::Respawn { life }, &revived)
                .is_ok()
        );
    }
}

/// Verifies an already-open socket's TLS identity before a reachable upgrade.
#[cfg(feature = "service-net")]
pub async fn connect_tls_stream(
    socket: TcpStream,
    tls: Arc<ClientConfig>,
    server_name: ServerName<'static>,
) -> Result<tokio_rustls::client::TlsStream<TcpStream>, String> {
    TlsConnector::from(tls)
        .connect(server_name, socket)
        .await
        .map_err(|_| "Chamber TLS identity refused".into())
}
