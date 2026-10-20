//! Load a recorded visual and keep its interpreter alive while the transcript
//! re-renders. Hosts supply file reads (bundled demo files or chat attachments).
use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, AppContext, Context, Entity, EventEmitter, IntoElement, Render, SharedString, Task,
    Window, div, prelude::*, px,
};
use openagents_chat_view::visuals::{Controls, VisualFrame, VisualLayoutChanged};

pub type Read = Rc<dyn Fn(String) -> Pin<Box<dyn Future<Output = Option<Vec<u8>>>>>>;

#[derive(Default)]
pub struct Frames(RefCell<HashMap<String, (String, Entity<Loading>)>>);

impl Frames {
    pub fn render<T: 'static>(
        &self,
        key: String,
        title: &str,
        path: &str,
        read: Read,
        changed: impl Fn(&mut T, &mut Context<T>) + 'static,
        cx: &mut Context<T>,
    ) -> AnyElement {
        let mut frames = self.0.borrow_mut();
        if frames.get(&key).is_some_and(|(old, _)| old != path) {
            frames.remove(&key);
        }
        let (_, frame) = frames.entry(key.clone()).or_insert_with(|| {
            let frame =
                cx.new(|cx| Loading::new(key.into(), title.to_owned(), path.to_owned(), read, cx));
            cx.subscribe(&frame, move |this, _, _: &VisualLayoutChanged, cx| {
                changed(this, cx)
            })
            .detach();
            (path.to_owned(), frame)
        });
        frame.clone().into_any_element()
    }

    pub fn clear(&self) {
        self.0.borrow_mut().clear();
    }
}

struct Loading {
    visual: Option<Entity<VisualFrame>>,
    failed: bool,
    _load: Task<()>,
}
impl EventEmitter<VisualLayoutChanged> for Loading {}
impl Loading {
    fn new(
        id: SharedString,
        title: String,
        path: String,
        read: Read,
        cx: &mut Context<Self>,
    ) -> Self {
        let load = cx.spawn(async move |this, cx| {
            let script = read(path.clone())
                .await
                .and_then(|bytes| String::from_utf8(bytes).ok());
            let mut files = HashMap::new();
            if let Some(stem) = path.strip_suffix("-visual.luau") {
                if let Some(names) = read(format!("{stem}-assets.json"))
                    .await
                    .and_then(|bytes| serde_json::from_slice::<Vec<String>>(&bytes).ok())
                {
                    for name in names
                        .into_iter()
                        .filter(|n| openagents_visual::assets::valid_name(n))
                        .take(openagents_visual::assets::MAX_ASSETS)
                    {
                        if let Some(bytes) = read(format!("{stem}-asset-{name}")).await {
                            files.insert(name, Arc::<[u8]>::from(bytes));
                        }
                    }
                }
            }
            let _ = this.update(cx, |this, cx| {
                if let Some(script) = script {
                    let visual = cx.new(|cx| {
                        VisualFrame::new(
                            id,
                            title,
                            &script,
                            openagents_visual::Assets::new(files),
                            controls(),
                            cx,
                        )
                    });
                    cx.subscribe(&visual, |_, _, _: &VisualLayoutChanged, cx| {
                        cx.emit(VisualLayoutChanged);
                        cx.notify();
                    })
                    .detach();
                    this.visual = Some(visual);
                } else {
                    this.failed = true;
                }
                cx.emit(VisualLayoutChanged);
                cx.notify();
            });
        });
        Self {
            visual: None,
            failed: false,
            _load: load,
        }
    }
}
impl Render for Loading {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        match &self.visual {
            Some(visual) => div().w_full().child(visual.clone()),
            None => div()
                .text_color(crate::theme::text_muted())
                .child(if self.failed {
                    "Visual unavailable"
                } else {
                    "Loading visual…"
                }),
        }
    }
}

