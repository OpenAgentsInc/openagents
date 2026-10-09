//! The app shell and composer shared by every web view (UI-06).
//!
//! The ChatGPT UI reference (`docs/web/chatgpt-ui-reference.md`, "Shared
//! shell") shows the same chrome on every view: a layout with a left panel
//! (conversation sidebar and navigation), a main content surface with a frame,
//! a scrolling viewport and a top fade, and one composer (body, input,
//! attachments, footer with dropdown labels and a model picker trigger). This
//! module builds that chrome as typed Maud builders:
//!
//! - [`Document`]: the `<html>` element with the server-chosen `data-theme`.
//! - [`AppShell`]: layout, [`Sidebar`] in the left panel (one toggle in its
//!   header, a [`ChatList`] of recent chats, [`LegalLinks`] at the bottom),
//!   header, main frame, and an optional docked composer.
//! - [`Composer`]: a plain `<form method="post">` enhanced with HTMX, with
//!   slots for the repository, branch and environment selectors
//!   ([`ComposerDropdown`]) and a [`ModelPickerTrigger`].
//! - [`ComposerAction`], [`ComposerPanel`]: footer icon buttons and the
//!   panels the selectors load above the composer.
//! - [`Message`]: one turn of a conversation thread.
//! - [`ThemeToggle`]: the button the theme script binds through
//!   [`THEME_TOGGLE_ATTR`].
//!
//! Styles live in `static/components/shell.css` ([`SHELL_CSS`]) and
//! `static/components/composer.css` ([`COMPOSER_CSS`]).

mod composer;
mod glyph;
mod layout;
mod theme;
mod thread;

pub use composer::{
    Composer, ComposerAction, ComposerDropdown, ComposerPanel, HxGet, ModelPickerTrigger,
    composer_panel_host,
};
pub use layout::{
    AppShell, ChatList, Document, LegalLinks, MainMode, NavItem, SIDEBAR_COOKIE,
    SIDEBAR_TOGGLE_ATTR, Sidebar, SidebarSection, sidebar_collapsed_from_cookie,
};
pub use theme::{THEME_COOKIE, THEME_TOGGLE_ATTR, Theme, ThemeToggle};
pub use thread::{Message, MessageRole};

/// The thread stylesheet: messages, the thread column, the home stage.
pub const THREAD_CSS: &str = include_str!("../../static/components/thread.css");

/// The shell stylesheet: layout, left panel, navigation, header, main frame.
pub const SHELL_CSS: &str = include_str!("../../static/components/shell.css");

/// The composer stylesheet, including the product-only composer and message
/// roles.
pub const COMPOSER_CSS: &str = include_str!("../../static/components/composer.css");

#[cfg(test)]
mod tests;
