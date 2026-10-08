//! Admitted host terminal workbench over the page's existing GPU renderer.
use coder_browser::{
    Admission, Direct, Incoming, Relayed,
    browser::{self, Socket},
    workbench::Workbench,
};
use coder_host_wire::TermRequest;
use coder_pty::{
    ext::{Features, Join},
    wire::{Attach, Mode, TerminalRef, TerminalResult, Value},
};
use serde::Deserialize;
use std::{cell::RefCell, rc::Rc};
use verse::ui::{Atlas, UiBatch};
use wasm_bindgen::{JsCast, closure::Closure, prelude::*};
use web_sys::{
    CompositionEvent, HtmlElement, HtmlTextAreaElement, InputEvent, KeyboardEvent, PointerEvent,
};
thread_local! { static ACTIVE: RefCell<Option<Rc<RefCell<Workbench>>>> = const { RefCell::new(None) }; }
thread_local! { static PERF: RefCell<(u64,f64,f64)> = const { RefCell::new((0,0.0,0.0)) }; }
#[wasm_bindgen]
pub fn host_terminal_receipt() -> String {
    PERF.with(|perf| { let (frames,total,max)=*perf.borrow(); serde_json::json!({"v":"openagents.browser-terminal.render-receipt.v1","frames":frames,"draw_ms_mean":if frames>0 {total/frames as f64} else {0.0},"draw_ms_max":max,"renderer":"Existing page wgpu WebGPU/WebGL2 selection","world_presence_contains_terminal_content":false,"limits":["Missing atlas glyphs use a declared fallback glyph","Local shell and provider helpers are unavailable","Reconnect requires fresh admission and snapshot; input is not retained"]}).to_string() })
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    device_key: [u8; 32],
    access: coder_access::protocol::Access,
    generation: u64,
    route: String,
    terminal: TerminalRef,
    session: Option<String>,
    mode: Mode,
    capabilities: Vec<String>,
}
enum Link {
    Direct(Direct<coder_browser::reach_socket::Socket>),
    Relay(Relayed<Socket>),
}
impl Link {
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
fn now() -> u64 {
    (js_sys::Date::now() / 1000.0) as u64
}
fn failure(_: impl std::fmt::Debug) -> JsValue {
    JsValue::from_str(
        "The admitted host terminal is unavailable. Reconnect and read its current state.",
    )
}
/// Called by an enrolled browser mount with its existing grant; this creates no new authority.
#[wasm_bindgen]
pub async fn open_host_terminal(config: &str) -> std::result::Result<(), JsValue> {
    if config.len() > 128 * 1024 {
        return Err(failure("bounds"));
    }
    let config: Config = serde_json::from_str(config).map_err(failure)?;
    let secret = secp256k1::SecretKey::from_byte_array(config.device_key).map_err(failure)?;
    let mut admission = Admission::new(
        secret,
        config.access,
        coder_access::RelayPolicy::Production,
        config.generation,
        now(),
    )
    .map_err(failure)?;
    admission.negotiate(&config.capabilities);
    let features = Features::advertised(&config.capabilities);
    let mut link = if config.route == "relay" {
        let socket = Socket::open(&admission, now()).await.map_err(failure)?;
        Link::Relay(Relayed::new(admission, socket))
    } else {
        Link::Direct(
            browser::direct(&config.route, admission, now())
                .await
                .map_err(failure)?,
        )
    };
    let mut attach = Attach::new(
        coder_browser::new_request_id(),
        config.terminal.clone(),
        config.mode,
        0,
        128 * 1024,
    );
    if features.snapshot {
        attach = attach.joining(Join::Snapshot);
    }
    if features.effects {
        attach = attach.with_effects();
    }
    if features.typist {
        attach = attach.with_typist();
    }
    let result = link
        .request(TermRequest::Attach(attach))
        .await
        .map_err(failure)?;
    let Some(Value::Attached {
        attachment, size, ..
    }) = result.value
    else {
        return Err(failure("attachment"));
    };
    let mut model = Workbench::new(
        config.terminal.clone(),
        attachment.clone(),
        config.mode,
        size,
        features,
    );
    if let Some(session) = config.session {
        if features.sessions {
            let result = link
                .request(TermRequest::SessionRead(coder_pty::ext::SessionRead::new(
                    coder_browser::new_request_id(),
                    session,
                )))
                .await
                .map_err(failure)?;
            if let Some(Value::Session { record }) = result.value {
                if !record.members.iter().any(|member| matches!(member, coder_pty::ext::Member::Terminal {terminal,..} if *terminal == config.terminal)) { return Err(failure("session binding")); }
                model.session = Some(record);
            }
        }
    }
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        // Resource identity only. No key, grant, terminal content, or input is stored here.
        let _ = storage.set_item("openagents.browser-terminal.resource", &serde_json::json!({"terminal":config.terminal,"session":model.session.as_ref().map(|s| &s.session)}).to_string());
    }
    let model = Rc::new(RefCell::new(model));
    mount_controls(&model)?;
    ACTIVE.with(|active| {
        if let Some(old) = active.borrow_mut().replace(model.clone()) {
            old.borrow_mut().disconnect();
        }
    });
    let detach =
        coder_pty::wire::Detach::new(coder_browser::new_request_id(), config.terminal, attachment);
    wasm_bindgen_futures::spawn_local(async move {
        loop {
            if !ACTIVE.with(|active| {
                active
                    .borrow()
                    .as_ref()
                    .is_some_and(|current| Rc::ptr_eq(current, &model))
            }) {
                break;
            }
            let request = model.borrow_mut().dispatch();
            if let Some(request) = request {
                match link.request(request).await {
                    Ok(result) => model.borrow_mut().result(&result),
                    Err(_) => {
                        model.borrow_mut().disconnect();
                        break;
                    }
                }
                continue;
            }
            // DOM carriage supplies whole binary frames. Polling output never retains shell input.
            match futures_util::future::select(
                Box::pin(link.next()),
                Box::pin(gloo_timers::future::TimeoutFuture::new(16)),
            )
            .await
            {
                futures_util::future::Either::Left((Ok(incoming), _)) => {
                    if model.borrow_mut().incoming(incoming).is_err() {
                        model.borrow_mut().disconnect();
                        break;
                    }
                }
                futures_util::future::Either::Left((Err(_), _)) => {
                    model.borrow_mut().disconnect();
                    break;
                }
                futures_util::future::Either::Right(_) => {}
            }
            refresh_proposals(&model);
        }
        // Detach once; route loss never causes a retry or closes the host terminal.
        let _ = link.request(TermRequest::Detach(detach)).await;
    });
    Ok(())
}
/// Detaches from the view. The host terminal remains alive.
#[wasm_bindgen]
pub fn close_host_terminal() {
    ACTIVE.with(|active| {
        if let Some(model) = active.borrow_mut().take() {
            model.borrow_mut().disconnect();
        }
    });
    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
        if let Some(panel) = document.get_element_by_id("host-terminal-controls") {
            panel.remove();
        }
    }
}
pub fn active() -> bool {
    ACTIVE.with(|active| active.borrow().is_some())
}
pub fn key(event: &KeyboardEvent, down: bool) -> bool {
    if !active() {
        return false;
    }
    if !down || event.is_composing() {
        return false;
    }
    let text_field = event
        .target()
        .is_some_and(|t| t.dyn_into::<HtmlTextAreaElement>().is_ok());
    if text_field && !event.ctrl_key() && !event.alt_key() && event.key().chars().count() == 1 {
        return false;
    }
    if text_field && event.ctrl_key() && event.code() == "KeyV" {
        return false;
    }
    // Browser-reserved chords stay available through explicit accessory buttons.
    if event.meta_key()
        || (event.ctrl_key()
            && matches!(
                event.code().as_str(),
                "KeyL" | "KeyW" | "KeyT" | "KeyN" | "KeyR"
            ))
    {
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
        key if key.chars().count() == 1 => (
            if event.code() == "KeyB" {
                C::KeyB
            } else {
                C::Unidentified
            },
            Logical::Character(key.into()),
        ),
        _ => return true,
    };
    let code = physical_code(&event.code()).unwrap_or(code);
    let mut modifiers = M::empty();
    if event.ctrl_key() {
        modifiers = modifiers | M::CONTROL;
    }
    if event.shift_key() {
        modifiers = modifiers | M::SHIFT;
    }
    if event.alt_key() {
        modifiers = modifiers | M::ALT;
    }
    ACTIVE.with(|active| {
        if let Some(model) = active.borrow().as_ref() {
            let mut model = model.borrow_mut();
            model.core.modifiers(modifiers);
            let text = (!event.ctrl_key() && !event.alt_key())
                .then(|| event.key())
                .filter(|s| s.chars().count() == 1);
            model.key(&terminal_core::input::KeyIn {
                code,
                logical,
                text,
                plain: None,
                pressed: true,
                repeat: event.repeat(),
                synthetic: false,
            });
        }
    });
    true
}
pub fn pointer(event: &PointerEvent, down: Option<bool>) -> bool {
    ACTIVE.with(|active| {
        let current = active.borrow();
        let Some(model) = current.as_ref() else {
            return false;
        };
        let mut model = model.borrow_mut();
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
        true
    })
}
pub fn draw(batch: &mut UiBatch, atlas: &Atlas, size: [f32; 2]) {
    let started = js_sys::Date::now();
    let was_active = active();
    ACTIVE.with(|active| {
        let current = active.borrow();
        let Some(model) = current.as_ref() else {
            return;
        };
        let mut model = model.borrow_mut();
        model.core.tick();
        let cell = terminal_gfx::draw::cell_size(atlas);
        model.core.cell = cell;
        model.core.area = terminal_core::Application::area_for(size, cell);
        let rect = model.core.area;
        batch.rect(
            atlas,
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            terminal_gfx::draw::field(1.0),
        );
        let label = if model.can_type() {
            "Host terminal · typist"
        } else {
            "Host terminal · watch or unavailable"
        };
        batch.text(
            atlas,
            rect.x + 8.0,
            rect.y + 4.0,
            label,
            terminal_gfx::draw::white(coder_ui::theme::Intensity::ThreeQuarters, 1.0),
        );
        if let Some(pane) = model.core.focused_pane() {
            let vt = &pane.session.vt;
            let top = terminal_core::select::top(vt, pane.scroll);
            let rows: Vec<_> = (0..vt.rows()).filter_map(|r| vt.line(top + r)).collect();
            let inner = terminal_gfx::draw::inner(rect, cell);
            let grid = terminal_gfx::draw::Grid { rows: rows.clone() };
            terminal_gfx::draw::grid(batch, atlas, [inner.x, inner.y], &grid);
            if let Some(selection) = pane.selection.filter(|s| !s.empty()) {
                for (r, row) in rows.iter().enumerate() {
                    let line = terminal_core::select::absolute(vt, top + r);
                    if let Some((from, to)) = selection.columns(vt, line, row.cells.len()) {
                        batch.rect(
                            atlas,
                            inner.x + from as f32 * cell[0],
                            inner.y + r as f32 * cell[1],
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
                    batch,
                    atlas,
                    [
                        inner.x + col as f32 * cell[0],
                        inner.y + row as f32 * cell[1],
                    ],
                    vt.row(row).and_then(|r| r.cells.get(col)),
                    vt.cursor_style().shape,
                    true,
                );
            }
        }
        if let Some(notice) = &model.notice {
            batch.text(
                atlas,
                rect.x + 8.0,
                rect.y + rect.h - cell[1],
                notice,
                terminal_gfx::draw::white(coder_ui::theme::Intensity::Half, 1.0),
            );
        }
    });
    if was_active {
        let elapsed = (js_sys::Date::now() - started).max(0.0);
        PERF.with(|perf| {
            let mut p = perf.borrow_mut();
            p.0 += 1;
            p.1 += elapsed;
            p.2 = p.2.max(elapsed);
        });
    }
}
fn mount_controls(model: &Rc<RefCell<Workbench>>) -> std::result::Result<(), JsValue> {
    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or_else(|| failure("document"))?;
    if let Some(old) = document.get_element_by_id("host-terminal-controls") {
        old.remove();
    }
    let panel = document.create_element("div")?.dyn_into::<HtmlElement>()?;
    panel.set_id("host-terminal-controls");
    crate::theme::declarations(
        &panel,
        &format!(
            "position:fixed;left:12px;bottom:12px;z-index:50;max-width:90vw;background:#{:06x};color:#{:06x};padding:8px",
            coder_ui::coder_noir::SURFACE_SUBTLE,
            coder_ui::coder_noir::CONTENT
        ),
    )?;
    let input = document
        .create_element("textarea")?
        .dyn_into::<HtmlTextAreaElement>()?;
    input.set_attribute("aria-label", "Host terminal text and IME input")?;
    crate::theme::control(&input)?;
    let current = model.clone();
    let callback =
        Closure::wrap(
            Box::new(move |_: CompositionEvent| current.borrow_mut().composition(true))
                as Box<dyn FnMut(CompositionEvent)>,
        );
    input
        .add_event_listener_with_callback("compositionstart", callback.as_ref().unchecked_ref())?;
    callback.forget();
    let current = model.clone();
    let field = input.clone();
    let callback = Closure::wrap(Box::new(move |event: CompositionEvent| {
        if let Some(text) = event.data() {
            current.borrow_mut().commit_composition(&text);
        }
        field.set_value("");
    }) as Box<dyn FnMut(CompositionEvent)>);
    input.add_event_listener_with_callback("compositionend", callback.as_ref().unchecked_ref())?;
    callback.forget();
    let current = model.clone();
    let field = input.clone();
    let callback = Closure::wrap(Box::new(move |event: InputEvent| {
        if !event.is_composing() {
            let text = field.value();
            if !text.is_empty() {
                if event.input_type() == "insertFromPaste" {
                    current.borrow_mut().paste(&text);
                } else {
                    current.borrow_mut().input(&text);
                }
                field.set_value("");
            }
        }
    }) as Box<dyn FnMut(InputEvent)>);
    input.add_event_listener_with_callback("input", callback.as_ref().unchecked_ref())?;
    callback.forget();
    panel.append_child(&input)?;
    for (label, bytes) in [
        ("Esc", "\u{1b}"),
        ("Tab", "\t"),
        ("Ctrl+C", "\u{3}"),
        ("Enter", "\r"),
    ] {
        let current = model.clone();
        let text = bytes.to_owned();
        button(&panel, label, move || current.borrow_mut().input(&text))?;
    }
    let current = model.clone();
    button(&panel, "Take typing", move || {
        let _ = current.borrow_mut().take_typist();
    })?;
    let current = model.clone();
    button(&panel, "Blocks", move || {
        let _ = current.borrow_mut().read_blocks(None);
    })?;
    let current = model.clone();
    button(&panel, "Proposals", move || {
        let _ = current.borrow_mut().read_proposals();
    })?;
    let current = model.clone();
    button(&panel, "Copy selection", move || {
        let text = {
            let mut model = current.borrow_mut();
            model.core.copy_selection();
            model.clipboard()
        };
        if let Some(text) = text {
            if let Some(window) = web_sys::window() {
                let promise = window.navigator().clipboard().write_text(&text);
                let current = current.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if wasm_bindgen_futures::JsFuture::from(promise).await.is_err() {
                        current.borrow_mut().notice=Some("Clipboard permission denied. Select text and use the browser copy control.".into());
                    }
                });
            }
        }
    })?;
    let current = model.clone();
    button(&panel, "Paste", move || {
        if let Some(window) = web_sys::window() {
            let promise = window.navigator().clipboard().read_text();
            let current = current.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match wasm_bindgen_futures::JsFuture::from(promise).await {
                    Ok(value) => {
                        if let Some(text) = value.as_string() {
                            current.borrow_mut().paste(&text)
                        }
                    }
                    Err(_) => {
                        current.borrow_mut().notice = Some(
                            "Clipboard permission denied. Paste into the terminal text field."
                                .into(),
                        )
                    }
                }
            });
        }
    })?;
    let proposals = document.create_element("div")?;
    proposals.set_id("host-terminal-proposals");
    panel.append_child(&proposals)?;
    let status = document.create_element("span")?;
    status.set_text_content(Some(model.borrow().share_status()));
    panel.append_child(&status)?;
    button(&panel, "Detach", close_host_terminal)?;
    document
        .body()
        .ok_or_else(|| failure("body"))?
        .append_child(&panel)?;
    Ok(())
}
fn button(
    parent: &web_sys::Element,
    label: &str,
    mut action: impl FnMut() + 'static,
) -> std::result::Result<HtmlElement, JsValue> {
    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or_else(|| failure("document"))?;
    let element = document
        .create_element("button")?
        .dyn_into::<HtmlElement>()?;
    element.set_text_content(Some(label));
    crate::theme::control(&element)?;
    let callback = Closure::wrap(Box::new(move || action()) as Box<dyn FnMut()>);
    element.add_event_listener_with_callback("click", callback.as_ref().unchecked_ref())?;
    callback.forget();
    parent.append_child(&element)?;
    Ok(element)
}
fn refresh_proposals(model: &Rc<RefCell<Workbench>>) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Some(parent) = document.get_element_by_id("host-terminal-proposals") else {
        return;
    };
    let revision = {
        let model = model.borrow();
        format!(
            "{:?}:{}:{}",
            model.state(),
            serde_json::to_string(&model.proposals).unwrap_or_default(),
            serde_json::to_string(&model.blocks).unwrap_or_default()
        )
    };
    if parent.get_attribute("data-proposals").as_deref() == Some(&revision) {
        return;
    }
    let _ = parent.set_attribute("data-proposals", &revision);
    parent.set_text_content(None);
    let entries = model
        .borrow()
        .proposals
        .as_ref()
        .map(|p| p.entries.clone())
        .unwrap_or_default();
    for entry in entries {
        if !matches!(
            entry.state,
            coder_pty::proposal::State::Pending | coder_pty::proposal::State::Warned { .. }
        ) {
            continue;
        }
        let Ok(row) = document.create_element("div") else {
            continue;
        };
        row.set_text_content(Some(&format!(
            "{} · revision {}",
            entry.proposal.command, entry.proposal.revision
        )));
        for approve in [true, false] {
            let current = model.clone();
            let p = entry.proposal.clone();
            let enabled = model.borrow().can_type();
            let element = button(
                &row,
                if approve {
                    "Approve exact revision"
                } else {
                    "Reject"
                },
                move || {
                    let _ = current
                        .borrow_mut()
                        .decide(&p.thread, &p.id, p.revision, approve);
                },
            );
            if !enabled {
                if let Ok(element) = element {
                    let _ = element.set_attribute("disabled", "");
                }
            }
        }
        let _ = parent.append_child(&row);
    }
    for block in &model.borrow().blocks {
        if let Ok(row) = document.create_element("div") {
            row.set_text_content(Some(&format!("Block {}: {}", block.block, block.command)));
            let _ = parent.append_child(&row);
        }
    }
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

pub fn wheel(delta: f32) -> bool {
    ACTIVE.with(|active| {
        let model = active.borrow();
        if let Some(model) = model.as_ref() {
            model.borrow_mut().core.scroll_focused(delta / 16.0);
            true
        } else {
            false
        }
    })
}
