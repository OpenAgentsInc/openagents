//! A data-only settings screen. It emits a view without performing effects.
use rust_native::style::{Space, Style, StylePatch, StyleSheet};
use rust_native::{Axis, Element, Node, TextRole, View};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
enum Intent {
    OpenPreferences,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let styles = StyleSheet::new([("panel".into(), StylePatch::padding(Space::Sm))])?;
    let view = View::new(
        "settings:example-mount",
        1,
        Node {
            key: "panel".into(),
            style: styles.compose(["panel"])?.resolve(Style::default()),
            element: Element::Stack {
                axis: Axis::Vertical,
                children: vec![
                    Node {
                        key: "status".into(),
                        style: Style::default(),
                        element: Element::Text {
                            value: "Preferences are ready to edit.".into(),
                            role: TextRole::Status,
                        },
                    },
                    Node {
                        key: "inspect".into(),
                        style: Style::default(),
                        element: Element::Button {
                            label: "Open preferences".into(),
                            enabled: true,
                            intent: Intent::OpenPreferences,
                        },
                    },
                ],
            },
        },
    )
    .validate()?;
    println!("{}", String::from_utf8(view.to_json()?)?);
    Ok(())
}
