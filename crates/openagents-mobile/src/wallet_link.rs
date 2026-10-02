//! Bringing the owner's wallet to a computer (`openagents wallet link`),
//! on the phone.
//!
//! While the app is open, the phone reads each connected computer's open
//! asks with NIP-HOST `wallet.link.list`, beside its spend requests. The
//! oldest waits on a sheet that names the computer and the six-digit code
//! the computer shows (`openagents_spark::link::code`). Only the owner's
//! **Approve** (after Face ID or the passcode) answers with the wallet seed
//! sealed to the computer's one-time key (`openagents_spark::link::seal`);
//! **Deny** answers with nothing. The seed leaves the wallet only inside
//! that envelope; it is never logged, put in the packet, or named in an
//! error. A computer without the operation (an older host) fails the list,
//! which is left alone for a while, as spend requests are.

use coder_computers::live::Terminals;
use coder_host::access::protocol::{Operation, Outcome};
use coder_host::access::wallet_link::{Ask, Sealed};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// How often the phone reads a computer's asks while the app is open.
const POLL_EVERY: Duration = Duration::from_secs(10);
/// How long a computer that could not be read is left alone.
const BACKOFF: Duration = Duration::from_secs(30);

/// How asks reach the computers.
pub trait Transport: Send + Sync {
    /// `wallet.link.list`: the computer's open asks.
    fn list(&self, host: &str) -> Result<Vec<Ask>, String>;
    /// `wallet.link.answer`: the sealed seed, or `None` for declined.
    fn answer(&self, host: &str, id: &str, sealed: Option<Sealed>) -> Result<(), String>;
}

/// The live transport: the Computers service's current link to each host.
pub struct Live {
    terminals: Terminals,
    handle: tokio::runtime::Handle,
}

impl Live {
    pub fn new(terminals: Terminals, handle: tokio::runtime::Handle) -> Self {
        Self { terminals, handle }
    }

    fn call(&self, host: &str, op: Operation) -> Result<Outcome, String> {
        let link = (self.terminals.links(host))().map_err(|error| error.to_string())?;
        self.handle
            .block_on(link.call(op))
            .map_err(|error| error.to_string())
    }
}

impl Transport for Live {
    fn list(&self, host: &str) -> Result<Vec<Ask>, String> {
        match self.call(host, Operation::ListWalletLinks {})? {
            Outcome::WalletLinks { links } => Ok(links),
            _ => Err("the computer did not list its wallet requests".into()),
        }
    }

    fn answer(&self, host: &str, id: &str, sealed: Option<Sealed>) -> Result<(), String> {
        match self.call(
            host,
            Operation::AnswerWalletLink {
                id: id.to_owned(),
                sealed,
            },
        )? {
            Outcome::WalletLinkAnswered { .. } => Ok(()),
            _ => Err("the computer did not record the answer".into()),
        }
    }
}

/// What the host shows.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct View {
    /// The oldest waiting ask.
    pub sheet: Option<Sheet>,
    /// An answer is being sent.
    pub busy: bool,
    /// What happened to the last answer, until dismissed.
    pub notice: Option<String>,
}

/// One computer's ask for the wallet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Sheet {
    pub id: String,
    pub host: String,
    /// The computer's name as this phone paired it.
    pub computer: String,
    /// The six digits the computer shows: `123 456`.
    pub code: String,
}

#[derive(Clone, Debug)]
struct Waiting {
    host: String,
    label: String,
    ask: Ask,
    code: String,
}

#[derive(Default)]
struct Shared {
    waiting: BTreeMap<(String, String), Waiting>,
    polling: bool,
    last_poll: Option<Instant>,
    backoff: BTreeMap<String, Instant>,
    busy: bool,
    notice: Option<String>,
}

/// The phone's side of wallet links.
#[derive(Clone, Default)]
pub struct Linking {
    shared: Arc<Mutex<Shared>>,
}

impl Linking {
    fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    /// Read again at the next poll.
    pub fn soon(&self) {
        let mut shared = self.lock();
        shared.last_poll = None;
        shared.backoff.clear();
    }

    /// Read the connected computers' asks in the background, at most every
    /// [`POLL_EVERY`]. `hosts` are the computers this phone may operate,
    /// with their labels.
    pub fn poll(&self, hosts: Vec<(String, String)>, transport: Arc<dyn Transport>) {
        {
            let mut shared = self.lock();
            if shared.polling || shared.last_poll.is_some_and(|at| at.elapsed() < POLL_EVERY) {
                return;
            }
            shared.polling = true;
            shared.last_poll = Some(Instant::now());
        }
        let worker = self.clone();
        std::thread::spawn(move || {
            worker.poll_now(&hosts, transport.as_ref(), now());
            worker.lock().polling = false;
        });
    }

