//! The page: the scene on its canvas, the step list, the per-step panels,
//! the verdict, and the Run form.
//!
//! The live flow (`flow`) reports through [`Show`]. Reports go through the
//! timeline (`steps::Player`) so each step is seen for long enough, and the
//! step list, panels and verdict change as the scene reaches them. Without
//! WebGL2 the page still works: the canvas is hidden and the lists update.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;
use web_sys::{Document, Element, HtmlElement, HtmlInputElement, Window};

use crate::copy;
use crate::gl::Renderer;
use crate::scene;
use crate::steps::{self, Event, Player, ms_label};

pub use crate::steps::{RunOptions, State, Step, Tamper};

/// Values longer than this many characters are cut in the middle until
/// tapped.
const VALUE_MAX: usize = 44;

/// A report that waits its turn behind the step animations.
enum Later {
    Panel(Step, String, Vec<(String, String)>),
    Verdict(bool, String, String),
    Lit(Option<bool>),
    Idle,
}

struct Dom {
    document: Document,
    rows: Vec<Element>,
    panels: Vec<Element>,
    verdict: Option<Element>,
    status: Option<Element>,
    run: Option<HtmlElement>,
    labels: Vec<HtmlElement>,
    left: Option<Element>,
    right: Option<Element>,
}

struct Inner {
    player: Player<Later>,
    lit: Option<bool>,
    motion: bool,
    running: bool,
    dom: Dom,
    renderer: Option<Renderer>,
}

/// What runs when the visitor presses Run.
type OnRun = Rc<RefCell<Option<Box<dyn Fn(RunOptions)>>>>;

/// The page's scene and HUD.
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

