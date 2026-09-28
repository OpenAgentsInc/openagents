//! One OpenAgents app lifetime: the Computers surface, the terminal screen
//! it opens, and the Tailnet surface.

use crate::tailnet::{Client, Outcome as TailnetOutcome};
use crate::tailnet_view::{self, Intent as TailnetIntent, Screen as TailnetScreen};
use coder_computers::cache::Cache;
use coder_computers::live::{Live, Saved, Settings, Store, Terminals};
use coder_computers::terminal::screen::{Outcome as TerminalOutcome, Terminal, TerminalPacket};
use coder_computers::{Capabilities, Computers, InputRequest, LocalHost, Outcome, Platform};
use rust_native::{Activation, ValidatedView, View};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use url::Url;

const TAILNET_REFRESH_LIMIT: Duration = Duration::from_secs(20);
const TAILNET_SIGN_IN_LIMIT: Duration = Duration::from_secs(300);

#[derive(Deserialize)]
pub struct Config {
    /// A private directory for the app's encrypted records.
    pub state_dir: PathBuf,
    /// This device's Nostr secret key, from the platform's protected store.
    pub secret_hex: String,
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    /// The current packet, without other work.
    Snapshot,
    /// The app moved to the foreground or the background.
    Lifecycle {
        active: bool,
    },
    ComputersActivate {
        instance: String,
        revision: u64,
        node: String,
    },
    ComputersInput {
        token: String,
        value: String,
    },
    ComputersCancel {
        token: String,
    },
    ComputersRefresh,
    TailnetActivate {
        instance: String,
        revision: u64,
        node: String,
    },
    /// Read the tailnet again in the background.
    TailnetRefresh,
    /// The browser opened the sign-in page: wait for approval in the
    /// background.
    TailnetWaitForSignIn,
    TerminalActivate {
        instance: String,
        revision: u64,
        node: String,
    },
    /// Poll the terminal. `known` is the view revision the host has.
    TerminalPoll {
        known: Option<u64>,
    },
    TerminalResize {
        rows: u16,
        cols: u16,
    },
    TerminalText {
        text: String,
    },
    TerminalKey {
        key: String,
        #[serde(default)]
        ctrl: bool,
        #[serde(default)]
        alt: bool,
        #[serde(default)]
        shift: bool,
    },
    TerminalPaste {
        text: String,
    },
}

impl Request {
    fn terminal(&self) -> bool {
        matches!(
            self,
            Self::TerminalPoll { .. }
                | Self::TerminalResize { .. }
                | Self::TerminalText { .. }
                | Self::TerminalKey { .. }
                | Self::TerminalPaste { .. }
        )
    }
}

/// The invitation QR code the Computers surface shows: one string of `1`
/// (dark) and `0` (light) per module row, quiet zone included.
#[derive(Serialize)]
pub struct QrModules {
    pub size: usize,
    pub rows: Vec<String>,
}

#[derive(Serialize)]
pub struct Packet {
    pub schema: &'static str,
    /// This device's public key, for display.
    pub device: String,
    pub computers: Option<serde_json::Value>,
    /// A value the Computers surface asks the host to collect.
    pub computers_input: Option<InputRequest>,
    pub computers_qr: Option<QrModules>,
    pub tailnet: Option<serde_json::Value>,
    /// The Tailnet surface is reading in the background.
    pub tailnet_loading: bool,
    /// Open this URL in the browser, then send `tailnet_wait_for_sign_in`.
    pub open_url: Option<String>,
    /// A terminal screen is open; poll it with `terminal_poll`.
    pub terminal: bool,
    /// App-level problems, such as an unavailable host client.
    pub notices: Vec<String>,
}

/// The encrypted store for the Computers record, keyed by the device key.
struct ComputersStore(Cache);

impl Store for ComputersStore {
    fn load(&mut self) -> Result<Option<Saved>, String> {
        self.0.read("computers")
    }
    fn save(&mut self, saved: &Saved) -> Result<(), String> {
        self.0.write("computers", saved)
    }
}

struct TailnetState {
    screen: TailnetScreen,
    loading: bool,
}

pub struct App {
    runtime: tokio::runtime::Runtime,
    device: String,
    computers: Option<Computers>,
    terminals: Option<Terminals>,
    terminal: Option<Terminal>,
    tailnet_client: Result<Arc<Client>, String>,
    tailnet: Arc<Mutex<TailnetState>>,
    tailnet_revision: u64,
    tailnet_view: Option<ValidatedView<TailnetIntent>>,
    notices: Vec<String>,
}

