//! **Nearby** on **Connect a computer** (`SCR-22`): the computers this
//! phone sees on its Wi-Fi, above the scanner.
//!
//! While the scanner shows and the app is in front, the phone listens for
//! computers over mDNS (`_openagents._udp`); the packet's
//! `nearby_listening` tells the Android host to hold its Wi-Fi multicast
//! lock exactly then. Tapping a computer pairs with it: both screens show
//! the same six-digit code, and the computer connects this phone only when
//! the person clicks **Connect** on it. The listing's names are untrusted
//! (anyone on the network can advertise one), so this screen says to
//! compare the code before the click.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use coder_host::client::nearby::{Code, NearbyBrowser, NearbyComputer, NearbyEvent};
use serde::Serialize;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

use crate::connect::Pairing;

/// Starting to listen, which binds the phone's endpoint first.
pub type Listening = Pin<Box<dyn Future<Output = Option<NearbyBrowser>> + Send>>;

/// What finds and pairs nearby computers: the live service, or a test fake.
pub trait Finder: Send + Sync {
    /// Start listening, or `None` when this phone cannot.
    fn listen(&self) -> Listening;
    /// Pair with `computer`; `show` gets the code to compare.
    fn pair(&self, computer: NearbyComputer, show: Box<dyn FnOnce(Code) + Send>) -> Pairing;
}

impl Finder for coder_computers::live::Pairing {
    fn listen(&self) -> Listening {
        let pairing = self.clone();
        Box::pin(async move { pairing.nearby().await })
    }
    fn pair(&self, computer: NearbyComputer, show: Box<dyn FnOnce(Code) + Send>) -> Pairing {
        let pairing = self.clone();
        Box::pin(async move { pairing.pair_nearby(&computer, phone_name(), show).await })
    }
}

/// The name this phone gives itself on the computer's prompt.
#[must_use]
pub fn phone_name() -> &'static str {
    if cfg!(target_os = "ios") {
        "iPhone"
    } else if cfg!(target_os = "android") {
        "Android phone"
    } else {
        "Phone"
    }
}

/// One computer in the list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Row {
    /// Sent back in `connect_nearby`.
    pub id: String,
    pub label: String,
}

/// The Nearby section, as the host draws it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct View {
    pub title: &'static str,
    pub computers: Vec<Row>,
    /// The line under the title while no computer shows.
    pub empty: Option<&'static str>,
}

pub(crate) type List = Arc<Mutex<BTreeMap<String, NearbyComputer>>>;

/// The nearby list and the code of a pairing in flight.
pub struct Nearby {
    finder: Option<Arc<dyn Finder>>,
    runtime: Option<Handle>,
    list: List,
    listener: Option<JoinHandle<()>>,
    code: Arc<Mutex<Option<Code>>>,
    /// The computer being paired, for the screen's words.
    chosen: Option<String>,
}

impl std::fmt::Debug for Nearby {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Nearby")
            .field("listening", &self.listening())
            .finish_non_exhaustive()
    }
}

impl Nearby {
    #[must_use]
    pub fn new(finder: Option<Arc<dyn Finder>>, runtime: Option<Handle>) -> Self {
        Self {
            finder,
            runtime,
            list: Arc::default(),
            listener: None,
            code: Arc::default(),
            chosen: None,
        }
    }

    /// Start listening, once.
    pub fn start(&mut self) {
        if self.listener.is_some() {
            return;
        }
        let (Some(finder), Some(runtime)) = (self.finder.clone(), self.runtime.clone()) else {
            return;
        };
        let list = self.list.clone();
        self.listener = Some(runtime.spawn(async move {
            use futures_util::StreamExt;
            let Some(browser) = finder.listen().await else {
                return;
            };
            let mut events = browser.events().await;
            while let Some(event) = events.next().await {
                apply(&list, event);
                crate::wake::computers();
            }
        }));
    }

    #[cfg(test)]
    pub(crate) fn list(&self) -> List {
        self.list.clone()
    }

    /// Stop listening and forget the list.
    pub fn stop(&mut self) {
        if let Some(listener) = self.listener.take() {
            listener.abort();
        }
        lock(&self.list).clear();
    }

    /// Whether the phone listens now: the Android host holds its multicast
    /// lock exactly then.
    #[must_use]
    pub fn listening(&self) -> bool {
        self.listener.is_some()
    }

