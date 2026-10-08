//! The page's private lifetime and serial, bounded standing checks.
use crate::resource::{MAX_RESOURCE_BYTES, Resource};
use crate::{MAX_STANDING_BYTES, Privacy};
use futures_util::future::{Either, select};
use gloo_timers::future::TimeoutFuture;
use js_sys::{Reflect, Uint8Array};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast, closure::Closure, prelude::*};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    AbortController, Document, Element, Event, EventTarget, HtmlInputElement, HtmlTextAreaElement,
    ReadableStreamDefaultReader, ReferrerPolicy, Request, RequestCache, RequestCredentials,
    RequestInit, RequestMode, RequestRedirect, Response, Window,
};

const POLL_MS: u32 = 5_000;
const FETCH_MS: u32 = 8_000;
const TITLE: &str = "Workspace · OpenAgents";
type Listener = (EventTarget, &'static str, Closure<dyn FnMut(Event)>);

thread_local! {
    static ACTIVE: RefCell<Option<Rc<Runtime>>> = const { RefCell::new(None) };
    static LOGIN: RefCell<Option<Rc<Login>>> = const { RefCell::new(None) };
}

struct Runtime {
    window: Window,
    document: Document,
    private: Element,
    resume: Element,
    privacy: RefCell<Privacy>,
    resource: RefCell<Option<Resource>>,
    pending: RefCell<Option<AbortController>>,
    listeners: RefCell<Vec<Listener>>,
    revealed: Cell<bool>,
}

struct Login {
    document: Document,
    root: Element,
    listeners: RefCell<Vec<Listener>>,
}

fn now() -> u64 {
    (js_sys::Date::now() / 1000.0) as u64
}

fn failure() -> JsValue {
    JsValue::from_str("The workspace needs fresh server admission. Reopen the workspace.")
}

/// Start the privacy guard. This grants no product, host, or provider rights.
#[wasm_bindgen]
pub fn start() -> Result<(), JsValue> {
    if ACTIVE.with(|active| active.borrow().is_some())
        || LOGIN.with(|login| login.borrow().is_some())
    {
        return Err(failure());
    }
    let window = web_sys::window().ok_or_else(failure)?;
    let document = window.document().ok_or_else(failure)?;
    let root = document.query_selector(".cloud")?.ok_or_else(failure)?;
    let Some(private) = document.get_element_by_id("cloud-private") else {
        return login(window, document, root);
    };
    private.set_attribute("hidden", "")?;
    let resume = document
        .get_element_by_id("cloud-resume")
        .ok_or_else(failure)?;
    let bytes = document
        .get_element_by_id("cloud-standing")
        .and_then(|standing| standing.text_content())
        .unwrap_or_default();
    let privacy = Privacy::admit(bytes.as_bytes(), now());
    let resource_bytes = document
        .get_element_by_id("cloud-resource-standing")
        .map(|element| element.text_content().unwrap_or_default());
    let resource = resource_bytes
        .as_deref()
        .and_then(|bytes| Resource::admit(bytes.as_bytes()));
    let invalid_resource = resource_bytes.is_some() && resource.is_none();
    let runtime = Rc::new(Runtime {
        window,
        document,
        private,
        resume,
        privacy: RefCell::new(privacy.unwrap_or_default()),
        resource: RefCell::new(resource),
        pending: RefCell::new(None),
        listeners: RefCell::new(Vec::new()),
        revealed: Cell::new(false),
    });
    if !runtime.visible_and_admitted() || invalid_resource {
        runtime.retire();
        return Err(failure());
    }
    for (target, name) in [
        (runtime.document.clone().into(), "visibilitychange"),
        (runtime.window.clone().into(), "pagehide"),
        (runtime.window.clone().into(), "pageshow"),
    ] {
        runtime.listen(target, name)?;
    }
    ACTIVE.with(|active| *active.borrow_mut() = Some(runtime.clone()));
    spawn_local(async move { runtime.poll().await });
    Ok(())
}

fn wipe_fields(root: &Element, selector: &str) {
    if let Ok(inputs) = root.query_selector_all(selector) {
        for index in 0..inputs.length() {
            let Some(node) = inputs.item(index) else {
                continue;
            };
            if let Some(input) = node.dyn_ref::<HtmlInputElement>() {
                input.set_value("");
                input.set_default_value("");
            }
            if let Some(input) = node.dyn_ref::<HtmlTextAreaElement>() {
                input.set_value("");
                let _ = input.set_default_value("");
            }
        }
    }
}

fn login(window: Window, document: Document, root: Element) -> Result<(), JsValue> {
    let runtime = Rc::new(Login {
        document,
        root,
        listeners: RefCell::new(Vec::new()),
    });
    for (target, name) in [
        (
            EventTarget::from(runtime.document.clone()),
            "visibilitychange",
        ),
        (EventTarget::from(window.clone()), "pagehide"),
        (EventTarget::from(window), "pageshow"),
    ] {
        let weak = Rc::downgrade(&runtime);
        let callback = Closure::wrap(Box::new(move |_: Event| {
            if let Some(runtime) = weak.upgrade()
                && (name != "visibilitychange" || runtime.document.hidden())
            {
                wipe_fields(&runtime.root, "input[type=password]");
            }
        }) as Box<dyn FnMut(Event)>);
        target.add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())?;
        runtime
            .listeners
            .borrow_mut()
            .push((target, name, callback));
    }
    if runtime.document.hidden() {
        wipe_fields(&runtime.root, "input[type=password]");
    }
    runtime.root.set_attribute("data-cloud-privacy-ready", "")?;
    LOGIN.with(|login| *login.borrow_mut() = Some(runtime));
    Ok(())
}

