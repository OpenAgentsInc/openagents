//! Kiln at the Grid terminal (#12456): Kiln's cards and its agent run,
//! drawn here in GPUI, with no page of Kiln's own over the world.
//!
//! The page loads Kiln's module when the panel opens (`homeKiln(true)`)
//! and starts it without markup (`start_headless`). Kiln keeps the
//! workspace, the agent, and the runner jobs; it publishes its state as
//! JSON through `kilnState()` and its run as `kilnTranscriptLatest`. This
//! view reads both, draws the cards from `kiln_view` with the shared Rust
//! Native adapter and the run with the desktop's transcript, and sends each
//! press back through `kilnAgent`, `kilnStop`, `kilnLoad`, and
//! `kilnPublish`.

use kiln_view::{Intent, Paints, State};
use openagents_chat_view::rust_native_gpui;
use openagents_web_kiln_transcript::KilnTranscript;
use rust_native::style::Color;

use super::*;

/// How often the panel reads Kiln's state, in ticks.
const READ_EVERY: u32 = 8;
/// The page global this view tells the world what Kiln is doing, for the
/// terminal's screen.
const SCREEN_GLOBAL: &str = "openagentsKilnScreen";
/// The widest the cards' column gets beside the run.
const CARDS_W: f32 = 340.;

pub(super) struct KilnUi {
    task: Entity<TextInput>,
    slice: Entity<TextInput>,
    transcript: Entity<KilnTranscript>,
    pub(super) state: State,
    state_json: String,
    run_json: String,
    ticks: u32,
    cards: ScrollHandle,
}

/// Calls a page function with string arguments; `false` when it isn't
/// there yet.
fn call_kiln(name: &str, args: &[&str]) -> bool {
    let Ok(f) = js_sys::Reflect::get(&js_sys::global(), &name.into()) else {
        return false;
    };
    let Some(f) = f.dyn_ref::<js_sys::Function>() else {
        return false;
    };
    let result = match args {
        [] => f.call0(&JsValue::UNDEFINED),
        [a] => f.call1(&JsValue::UNDEFINED, &JsValue::from_str(a)),
        _ => return false,
    };
    result.is_ok()
}

/// A page function's string answer.
fn read_kiln(name: &str) -> Option<String> {
    let f = js_sys::Reflect::get(&js_sys::global(), &name.into()).ok()?;
    let f = f.dyn_ref::<js_sys::Function>()?;
    f.call0(&JsValue::UNDEFINED).ok()?.as_string()
}

fn global_string(name: &str) -> Option<String> {
    js_sys::Reflect::get(&js_sys::global(), &name.into())
        .ok()?
        .as_string()
}

fn rn(color: gpui::Hsla) -> Color {
    let rgba = gpui::Rgba::from(color);
    let byte = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    Color::rgb(byte(rgba.r), byte(rgba.g), byte(rgba.b))
}

fn paints() -> Paints {
    Paints {
        text: rn(theme::text()),
        muted: rn(theme::text_muted()),
        card: Color::rgb(14, 14, 14),
        border: rn(theme::border()),
        good: rn(theme::success()),
        bad: rn(theme::danger()),
    }
}

impl LanderChat {
    /// Kiln's part of the tick while its panel is open: read its state and
    /// its run when they change.
    pub(super) fn kiln_tick(&mut self, cx: &mut Context<Self>) {
        let Some(ui) = self.grid.kiln_ui.as_mut() else {
            return;
        };
        ui.ticks = ui.ticks.wrapping_add(1);
        if let Some(run) = global_string("kilnTranscriptLatest")
            && run != ui.run_json
        {
            openagents_web_kiln_transcript::take(&run);
            ui.run_json = run;
        }
        if ui.ticks % READ_EVERY != 1 {
            return;
        }
        let Some(json) = read_kiln("kilnState") else {
            return;
        };
        if json == ui.state_json {
            return;
        }
        if let Ok(state) = serde_json::from_str::<State>(&json) {
            // The terminal's screen in the world says what Kiln is doing.
            // The crate it works on, or the task's first words.
            let screen = state
                .busy
                .then(|| match &state.repo {
                    Some(repo) => repo
                        .slice
                        .split(',')
                        .next()
                        .unwrap_or_default()
                        .rsplit('/')
                        .next()
                        .unwrap_or_default()
                        .to_owned(),
                    None => ui
                        .task
                        .read(cx)
                        .text()
                        .split_whitespace()
                        .take(4)
                        .collect::<Vec<_>>()
                        .join(" "),
                })
                .filter(|t| !t.is_empty());
            let _ = js_sys::Reflect::set(
                &js_sys::global(),
                &SCREEN_GLOBAL.into(),
                &screen.map_or(JsValue::NULL, |s| JsValue::from_str(&s)),
            );
            ui.state = state;
            ui.state_json = json;
            cx.notify();
        }
    }

