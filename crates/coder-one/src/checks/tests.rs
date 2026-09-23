use super::*;

fn scratch(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "coder-one-checks-{label}-{}-{}",
        std::process::id(),
        atif::now_ms()
    ))
}

#[tokio::test]
async fn every_scenario_separates_its_known_good_and_known_bad_candidates() {
    if crate::minitask::process::python().is_none() {
        eprintln!("python3 is not on PATH; the scenarios can't run here");
        return;
    }
    for case in synthetic::cases() {
        let recorder = Recorder::default();
        let report = check(&case.input, &recorder, &scratch(case.name)).await;
        let verdict = |id: &str| {
            report
                .verdicts
                .iter()
                .find(|v| v.scenario == id)
                .map_or("not run", |v| v.verdict.as_str())
        };
        for id in &case.fails {
            assert_eq!(
                verdict(id),
                "failed",
                "{} should fail {id}: {:#?}",
                case.name,
                report.verdicts
            );
        }
        for id in &case.passes {
            assert_eq!(
                verdict(id),
                "passed",
                "{} false alarm on {id}: {:#?}",
                case.name,
                report.verdicts
            );
        }
        if case.good {
            assert!(
                report.packets.is_empty(),
                "{}: {:?}",
                case.name,
                report.packets
            );
        } else {
            let packet = &report.packets[0];
            assert_eq!(packet.candidate, case.input.candidate.digest());
            assert!(!packet.hypotheses.is_empty());
            assert!(!packet.requirement_text.is_empty());
        }
        // Build, select, each run, and coverage are recorded under one
        // verify.checks invocation.
        let invocations = crate::record::invocations(&recorder.steps());
        let root = &invocations[0];
        assert_eq!(root.component, "verify.checks");
        for stage in [
            "verify.checks.build",
            "verify.checks.select",
            "verify.checks.coverage",
        ] {
            assert!(
                invocations
                    .iter()
                    .any(|i| i.component == stage && i.parent.as_deref() == Some(root.id.as_str())),
                "{} lacks {stage}",
                case.name
            );
        }
        assert_eq!(
            invocations
                .iter()
                .filter(|i| i.component == "verify.checks.run")
                .count(),
            report.scenarios.len()
        );
    }
}

#[test]
fn a_scenario_names_its_requirement_spans_and_derivation() {
    let case = synthetic::case("log-field-parser").unwrap();
    let map = crate::requirements::mechanical(&case.input.task.instruction);
    let context = Context {
        task: &case.input.task,
        map: &map,
        candidate: &case.input.candidate,
        observed: &case.input.observed,
    };
    let (scenarios, _) = build(&context);
    let message = scenarios
        .iter()
        .find(|s| s.id == "data.message-severity")
        .unwrap();
    assert!(!message.requirements.is_empty());
    let span = &message.spans[0];
    assert_eq!(
        &case.input.task.instruction[span.start..span.end],
        span.text
    );
    assert!(!message.expected.derivation.is_empty());
    assert_eq!(message.candidate, case.input.candidate.digest());
    let boundaries = scenarios
        .iter()
        .find(|s| s.id == "data.date-boundaries")
        .unwrap();
    let days: Vec<&str> = boundaries.params["days"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["date"].as_str().unwrap())
        .collect();
    assert!(
        days.contains(&"2025-08-06") && days.contains(&"2025-08-05"),
        "{days:?}"
    );
}

#[test]
fn a_candidate_without_the_named_interface_is_ineligible_not_failed() {
    let mut case = synthetic::case("cancel-awaited-cleanup").unwrap();
    case.input.candidate.files.clear();
    let map = crate::requirements::mechanical(&case.input.task.instruction);
    let context = Context {
        task: &case.input.task,
        map: &map,
        candidate: &case.input.candidate,
        observed: &case.input.observed,
    };
    let (scenarios, ineligible) = build(&context);
    assert!(scenarios.is_empty());
    assert!(
        ineligible.iter().any(|i| i.why.contains("run.py")),
        "{ineligible:?}"
    );
}

#[test]
fn the_selector_keeps_one_scenario_per_requirement_within_the_budget() {
    let case = synthetic::case("cancel-awaited-cleanup").unwrap();
    let map = crate::requirements::mechanical(&case.input.task.instruction);
    let context = Context {
        task: &case.input.task,
        map: &map,
        candidate: &case.input.candidate,
        observed: &case.input.observed,
    };
    let (scenarios, _) = build(&context);
    assert_eq!(scenarios.len(), 4);
    let (selected, record) = select(
        &scenarios,
        Budget {
            max_scenarios: 2,
            seconds: 600,
        },
    );
    assert_eq!(selected.len(), 2);
    assert_eq!(record["skipped"].as_array().unwrap().len(), 2);
}

#[test]
fn protected_verifier_test_names_never_enter_a_scenario() {
    // The retained verifier output names the protected tests; no scenario
    // built from a v3 candidate may carry one.
    let traces =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/traces");
    let Ok(recovered) = recover::recover_tree(&traces, "coder-one-jevprobe3-luna") else {
        return;
    };
    let mut checked = 0;
    for one in &recovered {
        let Some(input) = &one.input else { continue };
        let map = crate::requirements::mechanical(&input.task.instruction);
        let context = Context {
            task: &input.task,
            map: &map,
            candidate: &input.candidate,
            observed: &input.observed,
        };
        let (scenarios, _) = build(&context);
        let text = serde_json::to_string(&scenarios).unwrap();
        for name in &one.protected_names {
            assert!(
                !text.contains(name.as_str()),
                "{} carries {name}",
                one.trial
            );
        }
        checked += 1;
    }
    assert!(checked > 0 || recovered.is_empty());
}
