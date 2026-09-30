//! The accessibility tree of a chat-shaped view, read the way a platform
//! adapter reads it: through `accesskit_consumer`, the tree VoiceOver,
//! AT-SPI (Orca), and UI Automation are served from.

use super::*;
use crate::{App, capture};
use accesskit::{Action, ActionData, ActionRequest, TreeId};
use rust_native::style::{Color, Style};
use rust_native::{Axis, Element, Glyph, Icon, MessageRole, Node, TextRole, ValidatedView, View};
use std::time::Instant;

#[derive(Clone, serde::Serialize, PartialEq, Debug)]
enum Intent {
    Open(u8),
    Send,
    Toggle,
    Stop,
}

fn node<I>(key: &str, element: Element<I>) -> Node<I> {
    Node {
        key: key.into(),
        style: Style::default(),
        element,
    }
}

fn button(key: &str, label: &str, intent: Intent, icon: Option<Glyph>) -> Node<Intent> {
    node(
        key,
        Element::Button {
            label: label.into(),
            enabled: true,
            icon: icon.map(|glyph| Icon {
                glyph,
                circular: glyph == Glyph::ArrowUp,
                pill: false,
            }),
            shortcut: None,
            intent,
        },
    )
}

fn text<I>(key: &str, value: &str, role: TextRole) -> Node<I> {
    node(
        key,
        Element::Text {
            value: value.into(),
            role,
        },
    )
}

struct Chat {
    view: ValidatedView<Intent>,
    typed: String,
    focused: bool,
}

impl Chat {
    fn new() -> Chat {
        let sidebar = node(
            "sidebar",
            Element::List {
                label: "Chats".into(),
                children: vec![
                    button("chat-1", "Fix the login bug", Intent::Open(1), None),
                    button("chat-2", "Plan the launch", Intent::Open(2), None),
                ],
            },
        );
        let mut disabled = button("chat-3", "Archived chat", Intent::Open(3), None);
        if let Element::Button { enabled, .. } = &mut disabled.element {
            *enabled = false;
        }
        let root = node(
            "root",
            Element::Stack {
                axis: Axis::Vertical,
                children: vec![
                    sidebar,
                    disabled,
                    text("title", "Fix the login bug", TextRole::Heading),
                    node(
                        "chat-transcript",
                        Element::Surface {
                            resource: "chat-transcript".into(),
                            label: "Conversation".into(),
                        },
                    ),
                    node(
                        "chat-working",
                        Element::Working {
                            label: "Coder is working".into(),
                        },
                    ),
                    node(
                        "chat-composer",
                        Element::Composer {
                            token: "chat-1".into(),
                            placeholder: "Message OpenAgents".into(),
                            max_bytes: 4096,
                            enabled: true,
                            busy: false,
                            stop: Some(Intent::Stop),
                            choices: vec![],
                            draft: None,
                            focus: false,
                        },
                    ),
                    button("chat-send", "Send", Intent::Send, Some(Glyph::ArrowUp)),
                    button(
                        "auto-start",
                        "Start Coder automatically",
                        Intent::Toggle,
                        Some(Glyph::Unchecked),
                    ),
                ],
            },
        );
        Chat {
            view: View::new("chat", 1, root).validate().unwrap(),
            typed: "hello there".into(),
            focused: false,
        }
    }
}

/// The transcript's rows, as the desktop's transcript publishes them.
fn rows() -> Content {
    let card = Node {
        key: "offer".into(),
        style: Style {
            background: Some(Color::rgb(20, 20, 20)),
            ..Style::default()
        },
        element: Element::Stack {
            axis: Axis::Vertical,
            children: vec![
                text("offer-title", "Coder can fix this", TextRole::Body),
                node(
                    "offer-run",
                    Element::Button {
                        label: "Run Coder".into(),
                        enabled: true,
                        icon: None,
                        shortcut: None,
                        intent: (),
                    },
                ),
            ],
        },
    };
    let rows = vec![
        node(
            "m1",
            Element::Message {
                role: MessageRole::User,
                note: None,
                children: vec![text("m1-text", "Why can't I log in?", TextRole::Body)],
            },
        ),
        node(
            "m2",
            Element::Message {
                role: MessageRole::Assistant,
                note: Some("Just now".into()),
                children: vec![node(
                    "m2-text",
                    Element::Markdown {
                        blocks: rust_native::markdown::parse("The **session cookie** expired."),
                    },
                )],
            },
        ),
        card,
    ];
    Content {
        rows: rows.into_iter().map(Arc::new).collect(),
        bounds: HashMap::from([
            (
                "m2".into(),
                Rect {
                    x: 0.0,
                    y: 40.0,
                    w: 400.0,
                    h: 60.0,
                },
            ),
            (
                "offer-run".into(),
                Rect {
                    x: 10.0,
                    y: 120.0,
                    w: 100.0,
                    h: 30.0,
                },
            ),
        ]),
        conversation: true,
    }
}

