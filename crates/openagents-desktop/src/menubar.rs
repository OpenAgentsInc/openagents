//! The menu-bar item, screen `DSK-05`, and the updater's place in the app.
//!
//! An `NSStatusItem` whose menu shows where the Mac stands and offers the
//! app's few actions: the status line, **Open OpenAgents**, **Connect a
//! phone…**, **Pause Coder** (no new tasks start), **Quit OpenAgents**
//! (closes the window; Coder keeps running), and **Stop Coder on this Mac**
//! (unregisters the agent). When the updater has a checked build waiting,
//! the menu also offers **Restart to Update to VERSION**.
//!
//! What the menu holds is [`entries`], plain data tested on every platform.
//! [`MenuBar`] draws it with `objc2-app-kit` on macOS. The window reaches
//! this module through two calls: [`start`] when its event loop starts, and
//! [`sync`] on every tick, which redraws the menu from the model and returns
//! the intents the chosen items ask of the window. Everything else a menu
//! item does (bringing the window forward, quitting, unregistering the
//! agent, installing an update) happens here.

// The menu bar is macOS only; elsewhere its model is exercised by tests.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use openagents_desktop::model::{Intent, Model, Screen};

/// What a menu item asks the app to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MenuCommand {
    /// Show and focus the window.
    OpenWindow,
    /// Show the window on the QR code (`DSK-01`).
    ConnectPhone,
    /// Stop new Coder tasks from starting.
    PauseCoder,
    /// Let new Coder tasks start again.
    ResumeCoder,
    /// Install the waiting update and relaunch.
    InstallUpdate,
    /// Quit this process; the login agent keeps Coder running.
    Quit,
    /// Unregister the login agent, then quit.
    StopCoder,
}

impl MenuCommand {
    const ALL: [Self; 7] = [
        Self::OpenWindow,
        Self::ConnectPhone,
        Self::PauseCoder,
        Self::ResumeCoder,
        Self::InstallUpdate,
        Self::Quit,
        Self::StopCoder,
    ];

    /// The `NSMenuItem` tag that carries this command (never zero, the
    /// default tag).
    pub fn tag(self) -> isize {
        Self::ALL
            .iter()
            .position(|command| *command == self)
            .map_or(0, |index| index as isize + 1)
    }

    /// The command a tag carries.
    pub fn from_tag(tag: isize) -> Option<Self> {
        usize::try_from(tag - 1)
            .ok()
            .and_then(|index| Self::ALL.get(index).copied())
    }
}

/// Whether phones may start Coder here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Coder {
    /// No project is picked yet, so there is nothing to pause.
    #[default]
    NoProject,
    Running,
    Paused,
}

/// What the menu reflects.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuState {
    /// The host reaches its relay or has a direct address.
    pub online: bool,
    /// How many phones are connected.
    pub phones: usize,
    pub coder: Coder,
    /// The version of a checked update waiting to be installed.
    pub update_ready: Option<String>,
}

impl MenuState {
    /// The menu's view of the window's model.
    pub fn of(model: &Model) -> MenuState {
        let coder = if model.project().is_none() {
            Coder::NoProject
        } else if model.autostart() {
            Coder::Running
        } else {
            Coder::Paused
        };
        MenuState {
            online: model.host.as_ref().is_some_and(|host| host.status.online),
            phones: model.phones().len(),
            coder,
            update_ready: None,
        }
    }
}

/// One row of the menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// A line of text that cannot be chosen.
    Status(String),
    Separator,
    Item {
        title: String,
        command: MenuCommand,
        key: &'static str,
    },
}

fn item(title: &str, command: MenuCommand, key: &'static str) -> Entry {
    Entry::Item {
        title: title.to_owned(),
        command,
        key,
    }
}

/// The status line, in the Home screen's words (`DSK-03`).
pub fn status_line(state: &MenuState) -> String {
    let reach = match (state.online, state.phones) {
        (false, _) => "Offline.",
        (true, 0) => "Online.",
        (true, _) => "Online. Your phone can reach this Mac.",
    };
    if state.coder == Coder::Paused {
        format!("{reach} Coder is paused.")
    } else {
        reach.to_owned()
    }
}

