//! The page: the Grid scene on its canvas beside a chat transcript of the
//! round, in the site's own chat classes (`openagents-ui`): your message
//! as a user bubble, each step as a tool-call card with its raw data, the
//! answer as an assistant message, and the result.
//!
//! The live flow (`flow`) reports through [`Show`]. Reports go through the
//! timeline (`steps::Player`) so each step is seen for long enough, and the
//! cards, the answer and the result change as the scene reaches them.
//! Without WebGL2 the page still works: the canvas is hidden and the
//! transcript updates.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;
use web_sys::{Document, Element, HtmlElement, HtmlInputElement, Window};

use crate::bubble;
use crate::copy;
use crate::gl::Renderer;
use crate::icons;
use crate::scene;
use crate::steps::{self, Event, Player, ms_label};

pub use crate::bubble::Party;
pub use crate::steps::{Lane, RunOptions, State, Step, Tamper};
use serde_json::Value;

/// A padlock with a cross: the relay can't open what it carries.
const LOCK_X: &str = "<svg viewBox=\"0 0 16 16\" width=\"14\" height=\"14\" aria-hidden=\"true\"><path d=\"M4.5 7V5a3.5 3.5 0 0 1 7 0v2\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.5\"/><rect x=\"2.5\" y=\"7\" width=\"11\" height=\"8\" rx=\"1.5\" fill=\"currentColor\"/><path d=\"M5.5 9.2l5 4M10.5 9.2l-5 4\" stroke=\"#000\" stroke-width=\"1.6\"/></svg>";

/// Values longer than this many characters are cut in the middle until
/// tapped.
const VALUE_MAX: usize = 44;

/// A report that waits its turn behind the step animations.
enum Later {
    Panel(Step, String, Vec<(String, String)>),
    Answer(String, String),
    Verdict(bool, String, String),
    Lit(Option<bool>),
    Idle,
    Say(Party, String),
    Bubble(Party, String, Option<String>, Value, String),
}

struct Dom {
    document: Document,
    /// One tool-call card per step.
    cards: Vec<Element>,
    user: Option<Element>,
    answer: Element,
    verdict: Option<Element>,
    status: Option<Element>,
    run: Option<HtmlElement>,
    labels: Vec<HtmlElement>,
    /// The tag that follows the travelling shard.
    tag: Option<HtmlElement>,
    /// The stage the scene draws on, and the bubbles over it: one per
    /// party, floated over the scene, or stacked in a strip under it on
    /// phones and without WebGL2.
    stage: Option<Element>,
    bubble_box: Element,
    bubbles: Vec<HtmlElement>,
    narrow: Option<web_sys::MediaQueryList>,
    strip: Option<bool>,
    /// Who answers, for the answer's author.
    provider: String,
}

struct Inner {
    player: Player<Later>,
    lit: Option<bool>,
    motion: bool,
    running: bool,
    /// The message the visitor last sent, for its bubble.
    prompt: Option<String>,
    dom: Dom,
    renderer: Option<Renderer>,
}

/// What runs when the visitor presses Run.
type OnRun = Rc<RefCell<Option<Box<dyn Fn(RunOptions)>>>>;

/// The page's scene and transcript.
pub struct Show {
    inner: Rc<RefCell<Inner>>,
    run: OnRun,
}

fn now(window: &Window) -> f64 {
    window.performance().map_or(0.0, |p| p.now()) / 1000.0
}

fn make(document: &Document, tag: &str, class: &str) -> Element {
    let element = document.create_element(tag).expect("an element");
    if !class.is_empty() {
        element.set_class_name(class);
    }
    element
}

fn text(document: &Document, tag: &str, class: &str, words: &str) -> Element {
    let element = make(document, tag, class);
    element.set_text_content(Some(words));
    element
}

fn listen<E: wasm_bindgen::convert::FromWasmAbi + 'static>(
    target: &web_sys::EventTarget,
    name: &str,
    handler: impl FnMut(E) + 'static,
) {
    let closure = Closure::<dyn FnMut(E)>::new(handler);
    let _ = target.add_event_listener_with_callback(name, closure.as_ref().unchecked_ref());
    closure.forget();
}

