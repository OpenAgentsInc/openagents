//! The Grid's arches to hosted zones (`docs/verse/ice-cream-man-live.md`,
//! `docs/verse/games/the-haunted-house.md`).
//!
//! `?hosted=TENANT` puts an arch on the Grid that leads to a livehost
//! tenant on the same origin, such as The Ice Cream Man at `/t/icecream/`.
//! Several arches stand side by side with `?hosted=icecream,haunt` (or a
//! repeated `hosted=`); `hosted-sign=WORDS` letters them in the same order
//! (a comma list), and known tenants have their own lettering and site
//! ([`HostedPortal::list`]). As the player walks within
//! [`HOSTED_PREFETCH`] metres of an arch, the page fetches that zone's
//! first play (its shell, its logic module, its tuning, its models, and the
//! rooms near its spawn) into the browser's cache, so stepping through
//! loads from the cache. Walking through opens the zone over the Grid in a
//! frame; **Back to the Grid** closes it and puts the player in front of the
//! arch. The Grid never runs the zone's code: the zone runs in its own page.

use std::cell::RefCell;

use js_sys::Reflect;
use verse::runtime::WorldRuntime;
use verse::zones::gate::{HOSTED_PREFETCH, HostedPortal};
use wasm_bindgen::prelude::*;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, spawn_local};

#[derive(Default, Clone, Copy)]
enum Prefetch {
    #[default]
    Idle,
    Running,
    Done,
}

impl Prefetch {
    fn word(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "prefetching",
            Self::Done => "prefetched",
        }
    }
}

#[derive(Default)]
struct State {
    /// Each arch's tenant and prefetch.
    arches: Vec<(String, Prefetch)>,
    /// The open zone's tenant and its frame and button.
    overlay: Option<(String, web_sys::Element, web_sys::Element)>,
    back: bool,
    /// What checks last read from `window.verse_hosted`.
    shown: String,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// The hosted arches the page's address asks for.
#[must_use]
pub fn from_query(window: &web_sys::Window) -> Vec<HostedPortal> {
    let Some(params) = window
        .location()
        .search()
        .ok()
        .and_then(|search| web_sys::UrlSearchParams::new_with_str(&search).ok())
    else {
        return Vec::new();
    };
    let split = |name: &str| -> Vec<String> {
        params
            .get_all(name)
            .iter()
            .filter_map(|v| v.as_string())
            .flat_map(|v| v.split(',').map(str::to_owned).collect::<Vec<_>>())
            .collect()
    };
    let (arches, refused) = HostedPortal::list(&split("hosted"), &split("hosted-sign"));
    for error in refused {
        web_sys::console::warn_1(&error.into());
    }
    STATE.with_borrow_mut(|state| {
        state.arches = arches
            .iter()
            .map(|p| (p.tenant.clone(), Prefetch::Idle))
            .collect();
    });
    if !arches.is_empty() {
        listen_for_back(window);
    }
    arches
}

/// A zone page on this origin can ask to go back to the Grid itself (its
/// own in-world control) with `{"type": "hosted-back"}`.
fn listen_for_back(window: &web_sys::Window) {
    let origin = window.location().origin().unwrap_or_default();
    let on_message =
        Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
            if event.origin() != origin {
                return;
            }
            let kind = Reflect::get(&event.data(), &"type".into())
                .ok()
                .and_then(|v| v.as_string());
            if kind.as_deref() == Some("hosted-back") {
                STATE.with_borrow_mut(|state| state.back = state.overlay.is_some());
            }
        });
    let _ = window.add_event_listener_with_callback("message", on_message.as_ref().unchecked_ref());
    on_message.forget();
}

