//! Saved base tracking does not own or replace editable text.
#[derive(Default)]
pub struct ProjectCursor {
    pub version: u64,
    opening: Option<(u64, String)>,
}
#[derive(Debug, PartialEq)]
pub enum SaveFeedback {
    Saved,
    NewerEdits,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfirmationScope {
    pub account: Option<String>,
    pub project: String,
    pub epoch: u64,
    pub engine_session: String,
}
impl ConfirmationScope {
    pub fn accepts(&self, current: Option<&Self>) -> bool {
        current == Some(self)
    }
}
impl ProjectCursor {
    pub fn stage_open(&mut self, version: u64, document_sha256: String) {
        self.opening = Some((version, document_sha256));
    }
    pub fn cancel_open(&mut self) {
        self.opening = None;
    }
    pub fn opened(&mut self, document_sha256: &str) -> bool {
        let Some((version, hash)) = self.opening.take() else {
            return false;
        };
        if hash != document_sha256 {
            return false;
        }
        self.version = version;
        true
    }
    pub fn saved(
        &mut self,
        expected: u64,
        accepted: u64,
        sent_revision: u64,
        current_revision: u64,
        sent_source: &str,
        current_source: &str,
    ) -> Option<SaveFeedback> {
        if self.version != expected || accepted != expected.checked_add(1)? {
            return None;
        }
        self.version = accepted;
        Some(
            if sent_revision == current_revision && sent_source == current_source {
                SaveFeedback::Saved
            } else {
                SaveFeedback::NewerEdits
            },
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_applied_restore_receipt_cannot_complete_in_a_different_context() {
        let sent = ConfirmationScope {
            account: Some("alice".into()),
            project: "house-a".into(),
            epoch: 3,
            engine_session: "engine-a".into(),
        };
        let applied_receipt = (sent.clone(), "Alice's old restored tools");
        for next in [
            ConfirmationScope {
                account: Some("bob".into()),
                ..sent.clone()
            },
            ConfirmationScope {
                project: "house-b".into(),
                epoch: 4,
                ..sent.clone()
            },
            ConfirmationScope {
                engine_session: "replacement-frame".into(),
                ..sent.clone()
            },
        ] {
            let mut tools = "New context's tools";
            if applied_receipt.0.accepts(Some(&next)) {
                tools = applied_receipt.1;
            }
            assert_eq!(tools, "New context's tools");
            assert!(!sent.accepts(Some(&next)));
        }
        assert!(!sent.accepts(None));
        assert!(sent.accepts(Some(&sent)));
    }
    #[test]
    fn typing_during_a_save_keeps_new_text_and_the_acknowledged_base() {
        let mut cursor = ProjectCursor::default();
        let mut editor = "approved source".to_string();
        let posted = editor.clone();
        editor.push_str("\nunfinished new edit");
        assert_eq!(
            cursor.saved(0, 1, 4, 4, &posted, &editor),
            Some(SaveFeedback::NewerEdits)
        );
        assert_eq!(cursor.version, 1);
        assert_eq!(editor, "approved source\nunfinished new edit");
        assert_eq!(cursor.saved(0, 1, 4, 4, &posted, &editor), None);
        assert_eq!(
            cursor.saved(1, 2, 4, 5, &editor, &editor),
            Some(SaveFeedback::NewerEdits)
        );
        assert_eq!(
            cursor.saved(2, 3, 5, 5, &editor, &editor),
            Some(SaveFeedback::Saved)
        );
    }
    #[test]
    fn fetching_canceling_failing_or_importing_does_not_adopt_an_opened_base() {
        let mut cursor = ProjectCursor::default();
        cursor.version = 2;
        cursor.stage_open(8, "fetched".into());
        assert_eq!(cursor.version, 2);
        cursor.cancel_open();
        assert!(!cursor.opened("fetched"));
        assert_eq!(cursor.version, 2);
        cursor.stage_open(9, "failed".into());
        assert!(!cursor.opened("unrelated-import"));
        assert_eq!(cursor.version, 2);
        cursor.stage_open(10, "confirmed".into());
        assert!(cursor.opened("confirmed"));
        assert_eq!(cursor.version, 10);
        assert!(!cursor.opened("confirmed"));
    }
}