fn child(parent: &Element, selector: &str) -> Option<Element> {
    parent.query_selector(selector).ok().flatten()
}

fn icon(step: Step) -> &'static str {
    match step {
        Step::Fetch => icons::DOWNLOAD,
        Step::Chain => icons::CERTIFICATE,
        Step::Measure => icons::COMPARE,
        Step::Bind => icons::KEY,
        Step::Encrypt => icons::LOCK,
        Step::Relay => icons::SEND,
        Step::Decrypt => icons::SHIELD_LOCK,
        Step::Answer => icons::REPLY,
        Step::Receipt => icons::SHIELD_CHECK,
    }
}

/// A chat message: `article.oa-message[data-role]` with its author for
/// screen readers.
fn message(document: &Document, role: &str, author: &str) -> Element {
    let article = make(document, "article", "oa-message");
    let _ = article.set_attribute("data-role", role);
    let _ = article.append_child(&text(
        document,
        "h2",
        "oa-message-author oa-visually-hidden",
        author,
    ));
    article
}

/// A step's tool-call card, as the chat draws one: a `<details>` whose
/// summary has an icon, the step's name, a mono detail and a status badge,
/// and whose body has the step's plain line and its raw data.
fn card(document: &Document, step: Step) -> Element {
    let card = make(document, "details", "oa-tool-call");
    let _ = card.set_attribute("data-status", "waiting");
    let _ = card.set_attribute("data-step", step.id());
    let summary = make(document, "summary", "oa-tool-call__summary");
    let glyph = make(document, "span", "oa-tool-call__icon");
    let _ = glyph.set_attribute("aria-hidden", "true");
    glyph.set_inner_html(&icons::svg(icon(step)));
    let _ = summary.append_child(&glyph);
    let _ = summary.append_child(&text(
        document,
        "span",
        "oa-tool-call__title",
        copy::name(step),
    ));
    let _ = summary.append_child(&make(document, "code", "oa-tool-call__detail"));
    let status = make(document, "span", "oa-tool-call__status");
    let badge = make(document, "span", "oa-badge");
    let _ = status.append_child(&badge);
    let _ = summary.append_child(&status);
    let chevron = make(document, "span", "oa-tool-call__chevron");
    let _ = chevron.set_attribute("aria-hidden", "true");
    chevron.set_inner_html(&icons::svg(icons::CHEVRON_DOWN));
    let _ = summary.append_child(&chevron);
    let body = make(document, "div", "oa-tool-call__body");
    let _ = body.append_child(&text(document, "p", "att-line", copy::line(step)));
    let _ = body.append_child(&make(document, "p", "att-reason"));
    let _ = body.append_child(&make(document, "p", "att-panel-title"));
    let _ = body.append_child(&make(document, "dl", "att-kv"));
    let _ = card.append_child(&summary);
    let _ = card.append_child(&body);
    badge_for(&card, &State::Pending);
    card
}

/// Sets a card's status and badge.
fn badge_for(card: &Element, state: &State) {
    let (status, colour) = match state {
        State::Pending | State::Skipped => ("waiting", "secondary"),
        State::Running => ("running", "info"),
        State::Ok => ("done", "success"),
        State::Refused(_) => ("failed", "danger"),
    };
    let _ = card.set_attribute("data-status", status);
    if let Some(badge) = child(card, ".oa-badge") {
        let _ = badge.set_attribute("data-color", colour);
        let _ = badge.set_attribute("data-size", "sm");
        let _ = badge.set_attribute("data-variant", "soft");
        badge.set_text_content(Some(copy::state_word(state)));
    }
}

