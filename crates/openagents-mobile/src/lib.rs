//! The OpenAgents mobile app's Rust library. Rust builds each screen as a
//! Rust Native view; the thin SwiftUI host decodes and renders it.

use rust_native::style::{Color, Space, Style, TextAlign, TextWeight};
use rust_native::{Axis, Element, Node, TextRole, View, ViewError};
use serde::{Deserialize, Serialize};
use std::ptr;

/// Button intents. The first screen has none.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {}

const WHITE: Color = Color::rgb(255, 255, 255);
const GRAY: Color = Color::rgb(153, 153, 153);

fn text(key: &str, value: &str, role: TextRole, foreground: Color) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            foreground: Some(foreground),
            weight: (role == TextRole::Heading).then_some(TextWeight::Bold),
            align: Some(TextAlign::Center),
            ..Style::default()
        },
        element: Element::Text {
            value: value.into(),
            role,
        },
    }
}

/// The home screen as encoded Rust Native view JSON.
pub fn home() -> Result<Vec<u8>, ViewError> {
    let root = Node {
        key: "home".into(),
        style: Style {
            gap: Some(Space::Sm),
            padding_top: Some(Space::Lg),
            padding_end: Some(Space::Lg),
            padding_bottom: Some(Space::Lg),
            padding_start: Some(Space::Lg),
            ..Style::default()
        },
        element: Element::Stack {
            axis: Axis::Vertical,
            children: vec![
                text("home-title", "Hello, world", TextRole::Heading, WHITE),
                text(
                    "home-detail",
                    "OpenAgents, rendered from Rust.",
                    TextRole::Status,
                    GRAY,
                ),
            ],
        },
    };
    View::new("openagents.home", 1, root).validate()?.to_json()
}

#[repr(C)]
pub struct OpenAgentsMobileBuffer {
    pub data: *mut u8,
    pub len: usize,
}

/// Return the home view. An empty buffer means the view failed validation.
/// Free the result once using `openagents_mobile_buffer_free`.
#[unsafe(no_mangle)]
pub extern "C" fn openagents_mobile_home() -> OpenAgentsMobileBuffer {
    let mut bytes = std::panic::catch_unwind(home)
        .ok()
        .and_then(Result::ok)
        .unwrap_or_default()
        .into_boxed_slice();
    let result = OpenAgentsMobileBuffer {
        data: if bytes.is_empty() {
            ptr::null_mut()
        } else {
            bytes.as_mut_ptr()
        },
        len: bytes.len(),
    };
    std::mem::forget(bytes);
    result
}

/// # Safety
/// The buffer must be an unmodified, not-yet-freed result from this library.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn openagents_mobile_buffer_free(value: OpenAgentsMobileBuffer) {
    if !value.data.is_null() {
        unsafe {
            drop(Box::from_raw(ptr::slice_from_raw_parts_mut(
                value.data, value.len,
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_is_a_valid_rust_native_view() {
        let bytes = home().expect("home view");
        let view = View::<Intent>::from_json(&bytes).expect("round trip");
        assert_eq!(view.view().instance, "openagents.home");
        let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(
            json["root"]["element"]["props"]["children"][0]["element"]["props"]["value"],
            "Hello, world"
        );
    }

    #[test]
    fn ffi_buffer_round_trips() {
        let buffer = openagents_mobile_home();
        assert!(!buffer.data.is_null());
        let bytes = unsafe { std::slice::from_raw_parts(buffer.data, buffer.len) }.to_vec();
        unsafe { openagents_mobile_buffer_free(buffer) };
        assert_eq!(bytes, home().expect("home view"));
    }
}