    /// Pair with the listed computer `id`, or `None` when it is not listed.
    pub fn choose(&mut self, id: &str) -> Option<Pairing> {
        let finder = self.finder.clone()?;
        let computer = lock(&self.list).get(id).cloned()?;
        *lock(&self.code) = None;
        self.chosen = Some(name(&computer.label).to_owned());
        let code = self.code.clone();
        Some(finder.pair(
            computer,
            Box::new(move |shown| {
                *lock(&code) = Some(shown);
                crate::wake::computers();
            }),
        ))
    }

    /// The pairing finished; forget its code.
    pub fn finished(&mut self) {
        *lock(&self.code) = None;
        self.chosen = None;
    }

    /// What the screen says while a nearby pairing is in flight, and the
    /// code once it is known.
    #[must_use]
    pub fn progress(&self) -> Option<(String, Option<String>)> {
        let chosen = self.chosen.as_ref()?;
        Some(match *lock(&self.code) {
            Some(code) => (
                format!("Check that {chosen} shows this code, then click Connect on it."),
                Some(code.to_string()),
            ),
            None => (format!("Connecting to {chosen}…"), None),
        })
    }

    /// The Nearby section, while listening.
    #[must_use]
    pub fn view(&self) -> Option<View> {
        if !self.listening() {
            return None;
        }
        let mut computers: Vec<Row> = lock(&self.list)
            .iter()
            .map(|(id, computer)| Row {
                id: id.clone(),
                label: name(&computer.label).to_owned(),
            })
            .collect();
        computers.sort_by(|a, b| a.label.cmp(&b.label).then_with(|| a.id.cmp(&b.id)));
        let empty = computers
            .is_empty()
            .then_some("Computers on this Wi-Fi with OpenAgents open show here.");
        Some(View {
            title: "Nearby",
            computers,
            empty,
        })
    }
}

impl Drop for Nearby {
    fn drop(&mut self) {
        self.stop();
    }
}

/// A computer's name as it advertised it, or a plain word for none.
fn name(label: &str) -> &str {
    if label.trim().is_empty() {
        "Computer"
    } else {
        label
    }
}

