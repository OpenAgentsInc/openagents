//! The release acceptance gate's driver (#10080):
//! `openagents-desktop --acceptance DIR [--only NAMES]`.
//!
//! `scripts/release/acceptance.sh` runs it in a temporary HOME whose
//! control socket a real `coder host serve --control` from the same build
//! answers. This is the window's own model, not a stand-in: the real
//! [`DesktopApp`] with its chat panel, sidebar, settings file, Coder lane,
//! and Verse layer, run inline (each request answered before the next
//! tick) instead of in a window. Messages go through the host to the live
//! chat worker exactly as the window sends them, and the host adds the
//! desktop surface context (#10077). A coding reply starts Coder on this
//! computer through the same lane the window uses, on real engines, in the
//! scratch project. The phone scenario pairs a phone-shaped NIP-HOST client
//! with the host and presses Run Coder the way the phone does.
//!
//! Each scenario appends one line to `DIR/results.jsonl`
//! (`{"scenario","status","detail"}`), prints `PASS NAME: …` or
//! `FAIL NAME: …`, and leaves its evidence in `DIR/NAME/`: the chat
//! snapshot, Coder's event lines, the transcript's rows, and captures. The
//! process exits 1 when any scenario failed. docs/release/acceptance.md
//! lists the scenarios and the bugs each one guards.

use super::{DesktopApp, Runner};
use crate::worker::Context;
use openagents_chat::basic_coder::{Role, Turn};
use openagents_chat::coder_events::{CoderEvent, Line};
use openagents_desktop::chrome::{self, Page, State};
use openagents_desktop::control::{HostControl, SocketControl};
use openagents_desktop::model::{Agent, Intent, Model, Screen};
use rust_native_desktop::App;
use rust_native_desktop::input::TextInput;
use serde_json::{Value, json};
use std::cell::Cell;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

/// Every scenario, in the order they run. The chat scenarios share one
/// conversation where the owner's report did ("who are you", then "who can
/// you delegate to", then "do a test delegation now", #10073).
pub const SCENARIOS: [&str; 20] = [
    "ui-placeholder",
    "ui-starter-chips",
    "who-are-you",
    "ui-chips",
    "ui-engines-sidebar",
    "delegate-who",
    "delegate-now",
    "followup-chat",
    "followup-coder",
    "working-directory",
    "delegate-claude",
    "delegate-grok",
    "ui-stop-coder",
    "ui-no-attach",
    "open-deck",
    "ui-filter-sessions",
    "ui-no-verse",
    "route-map",
    "route-map-chat",
    "phone-claude",
];

/// How long a reply may take.
const REPLY_WAIT: Duration = Duration::from_secs(150);
/// After a coding reply, how long Coder may take to start.
const START_WAIT: Duration = Duration::from_secs(120);
/// How long a tiny Coder run may take to finish.
const RUN_WAIT: Duration = Duration::from_secs(600);
/// After an answer, how long to watch for a Coder start that must not come.
const NO_START_GRACE: Duration = Duration::from_secs(15);
/// The relay the Verse layer would watch; nothing listens there, so the
/// check never loads a real world.
const NO_RELAY: &str = "ws://127.0.0.1:9";

/// The results of the scenarios that ran.
type Outcome = Result<String, String>;

struct Gate {
    dir: PathBuf,
    results: std::fs::File,
    failed: usize,
    app: DesktopApp,
    layer: openagents_desktop::grid::Layer,
    /// How many times the Verse layer made its world.
    made: Rc<Cell<usize>>,
    /// The first time the world loaded while the Verse page was not
    /// showing (#10071).
    verse_leak: Option<String>,
    /// What every capture saw of **Filter sessions…** (#10072): the chat
    /// counts it was hidden at, and the first count it wrongly showed or
    /// hid at.
    filter_hidden: Vec<usize>,
    filter_shown: Vec<usize>,
    filter_wrong: Option<String>,
    engines: Engines,
    /// What the first capture of a running Coder saw of the transcript's
    /// **Stop Coder** (#10091): its width against the transcript's, or why
    /// it could not be measured.
    stop_seen: Option<Outcome>,
    /// Chat ids, and their sidebar numbers, by the scenario that made them.
    chats: std::collections::BTreeMap<&'static str, (String, u64)>,
}

/// Which engines' logins the script could make readable.
struct Engines {
    codex: bool,
    claude: bool,
    grok: bool,
    allow_missing: bool,
}

impl Engines {
    fn from_env() -> Engines {
        let text = std::env::var("OPENAGENTS_ACCEPTANCE_ENGINES").unwrap_or_default();
        let on = |name: &str| text.split(',').any(|pair| pair == format!("{name}=1"));
        Engines {
            codex: on("codex"),
            claude: on("claude"),
            grok: on("grok"),
            allow_missing: on("allow_missing"),
        }
    }
}

/// Runs the scenarios `only` names (all when `None`) and writes their
/// results into `dir`. Returns whether every one passed.
///
/// # Errors
/// A sentence when the gate cannot start at all.
pub fn run(dir: &Path, only: Option<&str>) -> Result<bool, String> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let chosen: Vec<&str> = match only {
        Some(names) => {
            let names: Vec<&str> = names.split(',').filter(|n| !n.is_empty()).collect();
            for name in &names {
                if !SCENARIOS.contains(name) {
                    return Err(format!("unknown scenario {name}"));
                }
            }
            SCENARIOS
                .into_iter()
                .filter(|name| names.contains(name))
                .collect()
        }
        None => SCENARIOS.to_vec(),
    };
    let home = super::super::home();
    let results = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("results.jsonl"))
        .map_err(|e| format!("results.jsonl: {e}"))?;
    let (app, layer, made) = window(home)?;
    let mut gate = Gate {
        dir: dir.to_path_buf(),
        results,
        failed: 0,
        app,
        layer,
        made,
        verse_leak: None,
        filter_hidden: Vec::new(),
        filter_shown: Vec::new(),
        filter_wrong: None,
        engines: Engines::from_env(),
        stop_seen: None,
        chats: Default::default(),
    };
    // Let the first refresh and engine report arrive, as a window's first
    // seconds do.
    pump(&mut gate, Duration::from_secs(20), |gate| {
        gate.app.model.host.is_some() && gate.app.model.engine.is_some()
    });
    for name in chosen {
        eprintln!("acceptance: {name}");
        let outcome = match name {
            "ui-placeholder" => ui_placeholder(&mut gate),
            "ui-starter-chips" => ui_starter_chips(&mut gate),
            "who-are-you" => who_are_you(&mut gate),
            "ui-chips" => ui_chips(&mut gate),
            "ui-engines-sidebar" => ui_engines(&mut gate),
            "delegate-who" => delegate_who(&mut gate),
            "delegate-now" => delegate_now(&mut gate),
            "followup-chat" => followup_chat(&mut gate),
            "followup-coder" => followup_coder(&mut gate),
            "working-directory" => working_directory(&mut gate),
            "delegate-claude" => delegate_claude(&mut gate),
            "delegate-grok" => delegate_grok(&mut gate),
            "ui-stop-coder" => ui_stop_coder(&mut gate),
            "ui-no-attach" => ui_no_attach(&mut gate),
            "open-deck" => open_deck(&mut gate),
            "ui-filter-sessions" => ui_filter(&mut gate),
            "ui-no-verse" => ui_no_verse(&mut gate),
            "route-map" => route_map(&mut gate),
            "route-map-chat" => route_map_chat(&mut gate),
            "phone-claude" => phone_claude(&mut gate),
            _ => unreachable!(),
        };
        gate.record(name, outcome);
    }
    Ok(gate.failed == 0)
}

/// The window's model over the host's control socket, inline, with the
/// Verse layer the window builds ([`super::super::backdrop`]) watching
/// nowhere.
fn window(
    home: PathBuf,
) -> Result<(DesktopApp, openagents_desktop::grid::Layer, Rc<Cell<usize>>), String> {
    let socket = crate::platform::control_path().ok_or("this system has no control socket")?;
    if !socket.exists() {
        return Err(format!("no host answers at {}", socket.display()));
    }
    let control: Box<dyn HostControl> = Box::new(SocketControl::new(socket));
    let context = Context::new(
        control,
        None,
        None,
        crate::platform::coder_path(),
        home.clone(),
    );
    let now = Instant::now();
    let mut app = DesktopApp::new(
        Model::new(now, Screen::Home, Agent::Enabled),
        Runner::Inline(context),
        false,
        true,
    );
    app.navigation = Some(State::empty());
    app.chat = Some(openagents_desktop::chat::Panel::new(now));
    app.use_settings_file(coder::task::settings::path());
    let grid = openagents_desktop::grid::Grid::new(NO_RELAY.into(), home, false);
    app.set_grid(grid.clone());
    let made = Rc::new(Cell::new(0));
    let count = made.clone();
    let watcher: openagents_desktop::grid::Watcher = Box::new(move || {
        count.set(count.get() + 1);
        openagents_desktop::backdrop::GridBackdrop::new(NO_RELAY, Box::new(|| true))
    });
    let layer = openagents_desktop::grid::Layer::new(grid, Some(watcher));
    app.present();
    Ok((app, layer, made))
}

impl Gate {
    fn record(&mut self, name: &str, outcome: Outcome) {
        let (status, detail) = match &outcome {
            Ok(detail) if detail.starts_with("SKIP ") => ("SKIP", detail[5..].to_owned()),
            Ok(detail) => ("PASS", detail.clone()),
            Err(detail) => ("FAIL", detail.clone()),
        };
        if status == "FAIL" {
            self.failed += 1;
        }
        let line = json!({ "scenario": name, "status": status, "detail": detail });
        let _ = writeln!(self.results, "{line}");
        println!("{status} {name}: {detail}");
    }

