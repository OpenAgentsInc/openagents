//! A transcript slot for a visual drawn by the separately downloaded module.
use std::{cell::RefCell, collections::HashMap};

use gpui::{prelude::*, *};
use wasm_bindgen::{JsCast, prelude::*};

#[derive(Default)]
pub struct Frames(RefCell<HashMap<String, (String, Entity<Slot>)>>);

impl Frames {
    pub fn render<T: 'static>(
        &self,
        key: String,
        title: &str,
        path: &str,
        device: Option<String>,
        changed: impl Fn(&mut T, &mut Context<T>) + 'static,
        cx: &mut Context<T>,
    ) -> AnyElement {
        let identity = format!("{device:?}\n{path}");
        let mut frames = self.0.borrow_mut();
        if frames.get(&key).is_some_and(|(old, _)| old != &identity) {
            frames.remove(&key);
        }
        let (_, slot) = frames.entry(key).or_insert_with(|| {
            let slot = cx.new(|cx| Slot::new(title.to_owned(), path.to_owned(), device, cx));
            cx.subscribe(&slot, move |this, _, _: &Resized, cx| changed(this, cx))
                .detach();
            (identity, slot)
        });
        slot.clone().into_any_element()
    }

    pub fn clear(&self) {
        self.0.borrow_mut().clear();
    }
}

struct Resized;
struct Slot {
    id: String,
    title: String,
    path: String,
    height: f32,
    device: Option<String>,
    listener: Closure<dyn FnMut(web_sys::CustomEvent)>,
    failed: bool,
    failure: Closure<dyn FnMut(web_sys::Event)>,
}
impl EventEmitter<Resized> for Slot {}

fn signal(value: serde_json::Value) {
    let Some(window) = web_sys::window() else {
        return;
    };
    if !js_sys::Reflect::get(&window, &"__oaVisualReady".into())
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        let pending = js_sys::Reflect::get(&window, &"__oaVisualPending".into())
            .unwrap_or(JsValue::UNDEFINED);
        let pending = if pending.is_object() {
            pending
        } else {
            js_sys::Object::new().into()
        };
        let key = JsValue::from_str(value["id"].as_str().unwrap_or_default());
        if value["remove"] == true || value["hide"] == true {
            let _ = js_sys::Reflect::delete_property(&js_sys::Object::from(pending), &key);
            return;
        }
        let _ = js_sys::Reflect::set(&pending, &key, &value.to_string().into());
        let _ = js_sys::Reflect::set(&window, &"__oaVisualPending".into(), &pending);
        if let Some(document) = window.document() {
            if document.get_element_by_id("oa-visual-loader").is_none() {
                if let Ok(script) = document.create_element("script") {
                    script.set_id("oa-visual-loader");
                    let _ = script.set_attribute("type", "module");
                    let url = js_sys::Reflect::get(&window, &"__oaVisualUrl".into())
                        .ok()
                        .and_then(|v| v.as_string())
                        .unwrap_or_else(|| "/gpui/build/visuals/start.js".into());
                    let _ = script.set_attribute("src", &url);
                    let on_error = Closure::once_into_js(move |_: web_sys::Event| {
                        if let Some(window) = web_sys::window() {
                            let _ = js_sys::Reflect::set(
                                &window,
                                &"__oaVisualFailed".into(),
                                &JsValue::TRUE,
                            );
                            if let Ok(event) = web_sys::Event::new("oa-visual-failed") {
                                window.dispatch_event(&event).ok();
                            }
                        }
                    });
                    let _ =
                        script.add_event_listener_with_callback("error", on_error.unchecked_ref());
                    if let Some(body) = document.body() {
                        let _ = body.append_child(&script);
                    }
                }
            }
        }
        return;
    }
    let init = web_sys::CustomEventInit::new();
    init.set_detail(&JsValue::from_str(&value.to_string()));
    if let Some(window) = web_sys::window()
        && let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict("oa-visual-slot", &init)
    {
        let _ = window.dispatch_event(&event);
    }
}

