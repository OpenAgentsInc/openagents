//! `playtest` fixtures: reports, session records, and awards, signed with
//! throwaway keys derived from a label. No key is stored anywhere.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::*;
use crate::contracts::{digest_bytes, prepare_closure, validate_instance};
use crate::domain::RelaySigner;
use crate::xp::{bind_quest, parse_award, parse_revocation, quest, revocation};

const AT: u64 = 1_790_000_000;
const ISSUE: &str = "OpenAgentsInc/openagents#9901";

fn signer(label: &str) -> RelaySigner {
    let hex: String = Sha256::digest(label.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    RelaySigner::from_secret_hex(&hex).expect("throwaway key")
}

fn sign_at(signer: &RelaySigner, at: u64, parts: Unsigned) -> Event {
    signer.sign(at, parts.kind, parts.tags, parts.content)
}

fn acceptance(contribution: &str) -> Value {
    let mut value = json!({
        "rule": "playtest", "contribution": contribution,
        "builds": ["1.0.0 (14)", "1.0.0 (15)"], "max_awards": 200,
    });
    match contribution {
        "bug" => value["severities"] = json!(["p2", "p3"]),
        "session" => {
            value["script"] = json!("session-2");
            value["format"] = json!("moderated");
        }
        _ => {}
    }
    value
}

fn spec(id: &str, acceptance: Value, tester_xp: u64) -> Value {
    json!({
        "id": id,
        "version": 1,
        "season": {"id": "playtest-s1", "opens_at": AT - 1_000, "closes_at": AT + 2_419_200},
        "title": "Reproducible bug report, P2 or P3",
        "objective": "Report a bug a triager can reproduce from your steps on a season build.",
        "acceptance": acceptance,
        "reference": null,
        "award": {"tester": tester_xp, "triager": 0},
    })
}

fn signed_quest(referee: &RelaySigner, contribution: &str) -> Event {
    sign_at(
        referee,
        AT,
        quest(&spec(
            &format!("playtest-s1.{contribution}"),
            acceptance(contribution),
            20,
        ))
        .unwrap(),
    )
}

/// The exact bytes of a private report, which only the triage key reads.
const PRIVATE: &str =
    r#"{"schema":"openagents.playtest-report.v1","what":"The ball fell through the floor."}"#;

fn digest() -> String {
    digest_bytes(PRIVATE.as_bytes())
        .trim_start_matches("sha256:")
        .to_string()
}

fn report(tester: &RelaySigner, kind: &str, build: &str, script: Option<&str>, at: u64) -> Event {
    sign_at(
        tester,
        at,
        playtest_report(build, "ios", kind, &digest(), script).unwrap(),
    )
}

fn bug_fields() -> PlaytestAward {
    PlaytestAward {
        issue: Some(ISSUE.into()),
        severity: Some("p2".into()),
        commit: None,
    }
}

fn body(event: &Event) -> Value {
    serde_json::from_str(&event.content).unwrap()
}

fn schema_check(file: &[u8], instance: &Value) {
    let digest = digest_bytes(file);
    let mut documents = BTreeMap::new();
    documents.insert(digest.clone(), file.to_vec());
    let closure = prepare_closure(&documents).expect("schema");
    validate_instance(&closure, &digest, instance).expect("instance matches its schema");
}

fn code<T: std::fmt::Debug>(result: Result<T, ContractError>) -> RefusalCode {
    result.unwrap_err().code
}

#[test]
fn a_bug_award_binds_its_report_issue_severity_and_derived_key() {
    let (referee, tester, triager) = (signer("referee"), signer("tester"), signer("triager"));
    let quest_event = signed_quest(&referee, "bug");
    let parsed_quest = parse_quest(&quest_event).unwrap();
    assert_eq!(parsed_quest.acceptance.rule, "playtest");
    assert_eq!(parsed_quest.acceptance.task, "bug");
    let accepted = parsed_quest.acceptance.playtest.as_ref().unwrap();
    assert_eq!(accepted.max_awards, 200);
    let report = report(&tester, "bug", "1.0.0 (15)", None, AT + 10);
    let award = sign_at(
        &referee,
        AT + 20,
        playtest_award(
            &quest_event,
            &report,
            None,
            triager.pubkey(),
            &bug_fields(),
            AT + 20,
        )
        .unwrap(),
    );
    let parsed = parse_award(&award).unwrap();
    assert_eq!(parsed.rule, "playtest");
    assert_eq!(parsed.key, format!("playtest:playtest-s1:report:{ISSUE}"));
    assert_ne!(parsed.key, parsed.coordinate);
    assert_eq!(parsed.role("tester").unwrap().xp, 20);
    let bound = bind_quest(&parsed, &quest_event).unwrap();
    bind_playtest(&parsed, &bound, &report, None).unwrap();
    schema_check(
        include_bytes!("../../../../../nips/openagents/schemas/xp-quest.v1.json"),
        &body(&quest_event),
    );
    schema_check(
        include_bytes!("../../../../../nips/openagents/schemas/xp-award.v1.json"),
        &body(&award),
    );
    schema_check(
        include_bytes!("../../../../../nips/openagents/schemas/xp-playtest-report.v1.json"),
        &body(&report),
    );
    // The report holds no text: only the digest of the private bytes.
    assert!(!report.content.contains("ball"));
    assert_eq!(parse_playtest_report(&report).unwrap().digest, digest());
}

#[test]
fn the_rule_refuses_contributions_that_do_not_qualify() {
    let (referee, tester, triager) = (signer("referee"), signer("tester"), signer("triager"));
    let quest_event = signed_quest(&referee, "bug");
    let build = |report: &Event, fields: &PlaytestAward| {
        playtest_award(
            &quest_event,
            report,
            None,
            triager.pubkey(),
            fields,
            AT + 50,
        )
    };
    // A build outside the season's list.
    let old = report(&tester, "bug", "1.0.0 (9)", None, AT + 10);
    assert_eq!(code(build(&old, &bug_fields())), RefusalCode::NotAdmitted);
    // An idea isn't a bug report.
    let idea = report(&tester, "idea", "1.0.0 (15)", None, AT + 10);
    assert_eq!(code(build(&idea, &bug_fields())), RefusalCode::NotAdmitted);
    // Filed before the season opened.
    let early = report(&tester, "bug", "1.0.0 (15)", None, AT - 5_000);
    assert_eq!(code(build(&early, &bug_fields())), RefusalCode::NotAdmitted);
    let good = report(&tester, "bug", "1.0.0 (15)", None, AT + 10);
    // A severity the quest doesn't pay for, no severity, and no issue.
    let mut p0 = bug_fields();
    p0.severity = Some("p0".into());
    assert_eq!(code(build(&good, &p0)), RefusalCode::NotAdmitted);
    let mut none = bug_fields();
    none.severity = None;
    assert_eq!(code(build(&good, &none)), RefusalCode::Malformed);
    let mut no_issue = bug_fields();
    no_issue.issue = None;
    assert_eq!(code(build(&good, &no_issue)), RefusalCode::Malformed);
    // The tester can't be the triager or the referee.
    assert_eq!(
        code(playtest_award(
            &quest_event,
            &good,
            None,
            tester.pubkey(),
            &bug_fields(),
            AT + 50
        )),
        RefusalCode::NotAdmitted
    );
    let own = report(&referee, "bug", "1.0.0 (15)", None, AT + 10);
    assert_eq!(code(build(&own, &bug_fields())), RefusalCode::NotAdmitted);
    // Accepted before it was filed.
    assert!(
        playtest_award(
            &quest_event,
            &good,
            None,
            triager.pubkey(),
            &bug_fields(),
            AT + 5
        )
        .is_err()
    );
}

#[test]
fn a_signed_award_with_another_key_is_refused() {
    let (referee, tester, triager) = (signer("referee"), signer("tester"), signer("triager"));
    let quest_event = signed_quest(&referee, "bug");
    let good = report(&tester, "bug", "1.0.0 (15)", None, AT + 10);
    let parts = playtest_award(
        &quest_event,
        &good,
        None,
        triager.pubkey(),
        &bug_fields(),
        AT + 20,
    )
    .unwrap();
    // A referee that keys by the quest coordinate would pay one issue twice
    // across quests; the reader re-derives the key and refuses it.
    let coordinate = crate::xp::coordinate(referee.pubkey(), "playtest-s1.bug@1");
    let forged = parts.content.replace(
        &format!("playtest:playtest-s1:report:{ISSUE}"),
        &format!("playtest:playtest-s1:report:{ISSUE}9"),
    );
    let award = referee.sign(AT + 20, parts.kind, parts.tags.clone(), forged);
    let parsed = parse_award(&award).unwrap();
    assert_eq!(
        code(bind_quest(&parsed, &quest_event)),
        RefusalCode::IdentityMismatch
    );
    let as_coordinate = parts.content.replace(
        &format!("playtest:playtest-s1:report:{ISSUE}\""),
        &format!("{coordinate}\""),
    );
    let award = referee.sign(AT + 20, parts.kind, parts.tags, as_coordinate);
    assert_eq!(code(parse_award(&award)), RefusalCode::Malformed);
}

#[test]
fn feedback_and_bug_quests_share_one_key_per_issue() {
    let (referee, tester, triager) = (signer("referee"), signer("tester"), signer("triager"));
    let bug_quest = signed_quest(&referee, "bug");
    let feedback_quest = signed_quest(&referee, "feedback");
    let good = report(&tester, "bug", "1.0.0 (15)", None, AT + 10);
    let bug = parse_award(&sign_at(
        &referee,
        AT + 20,
        playtest_award(
            &bug_quest,
            &good,
            None,
            triager.pubkey(),
            &bug_fields(),
            AT + 20,
        )
        .unwrap(),
    ))
    .unwrap();
    let fields = PlaytestAward {
        issue: Some(ISSUE.into()),
        ..PlaytestAward::default()
    };
    let feedback = parse_award(&sign_at(
        &referee,
        AT + 30,
        playtest_award(
            &feedback_quest,
            &good,
            None,
            triager.pubkey(),
            &fields,
            AT + 30,
        )
        .unwrap(),
    ))
    .unwrap();
    assert_eq!(bug.key, feedback.key);
    // A verified fix of the same issue has a key of its own.
    let verified_quest = signed_quest(&referee, "verified-fix");
    let verified = report(&tester, "verified", "1.0.0 (15)", None, AT + 40);
    let fix = parse_award(&sign_at(
        &referee,
        AT + 50,
        playtest_award(
            &verified_quest,
            &verified,
            None,
            triager.pubkey(),
            &fields,
            AT + 50,
        )
        .unwrap(),
    ))
    .unwrap();
    assert_eq!(fix.key, format!("playtest:playtest-s1:verified:{ISSUE}"));
}

#[test]
fn a_moderated_session_needs_the_moderators_record() {
    let (referee, tester, moderator) = (signer("referee"), signer("tester"), signer("moderator"));
    let quest_event = signed_quest(&referee, "session");
    let report = report(
        &tester,
        "session",
        "1.0.0 (15)",
        Some("session-2"),
        AT + 100,
    );
    let record = sign_at(
        &moderator,
        AT + 90,
        playtest_session(
            "session-2",
            "moderated",
            "1.0.0 (15)",
            tester.pubkey(),
            AT + 60,
        )
        .unwrap(),
    );
    schema_check(
        include_bytes!("../../../../../nips/openagents/schemas/xp-playtest-session.v1.json"),
        &body(&record),
    );
    let none = PlaytestAward::default();
    // Without the record, the award isn't built.
    assert_eq!(
        code(playtest_award(
            &quest_event,
            &report,
            None,
            moderator.pubkey(),
            &none,
            AT + 200
        )),
        RefusalCode::ContentUnavailable
    );
    // The triager must be the moderator who signed the record.
    assert_eq!(
        code(playtest_award(
            &quest_event,
            &report,
            Some(&record),
            signer("someone").pubkey(),
            &none,
            AT + 200
        )),
        RefusalCode::IdentityMismatch
    );
    let award = sign_at(
        &referee,
        AT + 200,
        playtest_award(
            &quest_event,
            &report,
            Some(&record),
            moderator.pubkey(),
            &none,
            AT + 200,
        )
        .unwrap(),
    );
    let parsed = parse_award(&award).unwrap();
    assert_eq!(parsed.evidence.len(), 2);
    assert_eq!(
        parsed.key,
        format!(
            "playtest:playtest-s1:playtest-s1.session@1:{}",
            tester.pubkey()
        )
    );
    let bound = bind_quest(&parsed, &quest_event).unwrap();
    bind_playtest(&parsed, &bound, &report, Some(&record)).unwrap();
    // A record for another tester, or signed by the tester, doesn't count.
    let other = sign_at(
        &moderator,
        AT + 90,
        playtest_session(
            "session-2",
            "moderated",
            "1.0.0 (15)",
            signer("x").pubkey(),
            AT + 60,
        )
        .unwrap(),
    );
    assert_eq!(
        code(check_playtest(&bound, &report, Some(&other))),
        RefusalCode::NotAdmitted
    );
    let own = sign_at(
        &tester,
        AT + 90,
        playtest_session(
            "session-2",
            "moderated",
            "1.0.0 (15)",
            tester.pubkey(),
            AT + 60,
        )
        .unwrap(),
    );
    assert_eq!(
        code(check_playtest(&bound, &report, Some(&own))),
        RefusalCode::NotAdmitted
    );
    // A report about another script doesn't complete this one.
    let wrong = self::report(
        &tester,
        "session",
        "1.0.0 (15)",
        Some("session-1"),
        AT + 100,
    );
    assert_eq!(
        code(check_playtest(&bound, &wrong, Some(&record))),
        RefusalCode::NotAdmitted
    );
}

#[test]
fn a_playtest_award_revokes_with_its_quest_in_the_a_tag() {
    let (referee, tester, triager) = (signer("referee"), signer("tester"), signer("triager"));
    let quest_event = signed_quest(&referee, "bug");
    let good = report(&tester, "bug", "1.0.0 (15)", None, AT + 10);
    let award = sign_at(
        &referee,
        AT + 20,
        playtest_award(
            &quest_event,
            &good,
            None,
            triager.pubkey(),
            &bug_fields(),
            AT + 20,
        )
        .unwrap(),
    );
    let revoked = sign_at(
        &referee,
        AT + 30,
        revocation(&award, "The steps reproduce a different bug.").unwrap(),
    );
    let parsed = parse_revocation(&revoked).unwrap();
    assert_eq!(parsed.award.id, award.id);
    assert!(parsed.key.starts_with(KEY_PREFIX));
    assert_eq!(
        revoked.tag_values("a").next(),
        Some(crate::xp::coordinate(referee.pubkey(), "playtest-s1.bug@1").as_str())
    );
}

#[test]
fn bad_playtest_quests_and_reports_are_refused() {
    let with = |contribution: &str, pointer: &str, value: Value| {
        let mut accepted = acceptance(contribution);
        if let Some(slot) = accepted.pointer_mut(pointer) {
            *slot = value;
        } else {
            accepted[pointer.trim_start_matches('/')] = value;
        }
        quest(&spec("playtest-s1.x", accepted, 10))
    };
    assert_eq!(
        code(with("bug", "/contribution", json!("joining"))),
        RefusalCode::UnsupportedFeature
    );
    assert_eq!(
        code(with("bug", "/max_awards", json!(0))),
        RefusalCode::Malformed
    );
    assert_eq!(
        code(with("bug", "/builds", json!([]))),
        RefusalCode::Malformed
    );
    assert_eq!(
        code(with("feedback", "/severities", json!(["p1"]))),
        RefusalCode::UnsupportedFeature
    );
    assert_eq!(
        code(with("session", "/format", json!("solo"))),
        RefusalCode::UnsupportedFeature
    );
    // Roles are the rule's: tester and triager.
    let mut roles = spec("playtest-s1.x", acceptance("bug"), 10);
    roles["award"] = json!({"author": 6, "runner": 4});
    assert!(quest(&roles).is_err());
    // A report's kind, platform, and digest have grammars; it has no text.
    assert!(playtest_report("1.0.0 (15)", "ios", "rant", &digest(), None).is_err());
    assert!(playtest_report("1.0.0 (15)", "web", "bug", &digest(), None).is_err());
    // The desktop's platforms and Give feedback's kind (#10127) are in it.
    assert!(playtest_report("1.0.0 (0)", "macos", "comment", &digest(), None).is_ok());
    assert!(playtest_report("1.0.0 (15)", "ios", "bug", "nothex", None).is_err());
    assert!(valid_issue(ISSUE) && !valid_issue("openagents#1") && !valid_issue("a/b#01"));
}
