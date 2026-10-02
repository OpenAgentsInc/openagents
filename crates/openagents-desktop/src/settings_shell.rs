//! Settings in the window (#10021): each choice applies at once and is kept
//! in the settings file's `app` section, beside Coder's own settings
//! (`coder::task::settings`), so it holds across restarts. Coder's page
//! (#10070) changes the `coder` section only through that loader, which
//! validates it: a file it refuses is never written.

use super::DesktopApp;
use openagents_chat_app::preferences::{Change, Preferences, SECTION};
use openagents_desktop::chrome::Page;
use openagents_desktop::model::{Intent, Screen};
use openagents_desktop::settings::{Action, CoderAgent, CoderChoices};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

/// The preferences in `file`: the defaults when it is missing or unreadable.
pub fn load(file: &Path) -> Preferences {
    std::fs::read(file).map_or_else(
        |_| Preferences::default(),
        |bytes| Preferences::from_bytes(&bytes),
    )
}

/// Writes `preferences` to `file`'s `app` section, keeping every other
/// section as it is. A file Coder can't read is left alone.
pub fn save(file: &Path, preferences: &Preferences) -> Result<(), String> {
    let mut settings = coder::task::settings::Settings::load(file)?;
    settings.other.insert(SECTION.into(), preferences.section());
    settings.save(file)
}

/// Coder's own settings in `file`, as its page shows them: every agent,
/// the ones on first in the order Coder tries them, then the ones turned
/// off. Agents are opt-out (#10184): each is on unless turned off.
pub fn coder_choices(file: &Path) -> CoderChoices {
    use coder::task::settings::{self, Settings, Start};
    let settings = match Settings::load(file) {
        Ok(settings) => settings,
        Err(why) => return CoderChoices::Unreadable(why),
    };
    let agents = settings
        .coder
        .listing()
        .into_iter()
        .map(|(provider, on)| CoderAgent {
            key: provider.as_str().to_owned(),
            name: settings::provider_name(provider).to_owned(),
            on,
            blocked: None,
        })
        .collect();
    CoderChoices::Read {
        ask_first: settings.coder.start == Start::AskFirst,
        agents,
    }
}

/// Makes a change on Coder's page in `file` through Coder's own loader,
/// which validates it; a file it can't read, or a change it refuses, is
/// left as it was.
fn change_coder(file: &Path, action: &Action) -> Result<(), String> {
    use coder::task::settings::{Settings, Start};
    let mut settings = Settings::load(file)?;
    match action {
        Action::CoderStart { ask_first } => {
            settings.coder.start = if *ask_first {
                Start::AskFirst
            } else {
                Start::AtOnce
            };
        }
        Action::CoderAgent { agent, on } => {
            let provider = coder::task::settings::agent(agent)
                .map_err(|_| format!("`{agent}` is not an agent Coder runs"))?;
            settings.allow(provider, *on)?;
        }
        _ => return Ok(()),
    }
    settings.save(file)
}

impl DesktopApp {
    /// Reads the preferences from `file`, applies them, and keeps later
    /// changes there.
    pub fn use_settings_file(&mut self, file: PathBuf) {
        let preferences = load(&file);
        if let Some(chat) = &mut self.chat {
            chat.read_coder_start_from(file.clone());
        }
        if let Some(state) = &mut self.navigation {
            state.settings.replace(preferences);
            state.settings.coder = coder_choices(&file);
            state.settings.file = Some(file);
        }
        self.apply_text_size();
        self.present();
    }

    /// The "Reduce motion" preference the Grid on the Verse page follows.
    pub fn reduce_motion(&self) -> Arc<AtomicBool> {
        self.navigation
            .as_ref()
            .map_or_else(Arc::default, |state| state.settings.motion())
    }

    /// Sizes the chat's text as the preferences say. The theme's sizes are
    /// read with each layout ([`rust_native_desktop::App::theme`]).
    fn apply_text_size(&mut self) {
        if let (Some(state), Some(chat)) = (&self.navigation, &mut self.chat) {
            chat.set_text_size(state.settings.preferences.text_size);
        }
    }