    fn kiln_ui(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.grid.kiln_ui.is_some() {
            return;
        }
        let task = cx.new(|cx| {
            let mut input = TextInput::multiline(kiln_view::TASK_PLACEHOLDER, cx);
            input.reset(kiln_view::DEFAULT_TASK, 0, cx);
            input
        });
        let slice = cx.new(|cx| {
            let mut input = TextInput::new(kiln_view::SLICE_LABEL, cx);
            input.reset(kiln_view::DEFAULT_SLICE, 0, cx);
            input
        });
        cx.subscribe(&task, |this, _, _: &Submitted, cx| {
            this.kiln_intent(Intent::Run, cx)
        })
        .detach();
        cx.subscribe(&slice, |this, _, _: &Submitted, cx| {
            this.kiln_intent(Intent::Load, cx)
        })
        .detach();
        let transcript = cx.new(|cx| KilnTranscript::new(window, cx));
        self.grid.kiln_ui = Some(KilnUi {
            task,
            slice,
            transcript,
            state: State::default(),
            state_json: String::new(),
            run_json: String::new(),
            ticks: 0,
            cards: ScrollHandle::new(),
        });
    }

    fn kiln_intent(&mut self, intent: Intent, cx: &mut Context<Self>) {
        let Some(ui) = self.grid.kiln_ui.as_mut() else {
            return;
        };
        match intent {
            Intent::Run => {
                let task = ui.task.read(cx).text();
                if !task.trim().is_empty() && call_kiln("kilnAgent", &[&task]) {
                    track("kiln_run", "{}");
                }
            }
            Intent::Stop => {
                call_kiln("kilnStop", &[]);
            }
            Intent::Load => {
                let slice = ui.slice.read(cx).text();
                if !slice.trim().is_empty() {
                    call_kiln("kilnLoad", &[slice.trim()]);
                }
            }
            Intent::Publish => {
                if call_kiln("kilnPublish", &[]) {
                    track("kiln_publish", "{}");
                }
            }
            Intent::Review(path) => {
                if let Some(window) = web_sys::window() {
                    let _ =
                        window.open_with_url_and_target_and_features(&path, "_blank", "noopener");
                }
            }
            Intent::Task | Intent::Slice => {}
        }
        // Read the state again on the next tick.
        ui.ticks = 0;
        ui.state_json.clear();
        cx.notify();
    }

    /// The panel's body: the cards beside the run on a wide page, over it
    /// on a narrow one.
    pub(super) fn render_kiln_body(
        &mut self,
        narrow: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.kiln_ui(window, cx);
        let Some(ui) = self.grid.kiln_ui.as_ref() else {
            return div().into_any_element();
        };
        let task_text = ui.task.read(cx).text();
        let slice_text = ui.slice.read(cx).text();
        let view = kiln_view::cards(&ui.state, &task_text, &slice_text, 1, &paints());
        let (task, slice) = (ui.task.clone(), ui.slice.clone());
        let resolve = view.clone();
        // The adapter asks for each field's control: this view's own text
        // inputs, kept across frames.
        let field = move |key: &str, _: &mut Context<LanderChat>| {
            let (input, tall) = match key {
                kiln_view::TASK_FIELD => (task.clone(), true),
                kiln_view::SLICE_FIELD => (slice.clone(), false),
                _ => return None,
            };
            Some(
                div()
                    .w_full()
                    .min_w(px(120.))
                    .when(tall, |d| d.min_h(px(132.)))
                    .px(px(10.))
                    .py(px(8.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(theme::border_strong())
                    .bg(theme::input_bg())
                    .text_size(px(13.))
                    .text_color(theme::text())
                    .child(input)
                    .into_any_element(),
            )
        };
        let cards = rust_native_gpui::mount_root_with_fields(
            &view.view().root,
            &view.view().instance,
            view.view().revision,
            move |this: &mut LanderChat, activation, cx| {
                if let Some(intent) = kiln_view::intent(&resolve, &activation.node) {
                    this.kiln_intent(intent, cx);
                }
            },
            field,
            cx,
        );
        let cards = div()
            .id("kiln-cards")
            .track_scroll(&ui.cards)
            .overflow_y_scroll()
            .flex_none()
            .p(px(12.))
            .child(cards);
        let run = div()
            .id("kiln-run")
            .flex_1()
            .min_h_0()
            .min_w_0()
            .child(ui.transcript.clone());
        let body = div()
            .id("kiln-body")
            .track_scroll(&self.grid.kiln_body)
            .flex_1()
            .min_h_0()
            .flex()
            .bg(gpui::hsla(0., 0., 0.035, 0.86));
        if narrow {
            body.flex_col()
                .child(cards.w_full().max_h(px(360.)))
                .child(run.w_full())
                .into_any_element()
        } else {
            body.flex_row()
                .child(
                    cards
                        .w(px(CARDS_W))
                        .h_full()
                        .border_r_1()
                        .border_color(theme::border()),
                )
                .child(run.h_full())
                .into_any_element()
        }
    }
}
