use super::*;
use background::{Layout, plugins};
use coder::package::Package;
use nostr::domain::{Event, RelaySigner};
use openagents_chat::client::{Coder, Ran};
use openagents_chat::plugin_flow::{Flow, Step, Test};
use openagents_chat::plugin_workbench::{Action, Engine, Outcome, Record, Source};
use route_contract::digest::Digest;
use route_contract::snapshot::CapabilityPin;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Relay {
    events: Vec<Event>,
}
impl crate::plugin_registry::Registry for Relay {
    fn query(&mut self, filter: Value) -> Result<Vec<Event>, String> {
        Ok(self
            .events
            .iter()
            .filter(|e| {
                filter["kinds"]
                    .as_array()
                    .is_none_or(|kinds| kinds.iter().any(|kind| kind == e.kind))
                    && filter["authors"]
                        .as_array()
                        .is_none_or(|keys| keys.iter().any(|key| key == &e.pubkey))
            })
            .cloned()
            .collect())
    }
    fn send(&mut self, event: Event) -> Result<(), String> {
        event.validate_crypto().map_err(|e| format!("{e:?}"))?;
        self.events.push(event);
        Ok(())
    }
}
struct Scratch {
    layout: Layout,
    signer: RelaySigner,
    relay: Mutex<Relay>,
    calls: Mutex<Vec<Vec<String>>>,
    workspace: PathBuf,
}
#[derive(Clone)]
struct ScratchEngine(Arc<Scratch>);
impl std::ops::Deref for ScratchEngine {
    type Target = Scratch;
    fn deref(&self) -> &Scratch {
        &self.0
    }
}
impl Engine for ScratchEngine {
    fn tests(&self, dir: &Path) -> Result<Vec<Test>, String> {
        coder::task::chat_client::Here.plugin_tests(dir)
    }
    fn run(&self, argv: &[String], _: std::time::Duration) -> Result<Ran, String> {
        self.calls.lock().unwrap().push(argv.to_vec());
        let value = match argv[1].as_str() {
            "test" => {
                return compare(
                    Path::new(&argv[3]),
                    Path::new(&argv[6]),
                    &self.workspace,
                    self.signer.pubkey(),
                );
            }
            "publish" => {
                let packed =
                    crate::plugin_registry::pack(Path::new(&argv[2]), self.signer.pubkey())?;
                let blobs =
                    crate::plugin_registry::DirStore::new(self.layout.home.join("blobs"), None);
                let published = crate::plugin_registry::publish(
                    &packed,
                    &self.signer,
                    &mut *self.relay.lock().unwrap(),
                    &blobs,
                    None,
                    1_790_000_000,
                )?;
                json!({"release":published.release.id,"listing":published.listing.id})
            }
            "install" => crate::plugin_local::install_into(&self.layout, Path::new(&argv[2]))?,
            "enable" => crate::plugin_local::enable_exact_in(
                &self.layout,
                &argv[2],
                true,
                Some((&argv[4], &argv[6])),
            )?,
            "use" => {
                let pin = CapabilityPin {
                    id: argv[2].clone(),
                    version: argv[4].clone(),
                    digest: Digest::try_from(argv[6].clone())?,
                };
                crate::plugin_use::scratch_workbench_use(
                    &self.layout,
                    pin,
                    &argv[8],
                    Path::new(&argv[12]),
                    &argv[10],
                    &self.layout.home.join("routes"),
                    &self.layout.home.join("artifacts"),
                )?
            }
            _ => return Err("Unsupported scratch command.".into()),
        };
        Ok(Ran {
            ok: true,
            output: serde_json::to_string(&value).unwrap(),
        })
    }
}

