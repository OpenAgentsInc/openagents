//! The production page's small DOM boundary. The world stays in its own frame.
use crate::*;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{Event, EventTarget, HtmlIFrameElement};
type Handler = Closure<dyn FnMut(Event)>;
#[wasm_bindgen]
pub struct BrowserDemo {
    pub(crate) host: Rc<RefCell<Option<BrowserHost>>>,
    changed: Closure<dyn FnMut()>,
    listeners: Vec<(EventTarget, &'static str, Handler)>,
}
#[wasm_bindgen]
impl BrowserDemo {
    pub fn snapshot(&self) -> String {
        self.host
            .borrow()
            .as_ref()
            .map(BrowserHost::snapshot)
            .unwrap_or_default()
    }
    pub fn reload(&self) {
        if let Some(h) = self.host.borrow().as_ref() {
            h.reload()
        }
    }
    pub fn close(&self) {
        if let Some(h) = self.host.borrow().as_ref() {
            h.close()
        }
    }
    /// Ask a character for a `reply`, `hint`, or `emote`; the nearest one
    /// when `npc` is absent. The line arrives as an `aethyr-npc` window event.
    pub fn npc_talk(&self, intent: String, npc: Option<String>) -> Result<String, JsValue> {
        let intent: TalkIntent = serde_json::from_value(serde_json::Value::String(intent))
            .map_err(|_| JsValue::from_str("Choose reply, hint, or emote."))?;
        let action = Action::Talk { npc, intent };
        action
            .validate()
            .map_err(|_| JsValue::from_str("Name a character in this world."))?;
        let text = serde_json::to_string(&action).map_err(|_| JsValue::from_str("talk"))?;
        self.host
            .borrow()
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Wait for the world to open."))?
            .request(text)
    }
    /// A chat message that starts with `@id` (for example `@ghost hello`)
    /// asks that character to reply. Other messages are not for characters.
    pub fn npc_chat(&self, text: String) -> Result<String, JsValue> {
        let id = npc_mention(&text)
            .ok_or_else(|| JsValue::from_str("Start with @ and a character's name."))?
            .to_ascii_lowercase();
        self.npc_talk("reply".into(), Some(id))
    }
}
impl Drop for BrowserDemo {
    fn drop(&mut self) {
        self.close();
        for (target, event, handler) in &self.listeners {
            let _ =
                target.remove_event_listener_with_callback(event, handler.as_ref().unchecked_ref());
        }
        let _ = &self.changed;
    }
}
#[wasm_bindgen]
pub fn mount_demo() -> Result<BrowserDemo, JsValue> {
    mount_demo_inner(false)
}
pub(crate) fn mount_demo_inner(player: bool) -> Result<BrowserDemo, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("window"))?;
    let doc = window
        .document()
        .ok_or_else(|| JsValue::from_str("document"))?;
    let get = |id| {
        doc.get_element_by_id(id)
            .ok_or_else(|| JsValue::from_str("The world page is incomplete."))
    };
    let frame = get("engine")?.dyn_into::<HtmlIFrameElement>()?;
    let url = frame
        .get_attribute("data-engine-url")
        .ok_or_else(|| JsValue::from_str("engine URL"))?;
    let config = get("engine-config")?
        .text_content()
        .ok_or_else(|| JsValue::from_str("world configuration"))?;
    let notice = get("world-notice")?;
    let host: Rc<RefCell<Option<BrowserHost>>> = Rc::new(RefCell::new(None));
    let weak = Rc::downgrade(&host);
    let mut observing_session = String::new();
    let mut npc_session = String::new();
    let mut npc_heard = 0u64;
    let changed = Closure::wrap(Box::new(move || {
        let Some(host) = weak.upgrade() else { return };
        let text = host
            .borrow()
            .as_ref()
            .map(BrowserHost::snapshot)
            .unwrap_or_default();
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
            return;
        };
        announce_npc_lines(&value, &mut npc_session, &mut npc_heard);
        if let Some(error) = value["error"].as_str() {
            notice.set_text_content(Some(error));
            let _ = notice.remove_attribute("hidden");
            let _ = notice.set_attribute("data-state", "error");
        } else if value["phase"].as_str() == Some("open") {
            if !player {
                let _ = notice.set_attribute("hidden", "");
            }
            if let Some(session) = value["session"].as_str() {
                if observing_session != session {
                    observing_session = session.into();
                    if let Some(h) = host.borrow().as_ref() {
                        let _ = h.request(r#"{"type":"observe","enabled":true}"#.into());
                    }
                }
            }
        } else {
            notice.set_text_content(Some(
                "Opening the world… The first visit can take a little longer.",
            ));
            let _ = notice.remove_attribute("hidden");
            let _ = notice.set_attribute("data-state", "opening");
        }
    }) as Box<dyn FnMut()>);
    let browser_host = BrowserHost::new(
        frame,
        url,
        config,
        changed.as_ref().unchecked_ref::<js_sys::Function>().clone(),
    )?;
    *host.borrow_mut() = Some(browser_host);
    let mut listeners = Vec::new();
    let weak = Rc::downgrade(&host);
    let retry = Closure::wrap(Box::new(move |event: Event| {
        event.prevent_default();
        if let Some(host) = weak.upgrade() {
            if let Some(h) = host.borrow().as_ref() {
                h.reload()
            }
        }
    }) as Box<dyn FnMut(Event)>);
    let target: EventTarget = get("world-retry")?.into();
    target.add_event_listener_with_callback("click", retry.as_ref().unchecked_ref())?;
    listeners.push((target, "click", retry));
    let element = doc
        .document_element()
        .ok_or_else(|| JsValue::from_str("page"))?;
    let fullscreen = Closure::wrap(Box::new(move |_: Event| {
        let _ = element.request_fullscreen();
    }) as Box<dyn FnMut(Event)>);
    let target: EventTarget = get("world-fullscreen")?.into();
    target.add_event_listener_with_callback("click", fullscreen.as_ref().unchecked_ref())?;
    listeners.push((target, "click", fullscreen));
    let weak = Rc::downgrade(&host);
    let leave = Closure::wrap(Box::new(move |_: Event| {
        if let Some(host) = weak.upgrade() {
            if let Some(h) = host.borrow().as_ref() {
                h.close()
            }
        }
    }) as Box<dyn FnMut(Event)>);
    let target: EventTarget = window.into();
    target.add_event_listener_with_callback("pagehide", leave.as_ref().unchecked_ref())?;
    listeners.push((target, "pagehide", leave));
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("window"))?;
    let reload_window = window.clone();
    let restored = Closure::wrap(Box::new(move |event: Event| {
        if js_sys::Reflect::get(event.as_ref(), &JsValue::from_str("persisted"))
            .ok()
            .and_then(|v| v.as_bool())
            == Some(true)
        {
            let _ = reload_window.location().reload();
        }
    }) as Box<dyn FnMut(Event)>);
    let target: EventTarget = window.into();
    target.add_event_listener_with_callback("pageshow", restored.as_ref().unchecked_ref())?;
    listeners.push((target, "pageshow", restored));
    Ok(BrowserDemo {
        host,
        changed,
        listeners,
    })
}

/// Each new character line becomes one `aethyr-npc` event on `window`, with
/// the `NpcLine` JSON as `detail`, so the page (for example its chat card)
/// can show it. Sequences restart with each engine session.
fn announce_npc_lines(value: &serde_json::Value, session: &mut String, heard: &mut u64) {
    let current = value["session"].as_str().unwrap_or_default();
    if current != session {
        *session = current.into();
        *heard = 0;
    }
    let Some(lines) = value["status"]["npc"]["lines"].as_array() else {
        return;
    };
    let Some(window) = web_sys::window() else {
        return;
    };
    for line in lines {
        let Some(sequence) = line["sequence"].as_u64() else {
            continue;
        };
        if sequence <= *heard {
            continue;
        }
        *heard = sequence;
        let Ok(detail) = js_sys::JSON::parse(&line.to_string()) else {
            continue;
        };
        let init = web_sys::CustomEventInit::new();
        init.set_detail(&detail);
        if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict("aethyr-npc", &init) {
            let _ = window.dispatch_event(&event);
        }
    }
}
