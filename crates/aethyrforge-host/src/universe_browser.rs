//! Rust entry controls and destination state over the private universe profile.
use crate::*;
use js_sys::Promise;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Event, EventTarget, HtmlDialogElement, HtmlElement};
type Handler = Closure<dyn FnMut(Event)>;
const SAVED: &str = "openagents.verse-map.v1";
fn window() -> web_sys::Window {
    web_sys::window().expect("browser")
}
fn element(id: &str) -> web_sys::Element {
    window()
        .document()
        .expect("document")
        .get_element_by_id(id)
        .expect("map element")
}
fn hidden(id: &str, yes: bool) {
    if yes {
        let _ = element(id).set_attribute("hidden", "");
    } else {
        let _ = element(id).remove_attribute("hidden");
    }
}
struct State {
    demo: BrowserDemo,
    visit: UniverseVisit,
    desired: Option<String>,
    restore_sent: bool,
    popup: Option<web_sys::Window>,
    account: Option<String>,
    csrf: Option<String>,
    tutorial: Value,
    tutorial_available: bool,
    tutorial_busy: bool,
    tutorial_serial: u64,
    tutorial_error: Option<String>,
}
impl State {
    fn identity(&mut self, session: &Value) {
        let account = session["account"]
            .as_str()
            .filter(|_| session["signed_in"] == true)
            .map(str::to_string);
        if self.account != account {
            self.tutorial_serial = self.tutorial_serial.wrapping_add(1);
            self.tutorial_busy = false;
            self.tutorial = Value::Null;
            self.tutorial_available = false;
            self.tutorial_error = if account.is_none() {
                Some("Log in again to open your planet.".into())
            } else {
                None
            };
        }
        self.account = account;
        self.csrf = session["csrf"].as_str().map(str::to_string);
        if self.account.is_some() && self.visit.choice.is_none() {
            self.visit.enter(EntryChoice::Member);
            self.save();
        }
    }
    fn save(&self) {
        if let Ok(Some(storage)) = window().session_storage() {
            let _ = storage.set_item(SAVED, &serde_json::to_string(&self.visit).unwrap());
        }
    }
    fn render(&self) {
        let progress = &self.tutorial["progress"];
        let owned = progress["planet"].is_object();
        let ready = !owned && progress["returned_unix"].is_u64();
        hidden("planet-ready", !ready);
        hidden("planet-owned", !owned);
        let button = element("planet-create");
        if self.tutorial_busy || !self.tutorial_available {
            button.set_attribute("disabled", "").ok();
        } else {
            button.remove_attribute("disabled").ok();
        }
        let setup = element("planet-setup");
        setup.remove_attribute("href").ok();
        if owned && self.tutorial_available && self.tutorial["planet_available"] == true {
            if let Some(id) = progress["planet"]["id"].as_str() {
                setup
                    .set_attribute("href", &format!("/new-lander/planet/{id}"))
                    .ok();
            }
        }
        element("planet-status").set_text_content(Some(self.tutorial_error.as_deref().unwrap_or(if self.tutorial_busy {"Creating your planet…"}else if owned && self.tutorial["planet_available"]==false && self.tutorial_available {"This planet's project was removed. It cannot be recreated by repeating the tutorial."}else if owned {"Welcome back. Your planet is waiting."}else if ready {"Your first planet is ready to create."}else {"Publish your own house, choose Play or Share, then return here."})));

        let entered = self.visit.choice.is_some();
        hidden("verse-account", entered);
        hidden("verse-choices", !entered);
        let exploration = element("universe-exploration");
        if self.visit.exploring {
            let _ = exploration.remove_attribute("inert");
        } else {
            let _ = exploration.set_attribute("inert", "");
        }
        let card = element("verse-entry")
            .dyn_into::<HtmlDialogElement>()
            .unwrap();
        let _ = card.set_attribute(
            "aria-labelledby",
            if entered {
                "choice-title"
            } else {
                "entry-title"
            },
        );
        let _ = card.set_attribute(
            "aria-describedby",
            if entered {
                "choice-description"
            } else {
                "entry-description"
            },
        );
        if self.visit.exploring {
            card.close();
        } else if !card.open() {
            let _ = card.show_modal();
        }
        let _ = element("verse-atlas")
            .set_attribute("data-entry", if entered { "guest" } else { "locked" });
        if let Some(world) = WORLDS.iter().find(|w| w.id == self.visit.selected) {
            let personal = world.id == "creator-worlds" && owned;
            element("world-title").set_text_content(Some(if personal {
                progress["planet"]["name"].as_str().unwrap_or("My planet")
            } else {
                world.name
            }));
            element("world-category").set_text_content(Some(if personal {
                "Your personal planet"
            } else {
                world.category
            }));
            element("world-description").set_text_content(Some(if personal {
                "Your private place in the Verse. Choose a name and make it your own."
            } else {
                world.description
            }));
            for world in WORLDS {
                hidden(
                    &format!("destination-{}", world.id),
                    world.id != self.visit.selected,
                );
                let _ = element(&format!("option-{}", world.id)).set_attribute(
                    "aria-pressed",
                    if world.id == self.visit.selected {
                        "true"
                    } else {
                        "false"
                    },
                );
            }
        }
    }
    fn select(&mut self, id: &str) {
        if self.visit.select(id).is_err() {
            return;
        }
        self.desired = Some(id.into());
        if let Some(host) = self.demo.host.borrow().as_ref() {
            let _ =
                host.request(serde_json::to_string(&Action::Select { target: id.into() }).unwrap());
        }
        self.save();
        self.render();
    }
}
#[wasm_bindgen]
pub struct BrowserUniverse {
    state: Rc<RefCell<State>>,
    listeners: Vec<(EventTarget, &'static str, Handler)>,
    timer: i32,
    tick: Closure<dyn FnMut()>,
}
#[wasm_bindgen]
impl BrowserUniverse {
    pub fn snapshot(&self) -> String {
        self.state.borrow().demo.snapshot()
    }
    pub fn visit(&self) -> String {
        serde_json::to_string(&self.state.borrow().visit).unwrap()
    }
    pub fn close(&self) {
        self.state.borrow().demo.close();
    }
}
impl Drop for BrowserUniverse {
    fn drop(&mut self) {
        window().clear_interval_with_handle(self.timer);
        for (target, event, handler) in &self.listeners {
            let _ =
                target.remove_event_listener_with_callback(event, handler.as_ref().unchecked_ref());
        }
        self.close();
        let _ = &self.tick;
    }
}
fn listen(
    listeners: &mut Vec<(EventTarget, &'static str, Handler)>,
    id: &str,
    event: &'static str,
    mut action: impl FnMut(Event) + 'static,
) -> Result<(), JsValue> {
    let target: EventTarget = element(id).into();
    let handler = Closure::wrap(Box::new(move |event| action(event)) as Box<dyn FnMut(Event)>);
    target.add_event_listener_with_callback(event, handler.as_ref().unchecked_ref())?;
    listeners.push((target, event, handler));
    Ok(())
}
async fn pause() {
    let p = Promise::new(&mut |resolve, _| {
        let _ = window().set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 2000);
    });
    let _ = JsFuture::from(p).await;
}
#[wasm_bindgen]
pub fn mount_universe() -> Result<BrowserUniverse, JsValue> {
    let mut visit = window()
        .session_storage()?
        .and_then(|s| s.get_item(SAVED).ok().flatten())
        .and_then(|s| serde_json::from_str::<UniverseVisit>(&s).ok())
        .filter(|v| v.validate().is_ok())
        .unwrap_or_default();
    if element("verse-atlas")
        .get_attribute("data-entry")
        .as_deref()
        == Some("member")
    {
        visit.enter(EntryChoice::Member);
    }
    let demo = mount_demo()?;
    if let Some(c) = &visit.checkpoint {
        if let Some(host) = demo.host.borrow().as_ref() {
            host.restore_checkpoint(serde_json::to_string(c).unwrap())?;
        }
    }
    let rc = Rc::new(RefCell::new(State {
        demo,
        visit,
        desired: None,
        restore_sent: false,
        popup: None,
        account: None,
        csrf: None,
        tutorial: Value::Null,
        tutorial_available: false,
        tutorial_busy: false,
        tutorial_serial: 0,
        tutorial_error: None,
    }));
    rc.borrow().render();
    let mut listeners = Vec::new();
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "continue-guest", "click", move |_| {
        if let Some(rc) = weak.upgrade() {
            let mut s = rc.borrow_mut();
            s.visit.enter(EntryChoice::Guest);
            s.save();
            s.render();
            let _ = element("verse-explore")
                .dyn_into::<HtmlElement>()
                .unwrap()
                .focus();
        }
    })?;
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "verse-explore", "click", move |_| {
        if let Some(rc) = weak.upgrade() {
            let mut s = rc.borrow_mut();
            if s.visit.explore().is_ok() {
                s.save();
                s.render();
                let _ = element("option-openagents")
                    .dyn_into::<HtmlElement>()
                    .unwrap()
                    .focus();
            }
        }
    })?;
    listen(&mut listeners, "verse-entry", "cancel", |event| {
        event.prevent_default()
    })?;
    for world in WORLDS {
        let weak = Rc::downgrade(&rc);
        listen(
            &mut listeners,
            &format!("option-{}", world.id),
            "click",
            move |_| {
                if let Some(rc) = weak.upgrade() {
                    rc.borrow_mut().select(world.id);
                }
            },
        )?;
    }
    let weak = Rc::downgrade(&rc);
    listen(
        &mut listeners,
        "verse-signin",
        "click",
        move |event| match window().open_with_url_and_target_and_features(
            "/auth/popup",
            "oa-verse-sign-in",
            "popup,width=560,height=760",
        ) {
            Ok(Some(popup)) => {
                event.prevent_default();
                if let Some(rc) = weak.upgrade() {
                    rc.borrow_mut().popup = Some(popup);
                }
            }
            _ => (),
        },
    )?;
    let weak = Rc::downgrade(&rc);
    let tick = Closure::wrap(Box::new(move || {
        let Some(rc) = weak.upgrade() else { return };
        let mut s = rc.borrow_mut();
        let value: Value = serde_json::from_str(&s.demo.snapshot()).unwrap_or(Value::Null);
        if value["phase"] == "open" {
            if !s.restore_sent {
                s.restore_sent = true;
                if s.visit.exploring {
                    let id = s.visit.selected.clone();
                    s.select(&id);
                }
            }
            if let Ok(c) =
                serde_json::from_value::<Checkpoint>(value["status"]["checkpoint"].clone())
            {
                if c.validate().is_ok() {
                    s.visit.checkpoint = Some(c);
                }
            }
            if let Some(id) = value["status"]["selection"]["id"].as_str() {
                if s.desired.as_deref() == Some(id) {
                    s.desired = None;
                }
                if s.desired.is_none() && s.visit.exploring && s.visit.selected != id {
                    if s.visit.select(id).is_ok() {
                        s.render();
                    }
                }
            }
            s.save();
        }
    }) as Box<dyn FnMut()>);
    let timer = window().set_interval_with_callback_and_timeout_and_arguments_0(
        tick.as_ref().unchecked_ref(),
        200,
    )?;
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "planet-create", "click", move |_| {
        let Some(rc) = weak.upgrade() else { return };
        let (account, csrf, serial) = {
            let mut s = rc.borrow_mut();
            if s.tutorial_busy || !s.tutorial_available {
                return;
            }
            let (Some(account), Some(csrf)) = (s.account.clone(), s.csrf.clone()) else {
                return;
            };
            s.tutorial_busy = true;
            s.tutorial_serial = s.tutorial_serial.wrapping_add(1);
            s.tutorial_error = None;
            s.render();
            (account, csrf, s.tutorial_serial)
        };
        let weak = weak.clone();
        spawn_local(async move {
            let result = tutorial_fetch(
                "/v1/verse/studio/tutorial",
                Some(json!({"csrf":csrf,"operation":"allocate"})),
            )
            .await;
            let session = tutorial_fetch("/v1/verse/studio/session", None).await;
            let Some(rc) = weak.upgrade() else { return };
            let mut s = rc.borrow_mut();
            if s.tutorial_serial != serial {
                return;
            }
            s.tutorial_busy = false;
            match session {
                Ok(session)
                    if session["signed_in"] == true
                        && session["account"].as_str() == Some(&account) => {}
                Ok(session) => {
                    s.identity(&session);
                    s.tutorial_available = false;
                    s.tutorial_error = Some("Log in again to open your planet.".into());
                    s.render();
                    return;
                }
                Err(e) => {
                    if let Ok(v) = result {
                        s.tutorial = v;
                    }
                    s.tutorial_available = false;
                    s.tutorial_error = Some(e);
                    s.render();
                    return;
                }
            }
            match result {
                Ok(v) => {
                    s.tutorial = v;
                    s.tutorial["planet_available"] = true.into();
                    s.tutorial_available = true;
                    s.tutorial_error = None;
                }
                Err(e) => {
                    s.tutorial_error = Some(e);
                }
            }
            s.render();
        });
    })?;
    let weak = Rc::downgrade(&rc);
    spawn_local(async move {
        loop {
            let Some(rc) = weak.upgrade() else { break };
            // This refresh owns the generation captured before its first await.
            let serial = rc.borrow().tutorial_serial;
            let session = tutorial_fetch("/v1/verse/studio/session", None).await;
            if rc.borrow().tutorial_serial == serial {
                match session {
                    Ok(session) => {
                        {
                            let mut s = rc.borrow_mut();
                            s.identity(&session);
                            s.render();
                        }
                        let (account, serial, busy) = {
                            let s = rc.borrow();
                            (s.account.clone(), s.tutorial_serial, s.tutorial_busy)
                        };
                        if let Some(account) = account.filter(|_| !busy) {
                            let result = async {
                                let mut v =
                                    tutorial_fetch("/v1/verse/studio/tutorial", None).await?;
                                if v["account"].as_str() != Some(&account) {
                                    return Err("Your account changed. Try again.".into());
                                }
                                {
                                    let s = rc.borrow();
                                    if s.tutorial_serial != serial || s.tutorial_busy {
                                        return Err("Your progress changed. Try again.".into());
                                    }
                                }
                                if v["progress"]["choice"].is_string()
                                    && v["progress"]["returned_unix"].is_null()
                                {
                                    let csrf = session["csrf"].as_str().unwrap_or_default();
                                    let updated = tutorial_fetch(
                                        "/v1/verse/studio/tutorial",
                                        Some(json!({"csrf":csrf,"operation":"return"})),
                                    )
                                    .await?;
                                    v["progress"] = updated["progress"].clone();
                                }
                                Ok::<_, String>(v)
                            }
                            .await;
                            let current = tutorial_fetch("/v1/verse/studio/session", None).await;
                            let mut s = rc.borrow_mut();
                            if s.tutorial_serial == serial
                                && !s.tutorial_busy
                                && s.account.as_ref() == Some(&account)
                            {
                                match current {
                                    Ok(current)
                                        if current["signed_in"] == true
                                            && current["account"].as_str() == Some(&account) =>
                                    {
                                        match result {
                                            Ok(v) => {
                                                let newly_ready =
                                                    s.tutorial["progress"]["returned_unix"]
                                                        .is_null()
                                                        && v["progress"]["returned_unix"].is_u64();
                                                s.tutorial = v;
                                                s.tutorial_available = true;
                                                s.tutorial_error = None;
                                                if newly_ready {
                                                    s.visit.enter(EntryChoice::Member);
                                                    let _ = s.visit.explore();
                                                    s.select("creator-worlds");
                                                }
                                            }
                                            Err(e) => {
                                                s.tutorial_available = false;
                                                s.tutorial_error = Some(e);
                                            }
                                        }
                                    }
                                    Ok(current) => {
                                        s.identity(&current);
                                        s.tutorial_available = false;
                                        s.tutorial_error =
                                            Some("Log in again to open your planet.".into());
                                    }
                                    Err(e) => {
                                        s.tutorial_available = false;
                                        s.tutorial_error = Some(e);
                                    }
                                }
                                s.render();
                            }
                        }
                    }
                    Err(e) => {
                        let mut s = rc.borrow_mut();
                        s.tutorial_available = false;
                        s.tutorial_error = Some(e);
                        s.render();
                    }
                }
            }
            drop(rc);
            pause().await;
        }
    });
    Ok(BrowserUniverse {
        state: rc,
        listeners,
        timer,
        tick,
    })
}

