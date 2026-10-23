//! Player chat UI. Native requests own identity, access, and room writes.
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, VecDeque},
    rc::Rc,
};
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::{JsFuture, spawn_local};
fn window() -> web_sys::Window {
    web_sys::window().unwrap()
}
fn node(id: &str) -> web_sys::Element {
    window().document().unwrap().get_element_by_id(id).unwrap()
}
fn hide(id: &str, yes: bool) {
    if yes {
        node(id).set_attribute("hidden", "").ok();
    } else {
        node(id).remove_attribute("hidden").ok();
    }
}
fn enabled(id: &str, yes: bool) {
    if yes {
        node(id).remove_attribute("disabled").ok();
    } else {
        node(id).set_attribute("disabled", "").ok();
    }
}
fn status(text: &str) {
    if node("visit-status").text_content().as_deref() != Some(text) {
        node("visit-status").set_text_content(Some(text));
    }
}
#[derive(Default)]
struct State {
    id: String,
    account: Option<String>,
    csrf: String,
    busy: bool,
    generation: u64,
    joined: bool,
    shown: bool,
    muted: bool,
    blocked: BTreeSet<String>,
    pending: Option<(String, String)>,
    ended: bool,
    anonymous: bool,
    queue: VecDeque<Intent>,
    rows: BTreeMap<String, (String, web_sys::Element)>,
}
fn storage_key(s: &State) -> String {
    format!(
        "openagents.verse.visit.v1:{}:{}",
        s.id,
        s.account.as_deref().unwrap_or("guest")
    )
}
fn preferences(s: &mut State, read: bool) {
    if let Ok(Some(storage)) = window().local_storage() {
        if read {
            let v = storage
                .get_item(&storage_key(s))
                .ok()
                .flatten()
                .and_then(|v| serde_json::from_str::<Value>(&v).ok())
                .unwrap_or(Value::Null);
            s.shown = v["shown"] == true;
            s.muted = v["muted"] == true;
            s.blocked = v["blocked"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|s| s.as_str().map(str::to_owned))
                .collect();
        } else {
            storage
                .set_item(
                    &storage_key(s),
                    &json!({"shown":s.shown,"muted":s.muted,"blocked":s.blocked}).to_string(),
                )
                .ok();
        }
    }
    hide("visit-unblock", s.blocked.is_empty());
    hide("visit-panel", !s.shown);
    node("visit-toggle").set_text_content(Some(if s.shown { "Hide chat" } else { "Show chat" }));
    node("visit-mute").set_text_content(Some(if s.muted { "Unmute chat" } else { "Mute chat" }));
}
fn random() -> String {
    let mut bytes = [0; 16];
    window()
        .crypto()
        .unwrap()
        .get_random_values_with_u8_array(&mut bytes)
        .unwrap();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
async fn request(path: &str, body: Option<Value>) -> Result<Value, (u16, String)> {
    let init = web_sys::RequestInit::new();
    init.set_credentials(web_sys::RequestCredentials::SameOrigin);
    if let Some(body) = body {
        init.set_method("POST");
        init.set_body(&JsValue::from_str(&body.to_string()));
    }
    let req = web_sys::Request::new_with_str_and_init(path, &init)
        .map_err(|_| (0, "Chat could not connect. Try again.".into()))?;
    req.headers().set("content-type", "application/json").ok();
    let response: web_sys::Response = JsFuture::from(window().fetch_with_request(&req))
        .await
        .map_err(|_| (0, "Chat could not connect. Try again.".into()))?
        .unchecked_into();
    let text = JsFuture::from(
        response
            .text()
            .map_err(|_| (0, "Chat could not connect. Try again.".into()))?,
    )
    .await
    .map_err(|_| (0, "Chat could not connect. Try again.".into()))?
    .as_string()
    .unwrap_or_default();
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if response.ok() {
        Ok(v)
    } else {
        Err((
            response.status(),
            v["message"]
                .as_str()
                .or(v["error"]["message"].as_str())
                .unwrap_or("This visit could not connect. Try again.")
                .into(),
        ))
    }
}
fn render(s: &mut State, v: &Value) {
    s.joined = v["joined"] == true;
    hide("visit-login", v["signed_in"] == true);
    hide("visit-join", s.joined || v["signed_in"] != true);
    hide("visit-leave", !s.joined);
    hide("visit-revoke", v["host"] != true);
    hide(
        "visit-reports",
        v["host"] != true || v["reports"].as_array().is_none_or(|r| r.is_empty()),
    );
    node("visit-reports").set_text_content(Some(&format!(
        "{} message reports. Review reported messages before ending this invitation.",
        v["reports"].as_array().map_or(0, Vec::len)
    )));
    enabled("visit-text", s.joined);
    enabled("visit-send", s.joined && !s.busy);
    let people = v["participants"].as_array().map_or(0, Vec::len);
    node("visit-presence").set_text_content(Some(&format!("{people} here")));
    status(if s.joined {
        if s.muted {
            "Chat muted for you."
        } else {
            "Connected."
        }
    } else if v["signed_in"] == true {
        "Join to see this visit's chat."
    } else {
        "Sign in to join this visit's chat."
    });
    if s.muted && s.joined {
        return;
    }
    let list = node("visit-messages");
    if !s.joined {
        clear_messages(s);
        return;
    }
    let mut wanted = BTreeSet::new();
    let mut before: Option<web_sys::Element> = None;
    let empty = Vec::new();
    for message in v["messages"].as_array().unwrap_or(&empty).iter().rev() {
        let participant = message["participant"].as_str().unwrap_or_default();
        if s.blocked.contains(participant) {
            continue;
        }
        let id = message["id"].as_str().unwrap_or_default().to_owned();
        wanted.insert(id.clone());
        let reported = v["host"] == true
            && v["reports"].as_array().is_some_and(|reports| {
                reports.iter().any(|report| {
                    report
                        .as_str()
                        .is_some_and(|r| r.ends_with(&format!(":{id}")))
                })
            });
        let fingerprint = format!("{}:{reported}", message);
        if let Some((previous, row)) = s.rows.get(&id) {
            if previous == &fingerprint {
                before = Some(row.clone());
                continue;
            }
        }
        let doc = window().document().unwrap();
        let focus = doc
            .active_element()
            .filter(|e| e.get_attribute("data-action").is_some())
            .and_then(|e| {
                let row = e.closest("[data-message-id]").ok().flatten()?;
                (row.get_attribute("data-message-id").as_deref() == Some(id.as_str())).then(|| {
                    (
                        e.get_attribute("data-action").unwrap_or_default(),
                        e.get_attribute("data-target").unwrap_or_default(),
                    )
                })
            });
        let row = doc.create_element("li").unwrap();
        row.set_attribute("data-message-id", &id).ok();
        let label = doc.create_element("strong").unwrap();
        label.set_text_content(message["label"].as_str());
        row.append_child(&label).ok();
        if reported {
            row.set_attribute("data-reported", "true").ok();
            let badge = doc.create_element("strong").unwrap();
            badge.set_text_content(Some(" · Reported"));
            row.append_child(&badge).ok();
        }
        let text = doc.create_element("p").unwrap();
        text.set_text_content(message["text"].as_str());
        row.append_child(&text).ok();
        if v["me"] != message["participant"] {
            for (action, label, target) in [
                ("block", "Hide this person's messages", participant),
                (
                    "report",
                    "Report message",
                    message["id"].as_str().unwrap_or_default(),
                ),
            ] {
                let b = doc.create_element("button").unwrap();
                b.set_text_content(Some(label));
                b.set_attribute("type", "button").ok();
                b.set_attribute("data-action", action).ok();
                b.set_attribute("data-target", target).ok();
                row.append_child(&b).ok();
            }
        }
        if let Some((_, old)) = s.rows.get(&id) {
            list.replace_child(&row, old).ok();
        } else {
            list.insert_before(&row, before.as_ref().map(|e| e.as_ref()))
                .ok();
        }
        if let Some((action, target)) = focus {
            if let Ok(Some(button)) = row.query_selector(&format!(
                "button[data-action='{action}'][data-target='{target}']"
            )) {
                let _ = button.unchecked_into::<web_sys::HtmlElement>().focus();
            }
        }
        before = Some(row.clone());
        s.rows.insert(id, (fingerprint, row));
    }
    s.rows.retain(|id, (_, row)| {
        if wanted.contains(id) {
            true
        } else {
            row.remove();
            false
        }
    });
}
fn clear_messages(s: &mut State) {
    s.rows.clear();
    node("visit-messages").set_inner_html("");
}
#[derive(Clone)]
struct Intent {
    operation: String,
    target: String,
    text: String,
    command_id: String,
    owner: Option<String>,
}
fn end_player(text: &str) {
    for id in ["engine", "world-retry", "world-fullscreen"] {
        if let Some(element) = window().document().and_then(|d| d.get_element_by_id(id)) {
            element.set_attribute("hidden", "").ok();
        }
    }
    if let Ok(player) = js_sys::Reflect::get(&window(), &JsValue::from_str("versePlayer")) {
        if let Ok(close) = js_sys::Reflect::get(&player, &JsValue::from_str("close")) {
            if let Some(close) = close.dyn_ref::<js_sys::Function>() {
                let _ = close.call0(&player);
            }
        }
    }
    node("world-notice").set_text_content(Some(text));
    hide("world-notice", false);
}
fn current(rc: &Rc<RefCell<State>>, generation: u64) -> bool {
    let s = rc.borrow();
    !s.ended && s.generation == generation
}
fn run(
    rc: Rc<RefCell<State>>,
    intent: Option<Intent>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()>>> {
    Box::pin(async move {
        let (id, generation) = {
            let mut s = rc.borrow_mut();
            if s.ended {
                return;
            }
            if s.busy {
                if let Some(intent) = intent {
                    if s.queue.len() < 8 {
                        s.queue.push_back(intent);
                    } else {
                        status("Let your current chat action finish first.");
                    }
                }
                return;
            }
            s.busy = true;
            s.generation += 1;
            (s.id.clone(), s.generation)
        };
        let result=async {
            let session=request("/v1/verse/studio/session",None).await?;
            if !current(&rc,generation){return Ok(Value::Null);}
            let account=session["account"].as_str().map(str::to_owned);
            {
                let mut s=rc.borrow_mut();
                if s.account!=account {
                    s.account=account.clone();s.pending=None;s.joined=false;s.queue.clear();
                    node("visit-text").unchecked_into::<web_sys::HtmlTextAreaElement>().set_value("");
                    clear_messages(&mut s);preferences(&mut s,true);
                }
                s.csrf=session["csrf"].as_str().unwrap_or_default().into();
            }
            let command=if let Some(intent)=&intent {
                if account.is_none() {return Err((401,"Sign in to chat.".into()));}
                if intent.owner!=account {return Err((401,"Your sign-in changed. Try the action again.".into()));}
                let s=rc.borrow();Some(json!({"csrf":s.csrf,"command_id":intent.command_id,"operation":intent.operation,"text":intent.text,"target":intent.target}))
            }else{None};
            if !current(&rc,generation){return Ok(Value::Null);}
            let mut value=request(&format!("/v1/verse/studio/visits/{id}"),command).await?;
            if !current(&rc,generation){return Ok(Value::Null);}
            if intent.is_none()&&value["joined"]==true {
                let body={let s=rc.borrow();json!({"csrf":s.csrf,"command_id":random(),"operation":"heartbeat"})};
                value=request(&format!("/v1/verse/studio/visits/{id}"),Some(body)).await?;
                if !current(&rc,generation){return Ok(Value::Null);}
            }
            let checked=request("/v1/verse/studio/session",None).await?;
            if checked["account"].as_str()!=account.as_deref(){return Err((401,"Your sign-in changed. Reload this visit.".into()));}
            Ok(value)
        }.await;
        if !current(&rc, generation) {
            return;
        }
        let mut s = rc.borrow_mut();
        s.busy = false;
        match result {
            Ok(v) if !v.is_null() => {
                if let Some(intent) = &intent {
                    if intent.operation == "send" {
                        let input =
                            node("visit-text").unchecked_into::<web_sys::HtmlTextAreaElement>();
                        if input.value() == intent.text {
                            input.set_value("");
                        }
                        if s.pending
                            .as_ref()
                            .is_some_and(|(id, _)| *id == intent.command_id)
                        {
                            s.pending = None;
                        }
                    }
                }
                if v["revoked"] == true {
                    s.ended = true;
                    end_player("This invitation has ended.");
                    status("This invitation has ended.");
                    clear_messages(&mut s);
                } else if s.queue.is_empty() {
                    render(&mut s, &v);
                    if intent.as_ref().is_some_and(|i| i.operation == "report") {
                        status("Message reported to the host.");
                    }
                }
            }
            Err((code, text)) => {
                status(&text);
                if [401, 403, 404, 410].contains(&code) {
                    s.joined = false;
                    s.pending = None;
                    s.queue.clear();
                    enabled("visit-text", false);
                    enabled("visit-send", false);
                    clear_messages(&mut s);
                    hide("visit-login", code != 401);
                    hide("visit-join", true);
                    hide("visit-leave", true);
                    if code != 401 || !s.anonymous {
                        s.ended = true;
                        end_player(&text);
                    }
                }
            }
            _ => {}
        }
        let next = s.queue.pop_front();
        drop(s);
        if let Some(intent) = next {
            spawn_local(run(rc.clone(), Some(intent)));
        }
    })
}
#[wasm_bindgen]
pub struct BrowserVisit {
    state: Rc<RefCell<State>>,
    timer: i32,
    tick: Closure<dyn FnMut()>,
    handlers: Vec<(web_sys::Element, Closure<dyn FnMut(web_sys::Event)>)>,
}
#[wasm_bindgen]
impl BrowserVisit {
    pub fn snapshot(&self) -> String {
        let s = self.state.borrow();
        json!({"joined":s.joined,"shown":s.shown,"muted":s.muted,"busy":s.busy,"ended":s.ended})
            .to_string()
    }
}
impl Drop for BrowserVisit {
    fn drop(&mut self) {
        {
            let mut s = self.state.borrow_mut();
            s.ended = true;
            s.generation += 1;
            s.queue.clear();
        }
        window().clear_interval_with_handle(self.timer);
        for (n, h) in &self.handlers {
            n.remove_event_listener_with_callback("click", h.as_ref().unchecked_ref())
                .ok();
        }
        let _ = &self.tick;
    }
}
#[wasm_bindgen]
pub fn mount_visit() -> Result<BrowserVisit, JsValue> {
    std::panic::set_hook(Box::new(|info| {
        if let Ok(console) = js_sys::Reflect::get(&window(), &JsValue::from_str("console")) {
            if let Ok(error) = js_sys::Reflect::get(&console, &JsValue::from_str("error")) {
                if let Some(error) = error.dyn_ref::<js_sys::Function>() {
                    let _ = error.call1(&console, &JsValue::from_str(&info.to_string()));
                }
            }
        }
    }));
    let id = node("visit")
        .get_attribute("data-id")
        .ok_or(JsValue::NULL)?;
    let anonymous = node("visit").get_attribute("data-anonymous").as_deref() == Some("true");
    let state = Rc::new(RefCell::new(State {
        id,
        anonymous,
        ..State::default()
    }));
    preferences(&mut state.borrow_mut(), true);
    let mut handlers = Vec::new();
    for id in [
        "visit-toggle",
        "visit-mute",
        "visit-join",
        "visit-leave",
        "visit-send",
        "visit-revoke",
        "visit-messages",
        "visit-unblock",
    ] {
        let rc = state.clone();
        let action = id.to_owned();
        let h = Closure::wrap(Box::new(move |event: web_sys::Event| {
            let mut s = rc.borrow_mut();
            if action == "visit-toggle" {
                s.shown = !s.shown;
                preferences(&mut s, false);
                return;
            }
            if action == "visit-unblock" {
                s.blocked.clear();
                preferences(&mut s, false);
                drop(s);
                spawn_local(run(rc.clone(), None));
                return;
            }
            if action == "visit-mute" {
                s.muted = !s.muted;
                preferences(&mut s, false);
                drop(s);
                spawn_local(run(rc.clone(), None));
                return;
            }
            let mut target = String::new();
            let operation = if action == "visit-messages" {
                let Some(el) = event
                    .target()
                    .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                else {
                    return;
                };
                target = el.get_attribute("data-target").unwrap_or_default();
                let a = el.get_attribute("data-action").unwrap_or_default();
                if a == "block" {
                    s.blocked.insert(target);
                    preferences(&mut s, false);
                    drop(s);
                    spawn_local(run(rc.clone(), None));
                    return;
                }
                if a != "report" {
                    return;
                }
                a
            } else {
                action.trim_start_matches("visit-").to_owned()
            };
            if operation=="revoke"&&!window().confirm_with_message("End this invitation for everyone? The published version stays available through its other links.").unwrap_or(false){return;}
            if operation == "send" {
                target = node("visit-text")
                    .unchecked_into::<web_sys::HtmlTextAreaElement>()
                    .value();
            }
            if operation == "leave" {
                s.joined = false;
                clear_messages(&mut s);
                enabled("visit-text", false);
                enabled("visit-send", false);
            }
            let text = if operation == "send" {
                target.clone()
            } else {
                String::new()
            };
            let command_id = if operation == "send" {
                if s.pending.as_ref().is_none_or(|(_, t)| *t != text) {
                    s.pending = Some((random(), text.clone()));
                }
                s.pending.as_ref().unwrap().0.clone()
            } else {
                random()
            };
            let intent = Intent {
                operation,
                target: if text.is_empty() {
                    target
                } else {
                    String::new()
                },
                text,
                command_id,
                owner: s.account.clone(),
            };
            drop(s);
            spawn_local(run(rc.clone(), Some(intent)));
        }) as Box<dyn FnMut(web_sys::Event)>);
        let n = node(id);
        n.add_event_listener_with_callback("click", h.as_ref().unchecked_ref())?;
        handlers.push((n, h));
    }
    let rc = state.clone();
    let tick =
        Closure::wrap(Box::new(move || spawn_local(run(rc.clone(), None))) as Box<dyn FnMut()>);
    let timer = window().set_interval_with_callback_and_timeout_and_arguments_0(
        tick.as_ref().unchecked_ref(),
        5000,
    )?;
    spawn_local(run(state.clone(), None));
    Ok(BrowserVisit {
        state,
        timer,
        tick,
        handlers,
    })
}