impl Drop for Login {
    fn drop(&mut self) {
        for (target, name, callback) in self.listeners.get_mut().drain(..) {
            let _ =
                target.remove_event_listener_with_callback(name, callback.as_ref().unchecked_ref());
        }
    }
}

impl Runtime {
    fn visible_and_admitted(&self) -> bool {
        !self.document.hidden() && self.privacy.borrow().active(now())
    }

    fn listen(self: &Rc<Self>, target: EventTarget, name: &'static str) -> Result<(), JsValue> {
        let weak = Rc::downgrade(self);
        let callback = Closure::wrap(Box::new(move |_: Event| {
            if let Some(runtime) = weak.upgrade()
                && (name == "pagehide" || !runtime.visible_and_admitted())
            {
                runtime.retire();
            }
        }) as Box<dyn FnMut(Event)>);
        target.add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())?;
        self.listeners.borrow_mut().push((target, name, callback));
        Ok(())
    }

    /// Clear first, then present a safe navigation that rechecks server admission.
    fn retire(&self) {
        self.privacy.borrow_mut().retire();
        self.resource.borrow_mut().take();
        if let Some(controller) = self.pending.borrow_mut().take() {
            controller.abort();
        }
        wipe_fields(&self.private, "input, textarea");
        self.private.set_text_content(None);
        let _ = self.private.set_attribute("hidden", "");
        for id in ["cloud-standing", "cloud-resource-standing"] {
            if let Some(standing) = self.document.get_element_by_id(id) {
                standing.set_text_content(None);
            }
        }
        self.document.set_title(TITLE);
        let _ = self.resume.remove_attribute("hidden");
    }

    fn remaining_ms(&self, ceiling: u32) -> u32 {
        self.privacy
            .borrow()
            .expires_at()
            .map(|until| {
                ((until as f64 * 1000.0 - js_sys::Date::now()).max(0.0) as u64)
                    .min(u64::from(ceiling)) as u32
            })
            .unwrap_or(0)
    }

    async fn poll(self: Rc<Self>) {
        loop {
            if !self.visible_and_admitted() {
                self.retire();
                return;
            }
            let accepted = self
                .read("/cloud/app/session", MAX_STANDING_BYTES)
                .await
                .is_some_and(|bytes| self.privacy.borrow_mut().refresh(&bytes, now()));
            if !accepted {
                self.retire();
                return;
            }
            let endpoint = self
                .resource
                .borrow()
                .as_ref()
                .map(|resource| resource.endpoint().to_owned());
            if let Some(endpoint) = endpoint {
                let accepted =
                    self.read(&endpoint, MAX_RESOURCE_BYTES)
                        .await
                        .is_some_and(|bytes| {
                            self.resource
                                .borrow()
                                .as_ref()
                                .is_some_and(|resource| resource.accepts(&bytes))
                        });
                if !accepted {
                    self.retire();
                    return;
                }
            }
            if !self.revealed.replace(true) {
                if self.private.remove_attribute("hidden").is_err()
                    || self.resume.set_attribute("hidden", "").is_err()
                {
                    self.retire();
                    return;
                }
            }
            TimeoutFuture::new(self.remaining_ms(POLL_MS)).await;
        }
    }

    async fn read(&self, endpoint: &str, maximum: usize) -> Option<Vec<u8>> {
        if !self.visible_and_admitted() {
            return None;
        }
        let controller = AbortController::new().ok()?;
        *self.pending.borrow_mut() = Some(controller.clone());
        let result = select(
            Box::pin(fetch_standing(&self.window, &controller, endpoint, maximum)),
            Box::pin(TimeoutFuture::new(self.remaining_ms(FETCH_MS))),
        )
        .await;
        self.pending.borrow_mut().take();
        controller.abort();
        match result {
            Either::Left((Ok(bytes), _)) if self.visible_and_admitted() => Some(bytes),
            _ => None,
        }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        for (target, name, callback) in self.listeners.get_mut().drain(..) {
            let _ =
                target.remove_event_listener_with_callback(name, callback.as_ref().unchecked_ref());
        }
        if let Some(controller) = self.pending.get_mut().take() {
            controller.abort();
        }
    }
}

