//! The desktop's transcript on the web: `openagents_chat_view::transcript`
//! drawing one chat that a web app holds.
//!
//! The app keeps the open chat's entries in a [`WebTranscript`] entity and
//! hands the view a [`WebHost`] over it. [`WebMedia`] draws the images the
//! app has already loaded; the desktop's diagrams, tool-call previews, and
//! full-size viewer aren't on the web yet.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, Image, IntoElement, ObjectFit,
    ParentElement, SharedString, Styled, Subscription, Window, div, prelude::*, px,
};
use openagents_chat_view::attachments::UserImageAttachment;
use openagents_chat_view::composer_mentions::SentMentionSpan;
use openagents_chat_view::diff::{DiffHighlights, FileDiff, LineKind};
use openagents_chat_view::markdown::render;
use openagents_chat_view::rpc::Rpc;
use openagents_chat_view::theme::Theme;
use openagents_chat_view::tool_progress::{self, ToolProgress};
use openagents_chat_view::transcript::host::{ChatPlace, TranscriptHost, TurnActivity};
use openagents_chat_view::transcript::{
    ChipOverlay, FoldState, HostLayout, PreparedTranscript, Row, ToolItem, TranscriptMedia,
    TranscriptView,
};
use openagents_chat_view::workspace_links::FileLinkRoot;
use openagents_doc::{SessionMessageEntry, TranscriptBaseline};

#[cfg(not(target_family = "wasm"))]
use gpui::img;
#[cfg(target_family = "wasm")]
use openagents_gpui_web::img;

/// The transcript as a web app draws it.
pub type Transcript = TranscriptView<WebMedia>;

/// The open chat as the app holds it.
#[derive(Default)]
pub struct WebTranscript {
    chat: Option<String>,
    entries: Vec<SessionMessageEntry>,
    revision: u64,
    /// The open chat's working turn, while it has one.
    activity: Option<TurnActivity>,
}

impl WebTranscript {
    /// Show `chat` with `entries`.
    pub fn open(&mut self, chat: Option<String>, entries: Vec<SessionMessageEntry>) {
        self.chat = chat;
        self.entries = entries;
        self.revision += 1;
    }

    /// Replace the open chat's entries.
    pub fn set_entries(&mut self, entries: Vec<SessionMessageEntry>) {
        self.entries = entries;
        self.revision += 1;
    }

    pub fn set_activity(&mut self, activity: Option<TurnActivity>) {
        self.activity = activity;
    }
}

/// The transcript's host over a [`WebTranscript`].
#[derive(Clone)]
pub struct WebHost(pub Entity<WebTranscript>);

impl WebHost {
    pub fn shared(state: &Entity<WebTranscript>) -> Rc<dyn TranscriptHost> {
        Rc::new(Self(state.clone()))
    }
}

