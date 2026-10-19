//! One autonomy card for any saved agent (#12045), a Rust Native view the
//! desktop, the phone and the website all mount: how the agent wakes, what
//! the account owner let it do (each grant removable, with a Decision log
//! switch), and, on the computer that runs its chat, that chat and its
//! latest decisions. Nothing here reads the agent's name for behavior.
use agent_contract::{AgentGrant, Autonomy, GrantOver, GrantScope, WakeEvent};
use rust_native::{Axis, Element, Node, TextRole, ValidatedView, View, ViewError, style::Style};
use serde::{Deserialize, Serialize};

/// What the card offers to change. The shell carries it out through the
/// account (grants) and asks for a fresh card.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Intent {
    /// Turn the agent's decision log on (grant `log.write`) or off (remove
    /// the grants that hold it).
    Log { on: bool },
    /// Remove one grant.
    Revoke { grant: String },
}

/// The part only the computer that runs the agent's chat knows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Here {
    /// The chat's title, when one is chosen.
    pub chat: Option<String>,
    /// When the next check-in is, already in the reader's words.
    pub next_check_in: Option<String>,
    /// The latest decision log lines, oldest first, already in plain words.
    pub log: Vec<String>,
    /// The agent kept a log here before the log grant existed, and the
    /// account hasn't taken that grant yet, so it's on without one.
    pub log_kept_before_grant: bool,
}

/// Everything the card shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    /// The agent's definition ID.
    pub agent: String,
    pub name: String,
    pub autonomy: Option<Autonomy>,
    pub grants: Vec<AgentGrant>,
    /// `None` away from the computer that runs the agent's chat.
    pub here: Option<Here>,
}

/// How many log lines the card shows.
pub const LOG_LINES: usize = 5;

impl Card {
    /// Whether the agent may keep a decision log.
    pub fn log_on(&self) -> bool {
        self.grants
            .iter()
            .any(|g| g.scopes.contains(&GrantScope::LogWrite))
            || self.here.as_ref().is_some_and(|h| h.log_kept_before_grant)
    }

    /// The grants that hold `log.write`, which turning the log off removes.
    pub fn log_grants(&self) -> Vec<String> {
        self.grants
            .iter()
            .filter(|g| g.scopes.contains(&GrantScope::LogWrite))
            .map(|g| g.id.clone())
            .collect()
    }

