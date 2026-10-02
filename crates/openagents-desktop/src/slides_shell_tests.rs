//! The slide viewer in the window (#10057): `open_presentation` lays the
//! viewer over the page, animates it open on the frame clock, takes the
//! keys, and animates closed before the overlay goes.

use super::*;
use openagents_desktop::fake::FakeHost;
use openagents_desktop::model::{Agent, Screen};
use openagents_desktop::slides::{Control, NODE, OPEN, Phase, RESOURCE};
use rust_native_desktop::input::{SurfaceInput, TextInput};
use rust_native_desktop::layout::Op;
use std::sync::atomic::Ordering;
use std::time::Duration;

const DECK: &str = "three-devdays-later";
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

fn key(app: &mut DesktopApp, name: &str, command: bool, now: Instant) -> bool {
    App::text_input(
        app,
        TextInput::Key {
            key: name,
            text: None,
            command,
            alt: false,
            shift: false,
        },
        now,
    )
}

/// The viewer's surface in a capture of the window, if it shows.
fn overlay(app: &mut DesktopApp) -> Option<rust_native_desktop::layout::Rect> {
    let (_, scene) = rust_native_desktop::capture(app, WIDTH, HEIGHT, 1.0);
    scene.ops.iter().find_map(|op| match op {
        Op::Surface { resource, rect, .. } if resource == RESOURCE => Some(*rect),
        _ => None,
    })
}

fn opacity(app: &DesktopApp) -> f32 {
    app.presentation().expect("the viewer shows").opacity()
}

#[test]
fn open_presentation_covers_the_window_and_animates_open_over_frames() {
    let (mut app, start) = shell();
    assert!(app.open_presentation("no-such-deck", start).is_err());
    assert!(app.presentation().is_none());
    app.open_presentation(DECK, start).expect("the deck opens");
    let rect = overlay(&mut app).expect("the viewer is the overlay");
    assert_eq!((rect.x, rect.y, rect.w, rect.h), (0.0, 0.0, WIDTH, HEIGHT));
    assert_eq!(app.modal_root(), Some(NODE));
    assert!(app.key_bindings().is_empty(), "the page's shortcuts wait");
    assert_eq!(opacity(&app), 0.0);
    let mut last = 0.0;
    for frame in 1..=14u64 {
        let now = start + Duration::from_millis(frame * 16);
        let wake = app.tick(now).expect("a wake");
        assert!(
            wake <= now + Duration::from_millis(16),
            "frame {frame} asks for the next"
        );
        let presentation = app.presentation().expect("the viewer shows");
        assert!(presentation.opacity() > last && presentation.opacity() < 1.0);
        assert!(presentation.scale() > 0.96 && presentation.scale() < 1.0);
        last = presentation.opacity();
    }
    app.tick(start + OPEN);
    let presentation = app.presentation().expect("the viewer shows");
    assert_eq!(presentation.phase(), Phase::Open);
    assert_eq!((presentation.opacity(), presentation.scale()), (1.0, 1.0));
}

#[test]
fn keys_navigate_and_the_counter_follows() {
    let (mut app, start) = shell();
    app.open_presentation(DECK, start).expect("the deck opens");
    let now = start + OPEN;
    app.tick(now);
    let total = app.presentation().unwrap().viewer().deck().len();
    assert!(key(&mut app, "ArrowRight", false, now));
    assert!(key(&mut app, "Space", false, now));
    assert_eq!(
        app.presentation().unwrap().counter(),
        format!("3 / {total}")
    );
    assert!(key(&mut app, "ArrowLeft", false, now));
    assert_eq!(
        app.presentation().unwrap().counter(),
        format!("2 / {total}")
    );
    assert!(
        key(&mut app, "x", false, now),
        "no plain key reaches the page"
    );
    assert!(!key(&mut app, "q", true, now), "quit stays the window's");
}

