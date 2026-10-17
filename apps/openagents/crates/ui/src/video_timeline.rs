//! A native timeline over Video Lab's shared edit rules and Labs SDK.

use std::{cell::Cell, rc::Rc};

use gpui::{
    AnyElement, App, AppContext as _, Bounds, Context, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    ParentElement, Pixels, Render, SharedString, StatefulInteractiveElement, Styled,
    StyledImage as _, Subscription, Window, WindowBounds, WindowOptions, canvas, div, img, px,
    size,
};
use gpui_base::input::{Input, InputEditorStyle, InputEvent, InputState};
use gpui_tokio::Tokio;
use lab_kit::Card;
use openagents_account::video_editor::{Clip, Gesture, Preview, Source, Timeline};

use crate::{
    lab_view::{LabLayoutChanged, LabView},
    theme::Theme,
};

const SCALE: f32 = 64.;

pub fn open(source: Source, cx: &mut App) {
    if !source.current() {
        return;
    }
    let source = source.fork();
    let bounds = Bounds::centered(None, size(px(1080.), px(850.)), cx);
    let _ = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        },
        move |window, cx| cx.new(|cx| VideoTimeline::new(source, window, cx)),
    );
}

struct Drag {
    id: String,
    start_x: Pixels,
    original: f64,
    at: f64,
    moved: bool,
    preview: Option<Preview>,
}

pub struct VideoTimeline {
    source: Source,
    timeline: Option<Timeline>,
    preview: Option<Entity<LabView>>,
    preview_version: u64,
    title: String,
    loading: bool,
    rendering: bool,
    render_version: u64,
    error: Option<String>,
    focus: FocusHandle,
    text: Option<Entity<InputState>>,
    _input: Option<Subscription>,
    drag: Option<Drag>,
    ruler: Rc<Cell<Option<Bounds<Pixels>>>>,
}