async fn tutorial_fetch(path: &str, body: Option<Value>) -> Result<Value, String> {
    let init = web_sys::RequestInit::new();
    init.set_credentials(web_sys::RequestCredentials::SameOrigin);
    init.set_method(if body.is_some() { "POST" } else { "GET" });
    if let Some(body) = body {
        init.set_body(&JsValue::from_str(&body.to_string()));
    }
    let request = web_sys::Request::new_with_str_and_init(path, &init)
        .map_err(|_| "Your progress could not open. Try again.".to_string())?;
    request
        .headers()
        .set("content-type", "application/json")
        .map_err(|_| "Try again.".to_string())?;
    let response = JsFuture::from(window().fetch_with_request(&request))
        .await
        .map_err(|_| "Your progress could not open. Try again.".to_string())?
        .dyn_into::<web_sys::Response>()
        .map_err(|_| "Try again.".to_string())?;
    let raw = JsFuture::from(response.text().map_err(|_| "Try again.".to_string())?)
        .await
        .map_err(|_| "Try again.".to_string())?
        .as_string()
        .unwrap_or_default();
    let v: Value = serde_json::from_str(&raw)
        .map_err(|_| "Your progress could not open. Try again.".to_string())?;
    if !response.ok() {
        return Err(v["error"]["message"]
            .as_str()
            .unwrap_or("Your progress could not open. Try again.")
            .into());
    }
    Ok(v)
}
