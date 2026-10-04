//! Sequential TLS chamber client with acknowledged control and no automatic replay.
use std::{net::SocketAddr, sync::Arc, time::Duration};

use rustls::{ClientConfig, pki_types::ServerName};
use secp256k1::{Keypair, Secp256k1};
use tokio::{io::AsyncWriteExt, net::TcpStream, time::timeout};
use tokio_rustls::{TlsConnector, client::TlsStream};

use super::{
    net::{read_frame, write_frame},
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
pub struct Client {
    stream: Option<TlsStream<TcpStream>>,
    instance: u64,
    tick: u64,
    control: Option<Control>,
    next_request: u64,
    logged_in: bool,
    player: bool,
    inventory_revision: u64,
}
impl Client {
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
    pub async fn connect_with_content(
        address: SocketAddr,
        server_name: ServerName<'static>,
        tls: Arc<ClientConfig>,
        instance: u64,
        content: Option<[u8; 32]>,
        key: &Keypair,
    ) -> Result<Self, String> {
        let (stream, hello) = timeout(DEADLINE, async {
            let socket = TcpStream::connect(address)
                .await
                .map_err(|_| "Cannot connect to chamber")?;
            socket
                .set_nodelay(true)
                .map_err(|_| "Cannot configure chamber client socket")?;
            let mut stream = TlsConnector::from(tls)
                .connect(server_name, socket)
                .await
                .map_err(|_| "Chamber TLS identity refused")?;
            let bytes = read_frame(&mut stream, MAX_RESPONSE_BYTES).await?;
            let hello: Hello = serde_json::from_slice(&bytes)
                .map_err(|_| "Malformed chamber opening challenge")?;
            if hello.version != VERSION || hello.challenge.instance() != instance {
                return Err("Chamber opening version or instance mismatch".to_string());
            }
            if hello.challenge.content() != content {
                return Err("Chamber scene or asset content mismatch".into());
            }
            Ok::<_, String>((stream, hello))
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
            stream: Some(stream),
            instance,
            tick: 0,
            control: None,
            next_request: 1,
            logged_in: false,
            player: false,
            inventory_revision: 0,
        };
        let response = client
            .request(Body::Authenticate {
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
        let response = timeout(DEADLINE, async {
            write_frame(&mut stream, &bytes, MAX_REQUEST_BYTES).await?;
            let bytes = read_frame(&mut stream, MAX_RESPONSE_BYTES).await?;
            serde_json::from_slice::<Response>(&bytes)
                .map_err(|_| "Malformed chamber response".to_string())
        })
        .await
        .map_err(|_| "Chamber request timed out")??;
        self.validate(request_id, &body, &response)?;
        self.tick = response.tick;
        if let Reply::Inventory { inventory } = &response.body {
            self.inventory_revision = inventory.revision;
        }
        let lost_control = self.player && response.control.is_none();
        self.control = response.control.clone();
        if !lost_control {
            self.stream = Some(stream);
        }
        Ok(response)
    }
    pub async fn inventory(&mut self) -> Result<super::wire::Inventory, String> {
        match self.request(Body::Inventory {}).await?.body {
            Reply::Inventory { inventory } => Ok(inventory),
            Reply::Refused { message, .. } => Err(message),
            _ => Err("Unexpected inventory response".into()),
        }
    }
    pub async fn claim_quest(&mut self, quest: u64) -> Result<Response, String> {
        let control = self.control().ok_or("Client has no admitted adventurer")?;
        self.request(Body::ClaimQuest {
            life: control.life,
            epoch: control.epoch,
            quest,
        })
        .await
    }
    pub async fn command(&mut self, intent: Intent<Ability>) -> Result<Response, String> {
        let control = self.control().ok_or("Client has no admitted adventurer")?;
        let command = Command {
            actor: control.life.into(),
            epoch: control.epoch,
            sequence: control
                .accepted_sequence
                .checked_add(1)
                .ok_or("Client command sequence exhausted")?,
            tick: self.tick,
            intent,
        };
        self.request(Body::Command {
            command: command.into(),
        })
        .await
    }
    pub async fn snapshot(&mut self) -> Result<State, String> {
        match self.request(Body::Snapshot {}).await?.body {
            Reply::Snapshot { state } => Ok(state),
            Reply::Refused { message, .. } => Err(message),
            _ => Err("Unexpected chamber snapshot outcome".into()),
        }
    }
    pub async fn events(&mut self, after: u64, limit: u16) -> Result<EventPage, String> {
        match self.request(Body::Events { after, limit }).await?.body {
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
        let response = self.request(Body::Events { after, limit }).await?;
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
        self.request(Body::Respawn { life }).await
    }
    pub async fn close(&mut self) -> Result<(), String> {
        let mut stream = self.stream.take().ok_or("Chamber client is disconnected")?;
        self.control = None;
        timeout(DEADLINE, stream.shutdown())
            .await
            .map_err(|_| "Chamber close timed out")?
            .map_err(|_| "Cannot close chamber transport".into())
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
            if control.life.instance != self.instance || (self.logged_in && !self.player) {
                return Err("Chamber response control identity mismatch".into());
            }
            if let Some(previous) = &self.control {
                if control.life.actor != previous.life.actor
                    || control.life.generation < previous.life.generation
                    || control.epoch < previous.epoch
                    || (control.epoch == previous.epoch
                        && (control.accepted_sequence < previous.accepted_sequence
                            || control.life.generation != previous.life.generation))
                {
                    return Err("Chamber response control fence regressed".into());
                }
            }
        } else if self.player && !matches!(r.body, Reply::Refused { .. }) {
            return Err("Chamber response lost player control".into());
        }
        match (&r.body, request) {
            (Reply::Refused { .. }, _) => Ok(()),
            (Reply::Accepted, Body::Authenticate { .. }) => Ok(()),
            (Reply::Accepted, Body::Command { command }) => {
                if r.control.as_ref().is_none_or(|c| {
                    c.life != command.actor
                        || c.epoch != command.epoch
                        || c.accepted_sequence < command.sequence
                }) {
                    return Err("Accepted chamber command has no matching acknowledgment".into());
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
            (Reply::Snapshot { state }, Body::Snapshot {}) => {
                state.validate_control(self.instance, &r.control)
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
                    || *revision > super::rewards::MAX_TRANSACTIONS as u64
                    || r.control
                        .as_ref()
                        .is_none_or(|c| c.life != *life || c.epoch != *epoch)
                {
                    return Err("Quest claim acknowledgment is incompatible".into());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::{
        net::tests::{gateway, key, start, tls},
        wire::{Action, Input, Life},
    };
    use tokio::{net::TcpListener, sync::oneshot};
    use tokio_rustls::TlsAcceptor;
    fn name() -> ServerName<'static> {
        ServerName::try_from("localhost").unwrap()
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
                target: 2,
                experience: 45,
                items: vec![Entry { id: 1, count: 1 }],
                quests: vec![Entry { id: 1, count: 1 }],
            }])
            .unwrap()
            .with_progression(super::super::progression::Config {
                version: 1,
                levels: vec![0, 100, 300],
                quests: vec![super::super::progression::Quest {
                    id: 1,
                    name: "Disrupt the summoning".into(),
                    objective: 1,
                    goal: 1,
                    experience: 75,
                    items: vec![Entry { id: 1, count: 2 }],
                }],
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
        let left = tokio::time::timeout(Duration::from_secs(4), async {
            loop {
                let inv = a.inventory().await.unwrap();
                if inv.experience == 45 {
                    break inv;
                }
                tokio::time::sleep(Duration::from_millis(33)).await;
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
        tokio::time::sleep(Duration::from_millis(70)).await;
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
        let exit = tokio::time::timeout(std::time::Duration::from_secs(3), server)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(exit.failure.as_deref(), Some("Cannot stage chamber commit"));
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
                target: 2,
                experience: 45,
                items: vec![],
                quests: vec![],
            }])
            .unwrap();
        let actor = g.game().player_life().actor;
        g.grant_reward(Transaction {
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
        let exit = tokio::time::timeout(std::time::Duration::from_secs(4), server)
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
        tokio::time::sleep(Duration::from_millis(30)).await;
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
        let task = tokio::spawn(async move {
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
    #[test]
    fn context_and_control_regressions_are_refused() {
        let life = Life {
            instance: 120,
            actor: 14,
            generation: 0,
        };
        let control = Control {
            life,
            epoch: 2,
            accepted_sequence: 3,
        };
        let client = Client {
            stream: None,
            instance: 120,
            tick: 10,
            control: Some(control.clone()),
            next_request: 2,
            logged_in: true,
            player: true,
            inventory_revision: 0,
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
        for field in 0..6 {
            let mut bad = response.clone();
            match field {
                0 => bad.version += 1,
                1 => bad.request_id += 1,
                2 => bad.instance += 1,
                3 => bad.tick -= 1,
                4 => bad.control.as_mut().unwrap().epoch -= 1,
                _ => bad.control.as_mut().unwrap().accepted_sequence -= 2,
            }
            assert!(client.validate(2, &request, &bad).is_err());
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
