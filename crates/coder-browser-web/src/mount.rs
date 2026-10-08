//! Thin DOM mounting around the admitted Rust workbench and renderer.

use crate::{Composition, gpu::Gpu};
use coder_browser::{
    Direct, Incoming, Relayed, State,
    browser::{self, Socket},
    pairing::{Pairing, Pins},
    workbench::Workbench,
};
use coder_host_wire::TermRequest;
use coder_pty::{
    ext::{Features, Join, MemberState, SessionRead},
    wire::{Attach, Mode, TerminalRef, TerminalResult, Value},
};
use futures_util::future::{AbortHandle, Abortable, Either, select};
use gloo_timers::future::TimeoutFuture;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use verse_gfx::ui::UiBatch;
use wasm_bindgen::{JsCast, closure::Closure, prelude::*};
use web_sys::{
    Element, Event, EventTarget, HtmlCanvasElement, HtmlInputElement, HtmlTextAreaElement,
};
use workbench::{Host, Kind, ResourceRef};
use workbench_session::{Consent, Current, Probe, Resolver, Saved};

thread_local! {
    static ACTIVE: RefCell<Option<Rc<Runtime>>> = const { RefCell::new(None) };
}

struct Listener {
    target: EventTarget,
    name: &'static str,
    callback: Closure<dyn FnMut(Event)>,
}
impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self
            .target
            .remove_event_listener_with_callback(self.name, self.callback.as_ref().unchecked_ref());
    }
}

enum Command {
    List,
    Read(String),
    Attach(TerminalRef, Mode),
    Detach,
    Thread(ResourceRef, Option<u64>),
}

struct Runtime {
    root: Element,
    surface: Element,
    pins: Pins,
    active: Cell<bool>,
    connecting: Cell<bool>,
    connected: Cell<bool>,
    pending: Cell<bool>,
    interactive: Cell<bool>,
    observe: Cell<bool>,
    command: RefCell<Option<Command>>,
    model: RefCell<Option<Workbench>>,
    saved: RefCell<Option<Saved>>,
    thread: RefCell<Option<(ResourceRef, coder_access::thread::ThreadPage)>>,
    terminal: RefCell<Option<TerminalRef>>,
    attachment: RefCell<Option<String>>,
    expires_at: Cell<u64>,
    epoch: Cell<u64>,
    transport_abort: RefCell<Option<AbortHandle>>,
    gpu: RefCell<Option<Gpu>>,
    listeners: RefCell<Vec<Listener>>,
    rows: RefCell<Vec<Listener>>,
    proposals: RefCell<Vec<Listener>>,
    aborts: RefCell<Vec<AbortHandle>>,
    composition: RefCell<Composition>,
    notice: Element,
    input: HtmlTextAreaElement,
    invitation: HtmlInputElement,
    sessions: Element,
    details: Element,
    decisions: Element,
    thread_pane: Element,
    frames: Cell<u64>,
    draw_ms: Cell<f64>,
    drawn_state: RefCell<String>,
}

enum Link {
    Direct(Direct<coder_browser::reach_socket::Socket>),
    Relay(Relayed<Socket>),
}
impl Link {
    fn admission(&self) -> &coder_browser::Admission {
        match self {
            Self::Direct(link) => &link.admission,
            Self::Relay(link) => &link.admission,
        }
    }
    fn take_incoming(&mut self) -> coder_browser::Result<Option<Incoming>> {
        match self {
            Self::Direct(link) => Ok(link.take_incoming()),
            Self::Relay(link) => link.take_incoming(now()),
        }
    }
    async fn request(&mut self, request: TermRequest) -> coder_browser::Result<TerminalResult> {
        match self {
            Self::Direct(link) => link.request(request, now()).await,
            Self::Relay(link) => link.request(request, now()).await,
        }
    }
    async fn next(&mut self) -> coder_browser::Result<Incoming> {
        match self {
            Self::Direct(link) => link.next().await,
            Self::Relay(link) => link.next(now()).await,
        }
    }
}

/// Starts only after the account and exact host standing revealed the private pane.
#[wasm_bindgen]
pub async fn start() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(failure)?;
    let document = window.document().ok_or_else(failure)?;
    let theme = document
        .get_element_by_id("coder-workbench-theme")
        .map_or_else(|| document.create_element("style"), Ok)?;
    theme.set_id("coder-workbench-theme");
    theme.set_text_content(Some(&coder_ui::coder_noir::css_variables()));
    if !theme.is_connected() {
        document
            .document_element()
            .ok_or_else(failure)?
            .append_child(&theme)?;
    }
    let private = document
        .get_element_by_id("cloud-private")
        .ok_or_else(failure)?;
    let root = document
        .get_element_by_id("cloud-workbench")
        .ok_or_else(failure)?;
    for _ in 0..100 {
        if document.hidden() || !root.is_connected() {
            return Err(failure());
        }
        if privacy_ready(&document) && !private.has_attribute("hidden") {
            break;
        }
        TimeoutFuture::new(100).await;
    }
    if !privacy_ready(&document)
        || document.hidden()
        || private.has_attribute("hidden")
        || !private.contains(Some(&root))
    {
        return Err(failure());
    }
    let config = document
        .get_element_by_id("cloud-workbench-config")
        .and_then(|node| node.text_content())
        .ok_or_else(failure)?;
    if config.len() > 16 * 1024 {
        return Err(failure());
    }
    let pins: Pins = serde_json::from_str(&config).map_err(|_| failure())?;
    // Validate all public pins before registering controls or opening a socket.
    let origin = window.location().origin()?;
    let _ = Pairing::new(pins.clone(), &origin, now()).map_err(|_| failure())?;
    let runtime = Runtime::mount(root, pins)?;
    ACTIVE.with(|active| {
        if let Some(old) = active.borrow_mut().replace(runtime.clone()) {
            old.retire();
        }
    });
    runtime.lifecycle()?;
    let (abort, registration) = AbortHandle::new_pair();
    runtime.aborts.borrow_mut().push(abort);
    let gpu = Abortable::new(Gpu::open(&runtime.surface), registration).await;
    if !runtime.visible() {
        runtime.retire();
        return Ok(());
    }
    match gpu {
        Ok(Ok(gpu)) => {
            runtime.wire_canvas(&gpu.canvas)?;
            runtime
                .root
                .set_attribute("data-terminal-renderer", gpu.backend)?;
            *runtime.gpu.borrow_mut() = Some(gpu);
            runtime.notice.set_text_content(Some(
                "Renderer ready. Redeem a current native host invitation to read its sessions.",
            ));
        }
        _ => {
            runtime.invitation.set_disabled(true);
            runtime
                .root
                .set_attribute("data-terminal-renderer", "unavailable")?;
            runtime.notice.set_text_content(Some("The shared terminal renderer is unavailable. WebGPU or WebGL2 is required. No terminal was attached."));
            return Ok(());
        }
    }
    runtime.root.set_attribute("data-workbench-ready", "")?;
    let frame = runtime.clone();
    let (abort, registration) = AbortHandle::new_pair();
    runtime.aborts.borrow_mut().push(abort);
    wasm_bindgen_futures::spawn_local(async move {
        let _ = Abortable::new(frame.frames(), registration).await;
    });
    Ok(())
}

/// Content-free renderer and lifecycle evidence for browser acceptance.
#[wasm_bindgen]
pub fn terminal_receipt() -> String {
    ACTIVE.with(|active| {
        let current = active.borrow();
        let Some(runtime) = current.as_ref() else { return "{\"active\":false}".into(); };
        let frames = runtime.frames.get();
        let model = runtime.model.borrow();
        serde_json::json!({
            "v":"openagents.cloud-terminal-render-receipt.v1",
            "active":runtime.active.get(),"connected":runtime.connected.get(),
            "frames":frames,"draw_ms_mean":if frames>0 {runtime.draw_ms.get()/frames as f64} else {0.0},
            "renderer":runtime.gpu.borrow().as_ref().map(|gpu|gpu.backend),
            "state":model.as_ref().map(|model|format!("{:?}",model.state())),
            "input":model.as_ref().is_some_and(Workbench::can_type),
            "has_terminal":model.is_some(),"browser_storage":false,
            "glyph_fallback":"Missing atlas glyphs use the shared renderer's fallback glyph.",
        }).to_string()
    })
}

