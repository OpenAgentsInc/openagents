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
        workspace: None,
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
        workspace: None,
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
        workspace: None,
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
            workspace: None,
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

/// A live-workspace check of `instruction` over the files in `files`, with
/// `options`.
async fn live_check(
    label: &str,
    instruction: &str,
    files: &[(&str, &str)],
    options: generic::Options,
    report: Option<&str>,
    edit: impl FnOnce(&mut crate::requirements::RequirementMap),
) -> Report {
    let dir = scratch(label);
    let work = dir.join("work");
    std::fs::create_dir_all(&work).unwrap();
    for (path, text) in files {
        std::fs::write(work.join(path), text).unwrap();
    }
    let mut map = crate::requirements::mechanical(instruction);
    edit(&mut map);
    let subject = Subject {
        label: label.to_string(),
        task: TaskText {
            title: label.to_string(),
            instruction: instruction.to_string(),
        },
        requirements: Some(map),
        provided: Vec::new(),
        inputs: None,
        budget: Budget::default(),
        live: Some(generic::Workspace {
            dir: String::new(),
            claimed: Vec::new(),
            command_sec: 30,
            report: report.map(str::to_string),
            options,
            root: None,
            collected: Vec::new(),
            suite: None,
        }),
        distrust: Vec::new(),
    };
    let input = subject.input(&work);
    let report = check(&input, &Recorder::default(), &dir.join("scratch")).await;
    let _ = std::fs::remove_dir_all(&dir);
    report
}

fn verdict_of<'a>(report: &'a Report, id: &str) -> &'a str {
    report
        .verdicts
        .iter()
        .find(|v| v.scenario == id)
        .map_or("not run", |v| v.verdict.as_str())
}

const DEPENDENCIES: &str = "Fix the planner so that it writes a correct plan to `plan.json`.\n\n\
Write any Python dependencies needed to run your code to `requirements.txt`.";

/// `verify.suite_checks` costs a frozen test at four times its run on the
/// untouched workspace, at least ten seconds and at most the command
/// bound; with no measurement, at the command bound.
#[test]
fn a_frozen_test_is_costed_at_its_measured_time() {
    assert_eq!(generic::measured_bound(Some(3_200), 477), 13);
    assert_eq!(generic::measured_bound(Some(230), 477), 10);
    assert_eq!(generic::measured_bound(None, 477), 477);
    assert_eq!(generic::measured_bound(Some(200_000), 120), 120);
    // Thirteen tests like the ones that took 0.2 to 3.2 seconds fit a
    // 1,433-second budget many times over; at 477 seconds, three do.
    let measured: u64 = [3_200u64; 6]
        .iter()
        .chain([230u64; 7].iter())
        .map(|ms| generic::measured_bound(Some(*ms), 477))
        .sum();
    assert!(measured < 1_433 / 5, "{measured}");
    assert_eq!(1_433 / 477, 3);
}

fn optional_on() -> generic::Options {
    generic::Options {
        optional_outputs: true,
        ..generic::Options::default()
    }
}

#[tokio::test]
async fn an_empty_optional_output_passes_only_when_the_option_is_on() {
    let files = [("requirements.txt", ""), ("plan.json", "{\"ok\": true}")];
    let off = live_check(
        "optional-off",
        DEPENDENCIES,
        &files,
        generic::Options::default(),
        None,
        |_| {},
    )
    .await;
    assert_eq!(
        verdict_of(&off, "generic.output:requirements.txt"),
        "failed"
    );
    assert_eq!(off.implementation, implementation());
    let on = live_check(
        "optional-on",
        DEPENDENCIES,
        &files,
        optional_on(),
        None,
        |_| {},
    )
    .await;
    assert_eq!(verdict_of(&on, "generic.output:requirements.txt"), "passed");
    assert!(on.packets.is_empty(), "{:?}", on.packets);
    assert_ne!(on.implementation, implementation());
    // A missing file still fails: optional means it may be empty.
    let missing = live_check(
        "optional-missing",
        DEPENDENCIES,
        &[("plan.json", "{}")],
        optional_on(),
        None,
        |_| {},
    )
    .await;
    assert_eq!(
        verdict_of(&missing, "generic.output:requirements.txt"),
        "failed"
    );
}

