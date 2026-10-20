//! Full-window call stage. Media stays in the browser; Codex stays on the computer.
use gpui::{
    Bounds, Context, Pixels, Render, Task, Window, canvas, div, fill, point, prelude::*, px, size,
};
use openagents_proto::voice::{VoiceChatContext, VoiceEvent, VoicePhase};
use openagents_voice_session::VoiceView;
use openagents_web_chat_view::theme;
use openagents_web_client::{
    voice::{Call, Media},
    web::Client,
};
use std::{rc::Rc, time::Duration};
use web_time::Instant;

pub struct Stage {
    pub open: bool,
    view: VoiceView,
    media: Option<Rc<Media>>,
    call: Option<Rc<Call>>,
    started: Option<Instant>,
    finished_seconds: Option<u64>,
    animation_since: Instant,
    notice: Option<String>,
    task: Option<Task<()>>,
    _ticker: Task<()>,
}
impl Stage {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        if this.open {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            open: false,
            view: VoiceView::default(),
            media: None,
            call: None,
            started: None,
            finished_seconds: None,
            animation_since: Instant::now(),
            notice: None,
            task: None,
            _ticker: ticker,
        }
    }
    pub fn start(&mut self, context: Option<VoiceChatContext>, cx: &mut Context<Self>) {
        if self.open {
            return;
        }
        self.open = true;
        self.view = VoiceView::new();
        self.notice = Some("Connecting…".into());
        self.started = None;
        self.finished_seconds = None;
        self.animation_since = Instant::now();
        let media = match Media::new() {
            Ok(media) => media,
            Err(_) => {
                self.view.phase = VoicePhase::Failed;
                self.notice = Some(
                    "Couldn't open audio. Check your browser's microphone settings and try again."
                        .into(),
                );
                cx.notify();
                return;
            }
        };
        self.media = Some(media.clone());
        self.task = Some(cx.spawn(async move |this, cx| {
            let result = async {
                let client = Client::from_page();
                let choice = client.voice_agent().await?;
                let device = choice["target"]["host_id"].as_str().filter(|id| !id.is_empty()).ok_or_else(|| anyhow::anyhow!("Choose a voice agent in the OpenAgents app, then try again."))?;
                let call = Call::with_media(device.to_owned(), media.clone());
                this.update(cx, |this, cx| { this.call = Some(call.clone()); cx.notify(); })?;
                let weak = this.clone();
                let mut updates = cx.clone();
                let waiting = this.clone();
                let mut waiting_cx = cx.clone();
                call.run(&client, context, move |event| {
                    let _ = weak.update(&mut updates, |this, cx| {
                        if let VoiceEvent::Closed { reason: Some(_), .. } = &event {
                            this.notice = Some("The call disconnected. Try again.".into());
                        }
                        this.view.reduce(event, Instant::now());
                        if this.view.phase == VoicePhase::Active && this.started.is_none() { this.started = Some(Instant::now()); }
                        cx.notify();
                    });
                }, move || {
                    let _ = waiting.update(&mut waiting_cx, |this, cx| {
                        this.notice = Some("Allow this browser on your computer: open Settings → Devices in the OpenAgents app.".into());
                        cx.notify();
                    });
                }).await
            }.await;
            media.close();
            let _ = this.update(cx, |this, cx| {
                if let Some(call) = this.call.take() { call.stop(); }
                this.media = None;
                this.finished_seconds = Some(this.started.map_or(0, |at| at.elapsed().as_secs()));
                this.view.phase = if result.is_ok() { VoicePhase::Closed } else { VoicePhase::Failed };
                this.notice = Some(match result { Ok(()) => "Call ended".into(), Err(error) => message(&error.to_string()) });
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn end(&mut self, cx: &mut Context<Self>) {
        if let Some(media) = self.media.take() {
            media.close();
        }
        if let Some(call) = self.call.take() {
            call.stop();
        }
        self.task = None;
        self.view.phase = VoicePhase::Closed;
        self.open = false;
        #[cfg(target_family = "wasm")]
        crate::signal("oa-page-close", crate::NAME);
        cx.notify();
    }
}
impl Drop for Stage {
    fn drop(&mut self) {
        if let Some(media) = &self.media {
            media.close();
        }
        if let Some(call) = &self.call {
            call.stop();
        }
    }
}
fn message(error: &str) -> String {
    if error.contains("Busy") {
        "Your computer is already on a call. End it there and try again.".into()
    } else if error.contains("VoiceAgentRequired") {
        "Choose a voice agent in the OpenAgents app, then try again.".into()
    } else if error.contains("ChatgptRequired") {
        "Sign in to Codex on your computer, then try again.".into()
    } else if error.contains("voice unavailable")
        || error.contains("unknown method")
        || error.contains("can't do that")
    {
        "Update OpenAgents on your computer, then try again.".into()
    } else {
        error.to_owned()
    }
}
impl Render for Stage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.view.phase == VoicePhase::Active;
        let ended = matches!(self.view.phase, VoicePhase::Closed | VoicePhase::Failed);
        let elapsed = self
            .finished_seconds
            .unwrap_or_else(|| self.started.map_or(0, |at| at.elapsed().as_secs()));
        let status = if active {
            if self.view.muted() {
                "Muted"
            } else if self.view.playing() {
                "Speaking…"
            } else {
                "Listening…"
            }
        } else {
            self.notice.as_deref().unwrap_or("Connecting…")
        };
        let preset =
            openagents_orb::resolve_preset(self.view.orb_state(), openagents_orb::OrbSize::Hero);
        let t = self.animation_since.elapsed().as_secs_f32();
        let frame =
            openagents_orb::engine::draw_mode(preset.mode, 128., t * preset.speed, &preset.opts);
        let orb = canvas(
            move |_, _, _| frame,
            move |bounds: Bounds<Pixels>, frame, window, _| {
                let scale = 2.;
                for dot in frame.dots {
                    let r = dot.r * scale;
                    window.paint_quad(
                        fill(
                            Bounds::new(
                                point(
                                    bounds.origin.x + px(dot.x * scale - r),
                                    bounds.origin.y + px(dot.y * scale - r),
                                ),
                                size(px(r * 2.), px(r * 2.)),
                            ),
                            theme::text().opacity(dot.a * (1. - dot.white)),
                        )
                        .corner_radii(px(r)),
                    );
                }
            },
        )
        .size(px(256.));
        div()
            .id("voice-stage")
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .bg(theme::bg())
            .text_color(theme::text())
            .font_family(theme::SANS)
            .gap(px(24.))
            .p(px(24.))
            .child(div().text_size(px(20.)).child("Voice"))
            .child(orb)
            .child(div().text_size(px(14.)).child(status.to_owned()))
            .child(
                div()
                    .text_size(px(13.))
                    .child(format!("{}:{:02}", elapsed / 60, elapsed % 60)),
            )
            .child(
                div()
                    .id("voice-captions")
                    .max_w(px(680.))
                    .min_h(px(72.))
                    .text_size(px(18.))
                    .child(self.view.caption().to_owned()),
            )
            .child(
                div()
                    .flex()
                    .gap(px(16.))
                    .when(active, |row| {
                        row.child(
                            div()
                                .id("voice-mute")
                                .cursor_pointer()
                                .px(px(20.))
                                .py(px(12.))
                                .rounded_full()
                                .bg(theme::surface_card())
                                .child(if self.view.muted() { "Unmute" } else { "Mute" })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let muted = !this.view.muted();
                                    if let Some(media) = &this.media {
                                        media.set_muted(muted);
                                    }
                                    this.view.set_muted(muted);
                                    cx.notify();
                                })),
                        )
                    })
                    .child(
                        div()
                            .id("voice-end")
                            .cursor_pointer()
                            .px(px(20.))
                            .py(px(12.))
                            .rounded_full()
                            .bg(theme::surface_card())
                            .child(if ended { "Close" } else { "End call" })
                            .on_click(cx.listener(|this, _, _, cx| this.end(cx))),
                    ),
            )
    }
}
