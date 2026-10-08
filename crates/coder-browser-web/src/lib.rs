//! The browser mount owns platform controls and rendering, never host authority.
//! Its native grant, key, terminal state, and input live only in page memory.

#[cfg(target_arch = "wasm32")]
mod gpu;
#[cfg(target_arch = "wasm32")]
mod mount;
#[cfg(target_arch = "wasm32")]
pub use mount::{start, terminal_receipt};

#[cfg(any(test, target_arch = "wasm32"))]
const INPUT_MAX: usize = 64 * 1024;

/// Browser IME may deliver both compositionend and its final input event.
/// Retain only the immediate duplicate marker, never an offline input queue.
#[derive(Default)]
#[cfg(any(test, target_arch = "wasm32"))]
struct Composition {
    active: bool,
    committed: Option<String>,
}

#[cfg(any(test, target_arch = "wasm32"))]
impl Composition {
    fn begin(&mut self) {
        self.active = true;
        self.committed = None;
    }

    fn end(&mut self, text: &str) -> bool {
        self.active = false;
        self.committed = (!text.is_empty() && text.len() <= INPUT_MAX).then(|| text.into());
        self.committed.is_some()
    }

    fn input(&mut self, text: &str, composing: bool, kind: &str) -> bool {
        if composing || self.active {
            return false;
        }
        let duplicate = self.committed.take().is_some_and(|old| {
            old == text
                && matches!(
                    kind,
                    "insertText" | "insertFromComposition" | "insertCompositionText"
                )
        });
        !duplicate && !text.is_empty() && text.len() <= INPUT_MAX
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composition_commits_once_and_later_input_is_not_replayed() {
        let mut input = Composition::default();
        input.begin();
        assert!(!input.input("日本語", true, "insertCompositionText"));
        assert!(input.end("日本語"));
        assert!(!input.input("日本語", false, "insertFromComposition"));
        assert!(input.input("日本語", false, "insertText"));
        assert!(input.committed.is_none());
    }

    #[test]
    fn composition_marker_is_bounded_and_new_composition_replaces_it() {
        let mut input = Composition::default();
        assert!(!input.end(&"x".repeat(INPUT_MAX + 1)));
        assert!(!input.input(&"x".repeat(INPUT_MAX + 1), false, "insertFromPaste"));
        assert!(input.end("a"));
        input.begin();
        assert!(input.committed.is_none());
        assert!(input.end("b"));
        assert!(!input.input("b", false, "insertText"));
        assert!(!input.input("", false, "insertText"));
    }
}
