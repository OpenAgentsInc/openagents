//! The Map page in the window (#10085): opened from the sidebar's footer
//! and the command palette, built only while it shows, painted into its
//! surface beside the side panel, described to screen readers, and its
//! next steps carried out by the window after the tap.

use super::tests::chat_fixture;
use super::*;
use openagents_chat_app::route_map::GapKind;
use openagents_desktop::fake::FakeHost;
use openagents_desktop::model::{Agent, Screen};
use openagents_desktop::route_map::{Action as MapAction, RESOURCE};
use rust_native_desktop::access::Tree;
use rust_native_desktop::access::accesskit::Role;
use rust_native_desktop::layout::Op;

const WIDTH: f32 = 1200.0;
const HEIGHT: f32 = 840.0;

fn shell() -> (DesktopApp, Instant) {
    let fake = FakeHost::new("Test computer", unix_now());
    let context = Context::new(
        Box::new(fake.clone()),
        Some(fake),
        None,
        None,
        std::env::temp_dir(),
    );
    let now = Instant::now();
    let mut app = DesktopApp::inline_shell(Model::new(now, Screen::Home, Agent::Enabled), context);
    app.tick(now);
    (app, now)
}

fn open_map(app: &mut DesktopApp, now: Instant) {
    app.click(
        Intent::Navigate {
            action: chrome::Action::Map,
        },
        now,
    );
}

fn surface(app: &mut DesktopApp) -> Option<rust_native_desktop::layout::Rect> {
    let _ = rust_native_desktop::capture(app, WIDTH, HEIGHT, 1.0);
    let (_, scene) = rust_native_desktop::capture(app, WIDTH, HEIGHT, 1.0);
    scene.ops.iter().find_map(|op| match op {
        Op::Surface { resource, rect, .. } if resource == RESOURCE => Some(*rect),
        _ => None,
    })
}

/// The footer's Map button sits beside Verse and opens the page; the
/// page exists only while it shows, so leaving releases it and no other
/// page pays for it.
#[test]
fn the_footer_opens_the_map_and_leaving_releases_it() {
    let (mut app, now) = shell();
    let (_, scene) = rust_native_desktop::capture(&mut app, WIDTH, HEIGHT, 1.0);
    let footer: Vec<&str> = scene
        .bounds
        .keys()
        .filter(|k| {
            k.starts_with("sidebar-")
                && ["sidebar-map", "sidebar-verse", "sidebar-settings"].contains(&k.as_str())
        })
        .map(String::as_str)
        .collect();
    assert_eq!(
        footer.len(),
        3,
        "Map, Verse, and Settings are in the footer"
    );
    let map = scene.bounds["sidebar-map"];
    let verse = scene.bounds["sidebar-verse"];
    assert!(
        map.x < verse.x && (map.y - verse.y).abs() < 1.0,
        "Map sits beside Verse"
    );
    assert!(app.map_view().is_none());
    assert_eq!(App::surface_version(&app, RESOURCE), None);
    open_map(&mut app, now);
    assert!(app.map_view().is_some());
    let rect = surface(&mut app).expect("the map's surface is laid out");
    assert!(rect.w > 400.0 && rect.h > 400.0, "{rect:?}");
    assert!(App::surface_version(&app, RESOURCE).is_some());
    app.click(
        Intent::Navigate {
            action: chrome::Action::Settings,
        },
        now,
    );
    assert!(app.map_view().is_none(), "leaving the page drops it");
    assert!(surface(&mut app).is_none());
    assert_eq!(App::surface_version(&app, RESOURCE), None);
    // Back returns to it, built again.
    app.click(
        Intent::Navigate {
            action: chrome::Action::Back,
        },
        now,
    );
    assert!(app.map_view().is_some());
}

/// The command palette opens the map ("Open the map").
#[test]
fn the_palette_opens_the_map() {
    let registry = openagents_chat_app::commands::registry(&[], None, false);
    let entry = registry
        .iter()
        .find(|e| e.key == "map")
        .expect("a map command");
    assert_eq!(entry.label, "Open the map");
    assert_eq!(entry.action, openagents_chat_app::commands::Action::Map);
    let (mut app, now) = chat_fixture(2);
    app.activate(
        Intent::Chat {
            action: openagents_desktop::chat_action::Action::Command { key: "map".into() },
        },
        now,
    );
    assert!(
        app.map_view().is_some(),
        "the palette's command opens the page"
    );
}

