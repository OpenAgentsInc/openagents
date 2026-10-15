//! The HUD and cards, drawn as plain elements over the canvas and styled
//! through the CSSOM (the page's policy admits no inline style): health,
//! keys, a toast line, the crosshair, the interact prompt with the front
//! door's unlock bar, the hot bar, the menu button, the touch controls,
//! and one card for the title, the tutorial, the menu and the run card.
//! Buttons queue an [`Action`] the game reads each frame.

use std::cell::RefCell;
use std::rc::Rc;

use icecream_rules::Tool;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{Document, Element, HtmlElement, PointerEvent};

use crate::copy;

/// A touch button that is held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hold {
    Jump,
    Fire,
    Use,
    /// Down and up; the game holds or toggles sprint by the setting.
    Sprint,
}

/// What a button asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Play,
    Resume,
    Menu,
    NewHouse,
    Tour,
    TourNext,
    TourBack,
    /// Close the tutorial: `true` from its last card, `false` from Skip.
    TourEnd(bool),
    Select(Tool),
    /// The next tool in the hot bar (`true`: the one before).
    Cycle(bool),
    /// The menu's settings.
    ToggleTimer,
    ToggleJumpscares,
    ToggleSprintMode,
    /// Easy, Normal, Hard, round and round.
    ToggleDifficulty,
    /// First or third person (V on a keyboard).
    ToggleView,
    /// After a win: play on in free roam with the sprinkle staff.
    KeepPlaying,
    Held(Hold, bool),
}

/// A card's contents.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Card {
    pub title: String,
    pub lines: Vec<String>,
    /// A small italic line, like the check-in note.
    pub note: Option<String>,
    pub buttons: Vec<(String, Action, bool)>,
    pub links: Vec<(String, String)>,
    /// The escape card: daylight behind it.
    pub sunny: bool,
    /// The scooped card: a waffle colour behind it.
    pub scooped: bool,
    /// A tiny line at the foot: the build's version.
    pub version: Option<String>,
}

/// A hot-bar slot's width in pixels for `count` slots in `room` pixels:
/// 84 when there's room, down to 44.
#[must_use]
pub fn slot_width(room: f32, count: usize) -> u32 {
    let n = count.max(1) as f32;
    ((room - 24.0) / n - 8.0).clamp(44.0, 84.0) as u32
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

const FONT: &str = "ui-rounded, 'Trebuchet MS', system-ui, sans-serif";
const INK: &str = "#3a2418";
const PANEL: [(&str, &str); 6] = [
    ("position", "absolute"),
    ("background", "rgba(255, 246, 236, 0.88)"),
    ("border", "3px solid #3a2418"),
    ("border-radius", "14px"),
    ("padding", "6px 12px"),
    ("box-shadow", "0 3px 0 #3a2418"),
];
const BUTTON: [(&str, &str); 8] = [
    ("font", "inherit"),
    ("font-weight", "800"),
    ("color", "#3a2418"),
    ("border", "3px solid #3a2418"),
    ("border-radius", "14px"),
    ("cursor", "pointer"),
    ("pointer-events", "auto"),
    ("box-shadow", "0 3px 0 #3a2418"),
];

/// No browser gesture starts on `element`: no double-tap zoom, no pan, no
/// long-press callout or text selection. iOS Safari ignores the page's
/// `user-scalable=no`, so every control says so itself. `manipulation`
/// first, for browsers that don't know `none` (an unknown value is
/// ignored and the first stays).
pub fn no_gestures(element: &HtmlElement) {
    css(element, &[("touch-action", "manipulation")]);
    css(
        element,
        &[
            ("touch-action", "none"),
            ("-webkit-tap-highlight-color", "transparent"),
            ("-webkit-touch-callout", "none"),
            ("user-select", "none"),
            ("-webkit-user-select", "none"),
        ],
    );
}

fn listen<E: wasm_bindgen::convert::FromWasmAbi + 'static>(
    target: &HtmlElement,
    name: &str,
    handler: impl FnMut(E) + 'static,
) {
    let closure = Closure::<dyn FnMut(E)>::new(handler);
    let _ = target.add_event_listener_with_callback(name, closure.as_ref().unchecked_ref());
    closure.forget();
}

/// A card's button: a click (a tap, a mouse click, Enter or Space).
fn on_click(target: &HtmlElement, actions: &Rc<RefCell<Vec<Action>>>, action: Action) {
    let actions = actions.clone();
    listen(target, "click", move |event: web_sys::Event| {
        event.prevent_default();
        event.stop_propagation();
        actions.borrow_mut().push(action);
    });
}

/// A button used while playing: it acts the moment a finger or the mouse
/// goes down. A click only comes after a lone finger lifts without
/// moving, so a tap made while another finger holds the stick, or that
/// slides a little, never clicked; `pointerdown` comes for every finger.
/// Enter or Space on the focused button still clicks (a click with no
/// pointer has `detail` 0).
fn on_press(target: &HtmlElement, actions: &Rc<RefCell<Vec<Action>>>, action: Action) {
    let pressed = actions.clone();
    listen(target, "pointerdown", move |event: PointerEvent| {
        if event.button() != 0 {
            return;
        }
        event.prevent_default();
        pressed.borrow_mut().push(action);
    });
    let clicked = actions.clone();
    listen(target, "click", move |event: web_sys::MouseEvent| {
        event.prevent_default();
        event.stop_propagation();
        if event.detail() == 0 {
            clicked.borrow_mut().push(action);
        }
    });
}

