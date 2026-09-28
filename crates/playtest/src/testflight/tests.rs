use serde_json::json;

use super::*;
use crate::triage::{Contribution, Entry, Log};

/// The shape App Store Connect returns, with made-up values and the
/// tester fields it always includes.
fn screenshot_page() -> Value {
    json!({
        "data": [{
            "type": "betaFeedbackScreenshotSubmissions",
            "id": "AB2nFgx8example",
            "attributes": {
                "createdDate": "2026-09-28T14:40:18.809Z",
                "comment": "The Gym door didn't open.",
                "email": "tester@example.com",
                "deviceModel": "iPhone18_2",
                "osVersion": "26.6.2",
                "appPlatform": "IOS",
                "screenshots": [{"url": "https://tf-feedback.example/1.jpg", "width": 1320, "height": 2868,
                                  "expirationDate": "2026-10-04T00:00:00Z"}]
            },
            "relationships": {
                "tester": {"data": {"type": "betaTesters", "id": "f4d4-tester"}},
                "build": {"data": {"type": "builds", "id": "a779-build"}}
            }
        }],
        "included": [
            {"type": "betaTesters", "id": "f4d4-tester",
             "attributes": {"firstName": "Ada", "lastName": "Lovelace", "email": "tester@example.com"}},
            {"type": "builds", "id": "a779-build", "attributes": {"version": "16"}}
        ],
        "links": {"next": "https://api.appstoreconnect.apple.com/v1/apps/1/betaFeedbackScreenshotSubmissions?cursor=Mg"}
    })
}

#[test]
fn a_page_reads_the_feedback_and_keeps_no_tester_identity() {
    let page = parse_page(Source::Screenshot, &screenshot_page()).unwrap();
    assert_eq!(page.feedback.len(), 1);
    assert!(page.next.as_deref().unwrap().ends_with("cursor=Mg"));
    let feedback = &page.feedback[0];
    assert_eq!(feedback.build.as_deref(), Some("16"));
    assert_eq!(feedback.build_id.as_deref(), Some("a779-build"));
    assert_eq!(feedback.images.len(), 1);
    assert_eq!(
        feedback.comment.as_deref(),
        Some("The Gym door didn't open.")
    );
    let kept = serde_json::to_string(feedback).unwrap();
    for identity in ["tester@example.com", "Ada", "Lovelace", "f4d4-tester"] {
        assert!(!kept.contains(identity), "{identity}");
    }
    assert!(parse_page(Source::Crash, &screenshot_page()).is_err());
    assert!(parse_page(Source::Crash, &json!({"errors": []})).is_err());
}

#[test]
fn a_draft_never_quotes_the_comment_and_names_the_build() {
    let mut feedback = parse_page(Source::Screenshot, &screenshot_page())
        .unwrap()
        .feedback
        .remove(0);
    feedback.app_version = Some("1.0.0".into());
    let draft = draft(&feedback);
    assert!(!draft.body.contains("Gym door"));
    assert!(draft.body.contains(PARAPHRASE));
    assert!(draft.title.ends_with("(write a title)"));
    assert!(draft.body.contains("OpenAgents 1.0.0 (16)"));
    assert_eq!(
        draft.labels,
        ["playtest", "source:testflight", "build:1.0.0-16"]
    );
    assert!(draft.code.starts_with("TF-") && draft.code.len() == 11);
    assert_eq!(draft.code, code("AB2nFgx8example"));
}

#[test]
fn testflight_entries_are_triaged_once_and_back_no_award() {
    let feedback = parse_page(Source::Screenshot, &screenshot_page())
        .unwrap()
        .feedback
        .remove(0);
    let entry = Entry::Testflight {
        at: 10,
        code: feedback.code(),
        submission: feedback.id.clone(),
        source: feedback.source,
        build: feedback.build_label(),
        created: feedback.created.clone(),
    };
    let mut log = Log::default();
    log.admit(&entry).unwrap();
    log.entries.push(entry.clone());
    assert!(log.admit(&entry).is_err(), "a submission is logged once");
    assert!(log.has_submission(&feedback.id));
    assert_eq!(log.pending(), [feedback.code().as_str()]);
    let filed = Entry::Filed {
        at: 20,
        code: feedback.code(),
        issue: 9950,
        contribution: Contribution::Feedback,
        severity: None,
        triager: Some("ab".repeat(32)),
    };
    log.admit(&filed).unwrap();
    log.entries.push(filed);
    assert!(log.pending().is_empty());
    assert!(log.acceptances().is_empty());
    // The log reads back what it wrote.
    let text: String = log.entries.iter().map(Log::line).collect();
    assert_eq!(Log::parse(&text).unwrap(), log);
}