#[tokio::test]
async fn a_missing_output_of_an_unsure_requirement_is_inconclusive() {
    let unsure = |map: &mut crate::requirements::RequirementMap| {
        for requirement in &mut map.requirements {
            if requirement.text.contains("plan.json") {
                requirement.binding = crate::requirements::Binding::Uncertain;
            }
        }
    };
    let on = live_check(
        "unsure-on",
        DEPENDENCIES,
        &[("requirements.txt", "numpy\n")],
        optional_on(),
        None,
        unsure,
    )
    .await;
    assert_eq!(verdict_of(&on, "generic.output:plan.json"), "inconclusive");
    assert!(!on.detected());
    let off = live_check(
        "unsure-off",
        DEPENDENCIES,
        &[("requirements.txt", "numpy\n")],
        generic::Options::default(),
        None,
        unsure,
    )
    .await;
    assert_eq!(verdict_of(&off, "generic.output:plan.json"), "failed");
}

#[tokio::test]
async fn a_self_reported_failure_is_a_failed_check_with_a_packet() {
    let files = [
        ("requirements.txt", "numpy\n"),
        (
            "plan.json",
            "{\"summary\": {\"route_feasible\": false}, \"legs\": [{\"takeoff_weight_ok\": false}]}",
        ),
    ];
    let options = generic::Options {
        self_report: true,
        ..generic::Options::default()
    };
    let report = live_check(
        "self-report",
        DEPENDENCIES,
        &files,
        options,
        Some("I checked all 24 orderings and every one breaks at least one weight limit."),
        |_| {},
    )
    .await;
    assert_eq!(verdict_of(&report, "generic.self-report"), "failed");
    let packet = report
        .packets
        .iter()
        .find(|p| p.scenario == "generic.self-report")
        .unwrap();
    let signals: Vec<&str> = packet
        .observations
        .iter()
        .map(|o| o["signal"].as_str().unwrap())
        .collect();
    assert_eq!(signals, ["infeasible", "output-flag"]);
    assert!(packet.requirement_text.contains("plan.json"));
    // Nothing reported: inconclusive, which leaves the requirement's state
    // to the other scenarios.
    let quiet = live_check(
        "self-report-quiet",
        DEPENDENCIES,
        &[
            ("requirements.txt", "numpy\n"),
            ("plan.json", "{\"route_feasible\": true}"),
        ],
        options,
        Some("The plan is correct and every leg is within limits."),
        |_| {},
    )
    .await;
    assert_eq!(verdict_of(&quiet, "generic.self-report"), "inconclusive");
    assert!(!quiet.detected());
    // Off by default.
    let off = live_check(
        "self-report-off",
        DEPENDENCIES,
        &files,
        generic::Options::default(),
        Some("every one breaks at least one weight limit"),
        |_| {},
    )
    .await;
    assert_eq!(verdict_of(&off, "generic.self-report"), "not run");
}

/// A live check with the behavior scenarios on, of the instruction
/// `instruction` makes from the workspace's absolute path, over `files`.
async fn behavior_check(
    label: &str,
    instruction: impl Fn(&str) -> String,
    files: &[(&str, &str)],
) -> Report {
    let dir = scratch(label);
    let work = dir.join("work");
    std::fs::create_dir_all(&work).unwrap();
    for (path, text) in files {
        std::fs::write(work.join(path), text).unwrap();
    }
    let text = instruction(&work.to_string_lossy());
    let subject = Subject {
        label: label.to_string(),
        task: TaskText {
            title: label.to_string(),
            instruction: text.clone(),
        },
        requirements: Some(crate::requirements::mechanical(&text)),
        provided: Vec::new(),
        inputs: None,
        budget: Budget::default(),
        live: Some(generic::Workspace {
            dir: String::new(),
            claimed: Vec::new(),
            command_sec: 30,
            report: None,
            options: generic::Options {
                behavior: true,
                ..generic::Options::default()
            },
            root: None,
            collected: Vec::new(),
            suite: None,
        }),
        distrust: Vec::new(),
    };
    let input = subject.input(&work);
    let report = check(&input, &Recorder::default(), &dir.join("scratch")).await;
    let _ = std::fs::remove_dir_all(&dir);
    report
}

fn filter_task(work: &str) -> String {
    format!(
        "Create a python file `{work}/filter.py` that removes JavaScript from HTML files to prevent XSS attacks.\n\n\
The script should take an HTML file as a command-line argument (argv[1]) and modify the file in-place to remove all JavaScript.\n\n\
Preserve the formatting of the HTML content, except for normalization that may occur during HTML parsing."
    )
}

