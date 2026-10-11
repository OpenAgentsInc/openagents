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
    SprintToggle,
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

fn on_click(target: &HtmlElement, actions: &Rc<RefCell<Vec<Action>>>, action: Action) {
    let actions = actions.clone();
    let closure = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        event.prevent_default();
        event.stop_propagation();
        actions.borrow_mut().push(action);
    });
    let _ = target.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
    closure.forget();
}

/// A button held down by a finger: down and up (or the finger leaving).
fn on_hold(target: &HtmlElement, actions: &Rc<RefCell<Vec<Action>>>, hold: Hold) {
    for (name, down) in [
        ("pointerdown", true),
        ("pointerup", false),
        ("pointercancel", false),
        ("pointerleave", false),
    ] {
        let actions = actions.clone();
        let closure = Closure::<dyn FnMut(PointerEvent)>::new(move |event: PointerEvent| {
            event.prevent_default();
            event.stop_propagation();
            actions.borrow_mut().push(Action::Held(hold, down));
        });
        let _ = target.add_event_listener_with_callback(name, closure.as_ref().unchecked_ref());
        closure.forget();
    }
}

fn button(document: &Document, label: &str, main: bool) -> HtmlElement {
    let b = element(document, "button");
    let _ = b.set_attribute("type", "button");
    b.set_text_content(Some(label));
    css(&b, &BUTTON);
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
    slots: Vec<(HtmlElement, HtmlElement)>,
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
    shown: Option<Card>,
    toast_until: f64,
}

/// The touch controls.
pub struct Touch {
    knob: HtmlElement,
    sprint: HtmlElement,
    use_button: HtmlElement,
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
                ("font-family", FONT),
                ("color", INK),
                ("user-select", "none"),
                ("-webkit-user-select", "none"),
                ("overflow", "hidden"),
            ],
        );
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
                ("left", "12px"),
                ("top", "12px"),
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
                ("left", "12px"),
                ("top", "60px"),
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

        let menu = button(document, &format!("\u{2261} {}", copy::MENU), false);
        css(
            &menu,
            &[
                ("position", "absolute"),
                ("right", "12px"),
                ("top", "12px"),
                ("padding", "6px 14px"),
                ("font-size", "16px"),
                ("margin", "0"),
            ],
        );
        on_click(&menu, &actions, Action::Menu);
        let _ = playing.append_child(&menu);

        let toast = element(document, "div");
        css(&toast, &PANEL);
        css(
            &toast,
            &[
                ("left", "50%"),
                ("top", "14px"),
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
                ("bottom", "108px"),
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
                ("bottom", "14px"),
                ("transform", "translateX(-50%)"),
                ("display", "flex"),
                ("gap", "8px"),
            ],
        );
        let slots: Vec<(HtmlElement, HtmlElement)> = Tool::ALL
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
                        ("width", if touch { "84px" } else { "96px" }),
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
                on_click(&slot, &actions, Action::Select(*tool));
                let _ = bar.append_child(&slot);
                (slot, count)
            })
            .collect();
        let _ = playing.append_child(&bar);

        let touch = touch.then(|| {
            let stick = element(document, "div");
            css(
                &stick,
                &[
                    ("position", "absolute"),
                    ("left", "28px"),
                    ("bottom", "110px"),
                    ("width", "120px"),
                    ("height", "120px"),
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
                    ("left", "35px"),
                    ("top", "35px"),
                    ("width", "50px"),
                    ("height", "50px"),
                    ("border-radius", "50%"),
                    ("background", "rgba(255, 143, 177, 0.9)"),
                    ("border", "3px solid #3a2418"),
                ],
            );
            let _ = stick.append_child(&knob);
            let _ = playing.append_child(&stick);
            let round = |label: &str, right: &str, bottom: &str, size: &str| {
                let b = button(document, label, false);
                css(
                    &b,
                    &[
                        ("position", "absolute"),
                        ("right", right),
                        ("bottom", bottom),
                        ("width", size),
                        ("height", size),
                        ("border-radius", "50%"),
                        ("padding", "0"),
                        ("font-size", "15px"),
                        ("margin", "0"),
                        ("touch-action", "none"),
                    ],
                );
                let _ = playing.append_child(&b);
                b
            };
            let fire = round(copy::FIRE, "24px", "120px", "84px");
            css(&fire, &[("background", "#ff8fb1")]);
            let jump = round(copy::JUMP, "118px", "104px", "64px");
            let use_button = round(copy::USE, "34px", "216px", "62px");
            let sprint = round(copy::SPRINT, "120px", "186px", "58px");
            on_hold(&fire, &actions, Hold::Fire);
            on_hold(&jump, &actions, Hold::Jump);
            on_hold(&use_button, &actions, Hold::Use);
            on_click(&sprint, &actions, Action::SprintToggle);
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
        for child in [
            &card_title,
            &card_note,
            &card_lines,
            &card_buttons,
            &card_links,
        ] {
            let _ = panel.append_child(child);
        }
        let _ = card.append_child(&panel);
        let _ = root.append_child(&card);

        Self {
            document: document.clone(),
            actions,
            playing,
            health_fill,
            health_text,
            key_dots,
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
            shown: None,
            toast_until: 0.0,
        }
    }

    pub fn take_actions(&self) -> Vec<Action> {
        std::mem::take(&mut *self.actions.borrow_mut())
    }

    pub fn set_playing(&self, visible: bool) {
        show(&self.playing, visible);
    }

    pub fn health(&self, health: f32) {
        let percent = format!("{}%", health.clamp(0.0, 100.0).round());
        let colour = if health > 60.0 {
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

    pub fn hot_bar(&self, selected: Tool, fudge: u32, farts: u32, keys: u32) {
        for (tool, (slot, count)) in Tool::ALL.iter().zip(&self.slots) {
            let text = match tool {
                Tool::Fudge => fudge.to_string(),
                Tool::Fart => farts.to_string(),
                Tool::Keys => format!("{keys}/3"),
            };
            count.set_text_content(Some(&text));
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