impl Show {
    /// Finds the page's parts (`#att-canvas`, `#att-steps`, `#att-form`),
    /// builds the step cards and labels, and starts the loop. `None` when
    /// the page lacks them.
    pub fn mount() -> Option<Rc<Self>> {
        let window = web_sys::window()?;
        let document = window.document()?;
        let by_id = |id: &str| document.get_element_by_id(id);
        let canvas = by_id("att-canvas")?
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .ok()?;
        let list = by_id("att-steps")?;
        let form = by_id("att-form")?;
        let root = document.document_element()?;

        // The step cards, and a place for the answer after its card.
        list.set_inner_html("");
        let mut cards = Vec::new();
        let answer = make(&document, "div", "att-answer");
        for step in Step::ALL {
            let card = card(&document, step);
            let _ = list.append_child(&card);
            if step == Step::Answer {
                let _ = list.append_child(&answer);
            }
            cards.push(card);
        }

        // Long values in the cards and the bubbles: tap to show whole, tap
        // again to shorten. One listener for the page.
        listen::<web_sys::Event>(document.as_ref(), "click", |event| {
            let Some(target) = event.target().and_then(|t| t.dyn_into::<Element>().ok()) else {
                return;
            };
            let Some(button) = target.closest(".att-value-long").ok().flatten() else {
                return;
            };
            let open = button.get_attribute("aria-expanded").as_deref() == Some("true");
            let words = if open {
                button.get_attribute("data-cut")
            } else {
                button.get_attribute("data-full")
            };
            button.set_text_content(words.as_deref());
            let _ = button.set_attribute("aria-expanded", if open { "false" } else { "true" });
            let _ = button.set_attribute(
                "title",
                if open {
                    copy::SHOW_ALL
                } else {
                    copy::SHOW_LESS
                },
            );
        });

        // Labels over the scene. The relay's is a button: hover or tap it
        // to see that it can't open what it carries.
        let mut labels = Vec::new();
        let mut tag = None;
        if let Some(layer) = by_id("att-labels") {
            layer.set_inner_html("");
            for (name, sub) in [
                (copy::YOU, copy::YOU_SUB),
                (copy::RELAY, copy::RELAY_SUB),
                (copy::PROVIDER, copy::PROVIDER_SUB),
            ] {
                let relay = name == copy::RELAY;
                let label = if relay {
                    let button = make(&document, "button", "att-label att-label-relay");
                    let _ = button.set_attribute("type", "button");
                    let _ = button.set_attribute("aria-expanded", "false");
                    button
                } else {
                    make(&document, "div", "att-label")
                };
                let _ = label.append_child(&text(&document, "strong", "", name));
                let _ = label.append_child(&text(&document, "span", "", sub));
                if relay {
                    let cant = make(&document, "span", "att-cant");
                    cant.set_inner_html(LOCK_X);
                    let _ = cant.append_child(&text(&document, "span", "", copy::CANT_OPEN));
                    let _ = label.append_child(&cant);
                    let toggle = label.clone();
                    listen::<web_sys::Event>(label.as_ref(), "click", move |_| {
                        let open = toggle.get_attribute("aria-expanded").as_deref() == Some("true");
                        let _ = toggle
                            .set_attribute("aria-expanded", if open { "false" } else { "true" });
                    });
                }
                let _ = layer.append_child(&label);
                if let Ok(label) = label.dyn_into::<HtmlElement>() {
                    labels.push(label);
                }
            }
            let moving = make(&document, "div", "att-tag");
            let _ = moving.set_attribute("aria-hidden", "true");
            let _ = layer.append_child(&moving);
            tag = moving.dyn_into::<HtmlElement>().ok();
        }

        let motion = !window
            .match_media("(prefers-reduced-motion: reduce)")
            .ok()
            .flatten()
            .is_some_and(|m| m.matches());
        // One bubble per party, empty and hidden until the flow fills it.
        let bubble_box = make(&document, "div", "att-bubbles");
        let mut bubbles = Vec::new();
        for party in Party::ALL {
            let bubble = make(&document, "details", "att-bubble");
            let _ = bubble.set_attribute("data-party", party.id());
            let _ = bubble.set_attribute("open", "");
            let _ = bubble.set_attribute("hidden", "");
            let _ = bubble.append_child(&text(&document, "summary", "", copy::party(party)));
            let _ = bubble.append_child(&make(&document, "div", "att-bubble-in"));
            let _ = bubble_box.append_child(&bubble);
            if let Ok(bubble) = bubble.dyn_into::<HtmlElement>() {
                bubbles.push(bubble);
            }
        }
        let stage = canvas.parent_element();
        let narrow = window.match_media("(max-width: 699px)").ok().flatten();
        let renderer = match Renderer::new(canvas) {
            Ok(renderer) => Some(renderer),
            Err(error) => {
                web_sys::console::warn_1(&error.into());
                let _ = root.class_list().add_1("att-no-gl");
                if let Some(note) = by_id("att-note") {
                    note.set_text_content(Some(copy::NO_WEBGL));
                }
                None
            }
        };
        if renderer.is_some() {
            let _ = root.class_list().add_1("att-gl");
        }
        let mut player = Player::default();
        player.reduced_motion = !motion;
        let inner = Rc::new(RefCell::new(Inner {
            player,
            lit: None,
            motion,
            running: false,
            prompt: None,
            dom: Dom {
                cards,
                user: by_id("att-user"),
                answer,
                verdict: by_id("att-verdict"),
                status: by_id("att-status"),
                run: by_id("att-run").and_then(|b| b.dyn_into::<HtmlElement>().ok()),
                labels,
                tag,
                stage,
                bubble_box,
                bubbles,
                narrow,
                strip: None,
                provider: copy::PROVIDER.to_string(),
                document: document.clone(),
            },
            renderer,
        }));
        let show = Rc::new(Self {
            inner: inner.clone(),
            run: Rc::new(RefCell::new(None)),
        });

        // Run: read the form, keep the message for its bubble, and hand it
        // to the flow.
        {
            let run = show.run.clone();
            let inner = inner.clone();
            let document = document.clone();
            listen::<web_sys::Event>(form.as_ref(), "submit", move |event| {
                event.prevent_default();
                if inner.borrow().running {
                    return;
                }
                let prompt = document
                    .get_element_by_id("att-prompt")
                    .and_then(|e| e.dyn_into::<HtmlInputElement>().ok())
                    .map(|input| input.value())
                    .unwrap_or_default();
                let tamper = document
                    .query_selector("input[name=att-tamper]:checked")
                    .ok()
                    .flatten()
                    .and_then(|e| e.dyn_into::<HtmlInputElement>().ok())
                    .map_or(Tamper::None, |input| Tamper::parse(&input.value()));
                let lane = document
                    .query_selector("input[name=att-lane]:checked")
                    .ok()
                    .flatten()
                    .and_then(|e| e.dyn_into::<HtmlInputElement>().ok())
                    .map_or(Lane::Cpu, |input| Lane::parse(&input.value()));
                let tamper = if lane.allows(tamper) {
                    tamper
                } else {
                    Tamper::None
                };
                let prompt = steps::clean_prompt(&prompt);
                inner.borrow_mut().prompt = Some(prompt.clone());
                if let Some(f) = run.borrow().as_ref() {
                    f(RunOptions {
                        lane,
                        tamper,
                        prompt,
                    });
                }
            });
        }

        // The frame loop.
        type FrameLoop = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;
        let next: FrameLoop = Rc::new(RefCell::new(None));
        let first = next.clone();
        let looping = window.clone();
        *first.borrow_mut() = Some(Closure::new(move |_: f64| {
            inner.borrow_mut().frame(&looping);
            if let Some(callback) = next.borrow().as_ref() {
                let _ = looping.request_animation_frame(callback.as_ref().unchecked_ref());
            }
        }));
        if let Some(callback) = first.borrow().as_ref() {
            let _ = window.request_animation_frame(callback.as_ref().unchecked_ref());
        }
        Some(show)
    }