#[tokio::test]
async fn the_filter_scenarios_tell_a_sanitizer_from_a_lossy_or_idle_one() {
    if crate::minitask::process::python().is_none() {
        return;
    }
    let idle = behavior_check(
        "filter-idle",
        filter_task,
        &[("filter.py", "import sys\nsys.exit(0)\n")],
    )
    .await;
    assert_eq!(verdict_of(&idle, "behavior.filter-removes"), "failed");
    assert_eq!(verdict_of(&idle, "behavior.filter-preserves"), "passed");
    let lossy = behavior_check(
        "filter-lossy",
        filter_task,
        &[(
            "filter.py",
            "import re, sys\np = sys.argv[1]\nt = open(p).read()\nt = re.sub(r'<!--.*?-->', '', t, flags=re.S)\nopen(p, 'w').write(t)\n",
        )],
    )
    .await;
    assert_eq!(verdict_of(&lossy, "behavior.filter-preserves"), "failed");
}

#[tokio::test]
async fn a_named_command_must_write_the_same_bytes_twice_when_the_task_says_so() {
    if crate::minitask::process::python().is_none() {
        return;
    }
    let task = |work: &str| {
        format!(
            "The command `python3 {work}/build.py --out {work}/out` should rebuild the report. Repeated rebuilds must produce deterministic outputs."
        )
    };
    let script = |body: &str| {
        format!(
            "import os, sys, time\nout = sys.argv[sys.argv.index('--out') + 1]\nos.makedirs(out, exist_ok=True)\nopen(os.path.join(out, 'report.txt'), 'w').write({body})\n"
        )
    };
    let steady = behavior_check("named-steady", task, &[("build.py", &script("'42\\n'"))]).await;
    assert_eq!(verdict_of(&steady, "behavior.named-command:1"), "passed");
    let drifting = behavior_check(
        "named-drifting",
        task,
        &[("build.py", &script("str(time.time_ns())"))],
    )
    .await;
    assert_eq!(verdict_of(&drifting, "behavior.named-command:1"), "failed");
}

#[tokio::test]
async fn a_selected_position_must_lie_in_the_range_it_was_chosen_for() {
    let task = |work: &str| {
        format!(
            "Identify the variant whose protein position overlaps with the domain. Write the final results to `{work}/report.json`."
        )
    };
    let report = |position: i64| {
        format!(
            r#"{{"domain": {{"protein_residue_start": 10, "protein_residue_end": 20}}, "selected": {{"protein_position": {position}}}}}"#
        )
    };
    let overlap = |r: &Report| {
        r.verdicts
            .iter()
            .find(|v| v.scenario.starts_with("behavior.json-overlap"))
            .map(|v| v.verdict.clone())
    };
    let inside = behavior_check("overlap-in", task, &[("report.json", &report(15))]).await;
    assert_eq!(overlap(&inside).as_deref(), Some("passed"));
    let outside = behavior_check("overlap-out", task, &[("report.json", &report(25))]).await;
    assert_eq!(overlap(&outside).as_deref(), Some("failed"));
}

#[test]
fn the_v7_policy_is_v6_with_the_behavior_scenarios_and_a_snapshot() {
    let read = |name: &str| {
        let text = std::fs::read_to_string(crate::policy::reference_dir().join(name)).unwrap();
        crate::policy::Manifest::parse(&text).unwrap()
    };
    let v6 = read("tunable-v6.json");
    let v7 = read("tunable-v7.json");
    let (six, seven) = (v6.policy.verify.unwrap(), v7.policy.verify.unwrap());
    assert!(!six.check_options().behavior);
    assert!(seven.check_options().behavior);
    assert!(seven.snapshot.is_some());
    assert_eq!(v6.policy.control, v7.policy.control);
    assert_eq!(v6.policy.executor, v7.policy.executor);
    assert_eq!(six.second, seven.second);
    assert!(seven.validate().is_empty());
}

#[test]
fn a_replayed_input_carries_no_verifier_test_name() {
    let labeled = labeled::Labeled {
        label: labeled::Label {
            schema: labeled::LABEL_SCHEMA.to_string(),
            job: "tb4--coder-one-tunable-v2--x".to_string(),
            trial: "x__1".to_string(),
            task: "x".to_string(),
            arm: "coder-one-tunable-v2".to_string(),
            reward: Some(0.0),
            excluded: None,
            failed_tests: vec!["test_hidden_packet_uses_manifest_paths".to_string()],
            passed_tests: 3,
            episode_flagged: None,
            collected: Vec::new(),
        },
        task_dir: PathBuf::from("/nonexistent"),
        trial_dir: PathBuf::from("/nonexistent"),
        instruction: "Write `/app/out.json`.".to_string(),
        requirements: None,
        report: Some("Done.".to_string()),
        claimed: Vec::new(),
    };
    let input = labeled::input(
        &labeled,
        Path::new("/nonexistent-root"),
        "/app",
        labeled::arm_options("v7").unwrap(),
    );
    let text = serde_json::to_string(&input).unwrap();
    assert!(!text.contains("test_hidden_packet_uses_manifest_paths"));
}