impl VideoTimeline {
    fn new(source: Source, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let text = cx.new(|cx| InputState::new(window, cx));
        let input = cx.subscribe_in(&text, window, |this, _, event, _, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.save_text(cx);
            }
        });
        cx.on_release(|this, _| this.source.close()).detach();
        let mut this = Self {
            source,
            timeline: None,
            preview: None,
            preview_version: 0,
            title: "Video timeline".into(),
            loading: false,
            rendering: false,
            render_version: 0,
            error: None,
            focus: cx.focus_handle(),
            text: Some(text),
            _input: Some(input),
            drag: None,
            ruler: Rc::new(Cell::new(None)),
        };
        this.focus.focus(window, cx);
        let source = this.source.clone();
        let closed = Tokio::spawn(cx, async move { source.closed().await });
        cx.spawn(async move |this, cx| {
            let _ = closed.await;
            let _ = this.update(cx, |this, cx| {
                this.clear();
                cx.notify();
            });
        })
        .detach();
        this.load(cx);
        this
    }

    fn clear(&mut self) {
        self.source.close();
        self.timeline = None;
        self.preview = None;
        self.drag = None;
        self.error = None;
        self.title = "Video timeline".into();
        self.preview_version = 0;
        self.text = None;
        self._input = None;
    }

    fn editable(&self) -> bool {
        self.source.current()
            && !self.loading
            && self.timeline.as_ref().is_some_and(Timeline::editable)
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        if self.loading || self.timeline.as_ref().is_some_and(Timeline::saving) {
            return;
        }
        self.loading = true;
        self.preview = None;
        self.error = None;
        self.drag = None;
        let source = self.source.clone();
        let job = Tokio::spawn(cx, async move { source.load().await });
        cx.spawn(async move |this, cx| {
            let result = job
                .await
                .unwrap_or_else(|_| Err("We couldn't open this film. Try again.".into()));
            let _ = this.update(cx, |this, cx| {
                if !this.source.current() {
                    this.clear();
                    cx.notify();
                    return;
                }
                this.loading = false;
                match result {
                    Ok((timeline, card)) => {
                        this.title = card.title.clone();
                        this.timeline = Some(timeline);
                        this.set_card(card, cx);
                    }
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn set_card(&mut self, card: Card, cx: &mut Context<Self>) {
        if !self.source.current()
            || self
                .timeline
                .as_ref()
                .is_none_or(|t| t.version().number != card.version)
        {
            return;
        }
        if self
            .preview
            .as_ref()
            .is_some_and(|view| view.read(cx).card() == &card)
        {
            return;
        }
        self.preview_version = card.version;
        let source = self.source.clone();
        let media_card = card.clone();
        let fetch: crate::lab_view::Fetch = Rc::new(move |id, cx| {
            let source = source.clone();
            let card = media_card.clone();
            let job = Tokio::spawn(cx, async move { source.media(&card, &id).await });
            cx.spawn(async move |_| job.await.ok().flatten())
        });
        let source = self.source.clone();
        let current: crate::lab_view::Current = Rc::new(move || source.current());
        let space = self.source.space().to_owned();
        let page = self.source.page(&card);
        let view = cx.new(|cx| {
            LabView::new(
                "timeline-preview".into(),
                card,
                &space,
                page,
                fetch,
                current,
                cx,
            )
        });
        cx.subscribe(&view, |_, _, _: &LabLayoutChanged, cx| cx.notify())
            .detach();
        self.preview = Some(view);
    }

    fn refresh_card(&mut self, cx: &mut Context<Self>) {
        let Some(version) = self.timeline.as_ref().map(|t| t.version().number) else {
            return;
        };
        let source = self.source.clone();
        let job = Tokio::spawn(cx, async move { source.card(version).await });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(card)) = job.await {
                let _ = this.update(cx, |this, cx| {
                    this.set_card(card, cx);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn edit(&mut self, gesture: Gesture, cx: &mut Context<Self>) {
        if !self.editable() {
            return;
        }
        let Some(timeline) = &mut self.timeline else {
            return;
        };
        let request = match timeline.begin(self.source.space(), gesture) {
            Ok(request) => request,
            Err(error) => {
                self.error = Some(error);
                cx.notify();
                return;
            }
        };
        self.error = None;
        let source = self.source.clone();
        let job = Tokio::spawn(cx, async move { source.save(request).await });
        cx.spawn(async move |this, cx| {
            let result = job
                .await
                .unwrap_or_else(|_| Err("We couldn't confirm the save.".into()));
            let _ = this.update(cx, |this, cx| {
                if !this.source.current() {
                    this.clear();
                    cx.notify();
                    return;
                }
                let Some(timeline) = &mut this.timeline else {
                    return;
                };
                let before = timeline.version().number;
                match result {
                    Ok(result) => {
                        if let Err(error) = timeline.finish(result) {
                            this.error = Some(error);
                        } else {
                            this.error = timeline.notice.clone();
                        }
                    }
                    Err(_) => {
                        timeline.unknown();
                        this.error = timeline.notice.clone();
                    }
                }
                if timeline.version().number != before {
                    this.preview = None;
                    this.refresh_card(cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn render_preview(&mut self, cx: &mut Context<Self>) {
        if !self.editable() || self.rendering {
            return;
        }
        let Some(version) = self.timeline.as_ref().map(|t| t.version().clone()) else {
            return;
        };
        self.rendering = true;
        self.render_version = version.number;
        self.error = None;
        let source = self.source.clone();
        let (cards, mut updates) = tokio::sync::mpsc::channel(4);
        let job = Tokio::spawn(cx, async move { source.render(&version, cards).await });
        cx.spawn(async move |this, cx| {
            while let Some(card) = updates.recv().await {
                if this
                    .update(cx, |this, cx| {
                        this.set_card(card, cx);
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
            let result = job.await.unwrap_or_else(|_| {
                Err("We couldn't follow the preview. Reload the film to check it.".into())
            });
            let _ = this.update(cx, |this, cx| {
                if !this.source.current() {
                    this.clear();
                    cx.notify();
                    return;
                }
                this.rendering = false;
                this.error = result.err();
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn select(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(timeline) = &mut self.timeline {
            timeline.select(id);
            if let Some(clip) = timeline.clips().into_iter().find(|c| c.id == id) {
                let value = if clip.track == "text" {
                    clip.label
                } else {
                    String::new()
                };
                if let Some(text) = &self.text {
                    text.update(cx, |text, cx| text.set_value(value, window, cx));
                }
            }
        }
        cx.notify();
    }

    fn save_text(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self
            .timeline
            .as_ref()
            .and_then(|t| t.selected.clone())
            .filter(|id| id.starts_with("text:"))
        else {
            return;
        };
        let Some(text) = &self.text else {
            return;
        };
        let value = text.read(cx).value().to_string();
        self.edit(Gesture::Text { id, value }, cx);
    }

    fn begin_drag(
        &mut self,
        clip: Clip,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus.focus(window, cx);
        self.select(&clip.id, window, cx);
        if let Some(timeline) = &mut self.timeline {
            timeline.seek(clip.at);
        }
        if self.editable() {
            self.drag = Some(Drag {
                id: clip.id,
                start_x: event.position.x,
                original: clip.at,
                at: clip.at,
                moved: false,
                preview: None,
            });
        }
        cx.stop_propagation();
    }

    fn drag(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let (Some(drag), Some(timeline)) = (&mut self.drag, &self.timeline) else {
            return;
        };
        let distance = event.position.x - drag.start_x;
        drag.moved |= distance.abs() >= px(3.);
        drag.at = (drag.original + f64::from(distance / px(SCALE))).max(0.);
        match timeline.preview(&Gesture::Move {
            id: drag.id.clone(),
            at: drag.at,
        }) {
            Ok(preview) => {
                drag.preview = Some(preview);
                self.error = None;
            }
            Err(error) => {
                drag.preview = None;
                self.error = Some(error);
            }
        }
        cx.notify();
    }

    fn finish_drag(&mut self, cx: &mut Context<Self>) {
        if let Some(drag) = self.drag.take() {
            if drag.moved && drag.preview.is_some() {
                self.edit(
                    Gesture::Move {
                        id: drag.id,
                        at: drag.at,
                    },
                    cx,
                );
            }
            cx.notify();
        }
    }

    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let direction = match event.keystroke.key.as_str() {
            "left" => -1.,
            "right" => 1.,
            _ => return,
        };
        let second = event.keystroke.modifiers.shift;
        let gesture = self
            .timeline
            .as_ref()
            .and_then(|timeline| timeline.nudge(direction, second));
        if let Some(gesture) = gesture {
            self.edit(gesture, cx);
        } else if let Some(timeline) = &mut self.timeline {
            timeline.step(direction, second);
        }
        window.prevent_default();
        cx.stop_propagation();
        cx.notify();
    }

    fn control(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        enabled: bool,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::of(cx).clone();
        let button = div()
            .id(id.into())
            .px(px(9.))
            .py(px(5.))
            .rounded(px(4.))
            .border_1()
            .border_color(theme.border)
            .text_size(px(12.))
            .child(label.into());
        if enabled {
            button
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, window, cx| action(this, window, cx)))
                .into_any_element()
        } else {
            button.opacity(0.4).into_any_element()
        }
    }

    fn tracks(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(timeline) = &self.timeline else {
            return div().into_any_element();
        };
        let theme = Theme::of(cx).clone();
        let preview = self.drag.as_ref().and_then(|drag| drag.preview.as_ref());
        let duration = preview.map_or(timeline.duration(), |p| p.duration);
        let clips = preview.map_or_else(|| timeline.clips(), |p| p.clips.clone());
        let width = (duration as f32 * SCALE).max(640.);
        let captured_bounds = self.ruler.clone();
        let mut ruler = div()
            .id("video-ruler")
            .relative()
            .w(px(width))
            .h(px(28.))
            .border_b_1()
            .border_color(theme.border)
            .cursor_pointer()
            .child(
                canvas(
                    move |bounds, _, _| captured_bounds.set(Some(bounds)),
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            );
        // The bounds come from the full scrollable ruler, including its offset.
        let ruler_bounds = self.ruler.clone();
        ruler = ruler.on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                if let (Some(bounds), Some(timeline)) = (ruler_bounds.get(), &mut this.timeline) {
                    timeline.seek(f64::from((event.position.x - bounds.origin.x) / px(SCALE)));
                    this.focus.focus(window, cx);
                    cx.notify();
                }
            }),
        );
        let step = (duration / 20.).ceil().max(1.) as usize;
        for second in (0..=(duration.ceil() as usize)).step_by(step) {
            ruler = ruler.child(
                div()
                    .absolute()
                    .left(px(second as f32 * SCALE))
                    .text_size(px(10.))
                    .child(format!("{second}s")),
            );
        }
        let mut rows = div().flex().flex_col().w(px(width)).child(ruler);
        let mut labels = div().w(px(56.)).pt(px(28.));
        for (track, label) in [("video", "Video"), ("audio", "Audio"), ("text", "Text")] {
            let mut track_clips: Vec<_> = clips.iter().filter(|clip| clip.track == track).collect();
            track_clips.sort_by(|a, b| a.at.total_cmp(&b.at));
            let mut ends = Vec::<f64>::new();
            let mut positions = Vec::new();
            for clip in track_clips {
                let level = ends
                    .iter()
                    .position(|end| *end <= clip.at + 0.001)
                    .unwrap_or(ends.len());
                if level == ends.len() {
                    ends.push(0.);
                }
                ends[level] = clip.at + clip.duration;
                positions.push((clip, level));
            }
            let height = (ends.len().max(1) * 60 + 8) as f32;
            labels = labels.child(
                div()
                    .h(px(height))
                    .pt(px(20.))
                    .text_size(px(12.))
                    .child(label),
            );
            let mut row = div()
                .id(SharedString::from(format!("track-{track}")))
                .relative()
                .w(px(width))
                .h(px(height))
                .border_b_1()
                .border_color(theme.border)
                .bg(theme.border.opacity(0.08));
            for (clip, level) in positions {
                let picked = timeline.selected.as_deref() == Some(clip.id.as_str());
                let dragging = self.drag.as_ref().is_some_and(|drag| drag.id == clip.id);
                let value = clip.clone();
                let label = if clip.link.is_some() || clip.embedded {
                    format!("{} · Linked", clip.label)
                } else {
                    clip.label.clone()
                };
                row = row.child(
                    div()
                        .id(SharedString::from(format!("timeline-{}", clip.id)))
                        .absolute()
                        .left(px(clip.at as f32 * SCALE))
                        .top(px(6. + level as f32 * 60.))
                        .w(px((clip.duration as f32 * SCALE).max(5.)))
                        .h(px(54.))
                        .px(px(6.))
                        .py(px(7.))
                        .rounded(px(4.))
                        .border_1()
                        .border_color(if picked { theme.accent } else { theme.border })
                        .bg(theme.accent.opacity(if dragging { 0.35 } else { 0.12 }))
                        .overflow_hidden()
                        .cursor_pointer()
                        .text_size(px(12.))
                        .child(label)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, event, window, cx| {
                                this.begin_drag(value.clone(), event, window, cx)
                            }),
                        ),
                );
            }
            row = row.child(
                div()
                    .absolute()
                    .left(px(timeline.playhead as f32 * SCALE))
                    .top_0()
                    .w(px(1.))
                    .h_full()
                    .bg(theme.accent),
            );
            rows = rows.child(row);
        }
        div()
            .flex()
            .gap(px(8.))
            .child(labels)
            .child(
                div()
                    .id("timeline-scroll")
                    .flex_1()
                    .min_w_0()
                    .overflow_x_scroll()
                    .child(rows),
            )
            .into_any_element()
    }

    fn inspector(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(timeline) = &self.timeline else {
            return div().into_any_element();
        };
        let clips = timeline.clips();
        let Some(clip) = clips
            .iter()
            .find(|clip| Some(&clip.id) == timeline.selected.as_ref())
        else {
            return div()
                .py(px(10.))
                .child("Select a clip to edit it.")
                .into_any_element();
        };
        let enabled = self.editable();
        let mut panel = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .py(px(10.))
            .child(clip.label.clone());
        if clip.track == "video" {
            let mut speeds = div()
                .flex()
                .items_center()
                .gap(px(5.))
                .child(format!("Speed · {}×", clip.speed));
            for speed in [0.25, 0.5, 1., 1.5, 2., 4.] {
                let id = clip.id.clone();
                speeds = speeds.child(Self::control(
                    format!("speed-{speed}"),
                    format!("{speed}×"),
                    enabled,
                    move |this, _, cx| {
                        this.edit(
                            Gesture::Speed {
                                id: id.clone(),
                                value: speed,
                            },
                            cx,
                        )
                    },
                    cx,
                ));
            }
            panel = panel.child(speeds);
            if clips
                .iter()
                .find(|c| c.track == "video")
                .is_some_and(|first| first.id != clip.id)
            {
                let mut transition = div().flex().gap(px(5.)).child(if clip.dissolve {
                    "Transition · Dissolve"
                } else {
                    "Transition · Cut"
                });
                for (value, label) in [(false, "Cut"), (true, "Dissolve")] {
                    let id = clip.id.clone();
                    transition = transition.child(Self::control(
                        format!("transition-{value}"),
                        label,
                        enabled,
                        move |this, _, cx| {
                            this.edit(
                                Gesture::Dissolve {
                                    id: id.clone(),
                                    enabled: value,
                                },
                                cx,
                            )
                        },
                        cx,
                    ));
                }
                panel = panel.child(transition);
            }
        } else if !clip.embedded {
            let id = clip.id.clone();
            let mut links = div()
                .id("link-choices")
                .flex()
                .items_center()
                .gap(px(5.))
                .overflow_x_scroll()
                .child("Link to");
            links = links.child(Self::control(
                "unlink-clip",
                "None",
                enabled,
                move |this, _, cx| {
                    this.edit(
                        Gesture::Link {
                            id: id.clone(),
                            shot: None,
                        },
                        cx,
                    )
                },
                cx,
            ));
            for video in clips.iter().filter(|c| c.track == "video") {
                let id = clip.id.clone();
                let shot = video.id.trim_start_matches("video:").to_owned();
                links = links.child(Self::control(
                    format!("link-{shot}"),
                    video.label.clone(),
                    enabled,
                    move |this, _, cx| {
                        this.edit(
                            Gesture::Link {
                                id: id.clone(),
                                shot: Some(shot.clone()),
                            },
                            cx,
                        )
                    },
                    cx,
                ));
            }
            panel = panel.child(links);
        }
        if clip.track == "text" {
            let mut caption = div().flex().items_center().gap(px(8.));
            caption = if let Some(text) = self.text.as_ref().filter(|_| enabled) {
                caption.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h(px(32.))
                        .p(px(4.))
                        .border_1()
                        .border_color(Theme::of(cx).border)
                        .child(Input::new(text)),
                )
            } else {
                caption.child(clip.label.clone())
            };
            panel = panel.child(caption.child(Self::control(
                "save-caption",
                "Save caption",
                enabled,
                |this, _, cx| this.save_text(cx),
                cx,
            )));
            let mut length = div().flex().gap(px(5.)).child("Length");
            for direction in [-1., 1.] {
                let id = clip.id.clone();
                let duration =
                    (clip.duration + direction / timeline.fps()).max(1. / timeline.fps());
                length = length.child(Self::control(
                    format!("caption-length-{direction}"),
                    if direction < 0. {
                        "−1 frame"
                    } else {
                        "+1 frame"
                    },
                    enabled,
                    move |this, _, cx| {
                        this.edit(
                            Gesture::Duration {
                                id: id.clone(),
                                value: duration,
                            },
                            cx,
                        )
                    },
                    cx,
                ));
            }
            panel = panel.child(length);
        }
        panel.into_any_element()
    }
}

impl Focusable for VideoTimeline {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for VideoTimeline {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        if let Some(text) = &self.text {
            text.update(cx, |text, _| {
                text.set_editor_style(InputEditorStyle {
                    foreground: theme.text,
                    muted_foreground: theme.text_faint,
                    background: gpui::transparent_black(),
                    border: theme.border,
                    selection: theme.accent.opacity(0.22),
                    caret: theme.caret,
                    ..Default::default()
                })
            });
        }
        if !self.source.current() {
            self.clear();
            return div()
                .size_full()
                .p(px(24.))
                .bg(theme.bg)
                .text_color(theme.text)
                .child("Open this film again from your chats.")
                .into_any_element();
        }
        let mut root = div()
            .id("video-timeline-editor")
            .size_full()
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .font_family(theme.font_sans.clone())
            .text_color(theme.text)
            .bg(theme.bg)
            .track_focus(&self.focus)
            .overflow_y_scroll()
            .key_context("VideoTimeline")
            .on_key_down(cx.listener(Self::key))
            .on_mouse_move(cx.listener(|this, event, _, cx| this.drag(event, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.finish_drag(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.finish_drag(cx)),
            );
        let state = if self.loading {
            "Opening film…".into()
        } else if self.timeline.as_ref().is_some_and(Timeline::saving) {
            "Saving…".into()
        } else if self.rendering {
            format!("Rendering version {}", self.render_version)
        } else {
            self.timeline.as_ref().map_or_else(String::new, |t| {
                format!("Saved version {}", t.version().number)
            })
        };
        let mut header = div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(self.title.clone())
            .child(state);
        header = header.child(Self::control(
            "reload-film",
            "Reload",
            !self.loading && !self.timeline.as_ref().is_some_and(Timeline::saving),
            |this, _, cx| this.load(cx),
            cx,
        ));
        header = header.child(Self::control(
            "render-preview",
            "Render preview",
            self.editable() && !self.rendering,
            |this, _, cx| this.render_preview(cx),
            cx,
        ));
        root = root.child(header);
        if let Some(error) = &self.error {
            root = root.child(div().child(error.clone()));
        }
        if let Some(timeline) = &self.timeline {
            let position = format!(
                "{:.3}s · frame {}",
                timeline.playhead,
                (timeline.playhead * timeline.fps()).round() as u64
            );
            let frame = self.preview.as_ref().and_then(|view| {
                view.read(cx).frame_at(if timeline.duration() > 0. {
                    timeline.playhead / timeline.duration()
                } else {
                    0.
                })
            });
            let mut pictures = div()
                .id("video-preview-area")
                .flex()
                .gap(px(12.))
                .max_h(px(320.))
                .overflow_y_scroll();
            if let Some(view) = &self.preview {
                pictures = pictures.child(div().flex_1().min_w_0().child(view.clone()));
            }
            if let Some(frame) = frame {
                pictures = pictures.child(
                    div()
                        .w(px(220.))
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(
                            img(frame)
                                .w_full()
                                .max_h(px(200.))
                                .object_fit(gpui::ObjectFit::Contain),
                        )
                        .child(position.clone())
                        .child(format!("Preview version {}", self.preview_version)),
                );
            }
            root = root
                .child(pictures)
                .child(position)
                .child(self.tracks(cx))
                .child(self.inspector(cx));
        }
        let _ = window;
        root.into_any_element()
    }
}