    fn panel(&self) -> &openagents_desktop::chat::Panel {
        self.app.chat.as_ref().expect("the chat panel")
    }

    fn evidence(&self, name: &str) -> PathBuf {
        let dir = self.dir.join(name);
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    /// Writes a PNG of the window at `width`×`height` into `name`'s
    /// evidence, and returns the scene.
    fn capture(
        &mut self,
        name: &str,
        file: &str,
        width: f32,
        height: f32,
        scale: f32,
    ) -> (rust_native_desktop::Frame, rust_native_desktop::Scene) {
        self.app.viewport(width, height, scale);
        self.app.present();
        let (frame, scene) = rust_native_desktop::capture(&mut self.app, width, height, scale);
        let chats = self
            .app
            .navigation
            .as_ref()
            .map_or(0, |state| state.total_chats);
        let shown = scene.bounds.contains_key("chat-search");
        if shown != (chats >= 5) && self.filter_wrong.is_none() {
            self.filter_wrong = Some(format!(
                "Filter sessions {} with {chats} chats ({name}/{file}.png)",
                if shown { "shows" } else { "is hidden" }
            ));
        }
        if shown {
            self.filter_shown.push(chats);
        } else {
            self.filter_hidden.push(chats);
        }
        if let Ok(png) = frame.png() {
            let _ = std::fs::write(self.evidence(name).join(format!("{file}.png")), png);
        }
        (frame, scene)
    }

    /// The chat's snapshot, Coder's lines, and the transcript's rows, saved
    /// into `name`'s evidence.
    fn save_chat(&mut self, name: &str) {
        self.app.present();
        let dir = self.evidence(name);
        if let Some(snapshot) = self.panel().state() {
            let _ = std::fs::write(
                dir.join("snapshot.json"),
                serde_json::to_vec_pretty(snapshot).unwrap_or_default(),
            );
        }
        if let Some(chat) = self.panel().selected_chat()
            && let Some(run) = self.panel().coder_run(chat)
        {
            let lines: Vec<String> = run
                .lines()
                .map(|line| serde_json::to_string(line).unwrap_or_default())
                .collect();
            let _ = std::fs::write(dir.join("coder-lines.jsonl"), lines.join("\n"));
        }
        let _ = std::fs::write(dir.join("transcript.txt"), self.transcript().join("\n\n"));
        if let Some(engine) = &self.app.model.engine {
            let _ = std::fs::write(
                dir.join("engine-report.json"),
                serde_json::to_vec_pretty(engine).unwrap_or_default(),
            );
        }
    }

    /// Each transcript row's words, as the window shows them.
    fn transcript(&self) -> Vec<String> {
        self.panel()
            .transcript_rows()
            .iter()
            .map(|row| {
                let mut words = Vec::new();
                text_of(row, &mut words);
                words.join(" ")
            })
            .collect()
    }

    /// Grok Build's login, for the scenario that runs on it (#10091).
    fn need_grok(&self) -> Option<Outcome> {
        (!self.engines.grok).then(|| {
            if self.engines.allow_missing {
                Ok("SKIP no Grok Build login (--allow-missing-engine)".into())
            } else {
                Err("no Grok Build login to run this scenario".into())
            }
        })
    }

    /// Measures the transcript's **Stop Coder** once, the first time a run
    /// shows it (#10091): it must be as wide as its words, as the phone
    /// draws it, never the reading band's width.
    fn measure_stop(&mut self, chat: &str) {
        if self.stop_seen.is_some()
            || !self
                .panel()
                .coder_run(chat)
                .is_some_and(|run| run.active() && run.task.is_some())
        {
            return;
        }
        let (_, scene) = self.capture("ui-stop-coder", "running-1200x840-1x", 1200.0, 840.0, 1.0);
        let Some(band) = scene.surface_rect(openagents_desktop::chat::TRANSCRIPT) else {
            return;
        };
        let Some(stop) = self.panel().transcript.control_bounds("coder-stop") else {
            return;
        };
        self.stop_seen = Some(if stop.w < 160.0 && stop.w < band.w / 3.0 {
            Ok(format!(
                "Stop Coder is {:.0}pt wide in a {:.0}pt transcript while Coder runs; capture running-1200x840-1x.png",
                stop.w, band.w
            ))
        } else {
            Err(format!(
                "Stop Coder spans {:.0}pt of a {:.0}pt transcript while Coder runs (running-1200x840-1x.png)",
                stop.w, band.w
            ))
        });
    }

    fn need(&self, codex_or_claude: bool, claude: bool) -> Option<Outcome> {
        let missing = if claude && !self.engines.claude {
            Some("Claude Code")
        } else if codex_or_claude && !self.engines.codex && !self.engines.claude {
            Some("Codex or Claude Code")
        } else {
            None
        };
        missing.map(|engine| {
            if self.engines.allow_missing {
                Ok(format!("SKIP no {engine} login (--allow-missing-engine)"))
            } else {
                Err(format!("no {engine} login to run this scenario"))
            }
        })
    }
}

/// The words a view node shows, in order.
fn text_of<I: std::fmt::Debug>(node: &rust_native::Node<I>, out: &mut Vec<String>) {
    use rust_native::Element;
    match &node.element {
        Element::Text { value, .. } => out.push(value.clone()),
        Element::Button { label, .. } => out.push(label.clone()),
        Element::Markdown { blocks, .. } => out.push(format!("{blocks:?}")),
        Element::Message { note, children, .. } => {
            if let Some(note) = note {
                out.push(note.clone());
            }
            children.iter().for_each(|child| text_of(child, out));
        }
        Element::Stack { children, .. }
        | Element::List { children, .. }
        | Element::Transcript { children, .. } => {
            children.iter().for_each(|child| text_of(child, out));
        }
        _ => {
            // Any other element's strings, so nothing shown is missed.
            out.push(format!("{:?}", node.element));
        }
    }
}

/// Ticks the window as its event loop would, with the Verse layer asked
/// for a frame each time, until `done` or `wait` passes. Returns `done`.
fn pump(gate: &mut Gate, wait: Duration, mut done: impl FnMut(&mut Gate) -> bool) -> bool {
    let deadline = Instant::now() + wait;
    loop {
        let now = Instant::now();
        gate.app.tick(now);
        let _ = rust_native_desktop::backdrop::Backdrop::next_frame(&mut gate.layer, now);
        let verse = gate
            .app
            .navigation
            .as_ref()
            .is_some_and(|state| state.page == Page::Grid);
        if !verse && gate.layer.loaded() && gate.verse_leak.is_none() {
            gate.verse_leak = Some(format!(
                "the Verse world was loaded while the page was {:?}",
                gate.app.navigation.as_ref().map(|state| state.page)
            ));
        }
        if done(gate) {
            return true;
        }
        if now >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(120));
    }
}

/// Opens a new chat, as the sidebar's New chat does, and returns its id.
fn new_chat(gate: &mut Gate, scenario: &'static str) -> Result<String, String> {
    let before = gate.panel().selected_chat().map(str::to_owned);
    gate.app.activate(
        Intent::Navigate {
            action: chrome::Action::NewChat,
        },
        Instant::now(),
    );
    let opened = pump(gate, Duration::from_secs(30), |gate| {
        let panel = gate.panel();
        let selected = panel.selected_chat().map(str::to_owned);
        selected.is_some()
            && selected != before
            && panel.state().and_then(|s| s.chat.clone()) == selected
            && gate
                .app
                .navigation
                .as_ref()
                .is_some_and(|state| state.selected().is_some())
    });
    if !opened {
        return Err("the host did not open a new chat".into());
    }
    let chat = gate.panel().selected_chat().unwrap_or_default().to_owned();
    let number = gate
        .app
        .navigation
        .as_ref()
        .and_then(State::selected)
        .map_or(0, |row| row.id);
    gate.chats.insert(scenario, (chat.clone(), number));
    Ok(chat)
}

/// Shows the chat a scenario opened earlier.
fn select(gate: &mut Gate, chat: &str) -> Result<(), String> {
    if gate.panel().selected_chat() == Some(chat) {
        return Ok(());
    }
    let id = gate
        .chats
        .values()
        .find(|(id, _)| id == chat)
        .map(|(_, number)| *number)
        .ok_or("the earlier chat is not known")?;
    gate.app.activate(
        Intent::Navigate {
            action: chrome::Action::SelectChat { id },
        },
        Instant::now(),
    );
    if pump(gate, Duration::from_secs(30), |gate| {
        gate.panel().selected_chat() == Some(chat)
            && gate.panel().state().and_then(|s| s.chat.as_deref()) == Some(chat)
    }) {
        Ok(())
    } else {
        Err("the earlier chat did not open again".into())
    }
}

/// Types `text` into the composer and presses Send, as a person does, then
/// waits for the reply. Returns the reply turn.
fn send(gate: &mut Gate, text: &str) -> Result<Turn, String> {
    let chat = gate
        .panel()
        .selected_chat()
        .ok_or("no chat is open")?
        .to_owned();
    let before = gate.panel().state().map_or(0, |s| s.total);
    let now = Instant::now();
    gate.app.text_input(TextInput::Commit(text), now);
    gate.app.activate(
        Intent::Chat {
            action: openagents_desktop::chat_action::Action::Send,
        },
        now,
    );
    let mut failure = None;
    let replied = pump(gate, REPLY_WAIT, |gate| {
        let Some(snapshot) = gate.panel().state() else {
            return false;
        };
        if snapshot.chat.as_deref() != Some(chat.as_str()) {
            return false;
        }
        if let Some(why) = &snapshot.failure {
            failure = Some(why.clone());
            return true;
        }
        !snapshot.busy
            && snapshot.total >= before + 2
            && snapshot
                .turns
                .last()
                .is_some_and(|turn| turn.role == Role::Assistant)
    });
    if let Some(why) = failure {
        return Err(format!("the chat failed: {why}"));
    }
    if !replied {
        return Err(format!("no reply within {}s", REPLY_WAIT.as_secs()));
    }
    let snapshot = gate.panel().state().ok_or("no chat state")?;
    let reply = snapshot.turns.last().cloned().ok_or("no reply")?;
    if reply.text.trim().is_empty() {
        return Err("the reply was empty".into());
    }
    Ok(reply)
}

