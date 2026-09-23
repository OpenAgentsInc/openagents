use std::time::Duration;

use serde_json::json;

use super::study::{self, Arm, Options, cell, preserve_one};
use super::*;
use crate::minitask::{find, process};

fn scratch(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "coder-one-repair-{label}-{}-{}",
        std::process::id(),
        atif::now_ms()
    ))
}

#[tokio::test]
async fn a_packet_brief_carries_the_observations_and_no_protected_verifier_text() {
    if process::python().is_none() {
        return;
    }
    let traces = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/traces");
    let recovered = checks::recover::recover_tree(&traces, "coder-one-jevprobe3-luna").unwrap();
    let mut briefs = 0;
    for one in recovered
        .iter()
        .filter(|r| r.task == "log-summary-date-ranges")
    {
        let Some(input) = &one.input else { continue };
        let report = checks::check(input, &Recorder::default(), &scratch("brief")).await;
        let found = gaps(&report, None);
        assert!(!found.is_empty(), "{} has no gap", one.trial);
        let brief = packet_brief(&input.task, &input.candidate, &report, &found);
        assert!(!one.protected_names.is_empty());
        brief.check_clean(&one.protected_names).unwrap();
        assert!(brief.text.contains(PACKET_MARK));
        assert!(brief.text.contains("data.message-severity"));
        assert!(
            brief
                .text
                .contains("The severity levels to count are exactly")
        );
        assert!(!brief.text.contains("coder-one-checks-run"));
        assert_eq!(brief.kind, BriefKind::Packet);
        briefs += 1;
    }
    assert_eq!(briefs, 3);
    // A protected string in the brief is refused.
    let brief = plain_brief(
        &TaskText {
            title: "t".to_string(),
            instruction: "Pass test_summary_structure_and_counts.".to_string(),
        },
        &checks::Candidate {
            label: "c".to_string(),
            origin: "synthetic".to_string(),
            files: std::collections::BTreeMap::new(),
            programs: Vec::new(),
            provided: std::collections::BTreeMap::new(),
        },
    );
    assert!(!brief.text.contains(PACKET_MARK));
    assert!(
        brief
            .check_clean(&["test_summary_structure_and_counts".to_string()])
            .is_err()
    );
}

#[test]
fn profiles_parse_scripted_names_and_cli_models() {
    assert_eq!(
        Profile::parse("fix-if-packet").unwrap(),
        Profile::Scripted("fix-if-packet".to_string())
    );
    assert_eq!(
        Profile::parse("codex:gpt-6-luna").unwrap().word(),
        "codex:gpt-6-luna"
    );
    assert!(Profile::parse("mend").is_err());
    for task in crate::minitask::CATALOG {
        for profile in scripts::PROFILES {
            assert!(
                scripts::script(task, profile).is_some(),
                "{} {profile}",
                task.id
            );
        }
    }
}

#[tokio::test]
async fn an_episode_repairs_once_in_a_fresh_session_and_rechecks() {
    if process::python().is_none() {
        return;
    }
    let out = scratch("episode");
    let ran = crate::minitask::run::run(crate::minitask::run::Options {
        task: find("log-severity").unwrap(),
        executor: crate::minitask::run::ExecutorChoice::Scripted {
            variant: "bad".to_string(),
            script: None,
        },
        out: out.clone(),
        jev: None,
        speed: 0.0,
        deadline: Duration::from_secs(60),
        controls: crate::session::Controls::default(),
        checks: true,
        brief: None,
        monitor: None,
        repair: Some(crate::minitask::run::Repair {
            profile: Profile::parse("fix-if-packet").unwrap(),
            policy: Policy {
                kind: BriefKind::Packet,
                trigger: Trigger::Detected,
                allowance: Duration::from_secs(60),
            },
        }),
    })
    .await
    .unwrap();
    assert_eq!(ran.grade.verdict, "passed", "{}", ran.grade.detail);
    let repair = &ran.manifest["repair"];
    assert_eq!(repair["ran"], json!(true));
    assert_eq!(repair["changed"], json!(true));
    assert_eq!(repair["session"]["fresh"], json!(true));
    assert_eq!(repair["session"]["resumes"], Value::Null);
    let first = ran.manifest["session"]["session_id"].as_str().unwrap();
    let second = repair["session"]["session_id"].as_str().unwrap();
    assert_ne!(first, second, "a repair is a new session");
    assert_eq!(repair["session"]["previous_session"], json!(first));
    assert_eq!(repair["recheck"]["summary"]["packets"], json!(0));
    assert!(ran.dir.join(FILE).is_file());
    assert!(ran.dir.join(RECHECK_FILE).is_file());
    assert!(ran.dir.join("artifacts/repair-1.brief.md").is_file());
    assert!(ran.dir.join("artifacts/delegate-2.stream.jsonl").is_file());
    // The timeline: verify.repair under the episode, with the fresh
    // session and the recheck as its children, before the grader.
    let log = atif::log::read_whole(&ran.dir.join(crate::episode::INVOCATION_LOG)).unwrap();
    let invocations = crate::record::invocations(&log.steps);
    let repair_id = invocations
        .iter()
        .find(|i| i.component == "verify.repair")
        .map(|i| i.id.clone())
        .unwrap();
    let children: Vec<(&str, Option<&str>)> = invocations
        .iter()
        .filter(|i| i.parent.as_deref() == Some(repair_id.as_str()))
        .map(|i| (i.component.as_str(), i.name.as_deref()))
        .collect();
    assert!(
        children
            .iter()
            .any(|(c, n)| *c == "exec.session" && n.is_some_and(|n| n.contains("fresh session"))),
        "{children:?}"
    );
    assert!(
        children.iter().any(|(c, _)| *c == "verify.checks"),
        "{children:?}"
    );
    let order: Vec<&str> = invocations
        .iter()
        .map(|i| i.component.as_str())
        .filter(|c| matches!(*c, "verify.checks" | "verify.repair" | "task.grade"))
        .collect();
    assert_eq!(
        order,
        [
            "verify.checks",
            "verify.repair",
            "verify.checks",
            "task.grade"
        ]
    );
    let _ = std::fs::remove_dir_all(out);
}

