//! **Connect a computer** (`SCR-22`) and **Connected** (`SCR-23`).
//!
//! The chat's **Connect a computer** offer and the row in Account >
//! Computers both open `SCR-22`: the camera, with **Point at the code on
//! your computer**, and **Paste a code** below it. The host draws the
//! camera and the paste field natively and hands Rust the text of one code;
//! Rust decides what it is ([`coder_computers::connect::classify`]) and
//! pairs in the background, so the screen shows **Connecting to…** instead
//! of blocking. A pairing that lands shows `SCR-23`: the computer's name, a
//! check, and **Done**, which closes the screen and returns to where it was
//! opened. The chat's offer then reads **Run Coder on** that computer.
//!
//! Nothing here holds authority. A code is a one-time capability the host
//! checks; this screen keeps no code once it is handed to the pairing, and
//! never logs one.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use coder_computers::connect::{MAX_CODE_BYTES, PairFailure, Paired, classify, clock_sentence};
use serde::Serialize;
use tokio::runtime::Handle;

/// Where the person gets the desktop app.
pub const GET_APP: &str = "Get OpenAgents for Mac at openagents.com/desktop.";

/// A pairing in flight.
pub type Pairing = Pin<Box<dyn Future<Output = Result<Paired, PairFailure>> + Send>>;

/// What pairs a code with a computer: the live service, or a test fake.
pub trait Pair: Send + Sync {
    /// Pair with the code's computer. The code has passed [`classify`].
    fn pair(&self, code: String) -> Pairing;
}

/// Which screen shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// `SCR-22`: the camera and **Paste a code**.
    Scan,
    /// A code is being paired.
    Connecting,
    /// `SCR-23`: the computer is connected.
    Connected,
}

/// The screen as the host draws it. Every word comes from here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct View {
    pub stage: Stage,
    pub title: &'static str,
    /// The line under the camera, while it scans.
    pub prompt: Option<&'static str>,
    /// The paste control's label, while it scans.
    pub paste: Option<&'static str>,
    /// Where to get the desktop app, while it scans.
    pub get_app: Option<&'static str>,
    /// What the last code came to, such as an expired code.
    pub notice: Option<String>,
    /// The computer's name, while connecting or once connected.
    pub computer: Option<String>,
    /// **Done**, once connected.
    pub done: Option<&'static str>,
    /// The longest code the host hands over.
    pub max_bytes: usize,
}

type Slot = Arc<Mutex<Option<Result<Paired, PairFailure>>>>;

#[derive(Debug)]
enum Phase {
    Closed,
    Scan,
    Connecting { slot: Slot },
    Connected { paired: Paired },
}

/// The Connect a computer screens' state.
pub struct Connect {
    phase: Phase,
    notice: Option<String>,
    pair: Option<Arc<dyn Pair>>,
    runtime: Option<Handle>,
    /// A computer paired here, for the app to reload and prefer.
    landed: Option<Paired>,
}

impl std::fmt::Debug for Connect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connect")
            .field("phase", &self.phase)
            .finish_non_exhaustive()
    }
}

impl Connect {
    /// Pair through `pair` on `runtime`. Without them a code is refused
    /// with a notice, as when the live client is unavailable.
    #[must_use]
    pub fn new(pair: Option<Arc<dyn Pair>>, runtime: Option<Handle>) -> Self {
        Self {
            phase: Phase::Closed,
            notice: None,
            pair,
            runtime,
            landed: None,
        }
    }

    /// Show `SCR-22`. A pairing already in flight keeps its screen.
    pub fn open(&mut self) {
        if matches!(self.phase, Phase::Closed | Phase::Connected { .. }) {
            self.phase = Phase::Scan;
            self.notice = None;
        }
    }

    /// Close the screens. A pairing in flight still finishes, and a computer
    /// it adds shows in the list, which the pairing's task wakes.
    pub fn close(&mut self) {
        self.phase = Phase::Closed;
        self.notice = None;
    }

