//! Keep the composer mounted while HTMX replaces transcript fragments.

use js_sys::{Function, Object, Reflect};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
};
use wasm_bindgen::{JsCast, closure::Closure, prelude::*};
use web_sys::{
    CustomEvent, Document, Element, Event, EventTarget, FocusOptions, HtmlFormElement,
    HtmlInputElement, HtmlTextAreaElement, KeyboardEvent, Window, XmlHttpRequest,
};
use zeroize::{Zeroize, Zeroizing};

const MAX_CHATS: usize = 256;
const MAX_DRAFT_BYTES: usize = 64 * 1024;
const BOTTOM_MARGIN: i32 = 80;
type Listener = (EventTarget, &'static str, bool, Closure<dyn FnMut(Event)>);
type Frame = Closure<dyn FnMut(f64)>;

thread_local! {
    static ACTIVE: RefCell<Option<Rc<Runtime>>> = const { RefCell::new(None) };
}

#[derive(Clone)]
struct Draft {
    text: Zeroizing<String>,
    start: u32,
    end: u32,
    direction: String,
    input_top: i32,
    thread_top: i32,
    following: bool,
}

impl Default for Draft {
    fn default() -> Self {
        Self {
            text: Zeroizing::new(String::new()),
            start: 0,
            end: 0,
            direction: "none".into(),
            input_top: 0,
            thread_top: 0,
            following: true,
        }
    }
}

struct Submission {
    xhr: XmlHttpRequest,
    chat: String,
    text: Zeroizing<String>,
}

struct Runtime {
    window: Window,
    document: Document,
    form: HtmlFormElement,
    input: HtmlTextAreaElement,
    private: Option<Element>,
    selected: RefCell<String>,
    drafts: RefCell<BTreeMap<String, Draft>>,
    submissions: RefCell<Vec<Submission>>,
    response_chat: RefCell<Option<String>>,
    presentation_deferred: RefCell<Option<String>>,
    presentation_request: RefCell<Option<XmlHttpRequest>>,
    presentation_loading: Cell<bool>,
    listeners: RefCell<Vec<Listener>>,
    frame: RefCell<Option<Frame>>,
    frame_id: Cell<Option<i32>>,
    composing: Cell<bool>,
    swapping: Cell<bool>,
    retired: Cell<bool>,
    access_retired: Cell<bool>,
}

fn field(value: &JsValue, name: &str) -> JsValue {
    Reflect::get(value, &JsValue::from_str(name)).unwrap_or(JsValue::UNDEFINED)
}

fn detail(event: &Event) -> JsValue {
    event
        .dyn_ref::<CustomEvent>()
        .map(CustomEvent::detail)
        .unwrap_or(JsValue::UNDEFINED)
}

/// Whether the response is a refused send (its reason goes in the
/// composer; the chat stays).
fn refused_response(event: &Event) -> bool {
    field(&detail(event), "xhr")
        .dyn_ref::<XmlHttpRequest>()
        .and_then(|xhr| {
            xhr.get_response_header("x-openagents-refused")
                .ok()
                .flatten()
        })
        .is_some()
}

fn missing() -> JsValue {
    JsValue::from_str("The chat composer could not start.")
}

/// Start local interaction without granting host or execution authority.
#[wasm_bindgen]
pub fn start() -> Result<(), JsValue> {
    // A boosted navigation (hx-boost) replaces the page's body: the runtime
    // bound to the old composer is dropped, with its listeners, and the new
    // page's composer gets its own. A runtime whose composer is still on the
    // page stays.
    let current = ACTIVE.with(|active| {
        active
            .borrow()
            .as_ref()
            .map(|runtime| runtime.form.is_connected())
    });
    match current {
        Some(true) => return Ok(()),
        Some(false) => {
            let stale = ACTIVE.with(|active| active.borrow_mut().take());
            drop(stale);
        }
        None => {}
    }
    let window = web_sys::window().ok_or_else(missing)?;
    let document = window.document().ok_or_else(missing)?;
    let Some(form) = document.get_element_by_id("chat-form") else {
        return Ok(());
    };
    let form = form.dyn_into::<HtmlFormElement>()?;
    let input = document
        .get_element_by_id("chat-input")
        .ok_or_else(missing)?
        .dyn_into::<HtmlTextAreaElement>()?;
    let htmx = field(window.as_ref(), "htmx");
    let config = field(&htmx, "config");
    if config.is_object() {
        for (name, value) in [
            ("allowEval", false),
            ("allowScriptTags", false),
            ("selfRequestsOnly", true),
        ] {
            Reflect::set(
                &config,
                &JsValue::from_str(name),
                &JsValue::from_bool(value),
            )?;
        }
    }
    let private = form.closest("#cloud-private")?;
    let runtime = Rc::new(Runtime {
        window,
        document,
        form,
        input,
        private,
        selected: RefCell::new(String::new()),
        drafts: RefCell::new(BTreeMap::new()),
        submissions: RefCell::new(Vec::new()),
        response_chat: RefCell::new(None),
        presentation_deferred: RefCell::new(None),
        presentation_request: RefCell::new(None),
        presentation_loading: Cell::new(false),
        listeners: RefCell::new(Vec::new()),
        frame: RefCell::new(None),
        frame_id: Cell::new(None),
        composing: Cell::new(false),
        swapping: Cell::new(false),
        retired: Cell::new(false),
        access_retired: Cell::new(false),
    });
    *runtime.selected.borrow_mut() = runtime.selected_key();
    runtime.sync_route();
    if let Some(thread) = runtime.thread() {
        thread.set_scroll_top(
            if thread.get_attribute("data-chat-scroll").as_deref() == Some("start") {
                0
            } else {
                thread.scroll_height()
            },
        );
    }
    runtime.save();
    for name in [
        "keydown",
        "input",
        "select",
        "compositionstart",
        "compositionend",
    ] {
        runtime.listen(runtime.input.clone().into(), name, false)?;
    }
    for name in [
        "click",
        "htmx:beforeRequest",
        "htmx:afterRequest",
        "htmx:beforeSwap",
        "htmx:oobBeforeSwap",
        "htmx:afterSwap",
        "htmx:oobAfterSwap",
        "htmx:afterSettle",
        "htmx:sseBeforeMessage",
        "htmx:sseMessage",
        "openagents-cloud-retired",
        "visibilitychange",
        "selectionchange",
    ] {
        runtime.listen(runtime.document.clone().into(), name, false)?;
    }
    runtime.listen(runtime.document.clone().into(), "scroll", true)?;
    runtime.listen(runtime.form.clone().into(), "submit", true)?;
    runtime.listen(runtime.window.clone().into(), "pagehide", false)?;
    runtime.listen(runtime.window.clone().into(), "pageshow", false)?;
    ACTIVE.with(|active| *active.borrow_mut() = Some(runtime));
    Ok(())
}

impl Runtime {
    fn usable(&self) -> bool {
        !self.retired.get()
            && self.form.is_connected()
            && self.private.as_ref().is_none_or(|private| {
                !self.document.hidden() && private.has_attribute("data-cloud-privacy-ready")
            })
    }

    fn selected_key(&self) -> String {
        for (id, prefix) in [("demo-selected", "demo"), ("chat-selected", "chat")] {
            if let Some(selected) = self.document.get_element_by_id(id)
                && let Some(selected) = selected.dyn_ref::<HtmlInputElement>()
            {
                return format!("{prefix}:{}", selected.value());
            }
        }
        self.form
            .get_attribute("data-chat-id")
            .unwrap_or_else(|| self.form.action())
    }

    fn send_button(&self) -> Option<Element> {
        self.form
            .query_selector("button[type=submit]")
            .ok()
            .flatten()
    }

    /// Whether a send is in flight or its button is off.
    fn sending(&self) -> bool {
        !self.submissions.borrow().is_empty()
            || self
                .send_button()
                .is_some_and(|button| button.has_attribute("disabled"))
    }

    fn thread(&self) -> Option<Element> {
        self.document.get_element_by_id("chat-thread")
    }

    fn real_chat(&self) -> Option<String> {
        let selected = self.document.get_element_by_id("chat-selected")?;
        let id = selected.dyn_ref::<HtmlInputElement>()?.value();
        (id.len() == 36
            && id.bytes().enumerate().all(|(index, byte)| {
                if matches!(index, 8 | 13 | 18 | 23) {
                    byte == b'-'
                } else {
                    byte.is_ascii_hexdigit()
                }
            }))
        .then_some(id)
    }

    fn sync_route(&self) {
        if let Some(id) = self.real_chat() {
            let path = format!("/chat/{id}");
            self.form.set_action(&path);
            let _ = self.form.set_attribute("hx-post", &path);
        }
    }

    fn current_stream(&self, event: &Event) -> bool {
        let selected = self.selected_key();
        let Some((kind, id)) = selected.split_once(':') else {
            return true;
        };
        if !matches!(kind, "chat" | "demo") {
            return true;
        }
        let data = detail(event);
        let message_id = field(&data, "lastEventId")
            .as_string()
            .or_else(|| field(&field(&data, "event"), "lastEventId").as_string());
        message_id.is_some_and(|message_id| {
            message_id
                .split_once(':')
                .is_some_and(|(chat, _)| chat == id)
        })
    }

    fn response_chat(&self, event: &Event) -> Option<String> {
        let xhr = field(&detail(event), "xhr");
        self.submissions
            .borrow()
            .iter()
            .find(|submission| Object::is(submission.xhr.as_ref(), &xhr))
            .map(|submission| submission.chat.clone())
            .or_else(|| {
                let path = field(&field(&detail(event), "requestConfig"), "path").as_string()?;
                let id = path
                    .split('?')
                    .next()?
                    .strip_prefix("/chat/")?
                    .strip_suffix("/transcript")?;
                Some(format!("chat:{id}"))
            })
    }

    fn latest_window(&self) -> bool {
        self.document
            .get_element_by_id("chat-history-window")
            .and_then(|window| {
                window
                    .dyn_ref::<HtmlInputElement>()
                    .map(HtmlInputElement::value)
            })
            .is_none_or(|window| window == "latest")
    }

    fn selecting_transcript(&self) -> bool {
        let Some(thread) = self.thread() else {
            return false;
        };
        let Some(selection) = self.window.get_selection().ok().flatten() else {
            return false;
        };
        !selection.is_collapsed()
            && (0..selection.range_count()).any(|index| {
                selection
                    .get_range_at(index)
                    .ok()
                    .is_some_and(|range| range.intersects_node(thread.as_ref()).unwrap_or(false))
            })
    }

    fn transcript_refresh(&self, name: &str, event: &Event) -> bool {
        if self.real_chat().is_none() {
            return false;
        }
        if name == "htmx:sseBeforeMessage" {
            return true;
        }
        field(&detail(event), "target")
            .dyn_into::<Element>()
            .ok()
            .or_else(|| {
                event
                    .target()
                    .and_then(|target| target.dyn_into::<Element>().ok())
            })
            .is_some_and(|target| target.closest("#chat-transcript").ok().flatten().is_some())
    }

    fn presentation_response(&self, event: &Event) -> bool {
        let xhr = field(&detail(event), "xhr");
        self.presentation_request
            .borrow()
            .as_ref()
            .is_some_and(|pending| Object::is(pending.as_ref(), &xhr))
    }

    fn fetch_deferred(&self) {
        if !self.usable()
            || self.presentation_loading.get()
            || !self.latest_window()
            || self.selecting_transcript()
        {
            return;
        }
        let Some(id) = self.real_chat() else {
            return;
        };
        let selected = self.selected_key();
        if self.presentation_deferred.borrow().as_deref() != Some(selected.as_str()) {
            return;
        }
        let htmx = field(self.window.as_ref(), "htmx");
        let Ok(ajax) = field(&htmx, "ajax").dyn_into::<Function>() else {
            return;
        };
        let Some(transcript) = self.document.get_element_by_id("chat-transcript") else {
            return;
        };
        let options = Object::new();
        for (name, value) in [
            ("target", JsValue::from_str("#chat-transcript")),
            ("swap", JsValue::from_str("innerHTML")),
            ("source", transcript.into()),
        ] {
            if Reflect::set(&options, &JsValue::from_str(name), &value).is_err() {
                return;
            }
        }
        self.presentation_deferred.borrow_mut().take();
        self.presentation_loading.set(true);
        let options: JsValue = options.into();
        let result: Result<JsValue, JsValue> = ajax.call3(
            &htmx,
            &JsValue::from_str("GET"),
            &JsValue::from_str(&format!("/chat/{id}/transcript")),
            &options,
        );
        if result.is_err() {
            self.presentation_loading.set(false);
            *self.presentation_deferred.borrow_mut() = Some(self.selected_key());
        }
    }

    fn save(&self) {
        if self.retired.get() {
            return;
        }
        let key = self.selected.borrow().clone();
        let mut drafts = self.drafts.borrow_mut();
        if !drafts.contains_key(&key)
            && drafts.len() >= MAX_CHATS
            && let Some(oldest) = drafts.keys().find(|saved| *saved != &key).cloned()
        {
            drafts.remove(&oldest);
        }
        let saved = drafts.entry(key).or_default();
        let mut text = self.input.value();
        if text.len() > MAX_DRAFT_BYTES {
            text.zeroize();
            return;
        }
        saved.text = Zeroizing::new(text);
        saved.start = self.input.selection_start().ok().flatten().unwrap_or(0);
        saved.end = self
            .input
            .selection_end()
            .ok()
            .flatten()
            .unwrap_or(saved.start);
        saved.direction = self
            .input
            .selection_direction()
            .ok()
            .flatten()
            .unwrap_or_else(|| "none".into());
        saved.input_top = self.input.scroll_top();
        if !self.swapping.get()
            && let Some(thread) = self.thread()
        {
            saved.thread_top = thread.scroll_top();
            saved.following = thread.scroll_height() - thread.client_height() - thread.scroll_top()
                <= BOTTOM_MARGIN;
        }
    }

    fn listen(
        self: &Rc<Self>,
        target: EventTarget,
        name: &'static str,
        capture: bool,
    ) -> Result<(), JsValue> {
        let weak = Rc::downgrade(self);
        let callback = Closure::wrap(Box::new(move |event: Event| {
            if let Some(runtime) = weak.upgrade() {
                runtime.event(name, &event);
            }
        }) as Box<dyn FnMut(Event)>);
        target.add_event_listener_with_callback_and_bool(
            name,
            callback.as_ref().unchecked_ref(),
            capture,
        )?;
        self.listeners
            .borrow_mut()
            .push((target, name, capture, callback));
        Ok(())
    }

    fn event(self: &Rc<Self>, name: &str, event: &Event) {
        if name == "htmx:sseBeforeMessage" {
            if !self.current_stream(event) {
                event.prevent_default();
                return;
            }
            // The SSE extension supplies the raw MessageEvent as detail.
            if field(&detail(event), "type").as_string().as_deref() == Some("retired") {
                self.retire_chat();
                event.prevent_default();
                return;
            }
        }
        match name {
            "pagehide" | "openagents-cloud-retired" => {
                self.retire();
                return;
            }
            "visibilitychange" if self.private.is_some() && self.document.hidden() => {
                self.retire();
                return;
            }
            "pageshow" if self.private.is_none() && !self.access_retired.get() => {
                self.retired.set(false);
                self.swapping.set(false);
                *self.selected.borrow_mut() = self.selected_key();
                self.sync_route();
                self.save();
                return;
            }
            _ => {}
        }
        if !self.usable() {
            if matches!(
                name,
                "submit"
                    | "htmx:beforeRequest"
                    | "htmx:beforeSwap"
                    | "htmx:oobBeforeSwap"
                    | "htmx:sseBeforeMessage"
            ) {
                event.prevent_default();
            }
            return;
        }
        match name {
            "submit" => {
                self.sync_route();
                if self.selected_key() != *self.selected.borrow() {
                    self.reconcile();
                }
            }
            "compositionstart" => self.composing.set(true),
            "compositionend" => {
                self.composing.set(false);
                self.save();
            }
            "keydown" => {
                if let Some(key) = event.dyn_ref::<KeyboardEvent>()
                    && (key.ctrl_key() || key.meta_key())
                    && key.key().eq_ignore_ascii_case("p")
                    && let Some(plugins) = self.document.get_element_by_id("demo-plugins")
                {
                    event.prevent_default();
                    if plugins.has_attribute("open") {
                        let _ = plugins.remove_attribute("open");
                    } else {
                        let _ = plugins.set_attribute("open", "");
                    }
                    return;
                }
                if let Some(key) = event.dyn_ref::<KeyboardEvent>()
                    && key.key() == "Enter"
                    && !key.shift_key()
                    && !key.is_composing()
                    && key.key_code() != 229
                    && !self.composing.get()
                {
                    event.prevent_default();
                    // Enter does nothing while a send is in flight or an
                    // answer is being written (the send button is off).
                    if !self.input.value().trim().is_empty() && !self.sending() {
                        let _ = self.form.request_submit();
                    }
                }
            }
            "input" | "select" => self.save(),
            "selectionchange" => self.fetch_deferred(),
            "scroll" if !self.swapping.get() => {
                if event
                    .target()
                    .and_then(|target| target.dyn_into::<Element>().ok())
                    .is_some_and(|target| {
                        target.id() == "chat-thread" || target.id() == "chat-input"
                    })
                {
                    self.save();
                }
            }
            "click" => self.click(event),
            "htmx:beforeRequest" => self.before_request(event),
            "htmx:afterRequest" => self.after_request(event),
            "htmx:beforeSwap" | "htmx:oobBeforeSwap" | "htmx:sseBeforeMessage" => {
                if name == "htmx:beforeSwap"
                    && self.response_chat(event).as_deref() == Some(self.selected_key().as_str())
                    && field(&detail(event), "xhr")
                        .dyn_ref::<XmlHttpRequest>()
                        .and_then(|xhr| xhr.status().ok())
                        .is_some_and(|status| matches!(status, 401 | 403 | 404))
                    && !refused_response(event)
                {
                    self.retire_chat();
                    event.prevent_default();
                    return;
                }
                if name == "htmx:beforeSwap" {
                    *self.response_chat.borrow_mut() = self.response_chat(event);
                }
                if name != "htmx:sseBeforeMessage"
                    && self
                        .response_chat(event)
                        .or_else(|| self.response_chat.borrow().clone())
                        .is_some_and(|chat| chat != self.selected_key())
                {
                    event.prevent_default();
                    return;
                }
                if name == "htmx:sseBeforeMessage" && !self.current_stream(event) {
                    event.prevent_default();
                    return;
                }
                if self.transcript_refresh(name, event) {
                    if self.presentation_response(event) && !self.latest_window() {
                        event.prevent_default();
                        return;
                    }
                    if name == "htmx:sseBeforeMessage" && !self.latest_window() {
                        event.prevent_default();
                        return;
                    }
                    if self.selecting_transcript()
                        || (name == "htmx:sseBeforeMessage" && self.presentation_loading.get())
                    {
                        *self.presentation_deferred.borrow_mut() = Some(self.selected_key());
                        event.prevent_default();
                        return;
                    }
                }
                if !self.swapping.get() {
                    self.save();
                    self.swapping.set(true);
                }
                self.queue();
            }
            "htmx:afterSwap" | "htmx:oobAfterSwap" | "htmx:afterSettle" | "htmx:sseMessage" => {
                if self.transcript_refresh(name, event)
                    && !self.presentation_loading.get()
                    && self.latest_window()
                {
                    self.presentation_deferred.borrow_mut().take();
                }
                self.sync_route();
                self.queue()
            }
            _ => {}
        }
    }

    fn click(&self, event: &Event) {
        let Some(target) = event
            .target()
            .and_then(|target| target.dyn_into::<Element>().ok())
        else {
            return;
        };
        if let Ok(Some(control)) = target.closest("[data-chat-history], #demo-history-start, #demo-history-end, #chat-history-start, #chat-history-end") {
            let start = control.get_attribute("data-chat-history").as_deref() == Some("start") || control.id().ends_with("-start");
            if let Some(thread) = self.thread() {
                if !control.has_attribute("hx-get") {
                    event.prevent_default();
                }
                thread.set_scroll_top(if start { 0 } else { thread.scroll_height() });
                self.save();
                if let Some(saved) = self.drafts.borrow_mut().get_mut(&*self.selected.borrow()) {
                    saved.thread_top = thread.scroll_top();
                    saved.following = !start;
                }
            }
            return;
        }
        if target.closest("#chat-card").ok().flatten().is_some()
            && target
                .closest(
                    "button, textarea, input, select, a, label, [role=button], [contenteditable]",
                )
                .ok()
                .flatten()
                .is_none()
        {
            let options = FocusOptions::new();
            options.set_prevent_scroll(true);
            let _ = self.input.focus_with_options(&options);
        }
    }

    fn before_request(&self, event: &Event) {
        let data = detail(event);
        if self.presentation_loading.get()
            && self.presentation_request.borrow().is_none()
            && self.response_chat(event).is_some()
            && let Ok(xhr) = field(&data, "xhr").dyn_into::<XmlHttpRequest>()
        {
            *self.presentation_request.borrow_mut() = Some(xhr);
        }
        let source = field(&data, "elt").dyn_into::<Element>().ok();
        if !source.is_some_and(|source| source.closest("#chat-form").ok().flatten().is_some()) {
            return;
        }
        let verb = field(&field(&data, "requestConfig"), "verb")
            .as_string()
            .unwrap_or_else(|| self.form.method());
        if !verb.eq_ignore_ascii_case("post") {
            return;
        }
        self.save();
        if let Ok(xhr) = field(&data, "xhr").dyn_into::<XmlHttpRequest>() {
            let mut submissions = self.submissions.borrow_mut();
            if submissions.len() < 8 {
                submissions.push(Submission {
                    xhr,
                    chat: self.selected.borrow().clone(),
                    text: Zeroizing::new(self.input.value()),
                });
            }
        }
    }

    fn after_request(&self, event: &Event) {
        if self.retired.get() {
            return;
        }
        let data = detail(event);
        let xhr = field(&data, "xhr");
        let status = xhr
            .dyn_ref::<XmlHttpRequest>()
            .and_then(|xhr| xhr.status().ok())
            .unwrap_or(0);
        // A refused send is answered 200 with this header so HTMX shows the
        // reason in the composer: the text stays in the box.
        let refused = xhr
            .dyn_ref::<XmlHttpRequest>()
            .and_then(|xhr| {
                xhr.get_response_header("x-openagents-refused")
                    .ok()
                    .flatten()
            })
            .is_some();
        let accepted = (200..300).contains(&status) && !refused;
        let retained_read = self.transcript_refresh("htmx:afterRequest", event)
            && self
                .response_chat(event)
                .is_some_and(|chat| chat == self.selected_key());
        self.response_chat.borrow_mut().take();
        let presentation = self
            .presentation_request
            .borrow()
            .as_ref()
            .is_some_and(|pending| Object::is(pending.as_ref(), &xhr));
        if presentation {
            self.presentation_request.borrow_mut().take();
            self.presentation_loading.set(false);
            if accepted {
                self.fetch_deferred();
            }
        }
        if retained_read {
            if accepted {
                if let Some(feedback) = self.document.get_element_by_id("chat-feedback") {
                    feedback.set_text_content(None);
                }
            } else {
                *self.presentation_deferred.borrow_mut() = Some(self.selected_key());
                if let Some(feedback) = self.document.get_element_by_id("chat-feedback") {
                    let message = if status == 0 {
                        "Latest messages could not load. Use Latest to try again.".into()
                    } else {
                        format!(
                            "Latest messages could not load (HTTP {status}). Use Latest to try again."
                        )
                    };
                    feedback.set_text_content(Some(&message));
                }
            }
        }
        let submission = {
            let mut submissions = self.submissions.borrow_mut();
            submissions
                .iter()
                .position(|pending| Object::is(pending.xhr.as_ref(), &xhr))
                .map(|index| submissions.remove(index))
        };
        let Some(submission) = submission else {
            return;
        };
        if !accepted {
            if !refused
                && submission.chat == self.selected_key()
                && let Some(feedback) = self.document.get_element_by_id("chat-feedback")
            {
                let message = if status == 0 {
                    "Your message was not accepted. Try again or reload.".into()
                } else {
                    format!("Your message was not accepted (HTTP {status}). Try again or reload.")
                };
                feedback.set_text_content(Some(&message));
            }
            return;
        }
        if submission.chat == self.selected_key()
            && let Some(feedback) = self.document.get_element_by_id("chat-feedback")
        {
            feedback.set_text_content(None);
        }
        if submission.chat == *self.selected.borrow() {
            if self.input.value() == *submission.text {
                self.input.set_value("");
                let _ = self.input.set_selection_range(0, 0);
                self.save();
            }
        } else if let Some(saved) = self.drafts.borrow_mut().get_mut(&submission.chat)
            && *saved.text == *submission.text
        {
            saved.text.zeroize();
            saved.start = 0;
            saved.end = 0;
        }
    }

    fn queue(self: &Rc<Self>) {
        if self.frame_id.get().is_some() {
            return;
        }
        let weak = Rc::downgrade(self);
        let callback = Closure::wrap(Box::new(move |_: f64| {
            if let Some(runtime) = weak.upgrade() {
                runtime.frame_id.set(None);
                runtime.reconcile();
            }
        }) as Box<dyn FnMut(f64)>);
        if let Ok(id) = self
            .window
            .request_animation_frame(callback.as_ref().unchecked_ref())
        {
            *self.frame.borrow_mut() = Some(callback);
            self.frame_id.set(Some(id));
        }
    }

    fn reconcile(&self) {
        if !self.usable() {
            return;
        }
        let next = self.selected_key();
        let changed = next != *self.selected.borrow();
        let saved = self
            .drafts
            .borrow()
            .get(&next)
            .cloned()
            .unwrap_or_else(|| Draft {
                following: self.thread().is_none_or(|thread| {
                    thread.get_attribute("data-chat-scroll").as_deref() != Some("start")
                }),
                ..Draft::default()
            });
        if changed {
            self.presentation_deferred.borrow_mut().take();
            *self.selected.borrow_mut() = next;
            let presentation = self.presentation_request.borrow_mut().take();
            self.presentation_loading.set(false);
            if let Some(request) = presentation {
                let _ = request.abort();
            }
            self.composing.set(false);
            self.input.set_value(&saved.text);
            let _ = self.input.set_selection_range_with_direction(
                saved.start,
                saved.end,
                &saved.direction,
            );
            self.input.set_scroll_top(saved.input_top);
        }
        if !self.selecting_transcript()
            && let Some(thread) = self.thread()
        {
            thread.set_scroll_top(if saved.following {
                thread.scroll_height()
            } else {
                saved.thread_top
            });
        }
        self.swapping.set(false);
        self.save();
    }

    fn retire(&self) {
        if self.retired.replace(true) {
            return;
        }
        self.drafts.borrow_mut().clear();
        self.response_chat.borrow_mut().take();
        self.presentation_deferred.borrow_mut().take();
        let presentation = self.presentation_request.borrow_mut().take();
        if let Some(request) = presentation {
            let _ = request.abort();
        }
        self.presentation_loading.set(false);
        let pending = std::mem::take(&mut *self.submissions.borrow_mut());
        for submission in pending {
            let _ = submission.xhr.abort();
        }
        self.input.set_value("");
        let _ = self.input.set_default_value("");
        if let Some(id) = self.frame_id.take() {
            let _ = self.window.cancel_animation_frame(id);
        }
        self.frame.borrow_mut().take();
        self.composing.set(false);
    }

    /// Stop transport before clearing values that require current server access.
    fn retire_chat(&self) {
        self.access_retired.set(true);
        self.retire();
        let htmx = field(self.window.as_ref(), "htmx");
        let trigger = field(&htmx, "trigger").dyn_into::<Function>().ok();
        for (attribute, event) in [
            ("hx-get", "htmx:abort"),
            ("hx-post", "htmx:abort"),
            ("hx-put", "htmx:abort"),
            ("hx-delete", "htmx:abort"),
            ("hx-patch", "htmx:abort"),
            ("sse-connect", "htmx:beforeCleanupElement"),
            ("data-sse-connect", "htmx:beforeCleanupElement"),
        ] {
            while let Ok(Some(element)) = self.document.query_selector(&format!("[{attribute}]")) {
                if let Some(trigger) = &trigger {
                    let data = Object::new();
                    let _ = Reflect::set(&data, &JsValue::from_str("elt"), element.as_ref());
                    let _ =
                        trigger.call3(&htmx, element.as_ref(), &JsValue::from_str(event), &data);
                }
                // A callback already queued by HTMX cannot reconnect this source.
                if element.remove_attribute(attribute).is_err() {
                    break;
                }
            }
        }
        for id in [
            "composer-controls",
            "composer-panel",
            "chat-ticket",
            "chat-transcript",
        ] {
            if let Some(element) = self.document.get_element_by_id(id) {
                element.set_text_content(None);
            }
        }
        if let Some(state) = self.document.get_element_by_id("composer-state")
            && let Some(state) = state.dyn_ref::<HtmlInputElement>()
        {
            state.set_value("");
            state.set_default_value("");
        }
        self.input.set_disabled(true);
        if let Some(feedback) = self.document.get_element_by_id("chat-feedback") {
            feedback.set_text_content(Some(
                "Access to this conversation is unavailable. Reopen it to check access.",
            ));
        }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        for (target, name, capture, callback) in self.listeners.get_mut().drain(..) {
            let _ = target.remove_event_listener_with_callback_and_bool(
                name,
                callback.as_ref().unchecked_ref(),
                capture,
            );
        }
        if let Some(id) = self.frame_id.take() {
            let _ = self.window.cancel_animation_frame(id);
        }
    }
}
