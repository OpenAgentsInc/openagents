//! The update strip (#10023): when the updater has downloaded and checked a
//! new build, a small strip floats at the top right of the window saying
//! **Update ready** with a **Restart to update** button.
//!
//! The button carries the same intent as Settings' update button
//! (`chrome::Action::Update`), so it runs the same install: on Linux and
//! Windows [`crate::updates::act`] replaces the AppImage or hands the MSI to
//! Windows Installer and relaunches; on a Mac it installs the staged bundle
//! the menu-bar item's updater holds and relaunches
//! ([`crate::menubar::install_update`]).
//!
//! The strip uses the window's one floating layer, so it steps aside while
//! the palette, a chat's menu, or "Scroll to bottom" is up, and it is not
//! shown on Settings, which offers the update itself.

use openagents_desktop::chrome::{Action, Page};
use openagents_desktop::model::Intent;
use rust_native::style::{Color, Space, Style, TextWeight};
use rust_native::{Axis, Element, Node, TextRole};
use rust_native_desktop::{OverlayLayout, OverlayPlacement};

/// The strip's root key.
pub const KEY: &str = "update-strip";
/// The strip's width, in points.
const WIDTH: u16 = 272;
/// The strip's button key.
pub const BUTTON: &str = "update-strip-restart";

/// Whether the strip shows: an update is `ready` and the page is not
/// Settings.
pub fn shows(ready: Option<&str>, page: Option<Page>) -> bool {
    ready.is_some() && page != Some(Page::Settings)
}

/// The strip.
pub fn node() -> Node<Intent> {
    let mut line = Node {
        key: format!("{KEY}-line"),
        style: Style::default(),
        element: Element::Text {
            value: "Update ready".into(),
            role: TextRole::Body,
        },
    };
    line.style.text_size = Some(13);
    line.style.line_height = Some(18);
    line.style.foreground = Some(openagents_chat_app::visual::current().text);
    let mut button = Node {
        key: BUTTON.into(),
        style: Style::default(),
        element: Element::Button {
            shortcut: None,
            label: "Restart to update".into(),
            enabled: true,
            icon: None,
            intent: Intent::Navigate {
                action: Action::Update,
            },
        },
    };
    button.style.background = Some(openagents_chat_app::visual::current().text);
    button.style.foreground = Some(openagents_chat_app::visual::current().on_text);
    button.style.weight = Some(TextWeight::Normal);
    button.style.text_size = Some(13);
    button.style.line_height = Some(18);
    button.style.button_padding = Some([10, 4]);
    button.style.radius = Some(12);
    let mut strip = Node {
        key: KEY.into(),
        style: Style::default(),
        element: Element::Stack {
            axis: Axis::Horizontal,
            children: vec![line, button],
        },
    };
    strip.style.gap = Some(Space::Sm);
    strip.style.radius = Some(16);
    strip.style.border = Some(openagents_chat_app::visual::current().border);
    strip.style.padding_points = Some([4, 4, 4, 14]);
    strip.style.background = Some(openagents_chat_app::visual::current().raised);
    strip
}

/// Where the strip floats: under the title bar, at the right.
pub fn layout() -> OverlayLayout {
    OverlayLayout {
        width: WIDTH,
        placement: OverlayPlacement::TopRight { top: 46, right: 12 },
        scrim: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find<'a>(node: &'a Node<Intent>, key: &str) -> Option<&'a Node<Intent>> {
        if node.key == key {
            return Some(node);
        }
        match &node.element {
            Element::Stack { children, .. } => children.iter().find_map(|child| find(child, key)),
            _ => None,
        }
    }

    #[test]
    fn a_downloaded_update_shows_the_strip_and_its_button_restarts() {
        assert!(shows(Some("1.1.0"), Some(Page::Chat(0))));
        assert!(shows(Some("1.1.0"), Some(Page::Grid)));
        assert!(shows(Some("1.1.0"), None));
        assert!(!shows(None, Some(Page::Chat(0))));
        // Settings offers it already.
        assert!(!shows(Some("1.1.0"), Some(Page::Settings)));
        let strip = node();
        let Some(Node {
            element: Element::Text { value, .. },
            ..
        }) = find(&strip, "update-strip-line")
        else {
            panic!("no line")
        };
        assert_eq!(value, "Update ready");
        let Some(Node {
            element:
                Element::Button {
                    label,
                    intent,
                    enabled: true,
                    ..
                },
            ..
        }) = find(&strip, BUTTON)
        else {
            panic!("no button")
        };
        assert_eq!(label, "Restart to update");
        // The same intent as Settings' update button, which runs the install.
        assert_eq!(
            *intent,
            Intent::Navigate {
                action: Action::Update
            }
        );
        for words in ["Update ready", "Restart to update"] {
            assert!(openagents_desktop::words::banned_in(words).is_empty());
        }
    }
}
