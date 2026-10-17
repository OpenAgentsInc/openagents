//! The Lab row (#11883): `lab-ui`'s Rust Native card for a saved chat's Lab
//! card, drawn with GPUI.
//!
//! The card's text, notes and its open button come from `lab_ui::card`, the
//! same view the phone and the web draw. Its one surface is the card's
//! picture, chosen by view in `openagents_account::lab_cards::pictures`:
//! an image with its strip, both sides of a compare, a video's poster (and
//! the video itself on macOS, through the Lab card's AVFoundation player), or
//! a model's or game's poster. A report draws no picture.
//!
//! Files come only from the card's own artifacts, read through the signed-in
//! account. When the account changes or signs out, the row drops its pictures
//! and stops playing.
//!
//! While the row exists it follows its document's `/v1/labs` events (#11943),
//! so a running job's state and new notes show without waiting for the
//! chat's history. A note added here is anchored at the video's playing
//! time.

use std::collections::HashMap;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, EventEmitter, Focusable as _, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled,
    StyledImage as _, Task, Window, div, img, px,
};
use lab_kit::Card;
use lab_ui::Intent;
use openagents_account::lab_cards::{Media, Pictures, pictures};
use rust_native::{Activation, Axis, Element, Node, TextRole};

use crate::image_media::MediaImage;
use crate::theme::Theme;

/// Reads one of the card's files.
pub type Fetch = Rc<dyn Fn(String, &mut App) -> Task<Option<Media>>>;
/// Whether the account that read the card is still the signed-in one.
pub type Current = Rc<dyn Fn() -> bool>;
/// Adds a note at a playing time; the error is the sentence to show.
pub type AddNote = Rc<dyn Fn(Option<f64>, String, &mut App) -> Task<Result<(), String>>>;

/// The note being written.
struct NoteDraft {
    input: gpui::Entity<crate::composer::ComposerInput>,
    at: Option<f64>,
    error: Option<String>,
    saving: bool,
    focus: bool,
    _events: gpui::Subscription,
}

/// The row's size changed (a picture arrived), so the transcript remeasures it.
pub struct LabLayoutChanged;

enum Load {
    Loading,
    Image(MediaImage),
    Video(String),
    Missing,
}

pub struct LabView {
    id: SharedString,
    space: String,
    revision: u64,
    card: Card,
    projection: Option<lab_ui::Projection>,
    pictures: Pictures,
    files: HashMap<String, Load>,
    page: Option<String>,
    fetch: Fetch,
    current: Current,
    #[cfg(target_os = "macos")]
    player: Option<crate::lab_card::player::VideoPlayer>,
    released: bool,
    add_note: Option<AddNote>,
    draft: Option<NoteDraft>,
    /// Stops the row's event follower.
    stop: Option<tokio_util::sync::CancellationToken>,
}

impl EventEmitter<LabLayoutChanged> for LabView {}

impl LabView {
    pub fn new(
        id: SharedString,
        card: Card,
        space: &str,
        page: Option<String>,
        fetch: Fetch,
        current: Current,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.on_release(|this, cx| this.release(cx)).detach();
        let projection = lab_ui::card(&card, space, &id, 1).ok();
        let pictures = pictures(&card);
        let mut this = Self {
            space: space.to_owned(),
            revision: 1,
            id,
            card,
            projection,
            pictures,
            files: HashMap::new(),
            page,
            fetch,
            current,
            #[cfg(target_os = "macos")]
            player: None,
            released: false,
            add_note: None,
            draft: None,
            stop: None,
        };
        this.load(cx);
        this
    }

    /// Lets the row add notes.
    pub fn set_add_note(&mut self, add_note: AddNote) {
        self.add_note = Some(add_note);
    }

    /// The follower to stop when the row goes away.
    pub fn set_follower(&mut self, stop: tokio_util::sync::CancellationToken) {
        if let Some(old) = self.stop.replace(stop) {
            old.cancel();
        }
    }