#[test]
fn f_toggles_full_screen_and_escape_steps_back_then_closes_the_overlay() {
    let (mut app, start) = shell();
    app.open_presentation(DECK, start).expect("the deck opens");
    let now = start + OPEN;
    app.tick(now);
    key(&mut app, "f", false, now);
    assert!(app.presentation().unwrap().fullscreen());
    key(&mut app, "F", false, now);
    assert!(!app.presentation().unwrap().fullscreen());
    key(&mut app, "f", false, now);
    key(&mut app, "Escape", false, now);
    let presentation = app.presentation().unwrap();
    assert!(!presentation.fullscreen() && presentation.phase() == Phase::Open);
    key(&mut app, "Escape", false, now);
    assert_eq!(app.presentation().unwrap().phase(), Phase::Closing);
    // Closing runs the animation backward, frame by frame.
    let mut last = 1.0;
    for frame in 1..=14u64 {
        app.tick(now + Duration::from_millis(frame * 16));
        let opacity = opacity(&app);
        assert!(opacity < last && opacity > 0.0, "frame {frame} fades");
        last = opacity;
    }
    assert!(overlay(&mut app).is_some(), "still showing while it closes");
    app.tick(now + OPEN);
    assert!(app.presentation().is_none(), "closed, the overlay goes");
    assert!(overlay(&mut app).is_none());
    assert_eq!(app.modal_root(), None);
    assert!(app.overlay_layout().is_none());
}

#[test]
fn the_close_button_animates_closed() {
    let (mut app, start) = shell();
    app.open_presentation(DECK, start).expect("the deck opens");
    let now = start + OPEN;
    app.tick(now);
    overlay(&mut app);
    let controls = app.presentation().unwrap().controls().to_vec();
    let (_, next) = controls.iter().find(|(c, _)| *c == Control::Next).unwrap();
    let down = |rect: PxRect| SurfaceInput::Down {
        x: rect.x + 4.0,
        y: rect.y + 4.0,
        shift: false,
    };
    assert!(app.surface_input(RESOURCE, down(*next), now));
    assert!(app.presentation().unwrap().counter().starts_with("2 / "));
    let (_, close) = controls.iter().find(|(c, _)| *c == Control::Close).unwrap();
    assert!(app.surface_input(RESOURCE, down(*close), now));
    assert_eq!(app.presentation().unwrap().phase(), Phase::Closing);
    app.tick(now + OPEN / 2);
    assert!(app.presentation().is_some());
    app.tick(now + OPEN);
    assert!(app.presentation().is_none());
}

#[test]
fn reduce_motion_opens_and_closes_at_once() {
    let (mut app, now) = shell();
    app.reduce_motion().store(true, Ordering::Relaxed);
    app.open_presentation(DECK, now).expect("the deck opens");
    assert_eq!(app.presentation().unwrap().phase(), Phase::Open);
    assert_eq!(opacity(&app), 1.0);
    key(&mut app, "Escape", false, now);
    app.tick(now);
    assert!(app.presentation().is_none(), "closed at once");
}

/// With `OPENAGENTS_SLIDES_CAPTURE=DIR`, writes the verification captures:
/// mid-open, the viewer, and full screen.
#[test]
fn captures_mid_open_viewer_and_full_screen() {
    let (mut app, start) = shell();
    app.open_presentation(DECK, start).expect("the deck opens");
    let directory = std::env::var_os("OPENAGENTS_SLIDES_CAPTURE").map(std::path::PathBuf::from);
    let write = |app: &mut DesktopApp, name: &str| {
        let (frame, scene) = rust_native_desktop::capture(app, WIDTH, HEIGHT, 2.0);
        assert!(scene.unsupported.is_empty(), "{:?}", scene.unsupported);
        if let Some(directory) = &directory {
            std::fs::create_dir_all(directory).unwrap();
            std::fs::write(directory.join(format!("{name}.png")), frame.png().unwrap()).unwrap();
        }
        frame
    };
    let before = write(&mut app, "slides-closed");
    app.tick(start + Duration::from_millis(60));
    let mid = write(&mut app, "slides-mid-open");
    app.tick(start + OPEN);
    let open = write(&mut app, "slides-viewer");
    assert_ne!(before.pixels, mid.pixels);
    assert_ne!(mid.pixels, open.pixels);
    key(&mut app, "f", false, start + OPEN);
    let full = write(&mut app, "slides-full-screen");
    assert_ne!(open.pixels, full.pixels);
}

fn offer(deck: &str) -> openagents_chat::router::Meta {
    openagents_chat::router::Meta {
        offers: vec![openagents_chat::router::Offer::OpenPresentation { deck: deck.into() }],
        ..Default::default()
    }
}

