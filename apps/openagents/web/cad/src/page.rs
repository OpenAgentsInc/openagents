use gpui::Context;
use openagents_chat_view::settings_forms::cad::{Action, State, view};
use openagents_web_page_host::{Host, Intent, Load, Page, Paints, Prompt, plain};
use rust_native::Node;
use serde_json::{Value, json};
use wasm_bindgen::{JsCast, closure::Closure};

const PLAYER: f32 = 360.;

pub struct Cad {
    state: State,
    path: String,
    generation: u64,
    pick: Option<Closure<dyn FnMut(web_sys::MessageEvent)>>,
}
type Cx<'a> = Context<'a, Host<Cad>>;
fn encode(text: &str) -> String {
    String::from(js_sys::encode_uri_component(text))
}
fn path(doc: &str, version: u64, project: &str, config: bool) -> String {
    if doc.is_empty() {
        String::new()
    } else {
        format!(
            "{doc}/{version}/{project}{}",
            if config { "/configurator" } else { "" }
        )
    }
}
impl Page for Cad {
    const NAME: &'static str = "cad";
    fn new() -> Self {
        Self {
            state: State::default(),
            path: String::new(),
            generation: 0,
            pick: None,
        }
    }
    fn title(&self) -> String {
        self.state
            .data
            .ready()
            .and_then(|d| d["title"].as_str())
            .unwrap_or(if self.state.world {
                "Designs in your world"
            } else {
                "Your designs"
            })
            .into()
    }
    fn view(&self, paints: &Paints) -> Node<Intent> {
        view(&self.state, paints)
    }
    fn pinned(&self) -> Option<f32> {
        (!self.state.world
            && !self.state.configurator
            && !self.state.document.is_empty()
            && self.state.data.ready().is_some())
        .then_some(PLAYER)
    }
    fn open(host: &mut Host<Self>, value: &str, cx: &mut Cx) {
        if host.page.pick.is_none() {
            install_pick(host, cx);
        }
        load(host, value.to_owned(), cx);
    }
    fn intent(host: &mut Host<Self>, intent: Intent, cx: &mut Cx) {
        let Intent::Cad(action) = intent else { return };
        if host.page.state.busy {
            return;
        }
        match action {
            Action::List => load(host, String::new(), cx),
            Action::Open(doc, version) => load(
                host,
                path(&doc, version, &host.page.state.project, false),
                cx,
            ),
            Action::Configurator => {
                let state = &host.page.state;
                let version = state
                    .data
                    .ready()
                    .and_then(|d| d["version"].as_u64())
                    .unwrap_or(0);
                load(
                    host,
                    path(&state.document, version, &state.project, true),
                    cx,
                );
            }
            Action::Reload => load(host, host.page.path.clone(), cx),
            Action::Visit(url) => host.go(&url),
            Action::Download(format) => {
                if let Some(frame) = web_sys::window()
                    .and_then(|w| w.document())
                    .and_then(|d| d.get_element_by_id("cad-player"))
                    .and_then(|f| f.dyn_into::<web_sys::HtmlIFrameElement>().ok())
                    .and_then(|f| f.content_window())
                {
                    if let Some(origin) = web_sys::window().and_then(|w| w.location().origin().ok())
                    {
                        let _ =
                            frame.post_message(&wasm_bindgen::JsValue::from_str(&format), &origin);
                    }
                }
            }
            Action::MoveFace => {
                if host.page.state.face.is_some()
                    && host
                        .page
                        .state
                        .data
                        .ready()
                        .is_some_and(|d| d["edits"] == true)
                {
                    host.ask(Prompt {purpose:"move".into(),label:"Move the selected face by this distance (mm). Saving creates a new version.".into(),placeholder:"Distance in mm".into(),value:"1".into()},cx);
                }
            }
            Action::Toggle(i) => {
                let state = &mut host.page.state;
                let Some(option) = state
                    .data
                    .ready()
                    .and_then(|d| d["options"].get(i))
                    .cloned()
                else {
                    return;
                };
                if let Some(index) = state
                    .controls
                    .iter()
                    .position(|c| c["path"] == option["path"])
                {
                    state.controls.remove(index);
                } else if state.controls.len() < 8 {
                    let value = option["value"].as_f64().unwrap_or(0.);
                    state.controls.push(json!({"path":option["path"],"label":option["label"],"min":value.min(value*0.5)-1.,"max":value.max(value*1.5)+1.,"step":0.1}));
                } else {
                    host.say(Some("Choose up to eight sliders.".into()), cx);
                }
            }
            Action::Edit(i, field) => {
                let Some(value) = host.page.state.controls.get(i).and_then(|c| c.get(&field))
                else {
                    return;
                };
                host.ask(
                    Prompt {
                        purpose: format!("control/{i}/{field}"),
                        label: match field.as_str() {
                            "label" => "Slider name",
                            "min" => "Minimum",
                            "max" => "Maximum",
                            _ => "Step",
                        }
                        .into(),
                        placeholder: String::new(),
                        value: value
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| value.to_string()),
                    },
                    cx,
                );
            }
            Action::Publish => {
                let state = &host.page.state;
                let version = state
                    .data
                    .ready()
                    .and_then(|d| d["version"].as_u64())
                    .unwrap_or(0);
                let body = json!({"project":state.project,"version":version,"controls":serde_json::to_string(&state.controls).unwrap_or_default(),"mode":"free"});
                save(host, "configurator", body, cx);
            }
            Action::Revoke(token) => {
                let body = json!({"project":host.page.state.project,"token":token});
                save(host, "configurator/revoke", body, cx);
            }
        }
    }
    fn submit(host: &mut Host<Self>, prompt: &Prompt, text: String, cx: &mut Cx) {
        if host.page.state.busy {
            return;
        }
        if prompt.purpose == "move" {
            let Some((part, face)) = host.page.state.face.clone() else {
                return;
            };
            let Some(delta) = text
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite() && *n != 0.)
            else {
                host.say(Some("Enter a distance other than zero.".into()), cx);
                return;
            };
            let state = &host.page.state;
            let version = state
                .data
                .ready()
                .and_then(|d| d["version"].as_u64())
                .unwrap_or(0);
            save(
                host,
                "pull",
                json!({"project":state.project,"version":version,"part":part,"face":face,"delta":delta}),
                cx,
            );
        } else if let Some(rest) = prompt.purpose.strip_prefix("control/") {
            let Some((i, field)) = rest.split_once('/') else {
                return;
            };
            let Some(control) = i
                .parse::<usize>()
                .ok()
                .and_then(|i| host.page.state.controls.get_mut(i))
            else {
                return;
            };
            let value = if field == "label" {
                json!(text.trim())
            } else {
                let Some(number) = text.parse::<f64>().ok().filter(|n| n.is_finite()) else {
                    host.say(Some("Enter a number.".into()), cx);
                    return;
                };
                json!(number)
            };
            control[field] = value;
        }
    }
}