impl TranscriptHost for WebHost {
    fn selected_chat<'a>(&self, cx: &'a App) -> Option<&'a str> {
        self.0.read(cx).chat.as_deref()
    }

    fn transcript_replayed(&self, _cx: &App) -> bool {
        true
    }

    fn transcript_revision(&self, cx: &App) -> u64 {
        self.0.read(cx).revision
    }

    fn entries<'a>(&self, doc: Option<&str>, cx: &'a App) -> &'a [SessionMessageEntry] {
        match doc {
            Some(_) => &[],
            None => &self.0.read(cx).entries,
        }
    }

    fn pending_echoes<'a>(&self, _cx: &'a App) -> &'a [SessionMessageEntry] {
        &[]
    }

    fn prepared<'a>(&self, _chat_id: &str, _cx: &'a App) -> Option<&'a Arc<PreparedTranscript>> {
        None
    }

    fn baseline<'a>(&self, _chat_id: &str, _cx: &'a App) -> Option<&'a Arc<TranscriptBaseline>> {
        None
    }

    fn chat_is_local(&self, _chat_id: &str, _cx: &App) -> bool {
        false
    }

    fn link_roots_revision(&self, _cx: &App) -> u64 {
        0
    }

    fn file_link_roots(&self, _chat_id: &str, _cx: &App) -> Vec<FileLinkRoot> {
        Vec::new()
    }

    fn chat_place(&self, _chat_id: Option<&str>, _cx: &App) -> Option<ChatPlace> {
        None
    }

    fn local_device_id<'a>(&self, _cx: &'a App) -> Option<&'a str> {
        None
    }

    fn transfer_percent(&self, _upload_id: &str, _cx: &App) -> Option<u8> {
        None
    }

    fn upload_progress_percent(&self, _cx: &App) -> Option<u8> {
        None
    }

    fn send_undelivered(&self, _chat_id: &str, _now: DateTime<Utc>, _cx: &App) -> bool {
        false
    }

    fn retry_send(&self, _chat_id: &str, _cx: &mut App) {}

    fn turn_activity(&self, chat_id: &str, _now: DateTime<Utc>, cx: &App) -> Option<TurnActivity> {
        let state = self.0.read(cx);
        (state.chat.as_deref() == Some(chat_id))
            .then_some(state.activity)
            .flatten()
    }

    fn running_tool(&self, now: DateTime<Utc>, cx: &App) -> Option<ToolProgress> {
        tool_progress::running(&self.0.read(cx).entries, &[], now)
    }

    fn rpc(&self, _cx: &App) -> Option<Rpc> {
        None
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn watch(
        &self,
        changed: Rc<dyn Fn(Option<&str>, &mut App)>,
        cx: &mut App,
    ) -> Vec<Subscription> {
        vec![cx.observe(&self.0, move |_, cx| changed(None, cx))]
    }
}

/// Images a web app has loaded, by the path a message names.
pub type Images = Rc<RefCell<HashMap<String, Arc<Image>>>>;

/// What an app draws under an entry, by the entry's id.
pub type AfterEntry = Rc<dyn Fn(&str, &mut Window, &mut App) -> Option<AnyElement>>;

/// A web transcript's media: the app's loaded images, pictures the site
/// serves by path, and the app's own content under an entry.
#[derive(Default)]
pub struct WebMedia {
    pub images: Images,
    /// The site an image's path is on, when the app loads images by URL.
    pub origin: Option<String>,
    pub after_entry: Option<AfterEntry>,
}

impl WebMedia {
    /// A picture at `path`: loaded already, or read from the site.
    fn picture(&self, path: &str, name: &str) -> Option<AnyElement> {
        if let Some(image) = self.images.borrow().get(path).cloned() {
            return Some(image_tile(image));
        }
        let origin = self.origin.as_ref()?;
        let name = SharedString::from(name.to_owned());
        Some(
            div()
                .rounded(px(10.0))
                .overflow_hidden()
                .h(px(THUMB_HEIGHT))
                .max_w(px(THUMB_MAX_WIDTH))
                .child(
                    img(gpui::SharedUri::from(format!("{origin}{path}")))
                        .h(px(THUMB_HEIGHT))
                        .max_w(px(THUMB_MAX_WIDTH))
                        .object_fit(ObjectFit::Cover)
                        .with_fallback(move || {
                            div()
                                .p(px(8.0))
                                .text_size(px(12.0))
                                .child(name.clone())
                                .into_any_element()
                        }),
                )
                .into_any_element(),
        )
    }
}

const THUMB_HEIGHT: f32 = 160.0;
const THUMB_MAX_WIDTH: f32 = 320.0;

fn image_tile(image: Arc<Image>) -> AnyElement {
    div()
        .rounded(px(10.0))
        .overflow_hidden()
        .h(px(THUMB_HEIGHT))
        .max_w(px(THUMB_MAX_WIDTH))
        .child(
            img(image)
                .h(px(THUMB_HEIGHT))
                .max_w(px(THUMB_MAX_WIDTH))
                .object_fit(ObjectFit::Contain),
        )
        .into_any_element()
}

