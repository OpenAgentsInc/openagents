//! Cached hosted transcript projection shared by native adapters.
use openagents_chat::basic_coder::{Role, Turn};
use rust_native::markdown::{Block, IncrementalMarkdown};
use rust_native::style::Style;
use rust_native::{Element, MessageRole, Node};
use std::collections::BTreeMap;

/// Adapter names and colors; message semantics remain shared.
pub struct Appearance<'a> {
    pub prefix: &'a str,
    pub body_suffix: &'a str,
    pub streaming_key: String,
    pub working_key: &'a str,
    pub working_label: &'a str,
    pub failed_key: &'a str,
    pub markdown_style: Style,
    pub status_style: Style,
}

#[derive(Default)]
pub struct Projection {
    parsed: BTreeMap<usize, (Turn, Vec<Block>)>,
    partial: IncrementalMarkdown,
}

pub struct Reply<'a> {
    pub busy: bool,
    pub partial: &'a str,
    pub failure: Option<&'a str>,
}

impl Projection {
    /// Parse changed messages only and retain no rows outside the observed page.
    pub fn rows<I>(
        &mut self,
        turns: &[Turn],
        start: usize,
        reply: Reply<'_>,
        appearance: &Appearance<'_>,
    ) -> Vec<Node<I>> {
        self.parsed
            .retain(|index, _| (*index >= start) && *index < start + turns.len());
        let mut rows = Vec::with_capacity(turns.len() + 2);
        for (offset, turn) in turns.iter().enumerate() {
            let index = start + offset;
            let entry = self
                .parsed
                .entry(index)
                .or_insert_with(|| (turn.clone(), rust_native::markdown::parse(&turn.text)));
            if entry.0 != *turn {
                *entry = (turn.clone(), rust_native::markdown::parse(&turn.text));
            }
            rows.push(message(
                &format!("{}{index}", appearance.prefix),
                if turn.role == Role::User {
                    MessageRole::User
                } else {
                    MessageRole::Assistant
                },
                entry.1.clone(),
                appearance,
            ));
        }
        self.partial.set(reply.partial);
        if !reply.partial.is_empty() {
            rows.push(message(
                &appearance.streaming_key,
                MessageRole::Assistant,
                self.partial.display_blocks().into_owned(),
                appearance,
            ));
        }
        if reply.busy {
            rows.push(Node {
                key: appearance.working_key.into(),
                style: Style::default(),
                element: Element::Working {
                    label: appearance.working_label.into(),
                },
            });
        } else if let Some(why) = reply.failure {
            rows.push(Node {
                key: appearance.failed_key.into(),
                style: Style::default(),
                element: Element::Message {
                    role: MessageRole::System,
                    note: None,
                    children: vec![Node {
                        key: format!("{}-text", appearance.failed_key),
                        style: appearance.status_style,
                        element: Element::Text {
                            value: why.into(),
                            role: rust_native::TextRole::Status,
                        },
                    }],
                },
            });
        }
        rows
    }
}

pub fn message<I>(
    key: &str,
    role: MessageRole,
    blocks: Vec<Block>,
    appearance: &Appearance<'_>,
) -> Node<I> {
    Node {
        key: key.into(),
        style: Style::default(),
        element: Element::Message {
            role,
            note: None,
            children: vec![Node {
                key: format!("{key}{}", appearance.body_suffix),
                style: appearance.markdown_style,
                element: Element::Markdown { blocks },
            }],
        },
    }
}

/// The newest reply may offer actions only after it completes successfully.
pub fn actionable(
    turns: &[Turn],
    busy: bool,
    failure: bool,
) -> Option<&openagents_chat::router::Meta> {
    if busy || failure {
        return None;
    }
    turns
        .last()
        .filter(|turn| turn.role == Role::Assistant && !turn.stopped)
        .and_then(|turn| turn.meta.as_ref())
}

