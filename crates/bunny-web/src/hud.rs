//! The page's HUD and cards, drawn as plain elements over the canvas from
//! the game's HUD elements (`verse_game::HudElement`): food left with a
//! ring, the size pips and growth bar, the score and munch-chain
//! multiplier, the power-up ring, an arrow toward the farmer, the pause and
//! turn-back buttons, and a card for titles, pauses and results. Buttons
//! queue an [`Action`] the game reads each frame.

use std::cell::RefCell;
use std::rc::Rc;

use verse_game::{HudElement, Icon};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{Document, Element, HtmlElement};

use crate::copy;

/// What a button asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Play garden N (1-based).
    Play(usize),
    Pause,
    Resume,
    Restart,
    Leave,
    /// The phone's turn-back button.
    TurnBack,
    Close,
}

/// A card's contents.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Card {
    pub title: String,
    pub swatch: Option<u32>,
    pub lines: Vec<String>,
    pub hint: Option<String>,
    /// Buttons: label, action, and whether it is the main one.
    pub buttons: Vec<(String, Action, bool)>,
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

fn hex(colour: u32) -> String {
    format!("#{colour:06x}")
}

const INK: &str = "#1e1e1e";
const PANEL: [(&str, &str); 5] = [
    ("position", "absolute"),
    ("background", "rgba(244, 244, 242, 0.92)"),
    ("border", "2px solid #1e1e1e"),
    ("border-radius", "12px"),
    ("padding", "8px 12px"),
];
const BUTTON: [(&str, &str); 7] = [
    ("font", "inherit"),
    ("font-weight", "650"),
    ("color", "#1e1e1e"),
    ("border", "2px solid #1e1e1e"),
    ("border-radius", "999px"),
    ("cursor", "pointer"),
    ("pointer-events", "auto"),
];

/// The colour of a power-up's ring.
fn power_colour(icon: Icon) -> &'static str {
    match icon {
        Icon::Power(0) => "#f2c230",
        Icon::Power(1) => "#2fb15a",
        Icon::Power(2) => "#e8d9a0",
        Icon::Power(3) => "#f2cf55",
        _ => "#b02cc0",
    }
}

pub struct Hud {
    pub root: HtmlElement,
    game: HtmlElement,
    food: HtmlElement,
    food_ring: HtmlElement,
    tier: HtmlElement,
    pips: Vec<HtmlElement>,
    bar: HtmlElement,
    score: HtmlElement,
    multiplier: HtmlElement,
    powers: HtmlElement,
    arrow: HtmlElement,
    arrow_mark: HtmlElement,
    pause: HtmlElement,
    turn_back: HtmlElement,
    card: HtmlElement,
    card_title: HtmlElement,
    card_swatch: HtmlElement,
    card_lines: HtmlElement,
    card_hint: HtmlElement,
    card_buttons: HtmlElement,
    shown_card: Option<Card>,
    actions: Rc<RefCell<Vec<Action>>>,
    touch: bool,
}

