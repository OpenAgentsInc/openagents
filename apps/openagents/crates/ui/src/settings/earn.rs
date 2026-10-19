//! Settings → Earn: share this computer and see what it earned (#12196).
//!
//! A thin view over the OpenAgents command line (docs/compute/pylon-1.0.md,
//! "Desktop and Verse"): the switch runs `openagents pylon up --yes` and
//! `down`, and every line is read from one command's `--json` result
//! (`pylon status`, `earnings`, `settings`, `wallet balance`). The page keeps
//! nothing of its own. The shell drops it when Settings leaves Earn, which
//! ends its 10-second refresh.
//!
//! A command this build's command line does not have answers with a usage
//! error; its row then says [`NOT_AVAILABLE`] instead of offering a control.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use futures::channel::oneshot;
use gpui::{
    ClipboardItem, Context, Entity, Focusable, SharedString, Subscription, Task, Window, div,
    prelude::*, px,
};
use serde_json::Value;

use crate::composer::{ComposerInput, ComposerInputEvent};
use crate::popover;
use crate::settings::widgets::{self, ActionTone};
use crate::theme::Theme;

/// The command line's exit code for a command or option it does not know.
const EXIT_USAGE: i32 = 64;
/// How often the page reads the machine while it is open.
const REFRESH: Duration = Duration::from_secs(10);
/// The wallet syncs before it answers, so its balance is read every sixth
/// refresh (once a minute).
const WALLET_EVERY: u32 = 6;
/// A row whose command this build does not have.
pub(crate) const NOT_AVAILABLE: &str = "Not available in this build";
/// The one command that installs the OpenAgents command line.
const INSTALL: &str = "curl -fsSL https://openagents.com/cli/install.sh | bash";
/// How `openagents wallet balance` starts its answer on a computer with no
/// wallet (`openagents_spark::computer::NOT_SET_UP`).
const NO_WALLET: &str = "This computer doesn't have a wallet yet";
/// Steps `pylon up` takes itself, so they never block turning it on.
const UP_DOES: [&str; 2] = ["openagents pylon setup", "openagents pylon selfcheck"];

/// One command's result.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Reply {
    /// It succeeded: its result (a pylon envelope's `data`, or the object
    /// a wallet command prints).
    Ok(Value),
    /// It failed: one line saying why, and the whole result it printed.
    Failed(String, Value),
    /// This build's command line has no such command.
    Unavailable,
}

impl Reply {
    /// Read a finished command from its exit code and what it printed.
    pub(crate) fn read(code: Option<i32>, stdout: &str) -> Self {
        if code == Some(EXIT_USAGE) {
            return Self::Unavailable;
        }
        let value = stdout
            .lines()
            .rev()
            .find_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
            .filter(Value::is_object)
            .unwrap_or(Value::Null);
        if code == Some(0) {
            return Self::Ok(if value.get("schema").is_some() {
                value["data"].clone()
            } else {
                value
            });
        }
        let message = value["summary"]
            .as_str()
            .or_else(|| value["error"].as_str())
            .unwrap_or("It didn't finish.")
            .to_string();
        Self::Failed(message, value)
    }

    fn data(&self) -> Option<&Value> {
        match self {
            Self::Ok(data) => Some(data),
            _ => None,
        }
    }
}

/// Run `openagents --json ARGS` and read its result. Blocks.
fn run(cli: &Path, args: &[&str]) -> Reply {
    match Command::new(cli)
        .arg("--json")
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    {
        Ok(out) => Reply::read(out.status.code(), &String::from_utf8_lossy(&out.stdout)),
        Err(_) => Reply::Failed(
            "The OpenAgents command line didn't start.".into(),
            Value::Null,
        ),
    }
}

/// Run `work` on its own thread: `pylon up` runs as long as this computer
/// serves when no host service keeps it, so commands never hold a shared
/// executor thread.
fn off_thread<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> oneshot::Receiver<T> {
    let (tx, rx) = oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(work());
    });
    rx
}

/// The machine's state in plain words, from `pylon status`'s data.
/// `setting_up` while this page's `up` is still running.
pub(crate) fn status_line(data: &Value, setting_up: bool) -> String {
    let state = data["state"].as_str().unwrap_or("off");
    let why = data["why"].as_str().unwrap_or_default();
    let idle = state == "off" || why.starts_with("not running");
    match state {
        "serving" => "Serving".into(),
        "starting" => format!("Setting up: {}", starting(why)),
        _ if setting_up && idle => {
            if data["model"].is_null() {
                "Setting up: downloading the model (6.5 GB)".into()
            } else {
                "Setting up: starting the model".into()
            }
        }
        "paused" => paused(why).into(),
        _ => "Off".into(),
    }
}

fn starting(why: &str) -> &'static str {
    if why.starts_with("checking") {
        "checking this computer's answers"
    } else if why.contains("15 minutes") {
        "the model hasn't started after 15 minutes"
    } else {
        "starting the model"
    }
}