    /// The card as a view.
    ///
    /// # Errors
    /// The view doesn't validate, for example an empty instance.
    pub fn view(&self, instance: &str, revision: u64) -> Result<ValidatedView<Intent>, ViewError> {
        let mut children = vec![text("name", &self.name, TextRole::Heading)];
        children.push(text(
            "wakes",
            &wakes_line(self.autonomy.as_ref()),
            TextRole::Body,
        ));

        // Grant controls are hidden: agents act without grants (#12135, #12155).
        // The grants stay in the data.
        // children.push(text("grants-title", "What it may do", TextRole::Heading));
        // let shown: Vec<&AgentGrant> = self
        //     .grants
        //     .iter()
        //     .filter(|g| g.scopes.iter().any(|s| *s != GrantScope::LogWrite))
        //     .collect();
        // if shown.is_empty() {
        //     children.push(text(
        //         "grants-none",
        //         "Nothing yet. It can only work in its own chat.",
        //         TextRole::Body,
        //     ));
        // }
        // for grant in shown {
        //     children.push(row(
        //         &format!("grant-{}", grant.id),
        //         vec![
        //             text(
        //                 &format!("grant-{}-what", grant.id),
        //                 &grant_line(grant),
        //                 TextRole::Body,
        //             ),
        //             button(
        //                 &format!("grant-{}-remove", grant.id),
        //                 "Remove",
        //                 Intent::Revoke {
        //                     grant: grant.id.clone(),
        //                 },
        //             ),
        //         ],
        //     ));
        // }

        let on = self.log_on();
        let moving = on
            && self.log_grants().is_empty()
            && self.here.as_ref().is_some_and(|h| h.log_kept_before_grant);
        children.push(row(
            "log",
            vec![
                text(
                    "log-state",
                    if on {
                        "Decision log: on"
                    } else {
                        "Decision log: off"
                    },
                    TextRole::Heading,
                ),
                enabled_if(
                    button(
                        "log-switch",
                        if on { "Turn off" } else { "Turn on" },
                        Intent::Log { on: !on },
                    ),
                    !moving,
                ),
            ],
        ));
        if moving {
            children.push(text(
                "log-moving",
                "It kept a log on this computer before this switch existed, so it stays on until your account saves the switch.",
                TextRole::Status,
            ));
        }

        match &self.here {
            Some(here) => {
                children.push(text("here-title", "On this computer", TextRole::Heading));
                children.push(text(
                    "here-chat",
                    &match &here.chat {
                        Some(chat) => format!("Runs in “{chat}”"),
                        None => "No chat chosen for check-ins.".into(),
                    },
                    TextRole::Body,
                ));
                if let Some(next) = &here.next_check_in {
                    children.push(text("here-next", next, TextRole::Body));
                }
                let start = here.log.len().saturating_sub(LOG_LINES);
                if here.log.is_empty() {
                    children.push(text("here-log-none", "No decisions yet.", TextRole::Status));
                }
                for (at, line) in here.log[start..].iter().enumerate() {
                    children.push(text(&format!("here-log-{at}"), line, TextRole::Body));
                }
            }
            None => children.push(text(
                "elsewhere",
                "Its chat and decision log are on the computer that runs it.",
                TextRole::Status,
            )),
        }

        let root = node(
            "agent-card",
            Element::Stack {
                axis: Axis::Vertical,
                children,
            },
        );
        View::new(instance, revision, root).validate()
    }
}