impl Runtime {
    fn mount(root: Element, pins: Pins) -> Result<Rc<Self>, JsValue> {
        root.set_text_content(None);
        root.set_class_name("cloud-workbench rn-view");
        root.set_attribute("style", "min-width:0;max-width:100%;overflow-wrap:anywhere")?;
        text(&root, "h2", "Host terminal workbench")?;
        text(
            &root,
            "p",
            "A native host invitation grants host-wide Terminal access. The account workspace is navigation context. The key and grant stay in this page's memory; leaving or hiding the page removes them.",
        )?;
        text(
            &root,
            "p",
            &format!(
                "Host {} · generation {} · native workspace {} · relay {} · route {}",
                pins.host,
                pins.generation,
                pins.workspace,
                pins.relay,
                pins.route.as_deref().unwrap_or("relay")
            ),
        )?;
        let invitation = field(&root, "Native host invitation", true, false, 16 * 1024)?
            .dyn_into::<HtmlInputElement>()?;
        invitation.set_type("password");
        invitation.set_id("cloud-terminal-invitation");
        invitation.set_attribute("autocomplete", "off")?;
        invitation.set_attribute("aria-label", "Native host invitation")?;
        invitation.set_attribute(
            "style",
            "display:block;width:100%;box-sizing:border-box;max-width:100%",
        )?;
        invitation.set_max_length(16384);
        invitation.set_class_name("rn-input");
        let redeem = text(&root, "button", "Redeem invitation")?;
        redeem.set_id("cloud-terminal-pair");
        redeem.set_attribute("type", "button")?;
        let notice = text(&root, "p", "Starting the shared terminal renderer…")?;
        notice.set_id("cloud-terminal-status");
        notice.set_attribute("role", "status")?;
        let surface = shared_surface(&root)?;
        let sessions = element(&root, "div")?;
        sessions.set_id("cloud-terminal-sessions");
        let details = element(&root, "div")?;
        details.set_id("cloud-terminal-members");
        let input = field(
            &root,
            "Terminal text and IME input",
            false,
            true,
            crate::INPUT_MAX,
        )?
        .dyn_into::<HtmlTextAreaElement>()?;
        input.set_id("cloud-terminal-input");
        input.set_attribute("aria-label", "Terminal text and IME input")?;
        input.set_attribute(
            "style",
            "display:block;width:100%;box-sizing:border-box;max-width:100%",
        )?;
        input.set_attribute("autocomplete", "off")?;
        input.set_rows(2);
        input.set_class_name("rn-input");
        input.set_disabled(true);
        let toolbar = element(&root, "div")?;
        toolbar.set_attribute(
            "style",
            "display:flex;flex-wrap:wrap;gap:8px;margin:12px 0;max-width:100%",
        )?;
        let decisions = element(&root, "div")?;
        decisions.set_id("cloud-terminal-proposals");
        let thread_pane = element(&root, "div")?;
        thread_pane.set_id("cloud-workbench-thread");
        text(
            &root,
            "p",
            "Missing atlas glyphs use the shared renderer's fallback glyph. Clipboard reads require an explicit gesture. Detaching leaves the native terminal alive. Reconnect with a fresh invitation and snapshot; pending input is never replayed.",
        )?;
        let runtime = Rc::new(Self {
            root,
            surface,
            pins,
            active: Cell::new(true),
            connecting: Cell::new(false),
            connected: Cell::new(false),
            pending: Cell::new(false),
            interactive: Cell::new(false),
            observe: Cell::new(false),
            command: RefCell::new(None),
            model: RefCell::new(None),
            saved: RefCell::new(None),
            thread: RefCell::new(None),
            terminal: RefCell::new(None),
            attachment: RefCell::new(None),
            expires_at: Cell::new(0),
            epoch: Cell::new(0),
            transport_abort: RefCell::new(None),
            gpu: RefCell::new(None),
            listeners: RefCell::new(vec![]),
            rows: RefCell::new(vec![]),
            proposals: RefCell::new(vec![]),
            aborts: RefCell::new(vec![]),
            composition: RefCell::new(Composition::default()),
            notice,
            input,
            invitation,
            sessions,
            details,
            decisions,
            thread_pane,
            frames: Cell::new(0),
            draw_ms: Cell::new(0.0),
            drawn_state: RefCell::new(String::new()),
        });
        runtime.listen(&redeem, "click", false, |runtime, _| runtime.pair())?;
        runtime.wire_input()?;
        for (label, bytes) in [
            ("Esc", "\u{1b}"),
            ("Tab", "\t"),
            ("Ctrl+C", "\u{3}"),
            ("Enter", "\r"),
        ] {
            let button = text(&toolbar, "button", label)?;
            button.set_attribute("type", "button")?;
            button.set_attribute("data-terminal-input-action", "")?;
            runtime.listen(&button, "click", false, move |runtime, _| {
                runtime.type_text(bytes, false)
            })?;
        }
        for (label, action) in [
            ("Take typing", 0),
            ("Read blocks", 1),
            ("Read proposals", 2),
            ("Copy selection", 3),
            ("Paste", 4),
            ("Detach", 5),
            ("Refresh sessions", 6),
        ] {
            let button = text(&toolbar, "button", label)?;
            button.set_attribute("type", "button")?;
            button.set_attribute("data-terminal-action", &action.to_string())?;
            runtime.listen(&button, "click", false, move |runtime, _| {
                runtime.action(action)
            })?;
        }
        Ok(runtime)
    }

    fn visible(&self) -> bool {
        self.active.get()
            && self.root.is_connected()
            && self.root.owner_document().is_some_and(|document| {
                !document.hidden()
                    && privacy_ready(&document)
                    && document
                        .get_element_by_id("cloud-private")
                        .is_some_and(|private| {
                            !private.has_attribute("hidden") && private.contains(Some(&self.root))
                        })
            })
    }

