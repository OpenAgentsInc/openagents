use super::*;
use crate::report::{self, Context, Randomness, Report, SCHEMA, Screenshot};
use secp256k1::{Secp256k1, SecretKey};

fn key(byte: u8) -> SecretKey {
    SecretKey::from_byte_array([byte; 32]).expect("key")
}

fn hex(secret: &SecretKey) -> String {
    secret.x_only_public_key(&Secp256k1::new()).0.to_string()
}

fn sample(happened: &str, quote: bool) -> Report {
    Report {
        schema: SCHEMA.into(),
        context: Context {
            app_version: "1.0.0".into(),
            build: "15".into(),
            platform: Platform::Ios,
            device: "iPhone17,1".into(),
            os_version: "26.0".into(),
            tab: Tab::Verse,
            route: Route::Gym,
            at: 1_790_000_000,
        },
        kind: Kind::Bug,
        happened: happened.into(),
        expected: "It bounces.".into(),
        steps: "Push the ball.\nHard.".into(),
        quote,
        task: None,
        session: None,
        screenshot: Some(Screenshot {
            jpeg_base64: "/9j/AAAA".into(),
            width: 10,
            height: 10,
        }),
        notes: vec![],
        chat: None,
    }
}

fn opened(report: &Report, tester: u8, wrapper: u8) -> Opened {
    let triage = key(4);
    let sealed = report::wrap(
        report,
        &key(tester),
        &triage.x_only_public_key(&Secp256k1::new()).0,
        &Randomness {
            wrapper: key(wrapper),
            seal_nonce: [1; 32],
            wrap_nonce: [wrapper; 32],
            seal_earlier: 0,
            wrap_earlier: 0,
        },
    )
    .expect("sealed");
    report::open(&sealed.wrap, &triage).expect("opened")
}

#[test]
fn deduplication_is_by_exact_identity_only() {
    let first = opened(&sample("The ball went through the wall.", true), 3, 9);
    // The same report delivered twice under different wraps is one report.
    let again = opened(&sample("The ball went through the wall.", true), 3, 10);
    assert_ne!(first.wrap_id, again.wrap_id);
    // A different report about the same thing is kept: the triager decides.
    let similar = opened(&sample("The ball went through the wall!", true), 5, 11);
    let kept = fresh(vec![first.clone(), again, similar.clone()], &Log::default());
    assert_eq!(kept.len(), 2);
    let mut log = Log::default();
    log.entries.push(Entry::received(&first, 1));
    let kept = fresh(vec![first, similar], &log);
    assert_eq!(kept.len(), 1);
}

#[test]
fn a_draft_quotes_the_tester_only_with_permission_and_never_embeds_the_screenshot() {
    let quoted = draft(&opened(
        &sample("The ball went through the wall.", true),
        3,
        9,
    ));
    assert_eq!(quoted.title, "grid: The ball went through the wall.");
    assert!(quoted.body.contains("> The ball went through the wall."));
    assert!(quoted.body.contains("> Hard."));
    assert!(quoted.body.contains("1.0.0 (15) on iOS 26.0"));
    assert!(quoted.body.contains("attached to the private report"));
    assert!(!quoted.body.contains("/9j/"));
    assert_eq!(quoted.labels, ["playtest", "build:1.0.0-15", "area:grid"]);

    let private = draft(&opened(
        &sample("My friend Alice's wallet broke.", false),
        3,
        9,
    ));
    assert!(!private.title.contains("Alice") && !private.body.contains("Alice"));
    assert!(!private.body.contains("It bounces"));
    assert!(private.body.contains(PARAPHRASE));
    assert_eq!(private.title, "grid: bug on verse/gym (write a title)");
    // The Markdown round-trips so a person can edit it.
    let back =
        Draft::from_markdown(&private.code, private.labels.clone(), &private.markdown()).unwrap();
    assert_eq!(back, private);
    assert!(Draft::from_markdown("PT-1", vec![], "no title").is_err());
}

