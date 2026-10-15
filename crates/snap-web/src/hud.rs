//! The page's HUD and cards, drawn as plain elements over the canvas: the
//! job card from the crew member who gave it, the pieces counter, the
//! storm clock and the plant gravity meter (from the game's HUD elements,
//! `verse_game::HudElement`), the tray of pieces with Turn, Undo and Go at
//! the bottom, the menu and help buttons, a hint line, and a card for the
//! job list, the job's ask, the menu, the tutorial and the results.
//! Buttons queue an [`Action`] the game reads each frame.

use std::cell::RefCell;
use std::rc::Rc;

use snap_rules::PieceKind;
use verse_game::{HudElement, Icon};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{Document, Element, HtmlElement};

use crate::copy;
use crate::kit::{self, Crew};

/// What a button asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    /// Play job N (1-based).
    Play(usize),
    /// A tray piece pressed, at this screen point.
    Pick(PieceKind, f32, f32),
    Turn,
    Undo,
    Go,
    /// Stop a run, or go back to building after one.
    Fix,
    NextJob,
    AllJobs,
    Menu,
    Resume,
    Restart,
    ToggleSound,
    ToggleContrast,
    Home,
    Close,
    /// The job's ask again.
    Ask,
    Tour,
    TourNext,
    TourBack,
    /// Close the tutorial, as seen: `true` from its last step's main
    /// button, `false` from Skip.
    TourEnd(bool),
}

/// A card's button.
#[derive(Clone, Debug, PartialEq)]
pub struct Button {
    pub label: String,
    pub action: Action,
    pub main: bool,
    pub enabled: bool,
}

impl Button {
    pub fn new(label: impl Into<String>, action: Action, main: bool) -> Self {
        Self {
            label: label.into(),
            action,
            main,
            enabled: true,
        }
    }
}

/// A card's contents.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Card {
    pub title: String,
    pub crew: Option<Crew>,
    pub lines: Vec<String>,
    pub stars: Option<u8>,
    /// A link under the buttons: its words and where it goes.
    pub link: Option<(String, String)>,
    pub buttons: Vec<Button>,
    /// Lay the buttons out in a list (the job list) rather than a row.
    pub list: bool,
}

pub fn element(document: &Document, tag: &str) -> HtmlElement {
    document
        .create_element(tag)
        .expect("an element")
        .dyn_into()
        .expect("an HTML element")
}

pub fn css(element: &HtmlElement, pairs: &[(&str, &str)]) {
    let style = element.style();
    for (name, value) in pairs {
        let _ = style.set_property(name, value);
    }
}

pub fn show(element: &HtmlElement, visible: bool) {
    css(element, &[("display", if visible { "" } else { "none" })]);
}

/// Shows a flex box as one, or hides it.
fn show_flex(element: &HtmlElement, visible: bool) {
    css(
        element,
        &[("display", if visible { "flex" } else { "none" })],
    );
}

fn hex(colour: u32) -> String {
    format!("#{colour:06x}")
}

const SVG: &str = "http://www.w3.org/2000/svg";
const INK: &str = "#f4f7ff";
const PANEL: [(&str, &str); 5] = [
    ("position", "absolute"),
    ("background", "rgba(16, 22, 48, 0.86)"),
    ("border", "2px solid rgba(160, 190, 255, 0.45)"),
    ("border-radius", "16px"),
    ("padding", "8px 12px"),
];
const BUTTON: [(&str, &str); 8] = [
    ("font", "inherit"),
    ("font-weight", "700"),
    ("color", "#101630"),
    ("background", "#f4f7ff"),
    ("border", "0"),
    ("border-radius", "999px"),
    ("cursor", "pointer"),
    ("pointer-events", "auto"),
];

fn svg(document: &Document, size: u32) -> Element {
    let svg = document
        .create_element_ns(Some(SVG), "svg")
        .expect("an svg");
    let _ = svg.set_attribute("viewBox", "0 0 40 40");
    let _ = svg.set_attribute("width", &size.to_string());
    let _ = svg.set_attribute("height", &size.to_string());
    let _ = svg.set_attribute("aria-hidden", "true");
    svg
}