    fn listen(
        self: &Rc<Self>,
        target: &EventTarget,
        name: &'static str,
        row: bool,
        mut action: impl FnMut(Rc<Self>, Event) + 'static,
    ) -> Result<(), JsValue> {
        let weak = Rc::downgrade(self);
        let callback = Closure::wrap(Box::new(move |event: Event| {
            if let Some(runtime) = weak.upgrade() {
                if runtime.visible() {
                    action(runtime, event);
                }
            }
        }) as Box<dyn FnMut(Event)>);
        target.add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())?;
        let listener = Listener {
            target: target.clone(),
            name,
            callback,
        };
        if row {
            self.rows.borrow_mut().push(listener);
        } else {
            self.listeners.borrow_mut().push(listener);
        }
        Ok(())
    }

    fn lifecycle(self: &Rc<Self>) -> Result<(), JsValue> {
        let window = web_sys::window().ok_or_else(failure)?;
        let document = window.document().ok_or_else(failure)?;
        for (target, name) in [
            (window.clone().into(), "pagehide"),
            (document.clone().into(), "visibilitychange"),
            (document.into(), "openagents-cloud-retired"),
        ] {
            let weak = Rc::downgrade(self);
            let callback = Closure::wrap(Box::new(move |_: Event| {
                if let Some(runtime) = weak.upgrade() {
                    if name != "visibilitychange"
                        || runtime
                            .root
                            .owner_document()
                            .is_none_or(|document| document.hidden())
                    {
                        runtime.retire();
                    }
                }
            }) as Box<dyn FnMut(Event)>);
            let target: EventTarget = target;
            target.add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())?;
            self.listeners.borrow_mut().push(Listener {
                target,
                name,
                callback,
            });
        }
        Ok(())
    }

    fn retire(&self) {
        if !self.active.replace(false) {
            return;
        }
        for abort in self.aborts.borrow_mut().drain(..) {
            abort.abort();
        }
        self.clear_terminal();
        self.gpu.borrow_mut().take();
        self.invitation.set_value("");
        self.invitation.set_default_value("");
        self.input.set_value("");
        let _ = self.input.set_default_value("");
        self.listeners.borrow_mut().clear();
        self.root.set_text_content(None);
        let _ = self.root.remove_attribute("data-workbench-ready");
        let _ = self.root.remove_attribute("data-terminal-renderer");
    }

    fn clear_terminal(&self) {
        self.epoch.set(self.epoch.get().wrapping_add(1));
        if let Some(abort) = self.transport_abort.borrow_mut().take() {
            abort.abort();
        }
        if let Some(mut model) = self.model.borrow_mut().take() {
            model.disconnect();
        }
        if let Some(gpu) = self.gpu.borrow_mut().as_mut() {
            gpu.clear();
        }
        self.saved.borrow_mut().take();
        self.thread.borrow_mut().take();
        self.terminal.borrow_mut().take();
        self.attachment.borrow_mut().take();
        self.command.borrow_mut().take();
        self.connected.set(false);
        self.connecting.set(false);
        self.pending.set(false);
        self.expires_at.set(0);
        self.interactive.set(false);
        self.observe.set(false);
        self.input.set_value("");
        self.input.set_disabled(true);
        self.invitation.set_disabled(false);
        let _ = self.input.set_default_value("");
        *self.composition.borrow_mut() = Composition::default();
        self.rows.borrow_mut().clear();
        self.proposals.borrow_mut().clear();
        self.sessions.set_text_content(None);
        self.details.set_text_content(None);
        self.decisions.set_text_content(None);
        self.thread_pane.set_text_content(None);
        self.drawn_state.borrow_mut().clear();
    }

    fn disconnect(&self) {
        self.clear_terminal();
        self.invitation.set_value("");
        self.invitation.set_default_value("");
        self.invitation.set_disabled(false);
        self.notice.set_text_content(Some("The native connection ended. Read the host's current state using a fresh invitation. The terminal remains on its host; no input was retained or replayed."));
    }

    fn pair(self: Rc<Self>) {
        if self.connecting.get() || self.connected.get() || self.gpu.borrow().is_none() {
            return;
        }
        let mut invitation = SecretText(self.invitation.value());
        self.invitation.set_value("");
        self.invitation.set_default_value("");
        if invitation.0.is_empty() || invitation.0.len() > 16 * 1024 {
            return;
        }
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_default();
        let Ok(pairing) = Pairing::new(self.pins.clone(), &origin, now()) else {
            self.notice.set_text_content(Some(
                "The native host disclosure is invalid. Reload its current standing.",
            ));
            return;
        };
        self.connecting.set(true);
        self.invitation.set_disabled(true);
        self.notice.set_text_content(Some(
            "Redeeming the native invitation and checking the exact host generation…",
        ));
        let epoch = self.epoch.get();
        let (abort, registration) = AbortHandle::new_pair();
        *self.transport_abort.borrow_mut() = Some(abort);
        let runtime = self.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let work = async {
                let enrolled = browser::enroll(pairing, &invitation.0, now()).await;
                erase(&mut invitation.0);
                let enrolled = enrolled.map_err(|_| failure())?;
                if !runtime.visible() || runtime.epoch.get() != epoch {
                    return Err(failure());
                }
                runtime.expires_at.set(enrolled.expires_at());
                let admission = enrolled.into_admission();
                runtime.observe.set(admission.can_observe(now()));
                let link = if let Some(route) = runtime.pins.route.as_ref() {
                    Link::Direct(
                        browser::direct(route, admission, now())
                            .await
                            .map_err(|_| failure())?,
                    )
                } else {
                    let socket = Socket::open(&admission, now())
                        .await
                        .map_err(|_| failure())?;
                    Link::Relay(Relayed::new(admission, socket))
                };
                if !runtime.enrollment_current(epoch) {
                    return Err(failure());
                }
                runtime.connected.set(true);
                runtime.connecting.set(false);
                *runtime.command.borrow_mut() = Some(Command::List);
                runtime.clone().transport(link, epoch).await
            };
            let result = Abortable::new(work, registration).await;
            if runtime.visible() && runtime.epoch.get() == epoch {
                if matches!(result, Ok(Err(_))) {
                    runtime.disconnect();
                }
                runtime.connecting.set(false);
            }
        });
    }

    fn command(&self, command: Command) {
        if self.connected.get() && !self.pending.get() && self.command.borrow().is_none() {
            *self.command.borrow_mut() = Some(command);
        }
    }

    fn enrollment_current(&self, epoch: u64) -> bool {
        self.visible() && self.epoch.get() == epoch && self.expires_at.get() > now()
    }

    async fn transport(self: Rc<Self>, mut link: Link, epoch: u64) -> Result<(), JsValue> {
        let mut source_read = js_sys::Date::now();
        loop {
            if !self.enrollment_current(epoch) || !link.admission().current(now()) {
                return Err(failure());
            }
            if js_sys::Date::now() - source_read >= 5000.0 {
                let original = self.saved.borrow().clone();
                if let Some(original) = original {
                    let session = original.record.session.clone().ok_or_else(failure)?;
                    self.pending.set(true);
                    let result = link
                        .request(TermRequest::SessionRead(SessionRead::new(
                            coder_browser::new_request_id(),
                            session,
                        )))
                        .await
                        .map_err(|_| failure())?;
                    if !self.enrollment_current(epoch) {
                        return Err(failure());
                    }
                    let Some(Value::Session { record }) = result.value else {
                        return Err(failure());
                    };
                    if record != original.record {
                        return Err(failure());
                    }
                    self.pending.set(false);
                }
                let thread = self.thread.borrow().clone();
                if let Some((resource, page)) = thread {
                    self.pending.set(true);
                    let before = page.start + page.turns.len() as u64;
                    let read = link
                        .admission()
                        .prepare_thread(&resource.id, Some(before), now())
                        .map_err(|_| failure())?;
                    let current = browser::read_thread(read, now())
                        .await
                        .map_err(|_| failure())?;
                    if !self.enrollment_current(epoch)
                        || current.thread != page.thread
                        || current.title != page.title
                        || current.start != page.start
                        || current.turns != page.turns
                        || current.total < page.total
                        || current.failure != page.failure
                        || current.busy != page.busy
                        || current.partial != page.partial
                        || current.coder != page.coder
                        || current.outside != page.outside
                    {
                        return Err(failure());
                    }
                    self.pending.set(false);
                }
                self.drain_incoming(&mut link)?;
                source_read = js_sys::Date::now();
            }
            let command = self.command.borrow_mut().take();
            if let Some(command) = command {
                self.pending.set(true);
                match command {
                    Command::List => {
                        let result = link
                            .request(TermRequest::SessionList(coder_pty::ext::SessionList::new(
                                coder_browser::new_request_id(),
                            )))
                            .await
                            .map_err(|_| failure())?;
                        if !self.enrollment_current(epoch) {
                            return Err(failure());
                        }
                        let Some(Value::Sessions { sessions }) = result.value else {
                            return Err(failure());
                        };
                        self.show_sessions(&sessions)?;
                    }
                    Command::Read(session) => {
                        let result = link
                            .request(TermRequest::SessionRead(SessionRead::new(
                                coder_browser::new_request_id(),
                                session.clone(),
                            )))
                            .await
                            .map_err(|_| failure())?;
                        if !self.enrollment_current(epoch) {
                            return Err(failure());
                        }
                        let Some(Value::Session { record }) = result.value else {
                            return Err(failure());
                        };
                        if record.session.as_deref() != Some(&session) {
                            return Err(failure());
                        }
                        let saved = Saved {
                            v: workbench_session::SCHEMA.into(),
                            owner: Host::Paired {
                                key: self.pins.host.clone(),
                            },
                            record,
                        };
                        saved.members().map_err(|_| failure())?;
                        *self.saved.borrow_mut() = Some(saved);
                        self.show_members()?;
                    }
                    Command::Attach(terminal, mode) => {
                        if self.model.borrow().is_some() {
                            return Err(failure());
                        }
                        let features = link.admission().features();
                        if !features.snapshot || !features.typist {
                            return Err(failure());
                        }
                        let mut attach = Attach::new(
                            coder_browser::new_request_id(),
                            terminal.clone(),
                            mode,
                            0,
                            128 * 1024,
                        )
                        .joining(Join::Snapshot)
                        .with_typist();
                        if features.effects {
                            attach = attach.with_effects();
                        }
                        let result = link
                            .request(TermRequest::Attach(attach))
                            .await
                            .map_err(|_| failure())?;
                        if !self.enrollment_current(epoch) {
                            return Err(failure());
                        }
                        let Some(Value::Attached {
                            attachment,
                            size,
                            running: true,
                            ..
                        }) = result.value
                        else {
                            return Err(failure());
                        };
                        *self.model.borrow_mut() = Some(Workbench::new(
                            terminal.clone(),
                            attachment.clone(),
                            mode,
                            size,
                            features,
                        ));
                        self.interactive.set(mode == Mode::Interact);
                        *self.terminal.borrow_mut() = Some(terminal);
                        *self.attachment.borrow_mut() = Some(attachment);
                        self.notice.set_text_content(Some("Attached to the exact native terminal. Waiting for its current snapshot and typist state…"));
                    }
                    Command::Detach => {
                        let terminal = self.terminal.borrow().clone();
                        let attachment = self.attachment.borrow().clone();
                        self.clear_terminal();
                        self.notice.set_text_content(Some("Detached from this view. The native terminal remains alive. A fresh invitation is required to reconnect."));
                        if let (Some(terminal), Some(attachment)) = (terminal, attachment) {
                            let _ = link
                                .request(TermRequest::Detach(coder_pty::wire::Detach::new(
                                    coder_browser::new_request_id(),
                                    terminal,
                                    attachment,
                                )))
                                .await;
                        }
                        return Ok(());
                    }
                    Command::Thread(resource, before) => {
                        if resource.host
                            != (Host::Paired {
                                key: self.pins.host.clone(),
                            })
                            || resource.kind != Kind::Thread
                            || resource.revision.is_some()
                        {
                            return Err(failure());
                        }
                        let read = link
                            .admission()
                            .prepare_thread(&resource.id, before, now())
                            .map_err(|_| failure())?;
                        let page = browser::read_thread(read, now())
                            .await
                            .map_err(|_| failure())?;
                        if !self.enrollment_current(epoch) || page.thread != resource.id {
                            return Err(failure());
                        }
                        self.show_thread(&resource, &page)?;
                        *self.thread.borrow_mut() = Some((resource, page));
                        self.show_members()?;
                    }
                }
                self.drain_incoming(&mut link)?;
                self.pending.set(false);
                continue;
            }
            let request = self
                .model
                .borrow_mut()
                .as_mut()
                .and_then(Workbench::dispatch);
            if let Some(request) = request {
                let result = link.request(request).await.map_err(|_| failure())?;
                if !self.enrollment_current(epoch) {
                    return Err(failure());
                }
                if matches!(
                    result.reason,
                    Some(
                        coder_pty::wire::Reason::Revoked
                            | coder_pty::wire::Reason::NotAdmitted
                            | coder_pty::wire::Reason::Stale
                            | coder_pty::wire::Reason::Lost
                    )
                ) {
                    return Err(failure());
                }
                self.drain_incoming(&mut link)?;
                if let Some(model) = self.model.borrow_mut().as_mut() {
                    model.result(&result);
                    if matches!(
                        model.state(),
                        State::Disconnected | State::Revoked | State::Stale
                    ) {
                        return Err(failure());
                    }
                }
                continue;
            }
            if self.model.borrow().is_some() {
                match select(Box::pin(link.next()), Box::pin(TimeoutFuture::new(16))).await {
                    Either::Left((Ok(incoming), _)) => {
                        if !self.enrollment_current(epoch) {
                            return Err(failure());
                        }
                        let mut current = self.model.borrow_mut();
                        let model = current.as_mut().ok_or_else(failure)?;
                        model.incoming(incoming).map_err(|_| failure())?;
                        if matches!(
                            model.state(),
                            State::Disconnected | State::Revoked | State::Stale
                        ) {
                            return Err(failure());
                        }
                    }
                    Either::Left((Err(_), _)) => return Err(failure()),
                    Either::Right(_) => {}
                }
            } else {
                TimeoutFuture::new(16).await;
            }
        }
    }

    fn drain_incoming(&self, link: &mut Link) -> Result<(), JsValue> {
        for _ in 0..128 {
            let Some(incoming) = link.take_incoming().map_err(|_| failure())? else {
                return Ok(());
            };
            if let Some(model) = self.model.borrow_mut().as_mut() {
                model.incoming(incoming).map_err(|_| failure())?;
                if matches!(
                    model.state(),
                    State::Disconnected | State::Revoked | State::Stale
                ) {
                    return Err(failure());
                }
            } else {
                return Err(failure());
            }
        }
        Err(failure())
    }

    fn show_sessions(
        self: &Rc<Self>,
        sessions: &[coder_pty::ext::SessionEntry],
    ) -> Result<(), JsValue> {
        self.rows.borrow_mut().clear();
        self.sessions.set_text_content(None);
        self.details.set_text_content(None);
        text(&self.sessions, "h3", "Native saved sessions")?;
        if sessions.is_empty() {
            text(
                &self.sessions,
                "p",
                "This host returned no saved sessions. Nothing was created.",
            )?;
        }
        for session in sessions {
            let row = element(&self.sessions, "div")?;
            text(
                &row,
                "p",
                &format!(
                    "{} · session {} · revision {} · {} members",
                    session.name, session.session, session.revision, session.members
                ),
            )?;
            let button = text(&row, "button", "Read exact native session")?;
            button.set_attribute("type", "button")?;
            let id = session.session.clone();
            self.listen(&button, "click", true, move |runtime, _| {
                runtime.command(Command::Read(id.clone()))
            })?;
        }
        self.notice.set_text_content(Some(
            "Native sessions read. Select a saved record to inspect its original members.",
        ));
        Ok(())
    }

    fn show_members(self: &Rc<Self>) -> Result<(), JsValue> {
        self.details.set_text_content(None);
        self.rows.borrow_mut().retain(|listener| {
            listener
                .target
                .dyn_ref::<web_sys::Node>()
                .is_some_and(web_sys::Node::is_connected)
        });
        let saved = self.saved.borrow();
        let saved = saved.as_ref().ok_or_else(failure)?;
        let projection = self.project(saved)?;
        text(
            &self.details,
            "h3",
            &format!(
                "{} · original layout revision {}",
                projection.name, projection.revision
            ),
        )?;
        for member in projection.members {
            let row = element(&self.details, "div")?;
            text(
                &row,
                "p",
                &format!(
                    "Member {} · {:?} · native {:?} · input {}",
                    member.member, member.resolution, member.native_terminal_state, member.input
                ),
            )?;
            let original = text(
                &row,
                "pre",
                &serde_json::to_string_pretty(&member.resource).map_err(|_| failure())?,
            )?;
            original.set_attribute(
                "style",
                "white-space:pre-wrap;overflow-wrap:anywhere;max-width:100%",
            )?;
            if member.resource.kind == Kind::Terminal
                && member.native_terminal_state == Some(MemberState::Live)
                && member.resolution == workbench_session::projection::Resolution::Ready
            {
                let terminal = TerminalRef {
                    terminal: member.resource.id.clone(),
                    generation: member.resource.generation.clone().ok_or_else(failure)?,
                };
                for (label, mode) in [
                    ("Watch exact terminal", Mode::Observe),
                    ("Attach for typing", Mode::Interact),
                ] {
                    let button = text(&row, "button", label)?;
                    button.set_attribute("type", "button")?;
                    let terminal = terminal.clone();
                    self.listen(&button, "click", true, move |runtime, _| {
                        if runtime.model.borrow().is_none() {
                            runtime.command(Command::Attach(terminal.clone(), mode));
                        }
                    })?;
                }
            } else if member.resource.kind == Kind::Thread
                && member.resource.host == projection.owner
                && member.resource.revision.is_none()
                && coder_access::thread::is_id(&member.resource.id)
                && self.observe.get()
            {
                let button = text(&row, "button", "Read original native thread")?;
                button.set_attribute("type", "button")?;
                let resource = member.resource.clone();
                self.listen(&button, "click", true, move |runtime, _| {
                    runtime.command(Command::Thread(resource.clone(), None))
                })?;
                text(
                    &row,
                    "p",
                    "Read-only through this page's separate native Observe right. Sending, running, and prepared follow-ups are unavailable here.",
                )?;
            } else {
                text(
                    &row,
                    "p",
                    "This original member is a label here. It has no viewer or recreation action.",
                )?;
            }
        }
        self.notice.set_text_content(Some("The native session and exact member references are shown. Closed, lost, unknown, and unsupported members offer no replacement action."));
        Ok(())
    }

    fn project(&self, saved: &Saved) -> Result<workbench_session::projection::Projection, JsValue> {
        let members = saved.members().map_err(|_| failure())?;
        let disclosure = coder_pty::proposal::digest(&(
            &self.pins.host,
            self.pins.generation,
            &self.pins.relay,
            &self.pins.route,
        ));
        let consents: Vec<_> = members
            .iter()
            .filter(|member| member.resource.host == saved.owner)
            .map(|member| {
                Consent::new(member.resource.clone(), disclosure.clone()).map_err(|_| failure())
            })
            .collect::<Result<_, _>>()?;
        let resolver = NativeResolver {
            owner: saved.owner.clone(),
            generation: coder_browser::terminal_generation(&self.pins.host, self.pins.generation),
            expires_at: self.expires_at.get(),
            disclosure,
            route: self.pins.route.clone().unwrap_or_else(|| "relay".into()),
            observe: self.observe.get(),
            thread: self
                .thread
                .borrow()
                .as_ref()
                .map(|(resource, _)| resource.clone()),
            live: saved
                .record
                .members
                .iter()
                .filter_map(|member| match member {
                    coder_pty::ext::Member::Terminal {
                        terminal,
                        state: Some(MemberState::Live),
                        ..
                    } => Some(terminal.clone()),
                    _ => None,
                })
                .collect(),
        };
        let input = self
            .terminal
            .borrow()
            .as_ref()
            .map(|terminal| workbench_session::projection::InputProof {
                resource: ResourceRef::terminal(
                    saved.owner.clone(),
                    terminal.generation.clone(),
                    terminal.terminal.clone(),
                ),
                current_snapshot: self
                    .model
                    .borrow()
                    .as_ref()
                    .is_some_and(|model| model.state() == State::Ready),
                current_attachment: self.attachment.borrow().clone(),
                is_current_typist: self
                    .model
                    .borrow()
                    .as_ref()
                    .is_some_and(Workbench::can_type),
            })
            .into_iter()
            .collect::<Vec<_>>();
        let mut panes = workbench::pane::Panes::new();
        if let Some((resource, page)) = self.thread.borrow().as_ref() {
            panes = panes.adapter_for_host(
                resource.host.clone(),
                Box::new(ThreadPane {
                    resource: resource.clone(),
                    title: page.title.clone(),
                    start: page.start,
                    total: page.total,
                }),
            );
        }
        workbench_session::projection::project(saved, &resolver, &consents, now(), &panes, &input)
            .map_err(|_| failure())
    }

    fn show_thread(
        self: &Rc<Self>,
        resource: &ResourceRef,
        page: &coder_access::thread::ThreadPage,
    ) -> Result<(), JsValue> {
        self.thread_pane.set_text_content(None);
        self.rows.borrow_mut().retain(|listener| {
            listener
                .target
                .dyn_ref::<web_sys::Node>()
                .is_some_and(web_sys::Node::is_connected)
        });
        text(&self.thread_pane, "h3", &page.title)?;
        text(
            &self.thread_pane,
            "pre",
            &serde_json::to_string_pretty(resource).map_err(|_| failure())?,
        )?;
        text(
            &self.thread_pane,
            "p",
            &format!(
                "Original native thread page · turns {}–{} of {} · busy {}. This is a read-only received page; the owner may append later turns.",
                page.start,
                page.start + page.turns.len() as u64,
                page.total,
                page.busy
            ),
        )?;
        for (index, turn) in page.turns.iter().enumerate() {
            message(&self.thread_pane, turn, page.start + index as u64)?;
        }
        if !page.partial.is_empty() {
            text(&self.thread_pane, "h3", "Original incomplete reply")?;
            markdown(&self.thread_pane, &page.partial)?;
        }
        if let Some(failure) = &page.failure {
            text(
                &self.thread_pane,
                "p",
                &format!("Original native failure: {failure}"),
            )?;
        }
        if page.start > 0 {
            let button = text(&self.thread_pane, "button", "Read earlier native turns")?;
            button.set_attribute("type", "button")?;
            let resource = resource.clone();
            let before = page.start;
            self.listen(&button, "click", true, move |runtime, _| {
                runtime.command(Command::Thread(resource.clone(), Some(before)))
            })?;
        }
        let original = element(&self.thread_pane, "details")?;
        text(&original, "summary", "Original native thread page JSON")?;
        text(
            &original,
            "pre",
            &serde_json::to_string_pretty(page).map_err(|_| failure())?,
        )?;
        Ok(())
    }

    fn wire_input(self: &Rc<Self>) -> Result<(), JsValue> {
        self.listen(&self.input, "compositionstart", false, |runtime, _| {
            runtime.composition.borrow_mut().begin();
            if let Some(model) = runtime.model.borrow_mut().as_mut() {
                model.composition(true);
            }
        })?;
        self.listen(&self.input, "compositionend", false, |runtime, event| {
            let Some(event) = event.dyn_ref::<web_sys::CompositionEvent>() else {
                return;
            };
            let text = event.data().unwrap_or_default();
            let accepted = runtime
                .model
                .borrow()
                .as_ref()
                .is_some_and(Workbench::can_type)
                && !runtime.pending.get()
                && runtime.composition.borrow_mut().end(&text);
            if !accepted {
                *runtime.composition.borrow_mut() = Composition::default();
            }
            if let Some(model) = runtime.model.borrow_mut().as_mut() {
                if accepted {
                    model.commit_composition(&text);
                } else {
                    model.composition(false);
                }
            }
            runtime.input.set_value("");
        })?;
        self.listen(&self.input, "input", false, |runtime, event| {
            let Some(event) = event.dyn_ref::<web_sys::InputEvent>() else {
                return;
            };
            let text = runtime.input.value();
            if runtime.composition.borrow_mut().input(
                &text,
                event.is_composing(),
                &event.input_type(),
            ) {
                runtime.type_text(&text, event.input_type() == "insertFromPaste");
            }
            if !event.is_composing() {
                runtime.input.set_value("");
            }
        })?;
        self.listen(&self.input, "keydown", false, |runtime, event| {
            if let Some(key) = event.dyn_ref::<web_sys::KeyboardEvent>() {
                if runtime.key(key) {
                    event.prevent_default();
                }
            }
        })?;
        Ok(())
    }

    fn wire_canvas(self: &Rc<Self>, canvas: &HtmlCanvasElement) -> Result<(), JsValue> {
        for (name, down) in [
            ("pointerdown", Some(true)),
            ("pointerup", Some(false)),
            ("pointermove", None),
        ] {
            self.listen(canvas, name, false, move |runtime, event| {
                let Some(event) = event.dyn_ref::<web_sys::PointerEvent>() else {
                    return;
                };
                if let Some(model) = runtime.model.borrow_mut().as_mut() {
                    let point = [event.offset_x() as f32, event.offset_y() as f32];
                    match down {
                        Some(true) => {
                            model.core.press(point);
                        }
                        Some(false) => {
                            model.core.release(point);
                        }
                        None => model.core.moved(point),
                    }
                }
            })?;
        }
        self.listen(canvas, "keydown", false, |runtime, event| {
            if let Some(key) = event.dyn_ref::<web_sys::KeyboardEvent>() {
                if runtime.key(key) {
                    event.prevent_default();
                }
            }
        })?;
        self.listen(canvas, "wheel", false, |runtime, event| {
            if let Some(event) = event.dyn_ref::<web_sys::WheelEvent>() {
                if let Some(model) = runtime.model.borrow_mut().as_mut() {
                    model.core.scroll_focused(event.delta_y() as f32 / 16.0);
                }
            }
        })?;
        Ok(())
    }

    fn type_text(&self, text: &str, paste: bool) {
        if text.len() > crate::INPUT_MAX {
            return;
        }
        if let Some(model) = self.model.borrow_mut().as_mut() {
            if model.can_type() && !self.pending.get() {
                if paste {
                    model.paste(text);
                } else {
                    model.input(text);
                }
            } else {
                self.notice.set_text_content(Some("Input was refused: this attachment is not the current typist with a current snapshot. No input was queued."));
            }
        }
    }

    fn key(&self, event: &web_sys::KeyboardEvent) -> bool {
        if event.is_composing()
            || event.meta_key()
            || (event.ctrl_key()
                && matches!(
                    event.code().as_str(),
                    "KeyL" | "KeyW" | "KeyT" | "KeyN" | "KeyR" | "KeyV"
                ))
        {
            return false;
        }
        let text_field = event
            .target()
            .is_some_and(|target| target.dyn_into::<HtmlTextAreaElement>().is_ok());
        if text_field && !event.ctrl_key() && !event.alt_key() && event.key().chars().count() == 1 {
            return false;
        }
        use terminal_core::input::{KeyCode as C, Logical, ModifiersState as M, NamedKey as N};
        let (code, logical) = match event.key().as_str() {
            "Enter" => (C::Enter, Logical::Named(N::Enter)),
            "Escape" => (C::Escape, Logical::Named(N::Escape)),
            "Backspace" => (C::Backspace, Logical::Named(N::Backspace)),
            "Delete" => (C::Unidentified, Logical::Named(N::Delete)),
            "Tab" => (C::Unidentified, Logical::Named(N::Tab)),
            "ArrowUp" => (C::ArrowUp, Logical::Named(N::ArrowUp)),
            "ArrowDown" => (C::ArrowDown, Logical::Named(N::ArrowDown)),
            "ArrowLeft" => (C::ArrowLeft, Logical::Named(N::ArrowLeft)),
            "ArrowRight" => (C::ArrowRight, Logical::Named(N::ArrowRight)),
            "Home" => (C::Unidentified, Logical::Named(N::Home)),
            "End" => (C::Unidentified, Logical::Named(N::End)),
            "PageUp" => (C::PageUp, Logical::Named(N::PageUp)),
            "PageDown" => (C::PageDown, Logical::Named(N::PageDown)),
            "Insert" => (C::Unidentified, Logical::Named(N::Insert)),
            "F1" => (C::F1, Logical::Named(N::F1)),
            "F2" => (C::Unidentified, Logical::Named(N::F2)),
            "F3" => (C::Unidentified, Logical::Named(N::F3)),
            "F4" => (C::Unidentified, Logical::Named(N::F4)),
            "F5" => (C::Unidentified, Logical::Named(N::F5)),
            "F6" => (C::Unidentified, Logical::Named(N::F6)),
            "F7" => (C::Unidentified, Logical::Named(N::F7)),
            "F8" => (C::Unidentified, Logical::Named(N::F8)),
            "F9" => (C::Unidentified, Logical::Named(N::F9)),
            "F10" => (C::Unidentified, Logical::Named(N::F10)),
            "F11" => (C::Unidentified, Logical::Named(N::F11)),
            "F12" => (C::Unidentified, Logical::Named(N::F12)),
            key if key.chars().count() == 1 => (C::Unidentified, Logical::Character(key.into())),
            _ => return false,
        };
        let code = physical_code(&event.code()).unwrap_or(code);
        let mut modifiers = M::empty();
        if event.ctrl_key() {
            modifiers = modifiers | M::CONTROL;
        }
        if event.alt_key() {
            modifiers = modifiers | M::ALT;
        }
        if event.shift_key() {
            modifiers = modifiers | M::SHIFT;
        }
        if self.pending.get() {
            return false;
        }
        let Some(model) = self.model.borrow_mut().as_mut().map(|model| {
            model.core.modifiers(modifiers);
            model.key(&terminal_core::input::KeyIn {
                code,
                logical,
                text: (!event.ctrl_key() && !event.alt_key())
                    .then(|| event.key())
                    .filter(|text| text.chars().count() == 1),
                plain: None,
                pressed: true,
                repeat: event.repeat(),
                synthetic: false,
            })
        }) else {
            return false;
        };
        model
    }

    fn action(self: Rc<Self>, action: u8) {
        match action {
            0 => {
                if let Some(model) = self.model.borrow_mut().as_mut() {
                    let _ = model.take_typist();
                }
            }
            1 => {
                if let Some(model) = self.model.borrow_mut().as_mut() {
                    let _ = model.read_blocks(None);
                }
            }
            2 => {
                if let Some(model) = self.model.borrow_mut().as_mut() {
                    let _ = model.read_proposals();
                }
            }
            3 => self.clipboard(false),
            4 => self.clipboard(true),
            5 => self.command(Command::Detach),
            6 => self.command(Command::List),
            _ => {}
        }
    }

    fn clipboard(self: Rc<Self>, paste: bool) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let clipboard = window.navigator().clipboard();
        let promise = if paste {
            clipboard.read_text()
        } else {
            let text = self.model.borrow_mut().as_mut().and_then(|model| {
                model.core.copy_selection();
                model.clipboard()
            });
            let Some(mut text) = text else {
                return;
            };
            let promise = clipboard.write_text(&text);
            erase(&mut text);
            promise
        };
        let (abort, registration) = AbortHandle::new_pair();
        self.aborts.borrow_mut().push(abort);
        let epoch = self.epoch.get();
        wasm_bindgen_futures::spawn_local(async move {
            let result =
                Abortable::new(wasm_bindgen_futures::JsFuture::from(promise), registration).await;
            if !self.enrollment_current(epoch) {
                return;
            }
            match result{
                Ok(Ok(value)) if paste=>{if let Some(mut text)=value.as_string(){self.type_text(&text,true);erase(&mut text);}}
                Ok(Ok(_))=>{},_=>self.notice.set_text_content(Some("Clipboard permission was refused. Use the terminal field or the browser's explicit copy control.")),
            }
        });
    }

    async fn frames(self: Rc<Self>) {
        while self.visible() {
            if self.connected.get() && self.expires_at.get() <= now() {
                self.disconnect();
            }
            let started = js_sys::Date::now();
            let success = {
                let mut gpu = self.gpu.borrow_mut();
                let Some(gpu) = gpu.as_mut() else {
                    return;
                };
                let mut batch = UiBatch { vertices: vec![] };
                let mut current = self.model.borrow_mut();
                if let Some(model) = current.as_mut() {
                    model.core.tick();
                    let cell = terminal_gfx::draw::cell_size(&gpu.atlas);
                    model.core.cell = cell;
                    model.core.area = terminal_core::Application::area_for(gpu.size(), cell);
                    let rect = model.core.area;
                    let can_type = model.can_type();
                    batch.rect(
                        &gpu.atlas,
                        rect.x,
                        rect.y,
                        rect.w,
                        rect.h,
                        terminal_gfx::draw::field(1.0),
                    );
                    if let Some(pane) = model.core.focused_pane() {
                        let vt = &pane.session.vt;
                        let top = terminal_core::select::top(vt, pane.scroll);
                        let rows: Vec<_> = (0..vt.rows())
                            .filter_map(|row| vt.line(top + row))
                            .collect();
                        let inner = terminal_gfx::draw::inner(rect, cell);
                        terminal_gfx::draw::grid(
                            &mut batch,
                            &gpu.atlas,
                            [inner.x, inner.y],
                            &terminal_gfx::draw::Grid { rows: rows.clone() },
                        );
                        if let Some(selection) =
                            pane.selection.filter(|selection| !selection.empty())
                        {
                            for (row, line) in rows.iter().enumerate() {
                                if let Some((from, to)) = selection.columns(
                                    vt,
                                    terminal_core::select::absolute(vt, top + row),
                                    line.cells.len(),
                                ) {
                                    batch.rect(
                                        &gpu.atlas,
                                        inner.x + from as f32 * cell[0],
                                        inner.y + row as f32 * cell[1],
                                        (to - from) as f32 * cell[0],
                                        cell[1],
                                        terminal_gfx::draw::selection(0.35),
                                    );
                                }
                            }
                        }
                        if pane.scroll == 0 {
                            let (row, col) = vt.cursor();
                            terminal_gfx::draw::cursor(
                                &mut batch,
                                &gpu.atlas,
                                [
                                    inner.x + col as f32 * cell[0],
                                    inner.y + row as f32 * cell[1],
                                ],
                                vt.row(row).and_then(|row| row.cells.get(col)),
                                vt.cursor_style().shape,
                                true,
                            );
                        }
                        let rows = ((rect.h - 30.0) / cell[1]).floor().clamp(2.0, 120.0) as u16;
                        let cols = ((rect.w - 8.0) / cell[0]).floor().clamp(2.0, 240.0) as u16;
                        let resize = can_type
                            && (vt.rows() != usize::from(rows) || vt.cols() != usize::from(cols));
                        if resize {
                            model.resize(rows, cols);
                        }
                    }
                    let ready = model.can_type();
                    self.input.set_disabled(!ready);
                    if !ready && !self.composition.borrow().active {
                        self.input.set_value("");
                    }
                    let state = format!(
                        "{:?}:{}:{}",
                        model.state(),
                        ready,
                        coder_pty::proposal::digest(&(&model.proposals, &model.blocks))
                    );
                    if *self.drawn_state.borrow() != state {
                        *self.drawn_state.borrow_mut() = state;
                        self.refresh_decisions(model);
                        self.notice.set_text_content(Some(&format!(
                            "Native attachment {:?} · input {} · {}",
                            model.state(),
                            if ready {
                                "current typist"
                            } else {
                                "unavailable"
                            },
                            model
                                .notice
                                .as_deref()
                                .unwrap_or("snapshot and owner state checked")
                        )));
                    }
                    self.update_buttons(true, ready, model.features);
                } else {
                    self.input.set_disabled(true);
                    self.update_buttons(false, false, Features::NONE);
                }
                gpu.draw(&batch).is_ok()
            };
            if !success {
                self.retire();
                let _ = text(
                    &self.root,
                    "p",
                    "The shared renderer stopped. This terminal view was retired. Reopen the workbench and read its current standing before reconnecting.",
                );
                return;
            }
            self.frames.set(self.frames.get() + 1);
            self.draw_ms
                .set(self.draw_ms.get() + (js_sys::Date::now() - started).max(0.0));
            TimeoutFuture::new(33).await;
        }
        self.retire();
    }

    fn update_buttons(&self, has_model: bool, ready: bool, features: Features) {
        let Ok(buttons) = self
            .root
            .query_selector_all("[data-terminal-input-action], [data-terminal-action]")
        else {
            return;
        };
        for index in 0..buttons.length() {
            let Some(button) = buttons
                .item(index)
                .and_then(|node| node.dyn_into::<web_sys::HtmlButtonElement>().ok())
            else {
                continue;
            };
            let enabled = if button.has_attribute("data-terminal-input-action") {
                ready
            } else {
                match button.get_attribute("data-terminal-action").as_deref() {
                    Some("0") => has_model && features.typist && self.interactive.get(),
                    Some("1") => has_model && features.blocks,
                    Some("2") => has_model && features.proposals,
                    Some("3") => has_model,
                    Some("4") => ready,
                    Some("5") => has_model,
                    Some("6") => self.connected.get(),
                    _ => false,
                }
            };
            button.set_disabled(!enabled || self.pending.get());
        }
    }

    fn refresh_decisions(self: &Rc<Self>, model: &Workbench) {
        self.proposals.borrow_mut().clear();
        self.decisions.set_text_content(None);
        if let Some(page) = &model.proposals {
            for entry in &page.entries {
                let Ok(row) = element(&self.decisions, "section") else {
                    continue;
                };
                let proposal = &entry.proposal;
                let _ = text(
                    &row,
                    "h3",
                    &format!(
                        "Original proposal {} · revision {} · {:?}",
                        proposal.id, proposal.revision, entry.state
                    ),
                );
                if let Ok(command) = text(&row, "pre", &proposal.command) {
                    let _ = command
                        .set_attribute("style", "white-space:pre-wrap;overflow-wrap:anywhere");
                }
                let _ = text(
                    &row,
                    "p",
                    &format!(
                        "Thread {} · terminal {} · generation {} · OS cwd {} · shell directory {} · context digest {}",
                        proposal.thread,
                        proposal.binding.terminal,
                        proposal.binding.generation,
                        proposal.binding.cwd,
                        proposal
                            .binding
                            .shell_directory
                            .as_deref()
                            .unwrap_or("Unavailable"),
                        proposal.binding.context_digest
                    ),
                );
                if matches!(
                    entry.state,
                    coder_pty::proposal::State::Pending | coder_pty::proposal::State::Warned { .. }
                ) {
                    for approve in [true, false] {
                        let Ok(button) = text(
                            &row,
                            "button",
                            if approve {
                                "Approve exact command and revision"
                            } else {
                                "Reject exact revision"
                            },
                        ) else {
                            continue;
                        };
                        let _ = button.set_attribute("type", "button");
                        if !model.can_type() {
                            let _ = button.set_attribute("disabled", "");
                        }
                        let proposal = proposal.clone();
                        let weak = Rc::downgrade(self);
                        let callback = Closure::wrap(Box::new(move |_: Event| {
                            if let Some(runtime) = weak.upgrade() {
                                if runtime.visible() {
                                    if let Some(model) = runtime.model.borrow_mut().as_mut() {
                                        let _ = model.decide(
                                            &proposal.thread,
                                            &proposal.id,
                                            proposal.revision,
                                            approve,
                                        );
                                    }
                                }
                            }
                        })
                            as Box<dyn FnMut(Event)>);
                        if button
                            .add_event_listener_with_callback(
                                "click",
                                callback.as_ref().unchecked_ref(),
                            )
                            .is_ok()
                        {
                            self.proposals.borrow_mut().push(Listener {
                                target: button.into(),
                                name: "click",
                                callback,
                            });
                        }
                    }
                }
            }
        }
        for block in &model.blocks {
            if let Ok(row) = element(&self.decisions, "section") {
                let _ = text(&row, "h3", &format!("Native block {}", block.block));
                let _ = text(&row, "pre", &block.command);
                let _ = text(
                    &row,
                    "pre",
                    &serde_json::to_string_pretty(block).unwrap_or_default(),
                );
            }
        }
    }
}

