use gpui::{prelude::*, *};
use openagents_gpui_web::{WebFonts, WebPlatform};
use openagents_web_chat_view::{assets, theme, visuals};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use wasm_bindgen::{JsCast, prelude::*};

// Only the bundled scripts are used here. Native file reads never serve the web.
#[allow(dead_code, unused_imports)]
#[path = "../../../crates/client/src/demo/visuals.rs"]
mod demo;

struct Host {
    platform: Rc<WebPlatform>,
    cx: AsyncApp,
    slots: HashMap<String, (AnyWindowHandle, web_sys::HtmlElement)>,
}
thread_local! {
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
    static LAST: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

#[wasm_bindgen]
pub fn mount() {
    console_error_panic_hook::set_once();
    let platform = Rc::new(WebPlatform::new(
        false,
        WebFonts {
            fallback_family: theme::SANS,
            faces: vec![],
        },
    ));
    let windows = platform.clone();
    let handle = Application::with_platform(platform)
        .with_assets(assets::Assets)
        .run_embedded(move |cx| {
            openagents_web_chat_view::install(cx);
            HOST.with(|host| {
                *host.borrow_mut() = Some(Host {
                    platform: windows,
                    cx: cx.to_async(),
                    slots: HashMap::new(),
                })
            });
            // Wait until the launch callback releases its App borrow.
            wasm_bindgen_futures::spawn_local(async {
                reposition();
            });
        });
    std::mem::forget(handle);
}

#[wasm_bindgen]
pub fn update(value: String) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&value) else {
        return;
    };
    let Some(id) = v["id"].as_str() else { return };
    LAST.with(|last| {
        let mut last = last.borrow_mut();
        if v["remove"] == true || v["hide"] == true {
            last.remove(id);
        } else {
            last.insert(id.to_owned(), value.clone());
        }
    });
    HOST.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(host) = slot.as_mut() else {
            return;
        };
        if v["remove"] == true {
            if let Some((handle, element)) = host.slots.remove(id) {
                let _ = handle.update(&mut host.cx, |_, window, _| window.remove_window());
                element.remove();
            }
            return;
        }
        if v["hide"] == true {
            if let Some((_, element)) = host.slots.get(id) {
                let _ = element.style().set_property("display", "none");
            }
            return;
        }
        let Some(document) = web_sys::window().and_then(|w| w.document()) else {
            return;
        };
        let Some(source) = document
            .query_selector("canvas:not([data-oa-visual])")
            .ok()
            .flatten()
        else {
            return;
        };
        let source_rect = source.get_bounding_client_rect();
        let number = |key| v[key].as_f64().unwrap_or(0.);
        if !host.slots.contains_key(id) {
            let Ok(element) = document
                .create_element("div")
                .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().map_err(Into::into))
            else {
                return;
            };
            element.set_id(id);
            let _ = element.set_attribute("aria-label", v["title"].as_str().unwrap_or("Visual"));
            element.style().set_property("position", "fixed").ok();
            element.style().set_property("z-index", "2").ok();
            let Some(body) = document.body() else { return };
            body.append_child(&element).ok();
            host.platform.mount_in(id);
            let title = v["title"].as_str().unwrap_or("Visual").to_owned();
            let path = v["path"].as_str().unwrap_or_default().to_owned();
            let own_id = id.to_owned();
            let device = v["device"].as_str().map(str::to_owned);
            let opened = host.cx.update(|cx| {
                cx.open_window(WindowOptions::default(), |_, cx| {
                    cx.new(|_| View {
                        id: own_id,
                        title,
                        path,
                        device,
                        frames: visuals::Frames::default(),
                        last_height: Rc::new(std::cell::Cell::new(0.)),
                    })
                })
            });
            let Ok(window) = opened else {
                element.remove();
                return;
            };
            if let Ok(Some(canvas)) = element.query_selector("canvas") {
                canvas.set_attribute("data-oa-visual", "").ok();
            }
            host.slots.insert(id.to_owned(), (window.into(), element));
        }
        let (_, element) = &host.slots[id];
        let x = number("x");
        let y = number("y");
        let width = number("width");
        let height = number("height");
        let clip = &v["clip"];
        let c = |i| clip[i].as_f64().unwrap_or(0.);
        let style = element.style();
        for (name, value) in [
            ("display", "block".into()),
            ("left", format!("{}px", source_rect.x() + x)),
            ("top", format!("{}px", source_rect.y() + y)),
            ("width", format!("{width}px")),
            ("height", format!("{height}px")),
            (
                "clip-path",
                format!(
                    "inset({}px {}px {}px {}px)",
                    (c(1) - y).max(0.),
                    (x + width - c(0) - c(2)).max(0.),
                    (y + height - c(1) - c(3)).max(0.),
                    (c(0) - x).max(0.)
                ),
            ),
        ] {
            style.set_property(name, &value).ok();
        }
    });
}

