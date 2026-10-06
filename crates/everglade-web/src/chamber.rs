//! Browser chamber mount over the shared authoritative client and presentation.
use serde::Deserialize;
use std::{cell::RefCell, rc::Rc, sync::Arc};
use verse::grid_engine::{Content as RenderContent, GridEngine, Kind};
use verse::imported::{Gpu, chamber, chamber_session::Session};
use verse::ui::Atlas;
use verse_engine::{
    assets::Pack,
    director::Scene,
    loading::{Budget, Prepared},
};
use verse_world::{
    controls::{Action, Binding, InputMap},
    service::client_runtime,
};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{
    Document, Event, HtmlCanvasElement, HtmlElement, KeyboardEvent, PointerEvent, Window,
};

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    websocket: String,
    host: String,
    grant: String,
    epoch: u64,
    generation: u64,
    instance: u64,
    content: String,
    pack: String,
    scene: String,
    assets: String,
    #[serde(default)]
    bindings: Vec<Binding>,
    #[serde(default)]
    mips: bool,
}
struct Source {
    pack: Pack,
    scene: Scene,
    files: Vec<(String, Vec<u8>)>,
    content: [u8; 32],
}
impl Source {
    fn prepared(&self) -> Result<Prepared, String> {
        let files: Vec<_> = self
            .files
            .iter()
            .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
            .collect();
        Prepared::from_bytes(
            self.pack.clone(),
            &files,
            Budget {
                encoded_total_bytes: 64 * 1024 * 1024,
                rgba_total_bytes: 128 * 1024 * 1024,
                ..Budget::default()
            },
        )
    }
}
struct Page {
    window: Window,
    canvas: HtmlCanvasElement,
    source: Rc<Source>,
    config: Config,
    secret: secp256k1::SecretKey,
    atlas: Atlas,
    engine: Option<GridEngine>,
    session: Option<Session>,
    connecting: Option<client_runtime::Task>,
    epoch: u64,
    active: bool,
    input: InputMap,
    notice: HtmlElement,
    captions: HtmlElement,
    audio: verse_engine::audio_bank::Scene,
    damage: u64,
    look: Option<(i32, [i32; 2])>,
    last: f64,
}
fn path(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 512
        || !value.is_ascii()
        || value.starts_with('/')
        || value.split('/').any(|p| {
            p.is_empty()
                || p == "."
                || p == ".."
                || p.chars()
                    .any(|c| !(c.is_ascii_alphanumeric() || "-_.".contains(c)))
        })
    {
        return Err("Chamber asset path must be relative to this page".into());
    }
    Ok(())
}
async fn fetch(window: &Window, path_value: &str, max: usize) -> Result<Vec<u8>, String> {
    client_runtime::timeout(
        std::time::Duration::from_secs(30),
        fetch_inner(window, path_value, max),
    )
    .await
    .map_err(|_| "Chamber content download timed out")?
}
async fn fetch_inner(window: &Window, path_value: &str, max: usize) -> Result<Vec<u8>, String> {
    struct Abort(web_sys::AbortController);
    impl Drop for Abort {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let abort =
        Abort(web_sys::AbortController::new().map_err(|_| "Cannot cancel chamber download")?);
    path(path_value)?;
    let init = web_sys::RequestInit::new();
    init.set_signal(Some(&abort.0.signal()));
    init.set_credentials(web_sys::RequestCredentials::Omit);
    init.set_redirect(web_sys::RequestRedirect::Error);
    let request = web_sys::Request::new_with_str_and_init(path_value, &init)
        .map_err(|_| "Cannot request chamber content")?;
    let response: web_sys::Response =
        wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|_| "Chamber content unavailable")?
            .dyn_into()
            .map_err(|_| "Invalid chamber response")?;
    if !response.ok() {
        return Err("Chamber content unavailable".into());
    }
    let body = response.body().ok_or("Chamber content has no body")?;
    let reader: web_sys::ReadableStreamDefaultReader = body
        .get_reader()
        .dyn_into()
        .map_err(|_| "Cannot read chamber content")?;
    let mut bytes = vec![];
    loop {
        let chunk = wasm_bindgen_futures::JsFuture::from(reader.read())
            .await
            .map_err(|_| "Chamber content interrupted")?;
        if js_sys::Reflect::get(&chunk, &JsValue::from_str("done"))
            .ok()
            .and_then(|x| x.as_bool())
            .unwrap_or(true)
        {
            break;
        }
        let array: js_sys::Uint8Array = js_sys::Reflect::get(&chunk, &JsValue::from_str("value"))
            .map_err(|_| "Invalid chamber chunk")?
            .dyn_into()
            .map_err(|_| "Invalid chamber chunk")?;
        let end = bytes
            .len()
            .checked_add(array.length() as usize)
            .ok_or("Chamber content exceeds budget")?;
        if end > max {
            let _ = reader.cancel();
            return Err("Chamber content exceeds budget".into());
        }
        let start = bytes.len();
        bytes.resize(end, 0);
        array.copy_to(&mut bytes[start..]);
    }
    Ok(bytes)
}
fn digest(text: &str) -> Result<[u8; 32], String> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err("Chamber content digest is malformed".into());
    }
    let mut bytes = [0; 32];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16)
            .map_err(|_| "Chamber content digest is malformed")?;
    }
    Ok(bytes)
}
fn size(window: &Window, canvas: &HtmlCanvasElement) -> [u32; 2] {
    let scale = window.device_pixel_ratio().clamp(1., 3.);
    [
        (canvas.client_width() as f64 * scale)
            .round()
            .clamp(1., 4096.) as u32,
        (canvas.client_height() as f64 * scale)
            .round()
            .clamp(1., 4096.) as u32,
    ]
}
async fn engine(
    window: &Window,
    canvas: &HtmlCanvasElement,
    source: &Source,
    atlas: &Atlas,
) -> Result<GridEngine, String> {
    let [width, height] = size(window, canvas);
    canvas.set_width(width);
    canvas.set_height(height);
    let mut descriptor =
        wgpu::InstanceDescriptor::new_with_display_handle(Box::new(crate::web::WebDisplay));
    let force_gl = window
        .location()
        .search()
        .is_ok_and(|query| query.split(['?', '&']).any(|part| part == "gl"));
    descriptor.backends = if force_gl {
        wgpu::Backends::GL
    } else {
        wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL
    };
    let instance = wgpu::util::new_instance_with_webgpu_detection(descriptor).await;
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
        .map_err(|_| "Cannot open chamber surface")?;
    let gpu = Gpu::open_async(instance, &surface).await?;
    let prepared = source.prepared()?;
    let statics = chamber::static_instances(
        &source.pack,
        verse_engine::source_position(source.scene.origin),
    );
    GridEngine::on_surface_with(
        gpu,
        surface,
        RenderContent {
            prepared,
            statics,
            kind: Kind::Chamber,
        },
        atlas,
        width,
        height,
    )
}
fn pause(page: &Rc<RefCell<Page>>) {
    let mut p = page.borrow_mut();
    p.active = false;
    p.epoch = p.epoch.saturating_add(1);
    p.input.clear();
    p.look = None;
    if let Some(task) = p.connecting.take() {
        task.abort();
    }
    p.session.take();
    p.engine.take();
    p.audio.clear_captions();
    p.audio.suspended = true;
    p.notice
        .set_text_content(Some("Chamber paused. Select Reconnect to resume."));
}
fn reconnect(page: &Rc<RefCell<Page>>) {
    let mut p = page.borrow_mut();
    if p.window.document().is_some_and(|d| d.hidden()) {
        return;
    }
    if let Some(task) = p.connecting.take() {
        task.abort();
    }
    p.session.take();
    p.input.clear();
    p.look = None;
    p.epoch = p.epoch.saturating_add(1);
    p.active = true;
    p.audio.suspended = false;
    p.notice.set_text_content(Some("Connecting to chamber…"));
    let (epoch, config, secret, source, window, canvas, atlas) = (
        p.epoch,
        p.config.clone(),
        p.secret,
        p.source.clone(),
        p.window.clone(),
        p.canvas.clone(),
        p.atlas.clone(),
    );
    let page = page.clone();
    p.connecting = Some(client_runtime::spawn(async move {
        let result = async {
            let fresh: Config =
                serde_json::from_slice(&fetch(&window, "chamber.json", 64 * 1024).await?)
                    .map_err(|_| "Invalid chamber configuration")?;
            if fresh.instance != config.instance || digest(&fresh.content)? != source.content {
                return Err("Chamber content or destination changed. Reload this page.".into());
            }
            let config = fresh;
            let opened = client_runtime::timeout(
                std::time::Duration::from_secs(10),
                coder_reach::browser::Socket::open(&config.websocket),
            )
            .await
            .map_err(|_| "Chamber WebSocket timed out")?
            .map_err(|_| "Chamber WebSocket unavailable")?;
            let reach = coder_reach::channel::ClientConfig {
                device: secret,
                host: config.host,
                grant: config.grant,
                epoch: config.epoch,
                generation: config.generation,
                timeout: std::time::Duration::from_secs(10),
            };
            let now = (js_sys::Date::now() / 1000.).max(0.) as u64;
            let channel = coder_reach::channel::connect(opened, &reach, now)
                .await
                .map_err(|e| format!("Chamber channel refused: {e}"))?;
            let client = verse_world::service::reach_client::join(
                channel,
                &secret,
                config.instance,
                Some(source.content),
            )
            .await?;
            let session = Session::start_browser(client, &source.scene)?;
            let gpu = engine(&window, &canvas, &source, &atlas).await?;
            Ok::<_, String>((session, gpu))
        }
        .await;
        let mut p = page.borrow_mut();
        if p.epoch != epoch || !p.active {
            return;
        }
        p.connecting = None;
        match result {
            Ok((session, engine)) => {
                p.session = Some(session);
                p.engine = Some(engine);
                p.damage = 0;
                p.notice.set_text_content(Some("Chamber connected"));
            }
            Err(error) => p.notice.set_text_content(Some(&error)),
        }
    }));
}
fn act(p: &mut Page, action: Action) {
    let Some(session) = &mut p.session else {
        return;
    };
    match action {
        Action::Jump => session.jump(&p.source.scene),
        Action::Cast(slot) => {
            if let Some(ability) = verse_world::play::Ability::ALL.get(slot as usize) {
                if session.view().target().is_none() {
                    session.target_nearest();
                }
                session.cast(&p.source.scene, *ability);
            }
        }
        Action::Target => session.target_nearest(),
        Action::Respawn => session.respawn(),
        _ => {}
    }
}
fn control(p: &mut Page, name: &str, down: bool) {
    if let Some(action) = p.input.change(name, down) {
        act(p, action);
    }
}
fn listen<E: JsCast + 'static>(
    target: &web_sys::EventTarget,
    event: &str,
    mut callback: impl FnMut(E) + 'static,
) -> Result<(), String> {
    let closure = Closure::wrap(Box::new(move |event: Event| {
        if let Ok(event) = event.dyn_into::<E>() {
            callback(event);
        }
    }) as Box<dyn FnMut(Event)>);
    target
        .add_event_listener_with_callback(event, closure.as_ref().unchecked_ref())
        .map_err(|_| "Cannot register chamber controls")?;
    closure.forget();
    Ok(())
}
fn button(document: &Document, panel: &HtmlElement, label: &str) -> Result<HtmlElement, String> {
    let element: HtmlElement = document
        .create_element("button")
        .map_err(|_| "Cannot create chamber button")?
        .dyn_into()
        .map_err(|_| "Cannot create chamber button")?;
    element.set_text_content(Some(label));
    element
        .set_attribute("aria-label", label)
        .map_err(|_| "Cannot label chamber button")?;
    let _ = element.style().set_property("min-height", "44px");
    let _ = element.style().set_property("min-width", "44px");
    panel
        .append_child(&element)
        .map_err(|_| "Cannot mount chamber button")?;
    Ok(element)
}
pub async fn run(
    window: Window,
    document: Document,
    canvas: HtmlCanvasElement,
) -> Result<(), String> {
    let config: Config = serde_json::from_slice(&fetch(&window, "chamber.json", 64 * 1024).await?)
        .map_err(|_| "Invalid chamber configuration")?;
    let pack: Pack = serde_json::from_slice(&fetch(&window, &config.pack, 32 * 1024 * 1024).await?)
        .map_err(|_| "Invalid chamber pack")?;
    pack.validate()?;
    let scene = Scene::from_json(&fetch(&window, &config.scene, 1024 * 1024).await?)?;
    path(&config.assets)?;
    let mut files = vec![];
    let mut total = 0;
    for texture in &pack.textures {
        path(&texture.file)?;
        let file = fetch(
            &window,
            &format!("{}/{}", config.assets, texture.file),
            16 * 1024 * 1024,
        )
        .await?;
        total += file.len();
        if total > 64 * 1024 * 1024 {
            return Err("Chamber textures exceed browser budget".into());
        }
        files.push((texture.file.clone(), file));
    }
    if config.mips {
        for (name, max) in [
            (verse_engine::mips::archive::MANIFEST, 1024 * 1024),
            (verse_engine::mips::archive::PAYLOAD, 64 * 1024 * 1024),
        ] {
            let file = fetch(&window, &format!("{}/{}", config.assets, name), max).await?;
            total += file.len();
            if total > 64 * 1024 * 1024 {
                return Err("Chamber source closure exceeds browser budget".into());
            }
            files.push((name.into(), file));
        }
    }
    let source = Rc::new(Source {
        pack,
        scene,
        files,
        content: digest(&config.content)?,
    });
    let admitted = source.prepared()?;
    if verse_content::remote_content::identity_prepared(&admitted, &source.scene)? != source.content
    {
        return Err("Browser chamber content differs from pinned identity".into());
    }
    drop(admitted);
    let storage = window
        .local_storage()
        .map_err(|_| "Browser identity storage unavailable")?
        .ok_or("Browser identity storage unavailable")?;
    let identity = match storage
        .get_item("openagents.grid.secret")
        .map_err(|_| "Browser identity storage unavailable")?
    {
        Some(secret) => verse::identity::Identity::from_secret_hex("Grid", &secret)?,
        None => {
            let identity =
                verse::identity::Identity::from_secret("Grid", verse::identity::random_secret())?;
            storage
                .set_item("openagents.grid.secret", &identity.secret_hex())
                .map_err(|_| "Cannot retain browser world identity")?;
            identity
        }
    };
    let panel: HtmlElement = document
        .create_element("div")
        .map_err(|_| "Cannot create chamber controls")?
        .dyn_into()
        .map_err(|_| "Cannot create chamber controls")?;
    panel
        .set_attribute("aria-label", "Chamber controls")
        .map_err(|_| "Cannot label chamber controls")?;
    let _ = panel.style().set_property("font-size", "1.125rem");
    document
        .body()
        .ok_or("No page body")?
        .append_child(&panel)
        .map_err(|_| "Cannot mount chamber controls")?;
    let identity_label: HtmlElement = document
        .create_element("p")
        .map_err(|_| "Cannot display browser identity")?
        .dyn_into()
        .map_err(|_| "Cannot display browser identity")?;
    let public_key = identity
        .secret
        .x_only_public_key(&secp256k1::Secp256k1::new())
        .0
        .to_string();
    identity_label.set_text_content(Some(&format!("World identity: {public_key}")));
    panel
        .append_child(&identity_label)
        .map_err(|_| "Cannot display browser identity")?;
    let notice: HtmlElement = document
        .create_element("p")
        .map_err(|_| "Cannot create chamber status")?
        .dyn_into()
        .map_err(|_| "Cannot create chamber status")?;
    notice
        .set_attribute("role", "status")
        .map_err(|_| "Cannot label chamber status")?;
    notice
        .set_attribute("aria-live", "polite")
        .map_err(|_| "Cannot label chamber status")?;
    panel
        .append_child(&notice)
        .map_err(|_| "Cannot mount chamber status")?;
    let captions: HtmlElement = document
        .create_element("p")
        .map_err(|_| "Cannot create chamber captions")?
        .dyn_into()
        .map_err(|_| "Cannot create chamber captions")?;
    captions
        .set_attribute("aria-label", "Combat captions")
        .map_err(|_| "Cannot label captions")?;
    panel
        .append_child(&captions)
        .map_err(|_| "Cannot mount captions")?;
    canvas
        .set_attribute("aria-label", "Chamber world")
        .map_err(|_| "Cannot label chamber canvas")?;
    canvas
        .set_attribute("tabindex", "0")
        .map_err(|_| "Cannot focus chamber canvas")?;
    let _ = canvas.style().set_property("touch-action", "none");
    let input = if config.bindings.is_empty() {
        InputMap::default()
    } else {
        InputMap::new(config.bindings.clone())?
    };
    let page = Rc::new(RefCell::new(Page {
        window: window.clone(),
        canvas,
        source,
        config,
        secret: identity.secret,
        atlas: verse::imported::original::atlas()?,
        engine: None,
        session: None,
        connecting: None,
        epoch: 0,
        active: false,
        input,
        notice,
        captions,
        audio: verse_engine::audio_bank::Scene::new(
            Arc::new(verse_engine::audio_bank::Bank::original()?),
            "en",
        )?,
        damage: 0,
        look: None,
        last: 0.,
    }));
    for (label, name) in [
        ("Forward", "TouchForward"),
        ("Backward", "TouchBackward"),
        ("Strafe left", "TouchLeft"),
        ("Strafe right", "TouchRight"),
    ] {
        let b = button(&document, &panel, label)?;
        let p = page.clone();
        listen::<PointerEvent>(b.as_ref(), "pointerdown", move |e| {
            e.prevent_default();
            if let Some(target) = e
                .current_target()
                .and_then(|t| t.dyn_into::<HtmlElement>().ok())
            {
                let _ = target.set_pointer_capture(e.pointer_id());
            }
            control(&mut p.borrow_mut(), name, true);
        })?;
        for event in ["pointerup", "pointercancel", "lostpointercapture"] {
            let p = page.clone();
            listen::<PointerEvent>(b.as_ref(), event, move |_| {
                control(&mut p.borrow_mut(), name, false)
            })?;
        }
    }
    for (i, ability) in verse_world::play::Ability::ALL.into_iter().enumerate() {
        let b = button(&document, &panel, ability.label())?;
        let p = page.clone();
        listen::<Event>(b.as_ref(), "click", move |_| {
            act(&mut p.borrow_mut(), Action::Cast(i as u8))
        })?;
    }
    for (label, action) in [
        ("Jump", Action::Jump),
        ("Target nearest", Action::Target),
        ("Respawn", Action::Respawn),
    ] {
        let b = button(&document, &panel, label)?;
        let p = page.clone();
        listen::<Event>(b.as_ref(), "click", move |_| {
            act(&mut p.borrow_mut(), action)
        })?;
    }
    let resume = button(&document, &panel, "Reconnect")?;
    let p = page.clone();
    listen::<Event>(resume.as_ref(), "click", move |_| reconnect(&p))?;
    for (event, down) in [("keydown", true), ("keyup", false)] {
        let p = page.clone();
        listen::<KeyboardEvent>(window.as_ref(), event, move |e| {
            if e.code() == "Tab" {
                return;
            }
            if e.target().is_some_and(|t| {
                t.dyn_into::<HtmlElement>()
                    .is_ok_and(|e| matches!(e.tag_name().as_str(), "INPUT" | "TEXTAREA" | "BUTTON"))
            }) {
                if !down {
                    control(&mut p.borrow_mut(), &e.code(), false);
                }
                return;
            }
            if e.ctrl_key() || e.meta_key() || e.alt_key() {
                if !down {
                    control(&mut p.borrow_mut(), &e.code(), false);
                }
                return;
            }
            let mapped = if p.borrow().config.bindings.is_empty() {
                e.code() == "Space"
            } else {
                p.borrow()
                    .config
                    .bindings
                    .iter()
                    .any(|b| b.control == e.code())
            };
            if mapped {
                e.prevent_default();
            }
            control(&mut p.borrow_mut(), &e.code(), down);
        })?;
    }
    let p = page.clone();
    listen::<Event>(window.as_ref(), "blur", move |_| pause(&p))?;
    let p = page.clone();
    listen::<Event>(document.as_ref(), "visibilitychange", move |_| {
        if p.borrow().window.document().is_some_and(|d| d.hidden()) {
            pause(&p);
        } else {
            reconnect(&p);
        }
    })?;
    let p = page.clone();
    listen::<Event>(window.as_ref(), "focus", move |_| {
        if !p.borrow().active {
            reconnect(&p);
        }
    })?;
    let canvas = page.borrow().canvas.clone();
    let p = page.clone();
    listen::<PointerEvent>(canvas.as_ref(), "pointerdown", move |e| {
        e.prevent_default();
        if let Some(canvas) = e
            .current_target()
            .and_then(|t| t.dyn_into::<HtmlCanvasElement>().ok())
        {
            let _ = canvas.set_pointer_capture(e.pointer_id());
        }
        let mut p = p.borrow_mut();
        p.look = Some((e.pointer_id(), [e.client_x(), e.client_y()]));
        if let Some(session) = &mut p.session {
            let camera = session.camera;
            session
                .controls
                .button(true, true, &mut session.yaw, &camera);
        }
    })?;
    let p = page.clone();
    listen::<PointerEvent>(canvas.as_ref(), "pointermove", move |e| {
        let mut p = p.borrow_mut();
        let Some((id, previous)) = p.look else {
            return;
        };
        if id != e.pointer_id() {
            return;
        }
        let at = [e.client_x(), e.client_y()];
        p.look = Some((id, at));
        if let Some(session) = &mut p.session {
            session.controls.motion(
                [(at[0] - previous[0]) as f64, (at[1] - previous[1]) as f64],
                &mut session.yaw,
                &mut session.camera,
            );
        }
    })?;
    for event in ["pointerup", "pointercancel", "lostpointercapture"] {
        let p = page.clone();
        listen::<PointerEvent>(canvas.as_ref(), event, move |e| {
            let mut p = p.borrow_mut();
            if p.look.is_some_and(|(id, _)| id == e.pointer_id()) {
                p.look = None;
                if let Some(session) = &mut p.session {
                    let camera = session.camera;
                    session
                        .controls
                        .button(true, false, &mut session.yaw, &camera);
                }
            }
        })?;
    }
    reconnect(&page);
    animate(window, page);
    Ok(())
}
fn animate(window: Window, page: Rc<RefCell<Page>>) {
    let callback = Rc::new(RefCell::new(None::<Closure<dyn FnMut(f64)>>));
    let next = callback.clone();
    let scheduler = window.clone();
    *callback.borrow_mut() = Some(Closure::wrap(Box::new(move |now: f64| {
        let mut p = page.borrow_mut();
        let dt = ((now - p.last) / 1000.).clamp(0., 0.1) as f32;
        p.last = now;
        if p.active {
            if let Ok(pads) = p.window.navigator().get_gamepads() {
                if let Some(pad) = pads
                    .iter()
                    .find_map(|v| v.dyn_into::<web_sys::Gamepad>().ok())
                {
                    let axes = pad.axes();
                    let axis = |i| {
                        axes.get(i)
                            .as_f64()
                            .filter(|n| n.is_finite())
                            .unwrap_or(0.)
                            .clamp(-1., 1.)
                    };
                    for (name, down) in [
                        ("PadLeft", axis(0) < -0.2),
                        ("PadRight", axis(0) > 0.2),
                        ("PadForward", axis(1) < -0.2),
                        ("PadBackward", axis(1) > 0.2),
                    ] {
                        control(&mut p, name, down);
                    }
                    let look = [axis(2) as f32, axis(3) as f32];
                    if p.look.is_none()
                        && let Some(session) = &mut p.session
                    {
                        session.camera.yaw = (session.camera.yaw - look[0] * 1.9 * dt)
                            .rem_euclid(std::f32::consts::TAU);
                        session.camera.pitch =
                            (session.camera.pitch + look[1] * 1.15 * dt).clamp(-1.2, 1.2);
                        if look[0].abs() > 0.2 {
                            session.yaw = session.camera.yaw;
                        }
                    }
                    for i in 0..2 {
                        let down = pad
                            .buttons()
                            .get(i)
                            .dyn_into::<web_sys::GamepadButton>()
                            .is_ok_and(|b| b.pressed());
                        control(&mut p, &format!("Pad{i}"), down);
                    }
                } else {
                    for name in [
                        "PadLeft",
                        "PadRight",
                        "PadForward",
                        "PadBackward",
                        "Pad0",
                        "Pad1",
                    ] {
                        control(&mut p, name, false);
                    }
                }
            }
            let held = p.input.held();
            let source = p.source.clone();
            let result = if let Some(session) = &mut p.session {
                session.step(&source.scene, held).map(|_| ())
            } else {
                Ok(())
            };
            if let Err(error) = result {
                p.session.take();
                p.notice
                    .set_text_content(Some(&format!("{error}. Reconnect to recover.")));
            }
            let session_frame = p.session.as_ref().and_then(|s| {
                s.frame_in(
                    &source.pack,
                    &p.atlas,
                    &source.scene,
                    size(&p.window, &p.canvas),
                    [
                        p.canvas.client_width().max(1) as f32,
                        p.canvas.client_height().max(1) as f32,
                    ],
                )
                .ok()
            });
            if let Some(mut frame) = session_frame {
                let _ = frame.scale_overlay(size(&p.window, &p.canvas).map(|value| value as f32));
                let [w, h] = size(&p.window, &p.canvas);
                if p.canvas.width() != w {
                    p.canvas.set_width(w);
                }
                if p.canvas.height() != h {
                    p.canvas.set_height(h);
                }
                if let Some(engine) = &mut p.engine {
                    let result = engine.resize(w, h).and_then(|_| {
                        engine.draw(frame.view, &frame.instances, &frame.ui, &frame.lighting)
                    });
                    if let Err(error) = result {
                        p.engine.take();
                        p.session.take();
                        p.input.clear();
                        p.look = None;
                        p.notice.set_text_content(Some(&format!(
                            "{error}. Reconnect to reopen graphics."
                        )));
                    }
                }
            }
            if p.engine.is_some()
                && let Some(session) = &p.session
            {
                let damage = session.damage_events;
                let life = session.owned_life();
                let resources = session.hud().map(|h| {
                    format!(
                        "Health {}/{}; mana {}/{}",
                        h.resources.hp, h.resources.max_hp, h.resources.mana, h.resources.max_mana
                    )
                });
                if damage > p.damage {
                    let _ = p.audio.caption("impact", life, now / 1000.);
                    p.damage = damage;
                }
                if let Some(resources) = resources {
                    if p.notice.text_content().as_deref() != Some(&resources) {
                        p.notice.set_text_content(Some(&resources));
                    }
                }
            }
            let text = p
                .audio
                .captions(now / 1000.)
                .take(4)
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            if p.captions.text_content().as_deref() != Some(&text) {
                p.captions.set_text_content(Some(&text));
            }
        }
        drop(p);
        if let Some(callback) = next.borrow().as_ref() {
            let _ = scheduler.request_animation_frame(callback.as_ref().unchecked_ref());
        }
    }) as Box<dyn FnMut(f64)>));
    if let Some(callback) = callback.borrow().as_ref() {
        let _ = window.request_animation_frame(callback.as_ref().unchecked_ref());
    }
}