    /// Runs a choice on Settings.
    pub(super) fn settings_action(&mut self, action: Action, now: Instant) {
        let Some(state) = &mut self.navigation else {
            return;
        };
        if let Some(change) = action.change()
            && state.settings.apply(change)
        {
            state.settings.notice = state.settings.file.as_ref().and_then(|file| {
                save(file, &state.settings.preferences).err().map(|_| {
                    "Couldn't save this setting. It applies until OpenAgents quits.".into()
                })
            });
            if matches!(change, Change::TextSize(_)) {
                self.apply_text_size();
            }
        }
        match action {
            Action::CoderStart { .. } | Action::CoderAgent { .. } => {
                if let Some(state) = &mut self.navigation
                    && let Some(file) = state.settings.file.clone()
                {
                    // A coding reply reads `coder.start` from the file each
                    // time, and the Coder lane reads it again at each
                    // start, so a saved change applies to the next one.
                    state.settings.notice = change_coder(&file, &action)
                        .err()
                        .map(|why| format!("Couldn't change Coder's settings: {why}"));
                    state.settings.coder = coder_choices(&file);
                }
            }
            Action::Pane { pane } => {
                let Some(state) = &mut self.navigation else {
                    return;
                };
                state.page = Page::Settings;
                state.settings.pane = pane;
                // Coder's settings as the file holds them now.
                if pane == openagents_desktop::settings::Pane::Coder
                    && let Some(file) = state.settings.file.clone()
                {
                    state.settings.coder = coder_choices(&file);
                }
                // Leaving Phones and computers cancels a shown code.
                if !state.shows_computers() && self.model.screen == Screen::Connect {
                    let requests = self.model.activate(Intent::Back, now);
                    self.send(requests, now);
                    let requests = self.model.tick(now);
                    self.send(requests, now);
                }
            }
            Action::Restore { chat } => {
                if let Some(request) = self.chat.as_mut().and_then(|panel| panel.restore(&chat)) {
                    if let Some(state) = &mut self.navigation {
                        state.settings.restoring.insert(chat);
                    }
                    self.send(vec![request], now);
                }
            }
            Action::TextSize { .. }
            | Action::ReduceMotion { .. }
            | Action::Notifications { .. } => {}
        }
        self.present();
    }