/// The menu for `state`, top to bottom.
pub fn entries(state: &MenuState) -> Vec<Entry> {
    let mut rows = vec![
        Entry::Status(status_line(state)),
        Entry::Separator,
        item("Open OpenAgents", MenuCommand::OpenWindow, "o"),
        item("Connect a phone…", MenuCommand::ConnectPhone, "n"),
    ];
    match state.coder {
        Coder::NoProject => {}
        Coder::Running => {
            rows.push(Entry::Separator);
            rows.push(item("Pause Coder", MenuCommand::PauseCoder, ""));
        }
        Coder::Paused => {
            rows.push(Entry::Separator);
            rows.push(item("Resume Coder", MenuCommand::ResumeCoder, ""));
        }
    }
    if let Some(version) = &state.update_ready {
        rows.push(Entry::Separator);
        rows.push(item(
            &format!("Restart to Update to {version}"),
            MenuCommand::InstallUpdate,
            "",
        ));
    }
    rows.extend([
        Entry::Separator,
        item("Quit OpenAgents", MenuCommand::Quit, "q"),
        item("Stop Coder on this Mac", MenuCommand::StopCoder, ""),
    ]);
    rows
}

/// The intent a command asks of the window, given its model, if any. The
/// model checks it again, so a stale menu cannot do more than a click.
pub fn intent_for(command: MenuCommand, model: &Model) -> Option<Intent> {
    match command {
        MenuCommand::ConnectPhone => {
            (model.screen != Screen::Connect).then_some(Intent::ConnectAnother)
        }
        MenuCommand::PauseCoder => model.autostart().then_some(Intent::ToggleAutostart),
        MenuCommand::ResumeCoder => {
            (model.project().is_some() && !model.autostart()).then_some(Intent::ToggleAutostart)
        }
        MenuCommand::OpenWindow
        | MenuCommand::InstallUpdate
        | MenuCommand::Quit
        | MenuCommand::StopCoder => None,
    }
}

#[cfg(target_os = "macos")]
pub use mac::{install_update, start, sync, update_ready};

