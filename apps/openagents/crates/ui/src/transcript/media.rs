//! Pictures and other media in a transcript: attachment tiles, generated
//! images, inline visuals, diagrams, tool-call image previews, diff bodies,
//! and the viewer that opens them full size. Each app supplies its own, so a
//! transcript draws text and structure the same everywhere while the media
//! come from wherever that app can read them.

use std::sync::Arc;

use gpui::{AnyElement, App, Context, SharedString, Window};

use super::{ChipOverlay, FoldState, Row, ToolItem, TranscriptView};
use crate::attachments::UserImageAttachment;
use crate::composer_mentions::SentMentionSpan;
use crate::diff::{DiffHighlights, FileDiff};
use crate::markdown::render;
use crate::theme::Theme;

/// A transcript's media. Every function receives the transcript it draws
/// for; the media's own state is [`TranscriptView::media`].
pub trait TranscriptMedia: Sized + 'static {
    /// What a tool chip's image previews hold while it is open.
    type ToolImages: Clone;

    fn new(cx: &mut Context<TranscriptView<Self>>) -> Self;
    /// The transcript is going away.
    fn released(this: &mut TranscriptView<Self>, cx: &mut App);
    /// The transcript switched to another chat.
    fn chat_changed(this: &mut TranscriptView<Self>, cx: &mut Context<TranscriptView<Self>>);
    /// The rows were rebuilt as `rows`, before they replace the old ones.
    fn rows_built(this: &mut TranscriptView<Self>, rows: &[Row]);
    /// The new rows are in place.
    fn rows_changed(this: &mut TranscriptView<Self>, cx: &mut Context<TranscriptView<Self>>);
    /// A frame starts, with the reading column `column` wide.
    fn begin_frame(
        this: &mut TranscriptView<Self>,
        column: f32,
        window: &mut Window,
        cx: &mut Context<TranscriptView<Self>>,
    );
    /// One row was drawn.
    fn after_row(this: &mut TranscriptView<Self>, cx: &mut Context<TranscriptView<Self>>);
    /// The reader scrolled near the top.
    fn scrolled_near_top(this: &mut TranscriptView<Self>, cx: &mut Context<TranscriptView<Self>>);

    /// The tiles above a sent message.
    fn user_attachments(
        this: &mut TranscriptView<Self>,
        row_id: &SharedString,
        attachments: &[UserImageAttachment],
        window: &Window,
        cx: &mut Context<TranscriptView<Self>>,
    ) -> AnyElement;
    /// What a sent message's attachment chips show beyond their labels.
    fn sent_chip_overlays(
        this: &mut TranscriptView<Self>,
        row_id: &SharedString,
        mentions: &[SentMentionSpan],
        theme: &Theme,
        cx: &mut Context<TranscriptView<Self>>,
    ) -> Vec<ChipOverlay>;
    /// Open a sent image chip's picture full size.
    fn open_sent_chip_image(
        this: &mut TranscriptView<Self>,
        path: &SharedString,
        window: &mut Window,
        cx: &mut Context<TranscriptView<Self>>,
    );
    /// An image the agent made.
    #[allow(clippy::too_many_arguments)]
    fn generated_image(
        this: &mut TranscriptView<Self>,
        row_id: &SharedString,
        owner: &str,
        path: &str,
        name: &str,
        mime_type: &str,
        cx: &mut Context<TranscriptView<Self>>,
    ) -> AnyElement;
    /// A visual the agent showed.
    fn visual(
        this: &mut TranscriptView<Self>,
        row_id: &SharedString,
        owner: &str,
        path: &str,
        title: &str,
        cx: &mut Context<TranscriptView<Self>>,
    ) -> AnyElement;
    /// How settled Markdown draws its diagrams, when it can.
    fn markdown_media(
        this: &mut TranscriptView<Self>,
        cx: &Context<TranscriptView<Self>>,
    ) -> Option<render::MediaUi>;
    /// A tool group closed: let its chips' previews go.
    fn release_tool_images(
        this: &mut TranscriptView<Self>,
        row_id: &SharedString,
        cx: &mut Context<TranscriptView<Self>>,
    );
    /// Hold the previews of each open chip in a tool group.
    fn hold_tool_images(
        this: &mut TranscriptView<Self>,
        row_id: &SharedString,
        tools: &[ToolItem],
        opens: &[bool],
        folds: &[FoldState],
        cx: &mut Context<TranscriptView<Self>>,
    ) -> Vec<Option<Self::ToolImages>>;
    /// One chip's image previews.
    fn tool_image_strip(
        this: &TranscriptView<Self>,
        chip: &SharedString,
        images: Self::ToolImages,
        collapses: bool,
        theme: &Theme,
        cx: &mut Context<TranscriptView<Self>>,
    ) -> AnyElement;
    /// Menus and viewers drawn over the whole transcript.
    fn overlays(
        this: &mut TranscriptView<Self>,
        window: &mut Window,
        cx: &mut Context<TranscriptView<Self>>,
    ) -> Vec<AnyElement>;
    /// A file diff's lines inside a tool chip.
    fn diff_body(
        file: &FileDiff,
        highlights: Option<Arc<DiffHighlights>>,
        theme: &Theme,
    ) -> AnyElement;
}