    /// One pass over `hosts`. Blocking.
    pub(crate) fn poll_now(&self, hosts: &[(String, String)], transport: &dyn Transport, now: u64) {
        for (host, label) in hosts {
            if self
                .lock()
                .backoff
                .get(host)
                .is_some_and(|until| Instant::now() < *until)
            {
                continue;
            }
            match transport.list(host) {
                Ok(asks) => {
                    let mut shared = self.lock();
                    shared
                        .waiting
                        .retain(|(waiting_host, _), _| waiting_host != host);
                    for ask in asks {
                        if ask.validate().is_err() || ask.expires_at <= now {
                            continue;
                        }
                        let Ok(code) = openagents_spark::link::code(&ask.key) else {
                            continue;
                        };
                        shared.waiting.insert(
                            (host.clone(), ask.id.clone()),
                            Waiting {
                                host: host.clone(),
                                label: label.clone(),
                                ask,
                                code,
                            },
                        );
                    }
                }
                Err(_) => {
                    self.lock()
                        .backoff
                        .insert(host.clone(), Instant::now() + BACKOFF);
                }
            }
        }
    }

    /// Approve the ask: `seal` seals the wallet seed to the ask's key
    /// (`None` when this phone has no wallet), and the envelope is sent in
    /// the background.
    pub fn approve(
        &self,
        host: &str,
        id: &str,
        seal: impl FnOnce(&str) -> Option<Result<openagents_spark::link::Sealed, String>>,
        transport: Option<Arc<dyn Transport>>,
    ) {
        let Some(waiting) = self.take(host, id) else {
            return;
        };
        let sealed = match seal(&waiting.ask.key) {
            None => {
                self.put_back(waiting, "Set up your wallet on this phone first.");
                return;
            }
            Some(Err(_)) => {
                self.put_back(
                    waiting,
                    "Your wallet could not be sealed for that computer.",
                );
                return;
            }
            Some(Ok(sealed)) => sealed,
        };
        let wire = serde_json::to_value(&sealed)
            .ok()
            .and_then(|value| serde_json::from_value::<Sealed>(value).ok());
        let Some(wire) = wire else {
            self.put_back(
                waiting,
                "Your wallet could not be sealed for that computer.",
            );
            return;
        };
        let done = format!("Your wallet is on {} now.", waiting.label);
        self.send(waiting, Some(wire), done, transport);
    }

    /// Decline the ask: the computer is told, and nothing is sent.
    pub fn deny(&self, host: &str, id: &str, transport: Option<Arc<dyn Transport>>) {
        let Some(waiting) = self.take(host, id) else {
            return;
        };
        let done = format!("Declined. {} did not get your wallet.", waiting.label);
        self.send(waiting, None, done, transport);
    }

    /// Clear the last notice.
    pub fn dismiss(&self) {
        self.lock().notice = None;
    }

    fn take(&self, host: &str, id: &str) -> Option<Waiting> {
        let mut shared = self.lock();
        if shared.busy {
            return None;
        }
        shared.waiting.remove(&(host.to_owned(), id.to_owned()))
    }

    fn put_back(&self, waiting: Waiting, notice: &str) {
        let mut shared = self.lock();
        shared.notice = Some(notice.into());
        shared
            .waiting
            .insert((waiting.host.clone(), waiting.ask.id.clone()), waiting);
    }

    fn send(
        &self,
        waiting: Waiting,
        sealed: Option<Sealed>,
        done: String,
        transport: Option<Arc<dyn Transport>>,
    ) {
        let Some(transport) = transport else {
            self.put_back(
                waiting,
                "That computer isn't connected. Try again in a moment.",
            );
            return;
        };
        {
            let mut shared = self.lock();
            shared.busy = true;
            shared.notice = None;
        }
        let worker = self.clone();
        let run = move || {
            let result = transport.answer(&waiting.host, &waiting.ask.id, sealed);
            let mut shared = worker.lock();
            shared.busy = false;
            shared.notice = Some(match result {
                Ok(()) => done,
                Err(_) => format!(
                    "{} could not be reached. Run `openagents wallet link` there again.",
                    waiting.label
                ),
            });
        };
        if cfg!(test) {
            run();
        } else {
            std::thread::spawn(run);
        }
    }

    /// Whether an ask waits or an answer is being sent, so the host asks for
    /// packets.
    pub fn live(&self) -> bool {
        let shared = self.lock();
        !shared.waiting.is_empty() || shared.busy || shared.polling
    }

