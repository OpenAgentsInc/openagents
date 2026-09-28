//! Fixtures over the shared synthetic NIP-42 relay. It authenticates
//! publishers and delivers private artifacts only to author or recipient.
//! It is not a production relay and proves no deployment or retention.
use crate::client::{self, Pending};
use crate::host::{Dispatch, Host};
use crate::protocol::*;
use crate::*;
use nostr::domain::Event;
use secp256k1::SecretKey;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

mod flows;
#[path = "../../../coder-control/src/tests/relay.rs"]
mod relay;
mod supersede;

const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;

pub(super) fn key() -> SecretKey {
    SecretKey::new(&mut secp256k1::rand::rng())
}
pub(super) fn now() -> u64 {
    unix_time().unwrap()
}

/// Records admitted dispatches keyed by request ID, like an idempotent task owner.
#[derive(Clone, Default)]
pub(super) struct Recorder(Arc<Mutex<Vec<(String, String, String)>>>, SpendStub);

/// Answers `spend.list` with no requests and records the sender and grant;
/// records a receipt as sent.
#[derive(Clone, Default)]
pub(super) struct SpendStub(pub Arc<Mutex<Vec<(String, String)>>>);
impl crate::host::Spends for SpendStub {
    fn list(
        &mut self,
        device: &str,
        grant: &crate::spend::Grant,
        _now: u64,
    ) -> std::result::Result<Vec<crate::spend::Entry>, Code> {
        self.0
            .lock()
            .unwrap()
            .push((device.into(), grant.grant.clone()));
        Ok(vec![])
    }
    fn settle(
        &mut self,
        device: &str,
        receipt: &crate::spend::Receipt,
        _now: u64,
    ) -> std::result::Result<crate::spend::Receipt, Code> {
        self.0
            .lock()
            .unwrap()
            .push((device.into(), receipt.request.clone()));
        Ok(receipt.clone())
    }
}

impl Dispatch for Recorder {
    fn spends(&mut self) -> Option<&mut dyn crate::host::Spends> {
        Some(&mut self.1)
    }
    fn dispatch(
        &mut self,
        request: &str,
        device: &str,
        op: &Operation,
    ) -> std::result::Result<Receipt, Code> {
        let mut seen = self.0.lock().unwrap();
        if !seen.iter().any(|(r, _, _)| r == request) {
            seen.push((request.into(), device.into(), op.name().into()));
        }
        Ok(Receipt {
            operation: op.name().into(),
            reference: request.into(),
        })
    }
    /// Records the admitted grant and epoch in the device column, so a
    /// test can see what a deferred effect would recheck.
    fn dispatch_as(
        &mut self,
        request: &str,
        device: &str,
        grant: Option<(&str, u64)>,
        op: &Operation,
    ) -> std::result::Result<Receipt, Code> {
        let who = match grant {
            Some((grant, epoch)) => format!("{device} {grant} {epoch}"),
            None => device.to_owned(),
        };
        self.dispatch(request, &who, op)
    }
}
impl Recorder {
    pub(super) fn seen(&self) -> Vec<(String, String, String)> {
        self.0.lock().unwrap().clone()
    }
    pub(super) fn count(&self) -> usize {
        self.0.lock().unwrap().len()
    }
}