/// Watches `chat` for [`NO_START_GRACE`]; an error names a Coder start.
fn no_start(gate: &mut Gate, chat: &str) -> Result<(), String> {
    let started = pump(gate, NO_START_GRACE, |gate| {
        gate.panel().coder_run(chat).is_some()
            || gate.panel().state().is_some_and(|s| s.coder.is_some())
    });
    if started {
        Err("Coder started for a message the reply answered".into())
    } else {
        Ok(())
    }
}

/// What Coder did for `chat`: started (with its `coder_started` event),
/// then finished or failed.
struct RunSeen {
    task: Option<String>,
    started: Option<openagents_chat::coder_events::Started>,
    finished: Option<openagents_chat::coder_events::Finished>,
    failure: Option<String>,
}

/// Waits for Coder to start for `chat`, and then, when `finish`, for its
/// turn to end.
fn follow_run(gate: &mut Gate, chat: &str, finish: bool) -> RunSeen {
    let mut seen = RunSeen {
        task: None,
        started: None,
        finished: None,
        failure: None,
    };
    let began = pump(gate, START_WAIT, |gate| {
        gate.panel().coder_run(chat).is_some()
    });
    if !began {
        seen.failure = Some(format!(
            "Coder did not start within {}s of the reply",
            START_WAIT.as_secs()
        ));
        return seen;
    }
    let wait = if finish { RUN_WAIT } else { START_WAIT };
    pump(gate, wait, |gate| {
        gate.measure_stop(chat);
        let Some(run) = gate.panel().coder_run(chat) else {
            return false;
        };
        let lines: Vec<&Line> = run.lines().collect();
        let started = lines
            .iter()
            .any(|line| matches!(line.event, CoderEvent::CoderStarted(_)));
        let ended = lines.iter().any(|line| {
            matches!(
                line.event,
                CoderEvent::Result(_) | CoderEvent::Failure(_) | CoderEvent::Stopped(_)
            )
        });
        // A start the runner refused shows as a failed run with no lines.
        let refused = run.task.is_none()
            && !run.busy()
            && gate
                .transcript()
                .iter()
                .any(|row| row.contains("Coder didn't") || row.contains("did not start"));
        refused || ended || (started && !finish)
    });
    gate.app.present();
    if let Some(run) = gate.panel().coder_run(chat) {
        seen.task = run.task.clone();
        for line in run.lines() {
            match &line.event {
                CoderEvent::CoderStarted(started) => seen.started = Some(started.clone()),
                CoderEvent::Result(done) => seen.finished = Some(done.clone()),
                CoderEvent::Failure(failure) => seen.failure = Some(failure.message.clone()),
                CoderEvent::Stopped(stopped) => {
                    seen.failure = Some(format!("Coder stopped: {stopped:?}"))
                }
                _ => {}
            }
        }
    }
    if seen.started.is_none() && seen.failure.is_none() {
        // The card's words say why it did not start.
        seen.failure = gate
            .transcript()
            .into_iter()
            .find(|row| row.contains("Coder didn't") || row.contains("did not start"))
            .or_else(|| Some("Coder did not start".into()));
    }
    if finish && seen.started.is_some() && seen.finished.is_none() && seen.failure.is_none() {
        seen.failure = Some(format!(
            "Coder did not finish within {}s",
            RUN_WAIT.as_secs()
        ));
    }
    seen
}

/// The first user step's message in a turn's ATIF trajectory: the prompt
/// Coder received.
fn first_prompt(trajectory: &Path) -> Option<String> {
    let text = std::fs::read_to_string(trajectory).ok()?;
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find_map(|value| {
            let step = value.get("step").unwrap_or(&value);
            (step.get("source").and_then(Value::as_str) == Some("User")
                || step.get("source").and_then(Value::as_str) == Some("user"))
            .then(|| step.get("message").map(|m| m.to_string()))
            .flatten()
        })
}

// The scenarios.

/// An empty chat's centered composer paints "Message OpenAgents…" (#10072).
fn ui_placeholder(gate: &mut Gate) -> Outcome {
    let chat = new_chat(gate, "who-are-you")?;
    let _ = chat;
    if !gate.panel().composer_centered() {
        return Err("an empty chat's composer is not centered".into());
    }
    let (frame, scene) = gate.capture(
        "ui-placeholder",
        "empty-chat-1200x840-2x",
        1200.0,
        840.0,
        2.0,
    );
    let field = scene
        .surface_rect(openagents_desktop::chat::COMPOSER)
        .ok_or("the composer field was not laid out")?;
    let ink = placeholder_ink(&frame, field, 2.0);
    let floor = (40.0 * 2.0 * 2.0) as usize;
    if ink <= floor {
        return Err(format!(
            "the empty chat's composer shows no placeholder ({ink} faint pixels, want more than {floor})"
        ));
    }
    Ok(format!(
        "centered composer paints its placeholder ({ink} faint pixels); capture empty-chat-1200x840-2x.png"
    ))
}

/// A new chat's starters (the phone's shared list) are chips directly
/// above the centered composer, in the follow-ups' row and style, inside
/// the column at the default and minimum sizes; a tap sends one, and once
/// the chat has a message they leave and the composer docks (#10097).
fn ui_starter_chips(gate: &mut Gate) -> Outcome {
    let chat = new_chat(gate, "ui-starter-chips")?;
    if !gate.panel().composer_centered() {
        return Err("a new chat's composer is not centered".into());
    }
    let starters: Vec<_> = openagents_chat_app::first_run::SUGGESTIONS
        .iter()
        .take(openagents_chat_app::first_run::SUGGESTIONS_SHOWN)
        .collect();
    let mut lines_at = vec![];
    let mut tapped: Option<String> = None;
    for (width, height, file) in [
        (1200.0, 840.0, "starters-1200x840-2x"),
        (760.0, 540.0, "starters-760x540-2x"),
    ] {
        let (_, scene) = gate.capture("ui-starter-chips", file, width, height, 2.0);
        let row = *scene.bounds.get("chat-starters").ok_or(format!(
            "no chat-starters row above the composer ({file}.png)"
        ))?;
        let card = *scene
            .bounds
            .get("chat-composer-card")
            .ok_or("no composer card")?;
        if row.y + row.h > card.y || card.y - (row.y + row.h) > 12.0 {
            return Err(format!(
                "the starters are not directly above the composer: {row:?} vs {card:?} ({file}.png)"
            ));
        }
        let mut lines = vec![];
        for starter in &starters {
            let key = format!("coder-suggest-{}", starter.id);
            let chip = scene
                .hits
                .iter()
                .find(|hit| hit.key == key)
                .ok_or(format!("no {:?} chip ({file}.png)", starter.label))?;
            if chip.rect.w >= card.w / 2.0 || chip.rect.h > 32.0 {
                return Err(format!("{key} is not a small chip: {:?}", chip.rect));
            }
            if chip.rect.x < row.x - 0.5 || chip.rect.x + chip.rect.w > row.x + row.w + 0.5 {
                return Err(format!("{key} leaves the column: {:?} {row:?}", chip.rect));
            }
            if gate.panel().transcript.control_bounds(&key).is_some() {
                return Err(format!("{key} is in the transcript"));
            }
            // A click at the chip's middle is routed as the window routes
            // it: to the chip, not the empty transcript beneath (#10098).
            let (x, y) = (
                chip.rect.x + chip.rect.w / 2.0,
                chip.rect.y + chip.rect.h / 2.0,
            );
            if let Some((surface, _)) = scene.surface_at(x, y) {
                return Err(format!(
                    "a click on {key} goes to the {surface} surface ({file}.png)"
                ));
            }
            let clicked = scene.hit(x, y).map(|hit| hit.key.clone());
            if clicked.as_deref() != Some(key.as_str()) {
                return Err(format!("a click on {key} reaches {clicked:?} ({file}.png)"));
            }
            if starter.id == starters[0].id {
                tapped.get_or_insert(key.clone());
            }
            if !lines.contains(&(chip.rect.y as i32)) {
                lines.push(chip.rect.y as i32);
            }
        }
        if let Some(send) = scene.hits.iter().find(|hit| hit.key == "chat-send")
            && let Some((surface, _)) = scene.surface_at(
                send.rect.x + send.rect.w / 2.0,
                send.rect.y + send.rect.h / 2.0,
            )
        {
            return Err(format!(
                "a click on Send goes to the {surface} surface ({file}.png)"
            ));
        }
        lines_at.push(lines.len());
    }
    // A tap on "Who are you?" sends it, as the phone's chip does: the key
    // the click at its middle reached.
    let first = starters[0];
    let tapped = tapped.ok_or("no click reached the first starter")?;
    let before = gate.panel().state().map_or(0, |s| s.total);
    gate.app.activate(
        Intent::Chat {
            action: openagents_desktop::chat_action::Action::Card { key: tapped },
        },
        Instant::now(),
    );
    let mut failure = None;
    let replied = pump(gate, REPLY_WAIT, |gate| {
        let Some(snapshot) = gate.panel().state() else {
            return false;
        };
        if snapshot.chat.as_deref() != Some(chat.as_str()) {
            return false;
        }
        if let Some(why) = &snapshot.failure {
            failure = Some(why.clone());
            return true;
        }
        !snapshot.busy
            && snapshot.total >= before + 2
            && snapshot
                .turns
                .last()
                .is_some_and(|turn| turn.role == Role::Assistant)
    });
    gate.save_chat("ui-starter-chips");
    if let Some(why) = failure {
        return Err(format!("the chat failed: {why}"));
    }
    if !replied {
        return Err(format!(
            "no reply to {:?} within {}s",
            first.message,
            REPLY_WAIT.as_secs()
        ));
    }
    let asked = gate
        .panel()
        .state()
        .and_then(|s| s.turns.iter().find(|turn| turn.role == Role::User))
        .map(|turn| turn.text.clone())
        .unwrap_or_default();
    if asked != first.message {
        return Err(format!("the tap sent {asked:?}, not {:?}", first.message));
    }
    let (_, scene) = gate.capture(
        "ui-starter-chips",
        "after-reply-1200x840-2x",
        1200.0,
        840.0,
        2.0,
    );
    if scene.bounds.contains_key("chat-starters")
        || scene
            .hits
            .iter()
            .any(|hit| hit.key.starts_with("coder-suggest-"))
    {
        return Err("the starters stay after the chat has a message".into());
    }
    if gate.panel().composer_centered() {
        return Err("the composer stays centered after a reply".into());
    }
    let followups = scene
        .hits
        .iter()
        .filter(|hit| hit.key.starts_with("coder-followup-"))
        .count();
    Ok(format!(
        "{} starters ({}) directly above the centered composer, on {} line(s) at 1200x840 and {} at 760x540; a tap sent {:?}; after the reply none remain and {followups} follow-up chips sit above the docked composer; captures starters-*.png, after-reply-1200x840-2x.png",
        starters.len(),
        starters
            .iter()
            .map(|starter| starter.label)
            .collect::<Vec<_>>()
            .join(", "),
        lines_at[0],
        lines_at[1],
        first.message
    ))
}

