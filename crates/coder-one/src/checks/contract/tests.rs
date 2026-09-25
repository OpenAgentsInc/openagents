use std::collections::BTreeMap;

use serde_json::json;

use super::extract::{self, Asking, Pristine};
use super::host::{Local, Memory, Ran};
use super::*;

const INSTRUCTION: &str = include_str!("../../../fixtures/contract/instruction.md");

const README: &str = "# Engine\n\nRun the linter:\n\n```sh\nmake -C /app lint\n```\n\nUsage: python apply.py RULES INPUT\n";

fn untouched() -> Memory {
    let mut files = BTreeMap::new();
    for (path, text) in [
        ("/app/data/sample_text.txt", "hello, world"),
        ("/app/data/sample_shouted.txt", "HELLO, WORLD"),
        ("/app/data/forbidden.json", "{}"),
        ("/app/engine/README.md", README),
        ("/app/notes.txt", "notes"),
    ] {
        files.insert(path.to_string(), text.as_bytes().to_vec());
    }
    Memory {
        files,
        ..Memory::default()
    }
}

async fn drafted() -> (Pristine, extract::Draft) {
    let host = untouched();
    let pristine = extract::gather(&host, INSTRUCTION, "/app").await;
    let draft = extract::draft(INSTRUCTION, "/app", &pristine);
    (pristine, draft)
}

fn find<'a>(items: &'a [Item], kind: Kind, what: &str) -> &'a Item {
    items
        .iter()
        .find(|i| {
            i.kind == kind
                && (i.command.as_deref().is_some_and(|c| c.contains(what))
                    || i.path.as_deref().is_some_and(|p| p.contains(what)))
        })
        .unwrap_or_else(|| panic!("no {kind:?} item for {what}: {items:#?}"))
}

#[test]
fn sentences_split_outside_code_spans_only() {
    let text = "Run `python a.py x.txt`. Then read `/app/b.md`! Done";
    assert_eq!(
        extract::sentences(text),
        vec!["Run `python a.py x.txt`.", "Then read `/app/b.md`!", "Done"]
    );
}

#[test]
fn segment_keeps_blocks_with_their_lead_and_drops_comments() {
    let (units, blocks) = extract::segment(INSTRUCTION);
    assert!(units.iter().all(|u| !u.text.contains("synthetic task")));
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].lang, "json");
    let lead = blocks[0].lead.expect("a lead");
    assert_eq!(units[lead].text, "Each rule has this schema:");
}

#[test]
fn outputs_are_told_from_inputs_by_the_nearest_cue() {
    assert_eq!(extract::is_output("Write the results to "), Some(true));
    assert_eq!(extract::is_output("The data is located at "), Some(false));
    assert_eq!(extract::is_output("and do not write to "), Some(false));
    assert_eq!(
        extract::is_output("The grader imports `a` from "),
        Some(false)
    );
    assert_eq!(
        extract::is_output("Consolidate the shards into a single file at "),
        Some(true)
    );
    assert_eq!(extract::is_output(""), None);
}

#[test]
fn commands_paths_and_identifiers_are_read_by_form() {
    assert!(extract::is_command("make -C /app test"));
    assert!(extract::is_command("python cracker.py <file>"));
    assert!(extract::is_command("./run.sh"));
    assert!(!extract::is_command("python"));
    assert!(!extract::is_command("/app/out.csv"));
    assert_eq!(
        extract::as_path("/app/x.json", "/app"),
        Some("/app/x.json".to_string())
    );
    assert_eq!(
        extract::as_path("data/x.txt", "/app"),
        Some("/app/data/x.txt".to_string())
    );
    assert_eq!(extract::as_path("make_engine", "/app"), None);
    assert!(extract::is_identifier("RECOVERY_KEYS"));
    assert!(!extract::is_identifier("f(x)"));
}