impl App for Chat {
    type Intent = Intent;
    fn title(&self) -> String {
        "OpenAgents".into()
    }
    fn tick(&mut self, _: Instant) -> Option<Instant> {
        None
    }
    fn view(&self) -> &ValidatedView<Intent> {
        &self.view
    }
    fn activate(&mut self, _: Intent, _: Instant) {}
    fn surface_size(&self, resource: &str, available: f32) -> Option<(f32, f32)> {
        match resource {
            "chat-transcript" => Some((available, 300.0)),
            "composer:chat-composer" => Some((available, 56.0)),
            _ => None,
        }
    }
    fn access_value(&self, key: &str) -> Option<String> {
        (key == "chat-composer").then(|| self.typed.clone())
    }
    fn access_focus(&self) -> Option<String> {
        self.focused.then(|| "chat-composer".into())
    }
    fn access_content(&self, resource: &str) -> Option<Content> {
        (resource == "chat-transcript").then(rows)
    }
}

fn tree(app: &mut Chat, focus: Option<&str>) -> Tree {
    let (_, scene) = capture(app, 600.0, 800.0, 2.0);
    Tree::of(app, &scene, focus, 2.0, 0.0)
}

/// Each node the platform adapter serves, one a line: role, the name a
/// screen reader speaks, its value, and what it admits.
fn outline(update: &TreeUpdate) -> String {
    fn write(node: accesskit_consumer::NodeRef<'_>, depth: usize, out: &mut String) {
        let data = node.data();
        let mut line = format!("{}{:?}", "  ".repeat(depth), node.role());
        // A label's text is its value (`label_comes_from_value`).
        if let Some(label) = node.label().or_else(|| {
            node.label_comes_from_value()
                .then(|| node.value())
                .flatten()
        }) {
            line.push_str(&format!(" {label:?}"));
        }
        if node.role() != accesskit::Role::Label
            && let Some(value) = node.value()
        {
            line.push_str(&format!(" = {value:?}"));
        }
        if let Some(description) = data.description() {
            line.push_str(&format!(" ({description})"));
        }
        if let Some(toggled) = node.toggled() {
            line.push_str(&format!(" [{toggled:?}]"));
        }
        if node.is_disabled() {
            line.push_str(" [disabled]");
        }
        let actions: Vec<&str> = [
            (Action::Click, "click"),
            (Action::Focus, "focus"),
            (Action::SetValue, "set value"),
        ]
        .into_iter()
        .filter(|(action, _)| data.supports_action(*action))
        .map(|(_, name)| name)
        .collect();
        if !actions.is_empty() {
            line.push_str(&format!(" <{}>", actions.join(", ")));
        }
        if node.is_focused() {
            line.push_str(" *focused*");
        }
        out.push_str(&line);
        out.push('\n');
        for child in node.children() {
            write(child, depth + 1, out);
        }
    }
    let tree = accesskit_consumer::Tree::new(update.clone(), true);
    let mut out = String::new();
    write(tree.state().root(), 0, &mut out);
    out
}

#[test]
fn a_screen_reader_reads_the_sidebar_the_replies_and_the_composer() {
    let mut app = Chat::new();
    let tree = tree(&mut app, None);
    assert_eq!(
        outline(&tree.update),
        r#"Window "OpenAgents" *focused*
  GenericContainer
    List "Chats"
      Button "Fix the login bug" <click, focus>
      Button "Plan the launch" <click, focus>
    Button "Archived chat" [disabled]
    Heading "Fix the login bug"
    Log "Conversation"
      Article "You"
        Label "Why can't I log in?"
      Article "Reply" (Just now)
        Label "The session cookie expired."
      Group
        Label "Coder can fix this"
        Button "Run Coder" <click>
    Status "Coder is working"
    MultilineTextInput "Message OpenAgents" = "hello there" <click, focus, set value>
    Button "Send" <click, focus>
    CheckBox "Start Coder automatically" [False] <click, focus>
"#
    );
}

#[test]
fn focus_follows_the_windows_focus_and_the_applications_text_cursor() {
    let mut app = Chat::new();
    let tree = tree(&mut app, Some("chat-send"));
    assert!(outline(&tree.update).contains("Button \"Send\" <click, focus> *focused*"));
    app.focused = true;
    let tree = super::tests::tree(&mut app, Some("chat-send"));
    assert_eq!(tree.update.focus, tree.id("chat-composer").unwrap());
    assert!(outline(&tree.update).contains("*focused*\n    Button \"Send\" <click, focus>\n"));
}

