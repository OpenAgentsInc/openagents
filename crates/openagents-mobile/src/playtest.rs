//! Report a problem, My reports, and playtest logging
//! (`docs/game/playtesting.md`, Feedback capture in the app).
//!
//! A report fills in the build, the device, the tab and screen, and the
//! time; the tester writes what happened, what they expected, and the
//! steps. It is sealed with NIP-17 to the OpenAgents triage key and signed
//! by the tester's Verse world key ([`playtest::report::wrap`]), then sent
//! to OpenAgents' relay on a background thread. Its code (`PT-1A2B3C4D`)
//! shows in **My reports**. Until a build carries the triage key
//! ([`playtest::TRIAGE_KEY`]) a report waits on the phone and is sent when
//! one does; it is never sent anywhere else.
//!
//! When the private report is sent, the app also publishes its public,
//! content-free NIP-XP playtest report (kind `3197`, [`playtest::report::public_record`])
//! signed by the same world key: the build, the platform, the kind, and the
//! private report's digest, and no text. A playtest award cites it, so an
//! accepted report can earn XP. It is kept on the phone until a relay
//! accepts it and sent again from My reports.
//!
//! **Playtest logging** is on for everyone in a build unless the build
//! turned it off ([`LOGGING`], set by `OPENAGENTS_PLAYTEST_LOGGING=off`
//! for a release). There is no switch in the app. The log holds only
//! closed structural values ([`playtest::session`]), stays on the phone,
//! and is attached to a report only when the tester chose to and the log
//! is exactly the one the preview showed them (its digest). A build with
//! logging off records nothing and deletes any log a playtest build left.
//!
//! Both live in the app's encrypted store: an index of reports, each
//! report's body under its own item while it waits or failed (a sent
//! report's body is erased), and the playtest log.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use coder_computers::cache::Cache;
use nostr::domain::Event;
use playtest::report::{self, Context, Kind, Platform, Randomness, Report, Screenshot, SharedChat};
use playtest::session::{self, Code, Log, Route, Tab};
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Whether this build keeps the playtest log: on unless the build was made
/// with `OPENAGENTS_PLAYTEST_LOGGING=off` (release mode). The build scripts
/// pass the variable to Cargo; see `docs/game/playtesting.md`.
pub const LOGGING: bool = logging_setting(option_env!("OPENAGENTS_PLAYTEST_LOGGING"));

/// Reads the build's `OPENAGENTS_PLAYTEST_LOGGING`: exactly `off` turns
/// playtest logging off; unset or anything else leaves it on.
#[must_use]
pub const fn logging_setting(value: Option<&str>) -> bool {
    let Some(value) = value else { return true };
    let bytes = value.as_bytes();
    !(bytes.len() == 3 && bytes[0] == b'o' && bytes[1] == b'f' && bytes[2] == b'f')
}

/// The one line the Playtest screen shows about playtest logging.
fn logging_note(on: bool) -> &'static str {
    if on {
        "Playtest logging is on in this build."
    } else {
        "Playtest logging is off in this build."
    }
}

/// The platform a report names for the OS the app runs on
/// (`std::env::consts::OS`): `android` on Android, `ios` otherwise.
#[must_use]
pub fn platform(os: &str) -> Platform {
    if os == "android" {
        Platform::Android
    } else {
        Platform::Ios
    }
}

/// Reports kept in My reports; older ones drop off.
const MAX_SAVED: usize = 50;

/// Where a report is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Kept on the phone until a build knows the triage key.
    Waiting,
    Sending,
    Sent,
    /// No relay accepted it; it is sent again from My reports.
    Failed,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Self::Waiting => "Saved on this phone",
            Self::Sending => "Sending",
            Self::Sent => "Sent",
            Self::Failed => "Not sent yet",
        }
    }
}