#[cfg(target_os = "macos")]
mod mac {
    use super::{Entry, MenuCommand, MenuState, entries, intent_for};
    use crate::mac::AGENT_PLIST;
    use objc2::rc::Retained;
    use objc2::runtime::{NSObject, NSObjectProtocol};
    use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, sel};
    use objc2_app_kit::{
        NSApplication, NSImage, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem,
        NSVariableStatusItemLength,
    };
    use objc2_foundation::{MainThreadMarker, NSString};
    use openagents_desktop::model::{Intent, Model};
    use openagents_desktop::update::{self, UpdateState, Updater};
    use rust_native_desktop::Waker;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};

    struct Ivars {
        on_command: Box<dyn Fn(MenuCommand)>,
    }

    define_class!(
        // SAFETY: NSObject has no subclassing requirements, the class is
        // used only on the main thread, and it does not implement Drop.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "OpenAgentsMenuBarTarget"]
        #[ivars = Ivars]
        struct Target;

        impl Target {
            #[unsafe(method(menuAction:))]
            fn menu_action(&self, sender: &NSMenuItem) {
                if let Some(command) = MenuCommand::from_tag(sender.tag()) {
                    (self.ivars().on_command)(command);
                }
            }
        }

        unsafe impl NSObjectProtocol for Target {}
    );

    impl Target {
        fn new(mtm: MainThreadMarker, on_command: Box<dyn Fn(MenuCommand)>) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(Ivars { on_command });
            // SAFETY: NSObject's init on a freshly allocated object.
            unsafe { msg_send![super(this), init] }
        }
    }

    /// The status item. It stays in the menu bar until dropped.
    pub struct MenuBar {
        mtm: MainThreadMarker,
        item: Retained<NSStatusItem>,
        menu: Retained<NSMenu>,
        target: Retained<Target>,
        shown: RefCell<Option<MenuState>>,
    }

    impl MenuBar {
        /// Adds the item to the menu bar. `on_command` runs on the main
        /// thread when an item is chosen.
        pub fn new(mtm: MainThreadMarker, on_command: impl Fn(MenuCommand) + 'static) -> Self {
            let item =
                NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
            if let Some(button) = item.button(mtm) {
                let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
                    &NSString::from_str("circle.hexagongrid.fill"),
                    Some(&NSString::from_str("OpenAgents")),
                );
                match image {
                    Some(image) => {
                        image.setTemplate(true);
                        button.setImage(Some(&image));
                    }
                    None => button.setTitle(&NSString::from_str("OpenAgents")),
                }
            }
            let menu = NSMenu::new(mtm);
            menu.setAutoenablesItems(false);
            item.setMenu(Some(&menu));
            let bar = MenuBar {
                mtm,
                item,
                menu,
                target: Target::new(mtm, Box::new(on_command)),
                shown: RefCell::new(None),
            };
            bar.show(&MenuState::default());
            bar
        }

        /// Redraws the menu for `state`. Unchanged state is a no-op.
        pub fn show(&self, state: &MenuState) {
            if self.shown.borrow().as_ref() == Some(state) {
                return;
            }
            self.menu.removeAllItems();
            for entry in entries(state) {
                let row = match entry {
                    Entry::Separator => NSMenuItem::separatorItem(self.mtm),
                    Entry::Status(text) => {
                        let row = self.row(&text, false, "");
                        row.setEnabled(false);
                        row
                    }
                    Entry::Item {
                        title,
                        command,
                        key,
                    } => {
                        let row = self.row(&title, true, key);
                        // SAFETY: the target lives as long as the menu bar,
                        // which owns the menu.
                        unsafe { row.setTarget(Some(&self.target)) };
                        row.setTag(command.tag());
                        row
                    }
                };
                self.menu.addItem(&row);
            }
            *self.shown.borrow_mut() = Some(state.clone());
        }

        fn row(&self, title: &str, action: bool, key: &str) -> Retained<NSMenuItem> {
            // SAFETY: `menuAction:` is the target's method, taking the item.
            unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    NSMenuItem::alloc(self.mtm),
                    &NSString::from_str(title),
                    action.then_some(sel!(menuAction:)),
                    &NSString::from_str(key),
                )
            }
        }
    }

    impl Drop for MenuBar {
        fn drop(&mut self) {
            NSStatusBar::systemStatusBar().removeStatusItem(&self.item);
        }
    }

    /// The menu bar, the commands it collected, and the updater.
    struct Tray {
        bar: MenuBar,
        commands: Rc<RefCell<Vec<MenuCommand>>>,
        updater: Option<Arc<Updater>>,
        update: Arc<Mutex<UpdateState>>,
    }

    thread_local! {
        static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) };
    }

    /// Adds the menu-bar item and, when running from an app bundle, starts
    /// the update checker. Call once, on the main thread, when the event
    /// loop starts; `waker` brings the next [`sync`].
    pub fn start(waker: Waker) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let commands = Rc::new(RefCell::new(Vec::new()));
        let queue = commands.clone();
        let wake = waker.clone();
        let bar = MenuBar::new(mtm, move |command| {
            queue.borrow_mut().push(command);
            wake.wake();
        });
        let update = Arc::new(Mutex::new(UpdateState::Idle));
        let updater = update::running_bundle()
            .and_then(|_| Updater::for_this_app().ok())
            .map(Arc::new);
        if let Some(updater) = &updater {
            let state = update.clone();
            let spawned = update::spawn_checker(updater.clone(), move |next| {
                if let Ok(mut current) = state.lock() {
                    *current = next;
                }
                waker.wake();
            });
            if let Err(error) = spawned {
                eprintln!("openagents-desktop: the update checker did not start: {error}");
            }
        }
        TRAY.with(|tray| {
            *tray.borrow_mut() = Some(Tray {
                bar,
                commands,
                updater,
                update,
            })
        });
    }

    /// Redraws the menu from `model` and runs the chosen items. Returns the
    /// intents the window should activate. Without [`start`] it does
    /// nothing, as in a capture.
    pub fn sync(model: &Model) -> Vec<Intent> {
        TRAY.with(|tray| {
            let tray = tray.borrow();
            let Some(tray) = tray.as_ref() else {
                return Vec::new();
            };
            let staged = match tray.update.lock().map(|state| state.clone()) {
                Ok(UpdateState::Ready(staged)) => Some(staged),
                _ => None,
            };
            let mut state = MenuState::of(model);
            state.update_ready = staged.as_ref().map(|staged| staged.version.to_string());
            tray.bar.show(&state);
            let commands = std::mem::take(&mut *tray.commands.borrow_mut());
            let mut intents = Vec::new();
            for command in commands {
                match command {
                    MenuCommand::OpenWindow => bring_forward(tray.bar.mtm),
                    MenuCommand::ConnectPhone => bring_forward(tray.bar.mtm),
                    MenuCommand::InstallUpdate => {
                        if let (Some(updater), Some(staged)) = (&tray.updater, &staged) {
                            install(updater, staged, &tray.update);
                        }
                    }
                    MenuCommand::Quit => std::process::exit(0),
                    MenuCommand::StopCoder => {
                        unregister_agent();
                        std::process::exit(0);
                    }
                    MenuCommand::PauseCoder | MenuCommand::ResumeCoder => {}
                }
                intents.extend(intent_for(command, model));
            }
            intents
        })
    }

    /// The version of a checked update waiting to be installed, for the
    /// window's update strip ([`crate::strip`]).
    pub fn update_ready() -> Option<String> {
        TRAY.with(|tray| {
            let tray = tray.borrow();
            match tray.as_ref()?.update.lock().ok()?.clone() {
                UpdateState::Ready(staged) => Some(staged.version.to_string()),
                _ => None,
            }
        })
    }

    /// Installs the waiting update and relaunches, as the menu's
    /// **Restart to Update** does; the update strip's button.
    pub fn install_update() {
        TRAY.with(|tray| {
            let tray = tray.borrow();
            let Some(tray) = tray.as_ref() else {
                return;
            };
            let staged = match tray.update.lock().map(|state| state.clone()) {
                Ok(UpdateState::Ready(staged)) => staged,
                _ => return,
            };
            if let Some(updater) = &tray.updater {
                install(updater, &staged, &tray.update);
            }
        });
    }

    fn install(updater: &Updater, staged: &update::Staged, state: &Mutex<UpdateState>) {
        let result = updater
            .install(staged)
            .and_then(|app| update::relaunch_after_exit(&app));
        match result {
            Ok(()) => std::process::exit(0),
            Err(error) => {
                eprintln!("openagents-desktop: the update did not install: {error}");
                if let Ok(mut current) = state.lock() {
                    *current = UpdateState::Failed(error.to_string());
                }
            }
        }
    }

    fn bring_forward(mtm: MainThreadMarker) {
        let app = NSApplication::sharedApplication(mtm);
        #[allow(deprecated)] // `activate` needs macOS 14; the app supports 13.
        app.activateIgnoringOtherApps(true);
        for window in app.windows().iter() {
            if window.isMiniaturized() {
                window.deminiaturize(None);
            }
            window.makeKeyAndOrderFront(None);
        }
    }

    /// Unregisters the login agent, which stops `coder host serve` and keeps
    /// it from starting at login.
    fn unregister_agent() {
        use objc2_service_management::SMAppService;
        // SAFETY: plain ServiceManagement calls on a name we own.
        unsafe {
            let service = SMAppService::agentServiceWithPlistName(&NSString::from_str(AGENT_PLIST));
            if let Err(error) = service.unregisterAndReturnError() {
                eprintln!(
                    "openagents-desktop: could not stop Coder: {}",
                    error.localizedDescription()
                );
            }
        }
    }
}

