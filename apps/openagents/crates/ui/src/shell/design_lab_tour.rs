//! The desktop tour for Design Lab (#12010): every reachable screen of the
//! desktop app, drawn offscreen with fixture data and saved as PNGs.
//!
//! Each [`Shot`] builds a fresh headless app with its own settings folder,
//! opens the whole [`Shell`] in a 1280 x 800 point window (16:10, drawn at 2x),
//! puts it in one state, and saves `<area>--<screen>.png`, or
//! `<area>--<screen>@dark.png` in dark mode. Nothing appears on screen, no
//! engine starts, and the signed-in account is a fixture the caller serves
//! from loopback, so no real account or chat is read.
//!
//! Add a screen by adding a [`Shot`] to [`shots`]. Element trees aren't saved:
//! GPUI keeps element bounds private to its test context, so the board offers
//! only box selection on desktop screens.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ::openagents_account::Controller;
use gpui::{AnyWindowHandle, HeadlessAppContext, WindowHandle, size};
use openagents_doc::SessionMessageEntry;
use serde_json::json;

use super::*;
use crate::openagents_account::AppAccount;

/// The window every shot draws: a 16:10 laptop window, in points.
pub const WINDOW: (f32, f32) = (1280.0, 800.0);

/// Which account the shot is signed in to. Both are fixtures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Account {
    SignedIn,
    SignedOut,
}

/// What the shell shows before the shot's last step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    /// A boot gate instead of the workspace.
    Gate(Gate),
    Home,
    /// The chat route with this fixture chat selected, or the new-chat canvas.
    Chat(Option<&'static str>),
    Settings(SettingsSection),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    SignIn,
    Organization,
    Failed,
}

/// The last step: opens a menu, sheet, or panel over the start.
pub type Step = fn(&mut Shell, &mut Window, &mut Context<Shell>);

/// One screen of the tour.
#[derive(Clone)]
pub struct Shot {
    pub area: &'static str,
    pub screen: String,
    pub account: Account,
    pub start: Start,
    pub one_list: bool,
    pub step: Option<Step>,
}

impl Shot {
    fn new(area: &'static str, screen: &str, start: Start) -> Self {
        Self {
            area,
            screen: screen.into(),
            account: Account::SignedIn,
            start,
            one_list: false,
            step: None,
        }
    }

    fn signed_out(mut self) -> Self {
        self.account = Account::SignedOut;
        self
    }

    fn one_list(mut self) -> Self {
        self.one_list = true;
        self
    }

    fn then(mut self, step: Step) -> Self {
        self.step = Some(step);
        self
    }

    /// `<area>--<screen>`, the name Design Lab files the screen under.
    pub fn name(&self) -> String {
        format!("{}--{}", self.area, self.screen)
    }

    /// The PNG's file name in light or dark mode.
    pub fn file(&self, dark: bool) -> String {
        format!("{}{}.png", self.name(), if dark { "@dark" } else { "" })
    }
}

/// Every screen the tour captures, in order. Add a screen here.
pub fn shots() -> Vec<Shot> {
    use Start::{Chat, Gate as G, Home};
    let mut shots = vec![
        // Before the workspace: the boot gates.
        Shot::new("welcome", "sign-in", G(Gate::SignIn)).signed_out(),
        Shot::new("welcome", "organization", G(Gate::Organization)),
        Shot::new("welcome", "engine-failed", G(Gate::Failed)),
        // Home, signed in (the identity row) and signed out.
        Shot::new("home", "home", Home),
        Shot::new("home", "signed-out", Home).signed_out(),
        Shot::new("home", "account-menu", Home).then(open_account_menu),
        // The sidebar: projects, pinned, sessions.
        Shot::new("sidebar", "by-project", Chat(Some(COMPOSER))),
        Shot::new("sidebar", "one-list", Chat(Some(COMPOSER))).one_list(),
        Shot::new("sidebar", "user-menu", Chat(Some(COMPOSER))).then(open_user_menu),
        Shot::new("sidebar", "rename-chat", Chat(Some(COMPOSER))).then(rename_chat),
        Shot::new("sidebar", "delete-chat", Chat(Some(COMPOSER))).then(delete_chat),
        // A chat: messages, tool rows, the composer, and its menus.
        Shot::new("chat", "conversation", Chat(Some(COMPOSER))),
        Shot::new("chat", "working", Chat(Some(WORKING))),
        Shot::new("chat", "new-session", Chat(None)).then(new_session),
        Shot::new("chat", "runs-on", Chat(Some(COMPOSER))).then(open_runs_on),
        Shot::new("chat", "model-menu", Chat(Some(COMPOSER))).then(open_model_menu),
        Shot::new("chat", "command-palette", Chat(Some(COMPOSER))).then(open_palette),
        Shot::new("chat", "voice", Chat(Some(COMPOSER))).then(open_voice),
        // The right pane's surfaces.
        Shot::new("panels", "terminal", Chat(Some(COMPOSER))).then(open_terminal),
        Shot::new("panels", "files", Chat(Some(COMPOSER))).then(open_files),
        Shot::new("panels", "changes", Chat(Some(COMPOSER))).then(open_changes),
    ];
    // Every settings page this build shows.
    for section in SettingsSection::ALL {
        if section.visible_in_nav() {
            shots.push(Shot::new(
                "settings",
                section.slug(),
                Start::Settings(section),
            ));
        }
    }
    shots
}