fn paused(why: &str) -> &'static str {
    let word = |w: &str| why.split(|c: char| !c.is_alphanumeric()).any(|x| x == w);
    if why.starts_with("owner busy") {
        "Paused: you're using this computer"
    } else if why.starts_with("no network") {
        "Paused: no internet connection"
    } else if why.starts_with("an engine stopped") {
        "Paused: restarting the model"
    } else if why.starts_with("finishing jobs") {
        "Stopping: finishing the work it took"
    } else if why.starts_with("not running") {
        "Paused: not running. Turn it off and on again to start it."
    } else if why.starts_with("could not start") {
        "Can't serve: the model couldn't start"
    } else if word("battery") {
        "Paused: on battery"
    } else if word("hot") || word("heat") || word("thermal") {
        "Paused: this computer is too hot"
    } else if why.contains("self-check") || why.contains("answers") {
        "Can't serve: this computer's answers didn't match the expected ones"
    } else {
        "Paused"
    }
}

/// "Can't serve: <what> — <fix>" for the first thing `pylon doctor` says
/// blocks this computer, leaving out the steps `up` takes itself.
pub(crate) fn blocked_line(doctor: &Value) -> Option<String> {
    let data = &doctor["data"];
    let from_blocks = data["blocks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|b| Some((b["what"].as_str()?, b["fix"].as_str()?)));
    let from_next = doctor["next"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|n| Some((n["why"].as_str()?, n["command"].as_str()?)));
    let (what, fix) = from_blocks
        .chain(from_next)
        .find(|(_, fix)| !UP_DOES.contains(fix))?;
    Some(format!(
        "Can't serve: {} — {}",
        plain_block(what),
        plain_fix(fix)
    ))
}

fn plain_block(what: &str) -> String {
    if what.contains("cannot be reached") {
        "this computer can't reach the internet".into()
    } else if what.contains("GB free") {
        format!("not enough disk space ({what})")
    } else if what.ends_with("is not installed") {
        "the model engine isn't installed".into()
    } else if what.contains("is not this release's engine") {
        "the model engine is out of date".into()
    } else if what.contains("self-check failed") {
        "this computer's answers didn't match the expected ones".into()
    } else {
        what.to_string()
    }
}

fn plain_fix(fix: &str) -> String {
    if fix.starts_with("check this computer's internet") {
        "check the internet connection".into()
    } else if fix.starts_with("cargo ") {
        "the model engine isn't published for this computer yet".into()
    } else if fix.starts_with("openagents ") {
        format!("run `{fix}` in Terminal")
    } else {
        fix.to_string()
    }
}

/// What `pylon earnings` reports, read leniently: whole sats or msat.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Earnings {
    pub decisions_today: u64,
    pub earned_sats: u64,
    pub paid_sats: u64,
    /// Where payouts go when it is not this computer's wallet: a Lightning
    /// address.
    pub elsewhere: Option<String>,
}

impl Earnings {
    pub(crate) fn from(data: &Value) -> Self {
        let sats = |key: &str| {
            data[format!("{key}_sats")]
                .as_u64()
                .or_else(|| data[format!("{key}_msat")].as_u64().map(|msat| msat / 1000))
        };
        let destination = match &data["destination"] {
            Value::String(text) => Some(text.as_str()),
            Value::Object(o) => ["address", "value", "lightning_address"]
                .iter()
                .find_map(|key| o.get(*key).and_then(Value::as_str)),
            _ => None,
        };
        Self {
            decisions_today: ["checked_today", "verified_today", "decisions_today"]
                .iter()
                .find_map(|key| data[*key].as_u64())
                .unwrap_or(0),
            earned_sats: sats("earned").unwrap_or(0),
            paid_sats: sats("paid").or_else(|| sats("sent")).unwrap_or(0),
            elsewhere: destination.filter(|d| d.contains('@')).map(str::to_string),
        }
    }

    /// The title and the line under it.
    pub(crate) fn lines(&self) -> (String, String) {
        let title = format!(
            "{} checked {} today",
            group(self.decisions_today),
            if self.decisions_today == 1 {
                "decision"
            } else {
                "decisions"
            }
        );
        let line = if self.earned_sats == 0 && self.paid_sats == 0 {
            "Nothing earned so far. Earnings follow demand and can be zero.".into()
        } else {
            format!(
                "{} earned so far · {} paid to you",
                sats_text(self.earned_sats),
                sats_text(self.paid_sats)
            )
        };
        (title, line)
    }
}

/// An amount the way the wallet writes it (BIP 177): `₿12,345`.
pub(crate) fn sats_text(sats: u64) -> String {
    format!("₿{}", group(sats))
}

fn group(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// This computer's wallet as `wallet balance` saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Wallet {
    Balance(u64),
    /// No wallet on this computer yet.
    Absent,
    Unreadable,
    Unavailable,
}

impl Wallet {
    pub(crate) fn from(reply: &Reply) -> Self {
        match reply {
            Reply::Ok(v) => v["balance_sats"]
                .as_u64()
                .map_or(Self::Unreadable, Self::Balance),
            Reply::Failed(message, _) if message.starts_with(NO_WALLET) => Self::Absent,
            Reply::Failed(..) => Self::Unreadable,
            Reply::Unavailable => Self::Unavailable,
        }
    }
}