/// Follow-up IDs preserve their index in the signed reply's metadata.
pub fn followups<'a>(
    meta: &'a openagents_chat::router::Meta,
    used: &'a [String],
) -> impl Iterator<Item = (usize, &'a openagents_chat::router::Followup)> + 'a {
    meta.followups.iter().enumerate().filter(|(_, followup)| {
        !openagents_chat::basic_chats::suggestion_used(
            used,
            followup.answer.as_deref(),
            &[&followup.label],
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_chat::router::{Followup, Meta};

    fn appearance() -> Appearance<'static> {
        Appearance {
            prefix: "message-",
            body_suffix: "-md",
            streaming_key: "partial".into(),
            working_key: "working",
            working_label: "Working…",
            failed_key: "failed",
            markdown_style: Style::default(),
            status_style: Style::default(),
        }
    }

    #[test]
    fn phone_and_desktop_projection_preserve_roles_blocks_and_reply_states() {
        let turns = vec![
            Turn::user("Question"),
            Turn::assistant("**Answer**\n\n```rust\n42\n```", None),
        ];
        let mut projection = Projection::default();
        let mut phone = appearance();
        phone.prefix = "talk-m";
        let desktop = appearance();
        let a: Vec<Node<()>> = projection.rows(
            &turns,
            2,
            Reply {
                busy: true,
                partial: "An *unfinished",
                failure: None,
            },
            &phone,
        );
        let b: Vec<Node<()>> = projection.rows(
            &turns,
            2,
            Reply {
                busy: true,
                partial: "An *unfinished",
                failure: None,
            },
            &desktop,
        );
        assert_eq!(a.len(), 4);
        for (a, b) in a.iter().zip(&b) {
            match (&a.element, &b.element) {
                (
                    Element::Message {
                        role: ar,
                        children: ac,
                        ..
                    },
                    Element::Message {
                        role: br,
                        children: bc,
                        ..
                    },
                ) => {
                    assert_eq!(ar, br);
                    assert_eq!(ac[0].element, bc[0].element);
                }
                (a, b) => assert_eq!(a, b),
            }
        }
        let failed: Vec<Node<()>> = projection.rows(
            &turns,
            2,
            Reply {
                busy: false,
                partial: "",
                failure: Some("Disconnected"),
            },
            &desktop,
        );
        assert_eq!(failed.last().unwrap().key, "failed");
        let done: Vec<Node<()>> = projection.rows(
            &turns,
            2,
            Reply {
                busy: false,
                partial: "",
                failure: None,
            },
            &desktop,
        );
        assert_eq!(done.len(), 2);
    }

    #[test]
    fn projection_drops_old_pages_and_parses_only_the_growing_tail() {
        let mut projection = Projection::default();
        let turns = vec![Turn::user("One"), Turn::assistant("Two", None)];
        let source = format!("{}\n\nTail", "Earlier paragraph.\n\n".repeat(100));
        let _: Vec<Node<()>> = projection.rows(
            &turns,
            0,
            Reply {
                busy: true,
                partial: &source,
                failure: None,
            },
            &appearance(),
        );
        let next = format!("{source} grows");
        let _: Vec<Node<()>> = projection.rows(
            &turns[1..],
            1,
            Reply {
                busy: true,
                partial: &next,
                failure: None,
            },
            &appearance(),
        );
        assert_eq!(projection.parsed.len(), 1);
        assert!(projection.partial.reparsed_bytes() < next.len() / 10);
    }

    #[test]
    fn stopped_failed_or_streaming_replies_never_offer_actions() {
        let meta: Meta =
            serde_json::from_str(r#"{"tier":"generate","offers":[],"followups":[]}"#).unwrap();
        let mut turns = vec![Turn::assistant("Done", Some(meta))];
        assert!(actionable(&turns, false, false).is_some());
        assert!(actionable(&turns, true, false).is_none());
        assert!(actionable(&turns, false, true).is_none());
        turns[0].stopped = true;
        assert!(actionable(&turns, false, false).is_none());
    }

    #[test]
    fn used_followups_disappear_by_answer_identity_or_normalized_words() {
        let mut meta: Meta =
            serde_json::from_str(r#"{"tier":"generate","offers":[],"followups":[]}"#).unwrap();
        meta.followups = vec![
            Followup {
                label: "Who are you?".into(),
                answer: None,
            },
            Followup {
                label: "Something else".into(),
                answer: Some("answer@v2".into()),
            },
            Followup {
                label: "A new question".into(),
                answer: None,
            },
        ];
        let used = vec![
            openagents_chat::basic_chats::words_mark("who are you").unwrap(),
            "id:answer".into(),
        ];
        let remaining: Vec<_> = followups(&meta, &used).map(|(index, _)| index).collect();
        assert_eq!(remaining, vec![2]);
    }
}
