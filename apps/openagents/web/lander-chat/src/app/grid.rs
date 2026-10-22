//! The Grid homepage's overlay (#12459): everything the page shows over the
//! world besides the world itself, drawn here rather than in HTML.
//!
//! - The top bar: the name, and the page's account links repeated. The
//!   links stay in the page, transparent, for keyboards and screen readers,
//!   and a click on the bar clicks them, so their visit events still fire.
//! - The chat (`super`), while the visitor is at the terminal, the help
//!   desk, or in Game Lab.
//! - Game Lab's panel: opened in the world, under its arch, it checks that
//!   the visitor is signed in, then shows their version of the game with
//!   Play and Compare beside the chat.
//! - Kiln's panel, for whom the site offers it: its cards and its run
//!   (`kiln`).
//!
//! The canvas covers the page, so each frame the chat tells the page which
//! boxes it drew in ([`model::Overlay`]): the page clips the canvas to them,
//! so the world under the rest still takes clicks, and stands the pages it
//! frames (Kiln, the games) in their boxes.

use std::collections::BTreeSet;

use gpui::{Bounds, Pixels};
use openagents_web_chat_view::labels::button;

use super::*;
use crate::model::{Frame, GameCard, Overlay, Rect};

/// How often the top bar reads the page's account links again, in ticks.
const LINKS_EVERY: u32 = 15;

/// One of the page's account links, as the top bar repeats it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Link {
    text: String,
    solid: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum LabState {
    #[default]
    Closed,
    Opening,
    SignIn,
    Open,
    Failed,
}

#[derive(Default)]
pub(super) struct Lab {
    pub(super) state: LabState,
    opened_at: Option<Instant>,
    card: Option<GameCard>,
    /// A message is on its way or being answered in Game Lab's chat.
    checking: bool,
    /// Versions already seen, so a saved version counts once.
    seen: BTreeSet<u64>,
    /// The game shown full screen.
    pub(super) playing: Option<String>,
    /// Bumped on each open, so an answer for a closed panel is dropped.
    generation: u64,
    /// The page's count of finished popup sign-ins last seen.
    sign_ins: f64,
}

#[derive(Default)]
pub(super) struct Grid {
    /// The chat's composer is open.
    pub(super) open: bool,
    links: Vec<Link>,
    pub(super) lab: Lab,
    pub(super) kiln: bool,
    /// Kiln's panel, made the first time it opens (#12456).
    pub(super) kiln_ui: Option<super::kiln::KilnUi>,
    ticks: u32,
    brand: ScrollHandle,
    bar: ScrollHandle,
    panel: ScrollHandle,
    previews: [ScrollHandle; 2],
    kiln_panel: ScrollHandle,
    pub(super) kiln_body: ScrollHandle,
    back: ScrollHandle,
    /// What the page was last told, as JSON.
    published: String,
}

/// Reads a page global as a flag.
fn flag(name: &str) -> bool {
    js_sys::Reflect::get(&js_sys::global(), &name.into())
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn set_global(name: &str, value: &JsValue) {
    let _ = js_sys::Reflect::set(&js_sys::global(), &name.into(), value);
}

/// Calls a page function with one argument.
pub(super) fn call(name: &str, arg: &JsValue) {
    if let Ok(callback) = js_sys::Reflect::get(&js_sys::global(), &name.into())
        && let Some(callback) = callback.dyn_ref::<js_sys::Function>()
    {
        let _ = callback.call1(&JsValue::UNDEFINED, arg);
    }
}

/// The page's account links, in order.
fn account_links() -> Vec<web_sys::HtmlElement> {
    let Some(list) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.query_selector_all(model::ACCOUNT_LINKS).ok())
    else {
        return Vec::new();
    };
    (0..list.length())
        .filter_map(|i| list.item(i))
        .filter_map(|node| node.dyn_into::<web_sys::HtmlElement>().ok())
        .collect()
}

/// Says `text` to screen readers in the page's Game Lab section, and shows
/// or hides its sign-in link.
fn say_lab(text: &str, sign_in: bool) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    if let Some(status) = document.get_element_by_id("game-lab-status") {
        status.set_text_content(Some(text));
    }
    if let Some(link) = document
        .get_element_by_id("game-lab-signin")
        .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
    {
        link.set_hidden(!sign_in);
    }
}

