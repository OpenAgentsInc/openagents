//! What the chat panel tells screen readers ([`rust_native_desktop::access`]):
//! the text typed in its fields, which field has the text cursor, and the
//! transcript's messages and controls.

use super::{Panel, TRANSCRIPT};
use rust_native_desktop::access::Content;

impl Panel {
    /// The text typed in the field `key`.
    pub fn access_value(&self, key: &str) -> Option<String> {
        match key {
            "chat-composer" => Some(self.draft().to_owned()),
            "chat-search" => Some(self.search.text().to_owned()),
            "command-query" => Some(self.command_query.text().to_owned()),
            "chat-rename" => self
                .rename
                .as_ref()
                .map(|(_, field)| field.text().to_owned()),
            _ => None,
        }
    }

    /// The field with the text cursor, by node key.
    pub fn access_focus(&self) -> Option<String> {
        let composer = self
            .session
            .selected
            .as_ref()
            .and_then(|id| self.fields.get(id))
            .is_some_and(|field| field.focused);
        let key = if self.search.focused {
            "chat-search"
        } else if self.command_query.focused {
            "command-query"
        } else if self.rename.as_ref().is_some_and(|(_, field)| field.focused) {
            "chat-rename"
        } else if composer {
            "chat-composer"
        } else {
            return None;
        };
        Some(key.into())
    }

    /// The transcript's rows, with the bounds of those on screen.
    pub fn access_content(&self, resource: &str) -> Option<Content> {
        (resource == TRANSCRIPT).then(|| self.transcript.access())
    }
}