#[test]
fn the_log_round_trips_and_answers_which_acceptances_back_awards() {
    let bug = opened(&sample("The ball went through the wall.", true), 3, 9);
    let dup = opened(&sample("Ball clips the wall.", true), 5, 10);
    let mut log = Log::default();
    let push = |log: &mut Log, entry: Entry| {
        log.admit(&entry).expect("admitted");
        log.entries.push(entry);
    };
    push(&mut log, Entry::received(&bug, 100));
    push(&mut log, Entry::received(&dup, 101));
    assert_eq!(log.pending(), [bug.code.as_str(), dup.code.as_str()]);
    let filed = Entry::Filed {
        at: 200,
        code: bug.code.clone(),
        issue: 9_900,
        contribution: Contribution::Bug,
        severity: Some(Severity::P2),
        triager: Some(hex(&key(7))),
    };
    push(&mut log, filed);
    // A second accepted reporter for the same issue is refused.
    let second = Entry::Filed {
        at: 201,
        code: dup.code.clone(),
        issue: 9_900,
        contribution: Contribution::Bug,
        severity: None,
        triager: None,
    };
    assert!(log.admit(&second).unwrap_err().contains("duplicate"));
    push(
        &mut log,
        Entry::Decided {
            at: 202,
            code: dup.code.clone(),
            decision: Decision::Duplicate,
            reason: "Same as the first report.".into(),
            issue: Some(9_900),
        },
    );
    assert!(log.pending().is_empty());
    // A triager can't accept their own report; a decided report can't be refiled.
    assert!(
        log.admit(&Entry::Decided {
            at: 1,
            code: dup.code.clone(),
            decision: Decision::Declined,
            reason: "x".into(),
            issue: None
        })
        .is_err()
    );
    push(
        &mut log,
        Entry::Verified {
            at: 300,
            issue: 9_900,
            fix_build: "1.0.0 (16)".into(),
            verified: Verified::Yes,
        },
    );
    assert!(
        log.admit(&Entry::Verified {
            at: 1,
            issue: 1,
            fix_build: "x".into(),
            verified: Verified::Yes
        })
        .is_err()
    );
    push(
        &mut log,
        Entry::Session {
            at: 400,
            tester: hex(&key(5)),
            script: "session-2".into(),
            format: Format::Moderated,
            build: "1.0.0 (15)".into(),
            code: None,
            moderator: Some(hex(&key(7))),
        },
    );
    let repeat = Entry::Session {
        at: 401,
        tester: hex(&key(5)),
        script: "session-2".into(),
        format: Format::Moderated,
        build: "1.0.0 (16)".into(),
        code: None,
        moderator: None,
    };
    assert!(log.admit(&repeat).is_err());

    let text: String = log.entries.iter().map(Log::line).collect();
    assert_eq!(text.lines().count(), log.entries.len());
    let back = Log::parse(&text).unwrap();
    assert_eq!(back, log);

    let accepted = back.acceptances();
    let kinds: Vec<(&str, u64)> = accepted
        .iter()
        .map(|a| (a.contribution.as_str(), a.accepted_at))
        .collect();
    assert_eq!(
        kinds,
        [("bug", 200), ("verified-fix", 300), ("session", 400)]
    );
    assert_eq!(accepted[0].tester, hex(&key(3)));
    assert_eq!(accepted[0].severity, Some(Severity::P2));
    assert_eq!(accepted[0].verified, Some(Verified::Yes));
    assert_eq!(accepted[1].build, "1.0.0 (16)");
    assert_eq!(accepted[2].tester, hex(&key(5)));
    // The duplicate reporter earns nothing.
    assert!(
        accepted
            .iter()
            .all(|a| a.code.as_deref() != Some(dup.code.as_str()))
    );
}

#[test]
fn a_bad_log_line_is_named() {
    let error = Log::parse("\n{\"event\":\"filed\"}\n").unwrap_err();
    assert!(error.starts_with("triage log line 2"), "{error}");
}
