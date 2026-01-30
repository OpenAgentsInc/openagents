mod buffer;
mod caret;
mod display_map;
mod editor;
mod element;
mod gpui_compat;
mod scroll;
mod syntax;
mod view;

pub use buffer::TextBuffer;
pub use caret::{Caret, Position, SelectionRange};
pub use display_map::{DisplayLine, DisplayMap};
pub use editor::{Editor, EditorSnapshot};
pub use element::EditorElement;
pub use gpui_compat::*;
pub use scroll::ScrollState;
pub use syntax::{HighlightSpan, SyntaxHighlighter, SyntaxLanguage};
pub use view::EditorView;

#[cfg(test)]
mod tests;
