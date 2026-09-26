use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Intent {
    InspectTask { task: String },
}

fn sample() -> View<Intent> {
    View::new(
        "task-panel:mount-1",
        4,
        Node {
            key: "root".into(),
            style: Style::default(),
            element: Element::Stack {
                axis: Axis::Vertical,
                children: vec![
                    Node {
                        key: "status".into(),
                        style: Style::default(),
                        element: Element::Text {
                            value: "Cost: unknown · café 日本語 👩🏽‍💻".into(),
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
                                task: "task-1".into(),
                            },
                        },
                    },
                ],
            },
        },
    )
}

#[test]
fn round_trip_and_activation_use_the_current_typed_intent() {
    let view = sample().validate().unwrap();
    let decoded = View::<Intent>::from_json(&view.to_json().unwrap()).unwrap();
    assert_eq!(view.view(), decoded.view());
    let mut event = Activation {
        instance: "task-panel:mount-1".into(),
        revision: 4,
        node: "inspect".into(),
    };
    assert_eq!(
        decoded.activate(&event).unwrap(),
        &Intent::InspectTask {
            task: "task-1".into()
        }
    );
    event.revision = 3;
    assert_eq!(decoded.activate(&event), Err(ViewError::StaleActivation));
    event.revision = 4;
    event.instance = "task-panel:mount-2".into();
    assert_eq!(decoded.activate(&event), Err(ViewError::StaleActivation));
}

#[test]
fn disabled_controls_and_text_cannot_dispatch() {
    let mut source = sample();
    if let Element::Stack { children, .. } = &mut source.root.element
        && let Element::Button { enabled, .. } = &mut children[1].element
    {
        *enabled = false;
    }
    let view = source.validate().unwrap();
    let mut event = Activation {
        instance: "task-panel:mount-1".into(),
        revision: 4,
        node: "inspect".into(),
    };
    assert_eq!(view.activate(&event), Err(ViewError::Disabled));
    event.node = "status".into();
    assert_eq!(view.activate(&event), Err(ViewError::NotInteractive));
}

#[test]
fn duplicate_keys_and_unknown_fields_are_rejected() {
    let mut source = sample();
    if let Element::Stack { children, .. } = &mut source.root.element {
        children[1].key = "status".into();
    }
    assert!(matches!(
        source.validate(),
        Err(ViewError::DuplicateNode(_))
    ));
    let mut json = serde_json::to_value(sample()).unwrap();
    json["run_command"] = serde_json::json!("bad");
    assert!(View::<Intent>::from_json(&serde_json::to_vec(&json).unwrap()).is_err());
    json.as_object_mut().unwrap().remove("run_command");
    json["root"]["element"]["props"]["children"][1]["element"]["props"]["shell"] =
        serde_json::json!("bad");
    assert!(View::<Intent>::from_json(&serde_json::to_vec(&json).unwrap()).is_err());
}

#[test]
fn resource_bounds_apply_to_constructed_and_decoded_views() {
    let mut source = sample();
    source.root.element = Element::Text {
        value: "x".repeat(MAX_TEXT_BYTES + 1),
        role: TextRole::Body,
    };
    assert!(matches!(source.validate(), Err(ViewError::TextLimit)));
    assert!(matches!(
        View::<Intent>::from_json(&vec![b' '; MAX_VIEW_BYTES + 1]),
        Err(ViewError::ViewLimit)
    ));
    let mut source = sample();
    for depth in 0..MAX_DEPTH {
        source.root = Node {
            key: format!("layer-{depth}"),
            style: Style::default(),
            element: Element::Stack {
                axis: Axis::Vertical,
                children: vec![source.root],
            },
        };
    }
    assert!(matches!(source.validate(), Err(ViewError::DepthLimit)));
    let mut source = sample();
    source.root.element = Element::Stack {
        axis: Axis::Vertical,
        children: (0..MAX_NODES)
            .map(|index| Node {
                key: format!("n-{index}"),
                style: Style::default(),
                element: Element::Text {
                    value: "".into(),
                    role: TextRole::Body,
                },
            })
            .collect(),
    };
    assert!(matches!(source.validate(), Err(ViewError::NodeLimit)));
}

#[test]
fn encoded_budget_includes_intents_and_escaping() {
    let mut source = sample();
    source.root.element = Element::Button {
        label: "Inspect".into(),
        enabled: true,
        intent: Intent::InspectTask {
            task: "x".repeat(MAX_VIEW_BYTES),
        },
    };
    assert!(matches!(source.validate(), Err(ViewError::ViewLimit)));
    let mut source = sample();
    source.root.element = Element::Button {
        label: "  ".into(),
        enabled: true,
        intent: Intent::InspectTask {
            task: "task-1".into(),
        },
    };
    assert!(matches!(source.validate(), Err(ViewError::MissingLabel)));
}

#[test]
fn deepest_supported_tree_round_trips() {
    let mut source = sample();
    for depth in 2..MAX_DEPTH {
        source.root = Node {
            key: format!("layer-{depth}"),
            style: Style::default(),
            element: Element::Stack {
                axis: Axis::Vertical,
                children: vec![source.root],
            },
        };
    }
    let validated = source.validate().unwrap();
    let decoded = View::<Intent>::from_json(&validated.to_json().unwrap()).unwrap();
    assert_eq!(validated.view(), decoded.view());
}

#[test]
fn json_nesting_bound_includes_intents_but_not_quoted_brackets() {
    let mut intent = serde_json::Value::Null;
    for _ in 0..MAX_JSON_DEPTH {
        intent = serde_json::json!([intent]);
    }
    let source = View::new(
        "instance",
        1,
        Node {
            key: "button".into(),
            style: Style::default(),
            element: Element::Button {
                label: "Inspect".into(),
                enabled: true,
                intent,
            },
        },
    );
    let encoded = serde_json::to_vec(&source).unwrap();
    assert!(matches!(
        View::<serde_json::Value>::from_json(&encoded),
        Err(ViewError::JsonDepthLimit)
    ));
    assert!(matches!(source.validate(), Err(ViewError::JsonDepthLimit)));

    let mut source = sample();
    source.root.element = Element::Text {
        value: "\\\"[{}]".repeat(200),
        role: TextRole::Body,
    };
    let validated = source.validate().unwrap();
    assert_eq!(
        View::<Intent>::from_json(&validated.to_json().unwrap())
            .unwrap()
            .view(),
        validated.view()
    );
}
