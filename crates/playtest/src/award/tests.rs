use nostr::domain::RelaySigner;
use secp256k1::{Secp256k1, SecretKey};
use serde_json::{Value, json};

use super::*;
use crate::report::{self, Context, Kind, Platform, Randomness, Report};
use crate::session::{Route, Tab};
use crate::triage::{Contribution, Entry, Log, Severity};

const AT: u64 = 1_790_000_000;

fn key(byte: u8) -> SecretKey {
    SecretKey::from_byte_array([byte; 32]).unwrap()
}

fn hex(secret: &SecretKey) -> String {
    secret.x_only_public_key(&Secp256k1::new()).0.to_string()
}

fn signer(secret: &SecretKey) -> RelaySigner {
    RelaySigner::from_secret_hex(&secret.display_secret().to_string()).unwrap()
}

fn sign(secret: &SecretKey, at: u64, parts: Unsigned) -> Event {
    signer(secret).sign(at, parts.kind, parts.tags, parts.content)
}

fn quest(referee: &SecretKey, contribution: &str, max_awards: u64) -> Event {
    let mut acceptance = json!({
        "rule": "playtest", "contribution": contribution,
        "builds": ["1.0.0 (15)", "1.0.0 (16)"], "max_awards": max_awards,
    });
    if contribution == "bug" {
        acceptance["severities"] = json!(["p2", "p3"]);
    }
    let spec: Value = json!({
        "id": format!("playtest-s1.{contribution}"),
        "version": 1,
        "season": {"id": "playtest-s1", "opens_at": AT - 1_000, "closes_at": AT + 2_419_200},
        "title": "Reproducible bug report",
        "objective": "Report a bug a triager can reproduce.",
        "acceptance": acceptance,
        "reference": null,
        "award": {"tester": 20, "triager": 0},
    });
    sign(referee, AT - 500, xp::quest(&spec).unwrap())
}

/// A tester files a report; the triage key opens it; the triager files it
/// as a P2 bug on #9950. Returns the acceptance and the tester's public
/// record.
fn accepted(tester: &SecretKey, triage: &SecretKey, triager: &str) -> (Acceptance, Event) {
    let filed = Report {
        schema: report::SCHEMA.into(),
        context: Context {
            app_version: "1.0.0".into(),
            build: "16".into(),
            platform: Platform::Ios,
            device: "iPhone17,1".into(),
            os_version: "26.0".into(),
            tab: Tab::Verse,
            route: Route::Gym,
            at: AT,
        },
        kind: Kind::Bug,
        happened: "The ball went through the wall.".into(),
        expected: "It bounces.".into(),
        steps: "Push it hard.".into(),
        quote: false,
        task: None,
        session: None,
        screenshot: None,
        notes: vec![],
    };
    let random = Randomness {
        wrapper: key(9),
        seal_nonce: [1; 32],
        wrap_nonce: [2; 32],
        seal_earlier: 60,
        wrap_earlier: 120,
    };
    let triage_public = triage.x_only_public_key(&Secp256k1::new()).0;
    let sealed = report::wrap(&filed, tester, &triage_public, &random).unwrap();
    let opened = report::open(&sealed.wrap, triage).unwrap();
    let mut log = Log::default();
    for entry in [
        Entry::received(&opened, AT + 10),
        Entry::Filed {
            at: AT + 100,
            code: opened.code.clone(),
            issue: 9950,
            contribution: Contribution::Bug,
            severity: Some(Severity::P2),
            triager: Some(triager.to_owned()),
        },
    ] {
        log.admit(&entry).unwrap();
        log.entries.push(entry);
    }
    (log.acceptances().remove(0), sealed.public)
}

