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

fn usage() -> microluna::TokenUsage {
    microluna::TokenUsage {
        input: 100,
        cached: 0,
        output: 10,
        reasoning: 0,
    }
}

fn run_call(id: &str, command: &str) -> microluna::Reply {
    microluna::fake::call(
        id,
        "run_command",
        &json!({ "command": command, "timeout_seconds": 20 }),
        usage(),
    )
}

fn finish_call(id: &str) -> microluna::Reply {
    microluna::fake::call(
        id,
        "finish",
        &json!({ "status": "done", "summary": "wrote it", "answer": "" }),
        usage(),
    )
}

/// The writer's commands read its own directory and the task files the
/// caller grants, and nothing beside them: a retained candidate's output
/// is out of reach, and a search of the temporary and home directories
/// finds it nowhere. The model is a scripted fake.
#[tokio::test]
async fn the_writer_reads_only_its_directory_and_the_granted_task_files() {
    let root = tempfile::tempdir().unwrap();
    let task = root.path().join("task");
    let candidate = root.path().join("candidate");
    std::fs::create_dir_all(&task).unwrap();
    std::fs::create_dir_all(&candidate).unwrap();
    std::fs::write(task.join("instruction.md"), "Sum the inputs.\n").unwrap();
    std::fs::write(candidate.join("out.step"), "a candidate\n").unwrap();
    let dir = root.path().join("writer");
    std::fs::create_dir_all(&dir).unwrap();
    if let Err(error) = coder_boundary::Boundary::writing(&dir)
        .confining_reads()
        .offline()
        .build()
    {
        eprintln!("skipped: no enforced boundary on this host ({error})");
        return;
    }
    let transport = microluna::fake::FakeTransport::new(vec![
        run_call(
            "c1",
            &format!(
                "cp spec.json spec-seen.json && cp {}/instruction.md seen.md",
                task.display()
            ),
        ),
        run_call(
            "c2",
            &format!("cp {}/out.step leaked.step", candidate.display()),
        ),
        run_call(
            "c3",
            "find /tmp /home /root -name out.step > found.txt 2>/dev/null; true",
        ),
        microluna::fake::call(
            "c4",
            "write_file",
            &json!({ "path": "oracle.py", "contents": "print()\n" }),
            usage(),
        ),
        finish_call("c5"),
    ]);
    let bounds = write::Bounds {
        readable: vec![task.clone()],
        ..write::Bounds::default()
    };
    let (oracle, record) = write::write(&transport, &spec_with(&["O1"]), &dir, &bounds, None)
        .await
        .unwrap();
    assert!(oracle.is_some(), "{record}");
    assert_eq!(record["bounds"]["reads"], json!("confined"));
    assert!(dir.join("spec-seen.json").exists());
    assert_eq!(
        std::fs::read_to_string(dir.join("seen.md")).unwrap(),
        "Sum the inputs.\n"
    );
    assert!(!dir.join("leaked.step").exists());
    let found = std::fs::read_to_string(dir.join("found.txt")).unwrap();
    assert!(found.trim().is_empty(), "{found}");
}

/// A task container can't confine reads, so a writer there without a
/// container of its own is refused before any model call.
#[tokio::test]
async fn the_writer_runs_no_command_in_a_task_container() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("writer");
    let transport = microluna::fake::FakeTransport::new(vec![
        run_call("c1", "echo x > ran.txt"),
        finish_call("c2"),
    ]);
    let bounds = write::Bounds {
        isolation: microluna::Isolation::TaskContainer,
        ..write::Bounds::default()
    };
    let (oracle, record) = write::write(&transport, &spec_with(&["O1"]), &dir, &bounds, None)
        .await
        .unwrap();
    assert!(oracle.is_none());
    assert!(!dir.join("ran.txt").exists(), "{record}");
    assert!(transport.requests().is_empty(), "{record}");
    assert_eq!(record["isolation"], json!("task-container"));
    assert!(
        record["refused"]
            .as_str()
            .unwrap()
            .contains("could read the candidate's files"),
        "{record}"
    );
}

/// Docker as a script: it answers each call from an in-memory writer
/// directory and records every call.
#[derive(Debug)]
struct FakeDocker {
    calls: std::sync::Mutex<Vec<Vec<String>>>,
    files: std::sync::Mutex<BTreeMap<String, Vec<u8>>>,
    /// What `docker image inspect` fails with, when it fails.
    image_error: Option<String>,
    /// What `docker inspect` says: the mounts, then the network mode.
    inspect: String,
}