    /// Shows a newer copy of the card (a note, a finished job). The pictures
    /// and a playing video stay when the card's files didn't change.
    pub fn set_card(&mut self, card: Card, cx: &mut Context<Self>) {
        if self.released || card == self.card {
            return;
        }
        let pictures = pictures(&card);
        self.projection = lab_ui::card(&card, &self.space, &self.id, self.revision + 1).ok();
        self.revision += 1;
        self.card = card;
        if pictures != self.pictures {
            #[cfg(target_os = "macos")]
            if let Some(player) = self.player.take() {
                player.pause();
            }
            let images = self.files.drain().filter_map(|(_, load)| match load {
                Load::Image(image) => Some(image),
                _ => None,
            });
            crate::image_media::release_media(images.collect::<Vec<_>>(), cx);
            self.pictures = pictures;
            self.load(cx);
        }
        cx.emit(LabLayoutChanged);
        cx.notify();
    }

    /// The video's playing time, once it has played.
    fn playing_at(&self) -> Option<f64> {
        #[cfg(target_os = "macos")]
        if let Some(player) = &self.player {
            let at = player.position();
            return at.is_finite().then_some(at);
        }
        None
    }

    fn start_note(&mut self, cx: &mut Context<Self>) {
        if self.add_note.is_none() || self.released {
            return;
        }
        let at = self.playing_at();
        let placeholder = match at {
            Some(at) => format!("Note at {}", lab_ui::clock(at)),
            None => "Your note".to_owned(),
        };
        let input = cx.new(|cx| {
            crate::composer::ComposerInput::new(placeholder, cx)
                .with_single_line()
                .with_text_metrics(13.0, 17.0)
                .with_accessibility_role(gpui::Role::TextInput)
        });
        let events = cx.subscribe(&input, |this: &mut Self, _, event, cx| {
            if matches!(event, crate::composer::ComposerInputEvent::Submitted) {
                this.submit_note(cx);
            }
        });
        self.draft = Some(NoteDraft {
            input,
            at,
            error: None,
            saving: false,
            focus: true,
            _events: events,
        });
        cx.emit(LabLayoutChanged);
        cx.notify();
    }

    fn submit_note(&mut self, cx: &mut Context<Self>) {
        let (Some(add_note), Some(draft)) = (self.add_note.clone(), self.draft.as_mut()) else {
            return;
        };
        let text = draft.input.read(cx).text().trim().to_owned();
        if text.is_empty() || draft.saving {
            return;
        }
        draft.saving = true;
        draft.error = None;
        let job = add_note(draft.at, text, cx);
        cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(()) => this.draft = None,
                    Err(message) => {
                        if let Some(draft) = this.draft.as_mut() {
                            draft.saving = false;
                            draft.error = Some(message);
                        }
                    }
                }
                cx.emit(LabLayoutChanged);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn cancel_note(&mut self, cx: &mut Context<Self>) {
        self.draft = None;
        cx.emit(LabLayoutChanged);
        cx.notify();
    }

    /// The files the picture shows, in the order it draws them.
    fn wanted(&self) -> Vec<String> {
        let p = &self.pictures;
        let pair = p
            .pair
            .iter()
            .flat_map(|(a, b)| [a.clone(), b.clone()])
            .flatten();
        let mut all: Vec<String> = Vec::new();
        for id in p
            .main
            .iter()
            .cloned()
            .chain(pair)
            .chain(p.plays.iter().cloned())
            .chain(p.strip.iter().cloned())
        {
            if !all.contains(&id) {
                all.push(id);
            }
        }
        all
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        for id in self.wanted() {
            self.files.insert(id.clone(), Load::Loading);
            let job = (self.fetch)(id.clone(), cx);
            cx.spawn(async move |this, cx| {
                let media = job.await;
                let decoded = match media {
                    Some(Media::Image(bytes)) => cx
                        .background_executor()
                        .spawn(async move {
                            crate::image_media::decode_image("application/octet-stream", bytes).ok()
                        })
                        .await
                        .map_or(Load::Missing, Load::Image),
                    Some(Media::Video(url)) => Load::Video(url),
                    Some(Media::Other) | None => Load::Missing,
                };
                let _ = this.update(cx, |this, cx| {
                    if this.released {
                        if let Load::Image(image) = decoded {
                            crate::image_media::release_media([image], cx);
                        }
                        return;
                    }
                    this.files.insert(id, decoded);
                    cx.emit(LabLayoutChanged);
                    cx.notify();
                });
            })
            .detach();
        }
    }