/// A button held down by a finger: down, then up when that finger lifts
/// or the browser takes it back. The button captures the finger, so
/// sliding off it doesn't let go; another finger on it is ignored.
fn on_hold(target: &HtmlElement, actions: &Rc<RefCell<Vec<Action>>>, hold: Hold) {
    let finger: Rc<std::cell::Cell<Option<i32>>> = Rc::new(std::cell::Cell::new(None));
    {
        let (actions, finger, element) = (actions.clone(), finger.clone(), target.clone());
        listen(target, "pointerdown", move |event: PointerEvent| {
            event.prevent_default();
            event.stop_propagation();
            if finger.get().is_some() || event.button() != 0 {
                return;
            }
            finger.set(Some(event.pointer_id()));
            let _ = element.set_pointer_capture(event.pointer_id());
            actions.borrow_mut().push(Action::Held(hold, true));
        });
    }
    for name in ["pointerup", "pointercancel", "lostpointercapture"] {
        let (actions, finger) = (actions.clone(), finger.clone());
        listen(target, name, move |event: PointerEvent| {
            event.stop_propagation();
            if finger.get() == Some(event.pointer_id()) {
                finger.set(None);
                actions.borrow_mut().push(Action::Held(hold, false));
            }
        });
    }
    listen(target, "click", |event: web_sys::Event| {
        event.prevent_default();
        event.stop_propagation();
    });
}

fn button(document: &Document, label: &str, main: bool) -> HtmlElement {
    let b = element(document, "button");
    let _ = b.set_attribute("type", "button");
    b.set_text_content(Some(label));
    css(&b, &BUTTON);
    no_gestures(&b);
    css(
        &b,
        &[
            ("padding", "10px 20px"),
            ("font-size", "18px"),
            ("background", if main { "#ff8fb1" } else { "#fff6ec" }),
            ("margin", "4px"),
        ],
    );
    b
}

pub struct Hud {
    document: Document,
    actions: Rc<RefCell<Vec<Action>>>,
    playing: HtmlElement,
    health_fill: HtmlElement,
    health_text: HtmlElement,
    key_dots: Vec<HtmlElement>,
    toast: HtmlElement,
    crosshair: HtmlElement,
    prompt: HtmlElement,
    prompt_key: HtmlElement,
    prompt_text: HtmlElement,
    unlock: HtmlElement,
    unlock_fill: HtmlElement,
    /// A hot-bar slot per tool: the button, its number and its count.
    slots: Vec<(HtmlElement, HtmlElement, HtmlElement)>,
    boss: HtmlElement,
    boss_fill: HtmlElement,
    flash: HtmlElement,
    closet: HtmlElement,
    daylight: HtmlElement,
    touch: Option<Touch>,
    card: HtmlElement,
    card_title: HtmlElement,
    card_note: HtmlElement,
    card_lines: HtmlElement,
    card_buttons: HtmlElement,
    card_links: HtmlElement,
    /// The build's version, small at the foot of the menu card.
    card_version: HtmlElement,
    shown: Option<Card>,
    toast_until: f64,
    /// The speedrun clock.
    timer: HtmlElement,
    /// The jumpscare's layers: a red flash and strawberry-syrup splats.
    scare_flash: HtmlElement,
    splats: Vec<HtmlElement>,
    /// Whether the hot bar ends with a Next button.
    has_next: bool,
    /// The top row's layout last set: narrow (a phone held upright) or
    /// not.
    narrow: Option<bool>,
}

/// The touch controls.
pub struct Touch {
    knob: HtmlElement,
    sprint: HtmlElement,
    use_button: HtmlElement,
}

/// Where the syrup splats land and how big they are: left and top in
/// percent of the screen, size in percent of its shorter side, and how
/// long after the lunge each lands (seconds).
/// They ring the screen, so his face stays in the middle.
const SPLATS: [(f32, f32, f32, f32); 9] = [
    (2.0, 4.0, 28.0, 0.0),
    (68.0, 2.0, 30.0, 0.03),
    (78.0, 54.0, 26.0, 0.06),
    (3.0, 58.0, 28.0, 0.04),
    (58.0, 76.0, 18.0, 0.09),
    (88.0, 30.0, 16.0, 0.08),
    (20.0, 80.0, 20.0, 0.11),
    (34.0, 4.0, 14.0, 0.05),
    (40.0, 84.0, 12.0, 0.1),
];

