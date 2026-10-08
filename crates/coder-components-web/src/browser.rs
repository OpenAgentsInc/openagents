use std::cell::RefCell;
use std::rc::Rc;

use coder_ui::catalog::{self, CatalogIntent, FixtureState};
use rust_native::view::{Activation, FieldChange};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{
    Element, Event, HtmlInputElement, HtmlSelectElement, HtmlTextAreaElement, InputEvent,
    KeyboardEvent,
};

thread_local! {
    static CONTROLLER: RefCell<Option<Rc<RefCell<Controller>>>> = const { RefCell::new(None) };
}

struct Controller {
    state: FixtureState,
    root: Element,
    composing: bool,
    committed_composition: Option<(String, String)>,
}

fn document() -> Result<web_sys::Document, JsValue> {
    web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| JsValue::from_str("Document unavailable"))
}

fn element(id: &str) -> Result<Element, JsValue> {
    document()?
        .get_element_by_id(id)
        .ok_or_else(|| JsValue::from_str(&format!("Missing catalog element {id}")))
}

fn value(element: &Element) -> Option<String> {
    if let Some(input) = element.dyn_ref::<HtmlInputElement>() {
        Some(input.value())
    } else if let Some(input) = element.dyn_ref::<HtmlTextAreaElement>() {
        Some(input.value())
    } else {
        element
            .dyn_ref::<HtmlSelectElement>()
            .map(|input| input.value())
    }
}

fn target(event: &Event) -> Option<Element> {
    event.target()?.dyn_into::<Element>().ok()
}

fn activation(control: &Element) -> Result<Activation, JsValue> {
    let node = control
        .closest("[data-rn-node]")?
        .ok_or_else(|| JsValue::from_str("Control has no node"))?;
    let view = control
        .closest("[data-rn-instance]")?
        .ok_or_else(|| JsValue::from_str("Control has no view"))?;
    Ok(Activation {
        instance: view.get_attribute("data-rn-instance").unwrap_or_default(),
        revision: view
            .get_attribute("data-rn-revision")
            .unwrap_or_default()
            .parse()
            .unwrap_or(0),
        node: node.get_attribute("data-rn-node").unwrap_or_default(),
    })
}