/// Pixels of the placeholder's faint ink inside `rect`.
fn placeholder_ink(
    frame: &rust_native_desktop::Frame,
    rect: rust_native_desktop::Rect,
    scale: f32,
) -> usize {
    let faint = openagents_chat_app::visual::FAINT;
    let mut count = 0;
    let x0 = (rect.x * scale) as usize;
    let y0 = (rect.y * scale) as usize;
    for y in y0..((rect.y + rect.h) * scale) as usize {
        for x in x0..((rect.x + rect.w) * scale) as usize {
            let [r, g, b] = frame.pixel(x, y);
            if r.abs_diff(faint.red) < 24
                && g.abs_diff(faint.green) < 24
                && b.abs_diff(faint.blue) < 24
            {
                count += 1;
            }
        }
    }
    count
}

/// "who are you" → an answer with suggestions, and no Coder start.
fn who_are_you(gate: &mut Gate) -> Outcome {
    let chat = match gate.chats.get("who-are-you") {
        Some((chat, _)) => chat.clone(),
        None => new_chat(gate, "who-are-you")?,
    };
    select(gate, &chat)?;
    let reply = send(gate, "who are you")?;
    gate.save_chat("who-are-you");
    let followups = reply.meta.as_ref().map_or(0, |meta| meta.followups.len());
    if followups == 0 {
        return Err(format!(
            "the reply has no suggestions: {:?}",
            excerpt(&reply.text)
        ));
    }
    no_start(gate, &chat)?;
    gate.save_chat("who-are-you");
    Ok(format!(
        "answered ({:?}) with {followups} suggestions; no Coder start",
        excerpt(&reply.text)
    ))
}

/// The latest reply's suggestions are chips directly above the composer,
/// not in the transcript (#10075).
fn ui_chips(gate: &mut Gate) -> Outcome {
    let chat = gate
        .chats
        .get("who-are-you")
        .map(|(chat, _)| chat.clone())
        .ok_or("needs who-are-you's reply")?;
    select(gate, &chat)?;
    // Run on its own (`--only ui-chips`), it asks who-are-you's question
    // first, so the chat has a reply with suggestions.
    if gate.panel().state().is_some_and(|s| s.turns.is_empty()) {
        who_are_you(gate)?;
    }
    let (_, scene) = gate.capture("ui-chips", "chips-1200x840-1x", 1200.0, 840.0, 1.0);
    let row = *scene
        .bounds
        .get("chat-followups")
        .ok_or("no chat-followups row above the composer")?;
    let card = *scene
        .bounds
        .get("chat-composer-card")
        .ok_or("no composer card")?;
    if row.y + row.h > card.y || card.y - (row.y + row.h) > 12.0 {
        return Err(format!(
            "the suggestions are not directly above the composer: {row:?} vs {card:?}"
        ));
    }
    let chips: Vec<_> = scene
        .hits
        .iter()
        .filter(|hit| hit.key.starts_with("coder-followup-"))
        .collect();
    if chips.is_empty() {
        return Err("no suggestion chips".into());
    }
    for chip in &chips {
        if chip.rect.w >= card.w / 2.0 || chip.rect.h > 32.0 {
            return Err(format!("{} is not a small chip: {:?}", chip.key, chip.rect));
        }
        if gate.panel().transcript.control_bounds(&chip.key).is_some() {
            return Err(format!("{} is in the transcript", chip.key));
        }
    }
    Ok(format!(
        "{} chips in a row {:.0}pt above the composer; none in the transcript",
        chips.len(),
        card.y - (row.y + row.h)
    ))
}

/// Each engine is a condensed row in the sidebar; the transcript has no
/// engine block (#10072).
fn ui_engines(gate: &mut Gate) -> Outcome {
    pump(gate, Duration::from_secs(20), |gate| {
        gate.app.model.engine.is_some()
    });
    // Each route, then each engine beside them a person's own runs can
    // use, such as Grok Build when installed (#10091).
    let routes = gate
        .app
        .model
        .engine
        .as_ref()
        .map(|engine| {
            engine.routes.len().min(4)
                + openagents_chat_app::engine::extra_accounts(engine)
                    .len()
                    .min(4)
        })
        .ok_or("the host sent no engine report")?;
    let (_, scene) = gate.capture(
        "ui-engines-sidebar",
        "sidebar-1200x840-1x",
        1200.0,
        840.0,
        1.0,
    );
    let rows: Vec<&String> = scene
        .bounds
        .keys()
        .filter(|key| key.starts_with("sidebar-engine-row-"))
        .collect();
    if rows.len() != routes || routes == 0 {
        return Err(format!(
            "{} engine rows in the sidebar for {routes} engines",
            rows.len()
        ));
    }
    if let Some(strip) = scene
        .bounds
        .keys()
        .find(|key| key.as_str() == "engine-strip" || key.starts_with("engine-strip-"))
    {
        return Err(format!("an engine block ({strip}) is still in the chat"));
    }
    let footer = scene.bounds.get("sidebar-footer").copied();
    for key in &rows {
        let row = scene.bounds[*key];
        if row.h > 32.0 {
            return Err(format!("{key} is not condensed: {row:?}"));
        }
        if footer.is_some_and(|footer| row.y >= footer.y) {
            return Err(format!("{key} is not above the sidebar footer"));
        }
    }
    let names: Vec<String> = gate
        .app
        .model
        .engine
        .as_ref()
        .map(|engine| {
            engine
                .routes
                .iter()
                .map(|r| r.name.clone())
                .chain(
                    openagents_chat_app::engine::extra_accounts(engine)
                        .into_iter()
                        .map(|a| a.name.clone()),
                )
                .collect()
        })
        .unwrap_or_default();
    if gate.transcript().iter().any(|row| {
        names
            .iter()
            .any(|name| row.contains(&format!("{name} ·")) && row.contains("signed in"))
    }) {
        return Err("the transcript shows an engine's status".into());
    }
    Ok(format!(
        "{} engine rows ({}) in the sidebar, none in the transcript",
        rows.len(),
        names.join(", ")
    ))
}

/// "who can you delegate to" → an answer, no start.
fn delegate_who(gate: &mut Gate) -> Outcome {
    let chat = gate
        .chats
        .get("who-are-you")
        .map(|(chat, _)| chat.clone())
        .ok_or("needs who-are-you's chat")?;
    select(gate, &chat)?;
    let reply = send(gate, "who can you delegate to")?;
    no_start(gate, &chat)?;
    gate.save_chat("delegate-who");
    Ok(format!(
        "answered ({:?}); no Coder start",
        excerpt(&reply.text)
    ))
}

