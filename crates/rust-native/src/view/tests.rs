use super::*;

#[test]
fn native_surfaces_are_local_labeled_resources_not_actions() {
    let view: View<()> = View::new(
        "window",
        1,
        Node {
            key: "canvas".into(),
            style: Style::default(),
            element: Element::Surface {
                resource: "drawing.canvas".into(),
                label: "Drawing canvas".into(),
            },
        },
    );
    let validated = view.clone().validate().unwrap();
    assert_eq!(
        validated.activate(&Activation {
            instance: "window".into(),
            revision: 1,
            node: "canvas".into()
        }),
        Err(ViewError::NotInteractive)
    );
    View::<()>::from_json(&validated.to_json().unwrap()).unwrap();
    let mut invalid = view.clone();
    invalid.root.element = Element::Surface {
        resource: "https://example.test/plugin".into(),
        label: "Remote".into(),
    };
    assert!(matches!(invalid.validate(), Err(ViewError::Identity)));
    let mut invalid = view;
    invalid.root.element = Element::Surface {
        resource: "canvas".into(),
        label: " ".into(),
    };
    assert!(matches!(invalid.validate(), Err(ViewError::MissingLabel)));
}

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
                            icon: None,
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
        icon: None,
        intent: Intent::InspectTask {
            task: "x".repeat(MAX_VIEW_BYTES),
        },
    };
    assert!(matches!(source.validate(), Err(ViewError::ViewLimit)));
    let mut source = sample();
    source.root.element = Element::Button {
        label: "  ".into(),
        enabled: true,
        icon: None,
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
                icon: None,
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

#[test]
fn list_rows_keep_identity_and_nested_button_activation() {
    let mut source = sample();
    let Element::Stack { children, .. } = source.root.element else {
        unreachable!()
    };
    source.root.element = Element::List {
        label: "Recorded messages".into(),
        children,
    };
    let validated = source.validate().unwrap();
    let decoded = View::<Intent>::from_json(&validated.to_json().unwrap()).unwrap();
    let event = Activation {
        instance: "task-panel:mount-1".into(),
        revision: 4,
        node: "inspect".into(),
    };
    assert_eq!(decoded.activate(&event), validated.activate(&event));
    assert!(
        decoded
            .activate(&Activation {
                node: "root".into(),
                ..event
            })
            .is_err()
    );
}

#[test]
fn list_labels_keys_and_window_bounds_are_checked() {
    let mut source = sample();
    source.root.element = Element::List {
        label: " ".into(),
        children: Vec::new(),
    };
    assert!(matches!(source.validate(), Err(ViewError::MissingLabel)));

    let mut source = sample();
    let row = Node {
        key: "row".into(),
        style: Style::default(),
        element: Element::Text {
            value: "literal".into(),
            role: TextRole::Markdown,
        },
    };
    source.root.element = Element::List {
        label: "Timeline".into(),
        children: vec![row.clone(), row.clone()],
    };
    assert!(matches!(
        source.validate(),
        Err(ViewError::DuplicateNode(_))
    ));
    let mut source = sample();
    source.root.element = Element::List {
        label: "Timeline".into(),
        children: (0..MAX_NODES)
            .map(|index| Node {
                key: format!("row-{index}"),
                ..row.clone()
            })
            .collect(),
    };
    assert!(matches!(source.validate(), Err(ViewError::NodeLimit)));
}

#[test]
fn markdown_preserves_source_and_old_schema_is_not_reinterpreted() {
    let mut source = sample();
    source.root.element = Element::Text {
        value: "# 日本語\n\n[link](https://example.invalid)\n<script>literal</script>".into(),
        role: TextRole::Markdown,
    };
    let validated = source.validate().unwrap();
    let bytes = validated.to_json().unwrap();
    assert_eq!(
        View::<Intent>::from_json(&bytes).unwrap().view(),
        validated.view()
    );
    let mut old = serde_json::to_value(validated.view()).unwrap();
    old["schema"] = "rust-native.view.v0".into();
    assert!(matches!(
        View::<Intent>::from_json(&serde_json::to_vec(&old).unwrap()),
        Err(ViewError::Schema)
    ));
}

mod conversation {
    use super::super::*;
    use crate::markdown;
    use crate::style::Style;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Chat {
        Earlier,
        Stop,
    }

    fn node(key: &str, element: Element<Chat>) -> Node<Chat> {
        Node {
            key: key.into(),
            style: Style::default(),
            element,
        }
    }

    /// A conversation with every conversation element, as an app builds it.
    fn sample() -> View<Chat> {
        let user = node(
            "m1",
            Element::Message {
                role: MessageRole::User,
                note: None,
                children: vec![node(
                    "m1-text",
                    Element::Markdown {
                        blocks: markdown::parse("Fix the **flaky** test in `ci.rs`."),
                    },
                )],
            },
        );
        let tool = node(
            "t1",
            Element::Tool {
                name: "Bash".into(),
                detail: "cargo test -p ci".into(),
                state: ToolState::Done,
                children: vec![node(
                    "t1-output",
                    Element::Text {
                        value: "test result: ok. 12 passed".into(),
                        role: TextRole::Code,
                    },
                )],
            },
        );
        let reply = node(
            "m2",
            Element::Message {
                role: MessageRole::Assistant,
                note: None,
                children: vec![node(
                    "m2-text",
                    Element::Markdown {
                        blocks: markdown::parse(
                            "Fixed it.\n\n- Seeded the RNG\n- Added a retry\n\n```rust\nlet seed = 7;\n```\n\n| a | b |\n|---|--:|\n| 1 | 2 |",
                        ),
                    },
                )],
            },
        );
        let system = node(
            "m3",
            Element::Message {
                role: MessageRole::System,
                note: Some("12:04".into()),
                children: vec![node(
                    "m3-text",
                    Element::Text {
                        value: "Context compacted.".into(),
                        role: TextRole::Status,
                    },
                )],
            },
        );
        let working = node(
            "working",
            Element::Working {
                label: "Coder is working".into(),
            },
        );
        let transcript = node(
            "transcript",
            Element::Transcript {
                source: None,
                label: "Messages".into(),
                children: vec![user, tool, reply, system, working],
                earlier: Some(Earlier {
                    label: "Load earlier messages".into(),
                    loading: false,
                    intent: Chat::Earlier,
                }),
            },
        );
        let composer = node(
            "composer",
            Element::Composer {
                token: "composer-1".into(),
                placeholder: "Message Coder".into(),
                max_bytes: 16 * 1024,
                enabled: true,
                busy: true,
                stop: Some(Chat::Stop),
                choices: vec![],
                draft: None,
                focus: false,
            },
        );
        View::new(
            "chat",
            1,
            node(
                "root",
                Element::Stack {
                    axis: Axis::Vertical,
                    children: vec![transcript, composer],
                },
            ),
        )
    }

    #[test]
    fn a_conversation_validates_and_activates() {
        let view = sample().validate().expect("valid");
        let activation = |node: &str| Activation {
            instance: "chat".into(),
            revision: 1,
            node: node.into(),
        };
        assert_eq!(view.activate(&activation("transcript")), Ok(&Chat::Earlier));
        assert_eq!(view.activate(&activation("composer")), Ok(&Chat::Stop));
        assert_eq!(
            view.activate(&activation("m2")),
            Err(ViewError::NotInteractive)
        );
        assert_eq!(view.accept_composer("composer-1", "hello"), Ok(()));
        assert_eq!(
            view.accept_composer("composer-2", "hello"),
            Err(crate::InputError::Stale)
        );
        assert_eq!(
            view.accept_composer("composer-1", &"x".repeat(16 * 1024 + 1)),
            Err(crate::InputError::TooLong)
        );
    }

    #[test]
    fn conversation_elements_refuse_bad_fields() {
        let mut bad = sample();
        if let Element::Stack { children, .. } = &mut bad.root.element {
            children[1].element = Element::Composer {
                token: "not an id".into(),
                placeholder: String::new(),
                max_bytes: 10,
                enabled: true,
                busy: false,
                stop: None,
                choices: vec![],
                draft: None,
                focus: false,
            };
        }
        assert_eq!(bad.validate().unwrap_err(), ViewError::Identity);
        let mut bad = sample();
        if let Element::Stack { children, .. } = &mut bad.root.element {
            children[1].element = Element::Working { label: " ".into() };
        }
        assert_eq!(bad.validate().unwrap_err(), ViewError::MissingLabel);
    }

    #[test]
    fn a_composer_answers_with_its_token_or_a_choices_token() {
        let composer = |choices: Vec<ComposerChoice>, draft: Option<&str>| {
            View::new(
                "chat",
                1,
                node(
                    "composer",
                    Element::<Chat>::Composer {
                        token: "send-1".into(),
                        placeholder: "Message Coder".into(),
                        max_bytes: 16,
                        enabled: true,
                        busy: false,
                        stop: None,
                        choices,
                        draft: draft.map(str::to_owned),
                        focus: false,
                    },
                ),
            )
        };
        let choice = |token: &str, label: &str| ComposerChoice {
            token: token.into(),
            label: label.into(),
        };
        let view = composer(
            vec![
                choice("queue-1", "Queue for next turn"),
                choice("stop-1", "Stop and send"),
            ],
            Some("Edit me"),
        )
        .validate()
        .expect("valid");
        for token in ["send-1", "queue-1", "stop-1"] {
            assert_eq!(view.accept_composer(token, "hello"), Ok(()));
        }
        assert_eq!(
            view.accept_composer("other-1", "hello"),
            Err(crate::InputError::Stale)
        );
        // A choice reuses no token and has a label; a draft fits the bound.
        for bad in [
            composer(vec![choice("send-1", "Again")], None),
            composer(vec![choice("a-1", "One"), choice("a-1", "Two")], None),
            composer(vec![choice("not an id", "One")], None),
        ] {
            assert_eq!(bad.validate().unwrap_err(), ViewError::Identity);
        }
        assert_eq!(
            composer(vec![choice("a-1", " ")], None)
                .validate()
                .unwrap_err(),
            ViewError::MissingLabel
        );
        assert_eq!(
            composer(vec![], Some("far too long for sixteen bytes"))
                .validate()
                .unwrap_err(),
            ViewError::TextLimit
        );
        // A composer without choices or a draft encodes as before.
        let plain = serde_json::to_value(composer(vec![], None)).unwrap();
        let props = &plain["root"]["element"]["props"];
        assert!(props.get("choices").is_none() && props.get("draft").is_none());
    }

    /// The fixture native adapters render and test against. Regenerate with
    /// `RUST_NATIVE_WRITE_FIXTURES=1`.
    #[test]
    fn conversation_fixture_is_current() {
        let json = serde_json::to_string_pretty(&sample()).expect("json") + "\n";
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/conversation.json");
        if std::env::var_os("RUST_NATIVE_WRITE_FIXTURES").is_some() {
            std::fs::write(path, &json).expect("write fixture");
        }
        assert_eq!(std::fs::read_to_string(path).expect("fixture"), json);
    }
}

#[test]
fn a_button_icon_is_optional_and_keeps_its_label() {
    let button = |icon| {
        View::new(
            "instance",
            1,
            Node {
                key: "new-chat".into(),
                style: Style::default(),
                element: Element::Button {
                    label: "New chat".into(),
                    enabled: true,
                    icon,
                    intent: Intent::InspectTask {
                        task: "task-1".into(),
                    },
                },
            },
        )
    };
    // A plain button encodes as before, so older adapters read it unchanged.
    let plain = button(None).validate().unwrap().to_json().unwrap();
    assert!(!String::from_utf8(plain.clone()).unwrap().contains("icon"));
    View::<Intent>::from_json(&plain).unwrap();
    let icon = Icon {
        glyph: Glyph::Compose,
        circular: true,
        pill: false,
    };
    let encoded = button(Some(icon)).validate().unwrap().to_json().unwrap();
    let decoded = View::<Intent>::from_json(&encoded).unwrap();
    assert!(matches!(
        decoded.view().root.element,
        Element::Button { icon: Some(i), .. } if i == icon
    ));
    // A circular glyph still needs a spoken name.
    let mut unnamed = button(Some(icon));
    if let Element::Button { label, .. } = &mut unnamed.root.element {
        *label = " ".into();
    }
    assert!(matches!(unnamed.validate(), Err(ViewError::MissingLabel)));
    // A glyph without `pill` encodes as before; a pill chip round-trips.
    assert!(!String::from_utf8(encoded).unwrap().contains("pill"));
    let chip = Icon {
        glyph: Glyph::History,
        circular: false,
        pill: true,
    };
    let encoded = button(Some(chip)).validate().unwrap().to_json().unwrap();
    assert!(
        String::from_utf8(encoded.clone())
            .unwrap()
            .contains(r#""glyph":"history","circular":false,"pill":true"#)
    );
    let decoded = View::<Intent>::from_json(&encoded).unwrap();
    assert!(matches!(
        decoded.view().root.element,
        Element::Button { icon: Some(i), .. } if i == chip
    ));
}

#[test]
fn a_wrapping_stack_round_trips() {
    let view: View<Intent> = View::new(
        "instance",
        1,
        Node {
            key: "chips".into(),
            style: Style::default(),
            element: Element::Stack {
                axis: Axis::Wrap,
                children: vec![],
            },
        },
    );
    let encoded = view.validate().unwrap().to_json().unwrap();
    assert!(
        String::from_utf8(encoded.clone())
            .unwrap()
            .contains(r#""axis":"wrap""#)
    );
    let decoded = View::<Intent>::from_json(&encoded).unwrap();
    assert!(matches!(
        decoded.view().root.element,
        Element::Stack {
            axis: Axis::Wrap,
            ..
        }
    ));
}

#[test]
fn checkbox_glyphs_round_trip() {
    for (glyph, name) in [(Glyph::Unchecked, "unchecked"), (Glyph::Checked, "checked")] {
        let icon = Icon {
            glyph,
            circular: false,
            pill: false,
        };
        let view: View<Intent> = View::new(
            "instance",
            1,
            Node {
                key: "allow".into(),
                style: Style::default(),
                element: Element::Button {
                    label: "Allow it".into(),
                    enabled: true,
                    icon: Some(icon),
                    intent: Intent::InspectTask {
                        task: "task-1".into(),
                    },
                },
            },
        );
        let encoded = view.validate().unwrap().to_json().unwrap();
        assert!(
            String::from_utf8(encoded.clone())
                .unwrap()
                .contains(&format!(r#""glyph":"{name}""#))
        );
        let decoded = View::<Intent>::from_json(&encoded).unwrap();
        assert!(matches!(
            decoded.view().root.element,
            Element::Button { icon: Some(i), .. } if i == icon
        ));
    }
}
