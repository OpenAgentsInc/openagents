use autopilot_app::{InboxAuditEntry, InboxSnapshot, InboxThreadSummary};
use chrono::Utc;

pub(crate) struct DesktopInboxState {
    snapshot: InboxSnapshot,
}

impl DesktopInboxState {
    pub(crate) fn new() -> Self {
        let mut state = Self {
            snapshot: InboxSnapshot::default(),
        };
        state.push_audit(
            "system",
            "bootstrap",
            "inbox state initialized; awaiting backend refresh",
        );
        state
    }

    pub(crate) fn snapshot(&self) -> InboxSnapshot {
        self.snapshot.clone()
    }

    pub(crate) fn replace_snapshot(&mut self, mut snapshot: InboxSnapshot) {
        if snapshot.audit_log.is_empty() {
            snapshot.audit_log = self.snapshot.audit_log.clone();
        }
        self.snapshot = snapshot;
        self.trim_audit();
    }

    pub(crate) fn apply_thread_detail(
        &mut self,
        thread: InboxThreadSummary,
        audit_log: Vec<InboxAuditEntry>,
    ) {
        if let Some(existing) = self
            .snapshot
            .threads
            .iter_mut()
            .find(|row| row.id == thread.id)
        {
            *existing = thread.clone();
        } else {
            self.snapshot.threads.push(thread.clone());
        }
        self.snapshot.selected_thread_id = Some(thread.id);
        if !audit_log.is_empty() {
            self.snapshot.audit_log = audit_log;
        }
        self.trim_audit();
    }

    pub(crate) fn select_thread(&mut self, thread_id: &str) {
        if self
            .snapshot
            .threads
            .iter()
            .any(|thread| thread.id == thread_id)
        {
            self.snapshot.selected_thread_id = Some(thread_id.to_string());
            self.push_audit(thread_id, "select_thread", "selected in desktop inbox pane");
        }
    }

    pub(crate) fn push_system_error(&mut self, detail: &str) {
        self.push_audit("system", "error", detail);
    }

    fn push_audit(&mut self, thread_id: &str, action: &str, detail: &str) {
        self.snapshot.audit_log.push(InboxAuditEntry {
            thread_id: thread_id.to_string(),
            action: action.to_string(),
            detail: detail.to_string(),
            created_at: Utc::now().to_rfc3339(),
        });
        self.trim_audit();
    }

    fn trim_audit(&mut self) {
        if self.snapshot.audit_log.len() > 200 {
            let drain = self.snapshot.audit_log.len().saturating_sub(200);
            self.snapshot.audit_log.drain(0..drain);
        }
    }
}

pub(crate) fn warm_inbox_domain_bridge() {
    let _ = DesktopInboxState::new();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inbox_state_bootstrap_starts_without_sample_threads() {
        let state = DesktopInboxState::new();
        let snapshot = state.snapshot();
        assert!(snapshot.threads.is_empty());
        assert!(!snapshot.audit_log.is_empty());
    }

    #[test]
    fn replace_snapshot_updates_selected_thread() {
        let mut state = DesktopInboxState::new();
        state.replace_snapshot(InboxSnapshot {
            threads: vec![InboxThreadSummary {
                id: "thread_1".to_string(),
                subject: "Subject".to_string(),
                from_address: "sender@example.com".to_string(),
                snippet: "Snippet".to_string(),
                category: "other".to_string(),
                risk: "medium".to_string(),
                policy: "draft_only".to_string(),
                draft_preview: "Draft".to_string(),
                pending_approval: true,
                updated_at: Utc::now().to_rfc3339(),
            }],
            selected_thread_id: Some("thread_1".to_string()),
            audit_log: vec![InboxAuditEntry {
                thread_id: "system".to_string(),
                action: "refresh".to_string(),
                detail: "snapshot refreshed".to_string(),
                created_at: Utc::now().to_rfc3339(),
            }],
        });

        let snapshot = state.snapshot();
        assert_eq!(snapshot.threads.len(), 1);
        assert_eq!(snapshot.selected_thread_id.as_deref(), Some("thread_1"));
    }
}