impl Controller {
    fn fixture_key(
        &mut self,
        control: &Element,
        key: &KeyboardEvent,
    ) -> Option<Result<(), JsValue>> {
        let mut component = self
            .state
            .fields
            .get("overlay")
            .cloned()
            .unwrap_or_else(|| self.state.component.clone());
        if component == "plugins.manager" && self.state.stage == "configure" {
            component = self
                .state
                .fields
                .get("editor-kind")
                .cloned()
                .unwrap_or(component);
        }
        let editing = control.get_attribute("data-rn-action").as_deref() == Some("change");
        let action = match (component.as_str(), key.key().as_str()) {
            ("plugins.manager", "ArrowUp") => Some("plugins.previous"),
            ("plugins.manager", "ArrowDown") => Some("plugins.next"),
            ("plugins.manager", " ") => Some("plugins.toggle"),
            ("plugins.manager", "Enter") => Some("plugins.configure"),
            ("models.picker", "ArrowUp") => Some("models.previous"),
            ("models.picker", "ArrowDown") => Some("models.next"),
            ("models.picker", "Enter") => Some("models.select"),
            ("settings.acp", "ArrowUp") => Some("acp.previous"),
            ("settings.acp", "ArrowDown") => Some("acp.next"),
            ("settings.acp", " " | "Enter") => Some("acp.toggle"),
            ("settings.acp", "r") => Some("acp.refresh"),
            ("sessions.resume", "ArrowUp") => Some("resume.previous"),
            ("sessions.resume", "ArrowDown") => Some("resume.next"),
            ("sessions.resume", "PageUp") => Some("resume.page-up"),
            ("sessions.resume", "PageDown") => Some("resume.page-down"),
            ("sessions.resume", "Home") => Some("resume.home"),
            ("sessions.resume", "End") => Some("resume.end"),
            ("sessions.resume", "Enter") => Some("resume.select"),
            ("approvals.disclosure", "y" | "Y") => Some("disclosure.confirm"),
            ("approvals.disclosure", "n" | "N") => Some("disclosure.reject"),
            ("approvals.disclosure", "Escape") => Some("disclosure.cancel"),
            ("approvals.disclosure", "ArrowUp" | "PageUp") => Some("disclosure.up"),
            ("approvals.disclosure", "ArrowDown" | "PageDown") => Some("disclosure.down"),
            ("approvals.disclosure", "Home") => Some("disclosure.home"),
            ("approvals.disclosure", "End") => Some("disclosure.end"),
            (component, "Escape") if component.starts_with("settings.") => Some("settings.cancel"),
            _ => None,
        };
        if let Some(action) =
            action.filter(|_| !editing || matches!(key.key().as_str(), "Enter" | "Escape"))
        {
            key.prevent_default();
            return Some(self.dispatch(
                CatalogIntent::Action {
                    name: action.to_owned(),
                },
                None,
            ));
        }
        if editing && key.key() == "Enter" && !key.shift_key() && component.starts_with("settings.")
        {
            let node = control
                .closest("[data-rn-node]")
                .ok()
                .flatten()
                .and_then(|node| node.get_attribute("data-rn-node"))
                .unwrap_or_default();
            let next = match node.as_str() {
                "field-endpoint" => "[data-rn-node=field-key] .rn-input",
                "field-key" if component == "settings.jev" => {
                    "[data-rn-node=field-jev-model] .rn-input"
                }
                "field-key" => "[data-rn-node=field-model] .rn-input",
                _ => "[data-rn-node=save-settings]",
            };
            if let Ok(Some(next)) = self.root.query_selector(next) {
                key.prevent_default();
                return Some(
                    next.dyn_ref::<web_sys::HtmlElement>()
                        .map_or(Ok(()), |next| next.focus()),
                );
            }
        }
        None
    }

    fn refresh(&mut self) -> Result<(), JsValue> {
        let view = catalog::view(&self.state.component, &self.state.variant, &self.state)
            .validate()
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        rust_native_web::mount(&self.root, &view)?;
        self.position_dialog()?;
        let doc = document()?;
        if let Some(properties) = doc.get_element_by_id("catalog-properties") {
            let mut controls = Vec::new();
            inspect(&view.view().root, &mut controls);
            let entry = catalog::entries()
                .into_iter()
                .find(|entry| entry.id == self.state.component);
            properties.set_text_content(Some(&serde_json::to_string_pretty(&serde_json::json!({
                "component": entry,
                "properties": {"variant":self.state.variant,"columns":self.state.width,"rows":self.state.height,"phase":self.state.phase,"elapsed_seconds":self.state.elapsed,"earlier_rows":self.state.scroll},
                "view_schema": "rust-native.view.v3",
                "event_identity": ["instance", "revision", "node"],
                "controls_and_resolved_styles": controls,
            })).map_err(|error| JsValue::from_str(&error.to_string()))?));
        }
        if let Some(events) = doc.get_element_by_id("catalog-events") {
            events.set_text_content(Some(&if self.state.events.is_empty() {
                "No interaction yet.".to_owned()
            } else {
                self.state.events.join("\n")
            }));
        }
        if let Some(status) = doc.get_element_by_id("catalog-status") {
            status.set_text_content(Some(&format!(
                "Rust/Wasm interaction ready · revision {}",
                self.state.revision
            )));
            status.set_class_name("catalog-live");
        }
        if let Some(dimensions) = doc.get_element_by_id("catalog-dimensions") {
            dimensions.set_text_content(Some(&format!(
                "{} × {} cells · 9 × 20 px",
                self.state.width, self.state.height
            )));
        }
        for (id, val) in [
            ("catalog-width", self.state.width.to_string()),
            ("catalog-height", self.state.height.to_string()),
            ("catalog-phase", self.state.phase.to_string()),
            ("catalog-elapsed", self.state.elapsed.to_string()),
            ("catalog-scroll", self.state.scroll.to_string()),
            ("catalog-variant", self.state.variant.clone()),
        ] {
            if let Some(control) = doc.get_element_by_id(id) {
                if let Some(input) = control.dyn_ref::<HtmlInputElement>() {
                    input.set_value(&val);
                }
                if let Some(select) = control.dyn_ref::<HtmlSelectElement>() {
                    select.set_value(&val);
                }
            }
        }
        if let Some(link) = doc.get_element_by_id("catalog-fullscreen") {
            link.set_attribute("href", &format!("/components/{}?variant={}&width={}&height={}&phase={}&elapsed={}&scroll={}&fullscreen=true", self.state.component, self.state.variant, self.state.width, self.state.height, self.state.phase, self.state.elapsed, self.state.scroll))?;
        }
        Ok(())
    }