struct NativeResolver {
    owner: Host,
    generation: String,
    expires_at: u64,
    disclosure: String,
    route: String,
    observe: bool,
    thread: Option<ResourceRef>,
    live: Vec<TerminalRef>,
}
impl Resolver for NativeResolver {
    fn current(&self, host: &Host) -> Option<Current> {
        (*host == self.owner).then(|| Current {
            host: self.owner.clone(),
            generation: Some(self.generation.clone()),
            route: Some(self.route.clone()),
            disclosure: self.disclosure.clone(),
            expires_at: self.expires_at,
            read: true,
            input: true,
            revoked: false,
            capabilities: if self.observe {
                vec![Kind::Terminal, Kind::Thread]
            } else {
                vec![Kind::Terminal]
            },
        })
    }
    fn resource(&self, resource: &ResourceRef) -> Probe {
        Probe {
            resource: resource.clone(),
            state: if (resource.kind == Kind::Terminal
                && self.live.iter().any(|terminal| {
                    terminal.terminal == resource.id
                        && Some(&terminal.generation) == resource.generation.as_ref()
                }))
                || self.thread.as_ref() == Some(resource)
            {
                workbench_session::State::Ready
            } else {
                workbench_session::State::Unavailable
            },
        }
    }
}

struct ThreadPane {
    resource: ResourceRef,
    title: String,
    start: u64,
    total: u64,
}
impl workbench::pane::PaneAdapter for ThreadPane {
    fn kind(&self) -> workbench::pane::PaneKind {
        workbench::pane::PaneKind::Thread
    }
    fn describe(&self, subject: &workbench::pane::Subject) -> workbench::pane::Description {
        let matches = matches!(subject,workbench::pane::Subject::Resource{resource}if resource==&self.resource);
        workbench::pane::Description {
            state: if matches {
                workbench::pane::PaneState::Ready
            } else {
                workbench::pane::PaneState::Unavailable
            },
            title: self.title.clone(),
            detail: format!(
                "Original native thread page starts at {} of {} retained turns; read-only.",
                self.start, self.total
            ),
            actions: vec![],
        }
    }
}