#[test]
fn a_distrusted_kind_fails_as_inconclusive_and_contradicts_nothing() {
    use super::{Scenario, Verdict, apply_distrust};
    let scenario = |id: &str, kind: &str| -> Scenario {
        serde_json::from_value(serde_json::json!({
            "id": id, "kind": kind, "requirements": ["R1"], "spans": [], "applies": [],
            "interface": "", "bounds": { "seconds": 1, "processes": 1 }, "effects": [],
            "candidate": "c", "input": "i",
            "expected": { "statement": "", "derivation": "" }, "params": {}
        }))
        .expect("a scenario")
    };
    let scenarios = vec![
        scenario("behavior.filter-preserves", "behavior.filter-preserves"),
        scenario("generic.output:/app/a", "generic.output"),
    ];
    let failed = |id: &str| -> Verdict {
        serde_json::from_value(serde_json::json!({
            "scenario": id, "verdict": "failed", "observations": [], "coverage": []
        }))
        .expect("a verdict")
    };
    let mut verdicts = vec![
        failed("behavior.filter-preserves"),
        failed("generic.output:/app/a"),
    ];
    apply_distrust(
        &scenarios,
        &mut verdicts,
        &["behavior.filter-preserves".to_string()],
    );
    assert_eq!(verdicts[0].verdict, "inconclusive");
    assert!(
        verdicts[0]
            .coverage
            .iter()
            .any(|c| c.contains("verify.distrust"))
    );
    assert_eq!(verdicts[1].verdict, "failed");
}

/// A frozen acceptance suite becomes one scenario per test on the
/// requirements it names: a green test observes its requirement, and a red
/// one contradicts it, instead of leaving both unobserved.
#[tokio::test]
async fn the_frozen_suite_observes_the_requirements_its_tests_name() {
    let dir = scratch("acceptance-scenarios");
    let work = dir.join("work");
    let suite = dir.join("accept-suite-1");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::create_dir_all(suite.join("tests")).unwrap();
    std::fs::write(work.join("hello.txt"), "hello\n").unwrap();
    std::fs::write(work.join("world.txt"), "nope\n").unwrap();
    std::fs::write(
        suite.join("tests/T1.sh"),
        "grep -qx hello \"$WORKSPACE/hello.txt\"\n",
    )
    .unwrap();
    std::fs::write(suite.join("tests/T2.sh"), "grep -qx world world.txt\n").unwrap();
    let test = |id: &str, requirement: &str, what: &str| {
        serde_json::json!({
            "id": id, "requirements": [requirement], "kind": "example", "what": what,
            "path": format!("tests/{id}.sh"),
        })
    };
    let record = dir.join("accept-suite-1.accept.json");
    std::fs::write(
        &record,
        serde_json::json!({
            "schema": crate::accept::SCHEMA, "dir": suite, "status": "accepted",
            "instruction_sha256": "", "tests": [
                test("T1", "R1", "hello.txt holds hello"),
                test("T2", "R2", "world.txt holds world"),
            ],
            "rejected": [], "coverage": [], "gaps": [], "start": null, "files": {},
            "digest": "", "rounds": [], "writer_usd": 0.0, "jev_usd": 0.0,
            "milliseconds": 0, "detail": null,
        })
        .to_string(),
    )
    .unwrap();
    assert_eq!(
        crate::accept::latest_record(&dir).as_deref(),
        Some(record.display().to_string().as_str())
    );
    let instruction = "Write hello.txt containing the word hello.\n\nThen write world.txt containing the word world.";
    let subject = Subject {
        label: "acceptance".to_string(),
        task: TaskText {
            title: "acceptance".to_string(),
            instruction: instruction.to_string(),
        },
        requirements: Some(crate::requirements::mechanical(instruction)),
        provided: Vec::new(),
        inputs: None,
        budget: Budget::default(),
        live: Some(generic::Workspace {
            dir: String::new(),
            claimed: Vec::new(),
            command_sec: 30,
            report: None,
            options: generic::Options::default(),
            root: None,
            collected: Vec::new(),
            suite: Some(record.display().to_string()),
        }),
        distrust: Vec::new(),
    };
    let input = subject.input(&work);
    let report = check(&input, &Recorder::default(), &dir.join("scratch")).await;
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(verdict_of(&report, "generic.acceptance:T1"), "passed");
    assert_eq!(verdict_of(&report, "generic.acceptance:T2"), "failed");
    let state = |id: &str| {
        report
            .coverage
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.state.clone())
    };
    assert_eq!(state("R1").as_deref(), Some("observed"));
    assert_eq!(state("R2").as_deref(), Some("contradicted"));
}
