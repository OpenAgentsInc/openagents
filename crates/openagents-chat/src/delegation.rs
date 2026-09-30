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

#[cfg(test)]
mod tests {
    use super::*;
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
