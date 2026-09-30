//! The window's native surroundings, headless (#10023): the menu bar's items
//! run the shared registry's commands, a notification's click opens its
//! chat, and a downloaded update shows the strip whose button restarts.
//! `OPENAGENTS_NATIVE_CAPTURE_DIR` saves the frames as PNGs.

use super::*;
use openagents_desktop::chat_action::Action as ChatAction;
use rust_native_desktop::App;

fn capture(app: &mut DesktopApp, name: &str) -> rust_native_desktop::layout::Scene {
    app.viewport(1200.0, 840.0, 1.0);
    let (frame, scene) = rust_native_desktop::capture(app, 1200.0, 840.0, 1.0);
    assert!(scene.unsupported.is_empty(), "{:?}", scene.unsupported);
    if let Some(path) = std::env::var_os("OPENAGENTS_NATIVE_CAPTURE_DIR") {
        let path = std::path::PathBuf::from(path);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join(format!("{name}.png")), frame.png().unwrap()).unwrap();
    }
    scene
}

fn run(app: &mut DesktopApp, intents: Vec<Intent>, now: Instant) {
    for intent in intents {
        app.activate(intent, now);
    }
}

fn selected(app: &DesktopApp) -> Option<String> {
    app.chat
        .as_ref()
        .and_then(|chat| chat.selected_chat())
        .map(str::to_owned)
}

/// The key of the menu-bar row titled `title`, built from the window's own
/// registry.
fn menu_key(app: &DesktopApp, title: &str) -> String {
    let registry = app.chat.as_ref().unwrap().command_registry();
    crate::appmenu::menus(&registry)
        .into_iter()
        .flat_map(|menu| menu.rows)
        .find_map(|row| match row {
            crate::appmenu::Row::Item {
                title: shown,
                run: crate::appmenu::Run::Command(key),
                ..
            } if shown == title => Some(key),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no {title} row"))
}

/// A window with two saved chats, the second selected.
fn two_chats() -> (DesktopApp, Instant, String, String) {
    use openagents_chat::basic_chats::Summary;
    use openagents_chat::service::{Command, Snapshot};
    let (mut app, now) = super::tests::chat_fixture(1);
    let first = selected(&app).unwrap();
    let panel = app.chat.as_mut().unwrap();
    let Request::Chat {
        ticket,
        command: Command::Create { chat: second },
    } = panel.new_chat()
    else {
        panic!("create")
    };
    let summary = |id: &str, title: &str, updated| Summary {
        id: id.into(),
        title: title.into(),
        started: updated,
        updated,
        coder: None,
        archived: false,
        pinned: false,
        named: true,
    };
    panel.outcome(
        ticket,
        Ok(Snapshot {
            chat: Some(second.clone()),
            chats: vec![
                summary(&second, "Docs", 2),
                summary(&first, "Fix the login bug", 1),
            ],
            list_total: 2,
            list_version: 1,
            ..Snapshot::default()
        }),
    );
    app.present();
    assert_eq!(selected(&app).as_deref(), Some(second.as_str()));
    (app, now, first, second)
}

#[test]
fn menu_bar_items_run_the_registry_commands() {
    let (mut app, now, first, _) = two_chats();

    // Chat > Switch to …: back to the first chat, even with the palette up.
    app.activate(
        Intent::Chat {
            action: ChatAction::Palette,
        },
        now,
    );
    let switch = app
        .chat
        .as_ref()
        .unwrap()
        .command_registry()
        .into_iter()
        .find(|entry| entry.key == format!("switch-{first}"))
        .unwrap();
    let key = menu_key(&app, &switch.label);
    run(&mut app, crate::native::intents(vec![key], vec![]), now);
    assert_eq!(selected(&app).as_deref(), Some(first.as_str()));
    assert!(app.chat.as_ref().unwrap().overlay_layout().is_none());

    // File > New chat: a new, selected conversation.
    let new = menu_key(&app, "New chat");
    assert_eq!(new, "new");
    let before = selected(&app);
    run(&mut app, crate::native::intents(vec![new], vec![]), now);
    assert_ne!(selected(&app), before);
    assert!(matches!(
        app.navigation.as_ref().unwrap().page,
        Page::Chat(_)
    ));

    // OpenAgents > Settings: the Settings page.
    let settings = menu_key(&app, "Settings");
    run(
        &mut app,
        crate::native::intents(vec![settings], vec![]),
        now,
    );
    assert_eq!(app.navigation.as_ref().unwrap().page, Page::Settings);
    capture(&mut app, "menu-settings");
}

#[test]
fn a_notification_click_opens_its_chat() {
    let (mut app, now, first, _) = two_chats();
    // Away on another page, with a menu open.
    app.activate(
        Intent::Navigate {
            action: chrome::Action::Grid,
        },
        now,
    );
    app.activate(
        Intent::Chat {
            action: ChatAction::Palette,
        },
        now,
    );

    // The notice the window showed for the first chat, clicked.
    let notice = openagents_desktop::notices::Notice {
        id: format!("coder-{first}"),
        title: "Fix the login bug".into(),
        body: "Coder finished".into(),
        urgent: false,
    };
    let chat = notice.chat().unwrap().to_owned();
    run(&mut app, crate::native::intents(vec![], vec![chat]), now);
    assert_eq!(selected(&app).as_deref(), Some(first.as_str()));
    assert!(matches!(
        app.navigation.as_ref().unwrap().page,
        Page::Chat(_)
    ));
    assert!(app.chat.as_ref().unwrap().overlay_layout().is_none());
    capture(&mut app, "notification-opened-chat");

    // A click for a chat that is gone changes nothing.
    run(
        &mut app,
        crate::native::intents(vec![], vec!["gone".into()]),
        now,
    );
    assert_eq!(selected(&app).as_deref(), Some(first.as_str()));
}

#[test]
fn a_downloaded_update_shows_the_strip_and_its_button_runs_the_update() {
    let (mut app, now) = super::tests::chat_fixture(4);
    let scene = capture(&mut app, "no-update");
    assert!(!scene.bounds.contains_key(crate::strip::BUTTON));

    app.update_ready = Some("1.1.0".into());
    app.present();
    let scene = capture(&mut app, "update-strip");
    let button = scene.bounds[crate::strip::BUTTON];
    assert!(button.x > 600.0 && button.y < 120.0, "{button:?}");
    let view = app.view().view();
    let intent = app
        .view()
        .activate(&rust_native::Activation {
            instance: view.instance.clone(),
            revision: view.revision,
            node: crate::strip::BUTTON.into(),
        })
        .unwrap()
        .clone();
    // Settings' update intent: the install and restart.
    assert_eq!(
        intent,
        Intent::Navigate {
            action: chrome::Action::Update
        }
    );
    app.activate(intent, now);

    // The palette takes the floating layer; the strip comes back after.
    app.activate(
        Intent::Chat {
            action: ChatAction::Palette,
        },
        now,
    );
    let scene = capture(&mut app, "update-strip-under-palette");
    assert!(!scene.bounds.contains_key(crate::strip::BUTTON));
    assert!(scene.bounds.contains_key("command-panel"));
    app.activate(
        Intent::Chat {
            action: ChatAction::DismissOverlay,
        },
        now,
    );
    assert!(
        capture(&mut app, "update-strip-back")
            .bounds
            .contains_key(crate::strip::BUTTON)
    );

    // Settings offers the update itself.
    app.activate(
        Intent::Navigate {
            action: chrome::Action::Settings,
        },
        now,
    );
    assert!(
        !capture(&mut app, "update-settings")
            .bounds
            .contains_key(crate::strip::BUTTON)
    );
}
