//! Managed browser events around the original portable demo and cell renderer.

use super::{Composition, view};
use coder_demo_ui::{Hit, Snapshot};
use coder_ui::demo::{DemoState, Key, KeyCode};
use serde::Deserialize;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast, closure::Closure, prelude::*};
use web_sys::{
    ClipboardEvent, CompositionEvent, Document, Element, Event, EventTarget, HtmlElement,
    HtmlTextAreaElement, InputEvent, KeyboardEvent, MouseEvent, WheelEvent, Window,
};
use zeroize::Zeroizing;

const TICK_MS: i32 = 125;
const MAX_INPUT_BYTES: usize = 65_536;

thread_local! {
    static DEMO: RefCell<Option<Rc<Runtime>>> = const { RefCell::new(None) };
}

struct Listener {
    target: EventTarget,
    name: &'static str,
    callback: Closure<dyn FnMut(Event)>,
}
impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self
            .target
            .remove_event_listener_with_callback(self.name, self.callback.as_ref().unchecked_ref());
    }
}

struct Timer {
    window: Window,
    handle: i32,
    _callback: Closure<dyn FnMut()>,
}
impl Drop for Timer {
    fn drop(&mut self) {
        self.window.clear_interval_with_handle(self.handle);
    }
}

struct Runtime {
    window: Window,
    document: Document,
    root: Element,
    mount: Element,
    surface: Element,
    input: HtmlTextAreaElement,
    state: RefCell<Option<DemoState>>,
    snapshot: RefCell<Option<Snapshot>>,
    composition: RefCell<Composition>,
    listeners: RefCell<Vec<Listener>>,
    timer: RefCell<Option<Timer>>,
    active: Cell<bool>,
    paused: Cell<bool>,
    selecting: Cell<bool>,
    selection_held: Cell<bool>,
    frames: Cell<u64>,
    visible_ms: Cell<f64>,
    resumed_at: Cell<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Initial {
    schema: String,
}

fn error(message: &str) -> JsValue {
    JsValue::from_str(message)
}

/// Mount Coder's original synthetic demo. It creates no connection or storage.
#[wasm_bindgen]
pub fn start_demo() -> Result<(), JsValue> {
    if DEMO.with(|slot| slot.borrow().is_some()) {
        return Err(error("The demo is already mounted."));
    }
    let window = web_sys::window().ok_or_else(|| error("Window unavailable."))?;
    let document = window
        .document()
        .ok_or_else(|| error("Document unavailable."))?;
    let root = document
        .get_element_by_id("demo-root")
        .ok_or_else(|| error("Demo mount unavailable."))?;
    if let Some(initial) = document.get_element_by_id("demo-initial") {
        let bytes = initial.text_content().unwrap_or_default();
        if bytes.len() > 1_024 {
            return Err(error("Demo descriptor exceeds its bound."));
        }
        let descriptor: Initial =
            serde_json::from_str(&bytes).map_err(|_| error("Invalid demo descriptor."))?;
        if descriptor.schema != "openagents.coder.demo.v1" {
            return Err(error("Unsupported demo descriptor."));
        }
    }
    let mount = document
        .get_element_by_id("demo-mount")
        .unwrap_or_else(|| root.clone());
    let semantic = view(1).validate().map_err(|e| error(&e.to_string()))?;
    rust_native_web::mount(&mount, &semantic)?;
    let surface = mount
        .query_selector("[data-rn-surface=coder-demo-grid]")?
        .ok_or_else(|| error("Native demo surface unavailable."))?;
    let input = mount
        .query_selector("[data-rn-node=demo-keyboard] textarea")?
        .ok_or_else(|| error("Native demo input unavailable."))?
        .dyn_into::<HtmlTextAreaElement>()?;
    for (name, value) in [
        ("autocomplete", "off"),
        ("autocapitalize", "off"),
        ("autocorrect", "off"),
        ("spellcheck", "false"),
        ("data-demo-input", ""),
    ] {
        input.set_attribute(name, value)?;
    }
    let runtime = Rc::new(Runtime {
        window,
        document,
        root,
        mount,
        surface,
        input,
        state: RefCell::new(Some(DemoState::default())),
        snapshot: RefCell::new(None),
        composition: RefCell::new(Composition::default()),
        listeners: RefCell::new(vec![]),
        timer: RefCell::new(None),
        active: Cell::new(true),
        paused: Cell::new(true),
        selecting: Cell::new(false),
        selection_held: Cell::new(false),
        frames: Cell::new(0),
        visible_ms: Cell::new(0.0),
        resumed_at: Cell::new(0.0),
    });
    runtime.render()?;
    runtime.install()?;
    DEMO.with(|slot| *slot.borrow_mut() = Some(runtime.clone()));
    if !runtime.document.hidden() {
        runtime.resume()?;
        runtime.focus()?;
    }
    Ok(())
}

impl Runtime {
    fn now_ms(&self) -> f64 {
        self.window
            .performance()
            .map_or_else(js_sys::Date::now, |p| p.now())
    }