impl Hud {
    pub fn new(document: &Document, parent: &Element, touch: bool, tablet: bool) -> Self {
        let actions = Rc::new(RefCell::new(Vec::new()));
        let root = element(document, "div");
        css(
            &root,
            &[
                ("position", "absolute"),
                ("inset", "0"),
                ("pointer-events", "none"),
                ("font-family", FONT),
                ("color", INK),
                ("user-select", "none"),
                ("-webkit-user-select", "none"),
                ("overflow", "hidden"),
            ],
        );
        no_gestures(&root);
        // A long press is a held button, never a menu.
        listen(&root, "contextmenu", |event: web_sys::Event| {
            event.prevent_default();
        });
        let _ = parent.append_child(&root);

        let flash = element(document, "div");
        css(
            &flash,
            &[
                ("position", "absolute"),
                ("inset", "0"),
                (
                    "box-shadow",
                    "inset 0 0 120px 40px rgba(255, 60, 110, 0.75)",
                ),
                ("opacity", "0"),
            ],
        );
        let _ = root.append_child(&flash);
        let closet = element(document, "div");
        css(
            &closet,
            &[
                ("position", "absolute"),
                ("inset", "0"),
                (
                    "background",
                    "repeating-linear-gradient(180deg, rgba(40, 24, 14, 0.94) 0 34px, rgba(40, 24, 14, 0.35) 34px 46px)",
                ),
                ("display", "none"),
            ],
        );
        let _ = root.append_child(&closet);
        let daylight = element(document, "div");
        css(
            &daylight,
            &[
                ("position", "absolute"),
                ("inset", "0"),
                (
                    "background",
                    "radial-gradient(circle at 70% 25%, #fffbe0 0 8%, #ffe58a 9% 14%, #bfe8ff 15% 100%)",
                ),
                ("opacity", "0"),
            ],
        );
        let _ = root.append_child(&daylight);

        // Everything shown while playing.
        let playing = element(document, "div");
        css(&playing, &[("position", "absolute"), ("inset", "0")]);
        let _ = root.append_child(&playing);

        let health = element(document, "div");
        css(&health, &PANEL);
        css(
            &health,
            &[
                ("left", "calc(12px + env(safe-area-inset-left))"),
                ("top", "calc(12px + env(safe-area-inset-top))"),
                ("display", "flex"),
                ("align-items", "center"),
                ("gap", "8px"),
                ("font-weight", "800"),
            ],
        );
        let label = element(document, "span");
        label.set_text_content(Some(copy::HEALTH));
        let bar = element(document, "div");
        css(
            &bar,
            &[
                ("width", "130px"),
                ("height", "14px"),
                ("border", "2px solid #3a2418"),
                ("border-radius", "8px"),
                ("background", "#fff"),
                ("overflow", "hidden"),
            ],
        );
        let health_fill = element(document, "div");
        css(
            &health_fill,
            &[
                ("height", "100%"),
                ("width", "100%"),
                ("background", "#7edb7a"),
            ],
        );
        let _ = bar.append_child(&health_fill);
        let health_text = element(document, "span");
        css(&health_text, &[("min-width", "30px")]);
        for child in [&label, &bar, &health_text] {
            let _ = health.append_child(child);
        }
        let _ = playing.append_child(&health);

        let keys = element(document, "div");
        css(&keys, &PANEL);
        css(
            &keys,
            &[
                ("left", "calc(12px + env(safe-area-inset-left))"),
                ("top", "calc(60px + env(safe-area-inset-top))"),
                ("display", "flex"),
                ("align-items", "center"),
                ("gap", "6px"),
                ("font-weight", "800"),
            ],
        );
        let keys_label = element(document, "span");
        keys_label.set_text_content(Some(copy::KEYS));
        let _ = keys.append_child(&keys_label);
        let key_dots: Vec<HtmlElement> = (0..3)
            .map(|_| {
                let dot = element(document, "span");
                css(
                    &dot,
                    &[
                        ("width", "16px"),
                        ("height", "16px"),
                        ("border-radius", "50%"),
                        ("border", "2px solid #3a2418"),
                        ("background", "#fff"),
                        ("display", "inline-block"),
                    ],
                );
                let _ = keys.append_child(&dot);
                dot
            })
            .collect();
        let _ = playing.append_child(&keys);

        // The Ice Cream Man's health, once the earned weapons hurt him.
        let boss = element(document, "div");
        css(&boss, &PANEL);
        css(
            &boss,
            &[
                ("left", "50%"),
                ("top", "calc(64px + env(safe-area-inset-top))"),
                ("transform", "translateX(-50%)"),
                ("display", "none"),
                ("align-items", "center"),
                ("gap", "8px"),
                ("font-weight", "800"),
            ],
        );
        let boss_label = element(document, "span");
        boss_label.set_text_content(Some(copy::MAN_HEALTH));
        let boss_bar = element(document, "div");
        css(
            &boss_bar,
            &[
                ("width", "160px"),
                ("height", "12px"),
                ("border", "2px solid #3a2418"),
                ("border-radius", "8px"),
                ("background", "#fff"),
                ("overflow", "hidden"),
            ],
        );
        let boss_fill = element(document, "div");
        css(
            &boss_fill,
            &[
                ("height", "100%"),
                ("width", "100%"),
                ("background", "#ff9db8"),
            ],
        );
        let _ = boss_bar.append_child(&boss_fill);
        let _ = boss.append_child(&boss_label);
        let _ = boss.append_child(&boss_bar);
        let _ = playing.append_child(&boss);

        let menu = button(document, &format!("\u{2261} {}", copy::MENU), false);
        css(
            &menu,
            &[
                ("position", "absolute"),
                ("right", "calc(12px + env(safe-area-inset-right))"),
                ("top", "calc(12px + env(safe-area-inset-top))"),
                ("padding", "6px 14px"),
                ("font-size", "16px"),
                ("margin", "0"),
            ],
        );
        let _ = menu.set_attribute("id", "icecream-menu");
        on_press(&menu, &actions, Action::Menu);
        let _ = playing.append_child(&menu);

        // The speedrun clock, beside the menu button.
        let timer = element(document, "div");
        css(&timer, &PANEL);
        css(
            &timer,
            &[
                ("right", "calc(118px + env(safe-area-inset-right))"),
                ("top", "calc(12px + env(safe-area-inset-top))"),
                ("font-weight", "800"),
                ("font-size", "20px"),
                ("font-variant-numeric", "tabular-nums"),
                ("display", "none"),
            ],
        );
        let _ = timer.set_attribute("id", "icecream-timer");
        let _ = playing.append_child(&timer);

        let toast = element(document, "div");
        css(&toast, &PANEL);
        css(
            &toast,
            &[
                ("left", "50%"),
                ("top", "calc(14px + env(safe-area-inset-top))"),
                ("transform", "translateX(-50%)"),
                ("font-weight", "800"),
                ("font-size", "18px"),
                ("white-space", "nowrap"),
                ("max-width", "70vw"),
                ("overflow", "hidden"),
                ("text-overflow", "ellipsis"),
                ("display", "none"),
            ],
        );
        let _ = playing.append_child(&toast);

        let crosshair = element(document, "div");
        css(
            &crosshair,
            &[
                ("position", "absolute"),
                ("left", "50%"),
                ("top", "50%"),
                ("width", "18px"),
                ("height", "18px"),
                ("margin", "-9px 0 0 -9px"),
                ("border", "3px solid #fff"),
                ("border-radius", "50%"),
                ("box-shadow", "0 0 0 2px #3a2418"),
                ("display", "none"),
            ],
        );
        let _ = playing.append_child(&crosshair);

        let prompt = element(document, "div");
        css(&prompt, &PANEL);
        css(
            &prompt,
            &[
                ("left", "50%"),
                ("bottom", "calc(112px + env(safe-area-inset-bottom))"),
                ("transform", "translateX(-50%)"),
                ("display", "none"),
                ("align-items", "center"),
                ("gap", "8px"),
                ("font-weight", "800"),
                ("white-space", "nowrap"),
            ],
        );
        let prompt_key = element(document, "kbd");
        prompt_key.set_text_content(Some("E"));
        css(
            &prompt_key,
            &[
                ("border", "2px solid #3a2418"),
                ("border-radius", "6px"),
                ("padding", "0 7px"),
                ("background", "#fff"),
                ("font-family", "inherit"),
            ],
        );
        let prompt_text = element(document, "span");
        let unlock = element(document, "div");
        css(
            &unlock,
            &[
                ("width", "90px"),
                ("height", "10px"),
                ("border", "2px solid #3a2418"),
                ("border-radius", "6px"),
                ("background", "#fff"),
                ("overflow", "hidden"),
                ("display", "none"),
            ],
        );
        let unlock_fill = element(document, "div");
        css(
            &unlock_fill,
            &[
                ("height", "100%"),
                ("width", "0%"),
                ("background", "#f7c52b"),
            ],
        );
        let _ = unlock.append_child(&unlock_fill);
        for child in [&prompt_key, &prompt_text, &unlock] {
            let _ = prompt.append_child(child);
        }
        let _ = playing.append_child(&prompt);

        let bar = element(document, "div");
        css(
            &bar,
            &[
                ("position", "absolute"),
                ("left", "50%"),
                ("bottom", "calc(4px + env(safe-area-inset-bottom))"),
                ("transform", "translateX(-50%)"),
                ("display", "flex"),
                ("gap", "8px"),
                // The gaps between slots are the bar's too, not the look
                // area's, so a tap a little off a slot isn't a look.
                ("padding", "10px 6px 4px"),
                ("pointer-events", "auto"),
            ],
        );
        let _ = bar.set_attribute("id", "icecream-hotbar");
        // Swipe along the bar to go through the tools: left for the next,
        // right for the one before.
        {
            let start: Rc<std::cell::Cell<Option<(i32, f32)>>> =
                Rc::new(std::cell::Cell::new(None));
            let down = start.clone();
            listen(&bar, "pointerdown", move |event: PointerEvent| {
                event.prevent_default();
                event.stop_propagation();
                down.set(Some((event.pointer_id(), event.client_x() as f32)));
            });
            let swiped = actions.clone();
            listen(&bar, "pointerup", move |event: PointerEvent| {
                if let Some((id, x)) = start.get()
                    && id == event.pointer_id()
                {
                    start.set(None);
                    let dx = event.client_x() as f32 - x;
                    if dx.abs() >= 36.0 {
                        swiped.borrow_mut().push(Action::Cycle(dx > 0.0));
                    }
                }
            });
        }
        let slots: Vec<(HtmlElement, HtmlElement, HtmlElement)> = Tool::ALL
            .iter()
            .enumerate()
            .map(|(i, tool)| {
                let slot = element(document, "button");
                let _ = slot.set_attribute("type", "button");
                let _ = slot.set_attribute("aria-label", copy::tool_name(*tool));
                css(&slot, &BUTTON);
                css(
                    &slot,
                    &[
                        ("width", if touch { "72px" } else { "84px" }),
                        ("height", "76px"),
                        ("background", "rgba(255, 246, 236, 0.9)"),
                        ("padding", "4px"),
                        ("font-size", "13px"),
                        ("line-height", "1.15"),
                        ("position", "relative"),
                    ],
                );
                let number = element(document, "span");
                number.set_text_content(Some(&(i + 1).to_string()));
                css(
                    &number,
                    &[
                        ("position", "absolute"),
                        ("left", "6px"),
                        ("top", "3px"),
                        ("font-size", "12px"),
                    ],
                );
                let name = element(document, "span");
                name.set_text_content(Some(copy::tool_name(*tool)));
                css(&name, &[("display", "block"), ("margin-top", "12px")]);
                let count = element(document, "span");
                css(
                    &count,
                    &[
                        ("display", "block"),
                        ("font-size", "18px"),
                        ("margin-top", "2px"),
                    ],
                );
                for child in [&number, &name, &count] {
                    let _ = slot.append_child(child);
                }
                no_gestures(&slot);
                for part in [&number, &name, &count] {
                    css(part, &[("pointer-events", "none")]);
                }
                let _ = slot.set_attribute("data-tool", &format!("{tool:?}").to_lowercase());
                on_press(&slot, &actions, Action::Select(*tool));
                let _ = bar.append_child(&slot);
                (slot, number, count)
            })
            .collect();
        // On a touch screen, a Next button at the end of the bar.
        let next = touch.then(|| {
            let next = button(document, copy::NEXT_ITEM, false);
            let _ = next.set_attribute("aria-label", copy::NEXT_ITEM_LABEL);
            let _ = next.set_attribute("id", "icecream-next");
            css(
                &next,
                &[
                    ("width", "64px"),
                    ("height", "76px"),
                    ("padding", "4px"),
                    ("margin", "0"),
                    ("font-size", "15px"),
                    ("background", "#c9f0ff"),
                ],
            );
            on_press(&next, &actions, Action::Cycle(false));
            let _ = bar.append_child(&next);
            next
        });
        let _ = playing.append_child(&bar);

        let touch = touch.then(|| {
            // A tablet's buttons are bigger: thumbs sit further apart.
            let s = if tablet { 1.3_f32 } else { 1.0 };
            let px = |n: f32| format!("{:.0}px", n * s);
            let stick = element(document, "div");
            css(
                &stick,
                &[
                    ("position", "absolute"),
                    (
                        "left",
                        &format!("calc({:.0}px + env(safe-area-inset-left))", 28.0 * s),
                    ),
                    ("bottom", "calc(120px + env(safe-area-inset-bottom))"),
                    ("width", &px(120.0)),
                    ("height", &px(120.0)),
                    ("border-radius", "50%"),
                    ("border", "3px solid rgba(58, 36, 24, 0.6)"),
                    ("background", "rgba(255, 246, 236, 0.35)"),
                ],
            );
            let knob = element(document, "div");
            css(
                &knob,
                &[
                    ("position", "absolute"),
                    ("left", &px(35.0)),
                    ("top", &px(35.0)),
                    ("width", &px(50.0)),
                    ("height", &px(50.0)),
                    ("border-radius", "50%"),
                    ("background", "rgba(255, 143, 177, 0.9)"),
                    ("border", "3px solid #3a2418"),
                ],
            );
            let _ = stick.append_child(&knob);
            let _ = playing.append_child(&stick);
            let round = |label: &str, id: &str, right: f32, bottom: f32, size: f32| {
                let b = button(document, label, false);
                let _ = b.set_attribute("id", id);
                css(
                    &b,
                    &[
                        ("position", "absolute"),
                        (
                            "right",
                            &format!("calc({:.0}px + env(safe-area-inset-right))", right * s),
                        ),
                        (
                            "bottom",
                            &format!(
                                "calc({:.0}px + env(safe-area-inset-bottom))",
                                16.0 + bottom * s
                            ),
                        ),
                        ("width", &px(size)),
                        ("height", &px(size)),
                        ("border-radius", "50%"),
                        ("padding", "0"),
                        ("font-size", "15px"),
                        ("margin", "0"),
                    ],
                );
                let _ = playing.append_child(&b);
                b
            };
            let fire = round(copy::FIRE, "icecream-fire", 24.0, 104.0, 84.0);
            css(&fire, &[("background", "#ff8fb1")]);
            let jump = round(copy::JUMP, "icecream-jump", 118.0, 92.0, 64.0);
            let use_button = round(copy::USE, "icecream-use", 34.0, 200.0, 62.0);
            let sprint = round(copy::SPRINT, "icecream-sprint", 120.0, 170.0, 62.0);
            let view = round(copy::VIEW, "icecream-view", 40.0, 276.0, 50.0);
            css(&view, &[("font-size", "13px")]);
            on_hold(&fire, &actions, Hold::Fire);
            on_hold(&jump, &actions, Hold::Jump);
            on_hold(&use_button, &actions, Hold::Use);
            on_hold(&sprint, &actions, Hold::Sprint);
            on_press(&view, &actions, Action::ToggleView);
            Touch {
                knob,
                sprint,
                use_button,
            }
        });

        let card = element(document, "div");
        css(
            &card,
            &[
                ("position", "absolute"),
                ("inset", "0"),
                ("display", "none"),
                ("align-items", "center"),
                ("justify-content", "center"),
                ("background", "rgba(58, 36, 24, 0.45)"),
                ("pointer-events", "auto"),
            ],
        );
        no_gestures(&card);
        let panel = element(document, "div");
        css(&panel, &PANEL);
        css(
            &panel,
            &[
                ("position", "relative"),
                ("max-width", "min(520px, 88vw)"),
                ("padding", "18px 22px"),
                ("text-align", "center"),
                ("background", "#fff6ec"),
            ],
        );
        let card_title = element(document, "h2");
        css(
            &card_title,
            &[
                ("margin", "0 0 8px"),
                ("font-size", "30px"),
                ("color", "#d94f7c"),
            ],
        );
        let card_note = element(document, "p");
        css(
            &card_note,
            &[
                ("margin", "4px auto 10px"),
                ("font-style", "italic"),
                ("background", "#fffbe0"),
                ("border", "2px dashed #b57b3a"),
                ("padding", "6px 10px"),
                ("display", "inline-block"),
            ],
        );
        let card_lines = element(document, "div");
        css(
            &card_lines,
            &[("font-size", "17px"), ("line-height", "1.4")],
        );
        let card_buttons = element(document, "div");
        css(&card_buttons, &[("margin-top", "12px")]);
        let card_links = element(document, "div");
        css(&card_links, &[("margin-top", "8px"), ("font-size", "15px")]);
        let card_version = element(document, "p");
        css(
            &card_version,
            &[
                ("margin", "10px 0 0"),
                ("font-size", "11px"),
                ("opacity", "0.6"),
                ("display", "none"),
            ],
        );
        let _ = card_version.set_attribute("id", "icecream-version");
        for child in [
            &card_title,
            &card_note,
            &card_lines,
            &card_buttons,
            &card_links,
            &card_version,
        ] {
            let _ = panel.append_child(child);
        }
        let _ = card.append_child(&panel);

        // The jumpscare, over everything but the card: a red flash, then
        // strawberry syrup and melted ice cream splatting the screen. No
        // blood: it's syrup, with pink scoops in it.
        let scare_flash = element(document, "div");
        css(
            &scare_flash,
            &[
                ("position", "absolute"),
                ("inset", "0"),
                (
                    "background",
                    "radial-gradient(circle at 50% 45%, rgba(255, 40, 70, 0.0) 0 25%, rgba(150, 0, 20, 0.85) 75%, rgba(60, 0, 8, 0.97) 100%)",
                ),
                ("opacity", "0"),
            ],
        );
        let _ = scare_flash.set_attribute("id", "icecream-scare");
        let _ = root.append_child(&scare_flash);
        let splats = SPLATS
            .iter()
            .enumerate()
            .map(|(i, (left, top, size, _))| {
                let splat = element(document, "div");
                let radius = [
                    "58% 42% 61% 39% / 47% 55% 45% 53%",
                    "41% 59% 44% 56% / 60% 38% 62% 40%",
                    "63% 37% 52% 48% / 39% 61% 41% 59%",
                ][i % 3];
                css(
                    &splat,
                    &[
                        ("position", "absolute"),
                        ("left", &format!("{left}%")),
                        ("top", &format!("{top}%")),
                        ("width", &format!("{size}vmin")),
                        ("height", &format!("{size}vmin")),
                        ("border-radius", radius),
                        (
                            "background",
                            "radial-gradient(circle at 35% 30%, #ff6f8e 0 12%, #e0102f 30%, #a10018 70%, #6d0010 100%)",
                        ),
                        ("box-shadow", "inset -6px -8px 0 rgba(60, 0, 8, 0.45), 0 0 0 3px rgba(255, 200, 215, 0.25)"),
                        ("opacity", "0"),
                        ("transform", "scale(0.2)"),
                    ],
                );
                // Drips running down from it, and a pink scoop melting in it.
                for (j, (x, h)) in [(22.0, 70.0), (55.0, 110.0), (78.0, 55.0)].iter().enumerate() {
                    if (i + j) % 2 == 1 && j != 1 {
                        continue;
                    }
                    let drip = element(document, "div");
                    css(
                        &drip,
                        &[
                            ("position", "absolute"),
                            ("left", &format!("{x}%")),
                            ("top", "70%"),
                            ("width", "12%"),
                            ("height", &format!("{h}%")),
                            ("border-radius", "0 0 50% 50% / 0 0 30% 30%"),
                            ("background", "linear-gradient(#a10018, #c8102e 80%, #e0102f)"),
                        ],
                    );
                    let _ = splat.append_child(&drip);
                }
                if i % 3 == 0 {
                    let scoop = element(document, "div");
                    css(
                        &scoop,
                        &[
                            ("position", "absolute"),
                            ("left", "30%"),
                            ("top", "26%"),
                            ("width", "34%"),
                            ("height", "30%"),
                            ("border-radius", "50% 50% 40% 40%"),
                            ("background", "#ffc2d4"),
                            ("box-shadow", "inset -4px -4px 0 #f59ab6"),
                        ],
                    );
                    let _ = splat.append_child(&scoop);
                }
                let _ = root.append_child(&splat);
                splat
            })
            .collect();
        let _ = root.append_child(&card);

        Self {
            document: document.clone(),
            actions,
            playing,
            health_fill,
            health_text,
            key_dots,
            boss,
            boss_fill,
            toast,
            crosshair,
            prompt,
            prompt_key,
            prompt_text,
            unlock,
            unlock_fill,
            slots,
            flash,
            closet,
            daylight,
            touch,
            card,
            card_title,
            card_note,
            card_lines,
            card_buttons,
            card_links,
            card_version,
            shown: None,
            toast_until: 0.0,
            timer,
            scare_flash,
            splats,
            has_next: next.is_some(),
            narrow: None,
        }
    }