    /// Whether Coder's notifications are wanted.
    pub(super) fn notifications_on(&self) -> bool {
        self.navigation
            .as_ref()
            .is_none_or(|state| state.settings.preferences.notifications)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::chat_fixture;
    use super::*;
    use crate::worker::Context;
    use openagents_chat_app::preferences::TextSize;
    use openagents_desktop::chat_action::Action as ChatAction;
    use openagents_desktop::chrome::Action as Navigate;
    use openagents_desktop::fake::FakeHost;
    use openagents_desktop::model::{Agent, Model};
    use openagents_desktop::settings::Pane;
    use rust_native::{Element, Node};
    use rust_native_desktop::App;
    use std::sync::atomic::Ordering;

    fn find<'a, I>(node: &'a Node<I>, key: &str) -> Option<&'a Node<I>> {
        if node.key == key {
            return Some(node);
        }
        match &node.element {
            Element::Stack { children, .. } => children.iter().find_map(|child| find(child, key)),
            _ => None,
        }
    }

    fn shows(app: &DesktopApp, key: &str) -> bool {
        find(&app.view().view().root, key).is_some()
    }

    fn setting(app: &mut DesktopApp, action: Action, now: Instant) {
        app.click(Intent::Settings { action }, now);
    }

    /// Writes a capture of the window when `OPENAGENTS_SETTINGS_CAPTURE_DIR`
    /// names a folder.
    fn capture(app: &mut DesktopApp, name: &str) {
        app.viewport(1200.0, 840.0, 1.0);
        let (frame, scene) = rust_native_desktop::capture(app, 1200.0, 840.0, 1.0);
        assert!(scene.unsupported.is_empty(), "{:?}", scene.unsupported);
        if let Some(path) = std::env::var_os("OPENAGENTS_SETTINGS_CAPTURE_DIR") {
            let path = PathBuf::from(path);
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join(format!("{name}.png")), frame.png().unwrap()).unwrap();
        }
    }

    /// The transcript's drawn height, shown in the chat at this text size.
    fn transcript_height(app: &mut DesktopApp) -> f32 {
        let page = app.navigation.as_ref().unwrap().page;
        app.navigation.as_mut().unwrap().page = Page::Chat(0);
        app.present();
        let _ = rust_native_desktop::capture(app, 1200.0, 840.0, 1.0);
        let height = app.chat.as_ref().unwrap().transcript.height();
        app.navigation.as_mut().unwrap().page = page;
        app.present();
        height
    }

    #[test]
    fn each_setting_persists_across_a_reopen_and_applies_at_once() {
        let home = tempfile::tempdir().unwrap();
        let file = home.path().join("settings.json");
        // Coder's own settings are kept as they are.
        std::fs::write(
            &file,
            r#"{"schema":"openagents.settings.v1","coder":{"start":"ask_first"}}"#,
        )
        .unwrap();
        let (mut app, now) = chat_fixture(12);
        app.use_settings_file(file.clone());
        assert_eq!(app.theme().body, 14.0);
        let default_height = transcript_height(&mut app);
        let motion = app.reduce_motion();
        app.click(
            Intent::Navigate {
                action: Navigate::Settings,
            },
            now,
        );
        setting(
            &mut app,
            Action::Pane {
                pane: Pane::TextSize,
            },
            now,
        );
        setting(
            &mut app,
            Action::TextSize {
                size: TextSize::Largest,
            },
            now,
        );
        setting(&mut app, Action::ReduceMotion { on: true }, now);
        setting(&mut app, Action::Notifications { on: false }, now);
        // Applied at once: the theme, the chat's text, the backdrop's
        // flag, and notifications.
        assert_eq!(app.theme().body, 18.0);
        assert_eq!(app.theme().status, 16.0);
        assert!(motion.load(Ordering::Relaxed));
        assert!(!app.notifications_on());
        let larger = transcript_height(&mut app);
        assert!(larger > default_height * 1.15, "{larger} {default_height}");
        let Some(Node {
            element: Element::Button { label, .. },
            ..
        }) = find(&app.view().view().root, "settings-text-largest")
        else {
            panic!("no choice")
        };
        assert_eq!(label, "Largest");
        let kept = coder::task::settings::Settings::load(&file).expect("still valid");
        assert_eq!(kept.coder.start, coder::task::settings::Start::AskFirst);

        // Reopened: the same choices, applied before anything is drawn.
        let (mut again, _) = chat_fixture(12);
        again.use_settings_file(file.clone());
        let state = again.navigation.as_ref().unwrap();
        assert_eq!(state.settings.preferences.text_size, TextSize::Largest);
        assert!(state.settings.preferences.reduce_motion);
        assert!(!state.settings.preferences.notifications);
        assert!(again.reduce_motion().load(Ordering::Relaxed));
        assert!(!again.notifications_on());
        assert_eq!(again.theme().body, 18.0);
        assert!(transcript_height(&mut again) > default_height * 1.15);

        // Back to the defaults, and they hold too.
        for action in [
            Action::TextSize {
                size: TextSize::Default,
            },
            Action::ReduceMotion { on: false },
            Action::Notifications { on: true },
        ] {
            setting(&mut again, action, now);
        }
        assert!((transcript_height(&mut again) - default_height).abs() < 0.5);
        let (mut third, _) = chat_fixture(0);
        third.use_settings_file(file);
        assert_eq!(
            third.navigation.as_ref().unwrap().settings.preferences,
            Preferences::default()
        );
    }

    #[test]
    fn a_setting_that_cannot_be_saved_still_applies_and_says_so() {
        let home = tempfile::tempdir().unwrap();
        let file = home.path().join("settings.json");
        std::fs::write(&file, "not the settings").unwrap();
        let (mut app, now) = chat_fixture(0);
        app.use_settings_file(file.clone());
        app.click(
            Intent::Navigate {
                action: Navigate::Settings,
            },
            now,
        );
        setting(&mut app, Action::Notifications { on: false }, now);
        assert!(!app.notifications_on());
        assert!(shows(&app, "settings-notice"));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "not the settings");
    }

    /// Coder's page (#10070) shows and changes `coder.start` and which
    /// agents may run, through Coder's own loader: each change is kept in
    /// the file's `coder` section beside `app`, holds across a reopen, and
    /// a change the loader refuses, or a file it can't read, writes nothing.
    #[test]
    fn coder_settings_persist_through_coders_loader_and_show_again() {
        use coder::task::capacity::Provider;
        use coder::task::settings::{Settings, Start};
        use openagents_desktop::settings::CoderChoices;
        let home = tempfile::tempdir().unwrap();
        let file = home.path().join("settings.json");
        std::fs::write(
            &file,
            r#"{"schema":"openagents.settings.v1","app":{"text_size":"larger"}}"#,
        )
        .unwrap();
        let (mut app, now) = chat_fixture(0);
        app.use_settings_file(file.clone());
        app.click(
            Intent::Navigate {
                action: Navigate::Settings,
            },
            now,
        );
        setting(&mut app, Action::Pane { pane: Pane::Coder }, now);
        capture(&mut app, "settings-coder");
        let Some(Node {
            element: Element::Button { label, .. },
            ..
        }) = find(&app.view().view().root, "settings-coder-at-once")
        else {
            panic!("no start choice")
        };
        assert_eq!(label, "Start at once");
        assert!(shows(&app, "settings-coder-agent-codex"));
        // Opt-out (#10184): every agent is on with nothing set, Devin too.
        assert!(!shows(&app, "settings-coder-agent-devin-line"));

        setting(&mut app, Action::CoderStart { ask_first: true }, now);
        for (agent, on) in [("grok", true), ("claude", false)] {
            setting(
                &mut app,
                Action::CoderAgent {
                    agent: agent.into(),
                    on,
                },
                now,
            );
        }
        let kept = Settings::load(&file).unwrap();
        assert_eq!(kept.coder.start, Start::AskFirst);
        assert_eq!(kept.coder.disabled, vec![Provider::Claude]);
        assert!(kept.coder.providers.is_empty());
        assert_eq!(
            kept.coder.provider_list(),
            vec![
                Provider::Codex,
                Provider::Grok,
                Provider::Devin,
                Provider::OpenCode
            ]
        );
        assert_eq!(kept.other["app"]["text_size"], "larger");
        assert!(!shows(&app, "settings-notice"));

        // A change Coder's loader refuses (the last agent off) writes
        // nothing and says so.
        for agent in ["devin", "opencode", "codex"] {
            setting(
                &mut app,
                Action::CoderAgent {
                    agent: agent.into(),
                    on: false,
                },
                now,
            );
        }
        let before = std::fs::read(&file).unwrap();
        change_coder(
            &file,
            &Action::CoderAgent {
                agent: "grok".into(),
                on: false,
            },
        )
        .unwrap_err();
        assert_eq!(std::fs::read(&file).unwrap(), before);
        for agent in ["devin", "opencode", "codex"] {
            setting(
                &mut app,
                Action::CoderAgent {
                    agent: agent.into(),
                    on: true,
                },
                now,
            );
        }

        // Reopened: the page shows what the file holds.
        let (mut again, _) = chat_fixture(0);
        again.use_settings_file(file.clone());
        let CoderChoices::Read { ask_first, agents } =
            &again.navigation.as_ref().unwrap().settings.coder
        else {
            panic!("read")
        };
        assert!(*ask_first);
        let on: Vec<&str> = agents
            .iter()
            .filter(|agent| agent.on)
            .map(|agent| agent.key.as_str())
            .collect();
        assert_eq!(on, ["codex", "grok", "devin", "opencode"]);
        assert_eq!(agents.last().unwrap().key, "claude");

        // A file the loader refuses is shown as such and never written.
        std::fs::write(&file, "not the settings").unwrap();
        let (mut broken, _) = chat_fixture(0);
        broken.use_settings_file(file.clone());
        broken.click(
            Intent::Navigate {
                action: Navigate::Settings,
            },
            now,
        );
        setting(&mut broken, Action::Pane { pane: Pane::Coder }, now);
        assert!(shows(&broken, "settings-coder-unreadable"));
        setting(&mut broken, Action::CoderStart { ask_first: false }, now);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "not the settings");
    }

    #[test]
    fn leaving_phones_and_computers_in_settings_cancels_the_code() {
        let fake = FakeHost::new("Test computer", super::super::unix_now());
        let context = Context::new(
            Box::new(fake.clone()),
            Some(fake.clone()),
            None,
            None,
            std::env::temp_dir(),
        );
        let now = Instant::now();
        let mut app =
            DesktopApp::inline_shell(Model::new(now, Screen::Home, Agent::Enabled), context);
        app.tick(now);
        for leave in [
            Intent::Settings {
                action: Action::Pane {
                    pane: Pane::Archived,
                },
            },
            Intent::Navigate {
                action: Navigate::SelectChat { id: 3 },
            },
            Intent::Navigate {
                action: Navigate::Grid,
            },
        ] {
            app.click(
                Intent::Navigate {
                    action: Navigate::Settings,
                },
                now,
            );
            setting(
                &mut app,
                Action::Pane {
                    pane: Pane::Computers,
                },
                now,
            );
            assert!(fake.open().is_empty(), "no code until asked for");
            // The pairing screen's own buttons keep the person on Settings.
            app.click(Intent::ConnectAnother, now);
            let state = app.navigation.as_ref().unwrap();
            assert_eq!(state.page, Page::Settings);
            assert!(app.model.codes.shown().is_some());
            assert_eq!(fake.open().len(), 1);
            assert!(shows(&app, "settings-pane-computers"));
            app.click(leave.clone(), now);
            assert!(app.model.codes.shown().is_none(), "{leave:?}");
            assert!(fake.open().is_empty(), "{leave:?}");
        }
    }

    #[test]
    fn an_archived_chat_is_restored_from_settings() {
        let (mut app, now) = chat_fixture(0);
        let request = app.chat.as_mut().unwrap().new_chat();
        app.send(vec![request], now);
        app.present();
        app.click(
            Intent::Chat {
                action: ChatAction::Archive,
            },
            now,
        );
        let id = {
            let archived = &app.navigation.as_ref().unwrap().settings.archived;
            assert_eq!(archived.len(), 1);
            archived[0].id.clone()
        };
        app.click(
            Intent::Navigate {
                action: Navigate::Settings,
            },
            now,
        );
        setting(
            &mut app,
            Action::Pane {
                pane: Pane::Archived,
            },
            now,
        );
        assert!(shows(&app, "settings-archived-0-restore"));
        capture(&mut app, "settings-archived-list");
        setting(&mut app, Action::Restore { chat: id.clone() }, now);
        let state = app.navigation.as_ref().unwrap();
        assert_eq!(state.page, Page::Settings, "restoring doesn't open it");
        assert!(state.settings.archived.is_empty());
        assert!(state.settings.restoring.is_empty());
        assert!(shows(&app, "settings-archived-empty"));
        assert!(
            state
                .chats
                .iter()
                .all(|chat| chat.section != openagents_desktop::chrome::Section::Archived)
        );
        // A chat that isn't archived asks for nothing.
        setting(&mut app, Action::Restore { chat: id }, now);
        assert!(
            app.navigation
                .as_ref()
                .unwrap()
                .settings
                .restoring
                .is_empty()
        );
    }

    #[test]
    fn every_settings_page_mounts_and_paints() {
        let (mut app, now) = chat_fixture(4);
        app.click(
            Intent::Navigate {
                action: Navigate::Settings,
            },
            now,
        );
        for pane in Pane::ALL {
            setting(&mut app, Action::Pane { pane }, now);
            let name = format!("settings-{pane:?}").to_lowercase();
            capture(&mut app, &name);
            for (width, height) in [(1200.0, 840.0), (760.0, 540.0)] {
                let (_, scene) = rust_native_desktop::capture(&mut app, width, height, 2.0);
                assert!(scene.unsupported.is_empty());
                let hit = scene
                    .hits
                    .iter()
                    .find(|hit| hit.key.starts_with("settings-pane-"))
                    .expect("the pages");
                assert!(hit.rect.x + hit.rect.w <= width);
            }
        }
        setting(
            &mut app,
            Action::Pane {
                pane: Pane::TextSize,
            },
            now,
        );
        setting(
            &mut app,
            Action::TextSize {
                size: TextSize::Largest,
            },
            now,
        );
        capture(&mut app, "settings-textsize-largest");
        setting(
            &mut app,
            Action::Pane {
                pane: Pane::Appearance,
            },
            now,
        );
        setting(&mut app, Action::ReduceMotion { on: true }, now);
        capture(&mut app, "settings-appearance-reduced");
    }
}