/// One report in My reports. The body isn't here; see the module note.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Saved {
    /// The content digest: this report's identity on the phone.
    digest: String,
    /// `PT-…`, once sealed.
    code: Option<String>,
    kind: Kind,
    at: u64,
    build: String,
    tab: Tab,
    route: Route,
    /// The start of what happened, for the list.
    summary: String,
    status: Status,
    sent_at: Option<u64>,
    error: Option<String>,
    screenshot: bool,
    /// The playtest log was attached. Stored as `session` by earlier builds.
    #[serde(alias = "session")]
    log: bool,
    /// The signed public record, kept until a relay accepts it.
    #[serde(default)]
    public: Option<Event>,
    /// A relay accepted the public record.
    #[serde(default)]
    published: bool,
}

/// A row of My reports.
#[derive(Clone, Debug, Serialize)]
pub struct Row {
    pub id: String,
    pub code: Option<String>,
    pub kind: String,
    pub kind_label: &'static str,
    pub at: u64,
    pub build: String,
    /// `verse/gym`.
    pub place: String,
    pub summary: String,
    pub status: Status,
    pub status_label: &'static str,
    pub error: Option<String>,
    pub screenshot: bool,
    /// The playtest log was attached.
    pub log: bool,
    /// The public, content-free record of this report is on the relay.
    pub published: bool,
}

/// The kinds a tester chooses from, with the words the form shows.
#[derive(Clone, Debug, Serialize)]
pub struct KindChoice {
    pub value: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
}

pub const KINDS: [KindChoice; 4] = [
    KindChoice {
        value: "bug",
        label: "Bug",
        hint: "Something broke or gave a wrong result.",
    },
    KindChoice {
        value: "confusing",
        label: "Confusing",
        hint: "You weren't sure what to do or what happened.",
    },
    KindChoice {
        value: "idea",
        label: "Idea",
        hint: "Something you wanted to do and couldn't.",
    },
    KindChoice {
        value: "felt-good",
        label: "Felt good",
        hint: "Something that was fun or worked well. Tell us that too.",
    },
];

fn kind_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Bug => "Bug",
        Kind::Confusing => "Confusing",
        Kind::Idea => "Idea",
        Kind::FeltGood => "Felt good",
        Kind::Comment => "Comment",
    }
}

/// What a report sends and to whom, shown on the form.
pub const PRIVACY: &str = "Sent privately to the OpenAgents team. It becomes public only as a GitHub issue we write, without Wallet or key screenshots, and quotes your words only if you allow it. A public note says you filed a report on this build, never what it says, so an accepted report can earn playtest XP. Reports earn nothing by themselves.";

/// Where testers report while the app can't send yet.
pub const FALLBACK: &str =
    "https://github.com/OpenAgentsInc/openagents/issues/new?template=playtest-report.yml";

/// The direct reply to `report_draft`.
#[derive(Serialize)]
pub struct DraftPacket {
    pub schema: &'static str,
    pub tab: Tab,
    pub route: Route,
    /// False on the Wallet tab and on screens that can show a key.
    pub screenshot_allowed: bool,
    /// This build knows the triage key, so a report sends now.
    pub triage_ready: bool,
    /// The open Coder chat's task ID, attached only if the tester ticks it.
    pub task: Option<String>,
    /// This build keeps the playtest log.
    pub logging: bool,
    /// The whole playtest log, one line per event, as it would be attached.
    pub log_lines: Vec<String>,
    /// Sent back with `report_send` so exactly this log is attached.
    pub log_digest: String,
    /// The open chat with OpenAgents, one line per message, exactly as
    /// **Share this chat** would send it; empty when no such chat is open.
    pub chat_lines: Vec<String>,
    /// Sent back with `report_send` so exactly this chat is attached.
    pub chat_digest: String,
    pub kinds: &'static [KindChoice],
    pub privacy: &'static str,
    pub fallback: &'static str,
}

/// Playtest logging's state for the Account screens.
#[derive(Serialize)]
pub struct LogRow {
    /// This build keeps the playtest log ([`LOGGING`]).
    pub on: bool,
    /// The line the Playtest screen shows about it.
    pub note: &'static str,
    pub started_at: Option<u64>,
    pub events: usize,
    pub lines: Vec<String>,
}