    /// Every step back to waiting, the box dark, the transcript cleared
    /// down to the visitor's message.
    pub fn reset(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.player.reset();
        inner.lit = None;
        for step in Step::ALL {
            inner.dom.row(step, &State::Pending, None);
            let card = &inner.dom.cards[step.index()];
            let _ = card.remove_attribute("open");
            for selector in [".att-reason", ".att-panel-title", ".att-kv"] {
                if let Some(part) = child(card, selector) {
                    part.set_inner_html("");
                }
            }
        }
        inner.dom.answer.set_inner_html("");
        for bubble in &inner.dom.bubbles {
            let _ = bubble.set_attribute("hidden", "");
            if let Some(body) = child(bubble, ".att-bubble-in") {
                body.set_inner_html("");
            }
        }
        if let Some(verdict) = &inner.dom.verdict {
            verdict.set_inner_html("");
            let _ = verdict.remove_attribute("data-ok");
            let _ = verdict.set_attribute("hidden", "");
        }
        let prompt = inner.prompt.clone();
        inner.dom.user_message(prompt.as_deref());
    }

    /// Reports a step's state, with the real milliseconds it took.
    pub fn step(&self, step: Step, state: State, ms: Option<f64>) {
        self.inner
            .borrow_mut()
            .player
            .push(Event::Step(step, state, ms));
    }