impl Default for FakeDocker {
    fn default() -> Self {
        FakeDocker {
            calls: std::sync::Mutex::default(),
            files: std::sync::Mutex::default(),
            image_error: None,
            inspect: "[]|none".to_string(),
        }
    }
}

impl FakeDocker {
    fn calls(&self) -> Vec<Vec<String>> {
        self.calls.lock().unwrap().clone()
    }

    fn relative(path: &str) -> String {
        path.strip_prefix(&format!("{}/", contain::ROOT))
            .unwrap_or(path)
            .to_string()
    }
}

impl contain::Docker for FakeDocker {
    fn call(&self, args: &[String], input: Option<&[u8]>) -> Result<Vec<u8>, String> {
        self.calls.lock().unwrap().push(args.to_vec());
        let has = |word: &str| args.iter().any(|a| a == word);
        let last = args.last().cloned().unwrap_or_default();
        let mut files = self.files.lock().unwrap();
        match args[0].as_str() {
            "image" => match &self.image_error {
                None => Ok(b"sha256:image\n".to_vec()),
                Some(error) => Err(error.clone()),
            },
            "create" => Ok(b"cid\n".to_vec()),
            "start" | "rm" => Ok(Vec::new()),
            "inspect" => Ok(self.inspect.clone().into_bytes()),
            "exec" if has("-i") => {
                files.insert(last, input.unwrap_or_default().to_vec());
                Ok(Vec::new())
            }
            "exec" if has("test") => files
                .contains_key(&Self::relative(&last))
                .then(Vec::new)
                .ok_or_else(|| "exit 1".to_string()),
            "exec" if args.iter().any(|a| a.contains("head -c")) => files
                .get(&last)
                .cloned()
                .ok_or_else(|| format!("{last}: no such file")),
            "exec" if has("rm") => {
                files.remove(&last);
                Ok(Vec::new())
            }
            "exec" => Ok(Vec::new()),
            "cp" => {
                let (_, from) = args[2].split_once(':').unwrap();
                let bytes = files
                    .get(&Self::relative(from))
                    .cloned()
                    .ok_or("no such file")?;
                std::fs::write(&args[3], bytes).map_err(|e| e.to_string())?;
                Ok(Vec::new())
            }
            other => Err(format!("unexpected docker {other}")),
        }
    }

    fn bounded(&self, args: Vec<String>, _wall: Duration) -> contain::Running<'_> {
        self.calls.lock().unwrap().push(args);
        Box::pin(async {
            supervise::Ended {
                ending: supervise::Ending::Exited(Some(0)),
                stdout: supervise::Captured {
                    text: "ran in the writer's container".to_string(),
                    bytes: 29,
                    truncated: false,
                },
                stderr: supervise::Captured::default(),
                elapsed: Duration::ZERO,
                memory: None,
            }
        })
    }
}

struct Contained {
    _root: tempfile::TempDir,
    dir: std::path::PathBuf,
    candidate: std::path::PathBuf,
    artifacts: std::path::PathBuf,
    docker: std::sync::Arc<FakeDocker>,
}

fn contained(docker: FakeDocker) -> (Contained, write::Bounds) {
    let root = tempfile::tempdir().unwrap();
    let candidate = root.path().join("candidate-app");
    let artifacts = root.path().join("retained-artifacts");
    std::fs::create_dir_all(&candidate).unwrap();
    std::fs::create_dir_all(&artifacts).unwrap();
    std::fs::write(candidate.join("out.step"), "a candidate\n").unwrap();
    let docker = std::sync::Arc::new(docker);
    let bounds = write::Bounds {
        isolation: microluna::Isolation::TaskContainer,
        container: Some(contain::Image {
            reference: "tbench-warm/t:environment-1".to_string(),
            withheld: vec![candidate.clone(), artifacts.clone()],
            docker: docker.clone(),
        }),
        ..write::Bounds::default()
    };
    let dir = artifacts.join("oracle-1");
    (
        Contained {
            dir,
            candidate,
            artifacts,
            docker,
            _root: root,
        },
        bounds,
    )
}