fn element(parent: &Element, tag: &str) -> Result<Element, JsValue> {
    let element = parent
        .owner_document()
        .ok_or_else(failure)?
        .create_element(tag)?;
    parent.append_child(&element)?;
    Ok(element)
}
fn text(parent: &Element, tag: &str, value: &str) -> Result<Element, JsValue> {
    if matches!(tag, "button" | "h2" | "h3" | "p" | "pre") {
        use rust_native::{Element as NativeElement, Node, TextRole, View, style::Style};
        let key = next_key();
        let html = if tag == "button" {
            rust_native_web::render_view(&coder_ui::control::submit(
                &key,
                value,
                coder_ui::control::Gate {
                    enabled: true,
                    reason: None,
                },
                coder_ui::workspace::Palette {
                    text: color(coder_ui::theme::Intensity::ThreeQuarters),
                    heading: color(coder_ui::theme::Intensity::Full),
                    secondary: color(coder_ui::theme::Intensity::Half),
                    border: color(coder_ui::theme::Intensity::Quarter),
                },
            ))
            .map_err(|_| failure())?
        } else {
            let role = match tag {
                "h2" | "h3" => TextRole::Heading,
                "pre" => TextRole::Code,
                _ => TextRole::Body,
            };
            rust_native_web::render_view(&View::<()>::new_v3(
                &key,
                1,
                Node {
                    key: key.clone(),
                    style: Style::default(),
                    element: NativeElement::Text {
                        value: value.into(),
                        role,
                    },
                },
            ))
            .map_err(|_| failure())?
        };
        let holder = element(parent, "div")?;
        holder.set_inner_html(&html);
        return holder
            .query_selector(if tag == "button" {
                "button"
            } else {
                ".rn-node"
            })?
            .ok_or_else(failure);
    }
    let element = element(parent, tag)?;
    element.set_text_content(Some(value));
    Ok(element)
}
fn next_key() -> String {
    thread_local! {static NEXT:Cell<u64>=const{Cell::new(0)};}
    NEXT.with(|next| {
        let value = next.get() + 1;
        next.set(value);
        format!("cloud-terminal-{value}")
    })
}
fn color(intensity: coder_ui::theme::Intensity) -> rust_native::style::Color {
    let value = intensity.color();
    rust_native::style::Color::rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}