const COMPOSER: &str = "chat-composer";
const WORKING: &str = "chat-focus";

fn open_account_menu(_: &mut Shell, _: &mut Window, cx: &mut Context<Shell>) {
    if let Some(account) = AppAccount::global(cx) {
        account.update(cx, |account, cx| account.fixture_open_menu(cx));
    }
}

fn open_user_menu(shell: &mut Shell, _: &mut Window, cx: &mut Context<Shell>) {
    shell.user_menu.open(());
    cx.notify();
}

fn rename_chat(shell: &mut Shell, _: &mut Window, cx: &mut Context<Shell>) {
    shell.open_rename_chat(COMPOSER.into(), cx);
}

fn delete_chat(shell: &mut Shell, _: &mut Window, cx: &mut Context<Shell>) {
    shell.delete_confirm = Some(COMPOSER.into());
    cx.notify();
}

fn new_session(shell: &mut Shell, _: &mut Window, cx: &mut Context<Shell>) {
    shell.open_new_session(None, cx);
}

fn open_runs_on(shell: &mut Shell, _: &mut Window, cx: &mut Context<Shell>) {
    shell
        .composer
        .update(cx, |composer, cx| composer.fixture_open_runs_on(cx));
}

fn open_model_menu(shell: &mut Shell, window: &mut Window, cx: &mut Context<Shell>) {
    shell
        .composer
        .update(cx, |composer, cx| composer.open_model_menu(window, cx));
}

fn open_palette(shell: &mut Shell, window: &mut Window, cx: &mut Context<Shell>) {
    shell.toggle_command_palette(window, cx);
}

fn open_voice(shell: &mut Shell, _: &mut Window, cx: &mut Context<Shell>) {
    shell.voice.update(cx, |voice, _| {
        voice.phase = openagents_proto::voice::VoicePhase::Active
    });
    shell.set_voice_stage_open(true, cx);
}

fn open_terminal(shell: &mut Shell, _: &mut Window, cx: &mut Context<Shell>) {
    shell.add_terminal_surface(cx);
}

fn open_files(shell: &mut Shell, window: &mut Window, cx: &mut Context<Shell>) {
    shell.add_files_surface(window, cx);
}

fn open_changes(shell: &mut Shell, window: &mut Window, cx: &mut Context<Shell>) {
    shell.add_diff_surface(window, cx);
}

/// What the tour needs from its caller: a fresh headless app per shot, the
/// tokio runtime the account client runs on, and the two fixture accounts.
pub struct Tour<'a> {
    pub new_app: &'a dyn Fn() -> HeadlessAppContext,
    pub tokio: tokio::runtime::Handle,
    pub signed_in: Arc<Controller>,
    pub signed_out: Arc<Controller>,
    /// Settings folders, one per shot. Never the person's own `~/.openagents`.
    pub scratch: PathBuf,
}

/// What a tour wrote and what it couldn't draw.
#[derive(Debug, Default)]
pub struct Report {
    pub written: Vec<PathBuf>,
    pub failed: Vec<(String, String)>,
}

