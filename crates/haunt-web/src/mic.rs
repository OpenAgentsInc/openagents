//! The house's mic button (#12485): what you say goes to Game Lab's chat
//! as your message, where the chat's model chooses the tool ("turn the
//! music down" becomes `haunt_audio`). Words are never matched here.
//!
//! It shows only in a house Game Lab frames on the Grid homepage, whose
//! page takes the words (`homeChatSay`), in a browser that can hear
//! (the Web Speech API, `SpeechRecognition` or `webkitSpeechRecognition`).

use std::cell::RefCell;
use std::rc::Rc;

use js_sys::{Function, Reflect};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

/// What the mic is doing.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Heard {
    #[default]
    Idle,
    Listening,
    Words(String),
    Missed,
}

pub struct Mic {
    recognizer: JsValue,
    state: Rc<RefCell<Heard>>,
    _on_result: Closure<dyn FnMut(JsValue)>,
    _on_end: Closure<dyn FnMut(JsValue)>,
}

/// The page around this one that takes words, when there is one.
fn say_function() -> Option<Function> {
    let window = web_sys::window()?;
    let parent = window.parent().ok()??;
    if js_sys::Object::is(&parent, &window) {
        return None;
    }
    Reflect::get(&parent, &"homeChatSay".into())
        .ok()?
        .dyn_into::<Function>()
        .ok()
}

impl Mic {
    /// A mic when this house is in Game Lab on the Grid and the browser can
    /// hear; otherwise none, and no button shows.
    pub fn new() -> Option<Self> {
        say_function()?;
        let window = web_sys::window()?;
        let make = ["SpeechRecognition", "webkitSpeechRecognition"]
            .iter()
            .find_map(|name| {
                Reflect::get(&window, &(*name).into())
                    .ok()?
                    .dyn_into::<Function>()
                    .ok()
            })?;
        let recognizer = Reflect::construct(&make, &js_sys::Array::new()).ok()?;
        let _ = Reflect::set(&recognizer, &"lang".into(), &"en-US".into());
        let _ = Reflect::set(&recognizer, &"interimResults".into(), &false.into());
        let _ = Reflect::set(&recognizer, &"maxAlternatives".into(), &1.into());
        let state = Rc::new(RefCell::new(Heard::Idle));
        let on_result = {
            let state = state.clone();
            Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
                let text = Reflect::get(&event, &"results".into())
                    .and_then(|r| Reflect::get(&r, &0.into()))
                    .and_then(|r| Reflect::get(&r, &0.into()))
                    .and_then(|r| Reflect::get(&r, &"transcript".into()))
                    .ok()
                    .and_then(|t| t.as_string())
                    .unwrap_or_default();
                *state.borrow_mut() = if text.trim().is_empty() {
                    Heard::Missed
                } else {
                    Heard::Words(text.trim().to_owned())
                };
            })
        };
        let on_end = {
            let state = state.clone();
            Closure::<dyn FnMut(JsValue)>::new(move |_| {
                let mut s = state.borrow_mut();
                if *s == Heard::Listening {
                    *s = Heard::Missed;
                }
            })
        };
        let _ = Reflect::set(&recognizer, &"onresult".into(), on_result.as_ref());
        let _ = Reflect::set(&recognizer, &"onend".into(), on_end.as_ref());
        let _ = Reflect::set(&recognizer, &"onerror".into(), on_end.as_ref());
        Some(Self {
            recognizer,
            state,
            _on_result: on_result,
            _on_end: on_end,
        })
    }

    #[must_use]
    pub fn listening(&self) -> bool {
        *self.state.borrow() == Heard::Listening
    }

    /// Starts listening, or stops.
    pub fn toggle(&self) {
        let method = if self.listening() { "stop" } else { "start" };
        if let Ok(f) = Reflect::get(&self.recognizer, &method.into())
            .and_then(|f| f.dyn_into::<Function>().map_err(JsValue::from))
        {
            let _ = f.call0(&self.recognizer);
        }
        if method == "start" {
            *self.state.borrow_mut() = Heard::Listening;
        }
    }

    /// What it heard since the last call, once.
    pub fn take(&self) -> Heard {
        let mut s = self.state.borrow_mut();
        match &*s {
            Heard::Words(_) | Heard::Missed => std::mem::take(&mut *s),
            other => other.clone(),
        }
    }
}

/// Sends `text` to Game Lab's chat as the visitor's message.
pub fn say(text: &str) -> bool {
    say_function().is_some_and(|f| f.call1(&JsValue::NULL, &text.into()).is_ok())
}