/// The direct reply to `reports`, `report_send`, and `playtest_clear`.
#[derive(Serialize)]
pub struct ReportsPacket {
    pub schema: &'static str,
    pub triage_ready: bool,
    pub log: LogRow,
    pub reports: Vec<Row>,
    /// The report this request filed.
    pub sent: Option<Row>,
    /// Why this request's report wasn't filed.
    pub error: Option<String>,
    pub fallback: &'static str,
    /// What **Give feedback**'s dialog says once it filed: `Sent`, or that
    /// it is saved until the build can send.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feedback: Option<&'static str>,
}

/// What the tester filled in and chose, from the form.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Form {
    pub app_version: String,
    pub build: String,
    pub device: String,
    pub os_version: String,
    pub tab: Tab,
    pub route: Route,
    pub kind: Kind,
    pub happened: String,
    #[serde(default)]
    pub expected: String,
    #[serde(default)]
    pub steps: String,
    #[serde(default)]
    pub quote: bool,
    #[serde(default)]
    pub include_task: bool,
    #[serde(default)]
    pub include_log: bool,
    #[serde(default)]
    pub log_digest: String,
    /// **Share this chat**: off unless the tester ticks it.
    #[serde(default)]
    pub include_chat: bool,
    #[serde(default)]
    pub chat_digest: String,
    #[serde(default)]
    pub screenshot: Option<Screenshot>,
}

/// **Give feedback** on selected text (#10127), from the selection menu:
/// what the host knows. Rust adds where the text came from.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedbackForm {
    pub app_version: String,
    pub build: String,
    pub device: String,
    pub os_version: String,
    pub tab: Tab,
    pub route: Route,
    /// The selected text.
    pub text: String,
    pub comment: String,
    /// The key of the transcript row the selection starts in.
    #[serde(default)]
    pub row: Option<String>,
}

/// Publishes a sealed report and its public record.
pub trait Relay: Send + Sync {
    /// Sends `wrap` (a gift wrap or a public record), authenticating as
    /// `auth` where the relay asks.
    /// Blocking.
    fn publish(&self, wrap: &Event, auth: &SecretKey) -> Result<(), String>;
}

/// OpenAgents' relay, over NIP-42. It authenticates as the wrap's one-time
/// key, so the relay doesn't learn which tester sent it.
pub struct LiveRelay;

impl Relay for LiveRelay {
    fn publish(&self, wrap: &Event, auth: &SecretKey) -> Result<(), String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "The phone could not start a connection.".to_string())?;
        runtime.block_on(async {
            let mut socket = nostr_transport::Connection::connect(
                playtest::RELAY,
                auth,
                Duration::from_secs(10),
            )
            .await?;
            socket.send(json!(["EVENT", wrap])).await?;
            for _ in 0..16 {
                let frame = socket.next().await?;
                if frame[0] == "OK" && frame[1] == wrap.id.as_str() {
                    let _ = socket.close().await;
                    return if frame[2] == true {
                        Ok(())
                    } else {
                        Err(format!(
                            "The relay refused the report: {}",
                            frame[3].as_str().unwrap_or("no reason")
                        ))
                    };
                }
            }
            Err("The relay didn't answer.".to_string())
        })
    }
}

struct Inner {
    saved: Vec<Saved>,
    log: Log,
    /// The last problem flags seen, to record only new problems.
    flags: (bool, bool, bool),
}

