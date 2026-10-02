use super::*;
use crate::report::{self, ChatRole, Platform, Randomness};
use crate::session::{Route, Tab};
use secp256k1::{Secp256k1, SecretKey};

fn key(byte: u8) -> SecretKey {
    SecretKey::from_byte_array([byte; 32]).expect("key")
}

fn context() -> Context {
    Context {
        app_version: "1.0.0".into(),
        build: "0".into(),
        platform: Platform::Macos,
        device: "Mac15,3".into(),
        os_version: "26.4".into(),
        tab: Tab::Coder,
        route: Route::Chat,
        at: 1_790_000_000,
    }
}

fn selection(text: &str) -> Selection {
    Selection {
        text: text.into(),
        thread: Some("chat-7f3a".into()),
        turn: Some(3),
        role: Some(ChatRole::Assistant),
        route: Some("chat".into()),
        tier: Some("model".into()),
        answer: None,
        model: Some("gpt-5.4".into()),
    }
}

#[test]
fn a_comment_on_a_selection_seals_and_the_triage_inbox_reads_it_back() {
    let report = report(
        context(),
        selection("  Coder runs on your phone.  "),
        " It runs on my computer, not the phone. ",
    )
    .expect("a report");
    assert_eq!(report.kind, Kind::Comment);
    assert_eq!(report.happened, "It runs on my computer, not the phone.");
    let (tester, triage) = (key(1), key(2));
    let triage_public = triage.x_only_public_key(&Secp256k1::new()).0;
    let random = Randomness {
        wrapper: key(9),
        seal_nonce: [1; 32],
        wrap_nonce: [2; 32],
        seal_earlier: 60,
        wrap_earlier: 120,
    };
    let sealed = report::wrap(&report, &tester, &triage_public, &random).expect("sealed");
    assert!(!sealed.wrap.content.contains("Coder runs"));
    // Only the triage key opens it, and it holds the quote and the comment.
    assert!(report::open(&sealed.wrap, &tester).is_err());
    let opened = report::open(&sealed.wrap, &triage).expect("opened");
    let read = opened.report.selection.as_ref().expect("the selection");
    assert_eq!(read.text, "Coder runs on your phone.");
    assert_eq!(read.thread.as_deref(), Some("chat-7f3a"));
    assert_eq!(read.turn, Some(3));
    assert_eq!(read.model.as_deref(), Some("gpt-5.4"));
    assert_eq!(
        opened.report.happened,
        "It runs on my computer, not the phone."
    );
    assert!(opened.report.chat.is_none() && opened.report.session.is_none());
    // The public record names the kind and the platform, never the words.
    let public = nostr::xp::playtest::parse_playtest_report(&sealed.public).expect("public");
    assert_eq!(
        (public.kind.as_str(), public.platform.as_str()),
        ("comment", "macos")
    );
    assert!(!sealed.public.content.contains("Coder runs"));
    // The draft names where the text came from and keeps the words private.
    let draft = crate::triage::draft(&opened);
    assert!(draft.body.contains("## Selected text"));
    assert!(draft.body.contains("model `gpt-5.4`"));
    assert!(!draft.body.contains("Coder runs on your phone."));
}

#[test]
fn feedback_needs_a_comment_and_a_safe_bounded_selection() {
    assert!(report(context(), selection("text"), "  ").is_err());
    assert!(report(context(), selection("   "), "why").is_err());
    let leaked = report(context(), selection("my key nsec1abc"), "why");
    assert!(leaked.unwrap_err().contains("secret key"));
    let mut odd = selection("text");
    odd.model = Some("a model with spaces".into());
    assert!(report(context(), odd, "why").is_err());
    let long = "a".repeat(MAX_TEXT_CHARS * 2);
    let clipped = report(context(), selection(&long), "too long").expect("clipped");
    let text = &clipped.selection.as_ref().expect("selection").text;
    assert_eq!(text.chars().count(), MAX_TEXT_CHARS);
    assert!(text.ends_with('…'));
}

#[test]
fn a_desktop_build_reads_the_triage_key_override() {
    let secret = key(5);
    let public = secret.x_only_public_key(&Secp256k1::new()).0;
    let npub = nostr::nip19::encode_npub(&public.serialize());
    assert_eq!(crate::triage_key(Some(&public.to_string())), Some(public));
    assert_eq!(crate::triage_key(Some(&npub)), Some(public));
    assert_eq!(crate::triage_key(Some("not a key")), None);
    assert_eq!(
        crate::triage_key(None),
        crate::TRIAGE_KEY.and_then(|k| k.parse().ok())
    );
}
