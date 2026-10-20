//! Browser state shared by the chrome and platform host. No native handles.

/// NSEvent supplies characters, while GPUI bindings name non-printing keys.
/// Shift+Tab can arrive as NSBackTabCharacter rather than a literal tab.
#[cfg(any(target_os = "macos", test))]
pub(super) fn native_shortcut_key(characters: &str) -> &str {
    match characters {
        "\t" | "\u{19}" => "tab",
        other => other,
    }
}

#[cfg(test)]
mod native_shortcut_tests {
    use super::*;

    #[test]
    fn native_tabs_match_the_configured_navigation_keystrokes() {
        for (characters, modifiers, binding) in [
            ("\t", "ctrl-", "ctrl-tab"),
            ("\t", "ctrl-shift-", "ctrl-shift-tab"),
            ("\u{19}", "ctrl-shift-", "ctrl-shift-tab"),
            ("\t", "alt-", "alt-tab"),
            ("l", "cmd-", "cmd-l"),
        ] {
            let native = format!("{modifiers}{}", native_shortcut_key(characters));
            assert_eq!(
                gpui::Keystroke::parse(&native).unwrap(),
                gpui::Keystroke::parse(binding).unwrap(),
            );
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
pub struct PageState {
    pub url: Option<String>,
    pub title: String,
    pub loading: bool,
    pub can_back: bool,
    pub can_forward: bool,
    pub error: Option<String>,
}

impl PageState {
    pub fn label(&self) -> String {
        if !self.title.trim().is_empty() {
            self.title.clone()
        } else {
            self.url
                .as_deref()
                .and_then(|s| url::Url::parse(s).ok())
                .and_then(|u| u.host_str().map(str::to_owned))
                .unwrap_or_else(|| "Browser".into())
        }
    }
}

pub use openagents_chat_view::web_address::{loopback, normalize_address, transcript_address};

pub fn allowed_navigation(address: &str) -> bool {
    url::Url::parse(address).is_ok_and(|u| {
        matches!(u.scheme(), "http" | "https")
            && u.host_str().is_some()
            && u.username().is_empty()
            && u.password().is_none()
    })
}

/// Subframe navigations: web content, including a frame's own documents
/// (`about:blank`, `about:srcdoc`, `data:`, `blob:`) — never a scheme that
/// leaves the page (`file:`, `javascript:`, app links).
pub fn allowed_frame_navigation(address: &str) -> bool {
    allowed_navigation(address)
        || url::Url::parse(address).is_ok_and(|u| matches!(u.scheme(), "about" | "data" | "blob"))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Presentation {
    #[default]
    Hidden,
    Live,
    /// Keep rendering while GPUI owns drag input.
    Passthrough,
}

pub fn presentation(active: bool, dragging: bool) -> Presentation {
    if !active {
        Presentation::Hidden
    } else if dragging {
        Presentation::Passthrough
    } else {
        Presentation::Live
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalizes_web_addresses_and_loopback_ports() {
        for (input, expected) in [
            ("localhost:3000", "http://localhost:3000/"),
            ("127.0.0.1:5173/a?x=1", "http://127.0.0.1:5173/a?x=1"),
            ("[::1]:8080", "http://[::1]:8080/"),
            ("[::1]", "http://[::1]/"),
            (" app.localhost:3000 ", "http://app.localhost:3000/"),
            ("example.com/path", "https://example.com/path"),
            ("https://localhost:3000", "https://localhost:3000/"),
            ("http://example.com", "http://example.com/"),
        ] {
            assert_eq!(normalize_address(input), Ok(expected.into()), "{input}");
        }
    }
    #[test]
    fn rejects_non_web_schemes_credentials_and_bad_input() {
        for input in [
            "",
            "javascript:alert(1)",
            "file:///tmp/a",
            "data:text/html,hi",
            "openagents://open/chat/a",
            "https://user:pass@example.com",
            "https://",
            "two words",
            "https://example.com/\nsecret",
        ] {
            assert!(normalize_address(input).is_err(), "{input}");
        }
        assert!(!allowed_navigation("javascript:alert(1)"));
        assert!(!allowed_navigation("https://user@example.com/"));
    }
    #[test]
    fn frames_load_web_content_but_never_leave_the_page() {
        for ok in [
            "https://example.com/embed",
            "about:blank",
            "about:srcdoc",
            "data:text/html,hi",
            "blob:https://example.com/0f3e",
        ] {
            assert!(allowed_frame_navigation(ok), "{ok}");
        }
        for blocked in [
            "openagents://open/chat/a",
            "ms-settings:privacy",
            "file:///C:/Windows/win.ini",
            "javascript:alert(1)",
            "mailto:a@example.com",
            "",
        ] {
            assert!(!allowed_frame_navigation(blocked), "{blocked}");
        }
    }
    #[test]
    fn native_visibility_never_outlives_its_surface() {
        assert_eq!(presentation(false, false), Presentation::Hidden);
        assert_eq!(presentation(false, true), Presentation::Hidden);
        assert_eq!(presentation(true, true), Presentation::Passthrough);
        assert_eq!(presentation(true, false), Presentation::Live);
    }
    #[test]
    fn tab_labels_fall_back_to_host_then_browser() {
        let mut page = PageState::default();
        assert_eq!(page.label(), "Browser");
        page.url = Some("http://localhost:3000/path".into());
        assert_eq!(page.label(), "localhost");
        page.title = "Local preview".into();
        assert_eq!(page.label(), "Local preview");
    }
}