/// The hosted arches' work for one frame: prefetch a zone when the player
/// is near its arch, open it when they walk through, close it when they
/// come back. `true` while a zone is open, so the Grid ignores its own
/// input.
pub fn frame(runtime: &mut WorldRuntime) -> bool {
    let near: Vec<String> = runtime
        .hosted_distances()
        .into_iter()
        .filter(|(_, d)| *d < HOSTED_PREFETCH)
        .map(|(t, _)| t)
        .collect();
    let start: Vec<String> = STATE.with_borrow_mut(|state| {
        state
            .arches
            .iter_mut()
            .filter(|(t, p)| matches!(p, Prefetch::Idle) && near.contains(t))
            .map(|(t, p)| {
                *p = Prefetch::Running;
                t.clone()
            })
            .collect()
    });
    for tenant in start {
        spawn_local(async move {
            let result = prefetch(&tenant).await;
            if let Err(error) = result {
                web_sys::console::warn_1(&format!("prefetch {tenant}: {error}").into());
            }
            STATE.with_borrow_mut(|state| {
                if let Some((_, p)) = state.arches.iter_mut().find(|(t, _)| *t == tenant) {
                    *p = Prefetch::Done;
                }
            });
        });
    }
    // One zone at a time: a crossing while one is open is ignored.
    if let Some(portal) = runtime.take_hosted_crossing()
        && STATE.with_borrow(|state| state.overlay.is_none())
    {
        open(&portal.tenant);
    }
    let back = STATE.with_borrow_mut(|state| std::mem::take(&mut state.back));
    if back {
        close();
        let _ = runtime.return_from_hosted();
    }
    publish(runtime);
    STATE.with_borrow(|state| state.overlay.is_some())
}

/// `window.verse_hosted` for checks: the nearest arch (where it is, where
/// to stand in front of it facing it, the player's distance, its
/// prefetch, whether its zone is open), and every arch by tenant.
fn publish(runtime: &WorldRuntime) {
    let distances = runtime.hosted_distances();
    let arches: Vec<serde_json::Value> = runtime
        .hosted_gates()
        .into_iter()
        .map(|(portal, gate)| {
            let (front, away) = gate.front();
            let distance = distances
                .iter()
                .find(|(t, _)| *t == portal.tenant)
                .map_or(f32::MAX, |(_, d)| *d);
            let (prefetch, open) = STATE.with_borrow(|state| {
                (
                    state
                        .arches
                        .iter()
                        .find(|(t, _)| *t == portal.tenant)
                        .map_or("idle", |(_, p)| p.word()),
                    state
                        .overlay
                        .as_ref()
                        .is_some_and(|(t, _, _)| *t == portal.tenant),
                )
            });
            serde_json::json!({
                "tenant": portal.tenant,
                "sign": portal.sign,
                "at": [gate.at.x, gate.at.z],
                "front": [front.x, front.z, away + std::f32::consts::PI],
                "distance": (distance * 10.0).round() / 10.0,
                "near": distance < HOSTED_PREFETCH,
                "prefetch": prefetch,
                "open": open,
            })
        })
        .collect();
    let Some(nearest) = arches
        .iter()
        .min_by(|a, b| {
            let d = |v: &serde_json::Value| v["distance"].as_f64().unwrap_or(f64::MAX);
            d(a).total_cmp(&d(b))
        })
        .cloned()
    else {
        return;
    };
    let mut value = nearest;
    value["open"] = STATE.with_borrow(|state| state.overlay.is_some()).into();
    value["arches"] = arches.into();
    let text = value.to_string();
    let changed = STATE.with_borrow_mut(|state| {
        let changed = state.shown != text;
        state.shown.clone_from(&text);
        changed
    });
    if changed && let Some(window) = web_sys::window() {
        let _ = Reflect::set(&window, &"verse_hosted".into(), &JsValue::from_str(&text));
    }
}

fn js_error(value: JsValue) -> String {
    value
        .dyn_ref::<js_sys::Error>()
        .map_or_else(|| format!("{value:?}"), |e| String::from(e.message()))
}

async fn fetch_bytes(window: &web_sys::Window, urls: &[String]) -> Result<Vec<Vec<u8>>, String> {
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
        out.push(js_sys::Uint8Array::new(&body.await.map_err(js_error)?).to_vec());
    }
    Ok(out)
}

/// The channel's manifest now: the first message of the host's stream.
async fn current_manifest(tenant: &str) -> Result<String, String> {
    let stream = web_sys::EventSource::new(&format!("/t/{tenant}/_live")).map_err(js_error)?;
    let slot: std::rc::Rc<RefCell<Option<js_sys::Function>>> = std::rc::Rc::default();
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        *slot.borrow_mut() = Some(resolve);
    });
    let on_message = {
        let slot = slot.clone();
        Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
            if let Some(resolve) = slot.borrow_mut().take() {
                let _ = resolve.call1(&JsValue::NULL, &event.data());
            }
        })
    };
    stream.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    let text = JsFuture::from(promise).await.map_err(js_error)?;
    stream.close();
    drop(on_message);
    let pointer: serde_json::Value =
        serde_json::from_str(&text.as_string().unwrap_or_default()).map_err(|e| e.to_string())?;
    pointer["manifest"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "the host named no manifest".into())
}

