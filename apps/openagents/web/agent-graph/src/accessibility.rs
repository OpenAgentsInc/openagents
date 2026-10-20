//! Real browser controls mirror the graph's units; the canvas and DOM share state.
use agent_graph::GraphSpec;
use gpui::{AppContext, Context};
use wasm_bindgen::{JsCast, closure::Closure};

use crate::GraphView;

pub(crate) fn install(spec: &GraphSpec, order: &[usize], cx: &mut Context<GraphView>) {
    let document = web_sys::window().unwrap().document().unwrap();
    let mount = document.get_element_by_id("gpui-mount").unwrap();
    let mirror = document.create_element("div").unwrap();
    mirror.set_id("agent-graph-mirror");
    mirror.set_attribute("role", "group").unwrap();
    mirror.set_attribute("aria-label", "Agent graph. Tab between parts, Enter to select, arrow keys to pan, plus and minus to zoom.").unwrap();
    // CSSOM also works under the site's policy, without an inline-style exception.
    mirror.unchecked_ref::<web_sys::HtmlElement>().style().set_css_text(
        "position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap",
    );
    mount.append_child(&mirror).unwrap();
    let status = document.create_element("div").unwrap();
    status.set_id("agent-graph-selection");
    status.set_attribute("role", "status").unwrap();
    status.set_attribute("aria-live", "polite").unwrap();
    status.set_attribute("aria-atomic", "true").unwrap();
    mirror.append_child(&status).unwrap();
    for &at in order {
        let unit = &spec.units[at];
        let button = document.create_element("button").unwrap();
        button.set_id(&format!("agent-graph-unit-{at}"));
        button.set_attribute("type", "button").unwrap();
        button.set_attribute("aria-pressed", "false").unwrap();
        button.set_text_content(Some(&format!("{}: {}", unit.title, unit.line)));
        mirror.append_child(&button).unwrap();
        let weak = cx.entity().downgrade();
        let app = cx.to_async();
        let listener = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(
            move |event: web_sys::KeyboardEvent| {
                if event.alt_key() || event.ctrl_key() || event.meta_key() {
                    return;
                }
                app.update(|cx| {
                    let _ = weak.update(cx, |view, cx| {
                        if view.keyboard(&event.key(), at) {
                            event.prevent_default();
                            event.stop_propagation();
                            cx.notify();
                        }
                    });
                });
            },
        );
        button
            .add_event_listener_with_callback("keydown", listener.as_ref().unchecked_ref())
            .unwrap();
        listener.forget();
        for (name, focused) in [("focus", true), ("blur", false), ("click", true)] {
            let weak = cx.entity().downgrade();
            let app = cx.to_async();
            let listener = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
                app.update(|cx| {
                    let _ = weak.update(cx, |view, cx| {
                        if name == "click" {
                            view.keyboard("Enter", at);
                        } else {
                            view.focus_unit(focused.then_some(at));
                        }
                        cx.notify();
                    });
                });
            });
            button
                .add_event_listener_with_callback(name, listener.as_ref().unchecked_ref())
                .unwrap();
            listener.forget();
        }
    }
}

pub(crate) fn sync(spec: &GraphSpec, selected: Option<usize>) {
    let document = web_sys::window().unwrap().document().unwrap();
    for at in 0..spec.units.len() {
        if let Some(button) = document.get_element_by_id(&format!("agent-graph-unit-{at}")) {
            let _ = button.set_attribute(
                "aria-pressed",
                if selected == Some(at) {
                    "true"
                } else {
                    "false"
                },
            );
        }
    }
    if let Some(status) = document.get_element_by_id("agent-graph-selection") {
        let text = selected
            .map(|at| format!("Selected {}. {}", spec.units[at].title, spec.units[at].line))
            .unwrap_or_default();
        if status.text_content().as_deref() != Some(&text) {
            status.set_text_content(Some(&text));
        }
    }
}