    /// The box: `None` dark, `Some(true)` lit, `Some(false)` refused.
    pub fn provider_lit(&self, lit: Option<bool>) {
        self.inner
            .borrow_mut()
            .player
            .push(Event::Other(Later::Lit(lit)));
    }

    /// Fills a step's card with `(label, value)` rows.
    pub fn panel(&self, step: Step, title: &str, rows: &[(String, String)]) {
        self.inner
            .borrow_mut()
            .player
            .push(Event::Other(Later::Panel(
                step,
                title.to_owned(),
                rows.to_vec(),
            )));
    }

    /// The answer, opened in the browser, as an assistant message after
    /// the Answer card.
    pub fn answer(&self, question: &str, answer: &str) {
        self.inner
            .borrow_mut()
            .player
            .push(Event::Other(Later::Answer(
                question.to_owned(),
                answer.to_owned(),
            )));
    }

    /// A speech bubble over `party` with plain text, replacing that
    /// party's bubble.
    pub fn say(&self, party: Party, text: &str) {
        self.inner
            .borrow_mut()
            .player
            .push(Event::Other(Later::Say(party, text.to_owned())));
    }

    /// An event bubble over `party`: `title`, an optional one-line `note`
    /// under it, then `event` (a JSON object) pretty-printed with the
    /// value of the field named `bold` in bold. Replaces that party's
    /// bubble.
    pub fn event_bubble(
        &self,
        party: Party,
        title: &str,
        note: Option<&str>,
        event: &Value,
        bold: &str,
    ) {
        self.inner
            .borrow_mut()
            .player
            .push(Event::Other(Later::Bubble(
                party,
                title.to_owned(),
                note.map(str::to_owned),
                event.clone(),
                bold.to_owned(),
            )));
    }

    /// The result, at the end of the transcript.
    pub fn verdict(&self, ok: bool, headline: &str, detail: &str) {
        self.inner
            .borrow_mut()
            .player
            .push(Event::Other(Later::Verdict(
                ok,
                headline.to_owned(),
                detail.to_owned(),
            )));
    }

    /// The small status line, shown at once.
    pub fn status(&self, text: &str) {
        if let Some(status) = &self.inner.borrow().dom.status {
            status.set_text_content(Some(text));
        }
    }

    /// Disables Run while a round runs. Turning it back on waits until the
    /// scene has caught up.
    pub fn set_running(&self, running: bool) {
        let mut inner = self.inner.borrow_mut();
        if running {
            inner.set_button(true);
        } else {
            inner.player.push(Event::Other(Later::Idle));
        }
    }

    /// The provider's label over the scene, for the chosen lane.
    pub fn provider(&self, name: &str, sub: &str) {
        let mut inner = self.inner.borrow_mut();
        inner.dom.provider = name.to_string();
        if let Some(label) = inner.dom.labels.get(2) {
            label.set_inner_html("");
            let _ = label.append_child(&text(&inner.dom.document, "strong", "", name));
            let _ = label.append_child(&text(&inner.dom.document, "span", "", sub));
        }
    }

    /// Each step card's plain line for the lane: the open lane's where
    /// the sealed line would be false.
    pub fn lines(&self, lane: Lane) {
        let inner = self.inner.borrow();
        for step in Step::ALL {
            let words = match lane {
                Lane::Open => copy::open_line(step).unwrap_or_else(|| copy::line(step)),
                Lane::Gpu | Lane::Cpu => copy::line(step),
            };
            if let Some(line) = child(&inner.dom.cards[step.index()], ".att-line") {
                line.set_text_content(Some(words));
            }
        }
    }

