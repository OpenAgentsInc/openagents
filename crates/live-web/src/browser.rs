//! The browser calls: fetches, the channel, acks, and resource timing.

use std::cell::RefCell;
use std::rc::Rc;

use js_sys::{Function, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use crate::Pointer;

/// A JavaScript error as text.
#[must_use]
pub fn js_error(value: JsValue) -> String {
    value
        .as_string()
        .or_else(|| {
            value
                .dyn_ref::<js_sys::Error>()
                .map(|e| String::from(e.message()))
        })
        .unwrap_or_else(|| format!("{value:?}"))
}

/// `performance.now()`.
#[must_use]
pub fn now() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map_or(0.0, |p| p.now())
}

/// Fetches every URL, starting all requests before waiting for any.
///
/// # Errors
/// A request failed or answered with an error status.
pub async fn fetch_all(urls: &[String]) -> Result<Vec<Vec<u8>>, String> {
    let window = web_sys::window().ok_or("no window")?;
    let requests: Vec<_> = urls
        .iter()
        .map(|url| JsFuture::from(window.fetch_with_str(url)))
        .collect();
    let mut bodies = Vec::new();
    for (request, url) in requests.into_iter().zip(urls) {
        let response: web_sys::Response = request
            .await
            .map_err(js_error)?
            .dyn_into()
            .map_err(js_error)?;
        if !response.ok() {
            return Err(format!("{url}: {}", response.status()));
        }
        bodies.push(JsFuture::from(response.array_buffer().map_err(js_error)?));
    }
    let mut out = Vec::new();
    for body in bodies {
        out.push(Uint8Array::new(&body.await.map_err(js_error)?).to_vec());
    }
    Ok(out)
}

/// Fetches one URL as text.
///
/// # Errors
/// The request failed, or the body isn't UTF-8.
pub async fn fetch_text(url: &str) -> Result<String, String> {
    let bytes = fetch_all(&[url.to_owned()]).await?.remove(0);
    String::from_utf8(bytes).map_err(|_| format!("{url} isn't text"))
}

#[derive(Default)]
struct Inbox {
    first: Option<Function>,
    pointers: Vec<Pointer>,
}

/// A tenant's channel: the pointer it names now, then each move.
pub struct Channel {
    pub tenant: String,
    inbox: Rc<RefCell<Inbox>>,
    /// Kept so the stream stays open.
    _stream: web_sys::EventSource,
    _on_message: Closure<dyn FnMut(web_sys::MessageEvent)>,
}

impl Channel {
    /// Opens the channel and waits for its first pointer.
    ///
    /// # Errors
    /// The host can't be reached, or it sent a bad pointer.
    pub async fn open(tenant: &str) -> Result<(Self, Pointer), String> {
        let inbox = Rc::new(RefCell::new(Inbox::default()));
        let stream = web_sys::EventSource::new(&format!("/t/{tenant}/_live")).map_err(js_error)?;
        let first = js_sys::Promise::new(&mut |resolve, _| {
            inbox.borrow_mut().first = Some(resolve);
        });
        let on_message = {
            let inbox = inbox.clone();
            Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
                let Some(text) = event.data().as_string() else {
                    return;
                };
                let Ok(pointer) = serde_json::from_str::<Pointer>(&text) else {
                    return;
                };
                let mut inbox = inbox.borrow_mut();
                if let Some(resolve) = inbox.first.take() {
                    let _ = resolve.call1(&JsValue::NULL, &JsValue::from_str(&text));
                } else {
                    inbox.pointers.push(pointer);
                }
            })
        };
        stream.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        let text = JsFuture::from(first).await.map_err(js_error)?;
        let pointer: Pointer = serde_json::from_str(&text.as_string().unwrap_or_default())
            .map_err(|e| format!("a bad pointer: {e}"))?;
        Ok((
            Self {
                tenant: tenant.to_owned(),
                inbox,
                _stream: stream,
                _on_message: on_message,
            },
            pointer,
        ))
    }

    /// The newest pointer since the last call, if the channel moved.
    pub fn latest(&self) -> Option<Pointer> {
        self.inbox.borrow_mut().pointers.drain(..).last()
    }

    /// Fetches the manifest a pointer names: its objects.
    ///
    /// # Errors
    /// The manifest doesn't load.
    pub async fn objects(&self, pointer: &Pointer) -> Result<icecream_zone::Objects, String> {
        let text = fetch_text(&format!(
            "/t/{}/_manifest/{}",
            self.tenant, pointer.manifest
        ))
        .await?;
        icecream_zone::objects(&text)
    }
}

/// Tells the host this page now shows `manifest` of `tenant`, with `note`'s
/// fields: livehost's ack stream, which deploys with `--wait` and
/// benchmarks read.
pub fn ack(tenant: &str, manifest: &str, seq: u64, kind: &str, mut note: serde_json::Value) {
    let Some(window) = web_sys::window() else {
        return;
    };
    note["tenant"] = tenant.into();
    note["manifest"] = manifest.into();
    note["seq"] = seq.into();
    note["kind"] = kind.into();
    note["page_epoch_ms"] = js_sys::Date::now().into();
    let init = web_sys::RequestInit::new();
    init.set_method("POST");
    init.set_body(&JsValue::from_str(&note.to_string()));
    let _ = window.fetch_with_str_and_init("/_livehost/ack", &init);
}

/// Bytes moved over the network for resources fetched since `since`
/// (`performance.now()`) whose address ends with one of `urls` (all
/// resources when `urls` is empty), from the browser's resource timing. A
/// cache hit moves none.
#[must_use]
pub fn transferred(urls: &[String], since: f64) -> (f64, u32) {
    let Some(performance) = web_sys::window().and_then(|w| w.performance()) else {
        return (0.0, 0);
    };
    let entries = performance.get_entries_by_type("resource");
    let mut bytes = 0.0;
    let mut files = 0;
    for entry in entries.iter() {
        let name = Reflect::get(&entry, &"name".into())
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        if !urls.is_empty() && !urls.iter().any(|u| name.ends_with(u.as_str())) {
            continue;
        }
        let start = Reflect::get(&entry, &"startTime".into())
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        if start < since {
            continue;
        }
        let size = Reflect::get(&entry, &"transferSize".into())
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        bytes += size;
        if size > 0.0 {
            files += 1;
        }
    }
    if urls.is_empty() {
        let navigation = performance.get_entries_by_type("navigation");
        for entry in navigation.iter() {
            bytes += Reflect::get(&entry, &"transferSize".into())
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
        }
    }
    (bytes, files)
}

/// Room for every chunk a long session fetches, so each update's bytes can
/// be counted.
pub fn keep_resource_timing() {
    if let Some(performance) = web_sys::window().and_then(|w| w.performance()) {
        performance.set_resource_timing_buffer_size(100_000);
    }
}