    /// Lays the top row out for a screen `width` CSS pixels wide. On a
    /// narrow screen (a phone held upright) Health and Menu fill the top
    /// row, so the timer goes under Menu and the boss bar and the message
    /// line drop below both rows; wide, the timer sits beside Menu and the
    /// message line between Health and Menu. Changes only when it flips.
    pub fn layout(&mut self, width: f32) {
        let narrow = width < 640.0;
        if self.narrow == Some(narrow) {
            return;
        }
        self.narrow = Some(narrow);
        let top = |px: u32| format!("calc({px}px + env(safe-area-inset-top))");
        let right = |px: u32| format!("calc({px}px + env(safe-area-inset-right))");
        let (timer_top, timer_right, boss_top, toast_top) = if narrow {
            (64, 12, 112, 160)
        } else {
            (12, 118, 64, 14)
        };
        css(
            &self.timer,
            &[("top", &top(timer_top)), ("right", &right(timer_right))],
        );
        css(&self.boss, &[("top", &top(boss_top))]);
        css(
            &self.toast,
            &[
                ("top", &top(toast_top)),
                ("max-width", if narrow { "88vw" } else { "70vw" }),
                ("white-space", if narrow { "normal" } else { "nowrap" }),
                ("text-align", "center"),
            ],
        );
    }

