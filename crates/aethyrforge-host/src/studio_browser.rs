//! First-edit UI and sign-in continuation. Only this host origin calls account APIs.
use crate::*;
use js_sys::Promise;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    Event, EventTarget, HtmlInputElement, HtmlTextAreaElement, MessageEvent, Request,
    RequestCredentials, RequestInit, Response,
};
const SAVED: &str = "openagents.verse.pending.v1";
type Handler = Closure<dyn FnMut(Event)>;
fn window() -> web_sys::Window {
    web_sys::window().expect("window")
}
fn element(id: &str) -> web_sys::Element {
    window()
        .document()
        .expect("document")
        .get_element_by_id(id)
        .expect("studio element")
}
fn hide(id: &str, hidden: bool) {
    let e = element(id);
    if hidden {
        let _ = e.set_attribute("hidden", "");
    } else {
        let _ = e.remove_attribute("hidden");
    }
    if id == "edit-auth" {
        let dialog: web_sys::HtmlDialogElement = e.unchecked_into();
        if hidden {
            dialog.close();
        } else if !dialog.open() {
            let _ = dialog.show_modal();
        }
    }
}
fn say(text: &str) {
    element("edit-status").set_text_content(Some(text));
}
fn input(id: &str) -> HtmlInputElement {
    element(id).unchecked_into()
}
fn note() -> HtmlTextAreaElement {
    element("edit-note").unchecked_into()
}
fn random() -> String {
    let mut bytes = [0u8; 24];
    window()
        .crypto()
        .expect("crypto")
        .get_random_values_with_u8_array(&mut bytes)
        .expect("random");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Resume {
    owner: Option<String>,
    pending: PendingChange,
}
struct State {
    demo: BrowserDemo,
    tab: String,
    account: Option<String>,
    csrf: Option<String>,
    pending: Option<Resume>,
    busy: bool,
    popup: Option<web_sys::Window>,
    selection: Option<SelectedObject>,
    owned_world: Option<String>,
}
impl State {
    fn snapshot(&self) -> Value {
        serde_json::from_str(&self.demo.snapshot()).unwrap_or(Value::Null)
    }
    fn persist(&self) {
        if let Ok(Some(storage)) = window().session_storage() {
            if let Some(p) = &self.pending {
                if let Ok(text) = serde_json::to_string(p) {
                    let _ = storage.set_item(SAVED, &text);
                }
            } else {
                let _ = storage.remove_item(SAVED);
            }
        }
    }
    fn request(&self, action: &Action) -> Result<String, JsValue> {
        self.demo
            .host
            .borrow()
            .as_ref()
            .ok_or_else(|| JsValue::from_str("The house is still opening."))?
            .request(
                serde_json::to_string(action)
                    .map_err(|_| JsValue::from_str("The change could not be sent."))?,
            )
    }
    fn auth_card(&self) {
        hide("edit-auth", false);
        let signed = self.account.is_some();
        element("edit-auth-title").set_text_content(Some(if signed {
            "Apply this change?"
        } else {
            "Sign in to make this change"
        }));
        element("edit-auth-description").set_text_content(Some(if signed {
            "Your requested change is ready. Confirm to apply it."
        } else {
            "Your view and requested change will stay here."
        }));
        hide("edit-signin", signed);
        hide("edit-confirm", !signed);
    }
}
#[wasm_bindgen]
pub struct BrowserStudio {
    state: Rc<RefCell<State>>,
    listeners: Vec<(EventTarget, &'static str, Handler)>,
    message: Closure<dyn FnMut(MessageEvent)>,
    channel: Option<web_sys::BroadcastChannel>,
    broadcast: Closure<dyn FnMut(MessageEvent)>,
    timer: i32,
    tick: Closure<dyn FnMut()>,
}
#[wasm_bindgen]
impl BrowserStudio {
    pub fn snapshot(&self) -> String {
        self.state.borrow().demo.snapshot()
    }
    pub fn pending_change(&self) -> String {
        serde_json::to_string(&self.state.borrow().pending).unwrap_or_default()
    }
    pub fn reload(&self) {
        self.state.borrow().demo.reload()
    }
    pub fn close(&self) {
        self.state.borrow().demo.close()
    }
}
impl Drop for BrowserStudio {
    fn drop(&mut self) {
        window().clear_interval_with_handle(self.timer);
        for (target, event, handler) in &self.listeners {
            let _ =
                target.remove_event_listener_with_callback(event, handler.as_ref().unchecked_ref());
        }
        let _ = window()
            .remove_event_listener_with_callback("message", self.message.as_ref().unchecked_ref());
        if let Some(c) = &self.channel {
            c.set_onmessage(None);
            c.close();
        }
        let _ = (&self.tick, &self.broadcast);
    }
}
async fn api(path: &str, body: Option<Value>) -> Result<Value, JsValue> {
    let init = RequestInit::new();
    init.set_method(if body.is_some() { "POST" } else { "GET" });
    init.set_credentials(RequestCredentials::SameOrigin);
    if let Some(body) = body {
        init.set_body(&JsValue::from_str(&body.to_string()));
    }
    let request = Request::new_with_str_and_init(&format!("/v1/verse/studio/{path}"), &init)?;
    request.headers().set("content-type", "application/json")?;
    let response: Response = JsFuture::from(window().fetch_with_request(&request))
        .await?
        .dyn_into()?;
    let text = JsFuture::from(response.text()?)
        .await?
        .as_string()
        .unwrap_or_default();
    let value: Value = serde_json::from_str(&text)
        .map_err(|_| JsValue::from_str("The change could not be checked. Try again."))?;
    if !response.ok() {
        return Err(JsValue::from_str(
            value["error"]["message"]
                .as_str()
                .unwrap_or("The change could not be checked. Try again."),
        ));
    }
    Ok(value)
}
async fn pause(ms: i32) {
    let promise = Promise::new(&mut |resolve, _| {
        let _ = window().set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
    });
    let _ = JsFuture::from(promise).await;
}
async fn refresh(weak: Weak<RefCell<State>>) {
    let Ok(value) = api("session", None).await else {
        return;
    };
    let Some(rc) = weak.upgrade() else { return };
    let mut s = rc.borrow_mut();
    let account = value["account"].as_str().map(str::to_string);
    let policy = resume_policy(
        s.pending.as_ref().and_then(|p| p.owner.as_deref()),
        s.account.as_deref(),
        account.as_deref(),
    );
    if s.owned_world
        .as_ref()
        .is_some_and(|owner| Some(owner) != account.as_ref())
    {
        let _ = s.request(&Action::RevokeAuthorization);
        s.demo.reload();
        s.owned_world = None;
    }
    if policy == ResumePolicy::Discard {
        s.pending = None;
        s.persist();
        note().set_value("");
        hide("edit-auth", true);
        say("Account changed. Choose a new change to continue.");
    }
    let was_signed = s.account.is_some();
    s.account = account;
    s.csrf = value["csrf"].as_str().map(str::to_string);
    if policy == ResumePolicy::Reconfirm && s.pending.is_some() {
        let _ = s.request(&Action::RevokeAuthorization);
        s.auth_card();
        say(if s.account.is_some() {
            "You're signed in. Confirm your change to apply it."
        } else {
            "Sign in again. Your requested change is still here."
        });
    }
    if !was_signed && s.account.is_some() {
        let account = s.account.clone();
        if let Some(pending) = s.pending.as_mut() {
            if pending.owner.is_none() {
                pending.owner = account;
            }
        }
        s.persist();
    }
    if let Some(popup) = &s.popup {
        if popup.closed().unwrap_or(true) {
            s.popup = None;
            if s.account.is_none() {
                say("Sign-in wasn't completed. Your change is still here.");
            }
        }
    }
}
async fn receipt(weak: &Weak<RefCell<State>>, action: Action) -> Result<(), JsValue> {
    let id = weak
        .upgrade()
        .ok_or(JsValue::NULL)?
        .borrow()
        .request(&action)?;
    for _ in 0..300 {
        pause(50).await;
        let rc = weak.upgrade().ok_or(JsValue::NULL)?;
        let value = rc.borrow().snapshot();
        if let Some(r) = value["receipts"]
            .as_array()
            .and_then(|rows| rows.iter().find(|r| r["request_id"].as_str() == Some(&id)))
        {
            if r["outcome"]["state"] == "applied" {
                return Ok(());
            }
            return Err(JsValue::from_str(
                r["outcome"]["message"]
                    .as_str()
                    .unwrap_or("The change could not be applied."),
            ));
        }
    }
    Err(JsValue::from_str(
        "The change is taking longer than expected. Check the world before trying again.",
    ))
}
async fn confirm(weak: Weak<RefCell<State>>) {
    let Some(rc) = weak.upgrade() else { return };
    let sent = {
        let mut s = rc.borrow_mut();
        if s.busy {
            return;
        }
        let Some(p) = s.pending.clone() else { return };
        let Some(csrf) = s.csrf.clone() else {
            s.auth_card();
            return;
        };
        let value = s.snapshot();
        s.busy = true;
        json!({"csrf":csrf,"tab":s.tab,"session":value["session"],"release":element("engine-release").get_attribute("content"),"project":value["status"]["project"],"action":p.pending.action,"confirmed":true})
    };
    let result = async {
        let value = api("grants", Some(sent.clone())).await?;
        let grant: ScopedGrant = serde_json::from_value(value["grant"].clone())
            .map_err(|_| JsValue::from_str("Permission could not be checked."))?;
        let action: Action = serde_json::from_value(sent["action"].clone())
            .map_err(|_| JsValue::from_str("Choose a change again."))?;
        api(
            "authorize",
            Some(json!({"csrf":sent["csrf"],"tab":sent["tab"],"grant":grant,"action":action})),
        )
        .await?;
        receipt(&weak, Action::Authorize { grant }).await?;
        receipt(&weak, action).await
    }
    .await;
    if let Some(rc) = weak.upgrade() {
        let mut s = rc.borrow_mut();
        s.busy = false;
        match result {
            Ok(()) => {
                s.owned_world = s.account.clone();
                s.pending = None;
                s.persist();
                hide("edit-auth", true);
                say("We applied your change.");
            }
            Err(error) => {
                let _ = s.request(&Action::RevokeAuthorization);
                say(error
                    .as_string()
                    .as_deref()
                    .unwrap_or("The change could not be applied. Try again."));
            }
        }
    }
}
fn selected(rc: &Rc<RefCell<State>>) {
    let mut s = rc.borrow_mut();
    let selection =
        serde_json::from_value::<SelectedObject>(s.snapshot()["status"]["selection"].clone()).ok();
    if selection == s.selection {
        return;
    }
    s.selection = selection.clone();
    if s.pending.is_none() {
        if let Some(v) = selection {
            element("edit-target").set_text_content(Some(&v.id));
            for (id, v) in ["edit-x", "edit-y", "edit-z"].iter().zip(v.position) {
                input(id).set_value(&v.to_string());
            }
        } else {
            element("edit-target").set_text_content(Some("Click an object in the house."));
        }
    }
    input("edit-submit").set_disabled(s.selection.is_none() || s.busy);
}
fn listen(
    listeners: &mut Vec<(EventTarget, &'static str, Handler)>,
    id: &str,
    event: &'static str,
    mut action: impl FnMut(Event) + 'static,
) -> Result<(), JsValue> {
    let target: EventTarget = element(id).into();
    let handler = Closure::wrap(Box::new(move |e| action(e)) as Box<dyn FnMut(Event)>);
    target.add_event_listener_with_callback(event, handler.as_ref().unchecked_ref())?;
    listeners.push((target, event, handler));
    Ok(())
}
#[wasm_bindgen]
pub fn mount_studio() -> Result<BrowserStudio, JsValue> {
    let pending = window()
        .session_storage()?
        .and_then(|s| s.get_item(SAVED).ok().flatten())
        .and_then(|s| serde_json::from_str::<Resume>(&s).ok())
        .filter(|r| r.pending.validate().is_ok());
    let demo = mount_demo()?;
    if let Some(p) = &pending {
        if let Some(h) = demo.host.borrow().as_ref() {
            h.restore_checkpoint(serde_json::to_string(&p.pending.checkpoint).unwrap())?;
        }
        note().set_value(&p.pending.draft);
        hide("edit-panel", false);
        if let Action::Propose { operations } = &p.pending.action {
            if let Some(Edit::Move { target, position }) = operations.first() {
                element("edit-target").set_text_content(Some(target));
                for (id, v) in ["edit-x", "edit-y", "edit-z"].iter().zip(position) {
                    input(id).set_value(&v.to_string());
                }
            }
        }
    }
    let rc = Rc::new(RefCell::new(State {
        demo,
        tab: random(),
        account: None,
        csrf: None,
        pending,
        busy: false,
        popup: None,
        selection: None,
        owned_world: None,
    }));
    let mut listeners = Vec::new();
    listen(&mut listeners, "world-edit", "click", |_| {
        hide("edit-panel", false)
    })?;
    listen(&mut listeners, "edit-close", "click", |_| {
        hide("edit-panel", true)
    })?;
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "edit-form", "submit", move |event| {
        event.prevent_default();
        let Some(rc) = weak.upgrade() else { return };
        let mut s = rc.borrow_mut();
        if s.busy {
            return;
        };
        let snapshot = s.snapshot();
        let Some(selection) = s.selection.clone() else {
            say("Click an object in the house first.");
            return;
        };
        let position = [
            input("edit-x").value_as_number(),
            input("edit-y").value_as_number(),
            input("edit-z").value_as_number(),
        ];
        let Ok(checkpoint) = serde_json::from_value(snapshot["status"]["checkpoint"].clone())
        else {
            return;
        };
        let pending = PendingChange {
            world: "haunted-house".into(),
            checkpoint,
            selection: Some(selection.id.clone()),
            panels: vec!["edit".into()],
            draft: note().value(),
            action: Action::Propose {
                operations: vec![Edit::Move {
                    target: selection.id,
                    position,
                }],
            },
        };
        if pending.validate().is_err() {
            say("Enter a valid position.");
            return;
        };
        s.pending = Some(Resume {
            owner: s.account.clone(),
            pending,
        });
        s.persist();
        s.auth_card();
        drop(s);
        spawn_local(refresh(weak.clone()));
    })?;
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "edit-cancel", "click", move |_| {
        hide("edit-auth", true);
        if let Some(rc) = weak.upgrade() {
            rc.borrow().persist();
        }
        say("Your requested change is still here.");
    })?;
    listen(&mut listeners, "edit-auth", "cancel", |event| {
        event.prevent_default();
        hide("edit-auth", true);
        say("Your requested change is still here.");
    })?;
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "edit-signin", "click", move |event| {
        let Some(rc) = weak.upgrade() else { return };
        rc.borrow().persist();
        match window().open_with_url_and_target_and_features(
            "/auth/popup",
            "oa-verse-sign-in",
            "popup,width=560,height=760",
        ) {
            Ok(Some(popup)) => {
                event.prevent_default();
                rc.borrow_mut().popup = Some(popup);
            }
            _ => (),
        }
    })?;
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "edit-confirm", "click", move |_| {
        spawn_local(confirm(weak.clone()))
    })?;
    let weak = Rc::downgrade(&rc);
    let message = Closure::wrap(Box::new(move |event: MessageEvent| {
        if event.origin() == window().location().origin().unwrap_or_default()
            && js_sys::Reflect::get(&event.data(), &JsValue::from_str("type"))
                .ok()
                .and_then(|v| v.as_string())
                .as_deref()
                == Some("openagents-signed-in")
        {
            spawn_local(refresh(weak.clone()));
        }
    }) as Box<dyn FnMut(MessageEvent)>);
    window().add_event_listener_with_callback("message", message.as_ref().unchecked_ref())?;
    let weak = Rc::downgrade(&rc);
    let broadcast =
        Closure::wrap(
            Box::new(move |_: MessageEvent| spawn_local(refresh(weak.clone())))
                as Box<dyn FnMut(MessageEvent)>,
        );
    let channel = web_sys::BroadcastChannel::new("openagents-sign-in").ok();
    if let Some(c) = &channel {
        c.set_onmessage(Some(broadcast.as_ref().unchecked_ref()));
    }
    let weak = Rc::downgrade(&rc);
    let mut polls = 0;
    let tick = Closure::wrap(Box::new(move || {
        let Some(rc) = weak.upgrade() else { return };
        selected(&rc);
        polls += 1;
        if polls % 10 == 0 {
            spawn_local(refresh(weak.clone()));
        }
    }) as Box<dyn FnMut()>);
    let timer = window().set_interval_with_callback_and_timeout_and_arguments_0(
        tick.as_ref().unchecked_ref(),
        200,
    )?;
    spawn_local(refresh(Rc::downgrade(&rc)));
    let weak = Rc::downgrade(&rc);
    spawn_local(async move {
        for _ in 0..600 {
            pause(200).await;
            let Some(rc) = weak.upgrade() else { return };
            let s = rc.borrow();
            if s.snapshot()["phase"] == "open" {
                let target = s.pending.as_ref().and_then(|p| p.pending.selection.clone());
                drop(s);
                if let Some(target) = target {
                    let _ = receipt(&weak, Action::Select { target }).await;
                }
                break;
            }
        }
    });
    Ok(BrowserStudio {
        state: rc,
        listeners,
        message,
        channel,
        broadcast,
        timer,
        tick,
    })
}
