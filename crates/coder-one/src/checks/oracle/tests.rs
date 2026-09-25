use std::collections::BTreeMap;

use super::*;
use crate::checks::contract::extract::{Entry, Pristine};
use crate::checks::contract::host::{Memory, Stat};

fn spec_with(cases: &[&str]) -> Spec {
    Spec {
        schema: String::new(),
        task: "t".to_string(),
        workdir: "/app".to_string(),
        instruction: "x".to_string(),
        definition: vec!["The output is the sum of the inputs.".to_string()],
        formats: Vec::new(),
        parameters: Vec::new(),
        boundaries: Vec::new(),
        inputs: Vec::new(),
        references: Vec::new(),
        cases: cases
            .iter()
            .map(|id| CaseSpec {
                id: (*id).to_string(),
                covers: Covers {
                    parameter: Some("k".to_string()),
                    value: Some((*id).to_string()),
                    ..Covers::default()
                },
            })
            .collect(),
        jev: Vec::new(),
        digest: String::new(),
    }
    .sealed()
}

fn written(spec: &Spec) -> Oracle {
    let mut files = BTreeMap::new();
    files.insert("oracle.py".to_string(), "print()".to_string());
    Oracle {
        schema: String::new(),
        task: "t".to_string(),
        source: Source::Written,
        command: None,
        origin: None,
        files,
        spec: Some(spec.digest.clone()),
        writer: json!({"usd": 0.01}),
        digest: String::new(),
    }
    .sealed()
}

#[test]
fn every_case_gets_a_result_and_a_missing_one_could_not_run() {
    let spec = spec_with(&["O1", "P1", "P2"]);
    let ran = Ran {
        exit: Some(0),
        stdout: "noise\n{\"case\":\"O1\",\"verdict\":\"passed\",\"expected\":\"3\",\"observed\":\"3\"}\n\
                 {\"case\":\"P1\",\"verdict\":\"failed\",\"expected\":4,\"observed\":5}\n"
            .to_string(),
        ..Ran::default()
    };
    let cases = written_cases(Some(&spec), &ran);
    assert_eq!(cases.len(), 3);
    assert_eq!(cases[0].verdict, Verdict::Passed);
    assert_eq!(cases[1].verdict, Verdict::Failed);
    assert_eq!(cases[1].expected.as_deref(), Some("4"));
    assert_eq!(cases[1].covers.value.as_deref(), Some("P1"));
    assert_eq!(cases[2].verdict, Verdict::CouldNotRun);
}

#[test]
fn a_checker_passes_on_exit_zero_and_a_missing_program_says_nothing() {
    let ok = checker_case(
        "python3 check.py",
        &Ran {
            exit: Some(0),
            ..Ran::default()
        },
    );
    assert_eq!(ok.verdict, Verdict::Passed);
    let bad = checker_case(
        "python3 check.py",
        &Ran {
            exit: Some(1),
            stderr: "mismatch at row 3".to_string(),
            ..Ran::default()
        },
    );
    assert_eq!(bad.verdict, Verdict::Failed);
    assert!(bad.detail.unwrap().contains("row 3"));
    let missing = checker_case(
        "python3 check.py",
        &Ran {
            exit: Some(127),
            ..Ran::default()
        },
    );
    assert_eq!(missing.verdict, Verdict::CouldNotRun);
}

#[tokio::test]
async fn a_written_oracle_runs_and_marks_itself_trivial_from_the_untouched_run() {
    let spec = spec_with(&["O1"]);
    let oracle = written(&spec);
    let command = oracle.invocation("/stage", "/app").unwrap();
    assert!(command.starts_with("python3 '/stage/oracle.py' '/app'"));
    let mut host = Memory::default();
    host.runs.insert(
        command,
        Ran {
            exit: Some(0),
            stdout: "{\"case\":\"O1\",\"verdict\":\"passed\"}\n".to_string(),
            ..Ran::default()
        },
    );
    let untouched = run(
        &oracle,
        Some(&spec),
        &host,
        "/stage",
        "/app",
        "untouched",
        None,
    )
    .await;
    assert_eq!(untouched.passed(), Some(true));
    assert!(!usable(&untouched));
    let later = run(
        &oracle,
        Some(&spec),
        &host,
        "/stage",
        "/app",
        "c1",
        Some(&untouched),
    )
    .await;
    assert!(later.trivially_passing());
    assert_eq!(
        later.authority,
        crate::accept::authority::Authority::IndependentlySupported
    );
    assert_eq!(later.provenance.component, "checks.oracle");
}

