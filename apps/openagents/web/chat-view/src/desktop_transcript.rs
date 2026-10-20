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
    ChipOverlay, FoldState, PreparedTranscript, Row, ToolItem, TranscriptMedia, TranscriptView,
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

/// A web transcript's media: the app's loaded images.
#[derive(Default)]
pub struct WebMedia {
    pub images: Images,
    /// Bundled demo files or authenticated attachment reads supplied by the app.
    pub visual_reader: Option<crate::visuals::Read>,
    pub(crate) visuals: crate::visuals::Frames,
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

    fn chat_changed(this: &mut Transcript, _cx: &mut Context<Transcript>) {
        this.media.visuals.clear();
    }

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
        let images = this.media.images.borrow();
        div()
            .w_full()
            .flex()
            .flex_row()
            .flex_wrap()
            .justify_end()
            .gap(px(8.0))
            .pb(px(8.0))
            .children(
                attachments
                    .iter()
                    .filter_map(|attachment| images.get(&attachment.path).cloned())
                    .map(image_tile),
            )
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
        _name: &str,
        _mime_type: &str,
        _cx: &mut Context<Transcript>,
    ) -> AnyElement {
        match this.media.images.borrow().get(path).cloned() {
            Some(image) => image_tile(image),
            None => div().into_any_element(),
        }
    }

    fn visual(
        this: &mut Transcript,
        row_id: &SharedString,
        _owner: &str,
        path: &str,
        title: &str,
        cx: &mut Context<Transcript>,
    ) -> AnyElement {
        let Some(read) = this.media.visual_reader.clone() else {
            return div().into_any_element();
        };
        let row = row_id.clone();
        this.media.visuals.render(
            row_id.to_string(),
            title,
            path,
            read,
            move |this: &mut Transcript, cx| {
                this.diagram_layout_changed(std::slice::from_ref(&row), cx);
            },
            cx,
        )
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

/// A transcript over `state`, drawing `images`.
pub fn new(state: &Entity<WebTranscript>, images: Images, cx: &mut App) -> Entity<Transcript> {
    cx.new(|cx| {
        let mut view = Transcript::new(WebHost::shared(state), cx);
        view.media.images = images;
        view
    })
}