/// Reports and the playtest log for one app lifetime.
pub struct Playtest {
    inner: Arc<Mutex<Inner>>,
    store: Option<Arc<Cache>>,
    relay: Arc<dyn Relay>,
    triage: Option<XOnlyPublicKey>,
    sending: Vec<std::thread::JoinHandle<()>>,
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn random() -> Randomness {
    use secp256k1::rand::{Rng, RngCore};
    let mut rng = secp256k1::rand::rng();
    let mut seal_nonce = [0; 32];
    let mut wrap_nonce = [0; 32];
    rng.fill_bytes(&mut seal_nonce);
    rng.fill_bytes(&mut wrap_nonce);
    Randomness {
        wrapper: SecretKey::new(&mut rng),
        seal_nonce,
        wrap_nonce,
        // NIP-59 moves outer timestamps back; an hour keeps relays happy.
        seal_earlier: rng.random_range(0..3_600),
        wrap_earlier: rng.random_range(0..3_600),
    }
}

fn lock(inner: &Mutex<Inner>) -> MutexGuard<'_, Inner> {
    inner
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn body_key(digest: &str) -> String {
    format!("playtest-report-{}", &digest[..32.min(digest.len())])
}

fn save(store: Option<&Cache>, inner: &Inner) {
    if let Some(store) = store {
        let _ = store.write("playtest-reports", &inner.saved);
        // The item keeps the name earlier builds gave it.
        let _ = store.write("playtest-session", &inner.log);
    }
}

fn row(saved: &Saved) -> Row {
    Row {
        id: saved.digest[..16.min(saved.digest.len())].to_owned(),
        code: saved.code.clone(),
        kind: session::name(&saved.kind),
        kind_label: kind_label(saved.kind),
        at: saved.at,
        build: saved.build.clone(),
        place: format!(
            "{}/{}",
            session::name(&saved.tab),
            session::name(&saved.route)
        ),
        summary: saved.summary.clone(),
        status: saved.status,
        status_label: saved.status.label(),
        error: saved.error.clone(),
        screenshot: saved.screenshot,
        log: saved.log,
        published: saved.published,
    }
}

impl Playtest {
    /// Opens the reports and playtest log from `store`, which may be
    /// missing (then nothing survives a relaunch). With `logging` the log
    /// records, even where an earlier build had the session turned off;
    /// without it the log is deleted and nothing is recorded.
    pub fn new(
        store: Option<Cache>,
        relay: Arc<dyn Relay>,
        triage: Option<&str>,
        logging: bool,
    ) -> Self {
        let saved = store
            .as_ref()
            .and_then(|s| s.read("playtest-reports").ok().flatten())
            .unwrap_or_default();
        let stored: Log = store
            .as_ref()
            .and_then(|s| s.read("playtest-session").ok().flatten())
            .unwrap_or_default();
        let mut log = stored.clone();
        if logging {
            log.on = true;
            log.started_at.get_or_insert_with(now);
        } else {
            log = Log::default();
        }
        let changed = log != stored;
        let mut inner = Inner {
            saved,
            log,
            flags: (false, false, false),
        };
        // A send the last run didn't finish is sent again from My reports.
        for saved in &mut inner.saved {
            if saved.status == Status::Sending {
                saved.status = Status::Failed;
            }
        }
        if changed {
            save(store.as_ref(), &inner);
        }
        Self {
            inner: Arc::new(Mutex::new(inner)),
            store: store.map(Arc::new),
            relay,
            triage: triage.and_then(|key| key.parse().ok()),
            sending: vec![],
        }
    }

    /// The live store and relay, with the triage key and playtest logging
    /// this build carries.
    pub fn live(store: Option<Cache>) -> Self {
        Self::new(store, Arc::new(LiveRelay), playtest::TRIAGE_KEY, LOGGING)
    }

    fn persist(&self, inner: &Inner) {
        save(self.store.as_deref(), inner);
    }

    fn record(&self, code: Code, place: Option<(Tab, Route)>) {
        let mut inner = lock(&self.inner);
        if !inner.log.on {
            return;
        }
        let Some((tab, route)) = place.or_else(|| inner.log.position()) else {
            return;
        };
        inner.log.record(session::Event {
            at: now(),
            tab,
            route,
            code,
        });
        self.persist(&inner);
    }

    /// Something structural happened where the tester is: a Gym card, a
    /// run, a publish. A code, never text.
    pub fn event(&self, code: Code) {
        self.record(code, None);
    }

    /// The tester moved to `tab` and `route`.
    pub fn screen(&self, tab: Tab, route: Route) {
        self.record(Code::Screen, Some((tab, route)));
    }

    /// The app came to the foreground or went to the background.
    pub fn lifecycle(&self, active: bool) {
        self.record(
            if active {
                Code::Foreground
            } else {
                Code::Background
            },
            None,
        );
    }