    /// A scanned or pasted code.
    pub fn code(&mut self, text: &str) {
        if !matches!(self.phase, Phase::Scan) {
            return;
        }
        let scanned = match classify(text) {
            Ok(scanned) => scanned,
            Err(message) => {
                self.notice = Some(message);
                return;
            }
        };
        let (Some(pair), Some(runtime)) = (self.pair.clone(), self.runtime.clone()) else {
            self.notice = Some("Computers are unavailable on this phone right now.".into());
            return;
        };
        let slot: Slot = Arc::default();
        let task = pair.pair(scanned.text().to_owned());
        let filled = slot.clone();
        runtime.spawn(async move {
            let outcome = task.await;
            *lock(&filled) = Some(outcome);
            crate::wake::computers();
        });
        self.notice = None;
        self.phase = Phase::Connecting { slot };
    }

    /// Move a finished pairing on: `SCR-23` when it landed, back to the
    /// scanner with the reason when it did not. Returns the computer that
    /// just paired, once.
    pub fn poll(&mut self) -> Option<Paired> {
        if let Phase::Connecting { slot } = &self.phase {
            let outcome = lock(slot).take();
            match outcome {
                None => {}
                Some(Ok(paired)) => {
                    self.notice = paired.clock_off.map(clock_sentence);
                    self.landed = Some(paired.clone());
                    self.phase = Phase::Connected { paired };
                }
                Some(Err(failure)) => {
                    self.notice = Some(match failure.clock_off {
                        Some(off) => format!("{} {}", failure.message, clock_sentence(off)),
                        None => failure.message,
                    });
                    self.phase = Phase::Scan;
                }
            }
        }
        self.landed.take()
    }

    /// Whether a pairing is in flight.
    #[cfg(test)]
    #[must_use]
    pub fn busy(&self) -> bool {
        matches!(self.phase, Phase::Connecting { .. })
    }