    fn visible(&self) -> bool {
        self.active.get() && !self.document.hidden() && self.root.is_connected()
    }

    fn dimensions(&self) -> (u16, u16) {
        let bounds = self.root.get_bounding_client_rect();
        (
            (bounds.width() / 9.0).floor().clamp(1.0, 240.0) as u16,
            (bounds.height() / 20.0).floor().clamp(1.0, 160.0) as u16,
        )
    }

    fn render(&self) -> Result<(), JsValue> {
        if !self.active.get() {
            return Ok(());
        }
        // Preserve the original nodes during a drag or copy gesture. The Rust
        // clock continues; the next frame draws once the selection is cleared.
        if self.selecting.get() || self.surface_selected() {
            self.selection_held.set(true);
            return Ok(());
        }
        self.selection_held.set(false);
        let (width, height) = self.dimensions();
        let mut state = self.state.borrow_mut();
        let Some(state) = state.as_mut() else {
            return Ok(());
        };
        let snapshot = coder_demo_ui::capture(state, width, height);
        // Only the owning Rust renderer's escaped SVG enters this registered surface.
        self.surface.set_inner_html(&coder_demo_ui::svg(&snapshot));
        self.surface
            .set_attribute("data-demo-columns", &width.to_string())?;
        self.surface
            .set_attribute("data-demo-rows", &height.to_string())?;
        let label = state.input();
        self.input.set_attribute(
            "aria-label",
            label.map_or("Coder demo navigation", |i| i.label),
        )?;
        self.input.set_attribute(
            "data-rn-secret",
            if label.is_some_and(|i| i.secret) {
                "true"
            } else {
                "false"
            },
        )?;
        self.input
            .set_attribute("inputmode", if label.is_some() { "text" } else { "none" })?;
        if let Some((x, y)) = snapshot.cursor {
            self.input.set_attribute(
                "style",
                &format!("--demo-input-x:{}px;--demo-input-y:{}px", x * 9, y * 20),
            )?;
        }
        if let Some(readable) = self.document.get_element_by_id("demo-readable") {
            let rows = snapshot
                .cells
                .chunks(usize::from(width))
                .map(|row| {
                    row.iter()
                        .map(|cell| cell.symbol.as_str())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n");
            readable.set_text_content(Some(&rows));
        }
        self.snapshot.replace(Some(snapshot));
        self.frames.set(self.frames.get().saturating_add(1));
        self.root.set_attribute("data-demo-ready", "")?;
        if let Some(status) = self.document.get_element_by_id("demo-status") {
            status.set_text_content(Some("Rust demo ready. All activity stays in this page."));
            status.set_attribute("hidden", "")?;
        }
        Ok(())
    }

    fn focus(&self) -> Result<(), JsValue> {
        let options = web_sys::FocusOptions::new();
        options.set_prevent_scroll(true);
        self.input.focus_with_options(&options)
    }

    fn clear_input(&self) {
        self.input.set_value("");
        let _ = self.input.set_default_value("");
    }

    fn surface_selected(&self) -> bool {
        let Ok(Some(selection)) = self.window.get_selection() else {
            return false;
        };
        !selection.is_collapsed()
            && selection.range_count() > 0
            && selection.get_range_at(0).ok().is_some_and(|range| {
                range
                    .intersects_node(self.surface.as_ref())
                    .unwrap_or(false)
            })
    }

    fn result(&self, result: Result<(), JsValue>) {
        if result.is_err() {
            self.retire("The demo needs to be reopened.");
        }
    }

    fn key(&self, key: Key, download: bool) -> Result<(), JsValue> {
        if !self.visible() {
            return Ok(());
        }
        let keep = self
            .state
            .borrow_mut()
            .as_mut()
            .is_none_or(|state| state.key(key));
        self.clear_input();
        if !keep {
            self.retire("The demo is closed. Reload to open it again.");
            return Ok(());
        }
        self.render()?;
        if download {
            self.download()?;
        }
        Ok(())
    }

    fn paste(&self, text: &str) -> Result<(), JsValue> {
        if !self.visible() || text.len() > MAX_INPUT_BYTES {
            return Ok(());
        }
        if let Some(state) = self.state.borrow_mut().as_mut() {
            state.paste(text);
        }
        self.clear_input();
        self.render()
    }

    fn download(&self) -> Result<(), JsValue> {
        let Some(download) = self
            .state
            .borrow_mut()
            .as_mut()
            .and_then(DemoState::take_download)
        else {
            return Ok(());
        };
        let bytes = js_sys::Uint8Array::from(download.bytes.as_slice());
        let parts = js_sys::Array::new();
        parts.push(&bytes);
        let options = web_sys::BlobPropertyBag::new();
        options.set_type("application/json");
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)?;
        let url = web_sys::Url::create_object_url_with_blob(&blob)?;
        let link = self.document.create_element("a")?;
        link.set_attribute("href", &url)?;
        link.set_attribute("download", &download.filename)?;
        link.dyn_ref::<HtmlElement>()
            .ok_or_else(|| error("Download unavailable."))?
            .click();
        web_sys::Url::revoke_object_url(&url)
    }

    fn pause(&self) {
        if !self.paused.replace(true) {
            self.visible_ms
                .set(self.visible_ms.get() + (self.now_ms() - self.resumed_at.get()).max(0.0));
        }
        self.timer.borrow_mut().take();
        self.clear_input();
        self.composition.replace(Composition::default());
        self.selecting.set(false);
        if let Some(state) = self.state.borrow_mut().as_mut() {
            state.retire_secrets();
        }
    }

    fn resume(self: &Rc<Self>) -> Result<(), JsValue> {
        if !self.visible() || !self.paused.replace(false) {
            return Ok(());
        }
        self.resumed_at.set(self.now_ms());
        let weak = Rc::downgrade(self);
        let callback = Closure::<dyn FnMut()>::new(move || {
            let Some(runtime) = weak.upgrade() else {
                return;
            };
            if !runtime.root.is_connected() {
                runtime.retire("Reopen the demo to start a fresh page.");
                return;
            }
            if !runtime.visible() || runtime.paused.get() {
                return;
            }
            let elapsed =
                runtime.visible_ms.get() + (runtime.now_ms() - runtime.resumed_at.get()).max(0.0);
            if let Some(state) = runtime.state.borrow_mut().as_mut() {
                state.elapsed_seconds = (elapsed / 1_000.0) as u64;
                state.tick();
            }
            runtime.result(runtime.render());
        });
        let handle = self
            .window
            .set_interval_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                TICK_MS,
            )?;
        self.timer.replace(Some(Timer {
            window: self.window.clone(),
            handle,
            _callback: callback,
        }));
        self.render()
    }