    /// Drops every picture and stops playing.
    fn release(&mut self, cx: &mut App) {
        self.released = true;
        self.draft = None;
        if let Some(stop) = self.stop.take() {
            stop.cancel();
        }
        #[cfg(target_os = "macos")]
        if let Some(player) = self.player.take() {
            player.pause();
        }
        let images = self.files.drain().filter_map(|(_, load)| match load {
            Load::Image(image) => Some(image),
            _ => None,
        });
        crate::image_media::release_media(images.collect::<Vec<_>>(), cx);
    }

    /// The card this row draws.
    pub fn card(&self) -> &Card {
        &self.card
    }

    /// Whether the row still shows anything: false once the account changed.
    pub fn showing(&self) -> bool {
        !self.released
    }

    /// How many pictures have arrived (for tests and fixtures).
    pub fn loaded(&self) -> usize {
        self.files
            .values()
            .filter(|l| matches!(l, Load::Image(_) | Load::Video(_)))
            .count()
    }

    fn activate(&mut self, node: String, cx: &mut Context<Self>) {
        let Some(projection) = &self.projection else {
            return;
        };
        let view = projection.view.view();
        let activation = Activation {
            instance: view.instance.clone(),
            revision: view.revision,
            node,
        };
        if let Ok(Intent::Open { .. }) = projection.view.activate(&activation)
            && let Some(page) = &self.page
        {
            cx.open_url(page);
        }
    }

    fn image(&self, id: Option<&String>) -> Option<std::sync::Arc<gpui::Image>> {
        match self.files.get(id?)? {
            Load::Image(image) => Some(image.image.clone()),
            _ => None,
        }
    }

    fn video(&self) -> Option<String> {
        match self.files.get(self.pictures.plays.as_ref()?)? {
            Load::Video(url) => Some(url.clone()),
            _ => None,
        }
    }

    #[cfg(target_os = "macos")]
    fn toggle(&mut self, cx: &mut Context<Self>) {
        match &self.player {
            Some(player) if player.playing() => player.pause(),
            Some(player) => player.play(),
            None => {
                self.player = self
                    .video()
                    .and_then(|url| crate::lab_card::player::VideoPlayer::new(&url));
                if let Some(player) = &self.player {
                    player.play();
                }
            }
        }
        cx.notify();
    }

    #[cfg(not(target_os = "macos"))]
    fn toggle(&mut self, cx: &mut Context<Self>) {
        cx.notify();
    }

    fn screen(
        &self,
        key: &str,
        image: Option<std::sync::Arc<gpui::Image>>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(SharedString::from(format!("{}:{key}", self.id)))
            .relative()
            .w_full()
            .aspect_ratio(16.0 / 9.0)
            .rounded(px(Theme::CONTROL_RADIUS))
            .overflow_hidden()
            .bg(gpui::black().opacity(0.35))
            .when_some(image, |d, image| {
                d.child(img(image).object_fit(gpui::ObjectFit::Contain).size_full())
            })
    }