fn shape(document: &Document, parent: &Element, tag: &str, attrs: &[(&str, &str)]) {
    let child = document.create_element_ns(Some(SVG), tag).expect("a shape");
    for (name, value) in attrs {
        let _ = child.set_attribute(name, value);
    }
    let _ = parent.append_child(&child);
}

/// A piece's picture for its tray button.
fn piece_icon(document: &Document, kind: PieceKind, size: u32) -> Element {
    let svg = svg(document, size);
    let fill = hex(kit::colour(kind));
    let frame = |points: &str| {
        shape(
            document,
            &svg,
            "polygon",
            &[
                ("points", points),
                ("fill", "rgba(255,255,255,0.25)"),
                ("stroke", &fill),
                ("stroke-width", "4"),
                ("stroke-linejoin", "round"),
            ],
        );
    };
    let solid = |points: &str, colour: &str| {
        shape(
            document,
            &svg,
            "polygon",
            &[
                ("points", points),
                ("fill", colour),
                ("stroke", "#0b1030"),
                ("stroke-width", "1.5"),
            ],
        );
    };
    match kind {
        PieceKind::Square => frame("8,8 32,8 32,32 8,32"),
        PieceKind::Triangle => frame("6,33 34,33 20,9"),
        PieceKind::Wedge => frame("8,32 32,32 8,8"),
        PieceKind::Panel => solid("8,8 32,8 32,32 8,32", &fill),
        PieceKind::Solar => {
            solid("13,4 27,4 27,36 13,36", &fill);
            for y in ["12", "20", "28"] {
                shape(
                    document,
                    &svg,
                    "line",
                    &[
                        ("x1", "13"),
                        ("x2", "27"),
                        ("y1", y),
                        ("y2", y),
                        ("stroke", "#9fbaff"),
                        ("stroke-width", "1.5"),
                    ],
                );
            }
        }
        PieceKind::Thruster => {
            solid("8,10 32,10 20,28", &fill);
            solid("15,28 25,28 28,36 12,36", "#2b2f3a");
        }
        PieceKind::Cargo => {
            solid("7,7 33,7 33,33 7,33", &fill);
            for (x1, y1, x2, y2) in [("7", "7", "33", "33"), ("33", "7", "7", "33")] {
                shape(
                    document,
                    &svg,
                    "line",
                    &[
                        ("x1", x1),
                        ("y1", y1),
                        ("x2", x2),
                        ("y2", y2),
                        ("stroke", "#8a4e22"),
                        ("stroke-width", "3"),
                    ],
                );
            }
        }
        PieceKind::Pod => {
            solid("7,7 33,7 33,33 7,33", &fill);
            shape(
                document,
                &svg,
                "circle",
                &[("cx", "20"), ("cy", "17"), ("r", "7"), ("fill", "#2fa84f")],
            );
            solid("15,24 25,24 23,31 17,31", "#d96c3f");
        }
        PieceKind::Module => solid("6,6 34,6 34,34 6,34", &fill),
        PieceKind::Strut => {
            shape(
                document,
                &svg,
                "polygon",
                &[
                    ("points", "8,8 32,8 32,32 8,32"),
                    ("fill", "none"),
                    ("stroke", "rgba(255,255,255,0.35)"),
                    ("stroke-width", "2"),
                    ("stroke-dasharray", "3 3"),
                ],
            );
            shape(
                document,
                &svg,
                "line",
                &[
                    ("x1", "9"),
                    ("y1", "31"),
                    ("x2", "31"),
                    ("y2", "9"),
                    ("stroke", &fill),
                    ("stroke-width", "5"),
                    ("stroke-linecap", "round"),
                ],
            );
        }
        PieceKind::Bolt => {
            solid("20,7 31,13.5 31,26.5 20,33 9,26.5 9,13.5", &fill);
            shape(
                document,
                &svg,
                "circle",
                &[("cx", "20"), ("cy", "20"), ("r", "5"), ("fill", "#6b7280")],
            );
        }
    }
    svg
}

