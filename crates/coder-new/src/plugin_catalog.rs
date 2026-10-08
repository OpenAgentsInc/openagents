//! Local installed-plugin snapshots for the live picker.

use std::{
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{Duration, Instant},
};

use background::{Layout, plugins::Installed};

use crate::{App, Mode, Screen};

const REFRESH_INTERVAL: Duration = Duration::from_secs(1);

/// Read the same registry as `openagents plugin installed`, off the UI thread.
/// The caller supplies the layout; constructing an app never reads the home directory.
pub struct Loader {
    layout: Layout,
    active: Option<Active>,
    last_scan: Option<Instant>,
    revision: Option<u64>,
}

struct Active {
    revision: u64,
    receiver: Receiver<Vec<Installed>>,
}

impl Loader {
    pub fn new(layout: Layout) -> Self {
        Self {
            layout,
            active: None,
            last_scan: None,
            revision: None,
        }
    }

    /// Refresh on each picker opening and once a second while it stays visible.
    pub fn sync(&mut self, app: &mut App) {
        self.sync_at(app, Instant::now());
    }

    fn sync_at(&mut self, app: &mut App, now: Instant) {
        let visible = app.mode == Mode::Live
            && matches!(app.screen, Screen::Plugins | Screen::PluginSettings);
        if !visible {
            self.last_scan = None;
            self.revision = None;
        } else if self.revision != Some(app.plugins.catalog_revision) {
            self.last_scan = None;
            self.revision = Some(app.plugins.catalog_revision);
        }
        // Retain an in-flight scan across closing or reopening, so only one worker runs.
        if let Some(active) = &self.active {
            match active.receiver.try_recv() {
                Ok(installed) => {
                    let current = visible && self.revision == Some(active.revision);
                    self.active = None;
                    if current {
                        let removed = app.plugins.replace_installed(installed);
                        if removed && app.screen == Screen::PluginSettings {
                            app.screen = Screen::Plugins;
                        }
                    }
                }
                Err(TryRecvError::Disconnected) => self.active = None,
                Err(TryRecvError::Empty) => {}
            }
        }
        if !visible
            || self.active.is_some()
            || self
                .last_scan
                .is_some_and(|last| now.saturating_duration_since(last) < REFRESH_INTERVAL)
        {
            return;
        }
        self.last_scan = Some(now);
        let layout = self.layout.clone();
        let (sender, receiver) = mpsc::channel();
        match thread::Builder::new()
            .name("coder-plugin-catalog".into())
            .spawn(move || {
                let _ = sender.send(background::plugins::installed(&layout));
            }) {
            Ok(_) => {
                self.active = Some(Active {
                    revision: app.plugins.catalog_revision,
                    receiver,
                });
            }
            Err(_) => app.notice = Some("Cannot refresh installed plugins.".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    fn package(layout: &Layout, slug: &str, version: &str) -> std::path::PathBuf {
        let dir = layout
            .extensions()
            .join(background::plugins::LOCAL_KEY)
            .join(slug)
            .join(version);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            serde_json::to_vec(&serde_json::json!({
                "v": 1, "slug": slug, "name": format!("Plugin {slug}"),
                "version": version, "summary": "Installed test plugin"
            }))
            .unwrap(),
        )
        .unwrap();
        dir
    }

    fn refresh(loader: &mut Loader, app: &mut App, now: Instant) {
        loader.sync_at(app, now);
        let deadline = Instant::now() + Duration::from_secs(4);
        while loader.active.is_some() {
            assert!(Instant::now() < deadline, "Plugin scan did not finish");
            thread::sleep(Duration::from_millis(5));
            loader.sync_at(app, now);
        }
    }

    fn command_p(app: &mut App) {
        assert!(app.handle(Event::Key(KeyEvent::new(
            KeyCode::Char('p'),
            KeyModifiers::SUPER
        ))));
    }

    #[test]
    fn command_p_and_an_open_picker_observe_installs_updates_and_enablement() {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::new(home.path(), None).unwrap();
        let mut loader = Loader::new(layout.clone());
        let mut app = App::default();
        app.set_mode(Mode::Live);
        let start = Instant::now();
        command_p(&mut app);
        refresh(&mut loader, &mut app, start);
        assert_eq!(app.plugins.definitions().count(), 8);
        assert!(!layout.extensions().exists());

        package(&layout, "z-last", "0.1.0");
        loader.sync_at(&mut app, start + REFRESH_INTERVAL / 2);
        assert!(loader.active.is_none());
        assert_eq!(app.plugins.definitions().count(), 8);
        refresh(&mut loader, &mut app, start + REFRESH_INTERVAL);
        assert_eq!(app.plugins.definitions().count(), 9);
        app.plugins.selected = 8;
        assert_eq!(app.plugins.selected_definition().name, "Plugin z-last");
        let selected = app.plugins.selected_definition().id.to_owned();

        package(&layout, "a-first", "0.1.0");
        package(&layout, "z-last", "0.2.0");
        background::plugins::set_enabled(&layout, &selected, true).unwrap();
        refresh(&mut loader, &mut app, start + REFRESH_INTERVAL * 2);
        assert_eq!(app.plugins.selected, 9);
        assert_eq!(app.plugins.selected_definition().id, selected);
        assert_eq!(app.plugins.selected_installed().unwrap().version, "0.2.0");
        assert!(app.plugins.enabled_for(&selected));
        assert_eq!(app.plugins.status_for(&selected), "Enabled");
        assert!(!app.plugins.toggle_selected());
        assert!(background::plugins::enabled(&layout).contains(&selected));

        // Opening again bypasses the interval, including after a closed picker.
        app.screen = Screen::Conversation;
        package(&layout, "b-middle", "0.1.0");
        command_p(&mut app);
        refresh(&mut loader, &mut app, start + REFRESH_INTERVAL * 2);
        assert_eq!(app.plugins.definitions().count(), 11);
        assert_eq!(app.plugins.selected_definition().id, selected);
        assert!(app.request.is_none());
    }

    #[test]
    fn removing_an_inspected_package_returns_to_the_list_and_preserves_builtin_drafts() {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::new(home.path(), None).unwrap();
        let dir = package(&layout, "hello", "0.1.0");
        let mut loader = Loader::new(layout);
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app.open_plugins();
        let start = Instant::now();
        refresh(&mut loader, &mut app, start);
        app.plugins.selected = 8;
        app.open_plugin_settings();
        refresh(&mut loader, &mut app, start);
        std::fs::remove_dir_all(dir).unwrap();
        refresh(&mut loader, &mut app, start + REFRESH_INTERVAL);
        assert!(app.screen == Screen::Plugins);
        assert!(app.plugins.selected < app.plugins.definitions().count());

        app.plugins.selected = 0;
        app.open_plugin_settings();
        app.plugins.paste("unsaved-test-key");
        let draft = app.plugins.field(true);
        refresh(&mut loader, &mut app, start + REFRESH_INTERVAL);
        refresh(&mut loader, &mut app, start + REFRESH_INTERVAL * 2);
        assert!(app.screen == Screen::PluginSettings);
        assert_eq!(app.plugins.field(true), draft);
        assert!(!app.plugins.key_configured);
    }

    #[test]
    fn demo_and_closed_pickers_discard_pending_snapshots_and_do_not_scan() {
        for demo in [false, true] {
            let home = tempfile::tempdir().unwrap();
            let layout = Layout::new(home.path(), None).unwrap();
            package(&layout, "hello", "0.1.0");
            let installed = background::plugins::installed(&layout);
            let mut loader = Loader::new(layout);
            let (sender, receiver) = mpsc::channel();
            sender.send(installed).unwrap();
            loader.active = Some(Active {
                revision: 0,
                receiver,
            });
            let mut app = App::default();
            if demo {
                app.open_plugins();
            } else {
                app.set_mode(Mode::Live);
            }
            loader.sync(&mut app);
            assert!(loader.active.is_none());
            assert!(loader.last_scan.is_none());
            assert_eq!(app.plugins.definitions().count(), 8);
        }
    }

    #[test]
    fn reopening_keeps_one_worker_and_discards_its_stale_snapshot() {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::new(home.path(), None).unwrap();
        package(&layout, "stale", "0.1.0");
        let old = background::plugins::installed(&layout);
        std::fs::remove_dir_all(layout.extensions()).unwrap();
        package(&layout, "current", "0.1.0");
        let mut loader = Loader::new(layout);
        let (sender, receiver) = mpsc::channel();
        loader.active = Some(Active {
            revision: 0,
            receiver,
        });
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app.open_plugins();
        let now = Instant::now();
        loader.sync_at(&mut app, now);
        assert_eq!(loader.active.as_ref().unwrap().revision, 0);

        sender.send(old).unwrap();
        loader.sync_at(&mut app, now);
        assert_eq!(app.plugins.definitions().count(), 8);
        assert_eq!(
            loader.active.as_ref().unwrap().revision,
            app.plugins.catalog_revision
        );
        refresh(&mut loader, &mut app, now);
        app.plugins.selected = 8;
        assert_eq!(app.plugins.selected_definition().name, "Plugin current");
    }
}