impl Tour<'_> {
    /// Draws each shot whose name `only` keeps, in each mode of `dark`, into
    /// `out`. A shot that fails or panics is reported and the tour goes on.
    pub fn run(&self, out: &Path, only: &dyn Fn(&Shot) -> bool, modes: &[bool]) -> Report {
        let mut report = Report::default();
        for shot in shots().into_iter().filter(|shot| only(shot)) {
            for &dark in modes {
                let file = shot.file(dark);
                let path = out.join(&file);
                let drawn = catch_unwind(AssertUnwindSafe(|| self.capture(&shot, dark, &path)));
                match drawn {
                    Ok(Ok(())) => report.written.push(path),
                    Ok(Err(error)) => report.failed.push((file, format!("{error:#}"))),
                    Err(panic) => {
                        let why = panic
                            .downcast_ref::<String>()
                            .cloned()
                            .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                            .unwrap_or_else(|| "it panicked".into());
                        report.failed.push((file, why));
                    }
                }
            }
        }
        report
    }

    fn capture(&self, shot: &Shot, dark: bool, path: &Path) -> anyhow::Result<()> {
        let data = self.scratch.join(format!(
            "{}{}",
            shot.name(),
            if dark { "-dark" } else { "" }
        ));
        let _ = std::fs::remove_dir_all(&data);
        std::fs::create_dir_all(&data)?;
        let mut cx = (self.new_app)();
        cx.allow_parking();
        let mut settings = UiSettings::default();
        // A headless window has no desktop blur behind it; draw the opaque
        // surface so the picture shows what the glass would sit on.
        settings.surface = openagents_theme::SurfacePreference::Opaque;
        settings.appearance = if dark {
            crate::appearance::AppearanceMode::Dark
        } else {
            crate::appearance::AppearanceMode::Light
        };
        if shot.one_list {
            settings.sidebar_organization = SidebarOrganization::InOneList;
        }
        let controller = match shot.account {
            Account::SignedIn => self.signed_in.clone(),
            Account::SignedOut => self.signed_out.clone(),
        };
        cx.update(|cx| {
            gpui_tokio::init_from_handle(cx, self.tokio.clone());
            gpui_base::init(cx);
            settings::init(settings.clone(), data.clone(), cx);
            let fonts = crate::typography::register_fonts(cx);
            crate::typography::init(
                settings.ui_font_family.clone(),
                settings.ui_font_size.clone(),
                settings.terminal_font_family.clone(),
                settings.terminal_font_size.clone(),
                settings.code_font_family.clone(),
                settings.code_font_size.clone(),
                fonts,
                cx,
            );
            crate::theme_library::init(data.clone(), cx);
            install_theme(dark, &settings, cx);
            crate::history::init(
                settings.git_history_columns.clone(),
                settings.git_history_column_widths.clone(),
                settings.git_history_column_order.clone(),
                settings.git_history_author_display.clone(),
                cx,
            );
            crate::composer::init(cx, settings.composer_send_behavior.clone());
            crate::terminal::panel::init(cx);
            AppAccount::install_fixture(controller, cx);
        });
        let state = cx.update(|cx| cx.new(|_| fixture_state()));
        let boot = EngineBootConfig {
            data_dir: data.clone(),
            ipc_port: 0,
            edge_url: openagents_brand::DISABLED_EDGE_URL.into(),
            edge_token: None,
            org_id: None,
            workos_client_id: None,
            default_harness: HarnessId::ClaudeCode,
        };
        let start = shot.start;
        let window: WindowHandle<Shell> =
            cx.open_window(size(px(WINDOW.0), px(WINDOW.1)), |_, cx| {
                cx.new(|cx| {
                    let mut shell = Shell::new(state.clone(), boot, cx);
                    shell.reduced_motion = true;
                    shell.splash = SplashPhase::Gone;
                    shell.debug_gate = Some(match start {
                        Start::Gate(Gate::SignIn) => GatePhase::SignIn,
                        Start::Gate(Gate::Organization) => GatePhase::OrgGate,
                        Start::Gate(Gate::Failed) => GatePhase::Failed(
                            "We couldn't start OpenAgents on this computer.".into(),
                        ),
                        _ => GatePhase::Ready,
                    });
                    shell
                })
            })?;
        let any: AnyWindowHandle = window.into();
        settle(&mut cx, any)?;
        let step = shot.step;
        update_shell(&mut cx, window, move |shell, _, cx| {
            match start {
                Start::Gate(_) => {}
                Start::Home => shell.set_route(Route::Home, cx),
                Start::Chat(chat) => {
                    shell.set_route(Route::Chat, cx);
                    select(shell, chat, cx);
                }
                Start::Settings(section) => shell.open_settings(section, cx),
            }
            cx.notify();
        })?;
        settle(&mut cx, any)?;
        if let Some(step) = step {
            update_shell(&mut cx, window, step)?;
            settle(&mut cx, any)?;
        }
        let image = cx.capture_screenshot(any)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        image.save(path)?;
        cx.update_window(any, |_, window, _| window.remove_window())?;
        cx.run_until_parked();
        let _ = std::fs::remove_dir_all(&data);
        Ok(())
    }
}