    /// The card's surface: its picture by view.
    fn picture(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::of(cx).clone();
        let small = crate::typography::ui_rems(12.0);
        if let Some((a, b)) = self.pictures.pair.clone() {
            let label = |side: Option<&lab_kit::Reference>| {
                side.map(|r| format!("v{}", r.version)).unwrap_or_default()
            };
            let compare = self.card.compare.clone();
            let side = |key: &str, id: Option<String>, text: String| {
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(self.screen(key, self.image(id.as_ref())))
                    .child(
                        div()
                            .text_size(small)
                            .text_color(theme.text_muted)
                            .child(text),
                    )
            };
            return div()
                .flex()
                .flex_row()
                .gap(px(8.))
                .child(side("a", a, label(compare.as_ref().map(|c| &c.a))))
                .child(side("b", b, label(compare.as_ref().map(|c| &c.b))))
                .into_any_element();
        }

        #[cfg(target_os = "macos")]
        let frame = match self.player.as_mut() {
            Some(player) => {
                if player.playing() {
                    window.request_animation_frame();
                }
                player.frame()
            }
            None => None,
        };
        #[cfg(not(target_os = "macos"))]
        let _ = window;
        let mut screen = self.screen("screen", None);
        #[cfg(target_os = "macos")]
        let playing = frame.is_some();
        #[cfg(not(target_os = "macos"))]
        let playing = false;
        #[cfg(target_os = "macos")]
        if let Some(frame) = frame {
            screen = screen.child(
                gpui::surface(frame)
                    .object_fit(gpui::ObjectFit::Contain)
                    .size_full(),
            );
        }
        if !playing && let Some(image) = self.image(self.pictures.main.as_ref()) {
            screen = screen.child(img(image).object_fit(gpui::ObjectFit::Contain).size_full());
        }
        let has_main = self.pictures.main.is_some() || self.video().is_some();
        let mut column = div().flex().flex_col().gap(px(6.));
        if has_main {
            column = column.child(screen);
        }
        if cfg!(target_os = "macos") && self.video().is_some() {
            #[cfg(target_os = "macos")]
            let label = if self.player.as_ref().is_some_and(|p| p.playing()) {
                "Pause"
            } else {
                "Play"
            };
            #[cfg(not(target_os = "macos"))]
            let label = "Play";
            column = column.child(
                div()
                    .id(SharedString::from(format!("{}:play", self.id)))
                    .px(px(10.))
                    .py(px(4.))
                    .rounded_md()
                    .text_size(small)
                    .text_color(theme.text)
                    .bg(theme.surface)
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| this.toggle(cx)))
                    .child(label),
            );
        }
        if !self.pictures.strip.is_empty() {
            let mut strip = div().flex().flex_row().gap(px(4.)).overflow_hidden();
            for id in self.pictures.strip.clone() {
                strip = strip.child(
                    div()
                        .h(px(48.))
                        .w(px(85.))
                        .rounded(px(4.))
                        .overflow_hidden()
                        .bg(gpui::black().opacity(0.35))
                        .when_some(self.image(Some(&id)), |d, image| {
                            d.child(img(image).object_fit(gpui::ObjectFit::Cover).size_full())
                        }),
                );
            }
            column = column.child(strip);
        }
        column.into_any_element()
    }

    fn mount(
        &mut self,
        node: &Node<Intent>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::of(cx).clone();
        match &node.element {
            Element::Stack { axis, children } => {
                let mut d = div().flex().gap(px(6.));
                d = if *axis == Axis::Horizontal {
                    d.flex_row()
                } else {
                    d.flex_col()
                };
                for child in children {
                    d = d.child(self.mount(child, window, cx));
                }
                d.into_any_element()
            }
            Element::Text { value, role } => {
                let size = match role {
                    TextRole::Heading => 15.0,
                    TextRole::Status => 12.0,
                    _ => 14.0,
                };
                div()
                    .text_size(crate::typography::ui_rems(size))
                    .text_color(if *role == TextRole::Heading {
                        theme.text
                    } else {
                        theme.text_muted
                    })
                    .when(*role == TextRole::Heading, |d| {
                        d.font_weight(gpui::FontWeight::SEMIBOLD)
                    })
                    .child(value.clone())
                    .into_any_element()
            }
            Element::Button { label, enabled, .. } => {
                let key = node.key.clone();
                div()
                    .id(SharedString::from(key.clone()))
                    .px(px(10.))
                    .py(px(4.))
                    .rounded_md()
                    .text_size(crate::typography::ui_rems(12.0))
                    .text_color(theme.text)
                    .bg(theme.surface)
                    .when(*enabled && self.page.is_some(), |d| {
                        d.cursor_pointer().on_click(
                            cx.listener(move |this, _, _, cx| this.activate(key.clone(), cx)),
                        )
                    })
                    .child(label.clone())
                    .into_any_element()
            }
            Element::Surface { .. } => self.picture(window, cx),
            _ => div().into_any_element(),
        }
    }
}