    /// The problems the app packet shows now: an app notice, a Wallet
    /// error, a Coder notice. Each new one is recorded as its code, never
    /// its words.
    pub fn observe(&self, notice: bool, wallet: bool, coder: bool) {
        let before = {
            let mut inner = lock(&self.inner);
            std::mem::replace(&mut inner.flags, (notice, wallet, coder))
        };
        if notice && !before.0 {
            self.record(Code::Notice, None);
        }
        if wallet && !before.1 {
            self.record(Code::WalletError, None);
        }
        if coder && !before.2 {
            self.record(Code::CoderError, None);
        }
    }

    /// Delete the playtest log. Logging goes on recording from now.
    pub fn clear_log(&self) {
        let mut inner = lock(&self.inner);
        inner.log.clear();
        if inner.log.on {
            inner.log.started_at = Some(now());
        }
        self.persist(&inner);
    }

    /// The form for the screen the tester is on.
    pub fn draft(
        &self,
        tab: Tab,
        route: Route,
        task: Option<String>,
        chat: Option<&SharedChat>,
    ) -> DraftPacket {
        let chat = chat.filter(|_| tab == Tab::Coder);
        self.record(Code::ReportOpened, Some((tab, route)));
        let inner = lock(&self.inner);
        DraftPacket {
            schema: "openagents.report-draft.v1",
            tab,
            route,
            screenshot_allowed: !route.sensitive(tab),
            triage_ready: self.triage.is_some(),
            task: task.filter(|_| tab == Tab::Coder),
            logging: inner.log.on,
            log_lines: if inner.log.on {
                inner.log.lines()
            } else {
                vec![]
            },
            log_digest: inner.log.digest(),
            chat_lines: chat.map(SharedChat::lines).unwrap_or_default(),
            chat_digest: chat.map(report::chat_digest).unwrap_or_default(),
            kinds: &KINDS,
            privacy: PRIVACY,
            fallback: FALLBACK,
        }
    }

    /// My reports, after sending again any report that waits or failed,
    /// when this build knows the triage key.
    pub fn reports(&mut self, world: Option<&SecretKey>) -> ReportsPacket {
        if let (Some(world), Some(_)) = (world, self.triage) {
            let retry: Vec<String> = lock(&self.inner)
                .saved
                .iter()
                .filter(|s| matches!(s.status, Status::Waiting | Status::Failed))
                .map(|s| s.digest.clone())
                .collect();
            for digest in retry {
                let body: Option<Report> = self
                    .store
                    .as_ref()
                    .and_then(|s| s.read(&body_key(&digest)).ok().flatten());
                if let Some(body) = body {
                    self.dispatch(body, world, &digest);
                }
            }
            let unpublished: Vec<(String, Event)> = lock(&self.inner)
                .saved
                .iter()
                .filter(|s| s.status == Status::Sent && !s.published)
                .filter_map(|s| Some((s.digest.clone(), s.public.clone()?)))
                .collect();
            for (digest, public) in unpublished {
                self.announce(public, *world, digest);
            }
        }
        self.packet(None, None)
    }

    /// Publishes a sent report's public record on a background thread,
    /// authenticating as the key that signed it.
    fn announce(&mut self, public: Event, world: SecretKey, digest: String) {
        let (inner, store, relay) = (self.inner.clone(), self.store.clone(), self.relay.clone());
        self.sending.retain(|h| !h.is_finished());
        self.sending.push(std::thread::spawn(move || {
            let result = relay.publish(&public, &world);
            let mut inner = lock(&inner);
            if result.is_ok()
                && let Some(saved) = inner.saved.iter_mut().find(|s| s.digest == digest)
            {
                saved.published = true;
                saved.public = None;
            }
            save(store.as_deref(), &inner);
        }));
    }

    /// Files a report from the form. `task` is the open Coder chat's task;
    /// `chat` is the open chat with OpenAgents, attached only when the
    /// tester ticked **Share this chat** and it is the one they saw.
    pub fn send(
        &mut self,
        form: Form,
        world: &SecretKey,
        task: Option<String>,
        chat: Option<SharedChat>,
        platform: Platform,
    ) -> ReportsPacket {
        match self.file(form, world, task, chat, platform) {
            Ok(digest) => {
                let sent = lock(&self.inner)
                    .saved
                    .iter()
                    .find(|s| s.digest == digest)
                    .map(row);
                self.packet(sent, None)
            }
            Err(error) => self.packet(None, Some(error)),
        }
    }