    fn position_dialog(&self) -> Result<(), JsValue> {
        let width = self.state.width * 9;
        let height = self.state.height * 20;
        self.root
            .set_attribute("style", &format!("width:{width}px;min-height:{height}px"))?;
        let bounds = self.root.get_bounding_client_rect();
        let popup_width = (self.state.width / 2)
            .clamp(44, 80)
            .min(self.state.width.saturating_sub(2))
            * 9;
        self.root.set_attribute("style", &format!(
            "width:{width}px;min-height:{height}px;--coder-dialog-left:{}px;--coder-dialog-top:{}px;--coder-dialog-width:{popup_width}px;--coder-preview-width:{width}px;--coder-preview-height:{height}px",
            bounds.left() + f64::from(width) / 2.0,
            bounds.top() + f64::from(height) / 2.0,
        ))
    }

    fn dispatch(&mut self, intent: CatalogIntent, input: Option<&str>) -> Result<(), JsValue> {
        let screen_before = (
            self.state.fields.get("overlay").cloned(),
            self.state.stage.clone(),
        );
        let reset = matches!(intent, CatalogIntent::Reset);
        let review_scroll = matches!(&intent, CatalogIntent::Action { name } if matches!(name.as_str(), "disclosure.up" | "disclosure.down" | "disclosure.home" | "disclosure.end"));
        if reset {
            rust_native_web::dispose(&self.root)?;
            self.composing = false;
            self.committed_composition = None;
        }
        catalog::reduce(&mut self.state, intent, input)
            .map_err(|error| JsValue::from_str(&error))?;
        self.refresh()?;
        if review_scroll {
            if let Some(viewport) = self
                .root
                .query_selector("[data-rn-node=disclosure-review]")?
            {
                viewport.set_scroll_top((self.state.scroll * 20).min(i32::MAX as usize) as i32);
            }
        }
        if reset
            || screen_before
                != (
                    self.state.fields.get("overlay").cloned(),
                    self.state.stage.clone(),
                )
        {
            self.focus_fixture()?;
        }
        Ok(())
    }

    fn focus_fixture(&self) -> Result<(), JsValue> {
        for selector in [
            "dialog [data-rn-node=field-model-search] .rn-input",
            "dialog [aria-pressed=true]",
        ] {
            if let Some(control) = self.root.query_selector(selector)? {
                if let Some(control) = control.dyn_ref::<web_sys::HtmlElement>() {
                    return focus_without_scrolling(control);
                }
            }
        }
        if let Some(field) = self.state.fields.get("focus") {
            if let Some(control) = self
                .root
                .query_selector(&format!("[data-rn-node=\"field-{field}\"] .rn-input"))?
            {
                if let Some(control) = control.dyn_ref::<web_sys::HtmlElement>() {
                    return focus_without_scrolling(control);
                }
            }
        }
        if let Some(control) = self
            .root
            .query_selector(".rn-input:enabled, [aria-pressed=true]:enabled, button:enabled")?
        {
            if let Some(control) = control.dyn_ref::<web_sys::HtmlElement>() {
                focus_without_scrolling(control)?;
            }
        }
        Ok(())
    }

