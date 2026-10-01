//! The Mac's menu bar for the window (#10023): **OpenAgents**, **File**,
//! **Chat**, and **Window** (which opens the Map page, #10085), built from
//! the shared command registry
//! (`openagents_chat_app::commands::registry`), so every chat item runs
//! the same command the palette runs, by the same key.
//!
//! What the menus hold is [`menus`], plain data tested on every platform.
//! On macOS [`start`] replaces winit's default main menu and [`sync`]
//! redraws it when the registry changes and returns the registry keys of
//! the items chosen since the last tick; the window runs each as
//! `chat_action::Action::Command`. The system rows (Hide, Quit, Minimize)
//! go to the responder chain with AppKit's own selectors, as winit's
//! default menu did.
//!
//! Chat items carry no key equivalents: the window's own shortcuts
//! (`commands::shortcut`) keep ⌘N, ⌘F, ⌘, and ⌘. so a composing or
//! overlay scope still holds them back. Linux and Windows have no native
//! menu bar under winit; there the in-window menus and palette are the
//! menus.

// The drawing is macOS only; elsewhere the model is exercised by tests.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use openagents_chat_app::commands::{Action, Entry};

/// The most "Switch to" rows the Chat menu lists (the palette lists all).
pub const SWITCH_LIMIT: usize = 10;

/// An AppKit action a row sends up the responder chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System {
    Hide,
    HideOthers,
    ShowAll,
    Quit,
    Minimize,
    Zoom,
    BringAllToFront,
}

/// What choosing a row does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Run {
    /// Runs this registry command in the window.
    Command(String),
    /// Sends this AppKit action.
    System(System),
}

/// One row of a menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    Separator,
    Item {
        title: String,
        run: Run,
        /// ⌘ plus this key; empty for none.
        key: &'static str,
        enabled: bool,
    },
}

/// One menu in the bar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Menu {
    pub title: &'static str,
    pub rows: Vec<Row>,
}

fn command(entry: &Entry) -> Row {
    Row::Item {
        title: entry.label.clone(),
        run: Run::Command(entry.key.clone()),
        key: "",
        enabled: entry.enabled,
    }
}

fn system(title: &str, action: System, key: &'static str) -> Row {
    Row::Item {
        title: title.into(),
        run: Run::System(action),
        key,
        enabled: true,
    }
}

/// The menu bar for `registry`, left to right. Every registry command
/// appears once, except "Switch to" rows past [`SWITCH_LIMIT`].
pub fn menus(registry: &[Entry]) -> Vec<Menu> {
    let find = |action: Action| registry.iter().filter(move |entry| entry.action == action);
    let mut app = vec![];
    app.extend(find(Action::Settings).map(command));
    app.extend([
        Row::Separator,
        system("Hide OpenAgents", System::Hide, "h"),
        system("Hide Others", System::HideOthers, ""),
        system("Show All", System::ShowAll, ""),
        Row::Separator,
        system("Quit OpenAgents", System::Quit, "q"),
    ]);
    let file: Vec<Row> = find(Action::NewChat)
        .chain(find(Action::Search))
        .map(command)
        .collect();
    let placed = |entry: &Entry| {
        matches!(
            entry.action,
            Action::Settings | Action::NewChat | Action::Search | Action::Map | Action::Switch(_)
        )
    };
    // The Map page (#10085) opens from the Window menu, above the
    // system's window rows.
    let mut window: Vec<Row> = find(Action::Map).map(command).collect();
    if !window.is_empty() {
        window.push(Row::Separator);
    }
    window.extend([
        system("Minimize", System::Minimize, "m"),
        system("Zoom", System::Zoom, ""),
        Row::Separator,
        system("Bring All to Front", System::BringAllToFront, ""),
    ]);
    let mut chat: Vec<Row> = find(Action::Stop).map(command).collect();
    let edits: Vec<Row> = registry
        .iter()
        .filter(|entry| !placed(entry) && entry.action != Action::Stop)
        .map(command)
        .collect();
    if !edits.is_empty() {
        if !chat.is_empty() {
            chat.push(Row::Separator);
        }
        chat.extend(edits);
    }
    let switches: Vec<Row> = registry
        .iter()
        .filter(|entry| matches!(entry.action, Action::Switch(_)))
        .take(SWITCH_LIMIT)
        .map(command)
        .collect();
    if !switches.is_empty() {
        if !chat.is_empty() {
            chat.push(Row::Separator);
        }
        chat.extend(switches);
    }
    vec![
        Menu {
            title: "OpenAgents",
            rows: app,
        },
        Menu {
            title: "File",
            rows: file,
        },
        Menu {
            title: "Chat",
            rows: chat,
        },
        Menu {
            title: "Window",
            rows: window,
        },
    ]
}