#[tokio::test]
async fn the_fixture_yields_every_stated_item() {
    let (_, draft) = drafted().await;
    let items = &draft.items;
    // Outputs and their formats.
    let tool = find(items, Kind::Path, "/app/shout.py");
    assert_eq!(tool.expect, Some(Expect::Exists));
    assert!(tool.not_executable.is_none());
    let rules = find(items, Kind::Format, "/app/rules.json");
    assert_eq!(
        rules.expect,
        Some(Expect::Json {
            top: Some(JsonTop::Array),
            shape: Some(json!({"name": "r1", "from": "a", "to": "b"})),
        })
    );
    assert_eq!(
        find(items, Kind::Format, "/app/order.txt").expect,
        Some(Expect::Lines { sorted: true })
    );
    assert_eq!(
        find(items, Kind::Format, "summary.csv").expect,
        Some(Expect::Table {
            delimiter: ',',
            columns: vec!["file".into(), "lines".into(), "words".into()],
            ordered: true,
        })
    );
    assert_eq!(
        find(items, Kind::Format, "report.tsv").expect,
        Some(Expect::Table {
            delimiter: '\t',
            columns: vec!["name".into(), "score".into()],
            ordered: false,
        })
    );
    // Inputs and things the task forbids aren't outputs.
    for path in [
        "notes.txt",
        "forbidden.json",
        "sample_text.txt",
        "README.md",
    ] {
        assert!(
            !items.iter().any(
                |i| i.kind == Kind::Path && i.path.as_deref().is_some_and(|p| p.contains(path))
            ),
            "{path} read as an output"
        );
    }
    // The interface.
    let interface = items
        .iter()
        .find(|i| i.kind == Kind::Interface)
        .expect("an interface item");
    let command = interface.command.as_deref().unwrap_or_default();
    assert!(
        command.contains("shout") && command.contains("VERSION"),
        "{command}"
    );
    // Stated exit statuses, with the stated bound.
    let missing_argument = find(items, Kind::ExitCode, "python shout.py");
    assert_eq!(missing_argument.command.as_deref(), Some("python shout.py"));
    assert_eq!(
        missing_argument.expect,
        Some(Expect::Exit {
            exit: Exit::NonZero
        })
    );
    assert_eq!(missing_argument.wall_sec, Some(20));
    assert!(missing_argument.stated_bound);
    find(items, Kind::ExitCode, extract::MISSING_INPUT);
    // The example: the same-length rule decided by code, the expected
    // output left for Jev.
    let length = items
        .iter()
        .find(|i| matches!(i.expect, Some(Expect::StdoutLength { chars: 12, .. })))
        .expect("a length item");
    assert_eq!(
        length.command.as_deref(),
        Some("python shout.py '/app/data/sample_text.txt'")
    );
    let pairs: Vec<&str> = draft
        .pending
        .iter()
        .filter_map(|p| match &p.ask {
            Asking::Pairs { expected, .. } => Some(expected.as_str()),
            Asking::Succeeds { .. } => None,
        })
        .collect();
    assert!(pairs.contains(&"/app/data/sample_shouted.txt"), "{pairs:?}");
    // Commands: a stated pass by code, the rest for Jev or not executable.
    assert_eq!(
        find(items, Kind::Command, "make -C /app test").expect,
        Some(Expect::Exit { exit: Exit::Zero })
    );
    let bench = find(items, Kind::Command, "make -C /app bench");
    assert!(bench.expect.is_none());
    let lint = find(items, Kind::Command, "make -C /app lint");
    assert_eq!(lint.source, "/app/engine/README.md");
    let apply = find(items, Kind::Command, "apply.py");
    assert!(
        apply
            .not_executable
            .as_deref()
            .unwrap_or_default()
            .contains("/app/apply.py")
    );
    let succeeds: Vec<&str> = draft
        .pending
        .iter()
        .filter_map(|p| match &p.ask {
            Asking::Succeeds { command, .. } => Some(command.as_str()),
            Asking::Pairs { .. } => None,
        })
        .collect();
    assert_eq!(succeeds, vec!["make -C /app bench", "make -C /app lint"]);
    // IDs are stable and unique.
    let ids: std::collections::BTreeSet<&str> = items.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids.len(), items.len());
}

#[tokio::test]
async fn an_output_already_there_is_not_a_check() {
    let mut host = untouched();
    host.files
        .insert("/app/shout.py".into(), b"print()".to_vec());
    let pristine = extract::gather(&host, INSTRUCTION, "/app").await;
    let draft = extract::draft(INSTRUCTION, "/app", &pristine);
    let tool = find(&draft.items, Kind::Path, "/app/shout.py");
    assert!(tool.not_executable.is_some());
}

