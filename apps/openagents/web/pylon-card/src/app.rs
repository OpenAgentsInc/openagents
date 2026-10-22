//! The card: the pylon's name, status, and key at the top (the key copies
//! on a click), its slots as boxes, then one row per detail its events
//! give. It redraws only when the page hands it something new.

use std::time::Duration;

use gpui::{
    ClipboardItem, Context, FontWeight, IntoElement, ParentElement, Render, Styled, Window, div,
    prelude::*, px,
};
use openagents_web_chat_view::theme;
use pylons_view::{
    Shown, rows,
    work::{WorkPhase, WorkSnapshot, WorkStep},
};
use web_time::Instant;

/// How often the card looks for a new handover from the page.
const POLL: Duration = Duration::from_millis(100);
/// How long "Copied" shows after the key is copied.
const COPIED: Duration = Duration::from_millis(1600);

pub struct PylonCard {
    version: u64,
    shown: Option<Shown>,
    copied: Option<Instant>,
}

impl PylonCard {
    pub fn new(_still: bool, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (version, shown) = crate::latest();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                let alive = this.update(cx, |this, cx| {
                    let (version, shown) = crate::latest();
                    let copied_over = this.copied.is_some_and(|at| at.elapsed() >= COPIED);
                    if copied_over {
                        this.copied = None;
                    }
                    if version != this.version {
                        this.version = version;
                        this.shown = shown;
                        cx.notify();
                    } else if copied_over {
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
            version,
            shown,
            copied: None,
        }
    }

    fn header(&self, shown: &Shown, cx: &mut Context<Self>) -> impl IntoElement {
        let pylon = &shown.pylon;
        let (dot, status) = if shown.gone {
            (theme::text_faint(), "Offline")
        } else if pylon.status == "draining" {
            (theme::text_muted(), pylon.status_words())
        } else {
            (theme::text(), pylon.status_words())
        };
        let npub = pylon.npub.clone();
        let copied = self.copied.is_some();
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .px(px(20.))
            .pt(px(18.))
            .pb(px(16.))
            .border_b_1()
            .border_color(theme::border())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(7.))
                    .text_size(px(12.5))
                    .text_color(theme::text_muted())
                    .child(div().size(px(7.)).rounded(px(4.)).bg(dot))
                    .child(status),
            )
            .child(
                div()
                    // Room for the page's close button.
                    .mr(px(36.))
                    .text_size(px(19.))
                    .line_height(px(25.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::text())
                    .child(pylon.name.clone()),
            )
            .child(
                div()
                    .id("pylon-key")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(npub.clone()));
                        this.copied = Some(Instant::now());
                        cx.notify();
                    }))
                    .child(
                        div()
                            .font_family(theme::MONO)
                            .text_size(px(12.5))
                            .text_color(theme::text_muted())
                            .child(pylon.short_npub()),
                    )
                    .child(
                        div()
                            .px(px(7.))
                            .h(px(20.))
                            .flex()
                            .items_center()
                            .rounded(px(10.))
                            .border_1()
                            .border_color(theme::border_strong())
                            .text_size(px(11.5))
                            .text_color(if copied {
                                theme::text()
                            } else {
                                theme::text_muted()
                            })
                            .child(if copied { "Copied" } else { "Copy key" }),
                    ),
            )
            .when(!shown.gone && pylon.total > 0, |header| {
                header.child(slots(pylon.busy, pylon.total))
            })
    }
}

/// One box a slot, filled when busy.
fn slots(busy: u32, total: u32) -> impl IntoElement {
    let mut row = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(px(4.))
        .mt(px(4.));
    for slot in 0..total.min(64) {
        let used = slot < busy;
        row = row.child(
            div()
                .size(px(10.))
                .rounded(px(2.))
                .border_1()
                .border_color(theme::text_muted())
                .when(used, |cell| cell.bg(theme::text())),
        );
    }
    row.child(
        div()
            .ml(px(6.))
            .text_size(px(12.))
            .text_color(theme::text_faint())
            .child(format!("{busy} of {total} busy")),
    )
}

impl Render for PylonCard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // On the Grid homepage the card is the only thing drawn over the
        // world: it draws its own panel and close mark, under the page's
        // transparent close button (#12459). Elsewhere it is a faint tint
        // over the page's glass (WebGPU draws it on a transparent canvas).
        let framed = on_grid();
        let mut root = div()
            .id("pylon-card")
            .size_full()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .when(framed, |root| {
                root.rounded(px(16.))
                    .border_1()
                    .border_color(theme::border_strong())
                    .bg(gpui::hsla(0., 0., 0.03, 0.9))
            })
            .when(!framed, |root| root.bg(gpui::hsla(0., 0., 0.04, 0.4)))
            .font_family(theme::SANS)
            .text_color(theme::text())
            .text_size(px(14.));
        root.text_style().font_fallbacks = Some(theme::fallbacks());
        let Some(shown) = self.shown.clone() else {
            return root;
        };
        let header = self.header(&shown, cx);
        if framed {
            root = root.child(
                div()
                    .absolute()
                    .top(px(16.))
                    .right(px(16.))
                    .size(px(20.))
                    .child(
                        openagents_web_chat_view::svg()
                            .path("icons/close.svg")
                            .size(px(20.))
                            .text_color(theme::text_muted()),
                    ),
            );
        }
        let mut list = div().flex().flex_col().px(px(20.)).py(px(10.));
        for row in rows(&shown) {
            list = list.child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .gap(px(16.))
                    .py(px(8.))
                    .border_b_1()
                    .border_color(theme::border())
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(13.))
                            .text_color(theme::text_muted())
                            .child(row.label),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .text_size(px(13.5))
                            .text_color(theme::text())
                            .child(row.value),
                    ),
            );
        }
        root.child(header).child(live_work(&shown, cx)).child(list)
    }
}

