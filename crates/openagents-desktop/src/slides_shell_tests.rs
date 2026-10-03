//! The slide viewer in the window (#10057): `open_presentation` lays the
//! viewer over the page, animates it open on the frame clock, takes the
//! keys, and animates closed before the overlay goes.

use super::*;
use openagents_desktop::fake::FakeHost;
use openagents_desktop::model::{Agent, Screen};
use openagents_desktop::route_live::FlowSource;
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
            control: false,
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

/// The Episode 289 deck's second slide hosts the Map page's graph alone,
/// full slide; its third hosts the same graph beside a scripted chat, live
/// and interactive: a click selects a node and shows its details, a drag
/// pans it, and the arrow keys still change slides unless a drag is held.
/// Its fourth slide goes on with the chat as a person makes a plugin and
/// others use it, and its fifth plays the map growing over the years. With
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
    // The second slide is the live map alone, full slide, to look around:
    // no chat, no lit route, and a click where the full-slide map draws
    // the router selects it and shows its details.
    assert!(key(&mut app, "ArrowRight", false, open));
    assert_eq!(app.presentation().unwrap().counter(), "2 / 8");
    app.tick(open);
    let _ = rust_native_desktop::capture(&mut app, WIDTH, HEIGHT, 2.0);
    {
        let slides = app.presentation().unwrap();
        assert!(slides.wants_routes() && !slides.on_chat());
        assert!(slides.chat().is_none(), "no chat on the map alone");
        let page = slides.routes().expect("the slide holds the live map");
        assert!(page.light().is_none());
        let slide = Layout::of(WIDTH, HEIGHT, false, 1.0).slide;
        let (w, h) = page.size();
        assert!(
            (w - slide.w).abs() < 1.0 && (h - slide.h).abs() < 1.0,
            "{w}x{h}"
        );
        let front = page.camera().to_screen(
            openagents_chat_app::route_map::layout::Point::default(),
            w,
            h,
        );
        let (cx, cy) = (slide.x + front.x, slide.y + front.y);
        App::surface_input(
            &mut app,
            RESOURCE,
            SurfaceInput::Down {
                x: cx,
                y: cy,
                shift: false,
            },
            open,
        );
        App::surface_input(&mut app, RESOURCE, SurfaceInput::Up { x: cx, y: cy }, open);
    }
    {
        let page = app.presentation().unwrap().routes().unwrap();
        assert_eq!(
            page.selected().map(|i| page.map().nodes[i].id.as_str()),
            Some("front")
        );
    }
    write(&mut app, "episode-289-map");
    assert!(app.presentation().unwrap().chat().is_none());
    // Esc lets the selection go; the map stays on the slide.
    assert!(key(&mut app, "Escape", false, open));
    assert_eq!(app.presentation().unwrap().counter(), "2 / 8");
    assert_eq!(
        app.presentation().unwrap().routes().unwrap().selected(),
        None
    );
    app.tick(open + Duration::from_secs(1));
    assert!(key(&mut app, "ArrowRight", false, open));
    assert_eq!(app.presentation().unwrap().counter(), "3 / 8");
    let wake = app.tick(open).expect("the chat asks for frames");
    assert!(wake <= open + Duration::from_millis(40));
    // Mid route: the first message's pulse on its way to its answer, then
    // the third's lit all the way to Codex under Coder.
    let exchange = openagents_desktop::route_chat::EXCHANGE;
    let lit = |app: &DesktopApp| {
        let page = app.presentation().unwrap().routes().unwrap();
        page.light().map(|light| {
            let target = *light.path.last().unwrap();
            (page.map().nodes[target].id.clone(), light.head, light.glow)
        })
    };
    app.tick(open + Duration::from_secs_f32(1.2));
    let first = write(&mut app, "episode-289-routes-chat-a");
    let (target, head, _) = lit(&app).expect("the first message's way is lit");
    assert_eq!(target, "answer:meta.who");
    assert!(head > 0.0 && head < 1.0, "{head}");
    app.tick(open + Duration::from_secs_f32(2.0 * exchange + 2.4));
    let third = write(&mut app, "episode-289-routes-chat-b");
    let (target, head, glow) = lit(&app).expect("the third message's way is lit");
    assert_eq!(target, "engine:codex");
    assert_eq!((head, glow), (1.0, 1.0));
    assert_ne!(first.pixels, third.pixels);
    // Between exchanges the light is out.
    app.tick(open + Duration::from_secs_f32(exchange + 0.1));
    assert!(lit(&app).is_none());
    let map = write(&mut app, "episode-289-routes");
    let slides = app.presentation().expect("the viewer shows");
    let page = slides.routes().expect("the slide holds the live map");
    assert!(page.map().nodes.len() > 20);
    assert_eq!(page.selected(), None);
    // A click on the router, where the fitted map draws it, selects it.
    // A chat plays in a column beside it, lighting each message's way.
    let (column, slide) =
        openagents_desktop::route_chat::split(Layout::of(WIDTH, HEIGHT, false, 1.0).slide);
    assert!(column.w > 0.25 * (column.w + slide.w) && column.w < 0.35 * (column.w + slide.w));
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
    assert_eq!(app.presentation().unwrap().counter(), "3 / 8");
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
    assert_eq!(app.presentation().unwrap().counter(), "3 / 8");
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
    // Once let go, the arrow changes slides: the chat goes on as a
    // person makes a plugin on the same map.
    assert!(key(&mut app, "ArrowRight", false, open));
    assert_eq!(app.presentation().unwrap().counter(), "4 / 8");
    let wake = app.tick(open).expect("the story asks for frames");
    assert!(wake <= open + Duration::from_millis(40));
    let story = |app: &DesktopApp| {
        let story = app.presentation().unwrap().plugin().unwrap();
        let light = story.light().map(|light| {
            let target = *light.path.last().unwrap();
            (story.map().nodes[target].id.clone(), light.missing)
        });
        (light, story.grown(), story.xp(), story.uses())
    };
    // No plugin serves it: the gap lights.
    app.tick(open + Duration::from_secs_f32(2.6));
    let missing = write(&mut app, "episode-289-plugin-a-missing");
    let (light, grown, _, _) = story(&app);
    assert_eq!(light, Some(("route:capability.missing".to_string(), true)));
    assert_eq!(grown, 0.0);
    // The plugin made and its XP awarded.
    let others = openagents_desktop::route_plugin::others_from();
    app.tick(open + Duration::from_secs_f32(others - 1.6));
    let made = write(&mut app, "episode-289-plugin-b-made");
    let (light, grown, xp, uses) = story(&app);
    assert_eq!(
        light.map(|(target, _)| target).as_deref(),
        Some(openagents_desktop::route_plugin::PLUGIN)
    );
    assert_eq!((grown, uses), (1.0, 0));
    assert!(xp > 25.0, "{xp}");
    // Others use it, and it holds there with the traffic flowing.
    let end = openagents_desktop::route_plugin::end();
    app.tick(open + Duration::from_secs_f32(end + 4.0));
    let used = write(&mut app, "episode-289-plugin-c-used");
    let (_, _, xp, uses) = story(&app);
    assert_eq!(xp, 225.0);
    assert!(uses > 5, "{uses}");
    assert_ne!(missing.pixels, made.pixels);
    assert_ne!(made.pixels, used.pixels);
    // The future plays next.
    assert!(key(&mut app, "ArrowRight", false, open));
    assert_eq!(app.presentation().unwrap().counter(), "5 / 8");
    let wake = app.tick(open).expect("the future asks for frames");
    assert!(wake <= open + Duration::from_millis(40));
    let future = |app: &DesktopApp| app.presentation().unwrap().future().unwrap().label();
    let end = openagents_desktop::route_future::END;
    // About a quarter of the way: mid 2027.
    app.tick(open + Duration::from_secs_f32(end * 0.25));
    assert_eq!(future(&app), "Oct 2027");
    let early = write(&mut app, "episode-289-future-a");
    app.tick(open + Duration::from_secs_f32(end * 0.6));
    assert_eq!(future(&app), "Apr 2029");
    let mid = write(&mut app, "episode-289-future-b");
    // It plays once and holds on December 2030, the traffic still flowing.
    app.tick(open + Duration::from_secs_f32(end + 20.0));
    assert_eq!(future(&app), "Dec 2030");
    let late = write(&mut app, "episode-289-future-c");
    app.tick(open + Duration::from_secs_f32(end + 21.0));
    assert_eq!(future(&app), "Dec 2030");
    assert!(
        app.tick(open + Duration::from_secs_f32(end + 21.0))
            .is_some()
    );
    assert_ne!(early.pixels, mid.pixels);
    assert_ne!(mid.pixels, late.pixels);
    assert_ne!(early.pixels, late.pixels);
}

