use autopilot_inbox_domain::{
    ThreadCategory, classify_thread, compose_local_draft, infer_style_signature_from_bodies,
};

fn build_inbox_draft_preview(subject: &str, body: &str, snippet: &str) -> String {
    let decision = classify_thread(subject, body, snippet);
    let style_signature = infer_style_signature_from_bodies(["Best,\nAutopilot"]);
    compose_local_draft(
        decision.category,
        subject,
        body,
        None,
        None,
        None,
        style_signature.as_str(),
    )
}

fn classify_inbox_category(subject: &str, body: &str, snippet: &str) -> ThreadCategory {
    classify_thread(subject, body, snippet).category
}

pub(crate) fn warm_inbox_domain_bridge() {
    let _ = classify_inbox_category("Inbox warmup", "Schedule a check-in tomorrow", "");
    let _ = build_inbox_draft_preview("Inbox warmup", "Schedule a check-in tomorrow", "");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_uses_shared_domain_category_logic() {
        let category = classify_inbox_category("Schedule", "Can we meet tomorrow?", "");
        assert_eq!(category, ThreadCategory::Scheduling);
        let preview = build_inbox_draft_preview("Schedule", "Can we meet tomorrow?", "");
        assert!(preview.contains("Subject context"));
    }
}