impl App {
    pub fn new(config: Config) -> Result<Self, String> {
        let secret = SecretKey::from_str(&config.secret_hex).map_err(|_| "invalid device key")?;
        let device = secret
            .x_only_public_key(&secp256k1::Secp256k1::new())
            .0
            .to_string();
        // Both ring and aws-lc-rs are linked; TLS clients that use the
        // process default need one chosen. An earlier install is kept.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let mut notices = vec![];
        let mut terminals = None;
        // A phone never runs a host; it reaches hosts through the live
        // client, which keeps its grants in their own encrypted store.
        let service: Box<dyn coder_computers::ComputersService + Send> =
            match Cache::open(&config.state_dir.join("computers"), &secret).and_then(|cache| {
                let mut settings = Settings::new(Platform::Phone);
                settings.now = now;
                Live::open(
                    settings,
                    secret,
                    Box::new(ComputersStore(cache)),
                    runtime.handle().clone(),
                )
                .map_err(|error| coder_computers::describe(&error))
            }) {
                Ok(live) => {
                    terminals = Some(live.terminals());
                    Box::new(live)
                }
                Err(reason) => {
                    notices.push(format!("Computers unavailable: {reason}"));
                    Box::new(coder_computers::Unavailable::new(
                        device.clone(),
                        LocalHost::NotSupported,
                        now,
                    ))
                }
            };
        let capabilities = Capabilities {
            platform: Platform::Phone,
            camera: true,
        };
        let computers = match Computers::new(service, capabilities, format!("computers:{}", id())) {
            Ok(computers) => Some(computers),
            Err(error) => {
                notices.push(format!("Computers unavailable: {}", error.message));
                None
            }
        };
        let tailnet_client = Client::open(&config.state_dir.join("tailscale")).map(Arc::new);
        Ok(Self {
            runtime,
            device,
            computers,
            terminals,
            terminal: None,
            tailnet_client,
            tailnet: Arc::new(Mutex::new(TailnetState {
                screen: TailnetScreen::Loading,
                loading: false,
            })),
            tailnet_revision: 0,
            tailnet_view: None,
            notices,
        })
    }

    /// Answer one request as JSON: a terminal request with the terminal
    /// packet, anything else with the app packet.
    pub fn respond(&mut self, request: Request) -> Vec<u8> {
        if request.terminal() {
            let packet = self.terminal_request(request);
            return serde_json::to_vec(&packet).unwrap_or_default();
        }
        let packet = self.call(request);
        serde_json::to_vec(&packet).unwrap_or_default()
    }

    pub fn call(&mut self, request: Request) -> Packet {
        let mut open_url = None;
        match request {
            Request::Snapshot => {}
            Request::Lifecycle { active } => {
                if let Some(computers) = self.computers.as_mut() {
                    let _ = computers.set_active(active);
                }
                if active {
                    self.load_tailnet(None, TAILNET_REFRESH_LIMIT);
                }
            }
            Request::ComputersActivate {
                instance,
                revision,
                node,
            } => {
                let event = Activation {
                    instance,
                    revision,
                    node,
                };
                if let Some(computers) = self.computers.as_mut()
                    && computers.activate(&event) == Ok(Outcome::Terminal)
                {
                    self.open_terminal();
                }
            }
            Request::ComputersInput { token, value } => {
                if let Some(computers) = self.computers.as_mut() {
                    let _ = computers.submit(&token, &value);
                }
            }
            Request::ComputersCancel { token } => {
                if let Some(computers) = self.computers.as_mut() {
                    let _ = computers.cancel_input(&token);
                }
            }
            Request::ComputersRefresh => {
                if let Some(computers) = self.computers.as_mut() {
                    let _ = computers.refresh();
                }
            }
            Request::TailnetActivate {
                instance,
                revision,
                node,
            } => {
                let event = Activation {
                    instance,
                    revision,
                    node,
                };
                let intent = self
                    .tailnet_view
                    .as_ref()
                    .and_then(|view| view.activate(&event).ok())
                    .copied();
                match intent {
                    Some(TailnetIntent::SignIn) => {
                        if let TailnetScreen::SignIn(url) = &self.lock_tailnet().screen {
                            open_url = Some(url.to_string());
                        }
                    }
                    Some(TailnetIntent::Refresh) => self.load_tailnet(None, TAILNET_REFRESH_LIMIT),
                    None => {}
                }
            }
            Request::TailnetRefresh => self.load_tailnet(None, TAILNET_REFRESH_LIMIT),
            Request::TailnetWaitForSignIn => {
                let url = match &self.lock_tailnet().screen {
                    TailnetScreen::SignIn(url) => Some(url.clone()),
                    _ => None,
                };
                if let Some(url) = url {
                    self.load_tailnet(Some(url), TAILNET_SIGN_IN_LIMIT);
                }
            }
            Request::TerminalActivate {
                instance,
                revision,
                node,
            } => {
                let event = Activation {
                    instance,
                    revision,
                    node,
                };
                if let Some(terminal) = self.terminal.as_mut()
                    && terminal.activate(&event) == Ok(TerminalOutcome::Closed)
                {
                    self.terminal = None;
                }
            }
            Request::TerminalPoll { .. }
            | Request::TerminalResize { .. }
            | Request::TerminalText { .. }
            | Request::TerminalKey { .. }
            | Request::TerminalPaste { .. } => {
                let _ = self.terminal_request(request);
            }
        }
        self.packet(open_url)
    }