/// Opening the map asks the host for this person's route counts; the
/// answer reaches the map's local counts on this computer.
#[test]
fn the_map_counts_this_persons_routes_locally() {
    let (mut app, now) = chat_fixture(2);
    open_map(&mut app, now);
    app.tick(now);
    assert!(
        app.chat.as_ref().unwrap().route_counts().is_some(),
        "the host answered the Routes command"
    );
}

/// A gap's chat step opens a new chat with its message in the composer,
/// unsent.
#[test]
fn a_chat_step_drafts_a_message_unsent() {
    let (mut app, now) = chat_fixture(2);
    open_map(&mut app, now);
    let gap = app
        .map_view()
        .unwrap()
        .gaps
        .iter()
        .position(|g| g.kind == GapKind::NoPlugin)
        .unwrap();
    app.click(
        Intent::Map {
            action: MapAction::GapStep { gap },
        },
        now,
    );
    assert!(matches!(
        app.navigation.as_ref().unwrap().page,
        Page::Chat(_)
    ));
    assert_eq!(
        app.chat.as_ref().unwrap().draft(),
        "Help me make a plugin that "
    );
    // Nothing was sent: the draft waits for the person.
    assert!(!app.chat.as_ref().unwrap().draft().is_empty());
}

/// Screen readers reach every node of the map, by name, inside the map's
/// surface, and a screen reader's click on one selects it and fills the
/// inspector, which is read like any other view.
#[test]
fn a_screen_reader_reads_the_map_and_selects_a_node() {
    let (mut app, now) = shell();
    open_map(&mut app, now);
    let _ = rust_native_desktop::capture(&mut app, WIDTH, HEIGHT, 2.0);
    let (_, scene) = rust_native_desktop::capture(&mut app, WIDTH, HEIGHT, 2.0);
    let tree = Tree::of(&app, &scene, None, 2.0, 0.0);
    let named = |role: Role, name: &str| {
        tree.update
            .nodes
            .iter()
            .find(|(_, node)| node.role() == role && node.label() == Some(name))
            .map(|(id, _)| *id)
    };
    assert!(named(Role::Button, "Router: OpenAgents").is_some());
    assert!(tree.update.nodes.iter().any(|(_, node)| {
        node.role() == Role::Button
            && node
                .label()
                .is_some_and(|l| l.starts_with("Plugin: Outline, Not packaged"))
    }));
    assert!(named(Role::Button, "Fit").is_some(), "the toolbar");
    let coder = named(Role::Button, "Coder: Coder").expect("Coder is named");
    let request = tree
        .request(&rust_native_desktop::access::accesskit::ActionRequest {
            action: rust_native_desktop::access::accesskit::Action::Click,
            target_tree: rust_native_desktop::access::accesskit::TreeId::ROOT,
            target_node: coder,
            data: None,
        })
        .expect("a click on a node on screen is admitted");
    rust_native_desktop::access::answer(&mut app, request, now);
    app.tick(now);
    let map = app.map_view().unwrap();
    let selected = app.map.as_ref().unwrap().selected().unwrap();
    assert_eq!(map.nodes[selected].id, "coder");
    let _ = rust_native_desktop::capture(&mut app, WIDTH, HEIGHT, 2.0);
    let (_, scene) = rust_native_desktop::capture(&mut app, WIDTH, HEIGHT, 2.0);
    let tree = Tree::of(&app, &scene, None, 2.0, 0.0);
    assert!(
        tree.update
            .nodes
            .iter()
            .any(
                |(_, node)| node.role() == Role::Heading && node.value() == Some("Coder")
                    || node.label() == Some("Coder")
            ),
        "the inspector's title is read"
    );
}

