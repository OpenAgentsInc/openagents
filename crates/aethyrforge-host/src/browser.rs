//! Browser transport. Application state stays in Rust; generated JS only loads Wasm.
use crate::*;
use js_sys::{Array, Function};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{Event, HtmlIFrameElement, MessageChannel, MessageEvent, MessagePort};

const BOOTSTRAP: &str = "openagents.aethyrforge.host/7/listening";
type Handler<T> = Closure<dyn FnMut(T)>;
fn js(e: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}
fn window() -> web_sys::Window {
    web_sys::window().expect("browser window")
}
fn now() -> u64 {
    js_sys::Date::now() as u64
}
fn nonce() -> Result<String, JsValue> {
    let mut bytes = [0u8; 24];
    window()
        .crypto()?
        .get_random_values_with_u8_array(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn send(port: &MessagePort, e: Envelope) -> Result<(), JsValue> {
    port.post_message(&JsValue::from_str(&e.encode().map_err(js)?))
}
fn wire(event: &MessageEvent) -> Result<String, Error> {
    let value = event.data();
    if !value.is_string() || js_sys::JsString::from(value.clone()).length() as usize > MAX_BYTES {
        return Err(Error::Invalid("transport size"));
    }
    value.as_string().ok_or(Error::Invalid("transport"))
}
fn notify(callback: &Option<Function>) {
    if let Some(callback) = callback {
        let _ = callback.call0(&JsValue::NULL);
    }
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    release: String,
    engine_abi: String,
    project_format: u16,
    project: Project,
}
struct HostState {
    frame: HtmlIFrameElement,
    url: String,
    config: Config,
    host: Option<Host>,
    port: Option<MessagePort>,
    listener: Option<Handler<MessageEvent>>,
    load: Option<Handler<Event>>,
    tick: Option<Closure<dyn FnMut()>>,
    window_listener: Option<Handler<MessageEvent>>,
    timer: Option<i32>,
    callback: Option<Function>,
    error: Option<String>,
    started: u64,
    closed: bool,
    checkpoint: Option<Checkpoint>,
}
impl HostState {
    fn disconnect(&mut self) {
        if let Some(host) = self.host.as_mut() {
            if let Some(checkpoint) = host.status.as_ref().and_then(|s| s.checkpoint.clone()) {
                self.checkpoint = Some(checkpoint)
            }
            host.shutdown();
        }
        if let Some(p) = self.port.take() {
            p.set_onmessage(None);
            p.close();
        }
        self.listener = None;
    }
    fn request(&mut self, action: Action) -> Result<String, JsValue> {
        let e = self
            .host
            .as_mut()
            .ok_or_else(|| js("The world is still connecting."))?
            .request(action, now())
            .map_err(js)?;
        let Message::Request { request_id, .. } = &e.message else {
            return Err(js("request"));
        };
        let id = request_id.clone();
        send(
            self.port
                .as_ref()
                .ok_or_else(|| js("The world disconnected."))?,
            e,
        )?;
        Ok(id)
    }
    fn snapshot(&self) -> String {
        #[derive(Serialize)]
        struct Snapshot<'a> {
            phase: Option<Phase>,
            session: Option<&'a str>,
            status: Option<&'a Status>,
            capabilities: Vec<Capability>,
            pending: usize,
            receipts: Vec<&'a Receipt>,
            error: &'a Option<String>,
        }
        let h = self.host.as_ref();
        serde_json::to_string(&Snapshot {
            phase: h.map(|h| h.phase),
            session: h.map(|h| h.session.as_str()),
            status: h.and_then(|h| h.status.as_ref()),
            capabilities: h.map(|h| h.capabilities.clone()).unwrap_or_default(),
            pending: h.map_or(0, Host::pending),
            receipts: h.map(|h| h.receipts().iter().collect()).unwrap_or_default(),
            error: &self.error,
        })
        .unwrap_or_default()
    }
    fn dispose(&mut self) {
        self.closed = true;
        self.disconnect();
        if let Some(l) = self.window_listener.take() {
            let _ =
                window().remove_event_listener_with_callback("message", l.as_ref().unchecked_ref());
        }
        if let Some(t) = self.timer.take() {
            window().clear_interval_with_handle(t)
        }
        if let Some(load) = self.load.as_ref() {
            let _ = self
                .frame
                .remove_event_listener_with_callback("load", load.as_ref().unchecked_ref());
        }
        self.callback = None;
        self.frame.remove();
    }
}
fn loaded(weak: &Weak<RefCell<HostState>>, connect: bool) -> Result<(), JsValue> {
    let Some(rc) = weak.upgrade() else {
        return Ok(());
    };
    let mut s = rc.borrow_mut();
    if s.closed {
        return Ok(());
    }
    if let Some(c) = s
        .host
        .as_ref()
        .and_then(|h| h.status.as_ref())
        .and_then(|s| s.checkpoint.clone())
    {
        s.checkpoint = Some(c)
    }
    // A replacement document starts at the pinned release base. Edited project
    // revisions belong to the previous frame and require an explicit restore.
    s.disconnect();
    s.error = None;
    s.started = now();
    let mut h = Host::new(
        &s.url,
        nonce()?,
        s.config.release.clone(),
        s.config.engine_abi.clone(),
        s.config.project_format,
        s.config.project.clone(),
    )
    .map_err(js)?;
    if !connect {
        s.host = Some(h);
        let cb = s.callback.clone();
        drop(s);
        notify(&cb);
        return Ok(());
    }
    let hello = h.hello().map_err(js)?.encode().map_err(js)?;
    let channel = MessageChannel::new()?;
    let port = channel.port1();
    let weak = Rc::downgrade(&rc);
    let bound_session = h.session.clone();
    let handler = Closure::wrap(Box::new(move |event: MessageEvent| {
        let Some(rc) = weak.upgrade() else { return };
        let mut s = rc.borrow_mut();
        if !s.host.as_ref().is_some_and(|h| h.session == bound_session) {
            return;
        }
        let result =
            wire(&event).and_then(|text| s.host.as_mut().ok_or(Error::Phase)?.receive(&text));
        match result {
            Ok(Message::Ready { .. }) => {
                s.started = now();
                let checkpoint = s.checkpoint.clone();
                if s.request(Action::Open { checkpoint }).is_err() {
                    s.error = Some("The world could not open.".into());
                }
            }
            Ok(Message::Revoked { .. }) => {
                s.error = Some("Access to this world ended.".into());
                s.disconnect();
                s.checkpoint = None;
            }
            Ok(Message::Reply { outcome, .. })
                if outcome.state != ResultState::Applied
                    && s.host.as_ref().is_some_and(|h| h.phase == Phase::Ready) =>
            {
                s.error = Some(outcome.message);
                s.disconnect();
            }
            Ok(_) => (),
            Err(_) => {
                s.error = Some("The world sent an invalid response.".into());
                s.disconnect();
            }
        }
        let cb = s.callback.clone();
        drop(s);
        notify(&cb);
    }) as Box<dyn FnMut(MessageEvent)>);
    port.set_onmessage(Some(handler.as_ref().unchecked_ref()));
    port.start();
    let transfer = Array::new();
    transfer.push(&channel.port2());
    let child = s
        .frame
        .content_window()
        .ok_or_else(|| js("The world frame is missing."))?;
    child.post_message_with_transfer(&JsValue::from_str(&hello), &h.origin, &transfer)?;
    s.host = Some(h);
    s.port = Some(port);
    s.listener = Some(handler);
    let cb = s.callback.clone();
    drop(s);
    notify(&cb);
    Ok(())
}

/// Own one cross-origin frame. Overlay changes do not recreate this value.
#[wasm_bindgen]
pub struct BrowserHost {
    state: Rc<RefCell<HostState>>,
}
#[wasm_bindgen]
impl BrowserHost {
    #[wasm_bindgen(constructor)]
    pub fn new(
        frame: HtmlIFrameElement,
        url: String,
        config: String,
        callback: Function,
    ) -> Result<BrowserHost, JsValue> {
        let u = endpoint(&url).map_err(js)?;
        if u.origin().ascii_serialization() == window().location().origin()? {
            return Err(js("The engine needs a separate origin."));
        }
        let c: Config = serde_json::from_str(&config).map_err(js)?;
        Host::new(
            &url,
            "validation".into(),
            c.release.clone(),
            c.engine_abi.clone(),
            c.project_format,
            c.project.clone(),
        )
        .map_err(js)?;
        frame.set_attribute(
            "sandbox",
            "allow-scripts allow-same-origin allow-pointer-lock",
        )?;
        frame.set_attribute(
            "allow",
            "fullscreen; camera 'none'; microphone 'none'; clipboard-read 'none'; clipboard-write 'none'",
        )?;
        frame.set_attribute("referrerpolicy", "no-referrer")?;
        let rc = Rc::new(RefCell::new(HostState {
            frame: frame.clone(),
            url: url.clone(),
            config: c,
            host: None,
            port: None,
            listener: None,
            load: None,
            tick: None,
            window_listener: None,
            timer: None,
            callback: Some(callback),
            error: None,
            started: now(),
            closed: false,
            checkpoint: None,
        }));
        let weak = Rc::downgrade(&rc);
        let onload = Closure::wrap(Box::new(move |_: Event| {
            if let Err(_) = loaded(&weak, false) {
                if let Some(rc) = weak.upgrade() {
                    let mut s = rc.borrow_mut();
                    s.error = Some("The world could not connect.".into());
                    let cb = s.callback.clone();
                    drop(s);
                    notify(&cb);
                }
            }
        }) as Box<dyn FnMut(Event)>);
        frame.add_event_listener_with_callback("load", onload.as_ref().unchecked_ref())?;
        rc.borrow_mut().load = Some(onload);
        let weak = Rc::downgrade(&rc);
        let boot = Closure::wrap(Box::new(move |event: MessageEvent| {
            let Some(rc) = weak.upgrade() else { return };
            let s = rc.borrow();
            let peer = event.source().is_some_and(|source| {
                s.frame
                    .content_window()
                    .is_some_and(|w| JsValue::from(source) == JsValue::from(w))
            });
            let valid = !s.closed
                && s.port.is_none()
                && s.host.as_ref().is_some_and(|h| {
                    h.phase == Phase::Connecting && h.check_peer(&event.origin(), peer).is_ok()
                })
                && event.data().as_string().as_deref() == Some(BOOTSTRAP);
            drop(s);
            if valid {
                let _ = loaded(&weak, true);
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        window().add_event_listener_with_callback("message", boot.as_ref().unchecked_ref())?;
        rc.borrow_mut().window_listener = Some(boot);
        let weak = Rc::downgrade(&rc);
        let tick = Closure::wrap(Box::new(move || {
            if let Some(rc) = weak.upgrade() {
                let mut s = rc.borrow_mut();
                if s.closed {
                    return;
                }
                let started = s.started;
                if let Some(h) = s.host.as_mut() {
                    h.expire(now());
                    if matches!(h.phase, Phase::Connecting | Phase::Opening)
                        && now().saturating_sub(started)
                            > if h.phase == Phase::Opening {
                                OPEN_DEADLINE_MS
                            } else {
                                DEADLINE_MS
                            }
                    {
                        s.error = Some("The world took too long to open. Try again.".into());
                        s.disconnect();
                    }
                }
                let cb = s.callback.clone();
                drop(s);
                notify(&cb);
            }
        }) as Box<dyn FnMut()>);
        let timer = window().set_interval_with_callback_and_timeout_and_arguments_0(
            tick.as_ref().unchecked_ref(),
            250,
        )?;
        rc.borrow_mut().timer = Some(timer);
        rc.borrow_mut().tick = Some(tick);
        frame.set_src(&url);
        Ok(Self { state: rc })
    }
    pub fn request(&self, action: String) -> Result<String, JsValue> {
        if action.len() > MAX_BYTES {
            return Err(js("The request is too large."));
        }
        let a: Action = serde_json::from_str(&action).map_err(js)?;
        self.state.borrow_mut().request(a)
    }
    pub fn snapshot(&self) -> String {
        self.state.borrow().snapshot()
    }
    pub fn restore_checkpoint(&self, checkpoint: String) -> Result<(), JsValue> {
        let c: Checkpoint = serde_json::from_str(&checkpoint).map_err(js)?;
        c.validate().map_err(js)?;
        let mut s = self.state.borrow_mut();
        if s.host
            .as_ref()
            .is_some_and(|h| !matches!(h.phase, Phase::Connecting | Phase::Ready))
        {
            return Err(js("The world is already open."));
        }
        s.checkpoint = Some(c);
        Ok(())
    }
    pub fn reload(&self) {
        let s = self.state.borrow();
        if !s.closed {
            s.frame.set_src(&s.url);
        }
    }
    pub fn close(&self) {
        let mut s = self.state.borrow_mut();
        let _ = s.request(Action::Close);
        s.dispose();
    }
}
impl Drop for BrowserHost {
    fn drop(&mut self) {
        self.state.borrow_mut().dispose();
    }
}

struct EndpointState {
    engine: Option<Engine>,
    port: Option<MessagePort>,
    listener: Option<Handler<MessageEvent>>,
    window_listener: Option<Handler<MessageEvent>>,
    callback: Option<Function>,
    last_status: u64,
    load_listener: Option<Handler<Event>>,
}
impl EndpointState {
    fn dispose(&mut self) {
        if let Some(p) = self.port.take() {
            p.set_onmessage(None);
            p.close();
        }
        self.engine = None;
        self.callback = None;
        if let Some(l) = self.load_listener.take() {
            let _ =
                window().remove_event_listener_with_callback("load", l.as_ref().unchecked_ref());
        }
        if let Some(l) = self.window_listener.take() {
            let _ =
                window().remove_event_listener_with_callback("message", l.as_ref().unchecked_ref());
        }
        self.listener = None;
    }
}
/// The approved engine imports this public SDK; its implementation stays private.
#[wasm_bindgen]
pub struct EngineEndpoint {
    state: Rc<RefCell<EndpointState>>,
}
#[wasm_bindgen]
impl EngineEndpoint {
    #[wasm_bindgen(constructor)]
    pub fn new(
        host_origin: String,
        release: String,
        abi: String,
        format: u16,
        capabilities: String,
        callback: Function,
    ) -> Result<EngineEndpoint, JsValue> {
        let host = endpoint(&host_origin).map_err(js)?;
        if host.origin().ascii_serialization() != host_origin
            || host_origin == window().location().origin()?
        {
            return Err(js("host origin"));
        }
        let caps: Vec<Capability> = serde_json::from_str(&capabilities).map_err(js)?;
        let rc = Rc::new(RefCell::new(EndpointState {
            engine: None,
            port: None,
            listener: None,
            window_listener: None,
            callback: Some(callback),
            last_status: 0,
            load_listener: None,
        }));
        let announce_origin = host_origin.clone();
        let weak = Rc::downgrade(&rc);
        let listener = Closure::wrap(Box::new(move |event: MessageEvent| {
            let Some(rc) = weak.upgrade() else { return };
            let parent = window().parent().ok().flatten();
            if event.origin() != host_origin
                || !event.source().is_some_and(|source| {
                    parent
                        .as_ref()
                        .is_some_and(|p| JsValue::from(source) == JsValue::from(p))
                })
                || event.ports().length() != 1
            {
                return;
            }
            let Some(text) = event.data().as_string() else {
                return;
            };
            let mut s = rc.borrow_mut();
            if s.engine.is_some() {
                return;
            }
            let Ok((engine, ready)) =
                Engine::accept(&text, release.clone(), abi.clone(), format, caps.clone())
            else {
                return;
            };
            let Ok(port) = event.ports().get(0).dyn_into::<MessagePort>() else {
                return;
            };
            let weak = Rc::downgrade(&rc);
            let onmessage = Closure::wrap(Box::new(move |event: MessageEvent| {
                let Some(rc) = weak.upgrade() else { return };
                let mut s = rc.borrow_mut();
                let result = wire(&event)
                    .and_then(|text| s.engine.as_mut().ok_or(Error::Phase)?.receive(&text));
                let Ok(message) = result else {
                    s.dispose();
                    return;
                };
                let callback = s.callback.clone();
                drop(s);
                if let (Some(callback), Ok(json)) = (callback, serde_json::to_string(&message)) {
                    let _ = callback.call1(&JsValue::NULL, &JsValue::from_str(&json));
                }
            }) as Box<dyn FnMut(MessageEvent)>);
            port.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
            port.start();
            if send(&port, ready).is_err() {
                port.close();
                return;
            }
            s.engine = Some(engine);
            s.port = Some(port);
            s.listener = Some(onmessage);
        }) as Box<dyn FnMut(MessageEvent)>);
        window().add_event_listener_with_callback("message", listener.as_ref().unchecked_ref())?;
        rc.borrow_mut().window_listener = Some(listener);
        let announce = move || {
            if let Ok(Some(parent)) = window().parent() {
                let _ = parent.post_message(&JsValue::from_str(BOOTSTRAP), &announce_origin);
            }
        };
        if window()
            .document()
            .is_some_and(|d| d.ready_state() == "complete")
        {
            announce();
        } else {
            let load = Closure::wrap(Box::new(move |_: Event| announce()) as Box<dyn FnMut(Event)>);
            window().add_event_listener_with_callback("load", load.as_ref().unchecked_ref())?;
            rc.borrow_mut().load_listener = Some(load);
        }
        Ok(Self { state: rc })
    }
    pub fn reply(&self, request_id: String, outcome: String) -> Result<(), JsValue> {
        if outcome.len() > MAX_BYTES {
            return Err(js("bytes"));
        }
        let o: Outcome = serde_json::from_str(&outcome).map_err(js)?;
        let mut s = self.state.borrow_mut();
        let e = s
            .engine
            .as_mut()
            .ok_or_else(|| js("session"))?
            .reply(&request_id, o)
            .map_err(js)?;
        send(s.port.as_ref().ok_or_else(|| js("port"))?, e)
    }
    pub fn status(&self, status: String) -> Result<bool, JsValue> {
        if status.len() > MAX_BYTES {
            return Err(js("bytes"));
        }
        let status: Status = serde_json::from_str(&status).map_err(js)?;
        let mut s = self.state.borrow_mut();
        if now().saturating_sub(s.last_status) < 100 {
            return Ok(false);
        }
        let e = s
            .engine
            .as_mut()
            .ok_or_else(|| js("session"))?
            .status(status)
            .map_err(js)?;
        send(s.port.as_ref().ok_or_else(|| js("port"))?, e)?;
        s.last_status = now();
        Ok(true)
    }
    pub fn lookup(&self, id: String) -> Result<String, JsValue> {
        let s = self.state.borrow();
        serde_json::to_string(&s.engine.as_ref().and_then(|e| e.lookup(&id))).map_err(js)
    }
    pub fn auth_required(&self, request_id: String, message: String) -> Result<(), JsValue> {
        let mut s = self.state.borrow_mut();
        let e = s
            .engine
            .as_mut()
            .ok_or_else(|| js("session"))?
            .auth_required(&request_id, message)
            .map_err(js)?;
        send(s.port.as_ref().ok_or_else(|| js("port"))?, e)
    }
    pub fn revoke(&self, message: String) -> Result<(), JsValue> {
        let mut s = self.state.borrow_mut();
        let e = s
            .engine
            .as_mut()
            .ok_or_else(|| js("session"))?
            .revoke(message)
            .map_err(js)?;
        send(s.port.as_ref().ok_or_else(|| js("port"))?, e)?;
        s.dispose();
        Ok(())
    }
    pub fn close(&self) {
        self.state.borrow_mut().dispose()
    }
}
impl Drop for EngineEndpoint {
    fn drop(&mut self) {
        self.state.borrow_mut().dispose();
    }
}