    /// The speedrun clock's text, `None` to hide it.
    pub fn timer(&self, text: Option<&str>) {
        match text {
            Some(text) => {
                show(&self.timer, true);
                self.timer.set_text_content(Some(text));
            }
            None => show(&self.timer, false),
        }
    }

    /// The jumpscare at `t` seconds after he lunges (`None`: none).
    /// Scary: a red flash and syrup splats landing hard as he hits the
    /// screen. Gentle (`scary` false): pink melt drips in slowly.
    pub fn scare(&self, t: Option<f32>, scary: bool) {
        let Some(t) = t else {
            css(&self.scare_flash, &[("opacity", "0")]);
            for splat in &self.splats {
                css(splat, &[("opacity", "0")]);
            }
            return;
        };
        // He reaches the screen at HIT seconds.
        const HIT: f32 = 0.42;
        let flash = if scary {
            if t < HIT {
                0.0
            } else {
                (0.55 + 0.45 * ((t - HIT) / 0.6).min(1.0)).min(1.0)
            }
        } else {
            0.0
        };
        css(&self.scare_flash, &[("opacity", &format!("{flash:.2}"))]);
        for (splat, (_, _, _, delay)) in self.splats.iter().zip(SPLATS) {
            let (opacity, scale, hue) = if scary {
                let local = t - HIT - delay;
                if local < 0.0 {
                    (0.0, 0.2, 0.0)
                } else {
                    // Lands big and fast, overshoots, settles.
                    let k = (local / 0.09).min(1.0);
                    let scale =
                        0.2 + 1.0 * k + 0.12 * (1.0 - ((local - 0.09) / 0.2).clamp(0.0, 1.0)) * k;
                    (0.95, scale, 0.0)
                }
            } else {
                // Gentle: melted strawberry ice cream, pale pink, fading in.
                let local = (t - delay * 4.0).max(0.0);
                (
                    (local / 1.0).min(0.7),
                    0.7 + 0.3 * (local / 1.0).min(1.0),
                    1.0,
                )
            };
            css(
                splat,
                &[
                    ("opacity", &format!("{opacity:.2}")),
                    ("transform", &format!("scale({scale:.3})")),
                    (
                        "filter",
                        if hue > 0.0 {
                            "saturate(0.35) brightness(1.6)"
                        } else {
                            "none"
                        },
                    ),
                ],
            );
        }
    }