/// The Episode 289 deck's sixth slide shows today's real traffic on the
/// route map from the public flow stream (#10198): replayed from the
/// fixture, each event's dot runs on the frame clock and the totals count
/// up; with the stream unreachable it says so and draws no dots. With
/// `OPENAGENTS_SLIDES_CAPTURE` set, the captures are kept there.
#[test]
fn episode_289_shows_live_traffic_from_the_flow_stream() {
    use openagents_desktop::route_live::{FIXTURE_PACE, Status, Totals, fixture};
    let (mut app, start) = shell();
    app.open_presentation("episode-289", start)
        .expect("the deck opens");
    let open = start + OPEN;
    app.tick(open);
    let events = fixture(include_str!(
        "../../../docs/payments/fixtures/flow-stream.jsonl"
    ));
    app.presentation_mut()
        .unwrap()
        .set_flow_source(FlowSource::Fixture(events));
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
    for _ in 0..5 {
        assert!(key(&mut app, "ArrowRight", false, open));
    }
    assert_eq!(app.presentation().unwrap().counter(), "6 / 8");
    let wake = app.tick(open).expect("the live map asks for frames");
    assert!(wake <= open + Duration::from_millis(40));
    let live = |app: &DesktopApp| {
        let live = app.presentation().unwrap().live().unwrap();
        (live.status(), live.totals(), live.pulses().len())
    };
    // The first call's white dot on its way out.
    app.tick(open + Duration::from_secs_f32(0.7));
    let (status, _, dots) = live(&app);
    assert_eq!(status, Status::Live);
    assert!(dots >= 1);
    let first = write(&mut app, "episode-289-live-a");
    // Mid stream: payments, shares, and a bonus in the air.
    app.tick(open + Duration::from_secs_f32(9.5));
    let (_, _, dots) = live(&app);
    assert!(dots >= 3, "{dots}");
    let pulses = app.presentation().unwrap().live().unwrap().pulses();
    assert!(pulses.iter().any(|p| p.ring), "the bonus wears its ring");
    let mid = write(&mut app, "episode-289-live-b");
    // The payout, and every total counted.
    app.tick(open + Duration::from_secs_f32(FIXTURE_PACE * 11.0 + 1.8));
    let (_, totals, _) = live(&app);
    assert_eq!(
        totals,
        Totals {
            received_sats: openagents_desktop::route_live::Sats::from_msat(64_000),
            paid_out_sats: openagents_desktop::route_live::Sats::from_msat(40_000),
            calls: 4,
        }
    );
    let late = write(&mut app, "episode-289-live-c");
    assert_ne!(first.pixels, mid.pixels);
    assert_ne!(mid.pixels, late.pixels);
    // A stream that can't be reached says so, and no dot is made up.
    app.presentation_mut()
        .unwrap()
        .set_flow_source(FlowSource::Url("http://127.0.0.1:9/flow".into()));
    let mut at = open + Duration::from_secs(20);
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        app.tick(at);
        let (status, _, dots) = live(&app);
        assert_eq!(dots, 0);
        if status == Status::Unreachable || std::time::Instant::now() > deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
        at += Duration::from_millis(50);
    }
    assert_eq!(live(&app).0, Status::Unreachable);
    let down = write(&mut app, "episode-289-live-unreachable");
    assert_ne!(down.pixels, late.pixels);
}