/// A crew member's face.
fn face(document: &Document, crew: Crew, size: u32) -> Element {
    let svg = svg(document, size);
    let colour = hex(crew.colour());
    shape(
        document,
        &svg,
        "line",
        &[
            ("x1", "20"),
            ("y1", "9"),
            ("x2", "20"),
            ("y2", "3"),
            ("stroke", "#c8cdd6"),
            ("stroke-width", "2"),
        ],
    );
    shape(
        document,
        &svg,
        "circle",
        &[
            ("cx", "20"),
            ("cy", "3.5"),
            ("r", "2.5"),
            ("fill", "#ff4d4d"),
        ],
    );
    match crew {
        Crew::Rivet | Crew::Tock => shape(
            document,
            &svg,
            "rect",
            &[
                ("x", "6"),
                ("y", "9"),
                ("width", "28"),
                ("height", "28"),
                ("rx", "6"),
                ("fill", &colour),
            ],
        ),
        _ => shape(
            document,
            &svg,
            "circle",
            &[("cx", "20"), ("cy", "23"), ("r", "15"), ("fill", &colour)],
        ),
    }
    let eyes: &[(&str, &str)] = if crew == Crew::Pip {
        &[("20", "6")]
    } else {
        &[("14", "4.5"), ("26", "4.5")]
    };
    for (cx, r) in eyes {
        shape(
            document,
            &svg,
            "circle",
            &[("cx", cx), ("cy", "21"), ("r", r), ("fill", "#ffffff")],
        );
        shape(
            document,
            &svg,
            "circle",
            &[("cx", cx), ("cy", "22"), ("r", "2"), ("fill", "#1e2230")],
        );
    }
    svg
}

fn on_click(target: &HtmlElement, actions: &Rc<RefCell<Vec<Action>>>, action: Action) {
    let actions = actions.clone();
    let closure = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        event.stop_propagation();
        actions.borrow_mut().push(action);
    });
    let _ = target.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
    let stop = Closure::<dyn FnMut(web_sys::Event)>::new(|event: web_sys::Event| {
        event.stop_propagation();
    });
    let _ = target.add_event_listener_with_callback("pointerdown", stop.as_ref().unchecked_ref());
    closure.forget();
    stop.forget();
}

/// A tray button: pressing it picks the piece up at the pointer.
fn on_pick(target: &HtmlElement, actions: &Rc<RefCell<Vec<Action>>>, kind: PieceKind) {
    let actions = actions.clone();
    let closure =
        Closure::<dyn FnMut(web_sys::PointerEvent)>::new(move |event: web_sys::PointerEvent| {
            event.stop_propagation();
            event.prevent_default();
            actions.borrow_mut().push(Action::Pick(
                kind,
                event.client_x() as f32,
                event.client_y() as f32,
            ));
        });
    let _ =
        target.add_event_listener_with_callback("pointerdown", closure.as_ref().unchecked_ref());
    closure.forget();
}

fn button(
    document: &Document,
    label: &str,
    actions: &Rc<RefCell<Vec<Action>>>,
    action: Action,
) -> HtmlElement {
    let b = element(document, "button");
    b.set_text_content(Some(label));
    let _ = b.set_attribute("type", "button");
    css(&b, &BUTTON);
    on_click(&b, actions, action);
    b
}

/// One tray slot.
struct Slot {
    kind: PieceKind,
    button: HtmlElement,
    badge: HtmlElement,
    left: u32,
}

/// What the tray shows now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Building,
    Running,
    Over,
}

pub struct Hud {
    pub root: HtmlElement,
    document: Document,
    play: HtmlElement,
    job_face: HtmlElement,
    job_goal: HtmlElement,
    job_crew: HtmlElement,
    shown_job: Option<(u8, Crew)>,
    pieces: HtmlElement,
    storm: HtmlElement,
    storm_text: HtmlElement,
    gravity: HtmlElement,
    gravity_band: HtmlElement,
    gravity_mark: HtmlElement,
    gravity_hold: HtmlElement,
    tray: HtmlElement,
    tray_row: HtmlElement,
    slots: Vec<Slot>,
    /// A tray button's side.
    slot_size: &'static str,
    focus: Option<usize>,
    turn: HtmlElement,
    undo: HtmlElement,
    go: HtmlElement,
    mode: Option<Mode>,
    hint: HtmlElement,
    hint_text: String,
    card: HtmlElement,
    card_face: HtmlElement,
    card_title: HtmlElement,
    card_stars: HtmlElement,
    card_lines: HtmlElement,
    card_buttons: HtmlElement,
    card_link: HtmlElement,
    shown_card: Option<Card>,
    actions: Rc<RefCell<Vec<Action>>>,
    touch: bool,
}