/// A chat reply's typed `open_presentation` offer, parsed from the
/// worker's NIP-CJ body, is dispatched to `open_presentation` with the
/// deck it names (#10058).
#[test]
fn a_typed_open_presentation_offer_opens_that_deck() {
    let (mut app, now) = shell();
    app.chat = Some(openagents_desktop::chat::Panel::new(now));
    let mut meta = openagents_chat::router::Meta::default();
    meta.offered(&serde_json::json!({
        "v": 2, "requires": [], "type": "offer", "offer": "open_presentation",
        "deck": "test-time-capabilities", "label": "Open Test-Time Capabilities"
    }));
    app.chat
        .as_mut()
        .expect("the chat panel")
        .receive_offers(Some(&meta));
    app.chat_presentation(now);
    assert_eq!(
        app.presentation().expect("the viewer shows").deck_id(),
        "test-time-capabilities"
    );
    assert!(
        app.chat.as_mut().unwrap().take_presentation().is_none(),
        "taken once"
    );
}

/// A deck `openagents_deck::decks()` doesn't list gets a plain refusal
/// that names the decks there are, and no viewer (#10058).
#[test]
fn an_unknown_deck_gets_a_plain_refusal() {
    let (mut app, now) = shell();
    app.chat = Some(openagents_desktop::chat::Panel::new(now));
    app.chat
        .as_mut()
        .unwrap()
        .receive_offers(Some(&offer("no-such-deck")));
    app.chat_presentation(now);
    assert!(app.presentation().is_none());
    let notice = app.chat.as_ref().unwrap().notice().expect("the refusal");
    assert!(notice.starts_with("We can't find that deck."), "{notice}");
    for deck in openagents_deck::decks() {
        assert!(notice.contains(&deck.title), "{notice}");
    }
}

/// Words alone never open the viewer: a reply with no offer, or another
/// offer, opens nothing, and a sentence naming a deck is not an offer.
#[test]
fn no_string_match_path_opens_the_viewer() {
    let (mut app, now) = shell();
    app.chat = Some(openagents_desktop::chat::Panel::new(now));
    let other = openagents_chat::router::Meta {
        offers: vec![openagents_chat::router::Offer::OpenScreen {
            screen: openagents_chat::router::Screen::Wallet,
        }],
        ..Default::default()
    };
    for meta in [
        None,
        Some(openagents_chat::router::Meta::default()),
        Some(other),
    ] {
        app.chat.as_mut().unwrap().receive_offers(meta.as_ref());
        app.chat_presentation(now);
        assert!(app.presentation().is_none());
    }
    let mut words = openagents_chat::router::Meta::default();
    words.offered(&serde_json::json!(
        "open the three-devdays-later presentation"
    ));
    words.offered(&serde_json::json!({"text": "Opening three-devdays-later."}));
    assert!(words.offers.is_empty());
    app.chat.as_mut().unwrap().receive_offers(Some(&words));
    app.chat_presentation(now);
    assert!(app.presentation().is_none());
}

/// A worker that answers every message with "Opening Three DevDays Later."
/// and the router's typed `open_presentation` offer for it.
struct Offering;

impl openagents_chat::basic_coder::Door for Offering {
    fn ask(
        &self,
        _: Vec<openagents_chat::basic_coder::Turn>,
        _: openagents_chat::router::Context,
        reply: std::sync::Arc<std::sync::Mutex<openagents_chat::basic_coder::Reply>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(30)).await;
            let mut reply = openagents_chat::basic_coder::lock(&reply);
            reply.text = "Opening Three DevDays Later.".into();
            reply.meta = offer(DECK);
            reply.done = true;
        })
    }
}

/// "open the three devdays later deck" typed and sent: the reply carrying
/// the typed offer arrives on the window's tick, from the background
/// worker and from the inline runner, and the viewer opens then, with no
/// further key or click (#10082).
#[test]
fn the_viewer_opens_when_the_offer_arrives_not_on_the_next_input() {
    for background in [false, true] {
        let fake = FakeHost::new("Test computer", unix_now());
        fake.answer_with(std::sync::Arc::new(Offering));
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
            TextInput::Commit("open the three devdays later deck"),
            now
        ));
        assert!(key(&mut app, "Enter", false, now));
        assert!(app.presentation().is_none(), "no reply yet");
        // From here on only the window's ticks run: no key, no click.
        while app.presentation().is_none() {
            assert!(
                Instant::now() < deadline,
                "background {background}: the offer arrived but the viewer did not open"
            );
            let now = Instant::now();
            let wake = app.tick(now);
            if let Some(viewer) = app.presentation() {
                assert!(
                    wake.is_some_and(|wake| wake <= now + Duration::from_millis(16)),
                    "the opening viewer asks for its next frame"
                );
                assert_eq!(viewer.deck_id(), DECK);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            app.chat.as_mut().unwrap().take_presentation(),
            None,
            "taken once"
        );
    }
}

