//! DOM events and GPU handles. Portable viewing behavior stays in the shared
//! workbench; this adapter never owns a simulator or controller.
use glam::Mat4;
use robot_workbench::view::{Bundle, Effect, Intent, State};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    Document, Element, Event, EventTarget, HtmlCanvasElement, HtmlInputElement, HtmlSelectElement,
    Window,
};

type Shared = Rc<RefCell<App>>;
thread_local! {static ROOT:RefCell<Option<Shared>>=const{RefCell::new(None)};}

struct Listener {
    target: EventTarget,
    name: String,
    callback: Closure<dyn FnMut(Event)>,
}
impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self.target.remove_event_listener_with_callback(
            &self.name,
            self.callback.as_ref().unchecked_ref(),
        );
    }
}

struct App {
    window: Window,
    document: Document,
    canvas: HtmlCanvasElement,
    base: String,
    chat: Option<String>,
    key: String,
    state: State,
    comparison: Option<State>,
    robot: Option<Rc<robot_description::Robot>>,
    other_robot: Option<Rc<robot_description::Robot>>,
    renderer: Option<verse::render::Renderer>,
    opening: bool,
    abort: Option<web_sys::AbortController>,
    visible: bool,
    selected: bool,
    dirty: bool,
    last_pointer: Option<(i32, i32)>,
    comparison_request: u64,
    listeners: Vec<Listener>,
    raf: Option<Closure<dyn FnMut(f64)>>,
    raf_id: Option<i32>,
    observer: Option<web_sys::IntersectionObserver>,
    observer_callback: Option<Closure<dyn FnMut(js_sys::Array)>>,
    frames: u64,
    first_frame: Option<f64>,
    last_frame: Option<f64>,
    frame_ms: f64,
    pose_ms: f64,
    pose_started: Option<f64>,
}
impl Drop for App {
    fn drop(&mut self) {
        if let Some(id) = self.raf_id.take() {
            let _ = self.window.cancel_animation_frame(id);
        }
        if let Some(a) = &self.abort {
            a.abort();
        }
        if let Some(o) = &self.observer {
            o.disconnect();
        }
    }
}

fn js(error: impl Into<JsValue>) -> String {
    let error = error.into();
    error
        .as_string()
        .unwrap_or_else(|| "The robot view couldn't finish this action".into())
}
fn element(document: &Document, id: &str) -> Result<Element, String> {
    document
        .get_element_by_id(id)
        .ok_or_else(|| format!("The player is missing {id}"))
}
fn status(document: &Document, text: &str) {
    if let Ok(node) = element(document, "status") {
        node.set_text_content(Some(text));
    }
}
fn now(window: &Window) -> f64 {
    window.performance().map_or(0.0, |p| p.now())
}

async fn bytes(
    window: &Window,
    url: &str,
    limit: usize,
    abort: Option<&web_sys::AbortController>,
    post: Option<&Value>,
) -> Result<Vec<u8>, String> {
    let options = web_sys::RequestInit::new();
    options.set_mode(web_sys::RequestMode::SameOrigin);
    if let Some(abort) = abort {
        options.set_signal(Some(&abort.signal()));
    }
    if let Some(body) = post {
        options.set_method("POST");
        options.set_body(&JsValue::from_str(&body.to_string()));
    }
    let request = web_sys::Request::new_with_str_and_init(url, &options).map_err(js)?;
    if post.is_some() {
        request
            .headers()
            .set("content-type", "application/json")
            .map_err(js)?;
    }
    let response: web_sys::Response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(js)?
        .dyn_into()
        .map_err(js)?;
    if response
        .headers()
        .get("content-length")
        .map_err(js)?
        .and_then(|n| n.parse::<usize>().ok())
        .is_some_and(|n| n > limit)
    {
        return Err("This robot file is too large".into());
    }
    let array = JsFuture::from(response.array_buffer().map_err(js)?)
        .await
        .map_err(js)?;
    let array = js_sys::Uint8Array::new(&array);
    if array.length() as usize > limit {
        return Err("This robot file is too large".into());
    }
    let result = array.to_vec();
    if !response.ok() {
        return Err(serde_json::from_slice::<Value>(&result)
            .ok()
            .and_then(|v| v["error"].as_str().map(str::to_owned))
            .unwrap_or_else(|| "Robot Lab couldn't answer. Try again".into()));
    }
    Ok(result)
}
async fn json_value(window: &Window, url: &str, post: Option<&Value>) -> Result<Value, String> {
    serde_json::from_slice(&bytes(window, url, 18 << 20, None, post).await?)
        .map_err(|_| "This robot reply couldn't be read".into())
}
async fn bundle(
    window: &Window,
    base: &str,
    document: &str,
    version: &str,
) -> Result<Bundle, String> {
    let value = bytes(
        window,
        &format!("{base}/api/view?document={document}&version={version}"),
        18 << 20,
        None,
        None,
    )
    .await?;
    let bundle: Bundle =
        serde_json::from_slice(&value).map_err(|_| "This robot scene couldn't be read")?;
    bundle.validate()?;
    Ok(bundle)
}
async fn robot(
    window: &Window,
    base: &str,
    bundle: &Bundle,
    abort: &web_sys::AbortController,
) -> Result<Rc<robot_description::Robot>, String> {
    let definition = &bundle.robot;
    let description = definition
        .source
        .files
        .get(&definition.description)
        .ok_or("This robot is missing its description")?;
    let source = bytes(
        window,
        &format!("{base}/api/asset/{}", description.sha256),
        1 << 20,
        Some(abort),
        None,
    )
    .await?;
    robot_workbench::view::verify_asset(description, &source)?;
    let mut meshes = BTreeMap::new();
    let mut total = 0_usize;
    for (name, asset) in &definition.source.files {
        if !name.ends_with(".stl") {
            continue;
        }
        let value = bytes(
            window,
            &format!("{base}/api/asset/{}", asset.sha256),
            robot_description::MAX_MESH_BYTES as usize,
            Some(abort),
            None,
        )
        .await?;
        robot_workbench::view::verify_asset(asset, &value)?;
        total += value.len();
        if total > robot_description::MAX_TOTAL_MESH_BYTES as usize {
            return Err("This robot's meshes are too large".into());
        }
        meshes.insert(name.clone(), value);
    }
    let text = std::str::from_utf8(&source).map_err(|_| "Use a UTF-8 robot description")?;
    let robot = robot_description::parse_with_reader(text, &|name| {
        meshes
            .get(name)
            .cloned()
            .ok_or_else(|| robot_description::Error::Read {
                path: name.into(),
                reason: "This mesh isn't saved with the robot".into(),
            })
    })
    .map_err(|e| e.to_string())?;
    Ok(Rc::new(robot))
}