    pub fn take_actions(&self) -> Vec<Action> {
        std::mem::take(&mut *self.actions.borrow_mut())
    }

    pub fn set_playing(&self, visible: bool) {
        show(&self.playing, visible);
    }

    /// The health bar: `health` as a share of full health in percent;
    /// `boosted` while an eaten cone's extra health lasts (gold, and over
    /// 100).
    pub fn health(&self, health: f32, boosted: bool) {
        let percent = format!("{}%", health.clamp(0.0, 100.0).round());
        let colour = if boosted {
            "#f7c52b"
        } else if health > 60.0 {
            "#7edb7a"
        } else if health > 30.0 {
            "#ffd34d"
        } else {
            "#ff5f7e"
        };
        css(
            &self.health_fill,
            &[("width", &percent), ("background", colour)],
        );
        self.health_text
            .set_text_content(Some(&format!("{}", health.max(0.0).round())));
    }

    pub fn keys(&self, count: u32) {
        for (i, dot) in self.key_dots.iter().enumerate() {
            let have = (i as u32) < count;
            css(
                dot,
                &[("background", if have { "#f7c52b" } else { "#fff" })],
            );
        }
    }

    /// The Ice Cream Man's health bar, `None` to hide it.
    pub fn boss(&self, share: Option<f32>) {
        match share {
            Some(share) => {
                css(&self.boss, &[("display", "flex")]);
                let percent = format!("{}%", (share * 100.0).clamp(0.0, 100.0).round());
                css(&self.boss_fill, &[("width", &percent)]);
            }
            None => css(&self.boss, &[("display", "none")]),
        }
    }