/// Every choosable row's action, in menu order: an item's `NSMenuItem`
/// tag is its index here plus one.
pub fn runs(menus: &[Menu]) -> Vec<Run> {
    menus
        .iter()
        .flat_map(|menu| &menu.rows)
        .filter_map(|row| match row {
            Row::Item { run, .. } => Some(run.clone()),
            Row::Separator => None,
        })
        .collect()
}

/// The registry key a chosen tag runs, if it is a command row.
pub fn command_for_tag(menus: &[Menu], tag: isize) -> Option<String> {
    let index = usize::try_from(tag.checked_sub(1)?).ok()?;
    match runs(menus).into_iter().nth(index)? {
        Run::Command(key) => Some(key),
        Run::System(_) => None,
    }
}

#[cfg(target_os = "macos")]
pub use mac::{start, sync};

#[cfg(target_os = "macos")]
mod mac {
    use super::{Menu, Row, Run, System, command_for_tag, menus};
    use objc2::rc::Retained;
    use objc2::runtime::{NSObject, NSObjectProtocol, Sel};
    use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, sel};
    use objc2_app_kit::{NSApplication, NSMenu, NSMenuItem};
    use objc2_foundation::{MainThreadMarker, NSString};
    use openagents_chat_app::commands::Entry;
    use rust_native_desktop::Waker;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Ivars {
        on_tag: Box<dyn Fn(isize)>,
    }

    define_class!(
        // SAFETY: NSObject has no subclassing requirements, the class is
        // used only on the main thread, and it does not implement Drop.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "OpenAgentsMainMenuTarget"]
        #[ivars = Ivars]
        struct Target;

        impl Target {
            #[unsafe(method(commandAction:))]
            fn command_action(&self, sender: &NSMenuItem) {
                (self.ivars().on_tag)(sender.tag());
            }
        }

        unsafe impl NSObjectProtocol for Target {}
    );

    impl Target {
        fn new(mtm: MainThreadMarker, on_tag: Box<dyn Fn(isize)>) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(Ivars { on_tag });
            // SAFETY: NSObject's init on a freshly allocated object.
            unsafe { msg_send![super(this), init] }
        }
    }

    fn selector(action: System) -> Sel {
        match action {
            System::Hide => sel!(hide:),
            System::HideOthers => sel!(hideOtherApplications:),
            System::ShowAll => sel!(unhideAllApplications:),
            System::Quit => sel!(terminate:),
            System::Minimize => sel!(performMiniaturize:),
            System::Zoom => sel!(performZoom:),
            System::BringAllToFront => sel!(arrangeInFront:),
        }
    }

    struct Bar {
        mtm: MainThreadMarker,
        target: Retained<Target>,
        shown: Option<Vec<Menu>>,
        chosen: Rc<RefCell<Vec<isize>>>,
    }

    impl Bar {
        fn show(&mut self, next: Vec<Menu>) {
            if self.shown.as_ref() == Some(&next) {
                return;
            }
            let mtm = self.mtm;
            let bar = NSMenu::new(mtm);
            let mut tag = 0isize;
            let mut windows = None;
            for menu in &next {
                let title = NSString::from_str(menu.title);
                let submenu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &title);
                submenu.setAutoenablesItems(false);
                for row in &menu.rows {
                    let item = match row {
                        Row::Separator => NSMenuItem::separatorItem(mtm),
                        Row::Item {
                            title,
                            run,
                            key,
                            enabled,
                        } => {
                            tag += 1;
                            let action = match run {
                                Run::Command(_) => sel!(commandAction:),
                                Run::System(system) => selector(*system),
                            };
                            // SAFETY: the action is the target's method or
                            // an AppKit responder action; both take the item.
                            let item = unsafe {
                                NSMenuItem::initWithTitle_action_keyEquivalent(
                                    NSMenuItem::alloc(mtm),
                                    &NSString::from_str(title),
                                    Some(action),
                                    &NSString::from_str(key),
                                )
                            };
                            if matches!(run, Run::Command(_)) {
                                // SAFETY: the target lives as long as this
                                // module's state, which outlives the menu.
                                unsafe { item.setTarget(Some(&self.target)) };
                            }
                            item.setTag(tag);
                            item.setEnabled(*enabled);
                            item
                        }
                    };
                    submenu.addItem(&item);
                }
                let top = NSMenuItem::new(mtm);
                top.setTitle(&title);
                top.setSubmenu(Some(&submenu));
                bar.addItem(&top);
                if menu.title == "Window" {
                    windows = Some(submenu);
                }
            }
            let app = NSApplication::sharedApplication(mtm);
            app.setMainMenu(Some(&bar));
            if let Some(windows) = windows {
                app.setWindowsMenu(Some(&windows));
            }
            self.shown = Some(next);
        }
    }

    thread_local! {
        static BAR: RefCell<Option<Bar>> = const { RefCell::new(None) };
    }

    /// Replaces winit's default main menu. Call once, on the main thread,
    /// when the event loop starts; `waker` brings the next [`sync`].
    pub fn start(waker: Waker) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let chosen = Rc::new(RefCell::new(Vec::new()));
        let queue = chosen.clone();
        let target = Target::new(
            mtm,
            Box::new(move |tag| {
                queue.borrow_mut().push(tag);
                waker.wake();
            }),
        );
        let mut bar = Bar {
            mtm,
            target,
            shown: None,
            chosen,
        };
        bar.show(menus(&[]));
        BAR.with(|cell| *cell.borrow_mut() = Some(bar));
    }

    /// Redraws the menus from `registry` and returns the registry keys of
    /// the command rows chosen since the last call. Without [`start`] it
    /// does nothing, as in a capture or a test.
    pub fn sync(registry: &[Entry]) -> Vec<String> {
        BAR.with(|cell| {
            let mut cell = cell.borrow_mut();
            let Some(bar) = cell.as_mut() else {
                return Vec::new();
            };
            // Tags index the menus the person chose from, so resolve them
            // before a redraw can renumber the rows.
            let tags = std::mem::take(&mut *bar.chosen.borrow_mut());
            let keys = bar.shown.as_ref().map_or_else(Vec::new, |shown| {
                tags.into_iter()
                    .filter_map(|tag| command_for_tag(shown, tag))
                    .collect()
            });
            bar.show(menus(registry));
            keys
        })
    }
}