fn world(
    state: &State,
    robot: &robot_description::Robot,
    shift: f32,
) -> Result<verse::mesh::Mesh, String> {
    let mut scene =
        verse::robot_lab::document_scene(robot, &state.displayed_scene(), state.step())?;
    if let Some(link) = state.link.as_ref().and_then(|name| robot.link(name)) {
        for vertex in &mut scene.robot[scene.link_ranges[link].clone()] {
            vertex.color = [0.2, 0.8, 1.0];
        }
    }
    let mut lit = scene.props;
    lit.extend(scene.robot);
    if shift != 0.0 {
        for vertex in &mut lit {
            vertex.pos[0] += shift;
        }
    }
    Ok(verse::robot_lab::shaded_mesh(&lit))
}
fn combined(app: &App) -> Result<verse::mesh::Mesh, String> {
    let robot = app.robot.as_ref().ok_or("The robot is still opening")?;
    let mut left = world(
        &app.state,
        robot,
        if app.comparison.is_some() { -0.7 } else { 0.0 },
    )?;
    if let (Some(other), Some(robot)) = (&app.comparison, &app.other_robot) {
        left.faces.extend(world(other, robot, 0.7)?.faces);
    }
    Ok(left)
}

fn notify(app: &App, event: &str, value: Value) {
    if let Ok(Some(parent)) = app.window.parent() {
        if parent != app.window {
            let origin = app.window.location().origin().unwrap_or_default();
            let _ = parent.post_message(
                &json!({"robot_lab":event,"key":app.key,"value":value})
                    .to_string()
                    .into(),
                &origin,
            );
        }
    }
}
fn request_frame(app: &Shared) {
    let mut data = app.borrow_mut();
    if data.raf_id.is_none()
        && data.state.active()
        && (data.renderer.is_some() || (data.state.recorded_camera() && data.state.playing()))
    {
        if let Some(callback) = &data.raf {
            data.raf_id = data
                .window
                .request_animation_frame(callback.as_ref().unchecked_ref())
                .ok();
        }
    }
}
fn resize(app: &mut App) {
    let width = (app.canvas.client_width().max(1) as u32).min(1280);
    let height = (app.canvas.client_height().max(1) as u32).min(800);
    if app.canvas.width() != width || app.canvas.height() != height {
        app.canvas.set_width(width);
        app.canvas.set_height(height);
        if let Some(renderer) = &mut app.renderer {
            let _ = renderer.resize(width, height);
        }
    }
}
fn draw(app: &Shared, time: f64) {
    let mut data = app.borrow_mut();
    data.raf_id = None;
    if !data.state.active() {
        return;
    }
    let changed = data.state.tick(time / 1000.0);
    let other_changed = data
        .comparison
        .as_mut()
        .is_some_and(|s| s.tick(time / 1000.0));
    if changed || other_changed {
        data.dirty = true;
        readout(&data);
    }
    resize(&mut data);
    if data.dirty && data.renderer.is_some() {
        match combined(&data) {
            Ok(world) => {
                if let Some(renderer) = &mut data.renderer {
                    if let Err(error) = renderer.replace_world(&world) {
                        status(&data.document, &error);
                    }
                }
            }
            Err(error) => status(&data.document, &error),
        }
        data.dirty = false;
    }
    let mut orbit = data.state.orbit;
    if data.comparison.is_some() {
        orbit.center[0] = 0.1;
        orbit.distance *= 2.0;
    }
    let (mut matrix, mut eye) = orbit.matrices(data.canvas.width(), data.canvas.height());
    if data.state.camera != "orbit" {
        if let Some(camera) = data
            .state
            .bundle()
            .scene
            .cameras
            .iter()
            .find(|c| c.id == data.state.camera)
        {
            let (mut at, mut target, mut up) = robot_workbench::view::camera_pose(&camera.pose);
            if let (Some((_, link)), Some(model)) = (&camera.robot_link, &data.robot) {
                let scene = data.state.displayed_scene();
                let values = data
                    .state
                    .step()
                    .map_or(&scene.robots[0].joints, |step| &step.joints);
                let joints = model
                    .movable()
                    .map(|joint| values[&joint.name])
                    .collect::<Vec<_>>();
                if let (Some(index), Ok(frames)) =
                    (model.link(link), model.forward_observed(&joints))
                {
                    let pose = &scene.robots[0].pose;
                    let base = glam::DMat4::from_rotation_translation(
                        glam::DQuat::from_array(pose.rotation),
                        glam::DVec3::from_array(pose.position),
                    ) * robot_description::z_up_to_y_up()
                        * frames[index];
                    at = base.transform_point3(at);
                    target = base.transform_point3(target);
                    up = base.transform_vector3(up);
                }
            }
            eye = at.as_vec3();
            let fy = camera.intrinsics[1] as f32;
            let fov = 2.0 * (camera.height as f32 / (2.0 * fy)).atan();
            matrix = Mat4::perspective_rh(
                fov,
                data.canvas.width() as f32 / data.canvas.height().max(1) as f32,
                0.02,
                100.0,
            ) * Mat4::look_at_rh(eye, target.as_vec3(), up.as_vec3());
        }
    }
    if let Some(renderer) = &mut data.renderer {
        match renderer.draw(
            verse::render::View {
                view_proj: matrix,
                eye,
            },
            &verse::mesh::Mesh::default(),
            &verse::ui::UiBatch::default(),
        ) {
            verse::render::DrawStatus::Presented => {
                data.frames += 1;
                if data.first_frame.is_none() {
                    data.first_frame = Some(time);
                }
                if let Some(previous) = data.last_frame.replace(time) {
                    data.frame_ms = time - previous;
                }
                if let Some(started) = data.pose_started.take() {
                    data.pose_ms = now(&data.window) - started;
                }
            }
            verse::render::DrawStatus::Error(error) => {
                web_sys::console::error_1(&error.into());
                status(
                    &data.document,
                    "The 3D view stopped. Select Orbit to reopen it",
                );
                data.renderer = None;
            }
            _ => (),
        }
    }
    drop(data);
    request_frame(app);
}