fn ask(tree: &Tree, key: &str, action: Action, data: Option<ActionData>) -> Option<Request> {
    tree.request(&ActionRequest {
        action,
        target_tree: TreeId::ROOT,
        target_node: tree.id(key).unwrap(),
        data,
    })
}

#[test]
fn requests_become_the_input_the_window_already_handles() {
    let mut app = Chat::new();
    let tree = tree(&mut app, None);
    assert_eq!(
        ask(&tree, "chat-1", Action::Click, None),
        Some(Request::Activate("chat-1".into()))
    );
    assert_eq!(
        ask(&tree, "chat-send", Action::Focus, None),
        Some(Request::Focus("chat-send".into()))
    );
    assert_eq!(ask(&tree, "chat-3", Action::Click, None), None, "disabled");
    assert_eq!(ask(&tree, "title", Action::Click, None), None, "text");
    // A control in the transcript is clicked at its center, in the
    // transcript's own points.
    assert_eq!(
        ask(&tree, "chat-transcript/offer-run", Action::Click, None),
        Some(Request::Click {
            resource: "chat-transcript".into(),
            x: 60.0,
            y: 135.0,
        })
    );
    // The composer is focused by a click at the end of its text, and its
    // text is replaced as typing replaces it.
    let Some(Request::Click { resource, x, y }) = ask(&tree, "chat-composer", Action::Focus, None)
    else {
        panic!("a composer focuses by a click");
    };
    assert_eq!(resource, "composer:chat-composer");
    assert!(x > 500.0 && y == 54.0, "{x}, {y}");
    assert_eq!(
        ask(
            &tree,
            "chat-composer",
            Action::SetValue,
            Some(ActionData::Value("Try again".into()))
        ),
        Some(Request::SetValue {
            key: "chat-composer".into(),
            resource: "composer:chat-composer".into(),
            x,
            y,
            value: "Try again".into(),
        })
    );
    assert_eq!(ask(&tree, "chat-composer", Action::SetValue, None), None);
}

#[test]
fn bounds_are_the_laid_out_rectangles_in_window_pixels() {
    let mut app = Chat::new();
    let (_, scene) = capture(&mut app, 600.0, 800.0, 2.0);
    let tree = Tree::of(&app, &scene, None, 2.0, 0.0);
    let send = scene.bounds["chat-send"];
    let bounds = tree
        .node(tree.id("chat-send").unwrap())
        .unwrap()
        .bounds()
        .unwrap();
    assert_eq!(
        (bounds.x0, bounds.y0, bounds.x1, bounds.y1),
        (
            f64::from(send.x) * 2.0,
            f64::from(send.y) * 2.0,
            f64::from(send.x + send.w) * 2.0,
            f64::from(send.y + send.h) * 2.0
        )
    );
    // A transcript row's bounds are its surface's origin plus its own.
    let surface = scene.surface_rect("chat-transcript").unwrap();
    let reply = tree
        .node(tree.id("chat-transcript/m2").unwrap())
        .unwrap()
        .bounds()
        .unwrap();
    assert_eq!(reply.y0, f64::from(surface.y + 40.0) * 2.0);
    // Rows off screen have no bounds, and their controls no click.
    let tree = Tree::of(&app, &scene, None, 2.0, 100.0);
    let moved = tree
        .node(tree.id("chat-send").unwrap())
        .unwrap()
        .bounds()
        .unwrap();
    assert_eq!(moved.y0, f64::from(send.y - 100.0) * 2.0);
}

#[test]
fn ids_stay_the_same_from_one_tree_to_the_next() {
    let mut app = Chat::new();
    let first = tree(&mut app, None);
    app.typed = "hello there, again".into();
    let second = tree(&mut app, None);
    for key in ["chat-1", "chat-composer", "chat-transcript/m2", "chat-send"] {
        assert_eq!(first.id(key), second.id(key), "{key}");
    }
    assert_ne!(first.update, second.update);
    // The platform adapter takes the next full tree as an update.
    let mut consumer = accesskit_consumer::Tree::new(first.update.clone(), true);
    struct Changes(usize);
    impl accesskit_consumer::TreeChangeHandler for Changes {
        fn node_added(&mut self, _: &accesskit_consumer::NodeRef) {}
        fn node_updated(
            &mut self,
            _: &accesskit_consumer::NodeRef,
            _: &accesskit_consumer::NodeRef,
        ) {
            self.0 += 1;
        }
        fn focus_moved(
            &mut self,
            _: Option<&accesskit_consumer::NodeRef>,
            _: Option<&accesskit_consumer::NodeRef>,
        ) {
        }
        fn node_removed(&mut self, _: &accesskit_consumer::NodeRef) {}
    }
    let mut changes = Changes(0);
    consumer.update_and_process_changes(second.update.clone(), &mut changes);
    assert_eq!(changes.0, 1, "only the composer's value changed");
}
