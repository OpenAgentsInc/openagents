//! A chat-shaped Rust Native window for checking the accessibility tree
//! against the platform's own accessibility API (#10024): a sidebar of
//! chats, a conversation whose rows the application paints, a composer, and
//! a send button. It keeps nothing on disk and talks to no network.
//!
//! `cargo run -p rust-native-desktop --example access_probe`, then query it
//! with `docs/desktop/verification/2026-09-30-accessibility/ax-probe.swift`
//! on the Mac, or Accerciser on Linux.

use rust_native::style::Style;
use rust_native::{Axis, Element, Glyph, Icon, MessageRole, Node, TextRole, ValidatedView, View};
use rust_native_desktop::access::Content;
use rust_native_desktop::input::{SurfaceInput, TextInput};
use rust_native_desktop::{App, window};
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone, serde::Serialize)]
enum Intent {
    Open(usize),
    Send,
}

struct Probe {
    chats: Vec<(&'static str, Vec<(MessageRole, String)>)>,
    open: usize,
    typed: String,
    focused: bool,
    revision: u64,
    view: ValidatedView<Intent>,
}

fn node<I>(key: &str, element: Element<I>) -> Node<I> {
    Node {
        key: key.into(),
        style: Style::default(),
        element,
    }
}

impl Probe {
    fn present(&mut self) {
        self.revision += 1;
        let rows = self
            .chats
            .iter()
            .enumerate()
            .map(|(index, (title, _))| {
                node(
                    &format!("chat-{index}"),
                    Element::Button {
                        label: (*title).into(),
                        enabled: index != self.open,
                        icon: None,
                        shortcut: None,
                        intent: Intent::Open(index),
                    },
                )
            })
            .collect();
        let root = node(
            "root",
            Element::Stack {
                axis: Axis::Vertical,
                children: vec![
                    node(
                        "sidebar",
                        Element::List {
                            label: "Chats".into(),
                            children: rows,
                        },
                    ),
                    node(
                        "title",
                        Element::Text {
                            value: self.chats[self.open].0.into(),
                            role: TextRole::Heading,
                        },
                    ),
                    node(
                        "chat-transcript",
                        Element::Surface {
                            resource: "chat-transcript".into(),
                            label: "Conversation".into(),
                        },
                    ),
                    node(
                        "chat-composer",
                        Element::Composer {
                            token: format!("chat-{}", self.open),
                            placeholder: "Message OpenAgents".into(),
                            max_bytes: 4096,
                            enabled: true,
                            busy: false,
                            stop: None,
                            choices: vec![],
                            draft: None,
                            focus: false,
                        },
                    ),
                    node(
                        "chat-send",
                        Element::Button {
                            label: "Send".into(),
                            enabled: !self.typed.trim().is_empty(),
                            icon: Some(Icon {
                                glyph: Glyph::ArrowUp,
                                circular: true,
                                pill: false,
                            }),
                            shortcut: None,
                            intent: Intent::Send,
                        },
                    ),
                ],
            },
        );
        self.view = View::new("access-probe", self.revision, root)
            .validate()
            .expect("a valid probe view");
    }
}

impl App for Probe {
    type Intent = Intent;
    fn title(&self) -> String {
        "Access probe".into()
    }
    fn tick(&mut self, _: Instant) -> Option<Instant> {
        None
    }
    fn view(&self) -> &ValidatedView<Intent> {
        &self.view
    }
    fn activate(&mut self, intent: Intent, _: Instant) {
        match intent {
            Intent::Open(index) => self.open = index,
            Intent::Send => {
                let text = std::mem::take(&mut self.typed);
                let chat = &mut self.chats[self.open].1;
                chat.push((MessageRole::User, text.clone()));
                chat.push((MessageRole::Assistant, format!("You said: {text}")));
            }
        }
        self.present();
    }
    fn text_input(&mut self, event: TextInput<'_>, _: Instant) -> bool {
        if !self.focused {
            return false;
        }
        match event {
            TextInput::Key {
                key: "a",
                command: true,
                ..
            } => {
                // Select all: the next text replaces everything.
                self.typed.clear();
            }
            TextInput::Key {
                key: "Backspace", ..
            } => {
                self.typed.pop();
            }
            TextInput::Key {
                text: Some(text), ..
            } if !text.chars().any(char::is_control) => self.typed.push_str(text),
            TextInput::Commit(text) => self.typed.push_str(text),
            _ => return false,
        }
        self.present();
        true
    }
    fn surface_input(&mut self, resource: &str, event: SurfaceInput, _: Instant) -> bool {
        if let SurfaceInput::Down { .. } = event {
            self.focused = resource == "composer:chat-composer";
        }
        true
    }
    fn surface_size(&self, resource: &str, available: f32) -> Option<(f32, f32)> {
        match resource {
            "chat-transcript" => Some((available, 240.0)),
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
        if resource != "chat-transcript" {
            return None;
        }
        let rows = self.chats[self.open]
            .1
            .iter()
            .enumerate()
            .map(|(index, (role, text))| {
                Arc::new(node(
                    &format!("m{index}"),
                    Element::Message {
                        role: *role,
                        note: None,
                        children: vec![node(
                            &format!("m{index}-text"),
                            Element::Text {
                                value: text.clone(),
                                role: TextRole::Body,
                            },
                        )],
                    },
                ))
            })
            .collect();
        Some(Content {
            rows,
            conversation: true,
            ..Content::default()
        })
    }
}

fn main() {
    let mut probe = Probe {
        chats: vec![
            (
                "Fix the login bug",
                vec![
                    (MessageRole::User, "Why can't I log in?".into()),
                    (
                        MessageRole::Assistant,
                        "The session cookie expired. Sign in again.".into(),
                    ),
                ],
            ),
            ("Plan the launch", vec![]),
        ],
        open: 0,
        typed: String::new(),
        focused: false,
        revision: 0,
        view: View::new(
            "access-probe",
            1,
            node(
                "empty",
                Element::Stack {
                    axis: Axis::Vertical,
                    children: vec![],
                },
            ),
        )
        .validate()
        .expect("a valid view"),
    };
    probe.present();
    if let Err(error) = window::run(probe, window::Options::default()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