/// The writer's container is made with no network and no mount, its
/// commands and files stay inside it, nothing names the candidate's or
/// the records' paths, and the one thing copied out is `oracle.py`.
#[tokio::test]
async fn the_writer_container_mounts_nothing_and_copies_out_only_the_oracle() {
    let (setup, bounds) = contained(FakeDocker::default());
    let transport = microluna::fake::FakeTransport::new(vec![
        run_call("c1", "ls /app && python3 oracle.py /app cases.json"),
        microluna::fake::call(
            "c2",
            "write_file",
            &json!({ "path": "oracle.py", "contents": "print('oracle')\n" }),
            usage(),
        ),
        microluna::fake::call(
            "c3",
            "write_file",
            &json!({ "path": "scratch/work.txt", "contents": "kept inside\n" }),
            usage(),
        ),
        finish_call("c4"),
    ]);
    let (oracle, record) = write::write(&transport, &spec_with(&["O1"]), &setup.dir, &bounds, None)
        .await
        .unwrap();
    let oracle = oracle.expect("an oracle");
    assert_eq!(oracle.files["oracle.py"], "print('oracle')\n");
    assert_eq!(record["isolation"], json!("writer-container"), "{record}");
    assert_eq!(record["container"]["network"], json!("none"));
    assert_eq!(record["container"]["copied_out"], json!(["oracle.py"]));
    assert_eq!(record["bounds"]["reads"], json!("container"));
    let calls = setup.docker.calls();
    let create = calls.iter().find(|c| c[0] == "create").unwrap();
    let network = create.iter().position(|a| a == "--network").unwrap();
    assert_eq!(create[network + 1], "none");
    for call in &calls {
        for arg in call {
            assert!(
                !matches!(arg.as_str(), "-v" | "--volume" | "--mount" | "--privileged")
                    && !arg.starts_with("--volume")
                    && !arg.starts_with("--mount")
                    && !arg.starts_with("--network=")
                    && !arg.starts_with("--volumes-from"),
                "{call:?}"
            );
            for withheld in [&setup.candidate, &setup.artifacts] {
                // The one path under the records is where oracle.py lands.
                if call[0] == "cp" && arg == &setup.dir.join("oracle.py").display().to_string() {
                    continue;
                }
                assert!(
                    !arg.contains(&withheld.display().to_string()),
                    "{call:?} names {}",
                    withheld.display()
                );
            }
        }
    }
    let copies: Vec<_> = calls.iter().filter(|c| c[0] == "cp").collect();
    assert_eq!(copies.len(), 1, "{copies:?}");
    assert_eq!(copies[0][2], format!("cid:{}/oracle.py", contain::ROOT));
    assert_eq!(
        copies[0][3],
        setup.dir.join("oracle.py").display().to_string()
    );
    // The command ran in the container's writer directory.
    let ran = calls
        .iter()
        .find(|c| c.iter().any(|a| a.contains("python3 oracle.py")))
        .unwrap();
    assert_eq!(&ran[..4], ["exec", "-w", contain::ROOT, "cid"]);
    // What the session made stayed in the container; on the host the
    // writer's directory holds the spec, the cases, and the oracle only.
    assert!(
        setup
            .docker
            .files
            .lock()
            .unwrap()
            .contains_key("scratch/work.txt")
    );
    let mut on_host: Vec<_> = std::fs::read_dir(&setup.dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    on_host.sort();
    assert_eq!(on_host, ["cases.json", "oracle.py", "spec.json"]);
    assert_eq!(calls.last().unwrap(), &["rm", "-f", "cid"]);
    assert_eq!(calls.iter().filter(|c| c[0] == "rm").count(), 1);
}

/// With no image to start from, the writer is refused before any model
/// call or container.
#[tokio::test]
async fn the_writer_is_refused_without_an_image() {
    let (setup, mut bounds) = contained(FakeDocker::default());
    bounds.container.as_mut().unwrap().reference = String::new();
    let transport = microluna::fake::FakeTransport::new(vec![finish_call("c1")]);
    let (oracle, record) = write::write(&transport, &spec_with(&["O1"]), &setup.dir, &bounds, None)
        .await
        .unwrap();
    assert!(oracle.is_none());
    assert!(transport.requests().is_empty());
    assert!(setup.docker.calls().is_empty());
    assert_eq!(record["isolation"], json!("writer-container"));
    assert!(
        record["refused"]
            .as_str()
            .unwrap()
            .contains("the task's image isn't known"),
        "{record}"
    );
}

/// An image that isn't on this machine is never pulled: the writer is
/// refused and no container is made.
#[tokio::test]
async fn the_writer_is_refused_when_the_image_is_missing() {
    let (setup, bounds) = contained(FakeDocker {
        image_error: Some("No such image".to_string()),
        ..FakeDocker::default()
    });
    let transport = microluna::fake::FakeTransport::new(vec![finish_call("c1")]);
    let (oracle, record) = write::write(&transport, &spec_with(&["O1"]), &setup.dir, &bounds, None)
        .await
        .unwrap();
    assert!(oracle.is_none());
    assert!(transport.requests().is_empty());
    assert!(setup.docker.calls().iter().all(|c| c[0] != "create"));
    assert!(
        record["refused"]
            .as_str()
            .unwrap()
            .contains("isn't on this machine"),
        "{record}"
    );
}

/// Where no Docker client runs, as inside a Harbor trial's task container,
/// the writer is refused and says why.
#[tokio::test]
async fn the_writer_is_refused_when_docker_cant_be_reached() {
    let (setup, bounds) = contained(FakeDocker {
        image_error: Some(format!("{}: No such file", contain::UNREACHABLE)),
        ..FakeDocker::default()
    });
    let transport = microluna::fake::FakeTransport::new(vec![finish_call("c1")]);
    let (oracle, record) = write::write(&transport, &spec_with(&["O1"]), &setup.dir, &bounds, None)
        .await
        .unwrap();
    assert!(oracle.is_none());
    assert!(transport.requests().is_empty());
    let why = record["refused"].as_str().unwrap();
    assert!(why.contains("Docker can't be reached"), "{why}");
    assert!(!why.contains("isn't on this machine"), "{why}");
}

/// A container with a mount from the host or a network is removed, and
/// the writer is refused.
#[tokio::test]
async fn the_writer_is_refused_when_its_container_sees_the_host() {
    let cases = [
        (
            r#"[{"Type":"bind","Source":"/srv/app","Destination":"/app"}]|none"#.to_string(),
            "bind mount at /app",
        ),
        ("[]|bridge".to_string(), "network is 'bridge'"),
    ];
    for (inspect, expected) in cases {
        let (setup, bounds) = contained(FakeDocker {
            inspect,
            ..FakeDocker::default()
        });
        let transport = microluna::fake::FakeTransport::new(vec![finish_call("c1")]);
        let (oracle, record) =
            write::write(&transport, &spec_with(&["O1"]), &setup.dir, &bounds, None)
                .await
                .unwrap();
        assert!(oracle.is_none());
        assert!(transport.requests().is_empty());
        assert!(
            record["refused"].as_str().unwrap().contains(expected),
            "{record}"
        );
        let calls = setup.docker.calls();
        assert_eq!(calls.last().unwrap(), &["rm", "-f", "cid"], "{calls:?}");
    }
    // A volume whose source is the candidate is refused too.
    let (setup, mut bounds) = contained(FakeDocker::default());
    let source = setup.candidate.display().to_string();
    let docker = std::sync::Arc::new(FakeDocker {
        inspect: format!(r#"[{{"Type":"volume","Source":"{source}","Destination":"/app"}}]|none"#),
        ..FakeDocker::default()
    });
    bounds.container.as_mut().unwrap().docker = docker.clone();
    let transport = microluna::fake::FakeTransport::new(vec![finish_call("c1")]);
    let (_, record) = write::write(&transport, &spec_with(&["O1"]), &setup.dir, &bounds, None)
        .await
        .unwrap();
    assert!(
        record["refused"]
            .as_str()
            .unwrap()
            .contains("holds the candidate's files"),
        "{record}"
    );
    assert_eq!(docker.calls().last().unwrap(), &["rm", "-f", "cid"]);
}

/// A real container of a small local image: commands and files stay in
/// it, it has no network, and `oracle.py` comes out. Needs Docker and a
/// local `alpine:3`, which it never pulls:
/// `cargo test -p coder-one -- --ignored a_real_writer_container`.
#[tokio::test]
#[ignore = "needs Docker and a local alpine:3 image"]
async fn a_real_writer_container_keeps_everything_but_the_oracle() {
    use microluna::Remote;
    let image = contain::Image {
        reference: "alpine:3".to_string(),
        withheld: Vec::new(),
        docker: std::sync::Arc::new(contain::Cli),
    };
    let name = format!("oracle-writer-test-{}", std::process::id());
    let container = contain::WriterContainer::start(&image, &name).unwrap();
    container.put("spec.json", b"{}\n").unwrap();
    let ran = container
        .run(
            "cat spec.json && echo made > made.txt && ls /sys/class/net && \
             (wget -q -T 3 -O /dev/null http://example.com && echo online || echo offline)",
            Duration::from_secs(20),
        )
        .await;
    assert!(ran.ending.success(), "{ran:?}");
    assert!(ran.stdout.text.contains("{}"), "{}", ran.stdout.text);
    assert!(ran.stdout.text.contains("offline"), "{}", ran.stdout.text);
    assert!(!ran.stdout.text.contains("eth0"), "{}", ran.stdout.text);
    assert_eq!(container.read("made.txt", 100).unwrap(), b"made\n");
    let root = tempfile::tempdir().unwrap();
    assert_eq!(
        container.copy_out(&root.path().join("oracle.py")).unwrap(),
        None
    );
    container.put("oracle.py", b"print('ok')\n").unwrap();
    let copied = container.copy_out(&root.path().join("oracle.py")).unwrap();
    assert_eq!(copied.as_deref(), Some("print('ok')\n"));
    assert_eq!(container.describe()["mounts"], json!([]));
    let id = container.id().to_string();
    drop(container);
    assert!(
        crate::accept::runner::docker(&["inspect", &id]).is_err(),
        "the container is removed"
    );
    // The whole writer, with a scripted model: a command writes the
    // oracle inside the container, and only it comes out.
    let transport = microluna::fake::FakeTransport::new(vec![
        run_call(
            "c1",
            "test -f cases.json && echo \"print('from the container')\" > oracle.py && echo x > left.txt",
        ),
        finish_call("c2"),
    ]);
    let bounds = write::Bounds {
        isolation: microluna::Isolation::TaskContainer,
        container: Some(image),
        ..write::Bounds::default()
    };
    let dir = root.path().join("writer");
    let (oracle, record) = write::write(&transport, &spec_with(&["O1"]), &dir, &bounds, None)
        .await
        .unwrap();
    let oracle = oracle.unwrap_or_else(|| panic!("{record}"));
    assert_eq!(oracle.files["oracle.py"], "print('from the container')\n");
    assert_eq!(record["isolation"], json!("writer-container"));
    assert!(!dir.join("left.txt").exists());
    let left = crate::accept::runner::docker(&[
        "ps",
        "-aq",
        "--filter",
        &format!("label={}", contain::LABEL),
    ])
    .unwrap();
    assert!(left.is_empty(), "{left}");
}

/// Places `oracle` and `spec` as the harness does, and returns the plan the
/// loop would read with `digest` in its environment.
fn delivered(
    oracle: &Oracle,
    spec: Option<&Spec>,
    digest: &str,
) -> (tempfile::TempDir, deliver::Plan) {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("oracle.json"),
        serde_json::to_string_pretty(oracle).unwrap(),
    )
    .unwrap();
    if let Some(spec) = spec {
        std::fs::write(
            root.path().join("spec.json"),
            serde_json::to_string_pretty(spec).unwrap(),
        )
        .unwrap();
    }
    let dir = root.path().display().to_string();
    let digest = digest.to_string();
    let plan = deliver::plan(
        |name| match name {
            deliver::DIR_ENV => Some(dir.clone()),
            deliver::DIGEST_ENV => Some(digest.clone()),
            _ => None,
        },
        true,
    );
    (root, plan)
}

#[test]
fn a_delivered_oracle_with_its_recorded_digest_is_used() {
    let spec = spec_with(&["O1", "P1"]);
    let oracle = written(&spec);
    let (root, plan) = delivered(&oracle, Some(&spec), &oracle.digest);
    let (found, record) = deliver::receive(&plan);
    let (got, got_spec) = found.expect("the oracle is used");
    assert_eq!(got.digest, oracle.digest);
    assert_eq!(got.files, oracle.files);
    assert_eq!(got_spec.unwrap().digest, spec.digest);
    assert_eq!(record["status"], "delivered");
    assert_eq!(record["verified"], true);
    assert_eq!(record["digest"], oracle.digest.as_str());
    // The loop runs it from the verified files, staged in its own place.
    let stage = root.path().join("stage");
    deliver::stage(&got, &stage).unwrap();
    assert_eq!(
        std::fs::read_to_string(stage.join("oracle.py")).unwrap(),
        "print()"
    );
}

#[test]
fn a_delivered_oracle_whose_digest_differs_from_the_record_is_refused() {
    let spec = spec_with(&["O1"]);
    let oracle = written(&spec);
    let (_root, plan) = delivered(&oracle, Some(&spec), &"0".repeat(64));
    let (found, record) = deliver::receive(&plan);
    assert!(found.is_none());
    assert_eq!(record["status"], "refused");
    assert_eq!(record["verified"], false);
    assert!(record["why"].as_str().unwrap().contains("host recorded"));
}

#[test]
fn a_changed_oracle_file_is_refused_even_with_its_old_digest() {
    let spec = spec_with(&["O1"]);
    let oracle = written(&spec);
    let mut changed = oracle.clone();
    changed
        .files
        .insert("oracle.py".to_string(), "print('passed')".to_string());
    let (_root, plan) = delivered(&changed, Some(&spec), &oracle.digest);
    let (found, record) = deliver::receive(&plan);
    assert!(found.is_none());
    assert_eq!(record["status"], "refused");
    assert!(record["why"].as_str().unwrap().contains("digest to"));
}

#[test]
fn a_delivered_oracle_with_another_spec_is_refused() {
    let spec = spec_with(&["O1"]);
    let oracle = written(&spec);
    let other = spec_with(&["O1", "P9"]);
    let (_root, plan) = delivered(&oracle, Some(&other), &oracle.digest);
    let (found, record) = deliver::receive(&plan);
    assert!(found.is_none());
    assert_eq!(record["status"], "refused");
}

#[test]
fn a_missing_delivered_oracle_leaves_the_loop_without_one_and_says_so() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("absent").display().to_string();
    let plan = deliver::plan(
        |name| match name {
            deliver::DIR_ENV => Some(dir.clone()),
            deliver::DIGEST_ENV => Some("a".repeat(64)),
            _ => None,
        },
        true,
    );
    let (found, record) = deliver::receive(&plan);
    assert!(found.is_none());
    assert_eq!(record["status"], "missing");
    assert!(record["why"].as_str().unwrap().contains("oracle.json"));
}

