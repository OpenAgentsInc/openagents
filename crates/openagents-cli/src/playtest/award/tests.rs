use playtest::report::{self, Context, Kind, Platform, Randomness, Report};
use playtest::session::{Route, Tab};
use playtest::triage::{Contribution, Entry, Severity};
use secp256k1::{Secp256k1, SecretKey};

use super::*;

const AT: u64 = 1_790_000_000;

fn key(byte: u8) -> SecretKey {
    SecretKey::from_byte_array([byte; 32]).unwrap()
}

fn signer(secret: &SecretKey) -> RelaySigner {
    RelaySigner::from_secret_hex(&secret.display_secret().to_string()).unwrap()
}

fn hex(secret: &SecretKey) -> String {
    secret.x_only_public_key(&Secp256k1::new()).0.to_string()
}

fn quest(referee: &SecretKey) -> Event {
    let spec = json!({
        "id": "playtest-s1.bug", "version": 1,
        "season": {"id": "playtest-s1", "opens_at": AT - 1_000, "closes_at": AT + 2_419_200},
        "title": "Reproducible bug report", "objective": "Report a bug a triager can reproduce.",
        "acceptance": {"rule": "playtest", "contribution": "bug", "builds": ["1.0.0 (16)"],
                       "severities": ["p2"], "max_awards": 50},
        "reference": null, "award": {"tester": 20, "triager": 0},
    });
    let parts = nostr::xp::quest(&spec).unwrap();
    signer(referee).sign(AT - 500, parts.kind, parts.tags, parts.content)
}

/// Writes a triage log with one report filed as a P2 bug on #9950, and
/// returns the tester's public record.
fn filed(home: &Path, tester: &SecretKey, triager: &str) -> Event {
    let report = Report {
        schema: report::SCHEMA.into(),
        context: Context {
            app_version: "1.0.0".into(),
            build: "16".into(),
            platform: Platform::Android,
            device: "Pixel 9".into(),
            os_version: "16".into(),
            tab: Tab::Verse,
            route: Route::Gym,
            at: AT,
        },
        kind: Kind::Bug,
        happened: "The ball went through the wall.".into(),
        expected: String::new(),
        steps: String::new(),
        quote: false,
        task: None,
        session: None,
        screenshot: None,
        notes: vec![],
        chat: None,
    };
    let triage = key(3);
    let random = Randomness {
        wrapper: key(9),
        seal_nonce: [1; 32],
        wrap_nonce: [2; 32],
        seal_earlier: 1,
        wrap_earlier: 2,
    };
    let sealed = report::wrap(
        &report,
        tester,
        &triage.x_only_public_key(&Secp256k1::new()).0,
        &random,
    )
    .unwrap();
    let opened = report::open(&sealed.wrap, &triage).unwrap();
    let mut log = load(home).unwrap();
    super::super::append(home, &mut log, Entry::received(&opened, AT + 10)).unwrap();
    super::super::append(
        home,
        &mut log,
        Entry::Filed {
            at: AT + 100,
            code: opened.code,
            issue: 9950,
            contribution: Contribution::Bug,
            severity: Some(Severity::P2),
            triager: Some(triager.to_owned()),
        },
    )
    .unwrap();
    sealed.public
}

fn request() -> Request {
    Request {
        code: None,
        issue: Some(9950),
        verified: false,
        script: None,
        tester: None,
        quest: "playtest-s1.bug@1".into(),
        session: None,
        triager: None,
        repo: "OpenAgentsInc/openagents".into(),
        commit: None,
    }
}

#[test]
fn a_filed_report_becomes_a_signed_award_with_a_test_referee_key() {
    let home = tempfile::tempdir().unwrap();
    let (referee, tester, triager) = (key(1), key(2), key(4));
    let public = filed(home.path(), &tester, &hex(&triager));
    let quest = quest(&referee);
    let mut filters = Vec::new();
    let mut fetch = |filter: Value| -> Result<Vec<Event>, String> {
        filters.push(filter.clone());
        let kinds = filter["kinds"][0].as_u64().unwrap_or_default();
        Ok(match u16::try_from(kinds).unwrap() {
            nostr::kinds::XP_QUEST => vec![quest.clone()],
            nostr::kinds::XP_PLAYTEST_REPORT => vec![public.clone()],
            _ => vec![],
        })
    };
    let (award, award_key) = sign(
        home.path(),
        &request(),
        &signer(&referee),
        Some(&hex(&referee)),
        &mut fetch,
    )
    .unwrap();
    assert_eq!(
        award_key,
        "playtest:playtest-s1:report:OpenAgentsInc/openagents#9950"
    );
    assert_eq!(award.pubkey, hex(&referee));
    let parsed = nostr::xp::parse_award(&award).unwrap();
    nostr::xp::bind_quest(&parsed, &quest).unwrap();
    assert_eq!(parsed.awardees[0].pubkey, hex(&tester));
    assert_eq!(parsed.evidence[0].id, public.id);
    // It asked for the tester's reports and the referee's own awards.
    assert!(filters.iter().any(|f| f["authors"][0] == hex(&tester)));
    assert!(
        filters
            .iter()
            .any(|f| f["kinds"][0] == nostr::kinds::XP_AWARD)
    );

    // Once that award is on the relay, the contribution doesn't pay again.
    let mut again = |filter: Value| -> Result<Vec<Event>, String> {
        let kinds = filter["kinds"][0].as_u64().unwrap_or_default();
        Ok(match u16::try_from(kinds).unwrap() {
            nostr::kinds::XP_QUEST => vec![quest.clone()],
            nostr::kinds::XP_PLAYTEST_REPORT => vec![public.clone()],
            nostr::kinds::XP_AWARD => vec![award.clone()],
            _ => vec![],
        })
    };
    let refused = sign(
        home.path(),
        &request(),
        &signer(&referee),
        Some(&hex(&referee)),
        &mut again,
    );
    assert!(refused.unwrap_err().contains("pays once"));
}

#[test]
fn without_the_playtest_referee_key_nothing_is_read_or_signed() {
    let home = tempfile::tempdir().unwrap();
    let (referee, tester) = (key(1), key(2));
    let _ = filed(home.path(), &tester, &hex(&key(4)));
    let mut calls = 0;
    let mut fetch = |_: Value| -> Result<Vec<Event>, String> {
        calls += 1;
        Ok(vec![])
    };
    let refused = sign(home.path(), &request(), &signer(&referee), None, &mut fetch);
    assert_eq!(refused.unwrap_err(), award::NO_REFEREE);
    // A key that isn't the trusted referee is refused too.
    let wrong = sign(
        home.path(),
        &request(),
        &signer(&key(7)),
        Some(&hex(&referee)),
        &mut fetch,
    );
    assert!(wrong.is_err());
    assert_eq!(calls, 0);
}

#[test]
fn the_acceptance_is_named_exactly() {
    let home = tempfile::tempdir().unwrap();
    let _ = filed(home.path(), &key(2), &hex(&key(4)));
    let accepted = load(home.path()).unwrap().acceptances();
    let mut by_issue = request();
    assert_eq!(
        select(accepted.clone(), &by_issue).unwrap().issue,
        Some(9950)
    );
    by_issue.issue = Some(1);
    assert!(select(accepted.clone(), &by_issue).is_err());
    let mut verified = request();
    verified.verified = true;
    assert!(select(accepted, &verified).is_err(), "not verified yet");
}