    fn retire(&self, message: &str) {
        if !self.active.replace(false) {
            return;
        }
        self.pause();
        self.state.borrow_mut().take();
        self.snapshot.borrow_mut().take();
        self.listeners.borrow_mut().clear();
        let _ = rust_native_web::dispose(&self.mount);
        if let Some(readable) = self.document.get_element_by_id("demo-readable") {
            readable.set_text_content(None);
        }
        let _ = self.root.remove_attribute("data-demo-ready");
        if let Some(status) = self.document.get_element_by_id("demo-status") {
            let _ = status.remove_attribute("hidden");
            status.set_text_content(Some(message));
        }
        DEMO.with(|slot| slot.borrow_mut().take());
    }

    fn listen(
        self: &Rc<Self>,
        target: EventTarget,
        name: &'static str,
        mut handler: impl FnMut(&Rc<Self>, Event) + 'static,
    ) -> Result<(), JsValue> {
        let weak = Rc::downgrade(self);
        let callback = Closure::<dyn FnMut(Event)>::new(move |event| {
            if let Some(runtime) = weak.upgrade() {
                handler(&runtime, event);
            }
        });
        target.add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())?;
        self.listeners.borrow_mut().push(Listener {
            target,
            name,
            callback,
        });
        Ok(())
    }