fn controls() -> Controls {
    Controls {
        button: |theme, primary| {
            div()
                .flex()
                .items_center()
                .rounded(px(8.))
                .min_h(px(32.))
                .px(px(10.))
                .py(px(5.))
                .text_size(px(12.5))
                .cursor_pointer()
                .bg(if primary {
                    theme.solid
                } else {
                    theme.wash(0.06)
                })
                .text_color(if primary { theme.on_solid } else { theme.text })
        },
        toggle: |theme, on, _| {
            div()
                .w(px(32.))
                .h(px(18.))
                .rounded(px(9.))
                .px(px(2.))
                .flex()
                .items_center()
                .when(on, |el| el.justify_end())
                .bg(if on { theme.accent } else { theme.border })
                .child(div().size(px(14.)).rounded(px(7.)).bg(theme.text))
        },
        tab_selection: |_, _, selected, _| if selected { 1. } else { 0. },
        tab: |theme, selected, _, id, _| {
            div()
                .id(id)
                .px(px(10.))
                .py(px(6.))
                .rounded(px(8.))
                .text_color(if selected {
                    theme.text
                } else {
                    theme.text_muted
                })
                .when(selected, |el| el.bg(theme.glass_hover()))
        },
    }
}

/// Attachment reads use the page's origin and cookie. Native filesystem paths
/// require a host reader; they must never turn into arbitrary fetches.
#[cfg(target_family = "wasm")]
pub fn browser_reader() -> Read {
    Rc::new(|path| {
        Box::pin(async move {
            use wasm_bindgen::JsCast;
            use wasm_bindgen_futures::JsFuture;
            if !(path.starts_with("/chat/") || path.starts_with("/v1/")) || path.starts_with("//") {
                return None;
            }
            let response = JsFuture::from(web_sys::window()?.fetch_with_str(&path))
                .await
                .ok()?;
            let response: web_sys::Response = response.dyn_into().ok()?;
            if !response.ok() {
                return None;
            }
            let bytes = JsFuture::from(response.array_buffer().ok()?).await.ok()?;
            Some(js_sys::Uint8Array::new(&bytes).to_vec())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_doc::{MessagePart, MessageStatus};

    #[gpui::test]
    fn desktop_transcript_visual_rows_load_the_shared_view(cx: &mut gpui::TestAppContext) {
        use crate::desktop_transcript::{Transcript, WebHost, WebTranscript};
        use openagents_doc::{MessageRole, SessionMessageEntry};

        cx.update(crate::install);
        let state = cx.update(|cx| {
            cx.new(|_| {
                let mut state = WebTranscript::default();
                state.open(
                    Some("recorded".into()),
                    vec![SessionMessageEntry {
                        id: "reply".into(),
                        role: MessageRole::Assistant,
                        parts: vec![MessagePart::Visual {
                            id: "chart".into(),
                            title: "Chart".into(),
                            path: "chart.luau".into(),
                        }],
                        created_at: 1,
                        device_id: "host".into(),
                        status: Some(MessageStatus::Complete),
                        continuation_of: None,
                        duration_ms: None,
                    }],
                );
                state
            })
        });
        let (view, cx) = cx.add_window_view(|_, cx| {
            let mut view = Transcript::new(WebHost::shared(&state), cx);
            view.media.visual_reader = Some(Rc::new(|_| {
                Box::pin(async {
                    Some(
                        br#"return ui.chart { x = { "A" }, series = { { values = { 2 } } } }"#
                            .to_vec(),
                    )
                })
            }));
            view
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear();
        });
        cx.run_until_parked();
        let first = cx.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear();
            let frames = view.read(cx).media.visuals.0.borrow();
            assert_eq!(
                frames.len(),
                1,
                "the desktop transcript visits its visual row"
            );
            let (_, frame) = frames.values().next().unwrap();
            assert!(frame.read(cx).visual.is_some());
            frame.entity_id()
        });
        cx.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear();
            let frames = view.read(cx).media.visuals.0.borrow();
            assert_eq!(frames.values().next().unwrap().1.entity_id(), first);
        });
    }
}
