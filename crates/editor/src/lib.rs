mod buffer;
mod caret;
mod editor;
mod view;

pub use buffer::TextBuffer;
pub use caret::{Caret, Position, SelectionRange};
pub use editor::{Editor, EditorSnapshot};
pub use view::EditorView;

#[cfg(test)]
mod tests;