    fn install(self: &Rc<Self>) -> Result<(), JsValue> {
        self.listen(self.input.clone().into(), "keydown", |runtime, event| {
            let Some(event) = event.dyn_ref::<KeyboardEvent>() else {
                return;
            };
            if runtime.composition.borrow().active || event.is_composing() {
                return;
            }
            if event.get_modifier_state("AltGraph") {
                return;
            }
            // The terminal consumes OS command shortcuts before its editor.
            // Cmd+P is Coder's explicit plugin shortcut; other Cmd gestures
            // remain browser actions and cannot become demo characters.
            if event.meta_key()
                && !(matches!(event.key().as_str(), "p" | "P")
                    && !event.ctrl_key()
                    && !event.alt_key()
                    && !event.shift_key())
            {
                return;
            }
            // Platform clipboard gestures stay with the native input control.
            if (event.ctrl_key() || event.meta_key()) && matches!(event.key().as_str(), "v" | "V") {
                return;
            }
            if (event.ctrl_key() || event.meta_key())
                && matches!(event.key().as_str(), "c" | "C")
                && runtime.surface_selected()
            {
                return;
            }
            let Some(code) = code(event) else {
                return;
            };
            event.prevent_default();
            runtime.result(runtime.key(
                Key {
                    code,
                    ctrl: event.ctrl_key(),
                    alt: event.alt_key(),
                    super_key: event.meta_key(),
                    shift: event.shift_key(),
                    release: false,
                },
                true,
            ));
        })?;
        self.listen(
            self.input.clone().into(),
            "beforeinput",
            |runtime, event| {
                let Some(event) = event.dyn_ref::<InputEvent>() else {
                    return;
                };
                if runtime.composition.borrow().active || event.is_composing() {
                    return;
                }
                match event.input_type().as_str() {
                    "insertLineBreak" | "insertParagraph" => {
                        event.prevent_default();
                        runtime.result(runtime.key(Key::new(KeyCode::Enter), true));
                    }
                    "deleteContentBackward" | "deleteContentForward" => {
                        event.prevent_default();
                        runtime.result(runtime.key(
                            Key::new(if event.input_type() == "deleteContentBackward" {
                                KeyCode::Backspace
                            } else {
                                KeyCode::Delete
                            }),
                            false,
                        ));
                    }
                    _ => {}
                }
            },
        )?;
        self.listen(self.input.clone().into(), "input", |runtime, event| {
            if event
                .dyn_ref::<InputEvent>()
                .is_some_and(|e| e.is_composing())
            {
                return;
            }
            let data = Zeroizing::new(
                event
                    .dyn_ref::<InputEvent>()
                    .and_then(InputEvent::data)
                    .unwrap_or_else(|| runtime.input.value()),
            );
            let text = runtime.composition.borrow_mut().input(&data);
            if let Some(text) = text {
                runtime.result(runtime.paste(&text));
            }
            runtime.clear_input();
        })?;
        self.listen(
            self.input.clone().into(),
            "compositionstart",
            |runtime, _| {
                runtime.composition.borrow_mut().begin();
            },
        )?;
        self.listen(
            self.input.clone().into(),
            "compositionend",
            |runtime, event| {
                let data = Zeroizing::new(
                    event
                        .dyn_ref::<CompositionEvent>()
                        .and_then(CompositionEvent::data)
                        .unwrap_or_else(|| runtime.input.value()),
                );
                let text = runtime.composition.borrow_mut().end(&data);
                if let Some(text) = text {
                    runtime.result(runtime.paste(&text));
                }
                runtime.clear_input();
            },
        )?;
        self.listen(self.input.clone().into(), "paste", |runtime, event| {
            let Some(event) = event.dyn_ref::<ClipboardEvent>() else {
                return;
            };
            if let Some(data) = event.clipboard_data() {
                event.prevent_default();
                if let Ok(text) = data.get_data("text/plain") {
                    runtime.result(runtime.paste(&Zeroizing::new(text)));
                }
            }
        })?;
        self.listen(self.surface.clone().into(), "click", |runtime, event| {
            let Some(event) = event.dyn_ref::<MouseEvent>() else {
                return;
            };
            if !runtime.visible() {
                return;
            }
            if runtime.surface_selected() {
                return;
            }
            let bounds = runtime.surface.get_bounding_client_rect();
            let x = ((f64::from(event.client_x()) - bounds.left()) / 9.0)
                .floor()
                .max(0.0) as u16;
            let y = ((f64::from(event.client_y()) - bounds.top()) / 20.0)
                .floor()
                .max(0.0) as u16;
            let (width, height) = runtime.dimensions();
            if let Some(state) = runtime.state.borrow_mut().as_mut() {
                match coder_demo_ui::hit(state, width, height, x, y) {
                    Some(Hit::Agent(index)) => state.select_agent(Some(index)),
                    Some(Hit::Plugin(index)) => state.plugins.selected = index,
                    _ => {}
                }
            }
            runtime.result(runtime.render().and_then(|_| runtime.focus()));
        })?;
        self.listen(
            self.surface.clone().into(),
            "mousedown",
            |runtime, event| {
                if event
                    .dyn_ref::<MouseEvent>()
                    .is_some_and(|event| event.button() == 0)
                {
                    runtime.selecting.set(true);
                }
            },
        )?;
        self.listen(self.document.clone().into(), "mouseup", |runtime, _| {
            runtime.selecting.set(false);
        })?;
        self.listen(
            self.document.clone().into(),
            "selectionchange",
            |runtime, _| {
                if runtime.visible()
                    && runtime.selection_held.get()
                    && !runtime.selecting.get()
                    && !runtime.surface_selected()
                {
                    runtime.result(runtime.render());
                }
            },
        )?;
        self.listen(self.root.clone().into(), "wheel", |runtime, event| {
            let Some(event) = event.dyn_ref::<WheelEvent>() else {
                return;
            };
            if !runtime.visible() || event.delta_y() == 0.0 {
                return;
            }
            event.prevent_default();
            if let Some(state) = runtime.state.borrow_mut().as_mut() {
                state.wheel(if event.delta_y() < 0.0 { -3 } else { 3 });
            }
            runtime.result(runtime.render());
        })?;
        self.listen(self.window.clone().into(), "resize", |runtime, _| {
            if runtime.visible() {
                runtime.result(runtime.render());
            }
        })?;
        self.listen(
            self.document.clone().into(),
            "visibilitychange",
            |runtime, _| {
                if runtime.document.hidden() {
                    runtime.pause();
                } else {
                    runtime.result(runtime.resume());
                }
            },
        )?;
        self.listen(self.window.clone().into(), "pagehide", |runtime, _| {
            runtime.retire("Reopen the demo to start a fresh page.");
        })?;
        Ok(())
    }
}