fn lifecycle(app: &Shared) {
    let mut data = app.borrow_mut();
    let was_active = data.state.active();
    let visible = data.visible && !data.document.hidden();
    let selected = data.selected;
    data.state.lifecycle(visible, selected);
    if let Some(other) = &mut data.comparison {
        other.lifecycle(visible, selected);
    }
    if !data.state.active() {
        if let Some(id) = data.raf_id.take() {
            let _ = data.window.cancel_animation_frame(id);
        }
        data.renderer = None;
        if let Some(abort) = data.abort.take() {
            abort.abort();
        }
        data.opening = false;
        data.canvas.set_hidden(true);
        if let Ok(poster) = element(&data.document, "poster") {
            let _ = poster.remove_attribute("hidden");
        }
        notify(&data, "resources", json!({"gpu":false}));
        readout(&data);
        return;
    }
    if data.state.recorded_camera() {
        if let Some(abort) = data.abort.take() {
            abort.abort();
        }
        data.opening = false;
        data.renderer = None;
        data.canvas.set_hidden(true);
        if let Ok(poster) = element(&data.document, "poster") {
            let _ = poster.set_attribute("hidden", "");
        }
        readout(&data);
        notify(&data, "resources", json!({"gpu":false}));
        drop(data);
        if !was_active {
            persist_selection(app);
        }
        request_frame(app);
        return;
    }
    if data.renderer.is_some() || data.opening {
        drop(data);
        if !was_active {
            persist_selection(app);
        }
        request_frame(app);
        return;
    }
    data.opening = true;
    let generation = data.state.generation();
    let weak = Rc::downgrade(app);
    drop(data);
    if !was_active {
        persist_selection(app);
    }
    spawn_local(async move {
        if let Some(app) = weak.upgrade() {
            match open_gpu(&app, generation).await {
                Ok(()) => request_frame(&app),
                Err(error) => {
                    let mut data = app.borrow_mut();
                    if data.state.generation() == generation {
                        data.opening = false;
                        data.canvas.set_hidden(true);
                        if let Ok(poster) = element(&data.document, "poster") {
                            let _ = poster.remove_attribute("hidden");
                        }
                        status(
                            &data.document,
                            "3D couldn't open. The saved preview is shown",
                        );
                        web_sys::console::error_1(&error.into());
                    }
                }
            }
        }
    });
}
async fn open_gpu(app: &Shared, generation: u64) -> Result<(), String> {
    let (window, base, bundle, other, old_robot, old_other) = {
        let data = app.borrow();
        (
            data.window.clone(),
            data.base.clone(),
            data.state.bundle().clone(),
            data.comparison.as_ref().map(|s| s.bundle().clone()),
            data.robot.clone(),
            data.other_robot.clone(),
        )
    };
    let abort = web_sys::AbortController::new().map_err(js)?;
    app.borrow_mut().abort = Some(abort.clone());
    let model = match old_robot {
        Some(robot) => robot,
        None => robot(&window, &base, &bundle, &abort).await?,
    };
    let other_model = if let Some(other) = &other {
        Some(if other.robot.source.files == bundle.robot.source.files {
            model.clone()
        } else if let Some(robot) = old_other {
            robot
        } else {
            robot(&window, &base, other, &abort).await?
        })
    } else {
        None
    };
    let (canvas, world) = {
        let mut data = app.borrow_mut();
        if !data.state.active() || data.state.generation() != generation {
            return Ok(());
        }
        data.robot = Some(model);
        data.other_robot = other_model;
        data.canvas.set_hidden(false);
        resize(&mut data);
        (data.canvas.clone(), combined(&data)?)
    };
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::BROWSER_WEBGPU;
    let candidate = wgpu::Instance::new(descriptor);
    // Probe before binding a canvas context: a browser can expose WebGPU but
    // have no usable adapter. A bound WebGPU canvas cannot become WebGL.
    let instance = if candidate
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            apply_limit_buckets: false,
        })
        .await
        .is_ok()
    {
        candidate
    } else {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::GL;
        wgpu::Instance::new(descriptor)
    };
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
        .map_err(|e| e.to_string())?;
    let renderer = verse::render::Renderer::from_surface_color_async(
        instance,
        surface,
        canvas.width(),
        canvas.height(),
        &world,
        &verse::ui::Atlas::new(16.0),
        verse::render::RenderOptions {
            sample_count: 1,
            max_extent: 2048,
            hdr: false,
        },
    )
    .await?;
    if let Some(error) = renderer.physical_error() {
        return Err(error.into());
    }
    let mut data = app.borrow_mut();
    if !data.state.mount(generation) {
        return Ok(());
    }
    data.renderer = Some(renderer);
    data.opening = false;
    data.dirty = false;
    data.abort = None;
    element(&data.document, "poster")?
        .set_attribute("hidden", "")
        .map_err(js)?;
    notify(&data, "resources", json!({"gpu":true}));
    readout(&data);
    Ok(())
}

fn readout(app: &App) {
    status(
        &app.document,
        &format!(
            "{} · Version {}",
            app.state.bundle().mode(),
            app.state.bundle().number
        ),
    );
    if let Ok(node) = element(&app.document, "readout") {
        node.set_text_content(Some(&app.state.readout()));
    }
    if let Ok(node) = element(&app.document, "joint") {
        if let Ok(select) = node.dyn_into::<HtmlSelectElement>() {
            select.set_value(&app.state.joint);
        }
    }
    if let (Some(model), Some(link)) = (&app.robot, &app.state.link) {
        let scene = app.state.displayed_scene();
        let values = app
            .state
            .step()
            .map_or(&scene.robots[0].joints, |s| &s.joints);
        let pose = model.movable().map(|j| values[&j.name]).collect::<Vec<_>>();
        if let (Ok(frames), Some(index)) = (model.forward_observed(&pose), model.link(link)) {
            let placement = &scene.robots[0].pose;
            let base = glam::DMat4::from_rotation_translation(
                glam::DQuat::from_array(placement.rotation),
                glam::DVec3::from_array(placement.position),
            ) * robot_description::z_up_to_y_up();
            let position = (base * frames[index]).transform_point3(glam::DVec3::ZERO);
            if let Ok(node) = element(&app.document, "link-position") {
                node.set_text_content(Some(&format!(
                    "{} · Position {:.3}, {:.3}, {:.3} m",
                    link, position.x, position.y, position.z
                )));
            }
        }
    }
    if let Ok(node) = element(&app.document, "frame") {
        if let Ok(input) = node.dyn_into::<HtmlInputElement>() {
            input.set_value(&app.state.frame.to_string());
        }
    }
    if let Some(step) = app.state.step() {
        if let Ok(node) = element(&app.document, "position") {
            if let Ok(input) = node.dyn_into::<HtmlInputElement>() {
                if let Some(value) = step.joints.get(&app.state.joint) {
                    input.set_value(&value.to_string());
                }
            }
        }
    }
    if let Ok(node) = element(&app.document, "play") {
        node.set_text_content(Some(if app.state.playing() { "Pause" } else { "Play" }));
    }
    if let Ok(node) = element(&app.document, "camera") {
        if let Ok(select) = node.dyn_into::<HtmlSelectElement>() {
            select.set_value(&app.state.camera);
        }
    }
    if let Some(other) = &app.comparison {
        if let Ok(node) = element(&app.document, "right-play") {
            node.set_text_content(Some(if other.playing() {
                "Pause right"
            } else {
                "Play right"
            }));
        }
        if let Ok(node) = element(&app.document, "right-frame") {
            if let Ok(input) = node.dyn_into::<HtmlInputElement>() {
                input.set_value(&other.frame.to_string());
            }
        }
    }
    if let Ok(image) = element(&app.document, "sensor") {
        if app.state.active() && app.state.recorded_camera() {
            let _ = image.remove_attribute("hidden");
            if let Some(sensor) = app.state.sensor() {
                let src = format!("{}/api/asset/{}", app.base, sensor.asset.sha256);
                if image.get_attribute("src").as_deref() != Some(&src) {
                    let _ = image.set_attribute("src", &src);
                }
                if let Ok(label) = element(&app.document, "sensor-time") {
                    let _ = label.remove_attribute("hidden");
                    label.set_text_content(Some(&format!(
                        "Recorded camera · {:.3} s",
                        sensor.timestamp_seconds
                    )));
                }
            } else {
                let _ = image.set_attribute("hidden", "");
                if let Ok(label) = element(&app.document, "sensor-time") {
                    let _ = label.remove_attribute("hidden");
                    label.set_text_content(Some("This camera frame is missing"));
                }
            }
        } else {
            let _ = image.set_attribute("hidden", "");
            if let Ok(label) = element(&app.document, "sensor-time") {
                let _ = label.set_attribute("hidden", "");
            }
        }
    }
}