    /// What to do when the visitor presses Run.
    pub fn on_run(&self, f: Box<dyn Fn(RunOptions)>) {
        *self.run.borrow_mut() = Some(f);
    }
}

impl Dom {
    /// The bubble for `party`, emptied and shown.
    fn bubble(&self, party: Party, kind: &str) -> Option<Element> {
        let bubble = &self.bubbles[party.index()];
        let _ = bubble.set_attribute("data-kind", kind);
        let _ = bubble.remove_attribute("hidden");
        let body = child(bubble, ".att-bubble-in")?;
        body.set_inner_html("");
        Some(body)
    }

    fn say(&self, party: Party, words: &str) {
        if let Some(body) = self.bubble(party, "say") {
            let _ = body.append_child(&text(&self.document, "p", "att-say", words));
        }
    }

    fn event_bubble(
        &self,
        party: Party,
        title: &str,
        note: Option<&str>,
        event: &Value,
        bold: &str,
    ) {
        let Some(body) = self.bubble(party, "event") else {
            return;
        };
        let _ = body.append_child(&text(&self.document, "p", "att-bubble-title", title));
        if let Some(note) = note {
            let _ = body.append_child(&text(&self.document, "p", "att-bubble-note", note));
        }
        let pre = make(&self.document, "pre", "att-json");
        for (i, line) in bubble::event_lines(event, bold).iter().enumerate() {
            if i > 0 {
                let _ = pre.append_child(&self.document.create_text_node("\n"));
            }
            for span in line {
                let node: web_sys::Node = match &span.full {
                    Some(full) => {
                        let button = text(
                            &self.document,
                            "button",
                            "att-value att-value-long",
                            &span.text,
                        );
                        let _ = button.set_attribute("type", "button");
                        let _ = button.set_attribute("aria-expanded", "false");
                        let _ = button.set_attribute("title", copy::SHOW_ALL);
                        let _ = button.set_attribute("data-full", full);
                        let _ = button.set_attribute("data-cut", &span.text);
                        button.into()
                    }
                    None if span.look == bubble::Look::Key => {
                        text(&self.document, "span", "att-k", &span.text).into()
                    }
                    None => self.document.create_text_node(&span.text).into(),
                };
                if span.bold {
                    let strong = make(&self.document, "strong", "");
                    let _ = strong.append_child(&node);
                    let _ = pre.append_child(&strong);
                } else {
                    let _ = pre.append_child(&node);
                }
            }
        }
        let _ = body.append_child(&pre);
    }

    /// Floats the bubbles over the scene, or stacks them under it.
    fn bubble_mode(&mut self, strip: bool) {
        if self.strip == Some(strip) {
            return;
        }
        self.strip = Some(strip);
        let Some(stage) = &self.stage else {
            return;
        };
        if strip {
            if let Some(parent) = stage.parent_node() {
                let _ = parent.insert_before(&self.bubble_box, stage.next_sibling().as_ref());
            }
            let _ = self.bubble_box.class_list().add_1("att-bubbles-strip");
            for bubble in &self.bubbles {
                let style = bubble.style();
                let _ = style.remove_property("transform");
                let _ = style.remove_property("visibility");
                let _ = style.remove_property("--att-tail");
            }
        } else {
            let _ = stage.append_child(&self.bubble_box);
            let _ = self.bubble_box.class_list().remove_1("att-bubbles-strip");
            for bubble in &self.bubbles {
                let _ = bubble.set_attribute("open", "");
            }
        }
    }

