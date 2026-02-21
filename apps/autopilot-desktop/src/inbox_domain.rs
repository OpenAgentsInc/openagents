use autopilot_app::{InboxAuditEntry, InboxSnapshot, InboxThreadSummary};
use autopilot_inbox_domain::{
    PolicyDecision, classify_thread, compose_local_draft, infer_style_signature_from_bodies,
    risk_to_str,
};
use chrono::Utc;

#[derive(Clone, Debug)]
struct DesktopInboxThread {
    id: String,
    subject: String,
    from_address: String,
    snippet: String,
    category: String,
    risk: String,
    policy: String,
    draft_preview: String,
    pending_approval: bool,
    updated_at: String,
}

pub(crate) struct DesktopInboxState {
    threads: Vec<DesktopInboxThread>,
    selected_thread_id: Option<String>,
    audit_log: Vec<InboxAuditEntry>,
}

impl DesktopInboxState {
    pub(crate) fn new() -> Self {
        let now = Utc::now().to_rfc3339();
        let style = infer_style_signature_from_bodies(["Best,\nAutopilot"]);
        let mut threads = Vec::new();
        for (id, subject, from_address, snippet, body) in [
            (
                "inbox-thread-001",
                "Can we reschedule the walkthrough?",
                "alex@acme.com",
                "Can we move tomorrow's call to next week?",
                "Can we move tomorrow's walkthrough to next week? Tuesday afternoon works on our side.",
            ),
            (
                "inbox-thread-002",
                "Updated pricing request",
                "finance@northstar.io",
                "Need revised quote for annual plan.",
                "Need a revised annual quote including a volume discount and implementation support.",
            ),
            (
                "inbox-thread-003",
                "Insurance claim paperwork",
                "legal@carrier.example",
                "Attorney requested supporting documents.",
                "Our attorney requested supporting documents related to the insurance claim and timelines.",
            ),
        ] {
            let decision = classify_thread(subject, body, snippet);
            let draft_preview =
                compose_local_draft(decision.category, subject, body, None, None, None, &style);
            let pending_approval = matches!(
                decision.policy,
                PolicyDecision::DraftOnly | PolicyDecision::SendWithApproval
            );
            threads.push(DesktopInboxThread {
                id: id.to_string(),
                subject: subject.to_string(),
                from_address: from_address.to_string(),
                snippet: snippet.to_string(),
                category: decision.category.as_str().to_string(),
                risk: risk_to_str(decision.risk).to_string(),
                policy: decision.policy.as_str().to_string(),
                draft_preview,
                pending_approval,
                updated_at: now.clone(),
            });
        }

        let selected_thread_id = threads.first().map(|thread| thread.id.clone());
        let mut state = Self {
            threads,
            selected_thread_id,
            audit_log: Vec::new(),
        };
        state.push_audit("system", "bootstrap", "inbox state initialized");
        state
    }

    pub(crate) fn snapshot(&self) -> InboxSnapshot {
        InboxSnapshot {
            threads: self
                .threads
                .iter()
                .map(|thread| InboxThreadSummary {
                    id: thread.id.clone(),
                    subject: thread.subject.clone(),
                    from_address: thread.from_address.clone(),
                    snippet: thread.snippet.clone(),
                    category: thread.category.clone(),
                    risk: thread.risk.clone(),
                    policy: thread.policy.clone(),
                    draft_preview: thread.draft_preview.clone(),
                    pending_approval: thread.pending_approval,
                    updated_at: thread.updated_at.clone(),
                })
                .collect(),
            selected_thread_id: self.selected_thread_id.clone(),
            audit_log: self.audit_log.clone(),
        }
    }

    pub(crate) fn refresh(&mut self) {
        let now = Utc::now().to_rfc3339();
        for thread in &mut self.threads {
            thread.updated_at = now.clone();
        }
        self.push_audit("system", "refresh", "inbox snapshot refreshed");
    }

    pub(crate) fn select_thread(&mut self, thread_id: &str) {
        if self.threads.iter().any(|thread| thread.id == thread_id) {
            self.selected_thread_id = Some(thread_id.to_string());
            self.push_audit(thread_id, "select_thread", "selected in desktop inbox pane");
        }
    }

    pub(crate) fn approve_draft(&mut self, thread_id: &str) {
        if let Some(thread) = self
            .threads
            .iter_mut()
            .find(|thread| thread.id == thread_id)
        {
            thread.pending_approval = false;
            thread.updated_at = Utc::now().to_rfc3339();
            self.push_audit(thread_id, "approve_draft", "draft marked approved");
        }
    }

    pub(crate) fn reject_draft(&mut self, thread_id: &str) {
        if let Some(thread) = self
            .threads
            .iter_mut()
            .find(|thread| thread.id == thread_id)
        {
            thread.pending_approval = true;
            thread.updated_at = Utc::now().to_rfc3339();
            self.push_audit(
                thread_id,
                "reject_draft",
                "draft flagged for manual revision",
            );
        }
    }

    pub(crate) fn load_audit(&mut self, thread_id: &str) {
        let detail = self
            .threads
            .iter()
            .find(|thread| thread.id == thread_id)
            .map(|thread| format!("loaded audit context for subject '{}'", thread.subject))
            .unwrap_or_else(|| "thread not found for audit load".to_string());
        self.push_audit(thread_id, "load_audit", &detail);
    }

    fn push_audit(&mut self, thread_id: &str, action: &str, detail: &str) {
        self.audit_log.push(InboxAuditEntry {
            thread_id: thread_id.to_string(),
            action: action.to_string(),
            detail: detail.to_string(),
            created_at: Utc::now().to_rfc3339(),
        });
        if self.audit_log.len() > 200 {
            let drain = self.audit_log.len().saturating_sub(200);
            self.audit_log.drain(0..drain);
        }
    }
}

pub(crate) fn warm_inbox_domain_bridge() {
    let mut state = DesktopInboxState::new();
    state.refresh();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inbox_state_bootstrap_has_threads_and_audit() {
        let state = DesktopInboxState::new();
        let snapshot = state.snapshot();
        assert!(!snapshot.threads.is_empty());
        assert!(!snapshot.audit_log.is_empty());
    }

    #[test]
    fn approve_and_reject_update_pending_state() {
        let mut state = DesktopInboxState::new();
        let thread_id = state
            .snapshot()
            .threads
            .first()
            .map(|thread| thread.id.clone())
            .unwrap_or_default();
        assert!(!thread_id.is_empty());

        state.approve_draft(&thread_id);
        let approved = state
            .snapshot()
            .threads
            .iter()
            .find(|thread| thread.id == thread_id)
            .map(|thread| thread.pending_approval)
            .unwrap_or(true);
        assert!(!approved);

        state.reject_draft(&thread_id);
        let rejected = state
            .snapshot()
            .threads
            .iter()
            .find(|thread| thread.id == thread_id)
            .map(|thread| thread.pending_approval)
            .unwrap_or(false);
        assert!(rejected);
    }
}