struct View {
    id: String,
    title: String,
    path: String,
    device: Option<String>,
    frames: visuals::Frames,
    last_height: Rc<std::cell::Cell<f32>>,
}
impl Render for View {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let browser = visuals::browser_reader();
        let device = self.device.clone();
        let read: visuals::Read = Rc::new(move |path| {
            let device = device.clone();
            let browser = browser.clone();
            Box::pin(async move {
                // Demo paths match exactly; no filesystem or arbitrary URL reads.
                if let Some((_, _, script)) = demo::DEMO_VISUALS
                    .iter()
                    .find(|(id, ..)| demo::path(id) == path)
                {
                    Some(script.as_bytes().to_vec())
                } else if path.starts_with("/chat/") || path.starts_with("/v1/") {
                    browser(path).await
                } else if let Some(device) = device {
                    attachment(&device, &path).await
                } else {
                    browser(path).await
                }
            })
        });
        let visual = self.frames.render(
            self.id.clone(),
            &self.title,
            &self.path,
            read,
            |_: &mut View, cx| cx.notify(),
            cx,
        );
        let id = self.id.clone();
        let last = self.last_height.clone();
        div().w_full().bg(theme::bg()).child(visual).child(
            canvas(
                move |bounds, _, _| {
                    // This zero-height marker follows the naturally sized visual.
                    let height = f32::from(bounds.origin.y).clamp(32., 4096.);
                    if (height - last.get()).abs() > 1. {
                        last.set(height);
                        let init = web_sys::CustomEventInit::new();
                        init.set_detail(
                            &serde_json::json!({"id": id, "height": height})
                                .to_string()
                                .into(),
                        );
                        if let (Some(window), Ok(event)) = (
                            web_sys::window(),
                            web_sys::CustomEvent::new_with_event_init_dict(
                                "oa-visual-height",
                                &init,
                            ),
                        ) {
                            window.dispatch_event(&event).ok();
                        }
                    }
                },
                |_, _, _, _| {},
            )
            .h(px(0.))
            .w_full(),
        )
    }
}

/// Reads through the existing computer grant and upload path restrictions.
async fn attachment(device: &str, path: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    let mut bytes = Vec::new();
    let mut offset = 0;
    loop {
        let reply = openagents_web_client::computer::computer_call(
            device.to_owned(),
            "ReadAttachmentChunk".into(),
            serde_json::json!({"path": path, "offset": offset}).to_string(),
        )
        .await
        .ok()?;
        let reply: serde_json::Value = serde_json::from_str(&reply).ok()?;
        let chunk = base64::engine::general_purpose::STANDARD
            .decode(reply["data"].as_str()?)
            .ok()?;
        if bytes.len() + chunk.len() > 16 * 1024 * 1024 {
            return None;
        }
        bytes.extend(chunk);
        if reply["done"].as_bool()? {
            return Some(bytes);
        }
        let next = reply["nextOffset"].as_u64()?;
        if next <= offset {
            return None;
        }
        offset = next;
    }
}

/// Follows document scrolling even when the parent GPUI frame did not change.
#[wasm_bindgen]
pub fn reposition() {
    let values: Vec<String> = LAST.with(|last| last.borrow().values().cloned().collect());
    for value in values {
        update(value);
    }
}