impl TranscriptMedia for WebMedia {
    type ToolImages = ();

    fn new(_cx: &mut Context<Transcript>) -> Self {
        Self::default()
    }

    fn released(_this: &mut Transcript, _cx: &mut App) {}

    fn chat_changed(_this: &mut Transcript, _cx: &mut Context<Transcript>) {}

    fn rows_built(_this: &mut Transcript, _rows: &[Row]) {}

    fn rows_changed(_this: &mut Transcript, _cx: &mut Context<Transcript>) {}

    fn begin_frame(
        _this: &mut Transcript,
        _column: f32,
        _window: &mut Window,
        _cx: &mut Context<Transcript>,
    ) {
    }

    fn after_row(_this: &mut Transcript, _cx: &mut Context<Transcript>) {}

    fn scrolled_near_top(_this: &mut Transcript, _cx: &mut Context<Transcript>) {}

    fn user_attachments(
        this: &mut Transcript,
        _row_id: &SharedString,
        attachments: &[UserImageAttachment],
        _window: &Window,
        _cx: &mut Context<Transcript>,
    ) -> AnyElement {
        let tiles: Vec<AnyElement> = attachments
            .iter()
            .filter_map(|attachment| this.media.picture(&attachment.path, &attachment.name))
            .collect();
        div()
            .w_full()
            .flex()
            .flex_row()
            .flex_wrap()
            .justify_end()
            .gap(px(8.0))
            .pb(px(8.0))
            .children(tiles)
            .into_any_element()
    }

    fn sent_chip_overlays(
        _this: &mut Transcript,
        _row_id: &SharedString,
        _mentions: &[SentMentionSpan],
        _theme: &Theme,
        _cx: &mut Context<Transcript>,
    ) -> Vec<ChipOverlay> {
        Vec::new()
    }

    fn open_sent_chip_image(
        _this: &mut Transcript,
        _path: &SharedString,
        _window: &mut Window,
        _cx: &mut Context<Transcript>,
    ) {
    }

    fn generated_image(
        this: &mut Transcript,
        _row_id: &SharedString,
        _owner: &str,
        path: &str,
        name: &str,
        _mime_type: &str,
        _cx: &mut Context<Transcript>,
    ) -> AnyElement {
        this.media
            .picture(path, name)
            .unwrap_or_else(|| div().into_any_element())
    }

    fn visual(
        _this: &mut Transcript,
        _row_id: &SharedString,
        _owner: &str,
        _path: &str,
        _title: &str,
        _cx: &mut Context<Transcript>,
    ) -> AnyElement {
        div().into_any_element()
    }

    fn markdown_media(
        _this: &mut Transcript,
        _cx: &Context<Transcript>,
    ) -> Option<render::MediaUi> {
        None
    }

    fn release_tool_images(
        _this: &mut Transcript,
        _row_id: &SharedString,
        _cx: &mut Context<Transcript>,
    ) {
    }

    fn hold_tool_images(
        _this: &mut Transcript,
        _row_id: &SharedString,
        tools: &[ToolItem],
        _opens: &[bool],
        _folds: &[FoldState],
        _cx: &mut Context<Transcript>,
    ) -> Vec<Option<()>> {
        vec![None; tools.len()]
    }

    fn tool_image_strip(
        _this: &Transcript,
        _chip: &SharedString,
        _images: (),
        _collapses: bool,
        _theme: &Theme,
        _cx: &mut Context<Transcript>,
    ) -> AnyElement {
        div().into_any_element()
    }

    fn overlays(
        _this: &mut Transcript,
        _window: &mut Window,
        _cx: &mut Context<Transcript>,
    ) -> Vec<AnyElement> {
        Vec::new()
    }

    fn after_entry(
        this: &mut Transcript,
        entry_id: &SharedString,
        window: &mut Window,
        cx: &mut Context<Transcript>,
    ) -> Option<AnyElement> {
        let after = this.media.after_entry.clone()?;
        after(entry_id, window, cx)
    }

