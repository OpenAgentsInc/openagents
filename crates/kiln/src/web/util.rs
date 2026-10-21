//! Small browser helpers: time, sleep, random bytes, and property access.

use js_sys::Reflect;
use wasm_bindgen::prelude::*;

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

/// A timer that clears its `setTimeout` when dropped, so a timer that
/// loses a race never calls into a dropped closure.
pub struct Sleep {
    rx: futures_channel::oneshot::Receiver<()>,
    id: JsValue,
    _callback: Closure<dyn FnMut()>,
}

impl Drop for Sleep {
    fn drop(&mut self) {
        let global = js_sys::global();
        if let Ok(clear) = Reflect::get(&global, &"clearTimeout".into())
            && let Ok(clear) = clear.dyn_into::<js_sys::Function>()
        {
            let _ = clear.call1(&global, &self.id);
        }
    }
}

impl std::future::Future for Sleep {
    type Output = ();
    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<()> {
        std::pin::Pin::new(&mut self.rx).poll(cx).map(|_| ())
    }
}

/// Wait `ms` milliseconds.
pub fn sleep(ms: i32) -> Sleep {
    let (tx, rx) = futures_channel::oneshot::channel();
    let mut tx = Some(tx);
    let callback = Closure::<dyn FnMut()>::new(move || {
        if let Some(tx) = tx.take() {
            let _ = tx.send(());
        }
    });
    let global = js_sys::global();
    let id = Reflect::get(&global, &"setTimeout".into())
        .ok()
        .and_then(|f| f.dyn_into::<js_sys::Function>().ok())
        .and_then(|f| f.call2(&global, callback.as_ref(), &JsValue::from(ms)).ok())
        .unwrap_or(JsValue::UNDEFINED);
    Sleep {
        rx,
        id,
        _callback: callback,
    }
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