/// Elsewhere there is no menu-bar item yet (Linux and Windows get theirs
/// with their builds); the window's calls do nothing.
#[cfg(not(target_os = "macos"))]
pub fn start(_waker: rust_native_desktop::Waker) {}

#[cfg(not(target_os = "macos"))]
pub fn sync(_model: &Model) -> Vec<Intent> {
    Vec::new()
}

#[cfg(not(target_os = "macos"))]
pub fn update_ready() -> Option<String> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn install_update() {}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_desktop::control::{Autostart, HostControl};
    use openagents_desktop::fake::FakeHost;
    use openagents_desktop::model::{Agent, Refreshed};
    use std::time::Instant;

    fn titles(state: &MenuState) -> Vec<String> {
        entries(state)
            .into_iter()
            .map(|entry| match entry {
                Entry::Status(text) => format!("[{text}]"),
                Entry::Separator => "---".into(),
                Entry::Item { title, .. } => title,
            })
            .collect()
    }

    #[test]
    fn the_menu_follows_dsk_05() {
        let state = MenuState {
            online: true,
            phones: 1,
            coder: Coder::Running,
            update_ready: None,
        };
        assert_eq!(
            titles(&state),
            [
                "[Online. Your phone can reach this Mac.]",
                "---",
                "Open OpenAgents",
                "Connect a phone…",
                "---",
                "Pause Coder",
                "---",
                "Quit OpenAgents",
                "Stop Coder on this Mac",
            ]
        );
    }

    #[test]
    fn the_menu_shows_a_paused_coder_and_a_waiting_update() {
        let state = MenuState {
            online: false,
            phones: 1,
            coder: Coder::Paused,
            update_ready: Some("0.3.0".into()),
        };
        assert_eq!(
            titles(&state),
            [
                "[Offline. Coder is paused.]",
                "---",
                "Open OpenAgents",
                "Connect a phone…",
                "---",
                "Resume Coder",
                "---",
                "Restart to Update to 0.3.0",
                "---",
                "Quit OpenAgents",
                "Stop Coder on this Mac",
            ]
        );
    }

    #[test]
    fn without_a_project_there_is_nothing_to_pause() {
        let state = MenuState {
            online: true,
            ..MenuState::default()
        };
        let rows = titles(&state);
        assert_eq!(rows[0], "[Online.]");
        assert!(
            !rows
                .iter()
                .any(|row| row.contains("Coder") && row != "Stop Coder on this Mac")
        );
    }

    #[test]
    fn every_command_round_trips_through_its_tag() {
        for command in MenuCommand::ALL {
            assert_ne!(command.tag(), 0);
            assert_eq!(MenuCommand::from_tag(command.tag()), Some(command));
        }
        assert_eq!(MenuCommand::from_tag(0), None);
        assert_eq!(MenuCommand::from_tag(99), None);
    }

    #[test]
    fn the_menu_uses_none_of_the_banned_words() {
        let state = MenuState {
            online: true,
            phones: 2,
            coder: Coder::Paused,
            update_ready: Some("1.0.0".into()),
        };
        for title in titles(&state) {
            assert!(
                openagents_desktop::words::banned_in(&title).is_empty(),
                "{title}"
            );
        }
    }

    #[test]
    fn commands_become_the_window_s_intents() {
        let now = Instant::now();
        let mut model = Model::new(now, Screen::Home, Agent::Enabled);
        assert_eq!(
            intent_for(MenuCommand::ConnectPhone, &model),
            Some(Intent::ConnectAnother)
        );
        // No project: pausing and resuming do nothing.
        assert_eq!(MenuState::of(&model).coder, Coder::NoProject);
        assert_eq!(intent_for(MenuCommand::PauseCoder, &model), None);
        assert_eq!(intent_for(MenuCommand::ResumeCoder, &model), None);
        for command in [
            MenuCommand::OpenWindow,
            MenuCommand::Quit,
            MenuCommand::StopCoder,
        ] {
            assert_eq!(intent_for(command, &model), None);
        }

        // A project with auto-start on: Pause flips it; Resume does not.
        let mut host: Box<dyn HostControl> = Box::new(FakeHost::new("Studio Mac", 0));
        host.add_project("/Users/kai/code/website").unwrap();
        host.set_autostart(Autostart {
            enabled: true,
            projects: vec!["website".into()],
            max_running: 1,
        })
        .unwrap();
        model.host = Some(Refreshed {
            status: host.status().unwrap(),
            devices: host.devices().unwrap(),
            projects: host.projects().unwrap(),
            autostart: host.autostart().unwrap(),
            nearby: None,
            watchers: Vec::new(),
            background: None,
        });
        assert_eq!(MenuState::of(&model).coder, Coder::Running);
        assert_eq!(
            intent_for(MenuCommand::PauseCoder, &model),
            Some(Intent::ToggleAutostart)
        );
        assert_eq!(intent_for(MenuCommand::ResumeCoder, &model), None);

        // On the code screen, Connect a phone only brings the window forward.
        model.screen = Screen::Connect;
        assert_eq!(intent_for(MenuCommand::ConnectPhone, &model), None);
    }
}