    /// Places each shown bubble just over its party's label, side by side,
    /// with its tail at the party.
    fn float_bubbles(&self) {
        let Some(stage) = &self.stage else {
            return;
        };
        let room = stage.get_bounding_client_rect();
        let labels: Vec<bubble::Rect> = self
            .labels
            .iter()
            .map(|label| {
                let r = label.get_bounding_client_rect();
                bubble::Rect {
                    left: r.left() - room.left(),
                    top: r.top() - room.top(),
                    right: r.right() - room.left(),
                    bottom: r.bottom() - room.top(),
                }
            })
            .collect();
        let mut shown = Vec::new();
        let mut wants = Vec::new();
        for (bubble, label) in self.bubbles.iter().zip(&self.labels) {
            if bubble.has_attribute("hidden") {
                continue;
            }
            let at = label.get_bounding_client_rect();
            let size = bubble.get_bounding_client_rect();
            wants.push(bubble::Want {
                x: at.left() + at.width() / 2.0 - room.left(),
                bottom: at.top() - room.top() - 12.0,
                width: size.width(),
                height: size.height(),
            });
            shown.push(bubble);
        }
        for (bubble, placed) in shown.iter().zip(bubble::arrange(
            &wants,
            &labels,
            room.width(),
            room.height(),
        )) {
            let style = bubble.style();
            let _ = style.set_property(
                "transform",
                &format!("translate({:.1}px, {:.1}px)", placed.left, placed.top),
            );
            let _ = style.set_property("--att-tail", &format!("{:.1}px", placed.tail));
            let _ = style.set_property("visibility", "visible");
        }
    }

    fn user_message(&self, prompt: Option<&str>) {
        let Some(slot) = &self.user else {
            return;
        };
        slot.set_inner_html("");
        let Some(prompt) = prompt else {
            return;
        };
        let article = message(&self.document, "user", copy::YOU);
        let bubble = make(&self.document, "div", "oa-message-bubble");
        bubble.set_text_content(Some(&format!("{prompt}\n\n{}", copy::QUESTION)));
        let _ = article.append_child(&bubble);
        let _ = slot.append_child(&article);
    }

    fn row(&self, step: Step, state: &State, ms: Option<f64>) {
        let card = &self.cards[step.index()];
        badge_for(card, state);
        if let Some(detail) = child(card, ".oa-tool-call__detail") {
            detail.set_text_content(Some(&ms.map(ms_label).unwrap_or_default()));
        }
        if let Some(reason) = child(card, ".att-reason") {
            let words = match state {
                State::Refused(why) => why.as_str(),
                _ => "",
            };
            reason.set_text_content(Some(words));
        }
        if matches!(state, State::Refused(_)) {
            let _ = card.set_attribute("open", "");
        }
    }

    fn panel(&self, step: Step, title: &str, rows: &[(String, String)]) {
        let card = &self.cards[step.index()];
        if let Some(head) = child(card, ".att-panel-title") {
            head.set_text_content(Some(title));
        }
        let Some(list) = child(card, ".att-kv") else {
            return;
        };
        list.set_inner_html("");
        for (label, value) in rows {
            let _ = list.append_child(&text(&self.document, "dt", "", label));
            let dd = make(&self.document, "dd", "");
            match steps::cut_value(value, VALUE_MAX) {
                Some(cut) => {
                    let button = make(&self.document, "button", "att-value att-value-long");
                    let _ = button.set_attribute("type", "button");
                    let _ = button.set_attribute("aria-expanded", "false");
                    let _ = button.set_attribute("title", copy::SHOW_ALL);
                    let _ = button.set_attribute("data-full", value);
                    let _ = button.set_attribute("data-cut", &cut);
                    button.set_text_content(Some(&cut));
                    let _ = dd.append_child(&button);
                }
                None => {
                    let _ = dd.append_child(&text(&self.document, "span", "att-value", value));
                }
            }
            let _ = list.append_child(&dd);
        }
        let _ = card.set_attribute("open", "");
    }

    fn answer(&self, question: &str, answer: &str) {
        self.answer.set_inner_html("");
        let article = message(&self.document, "assistant", &self.provider);
        let content = make(&self.document, "div", "oa-message-content");
        let _ = content.append_child(&text(&self.document, "p", "att-answer-question", question));
        let _ = content.append_child(&text(&self.document, "p", "att-answer-text", answer));
        let _ = content.append_child(&text(
            &self.document,
            "p",
            "att-answer-note",
            copy::OPENED_HERE,
        ));
        let _ = article.append_child(&content);
        let _ = self.answer.append_child(&article);
    }

