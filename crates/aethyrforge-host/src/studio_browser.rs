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
    #[serde(default)]
    project: Option<Project>,
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
        let list = element("edit-auth-changes");
        list.set_text_content(None);
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
        if p.project.as_ref().is_some_and(|base| {
            serde_json::to_value(base).ok().as_ref() != Some(&value["status"]["project"])
        }) {
            say("The world changed. Review your change and try again.");
            return;
        }
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
    let snapshot = rc.borrow().snapshot();
    let status = &snapshot["status"];
    let Ok(view) = serde_json::from_value::<AuthoringView>(status["authoring"].clone()) else {
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
        json!({"project":status["project"],"selection":selection,"authoring":view,"allow_world_effect":input("edit-kiln-world").checked()}).to_string();
    let task: HtmlTextAreaElement = element("edit-kiln-request").unchecked_into();
    if task.value().trim().is_empty() {
        return;
    }
    let run = async {
        if js_sys::Reflect::get(&window(), &"kilnVerseContext".into())?.is_function() {
            kiln_call("kilnVerseContext", Some(&context))?;
        } else {
            let p = kiln_call("loadVerseKiln", Some(&context))?;
            JsFuture::from(Promise::resolve(&p)).await?;
        }
        kiln_call("kilnAgent", Some(&task.value()))?;
        Ok::<(), JsValue>(())
    }
    .await;
    if let Err(error) = run {
        element("edit-kiln-status").set_text_content(Some(
            error
                .as_string()
                .as_deref()
                .unwrap_or("Kiln could not start. Your manual tools are still available."),
        ));
    }
}
fn queue_change(rc: &Rc<RefCell<State>>, mut action: Action) {
    let mut s = rc.borrow_mut();
    if s.busy {
        return;
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
    let demo = mount_demo()?;
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
        spawn_local(ask_kiln(weak.clone()));
    })?;
    listen(&mut listeners, "edit-kiln-stop", "click", move |_| {
        let _ = kiln_call("kilnStop", None);
    })?;
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "edit-kiln-review", "click", move |_| {
        let Some(rc) = weak.upgrade() else { return };
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
