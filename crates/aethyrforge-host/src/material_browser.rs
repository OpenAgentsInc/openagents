//! Scoped face drafts, local generation, and painting. Applying uses the existing confirmation path.
use super::*;
use base64::Engine as _;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData, PointerEvent};
#[derive(Clone)]
struct Work {
    document: MaterialDocument,
    generated: MaterialPixels,
    handmade: MaterialPixels,
    context: InputContext,
    owner: Option<String>,
}
#[derive(Default)]
pub(super) struct Controller {
    work: Option<Work>,
    serial: u64,
    previewing: Option<u64>,
    preview_feedback: String,
    dirty: bool,
    edited: bool,
    stale: bool,
    voice_candidate: bool,
    previous: Option<[f64; 2]>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceExport {
    schema: String,
    document: MaterialDocument,
    generated_png: String,
    handmade_png: String,
}
fn feedback_token() -> String {
    element("face-status")
        .get_attribute("data-feedback")
        .unwrap_or_default()
}
fn tell(message: &str) {
    let status = element("face-status");
    let next = feedback_token().parse::<u64>().unwrap_or(0).wrapping_add(1);
    let _ = status.set_attribute("data-feedback", &next.to_string());
    status.set_text_content(Some(message));
}
fn tell_current(token: &str, message: &str) {
    if feedback_token() == token {
        tell(message);
    }
}
fn canvas() -> HtmlCanvasElement {
    element("face-canvas").unchecked_into()
}
fn context(canvas: &HtmlCanvasElement) -> Result<CanvasRenderingContext2d, JsValue> {
    canvas
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("Painting is unavailable in this browser."))?
        .dyn_into()
        .map_err(Into::into)
}
fn temporary() -> Result<HtmlCanvasElement, JsValue> {
    let canvas: HtmlCanvasElement = window()
        .document()
        .unwrap()
        .create_element("canvas")?
        .unchecked_into();
    canvas.set_width(256);
    canvas.set_height(256);
    Ok(canvas)
}
fn put(canvas: &HtmlCanvasElement, pixels: &MaterialPixels) -> Result<(), JsValue> {
    context(canvas)?.put_image_data(
        &ImageData::new_with_u8_clamped_array_and_sh(wasm_bindgen::Clamped(&pixels.0), 256, 256)?,
        0.,
        0.,
    )
}
fn png(pixels: &MaterialPixels) -> Result<String, JsValue> {
    let canvas = temporary()?;
    put(&canvas, pixels)?;
    Ok(canvas
        .to_data_url_with_type("image/png")?
        .split_once(',')
        .ok_or(JsValue::NULL)?
        .1
        .into())
}
async fn pixels(encoded: &str) -> Result<MaterialPixels, JsValue> {
    if encoded.len() > 4 * 1024 * 1024 {
        return Err(JsValue::from_str(
            "This image is too large to open. Keep the exported file.",
        ));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| JsValue::from_str("Choose a valid PNG layer."))?;
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(bytes.as_slice()));
    let blob = web_sys::Blob::new_with_u8_array_sequence(&parts)?;
    let bitmap: web_sys::ImageBitmap =
        JsFuture::from(window().create_image_bitmap_with_blob(&blob)?)
            .await?
            .unchecked_into();
    if bitmap.width() != 256 || bitmap.height() != 256 {
        bitmap.close();
        return Err(JsValue::from_str("Use a 256 × 256 face image."));
    }
    let canvas = temporary()?;
    let ctx = context(&canvas)?;
    let result = ctx.draw_image_with_image_bitmap(&bitmap, 0., 0.);
    bitmap.close();
    result?;
    Ok(MaterialPixels(
        ctx.get_image_data(0., 0., 256., 256.)?.data().0,
    ))
}
fn encoded_layer(pixels: &MaterialPixels) -> Result<Option<MaterialImage>, JsValue> {
    if pixels.0.chunks_exact(4).all(|p| p[3] == 0) {
        return Ok(None);
    }
    let encoded = png(pixels)?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| JsValue::NULL)?;
    MaterialImage::new(&bytes).map(Some).map_err(|_|JsValue::from_str("This face PNG exceeds 16 KiB. Your layers are still here: export them, then reduce the colors or detail."))
}
fn bundle(work: &Work) -> Result<MaterialBundle, JsValue> {
    let generated = encoded_layer(&work.generated)?;
    let handmade = encoded_layer(&work.handmade)?;
    let mut document = work.document.clone();
    document.generated.image = generated.as_ref().map(|i| i.resource.clone());
    document.handmade.image = handmade.as_ref().map(|i| i.resource.clone());
    let bundle = MaterialBundle {
        document,
        generated,
        handmade,
    };
    bundle
        .validate()
        .map_err(|_| JsValue::from_str("Check the face layers before applying them."))?;
    Ok(bundle)
}
fn current(s: &State, work: &Work) -> bool {
    s.input_context().as_ref() == Some(&work.context)
        && work
            .owner
            .as_ref()
            .is_none_or(|a| s.account.as_ref() == Some(a))
}
fn redraw(work: &Work) -> Result<(), JsValue> {
    let g = if work.document.generated.visible {
        work.document.generated.strength
    } else {
        0.
    };
    let h = if work.document.handmade.visible {
        work.document.handmade.strength
    } else {
        0.
    };
    let pixels = work
        .generated
        .composite(&work.handmade, g, h)
        .map_err(|_| JsValue::NULL)?;
    put(&canvas(), &pixels)
}
fn controls(work: &Work) {
    let _ = element("face-controls").remove_attribute("disabled");
    let _ = element("face-canvas").set_attribute("aria-disabled", "false");
    for id in ["face-export", "face-discard"] {
        let _ = element(id).remove_attribute("disabled");
    }
    input("face-generated-visible").set_checked(work.document.generated.visible);
    input("face-handmade-visible").set_checked(work.document.handmade.visible);
    input("face-generated-strength").set_value_as_number(work.document.generated.strength);
    input("face-handmade-strength").set_value_as_number(work.document.handmade.strength);
    let _ = redraw(work);
}
fn changed(s: &mut State) {
    s.materials.serial = s.materials.serial.wrapping_add(1);
    s.materials.dirty = true;
    s.materials.edited = true;
    s.materials.stale = false;
    s.materials.voice_candidate = true;
    if s.projects.save.take().is_some() && !s.busy {
        hide("edit-auth", true);
    }
    projects::edited(s);
    if s.pending
        .as_ref()
        .is_some_and(|p| matches!(p.pending.action, Action::ApplyMaterial { .. }))
    {
        s.pending = None;
        s.persist();
        hide("edit-auth", true);
    }
}
fn failed(error: JsValue) {
    tell(
        error
            .as_string()
            .as_deref()
            .unwrap_or("The face layer could not change. Your sources are still here."),
    );
}
pub(super) async fn detail(
    weak: &Weak<RefCell<State>>,
    action: Action,
) -> Result<Outcome, JsValue> {
    let rc = weak.upgrade().ok_or(JsValue::NULL)?;
    let scope = projects::confirmation_scope(&rc.borrow()).ok_or(JsValue::NULL)?;
    let id = rc.borrow().request(&action)?;
    for _ in 0..300 {
        pause(50).await;
        let s = rc.borrow();
        if !scope.accepts(projects::confirmation_scope(&s).as_ref()) {
            return Err(JsValue::from_str(
                "The world changed. Open its current face material.",
            ));
        }
        if let Some(row) = s.snapshot()["receipts"]
            .as_array()
            .and_then(|r| r.iter().find(|r| r["request_id"] == id))
        {
            let outcome: Outcome =
                serde_json::from_value(row["outcome"].clone()).map_err(|_| JsValue::NULL)?;
            if outcome.state != ResultState::Applied {
                return Err(JsValue::from_str(&outcome.message));
            }
            return Ok(outcome);
        }
    }
    Err(JsValue::from_str(
        "The face preview took too long. Your applied material has not changed.",
    ))
}
fn token(s: &State, context: InputContext) -> Option<MaterialDraftToken> {
    Some(MaterialDraftToken {
        serial: s.materials.serial,
        context,
        scope: projects::confirmation_scope(s)?,
    })
}
fn accepts(s: &State, token: &MaterialDraftToken) -> bool {
    token.accepts(
        s.materials.serial,
        s.input_context().as_ref(),
        projects::confirmation_scope(s).as_ref(),
    )
}
async fn open(weak: Weak<RefCell<State>>) -> Option<MaterialDraftToken> {
    let rc = weak.upgrade()?;
    let (target, request, owner, pending) = {
        let mut s = rc.borrow_mut();
        let Some(context) = s.input_context() else {
            tell("Wait for the house to open.");
            return None;
        };
        let Some(selected) = &context.selection else {
            tell("Select a werewolf in the house first.");
            return None;
        };
        if selected.asset != "custom:haunt:werewolf" {
            tell("Face painting is available for the custom werewolf. Select one in the house.");
            return None;
        }
        if let Some(work) = &s.materials.work {
            if current(&s, work) {
                controls(work);
                tell("Your existing face draft is open. Preview or review it when you are ready.");
                return token(&s, context);
            }
            if s.materials.edited {
                tell(
                    "Your other face draft is still here. Export it, then choose Discard face draft before opening another face.",
                );
                return None;
            }
        }
        let target = MaterialTarget::werewolf_face(&selected.id);
        let pending = s
            .pending
            .as_ref()
            .and_then(|p| {
                if let Action::ApplyMaterial { material } = &p.pending.action {
                    Some(material.clone())
                } else {
                    None
                }
            })
            .filter(|m| m.document.target == target);
        s.materials.serial = s.materials.serial.wrapping_add(1);
        let request = token(&s, context)?;
        (target, request, s.account.clone(), pending)
    };
    let recovering_draft = pending.is_some();
    tell("Opening the face layers…");
    let feedback = feedback_token();
    let result = async {
        let material = if let Some(bundle) = pending {
            bundle
        } else {
            let outcome = detail(&weak, Action::InspectMaterial { target }).await?;
            let Some(Detail::Material { material }) = outcome.detail else {
                return Err(JsValue::NULL);
            };
            material
        };
        material.validate().map_err(|_| JsValue::NULL)?;
        let generated = match &material.generated {
            Some(image) => pixels(&image.png_base64).await?,
            None => MaterialPixels::default(),
        };
        let handmade = match &material.handmade {
            Some(image) => pixels(&image.png_base64).await?,
            None => MaterialPixels::default(),
        };
        Ok(Work {
            document: material.document,
            generated,
            handmade,
            context: request.context.clone(),
            owner,
        })
    }
    .await;
    let mut s = rc.borrow_mut();
    if !accepts(&s, &request) {
        return None;
    }
    match result {
        Ok(work) => {
            controls(&work);
            s.materials.work = Some(work);
            s.materials.edited = recovering_draft;
            s.materials.stale = false;
            tell_current(
                &feedback,
                "Paint on the color map, or describe scars. Apply your face draft before saving this world.",
            );
            Some(request)
        }
        Err(error) => {
            if feedback_token() == feedback {
                failed(error);
            }
            None
        }
    }
}
fn preview(rc: &Rc<RefCell<State>>) {
    {
        let mut s = rc.borrow_mut();
        s.materials.dirty = true;
        s.materials.preview_feedback = feedback_token();
    }
    start_preview(rc);
}
fn start_preview(rc: &Rc<RefCell<State>>) {
    let (work, serial, feedback) = {
        let mut s = rc.borrow_mut();
        let Some(work) = s.materials.work.clone() else {
            tell("Open a selected werewolf face first.");
            return;
        };
        if !current(&s, &work) {
            tell("The world or selection changed. Open the current face first.");
            return;
        }
        if s.materials.previewing.is_some() {
            return;
        }
        s.materials.previewing = Some(s.materials.serial);
        s.materials.dirty = false;
        (
            work,
            s.materials.serial,
            s.materials.preview_feedback.clone(),
        )
    };
    let weak = Rc::downgrade(rc);
    spawn_local(async move {
        let result = match bundle(&work) {
            Ok(material) => detail(&weak, Action::PreviewMaterial { material })
                .await
                .map(|_| ()),
            Err(e) => Err(e),
        };
        let Some(rc) = weak.upgrade() else { return };
        let mut s = rc.borrow_mut();
        if s.materials.previewing != Some(serial) {
            return;
        }
        s.materials.previewing = None;
        let same = s.materials.serial == serial
            && s.materials.work.as_ref().is_some_and(|w| current(&s, w));
        if same && feedback_token() == feedback {
            match result {
                Ok(()) => tell(
                    "Face preview ready. Review the change to apply it, or cancel the preview.",
                ),
                Err(e) => failed(e),
            }
        }
        let again = s.materials.dirty;
        drop(s);
        if again {
            start_preview(&rc);
        }
    });
}
fn generate(rc: &Rc<RefCell<State>>, request: &str) {
    let mut s = rc.borrow_mut();
    if s.busy {
        return;
    }
    let Some(work) = s.materials.work.as_ref() else {
        tell("Open a selected werewolf face first.");
        return;
    };
    if !current(&s, work) {
        tell("The selection changed. Open the current face first.");
        return;
    }
    let area = match element("face-area")
        .unchecked_into::<web_sys::HtmlSelectElement>()
        .value()
        .as_str()
    {
        "right_cheek" => ScarArea::RightCheek,
        "forehead" => ScarArea::Forehead,
        _ => ScarArea::LeftCheek,
    };
    let mut seed = [0u8; 4];
    let _ = window()
        .crypto()
        .unwrap()
        .get_random_values_with_u8_array(&mut seed);
    let recipe = match ScarRecipe::from_request(request, area, u32::from_le_bytes(seed)) {
        Ok(r) => r,
        Err(message) => {
            tell(message);
            say(message);
            return;
        }
    };
    let at = window().performance().unwrap().now();
    let generated = match generate_scars(&recipe) {
        Ok(p) => p,
        Err(_) => {
            tell("The local scar generator could not finish. Your previous layers are still here.");
            return;
        }
    };
    let elapsed = window().performance().unwrap().now() - at;
    let work = s.materials.work.as_mut().unwrap();
    work.generated = generated;
    work.document.recipe = Some(recipe);
    let _ = redraw(work);
    changed(&mut s);
    if elapsed < 1.0 {
        tell("Generated locally in under 1 ms. Preparing a preview…");
    } else {
        tell(&format!(
            "Generated locally in {elapsed:.0} ms. Preparing a preview…"
        ));
    }
    drop(s);
    say("Scars generated locally. Say Review change, then Confirm change to apply them.");
    preview(rc);
}
/// Text and recognized speech enter the same local generation handler.
pub(super) fn route(rc: &Rc<RefCell<State>>, request: &str) -> bool {
    if !request.to_ascii_lowercase().contains("scar") {
        return false;
    }
    let area = match element("face-area")
        .unchecked_into::<web_sys::HtmlSelectElement>()
        .value()
        .as_str()
    {
        "right_cheek" => ScarArea::RightCheek,
        "forehead" => ScarArea::Forehead,
        _ => ScarArea::LeftCheek,
    };
    if let Err(message) = ScarRecipe::from_request(request, area, 0) {
        tell(message);
        say(message);
        return true;
    }
    let ready = rc
        .borrow()
        .materials
        .work
        .as_ref()
        .is_some_and(|w| current(&rc.borrow(), w));
    if !ready {
        let selected = rc
            .borrow()
            .input_context()
            .and_then(|c| c.selection)
            .is_some_and(|s| s.asset == "custom:haunt:werewolf");
        if !selected {
            tell("Select a custom werewolf in the house first.");
            say("Select a custom werewolf in the house first.");
            return true;
        }
        let weak = Rc::downgrade(rc);
        let request = request.to_string();
        spawn_local(async move {
            let Some(request_token) = open(weak.clone()).await else {
                return;
            };
            if let Some(rc) = weak.upgrade() {
                if accepts(&rc.borrow(), &request_token)
                    && rc
                        .borrow()
                        .materials
                        .work
                        .as_ref()
                        .is_some_and(|w| current(&rc.borrow(), w))
                {
                    element("face-request")
                        .unchecked_into::<HtmlTextAreaElement>()
                        .set_value(&request);
                    {
                        let mut s = rc.borrow_mut();
                        s.visit.show("materials");
                        s.layout();
                    }
                    generate(&rc, &request);
                }
            }
        });
        return true;
    }
    element("face-request")
        .unchecked_into::<HtmlTextAreaElement>()
        .set_value(request);
    {
        let mut s = rc.borrow_mut();
        s.visit.show("materials");
        s.layout();
    }
    generate(rc, request);
    true
}
pub(super) fn review_voice(rc: &Rc<RefCell<State>>) -> bool {
    if !rc.borrow().materials.voice_candidate {
        return false;
    }
    review(rc)
}
pub(super) fn forget_voice(s: &mut State) {
    s.materials.voice_candidate = false;
}
pub(super) fn signed_in(s: &mut State) {
    if let Some(work) = s.materials.work.as_mut() {
        if work.owner.is_none() {
            work.owner = s.account.clone();
        }
    }
}
pub(super) fn review(rc: &Rc<RefCell<State>>) -> bool {
    let s = rc.borrow();
    if !s.materials.edited {
        return false;
    }
    let Some(work) = s.materials.work.as_ref() else {
        return false;
    };
    if !current(&s, work) {
        tell("The selection changed. Open the current face before applying it.");
        return true;
    }
    let result = bundle(work);
    drop(s);
    match result {
        Ok(material) => queue_change(rc, Action::ApplyMaterial { material }),
        Err(e) => failed(e),
    };
    true
}
fn stroke(rc: &Rc<RefCell<State>>, event: &PointerEvent, start: bool) {
    let mut s = rc.borrow_mut();
    if s.busy {
        return;
    }
    let Some(work) = s.materials.work.as_ref() else {
        return;
    };
    if !current(&s, work) {
        return;
    }
    if !start && s.materials.previous.is_none() {
        return;
    }
    let bounds = canvas().get_bounding_client_rect();
    let point = [
        ((event.client_x() as f64 - bounds.left()) / bounds.width()).clamp(0., 1.),
        ((event.client_y() as f64 - bounds.top()) / bounds.height()).clamp(0., 1.),
    ];
    let color = input("face-brush-color").value();
    let color =
        [1, 3, 5].map(|i| u8::from_str_radix(color.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0));
    let mode = if element("face-brush-mode")
        .unchecked_into::<web_sys::HtmlSelectElement>()
        .value()
        == "erase"
    {
        BrushMode::Erase
    } else {
        BrushMode::Paint
    };
    let points = if start {
        vec![point]
    } else {
        vec![s.materials.previous.unwrap(), point]
    };
    let brush = BrushStroke {
        points,
        radius: input("face-brush-size").value_as_number() / 256.,
        opacity: input("face-brush-opacity").value_as_number(),
        color,
        mode,
    };
    let work = s.materials.work.as_mut().unwrap();
    if work.handmade.stroke(&brush).is_err() {
        return;
    }
    let _ = redraw(work);
    s.materials.previous = Some(point);
    changed(&mut s);
    drop(s);
    preview(rc);
}
pub(super) fn clear(s: &mut State) {
    s.materials = Controller {
        serial: s.materials.serial.wrapping_add(1),
        ..Controller::default()
    };
    let _ = element("face-controls").set_attribute("disabled", "");
    let _ = element("face-canvas").set_attribute("aria-disabled", "true");
    for id in ["face-export", "face-discard"] {
        let _ = element(id).set_attribute("disabled", "");
    }
    let _ = put(&canvas(), &MaterialPixels::default());
    element("face-request")
        .unchecked_into::<HtmlTextAreaElement>()
        .set_value("");
    tell("Select a werewolf in the house to begin.");
}
pub(super) fn has_unsaved(s: &State) -> bool {
    s.materials.edited
}
pub(super) fn poll(rc: &Rc<RefCell<State>>) {
    let mut s = rc.borrow_mut();
    let Some(work) = s.materials.work.as_ref() else {
        return;
    };
    let valid = current(&s, work);
    if valid && s.materials.stale {
        controls(work);
        s.materials.stale = false;
        tell("Your face draft is ready to preview or review.");
    } else if !valid && !s.materials.stale {
        s.materials.stale = true;
        s.materials.previous = None;
        let _ = element("face-controls").set_attribute("disabled", "");
        let _ = element("face-canvas").set_attribute("aria-disabled", "true");
        tell(if s.materials.edited {
            "The world or selection changed. Your face draft is still here. Export it, then discard it to open the current face."
        } else {
            "The world or selection changed. Choose Edit selected werewolf face to continue painting."
        });
    }
}
pub(super) fn applied(s: &mut State) {
    s.materials.edited = false;
    s.materials.stale = false;
    s.materials.voice_candidate = false;
    let current = s.input_context();
    if let Some(work) = s.materials.work.as_mut() {
        if let Some(context) = current {
            work.context = context;
            work.owner = s.account.clone();
        }
    }
}
pub(super) fn mount(
    rc: &Rc<RefCell<State>>,
    listeners: &mut Vec<(EventTarget, &'static str, Handler)>,
) -> Result<(), JsValue> {
    let weak = Rc::downgrade(rc);
    listen(listeners, "face-open", "click", move |_| {
        let weak = weak.clone();
        spawn_local(async move {
            let _ = open(weak).await;
        });
    })?;
    let weak = Rc::downgrade(rc);
    listen(listeners, "face-generate", "click", move |_| {
        if let Some(rc) = weak.upgrade() {
            let text = element("face-request")
                .unchecked_into::<HtmlTextAreaElement>()
                .value();
            generate(&rc, &text);
        }
    })?;
    let weak = Rc::downgrade(rc);
    listen(listeners, "face-preview", "click", move |_| {
        if let Some(rc) = weak.upgrade() {
            preview(&rc);
        }
    })?;
    let weak = Rc::downgrade(rc);
    listen(listeners, "face-apply", "click", move |_| {
        if let Some(rc) = weak.upgrade() {
            review(&rc);
        }
    })?;
    let weak = Rc::downgrade(rc);
    listen(listeners, "face-discard", "click", move |_| {
        let Some(rc) = weak.upgrade() else { return };
        if rc.borrow().busy {
            return;
        }
        {
            let mut s = rc.borrow_mut();
            if s.pending
                .as_ref()
                .is_some_and(|p| matches!(p.pending.action, Action::ApplyMaterial { .. }))
            {
                s.pending = None;
                s.persist();
                hide("edit-auth", true);
            }
            clear(&mut s);
        }
        let serial = rc.borrow().materials.serial;
        let feedback = feedback_token();
        let weak = Rc::downgrade(&rc);
        spawn_local(async move {
            let result = detail(&weak, Action::ClearMaterialPreview).await;
            if let Some(rc) = weak.upgrade() {
                if rc.borrow().materials.serial == serial && feedback_token() == feedback {
                    match result {
                        Ok(_) => tell("Face draft discarded. The applied material is still here."),
                        Err(e) => failed(e),
                    }
                }
            }
        });
    })?;
    let weak = Rc::downgrade(rc);
    listen(listeners, "face-cancel", "click", move |_| {
        let Some(rc) = weak.upgrade() else { return };
        {
            let mut s = rc.borrow_mut();
            s.materials.serial = s.materials.serial.wrapping_add(1);
            s.materials.dirty = false;
        }
        tell("Closing the face preview…");
        let feedback = feedback_token();
        let weak = Rc::downgrade(&rc);
        spawn_local(async move {
            let result = detail(&weak, Action::ClearMaterialPreview).await;
            if feedback_token() == feedback {
                match result {
                    Ok(_) => tell("Applied face restored. Your editable layers are still here."),
                    Err(e) => failed(e),
                }
            }
        });
    })?;
    for id in [
        "face-generated-visible",
        "face-handmade-visible",
        "face-generated-strength",
        "face-handmade-strength",
    ] {
        let weak = Rc::downgrade(rc);
        listen(listeners, id, "input", move |_| {
            let Some(rc) = weak.upgrade() else { return };
            let mut s = rc.borrow_mut();
            if s.busy {
                return;
            }
            let Some(work) = s.materials.work.as_mut() else {
                return;
            };
            work.document.generated.visible = input("face-generated-visible").checked();
            work.document.handmade.visible = input("face-handmade-visible").checked();
            work.document.generated.strength = input("face-generated-strength").value_as_number();
            work.document.handmade.strength = input("face-handmade-strength").value_as_number();
            let _ = redraw(work);
            changed(&mut s);
            drop(s);
            preview(&rc);
        })?;
    }
    for (name, start) in [("pointerdown", true), ("pointermove", false)] {
        let weak = Rc::downgrade(rc);
        listen(listeners, "face-canvas", name, move |event| {
            let Some(rc) = weak.upgrade() else { return };
            if let Some(pointer) = event.dyn_ref::<PointerEvent>() {
                event.prevent_default();
                if start {
                    let _ = canvas().set_pointer_capture(pointer.pointer_id());
                }
                stroke(&rc, pointer, start);
            }
        })?;
    }
    for name in ["pointerup", "pointercancel", "lostpointercapture"] {
        let weak = Rc::downgrade(rc);
        listen(listeners, "face-canvas", name, move |_| {
            if let Some(rc) = weak.upgrade() {
                rc.borrow_mut().materials.previous = None;
            }
        })?;
    }
    let weak = Rc::downgrade(rc);
    listen(listeners, "face-export", "click", move |_| {
        let Some(rc) = weak.upgrade() else { return };
        let s = rc.borrow();
        let Some(work) = &s.materials.work else {
            return;
        };
        let context = s.input_context();
        if !crate::material::material_source_exportable(
            work.owner.as_deref(),
            s.account.as_deref(),
            &work.context.project,
            context.as_ref().map(|context| &context.project),
        ) {
            tell("Open this draft in its original world before exporting it.");
            return;
        }
        let result = (|| {
            let mut document = work.document.clone();
            document.generated.image = None;
            document.handmade.image = None;
            let source = SourceExport {
                schema: "openagents.verse.material.source-export.v1".into(),
                document,
                generated_png: png(&work.generated)?,
                handmade_png: png(&work.handmade)?,
            };
            let bytes = serde_json::to_vec_pretty(&source).map_err(|_| JsValue::NULL)?;
            let a = window().document().unwrap().create_element("a")?;
            a.set_attribute(
                "href",
                &format!(
                    "data:application/json;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(bytes)
                ),
            )?;
            a.set_attribute("download", "werewolf-face-layers.json")?;
            a.unchecked_into::<web_sys::HtmlElement>().click();
            Ok::<_, JsValue>(())
        })();
        if let Err(e) = result {
            failed(e)
        } else {
            tell("Editable face layers exported, including generated and handmade sources.");
        }
    })?;
    let weak = Rc::downgrade(rc);
    listen(listeners, "face-import", "change", move |_| {
        let Some(file) = input("face-import").files().and_then(|f| f.get(0)) else {
            return;
        };
        let weak = weak.clone();
        spawn_local(async move {
            let Some(rc) = weak.upgrade() else { return };
            let context = rc.borrow().input_context();
            let Some(context) = context else { return };
            let Some(selected) = context
                .selection
                .as_ref()
                .filter(|s| s.asset == "custom:haunt:werewolf")
            else {
                tell("Select a werewolf before opening its face layers.");
                return;
            };
            let target = MaterialTarget::werewolf_face(&selected.id);
            let owner = rc.borrow().account.clone();
            let request = {
                let mut s = rc.borrow_mut();
                s.materials.serial = s.materials.serial.wrapping_add(1);
                token(&s, context.clone())
            };
            let Some(request) = request else { return };
            tell("Opening face sources…");
            let feedback = feedback_token();
            let result = async {
                if file.size() > 2. * 1024. * 1024. {
                    return Err(JsValue::from_str("Choose a face-layer export up to 2 MiB."));
                }
                let text = JsFuture::from(file.text())
                    .await?
                    .as_string()
                    .ok_or(JsValue::NULL)?;
                let mut source: SourceExport = serde_json::from_str(&text)
                    .map_err(|_| JsValue::from_str("Choose an editable face-layer export."))?;
                if source.schema != "openagents.verse.material.source-export.v1" {
                    return Err(JsValue::from_str("Choose an editable face-layer export."));
                }
                source.document.validate().map_err(|_| JsValue::NULL)?;
                source.document.target = target;
                let generated = pixels(&source.generated_png).await?;
                let handmade = pixels(&source.handmade_png).await?;
                Ok(Work {
                    document: source.document,
                    generated,
                    handmade,
                    context: context.clone(),
                    owner: owner.clone(),
                })
            }
            .await;
            let mut s = rc.borrow_mut();
            if !accepts(&s, &request) {
                return;
            }
            match result {
                Ok(work) => {
                    controls(&work);
                    s.materials.work = Some(work);
                    changed(&mut s);
                    tell_current(
                        &feedback,
                        "Face sources opened. Preview and review them before applying.",
                    );
                }
                Err(e) => {
                    if feedback_token() == feedback {
                        failed(e);
                    }
                }
            }
        });
    })?;
    Ok(())
}