impl Hud {
    pub fn new(document: &Document, parent: &Element, touch: bool) -> Self {
        let actions = Rc::new(RefCell::new(Vec::new()));
        let root = element(document, "div");
        css(
            &root,
            &[
                ("position", "absolute"),
                ("inset", "0"),
                ("pointer-events", "none"),
                (
                    "font-family",
                    "system-ui, -apple-system, 'Segoe UI', sans-serif",
                ),
                ("color", INK),
                ("user-select", "none"),
                ("-webkit-user-select", "none"),
            ],
        );
        let play = element(document, "div");
        css(&play, &[("position", "absolute"), ("inset", "0")]);

        // Top left: the job and who gave it.
        let job = element(document, "button");
        let _ = job.set_attribute("type", "button");
        css(&job, &PANEL);
        css(
            &job,
            &[
                ("position", "relative"),
                ("display", "flex"),
                ("align-items", "center"),
                ("gap", "10px"),
                ("max-width", "min(58vw, 360px)"),
                ("color", INK),
                ("font", "inherit"),
                ("text-align", "left"),
                ("cursor", "pointer"),
                ("pointer-events", "auto"),
            ],
        );
        on_click(&job, &actions, Action::Ask);
        let job_face = element(document, "span");
        css(&job_face, &[("display", "flex"), ("flex", "none")]);
        let words = element(document, "span");
        css(&words, &[("display", "flex"), ("flex-direction", "column")]);
        let job_goal = element(document, "span");
        css(&job_goal, &[("font-size", "16px"), ("font-weight", "750")]);
        let job_crew = element(document, "span");
        css(&job_crew, &[("font-size", "13px"), ("opacity", "0.8")]);
        let _ = words.append_child(&job_goal);
        let _ = words.append_child(&job_crew);
        let _ = job.append_child(&job_face);
        let _ = job.append_child(&words);

        // Top right: help and menu.
        let corner = element(document, "div");
        css(
            &corner,
            &[
                ("position", "absolute"),
                ("top", "calc(10px + env(safe-area-inset-top))"),
                ("right", "calc(10px + env(safe-area-inset-right))"),
                ("display", "flex"),
                ("gap", "8px"),
            ],
        );
        let help = button(document, "?", &actions, Action::Tour);
        let _ = help.set_attribute("aria-label", copy::HOW_TO_PLAY);
        let menu = button(document, "\u{2630}", &actions, Action::Menu);
        let _ = menu.set_attribute("aria-label", copy::MENU);
        for b in [&help, &menu] {
            css(
                b,
                &[("width", "46px"), ("height", "46px"), ("font-size", "20px")],
            );
            let _ = corner.append_child(b);
        }

        // Under the job: pieces used, the storm clock, the gravity meter.
        let chips = element(document, "div");
        css(
            &chips,
            &[
                ("display", "flex"),
                ("flex-direction", "column"),
                ("gap", "6px"),
                ("align-items", "flex-start"),
            ],
        );
        // The job card with the chips under it, however tall it wraps.
        let left = element(document, "div");
        css(
            &left,
            &[
                ("position", "absolute"),
                ("top", "calc(10px + env(safe-area-inset-top))"),
                ("left", "calc(10px + env(safe-area-inset-left))"),
                ("display", "flex"),
                ("flex-direction", "column"),
                ("gap", "6px"),
                ("align-items", "flex-start"),
            ],
        );
        let _ = left.append_child(&job);
        let _ = left.append_child(&chips);
        let chip = |document: &Document| {
            let c = element(document, "div");
            css(&c, &PANEL);
            css(
                &c,
                &[
                    ("position", "relative"),
                    ("padding", "5px 10px"),
                    ("font-size", "14px"),
                    ("font-weight", "700"),
                ],
            );
            c
        };
        let pieces = chip(document);
        let storm = chip(document);
        css(&storm, &[("color", "#ffd166")]);
        let storm_text = element(document, "span");
        let _ = storm.append_child(&storm_text);
        let gravity = chip(document);
        css(&gravity, &[("min-width", "170px")]);
        let gravity_label = element(document, "div");
        gravity_label.set_text_content(Some(copy::GRAVITY));
        let gravity_bar = element(document, "div");
        css(
            &gravity_bar,
            &[
                ("position", "relative"),
                ("height", "14px"),
                ("margin-top", "4px"),
                ("border-radius", "7px"),
                ("background", "rgba(255,255,255,0.15)"),
                ("overflow", "hidden"),
            ],
        );
        let gravity_band = element(document, "div");
        css(
            &gravity_band,
            &[
                ("position", "absolute"),
                ("top", "0"),
                ("bottom", "0"),
                ("background", "rgba(76, 195, 138, 0.55)"),
            ],
        );
        let gravity_mark = element(document, "div");
        css(
            &gravity_mark,
            &[
                ("position", "absolute"),
                ("top", "0"),
                ("bottom", "0"),
                ("width", "4px"),
                ("background", "#ffffff"),
            ],
        );
        let gravity_hold = element(document, "div");
        css(
            &gravity_hold,
            &[
                ("height", "4px"),
                ("margin-top", "3px"),
                ("background", "#4cc38a"),
                ("width", "0%"),
            ],
        );
        let _ = gravity_bar.append_child(&gravity_band);
        let _ = gravity_bar.append_child(&gravity_mark);
        for child in [&gravity_label, &gravity_bar, &gravity_hold] {
            let _ = gravity.append_child(child);
        }
        for child in [&pieces, &storm, &gravity] {
            let _ = chips.append_child(child);
        }

        // The hint line, over the tray.
        let hint = element(document, "div");
        css(
            &hint,
            &[
                ("position", "absolute"),
                ("left", "50%"),
                ("transform", "translateX(-50%)"),
                ("bottom", "calc(108px + env(safe-area-inset-bottom))"),
                ("max-width", "86vw"),
                ("padding", "6px 14px"),
                ("border-radius", "999px"),
                ("background", "rgba(16, 22, 48, 0.8)"),
                ("font-size", "15px"),
                ("font-weight", "650"),
                ("text-align", "center"),
            ],
        );

        // The tray: pieces, then Turn, Undo and Go.
        let tray = element(document, "div");
        css(&tray, &PANEL);
        css(
            &tray,
            &[
                ("left", "calc(8px + env(safe-area-inset-left))"),
                ("right", "calc(8px + env(safe-area-inset-right))"),
                ("bottom", "calc(8px + env(safe-area-inset-bottom))"),
                ("display", "flex"),
                ("align-items", "center"),
                ("gap", "8px"),
                ("padding", "8px"),
                ("pointer-events", "auto"),
                ("touch-action", "none"),
            ],
        );
        let tray_row = element(document, "div");
        css(
            &tray_row,
            &[
                ("display", "flex"),
                ("gap", "8px"),
                ("flex", "1"),
                ("overflow-x", "auto"),
                ("min-width", "0"),
            ],
        );
        // On a narrow screen Turn and Undo are their arrows alone, so the
        // tray keeps room for pieces.
        let narrow = web_sys::window()
            .and_then(|w| w.inner_width().ok())
            .and_then(|w| w.as_f64())
            .is_some_and(|w| w < 600.0);
        let size = if narrow { "60px" } else { "72px" };
        let tool = |symbol: &str, words: &str, action: Action| {
            let label = if narrow {
                symbol.to_owned()
            } else {
                format!("{symbol} {words}")
            };
            let b = button(document, &label, &actions, action);
            let _ = b.set_attribute("aria-label", words);
            css(
                &b,
                &[
                    ("height", size),
                    ("min-width", if narrow { "48px" } else { "64px" }),
                    ("padding", if narrow { "0 8px" } else { "0 12px" }),
                    ("font-size", if narrow { "22px" } else { "15px" }),
                    ("border-radius", "18px"),
                    ("flex", "none"),
                ],
            );
            b
        };
        let turn = tool("\u{21bb}", copy::TURN, Action::Turn);
        let undo = tool("\u{21b6}", copy::UNDO, Action::Undo);
        let go = tool(copy::GO, copy::GO, Action::Go);
        go.set_text_content(Some(copy::GO));
        let _ = go.set_attribute("data-action", "go");
        css(
            &go,
            &[
                ("min-width", if narrow { "72px" } else { "96px" }),
                ("font-size", "24px"),
                ("background", "#3ddc84"),
                ("color", "#0b2a17"),
            ],
        );
        for child in [&tray_row, &turn, &undo, &go] {
            let _ = tray.append_child(child);
        }

        // The card.
        let card = element(document, "div");
        css(&card, &PANEL);
        css(
            &card,
            &[
                ("left", "50%"),
                ("top", "50%"),
                ("transform", "translate(-50%, -50%)"),
                ("width", "min(88vw, 440px)"),
                ("max-height", "84vh"),
                ("overflow-y", "auto"),
                ("padding", "18px 20px"),
                ("text-align", "center"),
                ("pointer-events", "auto"),
                ("box-sizing", "border-box"),
                ("background", "rgba(16, 22, 48, 0.95)"),
            ],
        );
        let card_face = element(document, "div");
        css(
            &card_face,
            &[("display", "flex"), ("justify-content", "center")],
        );
        let card_title = element(document, "h2");
        css(&card_title, &[("margin", "6px 0"), ("font-size", "26px")]);
        let card_stars = element(document, "div");
        css(
            &card_stars,
            &[
                ("font-size", "34px"),
                ("color", "#ffd23f"),
                ("letter-spacing", "4px"),
            ],
        );
        let card_lines = element(document, "div");
        css(
            &card_lines,
            &[("font-size", "17px"), ("line-height", "1.4")],
        );
        let card_buttons = element(document, "div");
        css(
            &card_buttons,
            &[
                ("display", "flex"),
                ("flex-wrap", "wrap"),
                ("gap", "10px"),
                ("justify-content", "center"),
                ("margin-top", "14px"),
            ],
        );
        let card_link = element(document, "a");
        css(
            &card_link,
            &[
                ("display", "inline-block"),
                ("margin-top", "12px"),
                ("color", "#9fd2ff"),
                ("font-size", "15px"),
            ],
        );
        for child in [
            &card_face,
            &card_title,
            &card_stars,
            &card_lines,
            &card_buttons,
            &card_link,
        ] {
            let _ = card.append_child(child);
        }

        for child in [&left, &hint] {
            let _ = play.append_child(child);
        }
        let _ = play.append_child(&corner);
        let _ = play.append_child(&tray);
        let _ = root.append_child(&play);
        let _ = root.append_child(&card);
        let _ = parent.append_child(&root);
        show(&card, false);
        show(&storm, false);
        show(&gravity, false);
        show(&hint, false);
        Self {
            root,
            document: document.clone(),
            play,
            job_face,
            job_goal,
            job_crew,
            shown_job: None,
            pieces,
            storm,
            storm_text,
            gravity,
            gravity_band,
            gravity_mark,
            gravity_hold,
            tray,
            tray_row,
            slots: Vec::new(),
            slot_size: size,
            focus: None,
            turn,
            undo,
            go,
            mode: None,
            hint,
            hint_text: String::new(),
            card,
            card_face,
            card_title,
            card_stars,
            card_lines,
            card_buttons,
            card_link,
            shown_card: None,
            actions,
            touch,
        }
    }