    /// The hot bar: a slot for each tool the player has, numbered in
    /// order, with `count`'s text under its name.
    /// The slots narrow to fit `room` pixels across (a small frame, like
    /// a chat card).
    pub fn hot_bar(
        &self,
        selected: Tool,
        owned: &[Tool],
        room: f32,
        count: impl Fn(Tool) -> String,
    ) {
        let width = slot_width(room, owned.len() + usize::from(self.has_next));
        let size = format!("{width}px");
        let font = if width < 70 { "10px" } else { "13px" };
        for (tool, (slot, number, count_text)) in Tool::ALL.iter().zip(&self.slots) {
            let Some(place) = owned.iter().position(|t| t == tool) else {
                show(slot, false);
                continue;
            };
            show(slot, true);
            css(slot, &[("width", &size), ("font-size", font)]);
            number.set_text_content(Some(&(place + 1).to_string()));
            count_text.set_text_content(Some(&count(*tool)));
            let on = *tool == selected;
            css(
                slot,
                &[
                    (
                        "background",
                        if on {
                            "#ffd6e4"
                        } else {
                            "rgba(255, 246, 236, 0.9)"
                        },
                    ),
                    ("transform", if on { "translateY(-6px)" } else { "none" }),
                    ("border-color", if on { "#d94f7c" } else { "#3a2418" }),
                ],
            );
        }
    }