fn rect_of(bounds: Bounds<Pixels>) -> [f32; 4] {
    [
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        f32::from(bounds.size.height),
    ]
}

/// `inner` cut to `outer`; `None` when nothing is left.
fn within(inner: [f32; 4], outer: [f32; 4]) -> Option<[f32; 4]> {
    let left = inner[0].max(outer[0]);
    let top = inner[1].max(outer[1]);
    let right = (inner[0] + inner[2]).min(outer[0] + outer[2]);
    let bottom = (inner[1] + inner[3]).min(outer[1] + outer[3]);
    (right - left > 1. && bottom - top > 1.).then_some([left, top, right - left, bottom - top])
}

impl LanderChat {
    /// The overlay's part of the tick, on the Grid homepage only.
    pub(super) fn grid_tick(&mut self, cx: &mut Context<Self>) {
        let open = flag(model::CHAT_OPEN_GLOBAL);
        if open != self.grid.open {
            self.grid.open = open;
            cx.notify();
        }
        let kiln = flag(model::KILN_OPEN_GLOBAL) && self.kiln_offered;
        if kiln != self.grid.kiln {
            self.grid.kiln = kiln;
            cx.notify();
        }
        // Kiln keeps working with its panel closed, and the terminal's
        // screen keeps saying so.
        if self.grid.kiln_ui.is_some() {
            self.kiln_tick(cx);
        }
        if self.grid.ticks % LINKS_EVERY == 0 {
            let links: Vec<Link> = account_links()
                .iter()
                .map(|a| Link {
                    text: a.text_content().unwrap_or_default().trim().to_owned(),
                    solid: a.class_list().contains("solid"),
                })
                .filter(|link| !link.text.is_empty())
                .collect();
            if links != self.grid.links {
                self.grid.links = links;
                cx.notify();
            }
        }
        self.grid.ticks = self.grid.ticks.wrapping_add(1);
        if self.grid.lab.playing.is_some() && !flag(model::GAME_PLAYING_GLOBAL) {
            self.grid.lab.playing = None;
            cx.notify();
        }
        // A popup sign-in finished: Game Lab opens signed in where the
        // visitor stands (#12480).
        let sign_ins = js_sys::Reflect::get(&js_sys::global(), &model::SIGNED_IN_GLOBAL.into())
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.);
        if sign_ins != self.grid.lab.sign_ins {
            self.grid.lab.sign_ins = sign_ins;
            if self.grid.lab.state == LabState::SignIn {
                self.open_lab(cx);
            }
        }
        let wanted = flag(model::GAME_LAB_GLOBAL);
        match (wanted, self.grid.lab.state) {
            (true, LabState::Closed) => self.open_lab(cx),
            (false, LabState::Closed) => {}
            (false, _) => self.close_lab(cx),
            _ => {}
        }
        if self.grid.lab.state == LabState::Open && is_game_lab() {
            let checking = self.live.is_some() || self.sending;
            if checking && !self.grid.lab.checking {
                track("game_lab_check", "{}");
            }
            if checking != self.grid.lab.checking {
                self.grid.lab.checking = checking;
                say_lab(lab_line(&self.grid.lab), false);
                cx.notify();
            }
        }
        self.publish_overlay();
    }

    /// Checks that the visitor may make their own version, then shows it.
    fn open_lab(&mut self, cx: &mut Context<Self>) {
        let lab = &mut self.grid.lab;
        lab.generation += 1;
        lab.state = LabState::Opening;
        lab.opened_at = Some(Instant::now());
        lab.card = None;
        lab.playing = None;
        lab.checking = false;
        let generation = lab.generation;
        say_lab(model::GAME_OPENING, false);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let read = Client::from_page()
                .get::<serde_json::Value>(model::GAME_SESSION)
                .await;
            this.update(cx, |this, cx| {
                if this.grid.lab.generation != generation
                    || this.grid.lab.state != LabState::Opening
                {
                    return;
                }
                let code = read
                    .as_ref()
                    .err()
                    .and_then(|e| e.downcast_ref::<Refusal>())
                    .map(|r| r.code.clone());
                match (&read, code.as_deref()) {
                    (Ok(_), _) => {
                        this.grid.lab.state = LabState::Open;
                        set_global("openagentsLab", &JsValue::from_str("game"));
                        call(model::TERMINAL_CALL, &JsValue::TRUE);
                        say_lab(model::GAME_ASK, false);
                        track("game_lab_opened", "{\"result\":\"open\"}");
                        this.load_card(None, false, false, cx);
                    }
                    (Err(_), Some("http_401" | "http_403" | "signed_out" | "forbidden")) => {
                        this.grid.lab.state = LabState::SignIn;
                        say_lab(model::GAME_SIGN_IN_LINE, true);
                        track("game_lab_opened", "{\"result\":\"sign_in\"}");
                    }
                    (Err(error), _) => {
                        log::warn!("Game Lab session: {error:#}");
                        this.grid.lab.state = LabState::Failed;
                        say_lab(model::GAME_FAILED, false);
                        track("game_lab_opened", "{\"result\":\"failed\"}");
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn close_lab(&mut self, cx: &mut Context<Self>) {
        let lab = &mut self.grid.lab;
        if let Some(at) = lab.opened_at.take() {
            track(
                "game_lab_closed",
                &format!("{{\"time\":{}}}", at.elapsed().as_millis()),
            );
        }
        lab.generation += 1;
        lab.state = LabState::Closed;
        lab.card = None;
        lab.checking = false;
        if lab.playing.take().is_some() {
            call(model::GAME_PLAY_CALL, &JsValue::NULL);
        }
        say_lab("", false);
        set_global("openagentsLab", &JsValue::UNDEFINED);
        call(model::TERMINAL_CALL, &JsValue::FALSE);
        cx.notify();
    }

    /// Reads the card for `version` (the newest when `None`), compared with
    /// the original when `compare`, and plays it at once when `autoplay`.
    fn load_card(
        &mut self,
        version: Option<u64>,
        compare: bool,
        autoplay: bool,
        cx: &mut Context<Self>,
    ) {
        let generation = self.grid.lab.generation;
        let path = model::game_card_path(version, compare);
        cx.spawn(async move |this, cx| {
            let read = Client::from_page().get::<GameCard>(&path).await;
            this.update(cx, |this, cx| {
                if this.grid.lab.generation != generation || this.grid.lab.state != LabState::Open {
                    return;
                }
                match read {
                    Ok(card) => {
                        if autoplay && !card.play.is_empty() {
                            this.play(card.play.clone(), "chat");
                        }
                        this.grid.lab.seen.insert(card.version);
                        this.grid.lab.card = Some(card);
                    }
                    Err(error) => log::warn!("Game Lab card: {error:#}"),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// A card from the chat's answer: the version it saved or compared.
    pub(super) fn game_cards(&mut self, cards: &serde_json::Value, cx: &mut Context<Self>) {
        if self.grid.lab.state != LabState::Open {
            return;
        }
        let Some((version, compare, autoplay)) = model::stream_card(cards) else {
            return;
        };
        // A card arrives only in an answer to the visitor, and it can come
        // after the answer ends, so a new version counts as saved however
        // the answer stands.
        if compare {
            track("game_lab_compared", "{}");
        } else if !autoplay && !self.grid.lab.seen.contains(&version) {
            track("game_lab_version_saved", "{}");
        }
        self.grid.lab.seen.insert(version);
        self.load_card(Some(version), compare, autoplay, cx);
    }

    /// Shows `src` full screen, over the world and the panel.
    fn play(&mut self, src: String, from: &str) {
        call(model::GAME_PLAY_CALL, &JsValue::from_str(&src));
        call(model::TERMINAL_CALL, &JsValue::FALSE);
        self.grid.lab.playing = Some(src);
        track("game_lab_played", &format!("{{\"from\":\"{from}\"}}"));
    }

    fn stop_playing(&mut self, cx: &mut Context<Self>) {
        if self.grid.lab.playing.take().is_some() {
            call(model::GAME_PLAY_CALL, &JsValue::NULL);
            call(model::TERMINAL_CALL, &JsValue::TRUE);
            cx.notify();
        }
    }

    /// Tells the page where the canvas draws and where its frames stand,
    /// when that changed.
    fn publish_overlay(&mut self) {
        let mut overlay = Overlay::default();
        let lab = &self.grid.lab;
        if lab.playing.is_some() {
            overlay.regions.push(rect_of(self.grid.back.bounds()));
        } else {
            overlay.regions.push(rect_of(self.grid.brand.bounds()));
            if !self.grid.links.is_empty() {
                overlay.regions.push(rect_of(self.grid.bar.bounds()));
            }
            if self.grid.open
                && let Some((x, y, w, h)) = self.kept
            {
                overlay.regions.push([x, y, w, h]);
            }
            if lab.state != LabState::Closed {
                let panel = rect_of(self.grid.panel.bounds());
                overlay.regions.push(panel);
                if let Some(card) = &lab.card {
                    let sides: Vec<(&'static str, &str, String)> = match &card.compare {
                        Some(compare) => vec![
                            (
                                "game-a",
                                compare.a.preview.as_str(),
                                format!("Version {}, the bot playing", compare.a.version),
                            ),
                            (
                                "game-b",
                                compare.b.preview.as_str(),
                                format!("Version {}, the bot playing", compare.b.version),
                            ),
                        ],
                        None => vec![(
                            "game",
                            card.preview.as_str(),
                            format!("{}, version {}", card.title, card.version),
                        )],
                    };
                    for ((id, src, title), handle) in sides.into_iter().zip(&self.grid.previews) {
                        if let Some([x, y, w, h]) = within(rect_of(handle.bounds()), panel)
                            && !src.is_empty()
                        {
                            overlay.frames.push(Frame {
                                id,
                                src: src.to_owned(),
                                title,
                                x,
                                y,
                                w,
                                h,
                                keep: false,
                            });
                        }
                    }
                }
            }
            if self.grid.kiln {
                // Kiln draws here, in GPUI (#12456); no page stands in it.
                overlay.regions.push(rect_of(self.grid.kiln_panel.bounds()));
            }
        }
        overlay.regions.retain(|r| r[2] > 0. && r[3] > 0.);
        let Ok(json) = serde_json::to_string(&overlay) else {
            return;
        };
        if json != self.grid.published {
            call(model::OVERLAY_CALL, &JsValue::from_str(&json));
            self.grid.published = json;
        }
    }

    /// The top bar: the name on the left, the account links on the right.
    pub(super) fn render_bar(&self, narrow: bool, cx: &mut Context<Self>) -> AnyElement {
        let edge = if narrow { 14. } else { 18. };
        let brand = div()
            .id("grid-brand")
            .track_scroll(&self.grid.brand)
            .absolute()
            .top(px(14.))
            .left(px(edge))
            .h(px(24.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.))
            .px(px(2.))
            .text_size(px(15.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme::text())
            .child(
                svg()
                    .path("icons/openagents-logo.svg")
                    .size(px(18.))
                    .text_color(theme::text()),
            )
            .child(model::BRAND);
        let mut links = div()
            .id("grid-account")
            .track_scroll(&self.grid.bar)
            .absolute()
            .top(px(10.))
            .right(px(if narrow { 12. } else { 16. }))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.));
        for (i, link) in self.grid.links.iter().enumerate() {
            let pill = div()
                .id(("grid-link", i))
                .h(px(30.))
                .px(px(14.))
                .flex()
                .items_center()
                .rounded(px(15.))
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .when(link.solid, |pill| {
                    pill.bg(theme::solid())
                        .text_color(theme::on_solid())
                        .hover(|s| s.bg(gpui::white()))
                })
                .when(!link.solid, |pill| {
                    pill.border_1()
                        .border_color(theme::border_strong())
                        .bg(gpui::hsla(0., 0., 0.04, 0.6))
                        .text_color(theme::text())
                        .hover(|s| s.bg(gpui::hsla(0., 0., 0.12, 0.75)))
                })
                .on_click(cx.listener(move |_, _: &ClickEvent, _, _| {
                    if let Some(a) = account_links().get(i) {
                        a.click();
                    }
                }))
                .child(SharedString::from(link.text.clone()));
            links = links.child(pill);
        }
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(brand)
            .when(!self.grid.links.is_empty(), |bar| bar.child(links))
            .into_any_element()
    }

    /// "Back to Game Lab" over a game shown full screen.
    pub(super) fn render_back(&self, cx: &mut Context<Self>) -> AnyElement {
        // Centered at the top, clear of the game's own corners.
        div()
            .absolute()
            .top(px(16.))
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .child(
                button("game-back", model::BACK_TO_LAB, false)
                    .track_scroll(&self.grid.back)
                    .bg(gpui::hsla(0., 0., 0.04, 0.85))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.stop_playing(cx))),
            )
            .into_any_element()
    }

    /// Game Lab's panel: its name and Back to the Grid, then what the
    /// visitor can do now.
    pub(super) fn render_lab(&self, w: f32, h: f32, cx: &mut Context<Self>) -> AnyElement {
        let Rect {
            x,
            y,
            w: width,
            h: room,
        } = if self.grid.lab.state == LabState::SignIn {
            model::sign_in_panel(w, h)
        } else {
            model::game_panel(w, h)
        };
        let lab = &self.grid.lab;
        let heading = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .child(
                div()
                    .text_size(px(16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(model::GAME_LAB),
            )
            .child(
                button("game-close", model::BACK_TO_GRID, false).on_click(cx.listener(
                    |_, _: &ClickEvent, _, _| {
                        call(model::GAME_LAB_CALL, &JsValue::FALSE);
                    },
                )),
            );
        let line = |text: &str| {
            div()
                .text_size(px(13.))
                .line_height(px(19.))
                .text_color(theme::text_muted())
                .child(SharedString::from(text.to_owned()))
        };
        let mut body = div()
            .id("game-panel")
            .track_scroll(&self.grid.panel)
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(width))
            .max_h(px(room))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(16.))
            .rounded(px(16.))
            .border_1()
            .border_color(theme::border_strong())
            .bg(gpui::hsla(0., 0., 0.035, 0.9))
            .child(heading)
            .child(line(lab_line(lab)));
        if lab.state == LabState::SignIn {
            body = body.child(div().flex().flex_row().child(
                button("game-sign-in", model::SIGN_IN, true).on_click(|_, _, _| {
                    // In a popup, so the Grid stays loaded; the page falls
                    // back to the full-page sign-in when popups are blocked.
                    call(model::SIGN_IN_CALL, &JsValue::from_str(model::GAME_SIGN_IN));
                }),
            ));
        }
        if lab.state == LabState::Open
            && let Some(card) = &lab.card
        {
            body = body.child(self.render_card_view(card, width - 32., cx));
        }
        body.into_any_element()
    }

    /// One version of the game, or two compared.
    fn render_card_view(&self, card: &GameCard, width: f32, cx: &mut Context<Self>) -> AnyElement {
        let caption = |text: String| {
            div()
                .text_size(px(12.5))
                .line_height(px(18.))
                .text_color(theme::text_muted())
                .child(SharedString::from(text))
        };
        let frame = |handle: &ScrollHandle, id: &'static str, w: f32| {
            div()
                .id(id)
                .track_scroll(handle)
                .flex_none()
                .w(px(w))
                .h(px((w * 0.625).round()))
                .rounded(px(10.))
                .overflow_hidden()
                .bg(gpui::hsla(0., 0., 0.02, 1.))
                .border_1()
                .border_color(theme::border())
        };
        let mut column = div().flex().flex_col().gap(px(10.)).child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(14.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(SharedString::from(card.heading())),
                )
                .when(!card.summary.is_empty(), |c| {
                    c.child(caption(card.summary.clone()))
                }),
        );
        match &card.compare {
            Some(compare) => {
                let half = ((width - 10.) / 2.).floor();
                let side = |side: &model::GameSide, handle, id| {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .w(px(half))
                        .child(frame(handle, id, half))
                        .child(caption(format!("v{}: {}", side.version, side.summary)))
                        .when(!side.stats.is_empty(), |c| {
                            c.child(caption(side.stats.clone()))
                        })
                };
                column = column
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(10.))
                            .child(side(&compare.a, &self.grid.previews[0], "game-preview-a"))
                            .child(side(&compare.b, &self.grid.previews[1], "game-preview-b")),
                    )
                    .child(caption(model::BOT_NOTE.to_owned()));
                for line in compare.differences.iter().chain(&compare.changes) {
                    column = column.child(caption(format!("\u{2022} {line}")));
                }
                let latest = card.version;
                column = column.child(div().flex().flex_row().child(
                    button("game-latest", format!("v{latest}"), false).on_click(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.load_card(Some(latest), false, false, cx)
                        },
                    )),
                ));
            }
            None => {
                let play = card.play.clone();
                let version = card.version;
                column = column
                    .child(frame(&self.grid.previews[0], "game-preview", width))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap(px(8.))
                            .child(button("game-play", model::PLAY, true).on_click(cx.listener(
                                move |this, _: &ClickEvent, _, cx| {
                                    this.play(play.clone(), "card");
                                    cx.notify();
                                },
                            )))
                            .when(card.can_compare, |row| {
                                row.child(button("game-compare", model::COMPARE, false).on_click(
                                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        track("game_lab_compared", "{}");
                                        this.load_card(Some(version), true, false, cx)
                                    }),
                                ))
                            }),
                    )
                    .when(!card.stats.is_empty(), |c| {
                        c.child(caption(card.stats.clone()))
                    });
            }
        }
        let mut meta = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.))
            .text_size(px(12.5))
            .text_color(theme::text_faint());
        if let Some(player) = card.player.clone() {
            meta = meta.child(
                div()
                    .id("game-own-window")
                    .cursor_pointer()
                    .text_color(theme::text_muted())
                    .hover(|s| s.text_color(theme::text()))
                    .on_click(move |_, _, _| {
                        if let Some(window) = web_sys::window() {
                            let _ = window.open_with_url_and_target_and_features(
                                &player, "_blank", "noopener",
                            );
                        }
                    })
                    .child(model::OWN_WINDOW),
            );
        }
        if card.versions > 0 {
            meta = meta.child(SharedString::from(card.versions_words()));
        }
        column.child(meta).into_any_element()
    }

    /// Kiln's panel: its name, a new tab, and Back to the Grid over Kiln's
    /// cards and its run.
    pub(super) fn render_kiln_panel(
        &mut self,
        w: f32,
        h: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Rect {
            x,
            y,
            w: width,
            h: height,
        } = model::kiln_panel(w, h);
        div()
            .id("kiln-panel")
            .track_scroll(&self.grid.kiln_panel)
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(width))
            .h(px(height))
            .flex()
            .flex_col()
            .rounded(px(16.))
            .overflow_hidden()
            .border_1()
            .border_color(theme::border_strong())
            .child(
                div()
                    .flex_none()
                    .bg(gpui::hsla(0., 0., 0.035, 0.88))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .px(px(14.))
                    .py(px(8.))
                    .border_b_1()
                    .border_color(theme::border())
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(model::KILN),
                    )
                    .child(
                        button("kiln-tab", model::NEW_TAB, false).on_click(|_, _, _| {
                            if let Some(window) = web_sys::window() {
                                let _ = window.open_with_url_and_target_and_features(
                                    model::KILN_PAGE,
                                    "_blank",
                                    "noopener",
                                );
                            }
                        }),
                    )
                    .child(
                        button("kiln-close", model::BACK_TO_GRID, false).on_click(cx.listener(
                            |_, _: &ClickEvent, _, _| {
                                call(model::OPEN_KILN, &JsValue::FALSE);
                            },
                        )),
                    ),
            )
            .child(self.render_kiln_body(width < 760., window, cx))
            .into_any_element()
    }
}

/// The panel's line for where Game Lab is.
fn lab_line(lab: &Lab) -> &'static str {
    match lab.state {
        LabState::Closed | LabState::Opening => model::GAME_OPENING,
        LabState::SignIn => model::GAME_SIGN_IN_LINE,
        LabState::Failed => model::GAME_FAILED,
        LabState::Open if lab.checking => model::GAME_CHECKING,
        LabState::Open => model::GAME_ASK,
    }
}