impl Slot {
    fn new(title: String, path: String, device: Option<String>, cx: &mut Context<Self>) -> Self {
        let id = format!("oa-visual-{}", cx.entity_id());
        let weak = cx.entity().downgrade();
        let mut async_cx = cx.to_async();
        let own_id = id.clone();
        let listener =
            Closure::<dyn FnMut(web_sys::CustomEvent)>::new(move |event: web_sys::CustomEvent| {
                let Some(detail) = event.detail().as_string() else {
                    return;
                };
                let Ok(value) = serde_json::from_str::<serde_json::Value>(&detail) else {
                    return;
                };
                if value["id"].as_str() != Some(&own_id) {
                    return;
                }
                let Some(height) = value["height"].as_f64() else {
                    return;
                };
                let height = (height as f32).clamp(32., 4096.);
                let _ = weak.update(&mut async_cx, |slot, cx| {
                    if (slot.height - height).abs() > 1. {
                        slot.height = height;
                        cx.emit(Resized);
                        cx.notify();
                    }
                });
            });
        let weak = cx.entity().downgrade();
        let mut async_cx = cx.to_async();
        let failure = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            let _ = weak.update(&mut async_cx, |slot, cx| {
                slot.failed = true;
                slot.height = 48.;
                cx.emit(Resized);
                cx.notify();
            });
        });
        if let Some(window) = web_sys::window() {
            let _ = window.add_event_listener_with_callback(
                "oa-visual-failed",
                failure.as_ref().unchecked_ref(),
            );
            let _ = window.add_event_listener_with_callback(
                "oa-visual-height",
                listener.as_ref().unchecked_ref(),
            );
        }
        Self {
            id,
            title,
            path,
            device,
            height: 320.,
            listener,
            failed: web_sys::window().is_some_and(|window| {
                js_sys::Reflect::get(&window, &"__oaVisualFailed".into())
                    .ok()
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            }),
            failure,
        }
    }
}
impl Drop for Slot {
    fn drop(&mut self) {
        if let Some(window) = web_sys::window() {
            let _ = window.remove_event_listener_with_callback(
                "oa-visual-height",
                self.listener.as_ref().unchecked_ref(),
            );
            let _ = window.remove_event_listener_with_callback(
                "oa-visual-failed",
                self.failure.as_ref().unchecked_ref(),
            );
        }
        signal(serde_json::json!({"id": self.id, "remove": true}));
    }
}

/// Kept in GPUI's element state, so an offscreen or removed row hides its canvas.
struct Visible(String);
impl Drop for Visible {
    fn drop(&mut self) {
        signal(serde_json::json!({"id": self.0, "hide": true}));
    }
}

impl Render for Slot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let (id, title, path) = (self.id.clone(), self.title.clone(), self.path.clone());
        let device = self.device.clone();
        let failed = self.failed;
        div().id("visual-slot").relative().w_full().h(px(self.height)).child(
            div().absolute().inset_0().text_color(crate::theme::text_muted()).child(if failed { "Couldn’t load this visual. Reload to try again." } else { "Loading visual…" })
        ).child(
            canvas(|_, _, _| (), move |bounds, _, window, _| {
                // The element state follows virtualization, rather than a timer:
                // an unchanged frame leaves the visual visible.
                window.with_global_id(ElementId::Name(id.clone().into()), |global, window| {
                    window.with_element_state(global, |state: Option<Visible>, _| {
                        ((), state.unwrap_or_else(|| Visible(id.clone())))
                    });
                });
                let clip = window.content_mask().bounds.intersect(&bounds);
                if failed { return; }
                signal(serde_json::json!({
                    "id": id, "title": title, "path": path, "device": device,
                    "x": f32::from(bounds.origin.x), "y": f32::from(bounds.origin.y),
                    "width": f32::from(bounds.size.width), "height": f32::from(bounds.size.height),
                    "clip": [f32::from(clip.origin.x), f32::from(clip.origin.y), f32::from(clip.size.width), f32::from(clip.size.height)]
                }));
            }).size_full()
        )
    }
}