/// Fetches the zone's first play into the cache and acks what it took.
async fn prefetch(tenant: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or("no window")?;
    let began = window.performance().map_or(0.0, |p| p.now());
    let manifest = current_manifest(tenant).await?;
    let text = fetch_bytes(&window, &[format!("/t/{tenant}/_manifest/{manifest}")]).await?;
    let objects = icecream_zone::objects(&String::from_utf8_lossy(&text[0]))?;
    let index = objects
        .get(icecream_zone::INDEX)
        .ok_or("the zone has no zone.json")?;
    let zone_text = fetch_bytes(
        &window,
        &[icecream_zone::object_url(icecream_zone::INDEX, index)],
    )
    .await?;
    let zone = icecream_zone::Zone::parse(&String::from_utf8_lossy(&zone_text[0]))?;
    // The page by its address; everything it loads by hash, which the
    // browser keeps.
    let mut urls = Vec::new();
    for path in zone.first_play() {
        if path.starts_with("templates/") {
            urls.push(format!("/t/{tenant}/"));
        } else if let Some(hash) = objects.get(&path) {
            urls.push(icecream_zone::object_url(&path, hash));
        }
    }
    let bodies = fetch_bytes(&window, &urls).await?;
    let bytes: usize = bodies.iter().map(Vec::len).sum();
    let ms = window.performance().map_or(0.0, |p| p.now()) - began;
    let ack = serde_json::json!({
        "tenant": tenant,
        "manifest": manifest,
        "kind": "prefetch",
        "files": urls.len(),
        "decoded_bytes": bytes,
        "prefetch_ms": ms.round(),
    });
    let init = web_sys::RequestInit::new();
    init.set_method("POST");
    init.set_body(&JsValue::from_str(&ack.to_string()));
    let _ = window.fetch_with_str_and_init("/_livehost/ack", &init);
    web_sys::console::info_1(&format!("Hosted zone prefetched {ack}").into());
    Ok(())
}

fn style(element: &web_sys::Element, css: &str) {
    let _ = element.set_attribute("style", css);
}

fn open(tenant: &str) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Some(body) = document.body() else {
        return;
    };
    let Ok(frame) = document.create_element("iframe") else {
        return;
    };
    let _ = frame.set_attribute("src", &format!("/t/{tenant}/"));
    let _ = frame.set_attribute("title", "Hosted zone");
    let _ = frame.set_attribute("allow", "autoplay; fullscreen; pointer-lock");
    style(
        &frame,
        "position:fixed;inset:0;width:100vw;height:100vh;border:0;z-index:2147483000;background:#2b1a12",
    );
    let Ok(back) = document.create_element("button") else {
        return;
    };
    back.set_text_content(Some("Back to the Grid"));
    style(
        &back,
        "position:fixed;top:12px;right:12px;z-index:2147483001;font:600 15px system-ui,sans-serif;\
         padding:8px 14px;border-radius:10px;border:1px solid #888;background:#111;color:#eee;cursor:pointer",
    );
    let click = Closure::<dyn FnMut()>::new(|| {
        STATE.with_borrow_mut(|state| state.back = true);
    });
    let _ = back.add_event_listener_with_callback("click", click.as_ref().unchecked_ref());
    click.forget();
    let _ = body.append_child(&frame);
    let _ = body.append_child(&back);
    if let Ok(html) = frame.clone().dyn_into::<web_sys::HtmlElement>() {
        let _ = html.focus();
    }
    let _ = Reflect::set(&frame, &"tabIndex".into(), &0.into());
    STATE.with_borrow_mut(|state| state.overlay = Some((tenant.to_owned(), frame, back)));
}

fn close() {
    if let Some((_, frame, back)) = STATE.with_borrow_mut(|state| state.overlay.take()) {
        frame.remove();
        back.remove();
    }
}