#[test]
fn a_task_container_never_writes_its_own_oracle() {
    // Nothing delivered, in a task container: no oracle, and no writer.
    let plan = deliver::plan(|_| None, true);
    assert!(matches!(plan, deliver::Plan::Unavailable(_)));
    let (found, record) = deliver::receive(&plan);
    assert!(found.is_none());
    assert_eq!(record["status"], "unavailable");
    // The host said it couldn't write one.
    let plan = deliver::plan(
        |name| (name == deliver::DIGEST_ENV).then(|| deliver::UNAVAILABLE.to_string()),
        true,
    );
    assert!(matches!(plan, deliver::Plan::Unavailable(_)));
    // Outside a task container with nothing delivered, the loop writes it.
    assert_eq!(deliver::plan(|_| None, false), deliver::Plan::WriteHere);
}

#[test]
fn a_found_checker_is_delivered_without_a_spec() {
    let oracle = Oracle {
        schema: String::new(),
        task: "t".to_string(),
        source: Source::Found,
        command: Some("./check.sh".to_string()),
        origin: Some("check.sh".to_string()),
        files: BTreeMap::new(),
        spec: None,
        writer: Value::Null,
        digest: String::new(),
    }
    .sealed();
    let (_root, plan) = delivered(&oracle, None, &oracle.digest);
    let (found, _) = deliver::receive(&plan);
    let (got, spec) = found.unwrap();
    assert_eq!(got.command.as_deref(), Some("./check.sh"));
    assert!(spec.is_none());
}

