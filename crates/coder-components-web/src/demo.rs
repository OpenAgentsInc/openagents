//! The local demo's native surface and portable browser input helpers.

#[cfg(target_arch = "wasm32")]
pub(crate) mod browser;

use coder_ui::source_theme;
use rust_native::{
    style::Style,
    view::{Axis, Element, Node, View},
};
use zeroize::Zeroizing;

/// The exact terminal frame is a locally registered drawing surface. The field
/// captures platform text input; the shared demo model owns every edit.
pub(crate) fn view(revision: u64) -> View<()> {
    let node = |key: &str, element| Node {
        key: key.to_owned(),
        style: source_theme::style(),
        element,
    };
    View::new_v3(
        "coder-native-demo",
        revision.max(1),
        Node {
            key: "demo-shell".into(),
            style: Style {
                background: Some(source_theme::BG_BASE),
                ..source_theme::style()
            },
            element: Element::Stack {
                axis: Axis::Vertical,
                children: vec![
                    node(
                        "demo-frame",
                        Element::Surface {
                            resource: "coder-demo-grid".into(),
                            label: "Coder demo terminal".into(),
                        },
                    ),
                    node(
                        "demo-keyboard",
                        Element::Field {
                            label: "Coder demo input. Arrow keys select agents; F2 opens plugins."
                                .into(),
                            value: String::new(),
                            placeholder: String::new(),
                            secret: false,
                            multiline: true,
                            enabled: true,
                            max_bytes: coder_ui::demo::MAX_DRAFT_BYTES,
                            on_change: (),
                        },
                    ),
                ],
            },
        },
    )
}

/// Composition commits once even when a browser follows compositionend with
/// a second input event carrying the same committed text.
#[derive(Default)]
pub(crate) struct Composition {
    pub active: bool,
    pub committed: Option<Zeroizing<String>>,
}

impl Composition {
    pub fn begin(&mut self) {
        self.active = true;
        self.committed = None;
    }

    pub fn end(&mut self, value: &str) -> Option<Zeroizing<String>> {
        self.active = false;
        self.committed = Some(Zeroizing::new(value.to_owned()));
        (!value.is_empty()).then(|| Zeroizing::new(value.to_owned()))
    }

    pub fn input(&mut self, value: &str) -> Option<Zeroizing<String>> {
        if self.active {
            return None;
        }
        if self.committed.take().as_ref().map(|text| text.as_str()) == Some(value) {
            return None;
        }
        (!value.is_empty()).then(|| Zeroizing::new(value.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_surface_and_keyboard_use_the_shared_contract() {
        assert!(view(1).validate().is_ok());
    }

    #[test]
    fn composition_commits_once_and_next_input_remains_independent() {
        let mut composition = Composition::default();
        composition.begin();
        assert_eq!(composition.input("漢"), None);
        assert_eq!(composition.end("漢"), Some(Zeroizing::new("漢".into())));
        assert_eq!(composition.input("漢"), None);
        assert_eq!(composition.input("字"), Some(Zeroizing::new("字".into())));
        composition.begin();
        assert_eq!(composition.end(""), None);
        assert_eq!(composition.input(""), None);
    }
}