pub(super) struct Fixture {
    _temp: tempfile::TempDir,
    pub dir: PathBuf,
    pub owner: SecretKey,
    pub host_key: String,
    pub relay: String,
    pub recorder: Recorder,
    _relay_task: Option<tokio::task::JoinHandle<()>>,
    _serve_task: Option<tokio::task::JoinHandle<()>>,
    pub handled: Option<mpsc::UnboundedReceiver<std::result::Result<String, Code>>>,
}
impl Fixture {
    /// A host with a locally established owner and no relay.
    pub fn local() -> Self {
        Self::build("ws://127.0.0.1:9".into(), None)
    }
    fn build(relay: String, relay_task: Option<tokio::task::JoinHandle<()>>) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("access");
        let owner = key();
        let host_key = Host::new(&dir, POLICY).init(&pubkey(&owner)).unwrap();
        Self {
            _temp: temp,
            dir,
            owner,
            host_key,
            relay,
            recorder: Recorder::default(),
            _relay_task: relay_task,
            _serve_task: None,
            handled: None,
        }
    }
    /// A host served over a fresh synthetic relay. Each request opens a new
    /// `Host` over the private store, so every request crosses a restart.
    /// `offset` shifts the host clock; `drop_first` loses the first reply
    /// after it is committed, like a crash between consumption and reply.
    pub async fn served(offset: u64, drop_first: bool) -> Self {
        let (url, relay_task, _) = relay::start().await;
        let mut fixture = Self::build(url, Some(relay_task));
        let (sender, receiver) = mpsc::unbounded_channel();
        let (dir, relay, mut recorder) = (
            fixture.dir.clone(),
            fixture.relay.clone(),
            fixture.recorder.clone(),
        );
        let secret = fixture.host().key().unwrap();
        let mut socket = coder_connect::transport::Receiver::connect(&relay, &secret, POLICY)
            .await
            .unwrap();
        fixture._serve_task = Some(tokio::spawn(async move {
            let mut dropped = !drop_first;
            while let Ok(event) = socket.next_request().await {
                let host = Host::new(&dir, POLICY);
                let result = host.handle_with_clock(
                    &event,
                    &relay,
                    || Ok(unix_time()? + offset),
                    &mut recorder,
                );
                match result {
                    Ok(reply) => {
                        if dropped {
                            socket.publish(&reply).await.unwrap();
                        } else {
                            dropped = true;
                        }
                        let _ = sender.send(Ok(reply.id));
                    }
                    Err(error) => {
                        let _ = sender.send(Err(error.code));
                    }
                }
            }
        }));
        fixture.handled = Some(receiver);
        fixture
    }
    pub fn host(&self) -> Host {
        Host::new(&self.dir, POLICY)
    }
    pub fn owner_client(&self) -> Client {
        Client::owner(&self.host_key, &self.relay, self.owner, POLICY).unwrap()
    }
    pub fn invite(&self, rights: &str) -> String {
        let now = now();
        self.host()
            .invite(
                &self.relay,
                Rights::parse_list(rights).unwrap(),
                now,
                now + 3600,
            )
            .unwrap()
            .code
    }
    /// The host's result for the next handled request.
    pub async fn next_handled(&mut self) -> std::result::Result<String, Code> {
        tokio::time::timeout(
            Duration::from_secs(10),
            self.handled.as_mut().unwrap().recv(),
        )
        .await
        .unwrap()
        .unwrap()
    }
    /// Enroll a fresh device over the relay with the given rights.
    pub async fn enroll(&self, rights: &str) -> (SecretKey, Client) {
        let device = key();
        let access = client::redeem(&self.invite(rights), &device, POLICY)
            .await
            .unwrap();
        (device, Client::device(access, device, POLICY).unwrap())
    }
}

/// Send one exact packet with a short deadline. A timeout means no signed reply.
pub(super) async fn exchange(
    relay: &str,
    secret: &SecretKey,
    pending: &Pending,
    host: &str,
) -> Result<Event> {
    tokio::time::timeout(Duration::from_secs(4), async {
        let mut session = coder_connect::transport::Session::connect(relay, secret, POLICY).await?;
        session
            .exchange_event(
                &pending.event,
                &pending.request.request,
                (pending.request.issued_at, pending.request.expires_at),
                host,
                &pubkey(secret),
            )
            .await
    })
    .await
    .map_err(|_| Error::new(Code::Transport, "no signed reply before the test deadline"))?
    .map_err(Error::from)
}

/// Build and sign an arbitrary request, bypassing client-side checks.
pub(super) fn forge(
    secret: &SecretKey,
    host: &str,
    relay: &str,
    grant: Option<(&str, u64)>,
    op: Operation,
) -> Pending {
    let now = now();
    let request = Request {
        v: REQUEST.into(),
        requires: vec![],
        request: random_id(),
        host: host.into(),
        grant: grant.map(|(g, _)| g.into()),
        epoch: grant.map(|(_, e)| e),
        relay: relay.into(),
        issued_at: now,
        expires_at: now + 60,
        op,
    };
    let event = seal(
        &request,
        REQUEST,
        secret,
        host,
        &request.request,
        now,
        now + 60,
    )
    .unwrap();
    Pending { request, event }
}

pub(super) fn task() -> Operation {
    Operation::CreateTask {
        task: TaskCreate {
            title: "Synthetic task".into(),
            prompt: "Summarize the synthetic fixture".into(),
            workspace: "fixture".into(),
        },
    }
}
pub(super) fn terminal() -> Operation {
    Operation::OpenTerminal { cols: 80, rows: 24 }
}
