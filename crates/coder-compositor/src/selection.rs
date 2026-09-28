//! What the seat carries between clients: the clipboard, the primary
//! selection, and a drag in progress.
//!
//! Smithay moves the bytes. The devices, the offers, the pipes, and the
//! drag grab are its `wl_data_device` and `zwp_primary_selection`
//! implementations, and this module is the compositor's own record of what
//! they are carrying, so the log and a later desk request can answer what
//! the clipboard holds without reaching into a client.

use smithay::wayland::selection::SelectionTarget;

/// Whether a drag is in progress, and what it carries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Drag {
    /// No drag is in progress.
    #[default]
    Idle,
    /// A client started a drag and holds the pointer until it drops.
    Holding {
        /// The types the source offers, in the order it offered them.
        mimes: Vec<String>,
        /// Whether the client gave the drag a surface to draw under the
        /// pointer, which the compositor draws there until the drop.
        icon: bool,
    },
}

/// The clipboard, the primary selection, and the drag, as the compositor
/// last saw them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    clipboard: Vec<String>,
    primary: Vec<String>,
    drag: Drag,
}

impl Selection {
    /// Records that a client took one selection, with the types it offers.
    /// An empty list of types is a client clearing the selection.
    pub fn set(&mut self, target: SelectionTarget, mimes: Vec<String>) {
        match target {
            SelectionTarget::Clipboard => self.clipboard = mimes,
            SelectionTarget::Primary => self.primary = mimes,
        }
    }

    /// The types one selection offers.
    pub fn mimes(&self, target: SelectionTarget) -> &[String] {
        match target {
            SelectionTarget::Clipboard => &self.clipboard,
            SelectionTarget::Primary => &self.primary,
        }
    }

    /// Records that a drag started.
    pub fn start_drag(&mut self, mimes: Vec<String>, icon: bool) {
        self.drag = Drag::Holding { mimes, icon };
    }

    /// Records that a drag ended, whether the target took it or not.
    pub fn end_drag(&mut self) {
        self.drag = Drag::Idle;
    }

    /// The drag in progress.
    pub fn drag(&self) -> &Drag {
        &self.drag
    }

    /// Whether a drag is in progress.
    pub fn dragging(&self) -> bool {
        self.drag != Drag::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mimes(list: &[&str]) -> Vec<String> {
        list.iter().map(|mime| (*mime).to_string()).collect()
    }

    #[test]
    fn a_fresh_seat_carries_nothing() {
        let selection = Selection::default();
        assert!(selection.mimes(SelectionTarget::Clipboard).is_empty());
        assert!(selection.mimes(SelectionTarget::Primary).is_empty());
        assert!(!selection.dragging());
    }

    #[test]
    fn the_clipboard_and_the_primary_selection_are_two_slots() {
        let mut selection = Selection::default();
        selection.set(SelectionTarget::Clipboard, mimes(&["text/plain"]));
        selection.set(SelectionTarget::Primary, mimes(&["image/png"]));
        assert_eq!(
            selection.mimes(SelectionTarget::Clipboard),
            mimes(&["text/plain"])
        );
        assert_eq!(
            selection.mimes(SelectionTarget::Primary),
            mimes(&["image/png"])
        );
    }

    #[test]
    fn the_last_client_to_copy_holds_the_clipboard() {
        let mut selection = Selection::default();
        selection.set(SelectionTarget::Clipboard, mimes(&["text/plain"]));
        selection.set(SelectionTarget::Clipboard, mimes(&["text/html"]));
        assert_eq!(
            selection.mimes(SelectionTarget::Clipboard),
            mimes(&["text/html"])
        );
    }

    #[test]
    fn a_client_that_clears_the_clipboard_leaves_the_primary_selection() {
        let mut selection = Selection::default();
        selection.set(SelectionTarget::Clipboard, mimes(&["text/plain"]));
        selection.set(SelectionTarget::Primary, mimes(&["text/plain"]));
        selection.set(SelectionTarget::Clipboard, Vec::new());
        assert!(selection.mimes(SelectionTarget::Clipboard).is_empty());
        assert_eq!(
            selection.mimes(SelectionTarget::Primary),
            mimes(&["text/plain"])
        );
    }

    #[test]
    fn a_drag_runs_from_the_grab_to_the_drop() {
        let mut selection = Selection::default();
        assert_eq!(selection.drag(), &Drag::Idle);
        selection.start_drag(mimes(&["text/uri-list"]), true);
        assert!(selection.dragging());
        assert_eq!(
            selection.drag(),
            &Drag::Holding {
                mimes: mimes(&["text/uri-list"]),
                icon: true,
            }
        );
        selection.end_drag();
        assert!(!selection.dragging());
    }

    #[test]
    fn a_drag_leaves_the_clipboard_alone() {
        let mut selection = Selection::default();
        selection.set(SelectionTarget::Clipboard, mimes(&["text/plain"]));
        selection.start_drag(mimes(&["text/uri-list"]), false);
        selection.end_drag();
        assert_eq!(
            selection.mimes(SelectionTarget::Clipboard),
            mimes(&["text/plain"])
        );
    }
}