#[test]
fn an_award_from_a_triage_log_acceptance_passes_the_playtest_rule() {
    let (referee_key, tester, triage, triager) = (key(1), key(2), key(3), key(4));
    let (acceptance, public) = accepted(&tester, &triage, &hex(&triager));
    let quest = quest(&referee_key, "bug", 200);
    // The public record is found by the tester and the digest.
    let other = sign(
        &tester,
        AT,
        xp::playtest::playtest_report("1.0.0 (16)", "ios", "bug", &"0".repeat(64), None).unwrap(),
    );
    let events = [other, public.clone()];
    assert_eq!(find_report(&acceptance, &events).unwrap().id, public.id);

    referee(&hex(&referee_key), Some(&hex(&referee_key))).unwrap();
    let plan = plan(
        &acceptance,
        &quest,
        &public,
        None,
        "OpenAgentsInc/openagents",
        None,
    )
    .unwrap();
    assert_eq!(
        plan.key,
        "playtest:playtest-s1:report:OpenAgentsInc/openagents#9950"
    );
    admit(&plan, &[]).unwrap();
    let award = sign(&referee_key, AT + 200, plan.unsigned.clone());
    let parsed = xp::parse_award(&award).unwrap();
    xp::bind_quest(&parsed, &quest).unwrap();
    assert_eq!(parsed.awardees[0].pubkey, hex(&tester));
    assert_eq!(parsed.awardees[0].xp, 20);
    assert_eq!(parsed.awardees[1].pubkey, hex(&triager));
    assert_eq!(parsed.accepted_at, AT + 100);
    let fields = parsed.playtest.unwrap();
    assert_eq!(
        fields.issue.as_deref(),
        Some("OpenAgentsInc/openagents#9950")
    );
    assert_eq!(fields.severity.as_deref(), Some("p2"));

    // The same key pays once; a revoked award frees it.
    assert!(
        admit(&plan, std::slice::from_ref(&award))
            .unwrap_err()
            .contains("pays once")
    );
    let revocation = sign(
        &referee_key,
        AT + 300,
        xp::revocation(&award, "Issued in error.").unwrap(),
    );
    admit(&plan, &[award.clone(), revocation]).unwrap();
    // A quest version out of max_awards takes no more.
    let full = super::plan(
        &acceptance,
        &quest_with_max(&referee_key, 1),
        &public,
        None,
        "OpenAgentsInc/openagents",
        None,
    )
    .unwrap();
    let first = sign(&referee_key, AT + 200, full.unsigned.clone());
    let mut other_key = full.clone();
    other_key.key = "playtest:playtest-s1:report:OpenAgentsInc/openagents#1".into();
    assert!(
        admit(&other_key, &[first])
            .unwrap_err()
            .contains("max_awards")
    );
}

fn quest_with_max(referee: &SecretKey, max: u64) -> Event {
    quest(referee, "bug", max)
}

#[test]
fn no_award_is_signed_without_the_playtest_referee_key() {
    let referee_key = key(1);
    assert_eq!(referee(&hex(&referee_key), None).unwrap_err(), NO_REFEREE);
    assert!(referee(&hex(&referee_key), Some(&hex(&key(5)))).is_err());
}

#[test]
fn a_mismatched_acceptance_is_refused() {
    let (referee_key, tester, triage, triager) = (key(1), key(2), key(3), key(4));
    let (acceptance, public) = accepted(&tester, &triage, &hex(&triager));
    let repo = "OpenAgentsInc/openagents";
    // A quest for another contribution.
    let feedback = quest(&referee_key, "feedback", 10);
    assert!(
        plan(&acceptance, &feedback, &public, None, repo, None)
            .unwrap_err()
            .contains("pays for")
    );
    let quest = quest(&referee_key, "bug", 10);
    // A record signed by someone else, or for another private report.
    let forged = sign(
        &key(7),
        AT,
        xp::playtest::playtest_report(
            "1.0.0 (16)",
            "ios",
            "bug",
            acceptance.digest.as_deref().unwrap(),
            None,
        )
        .unwrap(),
    );
    assert!(
        plan(&acceptance, &quest, &forged, None, repo, None)
            .unwrap_err()
            .contains("accepted tester")
    );
    let other = sign(
        &tester,
        AT,
        xp::playtest::playtest_report("1.0.0 (16)", "ios", "bug", &"1".repeat(64), None).unwrap(),
    );
    assert!(
        plan(&acceptance, &quest, &other, None, repo, None)
            .unwrap_err()
            .contains("different private report")
    );
    // No triager recorded.
    let mut unknown = acceptance.clone();
    unknown.triager = None;
    assert!(plan(&unknown, &quest, &public, None, repo, None).is_err());
    // The tester can't be the triager: the rule refuses it.
    let mut own = acceptance;
    own.triager = Some(hex(&tester));
    assert!(
        plan(&own, &quest, &public, None, repo, None)
            .unwrap_err()
            .contains("playtest rule")
    );
}
