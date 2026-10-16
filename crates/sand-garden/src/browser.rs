//! Browser events and the gallery, with editor state owned by Rust.
use crate::{
    Action, Drawing, Gallery, HEIGHT, Light, MAX_ACTIONS, MAX_POINTS, Point, Tool, WIDTH,
    paint::Painter,
};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    Element, Event, HtmlCanvasElement, HtmlDialogElement, HtmlInputElement, KeyboardEvent,
    PointerEvent, Request, RequestInit, Response,
};
type App = Rc<RefCell<Editor>>;
const DRAFT: &str = "blue-rush-sand-v1";
const TOOLS: &[&str] = &[
    "rake", "finger", "smooth", "water", "select", "conch", "stone", "glass", "tooth", "anemone",
];
struct Editor {
    canvas: HtmlCanvasElement,
    painter: Painter,
    drawing: Drawing,
    tool: Tool,
    size: f64,
    angle: f64,
    active: Vec<usize>,
    normals: Vec<Option<(f64, f64)>>,
    pointer: Option<i32>,
    mandala: bool,
    history: Vec<Drawing>,
    redo: Vec<Drawing>,
    color: String,
    palette: Vec<(Tool, String)>,
    selected: Option<usize>,
    drag_offset: Point,
    moving: bool,
    visitors: crate::visitors::Visitors,
    visitor_text: String,
    request: String,
    offset: usize,
    busy: bool,
    keyboard: Point,
}
fn doc() -> web_sys::Document {
    web_sys::window().unwrap().document().unwrap()
}
fn el(id: &str) -> Element {
    doc().get_element_by_id(id).unwrap()
}
fn text(id: &str, s: &str) {
    el(id).set_text_content(Some(s));
}
fn listen(
    target: &web_sys::EventTarget,
    name: &str,
    f: impl FnMut(Event) + 'static,
) -> Result<(), JsValue> {
    let c = Closure::wrap(Box::new(f) as Box<dyn FnMut(Event)>);
    target.add_event_listener_with_callback(name, c.as_ref().unchecked_ref())?;
    c.forget();
    Ok(())
}
fn button(id: &str, f: impl FnMut(Event) + 'static) -> Result<(), JsValue> {
    listen(el(id).as_ref(), "click", f)
}
fn open(id: &str) {
    let d: HtmlDialogElement = el(id).unchecked_into();
    let _ = d.show_modal();
}
fn close(id: &str) {
    let d: HtmlDialogElement = el(id).unchecked_into();
    d.close();
}
fn node(tag: &str, class: &str, text: &str) -> Element {
    let e = doc().create_element(tag).unwrap();
    e.set_class_name(class);
    e.set_text_content(Some(text));
    e
}
fn checkpoint(s: &mut Editor) {
    s.history.push(s.drawing.clone());
    if s.history.len() > 60 {
        s.history.remove(0);
    }
    s.redo.clear();
    s.request.clear();
}
fn present(s: &Editor) {
    s.painter.present(
        if s.drawing.visitors {
            &s.visitors.guests
        } else {
            &[]
        },
        s.selected.and_then(|i| s.drawing.actions.get(i)),
    );
}
fn settings(s: &Editor) {
    let input: HtmlInputElement = el("br-color").unchecked_into();
    input.set_value(&s.color);
    el("br-size")
        .unchecked_into::<HtmlInputElement>()
        .set_value(
            &s.selected
                .and_then(|i| s.drawing.actions.get(i))
                .map_or(s.size, |a| a.size)
                .to_string(),
        );
    input.set_disabled(!(s.tool.placeable() || s.tool == Tool::Water || s.selected.is_some()));
    text(
        "br-selected",
        &s.selected
            .and_then(|i| s.drawing.actions.get(i))
            .map_or_else(
                || "Color the next find or water".to_owned(),
                |a| format!("{} selected · Drag to move", a.tool.label()),
            ),
    );
    let _ = el("br-sun").set_attribute(
        "aria-pressed",
        if s.drawing.light == Light::Sun {
            "true"
        } else {
            "false"
        },
    );
    let _ = el("br-shade").set_attribute(
        "aria-pressed",
        if s.drawing.light == Light::Shade {
            "true"
        } else {
            "false"
        },
    );
    let _ = el("br-visitors").set_attribute(
        "aria-pressed",
        if s.drawing.visitors { "true" } else { "false" },
    );
    let water = (s.painter.water_ratio() * 100.).round() as usize;
    text(
        "br-habitat",
        &format!("{}% sand · {}% water", 100 - water, water),
    );
}
fn save(s: &Editor) {
    let saved = web_sys::window()
        .unwrap()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|storage| {
            serde_json::to_string(&s.drawing)
                .ok()
                .map(|json| storage.set_item(DRAFT, &json).is_ok())
        })
        .unwrap_or(false);
    text(
        "br-status",
        if s.drawing.points() >= MAX_POINTS || s.drawing.actions.len() >= MAX_ACTIONS {
            "Your design is full. Undo a mark to keep drawing."
        } else if saved {
            "Saved on this device"
        } else {
            "Keep this tab open or save an image."
        },
    );
    let _ = el("br-undo").toggle_attribute_with_force("disabled", s.history.is_empty());
    let _ = el("br-redo").toggle_attribute_with_force("disabled", s.redo.is_empty());
    settings(s);
    present(s);
}
fn point(c: &HtmlCanvasElement, e: &PointerEvent) -> Point {
    let r = c.get_bounding_client_rect();
    Point::new(
        (e.client_x() as f64 - r.left()) / r.width() * WIDTH,
        (e.client_y() as f64 - r.top()) / r.height() * HEIGHT,
    )
}
fn rotated(p: Point, k: usize, ways: usize) -> Point {
    let a = std::f64::consts::TAU * k as f64 / ways as f64;
    let (s, c) = a.sin_cos();
    Point::new(
        WIDTH / 2. + (p.x - WIDTH / 2.) * c - (p.y - HEIGHT / 2.) * s,
        HEIGHT / 2. + (p.x - WIDTH / 2.) * s + (p.y - HEIGHT / 2.) * c,
    )
}
fn begin(s: &mut Editor, p: Point) {
    if s.tool == Tool::Select {
        s.selected = s
            .drawing
            .actions
            .iter()
            .enumerate()
            .rev()
            .find(|(_, a)| {
                a.tool.placeable()
                    && a.points
                        .first()
                        .is_some_and(|q| (q.x - p.x).hypot(q.y - p.y) < 36. * a.size)
            })
            .map(|(i, _)| i);
        s.moving = false;
        if let Some(i) = s.selected {
            let a = &s.drawing.actions[i];
            s.drag_offset = Point {
                x: p.x - a.points[0].x,
                y: p.y - a.points[0].y,
            };
            s.color = a.color.clone().unwrap_or_else(|| a.tool.color().into());
        }
        settings(s);
        present(s);
        return;
    }
    s.selected = None;
    if s.drawing.actions.len() >= MAX_ACTIONS || s.drawing.points() >= MAX_POINTS {
        return;
    }
    s.redo.clear();
    s.request.clear();
    s.active.clear();
    s.normals.clear();
    let ways = if s.mandala { 6 } else { 1 };
    if s.drawing.actions.len() + ways > MAX_ACTIONS || s.drawing.points() + ways > MAX_POINTS {
        return;
    }
    checkpoint(s);
    for k in 0..ways {
        let a = Action {
            tool: s.tool,
            size: s.size,
            angle: (s.angle + std::f64::consts::TAU * k as f64 / ways as f64)
                % std::f64::consts::TAU,
            points: vec![rotated(p, k, ways)],
            color: Some(s.color.clone()),
        };
        s.painter.action(&a);
        s.active.push(s.drawing.actions.len());
        s.normals.push(None);
        s.drawing.actions.push(a);
    }
}
fn extend(s: &mut Editor, p: Point) {
    if s.tool == Tool::Select {
        if let Some(i) = s.selected {
            let next = Point::new(p.x - s.drag_offset.x, p.y - s.drag_offset.y);
            let old = s.drawing.actions[i].points[0];
            if (next.x - old.x).hypot(next.y - old.y) < 1.5 {
                return;
            }
            if !s.moving {
                checkpoint(s);
                s.moving = true;
            }
            s.drawing.actions[i].points[0] = next;
            s.painter.update_objects(&s.drawing);
            present(s);
        }
        return;
    }
    if s.active.is_empty() || !s.tool.draws() || s.drawing.points() + s.active.len() > MAX_POINTS {
        return;
    }
    let ways = s.active.len();
    // Subpixel pointer samples are redundant; there is no drag-start delay.
    let last = s.drawing.actions[s.active[0]].points.last().unwrap();
    if (p.x - last.x).hypot(p.y - last.y) < 1.5 {
        return;
    }
    for k in 0..ways {
        let q = rotated(p, k, ways);
        let a = &mut s.drawing.actions[s.active[k]];
        let last = *a.points.last().unwrap();
        s.normals[k] = Some(s.painter.extend(a, last, q, s.normals[k]));
        a.points.push(q);
    }
    present(s);
}
fn end(s: &mut Editor) {
    s.active.clear();
    s.normals.clear();
    s.pointer = None;
    save(s);
}
fn api() -> String {
    if web_sys::window()
        .unwrap()
        .location()
        .pathname()
        .unwrap_or_default()
        .starts_with("/studios/")
    {
        "/studios/blue-rush/garden/api".into()
    } else {
        "/garden/api".into()
    }
}
async fn fetch(path: &str, body: Option<serde_json::Value>) -> Result<serde_json::Value, String> {
    let opts = RequestInit::new();
    opts.set_method(if body.is_some() { "POST" } else { "GET" });
    if let Some(body) = body {
        opts.set_body(&JsValue::from_str(&body.to_string()));
    }
    let r = Request::new_with_str_and_init(&format!("{}{path}", api()), &opts)
        .map_err(|_| "Couldn't connect. Try again.")?;
    r.headers()
        .set("Content-Type", "application/json")
        .map_err(|_| "Couldn't connect. Try again.")?;
    r.headers()
        .set("x-sand-garden", "1")
        .map_err(|_| "Couldn't connect. Try again.")?;
    let response: Response = JsFuture::from(web_sys::window().unwrap().fetch_with_request(&r))
        .await
        .map_err(|_| "Couldn't connect. Try again.")?
        .unchecked_into();
    let body = JsFuture::from(response.text().map_err(|_| "Couldn't read the gallery.")?)
        .await
        .map_err(|_| "Couldn't read the gallery.")?
        .as_string()
        .unwrap_or_default();
    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|_| "The gallery couldn't open. Try again.")?;
    if !response.ok() {
        return Err(json["error"]
            .as_str()
            .unwrap_or("That didn't work. Try again.")
            .into());
    }
    Ok(json)
}
fn busy(s: &App, on: bool) {
    s.borrow_mut().busy = on;
    for id in ["br-publish", "br-gallery-prev", "br-gallery-next"] {
        let _ = el(id).toggle_attribute_with_force("disabled", on);
    }
}
fn gallery(s: App) {
    if s.borrow().busy {
        return;
    }
    busy(&s, true);
    text("br-gallery-status", "Loading creations…");
    let offset = s.borrow().offset;
    spawn_local(async move {
        let result = fetch(&format!("?offset={offset}"), None).await;
        busy(&s, false);
        match result.and_then(|v| {
            serde_json::from_value::<Gallery>(v)
                .map_err(|_| "The gallery couldn't open. Try again.".into())
        }) {
            Ok(g) => render_gallery(s, g),
            Err(e) => text("br-gallery-status", &e),
        }
    });
}
fn render_gallery(s: App, g: Gallery) {
    let grid = el("br-gallery-grid");
    grid.set_text_content(None);
    s.borrow_mut().offset = g.offset;
    text(
        "br-gallery-status",
        if g.total == 0 {
            "A fresh stretch of beach. Share the first design."
        } else {
            "Most loved first. New creations start at the bottom."
        },
    );
    text(
        "br-gallery-page",
        &if g.total == 0 {
            "0 creations".into()
        } else {
            format!(
                "{}–{} of {}",
                g.offset + 1,
                g.offset + g.items.len(),
                g.total
            )
        },
    );
    let _ = el("br-gallery-prev").toggle_attribute_with_force("disabled", g.offset == 0);
    let _ = el("br-gallery-next")
        .toggle_attribute_with_force("disabled", g.offset + g.items.len() >= g.total);
    for item in g.items {
        let card = node("article", "br-creation", "");
        let (preview, ctx) = crate::paint::canvas().unwrap();
        let _ = preview.set_attribute("role", "img");
        let _ = preview.set_attribute("aria-label", &item.title);
        preview.set_class_name("br-preview");
        if let Ok(p) = Painter::new(ctx) {
            p.redraw(&item.drawing);
        }
        let _ = card.append_child(&preview);
        let _ = card.append_child(&node("h4", "", &item.title));
        let actions = node("div", "br-creation-actions", "");
        let vote = node(
            "button",
            "br-tool br-vote",
            &format!("{} {}", if item.voted { "♥" } else { "♡" }, item.votes),
        );
        let _ = vote.set_attribute("type", "button");
        let _ = vote.set_attribute("aria-label", &format!("Upvote {}", item.title));
        let _ = vote.set_attribute("aria-pressed", if item.voted { "true" } else { "false" });
        let _ = vote.set_attribute("data-id", &item.id);
        let _ = vote.set_attribute("data-vote", if item.voted { "false" } else { "true" });
        let _ = actions.append_child(&vote);
        if item.can_delete {
            let delete = node("button", "br-tool br-delete", "Delete");
            let _ = delete.set_attribute("type", "button");
            let _ = delete.set_attribute("data-id", &item.id);
            let _ = delete.set_attribute("data-delete", "true");
            let _ = actions.append_child(&delete);
        }
        let _ = card.append_child(&actions);
        let _ = grid.append_child(&card);
    }
}
pub fn start() -> Result<(), JsValue> {
    let canvas: HtmlCanvasElement = el("br-sand").dyn_into()?;
    canvas.set_width(WIDTH as u32);
    canvas.set_height(HEIGHT as u32);
    let ctx = canvas.get_context("2d")?.unwrap().dyn_into()?;
    let painter = Painter::new(ctx)?;
    let draft = web_sys::window()
        .unwrap()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|s| s.get_item(DRAFT).ok().flatten())
        .and_then(|v| serde_json::from_str::<Drawing>(&v).ok())
        .filter(Drawing::within_limits)
        .unwrap_or_default();
    painter.redraw(&draft);
    let history = (0..draft.actions.len())
        .map(|n| Drawing {
            actions: draft.actions[..n].to_vec(),
            ..draft.clone()
        })
        .collect();
    let app = Rc::new(RefCell::new(Editor {
        canvas: canvas.clone(),
        painter,
        drawing: draft,
        tool: Tool::Rake,
        size: 1.,
        angle: 0.,
        active: vec![],
        normals: vec![],
        pointer: None,
        mandala: false,
        history,
        redo: vec![],
        color: Tool::Rake.color().into(),
        palette: vec![],
        selected: None,
        drag_offset: Point::new(0., 0.),
        moving: false,
        visitors: crate::visitors::Visitors::new((js_sys::Math::random() * u32::MAX as f64) as u32),
        visitor_text: String::new(),
        request: String::new(),
        offset: 0,
        busy: false,
        keyboard: Point::new(WIDTH / 2., HEIGHT / 2.),
    }));
    text("br-status", "Make yourself a little stretch of beach.");
    el("br-garden-tools").remove_attribute("hidden")?;
    for &tool in TOOLS {
        let app = app.clone();
        let tool = Tool::parse(tool).unwrap();
        let id = format!(
            "br-tool-{}",
            serde_json::to_value(tool).unwrap().as_str().unwrap()
        );
        let b = el(&id);
        // The tool swatches use the same painter as the objects placed in sand.
        if tool.placeable() {
            let (icon, ctx) = crate::paint::canvas()?;
            icon.set_class_name("br-tool-art");
            icon.set_attribute("aria-hidden", "true")?;
            let p = Painter::new(ctx)?;
            p.ctx.clear_rect(0., 0., WIDTH, HEIGHT);
            p.object(&Action {
                tool,
                size: 1.8,
                angle: -0.3,
                points: vec![Point::new(WIDTH / 2., HEIGHT / 2.)],
                color: None,
            });
            // Crop the full-size drawing to an icon without stretching its shape.
            let thumb: HtmlCanvasElement = doc().create_element("canvas")?.dyn_into()?;
            thumb.set_width(80);
            thumb.set_height(64);
            thumb.set_class_name("br-tool-art");
            thumb.set_attribute("aria-hidden", "true")?;
            let t: web_sys::CanvasRenderingContext2d =
                thumb.get_context("2d")?.unwrap().dyn_into()?;
            t.draw_image_with_html_canvas_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                &icon,
                WIDTH / 2. - 64.,
                HEIGHT / 2. - 50.,
                128.,
                100.,
                0.,
                0.,
                80.,
                64.,
            )?;
            b.insert_before(&thumb, b.first_child().as_ref())?;
        }
        listen(b.as_ref(), "click", move |_| {
            let mut s = app.borrow_mut();
            end(&mut s);
            s.tool = tool;
            s.selected = None;
            s.color = s
                .palette
                .iter()
                .find(|(t, _)| *t == tool)
                .map(|(_, c)| c.clone())
                .unwrap_or_else(|| tool.color().into());
            for &t in TOOLS {
                let _ = el(&format!("br-tool-{t}")).set_attribute("aria-pressed", "false");
            }
            let _ = el(&id).set_attribute("aria-pressed", "true");
            text(
                "br-status",
                if tool == Tool::Select {
                    "Tap a find to recolor or move it."
                } else if tool == Tool::Water {
                    "Paint water for a tide pool. Smooth brushes it away."
                } else if tool.draws() {
                    "Drag to shape the sand."
                } else {
                    "Tap the sand to place it. Use Undo to take it back."
                },
            );
            settings(&s);
            present(&s);
        })?;
    }
    {
        let a = app.clone();
        listen(canvas.as_ref(), "pointerdown", move |e| {
            let e: PointerEvent = e.unchecked_into();
            if e.button() != 0 {
                return;
            }
            e.prevent_default();
            let mut s = a.borrow_mut();
            if s.pointer.is_some() {
                return;
            }
            let p = point(&s.canvas, &e);
            let _ = s.canvas.set_pointer_capture(e.pointer_id());
            s.pointer = Some(e.pointer_id());
            begin(&mut s, p);
        })?;
    }
    {
        let a = app.clone();
        listen(canvas.as_ref(), "pointermove", move |e| {
            let e: PointerEvent = e.unchecked_into();
            let mut s = a.borrow_mut();
            if s.pointer != Some(e.pointer_id()) {
                return;
            }
            e.prevent_default();
            if js_sys::Reflect::has(&e, &JsValue::from_str("getCoalescedEvents")).unwrap_or(false) {
                for v in e.get_coalesced_events().iter() {
                    let sample: PointerEvent = v.unchecked_into();
                    let p = point(&s.canvas, &sample);
                    extend(&mut s, p);
                }
            }
            let p = point(&s.canvas, &e);
            extend(&mut s, p);
        })?;
    }
    for name in ["pointerup", "pointercancel", "lostpointercapture"] {
        let a = app.clone();
        listen(canvas.as_ref(), name, move |e| {
            let e: PointerEvent = e.unchecked_into();
            let mut s = a.borrow_mut();
            if s.pointer != Some(e.pointer_id()) {
                return;
            }
            if e.type_() == "pointerup" {
                let p = point(&s.canvas, &e);
                extend(&mut s, p);
            }
            end(&mut s);
        })?;
    }
    {
        let a = app.clone();
        listen(canvas.as_ref(), "keydown", move |e| {
            let e: KeyboardEvent = e.unchecked_into();
            let mut s = a.borrow_mut();
            let delta = if e.shift_key() { 18. } else { 6. };
            let mut p = s.keyboard;
            match e.key().as_str() {
                "ArrowLeft" => p.x -= delta,
                "ArrowRight" => p.x += delta,
                "ArrowUp" => p.y -= delta,
                "ArrowDown" => p.y += delta,
                " " | "Enter" => {
                    e.prevent_default();
                    if !e.repeat() {
                        if s.active.is_empty() {
                            begin(&mut s, p);
                        } else {
                            end(&mut s);
                        }
                    }
                    return;
                }
                "Escape" => {
                    end(&mut s);
                    return;
                }
                _ => return,
            }
            e.prevent_default();
            s.keyboard = Point::new(p.x, p.y);
            let p = s.keyboard;
            extend(&mut s, p);
            if s.tool == Tool::Select {
                save(&s);
            }
            text(
                "br-status",
                &format!(
                    "{}: {} across, {} down. Space starts or stops a mark.",
                    s.tool.label(),
                    p.x as i32,
                    p.y as i32
                ),
            );
        })?;
    }
    {
        let a = app.clone();
        listen(el("br-size").as_ref(), "input", move |_| {
            if let Ok(v) = el("br-size")
                .unchecked_into::<HtmlInputElement>()
                .value()
                .parse::<f64>()
            {
                let mut s = a.borrow_mut();
                s.size = v;
                if let Some(i) = s.selected {
                    checkpoint(&mut s);
                    s.drawing.actions[i].size = v;
                    s.painter.update_objects(&s.drawing);
                    save(&s);
                }
            }
        })?;
    }
    {
        let a = app.clone();
        button("br-rotate", move |_| {
            let mut s = a.borrow_mut();
            s.angle = (s.angle + std::f64::consts::PI / 6.) % std::f64::consts::TAU;
            if let Some(i) = s.selected {
                checkpoint(&mut s);
                s.drawing.actions[i].angle = (s.drawing.actions[i].angle
                    + std::f64::consts::PI / 6.)
                    % std::f64::consts::TAU;
                s.painter.update_objects(&s.drawing);
                save(&s);
                return;
            }
            text(
                "br-status",
                &format!(
                    "Next beach find turned {} degrees.",
                    s.angle.to_degrees().round()
                ),
            );
        })?;
    }
    {
        let a = app.clone();
        button("br-mandala", move |_| {
            let mut s = a.borrow_mut();
            s.mandala = !s.mandala;
            let _ = el("br-mandala")
                .set_attribute("aria-pressed", if s.mandala { "true" } else { "false" });
        })?;
    }
    {
        let a = app.clone();
        button("br-undo", move |_| {
            let mut s = a.borrow_mut();
            end(&mut s);
            if let Some(previous) = s.history.pop() {
                let current = std::mem::replace(&mut s.drawing, previous);
                s.redo.push(current);
                s.selected = None;
                s.request.clear();
                s.painter.redraw(&s.drawing);
                save(&s);
            }
        })?;
    }
    {
        let a = app.clone();
        button("br-redo", move |_| {
            let mut s = a.borrow_mut();
            end(&mut s);
            if let Some(next) = s.redo.pop() {
                let current = std::mem::replace(&mut s.drawing, next);
                s.history.push(current);
                s.selected = None;
                s.request.clear();
                s.painter.redraw(&s.drawing);
                save(&s);
            }
        })?;
    }
    button("br-clear", move |_| open("br-clear-card"))?;
    {
        let a = app.clone();
        button("br-clear-confirm", move |_| {
            let mut s = a.borrow_mut();
            s.drawing = Drawing::default();
            s.history.clear();
            s.redo.clear();
            s.selected = None;
            s.visitors.clear();
            end(&mut s);
            s.request.clear();
            s.painter.redraw(&s.drawing);
            close("br-clear-card");
        })?;
    }
    for (id, dialog) in [
        ("br-gallery-close", "br-gallery"),
        ("br-share-close", "br-share-card"),
        ("br-clear-cancel", "br-clear-card"),
    ] {
        button(id, move |_| close(dialog))?;
    }
    {
        let a = app.clone();
        button("br-gallery-open", move |_| {
            open("br-gallery");
            gallery(a.clone());
        })?;
    }
    {
        let a = app.clone();
        listen(el("br-gallery-grid").as_ref(), "click", move |e| {
            if a.borrow().busy {
                return;
            }
            let Some(target) = e
                .target()
                .and_then(|t| t.dyn_into::<Element>().ok())
                .and_then(|t| t.closest("button[data-id]").ok().flatten())
            else {
                return;
            };
            let Some(id) = target.get_attribute("data-id") else {
                return;
            };
            let (path, body) = if let Some(vote) = target.get_attribute("data-vote") {
                (
                    format!("/{id}/vote"),
                    serde_json::json!({"voted":vote=="true"}),
                )
            } else {
                if !target.has_attribute("data-confirm") {
                    let _ = target.set_attribute("data-confirm", "true");
                    target.set_text_content(Some("Delete for everyone?"));
                    return;
                }
                (format!("/{id}/delete"), serde_json::json!({}))
            };
            busy(&a, true);
            let a = a.clone();
            spawn_local(async move {
                let result = fetch(&path, Some(body)).await;
                busy(&a, false);
                match result {
                    Ok(_) => {
                        if path.ends_with("/delete") {
                            a.borrow_mut().offset = 0;
                        }
                        gallery(a);
                    }
                    Err(e) => text("br-gallery-status", &e),
                }
            });
        })?;
    }
    {
        let a = app.clone();
        button("br-gallery-prev", move |_| {
            let n = a.borrow().offset.saturating_sub(12);
            a.borrow_mut().offset = n;
            gallery(a.clone());
        })?;
    }
    {
        let a = app.clone();
        button("br-gallery-next", move |_| {
            a.borrow_mut().offset += 12;
            gallery(a.clone());
        })?;
    }
    {
        let a = app.clone();
        button("br-gallery-retry", move |_| gallery(a.clone()))?;
    }
    {
        let a = app.clone();
        button("br-share-open", move |_| {
            if a.borrow().drawing.actions.is_empty() {
                text("br-status", "Make a design before sharing it.");
                return;
            }
            text("br-share-status", "");
            open("br-share-card");
            let preview: HtmlCanvasElement = el("br-share-preview").unchecked_into();
            preview.set_width(WIDTH as u32);
            preview.set_height(HEIGHT as u32);
            let ctx: web_sys::CanvasRenderingContext2d =
                preview.get_context("2d").unwrap().unwrap().unchecked_into();
            if let Ok(p) = Painter::new(ctx) {
                p.redraw(&a.borrow().drawing);
            }
        })?;
    }
    {
        let a = app.clone();
        button("br-publish", move |_| {
            if a.borrow().busy {
                return;
            }
            let title = el("br-title").unchecked_into::<HtmlInputElement>().value();
            if title.trim().is_empty() {
                text("br-share-status", "Give your design a name.");
                return;
            }
            {
                let mut s = a.borrow_mut();
                if s.request.is_empty() {
                    s.request = (0..4)
                        .map(|_| {
                            format!("{:08x}", (js_sys::Math::random() * u32::MAX as f64) as u32)
                        })
                        .collect();
                }
            }
            let body = {
                let s = a.borrow();
                serde_json::json!({"title":title.trim(),"drawing":s.drawing,"request_id":s.request})
            };
            busy(&a, true);
            text("br-share-status", "Sharing your creation…");
            let a = a.clone();
            spawn_local(async move {
                // Establish the visitor cookie before the first write.
                let result = match fetch("", None).await {
                    Ok(_) => fetch("", Some(body)).await,
                    Err(e) => Err(e),
                };
                busy(&a, false);
                match result {
                    Ok(_) => {
                        close("br-share-card");
                        text("br-status", "Your creation is in the gallery.");
                        a.borrow_mut().offset = 0;
                        open("br-gallery");
                        gallery(a);
                    }
                    Err(e) => text("br-share-status", &e),
                }
            });
        })?;
    }
    {
        let a = app.clone();
        button("br-download", move |_| {
            if let Ok((canvas, ctx)) = crate::paint::canvas() {
                if let Ok(p) = Painter::new(ctx) {
                    p.redraw(&a.borrow().drawing);
                    if let Ok(url) = canvas.to_data_url_with_type("image/png") {
                        let link = node("a", "", "");
                        let _ = link.set_attribute("href", &url);
                        let _ = link.set_attribute("download", "my-blue-rush-beach.png");
                        link.unchecked_into::<web_sys::HtmlElement>().click();
                    }
                }
            }
        })?;
    }
    listen(el("br-title").as_ref(), "keydown", move |e| {
        let e: KeyboardEvent = e.unchecked_into();
        if e.key() == "Enter" {
            e.prevent_default();
            el("br-publish")
                .unchecked_into::<web_sys::HtmlElement>()
                .click();
        }
    })?;
    {
        let a = app.clone();
        listen(el("br-color").as_ref(), "input", move |_| {
            let color = el("br-color").unchecked_into::<HtmlInputElement>().value();
            if !crate::valid_color(&color) {
                return;
            }
            let mut s = a.borrow_mut();
            s.color = color.clone();
            if let Some(i) = s.selected {
                checkpoint(&mut s);
                s.drawing.actions[i].color = Some(color.clone());
                s.painter.update_objects(&s.drawing);
                save(&s);
            }
            let tool = s.tool;
            if let Some((_, c)) = s.palette.iter_mut().find(|(t, _)| *t == tool) {
                *c = color;
            } else {
                s.palette.push((tool, color));
            }
        })?;
    }
    for (id, light) in [("br-sun", Light::Sun), ("br-shade", Light::Shade)] {
        let a = app.clone();
        button(id, move |_| {
            let mut s = a.borrow_mut();
            if s.drawing.light == light {
                return;
            }
            checkpoint(&mut s);
            s.drawing.light = light;
            s.painter.update_objects(&s.drawing);
            save(&s);
        })?;
    }
    {
        let a = app.clone();
        button("br-visitors", move |_| {
            let mut s = a.borrow_mut();
            s.drawing.visitors = !s.drawing.visitors;
            s.visitors.clear();
            s.visitor_text.clear();
            text(
                "br-visitor-name",
                if s.drawing.visitors {
                    "A quiet moment at the shore"
                } else {
                    "Visitors are off"
                },
            );
            save(&s);
        })?;
    }
    animate(app.clone())?;
    save(&app.borrow());
    // The listeners hold the editor for the lifetime of the page.
    Ok(())
}