async fn fetch_standing(
    window: &Window,
    controller: &AbortController,
    endpoint: &str,
    maximum: usize,
) -> Result<Vec<u8>, JsValue> {
    let init = RequestInit::new();
    init.set_method("GET");
    init.set_credentials(RequestCredentials::SameOrigin);
    init.set_mode(RequestMode::SameOrigin);
    init.set_cache(RequestCache::NoStore);
    init.set_redirect(RequestRedirect::Error);
    init.set_referrer_policy(ReferrerPolicy::NoReferrer);
    init.set_signal(Some(&controller.signal()));
    let request = Request::new_with_str_and_init(endpoint, &init)?;
    request.headers().set("Accept", "application/json")?;
    let response: Response = JsFuture::from(window.fetch_with_request(&request))
        .await?
        .dyn_into()?;
    if !response.ok() || response.redirected() {
        return Err(failure());
    }
    let content_type = response.headers().get("Content-Type")?.unwrap_or_default();
    if content_type.split(';').next().map(str::trim) != Some("application/json") {
        return Err(failure());
    }
    if let Some(length) = response.headers().get("Content-Length")?
        && length
            .parse::<usize>()
            .ok()
            .is_none_or(|length| length > maximum)
    {
        return Err(failure());
    }
    let reader: ReadableStreamDefaultReader = response
        .body()
        .ok_or_else(failure)?
        .get_reader()
        .dyn_into()?;
    let mut bytes = Vec::new();
    loop {
        let part = JsFuture::from(reader.read()).await?;
        let done = Reflect::get(&part, &JsValue::from_str("done"))?
            .as_bool()
            .ok_or_else(failure)?;
        if done {
            reader.release_lock();
            return Ok(bytes);
        }
        let chunk: Uint8Array = Reflect::get(&part, &JsValue::from_str("value"))?.dyn_into()?;
        if bytes.len().saturating_add(chunk.length() as usize) > maximum {
            let _ = reader.cancel();
            reader.release_lock();
            return Err(failure());
        }
        let start = bytes.len();
        bytes.resize(start + chunk.length() as usize, 0);
        chunk.copy_to(&mut bytes[start..]);
    }
}
