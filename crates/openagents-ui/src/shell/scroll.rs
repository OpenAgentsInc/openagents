//! The floating scroll-to-bottom button over a scrolling thread.

use maud::{Markup, Render, html};

use crate::icons::{Icon, IconSize};

/// The data attribute `scroll-to-bottom.js` binds the button by; its value
/// is a CSS selector for the scrolling region it follows.
pub const SCROLL_TO_BOTTOM_ATTR: &str = "data-oa-scroll-bottom";

/// The data attribute of an optional link inside the scrolling region that
/// loads the newest messages (a transcript window that is not at its tail).
/// When one is present, the button clicks it instead of scrolling.
pub const SCROLL_TAIL_ATTR: &str = "data-oa-scroll-tail";

/// The data attribute that makes the button's region follow its newest
/// line: it opens scrolled to the end, and content swapped in while the
/// reader is near the end keeps the end in view.
pub const SCROLL_FOLLOW_ATTR: &str = "data-oa-scroll-follow";

/// An icon-only round button, centered at the bottom of its positioned
/// parent (just above a docked composer), that scrolls `target` to its end.
///
/// It renders `hidden`; `scroll-to-bottom.js` shows it only while the
/// region overflows and the reader is more than about a third of a screen
/// above the end (an `IntersectionObserver` on a sentinel at the region's
/// end), so short threads never show it. A click smooth-scrolls to the end,
/// or follows a [`SCROLL_TAIL_ATTR`] link when the newest messages are not
/// loaded. Without JavaScript it stays hidden.
#[derive(Clone, Debug)]
pub struct ScrollToBottom {
    target: String,
    follow: bool,
}

impl ScrollToBottom {
    /// A button for the scrolling region `target` (a CSS selector such as
    /// `#chat-thread`).
    #[must_use]
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            target: target.into(),
            follow: false,
        }
    }

    /// Keep the region on its newest line ([`SCROLL_FOLLOW_ATTR`]), for a
    /// live log that grows while it is read.
    #[must_use]
    pub fn follow(mut self) -> Self {
        self.follow = true;
        self
    }
}

impl Render for ScrollToBottom {
    fn render(&self) -> Markup {
        html! {
            button type="button" class="oa-scroll-bottom" data-oa-scroll-bottom=(self.target)
                data-oa-scroll-follow=[self.follow.then_some("")]
                aria-label="Scroll to bottom" title="Scroll to bottom" hidden {
                (Icon::ArrowDown.size(IconSize::Md))
            }
        }
    }
}