    fn verdict(&self, ok: bool, headline: &str, detail: &str) {
        let Some(verdict) = &self.verdict else {
            return;
        };
        verdict.set_inner_html("");
        let article = message(&self.document, "assistant", copy::RESULT);
        let content = make(&self.document, "div", "oa-message-content");
        let _ = content.append_child(&text(&self.document, "p", "att-verdict-head", headline));
        let _ = content.append_child(&text(&self.document, "p", "", detail));
        let _ = article.append_child(&content);
        let _ = verdict.append_child(&article);
        let _ = verdict.set_attribute("data-ok", if ok { "true" } else { "false" });
        let _ = verdict.remove_attribute("hidden");
    }
}

impl Inner {
    fn set_button(&mut self, running: bool) {
        self.running = running;
        if let Some(button) = &self.dom.run {
            let label = child(button, ".oa-button-inner").unwrap_or_else(|| button.clone().into());
            if running {
                let _ = button.set_attribute("disabled", "");
                let _ = button.set_attribute("aria-busy", "true");
                label.set_text_content(Some(copy::RUNNING));
            } else {
                let _ = button.remove_attribute("disabled");
                let _ = button.remove_attribute("aria-busy");
                label.set_text_content(Some(copy::RUN));
            }
        }
    }

    fn frame(&mut self, window: &Window) {
        let now = now(window);
        for event in self.player.pump(now) {
            match event {
                Event::Step(step, state, ms) => self.dom.row(step, &state, ms),
                Event::Other(Later::Panel(step, title, rows)) => {
                    self.dom.panel(step, &title, &rows);
                }
                Event::Other(Later::Answer(question, answer)) => {
                    self.dom.answer(&question, &answer);
                }
                Event::Other(Later::Verdict(ok, head, detail)) => {
                    self.dom.verdict(ok, &head, &detail);
                }
                Event::Other(Later::Lit(lit)) => self.lit = lit,
                Event::Other(Later::Idle) => self.set_button(false),
                Event::Other(Later::Say(party, words)) => self.dom.say(party, &words),
                Event::Other(Later::Bubble(party, title, note, event, bold)) => {
                    self.dom
                        .event_bubble(party, &title, note.as_deref(), &event, &bold);
                }
            }
        }
        let strip =
            self.renderer.is_none() || self.dom.narrow.as_ref().is_some_and(|m| m.matches());
        self.dom.bubble_mode(strip);
        let Some(renderer) = &self.renderer else {
            return;
        };
        let (w, h) = renderer.fit(window.device_pixel_ratio());
        if w < 2.0 || h < 2.0 {
            return;
        }
        let sway = if self.motion {
            0.08 * (now * 0.11).sin() as f32
        } else {
            0.0
        };
        let camera = scene::camera((w / h) as f32, 1.0, 0.0, sway);
        let frame = scene::frame(&self.player.anims, self.lit, now, self.motion);
        renderer.draw(&camera, &frame);
        let place = |label: &HtmlElement, anchor: glam::Vec3| {
            let clip = camera.view_proj * anchor.extend(1.0);
            let style = label.style();
            if clip.w <= 0.0 {
                let _ = style.set_property("visibility", "hidden");
                return;
            }
            let x = (f64::from(clip.x / clip.w) * 0.5 + 0.5) * w;
            let y = (0.5 - f64::from(clip.y / clip.w) * 0.5) * h;
            let _ = style.set_property("visibility", "visible");
            let _ = style.set_property(
                "transform",
                &format!("translate({x:.1}px, {y:.1}px) translate(-50%, -100%)"),
            );
        };
        for (label, anchor) in self.dom.labels.iter().zip(scene::label_anchors()) {
            place(label, anchor);
        }
        if !strip {
            self.dom.float_bubbles();
        }
        if let Some(tag) = &self.dom.tag {
            match frame.tag {
                Some((at, kind)) => {
                    let (words, class) = match kind {
                        scene::Tag::SealedInYourBrowser => (copy::TAG_OUT, "att-tag"),
                        scene::Tag::SealedToYourBrowser => (copy::TAG_BACK, "att-tag att-tag-back"),
                    };
                    if tag.text_content().as_deref() != Some(words) {
                        tag.set_text_content(Some(words));
                        tag.set_class_name(class);
                    }
                    place(tag, at);
                }
                None => {
                    let _ = tag.style().set_property("visibility", "hidden");
                }
            }
        }
    }
}