    fn control(&mut self, control: &Element, action: &str) -> Result<(), JsValue> {
        let event = activation(control)?;
        let view = catalog::view(&self.state.component, &self.state.variant, &self.state)
            .validate()
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        if action == "change" {
            let input = value(control).unwrap_or_default();
            if self.committed_composition.as_ref() == Some(&(event.node.clone(), input.clone())) {
                self.committed_composition = None;
                return Ok(());
            }
            let field = FieldChange {
                activation: event,
                value: input.clone(),
            };
            let intent = view
                .change_field(&field)
                .map_err(|error| JsValue::from_str(&error.to_string()))?
                .clone();
            self.dispatch(intent, Some(&input))
        } else {
            let intent = view
                .activate(&event)
                .map_err(|error| JsValue::from_str(&error.to_string()))?
                .clone();
            self.dispatch(intent, None)
        }
    }

    fn apply_controls(&mut self) -> Result<(), JsValue> {
        let variant = element("catalog-variant")
            .ok()
            .and_then(|control| value(&control))
            .unwrap_or_else(|| self.state.variant.clone());
        let width = element("catalog-width")
            .ok()
            .and_then(|control| value(&control))
            .and_then(|value| value.parse().ok())
            .unwrap_or(self.state.width)
            .clamp(24, 160);
        let height = element("catalog-height")
            .ok()
            .and_then(|control| value(&control))
            .and_then(|value| value.parse().ok())
            .unwrap_or(self.state.height)
            .clamp(12, 80);
        let phase = (element("catalog-phase")
            .ok()
            .and_then(|control| value(&control))
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(u64::from(self.state.phase))
            % 8) as u8;
        let elapsed = element("catalog-elapsed")
            .ok()
            .and_then(|control| value(&control))
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(self.state.elapsed)
            .min(86_400);
        let changed = variant != self.state.variant;
        if changed {
            let entry = catalog::entries()
                .into_iter()
                .find(|entry| entry.id == self.state.component)
                .ok_or_else(|| JsValue::from_str("Unknown component"))?;
            if !entry.variants.iter().any(|item| item.id == variant) {
                return Err(JsValue::from_str("Unknown fixture"));
            }
            let revision = self.state.revision.saturating_add(1);
            self.state = FixtureState::default_for(&self.state.component, &variant);
            self.state.revision = revision;
        }
        self.state.phase = phase;
        self.state.elapsed = elapsed;
        self.state.scroll = element("catalog-scroll")
            .ok()
            .and_then(|control| value(&control))
            .and_then(|value| value.parse().ok())
            .unwrap_or(self.state.scroll)
            .min(500_000);
        self.dispatch(CatalogIntent::Resize { width, height }, None)?;
        if changed {
            self.focus_fixture()?;
        }
        Ok(())
    }
}

fn focus_without_scrolling(control: &web_sys::HtmlElement) -> Result<(), JsValue> {
    let options = web_sys::FocusOptions::new();
    options.set_prevent_scroll(true);
    control.focus_with_options(&options)
}

fn report(result: Result<(), JsValue>) {
    if let Err(error) = result {
        if let Ok(doc) = document() {
            if let Some(status) = doc.get_element_by_id("catalog-status") {
                status.set_text_content(Some(
                    &error
                        .as_string()
                        .unwrap_or_else(|| "Fixture action refused".to_owned()),
                ));
                status.set_class_name("catalog-error");
            }
        }
    }
}

fn listen(
    target: &web_sys::EventTarget,
    name: &str,
    handler: impl FnMut(Event) + 'static,
) -> Result<(), JsValue> {
    let callback = Closure::<dyn FnMut(Event)>::new(handler);
    target.add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())?;
    callback.forget();
    Ok(())
}

fn listen_capture(
    target: &web_sys::EventTarget,
    name: &str,
    handler: impl FnMut(Event) + 'static,
) -> Result<(), JsValue> {
    let callback = Closure::<dyn FnMut(Event)>::new(handler);
    target.add_event_listener_with_callback_and_bool(
        name,
        callback.as_ref().unchecked_ref(),
        true,
    )?;
    callback.forget();
    Ok(())
}