    /// The lines of a diff, colored by kind, without the desktop's syntax
    /// highlighting.
    fn diff_body(
        file: &FileDiff,
        _highlights: Option<Arc<DiffHighlights>>,
        theme: &Theme,
    ) -> AnyElement {
        let line = |kind: LineKind, text: &str| {
            let (marker, bg) = match kind {
                LineKind::Add => ("+", Some(theme.success.opacity(0.12))),
                LineKind::Del => ("-", Some(theme.danger.opacity(0.12))),
                _ => (" ", None),
            };
            div()
                .w_full()
                .px(px(12.0))
                .font_family(theme.font_mono.clone())
                .text_size(px(12.0))
                .line_height(px(openagents_chat_view::diff::DIFF_LINE_HEIGHT))
                .text_color(theme.text)
                .whitespace_nowrap()
                .overflow_hidden()
                .when_some(bg, |row, bg| row.bg(bg))
                .child(SharedString::from(format!("{marker} {text}")))
        };
        div()
            .flex()
            .flex_col()
            .pb(px(openagents_chat_view::diff::BODY_BOTTOM_PAD))
            .children(file.hunks.iter().flat_map(|hunk| {
                std::iter::once(line(LineKind::Meta, &hunk.header))
                    .chain(hunk.lines.iter().map(|l| line(l.kind, &l.text)))
            }))
            .into_any_element()
    }
}

/// What changes an entry's rows: its parts, their text, and its status.
/// Apps compare it between frames to hand the transcript only changes.
pub fn shape(entries: &[SessionMessageEntry]) -> Vec<(usize, usize, u8)> {
    entries
        .iter()
        .map(|entry| {
            let text = entry
                .parts
                .iter()
                .map(|part| match part {
                    openagents_doc::MessagePart::Text { text, .. }
                    | openagents_doc::MessagePart::Reasoning { text, .. } => text.len(),
                    openagents_doc::MessagePart::Tool {
                        resolved, output, ..
                    } => usize::from(*resolved) + output.as_ref().map_or(0, String::len),
                    _ => 1,
                })
                .sum();
            (
                entry.parts.len(),
                text,
                entry.status.map_or(0, |s| s as u8 + 1),
            )
        })
        .collect()
}

/// Hand `entries` to `transcript` as `chat`, drawn from this frame on.
pub fn show(
    state: &Entity<WebTranscript>,
    transcript: &Entity<Transcript>,
    chat: Option<String>,
    entries: Vec<SessionMessageEntry>,
    activity: Option<TurnActivity>,
    cx: &mut App,
) {
    state.update(cx, |state, cx| {
        if state.chat != chat {
            state.open(chat, entries);
        } else {
            state.set_entries(entries);
        }
        state.set_activity(activity);
        cx.notify();
    });
    transcript.update(cx, |transcript, cx| transcript.host_changed(None, cx));
}

/// How a web page lays the transcript out for a `width` px wide view:
/// the column under the page's own header, narrower gutters on a phone,
/// and `fit_content` for a card that grows with its chat.
pub fn page_layout(width: f32, fit_content: bool) -> HostLayout {
    HostLayout {
        top: 16.0,
        bottom: 16.0,
        side: if width < 600.0 { 16.0 } else { 48.0 },
        fit_content,
    }
}

/// Lay `transcript` out for a `width` px wide view; a no-op while that
/// holds.
pub fn fit(transcript: &Entity<Transcript>, width: f32, fit_content: bool, cx: &mut App) {
    let layout = page_layout(width, fit_content);
    transcript.update(cx, |view, cx| view.set_host_layout(Some(layout), cx));
}

/// A transcript over `state`, drawing `images`.
pub fn new(state: &Entity<WebTranscript>, images: Images, cx: &mut App) -> Entity<Transcript> {
    cx.new(|cx| {
        let mut view = Transcript::new(WebHost::shared(state), cx);
        view.media.images = images;
        view
    })
}
