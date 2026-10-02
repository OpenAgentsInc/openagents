use super::*;
use crate::session::{Code, Event as SessionEvent};

fn key(byte: u8) -> SecretKey {
    SecretKey::from_byte_array([byte; 32]).expect("key")
}

fn public(secret: &SecretKey) -> XOnlyPublicKey {
    secret.x_only_public_key(&Secp256k1::new()).0
}

fn random() -> Randomness {
    Randomness {
        wrapper: key(9),
        seal_nonce: [1; 32],
        wrap_nonce: [2; 32],
        seal_earlier: 60,
        wrap_earlier: 120,
    }
}

fn report(tab: Tab, route: Route) -> Report {
    Report {
        schema: SCHEMA.into(),
        context: Context {
            app_version: "1.0.0".into(),
            build: "15".into(),
            platform: Platform::Ios,
            device: "iPhone17,1".into(),
            os_version: "26.0".into(),
            tab,
            route,
            at: 1_790_000_000,
        },
        kind: Kind::Bug,
        happened: "The ball went through the wall.".into(),
        expected: "It bounces.".into(),
        steps: "Push the ball hard at the Gym wall.".into(),
        quote: false,
        task: None,
        session: None,
        screenshot: None,
        notes: vec![],
        chat: None,
        selection: None,
    }
}

fn shot(bytes: usize) -> Screenshot {
    let mut b64 = String::from("/9j/");
    while b64.len() < bytes {
        b64.push_str("AAAA");
    }
    Screenshot {
        jpeg_base64: b64,
        width: 390,
        height: 844,
    }
}

#[test]
fn a_report_seals_to_the_triage_key_and_only_it_opens_it() {
    let (tester, triage) = (key(3), key(4));
    let mut sent = report(Tab::Verse, Route::Gym);
    sent.screenshot = Some(shot(2_000));
    let sealed = wrap(&sent, &tester, &public(&triage), &random()).expect("sealed");
    assert!(sealed.code.starts_with("PT-") && sealed.code.len() == 11);
    assert_eq!(sealed.wrap.kind, 1_059);
    // Nothing readable travels: not the text, the build, or the tester.
    let wire = serde_json::to_string(&sealed.wrap).unwrap();
    assert!(!wire.contains("ball") && !wire.contains("1.0.0"));
    assert!(!wire.contains(&public(&tester).to_string()));
    assert_eq!(
        sealed.wrap.gift_wrap_recipient(),
        Some(public(&triage).to_string().as_str())
    );
    let opened = open(&sealed.wrap, &triage).expect("opened");
    assert_eq!(opened.report, sent);
    assert_eq!(opened.tester, public(&tester).to_string());
    assert_eq!(
        (opened.code, opened.digest),
        (sealed.code, sealed.digest.clone())
    );
    assert_eq!(sealed.digest, digest(&sent.content()));
    assert!(open(&sealed.wrap, &key(5)).is_err());
}

#[test]
fn no_screenshot_leaves_the_wallet_or_a_key_screen() {
    for (tab, route) in [
        (Tab::Wallet, Route::Home),
        (Tab::Wallet, Route::History),
        (Tab::Account, Route::Identity),
        (Tab::Account, Route::Trainer),
    ] {
        let mut sent = report(tab, route);
        assert!(sent.check().is_ok());
        sent.screenshot = Some(shot(100));
        let error = wrap(&sent, &key(3), &public(&key(4)), &random()).unwrap_err();
        assert!(error.contains("never sent from the Wallet"), "{error}");
    }
}

#[test]
fn reports_are_bounded_and_refuse_secret_keys() {
    let mut sent = report(Tab::Coder, Route::Chat);
    sent.happened = " ".into();
    assert!(sent.check().is_err());
    sent.happened = "x".repeat(MAX_TEXT_CHARS + 1);
    assert!(sent.check().is_err());
    sent.happened = "my key is nsec1qqqq".into();
    assert!(sent.check().unwrap_err().contains("secret key"));
    let mut sent = report(Tab::Coder, Route::Chat);
    sent.context.build = "15; rm".into();
    assert!(sent.check().is_err());
    let mut sent = report(Tab::Coder, Route::Chat);
    sent.screenshot = Some(Screenshot {
        jpeg_base64: "iVBORw0K".into(),
        width: 1,
        height: 1,
    });
    assert!(sent.check().unwrap_err().contains("JPEG"));
    // The host can't add fields.
    let mut value = serde_json::to_value(report(Tab::Coder, Route::Chat)).unwrap();
    value["balance"] = serde_json::json!("21000 sats");
    assert!(serde_json::from_value::<Report>(value).is_err());
}

#[test]
fn a_report_too_large_to_seal_drops_its_screenshot_then_old_session_events() {
    let mut sent = report(Tab::Verse, Route::Home);
    sent.screenshot = Some(shot(MAX_CONTENT_BYTES));
    sent.session = Some(
        (0..session::MAX_EVENTS as u64)
            .map(|at| SessionEvent {
                at: 1_790_000_000 + at,
                tab: Tab::Verse,
                route: Route::Home,
                code: Code::Screen,
            })
            .collect(),
    );
    let fitted = sent.fit();
    assert!(fitted.screenshot.is_none());
    assert_eq!(fitted.notes, [Note::ScreenshotDropped]);
    assert!(fitted.content().len() <= MAX_CONTENT_BYTES);
    // The largest written report still seals.
    let mut big = report(Tab::Verse, Route::Home);
    big.happened = "é".repeat(MAX_TEXT_CHARS);
    big.expected = "é".repeat(MAX_TEXT_CHARS);
    big.steps = "é".repeat(MAX_TEXT_CHARS);
    big.session = fitted.session.clone();
    big.screenshot = Some(shot(30_000));
    let big = big.fit();
    assert!(wrap(&big, &key(3), &public(&key(4)), &random()).is_ok());
}

