//! Agent Lab in the GPUI web app (#12285), as its own module.
//!
//! The chat page's start script loads it at `?page=agent-lab` or when the
//! sidebar's Agent Lab asks (`oa-page-open`), puts its box over the chat,
//! and calls [`mount`] with the fonts the chat already fetched. Back to chat
//! sends `oa-page-close`; opening a task's chat sends `oa-open-chat` with
//! `{chat, device}`, which the chat answers by opening that chat on that
//! computer.

#[path = "lab.rs"]
pub mod agent_lab;

#[cfg(target_family = "wasm")]
mod page {
    use gpui::{
        AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div,
        prelude::*, px,
    };
    use openagents_web_chat_view::theme;

    use crate::agent_lab::{AgentLab, OpenTask};

    /// What the start script names this page.
    pub const NAME: &str = "agent-lab";

    pub struct LabPage {
        lab: Entity<AgentLab>,
    }

    impl LabPage {
        pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let lab = cx.new(AgentLab::new);
            cx.subscribe_in(&lab, window, |_, _, task: &OpenTask, _, _| {
                let detail = serde_json::json!({"chat": task.chat, "device": task.device});
                signal("oa-page-close", NAME);
                signal("oa-open-chat", &detail.to_string());
            })
            .detach();
            lab.update(cx, |lab, cx| lab.open(cx));
            Self { lab }
        }

        pub fn show(&mut self, cx: &mut Context<Self>) {
            self.lab.update(cx, |lab, cx| lab.open(cx));
        }
    }

    impl Render for LabPage {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let mut root = div()
                .size_full()
                .p(px(8.))
                .bg(theme::surface())
                .font_family(theme::SANS)
                .text_color(theme::text())
                .text_size(px(theme::BODY));
            root.text_style().font_fallbacks = Some(theme::fallbacks());
            root.child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .rounded(px(14.))
                    .border_1()
                    .border_color(theme::border())
                    .bg(theme::bg())
                    .overflow_hidden()
                    .child(
                        div()
                            .id("close-agent-lab")
                            .role(gpui::Role::Button)
                            .aria_label("Back to chat")
                            .tab_index(0)
                            .p(px(12.))
                            .cursor_pointer()
                            .child("Back to chat")
                            .on_click(|_, _, _| signal("oa-page-close", NAME))
                            .on_key_down(|event: &gpui::KeyDownEvent, _, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    signal("oa-page-close", NAME);
                                    cx.stop_propagation();
                                }
                            }),
                    )
                    .child(div().flex_1().min_h_0().child(self.lab.clone())),
            )
        }
    }

    /// Tell the page's script something on the window.
    pub fn signal(name: &str, detail: &str) {
        let init = web_sys::CustomEventInit::new();
        init.set_detail(&wasm_bindgen::JsValue::from_str(detail));
        if let Some(window) = web_sys::window()
            && let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(name, &init)
        {
            let _ = window.dispatch_event(&event);
        }
    }
}

#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg(target_family = "wasm")]
thread_local! {
    static PAGE: std::cell::RefCell<Option<(gpui::WeakEntity<page::LabPage>, gpui::AsyncApp)>> =
        const { std::cell::RefCell::new(None) };
}

/// Starts Agent Lab in the page's box, with the page's fonts, one
/// `Uint8Array` per face. `_path` is what follows `agent-lab/` in the
/// address (nothing yet).
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn mount(fonts: js_sys::Array, _path: String) {
    use std::borrow::Cow;
    use std::rc::Rc;
    use std::sync::Arc;

    use gpui::{AppContext, Application, WindowBounds, WindowOptions, px, size};
    use openagents_gpui_web::{FetchHttpClient, WebFonts, WebPlatform};
    use openagents_web_chat_view::{assets, composer, theme};

    console_error_panic_hook::set_once();
    openagents_gpui_web::init_logging();
    let faces: Vec<Cow<'static, [u8]>> = fonts
        .iter()
        .map(|face| Cow::Owned(js_sys::Uint8Array::new(&face).to_vec()))
        .collect();
    let platform = WebPlatform::new(
        false,
        WebFonts {
            fallback_family: theme::SANS,
            faces,
        },
    );
    let handle = Application::with_platform(Rc::new(platform))
        .with_assets(assets::Assets)
        .with_http_client(Arc::new(FetchHttpClient::new()))
        .run_embedded(move |cx| {
            openagents_web_chat_view::install(cx);
            composer::bind_keys(cx);
            let bounds = gpui::Bounds::centered(None, size(px(1280.), px(820.)), cx);
            let opened = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| page::LabPage::new(window, cx)),
            );
            if let Ok(entity) = opened.and_then(|window| window.entity(cx)) {
                let async_cx = cx.to_async();
                PAGE.with(|slot| *slot.borrow_mut() = Some((entity.downgrade(), async_cx)));
            }
            cx.activate(true);
        });
    std::mem::forget(handle);
}

/// Shows Agent Lab again (reads its state afresh).
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn show(_path: String) {
    PAGE.with(|slot| {
        if let Some((entity, cx)) = slot.borrow().as_ref() {
            let _ = entity.update(&mut cx.clone(), |page, cx| page.show(cx));
        }
    });
}
