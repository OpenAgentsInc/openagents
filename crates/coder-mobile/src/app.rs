use coder_computers::cache::Cache;
use coder_computers::live::{Live, Settings as LiveSettings, Store as LiveStore};
use coder_computers::{
    Capabilities, Computers, InputRequest, LocalHost, Outcome, Platform, synthetic::Synthetic,
};
use coder_connect::{Client, ConnectionCode, ErrorCode, Observation, Query, RelayPolicy};
use coder_history::{
    CatalogCursor, CatalogPage, CatalogRequest, Chat, TranscriptCursor, TranscriptPage,
    TranscriptRequest,
};
use rust_native::{Activation, ValidatedView, View};
use secp256k1::{Keypair, Secp256k1, SecretKey};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    str::FromStr,
};

pub(crate) const CATALOG_WINDOW: usize = 24;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub cache_dir: PathBuf,
    pub secret_hex: String,
    #[serde(default)]
    pub synthetic: bool,
    /// Test launches only: admit `ws://` loopback relays for the Computers
    /// surface and treat its hosts as running on this machine, so a
    /// simulator or test reaches a host on the same computer. With
    /// `synthetic`, the reader and world stay synthetic and the Computers
    /// surface still uses the live service. A scanned or pasted value can
    /// never set it.
    #[serde(default)]
    pub loopback_test: bool,
    /// Push wakes through a relay's PL executor and a push gateway. Absent
    /// leaves push off; `push_token` then reports that it is off.
    #[serde(default)]
    pub push: Option<crate::push::PushConfig>,
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Snapshot,
    Connect {
        code: String,
    },
    Activate {
        instance: String,
        revision: u64,
        node: String,
    },
    Refresh,
    RefreshNow,
    Foreground {
        active: bool,
    },
    Disconnect,
    Follow {
        enabled: bool,
        page: Option<String>,
    },
    /// Activate a control on the separate Computers surface.
    ComputersActivate {
        instance: String,
        revision: u64,
        node: String,
    },
    /// Answer the Computers surface's current input request.
    ComputersInput {
        token: String,
        value: String,
    },
    ComputersCancel {
        token: String,
    },
    ComputersRefresh,
    /// Activate a control on the terminal screen a host's **Terminal**
    /// control opened.
    TerminalActivate {
        instance: String,
        revision: u64,
        node: String,
    },
    /// Leave the terminal screen.
    TerminalClose,
    /// The terminal screen's state. `known` is the view revision the native
    /// host already has; the view is sent only when newer. Answered with a
    /// `TerminalPacket` by [`App::respond`].
    TerminalPoll {
        #[serde(default)]
        known: Option<u64>,
    },
    /// The grid the terminal page fits. The first report opens the shell at
    /// that size; later ones resize it.
    TerminalResize {
        rows: u16,
        cols: u16,
    },
    /// Text typed on the native keyboard.
    TerminalText {
        text: String,
    },
    /// One key: an editing key's name, such as `backspace` or `up`, or one
    /// character, with the modifiers held on a hardware keyboard.
    TerminalKey {
        key: String,
        #[serde(default)]
        ctrl: bool,
        #[serde(default)]
        alt: bool,
        #[serde(default)]
        shift: bool,
    },
    /// The clipboard's text, after the terminal screen asked to paste.
    TerminalPaste {
        text: String,
    },
    /// The application became active or moved to the background. Each host
    /// supervisor probes after a short absence and replaces its connection
    /// after a long one.
    Lifecycle {
        active: bool,
    },
    /// The platform issued or reissued its push token: an APNs device token
    /// as lowercase hexadecimal, or an FCM registration token.
    PushToken {
        token: String,
    },
    /// Stop wakes: revoke the lease and forget the token at the gateway.
    PushDisable,
}

impl Request {
    fn push(&self) -> bool {
        matches!(self, Self::PushToken { .. } | Self::PushDisable)
    }

    /// A terminal request answered with the terminal screen's smaller
    /// packet by [`App::respond`], so polling never resends other views.
    pub fn terminal(&self) -> bool {
        matches!(
            self,
            Self::TerminalPoll { .. }
                | Self::TerminalResize { .. }
                | Self::TerminalText { .. }
                | Self::TerminalKey { .. }
                | Self::TerminalPaste { .. }
        )
    }

    fn computers(&self) -> bool {
        matches!(
            self,
            Self::ComputersActivate { .. }
                | Self::ComputersInput { .. }
                | Self::ComputersCancel { .. }
                | Self::ComputersRefresh
                | Self::TerminalActivate { .. }
                | Self::TerminalClose
                | Self::Lifecycle { .. }
        )
    }
}

/// The QR code of the invitation the Computers surface shows, rendered in
/// Rust for the native host to draw: one string of `1` (dark) and `0`
/// (light) per module row, quiet zone included.
#[derive(Serialize)]
pub struct QrModules {
    pub size: usize,
    pub rows: Vec<String>,
}

