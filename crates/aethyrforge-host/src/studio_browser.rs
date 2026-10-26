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
#[path = "material_browser.rs"]
mod materials;
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
fn world_namespace() -> WorldNamespace {
    if window()
        .document()
        .and_then(|d| d.get_element_by_id("world-namespace"))
        .and_then(|e| e.get_attribute("content"))
        .as_deref()
        == Some("personal-planet")
    {
        WorldNamespace::PersonalPlanet
    } else {
        WorldNamespace::HauntedHouse
    }
}
fn project_path(project: &str) -> String {
    format!("projects/{project}/{}/main", world_namespace().key())
}
fn stored_key(base: &str) -> String {
    if world_namespace() == WorldNamespace::HauntedHouse {
        base.into()
    } else {
        format!("{base}:{}", world_namespace().key())
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
    materials: materials::Controller,
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
                let _ = storage.set_item(
                    &stored_key(VISIT),
                    &serde_json::to_string(&self.visit).unwrap(),
                );
            }
        }
    }
    fn layout(&self) {
        hide("edit-panel", !self.visit.open);
        hide("edit-shadow", !self.visit.open);
        // Link management remains available after the engine frame closes.
        if let Some(frame) = window()
            .document()
            .and_then(|d| d.get_element_by_id("engine"))
        {
            if self.visit.open {
                let _ = frame.set_attribute("inert", "");
            } else {
                let _ = frame.remove_attribute("inert");
            }
        }
        if self.visit.open {
            let _ = element("world-controls").set_attribute("inert", "");
        } else {
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
                    let _ = storage.set_item(&stored_key(SAVED), &text);
                }
            } else {
                let _ = storage.remove_item(&stored_key(SAVED));
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
                        Edit::Place { asset, position, .. } => format!(
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
                Action::ApplyMaterial { material } => vec![format!(
                    "Apply generated scars and handmade paint to {}'s face. Other objects stay as they are.",
                    material.document.target.entity
                )],
                Action::RestoreProject {document,..}|Action::RestoreMaterialProject{document,..}
                    if document.namespace==WorldNamespace::PersonalPlanet => {
                    PlanetRecipe::from_document(document).map(|r|vec![format!("{} · Starting time {} · Day cycle {} · Layout {}",r.style.label(),r.time_label(),if r.cycle_enabled{"running"}else{"paused"},r.seed),format!("Open a courtyard with {} objects. Confirm to replace the current view.",document.entities.len())]).unwrap_or_else(|_|vec!["Review this courtyard before applying it.".into()])
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
    /// See [`BrowserDemo::npc_talk`].
    pub fn npc_talk(&self, intent: String, npc: Option<String>) -> Result<String, JsValue> {
        self.state.borrow().demo.npc_talk(intent, npc)
    }
    /// See [`BrowserDemo::npc_chat`].
    pub fn npc_chat(&self, text: String) -> Result<String, JsValue> {
        self.state.borrow().demo.npc_chat(text)
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
        let body = body.to_string();
        if body.len() > MAX_AUTHORIZATION_BYTES {
            return Err(JsValue::from_str(
                "This change is too large. Export your sources and try fewer material layers.",
            ));
        }
        init.set_body(&JsValue::from_str(&body));
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
        materials::clear(&mut s);
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
    if was_signed && s.account != account {
        projects::account_changed(&mut s);
    }
    s.account = account;
    materials::signed_in(&mut s);
    s.csrf = value["csrf"].as_str().map(str::to_string);
    projects::bind_personal(&mut s);
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
    projects::preparing_planet(&rc.borrow());
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
        let history = matches!(action, Action::Undo | Action::Redo);
        let result = receipt(&weak, action, &scope).await;
        if history && result.is_ok() {
            projects::history_applied(&weak, &scope).await;
        }
        result
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
                if matches!(
                    sent["action"]["type"].as_str(),
                    Some("restore_project" | "restore_material_project")
                ) {
                    projects::restored(&mut s, &sent["action"]);
                }
                projects::record_materials(&mut s, &sent["action"]);
                if sent["action"]["type"] == "apply_material" {
                    materials::applied(&mut s);
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
                s.history.push("You · Change not applied".into());
                if s.history.len() > 32 {
                    s.history.remove(0);
                }
                if matches!(
                    sent["action"]["type"].as_str(),
                    Some("restore_project" | "restore_material_project")
                ) {
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
                    "Working on it…"
                } else {
                    "Ready."
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
/// Resolve with `None` when `promise` hasn't settled within `ms`.
async fn within(promise: Promise, ms: f64) -> Result<Option<JsValue>, JsValue> {
    if ms <= 0.0 {
        return Ok(None);
    }
    let late = JsValue::from_str("house-request-deadline");
    let timer = {
        let late = late.clone();
        Promise::new(&mut |resolve, _| {
            let late = late.clone();
            let done = Closure::once_into_js(move || {
                let _ = resolve.call1(&JsValue::NULL, &late);
            });
            let _ = window().set_timeout_with_callback_and_timeout_and_arguments_0(
                done.unchecked_ref(),
                ms as i32,
            );
        })
    };
    let value = JsFuture::from(Promise::race(&js_sys::Array::of2(&promise, &timer))).await?;
    Ok((value != late).then_some(value))
}
/// One decision request for what the local phrases don't cover.
async fn decide_remotely(
    request: &house_request::HouseRequest,
) -> Result<house_request::HouseAnswer, &'static str> {
    let started = js_sys::Date::now();
    let left = || HOUSE_REQUEST_DEADLINE_MS - (js_sys::Date::now() - started);
    let init = RequestInit::new();
    init.set_method("POST");
    init.set_credentials(RequestCredentials::SameOrigin);
    init.set_body(&JsValue::from_str(
        &serde_json::to_string(request).map_err(|_| house_request::NOT_UNDERSTOOD)?,
    ));
    let fetch = Request::new_with_str_and_init(house_request::ENDPOINT, &init)
        .and_then(|r| {
            r.headers().set("content-type", "application/json")?;
            Ok(window().fetch_with_request(&r))
        })
        .map_err(|_| house_request::NOT_UNDERSTOOD)?;
    let Some(response) = within(fetch, left()).await.map_err(|_| OFFLINE)? else {
        return Err(TOO_SLOW);
    };
    let response: Response = response.dyn_into().map_err(|_| OFFLINE)?;
    let status = response.status();
    let text = response.text().map_err(|_| OFFLINE)?;
    let Some(text) = within(text, left()).await.map_err(|_| OFFLINE)? else {
        return Err(TOO_SLOW);
    };
    let text = text.as_string().unwrap_or_default();
    if status == 429 {
        return Err("That's a lot of requests at once. Wait a moment and try again.");
    }
    if !(200..300).contains(&status) {
        return Err(house_request::NOT_UNDERSTOOD);
    }
    serde_json::from_str(&text).map_err(|_| house_request::NOT_UNDERSTOOD)
}
const HOUSE_REQUEST_DEADLINE_MS: f64 = 5_000.0;
const TOO_SLOW: &str = "Couldn't do that in time. Try 'add three pumpkins to my left'.";
const OFFLINE: &str = "Couldn't reach the house service. Check your connection and try again.";
/// A request applied in this session right away. Guests see it too; saving
/// and publishing still go through sign-in and their own confirmation.
async fn ask_house(weak: Weak<RefCell<State>>, text: String) {
    let Some(rc) = weak.upgrade() else { return };
    let (epoch, context, pose, selected, assets) = {
        let mut s = rc.borrow_mut();
        s.stop_request();
        let Some(context) = s.input_context() else {
            say("Wait for the house to open.");
            return;
        };
        let snapshot = s.snapshot();
        let Ok(checkpoint) =
            serde_json::from_value::<Checkpoint>(snapshot["status"]["checkpoint"].clone())
        else {
            say("Wait for the house to open.");
            return;
        };
        let epoch = s.input_turn.begin(context.clone());
        s.input_epoch = epoch;
        let assets = house_request::house_assets(
            &serde_json::from_str::<Vec<CreatorAsset>>(
                &element("creator-assets").text_content().unwrap_or_default(),
            )
            .unwrap_or_default(),
        );
        (
            epoch,
            context,
            house_request::Pose::from_checkpoint(&checkpoint),
            s.selection.as_ref().map(|v| v.id.clone()),
            assets,
        )
    };
    let current = |rc: &Rc<RefCell<State>>| {
        let s = rc.borrow();
        s.input_context()
            .is_some_and(|c| c == context && s.input_turn.current(epoch, &c))
    };
    let answer = match house_request::parse(&text, &assets) {
        Some(intent) => house_request::answer(&intent, &pose, selected.as_deref()),
        None => {
            element("edit-voice-status").set_text_content(Some(if rc.borrow().voice_enabled {
                "Listening · working on it…"
            } else {
                "Working on it…"
            }));
            say("Working on it…");
            decide_remotely(&house_request::HouseRequest {
                text: text.clone(),
                assets: assets.clone(),
                player: pose,
                selected: selected.clone(),
            })
            .await
        }
    };
    if !current(&rc) {
        return;
    }
    // The request is answered for the world it was asked in. Applying it
    // changes the project revision, which must not read as a stale request
    // (the poll would replace the answer with "The world or selection
    // changed"), so the turn ends here.
    rc.borrow_mut().input_turn.cancel();
    let result = async {
        let answer = answer.map_err(JsValue::from_str)?;
        let mut operations = Vec::new();
        let mut history = Vec::new();
        for edit in &answer.edits {
            match edit {
                house_request::HouseEdit::Place {
                    asset,
                    position,
                    scale,
                    ..
                } => {
                    if !assets.iter().any(|a| &a.id == asset) {
                        return Err(JsValue::from_str(house_request::NOT_UNDERSTOOD));
                    }
                    operations.push(Edit::Place {
                        asset: asset.clone(),
                        position: *position,
                        scale: *scale,
                    });
                }
                house_request::HouseEdit::Remove { target } => operations.push(Edit::Remove {
                    target: target.clone(),
                }),
                house_request::HouseEdit::Undo => history.push(Action::Undo),
                house_request::HouseEdit::Redo => history.push(Action::Redo),
            }
        }
        if operations.is_empty() == history.is_empty()
            || operations.len() > house_request::MAX_COUNT
            || history.len() > 1
        {
            return Err(JsValue::from_str(house_request::NOT_UNDERSTOOD));
        }
        let action = if operations.is_empty() {
            history.remove(0)
        } else {
            Action::Propose { operations }
        };
        preview(&weak, action).await?;
        Ok::<String, JsValue>(answer.say)
    }
    .await;
    // A newer request started while this one applied: it speaks for itself.
    if rc.borrow().input_turn.active() {
        return;
    }
    element("edit-voice-status").set_text_content(Some(if rc.borrow().voice_enabled {
        "Listening"
    } else {
        "Microphone off"
    }));
    match result {
        Ok(words) => say(&words),
        Err(error) => say(error
            .as_string()
            .as_deref()
            .unwrap_or(house_request::NOT_UNDERSTOOD)),
    }
}
/// Apply `action` in this session with a one-use local permission. The
/// engine's permit only limits execution inside this tab; saved and
/// published versions still need a signed-in, server-issued grant.
async fn preview(weak: &Weak<RefCell<State>>, action: Action) -> Result<(), JsValue> {
    let rc = weak.upgrade().ok_or(JsValue::NULL)?;
    let (grant, scope) = {
        let s = rc.borrow();
        if s.busy {
            return Err(JsValue::from_str("Finish the open change first."));
        }
        let value = s.snapshot();
        let project: Project = serde_json::from_value(value["status"]["project"].clone())
            .map_err(|_| JsValue::from_str("Wait for the house to open."))?;
        let session = value["session"].as_str().unwrap_or_default().to_string();
        let release = element("engine-release")
            .get_attribute("content")
            .unwrap_or_default();
        let token: String = (0..8)
            .map(|_| format!("{:08x}", (js_sys::Math::random() * 4_294_967_295.0) as u32))
            .collect();
        let grant = ScopedGrant {
            token,
            action_sha256: action_digest(&action)
                .map_err(|_| JsValue::from_str(house_request::NOT_UNDERSTOOD))?,
            session,
            release,
            draft: Project {
                id: project.id.clone(),
                branch: "session-preview".into(),
                revision: project.revision,
            },
            project,
            capability: action
                .capability()
                .ok_or_else(|| JsValue::from_str(house_request::NOT_UNDERSTOOD))?,
            expires_at: (js_sys::Date::now() / 1000.0) as u64 + 60,
        };
        let scope = projects::confirmation_scope(&s)
            .ok_or_else(|| JsValue::from_str("Wait for the house to open."))?;
        (grant, scope)
    };
    receipt(weak, Action::Authorize { grant }, &scope).await?;
    receipt(weak, action, &scope).await
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
    if projects::personal_request(rc, &text) {
        return;
    }
    if house_request::parse(&text, &[]).is_some_and(|i| {
        matches!(
            i,
            house_request::Intent::Undo
                | house_request::Intent::Redo
                | house_request::Intent::RemoveSelected
        )
    }) {
        spawn_local(ask_house(Rc::downgrade(rc), text.clone()));
        return;
    }
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
            rc.borrow_mut().stop_request();
            element("edit-cancel")
                .unchecked_into::<web_sys::HtmlElement>()
                .click();
        }
        Ok(RequestControl::ReviewCandidate) => {
            if materials::review_voice(rc) {
                return;
            }
            if let Some(control) = element("edit-kiln-review").dyn_ref::<web_sys::HtmlElement>() {
                control.click();
            }
        }
        Ok(RequestControl::ConfirmPublish) => {
            if element("publish-review").has_attribute("open") {
                input("publish-confirm-rights").set_checked(true);
                element("publish-confirm")
                    .unchecked_into::<web_sys::HtmlElement>()
                    .click();
            } else {
                say("Review Publish before confirming it.");
            }
        }
        Ok(RequestControl::CancelPublish) => {
            element("publish-cancel")
                .unchecked_into::<web_sys::HtmlElement>()
                .click();
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
                });
            rc.borrow_mut().stop_request();
            if let Some(control) = available {
                control.click();
            } else {
                say("This option isn't available here yet. Nothing has changed.");
            }
        }
        Ok(RequestControl::Ask) => {
            if !materials::route(rc, &text) {
                materials::forget_voice(&mut rc.borrow_mut());
                spawn_local(ask_house(Rc::downgrade(rc), text.clone()));
            }
        }
        Err(message) => say(message),
    }
}
fn queue_change(rc: &Rc<RefCell<State>>, mut action: Action) {
    let mut s = rc.borrow_mut();
    if s.busy {
        return;
    }
    s.projects.save = None;
    if !matches!(
        action,
        Action::RestoreProject { .. } | Action::RestoreMaterialProject { .. }
    ) {
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
        world: world_namespace().key().into(),
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
        .and_then(|s| s.get_item(&stored_key(SAVED)).ok().flatten())
        .and_then(|s| serde_json::from_str::<Resume>(&s).ok())
        .filter(|r| r.pending.validate().is_ok() && r.pending.world == world_namespace().key());
    let mut visit = window()
        .session_storage()?
        .and_then(|s| s.get_item(&stored_key(VISIT)).ok().flatten())
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
        materials: materials::Controller::default(),
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
        speech_started: 0.,
        voice_enabled: false,
        voice_session: None,
        voice_deadline: 0.,
        voice_next: 0.,
        speech_context: None,
    }));
    let mut listeners = Vec::new();
    projects::mount(&rc, &mut listeners)?;
    materials::mount(&rc, &mut listeners)?;
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
                .map_err(|_| JsValue::from_str("The change is not ready yet."))?;
            if value["project"] != rc.borrow().snapshot()["status"]["project"]
                || value["selection"] != rc.borrow().snapshot()["status"]["selection"]
            {
                return Err(JsValue::from_str(
                    "The world changed. Ask again with the current selection.",
                ));
            }
            let action: Action = serde_json::from_value(value["action"].clone())
                .map_err(|_| JsValue::from_str("The change could not be read."))?;
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
                .unwrap_or("The change could not be read."));
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
                        scale: None,
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
            world: world_namespace().key().into(),
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
            if projects::cancel_planet(&mut rc.borrow_mut()) {
                return;
            }
            projects::cancel_open(&mut rc.borrow_mut());
            rc.borrow().persist();
        }
        say("Your requested change is still here.");
    })?;
    listen(&mut listeners, "edit-auth", "cancel", |event| {
        event.prevent_default();
        element("edit-cancel")
            .unchecked_into::<web_sys::HtmlElement>()
            .click();
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
        materials::poll(&rc);
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