#[tokio::test]
async fn jev_settles_only_what_the_words_leave_open() {
    let (_, draft) = drafted().await;
    let mut answers = BTreeMap::new();
    for p in &draft.pending {
        let yes = match &p.ask {
            Asking::Succeeds { command, .. } => command.contains("lint"),
            Asking::Pairs { expected, .. } => expected.ends_with("sample_shouted.txt"),
        };
        answers.insert(p.id.clone(), if yes { 0.9 } else { 0.1 });
    }
    let items = extract::finish(draft, &answers);
    assert_eq!(
        find(&items, Kind::Command, "lint").expect,
        Some(Expect::Exit { exit: Exit::Zero })
    );
    let bench = find(&items, Kind::Command, "bench");
    assert!(
        bench
            .not_executable
            .as_deref()
            .unwrap_or_default()
            .contains("not stated to succeed")
    );
    let example = items
        .iter()
        .find(|i| matches!(i.expect, Some(Expect::StdoutEquals { .. })))
        .expect("the example");
    assert_eq!(
        example.expect,
        Some(Expect::StdoutEquals {
            reference: "/app/data/sample_shouted.txt".into(),
            text: "HELLO, WORLD".into(),
        })
    );
}

#[tokio::test]
async fn with_no_answers_open_items_are_not_executable() {
    let (_, draft) = drafted().await;
    let open: Vec<usize> = draft.pending.iter().map(|p| p.item).collect();
    let items = extract::finish(draft, &BTreeMap::new());
    for index in open {
        assert!(items[index].not_executable.is_some(), "{:?}", items[index]);
    }
}

fn exited(code: i32, stdout: &str) -> Ran {
    Ran {
        exit: Some(code),
        stdout: stdout.to_string(),
        ..Ran::default()
    }
}