/// Every text the card shows, for copy checks.
pub fn texts(view: &ValidatedView<Intent>) -> Vec<String> {
    fn walk(node: &Node<Intent>, out: &mut Vec<String>) {
        match &node.element {
            Element::Text { value, .. } => out.push(value.clone()),
            Element::Button { label, .. } => out.push(label.clone()),
            Element::Stack { children, .. } => children.iter().for_each(|c| walk(c, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(&view.view().root, &mut out);
    out
}

fn wakes_line(autonomy: Option<&Autonomy>) -> String {
    let Some(a) = autonomy.filter(|a| a.check_ins) else {
        return "Check-ins: off. It works when you message it.".into();
    };
    let every = a.every_minutes.map_or_else(
        || "Checks in on a schedule".to_owned(),
        |m| match m {
            60 => "Checks in every hour".to_owned(),
            m if m % 60 == 0 => format!("Checks in every {} hours", m / 60),
            m => format!("Checks in every {m} minutes"),
        },
    );
    let events: Vec<&str> = a.wakes.iter().filter_map(wake_words).collect();
    if events.is_empty() {
        format!("{every}.")
    } else {
        format!("{every}, and when {}.", join(&events))
    }
}

fn wake_words(wake: &WakeEvent) -> Option<&'static str> {
    Some(match wake {
        WakeEvent::ChatFinished => "a chat it manages finishes",
        WakeEvent::ChatErrored => "a chat it manages stops on an error",
        WakeEvent::ChatNeedsAnswer => "a chat it manages needs an answer",
        WakeEvent::IssueOpened => "an issue opens",
        WakeEvent::IssueClosed => "an issue closes",
        WakeEvent::ClaimStale => "a claimed issue goes quiet",
        WakeEvent::ChatStalled => "a chat it manages stalls",
        WakeEvent::ClaimNoProgress => "a claimed issue stops moving",
        WakeEvent::Other => return None,
    })
}

#[allow(dead_code)]
fn scope_words(scope: GrantScope) -> &'static str {
    match scope {
        GrantScope::ChatMessage => "message",
        GrantScope::ChatStart => "start",
        GrantScope::ChatSteer => "steer",
        GrantScope::ChatStop => "stop",
        GrantScope::LandSubmit => "submit changes to land",
        GrantScope::WorkDispatch => "send issue work to cloud computers",
        GrantScope::LogWrite => "keep a decision log",
    }
}

#[allow(dead_code)]
fn grant_line(grant: &AgentGrant) -> String {
    let chats: Vec<&str> = grant
        .scopes
        .iter()
        .filter(|s| {
            matches!(
                s,
                GrantScope::ChatMessage
                    | GrantScope::ChatStart
                    | GrantScope::ChatSteer
                    | GrantScope::ChatStop
            )
        })
        .map(|s| scope_words(*s))
        .collect();
    let mut parts = Vec::new();
    if !chats.is_empty() {
        parts.push(format!("{} chats", join(&chats)));
    }
    parts.extend(
        grant
            .scopes
            .iter()
            .filter(|s| matches!(s, GrantScope::LandSubmit | GrantScope::WorkDispatch))
            .map(|s| scope_words(*s).to_owned()),
    );
    let what = join(&parts.iter().map(String::as_str).collect::<Vec<_>>());
    let over = match &grant.over {
        GrantOver::Account => "on your whole account".to_owned(),
        GrantOver::Project(_) => "in one project".to_owned(),
        GrantOver::Chats(c) if c.len() == 1 => "in one chat".to_owned(),
        GrantOver::Chats(c) => format!("in {} chats", c.len()),
    };
    let mut line = format!("{what} {over}");
    if let Some(first) = line.get(..1) {
        line = first.to_uppercase() + &line[1..];
    }
    line
}

fn join(words: &[&str]) -> String {
    match words {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

fn node(key: &str, element: Element<Intent>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style::default(),
        element,
    }
}

fn text(key: &str, value: &str, role: TextRole) -> Node<Intent> {
    node(
        key,
        Element::Text {
            value: value.into(),
            role,
        },
    )
}

fn button(key: &str, label: &str, intent: Intent) -> Node<Intent> {
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
}

fn enabled_if(mut node: Node<Intent>, on: bool) -> Node<Intent> {
    if let Element::Button { enabled, .. } = &mut node.element {
        *enabled = on;
    }
    node
}

fn row(key: &str, children: Vec<Node<Intent>>) -> Node<Intent> {
    node(
        key,
        Element::Stack {
            axis: Axis::Horizontal,
            children,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_native::Activation;

    const AGENT: &str = "c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3";

    fn grant(id: &str, scopes: Vec<GrantScope>, over: GrantOver) -> AgentGrant {
        AgentGrant {
            id: id.into(),
            scopes,
            over,
            granted_at: 0,
        }
    }

    /// A reviewer agent that isn't Foreman: no name decides anything.
    fn reviewer() -> Card {
        Card {
            agent: AGENT.into(),
            name: "Release reviewer".into(),
            autonomy: Some(Autonomy {
                check_ins: true,
                every_minutes: Some(30),
                wakes: vec![WakeEvent::ChatFinished, WakeEvent::IssueClosed],
                ..Autonomy::default()
            }),
            grants: vec![grant(
                "g1",
                vec![GrantScope::ChatMessage, GrantScope::ChatSteer],
                GrantOver::Chats(vec!["a".into(), "b".into()]),
            )],
            here: None,
        }
    }

    fn activate(view: &ValidatedView<Intent>, key: &str) -> Intent {
        view.activate(&Activation {
            instance: view.view().instance.clone(),
            revision: view.view().revision,
            node: key.into(),
        })
        .unwrap()
        .clone()
    }

    #[test]
    fn any_agent_gets_the_card_with_its_wakes_grants_and_log_switch() {
        let card = reviewer();
        let view = card.view("agent-card-test", 1).unwrap();
        let texts = texts(&view);
        assert!(texts.contains(&"Release reviewer".to_owned()));
        assert!(
            texts.contains(
                &"Checks in every 30 minutes, and when a chat it manages finishes and an issue closes."
                    .to_owned()
            ),
            "{texts:?}"
        );
        // Grant rows are hidden while agents act without grants (#12155).
        assert!(!texts.iter().any(|t| t.contains("steer chats")), "{texts:?}");
        assert!(texts.contains(&"Decision log: off".to_owned()));
        assert!(
            texts.contains(
                &"Its chat and decision log are on the computer that runs it.".to_owned()
            )
        );
        assert_eq!(activate(&view, "log-switch"), Intent::Log { on: true });
    }

    #[test]
    fn the_log_grant_is_the_switch_and_this_computer_shows_the_log() {
        let mut card = reviewer();
        card.grants
            .push(grant("g2", vec![GrantScope::LogWrite], GrantOver::Account));
        card.here = Some(Here {
            chat: Some("Release review".into()),
            next_check_in: Some("Next check-in at 4:30 PM".into()),
            log: (1..=7).map(|i| format!("Decision {i}")).collect(),
            log_kept_before_grant: false,
        });
        assert_eq!(card.log_grants(), ["g2"]);
        let view = card.view("agent-card-test", 2).unwrap();
        let texts = texts(&view);
        assert!(texts.contains(&"Decision log: on".to_owned()));
        // The log grant is the switch, not a grant row.
        assert!(!texts.iter().any(|t| t.contains("keep a decision log")));
        assert!(texts.contains(&"Runs in “Release review”".to_owned()));
        assert!(!texts.contains(&"Decision 2".to_owned()));
        assert!(
            texts.contains(&"Decision 3".to_owned()) && texts.contains(&"Decision 7".to_owned())
        );
        assert_eq!(activate(&view, "log-switch"), Intent::Log { on: false });

        // A log kept before the grant existed shows on until it moves.
        card.grants.retain(|g| g.id != "g2");
        card.here.as_mut().unwrap().log_kept_before_grant = true;
        let texts = super::texts(&card.view("agent-card-test", 3).unwrap());
        assert!(texts.contains(&"Decision log: on".to_owned()));
        assert!(
            texts
                .iter()
                .any(|t| t.starts_with("It kept a log on this computer"))
        );
    }

    /// The card speaks plainly on every shell (#11031).
    #[test]
    fn every_card_text_passes_the_copy_guard() {
        let mut cards = vec![reviewer()];
        let mut here = reviewer();
        here.grants
            .push(grant("g2", vec![GrantScope::LogWrite], GrantOver::Account));
        here.here = Some(Here {
            chat: Some("Release review".into()),
            next_check_in: Some("Next check-in at 4:30 PM".into()),
            log: vec!["4:10 PM · Saw the build finish; decided to merge; did nothing".into()],
            log_kept_before_grant: false,
        });
        cards.push(here.clone());
        here.grants.clear();
        here.here.as_mut().unwrap().log_kept_before_grant = true;
        cards.push(here);
        for card in cards {
            for text in texts(&card.view("agent-card-test", 1).unwrap()) {
                let found = oa_copy::violations(&text, &[]);
                assert!(found.is_empty(), "{text}: {found:?}");
            }
        }
    }

    #[test]
    fn no_check_ins_and_no_grants_read_plainly() {
        let card = Card {
            agent: AGENT.into(),
            name: "Helper".into(),
            autonomy: None,
            grants: vec![grant(
                "g3",
                vec![GrantScope::LandSubmit],
                GrantOver::Account,
            )],
            here: None,
        };
        let texts = texts(&card.view("agent-card-test", 1).unwrap());
        assert!(texts.contains(&"Check-ins: off. It works when you message it.".to_owned()));
        assert!(
            texts.contains(&"Submit changes to land on your whole account".to_owned()),
            "{texts:?}"
        );
        let mut none = card.clone();
        none.grants.clear();
        let texts = super::texts(&none.view("agent-card-test", 1).unwrap());
        assert!(texts.contains(&"Nothing yet. It can only work in its own chat.".to_owned()));
    }
}