fn update_shell(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<Shell>,
    f: impl FnOnce(&mut Shell, &mut Window, &mut Context<Shell>),
) -> anyhow::Result<()> {
    cx.update_window(window.into(), |view, window, cx| {
        let shell = view
            .downcast::<Shell>()
            .map_err(|_| anyhow::anyhow!("the window doesn't hold the shell"))?;
        shell.update(cx, |shell, cx| f(shell, window, cx));
        anyhow::Ok(())
    })?
}

/// Draws a few frames, letting timers, animations, and loopback replies
/// land between them.
fn settle(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    for _ in 0..4 {
        cx.update_window(window, |_, window, cx| window.draw(cx).clear())?;
        cx.run_until_parked();
        cx.advance_clock(Duration::from_millis(500));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(40));
    }
    cx.update_window(window, |_, window, cx| window.draw(cx).clear())?;
    cx.run_until_parked();
    Ok(())
}

/// The light or dark palette, without asking the operating system.
fn install_theme(dark: bool, settings: &UiSettings, cx: &mut App) {
    use crate::appearance::{AppearanceMode, AppearanceState};
    use crate::theme::Appearance;
    let (mode, appearance, model) = if dark {
        (
            AppearanceMode::Dark,
            Appearance::Dark,
            openagents_theme::Appearance::Dark,
        )
    } else {
        (
            AppearanceMode::Light,
            Appearance::Light,
            openagents_theme::Appearance::Light,
        )
    };
    cx.set_global(AppearanceState {
        mode,
        system: appearance,
        themes: settings.theme_selection.clone(),
        accent: settings.accent.clone(),
        surface: settings.surface.clone(),
    });
    let variant = settings.theme_selection.variant_id(model).to_owned();
    Theme::install_selection(appearance, &variant, settings.accent, settings.surface, cx);
}

/// Selects a fixture chat and loads its messages, or opens the new-chat
/// canvas.
fn select(shell: &mut Shell, chat: Option<&'static str>, cx: &mut Context<Shell>) {
    let entries = chat.map(transcript).unwrap_or_default();
    shell.active_chat = chat.unwrap_or_default().into();
    shell.state.update(cx, |state, cx| {
        state.select_chat(chat.map(Into::into), cx);
        if chat.is_some() {
            let _ = state.receive_transcript_update(
                openagents_doc::TranscriptUpdate {
                    frame: openagents_doc::TranscriptFrame::reset(&entries),
                    replay_baseline: None,
                    context_usage: None,
                },
                cx,
            );
        }
    });
}

/// A signed-in workspace on this Mac: two projects, a pinned chat, a working
/// chat, and an archived one. Every name is made up.
fn fixture_state() -> AppState {
    let now = Utc::now();
    let ago = |minutes: i64| now - chrono::Duration::minutes(minutes);
    let transfer = openagents_proto::capabilities::CHAT_HOME_TRANSFER_V1;
    let config = json!({"harness": "claude-code", "model": "claude-opus-5-5",
        "reasoning": null, "sandbox": "workspace-write"});
    let mut state = AppState::new();
    state.connection = ConnectionStatus::Ready;
    state.workspace_scope = Some(WorkspaceScope::Local);
    state.local_device_id = Some("local".into());
    state.devices = serde_json::from_value(json!([
        {"id": "local", "name": "Alex's MacBook Pro", "platform": "macos",
         "lastSeenAt": null, "capabilities": [transfer]},
        {"id": "build", "name": "Build server", "platform": "linux",
         "lastSeenAt": now, "capabilities": [transfer]}
    ]))
    .unwrap_or_default();
    state.spaces = serde_json::from_value(json!([
        {"id": "fieldnotes", "deviceId": "local", "path": "/Users/alex/code/fieldnotes",
         "gitDetected": true, "createdAt": ago(60 * 24 * 9)},
        {"id": "launch", "deviceId": "local", "path": "/Users/alex/code/launch-site",
         "gitDetected": true, "createdAt": ago(60 * 24 * 3)}
    ]))
    .unwrap_or_default();
    state.chats = serde_json::from_value(json!([
        {"id": COMPOSER, "deviceId": "local", "spaceId": "fieldnotes",
         "title": "Polish the composer footer", "branch": "composer-footer",
         "archived": false, "createdAt": ago(35), "config": config},
        {"id": WORKING, "deviceId": "local", "spaceId": "fieldnotes",
         "title": "Fix composer focus after send", "branch": "main",
         "archived": false, "createdAt": ago(12), "config": config},
        {"id": "chat-film", "deviceId": "local", "spaceId": "launch",
         "title": "Plan the launch film", "archived": false, "createdAt": ago(180),
         "config": config},
        {"id": "chat-notes", "deviceId": "local", "spaceId": "launch",
         "title": "Release notes for 1.0", "archived": false, "createdAt": ago(60 * 26),
         "config": config},
        {"id": "chat-tests", "deviceId": "local", "spaceId": "fieldnotes",
         "title": "Why is the sync test flaky?", "archived": false,
         "createdAt": ago(60 * 50), "config": config},
        {"id": "chat-pricing", "deviceId": "local", "title": "Old pricing notes",
         "archived": true, "createdAt": ago(60 * 24 * 12), "config": config}
    ]))
    .unwrap_or_default();
    state.sessions = serde_json::from_value(json!([
        {"chatId": WORKING, "deviceId": "local", "status": "working",
         "startedAt": now - chrono::Duration::seconds(95), "updatedAt": now}
    ]))
    .unwrap_or_default();
    state.sidebar_preferences = serde_json::from_value(json!({
        "revision": 1, "synced": true, "initialized": true,
        "pinnedSessionIds": ["chat-film"], "sections": []
    }))
    .unwrap_or_default();
    state.selected_space = Some("fieldnotes".into());
    state.auto_selected = true;
    state.chats_synced = true;
    state.spaces_synced = true;
    state
}

