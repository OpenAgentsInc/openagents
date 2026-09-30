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

pub fn prompt(title: &str, turns: &[Turn]) -> String {
    crate::basic_chats::handoff(title, turns, MAX_PROMPT_BYTES)
}

/// The router's offer selects presentation only; the host admits execution.
pub fn offered(meta: Option<&crate::router::Meta>, computer_lane: bool) -> bool {
    computer_lane || meta.is_some_and(|meta| meta.offers.contains(&crate::router::Offer::RunCoder))
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