#[tokio::test]
async fn a_run_compares_by_code_and_says_why_it_couldnt() {
    let (_, draft) = drafted().await;
    let answers: BTreeMap<String, f64> = draft
        .pending
        .iter()
        .map(|p| (p.id.clone(), if matches!(p.ask, Asking::Pairs { ref expected, .. } if expected.ends_with("shouted.txt")) { 0.9 } else { 0.2 }))
        .collect();
    let plan = Plan::seal(
        "synthetic",
        "/app",
        INSTRUCTION,
        extract::finish(draft, &answers),
        Vec::new(),
    );
    let mut candidate = untouched();
    candidate
        .files
        .insert("/app/shout.py".into(), b"...".to_vec());
    candidate.files.insert(
        "/app/rules.json".into(),
        br#"[{"name":"r1","from":"a"}]"#.to_vec(),
    );
    candidate
        .files
        .insert("/app/order.txt".into(), b"a\nb\n".to_vec());
    candidate.files.insert(
        "/app/out/summary.csv".into(),
        b"file,lines,words\nx,1,2\n".to_vec(),
    );
    candidate.runs.insert(
        "python shout.py '/app/data/sample_text.txt'".into(),
        exited(0, "HELLO, WORLX"),
    );
    candidate
        .runs
        .insert("python shout.py".into(), exited(2, ""));
    candidate.runs.insert(
        format!("python shout.py {}", extract::MISSING_INPUT),
        exited(0, ""),
    );
    candidate.runs.insert(
        "make -C /app test".into(),
        Ran {
            timed_out: true,
            ..Ran::default()
        },
    );
    let results = run(&plan, &candidate).await;
    let outcome = |kind: Kind, what: &str| {
        let item = find(&plan.items, kind, what);
        results
            .iter()
            .find(|r| r.id == item.id)
            .expect("a result")
            .outcome
            .clone()
    };
    assert_eq!(outcome(Kind::Path, "shout.py").word(), "matched");
    match outcome(Kind::Format, "rules.json") {
        Outcome::Differed { diff, .. } => assert!(diff.contains("[0].to is missing"), "{diff}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(outcome(Kind::Format, "order.txt").word(), "matched");
    assert_eq!(outcome(Kind::Format, "summary.csv").word(), "matched");
    match outcome(Kind::Format, "report.tsv") {
        Outcome::CouldNotRun { why } => assert!(why.contains("no file"), "{why}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(outcome(Kind::ExitCode, "python shout.py").word(), "matched");
    assert_eq!(
        outcome(Kind::ExitCode, extract::MISSING_INPUT).word(),
        "differed"
    );
    let example = plan
        .items
        .iter()
        .find(|i| matches!(i.expect, Some(Expect::StdoutEquals { .. })))
        .expect("the example");
    match &results
        .iter()
        .find(|r| r.id == example.id)
        .expect("a result")
        .outcome
    {
        Outcome::Differed { diff, similarity } => {
            assert!(diff.contains("HELLO, WORLX"), "{diff}");
            assert!((similarity.unwrap_or_default() - 11.0 / 12.0).abs() < 1e-9);
        }
        other => panic!("{other:?}"),
    }
    match outcome(Kind::Command, "make -C /app test") {
        Outcome::CouldNotRun { why } => assert!(why.contains("timed out"), "{why}"),
        other => panic!("{other:?}"),
    }
    // The interface's python3 isn't in the memory host: not found, so it
    // couldn't run rather than differed.
    let interface = plan
        .items
        .iter()
        .find(|i| i.kind == Kind::Interface)
        .expect("an interface");
    assert_eq!(
        results
            .iter()
            .find(|r| r.id == interface.id)
            .expect("a result")
            .outcome
            .word(),
        "could_not_run"
    );
    assert_eq!(call(&results), Some("fail"));
    let value = report(&plan, "c1", &results);
    assert_eq!(value["schema"], REPORT_SCHEMA);
    assert_eq!(value["tally"]["format"]["differed"], 1);
}

#[test]
fn a_stated_bound_passed_is_a_difference_and_an_unstated_one_is_not() {
    let mut item = Item {
        id: "K1".into(),
        kind: Kind::Example,
        source: "instruction".into(),
        span: String::new(),
        command: Some("x".into()),
        path: None,
        expect: Some(Expect::Exit { exit: Exit::Zero }),
        wall_sec: Some(30),
        stated_bound: true,
        not_executable: None,
        decided_by: None,
    };
    let slow = Ran {
        timed_out: true,
        ..Ran::default()
    };
    assert_eq!(judge_run(&item, &slow).word(), "differed");
    item.stated_bound = false;
    assert_eq!(judge_run(&item, &slow).word(), "could_not_run");
    assert_eq!(judge_run(&item, &exited(127, "")).word(), "could_not_run");
    assert_eq!(judge_run(&item, &exited(0, "")).word(), "matched");
    assert_eq!(judge_run(&item, &exited(3, "")).word(), "differed");
}

#[test]
fn file_formats_are_judged_by_code() {
    let json_object = Expect::Json {
        top: Some(JsonTop::Object),
        shape: Some(json!({"clusters": [{"id": 1, "records": [{"source": "x"}]}]})),
    };
    assert_eq!(
        judge_file(
            &json_object,
            br#"{"clusters":[{"id":1,"records":[{"source":"a"}]}]}"#
        )
        .word(),
        "matched"
    );
    match judge_file(&json_object, br#"{"clusters":[{"id":1,"records":[{}]}]}"#) {
        Outcome::Differed { diff, .. } => {
            assert!(diff.contains(".clusters[0].records[0].source"), "{diff}")
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(judge_file(&json_object, b"[1]").word(), "differed");
    assert_eq!(judge_file(&json_object, b"not json").word(), "differed");
    let ordered = Expect::Table {
        delimiter: ',',
        columns: vec!["a".into(), "b".into()],
        ordered: true,
    };
    assert_eq!(
        judge_file(&ordered, b"\xef\xbb\xbfa,b\n1,2\n").word(),
        "matched"
    );
    assert_eq!(judge_file(&ordered, b"b,a\n").word(), "differed");
    let loose = Expect::Table {
        delimiter: '\t',
        columns: vec!["a".into(), "b".into()],
        ordered: false,
    };
    assert_eq!(judge_file(&loose, b"\"b\"\t\"a\"\tc\n").word(), "matched");
    let sorted = Expect::Lines { sorted: true };
    assert_eq!(judge_file(&sorted, b"a\nb\n").word(), "matched");
    assert_eq!(judge_file(&sorted, b"b\na\n").word(), "differed");
    assert_eq!(judge_file(&sorted, b"a\n\nb\n").word(), "differed");
    let zip = Expect::Signature {
        format: "zip".into(),
    };
    assert_eq!(judge_file(&zip, b"PK\x03\x04rest").word(), "matched");
    assert_eq!(judge_file(&zip, b"<html>").word(), "differed");
    let header = br#"{"w":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}"#;
    let mut tensors = (header.len() as u64).to_le_bytes().to_vec();
    tensors.extend_from_slice(header);
    tensors.extend_from_slice(&[0, 0, 0, 0]);
    let safetensors = Expect::Signature {
        format: "safetensors".into(),
    };
    assert_eq!(judge_file(&safetensors, &tensors).word(), "matched");
    assert_eq!(
        judge_file(&safetensors, b"\xff\xff\xff\xff\xff\xff\xff\xff{}").word(),
        "differed"
    );
}

#[test]
fn diffs_are_bounded_and_similarity_is_positional() {
    let long_a = "a\n".repeat(5000);
    let long_b = "b\n".repeat(5000);
    let text = diff(&long_a, &long_b);
    assert!(text.chars().count() <= DIFF_CHARS + 1);
    assert!(text.contains("line 1:"));
    assert!((similarity("abcd", "abxd") - 0.75).abs() < 1e-9);
    assert!((similarity("", "") - 1.0).abs() < 1e-9);
}

#[test]
fn the_call_and_score_follow_the_frozen_rule() {
    let result = |outcome: Outcome| Checked {
        id: "K".into(),
        kind: Kind::Path,
        outcome,
        milliseconds: 0,
    };
    let matched = result(Outcome::Matched {
        observed: String::new(),
    });
    let differed = result(Outcome::Differed {
        diff: String::new(),
        similarity: None,
    });
    let open = result(Outcome::CouldNotRun { why: String::new() });
    assert_eq!(call(&[matched.clone(), open.clone()]), Some("pass"));
    assert_eq!(call(&[matched.clone(), differed.clone()]), Some("fail"));
    assert_eq!(call(std::slice::from_ref(&open)), None);
    assert_eq!(score(&[matched, differed]), Some(0.5));
    assert_eq!(score(&[open]), None);
}

#[test]
fn documents_state_commands_in_blocks_usage_lines_and_prompts() {
    let found = extract::doc_commands(
        "Intro.\n\n```bash\n$ make build\n# a comment\n```\n\nlocal command: make -C /app repro\n$ ./check.sh --fast\nUsage:\n  python tool.py IN OUT\n\nnot a command\n",
    );
    let commands: Vec<&str> = found.iter().map(|(c, _)| c.as_str()).collect();
    assert_eq!(
        commands,
        vec![
            "make build",
            "make -C /app repro",
            "./check.sh --fast",
            "python tool.py IN OUT"
        ]
    );
}

#[tokio::test]
async fn a_local_run_is_confined_or_refused() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let host = Local {
        workdir: dir.path().to_path_buf(),
    };
    let ran = host
        .run("echo contract", std::time::Duration::from_secs(20))
        .await;
    match &ran.failed {
        Some(why) => assert!(why.contains("boundary") || why.contains("sandbox"), "{why}"),
        None => {
            assert_eq!(ran.exit, Some(0), "{ran:?}");
            assert_eq!(ran.stdout.trim(), "contract");
        }
    }
    std::fs::write(dir.path().join("a.txt"), "x").expect("a file");
    let path = dir.path().join("a.txt").display().to_string();
    assert_eq!(host.stat(&path).await, Ok(host::Stat::File(1)));
    assert_eq!(host.read(&path, 10).await, Ok(Some(b"x".to_vec())));
}

#[tokio::test]
async fn a_plan_is_sealed_with_a_digest_of_its_items() {
    let (_, draft) = drafted().await;
    let items = extract::finish(draft, &BTreeMap::new());
    let a = Plan::seal("t", "/app", INSTRUCTION, items.clone(), Vec::new());
    let b = Plan::seal("t", "/app", INSTRUCTION, items, Vec::new());
    assert_eq!(a.digest, b.digest);
    assert_eq!(a.schema, PLAN_SCHEMA);
    let text = serde_json::to_string(&a).expect("serializes");
    let back: Plan = serde_json::from_str(&text).expect("reads back");
    assert_eq!(back, a);
}