fn field(
    parent: &Element,
    label: &str,
    secret: bool,
    multiline: bool,
    max_bytes: usize,
) -> Result<Element, JsValue> {
    use rust_native::{Element as NativeElement, Node, View, style::Style};
    let key = next_key();
    let view = View::<()>::new_v3(
        &key,
        1,
        Node {
            key: key.clone(),
            style: Style::default(),
            element: NativeElement::Field {
                label: label.into(),
                value: String::new(),
                placeholder: String::new(),
                secret,
                multiline,
                enabled: true,
                max_bytes,
                on_change: (),
            },
        },
    );
    let holder = element(parent, "div")?;
    holder.set_inner_html(&rust_native_web::render_view(&view).map_err(|_| failure())?);
    holder
        .query_selector("input, textarea")?
        .ok_or_else(failure)
}
fn physical_code(code: &str) -> Option<terminal_core::input::KeyCode> {
    use terminal_core::input::KeyCode as C;
    match code {
        "KeyA" => Some(C::KeyA),
        "KeyB" => Some(C::KeyB),
        "KeyC" => Some(C::KeyC),
        "KeyD" => Some(C::KeyD),
        "KeyE" => Some(C::KeyE),
        "KeyF" => Some(C::KeyF),
        "KeyG" => Some(C::KeyG),
        "KeyH" => Some(C::KeyH),
        "KeyI" => Some(C::KeyI),
        "KeyJ" => Some(C::KeyJ),
        "KeyK" => Some(C::KeyK),
        "KeyL" => Some(C::KeyL),
        "KeyM" => Some(C::KeyM),
        "KeyN" => Some(C::KeyN),
        "KeyO" => Some(C::KeyO),
        "KeyP" => Some(C::KeyP),
        "KeyQ" => Some(C::KeyQ),
        "KeyR" => Some(C::KeyR),
        "KeyS" => Some(C::KeyS),
        "KeyT" => Some(C::KeyT),
        "KeyU" => Some(C::KeyU),
        "KeyV" => Some(C::KeyV),
        "KeyW" => Some(C::KeyW),
        "KeyX" => Some(C::KeyX),
        "KeyY" => Some(C::KeyY),
        "KeyZ" => Some(C::KeyZ),
        _ => None,
    }
}
fn shared_surface(parent: &Element) -> Result<Element, JsValue> {
    use rust_native::{Element as NativeElement, Node, View, style::Style};
    let key = next_key();
    let view = View::<()>::new_v3(
        &key,
        1,
        Node {
            key: key.clone(),
            style: Style::default(),
            element: NativeElement::Surface {
                resource: "cloud-terminal-grid".into(),
                label: "Native terminal · shared Rust GPU surface".into(),
            },
        },
    );
    let holder = element(parent, "div")?;
    holder.set_inner_html(&rust_native_web::render_view(&view).map_err(|_| failure())?);
    let surface = holder
        .query_selector("[data-rn-surface=cloud-terminal-grid]")?
        .ok_or_else(failure)?;
    // This mount registers only its compiled terminal renderer for the local resource.
    surface.set_text_content(None);
    surface.set_id("cloud-terminal-surface");
    surface.set_attribute("style", "width:100%;min-width:0;max-width:100%")?;
    Ok(surface)
}
fn markdown(parent: &Element, value: &str) -> Result<(), JsValue> {
    use rust_native::{Element as NativeElement, Node, View, style::Style};
    let key = next_key();
    let view = View::<()>::new_v3(
        &key,
        1,
        Node {
            key: key.clone(),
            style: Style::default(),
            element: NativeElement::Markdown {
                blocks: rust_native::markdown::parse(value),
            },
        },
    );
    let holder = element(parent, "div")?;
    holder.set_inner_html(&rust_native_web::render_view(&view).map_err(|_| failure())?);
    Ok(())
}
fn message(
    parent: &Element,
    turn: &coder_access::thread::ThreadTurn,
    index: u64,
) -> Result<(), JsValue> {
    use rust_native::{Element as NativeElement, MessageRole, Node, View, style::Style};
    let key = next_key();
    let body = Node {
        key: format!("{key}:body"),
        style: Style::default(),
        element: NativeElement::Markdown {
            blocks: rust_native::markdown::parse(&turn.text),
        },
    };
    let view = View::<()>::new_v3(
        &key,
        1,
        Node {
            key: key.clone(),
            style: Style::default(),
            element: NativeElement::Message {
                role: match turn.role {
                    coder_access::thread::ThreadRole::User => MessageRole::User,
                    coder_access::thread::ThreadRole::Assistant => MessageRole::Assistant,
                },
                note: Some(format!(
                    "Original turn {index} · model {} · stopped {}",
                    turn.model.as_deref().unwrap_or("Unavailable"),
                    turn.stopped
                )),
                children: vec![body],
            },
        },
    );
    let holder = element(parent, "div")?;
    holder.set_inner_html(&rust_native_web::render_view(&view).map_err(|_| failure())?);
    if !turn.extras.is_empty() {
        text(
            parent,
            "pre",
            &serde_json::to_string_pretty(&turn.extras).map_err(|_| failure())?,
        )?;
    }
    Ok(())
}
fn erase(value: &mut String) {
    // The owned string is no longer observable as text when its bytes are overwritten.
    unsafe {
        value.as_bytes_mut().fill(0);
    }
    value.clear();
}
struct SecretText(String);
impl Drop for SecretText {
    fn drop(&mut self) {
        erase(&mut self.0);
    }
}
fn privacy_ready(document: &web_sys::Document) -> bool {
    document
        .query_selector("#cloud-private[data-cloud-privacy-ready]")
        .ok()
        .flatten()
        .is_some()
}
fn now() -> u64 {
    (js_sys::Date::now() / 1000.0) as u64
}
fn failure() -> JsValue {
    JsValue::from_str(
        "The admitted native workbench is unavailable. Reload and read its current standing before reconnecting.",
    )
}