fn on_click(target: &HtmlElement, actions: &Rc<RefCell<Vec<Action>>>, action: Action) {
    let actions = actions.clone();
    let closure = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        event.stop_propagation();
        actions.borrow_mut().push(action);
    });
    let _ = target.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
    // Buttons on the canvas mustn't start a swipe.
    let stop = Closure::<dyn FnMut(web_sys::Event)>::new(|event: web_sys::Event| {
        event.stop_propagation();
    });
    let _ = target.add_event_listener_with_callback("pointerdown", stop.as_ref().unchecked_ref());
    closure.forget();
    stop.forget();
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
        let game = element(document, "div");
        css(&game, &[("position", "absolute"), ("inset", "0")]);
        // Top left: food left and size.
        let stats = element(document, "div");
        css(&stats, &PANEL);
        css(
            &stats,
            &[
                ("top", "calc(12px + env(safe-area-inset-top))"),
                ("left", "calc(12px + env(safe-area-inset-left))"),
                ("min-width", "116px"),
            ],
        );
        let food_row = element(document, "div");
        css(
            &food_row,
            &[
                ("display", "flex"),
                ("align-items", "center"),
                ("gap", "8px"),
            ],
        );
        let food_ring = element(document, "div");
        css(
            &food_ring,
            &[
                ("width", "30px"),
                ("height", "30px"),
                ("border-radius", "50%"),
                ("border", "2px solid #1e1e1e"),
                ("box-sizing", "border-box"),
                ("display", "flex"),
                ("align-items", "center"),
                ("justify-content", "center"),
            ],
        );
        let sprout = element(document, "div");
        css(
            &sprout,
            &[
                ("width", "12px"),
                ("height", "12px"),
                ("border-radius", "50% 0"),
                ("background", "#58b947"),
                ("border", "2px solid #1e1e1e"),
            ],
        );
        let _ = food_ring.append_child(&sprout);
        let food = element(document, "div");
        css(
            &food,
            &[
                ("font-size", "28px"),
                ("font-weight", "750"),
                ("line-height", "1"),
                ("color", "#c86400"),
                ("font-variant-numeric", "tabular-nums"),
            ],
        );
        food.set_attribute("aria-label", copy::CARROTS).ok();
        let _ = food_row.append_child(&food_ring);
        let _ = food_row.append_child(&food);
        let tier = element(document, "div");
        css(
            &tier,
            &[
                ("font-size", "14px"),
                ("font-weight", "650"),
                ("margin-top", "6px"),
            ],
        );
        let pip_row = element(document, "div");
        css(
            &pip_row,
            &[("display", "flex"), ("gap", "4px"), ("margin-top", "4px")],
        );
        let pips: Vec<HtmlElement> = (0..5)
            .map(|_| {
                let pip = element(document, "span");
                css(
                    &pip,
                    &[
                        ("width", "14px"),
                        ("height", "14px"),
                        ("border", "2px solid #1e1e1e"),
                        ("border-radius", "50%"),
                        ("box-sizing", "border-box"),
                    ],
                );
                let _ = pip_row.append_child(&pip);
                pip
            })
            .collect();
        let bar_frame = element(document, "div");
        css(
            &bar_frame,
            &[
                ("height", "6px"),
                ("margin-top", "5px"),
                ("border", "2px solid #1e1e1e"),
                ("border-radius", "4px"),
                ("overflow", "hidden"),
            ],
        );
        let bar = element(document, "div");
        css(&bar, &[("height", "100%"), ("background", "#1e1e1e")]);
        let _ = bar_frame.append_child(&bar);
        let powers = element(document, "div");
        css(
            &powers,
            &[("display", "flex"), ("gap", "6px"), ("margin-top", "6px")],
        );
        for child in [&food_row, &tier, &pip_row, &bar_frame, &powers] {
            let _ = stats.append_child(child);
        }
        // Top right: score, then pause.
        let score_box = element(document, "div");
        css(&score_box, &PANEL);
        css(
            &score_box,
            &[
                ("top", "calc(12px + env(safe-area-inset-top))"),
                ("right", "calc(70px + env(safe-area-inset-right))"),
                ("text-align", "right"),
                ("min-width", "70px"),
            ],
        );
        let score = element(document, "div");
        css(
            &score,
            &[
                ("font-size", "22px"),
                ("font-weight", "750"),
                ("font-variant-numeric", "tabular-nums"),
            ],
        );
        let multiplier = element(document, "div");
        css(
            &multiplier,
            &[
                ("font-size", "14px"),
                ("font-weight", "700"),
                ("color", "#c86400"),
            ],
        );
        let _ = score_box.append_child(&score);
        let _ = score_box.append_child(&multiplier);
        let pause = element(document, "button");
        pause.set_text_content(Some("II"));
        pause.set_attribute("aria-label", copy::PAUSE).ok();
        css(&pause, &BUTTON);
        css(
            &pause,
            &[
                ("position", "absolute"),
                ("top", "calc(12px + env(safe-area-inset-top))"),
                ("right", "calc(12px + env(safe-area-inset-right))"),
                ("width", "48px"),
                ("height", "48px"),
                ("font-size", "16px"),
                ("background", "rgba(244, 244, 242, 0.92)"),
            ],
        );
        on_click(&pause, &actions, Action::Pause);
        // Bottom centre on phones: turn back.
        let turn_back = element(document, "button");
        turn_back.set_text_content(Some("\u{21b6}"));
        turn_back.set_attribute("aria-label", copy::TURN_BACK).ok();
        css(&turn_back, &BUTTON);
        css(
            &turn_back,
            &[
                ("position", "absolute"),
                ("bottom", "calc(20px + env(safe-area-inset-bottom))"),
                ("left", "50%"),
                ("transform", "translateX(-50%)"),
                ("width", "64px"),
                ("height", "64px"),
                ("font-size", "30px"),
                ("background", "rgba(244, 244, 242, 0.92)"),
            ],
        );
        on_click(&turn_back, &actions, Action::TurnBack);
        // The farmer's arrow.
        let arrow = element(document, "div");
        css(
            &arrow,
            &[
                ("position", "absolute"),
                ("width", "0"),
                ("height", "0"),
                ("transition", "opacity 0.2s"),
            ],
        );
        let arrow_head = element(document, "div");
        css(
            &arrow_head,
            &[
                ("position", "absolute"),
                ("left", "-14px"),
                ("top", "-14px"),
                ("width", "28px"),
                ("height", "28px"),
                ("font-size", "26px"),
                ("line-height", "28px"),
                ("text-align", "center"),
                ("font-weight", "900"),
                ("color", "#1e1e1e"),
            ],
        );
        arrow_head.set_text_content(Some("\u{25b2}"));
        let arrow_mark = element(document, "div");
        css(
            &arrow_mark,
            &[
                ("position", "absolute"),
                ("left", "-12px"),
                ("top", "16px"),
                ("width", "24px"),
                ("text-align", "center"),
                ("font-size", "20px"),
                ("font-weight", "900"),
            ],
        );
        let _ = arrow.append_child(&arrow_head);
        let _ = arrow.append_child(&arrow_mark);
        for child in [&stats, &score_box, &pause, &turn_back, &arrow] {
            let _ = game.append_child(child);
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
                ("width", "min(88vw, 420px)"),
                ("max-height", "86vh"),
                ("overflow-y", "auto"),
                ("box-sizing", "border-box"),
                ("padding", "20px 22px"),
                ("text-align", "center"),
                ("pointer-events", "auto"),
                ("background", "rgba(244, 244, 242, 0.97)"),
            ],
        );
        let card_title = element(document, "h1");
        css(
            &card_title,
            &[
                ("margin", "0 0 6px"),
                ("font-size", "27px"),
                ("line-height", "1.15"),
            ],
        );
        let card_swatch = element(document, "div");
        css(
            &card_swatch,
            &[
                ("width", "34px"),
                ("height", "34px"),
                ("margin", "8px auto"),
                ("border", "3px solid #1e1e1e"),
                ("border-radius", "50%"),
            ],
        );
        let card_lines = element(document, "div");
        css(
            &card_lines,
            &[("font-size", "16px"), ("line-height", "1.4")],
        );
        let card_hint = element(document, "p");
        css(
            &card_hint,
            &[
                ("margin", "10px 0 0"),
                ("font-size", "14px"),
                ("color", "#55554f"),
            ],
        );
        let card_buttons = element(document, "div");
        css(
            &card_buttons,
            &[
                ("display", "flex"),
                ("flex-wrap", "wrap"),
                ("justify-content", "center"),
                ("gap", "8px"),
                ("margin-top", "14px"),
            ],
        );
        for child in [
            &card_title,
            &card_swatch,
            &card_lines,
            &card_hint,
            &card_buttons,
        ] {
            let _ = card.append_child(child);
        }
        let _ = root.append_child(&game);
        let _ = root.append_child(&card);
        let _ = parent.append_child(&root);
        let hud = Self {
            root,
            game,
            food,
            food_ring,
            tier,
            pips,
            bar,
            score,
            multiplier,
            powers,
            arrow,
            arrow_mark,
            pause,
            turn_back,
            card,
            card_title,
            card_swatch,
            card_lines,
            card_hint,
            card_buttons,
            shown_card: Some(Card::default()),
            actions,
            touch,
        };
        show(&hud.turn_back, touch);
        hud
    }

    /// The actions buttons queued since the last call.
    pub fn take_actions(&self) -> Vec<Action> {
        self.actions.borrow_mut().drain(..).collect()
    }

    /// Shows the in-garden HUD or hides it.
    pub fn show_game(&self, visible: bool) {
        show(&self.game, visible);
    }

    /// Shows the HUD's pause and turn-back buttons or hides them, as during
    /// a run in a garden.
    pub fn show_run_buttons(&self, visible: bool) {
        show(&self.pause, visible);
        show(&self.turn_back, visible && self.touch);
    }

    /// Draws the game's HUD elements.
    pub fn update(&self, elements: &[HudElement], width: f32, height: f32) {
        let mut arrow = false;
        let mut powers = Vec::new();
        for element in elements {
            match element {
                HudElement::Counter { value, ring, .. } => {
                    self.food.set_text_content(Some(&value.to_string()));
                    let done = ring.unwrap_or(0.0).clamp(0.0, 1.0) * 360.0;
                    css(
                        &self.food_ring,
                        &[(
                            "background",
                            &format!("conic-gradient(#f28a1e {done}deg, #e4e4e0 0)"),
                        )],
                    );
                }
                HudElement::Pips {
                    filled, bar, label, ..
                } => {
                    self.tier.set_text_content(Some(label));
                    for (index, pip) in self.pips.iter().enumerate() {
                        let on = index < usize::from(*filled);
                        css(pip, &[("background", if on { INK } else { "transparent" })]);
                    }
                    css(&self.bar, &[("width", &format!("{:.0}%", bar * 100.0))]);
                }
                HudElement::Score { value, multiplier } => {
                    self.score.set_text_content(Some(&value.to_string()));
                    let text = multiplier.map(|m| format!("\u{d7}{m}")).unwrap_or_default();
                    self.multiplier.set_text_content(Some(&text));
                }
                HudElement::Ring { icon, left } => powers.push((*icon, *left)),
                HudElement::EdgeArrow {
                    angle,
                    weight,
                    mark,
                } => {
                    // Shown when he is out of the view ahead.
                    if angle.abs() > 0.55 || *mark == Some(Icon::Danger) && angle.abs() > 0.35 {
                        arrow = true;
                        let (sx, cy) = (angle.sin(), angle.cos());
                        let reach = (width * 0.42).min(height * 0.4);
                        let x = width / 2.0 + sx * reach;
                        let y = height * 0.55 - cy * reach;
                        let scale = 0.8 + 0.7 * weight;
                        css(
                            &self.arrow,
                            &[
                                ("left", &format!("{x:.0}px")),
                                ("top", &format!("{y:.0}px")),
                                (
                                    "transform",
                                    &format!("rotate({angle:.3}rad) scale({scale:.2})"),
                                ),
                                ("opacity", &format!("{:.2}", 0.45 + 0.55 * weight)),
                            ],
                        );
                        let text = match mark {
                            Some(Icon::Danger) => "!",
                            Some(Icon::Search) => "?",
                            _ => "",
                        };
                        self.arrow_mark.set_text_content(Some(text));
                    }
                }
                HudElement::Map { .. } => {}
            }
        }
        show(&self.arrow, arrow);
        // Power-up rings, rebuilt only when their count changes.
        let children = self.powers.children();
        if children.length() as usize != powers.len() {
            self.powers.set_inner_html("");
            let document = web_sys::window()
                .and_then(|w| w.document())
                .expect("a document");
            for _ in &powers {
                let ring = element(&document, "div");
                css(
                    &ring,
                    &[
                        ("width", "22px"),
                        ("height", "22px"),
                        ("border-radius", "50%"),
                        ("border", "2px solid #1e1e1e"),
                        ("box-sizing", "border-box"),
                    ],
                );
                let _ = self.powers.append_child(&ring);
            }
        }
        for (index, (icon, left)) in powers.iter().enumerate() {
            if let Some(ring) = self
                .powers
                .children()
                .item(index as u32)
                .and_then(|e| e.dyn_into::<HtmlElement>().ok())
            {
                let deg = left.clamp(0.0, 1.0) * 360.0;
                css(
                    &ring,
                    &[(
                        "background",
                        &format!(
                            "conic-gradient({} {deg}deg, transparent 0)",
                            power_colour(*icon)
                        ),
                    )],
                );
            }
        }
    }

    /// Shows `card`, or hides the card with `None`; rebuilt only when it
    /// changes.
    pub fn card(&mut self, card: Option<Card>) {
        if self.shown_card == card {
            return;
        }
        show(&self.card, card.is_some());
        if let Some(content) = &card {
            self.card_title.set_text_content(Some(&content.title));
            show(&self.card_swatch, content.swatch.is_some());
            if let Some(colour) = content.swatch {
                css(&self.card_swatch, &[("background", &hex(colour))]);
            }
            self.card_lines.set_inner_html("");
            let document = web_sys::window()
                .and_then(|w| w.document())
                .expect("a document");
            for line in &content.lines {
                let p = element(&document, "p");
                css(&p, &[("margin", "0 0 4px")]);
                p.set_text_content(Some(line));
                let _ = self.card_lines.append_child(&p);
            }
            self.card_hint
                .set_text_content(Some(content.hint.as_deref().unwrap_or("")));
            show(&self.card_hint, content.hint.is_some());
            self.card_buttons.set_inner_html("");
            for (label, action, main) in &content.buttons {
                let button = element(&document, "button");
                button.set_text_content(Some(label));
                css(&button, &BUTTON);
                css(
                    &button,
                    &[
                        ("padding", if *main { "10px 26px" } else { "8px 16px" }),
                        ("font-size", if *main { "18px" } else { "15px" }),
                        ("background", if *main { "#f28a1e" } else { "#f4f4f2" }),
                    ],
                );
                on_click(&button, &self.actions, *action);
                let _ = self.card_buttons.append_child(&button);
            }
            // Focus the main button so Enter and Space press it.
            if let Some(first) = self
                .card_buttons
                .first_element_child()
                .and_then(|e| e.dyn_into::<HtmlElement>().ok())
            {
                let _ = first.focus();
            }
        }
        self.shown_card = card;
    }
}