    /// The actions queued since the last call.
    pub fn take(&self) -> Vec<Action> {
        std::mem::take(&mut *self.actions.borrow_mut())
    }

    /// The tray's top edge, in CSS pixels from the top of the page.
    pub fn tray_top(&self) -> f32 {
        self.tray.get_bounding_client_rect().top() as f32
    }

    /// Whether a screen point is over a HUD element that takes pointers.
    pub fn covers(&self, x: f32, y: f32) -> bool {
        let inside = |e: &HtmlElement| {
            let r = e.get_bounding_client_rect();
            r.width() > 0.0
                && f64::from(x) >= r.left()
                && f64::from(x) <= r.right()
                && f64::from(y) >= r.top()
                && f64::from(y) <= r.bottom()
        };
        inside(&self.tray) || (self.shown_card.is_some() && inside(&self.card))
    }

    pub fn set_playing(&self, visible: bool) {
        show(&self.play, visible);
    }

    pub fn set_job(&mut self, number: u8, crew: Crew) {
        if self.shown_job == Some((number, crew)) {
            return;
        }
        self.shown_job = Some((number, crew));
        self.job_face.set_inner_html("");
        let _ = self.job_face.append_child(&face(&self.document, crew, 44));
        self.job_goal.set_text_content(Some(copy::goal(number)));
        self.job_crew.set_text_content(Some(&format!(
            "{}, {}",
            copy::crew(crew),
            copy::crew_role(crew)
        )));
    }