/// "do a test delegation now", in the owner's conversation → Coder starts,
/// runs, and finishes in the linked-worktree project, on the message that
/// asked, with no Gym card and no decision rows (#10073, #10078).
fn delegate_now(gate: &mut Gate) -> Outcome {
    if let Some(skip) = gate.need(true, false) {
        return skip;
    }
    const ASK: &str = "do a test delegation now";
    let chat = match gate.chats.get("who-are-you") {
        Some((chat, _)) => chat.clone(),
        None => new_chat(gate, "who-are-you")?,
    };
    select(gate, &chat)?;
    let reply = send(gate, ASK)?;
    let meta = reply.meta.clone().unwrap_or_default();
    let mut problems = Vec::new();
    if !meta.cards.is_empty() {
        problems.push(format!(
            "the reply carries {} Gym card(s)",
            meta.cards.len()
        ));
    }
    let seen = follow_run(gate, &chat, true);
    gate.save_chat("delegate-now");
    let _ = gate.capture("delegate-now", "chat-1200x840-1x", 1200.0, 840.0, 1.0);
    if let Some(failure) = &seen.failure {
        problems.push(format!("Coder: {failure}"));
    }
    if gate
        .transcript()
        .iter()
        .any(|row| row.contains("openagents.microcoder.judge") || row.contains("\"state\":{"))
    {
        problems.push("a decision call's raw row is in the transcript".into());
    }
    let project = std::env::var("OPENAGENTS_ACCEPTANCE_PROJECT").unwrap_or_default();
    if let Some(started) = &seen.started {
        let checkout = std::fs::canonicalize(&started.checkout).unwrap_or_default();
        let wanted = std::fs::canonicalize(&project).unwrap_or_default();
        if checkout != wanted {
            problems.push(format!(
                "Coder ran in {} instead of the project {project}",
                started.checkout
            ));
        }
    }
    if let Some(done) = &seen.finished {
        match first_prompt(Path::new(&done.trajectory)) {
            Some(prompt) if prompt.to_lowercase().contains(ASK) => {}
            Some(prompt) => problems.push(format!(
                "Coder's prompt is not the message that asked: {:?}",
                excerpt(&prompt)
            )),
            None => problems.push(format!("no prompt in {}", done.trajectory)),
        }
        let _ = std::fs::copy(
            &done.trajectory,
            gate.evidence("delegate-now").join("turn.atif.jsonl"),
        );
    }
    if problems.is_empty() {
        let started = seen.started.as_ref();
        Ok(format!(
            "Coder {} on {} in {} and finished: {:?}",
            seen.task.unwrap_or_default(),
            started.map_or("?", |s| s.provider.as_str()),
            started.map_or("?", |s| s.checkout.as_str()),
            excerpt(&seen.finished.map(|f| f.summary).unwrap_or_default())
        ))
    } else {
        Err(problems.join("; "))
    }
}

/// The chat delegate-now's run finished in, with that run's task.
fn finished_chat(gate: &mut Gate) -> Result<(String, String), String> {
    let chat = gate
        .chats
        .get("who-are-you")
        .map(|(chat, _)| chat.clone())
        .ok_or("needs delegate-now's chat")?;
    select(gate, &chat)?;
    let finished = pump(gate, RUN_WAIT, |gate| {
        gate.panel()
            .coder_run(&chat)
            .is_some_and(|run| run.routes_followups() && run.finished())
    });
    if !finished {
        return Err("needs delegate-now's Coder run, finished".into());
    }
    let task = gate
        .panel()
        .coder_run(&chat)
        .and_then(|run| run.task.clone())
        .ok_or("the run has no task")?;
    Ok((chat, task))
}

/// The highest turn Coder started in `chat`.
fn last_turn(gate: &Gate, chat: &str) -> usize {
    gate.panel().coder_run(chat).map_or(0, |run| {
        run.lines()
            .filter_map(|line| match &line.event {
                CoderEvent::CoderStarted(started) => Some(started.turn),
                _ => None,
            })
            .max()
            .unwrap_or(0)
    })
}

/// After delegate-now's run finished, "summarize what happened" is the
/// router's: the composer says "Message OpenAgents…", the chat answers from
/// the run's result, and Coder takes no new turn (#10094).
fn followup_chat(gate: &mut Gate) -> Outcome {
    if let Some(skip) = gate.need(true, false) {
        return skip;
    }
    let (chat, _) = finished_chat(gate)?;
    let mut problems = Vec::new();
    let placeholder = gate.panel().composer_placeholder();
    if placeholder != "Message OpenAgents…" {
        problems.push(format!(
            "the composer says {placeholder:?} after the run finished"
        ));
    }
    let turns = last_turn(gate, &chat);
    let reply = send(gate, "summarize what happened")?;
    let meta = reply.meta.clone().unwrap_or_default();
    if openagents_chat::delegation::offered(Some(&meta), true) {
        problems.push(format!(
            "the reply hands it to Coder (route {:?})",
            meta.route
        ));
    }
    let more = pump(gate, NO_START_GRACE, |gate| last_turn(gate, &chat) > turns);
    if more {
        problems.push("Coder took another turn for a question about its run".into());
    }
    gate.save_chat("followup-chat");
    let _ = gate.capture("followup-chat", "chat-1200x840-1x", 1200.0, 840.0, 1.0);
    if problems.is_empty() {
        Ok(format!(
            "answered in chat on route {:?}: {:?}; no new Coder turn",
            meta.route.unwrap_or_default(),
            excerpt(&reply.text)
        ))
    } else {
        Err(problems.join("; "))
    }
}

/// Then "now also list the top-level files in a note" is more work: the
/// router hands it to Coder, which continues the same task as its next
/// turn and finishes, and the person's message shows before the
/// "Coder continued" card (#10094).
fn followup_coder(gate: &mut Gate) -> Outcome {
    if let Some(skip) = gate.need(true, false) {
        return skip;
    }
    const ASK: &str = "now also list the top-level files in a note";
    let (chat, task) = finished_chat(gate)?;
    let turns = last_turn(gate, &chat);
    let reply = send(gate, ASK)?;
    let meta = reply.meta.clone().unwrap_or_default();
    let mut problems = Vec::new();
    if !openagents_chat::delegation::offered(Some(&meta), true) {
        problems.push(format!(
            "the reply did not hand it to Coder (route {:?}): {:?}",
            meta.route,
            excerpt(&reply.text)
        ));
    }
    let next = turns + 1;
    let started = pump(gate, START_WAIT, |gate| last_turn(gate, &chat) >= next);
    if !started {
        problems.push(format!(
            "Coder did not start turn {next} within {}s",
            START_WAIT.as_secs()
        ));
    }
    let ended = started
        && pump(gate, RUN_WAIT, |gate| {
            gate.panel().coder_run(&chat).is_some_and(|run| {
                run.lines().any(|line| match &line.event {
                    CoderEvent::Result(done) => done.turn >= next,
                    CoderEvent::Failure(failed) => failed.turn >= next,
                    CoderEvent::Stopped(stopped) => stopped.turn >= next,
                    _ => false,
                })
            })
        });
    gate.app.present();
    gate.save_chat("followup-coder");
    let _ = gate.capture("followup-coder", "chat-1200x840-1x", 1200.0, 840.0, 1.0);
    if started && !ended {
        problems.push(format!(
            "turn {next} did not end within {}s",
            RUN_WAIT.as_secs()
        ));
    }
    let run = gate.panel().coder_run(&chat);
    if run.and_then(|run| run.task.clone()).as_deref() != Some(task.as_str()) {
        problems.push("the follow-up started another task instead of the next turn".into());
    }
    let finished = run.and_then(|run| {
        run.lines().find_map(|line| match &line.event {
            CoderEvent::Result(done) if done.turn >= next => Some(done.clone()),
            _ => None,
        })
    });
    if ended && finished.is_none() {
        problems.push(format!("turn {next} ended without a result"));
    }
    // The person's message, then the card it started.
    let rows = gate.transcript();
    let asked = rows.iter().position(|row| row.contains(ASK));
    let card = rows
        .iter()
        .position(|row| row.contains(&format!("(turn {next})")));
    match (asked, card) {
        (Some(asked), Some(card)) if asked < card => {}
        (Some(_), Some(_)) => {
            problems.push("the \"Coder continued\" card shows above the message".into())
        }
        _ => problems.push(format!(
            "the transcript lacks the message or the turn {next} card"
        )),
    }
    if problems.is_empty() {
        Ok(format!(
            "Coder continued task {task} as turn {next} and finished: {:?}",
            excerpt(&finished.map(|done| done.summary).unwrap_or_default())
        ))
    } else {
        Err(problems.join("; "))
    }
}

/// "What's the working directory right now?" on the desktop → names the
/// project folder, never "connect a computer", and starts no Coder
/// (#10077, #10079).
fn working_directory(gate: &mut Gate) -> Outcome {
    let chat = new_chat(gate, "working-directory")?;
    let reply = send(gate, "What's the working directory right now?")?;
    gate.save_chat("working-directory");
    let project = std::env::var("OPENAGENTS_ACCEPTANCE_PROJECT").unwrap_or_default();
    let folder = Path::new(&project)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let text = reply.text.to_lowercase();
    let mut problems = Vec::new();
    if folder.is_empty() || !text.contains(&folder.to_lowercase()) {
        problems.push(format!(
            "the answer does not name the project folder {folder}: {:?}",
            excerpt(&reply.text)
        ));
    }
    if text.contains("connect a computer") || text.contains("connect your computer") {
        problems.push("the answer tells the person to connect a computer".into());
    }
    if let Err(started) = no_start(gate, &chat) {
        problems.push(started);
    }
    gate.save_chat("working-directory");
    if problems.is_empty() {
        Ok(format!(
            "answered {:?}; no Coder start",
            excerpt(&reply.text)
        ))
    } else {
        Err(problems.join("; "))
    }
}