#[tokio::test]
async fn the_host_step_refuses_an_image_that_isnt_on_the_machine() {
    let root = tempfile::tempdir().unwrap();
    let task_dir = root.path().join("some-task");
    std::fs::create_dir_all(&task_dir).unwrap();
    std::fs::write(task_dir.join("instruction.md"), "Print the sum.\n").unwrap();
    let docker = std::sync::Arc::new(FakeDocker {
        image_error: Some("docker image failed: No such image".to_string()),
        ..FakeDocker::default()
    });
    let options = host::Options {
        task_dir,
        image: "example/task:1".to_string(),
        out: root.path().join("out"),
        workdir: None,
        bounds: write::Bounds::default(),
    };
    let wire: Result<&crate::micro::Wire, String> = Err("no model in tests".to_string());
    let record = host::write_for_trial(
        wire,
        docker.clone(),
        &crate::component::jev::JevMode::Off,
        &options,
    )
    .await
    .unwrap();
    assert_eq!(record["status"], "unavailable");
    assert!(
        record["why"]
            .as_str()
            .unwrap()
            .contains("isn't on this machine")
    );
    assert!(record["digest"].is_null());
    assert!(root.path().join("out/record.json").is_file());
    assert!(!root.path().join("out/oracle.json").exists());
    // Only the image was inspected: nothing was created or pulled.
    assert!(docker.calls().iter().all(|c| c[0] == "image"));
}