/// Elsewhere there is no native menu bar; the window's calls do nothing.
#[cfg(not(target_os = "macos"))]
pub fn start(_waker: rust_native_desktop::Waker) {}

#[cfg(not(target_os = "macos"))]
pub fn sync(_registry: &[Entry]) -> Vec<String> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_chat::basic_chats::Summary;

    fn summary(id: &str, title: &str, updated: u64) -> Summary {
        Summary {
            id: id.into(),
            title: title.into(),
            started: updated,
            updated,
            coder: None,
            archived: false,
            pinned: false,
            named: false,
        }
    }

    fn titles(menus: &[Menu]) -> Vec<(String, Vec<String>)> {
        menus
            .iter()
            .map(|menu| {
                (
                    menu.title.to_owned(),
                    menu.rows
                        .iter()
                        .map(|row| match row {
                            Row::Separator => "---".to_owned(),
                            Row::Item { title, enabled, .. } if !enabled => format!("({title})"),
                            Row::Item { title, .. } => title.clone(),
                        })
                        .collect(),
                )
            })
            .collect()
    }

    #[test]
    fn the_menu_bar_is_the_command_registry() {
        let chats = [
            summary("a", "Fix the login bug", 2),
            summary("b", "Docs", 1),
        ];
        let registry = openagents_chat_app::commands::registry(&chats, Some("a"), false);
        let bar = menus(&registry);
        assert_eq!(
            titles(&bar),
            [
                (
                    "OpenAgents".into(),
                    vec![
                        "Settings".into(),
                        "---".into(),
                        "Hide OpenAgents".into(),
                        "Hide Others".into(),
                        "Show All".into(),
                        "---".into(),
                        "Quit OpenAgents".into(),
                    ]
                ),
                (
                    "File".into(),
                    vec!["New chat".into(), "Search chats".into()]
                ),
                (
                    "Chat".into(),
                    vec![
                        "(Stop receiving reply)".into(),
                        "---".into(),
                        "Rename chat".into(),
                        "Pin chat".into(),
                        "Archive chat".into(),
                        "(Restore chat)".into(),
                        "---".into(),
                        "Switch to Fix the login bug".into(),
                        "Switch to Docs".into(),
                    ]
                ),
                (
                    "Window".into(),
                    vec![
                        "Open the map".into(),
                        "---".into(),
                        "Minimize".into(),
                        "Zoom".into(),
                        "---".into(),
                        "Bring All to Front".into(),
                    ]
                ),
            ]
        );
        // Every registry command is a row that runs it by its key, with the
        // registry's enabled state.
        let rows: Vec<_> = bar.iter().flat_map(|menu| &menu.rows).collect();
        for entry in &registry {
            assert!(
                rows.iter().any(|row| matches!(row,
                    Row::Item { run: Run::Command(key), enabled, title, .. }
                    if *key == entry.key && *enabled == entry.enabled && *title == entry.label)),
                "{} is missing",
                entry.key
            );
        }
        for row in rows {
            if let Row::Item { title, .. } = row {
                assert!(
                    openagents_desktop::words::banned_in(title).is_empty(),
                    "{title}"
                );
            }
        }
    }

    #[test]
    fn tags_resolve_to_registry_keys_and_system_rows_run_nothing_here() {
        let chats = [summary("a", "Fix the login bug", 2)];
        let registry = openagents_chat_app::commands::registry(&chats, Some("a"), true);
        let bar = menus(&registry);
        let runs = runs(&bar);
        // Tag 1 is Settings; its key is the registry's.
        assert_eq!(command_for_tag(&bar, 1).as_deref(), Some("settings"));
        for (index, run) in runs.iter().enumerate() {
            let tag = index as isize + 1;
            match run {
                Run::Command(key) => assert_eq!(command_for_tag(&bar, tag).as_ref(), Some(key)),
                Run::System(_) => assert_eq!(command_for_tag(&bar, tag), None),
            }
        }
        assert_eq!(command_for_tag(&bar, 0), None);
        assert_eq!(command_for_tag(&bar, runs.len() as isize + 1), None);
        // Busy: Stop is enabled.
        assert!(bar[2].rows.iter().any(|row| matches!(row,
            Row::Item { run: Run::Command(key), enabled: true, .. } if key == "stop")));
    }

    #[test]
    fn the_chat_menu_lists_at_most_ten_chats_and_nothing_without_chats() {
        let chats: Vec<_> = (0..14)
            .map(|i| summary(&format!("c{i}"), &format!("Chat {i}"), i))
            .collect();
        let registry = openagents_chat_app::commands::registry(&chats, None, false);
        let switches = menus(&registry)[2]
            .rows
            .iter()
            .filter(|row| matches!(row, Row::Item { title, .. } if title.starts_with("Switch to")))
            .count();
        assert_eq!(switches, SWITCH_LIMIT);
        let empty = menus(&openagents_chat_app::commands::registry(&[], None, false));
        assert_eq!(
            titles(&empty)[2].1,
            [
                "(Stop receiving reply)",
                "---",
                "(Rename chat)",
                "(Pin chat)",
                "(Archive chat)",
                "(Restore chat)"
            ]
        );
    }
}