/// The Episode 289 deck's second slide hosts the Map page's graph, live
/// and interactive: a click selects a node and shows its details, a drag
/// pans it, and the arrow keys still change slides unless a drag is held.
/// Its third slide plays the map growing over the years. With
/// `OPENAGENTS_SLIDES_CAPTURE` set, the captures are kept there.
#[test]
fn episode_289_hosts_the_live_route_map_and_its_future() {
    use openagents_desktop::slides::Layout;
    let (mut app, start) = shell();
    app.open_presentation("episode-289", start)
        .expect("the deck opens");
    let open = start + OPEN;
    app.tick(open);
    let directory = std::env::var_os("OPENAGENTS_SLIDES_CAPTURE").map(std::path::PathBuf::from);
    let write = |app: &mut DesktopApp, name: &str| {
        let (frame, scene) = rust_native_desktop::capture(app, WIDTH, HEIGHT, 2.0);
        assert!(scene.unsupported.is_empty(), "{:?}", scene.unsupported);
        if let Some(directory) = &directory {
            std::fs::create_dir_all(directory).unwrap();
            std::fs::write(directory.join(format!("{name}.png")), frame.png().unwrap()).unwrap();
        }
        frame
    };
    assert!(key(&mut app, "ArrowRight", false, open));
    let map = write(&mut app, "episode-289-routes");
    let slides = app.presentation().expect("the viewer shows");
    let page = slides.routes().expect("the slide holds the live map");
    assert!(page.map().nodes.len() > 20);
    assert_eq!(page.selected(), None);
    // A click on the router, where the fitted map draws it, selects it.
    let slide = Layout::of(WIDTH, HEIGHT, false, 1.0).slide;
    let (w, h) = page.size();
    let front = page.camera().to_screen(
        openagents_chat_app::route_map::layout::Point::default(),
        w,
        h,
    );
    let (cx, cy) = (slide.x + front.x, slide.y + front.y);
    let input = |app: &mut DesktopApp, event: SurfaceInput| {
        App::surface_input(app, RESOURCE, event, open);
    };
    input(
        &mut app,
        SurfaceInput::Down {
            x: cx,
            y: cy,
            shift: false,
        },
    );
    input(&mut app, SurfaceInput::Up { x: cx, y: cy });
    let page = app.presentation().unwrap().routes().unwrap();
    assert_eq!(
        page.selected().map(|i| page.map().nodes[i].id.as_str()),
        Some("front"),
        "a click selects, and the slide stays"
    );
    assert_eq!(app.presentation().unwrap().counter(), "2 / 4");
    // Tab steps to the next node; its details show on the slide.
    assert!(key(&mut app, "Tab", false, open));
    let selected = write(&mut app, "episode-289-routes-selected");
    assert_ne!(map.pixels, selected.pixels);
    // A drag pans the map; an arrow during it doesn't change the slide.
    let before = app.presentation().unwrap().routes().unwrap().camera();
    input(
        &mut app,
        SurfaceInput::Down {
            x: cx,
            y: cy,
            shift: false,
        },
    );
    input(
        &mut app,
        SurfaceInput::Move {
            x: cx + 60.0,
            y: cy + 20.0,
        },
    );
    assert!(key(&mut app, "ArrowRight", false, open));
    assert_eq!(app.presentation().unwrap().counter(), "2 / 4");
    input(
        &mut app,
        SurfaceInput::Up {
            x: cx + 60.0,
            y: cy + 20.0,
        },
    );
    assert_ne!(
        app.presentation().unwrap().routes().unwrap().camera(),
        before
    );
    // Once let go, the arrow changes slides: the future plays.
    assert!(key(&mut app, "ArrowRight", false, open));
    assert_eq!(app.presentation().unwrap().counter(), "3 / 4");
    let wake = app.tick(open).expect("the future asks for frames");
    assert!(wake <= open + Duration::from_millis(40));
    let future = |app: &DesktopApp| app.presentation().unwrap().future().unwrap().year();
    app.tick(open + Duration::from_secs(10));
    assert_eq!(future(&app), 2027);
    let early = write(&mut app, "episode-289-future-a");
    app.tick(open + Duration::from_secs(33));
    assert_eq!(future(&app), 2030);
    let late = write(&mut app, "episode-289-future-b");
    assert_ne!(early.pixels, late.pixels);
}