    /// The screen, while it shows.
    #[must_use]
    pub fn view(&self) -> Option<View> {
        let scanning = |notice: Option<String>| View {
            stage: Stage::Scan,
            title: "Connect a computer",
            prompt: Some("Point at the code on your computer"),
            paste: Some("Paste a code"),
            get_app: Some(GET_APP),
            notice,
            computer: None,
            done: None,
            max_bytes: MAX_CODE_BYTES,
        };
        match &self.phase {
            Phase::Closed => None,
            Phase::Scan => Some(scanning(self.notice.clone())),
            Phase::Connecting { .. } => Some(View {
                stage: Stage::Connecting,
                title: "Connect a computer",
                prompt: None,
                paste: None,
                get_app: None,
                notice: Some("Connecting to your computer…".into()),
                computer: None,
                done: None,
                max_bytes: MAX_CODE_BYTES,
            }),
            Phase::Connected { paired } => Some(View {
                stage: Stage::Connected,
                title: "Connected",
                prompt: None,
                paste: None,
                get_app: None,
                notice: self.notice.clone(),
                computer: Some(paired.label.clone()),
                done: Some("Done"),
                max_bytes: MAX_CODE_BYTES,
            }),
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_computers::connect::PairedOver;
    use std::time::Duration;

    /// A fake computer: answers each code with what the test set.
    struct Fake(Result<Paired, PairFailure>);

    impl Pair for Fake {
        fn pair(&self, _: String) -> Pairing {
            let answer = self.0.clone();
            Box::pin(async move { answer })
        }
    }

    fn studio() -> Paired {
        Paired {
            host: "ab".repeat(32),
            label: "Studio Mac".into(),
            over: PairedOver::Iroh,
            clock_off: None,
        }
    }

    fn settle(connect: &mut Connect) -> Option<Paired> {
        for _ in 0..200 {
            if !connect.busy() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
            if let Some(paired) = connect.poll() {
                return Some(paired);
            }
        }
        connect.poll()
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap()
    }

    #[test]
    fn the_scanner_screen_says_what_to_point_at_and_offers_paste() {
        let mut connect = Connect::new(None, None);
        assert_eq!(connect.view(), None);
        connect.open();
        let view = connect.view().unwrap();
        assert_eq!(view.stage, Stage::Scan);
        assert_eq!(view.title, "Connect a computer");
        assert_eq!(view.prompt, Some("Point at the code on your computer"));
        assert_eq!(view.paste, Some("Paste a code"));
        assert_eq!(view.get_app, Some(GET_APP));
        assert_eq!(view.notice, None);
        connect.close();
        assert_eq!(connect.view(), None);
    }

    #[test]
    fn a_code_that_is_not_a_computers_is_refused_before_anything_is_dialed() {
        let mut connect = Connect::new(None, None);
        connect.open();
        connect.code("lnbc1...");
        let view = connect.view().unwrap();
        assert_eq!(view.stage, Stage::Scan);
        assert!(view.notice.unwrap().contains("isn't a code"));
    }

    #[test]
    fn a_paired_computer_shows_connected_then_done_closes() {
        let runtime = runtime();
        let mut connect = Connect::new(
            Some(Arc::new(Fake(Ok(studio())))),
            Some(runtime.handle().clone()),
        );
        connect.open();
        connect.code("openagents-connect:AQID");
        assert_eq!(connect.view().unwrap().stage, Stage::Connecting);
        let paired = settle(&mut connect).expect("the pairing lands");
        assert_eq!(paired.label, "Studio Mac");
        let view = connect.view().unwrap();
        assert_eq!(view.stage, Stage::Connected);
        assert_eq!(view.title, "Connected");
        assert_eq!(view.computer.as_deref(), Some("Studio Mac"));
        assert_eq!(view.done, Some("Done"));
        // The landed computer is handed over once.
        assert_eq!(connect.poll(), None);
        connect.close();
        assert_eq!(connect.view(), None);
    }

    #[test]
    fn a_refused_pairing_returns_to_the_scanner_with_its_reason_and_the_clock() {
        let runtime = runtime();
        let failure = PairFailure {
            message: "This code has expired. Show a new code on your computer and scan again."
                .into(),
            clock_off: Some(300),
        };
        let mut connect = Connect::new(
            Some(Arc::new(Fake(Err(failure)))),
            Some(runtime.handle().clone()),
        );
        connect.open();
        connect.code("openagents-connect:AQID");
        assert_eq!(settle(&mut connect), None);
        let view = connect.view().unwrap();
        assert_eq!(view.stage, Stage::Scan);
        let notice = view.notice.unwrap();
        assert!(notice.contains("expired"), "{notice}");
        assert!(notice.contains("clock is off by 5 minutes"), "{notice}");
    }

    #[test]
    fn a_connected_computer_with_an_off_clock_says_so() {
        let runtime = runtime();
        let mut paired = studio();
        paired.clock_off = Some(90);
        let mut connect = Connect::new(
            Some(Arc::new(Fake(Ok(paired)))),
            Some(runtime.handle().clone()),
        );
        connect.open();
        connect.code("openagents-connect:AQID");
        settle(&mut connect).unwrap();
        assert!(
            connect
                .view()
                .unwrap()
                .notice
                .unwrap()
                .contains("clock is off by 2 minutes")
        );
    }

    /// The app opens the scanner from its own packet, refuses a code that
    /// is not a computer's before dialing, closes it, and never carries the
    /// iroh key it was launched with.
    #[test]
    fn the_app_shows_connect_and_its_packet_never_carries_the_iroh_key() {
        use crate::{App, Config, Launch, Request};
        const SECRET: &str = "0000000000000000000000000000000000000000000000000000000000000003";
        const IROH: &str = "5f1b2e7d9c3a48b6a0e4c2d18f7b3a9e6d5c4b3a2918f7e6d5c4b3a291807f6e";
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::open(
            Config {
                state_dir: dir.path().to_path_buf(),
                secret_hex: SECRET.into(),
            },
            Launch {
                iroh_secret_hex: Some(IROH.into()),
                ..Launch::default()
            },
        )
        .unwrap();
        let packet = |bytes: Vec<u8>| -> serde_json::Value {
            let text = String::from_utf8(bytes).unwrap();
            assert!(!text.contains(IROH), "the packet carries the iroh key");
            serde_json::from_str(&text).unwrap()
        };
        assert!(packet(app.respond(Request::Snapshot))["connect"].is_null());
        let open = packet(app.respond(Request::ConnectOpen));
        assert_eq!(open["connect"]["stage"], "scan");
        assert_eq!(
            open["connect"]["prompt"],
            "Point at the code on your computer"
        );
        let refused = packet(app.respond(Request::ConnectCode {
            value: "coder-pair:AAAA".into(),
        }));
        assert_eq!(refused["connect"]["stage"], "scan");
        assert!(
            refused["connect"]["notice"]
                .as_str()
                .unwrap()
                .contains("Chats pairing code")
        );
        assert!(packet(app.respond(Request::ConnectClose))["connect"].is_null());
    }
}
