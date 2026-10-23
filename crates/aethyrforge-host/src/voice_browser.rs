//! The existing Haunt Web Speech input, adapted for scoped, cancelable studio requests.
use js_sys::{Function, Reflect};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::{JsCast, prelude::*};

pub fn supported() -> bool {
    constructor().is_some()
}
fn constructor() -> Option<Function> {
    let window = web_sys::window()?;
    ["SpeechRecognition", "webkitSpeechRecognition"]
        .iter()
        .find_map(|name| Reflect::get(&window, &(*name).into()).ok()?.dyn_into().ok())
}
pub enum SpeechEvent {
    Started,
    Words(String),
    Ended,
    Failed(String),
}
pub struct Speech {
    recognizer: JsValue,
    active: Rc<Cell<bool>>,
    callbacks: Vec<JsValue>,
}
impl Speech {
    /// Called synchronously from the person's Listen click, never from a timer or mount.
    pub fn start(mut event: impl FnMut(SpeechEvent) + 'static) -> Result<Self, JsValue> {
        let make = constructor().ok_or_else(|| {
            JsValue::from_str("Voice input isn't available here. Type your request.")
        })?;
        let recognizer = Reflect::construct(&make, &js_sys::Array::new())?;
        for (key, value) in [
            ("lang", JsValue::from_str("en-US")),
            ("interimResults", false.into()),
            ("continuous", true.into()),
            ("maxAlternatives", 1.into()),
        ] {
            Reflect::set(&recognizer, &key.into(), &value)?;
        }
        let active = Rc::new(Cell::new(true));
        let callback = Rc::new(std::cell::RefCell::new(move |e| event(e)));
        let mut callbacks = Vec::new();
        for name in ["onresult", "onend", "onerror", "onspeechstart"] {
            let active = active.clone();
            let callback = callback.clone();
            let handler = Closure::<dyn FnMut(JsValue)>::new(move |raw: JsValue| {
                if !active.get() {
                    return;
                }
                let result = match name {
                    "onspeechstart" => SpeechEvent::Started,
                    "onresult" => {
                        let index = Reflect::get(&raw, &"resultIndex".into())
                            .ok()
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.) as u32;
                        let text = Reflect::get(&raw, &"results".into())
                            .and_then(|r| Reflect::get(&r, &index.into()))
                            .and_then(|r| Reflect::get(&r, &0.into()))
                            .and_then(|r| Reflect::get(&r, &"transcript".into()))
                            .ok()
                            .and_then(|t| t.as_string())
                            .unwrap_or_default();
                        if text.trim().is_empty() || text.len() > 4800 {
                            SpeechEvent::Failed(
                                "We didn't catch a short request. Try again or type it.".into(),
                            )
                        } else {
                            SpeechEvent::Words(text.trim().into())
                        }
                    }
                    "onerror" => {
                        active.set(false);
                        let code = Reflect::get(&raw, &"error".into())
                            .ok()
                            .and_then(|v| v.as_string())
                            .unwrap_or_default();
                        SpeechEvent::Failed(match code.as_str(){
                            "not-allowed"|"service-not-allowed"=>"Microphone access was denied. Type your request or allow access in your browser.",
                            "audio-capture"=>"The microphone isn't available. Check it or type your request.",
                            "network"=>"Voice input couldn't connect. Type your request or try again.",
                            _=>"Voice input stopped. Type your request or try again.",
                        }.into())
                    }
                    _ => {
                        active.set(false);
                        SpeechEvent::Ended
                    }
                };
                callback.borrow_mut()(result);
            });
            // A queued browser event may retain this function after abort. Keep its
            // Rust closure alive through JS ownership; the inactive guard makes it inert.
            let handler = handler.into_js_value();
            Reflect::set(&recognizer, &name.into(), &handler)?;
            callbacks.push(handler);
        }
        let speech = Self {
            recognizer,
            active,
            callbacks,
        };
        speech.call("start")?;
        Ok(speech)
    }
    fn call(&self, name: &str) -> Result<(), JsValue> {
        Reflect::get(&self.recognizer, &name.into())?
            .dyn_into::<Function>()?
            .call0(&self.recognizer)?;
        Ok(())
    }
    pub fn stop(&self) {
        self.active.set(false);
        let _ = self.call("abort");
    }
}
impl Drop for Speech {
    fn drop(&mut self) {
        self.stop();
        for name in ["onresult", "onend", "onerror", "onspeechstart"] {
            let _ = Reflect::set(&self.recognizer, &name.into(), &JsValue::NULL);
        }
        let _ = &self.callbacks;
    }
}
