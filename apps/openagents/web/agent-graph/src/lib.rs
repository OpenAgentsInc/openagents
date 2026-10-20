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
mod app;
#[cfg(target_family = "wasm")]
mod assets;
#[cfg(target_family = "wasm")]
mod theme;

/// A graph from the page, checked before anything draws it.
pub fn parse(json: &str) -> Result<GraphSpec, String> {
    let spec: GraphSpec = serde_json::from_str(json).map_err(|e| e.to_string())?;
    spec.validate().map_err(|e| e.to_string())?;
    Ok(spec)
}

#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

/// Draws `spec` (a graph's JSON) in the page. `still` turns off the moving
/// data, for readers who ask for less motion.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn mount(spec: &str, still: bool, chat: Option<String>) -> Result<(), JsValue> {
    use std::rc::Rc;

    use gpui::{AppContext, Application, WindowBounds, WindowOptions, px, size};
    use openagents_gpui_web::{WebFonts, WebPlatform};

    console_error_panic_hook::set_once();
    openagents_gpui_web::init_logging();
    let spec = parse(spec).map_err(|e| JsValue::from_str(&e))?;
    let events = chat
        .as_deref()
        .map(agent_graph::events::from_chat)
        .transpose()
        .map_err(|e| JsValue::from_str(&e.to_string()))?
        .unwrap_or_default();
    let platform = WebPlatform::new(
        false,
        WebFonts {
            fallback_family: theme::SANS,
            faces: assets::font_faces(),
        },
    );
    // The browser owns the run loop: hold the embedded handle for the life
    // of the page.
    let handle = Application::with_platform(Rc::new(platform)).run_embedded(move |cx| {
        let bounds = gpui::Bounds::centered(None, size(px(1200.), px(760.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            move |_, cx| cx.new(|_| app::GraphView::new(spec, still, events)),
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