/// A registered experiment's protocol is frozen: the file in the checkout
/// still has the digest the registry names, and each id is its directory.
#[test]
fn every_registered_experiment_names_its_frozen_protocol() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut ids = std::collections::BTreeSet::new();
    for experiment in EXPERIMENTS {
        assert!(
            ids.insert(experiment.id),
            "{} is listed twice",
            experiment.id
        );
        assert!(
            experiment.protocol.starts_with(&format!(
                "bench/terminal-bench/experiments/{}/",
                experiment.id
            )),
            "{experiment:?}"
        );
        let bytes = std::fs::read(root.join(experiment.protocol)).unwrap();
        assert_eq!(
            crate::accept::sha256(&bytes),
            experiment.protocol_sha256,
            "{} changed after it was registered",
            experiment.protocol
        );
        assert_eq!(
            preregistered(experiment.id, experiment.protocol_sha256),
            Some(experiment)
        );
    }
    assert!(preregistered("2026-09-25-oracle-live", "").is_none());
}

/// A directory the instruction names contributes the heads of its files,
/// even a file too large to keep whole, so the writer sees the inputs'
/// structure.
#[tokio::test]
async fn a_named_directory_gives_the_writer_its_files_heads() {
    use crate::checks::contract::extract;
    use crate::checks::contract::host::Local;
    let dir = tempfile::tempdir().expect("a directory");
    let data = dir.path().join("data");
    std::fs::create_dir(&data).unwrap();
    let mut big = String::from("record_id,ssn,name\n");
    for i in 0..20_000 {
        big.push_str(&format!("R{i:05},{:09},Name {i}\n", 100_000_000 + i));
    }
    assert!(big.len() > extract::TEXT_MAX);
    std::fs::write(data.join("big.csv"), &big).unwrap();
    std::fs::write(data.join("small.csv"), "id,value\n1,2\n").unwrap();
    let workdir = dir.path().to_string_lossy().to_string();
    let instruction = format!("Two CSV files at `{workdir}/data/` hold the records.");
    let host = Local {
        workdir: dir.path().to_path_buf(),
    };
    let pristine = extract::gather(&host, &instruction, &workdir).await;
    let big_path = format!("{workdir}/data/big.csv");
    let head = pristine
        .heads
        .get(&big_path)
        .expect("the large file's head");
    assert!(head.starts_with("record_id,ssn,name\nR00000,"));
    assert!(head.len() <= extract::HEAD_MAX);
    let inputs = super::define::input_heads(&pristine);
    assert!(
        inputs
            .iter()
            .any(|i| i.path == big_path && i.head.contains("record_id,ssn,name"))
    );
    assert!(inputs.iter().any(|i| i.path.ends_with("small.csv")));
}

#[test]
fn a_command_the_instruction_calls_a_specification_is_a_reference() {
    let instruction = "Repair `/app/tool` so it rebuilds the report. You can probe production \
        behaviour by using the diagnostic black-box command named `legacy-score` that is installed \
        in `PATH`. Treat `legacy-score` as the scorer behavior specification. Write \
        `report.csv` with the scores.";
    let found = super::find::find(instruction, "/app", &Default::default());
    assert_eq!(found.references, ["legacy-score"]);
    let plain = "Run `make-report` to build the report, then write `out.csv`.";
    assert!(
        super::find::find(plain, "/app", &Default::default())
            .references
            .is_empty()
    );
}