    /// The tray's pieces and how many are left; `focus` outlines the slot
    /// the keyboard has picked.
    pub fn set_tray(&mut self, tray: &[(PieceKind, u32)], focus: Option<usize>) {
        let kinds: Vec<PieceKind> = tray.iter().map(|(k, _)| *k).collect();
        if kinds != self.slots.iter().map(|s| s.kind).collect::<Vec<_>>() {
            self.tray_row.set_inner_html("");
            self.slots.clear();
            for kind in kinds {
                let b = element(&self.document, "button");
                let _ = b.set_attribute("type", "button");
                let _ = b.set_attribute("data-piece", kind.word());
                css(&b, &BUTTON);
                css(
                    &b,
                    &[
                        ("position", "relative"),
                        ("width", self.slot_size),
                        ("height", self.slot_size),
                        ("flex", "none"),
                        ("border-radius", "18px"),
                        ("background", "rgba(255,255,255,0.1)"),
                        ("display", "flex"),
                        ("align-items", "center"),
                        ("justify-content", "center"),
                        ("touch-action", "none"),
                    ],
                );
                let _ = b.append_child(&piece_icon(&self.document, kind, 50));
                let badge = element(&self.document, "span");
                css(
                    &badge,
                    &[
                        ("position", "absolute"),
                        ("right", "4px"),
                        ("bottom", "3px"),
                        ("min-width", "22px"),
                        ("padding", "1px 5px"),
                        ("border-radius", "999px"),
                        ("background", "#f4f7ff"),
                        ("color", "#101630"),
                        ("font-size", "14px"),
                        ("font-weight", "800"),
                        ("box-sizing", "border-box"),
                    ],
                );
                let _ = b.append_child(&badge);
                on_pick(&b, &self.actions, kind);
                let _ = self.tray_row.append_child(&b);
                self.slots.push(Slot {
                    kind,
                    button: b,
                    badge,
                    left: u32::MAX,
                });
            }
        }
        for (slot, (_, left)) in self.slots.iter_mut().zip(tray) {
            if slot.left != *left {
                slot.left = *left;
                slot.badge.set_text_content(Some(&left.to_string()));
                let _ = slot
                    .button
                    .set_attribute("aria-label", &copy::tray_label(slot.kind, *left));
                css(
                    &slot.button,
                    &[("opacity", if *left == 0 { "0.35" } else { "1" })],
                );
            }
        }
        if self.focus != focus {
            self.focus = focus;
            for (index, slot) in self.slots.iter().enumerate() {
                css(
                    &slot.button,
                    &[(
                        "outline",
                        if Some(index) == focus {
                            "3px solid #ffd23f"
                        } else {
                            "none"
                        },
                    )],
                );
            }
            css(
                &self.go,
                &[(
                    "outline",
                    if focus == Some(self.slots.len()) {
                        "3px solid #ffd23f"
                    } else {
                        "none"
                    },
                )],
            );
        }
    }