/// Mount synthetic fixtures and their local Rust event controller once.
#[wasm_bindgen]
pub fn start() -> Result<(), JsValue> {
    if CONTROLLER.with(|slot| slot.borrow().is_some()) {
        return Ok(());
    }
    let initial = element("catalog-initial")?
        .text_content()
        .unwrap_or_default();
    let state: FixtureState =
        serde_json::from_str(&initial).map_err(|error| JsValue::from_str(&error.to_string()))?;
    let controller = Rc::new(RefCell::new(Controller {
        state,
        root: element("catalog-preview")?,
        composing: false,
        committed_composition: None,
    }));
    controller.borrow_mut().refresh()?;
    controller.borrow().focus_fixture()?;
    CONTROLLER.with(|slot| *slot.borrow_mut() = Some(controller.clone()));
    let root: web_sys::EventTarget = controller.borrow().root.clone().into();
    let owned = controller.clone();
    listen(&root, "click", move |event| {
        let Some(target) = target(&event) else {
            return;
        };
        let Ok(Some(control)) = target.closest("[data-rn-action]") else {
            return;
        };
        let action = control.get_attribute("data-rn-action").unwrap_or_default();
        if matches!(action.as_str(), "activate" | "dismiss") {
            event.prevent_default();
            report(owned.borrow_mut().control(&control, &action));
        }
    })?;
    let owned = controller.clone();
    listen(&root, "input", move |event| {
        if owned.borrow().composing
            || event
                .dyn_ref::<InputEvent>()
                .is_some_and(|event| event.is_composing())
        {
            return;
        }
        let Some(control) = target(&event) else {
            return;
        };
        if control.get_attribute("data-rn-action").as_deref() == Some("change") {
            report(owned.borrow_mut().control(&control, "change"));
        }
    })?;
    let owned = controller.clone();
    listen(&root, "compositionstart", move |_| {
        if let Ok(mut controller) = owned.try_borrow_mut() {
            controller.composing = true;
        }
    })?;
    let owned = controller.clone();
    listen(&root, "compositionend", move |event| {
        let Some(control) = target(&event) else {
            return;
        };
        // A retiring editor can end composition synchronously during a mount.
        // That lifetime is already being discarded; it cannot update the new view.
        let Ok(mut controller) = owned.try_borrow_mut() else {
            return;
        };
        controller.composing = false;
        let committed = activation(&control)
            .ok()
            .and_then(|event| value(&control).map(|value| (event.node, value)));
        report(controller.control(&control, "change"));
        controller.committed_composition = committed;
    })?;
    let owned = controller.clone();
    listen_capture(&root, "cancel", move |event| {
        let Some(dialog) = target(&event) else {
            return;
        };
        let Ok(Some(close)) = dialog.query_selector("[data-rn-action=dismiss]") else {
            return;
        };
        event.prevent_default();
        report(owned.borrow_mut().control(&close, "dismiss"));
    })?;
    let owned = controller.clone();
    listen_capture(&root, "scroll", move |event| {
        let Some(viewport) = target(&event) else {
            return;
        };
        if viewport.get_attribute("data-rn-node").as_deref() != Some("disclosure-review") {
            return;
        }
        if viewport.scroll_height() - viewport.client_height() - viewport.scroll_top() > 2 {
            return;
        }
        let Ok(mut controller) = owned.try_borrow_mut() else {
            return;
        };
        if controller
            .state
            .flags
            .get("reviewed")
            .copied()
            .unwrap_or(false)
        {
            return;
        }
        report(controller.dispatch(
            CatalogIntent::Action {
                name: "review-complete".to_owned(),
            },
            None,
        ));
    })?;
    let owned = controller.clone();
    listen(&root, "keydown", move |event| {
        let Some(key) = event.dyn_ref::<KeyboardEvent>() else {
            return;
        };
        if owned.borrow().composing || key.is_composing() {
            return;
        }
        let Some(control) = target(&event) else {
            return;
        };
        let handled = owned.borrow_mut().fixture_key(&control, key);
        if let Some(result) = handled {
            report(result);
            return;
        }
        if key.key() == "Escape" {
            if let Ok(Some(dialog)) = control.closest("[role=dialog]") {
                if let Ok(Some(close)) = dialog.query_selector("[data-rn-action=dismiss]") {
                    event.prevent_default();
                    report(owned.borrow_mut().control(&close, "dismiss"));
                }
            } else if owned.borrow().state.fields.contains_key("overlay") {
                event.prevent_default();
                report(owned.borrow_mut().dispatch(
                    CatalogIntent::Action {
                        name: "close-overlay".to_owned(),
                    },
                    None,
                ));
            } else if owned
                .borrow()
                .state
                .flags
                .get("busy")
                .copied()
                .unwrap_or(false)
            {
                event.prevent_default();
                report(owned.borrow_mut().dispatch(
                    CatalogIntent::Action {
                        name: "stop".to_owned(),
                    },
                    None,
                ));
            }
        } else if key.key() == "F2" && owned.borrow().state.component == "screen.main" {
            event.prevent_default();
            let name = if owned
                .borrow()
                .state
                .fields
                .get("overlay")
                .is_some_and(|overlay| overlay == "plugins.manager")
            {
                "close-overlay"
            } else {
                "open-plugins"
            };
            report(owned.borrow_mut().dispatch(
                CatalogIntent::Action {
                    name: name.to_owned(),
                },
                None,
            ));
        } else if key.key() == "Tab"
            && !key.shift_key()
            && control
                .closest("[data-rn-node$=\"-composer-draft\"]")
                .ok()
                .flatten()
                .is_some()
        {
            let selected = owned
                .borrow()
                .root
                .query_selector("[data-rn-node^=\"main-slash-\"][aria-pressed=\"true\"]")
                .ok()
                .flatten();
            if let Some(selected) = selected {
                event.prevent_default();
                report(owned.borrow_mut().control(&selected, "activate"));
            }
        } else if matches!(key.key().as_str(), "ArrowUp" | "ArrowDown")
            && owned
                .borrow()
                .root
                .query_selector("[data-rn-node=main-slash]")
                .ok()
                .flatten()
                .is_some()
        {
            event.prevent_default();
            report(
                owned.borrow_mut().dispatch(
                    CatalogIntent::Action {
                        name: if key.key() == "ArrowUp" {
                            "slash.previous"
                        } else {
                            "slash.next"
                        }
                        .to_owned(),
                    },
                    None,
                ),
            );
        } else if matches!(key.key().as_str(), "PageUp" | "PageDown")
            && owned.borrow().state.component == "screen.main"
            && !owned.borrow().state.fields.contains_key("overlay")
        {
            event.prevent_default();
            let page = owned.borrow().state.height.saturating_sub(10) as i16;
            report(owned.borrow_mut().dispatch(
                CatalogIntent::Scroll {
                    delta: if key.key() == "PageUp" { page } else { -page },
                },
                None,
            ));
        } else if matches!(key.key().as_str(), "ArrowUp" | "ArrowDown")
            && control.get_attribute("data-rn-action").as_deref() == Some("activate")
        {
            let current = owned.borrow().state.selected;
            let next = if key.key() == "ArrowUp" {
                current.saturating_sub(1)
            } else {
                current.saturating_add(1)
            };
            event.prevent_default();
            report(
                owned
                    .borrow_mut()
                    .dispatch(CatalogIntent::Select { index: next }, None),
            );
        } else if key.key() == "Enter"
            && !key.shift_key()
            && control
                .closest("[data-rn-node=\"composer-draft\"], [data-rn-node$=\"-composer-draft\"]")
                .ok()
                .flatten()
                .is_some()
        {
            event.prevent_default();
            report(owned.borrow_mut().dispatch(
                CatalogIntent::Action {
                    name: "send".to_owned(),
                },
                None,
            ));
        } else if key.key() == "Enter"
            && control.get_attribute("data-rn-action").as_deref() == Some("change")
        {
            let component = owned
                .borrow()
                .state
                .fields
                .get("overlay")
                .cloned()
                .unwrap_or_else(|| owned.borrow().state.component.clone());
            let name = if component == "models.picker" {
                Some("models.select")
            } else if component.starts_with("settings.") || component == "plugins.manager" {
                Some("settings.save")
            } else {
                None
            };
            if let Some(name) = name {
                event.prevent_default();
                report(owned.borrow_mut().dispatch(
                    CatalogIntent::Action {
                        name: name.to_owned(),
                    },
                    None,
                ));
            }
        }
    })?;
    if let Some(window) = web_sys::window() {
        let owned = controller.clone();
        listen(window.as_ref(), "resize", move |_| {
            if let Ok(controller) = owned.try_borrow() {
                report(controller.position_dialog());
            }
        })?;
        let owned = controller.clone();
        listen_capture(window.as_ref(), "scroll", move |_| {
            if let Ok(controller) = owned.try_borrow() {
                report(controller.position_dialog());
            }
        })?;
    }
    if let Ok(controls) = element("catalog-controls") {
        let owned = controller.clone();
        listen(controls.as_ref(), "submit", move |event| {
            event.prevent_default();
            report(owned.borrow_mut().apply_controls());
        })?;
    }
    if let Ok(reset) = element("catalog-reset") {
        let owned = controller.clone();
        listen(reset.as_ref(), "click", move |_| {
            report(owned.borrow_mut().dispatch(CatalogIntent::Reset, None));
        })?;
    }
    if let Ok(tick) = element("catalog-tick") {
        let owned = controller.clone();
        listen(tick.as_ref(), "click", move |_| {
            report(owned.borrow_mut().dispatch(CatalogIntent::Tick, None));
        })?;
    }
    if let Ok(search) = element("catalog-search") {
        listen(search.as_ref(), "input", move |event| {
            let query = target(&event)
                .and_then(|control| value(&control))
                .unwrap_or_default()
                .to_lowercase();
            let Ok(doc) = document() else {
                return;
            };
            let Ok(items) = doc.query_selector_all("[data-catalog-entry]") else {
                return;
            };
            for index in 0..items.length() {
                if let Some(item) = items
                    .item(index)
                    .and_then(|node| node.dyn_into::<Element>().ok())
                {
                    let matches = item
                        .get_attribute("data-search")
                        .unwrap_or_default()
                        .contains(&query);
                    if matches {
                        let _ = item.remove_attribute("hidden");
                    } else {
                        let _ = item.set_attribute("hidden", "");
                    }
                }
            }
        })?;
    }
    Ok(())
}