/// "do a test delegation to claude" → Coder starts on Claude Code, the
/// start card agrees with the engine readings, and the message shows once
/// (#10076).
fn delegate_claude(gate: &mut Gate) -> Outcome {
    if let Some(skip) = gate.need(false, true) {
        return skip;
    }
    const ASK: &str = "do a test delegation to claude";
    let chat = new_chat(gate, "delegate-claude")?;
    let reply = send(gate, ASK)?;
    let engine = reply.meta.as_ref().and_then(|meta| meta.engine);
    let seen = follow_run(gate, &chat, true);
    gate.save_chat("delegate-claude");
    let _ = gate.capture("delegate-claude", "chat-1200x840-1x", 1200.0, 840.0, 1.0);
    let mut problems = Vec::new();
    if engine != Some(nostr::cj_conversation::Engine::ClaudeCode) {
        problems.push(format!(
            "the reply's offer asked for {engine:?}, not Claude Code"
        ));
    }
    match &seen.started {
        Some(started) if started.provider == "claude" => {}
        Some(started) => problems.push(format!(
            "Coder started on {} ({}): {:?}",
            started.provider, started.model, started.reason
        )),
        None => problems.push(format!(
            "Coder did not start: {}",
            seen.failure.clone().unwrap_or_default()
        )),
    }
    if let Some(started) = &seen.started
        && let Some(contradiction) = contradicts_readings(gate, &started.reason)
    {
        problems.push(contradiction);
    }
    if let Some(failure) = &seen.failure
        && seen.started.is_some()
    {
        problems.push(format!("Coder: {failure}"));
    }
    let shown = gate
        .transcript()
        .iter()
        .filter(|row| row.to_lowercase().contains(ASK))
        .count();
    if shown != 1 {
        problems.push(format!("the message shows {shown} times in the chat"));
    }
    if problems.is_empty() {
        Ok(format!(
            "Coder started on Claude Code ({}) and finished; message shown once",
            seen.started.map(|s| s.reason).unwrap_or_default()
        ))
    } else {
        Err(problems.join("; "))
    }
}

/// "do a test delegation to grok" → with no settings file, Grok Build is
/// allowed by default: the offer names Grok Build, and real Grok Build
/// starts and finishes in the scratch project (#10091).
fn delegate_grok(gate: &mut Gate) -> Outcome {
    if let Some(skip) = gate.need_grok() {
        return skip;
    }
    const ASK: &str = "do a test delegation to grok";
    let chat = new_chat(gate, "delegate-grok")?;
    let reply = send(gate, ASK)?;
    let engine = reply.meta.as_ref().and_then(|meta| meta.engine);
    let seen = follow_run(gate, &chat, true);
    gate.save_chat("delegate-grok");
    let _ = gate.capture("delegate-grok", "chat-1200x840-1x", 1200.0, 840.0, 1.0);
    let mut problems = Vec::new();
    if engine != Some(nostr::cj_conversation::Engine::GrokBuild) {
        problems.push(format!(
            "the reply's offer asked for {engine:?}, not Grok Build"
        ));
    }
    match &seen.started {
        Some(started) if started.provider == "grok" => {}
        Some(started) => problems.push(format!(
            "Coder started on {} ({}): {:?}",
            started.provider, started.model, started.reason
        )),
        None => problems.push(format!(
            "Coder did not start: {}",
            seen.failure.clone().unwrap_or_default()
        )),
    }
    if gate
        .transcript()
        .iter()
        .any(|row| row.contains("not one of the engines your Coder settings allow"))
    {
        problems.push("the chat says Grok Build is not allowed".into());
    }
    if let Some(failure) = &seen.failure
        && seen.started.is_some()
    {
        problems.push(format!("Coder: {failure}"));
    }
    if seen.started.is_some() && seen.finished.is_none() && seen.failure.is_none() {
        problems.push("Coder did not finish".into());
    }
    let project = std::env::var("OPENAGENTS_ACCEPTANCE_PROJECT").unwrap_or_default();
    if let Some(started) = &seen.started {
        let checkout = std::fs::canonicalize(&started.checkout).unwrap_or_default();
        let wanted = std::fs::canonicalize(&project).unwrap_or_default();
        if checkout != wanted {
            problems.push(format!(
                "Coder ran in {} instead of the project {project}",
                started.checkout
            ));
        }
    }
    let mut commands = 0;
    if let Some(done) = &seen.finished {
        let _ = std::fs::copy(
            &done.trajectory,
            gate.evidence("delegate-grok").join("turn.atif.jsonl"),
        );
        // At the default access Grok Build runs inside Coder's boundary,
        // and the host allows the commands it asks for (#10092): the turn
        // ran at least one.
        commands = ran_commands(&std::fs::read_to_string(&done.trajectory).unwrap_or_default());
        if commands == 0 {
            problems.push("Grok Build ran no shell command".into());
        }
    }
    if problems.is_empty() {
        let started = seen.started.as_ref();
        Ok(format!(
            "Coder {} started on Grok Build ({}, {:?}), ran {commands} command(s), and finished: {:?}",
            seen.task.unwrap_or_default(),
            started.map_or("?", |s| s.model.as_str()),
            started.map(|s| s.reason.clone()).unwrap_or_default(),
            excerpt(&seen.finished.map(|f| f.summary).unwrap_or_default())
        ))
    } else {
        Err(problems.join("; "))
    }
}

/// The shell commands a Grok Build turn's trajectory shows completed: tool
/// calls with a `command` argument. Grok Build runs a read-only command
/// such as `ls` without asking, and asks for the rest, which the host
/// allows inside its boundary.
fn ran_commands(trajectory: &str) -> usize {
    trajectory
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|line| line.pointer("/step/call").cloned())
        .filter(|call| call["arguments"]["command"].is_string() && call["outcome"] == "Completed")
        .count()
}

/// While Coder runs, the transcript's **Stop Coder** is as wide as its
/// words, as on the phone, not the transcript's width (#10091). Measured
/// during the first run an earlier scenario followed; alone, it starts one.
fn ui_stop_coder(gate: &mut Gate) -> Outcome {
    if gate.stop_seen.is_none() {
        if let Some(skip) = gate.need(true, false) {
            return skip;
        }
        let chat = new_chat(gate, "ui-stop-coder")?;
        send(gate, "do a test delegation now")?;
        let seen = follow_run(gate, &chat, false);
        if gate.stop_seen.is_none() {
            pump(gate, START_WAIT, |gate| {
                gate.measure_stop(&chat);
                gate.stop_seen.is_some()
                    || gate
                        .panel()
                        .coder_run(&chat)
                        .is_some_and(|run| !run.active())
            });
        }
        gate.save_chat("ui-stop-coder");
        if gate.stop_seen.is_none() {
            return Err(format!(
                "no running Coder showed Stop Coder to measure{}",
                seen.failure.map(|f| format!(": {f}")).unwrap_or_default()
            ));
        }
    }
    gate.stop_seen
        .clone()
        .unwrap_or_else(|| Err("not measured".into()))
}

/// A start reason that calls a provider out of capacity when the host's
/// own engine readings say it is not (#10073).
fn contradicts_readings(gate: &Gate, reason: &str) -> Option<String> {
    let engine = serde_json::to_value(gate.app.model.engine.as_ref()?).ok()?;
    let lower = reason.to_lowercase();
    if !lower.contains("limit") {
        return None;
    }
    for route in engine["routes"].as_array()? {
        let name = route["name"].as_str().unwrap_or_default();
        if !lower.contains(&format!("{} reached", name.to_lowercase()))
            && !lower.contains(&format!("{} is at its", name.to_lowercase()))
        {
            continue;
        }
        let usage = &route["usage"];
        let reached = usage["limit_reached"].as_bool().unwrap_or(false);
        let used = usage["used_percent"].as_u64().unwrap_or(0);
        if !reached && used < 95 {
            return Some(format!(
                "the start card says {name} is at its limit while its reading is {used}%"
            ));
        }
    }
    None
}

/// The desktop is text only (#10095, the switch the phone shares, #10093):
/// the composer has no attach control, and an image dropped on the window
/// or pasted is dropped quietly; the draft stays words only.
fn ui_no_attach(gate: &mut Gate) -> Outcome {
    if gate.panel().attachments_enabled() {
        return Err("attachments are on: the desktop should be text only".into());
    }
    new_chat(gate, "ui-no-attach")?;
    let mut problems = Vec::new();
    for (file, width, height, scale) in [
        ("composer-1200x840-2x", 1200.0, 840.0, 2.0),
        ("composer-760x540-1x", 760.0, 540.0, 1.0),
    ] {
        let (_, scene) = gate.capture("ui-no-attach", file, width, height, scale);
        if !scene.hits.iter().any(|hit| hit.key == "chat-send") {
            problems.push(format!("no Send control ({file})"));
        }
        for key in ["chat-attach", "chat-paste-image"] {
            if scene.hits.iter().any(|hit| hit.key == key) {
                problems.push(format!("the composer shows {key} ({file})"));
            }
        }
    }
    // An image dropped on the window, as from the Finder.
    let image = openagents_chat_app::attachments::Image::pixels(
        64,
        32,
        (0..64 * 32)
            .flat_map(|i| {
                if i % 64 < 32 {
                    [220, 30, 30, 255]
                } else {
                    [30, 30, 220, 255]
                }
            })
            .collect(),
    )?;
    let path = gate.evidence("ui-no-attach").join("dropped.png");
    std::fs::write(&path, image.bytes.as_slice()).map_err(|e| e.to_string())?;
    gate.app.dropped_file(path, Instant::now());
    // A pasted image, through the composer's image paste (no clipboard is
    // read while attachments are off).
    gate.app.activate(
        Intent::Chat {
            action: openagents_desktop::chat_action::Action::PasteImage,
        },
        Instant::now(),
    );
    // Give a read the time it would take; nothing may arrive.
    pump(gate, Duration::from_secs(3), |gate| {
        !gate.panel().images().is_empty()
    });
    let panel = gate.panel();
    if !panel.images().is_empty() {
        problems.push(format!(
            "{} image(s) reached the draft",
            panel.images().len()
        ));
    }
    if let Some(notice) = panel.notice() {
        problems.push(format!("a notice showed: {notice:?}"));
    }
    if !panel.draft().is_empty() {
        problems.push(format!("the draft took {:?}", excerpt(panel.draft())));
    }
    let (_, scene) = gate.capture("ui-no-attach", "after-drop-1200x840-2x", 1200.0, 840.0, 2.0);
    if scene
        .hits
        .iter()
        .any(|hit| hit.key.starts_with("image-remove-") || hit.key == "chat-attach")
    {
        problems.push("an image card or attach control shows after the drop".into());
    }
    if problems.is_empty() {
        Ok("no attach control at 1200x840 or 760x540; a dropped PNG and an image paste were dropped quietly, draft empty; captures composer-*.png, after-drop-1200x840-2x.png".into())
    } else {
        Err(problems.join("; "))
    }
}

