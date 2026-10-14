//! Semantic controls for registered Verse surfaces or a poster/player adapter.
use crate::view::{Intent, State};
use rust_native::{Axis, Element, Node, TextRole, ValidatedView, View};

pub fn card(
    state: &State,
    instance: &str,
    revision: u64,
    resource: &str,
) -> Result<ValidatedView<Intent>, String> {
    let node = |key: &str, element| Node {
        key: key.into(),
        style: Default::default(),
        element,
    };
    let button = |key: &str, label: &str, intent| {
        node(
            key,
            Element::Button {
                label: label.into(),
                enabled: true,
                icon: None,
                shortcut: None,
                intent,
            },
        )
    };
    let bundle = state.bundle();
    let mut children = vec![
        node(
            "title",
            Element::Text {
                value: bundle.scene.name.clone(),
                role: TextRole::Heading,
            },
        ),
        node(
            "mode",
            Element::Text {
                value: format!("{} · Version {}", bundle.mode(), bundle.number),
                role: TextRole::Status,
            },
        ),
        node(
            "scene",
            Element::Surface {
                resource: resource.into(),
                label: "Robot and table in 3D".into(),
            },
        ),
        button("orbit", "Orbit", Intent::Activate),
        button("reset-view", "Reset view", Intent::ResetView),
        button("expand", "Expand view", Intent::Expanded),
        node(
            "joint-position",
            Element::Text {
                value: state.readout(),
                role: TextRole::Body,
            },
        ),
    ];
    if bundle.episode.is_some() {
        children.push(button(
            "play",
            if state.playing() { "Pause" } else { "Play" },
            if state.playing() {
                Intent::Pause
            } else {
                Intent::Play
            },
        ));
        children.push(node(
            "frame",
            Element::Field {
                label: "Frame".into(),
                value: state.frame.to_string(),
                placeholder: String::new(),
                secret: false,
                multiline: false,
                enabled: true,
                max_bytes: 10,
                on_change: Intent::Seek,
            },
        ));
    } else {
        for joint in bundle
            .robot
            .joints
            .iter()
            .filter(|j| j.kind != robot_lab::document::JointKind::Fixed)
        {
            children.push(button(
                &format!("joint-{}", joint.name),
                &joint.name,
                Intent::ChooseJoint {
                    joint: joint.name.clone(),
                },
            ));
        }
        let source = &bundle.scene.robots[0].joints;
        let value = state
            .draft
            .get(&state.joint)
            .or_else(|| source.get(&state.joint))
            .copied()
            .unwrap_or(0.0);
        children.push(node(
            "pose",
            Element::Field {
                label: "Joint position".into(),
                value: value.to_string(),
                placeholder: String::new(),
                secret: false,
                multiline: false,
                enabled: true,
                max_bytes: 32,
                on_change: Intent::JointValue {
                    joint: state.joint.clone(),
                },
            },
        ));
        if !state.draft.is_empty() {
            children.push(button("save-pose", "Save pose", Intent::ApplyPose));
        }
    }
    for link in &bundle.robot.links {
        children.push(button(
            &format!("link-{}", link.name),
            &format!("Inspect {}", link.name),
            Intent::ChooseLink {
                link: link.name.clone(),
            },
        ));
    }
    for camera in &bundle.scene.cameras {
        children.push(button(
            &format!("camera-{}", camera.id),
            &format!("Camera: {}", camera.id),
            Intent::ChooseCamera {
                camera: camera.id.clone(),
            },
        ));
    }
    children.push(button("compare", "Compare versions", Intent::Compare));
    children.push(node(
        "note",
        Element::Field {
            label: "Note".into(),
            value: state.note().into(),
            placeholder: "What did you notice?".into(),
            secret: false,
            multiline: false,
            enabled: true,
            max_bytes: 1024,
            on_change: Intent::NoteDraft,
        },
    ));
    children.push(button("save-note", "Save note", Intent::AddNote));
    View::new_v3(
        instance,
        revision,
        node(
            "card",
            Element::Stack {
                axis: Axis::Vertical,
                children,
            },
        ),
    )
    .validate()
    .map_err(|e| e.to_string())
}