/// The user's own pause choices from `pylon settings`.
pub(crate) fn pauses(data: &Value) -> (bool, bool) {
    let on = |key: &str| data[key].as_bool().unwrap_or(data[key] == "on");
    (on("pause_on_battery"), on("pause_when_hot"))
}

#[derive(Clone, Copy)]
enum Pause {
    Battery,
    Hot,
}

impl Pause {
    fn flag(self) -> &'static str {
        match self {
            Self::Battery => "--pause-on-battery",
            Self::Hot => "--pause-when-hot",
        }
    }
}

/// One refresh's reads.
struct Read {
    cli: Option<PathBuf>,
    status: Option<Reply>,
    earnings: Option<Reply>,
    pauses: Option<Reply>,
    wallet: Option<Reply>,
}

pub struct EarnPage {
    scroll: widgets::PageScroll,
    /// The command line, once looked for; `Some(None)` when it isn't
    /// installed.
    cli: Option<Option<PathBuf>>,
    status: Option<Reply>,
    earnings: Option<Reply>,
    pauses: Option<Reply>,
    wallet: Option<Reply>,
    /// This page's `up` still runs.
    setting_up: bool,
    /// `down` runs.
    stopping: bool,
    /// Why turning on failed, from `doctor`.
    blocked: Option<String>,
    error: Option<SharedString>,
    payout_open: bool,
    payout_unavailable: bool,
    payout_error: Option<SharedString>,
    payout_input: Entity<ComposerInput>,
    /// Shown once after this page made the wallet, until dismissed.
    recovery_words: Option<String>,
    refreshing: bool,
    /// A pause, payout, or wallet command runs.
    busy: bool,
    ticks: u32,
    _refresh: Task<()>,
    _payout_events: Subscription,
}