    fn file(
        &mut self,
        form: Form,
        world: &SecretKey,
        task: Option<String>,
        chat: Option<SharedChat>,
        platform: Platform,
    ) -> Result<String, String> {
        if form.screenshot.is_some() && form.route.sensitive(form.tab) {
            return Err(
                "Screenshots are never sent from the Wallet or a key screen. Describe it in words."
                    .into(),
            );
        }
        let session = if form.include_log {
            let inner = lock(&self.inner);
            if !inner.log.on {
                return Err(
                    "Playtest logging is off in this build, so there's no log to attach.".into(),
                );
            }
            if inner.log.digest() != form.log_digest {
                return Err(
                    "The playtest log changed since you looked at it. Check it again before sending."
                        .into(),
                );
            }
            Some(inner.log.events.iter().copied().collect())
        } else {
            None
        };
        let chat = if form.include_chat {
            let Some(chat) = chat.filter(|_| form.tab == Tab::Coder) else {
                return Err("There's no chat to share on this screen.".into());
            };
            if report::chat_digest(&chat) != form.chat_digest {
                return Err(
                    "The chat changed since you looked at it. Check it again before sending."
                        .into(),
                );
            }
            Some(chat)
        } else {
            None
        };
        let report = Report {
            schema: report::SCHEMA.into(),
            context: Context {
                app_version: form.app_version,
                build: form.build,
                platform,
                device: form.device,
                os_version: form.os_version,
                tab: form.tab,
                route: form.route,
                at: now(),
            },
            kind: form.kind,
            happened: form.happened.trim().to_owned(),
            expected: form.expected.trim().to_owned(),
            steps: form.steps.trim().to_owned(),
            quote: form.quote,
            task: if form.include_task && form.tab == Tab::Coder {
                task
            } else {
                None
            },
            session,
            screenshot: form.screenshot,
            notes: vec![],
            chat,
            selection: None,
        }
        .fit();
        self.keep(report, world)
    }

    /// Files **Give feedback**'s comment on `selection` (#10127): kept and
    /// sent as any report, so it shows in My reports and waits there until
    /// a build knows the triage key.
    pub fn feedback(
        &mut self,
        form: FeedbackForm,
        selection: report::Selection,
        world: &SecretKey,
        platform: Platform,
    ) -> ReportsPacket {
        let context = Context {
            app_version: form.app_version,
            build: form.build,
            platform,
            device: form.device,
            os_version: form.os_version,
            tab: form.tab,
            route: form.route,
            at: now(),
        };
        let filed = playtest::feedback::report(context, selection, &form.comment)
            .and_then(|report| self.keep(report, world));
        match filed {
            Ok(digest) => {
                let sent = lock(&self.inner)
                    .saved
                    .iter()
                    .find(|s| s.digest == digest)
                    .map(row);
                let mut packet = self.packet(sent, None);
                packet.feedback = Some(if self.triage.is_some() {
                    playtest::feedback::SENT
                } else {
                    playtest::feedback::SAVED
                });
                packet
            }
            Err(error) => self.packet(None, Some(error)),
        }
    }

    /// Keeps `report` in My reports and sends it when this build knows the
    /// triage key. Returns its digest.
    fn keep(&mut self, report: Report, world: &SecretKey) -> Result<String, String> {
        report.check()?;
        let digest = report::digest(&report.content());
        let summary: String = report.happened.chars().take(80).collect();
        {
            let mut inner = lock(&self.inner);
            if inner.saved.iter().any(|s| s.digest == digest) {
                return Err("This report was already filed.".into());
            }
            inner.saved.insert(
                0,
                Saved {
                    digest: digest.clone(),
                    code: None,
                    kind: report.kind,
                    at: report.context.at,
                    build: report.context.build_label(),
                    tab: report.context.tab,
                    route: report.context.route,
                    summary,
                    status: Status::Waiting,
                    sent_at: None,
                    error: None,
                    screenshot: report.screenshot.is_some(),
                    log: report.session.is_some(),
                    public: None,
                    published: false,
                },
            );
            while inner.saved.len() > MAX_SAVED {
                if let Some(old) = inner.saved.pop()
                    && let Some(store) = &self.store
                {
                    let _ = store.write(&body_key(&old.digest), &Option::<Report>::None);
                }
            }
            if let Some(store) = &self.store {
                store
                    .write(&body_key(&digest), &report)
                    .map_err(|_| "The report couldn't be saved on this phone.".to_string())?;
            }
            self.persist(&inner);
        }
        if self.triage.is_some() {
            self.dispatch(report, world, &digest);
        }
        Ok(digest)
    }