#[test]
fn seals_are_stable_and_ignore_the_writer_record() {
    let spec = spec_with(&["O1"]);
    assert_eq!(spec.digest, spec.clone().sealed().digest);
    let a = written(&spec);
    let mut b = a.clone();
    b.writer = json!({"usd": 0.5});
    assert_eq!(a.digest, b.sealed().digest);
}

fn pristine(files: &[(&str, &str)]) -> Pristine {
    let mut p = Pristine::default();
    for (path, text) in files {
        p.entries.insert(
            (*path).to_string(),
            Entry {
                stat: Stat::File(text.len() as u64),
                text: Some((*text).to_string()),
            },
        );
    }
    p
}

#[test]
fn a_stated_checker_command_is_found_and_the_code_under_test_is_not() {
    let instruction = "Fix the bug in `/app/check_totals.py` so totals are right.\n\n\
                       When you're done, run `python3 /app/validate_output.py /app/out.json` \
                       to confirm the output. The file `/app/reference_impl.py` is provided.";
    let p = pristine(&[
        ("/app/check_totals.py", "x"),
        ("/app/validate_output.py", "x"),
        ("/app/reference_impl.py", "x"),
    ]);
    let found = find::find(instruction, "/app", &p);
    assert_eq!(
        found.command.as_deref(),
        Some("python3 /app/validate_output.py /app/out.json")
    );
    assert_eq!(found.references, vec!["/app/reference_impl.py".to_string()]);
}

#[test]
fn a_named_runnable_checker_file_is_found_by_its_name() {
    let instruction = "The routes must pass the checker at `/app/tools/verify.py`.";
    let p = pristine(&[("/app/tools/verify.py", "x")]);
    let found = find::find(instruction, "/app", &p);
    assert_eq!(
        found.command.as_deref(),
        Some("python3 /app/tools/verify.py")
    );
    assert!(
        find::find("Nothing to see here.", "/app", &Pristine::default())
            .command
            .is_none()
    );
}

#[test]
fn stated_values_are_listed_with_their_names() {
    let values = define::values_in(
        "Flag a user when the score exceeds a threshold of 0.75 within a window of 30 minutes; \
         use `k=5` neighbors and read `/app/data/v2.csv`.",
    );
    let pairs: Vec<(&str, &str)> = values
        .iter()
        .map(|v| (v.name.as_str(), v.value.as_str()))
        .collect();
    assert!(pairs.contains(&("k", "5")));
    assert!(pairs.iter().any(|(_, v)| *v == "0.75"));
    assert!(pairs.iter().any(|(_, v)| *v == "30"));
    assert!(!pairs.iter().any(|(_, v)| v.contains("v2")));
}

#[test]
fn questions_name_no_task() {
    for text in [
        define::DEFINES,
        define::BOUNDARY,
        define::PARAMETER,
        write::TASK,
        write::PROTOCOL,
    ] {
        for word in ["terminal-bench", "verifier", "hidden test"] {
            assert!(!text.to_lowercase().contains(word), "{word} in {text}");
        }
    }
}

#[test]
fn the_brief_holds_the_spec_and_nothing_else() {
    let spec = spec_with(&["O1", "P1"]);
    let brief = write::brief(&spec);
    let text = serde_json::to_string(&brief.input()).unwrap();
    assert!(text.contains("sum of the inputs"));
    assert!(text.contains("\\\"P1\\\""));
}