    pub fn set_mode(&mut self, mode: Mode) {
        if self.mode == Some(mode) {
            return;
        }
        self.mode = Some(mode);
        let building = mode == Mode::Building;
        show_flex(&self.tray_row, building);
        show(&self.turn, building);
        show(&self.undo, building);
        show_flex(&self.tray, mode != Mode::Over);
        self.go
            .set_text_content(Some(if building { copy::GO } else { copy::STOP }));
        css(
            &self.go,
            &[("background", if building { "#3ddc84" } else { "#ffb454" })],
        );
        // Go is replaced while a run plays; the click queues Go, which the
        // game reads as Stop then.
        show(&self.go, mode != Mode::Over);
    }

    /// The HUD elements the game asks for: the pieces counter, the storm
    /// clock, and the gravity meter (with the band it needs, as fractions
    /// of the meter).
    pub fn set_elements(&self, hud: &[HudElement], band: Option<(f32, f32)>, storm_seconds: u32) {
        let mut storm = false;
        let mut gravity = false;
        for element in hud {
            match element {
                HudElement::Counter {
                    icon: Icon::Score,
                    value,
                    ..
                } => {
                    self.pieces
                        .set_text_content(Some(&format!("{}: {value}", copy::PIECES)));
                }
                HudElement::Ring {
                    icon: Icon::Clock, ..
                } => {
                    storm = true;
                    self.storm_text.set_text_content(Some(&format!(
                        "\u{26a1} {} {}:{:02}",
                        copy::STORM,
                        storm_seconds / 60,
                        storm_seconds % 60
                    )));
                }
                HudElement::Pips { filled, bar, .. } => {
                    gravity = true;
                    let at = (f32::from(*filled) / 5.0 * 100.0).min(98.0);
                    css(&self.gravity_mark, &[("left", &format!("{at}%"))]);
                    css(
                        &self.gravity_hold,
                        &[("width", &format!("{}%", bar * 100.0))],
                    );
                    if let Some((low, high)) = band {
                        css(
                            &self.gravity_band,
                            &[
                                ("left", &format!("{}%", low * 100.0)),
                                ("width", &format!("{}%", (high - low) * 100.0)),
                            ],
                        );
                    }
                }
                _ => {}
            }
        }
        show(&self.storm, storm);
        show(&self.gravity, gravity);
    }