    /// Seals `report` and sends it on a background thread.
    fn dispatch(&mut self, report: Report, world: &SecretKey, digest: &str) {
        let Some(triage) = self.triage else { return };
        let random = random();
        let update = |inner: &mut Inner, f: &dyn Fn(&mut Saved)| {
            if let Some(saved) = inner.saved.iter_mut().find(|s| s.digest == digest) {
                f(saved);
            }
        };
        let sealed = match report::wrap(&report, world, &triage, &random) {
            Ok(sealed) => sealed,
            Err(error) => {
                let mut inner = lock(&self.inner);
                update(&mut inner, &|s| {
                    s.status = Status::Failed;
                    s.error = Some(error.clone());
                });
                self.persist(&inner);
                return;
            }
        };
        {
            let mut inner = lock(&self.inner);
            update(&mut inner, &|s| {
                s.status = Status::Sending;
                s.code = Some(sealed.code.clone());
                s.error = None;
                if !s.published {
                    s.public = Some(sealed.public.clone());
                }
            });
            self.persist(&inner);
        }
        let (inner, store, relay) = (self.inner.clone(), self.store.clone(), self.relay.clone());
        let digest = digest.to_owned();
        let world = *world;
        self.sending.retain(|h| !h.is_finished());
        self.sending.push(std::thread::spawn(move || {
            let result = relay.publish(&sealed.wrap, &random.wrapper);
            // The public record follows the private report, never alone.
            let published = result.is_ok() && relay.publish(&sealed.public, &world).is_ok();
            let mut inner = lock(&inner);
            let place = inner.log.position();
            if let Some(saved) = inner.saved.iter_mut().find(|s| s.digest == digest) {
                match &result {
                    Ok(()) => {
                        saved.status = Status::Sent;
                        saved.sent_at = Some(now());
                        saved.error = None;
                        if published {
                            saved.published = true;
                            saved.public = None;
                        }
                        // A sent report's body leaves the phone's store.
                        if let Some(store) = &store {
                            let _ = store.write(&body_key(&digest), &Option::<Report>::None);
                        }
                    }
                    Err(error) => {
                        saved.status = Status::Failed;
                        saved.error = Some(error.clone());
                    }
                }
            }
            if let Some((tab, route)) = place {
                let code = if result.is_ok() {
                    Code::ReportSent
                } else {
                    Code::ReportFailed
                };
                inner.log.record(session::Event {
                    at: now(),
                    tab,
                    route,
                    code,
                });
            }
            save(store.as_deref(), &inner);
        }));
    }

    /// Waits for every send in flight. Tests only need it.
    #[cfg(test)]
    pub fn wait(&mut self) {
        for handle in self.sending.drain(..) {
            let _ = handle.join();
        }
    }

    /// My reports with why this request's report wasn't filed.
    pub fn refuse(&self, error: &str) -> ReportsPacket {
        self.packet(None, Some(error.to_owned()))
    }

    fn packet(&self, sent: Option<Row>, error: Option<String>) -> ReportsPacket {
        let inner = lock(&self.inner);
        ReportsPacket {
            schema: "openagents.reports.v1",
            triage_ready: self.triage.is_some(),
            log: LogRow {
                on: inner.log.on,
                note: logging_note(inner.log.on),
                started_at: inner.log.started_at,
                events: inner.log.events.len(),
                lines: inner.log.lines(),
            },
            reports: inner.saved.iter().map(row).collect(),
            sent,
            error,
            fallback: FALLBACK,
            feedback: None,
        }
    }
}

#[cfg(test)]
#[path = "playtest_tests.rs"]
mod tests;