fn listen(
    app: &Shared,
    target: &EventTarget,
    name: &str,
    mut callback: impl FnMut(&Shared, Event) + 'static,
) -> Result<(), String> {
    let weak = Rc::downgrade(app);
    let handler = Closure::wrap(Box::new(move |event: Event| {
        if let Some(app) = weak.upgrade() {
            callback(&app, event);
        }
    }) as Box<dyn FnMut(Event)>);
    target
        .add_event_listener_with_callback(name, handler.as_ref().unchecked_ref())
        .map_err(js)?;
    app.borrow_mut().listeners.push(Listener {
        target: target.clone(),
        name: name.into(),
        callback: handler,
    });
    Ok(())
}
fn add(
    document: &Document,
    parent: &Element,
    tag: &str,
    id: &str,
    text: &str,
) -> Result<Element, String> {
    let node = document.create_element(tag).map_err(js)?;
    if !id.is_empty() {
        node.set_id(id);
    }
    node.set_text_content(Some(text));
    parent.append_child(&node).map_err(js)?;
    Ok(node)
}
fn button(
    app: &Shared,
    parent: &Element,
    id: &str,
    label: &str,
    callback: impl FnMut(&Shared, Event) + 'static,
) -> Result<(), String> {
    let node = {
        let data = app.borrow();
        add(&data.document, parent, "button", id, label)?
    };
    listen(app, node.unchecked_ref(), "click", callback)
}
fn select(
    app: &Shared,
    parent: &Element,
    id: &str,
    label: &str,
    options: Vec<(String, String)>,
) -> Result<HtmlSelectElement, String> {
    let data = app.borrow();
    let holder = add(&data.document, parent, "label", "", label)?;
    let select: HtmlSelectElement = add(&data.document, &holder, "select", id, "")?
        .dyn_into()
        .map_err(js)?;
    for (value, label) in options {
        let option = add(&data.document, select.unchecked_ref(), "option", "", &label)?;
        option.set_attribute("value", &value).map_err(js)?;
    }
    Ok(select)
}
fn apply(app: &Shared, intent: Intent, value: Option<&str>) {
    let began = now(&app.borrow().window);
    let pose = matches!(intent, Intent::JointValue { .. });
    let joint = matches!(
        intent,
        Intent::ChooseLink { .. } | Intent::ChooseJoint { .. }
    );
    let camera = matches!(intent, Intent::ChooseCamera { .. } | Intent::ResetView);
    let keep = matches!(
        intent,
        Intent::Seek | Intent::Pause | Intent::ChooseCamera { .. }
    );
    let effect = app.borrow_mut().state.apply(intent, value);
    match effect {
        Ok(Effect::Draw) => {
            let mut data = app.borrow_mut();
            if camera {
                data.state.remount();
                data.renderer = None;
                data.opening = false;
                if let Some(abort) = data.abort.take() {
                    abort.abort();
                }
            }
            data.dirty = true;
            if pose {
                data.pose_started = Some(began);
            }
            readout(&data);
            drop(data);
            if joint {
                update_joint_input(app);
            }
            if camera {
                lifecycle(app);
            }
            request_frame(app);
            if keep {
                persist_selection(app);
            }
        }
        Ok(effect) => save_effect(app, effect),
        Err(error) => status(&app.borrow().document, &error),
    }
}

fn persist_selection(app: &Shared) {
    let weak = Rc::downgrade(app);
    spawn_local(async move {
        if let Some(app) = weak.upgrade() {
            let (window, base, chat, generation) = {
                let data = app.borrow();
                let Some(chat) = data.chat.clone() else {
                    return;
                };
                if !data.state.active() {
                    return;
                }
                (
                    data.window.clone(),
                    data.base.clone(),
                    chat,
                    data.state.generation(),
                )
            };
            for _ in 0..2 {
                let restored = match json_value(
                    &window,
                    &format!("{base}/api/tool/robot_restore"),
                    Some(&json!({"chat":chat})),
                )
                .await
                {
                    Ok(v) => v,
                    Err(_) => return,
                };
                let selection: Option<robot_lab::Selection> =
                    serde_json::from_value(restored["result"]["selection"].clone()).unwrap_or(None);
                let selection_base = selection
                    .as_ref()
                    .and_then(|s| s.sha256().ok())
                    .unwrap_or_else(|| "none".into());
                let arguments = {
                    let data = app.borrow();
                    if !data.state.active() || data.state.generation() != generation {
                        return;
                    }
                    let reference = &data.state.bundle().subject;
                    json!({"chat":chat,"document":reference.document,"version":reference.version,
                    "frame":data.state.bundle().episode.as_ref().map(|_|data.state.frame as u32),"camera":data.state.camera,
                    "comparison":data.comparison.as_ref().map(|s|&s.bundle().subject),"selection_base":selection_base})
                };
                if json_value(
                    &window,
                    &format!("{base}/api/tool/robot_select"),
                    Some(&arguments),
                )
                .await
                .is_ok()
                {
                    return;
                }
            }
        }
    });
}
fn save_effect(app: &Shared, effect: Effect) {
    let weak = Rc::downgrade(app);
    spawn_local(async move {
        if let Some(app) = weak.upgrade() {
            if let Err(error) = save(&app, effect).await {
                status(&app.borrow().document, &error);
            }
        }
    });
}
async fn save(app: &Shared, effect: Effect) -> Result<(), String> {
    let (window, base, chat, key) = {
        let data = app.borrow();
        (
            data.window.clone(),
            data.base.clone(),
            data.chat.clone(),
            data.key.clone(),
        )
    };
    match effect {
        Effect::SaveNote {
            subject,
            frame,
            text,
        } => {
            json_value(&window,&format!("{base}/api/tool/robot_add_note"),Some(&json!({"document":subject.document,"version":subject.version,"frame":frame,"text":text}))).await?;
            status(&app.borrow().document, "Note saved");
            load_notes(app).await?;
        }
        Effect::SavePose {
            base: reference,
            joints,
        } => {
            let mut scene = app.borrow().state.bundle().scene.clone();
            scene.robots[0].joints.extend(joints);
            let result=json_value(&window,&format!("{base}/api/tool/robot_edit"),Some(&json!({"document":reference.document,"base":reference.version,"scene_json":serde_json::to_string(&scene).map_err(|_|"This pose couldn't be saved")?,"summary":"Saved joint pose"}))).await?;
            let reference = serde_json::from_value(result["card"]["reference"].clone())
                .map_err(|_| "This pose couldn't be saved")?;
            {
                let data = app.borrow();
                notify(
                    &data,
                    "saved",
                    json!({"card":result["card"],"summary":result["summary"],"chat":chat,"key":key}),
                );
                // An inline card keeps its original subject. The parent adds a
                // new card for the saved version; a standalone player opens it.
                let standalone = data
                    .window
                    .parent()
                    .ok()
                    .flatten()
                    .is_none_or(|p| p == data.window);
                drop(data);
                if standalone {
                    open_reference(app, &reference).await?;
                } else {
                    status(&app.borrow().document, "Pose saved as a new version");
                }
            }
        }
        Effect::ChooseComparison { subject } => {
            let versions = json_value(
                &window,
                &format!("{base}/api/tool/robot_versions"),
                Some(&json!({"document":subject.document})),
            )
            .await?;
            comparison_choices(app, &versions["result"]["versions"])?;
        }
        Effect::Draw => (),
    }
    Ok(())
}
async fn open_reference(app: &Shared, reference: &robot_lab::Reference) -> Result<(), String> {
    let window = app.borrow().window.clone();
    let params = web_sys::UrlSearchParams::new_with_str(&window.location().search().map_err(js)?)
        .map_err(js)?;
    params.set("document", &reference.document);
    params.set("version", &reference.version);
    params.delete("compare_document");
    params.delete("compare_version");
    window
        .location()
        .set_search(&format!("?{}", params.to_string()))
        .map_err(js)?;
    Ok(())
}
async fn load_notes(app: &Shared) -> Result<(), String> {
    let (window, base, reference, document) = {
        let data = app.borrow();
        (
            data.window.clone(),
            data.base.clone(),
            data.state.bundle().subject.clone(),
            data.document.clone(),
        )
    };
    let reply = json_value(
        &window,
        &format!("{base}/api/tool/robot_notes"),
        Some(&json!({"document":reference.document,"version":reference.version})),
    )
    .await?;
    let parent = element(&document, "saved-notes")?;
    parent.set_text_content(None);
    for note in reply["result"]["notes"].as_array().into_iter().flatten() {
        let text = note["text"].as_str().unwrap_or("");
        let label = note["episode_step"]
            .as_u64()
            .map_or(text.to_owned(), |step| format!("Frame {step}: {text}"));
        add(&document, &parent, "p", "", &label)?;
    }
    Ok(())
}

