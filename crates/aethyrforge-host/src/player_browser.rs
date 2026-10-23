//! Minimal version-pinned player. No account requests or editor grant controls.
use crate::*;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, prelude::*};
#[wasm_bindgen]
pub struct BrowserPlayer {
    demo: Rc<BrowserDemo>,
    timer: i32,
    tick: Closure<dyn FnMut()>,
    release: String,
    frame: web_sys::Element,
    load: Closure<dyn FnMut(web_sys::Event)>,
}
#[wasm_bindgen]
impl BrowserPlayer {
    pub fn snapshot(&self) -> String {
        json!({"release":self.release,"engine":serde_json::from_str::<Value>(&self.demo.snapshot()).unwrap_or(Value::Null)}).to_string()
    }
    pub fn close(&self) {
        self.demo.close();
    }
}
impl Drop for BrowserPlayer {
    fn drop(&mut self) {
        if let Some(w) = web_sys::window() {
            w.clear_interval_with_handle(self.timer);
        }
        let _ = self
            .frame
            .remove_event_listener_with_callback("load", self.load.as_ref().unchecked_ref());
        self.demo.close();
        let _ = &self.tick;
    }
}
#[wasm_bindgen]
pub fn mount_player() -> Result<BrowserPlayer, JsValue> {
    let window = web_sys::window().ok_or(JsValue::NULL)?;
    let doc = window.document().ok_or(JsValue::NULL)?;
    let node = doc
        .get_element_by_id("playable-release")
        .ok_or(JsValue::NULL)?;
    let release: PlayableRelease =
        serde_json::from_str(&node.text_content().unwrap_or_default())
            .map_err(|_| JsValue::from_str("This playable version could not be read."))?;
    release
        .validate()
        .map_err(|_| JsValue::from_str("This playable version does not match its content."))?;
    let id = release.id.clone();
    let action =
        serde_json::to_string(&Action::LoadPlayable { release }).map_err(|_| JsValue::NULL)?;
    let demo = Rc::new(crate::demo::mount_demo_inner(true)?);
    let weak = Rc::downgrade(&demo);
    let notice = doc.get_element_by_id("world-notice").ok_or(JsValue::NULL)?;
    let frame = doc.get_element_by_id("engine").ok_or(JsValue::NULL)?;
    frame.set_attribute("hidden", "")?;
    let state = Rc::new(RefCell::new((String::new(), None::<String>, false)));
    let loading_frame = frame.clone();
    let loading_notice = notice.clone();
    let load = Closure::wrap(Box::new(move |_: web_sys::Event| {
        let _ = loading_frame.set_attribute("hidden", "");
        let _ = loading_notice.remove_attribute("hidden");
        loading_notice.set_text_content(Some("Opening this playable version…"));
    }) as Box<dyn FnMut(web_sys::Event)>);
    frame.add_event_listener_with_callback("load", load.as_ref().unchecked_ref())?;
    let tick_frame = frame.clone();
    let tick = Closure::wrap(Box::new(move || {
        let Some(demo) = weak.upgrade() else { return };
        let Ok(v) = serde_json::from_str::<Value>(&demo.snapshot()) else {
            return;
        };
        let mut state = state.borrow_mut();
        let session = v["session"].as_str().unwrap_or_default();
        if state.0 != session {
            *state = (session.into(), None, false);
            let _ = tick_frame.set_attribute("hidden", "");
        }
        if v["error"].as_str().is_some() || v["phase"] == "closed" {
            state.2 = false;
            let _ = tick_frame.set_attribute("hidden", "");
            let _ = notice.remove_attribute("hidden");
            notice.set_text_content(Some(
                "This version could not open. Choose Retry to try again.",
            ));
            return;
        }
        if v["phase"] != "open" {
            state.2 = false;
            let _ = tick_frame.set_attribute("hidden", "");
            let _ = notice.remove_attribute("hidden");
            notice.set_text_content(Some("Opening this playable version…"));
            return;
        }
        if state.2 {
            return;
        }
        let _ = tick_frame.set_attribute("hidden", "");
        let _ = notice.remove_attribute("hidden");
        notice.set_text_content(Some("Opening this playable version…"));
        if let Some(request) = state.1.as_ref() {
            if let Some(receipt) = v["receipts"]
                .as_array()
                .and_then(|r| r.iter().find(|r| r["request_id"] == *request))
            {
                if receipt["outcome"]["state"] == "applied" {
                    state.2 = true;
                    let _ = tick_frame.remove_attribute("hidden");
                    let _ = notice.set_attribute("hidden", "");
                } else {
                    notice.set_text_content(Some(
                        "This version could not open. Choose Retry to try again.",
                    ));
                }
            }
        } else if let Some(host) = demo.host.borrow().as_ref() {
            match host.request(action.clone()) {
                Ok(request) => state.1 = Some(request),
                Err(_) => notice.set_text_content(Some("This playable version could not open.")),
            }
        }
    }) as Box<dyn FnMut()>);
    let timer = window.set_interval_with_callback_and_timeout_and_arguments_0(
        tick.as_ref().unchecked_ref(),
        100,
    )?;
    Ok(BrowserPlayer {
        demo,
        timer,
        tick,
        release: id,
        frame,
        load,
    })
}
