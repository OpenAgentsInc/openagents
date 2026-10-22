//! The view: the desktop's transcript over Kiln's run, redrawn when Kiln
//! hands over a new one, with the working turn's timer while the agent
//! works.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AppContext as _, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div,
    prelude::*, px,
};
use openagents_chat_view::transcript::host::TurnActivity;
use openagents_doc::SessionMessageEntry;
use openagents_web_chat_view::desktop_transcript::{self, WebTranscript};
use openagents_web_chat_view::theme;

/// How often the view looks for a new run from the page.
const POLL: Duration = Duration::from_millis(100);
const MOUNT_ID: &str = "gpui-mount";

/// Tells the page something: an event on `#gpui-mount`.
#[cfg_attr(not(feature = "export"), allow(dead_code))]
pub(crate) fn signal(name: &str) {
    let mount = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(MOUNT_ID));
    if let (Some(mount), Ok(event)) = (mount, web_sys::Event::new(name)) {
        let _ = mount.dispatch_event(&event);
    }
}

pub struct KilnTranscript {
    version: u64,
    entries: Vec<SessionMessageEntry>,
    working: Option<TurnActivity>,
    shared: Entity<WebTranscript>,
    transcript: Entity<desktop_transcript::Transcript>,
    published: Option<(Vec<(usize, usize, u8)>, bool)>,
}

impl KilnTranscript {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let shared = cx.new(|_| WebTranscript::default());
        let transcript =
            desktop_transcript::new(&shared, Rc::new(RefCell::new(HashMap::new())), cx);
        transcript.update(cx, |view, cx| view.set_rail_enabled(false, cx));
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                let alive = this.update(cx, |this, cx| {
                    let (version, run) = crate::latest();
                    if version != this.version {
                        this.version = version;
                        this.entries = crate::entries(&run);
                        this.working = run.working.as_ref().map(|_| {
                            let started = run
                                .started_ms
                                .unwrap_or_else(|| openagents_web_chat_view::now_ms());
                            TurnActivity {
                                sending: false,
                                queued: false,
                                elapsed_secs: Some(
                                    (openagents_web_chat_view::now_ms() - started) / 1000,
                                ),
                                turn_ms: started,
                            }
                        });
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
        Self {
            version: 0,
            entries: Vec::new(),
            working: None,
            shared,
            transcript,
            published: None,
        }
    }
}

impl Render for KilnTranscript {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shape = (
            desktop_transcript::shape(&self.entries),
            self.working.is_some(),
        );
        if self.published.as_ref() != Some(&shape) {
            self.published = Some(shape);
            desktop_transcript::show(
                &self.shared,
                &self.transcript,
                Some("kiln".into()),
                self.entries.clone(),
                self.working,
                cx,
            );
        }
        desktop_transcript::fit(
            &self.transcript,
            f32::from(window.viewport_size().width),
            false,
            cx,
        );
        let mut root = div()
            .id("kiln-transcript")
            .size_full()
            .flex()
            .flex_col()
            .font_family(theme::SANS)
            .text_color(theme::text())
            .text_size(px(theme::BODY));
        root.text_style().font_fallbacks = Some(theme::fallbacks());
        root.child(
            div()
                .flex_1()
                .min_h_0()
                .w_full()
                .child(self.transcript.clone()),
        )
    }
}