impl Show {
    /// Finds the page's parts (`#att-canvas`, `#att-steps`, `#att-panels`,
    /// `#att-form`), builds the step list and panels, and starts the loop.
    /// `None` when the page lacks them.
    pub fn mount() -> Option<Rc<Self>> {
        let window = web_sys::window()?;
        let document = window.document()?;
        let by_id = |id: &str| document.get_element_by_id(id);
        let canvas = by_id("att-canvas")?
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .ok()?;
        let list = by_id("att-steps")?;
        let panels_box = by_id("att-panels")?;
        let form = by_id("att-form")?;
        let root = document.document_element()?;

        // The step list and one hidden panel per step.
        list.set_inner_html("");
        panels_box.set_inner_html("");
        let mut rows = Vec::new();
        let mut panels = Vec::new();
        for (i, step) in Step::ALL.into_iter().enumerate() {
            let row = make(&document, "li", "att-step");
            let _ = row.set_attribute("data-step", step.id());
            let _ = row.set_attribute("data-state", "pending");
            let mark = make(&document, "span", "att-step-mark");
            mark.set_text_content(Some(&(i + 1).to_string()));
            let body = make(&document, "div", "att-step-body");
            let head = make(&document, "div", "att-step-head");
            let name = make(&document, "span", "att-step-name");
            name.set_text_content(Some(copy::name(step)));
            let state = make(&document, "span", "att-step-state");
            state.set_text_content(Some(copy::state_word(&State::Pending)));
            let ms = make(&document, "span", "att-step-ms");
            let _ = head.append_child(&name);
            let _ = head.append_child(&ms);
            let _ = head.append_child(&state);
            let line = make(&document, "p", "att-step-line");
            line.set_text_content(Some(copy::line(step)));
            let reason = make(&document, "p", "att-step-reason");
            let _ = body.append_child(&head);
            let _ = body.append_child(&line);
            let _ = body.append_child(&reason);
            let _ = row.append_child(&mark);
            let _ = row.append_child(&body);
            let _ = list.append_child(&row);
            rows.push(row);

            let panel = make(&document, "section", "att-panel");
            let _ = panel.set_attribute("data-step", step.id());
            let _ = panel.set_attribute("hidden", "");
            let title = make(&document, "h3", "att-panel-title");
            let rows_box = make(&document, "dl", "att-panel-rows");
            let _ = panel.append_child(&title);
            let _ = panel.append_child(&rows_box);
            let _ = panels_box.append_child(&panel);
            panels.push(panel);
        }

        // Long values: tap to show whole, tap again to shorten.
        listen::<web_sys::Event>(panels_box.as_ref(), "click", |event| {
            let Some(target) = event.target().and_then(|t| t.dyn_into::<Element>().ok()) else {
                return;
            };
            let Some(button) = target.closest(".att-value-long").ok().flatten() else {
                return;
            };
            let open = button.get_attribute("aria-expanded").as_deref() == Some("true");
            let text = if open {
                button.get_attribute("data-cut")
            } else {
                button.get_attribute("data-full")
            };
            button.set_text_content(text.as_deref());
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

        // Station labels over the scene.
        let mut labels = Vec::new();
        if let Some(layer) = by_id("att-labels") {
            layer.set_inner_html("");
            for (name, sub) in [
                (copy::YOU, copy::YOU_SUB),
                (copy::RELAY, copy::RELAY_SUB),
                (copy::PROVIDER, copy::PROVIDER_SUB),
            ] {
                let label = make(&document, "div", "att-label");
                let strong = make(&document, "strong", "");
                strong.set_text_content(Some(name));
                let small = make(&document, "span", "");
                small.set_text_content(Some(sub));
                let _ = label.append_child(&strong);
                let _ = label.append_child(&small);
                let _ = layer.append_child(&label);
                if let Ok(label) = label.dyn_into::<HtmlElement>() {
                    labels.push(label);
                }
            }
        }

        let motion = !window
            .match_media("(prefers-reduced-motion: reduce)")
            .ok()
            .flatten()
            .is_some_and(|m| m.matches());
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
        let run_button = by_id("att-run").and_then(|b| b.dyn_into::<HtmlElement>().ok());
        if let Some(button) = &run_button {
            button.set_text_content(Some(copy::RUN));
        }
        let inner = Rc::new(RefCell::new(Inner {
            player,
            lit: None,
            motion,
            running: false,
            dom: Dom {
                rows,
                panels,
                verdict: by_id("att-verdict"),
                status: by_id("att-status"),
                run: run_button,
                labels,
                left: by_id("att-left"),
                right: by_id("att-right"),
                document: document.clone(),
            },
            renderer,
        }));
        let show = Rc::new(Self {
            inner: inner.clone(),
            run: Rc::new(RefCell::new(None)),
        });

        // Run: read the form and hand it to the flow.
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
                if let Some(f) = run.borrow().as_ref() {
                    f(RunOptions {
                        tamper,
                        prompt: steps::clean_prompt(&prompt),
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

    /// Every step back to waiting, the vault dark, the panels and verdict
    /// cleared.
    pub fn reset(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.player.reset();
        inner.lit = None;
        for step in Step::ALL {
            inner.dom.row(step, &State::Pending, None);
            let panel = &inner.dom.panels[step.index()];
            let _ = panel.set_attribute("hidden", "");
        }
        if let Some(verdict) = &inner.dom.verdict {
            verdict.set_inner_html("");
            let _ = verdict.remove_attribute("data-ok");
            let _ = verdict.set_attribute("hidden", "");
        }
    }

    /// Reports a step's state, with the real milliseconds it took.
    pub fn step(&self, step: Step, state: State, ms: Option<f64>) {
        self.inner
            .borrow_mut()
            .player
            .push(Event::Step(step, state, ms));
    }

    /// The vault: `None` dark, `Some(true)` lit gold, `Some(false)` refused.
    pub fn provider_lit(&self, lit: Option<bool>) {
        self.inner
            .borrow_mut()
            .player
            .push(Event::Other(Later::Lit(lit)));
    }

    /// Fills a step's panel with `(label, value)` rows.
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

    /// The big result line.
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

    /// What to do when the visitor presses Run.
    pub fn on_run(&self, f: Box<dyn Fn(RunOptions)>) {
        *self.run.borrow_mut() = Some(f);
    }
}

impl Dom {
    fn row(&self, step: Step, state: &State, ms: Option<f64>) {
        let row = &self.rows[step.index()];
        let _ = row.set_attribute("data-state", state.id());
        if let Some(chip) = child(row, ".att-step-state") {
            chip.set_text_content(Some(copy::state_word(state)));
        }
        if let Some(cell) = child(row, ".att-step-ms") {
            cell.set_text_content(Some(&ms.map(ms_label).unwrap_or_default()));
        }
        if let Some(reason) = child(row, ".att-step-reason") {
            let text = match state {
                State::Refused(why) => why.as_str(),
                _ => "",
            };
            reason.set_text_content(Some(text));
        }
    }

    fn panel(&self, step: Step, title: &str, rows: &[(String, String)]) {
        let panel = &self.panels[step.index()];
        if let Some(head) = child(panel, ".att-panel-title") {
            head.set_text_content(Some(title));
        }
        let Some(list) = child(panel, ".att-panel-rows") else {
            return;
        };
        list.set_inner_html("");
        for (label, value) in rows {
            let dt = make(&self.document, "dt", "");
            dt.set_text_content(Some(label));
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
                    let span = make(&self.document, "span", "att-value");
                    span.set_text_content(Some(value));
                    let _ = dd.append_child(&span);
                }
            }
            let _ = list.append_child(&dt);
            let _ = list.append_child(&dd);
        }
        let _ = panel.remove_attribute("hidden");
    }

    fn verdict(&self, ok: bool, headline: &str, detail: &str) {
        let Some(verdict) = &self.verdict else {
            return;
        };
        verdict.set_inner_html("");
        let head = make(&self.document, "p", "att-verdict-head");
        head.set_text_content(Some(headline));
        let body = make(&self.document, "p", "att-verdict-detail");
        body.set_text_content(Some(detail));
        let _ = verdict.append_child(&head);
        let _ = verdict.append_child(&body);
        let _ = verdict.set_attribute("data-ok", if ok { "true" } else { "false" });
        let _ = verdict.remove_attribute("hidden");
    }

    /// The share of the canvas between the two HUD columns, and its centre
    /// in clip space; the whole canvas when the columns are stacked.
    fn region(&self, width: f64) -> (f32, f32) {
        let (Some(left), Some(right)) = (&self.left, &self.right) else {
            return (1.0, 0.0);
        };
        let (l, r) = (
            left.get_bounding_client_rect(),
            right.get_bounding_client_rect(),
        );
        let gap = r.left() - l.right();
        if width <= 0.0 || gap < width * 0.3 || l.width() <= 0.0 || r.width() <= 0.0 {
            return (1.0, 0.0);
        }
        let centre = (l.right() + r.left()) / 2.0;
        // Leave room for the vault's label beside the panels.
        let gap = gap - 80.0;
        ((gap / width) as f32, (centre / width * 2.0 - 1.0) as f32)
    }
}

impl Inner {
    fn set_button(&mut self, running: bool) {
        self.running = running;
        if let Some(button) = &self.dom.run {
            if running {
                let _ = button.set_attribute("disabled", "");
                let _ = button.set_attribute("aria-busy", "true");
                button.set_text_content(Some(copy::RUNNING));
            } else {
                let _ = button.remove_attribute("disabled");
                let _ = button.remove_attribute("aria-busy");
                button.set_text_content(Some(copy::RUN));
            }
        }
    }

    fn frame(&mut self, window: &Window) {
        let now = now(window);
        for event in self.player.pump(now) {
            match event {
                Event::Step(step, state, ms) => self.dom.row(step, &state, ms),
                Event::Other(Later::Panel(step, title, rows)) => {
                    self.dom.panel(step, &title, &rows)
                }
                Event::Other(Later::Verdict(ok, head, detail)) => {
                    self.dom.verdict(ok, &head, &detail)
                }
                Event::Other(Later::Lit(lit)) => self.lit = lit,
                Event::Other(Later::Idle) => self.set_button(false),
            }
        }
        let Some(renderer) = &self.renderer else {
            return;
        };
        let (w, h) = renderer.fit(window.device_pixel_ratio());
        if w < 2.0 || h < 2.0 {
            return;
        }
        let (region, centre) = self.dom.region(w);
        let sway = if self.motion {
            0.07 * (now * 0.11).sin() as f32
        } else {
            0.0
        };
        let camera = scene::camera((w / h) as f32, region, centre, sway);
        let frame = scene::frame(&self.player.anims, self.lit, now, self.motion);
        let vault = match self.lit {
            Some(true) => scene::gold().map(|c| c * 0.9),
            Some(false) => scene::red().map(|c| c * 0.35),
            None => [0.0; 3],
        };
        let lamps = [
            (scene::ORB + Vec3::new(0.0, 0.2, 0.8), [0.55, 0.38, 0.2]),
            (scene::OBELISK_FOOT + Vec3::new(0.0, 2.2, 1.6), vault),
        ];
        renderer.draw(&camera, &frame, lamps);
        for (label, anchor) in self.dom.labels.iter().zip(scene::label_anchors()) {
            let clip = camera.view_proj * anchor.extend(1.0);
            let style = label.style();
            if clip.w <= 0.0 {
                let _ = style.set_property("visibility", "hidden");
                continue;
            }
            let x = (f64::from(clip.x / clip.w) * 0.5 + 0.5) * w;
            let y = (0.5 - f64::from(clip.y / clip.w) * 0.5) * h;
            let _ = style.set_property("visibility", "visible");
            let _ = style.set_property(
                "transform",
                &format!("translate(-50%, -100%) translate({x:.1}px, {y:.1}px)"),
            );
        }
    }
}
