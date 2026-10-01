//! Conversation handoff and project selection shared by phone and desktop.
use crate::basic_coder::Turn;

pub const MAX_PROMPT_BYTES: usize = 16 * 1024;

pub fn project(listed: &[String], used: impl Fn(&str) -> Option<u64>) -> Option<String> {
    listed
        .iter()
        .filter_map(|label| Some((used(label)?, label)))
        .max()
        .map(|(_, label)| label)
        .or_else(|| listed.iter().find(|label| *label == "openagents"))
        .or_else(|| listed.first())
        .cloned()
}

/// The prompt a Coder run starts with: the message that asked for the
/// work, titled by it, then bounded context ([`crate::basic_chats::handoff`]).
/// `chat_title` titles it only when the conversation has no user turn.
pub fn prompt(chat_title: &str, turns: &[Turn]) -> String {
    crate::basic_chats::handoff(&title(chat_title, turns), turns, MAX_PROMPT_BYTES)
}

/// The task's title for a Coder run started from a conversation: the
/// message that asked for the work, not the chat's title, which is its
/// first message (#10073).
pub fn title(chat_title: &str, turns: &[Turn]) -> String {
    crate::basic_chats::handoff_title(chat_title, turns)
}

/// Whether a reply offers Coder. The router's offer selects presentation
/// only; the host admits execution.
///
/// Precedence when one reply carries several things (#10073): an explicit
/// [`Offer::RunCoder`](crate::router::Offer::RunCoder) offers Coder; else a
/// typed offer or card for another action (a Gym test, a result, a deck, a
/// screen, a command) is what the router chose, and the reply does not
/// also offer Coder, even when the worker judged the thread's lane a
/// computer's; else the computer lane offers it. So one message never
/// yields both a Gym offer and a Coder start.
pub fn offered(meta: Option<&crate::router::Meta>, computer_lane: bool) -> bool {
    use crate::router::Offer;
    if meta.is_some_and(|meta| meta.offers.contains(&Offer::RunCoder)) {
        return true;
    }
    let other = meta.is_some_and(|meta| !meta.offers.is_empty() || !meta.cards.is_empty());
    computer_lane && !other
}

/// Put this computer's prediction of who runs Coder on the reply that
/// offers it: the last turn, when it is a reply that offers Coder, or any
/// reply when the thread's computer lane holds it. `predict` is asked only
/// then, so a thread without the offer reads no login state. An earlier
/// prediction is replaced; with none, it is cleared.
pub fn attach_runner(
    turns: &mut [Turn],
    computer_lane: bool,
    predict: impl FnOnce() -> Option<crate::coder_events::Runner>,
) {
    let Some(last) = turns.last_mut() else {
        return;
    };
    if last.role != crate::basic_coder::Role::Assistant || last.stopped {
        return;
    }
    if !offered(last.meta.as_ref(), computer_lane) {
        return;
    }
    let runner = predict();
    if runner.is_none() && last.meta.is_none() {
        return;
    }
    last.meta.get_or_insert_with(Default::default).runner = runner;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_reply_offering_coder_carries_the_prediction() {
        use crate::coder_events::Runner;
        use crate::router::{Meta, Offer};
        let offering = Some(Meta {
            offers: vec![Offer::RunCoder],
            ..Meta::default()
        });
        let mut turns = vec![
            Turn::user("fix the parser"),
            Turn::assistant("Coder can do that.", offering),
        ];
        attach_runner(&mut turns, false, || {
            Some(Runner::NotSignedIn { providers: vec![] })
        });
        assert_eq!(
            turns[1].meta.as_ref().unwrap().runner,
            Some(Runner::NotSignedIn { providers: vec![] })
        );
        let mut plain = vec![Turn::user("hi"), Turn::assistant("Hello.", None)];
        attach_runner(&mut plain, false, || panic!("no offer, no prediction"));
        assert!(plain[1].meta.is_none());
        // The computer lane makes any reply an offer.
        attach_runner(&mut plain, true, || {
            Some(Runner::NotSignedIn { providers: vec![] })
        });
        assert!(plain[1].meta.as_ref().unwrap().runner.is_some());
    }
    /// One message never yields both a Gym offer and a Coder start
    /// (#10073): a reply carrying the router's Gym card and `start_eval`
    /// offers no Coder, even on a computer lane; an explicit Run Coder
    /// offer still does, and a plain reply on a computer lane does.
    #[test]
    fn another_typed_action_outranks_the_computer_lane() {
        use crate::router::{Meta, Offer};
        let gym = Meta {
            offers: vec![Offer::StartEval {
                body: serde_json::json!({"offer": "start_eval"}),
            }],
            cards: vec![serde_json::json!({"card": "tool"})],
            ..Meta::default()
        };
        assert!(!offered(Some(&gym), true));
        let card_only = Meta {
            cards: vec![serde_json::json!({"card": "tool"})],
            ..Meta::default()
        };
        assert!(!offered(Some(&card_only), true));
        let deck = Meta {
            offers: vec![Offer::OpenPresentation { deck: "d".into() }],
            ..Meta::default()
        };
        assert!(!offered(Some(&deck), true));
        let both = Meta {
            offers: vec![
                Offer::StartEval {
                    body: serde_json::json!({"offer": "start_eval"}),
                },
                Offer::RunCoder,
            ],
            ..Meta::default()
        };
        assert!(offered(Some(&both), false));
        assert!(offered(Some(&Meta::default()), true));
        assert!(offered(None, true));
        assert!(!offered(None, false));
    }

    #[test]
    fn the_prompt_and_title_name_the_request() {
        let turns = vec![
            Turn::user("who are you"),
            Turn::assistant("We are OpenAgents.", None),
            Turn::user("do a test delegation now"),
        ];
        assert_eq!(title("who are you", &turns), "do a test delegation now");
        assert!(prompt("who are you", &turns).starts_with("do a test delegation now\n"));
    }

    #[test]
    fn project_prefers_last_used_then_openagents_then_first() {
        let listed = vec!["first".into(), "openagents".into(), "last".into()];
        assert_eq!(project(&listed, |_| None).as_deref(), Some("openagents"));
        assert_eq!(
            project(&listed, |label| (label == "last").then_some(10)).as_deref(),
            Some("last")
        );
        assert_eq!(project(&listed[..1], |_| None).as_deref(), Some("first"));
        assert!(project(&[], |_| None).is_none());
    }
}
