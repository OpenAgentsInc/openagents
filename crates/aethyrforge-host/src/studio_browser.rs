//! First-edit UI and sign-in continuation. Only this host origin calls account APIs.
use crate::*;
use js_sys::Promise;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::{Rc, Weak},
};
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    Event, EventTarget, HtmlInputElement, HtmlTextAreaElement, MessageEvent, Request,
    RequestCredentials, RequestInit, Response,
};
const SAVED: &str = "openagents.verse.pending.v1";
const VISIT: &str = "openagents.verse.studio.v1";
#[path = "project_browser.rs"]
mod projects;
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
    element("voice-response").set_text_content(Some(text));
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
    #[serde(default)]
    project: Option<Project>,
    pending: PendingChange,
}
struct State {
    projects: projects::Controller,
    demo: BrowserDemo,
    tab: String,
    account: Option<String>,
    csrf: Option<String>,
    pending: Option<Resume>,
    busy: bool,
    popup: Option<web_sys::Window>,
    selection: Option<SelectedObject>,
    owned_world: Option<String>,
    visit: StudioVisit,
    history: Vec<String>,
    restore_complete: bool,
    input_turn: InputTurn,
    input_epoch: u64,
    speech: Option<crate::voice_browser::Speech>,
    speech_event: Rc<RefCell<VecDeque<crate::voice_browser::SpeechEvent>>>,
    kiln_loading: bool,
    speech_started: f64,
    voice_enabled: bool,
    voice_session: Option<String>,
    voice_deadline: f64,
    voice_next: f64,
    speech_context: Option<InputContext>,
}
impl State {
    fn stop_request(&mut self) {
        self.input_turn.cancel();
        let _ = kiln_call("kilnStop", None);
    }
    fn stop_input(&mut self) {
        self.stop_request();
        self.voice_enabled = false;
        self.voice_session = None;
        self.speech_context = None;
        self.speech.take();
        self.speech_event.borrow_mut().clear();
        let _ = kiln_call("kilnStop", None);
        element("edit-voice-start").set_text_content(Some("Listen"));
        element("edit-voice-status").set_text_content(Some("Microphone off"));
    }
    fn input_context(&self) -> Option<InputContext> {
        let v = self.snapshot();
        if v["phase"] != "open" {
            return None;
        }
        Some(InputContext {
            session: v["session"].as_str()?.into(),
            project: serde_json::from_value(v["status"]["project"].clone()).ok()?,
            selection: serde_json::from_value(v["status"]["selection"].clone()).ok(),
        })
    }
    fn snapshot(&self) -> Value {
        serde_json::from_str(&self.demo.snapshot()).unwrap_or(Value::Null)
    }
    fn save_visit(&mut self) {
        self.visit.note = note().value();
        self.visit.source = element("edit-source")
            .unchecked_into::<HtmlTextAreaElement>()
            .value();
        self.visit.request = element("edit-kiln-request")
            .unchecked_into::<HtmlTextAreaElement>()
            .value();
        let snapshot = self.snapshot();
        if let Ok(pose) = serde_json::from_value(snapshot["status"]["checkpoint"].clone()) {
            self.visit.checkpoint = Some(pose);
        }
        if self.restore_complete {
            self.visit.mesh = snapshot["status"]["view"] == "mesh";
            self.visit.selection = snapshot["status"]["selection"]["id"]
                .as_str()
                .map(str::to_string);
        }
        if self.visit.owner.is_none() {
            self.visit.owner = self.account.clone();
        }
        if self.visit.validate().is_ok() {
            if let Ok(Some(storage)) = window().session_storage() {
                let _ = storage.set_item(VISIT, &serde_json::to_string(&self.visit).unwrap());
            }
        }
    }
    fn layout(&self) {
        hide("edit-panel", !self.visit.open);
        hide("edit-shadow", !self.visit.open);
        if self.visit.open {
            let _ = element("engine").set_attribute("inert", "");
            let _ = element("world-controls").set_attribute("inert", "");
        } else {
            let _ = element("engine").remove_attribute("inert");
            let _ = element("world-controls").remove_attribute("inert");
        }
        let _ = element("edit-panel").set_attribute("data-active", &self.visit.active);
        let compact = window()
            .inner_width()
            .ok()
            .and_then(|n| n.as_f64())
            .is_some_and(|n| n < 800.);
        let _ = element("jarvis-toolbar")
            .set_attribute("role", if compact { "tablist" } else { "toolbar" });
        for panel in PANELS {
            let _ = element(&format!("panel-{panel}"))
                .set_attribute("role", if compact { "tabpanel" } else { "region" });
            hide(
                &format!("panel-{panel}"),
                !self.visit.panels.iter().any(|p| p == panel),
            );
            let tab = element(&format!("tab-{panel}"));
            let _ = tab.set_attribute("role", if compact { "tab" } else { "button" });
            let _ = tab.set_attribute(
                "aria-selected",
                if panel == self.visit.active {
                    "true"
                } else {
                    "false"
                },
            );
            if !compact {
                let _ = tab.remove_attribute("aria-selected");
                let _ = tab.set_attribute(
                    "aria-expanded",
                    if self.visit.panels.iter().any(|p| p == panel) {
                        "true"
                    } else {
                        "false"
                    },
                );
            } else {
                let _ = tab.remove_attribute("aria-expanded");
            }
            let _ = tab.set_attribute(
                "tabindex",
                if panel == self.visit.active {
                    "0"
                } else {
                    "-1"
                },
            );
        }
    }
    fn workspace(&mut self, open: bool) {
        self.visit.open = open;
        if open && self.visit.panels.is_empty() {
            self.visit.panels = PANELS.iter().map(|p| p.to_string()).collect();
        }
        self.layout();
        self.save_visit();
        let id = if open {
            format!("tab-{}", self.visit.active)
        } else {
            "world-edit".into()
        };
        if let Some(e) = element(&id).dyn_ref::<web_sys::HtmlElement>() {
            let _ = e.focus();
        }
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
        let list = element("edit-auth-changes");
        list.set_text_content(None);
        if self.projects.save.is_some() {
            element("edit-auth-title").set_text_content(Some(if signed {
                "Save this world?"
            } else {
                "Sign in to save this world"
            }));
            element("edit-auth-description").set_text_content(Some("Your editable content and unfinished source draft will be saved. Publishing is a separate choice."));
            hide("edit-signin", signed);
            hide("edit-confirm", !signed);
            return;
        }
        if let Some(p) = &self.pending {
            let changes = match &p.pending.action {
                Action::Propose { operations } => operations
                    .iter()
                    .map(|op| match op {
                        Edit::Move { target, position } => format!(
                            "Move {target} to {}, {}, {}",
                            position[0], position[1], position[2]
                        ),
                        Edit::Material {
                            target,
                            parameter,
                            value,
                        } => format!("{target}: {parameter} = {value}"),
                        Edit::Place { asset, position } => format!(
                            "Place {asset} at {}, {}, {}",
                            position[0], position[1], position[2]
                        ),
                        Edit::Remove { target } => format!("Remove {target}"),
                    })
                    .collect::<Vec<_>>(),
                Action::Draft { .. } => {
                    vec!["Keep the world-effect source shown in the editor as a draft.".into()]
                }
                Action::Compile { .. } => {
                    vec![
                        "Compile and apply the world effect to every material in the house.".into(),
                    ]
                }
                Action::Undo => vec!["Undo the most recent change.".into()],
                Action::Redo => vec!["Redo the last undone change.".into()],
                _ => vec!["Apply the requested change.".into()],
            };
            for change in changes {
                if let Ok(item) = window().document().expect("document").create_element("li") {
                    item.set_text_content(Some(&change));
                    let _ = list.append_child(&item);
                }
            }
        }
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
        let mut state = self.state.borrow_mut();
        state.stop_input();
        state.demo.close()
    }
}
impl Drop for BrowserStudio {
    fn drop(&mut self) {
        let mut state = self.state.borrow_mut();
        state.stop_input();
        state.demo.close();
        let _ = kiln_call("kilnStop", None);
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
    let (code, value) = api_response(path, body).await?;
    if !(200..300).contains(&code) {
        return Err(JsValue::from_str(
            value["error"]["message"]
                .as_str()
                .unwrap_or("The change could not be checked. Try again."),
        ));
    }
    Ok(value)
}
async fn api_response(path: &str, body: Option<Value>) -> Result<(u16, Value), JsValue> {
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
    Ok((response.status(), value))
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
    // A confirmed request may already have reached the private worker before its
    // receipt is polled. End that worker even before owned_world is assigned.
    if s.account.is_some() && s.account != account && s.busy {
        let _ = s.request(&Action::RevokeAuthorization);
        s.demo.reload();
        projects::world_reloading(&mut s);
        s.owned_world = None;
        s.busy = false;
        s.restore_complete = false;
    }
    let policy = resume_policy(
        s.pending
            .as_ref()
            .and_then(|p| p.owner.as_deref())
            .or(s.visit.owner.as_deref()),
        s.account.as_deref(),
        account.as_deref(),
    );
    if s.owned_world
        .as_ref()
        .is_some_and(|owner| Some(owner) != account.as_ref())
    {
        let _ = s.request(&Action::RevokeAuthorization);
        s.demo.reload();
        projects::world_reloading(&mut s);
        s.owned_world = None;
    }
    if policy == ResumePolicy::Discard {
        projects::clear(&mut s);
        s.stop_input();
        s.pending = None;
        s.persist();
        note().set_value("");
        element("edit-source")
            .unchecked_into::<HtmlTextAreaElement>()
            .set_value("");
        element("edit-kiln-request")
            .unchecked_into::<HtmlTextAreaElement>()
            .set_value("");
        let _ = kiln_call("kilnVerseClear", None);
        s.history.clear();
        s.visit.discard_for(account.clone());
        s.layout();
        s.save_visit();
        hide("edit-auth", true);
        say("Account changed. Choose a new change to continue.");
    }
    let was_signed = s.account.is_some();
    s.account = account;
    s.csrf = value["csrf"].as_str().map(str::to_string);
    if policy == ResumePolicy::Reconfirm && (s.pending.is_some() || s.projects.save.is_some()) {
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
fn require_scope(weak: &Weak<RefCell<State>>, scope: &ConfirmationScope) -> Result<(), JsValue> {
    let rc = weak.upgrade().ok_or(JsValue::NULL)?;
    if scope.accepts(projects::confirmation_scope(&rc.borrow()).as_ref()) {
        Ok(())
    } else {
        Err(JsValue::from_str(
            "The account, project, or world changed. Review your change again.",
        ))
    }
}
async fn receipt(
    weak: &Weak<RefCell<State>>,
    action: Action,
    scope: &ConfirmationScope,
) -> Result<(), JsValue> {
    require_scope(weak, scope)?;
    let id = weak
        .upgrade()
        .ok_or(JsValue::NULL)?
        .borrow()
        .request(&action)?;
    for _ in 0..300 {
        pause(50).await;
        require_scope(weak, scope)?;
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
    if weak
        .upgrade()
        .is_some_and(|rc| rc.borrow().projects.save.is_some())
    {
        projects::confirm_save(weak).await;
        return;
    }
    let Some(rc) = weak.upgrade() else { return };
    let (sent, scope) = {
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
        if p.project.as_ref().is_some_and(|base| {
            serde_json::to_value(base).ok().as_ref() != Some(&value["status"]["project"])
        }) {
            say("The world changed. Review your change and try again.");
            return;
        }
        let Some(scope) = projects::confirmation_scope(&s) else {
            s.auth_card();
            return;
        };
        s.busy = true;
        (
            json!({"csrf":csrf,"tab":s.tab,"session":value["session"],"release":element("engine-release").get_attribute("content"),"project":value["status"]["project"],"action":p.pending.action,"confirmed":true}),
            scope,
        )
    };
    let result = async {
        require_scope(&weak, &scope)?;
        let value = api("grants", Some(sent.clone())).await?;
        require_scope(&weak, &scope)?;
        let grant: ScopedGrant = serde_json::from_value(value["grant"].clone())
            .map_err(|_| JsValue::from_str("Permission could not be checked."))?;
        let action: Action = serde_json::from_value(sent["action"].clone())
            .map_err(|_| JsValue::from_str("Choose a change again."))?;
        api(
            "authorize",
            Some(json!({"csrf":sent["csrf"],"tab":sent["tab"],"grant":grant,"action":action})),
        )
        .await?;
        require_scope(&weak, &scope)?;
        receipt(&weak, Action::Authorize { grant }, &scope).await?;
        require_scope(&weak, &scope)?;
        receipt(&weak, action, &scope).await
    }
    .await;
    if let Some(rc) = weak.upgrade() {
        let mut s = rc.borrow_mut();
        if !scope.accepts(projects::confirmation_scope(&s).as_ref()) {
            return;
        }
        s.busy = false;
        match result {
            Ok(()) => {
                if sent["action"]["type"] == "restore_project" {
                    projects::restored(&mut s, &sent["action"]);
                }
                s.owned_world = scope.account.clone();
                s.pending = None;
                s.persist();
                hide("edit-auth", true);
                if let Some(action) = sent.get("action") {
                    let message = if action["type"] == "compile" {
                        "World effect compiled"
                    } else if action["type"] == "draft" {
                        "World-effect draft kept"
                    } else if action["type"] == "undo" {
                        "Change undone"
                    } else if action["type"] == "redo" {
                        "Change redone"
                    } else {
                        "Project change applied"
                    };
                    s.history.push(format!("You · {message}"));
                    if s.history.len() > 32 {
                        s.history.remove(0);
                    }
                    if action["type"] == "compile" {
                        s.workspace(false);
                    }
                }
                say("We applied your change.");
            }
            Err(error) => {
                if sent["action"]["type"] == "restore_project" {
                    projects::cancel_open(&mut s);
                }
                hide("edit-auth", true);
                if sent["action"]["type"] == "compile" {
                    s.visit.show("compile");
                    s.layout();
                }
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
    if let Ok(raw) = kiln_call("kilnState", None) {
        if let Ok(value) = serde_json::from_str::<Value>(&raw.as_string().unwrap_or_default()) {
            let text = value["problem"]
                .as_str()
                .or_else(|| value["working"].as_str())
                .unwrap_or(if value["busy"] == true {
                    "Kiln is working."
                } else {
                    "Kiln is ready for review."
                });
            element("edit-kiln-status").set_text_content(Some(text));
        }
    }
    let mut s = rc.borrow_mut();
    let selection =
        serde_json::from_value::<SelectedObject>(s.snapshot()["status"]["selection"].clone()).ok();
    if let Ok(view) =
        serde_json::from_value::<AuthoringView>(s.snapshot()["status"]["authoring"].clone())
    {
        hide("author-tools", false);
        hide("prop-tools", false);
        let source: HtmlTextAreaElement = element("edit-source").unchecked_into();
        if source.value().is_empty() {
            source.set_value(&view.draft);
        }
        element("edit-source-status").set_text_content(Some(&if view.diagnostics.is_empty() {
            if view.draft == view.active_source {
                "The compiled world effect is active."
            } else {
                "Your source draft has changes to compile."
            }
            .into()
        } else {
            view.diagnostics.join("\n")
        }));
        let ids = view
            .materials
            .iter()
            .filter(|m| {
                selection
                    .as_ref()
                    .is_some_and(|s| m.objects.contains(&s.id))
            })
            .map(|m| m.id.as_str())
            .collect::<Vec<_>>();
        element("edit-material-ids").set_text_content(Some(&ids.join(", ")));
        if input("edit-material").value().is_empty() {
            if let Some(id) = ids.first() {
                input("edit-material").set_value(id);
            }
        }
        for (id, enabled) in [("edit-undo", view.can_undo), ("edit-redo", view.can_redo)] {
            if enabled {
                let _ = element(id).remove_attribute("disabled");
            } else {
                let _ = element(id).set_attribute("disabled", "");
            }
        }
    }
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
fn kiln_call(name: &str, arg: Option<&str>) -> Result<JsValue, JsValue> {
    let f: js_sys::Function =
        js_sys::Reflect::get(&window(), &JsValue::from_str(name))?.dyn_into()?;
    if let Some(arg) = arg {
        f.call1(&JsValue::NULL, &JsValue::from_str(arg))
    } else {
        f.call0(&JsValue::NULL)
    }
}
async fn ask_kiln(weak: Weak<RefCell<State>>) {
    let Some(rc) = weak.upgrade() else { return };
    let (snapshot, epoch) = {
        let mut s = rc.borrow_mut();
        if s.kiln_loading {
            say("Kiln is opening. Try your request again in a moment.");
            return;
        }
        s.stop_request();
        let Some(context) = s.input_context() else {
            return;
        };
        let epoch = s.input_turn.begin(context);
        s.input_epoch = epoch;
        s.kiln_loading = true;
        (s.snapshot(), epoch)
    };
    let status = &snapshot["status"];
    let Ok(view) = serde_json::from_value::<AuthoringView>(status["authoring"].clone()) else {
        rc.borrow_mut().kiln_loading = false;
        say("Wait for the editing tools to open.");
        return;
    };
    let selection = status["selection"].clone();
    let mut view = view;
    view.materials.retain(|m| {
        selection["id"]
            .as_str()
            .is_some_and(|id| m.objects.iter().any(|s| s == id))
    });
    let context =
        json!({"project":status["project"],"selection":selection,"authoring":view,"allow_world_effect":input("edit-kiln-world").checked(),"assets":serde_json::from_str::<Vec<CreatorAsset>>(&element("creator-assets").text_content().unwrap_or_default()).unwrap_or_default(),"camera":status["checkpoint"]}).to_string();
    let task: HtmlTextAreaElement = element("edit-kiln-request").unchecked_into();
    let task = task.value();
    if task.trim().is_empty() {
        rc.borrow_mut().kiln_loading = false;
        return;
    }
    element("edit-voice-status").set_text_content(Some(if rc.borrow().voice_enabled {
        "Listening · Kiln is processing"
    } else {
        "Processing request · microphone off"
    }));
    say("Kiln is working. Say Review change when it is ready, or use the review button.");
    let run = async {
        if js_sys::Reflect::get(&window(), &"kilnVerseContext".into())?.is_function() {
            kiln_call("kilnVerseContext", Some(&context))?;
        } else {
            let p = kiln_call("loadVerseKiln", Some(&context))?;
            JsFuture::from(Promise::resolve(&p)).await?;
        }
        let current = rc
            .borrow()
            .input_context()
            .is_some_and(|c| rc.borrow().input_turn.current(epoch, &c));
        if !current {
            return Err(JsValue::from_str(
                "The request stopped. Review the current world before trying again.",
            ));
        }
        // Loading can yield. Reinstall the captured context only after the freshness check.
        kiln_call("kilnVerseContext", Some(&context))?;
        kiln_call("kilnAgent", Some(&task))?;
        Ok::<(), JsValue>(())
    }
    .await;
    rc.borrow_mut().kiln_loading = false;
    if let Err(error) = run {
        element("edit-kiln-status").set_text_content(Some(
            error
                .as_string()
                .as_deref()
                .unwrap_or("Kiln could not start. Your manual tools are still available."),
        ));
    }
}
fn start_speech(s: &mut State) {
    let Some(context) = s.input_context() else {
        return;
    };
    s.speech_context = Some(context);
    let inbox = s.speech_event.clone();
    match crate::voice_browser::Speech::start(move |event| {
        let mut q = inbox.borrow_mut();
        if q.len() < 8 {
            q.push_back(event);
        } else {
            q.clear();
            q.push_back(crate::voice_browser::SpeechEvent::Failed(
                "Voice input is busy. Try one request at a time.".into(),
            ));
        }
    }) {
        Ok(speech) => {
            s.speech_started = js_sys::Date::now();
            s.speech = Some(speech);
            element("edit-voice-status").set_text_content(Some("Listening · say Stop to mute"));
            element("edit-voice-start").set_text_content(Some("Listening"));
        }
        Err(_) => {
            s.stop_input();
            element("edit-voice-status").set_text_content(Some(
                "Voice input couldn't start. Type your request or try again.",
            ));
        }
    }
}
fn route_request(rc: &Rc<RefCell<State>>) {
    let text = element("edit-kiln-request")
        .unchecked_into::<HtmlTextAreaElement>()
        .value();
    let capabilities: Vec<Capability> =
        serde_json::from_value(rc.borrow().snapshot()["capabilities"].clone()).unwrap_or_default();
    // Feedback describes this request; earlier failures remain in their own tool state.
    say("");
    element("voice-heard").set_text_content(Some(&text));
    let control = request_control(&text, &capabilities);
    match control {
        Ok(RequestControl::Stop) => {
            rc.borrow_mut().stop_input();
            say("Listening stopped.");
        }
        Ok(RequestControl::Options) => {
            hide("request-choices", false);
            say("Choose an option, or say its name.");
        }
        Ok(RequestControl::CancelReview) => {
            hide("edit-auth", true);
            rc.borrow_mut().stop_request();
            say("Your requested change is still here.");
        }
        Ok(RequestControl::ReviewCandidate) => {
            if let Some(control) = element("edit-kiln-review").dyn_ref::<web_sys::HtmlElement>() {
                control.click();
            }
        }
        Ok(RequestControl::ConfirmChange) => {
            let s = rc.borrow();
            let allowed = element("edit-auth").has_attribute("open")
                && s.account.is_some()
                && (s.projects.save.is_some()
                    || s.pending
                        .as_ref()
                        .is_some_and(|p| !matches!(p.pending.action, Action::Publish { .. })));
            drop(s);
            if allowed {
                say("Applying your reviewed change…");
                spawn_local(confirm(Rc::downgrade(rc)));
            } else {
                say("Sign in, then review the change before confirming it.");
            }
        }
        Ok(RequestControl::Inspect) => {
            hide("request-choices", true);
            rc.borrow_mut().workspace(false);
            say("You can keep exploring.");
        }
        Ok(RequestControl::Map) => {
            rc.borrow_mut().stop_input();
            let _ = window()
                .location()
                .set_href("/new-lander/verse?world=haunted-house");
        }
        Ok(RequestControl::Review(action)) => {
            rc.borrow_mut().stop_request();
            if matches!(action, Action::View { .. }) {
                match rc.borrow().request(&action) {
                    Ok(_) => say("Changing the view…"),
                    Err(_) => say("The view could not change. Try again."),
                }
            } else {
                say("Review the change before confirming it.");
                queue_change(rc, action);
            }
        }
        Ok(RequestControl::Option(option)) => {
            let available = window()
                .document()
                .and_then(|d| d.get_element_by_id(option.control_id()))
                .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
                .filter(|e| {
                    !e.has_attribute("disabled")
                        && !e.has_attribute("hidden")
                        && e.get_attribute("data-tutorial-option").as_deref() == Some("ready")
                        && e.offset_width() > 0
                });
            rc.borrow_mut().stop_request();
            if let Some(control) = available {
                control.click();
            } else {
                say("This option isn't available here yet. Nothing has changed.");
            }
        }
        Ok(RequestControl::AskKiln) => spawn_local(ask_kiln(Rc::downgrade(rc))),
        Err(message) => say(message),
    }
}
fn queue_change(rc: &Rc<RefCell<State>>, mut action: Action) {
    let mut s = rc.borrow_mut();
    if s.busy {
        return;
    }
    s.projects.save = None;
    if !matches!(action, Action::RestoreProject { .. }) {
        projects::cancel_open(&mut s);
    }
    let snapshot = s.snapshot();
    if let Action::Compile { source_sha256, .. } = &mut action {
        use sha2::Digest;
        let Some(source) = snapshot["status"]["authoring"]["draft"].as_str() else {
            say("Wait for the source draft to open.");
            return;
        };
        let editor: HtmlTextAreaElement = element("edit-source").unchecked_into();
        if editor.value() != source {
            say("Keep your source draft before compiling it.");
            return;
        }
        *source_sha256 = Some(format!("{:x}", sha2::Sha256::digest(source.as_bytes())));
    }
    let Ok(checkpoint) = serde_json::from_value(snapshot["status"]["checkpoint"].clone()) else {
        say("Wait for the house to open.");
        return;
    };
    let pending = PendingChange {
        world: "haunted-house".into(),
        checkpoint,
        selection: s.selection.as_ref().map(|s| s.id.clone()),
        panels: vec!["edit".into()],
        draft: note().value(),
        action,
    };
    if pending.validate().is_err() {
        say("Choose a valid project change.");
        return;
    }
    s.pending = Some(Resume {
        owner: s.account.clone(),
        project: serde_json::from_value(snapshot["status"]["project"].clone()).ok(),
        pending,
    });
    s.persist();
    s.auth_card();
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
    let mut visit = window()
        .session_storage()?
        .and_then(|s| s.get_item(VISIT).ok().flatten())
        .and_then(|s| serde_json::from_str::<StudioVisit>(&s).ok())
        .filter(|v| v.validate().is_ok())
        .unwrap_or_default();
    if pending.is_some() {
        visit.open = true;
    }
    note().set_value(&visit.note);
    element("edit-source")
        .unchecked_into::<HtmlTextAreaElement>()
        .set_value(&visit.source);
    element("edit-kiln-request")
        .unchecked_into::<HtmlTextAreaElement>()
        .set_value(&visit.request);
    let demo = mount_demo()?;
    if let Some(pose) = visit.checkpoint.as_ref() {
        if let Some(h) = demo.host.borrow().as_ref() {
            h.restore_checkpoint(serde_json::to_string(pose).unwrap())?;
        }
    }
    if let Some(p) = &pending {
        if let Some(h) = demo.host.borrow().as_ref() {
            h.restore_checkpoint(serde_json::to_string(&p.pending.checkpoint).unwrap())?;
        }
        note().set_value(&p.pending.draft);
        if let Action::Draft { source, .. } = &p.pending.action {
            let editor: HtmlTextAreaElement = element("edit-source").unchecked_into();
            editor.set_value(source);
        }
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
        projects: projects::Controller::default(),
        demo,
        tab: random(),
        account: None,
        csrf: None,
        pending,
        busy: false,
        popup: None,
        selection: None,
        owned_world: None,
        visit,
        history: Vec::new(),
        restore_complete: false,
        input_turn: InputTurn::default(),
        input_epoch: 0,
        speech: None,
        speech_event: Rc::new(RefCell::new(VecDeque::new())),
        kiln_loading: false,
        speech_started: 0.,
        voice_enabled: false,
        voice_session: None,
        voice_deadline: 0.,
        voice_next: 0.,
        speech_context: None,
    }));
    let mut listeners = Vec::new();
    projects::mount(&rc, &mut listeners)?;
    let speech_supported = crate::voice_browser::supported();
    hide("world-voice", false);
    hide("edit-voice-start", !speech_supported);
    hide("edit-voice-stop", !speech_supported);
    if !speech_supported {
        element("edit-voice-status").set_text_content(Some(
            "Voice isn't available here. Type your request in Jarvis.",
        ));
    }
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "edit-voice-start", "click", move |_| {
        let Some(rc) = weak.upgrade() else {
            return;
        };
        let mut s = rc.borrow_mut();
        s.stop_input();
        say("");
        element("voice-heard").set_text_content(None);
        let Some(context) = s.input_context() else {
            say("Wait for the house to open.");
            return;
        };
        s.voice_enabled = true;
        s.voice_session = Some(context.session.clone());
        s.voice_deadline = js_sys::Date::now() + 180_000.;
        start_speech(&mut s);
    })?;
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "edit-voice-stop", "click", move |_| {
        if let Some(rc) = weak.upgrade() {
            rc.borrow_mut().stop_input();
        }
    })?;
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "edit-auth-voice-stop", "click", move |_| {
        if let Some(rc) = weak.upgrade() {
            rc.borrow_mut().stop_input();
        }
    })?;
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "edit-request-review", "click", move |_| {
        if let Some(rc) = weak.upgrade() {
            route_request(&rc);
        }
    })?;
    for (id, command) in [
        ("request-options", "show options"),
        ("request-inspect", "inspect"),
        ("request-mesh", "show mesh"),
        ("request-map", "map"),
        ("request-compile", "compile"),
        ("request-undo", "undo"),
    ] {
        let weak = Rc::downgrade(&rc);
        listen(&mut listeners, id, "click", move |_| {
            if let Some(rc) = weak.upgrade() {
                element("edit-kiln-request")
                    .unchecked_into::<HtmlTextAreaElement>()
                    .set_value(command);
                route_request(&rc);
            }
        })?;
    }
    let weak = Rc::downgrade(&rc);
    let visibility = Closure::wrap(Box::new(move |_: Event| {
        if window().document().is_some_and(|d| d.hidden()) {
            if let Some(rc) = weak.upgrade() {
                rc.borrow_mut().stop_input();
            }
        }
    }) as Box<dyn FnMut(Event)>);
    let target: EventTarget = window().document().expect("document").into();
    target.add_event_listener_with_callback(
        "visibilitychange",
        visibility.as_ref().unchecked_ref(),
    )?;
    listeners.push((target, "visibilitychange", visibility));
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "world-edit", "click", move |_| {
        if let Some(rc) = weak.upgrade() {
            rc.borrow_mut().workspace(true);
        }
    })?;
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "edit-close", "click", move |_| {
        if let Some(rc) = weak.upgrade() {
            rc.borrow_mut().workspace(false);
        }
    })?;
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "edit-pick", "click", move |_| {
        if let Some(rc) = weak.upgrade() {
            rc.borrow_mut().workspace(false);
            say("Click to look, aim at an object, then click to select it.");
        }
    })?;
    for panel in PANELS {
        let weak = Rc::downgrade(&rc);
        // The DOM IDs are fixed by the Rust page; listeners use stable names for cleanup.
        let tab = format!("tab-{panel}");
        listen(&mut listeners, &tab, "click", move |_| {
            if let Some(rc) = weak.upgrade() {
                let mut s = rc.borrow_mut();
                s.visit.show(panel);
                s.layout();
                s.save_visit();
            }
        })?;
        let weak = Rc::downgrade(&rc);
        let close = format!("close-{panel}");
        listen(&mut listeners, &close, "click", move |_| {
            if let Some(rc) = weak.upgrade() {
                let mut s = rc.borrow_mut();
                s.visit.hide(panel);
                s.layout();
                s.save_visit();
            }
        })?;
    }
    for id in ["edit-note", "edit-source", "edit-kiln-request"] {
        let weak = Rc::downgrade(&rc);
        listen(&mut listeners, id, "input", move |_| {
            if let Some(rc) = weak.upgrade() {
                if id == "edit-kiln-request" {
                    rc.borrow_mut().stop_request();
                }
                rc.borrow_mut().save_visit();
            }
        })?;
    }
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "world-mesh", "click", move |_| {
        let Some(rc) = weak.upgrade() else { return };
        let current = rc.borrow().snapshot()["status"]["view"].as_str() == Some("mesh");
        let _ = rc.borrow().request(&Action::View {
            mode: if current {
                ViewMode::Shaded
            } else {
                ViewMode::Mesh
            },
        });
    })?;
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "edit-auth", "cancel", move |event| {
        event.prevent_default();
        hide("edit-auth", true);
        if let Some(rc) = weak.upgrade() {
            projects::cancel_open(&mut rc.borrow_mut());
            rc.borrow_mut().save_visit();
        }
        say("Your requested change is still here.");
    })?;
    let weak = Rc::downgrade(&rc);
    let key = Closure::wrap(Box::new(move |event: Event| {
        let Ok(key) = event.dyn_into::<web_sys::KeyboardEvent>() else {
            return;
        };
        let Some(rc) = weak.upgrade() else { return };
        if element("edit-auth").has_attribute("open") {
            return;
        }
        if key.key() == "Escape" && rc.borrow().visit.open {
            key.prevent_default();
            rc.borrow_mut().workspace(false);
            return;
        }
        let target = key
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok());
        if let Some(id) = target.as_ref().map(|e| e.id()) {
            if let Some(panel) = id.strip_prefix("tab-") {
                let index = PANELS.iter().position(|p| *p == panel).unwrap_or(0);
                let next = match key.key().as_str() {
                    "ArrowRight" => Some((index + 1) % 5),
                    "ArrowLeft" => Some((index + 4) % 5),
                    "Home" => Some(0),
                    "End" => Some(4),
                    _ => None,
                };
                if let Some(next) = next {
                    key.prevent_default();
                    let mut s = rc.borrow_mut();
                    s.visit.show(PANELS[next]);
                    s.layout();
                    s.save_visit();
                    if let Some(tab) =
                        element(&format!("tab-{}", PANELS[next])).dyn_ref::<web_sys::HtmlElement>()
                    {
                        let _ = tab.focus();
                    }
                    return;
                }
            }
        }
        let typing = target.as_ref().is_some_and(|e| {
            matches!(e.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT")
                || e.has_attribute("contenteditable")
        });
        if !typing && key.alt_key() && key.key().eq_ignore_ascii_case("e") {
            key.prevent_default();
            rc.borrow_mut().workspace(true);
        }
    }) as Box<dyn FnMut(Event)>);
    let target: EventTarget = window().into();
    target.add_event_listener_with_callback("keydown", key.as_ref().unchecked_ref())?;
    listeners.push((target, "keydown", key));
    for name in ["pagehide", "offline"] {
        let weak = Rc::downgrade(&rc);
        let handler = Closure::wrap(Box::new(move |_: Event| {
            if let Some(rc) = weak.upgrade() {
                rc.borrow_mut().stop_input();
            }
        }) as Box<dyn FnMut(Event)>);
        let target: EventTarget = window().into();
        target.add_event_listener_with_callback(name, handler.as_ref().unchecked_ref())?;
        listeners.push((target, name, handler));
    }
    let weak = Rc::downgrade(&rc);
    let resize = Closure::wrap(Box::new(move |_: Event| {
        if let Some(rc) = weak.upgrade() {
            rc.borrow().layout();
        }
    }) as Box<dyn FnMut(Event)>);
    let target: EventTarget = window().into();
    target.add_event_listener_with_callback("resize", resize.as_ref().unchecked_ref())?;
    listeners.push((target, "resize", resize));
    window()
        .document()
        .expect("document")
        .body()
        .expect("body")
        .set_attribute("data-jarvis-ready", "true")?;
    hide("jarvis-toolbar", false);
    rc.borrow().layout();
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "edit-capture", "click", move |_| {
        let weak = weak.clone();
        spawn_local(async move {
            let Some(rc) = weak.upgrade() else { return };
            let Ok(id) = rc.borrow().request(&Action::Capture) else {
                return;
            };
            for _ in 0..300 {
                pause(50).await;
                let value = rc.borrow().snapshot();
                if let Some(r) = value["receipts"]
                    .as_array()
                    .and_then(|rows| rows.iter().find(|r| r["request_id"] == id))
                {
                    let detail = &r["outcome"]["detail"];
                    if detail["type"] == "capture_preview" {
                        if let Some(png) = detail["png"].as_str() {
                            let _ = element("edit-capture-image")
                                .set_attribute("src", &format!("data:image/png;base64,{png}"));
                            hide("edit-capture-image", false);
                            say("Preview captured.");
                        }
                    } else {
                        say(r["outcome"]["message"]
                            .as_str()
                            .unwrap_or("The preview could not be captured."));
                    }
                    break;
                }
            }
        });
    })?;
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "edit-kiln-ask", "click", move |_| {
        if let Some(rc) = weak.upgrade() {
            route_request(&rc);
        }
    })?;
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "edit-kiln-stop", "click", move |_| {
        if let Some(rc) = weak.upgrade() {
            rc.borrow_mut().stop_input();
        }
    })?;
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "edit-kiln-review", "click", move |_| {
        let Some(rc) = weak.upgrade() else { return };
        if !rc
            .borrow()
            .input_context()
            .is_some_and(|c| rc.borrow().input_turn.current(rc.borrow().input_epoch, &c))
        {
            say("This request ended. Ask again with the current world and selection.");
            return;
        }
        let result = kiln_call("kilnVerseCandidate", None).and_then(|raw| {
            let value: Value = serde_json::from_str(&raw.as_string().unwrap_or_default())
                .map_err(|_| JsValue::from_str("Kiln's change is not ready yet."))?;
            if value["project"] != rc.borrow().snapshot()["status"]["project"]
                || value["selection"] != rc.borrow().snapshot()["status"]["selection"]
            {
                return Err(JsValue::from_str(
                    "The world changed. Ask Kiln again with the current selection.",
                ));
            }
            let action: Action = serde_json::from_value(value["action"].clone())
                .map_err(|_| JsValue::from_str("Kiln's change could not be read."))?;
            if let Action::Draft { source, .. } = &action {
                let field: HtmlTextAreaElement = element("edit-source").unchecked_into();
                field.set_value(source);
            }
            queue_change(&rc, action);
            Ok(())
        });
        if let Err(error) = result {
            say(error
                .as_string()
                .as_deref()
                .unwrap_or("Kiln's change could not be read."));
        }
    })?;
    for (id, action) in [
        ("edit-undo", Action::Undo),
        ("edit-redo", Action::Redo),
        (
            "edit-source-compile",
            Action::Compile {
                asset: Some(EFFECT_ASSET.into()),
                source_sha256: None,
            },
        ),
    ] {
        let weak = Rc::downgrade(&rc);
        listen(&mut listeners, id, "click", move |_| {
            if let Some(rc) = weak.upgrade() {
                queue_change(&rc, action.clone());
            }
        })?;
    }
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "edit-source-draft", "click", move |_| {
        if let Some(rc) = weak.upgrade() {
            let source: HtmlTextAreaElement = element("edit-source").unchecked_into();
            queue_change(
                &rc,
                Action::Draft {
                    asset: EFFECT_ASSET.into(),
                    source: source.value(),
                },
            );
        }
    })?;
    for id in ["edit-material-apply", "edit-fog-apply"] {
        let weak = Rc::downgrade(&rc);
        listen(&mut listeners, id, "click", move |_| {
            if let Some(rc) = weak.upgrade() {
                let (target, parameter, value) = if id == "edit-fog-apply" {
                    (
                        "environment".into(),
                        "fogDensity".into(),
                        input("edit-fog").value_as_number(),
                    )
                } else {
                    (
                        input("edit-material").value(),
                        input("edit-parameter").value(),
                        input("edit-value").value_as_number(),
                    )
                };
                queue_change(
                    &rc,
                    Action::Propose {
                        operations: vec![Edit::Material {
                            target,
                            parameter,
                            value,
                        }],
                    },
                );
            }
        })?;
    }
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "edit-place", "click", move |_| {
        if let Some(rc) = weak.upgrade() {
            let asset = input("edit-place-asset").value();
            let assets: Vec<CreatorAsset> =
                serde_json::from_str(&element("creator-assets").text_content().unwrap_or_default())
                    .unwrap_or_default();
            if !assets.iter().any(|a| a.id == asset) {
                say("Choose a prop from the list.");
                return;
            }
            queue_change(
                &rc,
                Action::Propose {
                    operations: vec![Edit::Place {
                        asset,
                        position: [
                            input("edit-x").value_as_number(),
                            input("edit-y").value_as_number(),
                            input("edit-z").value_as_number(),
                        ],
                    }],
                },
            );
        }
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
            project: serde_json::from_value(snapshot["status"]["project"].clone()).ok(),
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
            projects::cancel_open(&mut rc.borrow_mut());
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
        projects::poll(&rc);
        let mut spoken = false;
        {
            let mut s = rc.borrow_mut();
            let current = s.input_context();
            if s.input_turn.active() && current.as_ref().is_none_or(|c| s.input_turn.changed(c)) {
                s.stop_request();
                say("The world or selection changed. Ask again with the current selection.");
            }
            if s.voice_enabled
                && (js_sys::Date::now() > s.voice_deadline
                    || current
                        .as_ref()
                        .is_none_or(|c| Some(&c.session) != s.voice_session.as_ref()))
            {
                s.stop_input();
            }
            let events: Vec<_> = s.speech_event.borrow_mut().drain(..).collect();
            for event in events {
                if matches!(event, crate::voice_browser::SpeechEvent::Started) {
                    s.speech_context = current.clone();
                    s.speech_started = js_sys::Date::now();
                    continue;
                }
                let current = current.as_ref() == s.speech_context.as_ref() && current.is_some();

                if s.voice_enabled && current {
                    match event {
                        crate::voice_browser::SpeechEvent::Words(text) => {
                            element("edit-kiln-request")
                                .unchecked_into::<HtmlTextAreaElement>()
                                .set_value(&text);
                            element("voice-heard").set_text_content(Some(&text));
                            s.save_visit();
                            spoken = true;
                        }
                        crate::voice_browser::SpeechEvent::Started => {}
                        crate::voice_browser::SpeechEvent::Ended => {
                            s.speech.take();
                            s.voice_next = js_sys::Date::now() + 350.;
                        }
                        crate::voice_browser::SpeechEvent::Failed(message) => {
                            s.stop_input();
                            element("edit-voice-status").set_text_content(Some(&message));
                        }
                    }
                } else if s.voice_enabled {
                    element("edit-voice-status")
                        .set_text_content(Some("Selection changed · request ignored"));
                }
            }
            if s.speech.is_some() && js_sys::Date::now() - s.speech_started > 45_000. {
                s.stop_input();
                element("edit-voice-status")
                    .set_text_content(Some("Microphone off · listening timed out"));
            }
            if s.voice_enabled && s.speech.is_none() && js_sys::Date::now() >= s.voice_next {
                start_speech(&mut s);
            }
        }
        if spoken {
            route_request(&rc);
        }
        let snapshot = rc.borrow().snapshot();
        if let Some(view) = snapshot["status"]["view"].as_str() {
            hide("world-mesh", false);
            let _ = element("world-mesh").set_attribute(
                "aria-pressed",
                if view == "mesh" { "true" } else { "false" },
            );
        }
        let list = element("edit-history-entries");
        list.set_text_content(None);
        for row in &rc.borrow().history {
            if let Ok(li) = window().document().expect("document").create_element("li") {
                li.set_text_content(Some(row));
                let _ = list.append_child(&li);
            }
        }
        polls += 1;
        if polls % 5 == 0 {
            rc.borrow_mut().save_visit();
        }
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
                let target = s
                    .pending
                    .as_ref()
                    .and_then(|p| p.pending.selection.clone())
                    .or_else(|| s.visit.selection.clone());
                let Some(scope) = projects::confirmation_scope(&s) else {
                    return;
                };
                let mesh = s.visit.mesh;
                drop(s);
                if let Some(target) = target {
                    if receipt(&weak, Action::Select { target }, &scope)
                        .await
                        .is_err()
                        && require_scope(&weak, &scope).is_err()
                    {
                        return;
                    }
                }
                if require_scope(&weak, &scope).is_err() {
                    return;
                }
                if mesh {
                    let _ = receipt(
                        &weak,
                        Action::View {
                            mode: ViewMode::Mesh,
                        },
                        &scope,
                    )
                    .await;
                }
                if require_scope(&weak, &scope).is_err() {
                    return;
                }
                if let Some(rc) = weak.upgrade() {
                    rc.borrow_mut().restore_complete = true;
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