    fn open_terminal(&mut self) {
        let Some(computers) = self.computers.as_mut() else {
            return;
        };
        let Some(host) = computers.take_terminal() else {
            return;
        };
        let label = computers
            .snapshot()
            .host(&host)
            .map_or_else(|| "this computer".to_owned(), |record| record.label.clone());
        match Terminal::open(
            host,
            label,
            self.terminals.clone(),
            self.runtime.handle().clone(),
        ) {
            Ok(terminal) => self.terminal = Some(terminal),
            Err(error) => self.notices.push(format!("Terminal unavailable: {error}")),
        }
    }

    fn terminal_request(&mut self, request: Request) -> TerminalPacket {
        let Some(terminal) = self.terminal.as_mut() else {
            return TerminalPacket::closed();
        };
        let mut known = None;
        match request {
            Request::TerminalPoll { known: have } => known = have,
            Request::TerminalResize { rows, cols } => terminal.resize(rows, cols),
            Request::TerminalText { text } => terminal.text(&text),
            Request::TerminalKey {
                key,
                ctrl,
                alt,
                shift,
            } => terminal.key(&key, coder_vt::Modifiers { ctrl, alt, shift }),
            Request::TerminalPaste { text } => terminal.paste(&text),
            _ => {}
        }
        terminal.packet(known)
    }

    fn lock_tailnet(&self) -> std::sync::MutexGuard<'_, TailnetState> {
        self.tailnet
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    /// Read the tailnet on the runtime. A read already in flight wins.
    fn load_tailnet(&mut self, followup: Option<Url>, limit: Duration) {
        let client = match &self.tailnet_client {
            Ok(client) => client.clone(),
            Err(error) => {
                self.lock_tailnet().screen = TailnetScreen::Failed(error.clone());
                return;
            }
        };
        {
            let mut state = self.lock_tailnet();
            if state.loading {
                return;
            }
            state.loading = true;
        }
        let shared = self.tailnet.clone();
        let read = self
            .runtime
            .spawn(async move { client.devices(followup, limit).await });
        self.runtime.spawn(async move {
            // A panicked read still ends loading, with an error to retry.
            let screen = match read.await {
                Ok(Ok(TailnetOutcome::Devices(tailnet))) => TailnetScreen::Devices(tailnet),
                Ok(Ok(TailnetOutcome::SignIn(url))) => TailnetScreen::SignIn(url),
                Ok(Err(error)) => TailnetScreen::Failed(error),
                Err(_) => TailnetScreen::Failed("Reading the tailnet failed.".into()),
            };
            let mut state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
            state.screen = screen;
            state.loading = false;
        });
    }

    fn render_tailnet(&mut self) -> Option<serde_json::Value> {
        self.tailnet_revision += 1;
        let root = tailnet_view::root(&self.lock_tailnet().screen);
        let view = View::new("openagents.tailnet", self.tailnet_revision, root)
            .validate()
            .ok()?;
        let value = serde_json::to_value(view.view()).ok();
        self.tailnet_view = Some(view);
        value
    }

    fn packet(&mut self, open_url: Option<String>) -> Packet {
        let tailnet = self.render_tailnet();
        let computers = self.computers.as_ref();
        Packet {
            schema: "openagents.mobile.v1",
            device: self.device.clone(),
            computers: computers
                .and_then(Computers::view)
                .and_then(|view| serde_json::to_value(view.view()).ok()),
            computers_input: computers.and_then(Computers::input).cloned(),
            computers_qr: computers
                .and_then(Computers::invitation_qr)
                .map(|modules| QrModules {
                    size: modules.len(),
                    rows: modules
                        .iter()
                        .map(|row| {
                            row.iter()
                                .map(|dark| if *dark { '1' } else { '0' })
                                .collect()
                        })
                        .collect(),
                }),
            tailnet,
            tailnet_loading: self.lock_tailnet().loading,
            open_url,
            terminal: self.terminal.is_some(),
            notices: self.notices.clone(),
        }
    }

    #[cfg(test)]
    pub(crate) fn set_tailnet(&mut self, screen: TailnetScreen) {
        self.lock_tailnet().screen = screen;
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn id() -> String {
    use secp256k1::rand::RngCore;
    let mut bytes = [0u8; 16];
    secp256k1::rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