/// The list's key for a computer: its `EndpointId` in hex.
#[must_use]
pub fn id(computer: &NearbyComputer) -> String {
    computer
        .endpoint
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn apply(list: &List, event: NearbyEvent) {
    let mut list = lock(list);
    match event {
        NearbyEvent::Found(computer) => {
            list.insert(id(&computer), computer);
        }
        NearbyEvent::Lost(endpoint) => {
            list.retain(|_, computer| computer.endpoint != endpoint);
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use coder_computers::connect::{PairFailure, Paired, PairedOver};

    pub(crate) fn computer(byte: u8, label: &str) -> NearbyComputer {
        let secret = openagents_connect::iroh::SecretKey::from_bytes(&[byte; 32]);
        NearbyComputer {
            endpoint: secret.public(),
            label: label.into(),
            addrs: vec!["192.168.1.20:4433".parse().unwrap()],
        }
    }

    /// Lists fixed computers; a pairing shows `code`, then answers.
    pub(crate) struct Fake {
        pub code: u32,
        pub answer: Result<Paired, PairFailure>,
    }

    impl Finder for Fake {
        fn listen(&self) -> Listening {
            Box::pin(async { None })
        }
        fn pair(&self, _: NearbyComputer, show: Box<dyn FnOnce(Code) + Send>) -> Pairing {
            show(Code::new(self.code).unwrap());
            let answer = self.answer.clone();
            Box::pin(async move { answer })
        }
    }

    pub(crate) fn studio() -> Paired {
        Paired {
            host: "ab".repeat(32),
            label: "Studio Mac".into(),
            over: PairedOver::Iroh,
            clock_off: None,
        }
    }

    #[test]
    fn found_and_lost_computers_come_and_go_by_endpoint() {
        let list: List = Arc::default();
        let studio = computer(1, "Studio Mac");
        apply(&list, NearbyEvent::Found(studio.clone()));
        apply(&list, NearbyEvent::Found(computer(2, "")));
        assert_eq!(lock(&list).len(), 2);
        apply(&list, NearbyEvent::Lost(studio.endpoint));
        let left: Vec<_> = lock(&list).values().map(|c| c.label.clone()).collect();
        assert_eq!(left, vec![String::new()]);
    }

    #[test]
    fn the_list_shows_only_while_listening_and_names_every_computer() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let mut nearby = Nearby::new(
            Some(Arc::new(Fake {
                code: 1,
                answer: Ok(studio()),
            })),
            Some(runtime.handle().clone()),
        );
        assert_eq!(nearby.view(), None);
        nearby.start();
        assert!(nearby.listening());
        let empty = nearby.view().unwrap();
        assert!(empty.computers.is_empty());
        assert!(empty.empty.is_some());
        apply(&nearby.list, NearbyEvent::Found(computer(2, "")));
        apply(&nearby.list, NearbyEvent::Found(computer(1, "Studio Mac")));
        let view = nearby.view().unwrap();
        let labels: Vec<_> = view.computers.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, vec!["Computer", "Studio Mac"]);
        assert_eq!(view.empty, None);
        nearby.stop();
        assert!(!nearby.listening());
        assert_eq!(nearby.view(), None);
    }

    fn connect(
        answer: Result<Paired, PairFailure>,
    ) -> (crate::connect::Connect, tokio::runtime::Runtime) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let connect = crate::connect::Connect::new(None, Some(runtime.handle().clone()))
            .with_nearby(Some(Arc::new(Fake {
                code: 482_913,
                answer,
            })));
        (connect, runtime)
    }

    fn settle(connect: &mut crate::connect::Connect) -> Option<Paired> {
        for _ in 0..200 {
            if let Some(paired) = connect.poll() {
                return Some(paired);
            }
            if connect
                .view()
                .is_some_and(|v| v.stage != crate::connect::Stage::Connecting)
            {
                return None;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        None
    }

    #[test]
    fn a_tapped_computer_shows_the_code_to_compare_then_connects() {
        let (mut connect, _runtime) = connect(Ok(studio()));
        assert!(!connect.listening());
        connect.open();
        assert!(connect.listening(), "the scanner listens");
        let studio_mac = computer(1, "Studio Mac");
        apply(
            &connect_list(&connect),
            NearbyEvent::Found(studio_mac.clone()),
        );
        let view = connect.view().unwrap();
        let listed = view.nearby.unwrap();
        assert_eq!(listed.computers[0].label, "Studio Mac");
        connect.nearby(&listed.computers[0].id);
        assert!(!connect.listening(), "pairing stops the listing");
        let view = connect.view().unwrap();
        assert_eq!(view.code.as_deref(), Some("482 913"));
        assert_eq!(
            view.notice.as_deref(),
            Some("Check that Studio Mac shows this code, then click Connect on it.")
        );
        assert!(view.nearby.is_none());
        assert_eq!(
            settle(&mut connect).map(|p| p.label),
            Some("Studio Mac".into())
        );
        let view = connect.view().unwrap();
        assert_eq!(view.stage, crate::connect::Stage::Connected);
        assert_eq!(view.code, None);
        assert!(!connect.listening());
    }

    #[test]
    fn a_computer_that_did_not_connect_returns_to_the_list_with_the_reason() {
        let (mut connect, _runtime) = connect(Err(PairFailure::new(
            "Studio Mac didn't connect this phone. If the codes matched, try again and click Connect on the computer.",
        )));
        connect.open();
        apply(
            &connect_list(&connect),
            NearbyEvent::Found(computer(1, "Studio Mac")),
        );
        connect.nearby(&id(&computer(1, "Studio Mac")));
        assert_eq!(settle(&mut connect), None);
        let view = connect.view().unwrap();
        assert_eq!(view.stage, crate::connect::Stage::Scan);
        assert!(
            view.notice
                .unwrap()
                .starts_with("Studio Mac didn't connect")
        );
        assert!(connect.listening(), "the scanner listens again");
        // An unlisted computer is refused with a sentence.
        connect.nearby("ff");
        assert_eq!(
            connect.view().unwrap().notice.as_deref(),
            Some("That computer isn't on this Wi-Fi anymore.")
        );
        // In the background the phone stops listening.
        connect.set_active(false);
        assert!(!connect.listening());
        connect.set_active(true);
        assert!(connect.listening());
        connect.close();
        assert!(!connect.listening());
    }

    fn connect_list(connect: &crate::connect::Connect) -> List {
        connect.nearby_list()
    }
}