fn animate(app: App) -> Result<(), JsValue> {
    let callback: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let next = callback.clone();
    let mut last = 0.;
    let motion = web_sys::window()
        .unwrap()
        .match_media("(prefers-reduced-motion: reduce)")?
        .map(|q| !q.matches())
        .unwrap_or(true);
    *callback.borrow_mut() = Some(Closure::wrap(Box::new(move |now: f64| {
        if now - last >= 40. && !doc().hidden() {
            last = now;
            let mut s = app.borrow_mut();
            let r = s.canvas.get_bounding_client_rect();
            let height = web_sys::window()
                .unwrap()
                .inner_height()
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(1000.);
            if r.bottom() > 0. && r.top() < height && s.drawing.visitors {
                let h = crate::visitors::Habitat {
                    water: s.painter.water_ratio(),
                    anemones: s
                        .drawing
                        .actions
                        .iter()
                        .filter(|a| a.tool == Tool::Anemone && s.painter.is_water(a.points[0]))
                        .count(),
                    shells: s
                        .drawing
                        .actions
                        .iter()
                        .filter(|a| a.tool == Tool::Conch)
                        .count(),
                };
                let Editor {
                    painter, visitors, ..
                } = &mut *s;
                visitors.step(now / 1000., h, |p| painter.is_water(p), motion);
                let names = s
                    .visitors
                    .guests
                    .iter()
                    .map(|g| g.kind.label())
                    .collect::<Vec<_>>()
                    .join(" · ");
                if names != s.visitor_text {
                    s.visitor_text = names.clone();
                    text(
                        "br-visitor-name",
                        if names.is_empty() {
                            "A quiet moment at the shore"
                        } else {
                            &names
                        },
                    );
                }
                present(&s);
            }
        }
        let _ = web_sys::window()
            .unwrap()
            .request_animation_frame(next.borrow().as_ref().unwrap().as_ref().unchecked_ref());
    }) as Box<dyn FnMut(f64)>));
    web_sys::window()
        .unwrap()
        .request_animation_frame(callback.borrow().as_ref().unwrap().as_ref().unchecked_ref())?;
    Ok(())
}
