//! What a chip looks like: its pill, icon and hover preview. The drawing is
//! `openagents_chat_view::composer_chips`, shared with the transcript.
pub use openagents_chat_view::composer_chips::*;

/// A staged attachment an attachment chip can refer to; an image chip opens
/// its picture on click.
pub(super) struct ChipAttachment {
    pub(super) image: Option<std::sync::Arc<gpui::Image>>,
}
