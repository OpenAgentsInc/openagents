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
}
impl Client {
    pub async fn connect(
        address: SocketAddr,
        server_name: ServerName<'static>,
        tls: Arc<ClientConfig>,
        instance: u64,
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
        let lost_control = self.player && response.control.is_none();
        self.control = response.control.clone();
        if !lost_control {
            self.stream = Some(stream);
        }
        Ok(response)
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
                let mut sources = std::collections::BTreeSet::new();
                let mut lives = std::collections::BTreeSet::new();
                let snapshot_sources: std::collections::BTreeSet<_> =
                    state.snapshot.actors.iter().map(|a| a.id).collect();
                if state.actors.len() != state.snapshot.actors.len()
                    || state.actors.iter().any(|a| {
                        a.life.instance != self.instance
                            || !sources.insert(a.source)
                            || !lives.insert(verse_engine::core::LifeId::from(a.life))
                    })
                    || state.snapshot.actors.iter().any(|a| {
                        !sources.contains(&a.id)
                            || !a.pos.iter().all(|v| v.is_finite())
                            || !a.yaw.is_finite()
                    })
                    || sources != snapshot_sources
                    || snapshot_sources.len() != state.snapshot.actors.len()
                {
                    return Err("Invalid chamber snapshot life bindings".into());
                }
                state.presentation.validate(self.instance, &state.actors)?;
                Ok(())
            }
            (Reply::Events { page }, Body::Events { after, limit }) => {
                let mut serial = *after;
                if page.events.len() > usize::from(*limit)
                    || page.next > page.latest
                    || page.latest < *after
                    || page.gap
                        != page
                            .oldest
                            .is_some_and(|oldest| after.saturating_add(1) < oldest)
                    || page.events.iter().any(|e| {
                        let invalid = e.instance != self.instance
                            || e.actor.is_some_and(|a| a.instance != self.instance)
                            || e.serial <= serial
                            || e.serial > page.latest
                            || e.tick > r.tick
                            || !e.time.is_finite()
                            || e.time < 0.;
                        serial = e.serial;
                        invalid
                    })
                    || page.next != serial
                {
                    return Err("Invalid chamber event cursor page".into());
                }
                Ok(())
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
            state_a
                .presentation
                .effects
                .iter()
                .filter(|e| e.shield > 0)
                .count(),
            2
        );
        let observed = spectator.snapshot().await.unwrap();
        assert_eq!(
            serde_json::to_vec(&state_a.actors).unwrap(),
            serde_json::to_vec(&observed.actors).unwrap()
        );
        assert!(spectator.events(0, 64).await.is_ok());
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