/// "open the three devdays later deck" on the desktop → the typed offer
/// opens that deck in the slide viewer (#10058).
fn open_deck(gate: &mut Gate) -> Outcome {
    let chat = new_chat(gate, "open-deck")?;
    let reply = send(gate, "open the three devdays later deck")?;
    pump(gate, Duration::from_secs(3), |_| false);
    gate.save_chat("open-deck");
    let offered = openagents_desktop::chat::presentation_offer(reply.meta.as_ref());
    let opened = gate.app.slides.is_some();
    // A deck held for the viewer that nothing opened yet: the window opens
    // it only on the person's next key or click.
    let held = if opened {
        None
    } else {
        gate.app
            .chat
            .as_mut()
            .and_then(|panel| panel.take_presentation())
    };
    let _ = gate.capture("open-deck", "deck-1200x840-1x", 1200.0, 840.0, 1.0);
    // Close it again, so later scenarios see the chat.
    gate.app.slides = None;
    gate.app.present();
    let _ = chat;
    match (offered.as_deref(), opened) {
        (Some("three-devdays-later"), true) => {
            Ok("typed open_presentation offer for three-devdays-later; the viewer opened".into())
        }
        (Some(deck), false) if held.as_deref() == Some(deck) => Err(format!(
            "the typed offer for {deck} arrived, but the viewer did not open; the deck waits \
             for the person's next key or click (reply {:?})",
            excerpt(&reply.text)
        )),
        (offered, opened) => Err(format!(
            "offer {offered:?}, viewer opened: {opened}; reply {:?}",
            excerpt(&reply.text)
        )),
    }
}

/// "Filter sessions…" hides with fewer than five chats and shows with
/// five or more (#10072), in every capture the gate took.
fn ui_filter(gate: &mut Gate) -> Outcome {
    let count = |gate: &Gate| gate.app.navigation.as_ref().map_or(0, |s| s.total_chats);
    // Both sides must be seen: below five, and at five or more.
    if gate.filter_hidden.iter().all(|chats| *chats >= 5) {
        let shown = count(gate);
        if shown >= 5 {
            return Err(format!(
                "never saw the sidebar with fewer than five chats (now {shown}); run it with the chat scenarios"
            ));
        }
        let _ = gate.capture(
            "ui-filter-sessions",
            &format!("sidebar-{shown}-chats"),
            1200.0,
            840.0,
            1.0,
        );
    }
    while count(gate) < 5 {
        new_chat(gate, "ui-filter-sessions")?;
        send(gate, "hi")?;
    }
    let shown = count(gate);
    let _ = gate.capture(
        "ui-filter-sessions",
        &format!("sidebar-{shown}-chats"),
        1200.0,
        840.0,
        1.0,
    );
    if let Some(wrong) = &gate.filter_wrong {
        return Err(wrong.clone());
    }
    let hidden: std::collections::BTreeSet<usize> = gate.filter_hidden.iter().copied().collect();
    let showed: std::collections::BTreeSet<usize> = gate.filter_shown.iter().copied().collect();
    Ok(format!(
        "hidden at {hidden:?} chats, shown at {showed:?} chats"
    ))
}

/// No Verse behind chat: the world never loads on a chat page, loads on
/// the Verse page, and is released when the person leaves it (#10071).
fn ui_no_verse(gate: &mut Gate) -> Outcome {
    if let Some(leak) = &gate.verse_leak {
        return Err(leak.clone());
    }
    if gate.made.get() != 0 {
        return Err("the Verse world was made before the Verse page opened".into());
    }
    let (_, scene) = gate.capture("ui-no-verse", "chat-1200x840-1x", 1200.0, 840.0, 1.0);
    if let Some(key) = scene.bounds.keys().find(|key| key.starts_with("grid-")) {
        return Err(format!("the chat page lays out the Verse's {key}"));
    }
    let look = rust_native_desktop::backdrop::Backdrop::look(&gate.layer);
    if look.is_none_or(|look| look.dim < 1.0) {
        return Err(format!("the chat page's backdrop is not plain: {look:?}"));
    }
    let chat = gate.app.navigation.as_ref().map(|state| state.page);
    gate.app.activate(
        Intent::Navigate {
            action: chrome::Action::Grid,
        },
        Instant::now(),
    );
    pump(gate, Duration::from_secs(2), |gate| gate.layer.loaded());
    let loaded = gate.layer.loaded() && gate.made.get() == 1;
    let _ = gate.capture("ui-no-verse", "verse-page-1200x840-1x", 1200.0, 840.0, 1.0);
    gate.app.activate(
        Intent::Navigate {
            action: chrome::Action::Back,
        },
        Instant::now(),
    );
    pump(gate, Duration::from_secs(2), |gate| !gate.layer.loaded());
    if !loaded {
        return Err("the Verse page did not load its world (the check could not see it)".into());
    }
    if gate.layer.loaded() {
        return Err("leaving the Verse page did not release its world".into());
    }
    Ok(format!(
        "no world on the chat page ({chat:?}); loaded only on the Verse page and released after"
    ))
}

/// The Map page (#10085): open it from the sidebar's footer, zoom into
/// `work.dispatch`, inspect Coder, open the Gaps panel, and find
/// `capability.missing` there as a gap with a next step.
fn route_map(gate: &mut Gate) -> Outcome {
    use openagents_chat_app::route_map::GapKind;
    use openagents_desktop::route_map::{Action as Map, Panel};
    let now = Instant::now();
    let previous = gate.app.navigation.as_ref().map(|state| state.page);
    gate.app.activate(
        Intent::Navigate {
            action: chrome::Action::Map,
        },
        now,
    );
    let _ = gate.capture("route-map", "map-1200x840-1x", 1200.0, 840.0, 1.0);
    let (_, scene) = gate.capture("route-map", "map-1200x840-1x", 1200.0, 840.0, 1.0);
    if !scene.bounds.contains_key("route-map-surface") {
        return Err("the Map page laid out no map surface".into());
    }
    let find = |gate: &Gate, id: &str| gate.app.map_view().and_then(|map| map.find(id));
    let (Some(dispatch), Some(coder)) = (find(gate, "route:work.dispatch"), find(gate, "coder"))
    else {
        return Err("the map has no work.dispatch route or no Coder".into());
    };
    let map = |gate: &mut Gate, action| {
        gate.app.activate(Intent::Map { action }, Instant::now());
        gate.app.settle_map();
    };
    let zoom_before = gate.app.map.as_ref().map(|page| page.camera().zoom);
    map(gate, Map::Select { node: dispatch });
    let zoom_after = gate.app.map.as_ref().map(|page| page.camera().zoom);
    let _ = gate.capture("route-map", "work-dispatch-1200x840-1x", 1200.0, 840.0, 1.0);
    map(gate, Map::Select { node: coder });
    let (_, scene) = gate.capture(
        "route-map",
        "inspector-coder-1200x840-1x",
        1200.0,
        840.0,
        1.0,
    );
    let inspected = gate.app.map.as_ref().and_then(|page| page.selected()) == Some(coder)
        && scene.bounds.contains_key("map-inspector-title");
    map(gate, Map::Panel { panel: Panel::Gaps });
    let gap = gate.app.map_view().and_then(|map| {
        map.gaps
            .iter()
            .position(|gap| gap.kind == GapKind::NoPlugin)
            .map(|index| (index, map.gaps[index].step.label().to_string()))
    });
    let (_, scene) = gate.capture("route-map", "gaps-1200x840-1x", 1200.0, 840.0, 1.0);
    // Leave the map: back to the chat that showed, or to Phones and
    // computers when none did.
    let listed = |gate: &Gate, id: u64| {
        gate.app
            .navigation
            .as_ref()
            .is_some_and(|state| state.chats.iter().any(|chat| chat.id == id))
    };
    let action = match previous {
        Some(chrome::Page::Chat(id)) if listed(gate, id) => chrome::Action::SelectChat { id },
        _ => chrome::Action::Computers,
    };
    gate.app
        .activate(Intent::Navigate { action }, Instant::now());
    let released = gate.app.map_view().is_none();
    let zoomed = matches!((zoom_before, zoom_after), (Some(a), Some(b)) if b > a);
    match gap {
        Some((index, step))
            if zoomed
                && inspected
                && released
                && scene.bounds.contains_key(&format!("map-gap-{index}")) =>
        {
            Ok(format!(
                "opened from the footer; zoomed into work.dispatch; inspected Coder; capability.missing is a gap with \"{step}\"; released on leaving"
            ))
        }
        Some(_) => Err(format!(
            "zoomed {zoomed}, inspected Coder {inspected}, gap shown in the panel {}, released {released}",
            scene.bounds.keys().any(|k| k.starts_with("map-gap-"))
        )),
        None => Err("capability.missing is not a gap on the map".into()),
    }
}

