//! Bounded notification-tap identity and retry state; the server still authorizes every destination.
use super::*;

pub(super) struct PendingTap {
    pub link: String,
    pub request: String,
    pub generation: Option<u64>,
    pub attempts: u8,
}
impl PendingTap {
    pub fn new(link: String, request: String, generation: Option<u64>) -> Option<Self> {
        owner_notification_chat_from_link(link.clone())?;
        if request.len() > 128 || !openagents_account::openagents::agents::valid_id(&request) {
            return None;
        }
        Some(Self {
            link,
            request,
            generation,
            attempts: 0,
        })
    }
    pub fn matches(&self, note: &OwnerNoteView) -> bool {
        note.request_id == self.request
            && note.deep_link == self.link
            && owner_notification_chat_from_link(self.link.clone()).as_deref()
                == Some(note.chat_id.as_str())
    }
    pub fn accepts(&self, generation: u64) -> bool {
        self.generation
            .is_none_or(|expected| expected == generation)
    }
}
#[derive(uniffi::Record)]
pub struct AndroidNotificationOpen {
    pub retry_after_ms: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn note() -> OwnerNoteView {
        OwnerNoteView {
            request_id: "a".repeat(64),
            chat_id: "fixture-chat".into(),
            title: "Fixture notice".into(),
            body: "Fixture message".into(),
            deep_link: "openagents://chat/fixture-chat".into(),
        }
    }
    #[test]
    fn notification_taps_bind_request_chat_and_current_account() {
        let note = note();
        let tap =
            PendingTap::new(note.deep_link.clone(), note.request_id.clone(), Some(7)).unwrap();
        assert!(tap.matches(&note));
        assert!(tap.accepts(7));
        assert!(!tap.accepts(8));
        let mut other = note.clone();
        other.request_id = "b".repeat(64);
        assert!(!tap.matches(&other));
        let mut other = note.clone();
        other.chat_id = "another-chat".into();
        assert!(!tap.matches(&other));
        assert!(
            PendingTap::new(
                "https://example.invalid/chat/x".into(),
                note.request_id,
                None
            )
            .is_none()
        );
    }
    #[test]
    fn an_initial_sign_in_can_resolve_a_cold_tap_but_a_bound_account_cannot_transfer_it() {
        let note = note();
        assert!(
            PendingTap::new(note.deep_link.clone(), note.request_id.clone(), None)
                .unwrap()
                .accepts(8)
        );
        assert!(
            !PendingTap::new(note.deep_link, note.request_id, Some(7))
                .unwrap()
                .accepts(8)
        );
    }
}