fn code(event: &KeyboardEvent) -> Option<KeyCode> {
    Some(match event.key().as_str() {
        "Enter" => KeyCode::Enter,
        "Escape" => KeyCode::Esc,
        "Tab" if event.shift_key() => KeyCode::BackTab,
        "Tab" => KeyCode::Tab,
        "Backspace" => KeyCode::Backspace,
        "Delete" => KeyCode::Delete,
        "ArrowLeft" => KeyCode::Left,
        "ArrowRight" => KeyCode::Right,
        "ArrowUp" => KeyCode::Up,
        "ArrowDown" => KeyCode::Down,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        "F2" => KeyCode::F(2),
        text => {
            let mut chars = text.chars();
            let ch = chars.next()?;
            if chars.next().is_some() || ch.is_control() {
                return None;
            }
            KeyCode::Char(ch)
        }
    })
}

/// Content-free proof of the mounted renderer and its managed lifetime.
#[wasm_bindgen]
pub fn demo_receipt() -> String {
    DEMO.with(|slot| {
        let slot = slot.borrow();
        let Some(runtime) = slot.as_ref() else {
            return "{\"active\":false,\"browser_storage\":false}".into();
        };
        let snapshot = runtime.snapshot.borrow();
        let state = runtime.state.borrow();
        serde_json::json!({
            "active":runtime.active.get(), "paused":runtime.paused.get(),
            "selection_held":runtime.selection_held.get(),
            "frames":runtime.frames.get(), "timer":runtime.timer.borrow().is_some(),
            "columns":snapshot.as_ref().map(|s|s.width), "rows":snapshot.as_ref().map(|s|s.height),
            "cursor":snapshot.as_ref().and_then(|s|s.cursor),
            "phase":state.as_ref().map(|s|s.animation_frame),
            "elapsed_seconds":state.as_ref().map(|s|s.elapsed_seconds),
            "selected_agent":state.as_ref().and_then(|s|s.selected_agent),
            "draft_bytes":state.as_ref().map(|s|s.draft.text.len()),
            "draft_cursor":state.as_ref().map(|s|s.draft.cursor),
            "messages":state.as_ref().map(|s|s.messages.len()),
            "renderer":"original-ratatui-cells", "browser_storage":false,
        })
        .to_string()
    })
}