fn inspect(node: &rust_native::view::Node<CatalogIntent>, out: &mut Vec<serde_json::Value>) {
    use rust_native::view::Element;
    let control = match &node.element {
        Element::Button {
            label,
            enabled,
            intent,
            ..
        }
        | Element::Choice {
            label,
            enabled,
            intent,
            ..
        } => Some(serde_json::json!({"label":label,"enabled":enabled,"intent":intent})),
        Element::Field {
            label,
            secret,
            max_bytes,
            enabled,
            on_change,
            ..
        } => Some(
            serde_json::json!({"label":label,"secret":secret,"max_bytes":max_bytes,"enabled":enabled,"on_change":on_change}),
        ),
        Element::Dialog {
            label, on_close, ..
        } => Some(serde_json::json!({"label":label,"on_close":on_close})),
        _ => None,
    };
    out.push(serde_json::json!({"node":node.key,"style":node.style,"control":control}));
    match &node.element {
        Element::Stack { children, .. }
        | Element::List { children, .. }
        | Element::Choice { children, .. }
        | Element::Dialog { children, .. }
        | Element::Transcript { children, .. }
        | Element::Message { children, .. }
        | Element::Tool { children, .. } => {
            for child in children {
                inspect(child, out);
            }
        }
        _ => {}
    }
}
