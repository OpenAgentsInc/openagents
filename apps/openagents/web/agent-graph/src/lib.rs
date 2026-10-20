//! AgentGraph in a browser: GPUI draws any agent's graph
//! (`openagents.agent-graph.v1`, from `crates/agent-graph`) into a canvas
//! through `openagents-gpui-web`.
//!
//! The page hands [`mount`] the graph's JSON, so one module draws every
//! agent. The canvas goes in the page's `#gpui-mount` box. Units are drawn
//! after Unit (samuelmtimbo/unit, MIT): rounded boxes with typed pins on
//! their edges, graph units holding their children, and data moving along
//! the links.

use agent_graph::GraphSpec;

#[cfg(target_family = "wasm")]
mod accessibility;
pub mod assets;

pub use openagents_agent_graph_view::{GraphView, theme};

/// A graph from the page, checked before anything draws it.
pub fn parse(json: &str) -> Result<GraphSpec, String> {
    let spec: GraphSpec = serde_json::from_str(json).map_err(|e| e.to_string())?;
    spec.validate().map_err(|e| e.to_string())?;
    Ok(spec)
}

#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

/// Draws `spec` (a graph's JSON) in the page. `still` turns off the moving
/// data, for readers who ask for less motion. `fonts` holds the
/// [`assets::FONT_FILES`] faces, one `Uint8Array` each, in that order.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn mount(
    spec: &str,
    still: bool,
    chat: Option<String>,
    fonts: js_sys::Array,
) -> Result<(), JsValue> {
    use std::rc::Rc;

    use gpui::{AppContext, Application, WindowBounds, WindowOptions, px, size};
    use openagents_gpui_web::{WebFonts, WebPlatform};

    console_error_panic_hook::set_once();
    openagents_gpui_web::init_logging();
    let spec = parse(spec).map_err(|e| JsValue::from_str(&e))?;
    let document = web_sys::window().unwrap().document().unwrap();
    if let Some(mount) = document.get_element_by_id("gpui-mount") {
        let button = document.create_element("button")?;
        button.set_text_content(Some("Scroll past graph"));
        button.set_attribute("type", "button")?;
        // A skip link: out of the layout and out of sight until it has
        // focus, so adding it moves nothing on the page (the graph's first
        // frame is already showing).
        let hide = |button: &web_sys::HtmlElement, hidden: bool| {
            let style = button.style();
            let (clip, size) = if hidden {
                ("inset(50%)", "1px")
            } else {
                ("none", "auto")
            };
            style.set_property("position", "absolute").ok();
            style.set_property("z-index", "3").ok();
            style.set_property("clip-path", clip).ok();
            style.set_property("width", size).ok();
            style.set_property("height", size).ok();
            style.set_property("overflow", "hidden").ok();
            style.set_property("white-space", "nowrap").ok();
        };
        let element: web_sys::HtmlElement = button.clone().dyn_into()?;
        hide(&element, true);
        for (event, hidden) in [("focus", false), ("blur", true)] {
            let target = element.clone();
            let toggle = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| hide(&target, hidden));
            button.add_event_listener_with_callback(event, toggle.as_ref().unchecked_ref())?;
            toggle.forget();
        }
        mount.insert_adjacent_element("beforebegin", &button)?;
        let skip = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            if let Some(window) = web_sys::window() {
                window.scroll_by_with_x_and_y(0.0, mount.get_bounding_client_rect().bottom());
            }
        });
        button.add_event_listener_with_callback("click", skip.as_ref().unchecked_ref())?;
        skip.forget();
    }
    let events = chat
        .as_deref()
        .map(agent_graph::events::from_chat)
        .transpose()
        .map_err(|e| JsValue::from_str(&e.to_string()))?
        .unwrap_or_default();
    if fonts.length() as usize != assets::FONT_FILES.len() {
        return Err(JsValue::from_str(
            "the page didn't hand over the graph's fonts",
        ));
    }
    let platform = WebPlatform::new(false, WebFonts::from_js(theme::SANS, &fonts));
    // The browser owns the run loop: hold the embedded handle for the life
    // of the page.
    let handle = Application::with_platform(Rc::new(platform)).run_embedded(move |cx| {
        let bounds = gpui::Bounds::centered(None, size(px(1200.), px(760.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            move |_, cx| {
                cx.new(|cx| {
                    let view = GraphView::new(spec.clone(), still, events)
                        .with_selection_hook(accessibility::sync);
                    accessibility::install(&spec, &view.tab_order(), cx);
                    view
                })
            },
        )
        .expect("the page has a box to draw into");
        cx.activate(true);
    });
    std::mem::forget(handle);
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_graph_must_be_valid_to_draw() {
        assert!(super::parse("{}").is_err());
        let mut spec = agent_graph::GraphSpec::new("t", "T", "");
        spec.units.push(agent_graph::Unit::new(
            "a",
            "A",
            "",
            agent_graph::Accent::Info,
        ));
        let json = serde_json::to_string(&spec).unwrap();
        assert_eq!(super::parse(&json).unwrap(), spec);
        spec.units.push(spec.units[0].clone());
        let json = serde_json::to_string(&spec).unwrap();
        assert!(super::parse(&json).is_err());
    }
}
