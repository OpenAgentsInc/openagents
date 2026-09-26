//! A data-only screen example. It performs no task, network, or model action.
use rust_native::style::{Space, Style, StylePatch, StyleSheet};
use rust_native::{Axis, Element, Node, TextRole, View};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
enum Intent {
    InspectTask { task: String },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let styles = StyleSheet::new([("panel".into(), StylePatch::padding(Space::Sm))])?;
    let view = View::new(
        "task-status:example-mount",
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
                            value: "Execution: stopped. Checks: not run. Cost: unknown.".into(),
                            role: TextRole::Status,
                        },
                    },
                    Node {
                        key: "inspect".into(),
                        style: Style::default(),
                        element: Element::Button {
                            label: "Inspect task".into(),
                            enabled: true,
                            intent: Intent::InspectTask {
                                task: "example-task".into(),
                            },
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
