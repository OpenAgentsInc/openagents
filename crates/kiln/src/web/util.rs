//! Small browser helpers: time, sleep, random bytes, and property access.

use js_sys::{Promise, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

/// Milliseconds on the page's monotonic clock.
pub fn now_ms() -> f64 {
    Reflect::get(&js_sys::global(), &"performance".into())
        .ok()
        .and_then(|p| p.dyn_into::<web_sys::Performance>().ok())
        .map_or_else(js_sys::Date::now, |p| p.now())
}

/// Seconds since the Unix epoch.
pub fn unix() -> u64 {
    (js_sys::Date::now() / 1000.0) as u64
}

/// Wait `ms` milliseconds.
pub async fn sleep(ms: i32) {
    let promise = Promise::new(&mut |resolve, _| {
        let global = js_sys::global();
        if let Ok(set) = Reflect::get(&global, &"setTimeout".into())
            && let Ok(set) = set.dyn_into::<js_sys::Function>()
        {
            let _ = set.call2(&global, &resolve, &JsValue::from(ms));
        }
    });
    let _ = JsFuture::from(promise).await;
}

/// 32 random bytes from the browser.
pub fn random32() -> [u8; 32] {
    let mut out = [0u8; 32];
    getrandom::fill(&mut out).unwrap_or_else(|_| wasm_bindgen::throw_str("no randomness"));
    out
}

/// `object[key]`.
pub fn get(object: &JsValue, key: &str) -> JsValue {
    Reflect::get(object, &key.into()).unwrap_or(JsValue::UNDEFINED)
}

/// `object[key] = value`.
pub fn set(object: &JsValue, key: &str, value: impl Into<JsValue>) {
    let _ = Reflect::set(object, &key.into(), &value.into());
}

/// A JavaScript error as text.
pub fn text(error: &JsValue) -> String {
    error
        .as_string()
        .or_else(|| {
            error
                .dyn_ref::<js_sys::Error>()
                .map(|e| String::from(e.message()))
        })
        .unwrap_or_else(|| format!("{error:?}"))
}