/// Script the author and baseline only; resolve, evaluate, sign, install,
/// enable, and independent read-only workflow reuse use the existing engines.
#[test]
fn scratch_missing_capability_reaches_exact_publish_install_enable_and_independent_reuse() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    fs::create_dir(&home).unwrap();
    let workspace = temp.path().join("independent-work");
    fs::create_dir_all(workspace.join("src")).unwrap();
    fs::write(
        workspace.join("src/lib.rs"),
        "pub fn size(secret: &str) -> bool {\n    secret.len() >= \"8\"\n}\n",
    )
    .unwrap();
    let authored_cases = temp.path().join("authored-test-fixtures");
    fs::create_dir_all(authored_cases.join("src")).unwrap();
    fs::write(
        authored_cases.join("src/lib.rs"),
        "pub fn size(secret: &str) -> bool {\n    secret.len() >= \"8\"\n}\n",
    )
    .unwrap();
    fs::write(
        workspace.join("src/new_task.rs"),
        "fn independent() {\n    let retries: u32 = 3;\n    let label: &str = retries;\n}\n",
    )
    .unwrap();
    let signer = RelaySigner::from_secret_hex(&"03".repeat(32)).unwrap();
    let draft = temp.path().join("author-task/plugins/missing-error");
    fs::create_dir_all(draft.join("programs")).unwrap();
    fs::create_dir_all(draft.join("skills")).unwrap();
    let program = include_str!("../../../plugin-explain-error/programs/explain-error.json");
    fs::write(draft.join("programs/explain-error.json"), program).unwrap();
    fs::write(
        draft.join("skills/error.md"),
        "Use the workflow to inspect an error's cited file.",
    )
    .unwrap();
    fs::write(
        draft.join("README.md"),
        "Inspect compiler errors; read files only.",
    )
    .unwrap();
    for case in ["one", "two", "three"] {
        let dir = draft.join("evals").join(case);
        fs::create_dir_all(dir.join("graders")).unwrap();
        fs::write(dir.join("prompt.md"),"+++\nv = \"openagents.eval-case.v1\"\nkind = \"should-fire\"\n[run]\nallowed_operations = [\"read\"]\n+++\nerror[E0308]: mismatched types\n --> src/lib.rs:2:22\n").unwrap();
        fs::write(
            dir.join("graders/error.md"),
            "+++\ntype = \"regex\"\ntarget = \"last_message\"\nmatch = \"contains\"\n+++\nE0308\n",
        )
        .unwrap();
    }
    let package = json!({"v":1,"slug":"missing-error","version":"0.1.0","publisher":signer.pubkey(),"program":{"name":"explain-error","digest":coder::package::digest(program)}});
    fs::write(
        draft.join("package.json"),
        serde_json::to_vec(&package).unwrap(),
    )
    .unwrap();
    let backend = Arc::new(Scratch {
        layout: Layout::new(&home, None).unwrap(),
        signer: signer.clone(),
        relay: Mutex::new(Relay::default()),
        calls: Mutex::new(Vec::new()),
        workspace: authored_cases,
    });
    let root = temp.path().join("flow");
    let owner = Owner::open(root.clone(), ScratchEngine(backend.clone()));
    let source = Source {
        flow: "missing-capability".into(),
        thread: "author-thread".into(),
        task: "author-task".into(),
    };
    let mut snapshot = openagents_chat::service::Snapshot::default();
    snapshot.chat = Some(source.thread.clone());
    snapshot.coder = Some(openagents_chat::basic_chats::Spawned {
        host: "local".into(),
        task: source.task.clone(),
        project: None,
        at: None,
    });
    snapshot
        .turns
        .push(openagents_chat::basic_coder::Turn::user(
            "Make a missing capability for explaining compiler errors.",
        ));
    let mut reply = openagents_chat::basic_coder::Turn::assistant(
        "Drafting the package and tests in the task's worktree.",
        None,
    );
    reply.meta = Some(openagents_chat::router::Meta {
        plugin: Some(Flow::at(Step::Draft, None)),
        ..Default::default()
    });
    snapshot.turns.push(reply);
    let record = owner
        .freeze_routed(
            source.flow.clone(),
            &snapshot,
            &draft,
            Declarations {
                author: signer.pubkey().into(),
                fee_msat: None,
                payout: None,
            },
        )
        .unwrap();
    assert_eq!(record.tests.len(), 3);
    assert_eq!(record.turns, snapshot.turns);
    assert_eq!(record.source, source);
    assert!(backend.calls.lock().unwrap().is_empty());
    for (id, action) in [
        ("comparison", Action::Compare),
        ("publish", Action::Publish),
        ("install", Action::Install),
        ("enable", Action::Enable),
    ] {
        let request = request(&record, id, action.clone());
        let result = owner.apply(request.clone()).unwrap();
        assert!(
            matches!(result, Outcome::Finished { ok: true, .. }),
            "{id}: {result:?}"
        );
        let reopened = Owner::open(root.clone(), ScratchEngine(backend.clone()));
        assert_eq!(reopened.apply(request).unwrap(), result);
        if action == Action::Install {
            assert!(
                !plugins::find(&backend.layout, &record.release.id)
                    .unwrap()
                    .enabled
            );
        }
    }
    assert_eq!(backend.relay.lock().unwrap().events.len(), 2);
    assert_eq!(backend.calls.lock().unwrap().len(), 4);
    let independent = Action::Reuse {
        source_task: "independent-task".into(),
        thread: "independent-thread".into(),
        request: "error[E0308]: mismatched types\n --> src/new_task.rs:3:18\n".into(),
        workspace,
    };
    let reuse = request(&record, "reuse", independent);
    let result = owner.apply(reuse.clone()).unwrap();
    assert!(
        matches!(result, Outcome::Finished { ok: true, .. }),
        "{result:?}"
    );
    assert_eq!(owner.apply(reuse).unwrap(), result);
    assert_eq!(backend.calls.lock().unwrap().len(), 5);
    let kept = owner.read().unwrap();
    assert_eq!(kept.source, source);
    assert_eq!(kept.attempts.len(), 5);
    let executed = kept.attempts["reuse"].reuse.as_ref().unwrap();
    assert_eq!(executed["thread"], "independent-thread");
    assert!(executed["request"].as_str().unwrap().starts_with("use-"));
    assert!(!executed["outputs"].as_array().unwrap().is_empty());
    assert!(executed["text"].as_str().unwrap().contains("new_task.rs"));
    let comparison = kept.attempts["comparison"].comparison.as_ref().unwrap();
    assert!(
        comparison
            .measurements
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["arm"] == "baseline" && m["metric"] == "cost_usd")
    );
    assert!(
        comparison
            .measurements
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["arm"] == "subject" && m["metric"] == "cost_usd")
    );
}
fn request(record: &Record, id: &str, action: Action) -> Request {
    Request {
        id: id.into(),
        source: record.source.clone(),
        release: record.release.clone(),
        tree: record.tree.clone(),
        action,
    }
}
fn compare(dir: &Path, output: &Path, workspace: &Path, author: &str) -> Result<Ran, String> {
    use ext_eval::{
        Arm, ArmSetup, ArtifactRef, Doors, Identity, LoadOptions, Plan, RunOutcome, RunRecord,
        Suite, Trajectory,
    };
    let suite =
        Suite::load(&dir.join("evals"), LoadOptions::default()).map_err(|e| e.to_string())?;
    let plan = Plan {
        runs: Some(1),
        ..Plan::default()
    };
    let package = Package::load(&dir.join("package.json"))?;
    let package_bytes = fs::read(dir.join("package.json")).map_err(|e| e.to_string())?;
    let definition = |id: &str, bytes: &[u8]| json!({"id":format!("{author}:{id}"),"artifact":ArtifactRef::of(bytes,"application/json",Some("openagents.ext-package.v1")).value()});
    let setup = |id: &str, bytes: &[u8]| ArmSetup {
        definition: definition(id, bytes),
        lock: ArtifactRef::of(
            b"scratch lock",
            "application/json",
            Some("openagents.lock.v1"),
        ),
        door: "scripted-author-and-baseline".into(),
        run: json!({"allowed_operations":["read"]}),
    };
    let identity = Identity {
        author: author.into(),
        package: package.slug.clone(),
        component: "authored-tests".into(),
        evaluator: author.into(),
        subject: setup(&format!("{}/explain-error", package.slug), &package_bytes),
        baseline: Some(setup("coder/coder", b"scripted baseline")),
        started_at: 1_790_000_000,
        ended_at: 1_790_000_001,
        requester: None,
        suite_release: None,
        environment: None,
        defaults: None,
        partial: None,
    };
    let mut runs = Vec::new();
    for (case, arm, attempt) in plan.attempts(&suite) {
        let prompt = &suite.case(&case).unwrap().prompt;
        let message = if arm == Arm::Subject {
            crate::ext_run::execute(dir, workspace, prompt)?.to_string()
        } else {
            "No plugin admitted in the scripted baseline.".into()
        };
        let document = json!({"schema_version":"ATIF-v1.8","session_id":format!("{case}-{arm:?}"),"trajectory_id":format!("{case}-{arm:?}"),"agent":{"name":"scratch","version":"1","model_name":"scripted"},"steps":[{"step_id":1,"timestamp":"2026-10-06T00:00:00Z","source":"user","message":prompt},{"step_id":2,"timestamp":"2026-10-06T00:00:01Z","source":"agent","message":message,"model_name":"scripted"}]});
        let mut record = RunRecord::new(&case, arm, attempt, RunOutcome::Completed);
        record.trajectory = Some(Trajectory::from_bytes(
            &serde_json::to_vec(&document).unwrap(),
        )?);
        record.cost_usd = Some(0.0);
        record.seconds = Some(0.001);
        runs.push(record);
    }
    let gate = ext_eval::load_gate().map_err(|e| e.to_string())?;
    let evaluation = ext_eval::evaluate(
        &suite,
        &plan,
        runs,
        &identity,
        (&gate.0, &gate.1),
        Doors {
            decision: None,
            judge: None,
            replayer: None,
        },
    )
    .map_err(|e| e.to_string())?;
    evaluation
        .write(&output.join("run1"))
        .map_err(|e| e.to_string())?;
    Ok(Ran {
        ok: true,
        output: format!(
            "Scripted baseline vs actual bounded workflow: {:?}",
            evaluation.scores
        ),
    })
}