#[tokio::test]
async fn a_spent_deadline_leaves_no_time_for_a_repair() {
    if process::python().is_none() {
        return;
    }
    let out = scratch("deadline");
    let task = find("log-severity").unwrap();
    let preserved = preserve_one(&task, "bad", &out).await.unwrap();
    let work = preserved.dir.join("work");
    let input = checks::workspace_input(&task, &work);
    let recorder = Recorder::default();
    let deadline = Deadline::new(Some(Duration::from_millis(300)), Duration::ZERO);
    let subject = checks::Subject::mini(&task);
    let place = Place {
        task: Some(&task),
        subject: &subject,
        work: &work,
        dir: &preserved.dir,
        artifacts: &preserved.dir.join("artifacts"),
        recorder: &recorder,
        deadline: &deadline,
        jev: None,
        previous_session: None,
        support_params: None,
    };
    let repaired = attempt::<crate::scripted::Scripted>(
        &place,
        (&input, &preserved.report),
        None,
        Policy {
            kind: BriefKind::Packet,
            trigger: Trigger::Detected,
            allowance: Duration::from_secs(60),
        },
        |_| panic!("no executor starts without time"),
    )
    .await
    .unwrap();
    assert!(!repaired.ran);
    assert!(
        repaired.record["skipped"]
            .as_str()
            .unwrap()
            .contains("deadline")
    );
    assert_eq!(deadline.cuts().len(), 1);
    let _ = std::fs::remove_dir_all(out);
}

#[tokio::test]
async fn the_study_separates_the_packet_from_extra_sampling_and_counts_damage() {
    if process::python().is_none() {
        return;
    }
    let out = scratch("study");
    let options = Options {
        tasks: vec![find("log-severity").unwrap()],
        same: Profile::parse("fix-if-packet").unwrap(),
        other: Profile::parse("break").unwrap(),
        trigger: Trigger::Always,
        allowance: Duration::from_secs(60),
        out: out.clone(),
    };
    let result = study::run(&options).await.unwrap();
    let arm = |name: &str| {
        result["arms"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["arm"] == name)
            .unwrap()
            .clone()
    };
    assert_eq!(arm("none")["recovered"], json!(0));
    assert_eq!(arm("none")["repairs_run"], json!(0));
    assert_eq!(arm("fresh")["recovered"], json!(0));
    assert_eq!(arm("packet-same")["recovered"], json!(1));
    assert_eq!(arm("packet-same")["damaged"], json!(0));
    assert_eq!(arm("packet-other")["damaged"], json!(1));
    assert_eq!(arm("packet-same")["cost_usd"], json!(0.0));
    assert!(out.join("study.json").is_file());
    // Each arm worked on its own copy: the preserved workspace is as the
    // bad script left it.
    let preserved = &result["candidates"][0];
    assert_eq!(preserved["passed"], json!(false));
    let grade = crate::minitask::grade(
        &find("log-severity").unwrap(),
        &PathBuf::from(preserved["dir"].as_str().unwrap()).join("work"),
        &out.join("regrade"),
    )
    .await;
    assert_eq!(grade.verdict, "failed");
    let _ = std::fs::remove_dir_all(out);
}

#[tokio::test]
async fn an_arm_with_no_repair_leaves_the_copy_as_it_was() {
    if process::python().is_none() {
        return;
    }
    let out = scratch("none");
    let task = find("git-recovery").unwrap();
    let preserved = preserve_one(&task, "good", &out.join("p")).await.unwrap();
    let row = cell(
        &preserved,
        &Arm {
            name: "none",
            repair: None,
        },
        Trigger::Detected,
        Duration::from_secs(5),
        &out.join("arm"),
    )
    .await
    .unwrap();
    // The copy kept `.git`, so the git grader passes it as it passed the
    // original.
    assert_eq!(row["after"], json!(true));
    assert_eq!(row["triggered"], json!(false));
    let _ = std::fs::remove_dir_all(out);
}

#[test]
fn every_trigger_round_trips_through_its_word() {
    for trigger in [Trigger::Detected, Trigger::Checked, Trigger::Always] {
        assert_eq!(Trigger::parse(trigger.word()), Ok(trigger));
        let json = serde_json::to_value(trigger).unwrap();
        assert_eq!(json, json!(trigger.word()));
    }
    assert!(Trigger::parse("sometimes").is_err());
}