fn note(text: impl Into<gpui::SharedString>) -> gpui::Div {
    div()
        .text_size(px(12.5))
        .text_color(theme::text_muted())
        .child(text.into())
}

pub(crate) fn phase_words(phase: &WorkPhase) -> &'static str {
    match phase {
        WorkPhase::Queued => "Waiting to start",
        WorkPhase::Working => "Working",
        WorkPhase::Checking => "Running checks",
        WorkPhase::Done => "Finished",
        WorkPhase::Failed => "Needs attention",
    }
}

fn work_step(step: &WorkStep) -> gpui::Div {
    let mut row = div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .py(px(9.))
        .border_b_1()
        .border_color(theme::border());
    row = row.child(div().text_size(px(13.)).child(step.label.clone()));
    if let Some(path) = &step.path {
        row = row.child(
            div()
                .font_family(theme::MONO)
                .text_size(px(11.5))
                .text_color(theme::text_muted())
                .child(path.clone()),
        );
    }
    if !step.summary.is_empty() {
        row = row.child(note(step.summary.clone()));
    }
    row
}

fn live_work(shown: &Shown, cx: &mut Context<PylonCard>) -> gpui::Div {
    let mut panel = div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .px(px(20.))
        .py(px(16.))
        .border_b_1()
        .border_color(theme::border());
    let view = shown.work.as_ref();
    let Some(work) = view.and_then(|view| view.snapshot.as_ref()) else {
        let message = match shown.work_status.as_str() {
            "loading" => "Loading live work…",
            "signin" => "Sign in to see live work.",
            "forbidden" => "Live work is available to OpenAgents staff.",
            "unavailable" => "Live work is unavailable. We'll try again.",
            "empty" => "No live work is being shared from this Pylon.",
            _ => return div(),
        };
        panel = panel.child(note(message));
        if shown.work_status == "signin" {
            panel = panel.child(
                div()
                    .id("pylon-work-signin")
                    .cursor_pointer()
                    .text_size(px(13.))
                    .child("Sign in")
                    .on_click(cx.listener(|_, _, _, _| {
                        if let Some(window) = web_sys::window() {
                            let _ = window.location().set_href("/login?return_to=%2F");
                        }
                    })),
            );
        }
        return panel;
    };
    // A stopped publisher must look stopped even between HTTP updates.
    let age = shown.now.saturating_sub(work.observed_at);
    let stale = view.is_some_and(|view| view.stale) || age > 120;
    let state = if stale {
        "Updates paused"
    } else {
        phase_words(&work.phase)
    };
    panel = panel
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(10.))
                .child(
                    div()
                        .text_size(px(14.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Live work"),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(theme::text_muted())
                        .child(state),
                ),
        )
        .child(
            div()
                .text_size(px(16.))
                .line_height(px(22.))
                .child(work.title.clone()),
        );
    if let Some(issue) = work.issue {
        panel = panel.child(
            div()
                .id("pylon-work-issue")
                .cursor_pointer()
                .text_size(px(12.5))
                .text_color(theme::text_muted())
                .child(format!("Issue #{issue} ↗"))
                .on_click(cx.listener(move |_, _, _, _| {
                    if let Some(window) = web_sys::window() {
                        let _ = window.open_with_url_and_target(
                            &format!("https://github.com/OpenAgentsInc/openagents/issues/{issue}"),
                            "_blank",
                        );
                    }
                })),
        );
    }
    panel = panel
        .child(note(format!(
            "{} turns · {} tool calls · {} changed files",
            work.turns, work.calls, work.changed
        )))
        .child(note(format!("Updated {}", pylons_view::ago(age))));
    if let Some(step) = &work.current {
        panel = panel.child(work_step(step));
        panel = panel.child(note(format!(
            "Last activity {}",
            pylons_view::ago(shown.now.saturating_sub(step.at))
        )));
    }
    panel = work_details(panel, work);
    if !work.steps.is_empty() {
        panel = panel.child(
            div()
                .mt(px(4.))
                .text_size(px(12.))
                .text_color(theme::text_muted())
                .child("Recent activity"),
        );
        for step in work.steps.iter().rev().take(12) {
            panel = panel.child(work_step(step));
        }
    }
    panel
}

fn work_details(mut panel: gpui::Div, work: &WorkSnapshot) -> gpui::Div {
    for check in &work.checks {
        panel = panel.child(note(format!("{} · {}", check.label, check.status)));
        if !check.summary.is_empty() {
            panel = panel.child(note(check.summary.clone()));
        }
    }
    if !work.files.is_empty() {
        panel = panel.child(note("Changed files"));
        for path in &work.files {
            panel = panel.child(
                div()
                    .font_family(theme::MONO)
                    .text_size(px(11.5))
                    .child(path.clone()),
            );
        }
    }
    if let Some(answer) = &work.answer {
        let model = answer.model.split('@').next().unwrap_or(&answer.model);
        panel = panel.child(note(format!(
            "{model}{}",
            if answer.lossy { " · experimental" } else { "" }
        )));
    }
    panel
}

/// Whether the card is on the Grid homepage, `/`.
fn on_grid() -> bool {
    web_sys::window()
        .and_then(|w| w.location().pathname().ok())
        .is_some_and(|path| path == "/" || path.is_empty())
}
