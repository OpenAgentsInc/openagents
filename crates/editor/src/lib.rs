mod buffer;
mod caret;
mod editor;
mod syntax;
mod view;

pub use buffer::TextBuffer;
pub use caret::{Caret, Position, SelectionRange};
pub use editor::{Editor, EditorSnapshot};
pub use syntax::{HighlightSpan, SyntaxHighlighter, SyntaxLanguage};
pub use view::EditorView;

#[cfg(test)]
mod tests;