impl EarnPage {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self::reading(true, cx)
    }

    /// `read`: whether the page reads the machine (tests draw it without).
    fn reading(read: bool, cx: &mut Context<Self>) -> Self {
        let refresh = cx.spawn(async move |this, cx| {
            while read {
                if this.update(cx, |this, cx| this.refresh(cx)).is_err() {
                    break;
                }
                cx.background_executor().timer(REFRESH).await;
            }
        });
        let payout_input = cx.new(|cx| ComposerInput::new("name@example.com", cx));
        let payout_events = cx.subscribe(&payout_input, |this: &mut Self, _, event, cx| {
            if matches!(event, ComposerInputEvent::Submitted) {
                this.save_payout(cx);
            }
        });
        Self {
            scroll: widgets::PageScroll::default(),
            cli: None,
            status: None,
            earnings: None,
            pauses: None,
            wallet: None,
            setting_up: false,
            stopping: false,
            blocked: None,
            error: None,
            payout_open: false,
            payout_unavailable: false,
            payout_error: None,
            payout_input,
            recovery_words: None,
            refreshing: false,
            busy: false,
            ticks: 0,
            _refresh: refresh,
            _payout_events: payout_events,
        }
    }

    fn cli(&self) -> Option<PathBuf> {
        self.cli.clone().flatten()
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.refreshing || matches!(self.cli, Some(None)) {
            return;
        }
        self.refreshing = true;
        let wallet = self.ticks % WALLET_EVERY == 0;
        self.ticks = self.ticks.wrapping_add(1);
        let known = self.cli.clone();
        let rx = off_thread(move || {
            let cli = known
                .flatten()
                .or_else(openagents_harness::find_openagents_cli);
            let Some(path) = cli.clone() else {
                return Read {
                    cli,
                    status: None,
                    earnings: None,
                    pauses: None,
                    wallet: None,
                };
            };
            Read {
                status: Some(run(&path, &["pylon", "status"])),
                earnings: Some(run(&path, &["pylon", "earnings"])),
                pauses: Some(run(&path, &["pylon", "settings"])),
                wallet: wallet.then(|| run(&path, &["wallet", "balance"])),
                cli,
            }
        });
        cx.spawn(async move |this, cx| {
            let Ok(read) = rx.await else { return };
            this.update(cx, |this, cx| {
                this.refreshing = false;
                this.cli = Some(read.cli);
                this.status = read.status.or(this.status.take());
                this.earnings = read.earnings.or(this.earnings.take());
                this.pauses = read.pauses.or(this.pauses.take());
                this.wallet = read.wallet.or(this.wallet.take());
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// The switch's position: what the user last asked for while it runs,
    /// otherwise what `status` says.
    fn earning(&self) -> bool {
        if self.stopping {
            return false;
        }
        self.setting_up
            || self
                .status
                .as_ref()
                .and_then(Reply::data)
                .is_some_and(|d| d["state"].as_str().is_some_and(|s| s != "off"))
    }

    fn toggle_earning(&mut self, cx: &mut Context<Self>) {
        let Some(cli) = self.cli() else { return };
        if self.stopping {
            return;
        }
        self.error = None;
        self.blocked = None;
        if self.earning() {
            self.stopping = true;
            let rx = off_thread(move || run(&cli, &["pylon", "down"]));
            cx.spawn(async move |this, cx| {
                let Ok(reply) = rx.await else { return };
                this.update(cx, |this, cx| {
                    this.stopping = false;
                    if let Reply::Failed(message, _) = reply {
                        this.error =
                            Some(format!("Couldn't stop: {}", plain_block(&message)).into());
                    }
                    this.refresh(cx);
                    cx.notify();
                })
                .ok();
            })
            .detach();
        } else {
            self.setting_up = true;
            let rx = off_thread(move || {
                let up = run(&cli, &["pylon", "up", "--yes"]);
                let doctor =
                    matches!(up, Reply::Failed(..)).then(|| run(&cli, &["pylon", "doctor"]));
                (up, doctor)
            });
            cx.spawn(async move |this, cx| {
                let Ok((up, doctor)) = rx.await else { return };
                this.update(cx, |this, cx| {
                    this.setting_up = false;
                    match up {
                        Reply::Ok(_) => {}
                        Reply::Unavailable => this.status = Some(Reply::Unavailable),
                        Reply::Failed(message, _) => {
                            this.blocked = doctor.and_then(|d| match d {
                                Reply::Failed(_, value) => blocked_line(&value),
                                _ => None,
                            });
                            if this.blocked.is_none() && !this.stopping {
                                this.error = Some(
                                    format!("Couldn't turn on: {}", plain_block(&message)).into(),
                                );
                            }
                        }
                    }
                    this.refresh(cx);
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        cx.notify();
    }

    fn toggle_pause(&mut self, pause: Pause, cx: &mut Context<Self>) {
        let (Some(cli), Some(data)) = (self.cli(), self.pauses.as_ref().and_then(Reply::data))
        else {
            return;
        };
        if self.busy {
            return;
        }
        let (battery, hot) = pauses(data);
        let on = match pause {
            Pause::Battery => !battery,
            Pause::Hot => !hot,
        };
        self.busy = true;
        let rx = off_thread(move || {
            let set = run(
                &cli,
                &[
                    "pylon",
                    "settings",
                    pause.flag(),
                    if on { "on" } else { "off" },
                ],
            );
            (set, run(&cli, &["pylon", "settings"]))
        });
        cx.spawn(async move |this, cx| {
            let Ok((set, read)) = rx.await else { return };
            this.update(cx, |this, cx| {
                this.busy = false;
                match set {
                    Reply::Unavailable => this.pauses = Some(Reply::Unavailable),
                    Reply::Failed(message, _) => this.error = Some(message.into()),
                    Reply::Ok(_) => this.pauses = Some(read),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// `payout set ADDRESS`, or this computer's wallet when `address` is
    /// `None`.
    fn set_payout(&mut self, address: Option<String>, cx: &mut Context<Self>) {
        let Some(cli) = self.cli() else { return };
        if self.busy {
            return;
        }
        self.busy = true;
        self.payout_error = None;
        let rx = off_thread(move || {
            let mut args = vec!["pylon", "payout", "set"];
            if let Some(address) = &address {
                args.push(address);
            }
            let set = run(&cli, &args);
            (set, run(&cli, &["pylon", "earnings"]))
        });
        cx.spawn(async move |this, cx| {
            let Ok((set, earnings)) = rx.await else {
                return;
            };
            this.update(cx, |this, cx| {
                this.busy = false;
                match set {
                    Reply::Unavailable => {
                        this.payout_unavailable = true;
                        this.payout_open = false;
                    }
                    Reply::Failed(message, _) => this.payout_error = Some(message.into()),
                    Reply::Ok(_) => {
                        this.payout_open = false;
                        this.earnings = Some(earnings);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn save_payout(&mut self, cx: &mut Context<Self>) {
        let address = self.payout_input.read(cx).text().trim().to_string();
        if address.is_empty() {
            self.payout_error = Some("Enter a Lightning address, such as name@example.com.".into());
            cx.notify();
            return;
        }
        self.set_payout(Some(address), cx);
    }

    fn open_payout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.payout_open = true;
        self.payout_error = None;
        let current = self
            .earnings
            .as_ref()
            .and_then(Reply::data)
            .and_then(|d| Earnings::from(d).elsewhere)
            .unwrap_or_default();
        self.payout_input
            .update(cx, |input, cx| input.set_text(current, cx));
        window.focus(&self.payout_input.read(cx).focus_handle(cx), cx);
        cx.notify();
    }

    fn create_wallet(&mut self, cx: &mut Context<Self>) {
        let Some(cli) = self.cli() else { return };
        if self.busy {
            return;
        }
        self.busy = true;
        let rx = off_thread(move || {
            let made = run(&cli, &["wallet", "create", "--show-words"]);
            (made, run(&cli, &["wallet", "balance"]))
        });
        cx.spawn(async move |this, cx| {
            let Ok((made, balance)) = rx.await else {
                return;
            };
            this.update(cx, |this, cx| {
                this.busy = false;
                match made {
                    Reply::Ok(value) => {
                        this.recovery_words = value["recovery_words"].as_str().map(str::to_string);
                    }
                    Reply::Failed(message, _) => this.error = Some(message.into()),
                    Reply::Unavailable => {}
                }
                this.wallet = Some(balance);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn on_scroll_hovered(&mut self, hovered: &bool, _: &mut Window, cx: &mut Context<Self>) {
        if self.scroll.set_list_hovered(*hovered) {
            cx.notify();
        }
    }
}

impl popover::ScrollRailHost for EarnPage {
    fn rail_bar(&mut self) -> &mut popover::MenuScrollbarState {
        self.scroll.rail_bar()
    }

    fn rail_scroll(&self) -> Option<gpui::ScrollHandle> {
        self.scroll.rail_scroll()
    }
}

/// A row's title over its quiet line.
fn row_text(theme: &Theme, title: impl Into<SharedString>, line: Option<String>) -> gpui::Div {
    div()
        .flex_1()
        .min_w(px(160.0))
        .flex()
        .flex_col()
        .child(widgets::row_title(theme, title))
        .when_some(line, |el, line| {
            el.child(widgets::meta_line(
                theme,
                vec![div().child(SharedString::from(line)).into_any_element()],
            ))
        })
}

fn quiet(theme: &Theme, text: &'static str) -> gpui::Div {
    div()
        .flex_none()
        .text_size(crate::typography::ui_rems(12.5))
        .text_color(theme.text_muted)
        .child(text)
}

/// A switch that runs `act` on click, Enter, or Space.
fn switch(
    theme: &Theme,
    id: &'static str,
    label: &'static str,
    on: bool,
    act: fn(&mut EarnPage, &mut Context<EarnPage>),
    cx: &mut Context<EarnPage>,
) -> gpui::Stateful<gpui::Div> {
    let accent = theme.accent;
    div()
        .id(id)
        .flex_none()
        .w(px(widgets::SWITCH_WIDTH))
        .h(px(40.0))
        .flex()
        .items_center()
        .justify_center()
        .role(gpui::Role::Switch)
        .aria_label(label)
        .aria_toggled(if on {
            gpui::Toggled::True
        } else {
            gpui::Toggled::False
        })
        .child(widgets::toggle_switch(theme, on, id))
        .tab_index(0)
        .focus_visible(move |style| style.border_2().border_color(accent))
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, _, cx| act(this, cx)))
        .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
            if !event.is_held && matches!(event.keystroke.key.as_str(), "enter" | "space") {
                cx.stop_propagation();
                act(this, cx);
            }
        }))
}

impl Render for EarnPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).for_settings_surface();
        let installed = self.cli();
        let looked = self.cli.is_some();
        let earning = self.earning();

        // This computer.
        let status = match (&self.status, &self.blocked) {
            _ if self.stopping => "Stopping: finishing the work it took".to_string(),
            (_, Some(blocked)) => blocked.clone(),
            (Some(Reply::Ok(data)), _) => status_line(data, self.setting_up),
            (Some(Reply::Unavailable), _) => NOT_AVAILABLE.into(),
            (Some(Reply::Failed(..)), _) => "Couldn't read this computer's state".into(),
            (None, _) if self.setting_up => "Setting up".into(),
            (None, _) => "Checking this computer…".into(),
        };
        let share = widgets::card_row(&theme, true);
        let share = if !looked {
            share.child(row_text(
                &theme,
                "Share this computer and earn",
                Some(status),
            ))
        } else if installed.is_none() {
            share
                .child(row_text(
                    &theme,
                    "Install the OpenAgents command line to earn",
                    Some(INSTALL.into()),
                ))
                .child(
                    widgets::text_action(&theme, ActionTone::Outlined, "Copy command")
                        .id("earn-copy-install")
                        .on_click(|_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(INSTALL.into()))
                        }),
                )
        } else if matches!(self.status, Some(Reply::Unavailable)) {
            share
                .child(row_text(&theme, "Share this computer and earn", None))
                .child(quiet(&theme, NOT_AVAILABLE))
        } else {
            share
                .child(row_text(
                    &theme,
                    "Share this computer and earn",
                    Some(status),
                ))
                .child(switch(
                    &theme,
                    "earn-switch",
                    "Share this computer and earn",
                    earning,
                    Self::toggle_earning,
                    cx,
                ))
        };
        let computer = widgets::section_card(&theme).mt_0().child(share);

        // Earnings and where they go.
        let earnings = self.earnings.as_ref();
        let mut paid = widgets::section_card(&theme).mt_0();
        let destination = earnings.and_then(Reply::data).map(Earnings::from);
        paid = paid.child(match (earnings, &destination) {
            (_, Some(e)) => {
                let (title, line) = e.lines();
                widgets::card_row(&theme, true).child(row_text(&theme, title, Some(line)))
            }
            (Some(Reply::Unavailable), _) => widgets::card_row(&theme, true)
                .child(row_text(&theme, "Earned so far", None))
                .child(quiet(&theme, NOT_AVAILABLE)),
            (Some(Reply::Failed(..)), _) => widgets::card_row(&theme, true).child(row_text(
                &theme,
                "Earned so far",
                Some("Couldn't read your earnings right now".into()),
            )),
            _ => widgets::card_row(&theme, true).child(row_text(&theme, "Earned so far", None)),
        });
        let elsewhere = destination.as_ref().and_then(|e| e.elsewhere.clone());
        let wallet = self.wallet.as_ref().map(Wallet::from);
        let wallet_row = match (&elsewhere, &wallet) {
            (Some(address), _) => widgets::card_row(&theme, false)
                .child(row_text(&theme, format!("Earnings go to {address}"), None))
                .when(!self.busy, |el| {
                    el.child(
                        widgets::text_action(
                            &theme,
                            ActionTone::Quiet,
                            "Send to your wallet instead",
                        )
                        .id("earn-payout-wallet")
                        .on_click(cx.listener(|this, _, _, cx| this.set_payout(None, cx))),
                    )
                }),
            (None, Some(Wallet::Absent)) => widgets::card_row(&theme, false)
                .child(row_text(
                    &theme,
                    "Earnings go to your wallet",
                    Some("This computer doesn't have a wallet yet".into()),
                ))
                .when(!self.busy, |el| {
                    el.child(
                        widgets::text_action(&theme, ActionTone::Outlined, "Create wallet")
                            .id("earn-create-wallet")
                            .on_click(cx.listener(|this, _, _, cx| this.create_wallet(cx))),
                    )
                }),
            (None, wallet) => widgets::card_row(&theme, false).child(row_text(
                &theme,
                "Earnings go to your wallet",
                match wallet {
                    Some(Wallet::Balance(sats)) => Some(format!("Balance {}", sats_text(*sats))),
                    Some(Wallet::Unavailable) => Some(NOT_AVAILABLE.into()),
                    Some(Wallet::Unreadable) => Some("Couldn't read the balance right now".into()),
                    _ => None,
                },
            )),
        };
        paid = paid.child(wallet_row);
        if let Some(words) = self.recovery_words.clone() {
            paid = paid.child(
                widgets::card_row(&theme, false)
                    .child(
                        row_text(
                            &theme,
                            "Your recovery words",
                            Some("Write them down in order and keep them somewhere safe: they are the only way back to this wallet if this computer is lost, and anyone who has them can spend from it.".into()),
                        )
                        .child(
                            div()
                                .mt(px(8.0))
                                .text_size(crate::typography::ui_rems(13.0))
                                .text_color(theme.text)
                                .child(words),
                        ),
                    )
                    .child(
                        widgets::text_action(&theme, ActionTone::Solid, "I wrote them down")
                            .id("earn-words-saved")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.recovery_words = None;
                                cx.notify();
                            })),
                    ),
            );
        }
        let payout_unavailable =
            self.payout_unavailable || matches!(earnings, Some(Reply::Unavailable));
        let payout_row = if payout_unavailable {
            widgets::card_row(&theme, false)
                .child(row_text(&theme, "Send earnings somewhere else", None))
                .child(quiet(&theme, NOT_AVAILABLE))
        } else if self.payout_open {
            widgets::card_row(&theme, false).child(
                div()
                    .flex_1()
                    .min_w(px(160.0))
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(widgets::field_label(&theme, "Payout address"))
                    .child(popover::dialog_field(
                        self.payout_input.clone().into_any_element(),
                    ))
                    .when_some(self.payout_error.clone(), |el, error| {
                        el.child(
                            div()
                                .text_size(crate::typography::ui_rems(12.0))
                                .text_color(theme.danger_muted)
                                .child(error),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.0))
                            .child(
                                widgets::text_action(&theme, ActionTone::Solid, "Save")
                                    .id("earn-payout-save")
                                    .when(!self.busy, |el| {
                                        el.on_click(
                                            cx.listener(|this, _, _, cx| this.save_payout(cx)),
                                        )
                                    }),
                            )
                            .child(
                                widgets::text_action(&theme, ActionTone::Quiet, "Cancel")
                                    .id("earn-payout-cancel")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.payout_open = false;
                                        this.payout_error = None;
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
        } else {
            widgets::card_row(&theme, false).child(
                widgets::text_action(&theme, ActionTone::Quiet, "Send earnings somewhere else")
                    .id("earn-payout-open")
                    .on_click(cx.listener(|this, _, window, cx| this.open_payout(window, cx))),
            )
        };
        if installed.is_some() {
            paid = paid.child(payout_row);
        }

        // The user's own pause choices.
        let pause_row = |first: bool,
                         id: &'static str,
                         label: &'static str,
                         on: Option<bool>,
                         act: fn(&mut EarnPage, &mut Context<EarnPage>),
                         cx: &mut Context<EarnPage>| {
            let row = widgets::card_row(&theme, first).child(row_text(&theme, label, None));
            match on {
                Some(on) => row.child(switch(&theme, id, label, on, act, cx)),
                None if matches!(self.pauses, Some(Reply::Unavailable)) => {
                    row.child(quiet(&theme, NOT_AVAILABLE))
                }
                None => row,
            }
        };
        let current = self.pauses.as_ref().and_then(Reply::data).map(pauses);
        let pause_card = widgets::section_card(&theme)
            .mt_0()
            .child(pause_row(
                true,
                "earn-pause-battery",
                "Pause on battery",
                current.map(|(battery, _)| battery),
                |this, cx| this.toggle_pause(Pause::Battery, cx),
                cx,
            ))
            .child(pause_row(
                false,
                "earn-pause-hot",
                "Pause when hot",
                current.map(|(_, hot)| hot),
                |this, cx| this.toggle_pause(Pause::Hot, cx),
                cx,
            ));

        let mut column = widgets::page_column()
            .child(widgets::page_header(&theme, "Earn", None))
            .child(widgets::page_subtitle(
                &theme,
                "Let this computer do paid work for others while you aren't using it.",
            ))
            .child(widgets::section(&theme, "This computer", computer).mt(px(24.0)));
        if let Some(error) = self.error.clone() {
            column = column.child(widgets::error_strip(&theme, error));
        }
        if installed.is_some() {
            column = column
                .child(widgets::section(&theme, "Earnings", paid))
                .child(widgets::section(&theme, "Pauses", pause_card));
        }

        let scrollbar = popover::rail(self, "earn-page-scrollbar", &theme, cx);
        div()
            .id("earn-page-host")
            .relative()
            .size_full()
            .on_hover(cx.listener(Self::on_scroll_hovered))
            .child(
                crate::edge_fade::edge_faded(
                    16.0,
                    true,
                    true,
                    div()
                        .id("earn-page")
                        .size_full()
                        .overflow_y_scroll()
                        .track_scroll(&self.scroll.scroll)
                        .child(column),
                )
                .fade_overflow_y(&self.scroll.scroll),
            )
            .children(scrollbar)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn envelope(data: Value) -> String {
        json!({"schema": "openagents.pylon/1", "command": "status", "ok": true,
               "status": "ok", "summary": "s", "data": data})
        .to_string()
    }

    fn status(state: &str, why: Option<&str>) -> String {
        let reply = Reply::read(
            Some(0),
            &envelope(json!({"state": state, "why": why, "model": "clef-flash"})),
        );
        status_line(reply.data().expect("ok"), false)
    }

    #[test]
    fn each_status_reads_in_plain_words() {
        assert_eq!(status("serving", None), "Serving");
        assert_eq!(
            status(
                "paused",
                Some("owner busy: this computer's own work comes first")
            ),
            "Paused: you're using this computer"
        );
        assert_eq!(
            status("paused", Some("no network: the relay cannot be reached")),
            "Paused: no internet connection"
        );
        assert_eq!(
            status("paused", Some("an engine stopped; restarting it")),
            "Paused: restarting the model"
        );
        assert_eq!(
            status("paused", Some("finishing jobs before it stops")),
            "Stopping: finishing the work it took"
        );
        assert_eq!(
            status("starting", Some("the engine is warming up")),
            "Setting up: starting the model"
        );
        assert_eq!(
            status("starting", Some("checking this computer's answers")),
            "Setting up: checking this computer's answers"
        );
        assert_eq!(
            status(
                "off",
                Some("turned off; `openagents pylon up` turns it back on")
            ),
            "Off"
        );
        assert_eq!(status("paused", Some("the host service is busy")), "Paused");
    }

    #[test]
    fn turning_on_reads_as_setting_up() {
        let data = json!({"state": "off", "why": "not set up yet", "model": null});
        assert_eq!(
            status_line(&data, true),
            "Setting up: downloading the model (6.5 GB)"
        );
        let data = json!({"state": "paused", "why": "not running: start the host service", "model": "clef-flash"});
        assert_eq!(status_line(&data, true), "Setting up: starting the model");
        // Once the machine reports its own state, that wins.
        let data = json!({"state": "serving", "model": "clef-flash"});
        assert_eq!(status_line(&data, true), "Serving");
    }

    #[test]
    fn a_blocked_computer_names_the_block_and_its_fix() {
        let doctor = json!({"schema": "openagents.pylon/1", "ok": false, "status": "blocked",
        "data": {"blocks": [
            {"what": "the decision model is not downloaded", "fix": "openagents pylon setup"},
            {"what": "the relay wss://relay.example cannot be reached",
             "fix": "check this computer's internet connection; a pylon only needs to dial out"}
        ]}});
        assert_eq!(
            blocked_line(&doctor).as_deref(),
            Some(
                "Can't serve: this computer can't reach the internet — check the internet connection"
            )
        );
        let disk = json!({"status": "blocked", "data": {}, "next": [
            {"command": "free up space under /Users/me/.openagents/models",
             "why": "3.0 GB free; the model needs 6.5 GB"}]});
        assert_eq!(
            blocked_line(&disk).as_deref(),
            Some(
                "Can't serve: not enough disk space (3.0 GB free; the model needs 6.5 GB) — free up space under /Users/me/.openagents/models"
            )
        );
        let only_setup = json!({"data": {"blocks": [
            {"what": "the decision model is not downloaded", "fix": "openagents pylon setup"}]}});
        assert_eq!(blocked_line(&only_setup), None);
    }

    #[test]
    fn a_usage_error_means_not_in_this_build() {
        let usage = json!({"schema": "openagents.pylon/1", "ok": false, "status": "error",
            "summary": "unknown command `earnings`", "exit_code": 64});
        assert_eq!(
            Reply::read(Some(64), &usage.to_string()),
            Reply::Unavailable
        );
        assert_eq!(
            Reply::read(Some(1), r#"{"error": "nope"}"#),
            Reply::Failed("nope".into(), json!({"error": "nope"}))
        );
        assert_eq!(
            Reply::read(Some(0), "Syncing…\n{\"balance_sats\": 5}"),
            Reply::Ok(json!({"balance_sats": 5}))
        );
    }

    #[test]
    fn zero_earnings_say_they_follow_demand() {
        let e = Earnings::from(&json!({"checked_today": 0, "earned_msat": 0}));
        assert_eq!(
            e.lines(),
            (
                "0 checked decisions today".to_string(),
                "Nothing earned so far. Earnings follow demand and can be zero.".to_string()
            )
        );
    }

    #[test]
    fn earnings_read_as_earned_so_far() {
        let e = Earnings::from(&json!({
            "checked_today": 1, "earned_msat": 1_234_000, "paid_sats": 1000,
            "destination": "alice@getalby.com"}));
        assert_eq!(
            e.lines(),
            (
                "1 checked decision today".to_string(),
                "₿1,234 earned so far · ₿1,000 paid to you".to_string()
            )
        );
        assert_eq!(e.elsewhere.as_deref(), Some("alice@getalby.com"));
        // A Spark address (this computer's wallet) is not "somewhere else".
        let wallet = Earnings::from(&json!({"destination": {"address": "spark1qxyz"}}));
        assert_eq!(wallet.elsewhere, None);
    }

    #[test]
    fn the_wallet_reads_its_balance_or_its_absence() {
        assert_eq!(
            Wallet::from(&Reply::Ok(json!({"balance_sats": 42}))),
            Wallet::Balance(42)
        );
        assert_eq!(
            Wallet::from(&Reply::Failed(
                "This computer doesn't have a wallet yet. Run `openagents wallet create` to start one.".into(),
                Value::Null
            )),
            Wallet::Absent
        );
        assert_eq!(sats_text(1_234_567), "₿1,234,567");
        assert_eq!(sats_text(0), "₿0");
    }

    #[gpui::test]
    fn the_section_draws_every_row_state(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        cx.update(|cx| {
            crate::settings::init(Default::default(), dir.path(), cx);
            gpui_base::init(cx);
            cx.set_global(Theme::default());
        });
        let window = cx.add_window(|_, cx| EarnPage::reading(false, cx));
        let screens: [fn(&mut EarnPage); 4] = [
            // Not looked for the command line yet.
            |_| {},
            // No command line on this computer.
            |page| page.cli = Some(None),
            // Serving, with earnings, a wallet, and both pauses.
            |page| {
                page.cli = Some(Some("/nonexistent/openagents".into()));
                page.status = Some(Reply::Ok(json!({"state": "serving"})));
                page.earnings = Some(Reply::Ok(json!({"checked_today": 3, "earned_sats": 12})));
                page.wallet = Some(Reply::Ok(json!({"balance_sats": 5000})));
                page.pauses = Some(Reply::Ok(json!({"pause_on_battery": true})));
                page.recovery_words = Some("1. one 2. two".into());
            },
            // A build without the newer commands, and no wallet yet.
            |page| {
                page.cli = Some(Some("/nonexistent/openagents".into()));
                page.status = Some(Reply::Ok(json!({"state": "off"})));
                page.earnings = Some(Reply::Unavailable);
                page.pauses = Some(Reply::Unavailable);
                page.wallet = Some(Reply::Failed(NO_WALLET.into(), Value::Null));
                page.blocked = Some("Can't serve: the model engine is out of date".into());
            },
        ];
        for screen in screens {
            window
                .update(cx, |page, window, cx| {
                    screen(page);
                    let _ = page.render(window, cx).into_any_element();
                    cx.notify();
                })
                .unwrap();
            cx.run_until_parked();
        }
        // "Send earnings somewhere else" opens the field with the saved
        // address. (Its input is drawn by the window, not by hand here.)
        window
            .update(cx, |page, window, cx| {
                page.earnings = Some(Reply::Ok(json!({"destination": "alice@getalby.com"})));
                page.open_payout(window, cx);
                assert!(page.payout_open);
                assert_eq!(page.payout_input.read(cx).text(), "alice@getalby.com");
            })
            .unwrap();
    }

    #[test]
    fn pauses_are_off_unless_the_user_turned_them_on() {
        assert_eq!(pauses(&json!({})), (false, false));
        assert_eq!(
            pauses(&json!({"pause_on_battery": true, "pause_when_hot": "on"})),
            (true, true)
        );
    }

    #[test]
    fn page_copy_has_no_system_words() {
        let copy = [
            status("paused", Some("no network: the relay cannot be reached")),
            status("paused", Some("not running: start the host service")),
            plain_block("the relay wss://x cannot be reached"),
            plain_fix("check this computer's internet connection; a pylon only needs to dial out"),
        ];
        for line in copy {
            for word in [
                "beacon",
                "lease",
                "admission",
                "self-check",
                "relay",
                "pylon",
                "aggregate",
            ] {
                assert!(!line.contains(word), "{line}");
            }
        }
    }
}