    /// The gravity meter's needle placed exactly, from 0 to 1.
    pub fn set_gravity_needle(&self, at: f32) {
        css(
            &self.gravity_mark,
            &[("left", &format!("{}%", (at * 100.0).clamp(0.0, 98.0)))],
        );
    }

    pub fn set_hint(&mut self, text: &str) {
        if self.hint_text == text {
            return;
        }
        self.hint_text = text.to_owned();
        self.hint.set_text_content(Some(text));
        show(&self.hint, !text.is_empty());
    }

    pub fn touch(&self) -> bool {
        self.touch
    }

    pub fn set_card(&mut self, card: Option<Card>) {
        if self.shown_card == card {
            return;
        }
        self.shown_card = card.clone();
        let Some(card) = card else {
            show(&self.card, false);
            return;
        };
        self.card_face.set_inner_html("");
        if let Some(crew) = card.crew {
            let _ = self.card_face.append_child(&face(&self.document, crew, 72));
        }
        self.card_title.set_text_content(Some(&card.title));
        self.card_stars
            .set_text_content(Some(&card.stars.map(copy::stars).unwrap_or_default()));
        show(&self.card_stars, card.stars.is_some());
        self.card_lines.set_inner_html("");
        for line in &card.lines {
            let p = element(&self.document, "p");
            css(&p, &[("margin", "6px 0")]);
            p.set_text_content(Some(line));
            let _ = self.card_lines.append_child(&p);
        }
        self.card_buttons.set_inner_html("");
        css(
            &self.card_buttons,
            &[("flex-direction", if card.list { "column" } else { "row" })],
        );
        let mut first = None;
        for b in &card.buttons {
            let el = button(&self.document, &b.label, &self.actions, b.action);
            css(
                &el,
                &[
                    ("min-height", "48px"),
                    ("padding", "10px 20px"),
                    ("font-size", "17px"),
                    ("background", if b.main { "#3ddc84" } else { "#f4f7ff" }),
                    ("text-align", if card.list { "left" } else { "center" }),
                ],
            );
            if !b.enabled {
                let _ = el.set_attribute("disabled", "");
                css(&el, &[("opacity", "0.4"), ("cursor", "default")]);
            }
            if b.main && first.is_none() {
                first = Some(el.clone());
            }
            let _ = self.card_buttons.append_child(&el);
        }
        match &card.link {
            Some((words, href)) => {
                self.card_link.set_text_content(Some(words));
                let _ = self.card_link.set_attribute("href", href);
                show(&self.card_link, true);
            }
            None => show(&self.card_link, false),
        }
        show(&self.card, true);
        if let Some(first) = first {
            let _ = first.focus();
        }
    }
}