/// Keys reach the map on its page and not elsewhere; Reduce motion makes
/// the camera move at once.
#[test]
fn keys_reach_the_map_and_reduce_motion_moves_at_once() {
    use std::sync::atomic::Ordering;
    let (mut app, now) = shell();
    app.reduce_motion().store(true, Ordering::Relaxed);
    open_map(&mut app, now);
    let _ = surface(&mut app);
    let before = app.map.as_ref().unwrap().camera();
    let taken = App::text_input(
        &mut app,
        rust_native_desktop::input::TextInput::Key {
            key: "=",
            text: None,
            control: false,
            command: true,
            alt: false,
            shift: false,
        },
        now,
    );
    assert!(taken);
    let page = app.map.as_ref().unwrap();
    assert!(!page.animating(), "Reduce motion: no easing");
    assert!(page.camera().zoom > before.zoom);
    app.reduce_motion().store(false, Ordering::Relaxed);
}

/// A worker that answers every message with the router's typed
/// `open_screen` offer for `routes.map` (bank entry `meta.map.desktop`).
struct MapOffering;

impl openagents_chat::basic_coder::Door for MapOffering {
    fn ask(
        &self,
        _: Vec<openagents_chat::basic_coder::Turn>,
        _: openagents_chat::router::Context,
        reply: std::sync::Arc<std::sync::Mutex<openagents_chat::basic_coder::Reply>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        Box::pin(async move {
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            let mut reply = openagents_chat::basic_coder::lock(&reply);
            reply.text = "Here is how we route things.".into();
            reply.meta = openagents_chat::router::Meta {
                answer: Some("meta.map.desktop".into()),
                route: Some("meta.map".into()),
                offers: vec![openagents_chat::router::Offer::OpenScreen {
                    screen: openagents_chat::router::Screen::RoutesMap,
                }],
                ..Default::default()
            };
            reply.done = true;
        })
    }
}

/// "show me how you route things" typed and sent: the reply carrying the
/// typed `routes.map` offer arrives on the window's tick, from the inline
/// and the background runner, and the Map page opens then, with no tap on
/// **Open the map** and no other key or click. The offer is taken once:
/// leaving the page, later ticks don't open it again (#10102).
#[test]
fn the_map_opens_when_the_offer_arrives_without_a_tap() {
    use rust_native_desktop::input::TextInput;
    use std::time::Duration;
    for background in [false, true] {
        let fake = FakeHost::new("Test computer", unix_now());
        fake.answer_with(std::sync::Arc::new(MapOffering));
        let context = Context::new(
            Box::new(fake.clone()),
            Some(fake),
            None,
            None,
            std::env::temp_dir(),
        );
        let now = Instant::now();
        let mut app =
            DesktopApp::inline_chat(Model::new(now, Screen::Home, Agent::Enabled), context);
        if background {
            let Runner::Inline(context) = std::mem::replace(&mut app.runner, Runner::Pending(None))
            else {
                unreachable!("an inline chat")
            };
            app.runner = Runner::Background(Worker::start(context, Waker::new(|| {})));
        }
        app.activate(
            Intent::Navigate {
                action: chrome::Action::NewChat,
            },
            now,
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        while background && app.chat.as_ref().unwrap().selected_chat().is_none() {
            assert!(Instant::now() < deadline, "the new chat opens");
            app.tick(Instant::now());
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(App::text_input(
            &mut app,
            TextInput::Commit("show me how you route things"),
            now
        ));
        assert!(App::text_input(
            &mut app,
            TextInput::Key {
                key: "Enter",
                text: None,
                control: false,
                command: false,
                alt: false,
                shift: false,
            },
            now
        ));
        assert!(app.map_view().is_none(), "no reply yet");
        // From here on only the window's ticks run: no key, no click.
        while app.map_view().is_none() {
            assert!(
                Instant::now() < deadline,
                "background {background}: the offer arrived but the Map page did not open"
            );
            app.tick(Instant::now());
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            !app.chat.as_mut().unwrap().take_map(),
            "background {background}: taken once"
        );
        app.activate(
            Intent::Navigate {
                action: chrome::Action::Settings,
            },
            Instant::now(),
        );
        for _ in 0..5 {
            app.tick(Instant::now());
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            app.map_view().is_none(),
            "background {background}: the offer opened the page once"
        );
    }
}