#[test]
fn a_message_without_the_marker_is_not_a_report() {
    let (tester, triage) = (key(3), key(4));
    let rumor = nip17::chat_rumor(
        &public(&tester).to_string(),
        1_790_000_000,
        &report(Tab::Verse, Route::Home).content(),
        vec![Tag::new(vec!["p".into(), public(&triage).to_string()])],
    )
    .unwrap();
    let seal = nip17::seal(
        &rumor,
        &tester,
        &public(&triage),
        1_790_000_000,
        [1; 32],
        None,
    )
    .unwrap();
    let wrap = nip17::gift_wrap(
        &seal,
        &key(9),
        &public(&triage),
        1_790_000_000,
        [2; 32],
        None,
    )
    .unwrap();
    assert_eq!(open(&wrap, &triage).unwrap_err(), "not a playtest report");
}

#[test]
fn the_code_is_the_rumor_prefix_in_capitals() {
    assert_eq!(code("1a2b3c4d5e6f"), "PT-1A2B3C4D");
}

#[test]
fn the_public_record_commits_to_the_opened_report_and_holds_no_text() {
    let tester = key(3);
    let triage = key(4);
    let mut filed = report(Tab::Verse, Route::Gym);
    filed.kind = Kind::FeltGood;
    filed.context.platform = Platform::Android;
    let sealed = wrap(&filed, &tester, &public(&triage), &random()).expect("sealed");
    let opened = open(&sealed.wrap, &triage).expect("opened");

    let record = nostr::xp::playtest::parse_playtest_report(&sealed.public).expect("a valid 3197");
    assert_eq!(sealed.public.kind, nostr::kinds::XP_PLAYTEST_REPORT);
    assert_eq!(sealed.public.pubkey, opened.tester);
    assert_eq!(record.digest, opened.digest);
    assert_eq!(record.digest, sealed.digest);
    assert_eq!(record.build, "1.0.0 (15)");
    assert_eq!(record.platform, "android");
    assert_eq!(record.kind, "felt-good");
    assert_eq!(record.script, None);
    assert_eq!(sealed.public.created_at, filed.context.at);
    for words in [
        &filed.happened,
        &filed.expected,
        &filed.steps,
        &filed.context.device,
    ] {
        assert!(!sealed.public.content.contains(words.as_str()));
    }
    assert!(!sealed.public.content.contains(&public(&triage).to_string()));
}

fn shared(reason: ShareReason, turns: usize) -> SharedChat {
    SharedChat {
        reason,
        turns: (0..turns)
            .map(|at| ChatTurn {
                role: if at % 2 == 0 {
                    ChatRole::User
                } else {
                    ChatRole::Assistant
                },
                text: format!("message {at} about the relay"),
                answer: (at % 2 == 1).then(|| "meta.model@1".to_owned()),
                tier: (at % 2 == 1).then(|| "canned".to_owned()),
                judgment: (at % 2 == 1).then(|| r#"{"type":"judgment","answer_p":0.9}"#.to_owned()),
            })
            .collect(),
    }
}

/// A chat travels only inside the sealed report, from the Chat tab, and
/// never in the public record; one with a secret key in it is refused.
#[test]
fn a_shared_chat_rides_only_inside_the_sealed_report() {
    let (tester, triage) = (key(3), key(4));
    let mut filed = report(Tab::Coder, Route::Chat);
    filed.chat = Some(shared(ShareReason::WrongAnswer, 2));
    let sealed = wrap(&filed, &tester, &public(&triage), &random()).expect("sealed");
    let wire = serde_json::to_string(&sealed.wrap).unwrap();
    assert!(!wire.contains("relay") && !wire.contains("meta.model"));
    assert!(!sealed.public.content.contains("relay"));
    assert!(!sealed.public.content.contains("meta.model"));
    let opened = open(&sealed.wrap, &triage).expect("opened");
    assert_eq!(opened.report.chat, filed.chat);
    assert_eq!(
        filed.chat.as_ref().unwrap().lines()[1],
        "OpenAgents (prepared answer meta.model@1): message 1 about the relay"
    );

    let mut elsewhere = filed.clone();
    elsewhere.context.tab = Tab::Wallet;
    elsewhere.context.route = Route::Home;
    assert!(elsewhere.check().unwrap_err().contains("Chat tab"));
    let mut secret = filed.clone();
    secret.chat.as_mut().unwrap().turns[0].text = "here: nsec1qqqq".into();
    assert!(secret.check().unwrap_err().contains("secret key"));
    let mut empty = filed;
    empty.chat.as_mut().unwrap().turns.clear();
    assert!(empty.check().is_err());
}

#[test]
fn a_long_shared_chat_keeps_its_newest_messages_to_fit() {
    let mut filed = report(Tab::Coder, Route::Chat);
    let mut chat = shared(ShareReason::Shared, 150);
    for turn in &mut chat.turns {
        turn.text = format!("{} {}", turn.text, "x".repeat(900));
    }
    let newest = chat.turns.last().unwrap().clone();
    filed.chat = Some(chat);
    let fitted = filed.fit();
    assert!(fitted.content().len() <= MAX_CONTENT_BYTES);
    assert_eq!(fitted.notes, [Note::ChatTrimmed]);
    assert_eq!(fitted.chat.as_ref().unwrap().turns.last(), Some(&newest));
    assert!(fitted.check().is_ok());
    // The digest names exactly the previewed chat.
    let a = shared(ShareReason::Shared, 2);
    let mut b = a.clone();
    assert_eq!(chat_digest(&a), chat_digest(&b));
    b.turns[0].text.push('!');
    assert_ne!(chat_digest(&a), chat_digest(&b));
}