fn comparison_choices(app: &Shared, versions: &Value) -> Result<(), String> {
    let options = versions
        .as_array()
        .ok_or("These versions couldn't be opened")?
        .iter()
        .map(|v| {
            let number = v["number"].as_u64().unwrap_or(0);
            let summary = v["summary"].as_str().unwrap_or("");
            // The version reference includes its timestamp and parent; use the
            // exact bytes returned by the store, with float round trips preserved.
            let value: robot_lab::Version =
                serde_json::from_value(v.clone()).map_err(|_| "This version couldn't be read")?;
            Ok((
                value.reference().map_err(|e| e.to_string())?,
                format!("Version {number} · {summary}"),
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let parent = element(&app.borrow().document, "inspector")?;
    let selector = select(
        app,
        &parent,
        "compare-version",
        "Compare with",
        options
            .iter()
            .map(|(r, label)| (r.version.clone(), label.clone()))
            .collect(),
    )?;
    let document = app.borrow().state.bundle().subject.document.clone();
    listen(
        app,
        selector.unchecked_ref(),
        "change",
        move |app, event| {
            let Ok(selector) = event
                .target()
                .ok_or(())
                .and_then(|e| e.dyn_into::<HtmlSelectElement>().map_err(|_| ()))
            else {
                return;
            };
            let version = selector.value();
            let document = document.clone();
            let weak = Rc::downgrade(app);
            spawn_local(async move {
                if let Some(app) = weak.upgrade() {
                    if let Err(error) = load_comparison(&app, &document, &version).await {
                        status(&app.borrow().document, &error);
                    }
                }
            });
        },
    )?;
    Ok(())
}
async fn load_comparison(app: &Shared, document: &str, version: &str) -> Result<(), String> {
    let request = {
        let mut data = app.borrow_mut();
        data.comparison_request += 1;
        data.comparison_request
    };
    let (window, base) = {
        let data = app.borrow();
        (data.window.clone(), data.base.clone())
    };
    let other = bundle(&window, &base, document, version).await?;
    if app.borrow().comparison_request != request {
        return Ok(());
    }
    let mut state = State::new(other)?;
    {
        let data = app.borrow();
        state.lifecycle(data.visible, data.selected);
    }
    {
        let mut data = app.borrow_mut();
        data.comparison = Some(state);
        data.state.remount();
        data.state.camera = "orbit".into();
        if let Some(abort) = data.abort.take() {
            abort.abort();
        }
        data.other_robot = None;
        data.renderer = None;
        data.opening = false;
        data.dirty = true;
    }
    lifecycle(app);
    comparison_controls(app)?;
    persist_selection(app);
    Ok(())
}
fn comparison_controls(app: &Shared) -> Result<(), String> {
    let document = app.borrow().document.clone();
    let parent = element(&document, "comparison-label")?;
    parent.set_text_content(None);
    if let Some(other) = &app.borrow().comparison {
        add(
            &document,
            &parent,
            "span",
            "",
            &format!("Version {}", app.borrow().state.bundle().number),
        )?;
        add(
            &document,
            &parent,
            "span",
            "",
            &format!("Version {}", other.bundle().number),
        )?;
    }
    if let Some(length) = app
        .borrow()
        .comparison
        .as_ref()
        .and_then(|s| s.bundle().episode.as_ref().map(|e| e.steps.len()))
    {
        let label = add(&document, &parent, "label", "", "Right frame")?;
        let input: HtmlInputElement = add(&document, &label, "input", "right-frame", "")?
            .dyn_into()
            .map_err(js)?;
        input.set_type("range");
        input.set_min("0");
        input.set_max(&(length - 1).to_string());
        input.set_step("1");
        listen(app, input.unchecked_ref(), "input", |app, event| {
            if let Some(value) = event
                .target()
                .and_then(|e| e.dyn_into::<HtmlInputElement>().ok())
                .and_then(|i| i.value().parse::<usize>().ok())
            {
                let mut data = app.borrow_mut();
                if let Some(other) = &mut data.comparison {
                    let _ = other.seek(value);
                }
                data.dirty = true;
            }
        })?;
        button(app, &parent, "right-play", "Play right", |app, _| {
            let mut data = app.borrow_mut();
            if let Some(other) = &mut data.comparison {
                if other.playing() {
                    other.pause();
                } else {
                    let _ = other.play();
                }
            }
        })?;
        button(app, &parent, "match-time", "Match time", |app, _| {
            let mut data = app.borrow_mut();
            let time = data.state.step().map_or(0.0, |s| s.time_seconds);
            if let Some(other) = &mut data.comparison {
                if let Some(episode) = &other.bundle().episode {
                    let frame = episode
                        .steps
                        .partition_point(|s| s.time_seconds <= time)
                        .saturating_sub(1);
                    let _ = other.seek(frame);
                }
            }
            data.dirty = true;
        })?;
    }
    Ok(())
}

fn controls(app: &Shared) -> Result<(), String> {
    let document = app.borrow().document.clone();
    let controls = element(&document, "controls")?;
    button(app, &controls, "orbit", "Orbit", |app, _| {
        app.borrow_mut().selected = true;
        notify(&app.borrow(), "select", Value::Null);
        lifecycle(app);
    })?;
    button(app, &controls, "reset-view", "Reset view", |app, _| {
        apply(app, Intent::ResetView, None)
    })?;
    button(app, &controls, "expand", "Expand view", |app, _| {
        apply(app, Intent::Expanded, None);
        let data = app.borrow();
        notify(&data, "expand", json!(data.state.expanded));
        if let Some(body) = data.document.body() {
            let _ = body.style().set_property(
                "max-width",
                if data.state.expanded {
                    "none"
                } else {
                    "1100px"
                },
            );
        }
    })?;
    let joints = app
        .borrow()
        .state
        .bundle()
        .robot
        .joints
        .iter()
        .filter(|j| j.kind != robot_lab::document::JointKind::Fixed)
        .map(|j| (j.name.clone(), j.name.replace('_', " ")))
        .collect();
    let selector = select(app, &controls, "joint", "Joint", joints)?;
    listen(app, selector.unchecked_ref(), "change", |app, event| {
        if let Some(target) = event
            .target()
            .and_then(|t| t.dyn_into::<HtmlSelectElement>().ok())
        {
            apply(
                app,
                Intent::ChooseJoint {
                    joint: target.value(),
                },
                None,
            );
            update_joint_input(app);
        }
    })?;
    let holder = add(&document, &controls, "label", "", "Position")?;
    let input: HtmlInputElement = add(&document, &holder, "input", "position", "")?
        .dyn_into()
        .map_err(js)?;
    input.set_type("range");
    input.set_step("0.001");
    input
        .set_attribute("aria-label", "Joint position")
        .map_err(js)?;
    if app.borrow().state.bundle().episode.is_none() {
        listen(app, input.unchecked_ref(), "input", |app, event| {
            if let Some(target) = event
                .target()
                .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
            {
                let joint = app.borrow().state.joint.clone();
                apply(app, Intent::JointValue { joint }, Some(&target.value()));
            }
        })?;
        button(app, &controls, "save-pose", "Save pose", |app, _| {
            apply(app, Intent::ApplyPose, None)
        })?;
    } else {
        input.set_disabled(true);
        button(app, &controls, "play", "Play", |app, _| {
            let intent = if app.borrow().state.playing() {
                Intent::Pause
            } else {
                Intent::Play
            };
            apply(app, intent, None);
        })?;
        let holder = add(&document, &controls, "label", "", "Frame")?;
        let frame: HtmlInputElement = add(&document, &holder, "input", "frame", "")?
            .dyn_into()
            .map_err(js)?;
        frame.set_type("range");
        frame.set_min("0");
        frame.set_max(
            &(app
                .borrow()
                .state
                .bundle()
                .episode
                .as_ref()
                .unwrap()
                .steps
                .len()
                - 1)
            .to_string(),
        );
        frame.set_step("1");
        frame.set_value("0");
        listen(app, frame.unchecked_ref(), "input", |app, event| {
            if let Some(target) = event
                .target()
                .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
            {
                apply(app, Intent::Seek, Some(&target.value()));
            }
        })?;
    }
    let links = app
        .borrow()
        .state
        .bundle()
        .robot
        .links
        .iter()
        .map(|l| (l.name.clone(), l.name.replace('_', " ")))
        .collect();
    let links = select(app, &controls, "link", "Link", links)?;
    listen(app, links.unchecked_ref(), "change", |app, event| {
        if let Some(target) = event
            .target()
            .and_then(|t| t.dyn_into::<HtmlSelectElement>().ok())
        {
            apply(
                app,
                Intent::ChooseLink {
                    link: target.value(),
                },
                None,
            );
        }
    })?;
    let mut cameras = vec![("orbit".into(), "Orbit view".into())];
    cameras.extend(
        app.borrow()
            .state
            .bundle()
            .scene
            .cameras
            .iter()
            .map(|c| (c.id.clone(), c.id.clone())),
    );
    let cameras = select(app, &controls, "camera", "Camera", cameras)?;
    listen(app, cameras.unchecked_ref(), "change", |app, event| {
        if let Some(target) = event
            .target()
            .and_then(|t| t.dyn_into::<HtmlSelectElement>().ok())
        {
            apply(
                app,
                Intent::ChooseCamera {
                    camera: target.value(),
                },
                None,
            );
        }
    })?;
    button(app, &controls, "compare", "Compare versions", |app, _| {
        apply(app, Intent::Compare, None)
    })?;
    add(
        &document,
        &element(&document, "inspector")?,
        "p",
        "readout",
        "",
    )?;
    add(
        &document,
        &element(&document, "inspector")?,
        "p",
        "link-position",
        "",
    )?;
    let notes = element(&document, "notes")?;
    let label = add(&document, &notes, "label", "", "Note")?;
    let note: HtmlInputElement = add(&document, &label, "input", "note", "")?
        .dyn_into()
        .map_err(js)?;
    note.set_max_length(1024);
    note.set_placeholder("What did you notice?");
    listen(app, note.unchecked_ref(), "input", |app, event| {
        if let Some(target) = event
            .target()
            .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
        {
            apply(app, Intent::NoteDraft, Some(&target.value()));
        }
    })?;
    button(app, &notes, "save-note", "Save note", |app, _| {
        apply(app, Intent::AddNote, None)
    })?;
    add(&document, &notes, "div", "saved-notes", "")?;
    update_joint_input(app);
    readout(&app.borrow());
    Ok(())
}
fn update_joint_input(app: &Shared) {
    let data = app.borrow();
    if let Ok(input) = element(&data.document, "position")
        .and_then(|e| e.dyn_into::<HtmlInputElement>().map_err(js))
    {
        if let Some(joint) = data
            .state
            .bundle()
            .robot
            .joints
            .iter()
            .find(|j| j.name == data.state.joint)
        {
            let (min, max) = joint
                .limits
                .map_or((-std::f64::consts::PI, std::f64::consts::PI), |l| {
                    (l.lower, l.upper)
                });
            input.set_min(&min.to_string());
            input.set_max(&max.to_string());
            input.set_value(
                &data
                    .state
                    .draft
                    .get(&joint.name)
                    .or_else(|| data.state.bundle().scene.robots[0].joints.get(&joint.name))
                    .copied()
                    .unwrap_or(0.0)
                    .to_string(),
            );
        }
    }
}

fn events(app: &Shared) -> Result<(), String> {
    let canvas = app.borrow().canvas.clone();
    listen(app, canvas.unchecked_ref(), "pointerdown", |app, event| {
        if let Ok(event) = event.dyn_into::<web_sys::PointerEvent>() {
            let mut data = app.borrow_mut();
            data.last_pointer = Some((event.client_x(), event.client_y()));
            let _ = data.canvas.set_pointer_capture(event.pointer_id());
        }
    })?;
    listen(app, canvas.unchecked_ref(), "pointerup", |app, _| {
        app.borrow_mut().last_pointer = None
    })?;
    listen(app, canvas.unchecked_ref(), "pointercancel", |app, _| {
        app.borrow_mut().last_pointer = None
    })?;
    listen(app, canvas.unchecked_ref(), "pointermove", |app, event| {
        if let Ok(event) = event.dyn_into::<web_sys::PointerEvent>() {
            let mut data = app.borrow_mut();
            if let Some((x, y)) = data
                .last_pointer
                .replace((event.client_x(), event.client_y()))
            {
                if event.buttons() != 0 {
                    data.state
                        .orbit
                        .turn((event.client_x() - x) as f32, (event.client_y() - y) as f32);
                    data.state.camera = "orbit".into();
                } else {
                    data.last_pointer = None;
                }
            }
        }
    })?;
    listen(app, canvas.unchecked_ref(), "wheel", |app, event| {
        if let Ok(event) = event.dyn_into::<web_sys::WheelEvent>() {
            event.prevent_default();
            app.borrow_mut().state.orbit.zoom(event.delta_y() as f32);
        }
    })?;
    listen(app, canvas.unchecked_ref(), "keydown", |app, event| {
        if let Ok(event) = event.dyn_into::<web_sys::KeyboardEvent>() {
            let mut data = app.borrow_mut();
            match event.key().as_str() {
                "ArrowLeft" => data.state.orbit.turn(-15.0, 0.0),
                "ArrowRight" => data.state.orbit.turn(15.0, 0.0),
                "ArrowUp" => data.state.orbit.turn(0.0, -15.0),
                "ArrowDown" => data.state.orbit.turn(0.0, 15.0),
                "+" => data.state.orbit.zoom(-100.0),
                "-" => data.state.orbit.zoom(100.0),
                _ => return,
            }
            data.state.camera = "orbit".into();
            event.prevent_default();
        }
    })?;
    let document = app.borrow().document.clone();
    listen(
        app,
        document.unchecked_ref(),
        "visibilitychange",
        |app, _| lifecycle(app),
    )?;
    let window = app.borrow().window.clone();
    listen(app, window.unchecked_ref(), "pagehide", |app, _| {
        app.borrow_mut().selected = false;
        lifecycle(app);
    })?;
    listen(app, window.unchecked_ref(), "message", |app, event| {
        let Ok(message) = event.dyn_into::<web_sys::MessageEvent>() else {
            return;
        };
        let data = app.borrow();
        if message.origin() != data.window.location().origin().unwrap_or_default() {
            return;
        }
        let parent = data.window.parent().ok().flatten();
        let source = js_sys::Reflect::get(message.as_ref(), &"source".into()).ok();
        if parent
            .as_ref()
            .is_none_or(|p| source.as_ref() != Some(p.as_ref()))
        {
            return;
        }
        let Some(text) = message.data().as_string().filter(|s| s.len() < 4096) else {
            return;
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            return;
        };
        if value["robot_lab"] != "selected" || value["key"] != data.key {
            return;
        }
        drop(data);
        app.borrow_mut().selected = value["value"].as_bool().unwrap_or(false);
        lifecycle(app);
    })?;
    let weak = Rc::downgrade(app);
    let callback = Closure::wrap(Box::new(move |entries: js_sys::Array| {
        if let Some(app) = weak.upgrade() {
            for entry in entries.iter() {
                if let Ok(entry) = entry.dyn_into::<web_sys::IntersectionObserverEntry>() {
                    app.borrow_mut().visible = entry.is_intersecting();
                    lifecycle(&app);
                }
            }
        }
    }) as Box<dyn FnMut(js_sys::Array)>);
    let observer =
        web_sys::IntersectionObserver::new(callback.as_ref().unchecked_ref()).map_err(js)?;
    observer.observe(
        document
            .body()
            .ok_or("The player is missing its page")?
            .unchecked_ref(),
    );
    app.borrow_mut().observer = Some(observer);
    app.borrow_mut().observer_callback = Some(callback);
    Ok(())
}

#[wasm_bindgen]
pub fn start() {
    console_error_panic_hook::set_once();
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    if document.get_element_by_id("robot-canvas").is_none() {
        start_cards(window, document);
        return;
    }
    spawn_local(async move {
        if let Err(error) = open(window, document.clone()).await {
            status(&document, &error);
        }
    });
}
async fn open(window: Window, document: Document) -> Result<(), String> {
    let params = web_sys::UrlSearchParams::new_with_str(&window.location().search().map_err(js)?)
        .map_err(js)?;
    let document_id = params
        .get("document")
        .ok_or("Choose a robot scene to open")?;
    let version = params
        .get("version")
        .ok_or("Choose a saved robot version")?;
    let reference = robot_lab::Reference {
        document: document_id.clone(),
        version: version.clone(),
    };
    reference.validate().map_err(|e| e.to_string())?;
    let path = window.location().pathname().map_err(js)?;
    let prefix = path.strip_suffix("/play").unwrap_or("");
    let base = format!("{}{}", window.location().origin().map_err(js)?, prefix);
    let loaded = bundle(&window, &base, &document_id, &version).await?;
    let mut initial = State::new(loaded)?;
    let saved = if let Some(chat) = params.get("chat") {
        json_value(
            &window,
            &format!("{base}/api/tool/robot_restore"),
            Some(&json!({"chat":chat})),
        )
        .await
        .ok()
        .and_then(|value| {
            serde_json::from_value::<Option<robot_lab::Selection>>(
                value["result"]["selection"].clone(),
            )
            .ok()
        })
        .flatten()
    } else {
        None
    };
    let saved =
        saved.filter(|s| s.episode.as_ref().unwrap_or(&s.scene) == &initial.bundle().subject);
    if let Some(saved) = &saved {
        if let Some(frame) = saved.step {
            initial.seek(frame as usize)?;
        }
        initial.choose_camera(&saved.camera)?;
    }
    let canvas: HtmlCanvasElement = element(&document, "robot-canvas")?.dyn_into().map_err(js)?;
    let comparison = match (
        params.get("compare_document"),
        params.get("compare_version"),
    ) {
        (Some(id), Some(version)) => {
            Some(State::new(bundle(&window, &base, &id, &version).await?)?)
        }
        _ => {
            if let Some(reference) = saved.as_ref().and_then(|s| s.comparison.as_ref()) {
                Some(State::new(
                    bundle(&window, &base, &reference.document, &reference.version).await?,
                )?)
            } else {
                None
            }
        }
    };
    let standalone = window.parent().ok().flatten().is_none_or(|p| p == window);
    let app = Rc::new(RefCell::new(App {
        window,
        document: document.clone(),
        canvas,
        base: base.clone(),
        chat: params.get("chat"),
        key: params.get("key").unwrap_or_else(|| version.clone()),
        state: initial,
        comparison,
        robot: None,
        other_robot: None,
        renderer: None,
        opening: false,
        abort: None,
        visible: true,
        selected: standalone,
        dirty: false,
        last_pointer: None,
        comparison_request: 0,
        listeners: vec![],
        raf: None,
        raf_id: None,
        observer: None,
        observer_callback: None,
        frames: 0,
        first_frame: None,
        last_frame: None,
        frame_ms: 0.0,
        pose_ms: 0.0,
        pose_started: None,
    }));
    element(&document, "poster")?
        .set_attribute(
            "src",
            &format!("{base}/api/poster?document={document_id}&version={version}"),
        )
        .map_err(js)?;
    element(&document, "poster")?
        .remove_attribute("hidden")
        .map_err(js)?;
    controls(&app)?;
    comparison_controls(&app)?;
    events(&app)?;
    let weak = Rc::downgrade(&app);
    app.borrow_mut().raf = Some(Closure::wrap(Box::new(move |time: f64| {
        if let Some(app) = weak.upgrade() {
            draw(&app, time);
        }
    }) as Box<dyn FnMut(f64)>));
    ROOT.with(|root| *root.borrow_mut() = Some(app.clone()));
    let notes = app.clone();
    spawn_local(async move {
        let _ = load_notes(&notes).await;
    });
    notify(&app.borrow(), "ready", json!({"restored":saved.is_some()}));
    lifecycle(&app);
    Ok(())
}

/// Select one authenticated inline frame. The containing chat registers only
/// its own same-origin frames; a message cannot select an arbitrary window.
fn start_cards(window: Window, document: Document) {
    if let Some(root) = document.document_element() {
        if root.has_attribute("data-robot-manager") {
            return;
        }
        let _ = root.set_attribute("data-robot-manager", "");
    }
    let origin = window.location().origin().unwrap_or_default();
    let weak_window = window.clone();
    let callback = Closure::wrap(Box::new(move |message: web_sys::MessageEvent| {
        if message.origin() != origin {
            return;
        }
        let Some(text) = message.data().as_string().filter(|s| s.len() < 16384) else {
            return;
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            return;
        };
        let Some(key) = value["key"].as_str() else {
            return;
        };
        let source = js_sys::Reflect::get(message.as_ref(), &"source".into()).ok();
        let Ok(frames) = document.query_selector_all("iframe[data-robot-card]") else {
            return;
        };
        let mut sender = None;
        for index in 0..frames.length() {
            if let Some(frame) = frames
                .item(index)
                .and_then(|n| n.dyn_into::<web_sys::HtmlIFrameElement>().ok())
            {
                if frame.get_attribute("data-robot-card").as_deref() == Some(key)
                    && frame
                        .content_window()
                        .as_ref()
                        .is_some_and(|w| source.as_ref() == Some(w.as_ref()))
                {
                    sender = Some(frame);
                    break;
                }
            }
        }
        let Some(sender) = sender else { return };
        match value["robot_lab"].as_str() {
            Some("select" | "ready") => {
                if value["robot_lab"] == "select" {
                    if let Some(root) = document.document_element() {
                        let _ = root.set_attribute("data-robot-chosen", key);
                    }
                } else if value["value"]["restored"] == true {
                    let _ = sender.set_attribute("data-robot-restored", "");
                }
                let selected = if value["robot_lab"] == "ready" {
                    let chosen = document
                        .document_element()
                        .and_then(|root| root.get_attribute("data-robot-chosen"));
                    (0..frames.length())
                        .filter_map(|index| {
                            frames
                                .item(index)
                                .and_then(|n| n.dyn_into::<web_sys::HtmlIFrameElement>().ok())
                        })
                        .find(|frame| {
                            if let Some(chosen) = &chosen {
                                frame.get_attribute("data-robot-card").as_ref() == Some(chosen)
                            } else {
                                frame.has_attribute("data-robot-restored")
                            }
                        })
                        .or_else(|| {
                            frames
                                .item(frames.length().saturating_sub(1))
                                .and_then(|n| n.dyn_into::<web_sys::HtmlIFrameElement>().ok())
                        })
                        .unwrap_or(sender.clone())
                } else {
                    sender.clone()
                };
                for index in 0..frames.length() {
                    if let Some(frame) = frames
                        .item(index)
                        .and_then(|n| n.dyn_into::<web_sys::HtmlIFrameElement>().ok())
                    {
                        if let Some(target) = frame.content_window() {
                            let active = frame == selected;
                            let key = frame.get_attribute("data-robot-card").unwrap_or_default();
                            let _ = target.post_message(
                                &json!({"robot_lab":"selected","key":key,"value":active})
                                    .to_string()
                                    .into(),
                                &origin,
                            );
                        }
                    }
                }
            }
            Some("expand") => {
                let style = sender.style();
                if value["value"].as_bool() == Some(true) {
                    for (key, value) in [
                        ("position", "fixed"),
                        ("inset", "0"),
                        ("width", "100vw"),
                        ("height", "100vh"),
                        ("z-index", "9999"),
                    ] {
                        let _ = style.set_property(key, value);
                    }
                } else {
                    for key in ["position", "inset", "width", "height", "z-index"] {
                        let _ = style.remove_property(key);
                    }
                }
            }
            Some("resources") => {
                let _ = sender.set_attribute(
                    "data-robot-gpu",
                    if value["value"]["gpu"] == true {
                        "active"
                    } else {
                        "off"
                    },
                );
            }
            Some("saved") => {
                // The chat adapter owns adding the new card. Dispatch data to
                // its authenticated endpoint; never trust a player URL.
                if let Some(endpoint) = sender.get_attribute("data-robot-save") {
                    let window = weak_window.clone();
                    let body = value["value"].clone();
                    spawn_local(async move {
                        if let Ok(reply) = json_value(&window, &endpoint, Some(&body)).await {
                            if let Some(url) = reply["reload"].as_str() {
                                if url.starts_with("/chat/") && !url.contains(['\n', '\r']) {
                                    let _ = window.location().set_href(url);
                                }
                            }
                        }
                    });
                }
            }
            _ => (),
        }
    }) as Box<dyn FnMut(web_sys::MessageEvent)>);
    if window
        .add_event_listener_with_callback("message", callback.as_ref().unchecked_ref())
        .is_ok()
    {
        callback.forget();
    }
}

#[wasm_bindgen]
pub fn metrics() -> String {
    ROOT.with(|root|root.borrow().as_ref().map_or_else(||"{}".into(),|app|{
    let data=app.borrow();json!({"active_gpu":data.renderer.is_some(),"frames":data.frames,"frame_ms":data.frame_ms,"pose_ms":data.pose_ms,"subject":data.state.bundle().subject,"frame":data.state.frame,"camera":data.state.camera,"comparison":data.comparison.as_ref().map(|s|&s.bundle().subject)}).to_string()
}))
}