/// The messages of a fixture chat.
fn transcript(chat: &str) -> Vec<SessionMessageEntry> {
    let now = Utc::now().timestamp_millis();
    let at = |minutes: i64| now - minutes * 60_000;
    let tool = |id: &str, call: serde_json::Value, output: Option<&str>| {
        json!({"kind": "tool", "id": id, "call": call, "resolved": output.is_some(),
               "output": output})
    };
    let entries = match chat {
        COMPOSER => json!([
            {"id": "u1", "role": "user", "createdAt": at(34), "deviceId": "local",
             "parts": [{"kind": "text", "id": "t", "text":
                "The composer footer wraps on narrow windows. Can you tidy it up and keep the Runs on control on one line?"}]},
            {"id": "a1", "role": "assistant", "createdAt": at(33), "deviceId": "local",
             "status": "complete", "durationMs": 74_000,
             "parts": [
                {"kind": "text", "id": "t1", "text": "I'll look at how the footer lays out first."},
                tool("s1", json!({"kind": "search", "pattern": "composer-footer",
                    "path": "crates/ui/src"}), Some("4 matches in 2 files")),
                tool("r1", json!({"kind": "readFile", "path": "crates/ui/src/composer.rs"}),
                    Some("Read 240 lines")),
                tool("e1", json!({"kind": "editFile", "path": "crates/ui/src/composer.rs",
                    "old_string": "        .flex_wrap()\n", "new_string": "        .min_w_0()\n        .overflow_hidden()\n"}),
                    Some("Edited composer.rs")),
                tool("x1", json!({"kind": "exec",
                    "command": "cargo test -p openagents-ui composer"}),
                    Some("test result: ok. 42 passed; 0 failed")),
                {"kind": "text", "id": "t2", "text":
                    "The footer keeps its controls on one line now:\n\n- **Runs on** shrinks to its icon below 520 points.\n- The model name truncates instead of wrapping.\n\nThe composer tests pass."}
             ]},
            {"id": "u2", "role": "user", "createdAt": at(5), "deviceId": "local",
             "parts": [{"kind": "text", "id": "t", "text": "Nice. Does it hold up in dark mode?"}]},
            {"id": "a2", "role": "assistant", "createdAt": at(4), "deviceId": "local",
             "status": "complete", "durationMs": 21_000,
             "parts": [{"kind": "text", "id": "t", "text":
                "Yes. The footer uses the theme's surface and text colors, so dark mode follows without changes."}]}
        ]),
        WORKING => json!([
            {"id": "u1", "role": "user", "createdAt": at(2), "deviceId": "local",
             "parts": [{"kind": "text", "id": "t", "text":
                "After I send a message the composer loses focus. Please fix it and run the tests."}]},
            {"id": "a1", "role": "assistant", "createdAt": at(1), "deviceId": "local",
             "status": "streaming",
             "parts": [
                {"kind": "text", "id": "t1", "text": "Found it: the send handler blurs the input. Running the tests now."},
                tool("x1", json!({"kind": "exec", "command": "cargo test -p openagents-ui"}), None)
             ]}
        ]),
        _ => json!([]),
    };
    serde_json::from_value(entries).unwrap_or_default()
}