    pub fn crosshair(&self, visible: bool) {
        show(&self.crosshair, visible);
    }

    /// The interact prompt, its key and the front door's unlock bar.
    pub fn prompt(&self, text: Option<&str>, unlock: Option<f32>, touch: bool) {
        match text {
            Some(text) => {
                css(&self.prompt, &[("display", "flex")]);
                self.prompt_text.set_text_content(Some(text));
                show(&self.prompt_key, !touch);
            }
            None => css(&self.prompt, &[("display", "none")]),
        }
        match unlock {
            Some(share) => {
                show(&self.unlock, true);
                css(
                    &self.unlock_fill,
                    &[("width", &format!("{}%", (share * 100.0).clamp(0.0, 100.0)))],
                );
            }
            None => show(&self.unlock, false),
        }
        if let Some(touch) = &self.touch {
            css(
                &touch.use_button,
                &[(
                    "background",
                    if text.is_some() { "#f7c52b" } else { "#fff6ec" },
                )],
            );
        }
    }

    pub fn sprint(&self, on: bool) {
        if let Some(touch) = &self.touch {
            css(
                &touch.sprint,
                &[("background", if on { "#7edb7a" } else { "#fff6ec" })],
            );
        }
    }

    /// The stick's knob, offset by up to its reach.
    pub fn stick(&self, dx: f32, dy: f32) {
        if let Some(touch) = &self.touch {
            css(
                &touch.knob,
                &[("transform", &format!("translate({dx:.0}px, {dy:.0}px)"))],
            );
        }
    }

    pub fn toast(&mut self, text: &str, now: f64) {
        self.toast.set_text_content(Some(text));
        show(&self.toast, true);
        self.toast_until = now + 2.6;
    }

    pub fn tick(&mut self, now: f64, hurt: f32, hidden: bool, daylight: f32) {
        if self.toast_until > 0.0 && now > self.toast_until {
            show(&self.toast, false);
            self.toast_until = 0.0;
        }
        css(
            &self.flash,
            &[("opacity", &format!("{:.2}", hurt.clamp(0.0, 1.0)))],
        );
        show(&self.closet, hidden);
        css(
            &self.daylight,
            &[("opacity", &format!("{:.2}", daylight.clamp(0.0, 1.0)))],
        );
    }

    /// Shows `card`, or hides the card. Redraws only on a change.
    pub fn card(&mut self, card: Option<Card>) {
        if self.shown == card {
            return;
        }
        match &card {
            None => css(&self.card, &[("display", "none")]),
            Some(card) => {
                css(&self.card, &[("display", "flex")]);
                let background = if card.sunny {
                    "rgba(191, 232, 255, 0.35)"
                } else if card.scooped {
                    "rgba(217, 160, 91, 0.6)"
                } else {
                    "rgba(58, 36, 24, 0.45)"
                };
                css(&self.card, &[("background", background)]);
                self.card_title.set_text_content(Some(&card.title));
                match &card.note {
                    Some(note) => {
                        self.card_note.set_text_content(Some(note));
                        css(&self.card_note, &[("display", "inline-block")]);
                    }
                    None => show(&self.card_note, false),
                }
                match &card.version {
                    Some(version) => {
                        self.card_version.set_text_content(Some(version));
                        show(&self.card_version, true);
                    }
                    None => show(&self.card_version, false),
                }
                self.card_lines.set_text_content(None);
                for line in &card.lines {
                    let p = element(&self.document, "p");
                    css(&p, &[("margin", "4px 0")]);
                    p.set_text_content(Some(line));
                    let _ = self.card_lines.append_child(&p);
                }
                self.card_buttons.set_text_content(None);
                let mut first = None;
                for (label, action, main) in &card.buttons {
                    let b = button(&self.document, label, *main);
                    on_click(&b, &self.actions, *action);
                    let _ = self.card_buttons.append_child(&b);
                    if *main && first.is_none() {
                        first = Some(b);
                    }
                }
                self.card_links.set_text_content(None);
                for (label, href) in &card.links {
                    let a = element(&self.document, "a");
                    let _ = a.set_attribute("href", href);
                    a.set_text_content(Some(label));
                    css(
                        &a,
                        &[
                            ("color", "#d94f7c"),
                            ("font-weight", "800"),
                            ("margin", "0 10px"),
                        ],
                    );
                    let _ = self.card_links.append_child(&a);
                }
                if let Some(first) = first {
                    let _ = first.focus();
                }
            }
        }
        self.shown = card;
    }
}