impl Render for LabView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.released && !(self.current)() {
            self.release(cx);
        }
        let theme = Theme::of(cx).clone();
        let frame = div()
            .id(self.id.clone())
            .w_full()
            .p(px(12.))
            .rounded(px(Theme::CONTROL_RADIUS))
            .border_1()
            .border_color(theme.border);
        if self.released {
            return frame;
        }
        let root = self.projection.as_ref().map(|p| p.view.view().root.clone());
        let frame = match root {
            Some(root) => frame.child(self.mount(&root, window, cx)),
            None => frame.child(self.card.title.clone()),
        };
        frame.when_some(self.notes(window, cx), |d, notes| d.child(notes))
    }
}

impl LabView {
    /// The note control under the card: a button, or the note being written.
    fn notes(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.add_note.as_ref()?;
        let theme = Theme::of(cx).clone();
        let small = crate::typography::ui_rems(12.0);
        let button = |id: &str, label: String| {
            div()
                .id(SharedString::from(format!("{}:{id}", self.id)))
                .px(px(10.))
                .py(px(4.))
                .rounded_md()
                .text_size(small)
                .text_color(theme.text)
                .bg(theme.surface)
                .cursor_pointer()
                .child(label)
        };
        let Some(draft) = self.draft.as_mut() else {
            let label = match self.playing_at() {
                Some(at) => format!("Add a note at {}", lab_ui::clock(at)),
                None => "Add a note".to_owned(),
            };
            return Some(
                div()
                    .pt(px(8.))
                    .flex()
                    .child(
                        button("note", label)
                            .on_click(cx.listener(|this, _, _, cx| this.start_note(cx))),
                    )
                    .into_any_element(),
            );
        };
        if draft.focus {
            draft.focus = false;
            window.focus(&draft.input.focus_handle(cx), cx);
        }
        let input = draft.input.clone();
        let error = draft.error.clone();
        let saving = draft.saving;
        Some(
            div()
                .pt(px(8.))
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(6.))
                        .items_center()
                        .child(
                            div()
                                .flex_1()
                                .px(px(8.))
                                .py(px(4.))
                                .rounded_md()
                                .border_1()
                                .border_color(theme.border)
                                .child(input),
                        )
                        .child(
                            button("note-add", if saving { "Adding…" } else { "Add" }.into())
                                .on_click(cx.listener(|this, _, _, cx| this.submit_note(cx))),
                        )
                        .child(
                            button("note-cancel", "Cancel".into())
                                .on_click(cx.listener(|this, _, _, cx| this.cancel_note(cx))),
                        ),
                )
                .when_some(error, |d, error| {
                    d.child(
                        div()
                            .text_size(small)
                            .text_color(theme.text_muted)
                            .child(error),
                    )
                })
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// A 1x1 PNG.
    const PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8,
        0xcf, 0xc0, 0xf0, 0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    fn fixtures() -> Vec<(String, Card)> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../crates/lab-kit/fixtures/cards");
        let mut all: Vec<(String, Card)> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .map(|p| {
                let card = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
                (p.file_stem().unwrap().to_string_lossy().into_owned(), card)
            })
            .collect();
        all.sort_by(|a, b| a.0.cmp(&b.0));
        all
    }

    /// Images for every file, and a video link for a render.
    fn fetch() -> Fetch {
        Rc::new(|id: String, _: &mut App| {
            Task::ready(Some(
                if id.starts_with("art_render") || id == "art_side_by_side" {
                    Media::Video(format!("https://example.test/{id}.mp4"))
                } else {
                    Media::Image(PNG.to_vec())
                },
            ))
        })
    }

    #[gpui::test]
    fn every_fixture_card_draws_and_signing_out_clears_it(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| cx.set_global(Theme::default()));
        for (name, card) in fixtures() {
            let signed_in = Rc::new(Cell::new(true));
            let current: Current = {
                let signed_in = signed_in.clone();
                Rc::new(move || signed_in.get())
            };
            let (view, cx) = cx.add_window_view(|_, cx| {
                LabView::new(
                    SharedString::from(format!("lab-{name}")),
                    card.clone(),
                    "me",
                    card.page.clone(),
                    fetch(),
                    current,
                    cx,
                )
            });
            cx.run_until_parked();
            let expected = view.read_with(cx, |v, _| v.wanted().len());
            for size in [
                gpui::size(px(900.0), px(700.0)),
                gpui::size(px(360.0), px(700.0)),
            ] {
                cx.simulate_resize(size);
                cx.update(|window, cx| {
                    window.refresh();
                    window.draw(cx).clear();
                });
            }
            let loaded = view.read_with(cx, |v, _| v.loaded());
            assert_eq!(loaded, expected, "{name}");
            signed_in.set(false);
            cx.update(|window, cx| {
                window.refresh();
                window.draw(cx).clear();
            });
            view.read_with(cx, |v, _| {
                assert!(!v.showing(), "{name}");
                assert_eq!(v.loaded(), 0, "{name}");
            });
        }
    }

    #[gpui::test]
    fn a_newer_card_keeps_its_pictures_and_notes_can_be_added(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| cx.set_global(Theme::default()));
        let card = fixtures()
            .into_iter()
            .find(|(n, _)| n == "image")
            .unwrap()
            .1;
        let added: Rc<std::cell::RefCell<Vec<(Option<f64>, String)>>> = Rc::default();
        let (view, cx) = cx.add_window_view(|_, cx| {
            let mut view = LabView::new(
                SharedString::from("lab-image"),
                card.clone(),
                "me",
                None,
                fetch(),
                Rc::new(|| true),
                cx,
            );
            let added = added.clone();
            view.set_add_note(Rc::new(move |at, text, _| {
                added.borrow_mut().push((at, text));
                Task::ready(Ok(()))
            }));
            view
        });
        cx.run_until_parked();
        let loaded = view.read_with(cx, |v, _| v.loaded());
        assert!(loaded > 0);
        let mut newer = card.clone();
        newer.notes.push(lab_kit::NoteBrief {
            locator: lab_kit::Locator::Time { seconds: 3.0 },
            author: "me".into(),
            text: "Hold this".into(),
        });
        view.update(cx, |v, cx| v.set_card(newer.clone(), cx));
        view.read_with(cx, |v, _| {
            assert_eq!(v.card(), &newer);
            assert_eq!(v.loaded(), loaded, "the pictures stay");
        });
        view.update(cx, |v, cx| v.start_note(cx));
        cx.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear();
        });
        view.update(cx, |v, cx| {
            let input = v.draft.as_ref().unwrap().input.clone();
            input.update(cx, |input, cx| input.set_text("Warmer", cx));
            v.submit_note(cx);
        });
        cx.run_until_parked();
        assert_eq!(&*added.borrow(), &[(None, "Warmer".to_owned())]);
        view.read_with(cx, |v, _| assert!(v.draft.is_none()));
    }
}