/// The Episode 289 deck's seventh slide shows two essays as GitHub link
/// cards and its eighth shows openagents.com/download in a browser window.
/// They come in on the frame clock; the pointer over a card brightens it,
/// and a click on one asks for its link instead of changing slides. With
/// `OPENAGENTS_SLIDES_CAPTURE` set, the captures are kept there.
#[test]
fn episode_289_shows_the_essays_and_the_download_page_as_link_cards() {
    use openagents_desktop::slide_embeds::{DOWNLOAD, DOWNLOAD_URL, ESSAYS, cards};
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
    // The live slide passes on the way: a fixture, never the network.
    app.presentation_mut()
        .unwrap()
        .set_flow_source(FlowSource::Fixture(Vec::new()));
    for _ in 0..6 {
        assert!(key(&mut app, "ArrowRight", false, open));
    }
    assert_eq!(app.presentation().unwrap().counter(), "7 / 8");
    let wake = app.tick(open).expect("the cards ask for frames");
    assert!(wake <= open + Duration::from_millis(20));
    let entering = write(&mut app, "episode-289-essays-entering");
    app.tick(open + Duration::from_secs(2));
    assert!(!app.presentation().unwrap().embeds().unwrap().entering());
    let input = |app: &mut DesktopApp, event: SurfaceInput| {
        App::surface_input(app, RESOURCE, event, open + Duration::from_secs(2));
    };
    let slide = Layout::of(WIDTH, HEIGHT, false, 1.0).slide;
    let second = cards(ESSAYS, slide)[1];
    let (x, y) = (second.x + second.w / 2.0, second.y + second.h / 2.0);
    let shown = write(&mut app, "episode-289-essays");
    input(&mut app, SurfaceInput::Move { x, y });
    assert_eq!(
        app.presentation().unwrap().embeds().unwrap().hovered(),
        Some(1)
    );
    let hovered = write(&mut app, "episode-289-essays-hover");
    assert_ne!(entering.pixels, shown.pixels);
    assert_ne!(shown.pixels, hovered.pixels);
    // A click on a card opens it, and the slide stays.
    input(&mut app, SurfaceInput::Down { x, y, shift: false });
    input(&mut app, SurfaceInput::Up { x, y });
    assert_eq!(app.presentation().unwrap().counter(), "7 / 8");
    // A click beside the cards goes on.
    let (gap_x, gap_y) = (slide.x + 4.0, slide.y + 4.0);
    input(
        &mut app,
        SurfaceInput::Down {
            x: gap_x,
            y: gap_y,
            shift: false,
        },
    );
    assert_eq!(app.presentation().unwrap().counter(), "8 / 8");
    app.tick(open + Duration::from_secs(4));
    app.tick(open + Duration::from_secs(6));
    write(&mut app, "episode-289-download");
    let window = cards(DOWNLOAD, slide)[0];
    let slides = app.slides.as_mut().unwrap();
    slides.input(
        SurfaceInput::Down {
            x: window.x + 40.0,
            y: window.y + 200.0,
            shift: false,
        },
        open + Duration::from_secs(6),
    );
    assert_eq!(slides.take_link().as_deref(), Some(DOWNLOAD_URL));
    assert_eq!(slides.take_link(), None, "taken once");
}