/// The encrypted cache that holds the Computers record. Its key comes from
/// the device identity in the platform's protected store.
struct ComputersCache(Cache);

impl LiveStore for ComputersCache {
    fn load(&mut self) -> Result<Option<coder_computers::live::Saved>, String> {
        self.0.read("computers")
    }
    fn save(&mut self, saved: &coder_computers::live::Saved) -> Result<(), String> {
        self.0.write("computers", saved)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Intent {
    Open { source: String },
    Back,
    Earlier,
    Later,
    Follow,
    Refresh,
    MoreChats,
    PreviousChats,
    NextChats,
    Reload,
    Raw { record: u64 },
    TextPart { record: u64, part: usize },
    Details,
    Disconnect,
}

/// What one native call returns: the application packet, or the terminal
/// screen's smaller packet for a terminal request.
#[derive(Serialize)]
#[serde(untagged)]
pub enum Reply {
    Packet(Box<Packet>),
    Terminal(coder_computers::terminal::screen::TerminalPacket),
}

#[derive(Serialize)]
pub struct Packet {
    pub schema: &'static str,
    pub public_key: String,
    pub paired: bool,
    /// This call saved a verified pairing. A first-read error is separate
    /// from admission and must not keep the user in the pairing form.
    pub pairing_completed: bool,
    pub reading: bool,
    pub status: String,
    pub error: Option<String>,
    pub follow_target: Option<String>,
    pub follow_page: Option<String>,
    pub view: Option<serde_json::Value>,
    /// The Computers surface: its own instance and revisions, independent
    /// of the reader's view.
    pub computers: Option<serde_json::Value>,
    /// A value the Computers surface asks the native host to collect.
    pub computers_input: Option<InputRequest>,
    /// The invitation QR code the Computers surface shows, if any.
    pub computers_qr: Option<QrModules>,
    /// First run finished on the last call; the host returns to its
    /// existing onboarding.
    pub computers_exit: bool,
    /// The terminal screen a host's **Terminal** control opened, as its own
    /// Rust Native view. See [`coder_computers::terminal::screen`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal: Option<serde_json::Value>,
    /// The terminal screen asked for the clipboard's text; the native host
    /// answers with `terminal_paste`.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub terminal_paste: bool,
    /// Push wake status, present once the app is configured for push or a
    /// push request arrived.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct CatalogState {
    pub snapshot: String,
    pub next: Option<CatalogCursor>,
    pub pages: u32,
    pub checked_at: u64,
    #[serde(default)]
    pub refresh_next: Option<CatalogCursor>,
    #[serde(default)]
    pub refresh_page: u32,
}

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct TranscriptState {
    pub cursor: Option<TranscriptCursor>,
    pub snapshot_bytes: u64,
    pub has_more: bool,
    pub pending_line: bool,
    pub checked_at: u64,
}

pub struct App {
    pub(crate) cache: Cache,
    pub(crate) secret: SecretKey,
    pub(crate) public_key: String,
    pub(crate) code: Option<ConnectionCode>,
    client: Option<Client>,
    runtime: tokio::runtime::Runtime,
    pub(crate) catalog: Vec<Chat>,
    pub(crate) catalog_state: CatalogState,
    pub(crate) catalog_start: usize,
    pub(crate) selected: Option<Chat>,
    pub(crate) transcript: TranscriptState,
    /// None follows the latest cached page. Other values select a stable page.
    pub(crate) window: Option<String>,
    pub(crate) raw: BTreeSet<u64>,
    pub(crate) text_parts: BTreeMap<u64, usize>,
    pub(crate) details: bool,
    retry_after: u64,
    pub(crate) status: String,
    pub(crate) error: Option<String>,
    pub(crate) notices: Vec<String>,
    pub(crate) instance: String,
    revision: u64,
    current: Option<ValidatedView<Intent>>,
    active: bool,
    pub(crate) synthetic: bool,
    computers: Option<Computers>,
    computers_exit: bool,
    /// The terminal screen a host's Terminal control opened.
    terminal: Option<coder_computers::terminal::screen::Terminal>,
    /// Reads each linked host's current link for terminal sessions.
    terminals: Option<coder_computers::live::Terminals>,
    pairing_completed: bool,
    push: Option<crate::push::Push>,
    push_status: Option<String>,
}

impl App {
    pub fn new(config: Config) -> Result<Self, String> {
        let secret =
            SecretKey::from_str(&config.secret_hex).map_err(|_| "invalid device identity")?;
        let public_key = Keypair::from_secret_key(&Secp256k1::new(), &secret)
            .x_only_public_key()
            .0
            .to_string();
        let cache = Cache::open(&config.cache_dir, &secret)?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|_| "mobile runtime unavailable")?;
        let mut app = Self {
            cache,
            secret,
            public_key,
            code: None,
            client: None,
            runtime,
            catalog: vec![],
            catalog_state: CatalogState::default(),
            catalog_start: 0,
            selected: None,
            transcript: TranscriptState::default(),
            window: None,
            raw: BTreeSet::new(),
            text_parts: BTreeMap::new(),
            details: false,
            retry_after: 0,
            status: "Not paired".into(),
            error: None,
            notices: vec![],
            instance: coder_connect::protocol::random_id(),
            revision: 0,
            current: None,
            active: false,
            synthetic: config.synthetic,
            computers: None,
            computers_exit: false,
            terminal: None,
            terminals: None,
            pairing_completed: false,
            push: None,
            push_status: None,
        };
        if let Some(push) = config.push.clone() {
            match crate::push::Push::open(
                push,
                &config.cache_dir,
                &app.secret,
                config.loopback_test,
            ) {
                Ok(push) => {
                    app.push_status = Some(push.status.clone());
                    app.push = Some(push);
                }
                Err(reason) => app.push_status = Some(format!("Wakes unavailable: {reason}")),
            }
        }
        // A phone never runs a host. The normal app reaches its computers
        // through the live host client; synthetic mode uses the fixture.
        // A test launch may pair the synthetic world and reader with the live
        // Computers service over loopback relays.
        let service: Box<dyn coder_computers::ComputersService + Send> =
            if config.synthetic && !config.loopback_test {
                Box::new(Synthetic::fixture(Platform::Phone, now))
            } else {
                match app.live_computers(&config) {
                    Ok(live) => {
                        app.terminals = Some(live.terminals());
                        Box::new(live)
                    }
                    Err(reason) => {
                        app.notices.push(format!("Computers unavailable: {reason}"));
                        Box::new(coder_computers::Unavailable::new(
                            app.public_key.clone(),
                            LocalHost::NotSupported,
                            now,
                        ))
                    }
                }
            };
        let capabilities = Capabilities {
            platform: Platform::Phone,
            camera: true,
        };
        let instance = format!("computers:{}", coder_connect::protocol::random_id());
        match Computers::new(service, capabilities, instance) {
            Ok(computers) => app.computers = Some(computers),
            Err(error) => app
                .notices
                .push(format!("Computers unavailable: {}", error.message)),
        }
        match app.cache.read::<ConnectionCode>("connection") {
            Ok(Some(code)) => match app.make_client(code.clone()) {
                Ok(client) => {
                    app.code = Some(code);
                    app.client = Some(client);
                    if let Err(e) = app.restore_catalog() {
                        app.error = Some(e);
                    }
                    app.status = "Cached".into();
                }
                Err(_) => {
                    app.cache.erase("")?;
                    app.error =
                        Some("Pairing expired or belongs to another device. Pair again.".into());
                }
            },
            Ok(None) => {}
            Err(error) => {
                app.error = Some(error);
            }
        }
        if config.synthetic {
            app.seed_synthetic()?;
        }
        app.rebuild()?;
        Ok(app)
    }

    /// The live Computers service. Its grants live in their own encrypted
    /// cache directory, so erasing the chats pairing never erases them.
    fn live_computers(&self, config: &Config) -> Result<Live, String> {
        let cache = Cache::open(&config.cache_dir.join("computers"), &self.secret)?;
        let mut settings = LiveSettings::new(Platform::Phone);
        settings.now = now;
        if config.loopback_test {
            settings.policy = RelayPolicy::LoopbackTest;
            settings.locality = coder_computers::live::Locality::SameMachine;
        }
        Live::open(
            settings,
            self.secret,
            Box::new(ComputersCache(cache)),
            self.runtime.handle().clone(),
        )
        .map_err(|error| coder_computers::describe(&error))
    }

    fn policy(&self) -> RelayPolicy {
        if self.synthetic {
            RelayPolicy::LoopbackTest
        } else {
            RelayPolicy::Production
        }
    }
    fn make_client(&self, code: ConnectionCode) -> coder_connect::Result<Client> {
        Client::new_with_policy(code, self.secret, self.policy())
    }

    /// Answer one native call: a terminal request with the terminal
    /// screen's packet, anything else with the application packet.
    pub fn respond(&mut self, request: Request) -> Reply {
        if request.terminal() {
            Reply::Terminal(self.terminal_request(request))
        } else {
            Reply::Packet(Box::new(self.call(request)))
        }
    }

    fn terminal_request(
        &mut self,
        request: Request,
    ) -> coder_computers::terminal::screen::TerminalPacket {
        let Some(terminal) = self.terminal.as_mut() else {
            return coder_computers::terminal::screen::TerminalPacket::closed();
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

    pub fn call(&mut self, request: Request) -> Packet {
        self.pairing_completed = false;
        if request.terminal() {
            let _ = self.terminal_request(request);
            return self.packet();
        }
        // Lifecycle callbacks can follow an in-flight pairing call. They must
        // not erase its failure before the user can read it and retry.
        if request.push() {
            self.push_request(request);
            return self.packet();
        }
        if request.computers() {
            // Computers refusals show on that surface; the reader's error,
            // view, and pairing state stay as they were.
            self.computers(request);
            let packet = self.packet();
            self.computers_exit = false;
            return packet;
        }
        if !matches!(
            request,
            Request::Snapshot | Request::Foreground { .. } | Request::Refresh | Request::RefreshNow
        ) {
            self.error = None;
        }
        let result = self.handle(request);
        if let Err(error) = result {
            self.error = Some(error);
        }
        if let Err(error) = self.rebuild() {
            self.error = Some(error);
            self.current = None;
        }
        self.packet()
    }

    fn push_request(&mut self, request: Request) {
        let Some(push) = self.push.as_mut() else {
            // Keep a configuration refusal from startup; otherwise say push
            // is off.
            if self.push_status.is_none() {
                self.push_status = Some("Push notifications are off in this build.".into());
            }
            return;
        };
        let result = match request {
            Request::PushToken { token } => push.token(
                &self.runtime,
                &self.secret,
                &self.public_key,
                token.trim(),
                now(),
            ),
            Request::PushDisable => {
                push.disable(&self.runtime, &self.secret, &self.public_key, now())
            }
            _ => return,
        };
        self.push_status = Some(match result {
            Ok(()) => push.status.clone(),
            Err(reason) => format!("{}: {reason}", push.status),
        });
    }

    fn computers(&mut self, request: Request) {
        match request {
            Request::TerminalClose => {
                self.terminal = None;
                return;
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
                    && terminal.activate(&event)
                        == Ok(coder_computers::terminal::screen::Outcome::Closed)
                {
                    self.terminal = None;
                }
                return;
            }
            _ => {}
        }
        let Some(computers) = self.computers.as_mut() else {
            return;
        };
        // A refusal is already the screen's notice.
        let outcome = match request {
            Request::ComputersActivate {
                instance,
                revision,
                node,
            } => computers.activate(&Activation {
                instance,
                revision,
                node,
            }),
            Request::ComputersInput { token, value } => computers.submit(&token, &value),
            Request::ComputersCancel { token } => computers.cancel_input(&token),
            Request::ComputersRefresh => computers
                .refresh()
                .map(|()| Outcome::Updated)
                .map_err(coder_computers::Refusal::Failed),
            Request::Lifecycle { active } => computers
                .set_active(active)
                .map(|()| Outcome::Updated)
                .map_err(coder_computers::Refusal::Failed),
            _ => return,
        };
        self.computers_exit = outcome == Ok(Outcome::ContinueOnboarding);
        // The Terminal control passed the shared authority check; the
        // terminal screen takes over for that host (#9733).
        if outcome == Ok(Outcome::Terminal)
            && let Some(host) = computers.take_terminal()
        {
            let label = computers
                .snapshot()
                .host(&host)
                .map_or_else(|| "this computer".to_owned(), |record| record.label.clone());
            match coder_computers::terminal::screen::Terminal::open(
                host,
                label,
                self.terminals.clone(),
                self.runtime.handle().clone(),
            ) {
                Ok(terminal) => self.terminal = Some(terminal),
                Err(error) => self.notices.push(format!("Terminal unavailable: {error}")),
            }
        }
    }

    fn handle(&mut self, request: Request) -> Result<(), String> {
        if !matches!(request, Request::Connect { .. } | Request::Disconnect)
            && self.code.as_ref().is_some_and(|c| c.expires_at <= now())
        {
            self.disconnect()?;
            return Err("Pairing expired. Pair again on the computer.".into());
        }
        match request {
            Request::Snapshot => Ok(()),
            // `call` routes these to the Computers surface first.
            Request::ComputersActivate { .. }
            | Request::ComputersInput { .. }
            | Request::ComputersCancel { .. }
            | Request::ComputersRefresh
            | Request::TerminalActivate { .. }
            | Request::TerminalClose
            | Request::TerminalPoll { .. }
            | Request::TerminalResize { .. }
            | Request::TerminalText { .. }
            | Request::TerminalKey { .. }
            | Request::TerminalPaste { .. }
            | Request::Lifecycle { .. }
            | Request::PushToken { .. }
            | Request::PushDisable => Ok(()),
            Request::Follow { enabled, page } => {
                let keys = self.page_keys()?;
                self.window = if enabled {
                    None
                } else {
                    Some(
                        keys.into_iter()
                            .find(|key| Some(key) == page.as_ref())
                            .ok_or("This page needs to reload.")?,
                    )
                };
                Ok(())
            }
            Request::Foreground { active } => {
                let entering = active && !self.active;
                self.active = active;
                if entering && self.client.is_some() && self.error.is_none() {
                    return self.force_refresh();
                }
                Ok(())
            }
            Request::Disconnect => self.disconnect(),
            Request::Connect { code } => {
                let text = code.trim();
                let code = if text.starts_with("coder-pair:") {
                    self.runtime
                        .block_on(coder_connect::pairing::redeem(
                            text,
                            &self.secret,
                            self.policy(),
                        ))
                        .map_err(|e| pairing_error(&e))?
                } else {
                    ConnectionCode::parse(text.as_bytes()).map_err(|_| {
                        "Invalid pairing code. Scan or paste the code from your computer."
                            .to_owned()
                    })?
                };
                let client = self.make_client(code.clone()).map_err(|e| e.to_string())?;
                self.disconnect()?;
                self.cache.write("connection", &code)?;
                self.code = Some(code);
                self.client = Some(client);
                self.pairing_completed = true;
                self.status = "Loading chats…".into();
                // The explicit pairing action includes the first bounded read.
                // A failed read retains the grant and reports a sync error.
                self.refresh_catalog(true)
            }
            Request::Refresh => self.refresh(),
            Request::RefreshNow => self.force_refresh(),
            Request::Activate {
                instance,
                revision,
                node,
            } => {
                let intent = self
                    .current
                    .as_ref()
                    .ok_or("view is unavailable")?
                    .activate(&Activation {
                        instance,
                        revision,
                        node,
                    })
                    .map_err(|_| "Screen changed. Try the action again.")?
                    .clone();
                self.intent(intent)
            }
        }
    }

    fn intent(&mut self, intent: Intent) -> Result<(), String> {
        match intent {
            Intent::Open { source } => {
                let chat = self
                    .catalog
                    .iter()
                    .find(|c| c.source_id.as_deref() == Some(&source))
                    .cloned()
                    .ok_or("Chat is no longer in the current list.")?;
                self.transcript = self
                    .cache
                    .read(&format!("chat_{source}"))?
                    .unwrap_or_default();
                self.selected = Some(chat);
                self.window = None;
                self.raw.clear();
                self.text_parts.clear();
                self.details = false;
                self.retry_after = 0;
                self.status = if self.transcript.cursor.is_some() {
                    "Cached"
                } else {
                    "Loading chat…"
                }
                .into();
                self.refresh()
            }
            Intent::Back => {
                self.selected = None;
                self.raw.clear();
                self.details = false;
                Ok(())
            }
            Intent::Earlier | Intent::Later => {
                let keys = self.page_keys()?;
                let index = self.window_index(&keys);
                let next = if matches!(intent, Intent::Earlier) {
                    index.saturating_sub(1)
                } else {
                    index.saturating_add(1).min(keys.len().saturating_sub(1))
                };
                self.window = keys.get(next).cloned();
                Ok(())
            }
            Intent::Follow => {
                self.window = None;
                Ok(())
            }
            Intent::Refresh => self.force_refresh(),
            Intent::MoreChats => self.refresh_catalog(false),
            Intent::PreviousChats => {
                self.catalog_start = self.catalog_start.saturating_sub(CATALOG_WINDOW);
                Ok(())
            }
            Intent::NextChats => {
                self.catalog_start = (self.catalog_start + CATALOG_WINDOW)
                    .min(self.catalog.len().saturating_sub(1) / CATALOG_WINDOW * CATALOG_WINDOW);
                Ok(())
            }
            Intent::Reload => {
                self.retry_after = 0;
                if let Some(source) = self.source() {
                    self.cache.erase(&format!("page_{source}_"))?;
                    self.cache.erase(&format!("chat_{source}"))?;
                    self.transcript = TranscriptState::default();
                    self.window = None;
                }
                self.refresh()
            }
            Intent::TextPart { record, part } => {
                self.text_parts.insert(record, part);
                Ok(())
            }
            Intent::Raw { record } => {
                if !self.raw.remove(&record) {
                    self.raw.insert(record);
                }
                Ok(())
            }
            Intent::Details => {
                self.details = !self.details;
                Ok(())
            }
            Intent::Disconnect => self.disconnect(),
        }
    }

    fn disconnect(&mut self) -> Result<(), String> {
        self.client = None;
        self.code = None;
        self.catalog.clear();
        self.catalog_state = CatalogState::default();
        self.catalog_start = 0;
        self.selected = None;
        self.transcript = TranscriptState::default();
        self.raw.clear();
        self.notices.clear();
        self.retry_after = 0;
        self.text_parts.clear();
        self.details = false;
        self.status = "Not paired".into();
        self.cache.erase("")
    }

    fn restore_catalog(&mut self) -> Result<(), String> {
        self.catalog_state = self.cache.read("catalog_state")?.unwrap_or_default();
        if self.catalog_state.pages > 128 {
            return Err("cached catalog exceeds its page bound".into());
        }
        for index in 0..self.catalog_state.pages {
            let key = format!("catalog_{}_{}", self.catalog_state.snapshot, index);
            if let Some(page) = self.cache.read::<CatalogPage>(&key)? {
                self.catalog.extend(page.entries);
            } else {
                self.notices
                    .push("Some chats need to reload. Refresh the list.".into());
            }
        }
        Ok(())
    }

    fn observe(&mut self, query: Query) -> Result<Observation, String> {
        let client = self
            .client
            .as_ref()
            .ok_or("Pair this phone with the computer first.")?;
        let outcome = self.runtime.block_on(async {
            tokio::time::timeout(std::time::Duration::from_secs(8), client.observe(query)).await
        });
        let outcome = outcome.unwrap_or_else(|_| {
            Err(coder_connect::Error::new(
                ErrorCode::Transport,
                "The computer didn't answer in time. Saved chats are still here.",
            ))
        });
        match outcome {
            Ok(result) => {
                self.retry_after = 0;
                self.error = None;
                self.status = "Connected".into();
                Ok(result)
            }
            Err(error) => {
                self.retry_after = now().saturating_add(if error.code == ErrorCode::RateLimited {
                    60
                } else {
                    15
                });
                match error.code {
                    ErrorCode::Revoked | ErrorCode::Expired | ErrorCode::Forbidden => {
                        self.disconnect()?;
                        Err("This phone's access ended and its saved chats were removed. Pair again on the computer.".into())
                    }
                    ErrorCode::SourceChanged => {
                        self.status = "Chats changed · reload".into();
                        Err("Chat files changed. Reload the chat or pair again.".into())
                    }
                    ErrorCode::Conflict => {
                        self.catalog_state.next = None;
                        self.catalog_state.refresh_next = None;
                        self.status = "Chat list changed · refresh".into();
                        Err("Chat list changed. Refresh to reload it.".into())
                    }
                    _ => {
                        self.status = "Offline".into();
                        Err(match error.code {
                            ErrorCode::Transport | ErrorCode::Unavailable => {
                                "Computer unavailable. Keep its pairing command running.".into()
                            }
                            ErrorCode::RateLimited => "Computer busy. Retrying shortly.".into(),
                            _ => error.to_string(),
                        })
                    }
                }
            }
        }
    }

    fn force_refresh(&mut self) -> Result<(), String> {
        self.retry_after = 0;
        self.catalog_state.checked_at = 0;
        self.refresh()
    }

    fn refresh(&mut self) -> Result<(), String> {
        if !self.active || now() < self.retry_after {
            return Ok(());
        }
        if self.client.is_none() {
            return Ok(());
        }
        if self.selected.is_some() {
            let start = std::time::Instant::now();
            for _ in 0..8 {
                self.refresh_transcript()?;
                if !self.transcript.has_more || start.elapsed() >= std::time::Duration::from_secs(2)
                {
                    break;
                }
            }
            Ok(())
        } else {
            // A catalog refresh starts from its first page after the prior snapshot
            // is complete. Intermediate pages retain their membership cursor.
            if self.catalog_state.next.is_none()
                && self.catalog_state.refresh_next.is_none()
                && now().saturating_sub(self.catalog_state.checked_at) < 30
            {
                return Ok(());
            }
            self.refresh_catalog(
                self.catalog_state.next.is_none() && self.catalog_state.refresh_next.is_none(),
            )
        }
    }

    fn refresh_catalog(&mut self, restart: bool) -> Result<(), String> {
        let request = CatalogRequest {
            cursor: if restart {
                None
            } else {
                self.catalog_state
                    .refresh_next
                    .clone()
                    .or_else(|| self.catalog_state.next.clone())
            },
            limit: 32,
        };
        let Observation::Catalog(page) = self.observe(Query::Catalog(request))? else {
            return Err("computer returned another page type".into());
        };
        self.apply_catalog(page, restart)
    }

    pub(crate) fn apply_catalog(&mut self, page: CatalogPage, restart: bool) -> Result<(), String> {
        if (restart || self.catalog_state.refresh_next.is_some())
            && page.snapshot == self.catalog_state.snapshot
            && self.catalog_state.pages > 0
        {
            // Walk mutable metadata without removing the already visible rows.
            let index = if restart {
                0
            } else {
                self.catalog_state.refresh_page
            };
            self.cache
                .write(&format!("catalog_{}_{}", page.snapshot, index), &page)?;
            for updated in page.entries {
                if let Some(existing) = self
                    .catalog
                    .iter_mut()
                    .find(|chat| chat.id == updated.id && chat.source_id == updated.source_id)
                {
                    *existing = updated;
                }
            }
            self.catalog_state.refresh_page = index + 1;
            self.catalog_state.refresh_next = if index + 1 < self.catalog_state.pages {
                page.next
            } else {
                None
            };
            self.catalog_state.checked_at = now();
            self.notices = page
                .notices
                .iter()
                .map(|n| format!("Notice: {}", n.code))
                .collect();
            return self.cache.write("catalog_state", &self.catalog_state);
        }
        if restart || page.snapshot != self.catalog_state.snapshot {
            self.catalog.clear();
            self.catalog_state = CatalogState::default();
            self.catalog_state.snapshot = page.snapshot.clone();
        }
        if self.catalog_state.pages >= 128 {
            return Err(
                "This computer has more than 4,096 chats. Only the first 4,096 show.".into(),
            );
        }
        let key = format!("catalog_{}_{}", page.snapshot, self.catalog_state.pages);
        self.cache.write(&key, &page)?;
        self.notices = page
            .notices
            .iter()
            .map(|n| format!("Notice: {}", n.code))
            .collect();
        self.catalog_state.pages += 1;
        self.catalog_state.next = page.next.clone();
        self.catalog_state.checked_at = now();
        self.cache.write("catalog_state", &self.catalog_state)?;
        self.catalog.extend(page.entries);
        self.catalog_start = self
            .catalog_start
            .min(self.catalog.len().saturating_sub(1) / CATALOG_WINDOW * CATALOG_WINDOW);
        Ok(())
    }

    fn refresh_transcript(&mut self) -> Result<(), String> {
        let source = self.source().ok_or("select a chat first")?;
        let request = TranscriptRequest {
            source_id: source,
            cursor: self.transcript.cursor.clone(),
            max_bytes: coder_history::MAX_PAGE_BYTES,
            end: None,
        };
        let Observation::Page(page) = self.observe(Query::Page(request))? else {
            return Err("computer returned another page type".into());
        };
        self.apply_page(page)
    }

    pub(crate) fn apply_page(&mut self, page: TranscriptPage) -> Result<(), String> {
        let source = self.source().ok_or("select a chat first")?;
        if page.source_id != source
            || page.next.source_id != source
            || page.next.incarnation != page.incarnation
        {
            return Err("This chat changed. Reload it.".into());
        }
        let start = self.transcript.cursor.as_ref().map_or(0, |c| c.offset);
        if self
            .transcript
            .cursor
            .as_ref()
            .is_some_and(|c| c.incarnation != page.incarnation)
        {
            return Err("This chat changed. Reload it.".into());
        }
        let mut offset = start;
        use base64::Engine;
        for chunk in &page.chunks {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&chunk.raw_base64)
                .map_err(|_| "Couldn't read this chat. Reload it.")?;
            if chunk.offset != offset
                || chunk.end_offset <= offset
                || chunk.end_offset - offset != bytes.len() as u64
            {
                return Err("Couldn't read this chat. Reload it.".into());
            }
            offset = chunk.end_offset;
        }
        if page.next.offset != offset
            || offset.saturating_sub(start) > u64::from(coder_history::MAX_PAGE_BYTES)
            || offset > page.snapshot_bytes
        {
            return Err("Couldn't read this chat. Reload it.".into());
        }
        if !page.chunks.is_empty() {
            self.cache
                .write(&format!("page_{source}_{start:020}"), &page)?;
        }
        self.transcript = TranscriptState {
            cursor: Some(page.next),
            snapshot_bytes: page.snapshot_bytes,
            has_more: page.has_more,
            pending_line: page.pending_line,
            checked_at: now(),
        };
        self.cache
            .write(&format!("chat_{source}"), &self.transcript)?;
        self.notices = page
            .notices
            .iter()
            .map(|n| format!("Notice: {}", n.code))
            .collect();
        Ok(())
    }

    pub(crate) fn source(&self) -> Option<String> {
        self.selected.as_ref().and_then(|c| c.source_id.clone())
    }
    pub(crate) fn page_keys(&self) -> Result<Vec<String>, String> {
        match self.source() {
            Some(source) => self.cache.keys(&format!("page_{source}_")),
            None => Ok(vec![]),
        }
    }
    pub(crate) fn window_index(&self, keys: &[String]) -> usize {
        self.window
            .as_ref()
            .map_or(keys.len().saturating_sub(1), |wanted| {
                keys.iter().position(|k| k == wanted).unwrap_or(0)
            })
    }
    fn rebuild(&mut self) -> Result<(), String> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or("view revision exhausted")?;
        self.current = Some(
            View::new(
                self.instance.clone(),
                self.revision,
                crate::render::root(self)?,
            )
            .validate()
            .map_err(|e| e.to_string())?,
        );
        Ok(())
    }
    fn packet(&mut self) -> Packet {
        if let Some(terminal) = self.terminal.as_mut() {
            terminal.redraw();
        }
        Packet {
            schema: "coder.mobile.v1",
            public_key: self.public_key.clone(),
            paired: self.code.is_some() || (self.synthetic && !self.catalog.is_empty()),
            pairing_completed: self.pairing_completed && self.code.is_some(),
            reading: self.selected.is_some(),
            status: self.status.clone(),
            error: self.error.clone(),
            follow_page: self
                .page_keys()
                .ok()
                .and_then(|keys| keys.get(self.window_index(&keys)).cloned()),
            follow_target: if self.selected.is_some() && self.window.is_none() {
                Some("timeline-end".into())
            } else {
                None
            },
            view: self
                .current
                .as_ref()
                .and_then(|v| serde_json::to_value(v.view()).ok()),
            computers: self
                .computers
                .as_ref()
                .and_then(Computers::view)
                .and_then(|v| serde_json::to_value(v.view()).ok()),
            computers_input: self.computers.as_ref().and_then(Computers::input).cloned(),
            computers_qr: self
                .computers
                .as_ref()
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
            computers_exit: self.computers_exit,
            terminal: self
                .terminal
                .as_ref()
                .and_then(coder_computers::terminal::screen::Terminal::view),
            terminal_paste: self
                .terminal
                .as_ref()
                .is_some_and(coder_computers::terminal::screen::Terminal::wants_paste),
            push: self.push_status.clone(),
        }
    }

    fn seed_synthetic(&mut self) -> Result<(), String> {
        if self.code.is_some() {
            return Ok(());
        }
        self.status = "Preview · no computer connected".into();
        self.apply_catalog(
            CatalogPage {
                snapshot: "synthetic".into(),
                entries: vec![Chat {
                    id: "synthetic-chat".into(),
                    harness: coder_history::Harness::Codex,
                    native_id: Some("synthetic".into()),
                    title: "Read-only chat preview".into(),
                    title_truncated: false,
                    updated_at: None,
                    archived: false,
                    subagent: false,
                    source_id: Some("synthetic".into()),
                    status: coder_history::SourceStatus::Available,
                }],
                next: None,
                notices: vec![],
            },
            true,
        )?;
        use base64::Engine;
        self.selected = self.catalog.first().cloned();
        self.cache.erase("page_synthetic_")?;
        self.transcript = TranscriptState::default();
        let mut offset = 0;
        for page_index in 0..2 {
            let mut chunks = Vec::new();
            for index in 0..16 {
                let number = page_index * 16 + index;
                let body = format!(
                    "# Native timeline\nSynthetic message {}.\n\nUnicode: café 日本語 👩🏽‍💻\n\nNo model or task was started.",
                    number + 1
                );
                let mut bytes=serde_json::to_vec(&serde_json::json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":body}]}})).map_err(|_| "synthetic record encoding failed")?;
                bytes.push(b'\n');
                let end = offset + bytes.len() as u64;
                chunks.push(coder_history::RecordChunk {
                    id: format!("synthetic-{number}"),
                    index: number,
                    record_offset: offset,
                    offset,
                    end_offset: end,
                    raw_base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
                    complete: true,
                    oversized: false,
                    readable: coder_history::readable_record(&bytes),
                });
                offset = end;
            }
            let cursor = TranscriptCursor {
                source_id: "synthetic".into(),
                incarnation: "synthetic-v1".into(),
                offset,
                record_offset: offset,
                record_index: (page_index + 1) * 16,
                prefix_sha256: "synthetic".into(),
            };
            self.apply_page(TranscriptPage {
                source_id: "synthetic".into(),
                incarnation: "synthetic-v1".into(),
                snapshot_bytes: offset,
                chunks,
                next: cursor,
                has_more: false,
                pending_line: false,
                notices: vec![],
                previous: None,
            })?;
        }
        self.selected = None;
        Ok(())
    }
}

pub(crate) fn now() -> u64 {
    coder_connect::unix_time().unwrap_or(0)
}

fn pairing_error(error: &coder_connect::Error) -> String {
    match error.code {
        ErrorCode::Transport | ErrorCode::Unavailable => {
            "Computer unavailable. Keep its pairing command running and try again.".into()
        }
        ErrorCode::Expired => "Pairing code expired. Run the pairing command again.".into(),
        ErrorCode::Conflict | ErrorCode::Revoked | ErrorCode::Forbidden => {
            "Pairing code refused or already used. Generate a new code on the computer.".into()
        }
        ErrorCode::SourceChanged => "Chat folders changed. Run the pairing command again.".into(),
        ErrorCode::RateLimited => "The computer is busy. Wait briefly, then try again.".into(),
        ErrorCode::Malformed | ErrorCode::Unsupported | ErrorCode::Bounds => {
            "Invalid pairing code. Scan or paste the code from your computer.".into()
        }
    }
}