    pub fn view(&self) -> View {
        let now = now();
        let shared = self.lock();
        let mut waiting: Vec<&Waiting> = shared
            .waiting
            .values()
            .filter(|waiting| waiting.ask.expires_at > now)
            .collect();
        waiting.sort_by(|a, b| (a.ask.created_at, &a.ask.id).cmp(&(b.ask.created_at, &b.ask.id)));
        View {
            sheet: waiting.first().map(|waiting| Sheet {
                id: waiting.ask.id.clone(),
                host: waiting.host.clone(),
                computer: if waiting.label.trim().is_empty() {
                    waiting.ask.computer.clone()
                } else {
                    waiting.label.clone()
                },
                code: waiting.code.clone(),
            }),
            busy: shared.busy,
            notice: shared.notice.clone(),
        }
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_spark::link::Requester;
    use openagents_spark::seed::Seed;

    #[derive(Default)]
    struct Fake {
        asks: Mutex<Vec<Ask>>,
        answers: Mutex<Vec<(String, String, Option<Sealed>)>>,
    }

    impl Transport for Fake {
        fn list(&self, _host: &str) -> Result<Vec<Ask>, String> {
            Ok(self.asks.lock().unwrap().clone())
        }
        fn answer(&self, host: &str, id: &str, sealed: Option<Sealed>) -> Result<(), String> {
            self.answers
                .lock()
                .unwrap()
                .push((host.into(), id.into(), sealed));
            Ok(())
        }
    }

    const HOST: &str = "host-key";

    fn ask(key: &str) -> Ask {
        Ask {
            id: "ab".repeat(16),
            key: key.into(),
            computer: "studio".into(),
            created_at: now(),
            expires_at: now() + 600,
        }
    }

    fn seed() -> Seed {
        Seed::from_entropy(vec![9u8; 16]).unwrap()
    }

    fn listed(key: &str) -> (Linking, Arc<Fake>) {
        let fake = Arc::new(Fake::default());
        fake.asks.lock().unwrap().push(ask(key));
        let linking = Linking::default();
        linking.poll_now(&[(HOST.into(), "Studio Mac".into())], fake.as_ref(), now());
        (linking, fake)
    }

    #[test]
    fn a_listed_ask_shows_the_computer_and_its_code() {
        let computer = Requester::new().unwrap();
        let (linking, _) = listed(&computer.public_hex());
        let sheet = linking.view().sheet.expect("a sheet");
        assert_eq!(sheet.computer, "Studio Mac");
        assert_eq!(sheet.host, HOST);
        assert_eq!(sheet.code, computer.code());
        assert!(linking.live());
    }

    #[test]
    fn approve_seals_the_seed_to_the_asking_computer() {
        let computer = Requester::new().unwrap();
        let (linking, fake) = listed(&computer.public_hex());
        let id = linking.view().sheet.unwrap().id;
        let transport: Arc<dyn Transport> = fake.clone();
        linking.approve(
            HOST,
            &id,
            |key| Some(openagents_spark::link::seal(&seed(), key)),
            Some(transport),
        );
        let answers = fake.answers.lock().unwrap();
        let (host, answered, sealed) = &answers[0];
        assert_eq!((host.as_str(), answered.as_str()), (HOST, id.as_str()));
        let wire = sealed.clone().expect("an envelope");
        let envelope: openagents_spark::link::Sealed =
            serde_json::from_value(serde_json::to_value(&wire).unwrap()).unwrap();
        let opened = computer.open(&envelope).expect("the computer opens it");
        assert_eq!(opened.entropy, seed().entropy);
        let view = linking.view();
        assert!(view.sheet.is_none());
        assert_eq!(
            view.notice.as_deref(),
            Some("Your wallet is on Studio Mac now.")
        );
        // Nothing in the packet view names the seed.
        let json = serde_json::to_string(&view).unwrap();
        assert!(!json.contains(&seed().mnemonic), "{json}");
    }

    #[test]
    fn deny_answers_with_nothing() {
        let computer = Requester::new().unwrap();
        let (linking, fake) = listed(&computer.public_hex());
        let id = linking.view().sheet.unwrap().id;
        let transport: Arc<dyn Transport> = fake.clone();
        linking.deny(HOST, &id, Some(transport));
        assert_eq!(fake.answers.lock().unwrap()[0].2, None);
        assert!(linking.view().sheet.is_none());
        linking.dismiss();
        assert_eq!(linking.view().notice, None);
    }

    #[test]
    fn without_a_wallet_nothing_is_answered() {
        let computer = Requester::new().unwrap();
        let (linking, fake) = listed(&computer.public_hex());
        let id = linking.view().sheet.unwrap().id;
        let transport: Arc<dyn Transport> = fake.clone();
        linking.approve(HOST, &id, |_| None, Some(transport));
        assert!(fake.answers.lock().unwrap().is_empty());
        let view = linking.view();
        assert!(view.sheet.is_some());
        assert_eq!(
            view.notice.as_deref(),
            Some("Set up your wallet on this phone first.")
        );
    }
}