fn load(host: &mut Host<Cad>, value: String, cx: &mut Cx) {
    host.page.generation += 1;
    let generation = host.page.generation;
    host.unask(cx);
    let mut parts = value.split('/');
    let doc = parts.next().unwrap_or("").to_owned();
    let world = doc == "_world";
    let version = parts
        .next()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let project = parts.next().unwrap_or("").to_owned();
    let config = parts.next() == Some("configurator");
    host.page.path = path(&doc, version, &project, config);
    host.page.state = State {
        document: doc.clone(),
        project: project.clone(),
        configurator: config,
        world,
        data: Load::Loading,
        ..State::default()
    };
    host.moved(&host.page.path);
    player(None);
    let stamp = host.page.path.clone();
    let client = host.client.clone();
    cx.spawn(async move |this, cx| {
        let endpoint = if world {
            "/v1/cad/world/placements".into()
        } else if doc.is_empty() {
            "/v1/cad".into()
        } else {
            format!(
                "/v1/cad/{}{}",
                encode(&doc),
                if config { "/configurator" } else { "" }
            )
        };
        let data = client
            .get::<Value>(&format!(
                "{endpoint}?v={version}&project={}",
                encode(&project)
            ))
            .await;
        this.update(cx, |host, cx| {
            if host.page.path != stamp || host.page.generation != generation {
                return;
            }
            match data {
                Ok(data) => {
                    let version = data["version"].as_u64().unwrap_or(0);
                    if !world && !doc.is_empty() && !config {
                        player(Some(&format!(
                            "/cad/{}/viewer?v={version}&project={}",
                            encode(&doc),
                            encode(&project)
                        )));
                    }
                    host.page.state.data = Load::Ready(data);
                }
                Err(error) => host.page.state.data = Load::Failed(plain(&error)),
            }
            host.changed(cx);
        })
        .ok();
    })
    .detach();
}
fn save(host: &mut Host<Cad>, suffix: &str, body: Value, cx: &mut Cx) {
    host.page.state.busy = true;
    let stamp = host.page.path.clone();
    let generation = host.page.generation;
    let endpoint = format!("/v1/cad/{}/{}", encode(&host.page.state.document), suffix);
    let suffix = suffix.to_owned();
    let client = host.client.clone();
    cx.spawn(async move |this, cx| {
        let result = client.send::<Value>("POST", &endpoint, Some(&body)).await;
        this.update(cx, |host, cx| {
            if host.page.path != stamp || host.page.generation != generation {
                return;
            }
            host.page.state.busy = false;
            match result {
                Ok(data) => {
                    if suffix == "configurator" {
                        host.go(data["url"].as_str().unwrap_or("/cad"));
                    } else {
                        let next = if suffix == "pull" {
                            path(
                                &host.page.state.document,
                                data["version"].as_u64().unwrap_or(0),
                                &host.page.state.project,
                                false,
                            )
                        } else {
                            stamp
                        };
                        load(host, next, cx);
                        host.say(
                            Some(
                                if suffix == "pull" {
                                    "Version saved."
                                } else {
                                    "Sharing stopped."
                                }
                                .into(),
                            ),
                            cx,
                        );
                    }
                }
                Err(error) => host.say(Some(plain(&error)), cx),
            }
            host.changed(cx);
        })
        .ok();
    })
    .detach();
}
fn player(src: Option<&str>) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    if let Some(old) = document.get_element_by_id("cad-player") {
        old.remove();
    }
    let (Some(src), Some(parent)) = (src, document.get_element_by_id("gpui-page-cad")) else {
        return;
    };
    let Some(frame) = document
        .create_element("iframe")
        .ok()
        .and_then(|frame| frame.dyn_into::<web_sys::HtmlIFrameElement>().ok())
    else {
        return;
    };
    frame.set_id("cad-player");
    let _ = frame.set_attribute("src", src);
    let _ = frame.set_attribute("title", "Design viewer");
    frame.style().set_css_text(&format!(
        "position:absolute;top:52px;left:0;width:100%;height:{PLAYER}px;border:0;"
    ));
    let _ = parent.append_child(&frame);
}
fn install_pick(host: &mut Host<Cad>, cx: &mut Cx) {
    let entity = cx.entity().downgrade();
    let mut async_cx = cx.to_async();
    let callback =
        Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
            let Some(window) = web_sys::window() else {
                return;
            };
            if window.location().origin().ok().as_deref() != Some(event.origin().as_str()) {
                return;
            }
            let Some(frame) = window
                .document()
                .and_then(|d| d.get_element_by_id("cad-player"))
                .and_then(|f| f.dyn_into::<web_sys::HtmlIFrameElement>().ok())
            else {
                return;
            };
            let Some(source) = event.source() else { return };
            if frame
                .content_window()
                .is_none_or(|w| !js_sys::Object::is(w.as_ref(), source.as_ref()))
            {
                return;
            }
            let Some(text) = js_sys::JSON::stringify(&event.data())
                .ok()
                .and_then(|s| s.as_string())
            else {
                return;
            };
            let Ok(value) = serde_json::from_str::<Value>(&text) else {
                return;
            };
            let _ = entity.update(&mut async_cx, |host, cx| {
                if value["cadReady"] == true {
                    host.page.state.files_ready = true;
                }
                if let Some(status) = value["cadStatus"].as_str() {
                    host.say(Some(status.into()), cx);
                }
                if !host.page.state.busy {
                    if let (Some(part), Some(face)) = (
                        value["cadFace"]["part"].as_str(),
                        value["cadFace"]["face"].as_str(),
                    ) {
                        host.page.state.face = Some((part.into(), face.into()));
                    }
                }
                host.changed(cx);
            });
        });
    if let Some(window) = web_sys::window() {
        let _ =
            window.add_event_listener_with_callback("message", callback.as_ref().unchecked_ref());
    }
    host.page.pick = Some(callback);
}