/// From chat: "show me how you route things" gets the router's typed
/// `routes.map` offer, whose tap opens the Map page (#10085).
fn route_map_chat(gate: &mut Gate) -> Outcome {
    new_chat(gate, "route-map-chat")?;
    let reply = send(gate, "show me how you route things")?;
    gate.save_chat("route-map-chat");
    let offered = reply.meta.as_ref().is_some_and(|meta| {
        meta.offers.iter().any(|offer| {
            matches!(
                offer,
                openagents_chat::router::Offer::OpenScreen {
                    screen: openagents_chat::router::Screen::RoutesMap
                }
            )
        })
    });
    if !offered {
        return Err(format!(
            "no routes.map offer (route {:?}, answer {:?}); reply {:?}",
            reply.meta.as_ref().and_then(|m| m.route.clone()),
            reply.meta.as_ref().and_then(|m| m.answer.clone()),
            excerpt(&reply.text)
        ));
    }
    let _ = gate.capture("route-map-chat", "offer-1200x840-1x", 1200.0, 840.0, 1.0);
    let key = gate
        .app
        .chat
        .as_ref()
        .and_then(|panel| {
            let mut buttons = Vec::new();
            for row in panel.transcript_rows() {
                text_buttons(row, &mut buttons);
            }
            buttons
                .into_iter()
                .find(|(_, label)| label == "Open the map")
                .map(|(key, _)| key)
        })
        .ok_or("the offer shows no Open the map button")?;
    gate.app.activate(
        Intent::Chat {
            action: openagents_desktop::chat_action::Action::Card { key },
        },
        Instant::now(),
    );
    let opened = gate.app.map_view().is_some();
    let _ = gate.capture("route-map-chat", "map-1200x840-1x", 1200.0, 840.0, 1.0);
    gate.app.activate(
        Intent::Navigate {
            action: chrome::Action::Back,
        },
        Instant::now(),
    );
    if opened {
        Ok(format!(
            "typed routes.map offer on {:?}; Open the map opened the Map page",
            reply.meta.as_ref().and_then(|m| m.answer.clone())
        ))
    } else {
        Err("the tap on Open the map did not open the Map page".into())
    }
}

/// Every button in a transcript row: its key and label.
fn text_buttons(node: &rust_native::Node<()>, out: &mut Vec<(String, String)>) {
    match &node.element {
        rust_native::Element::Button { label, .. } => out.push((node.key.clone(), label.clone())),
        rust_native::Element::Stack { children, .. } => {
            children.iter().for_each(|child| text_buttons(child, out));
        }
        _ => {}
    }
}

/// A phone-shaped client, paired over NIP-HOST, asks "do a test delegation
/// to claude" as the phone does and presses Run Coder: the computer's run
/// must start on Claude Code (#10081).
fn phone_claude(gate: &mut Gate) -> Outcome {
    if let Some(skip) = gate.need(false, true) {
        return skip;
    }
    use coder_computers::live::{FileStore, Live, Settings, load_or_create_key};
    use coder_computers::{Capabilities, Computers, Platform};
    use openagents_chat::basic_coder::{Door, Relay, Reply};
    const ASK: &str = "do a test delegation to claude";
    let evidence = gate.evidence("phone-claude");
    // The phone's pairing: the code the app shows, redeemed by the phone.
    let socket = crate::platform::control_path().ok_or("no control socket")?;
    let mut control = SocketControl::new(socket);
    let invite = control
        .invite()
        .map_err(|e| format!("the host made no invitation: {e}"))?;
    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let store = super::super::home().join(".openagents/acceptance-phone");
    std::fs::create_dir_all(&store).map_err(|e| e.to_string())?;
    let secret = load_or_create_key(&store)?;
    let live = Live::open(
        Settings::new(Platform::Phone),
        secret,
        Box::new(FileStore::open(&store)?),
        runtime.handle().clone(),
    )
    .map_err(|e| e.to_string())?;
    // The phone scans the app's code (`openagents-connect:`).
    let paired = runtime
        .block_on(async {
            tokio::time::timeout(Duration::from_secs(90), live.pairing().pair(&invite.code)).await
        })
        .map_err(|_| "pairing timed out".to_owned())?
        .map_err(|failure| format!("pairing failed: {failure:?}"))?;
    let host = paired.host.clone();
    let mut computers = Computers::new(
        Box::new(live),
        Capabilities {
            platform: Platform::Phone,
            camera: false,
        },
        "acceptance",
    )
    .map_err(|e| e.to_string())?;
    let _ = computers.finish_first_run();
    let deadline = Instant::now() + Duration::from_secs(90);
    let workspace = loop {
        // Ready as the phone's Run Coder sees it: the computer's workspaces
        // listed and its presence (what it supports) received.
        let listed = computers
            .refresh_workspaces(&host)
            .ok()
            .and_then(|()| computers.snapshot().host(&host).cloned())
            .filter(|record| record.presence.is_some())
            .and_then(|record| {
                record
                    .workspaces
                    .as_ref()
                    .and_then(|listed| openagents_chat::delegation::project(listed, |_| None))
            });
        if let Some(workspace) = listed {
            break workspace;
        }
        if Instant::now() >= deadline {
            return Err("the paired phone never saw the computer's workspace".into());
        }
        let _ = computers.refresh();
        std::thread::sleep(Duration::from_secs(2));
    };
    let label = computers
        .snapshot()
        .host(&host)
        .map(|record| record.label.clone())
        .unwrap_or_else(|| "Acceptance Mac".into());
    // The phone's own chat: straight to the hosted worker, with the phone's
    // context naming its paired computer (`CoderTab::context`).
    let chat_secret = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
    let door = Relay::new(
        openagents_chat::basic_coder::RELAY,
        openagents_chat::basic_coder::WORKER,
        chat_secret,
    )?;
    let mut turns = vec![Turn::user(ASK)];
    let context = openagents_chat::router::Context {
        surface: openagents_chat::router::Surface::Phone,
        computer_ready: true,
        computer: Some(openagents_chat::router::Computer::Paired {
            name: label.clone(),
        }),
        ..openagents_chat::router::Context::default()
    };
    let reply = std::sync::Arc::new(std::sync::Mutex::new(Reply::default()));
    runtime.block_on(async {
        let asking = door.ask(turns.clone(), context, reply.clone());
        let _ = tokio::time::timeout(REPLY_WAIT, asking).await;
    });
    let reply = openagents_chat::basic_coder::lock(&reply).clone();
    if let Some(failure) = &reply.failure {
        return Err(format!("the phone's chat failed: {failure:?}"));
    }
    if !reply.done {
        return Err("the phone's chat got no reply".into());
    }
    let _ = std::fs::write(
        evidence.join("phone-reply.json"),
        serde_json::to_vec_pretty(&json!({
            "text": reply.text,
            "meta": serde_json::to_value(&reply.meta).unwrap_or(Value::Null),
        }))
        .unwrap_or_default(),
    );
    let engine = reply.meta.engine;
    turns.push(Turn::assistant(
        reply.text.clone(),
        Some(reply.meta.clone()),
    ));
    // Run Coder, as the phone presses it (`CoderTab::run_coder`).
    let title = openagents_chat::delegation::title(ASK, &turns);
    let prompt = openagents_chat::delegation::prompt(ASK, &turns);
    let _ = title;
    // The engine the reply's typed offer named (#10081), as
    // `CoderTab::run_coder` reads it.
    let requested = openagents_chat::delegation::requested(&turns);
    let task = computers
        .start_task_requesting(&host, &workspace, &prompt, &[], requested)
        .map_err(|refusal| format!("Run Coder failed: {}", refusal.reason()))?;
    // The computer's auto-start journal says which engine it started on.
    let root = super::super::home().join(".openagents/host");
    let deadline = Instant::now() + START_WAIT;
    let started = loop {
        let entries = coder::task::autostart::journal(&root);
        let mine: Vec<_> = entries
            .iter()
            .filter(|entry| entry.task.as_deref() == Some(task.as_str()))
            .collect();
        if let Some(entry) = mine.iter().find(|entry| entry.event == "started") {
            break Ok(entry.detail.clone().unwrap_or_default());
        }
        if let Some(entry) = mine.iter().find(|entry| {
            matches!(
                entry.event.as_str(),
                "refused" | "no_capacity" | "not_started" | "skipped"
            )
        }) {
            break Err(format!(
                "the computer did not start the task: {} {}",
                entry.event,
                entry.detail.clone().unwrap_or_default()
            ));
        }
        if Instant::now() >= deadline {
            break Err(format!(
                "the computer did not start task {task} within {}s",
                START_WAIT.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_secs(1));
    };
    let journal: Vec<String> = coder::task::autostart::journal(&root)
        .iter()
        .filter(|entry| entry.task.as_deref() == Some(task.as_str()))
        .map(|entry| serde_json::to_string(entry).unwrap_or_default())
        .collect();
    let _ = std::fs::write(evidence.join("autostart.jsonl"), journal.join("\n"));
    let detail = started?;
    let provider = detail
        .split_whitespace()
        .nth(1)
        .and_then(|route| route.split(':').next())
        .unwrap_or_default()
        .to_owned();
    let mut problems = Vec::new();
    if engine != Some(nostr::cj_conversation::Engine::ClaudeCode) {
        problems.push(format!(
            "the phone's reply asked for {engine:?}, not Claude Code"
        ));
    }
    if provider != "claude" {
        problems.push(format!(
            "the computer started task {task} on {provider} ({detail})"
        ));
    }
    if problems.is_empty() {
        Ok(format!(
            "the computer started task {task} on Claude Code ({detail})"
        ))
    } else {
        Err(problems.join("; "))
    }
}

fn excerpt(text: &str) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() > 160 {
        format!("{}…", flat.chars().take(160).collect::<String>())
    } else {
        flat
    }
}
