//! Harvesting with a fake model and a fake embedder: no network.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use serde_json::json;

use super::*;
use crate::lint::Corpus;
use crate::search::{Embed, Retriever};
use crate::{Base, Entry, Status};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("knowledge-harvest-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct FakeModel {
    reply: Proposals,
    prompt: RefCell<String>,
}

impl Propose for FakeModel {
    fn model(&self) -> &str {
        "fake/model"
    }

    fn provider(&self) -> &str {
        "fake"
    }

    async fn propose(&self, system: &str, prompt: &str) -> Result<(Proposals, Cost), String> {
        assert!(system.contains("Never name the task"));
        *self.prompt.borrow_mut() = prompt.to_string();
        Ok((self.reply.clone(), Cost::known(0.002, Basis::ListPrice)))
    }
}

/// Embeds text by whether it holds "kernel" or "zeta".
struct Words;

impl Embed for Words {
    fn model(&self) -> &str {
        "words"
    }

    async fn embed(
        &self,
        inputs: Vec<String>,
    ) -> Result<(Vec<Vec<f32>>, Option<f64>), crate::search::EmbedError> {
        let vectors = inputs
            .iter()
            .map(|t| {
                let t = t.to_lowercase();
                vec![
                    if t.contains("kernel") { 1.0 } else { 0.0 },
                    if t.contains("zeta") { 1.0 } else { 0.0 },
                ]
            })
            .collect();
        Ok((vectors, Some(0.0001)))
    }
}

fn existing(id: &str, title: &str, status: &str) -> String {
    format!(
        "---\nid: {id}\nversion: 1\nkind: method\ntitle: {title}\nsummary: About {title}.\ntags: [x]\napplies_when: Code uses it.\nstatus: {status}\nauthor: openagents\nprovenance:\n  written_from: [reference]\n  cites: [\"Book\"]\nevidence: []\n---\n\n## Details\n\nBody.\n"
    )
}

fn knowledge(name: &str) -> PathBuf {
    let dir = scratch(name);
    std::fs::write(
        dir.join("stats.estimators.md"),
        existing("stats.estimators", "Kernel estimators", "admitted"),
    )
    .unwrap();
    std::fs::write(
        dir.join("slip.guessing.md"),
        existing("slip.guessing", "Guessing formats", "candidate"),
    )
    .unwrap();
    dir
}

fn run_dir(name: &str) -> PathBuf {
    let dir = scratch(&format!("{name}-run")).join("drift-watch-1790000001");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("summary.json"),
        json!({
            "task": "drift-watch", "reward": 0.0,
            "outcome": {"steps": 2, "ending": {"reason": "tests_held"},
                        "knowledge": [{"id": "stats.estimators"}]},
            "verifier_output": "FAILED test_state.py::test_hidden - AssertionError",
        })
        .to_string(),
    )
    .unwrap();
    let events = [
        json!({"event": "started", "task": "drift-watch"}),
        json!({"event": "generated", "step": 1, "generated": {"action": {"Ok": {
            "rationale": "Read the monitor's code first.", "commands": ["cat monitor.py"], "finished": false}}}}),
        json!({"event": "ran", "step": 1, "result": {"command": "cat monitor.py", "exit": 0, "output": "def score(): ..."}}),
        json!({"event": "tested", "step": 1, "results": [{"exit": 1}, {"exit": 0}]}),
        json!({"event": "generated", "step": 2, "generated": {"action": {"Ok": {
            "rationale": "Trust the docstring's estimator.", "commands": [], "finished": true}}}}),
    ];
    let lines: Vec<String> = events.iter().map(ToString::to_string).collect();
    std::fs::write(dir.join("events.jsonl"), lines.join("\n")).unwrap();
    dir
}

fn proposal(id: &str, title: &str, body: &str, updates: &str) -> Proposal {
    Proposal {
        id: id.to_string(),
        kind: "slip".to_string(),
        title: title.to_string(),
        summary: format!("{title}, in general."),
        tags: vec!["Code Review".to_string()],
        applies_when: "A comment describes the code's method.".to_string(),
        body: format!("## Details\n\n{body}\n\n## How to check\n\nCompare with the definition."),
        cites: vec!["Gretton et al. 2012".to_string()],
        updates: updates.to_string(),
    }
}

fn retriever(dir: &Path) -> Retriever<Words> {
    Retriever::new(
        Base {
            entries: Base::read(dir).0,
        },
        Words,
        None,
    )
}

#[test]
fn the_record_holds_each_step_and_the_verdict() {
    let record = record(&run_dir("record")).unwrap();
    assert_eq!(record.run, "drift-watch-1790000001");
    assert_eq!(record.task, "drift-watch");
    for part in [
        "Step 1: Read the monitor's code first.",
        "$ cat monitor.py",
        "acceptance tests: 1 of 2 pass",
        "(said the task is finished)",
        "Knowledge entries shown: stats.estimators",
        "FAILED test_state.py::test_hidden",
    ] {
        assert!(
            record.text.contains(part),
            "{part} is missing:\n{}",
            record.text
        );
    }
}

#[tokio::test]
async fn proposals_become_candidates_new_versions_or_refusals() {
    let dir = knowledge("write");
    let model = FakeModel {
        reply: Proposals {
            entries: vec![
                proposal(
                    "slip.comment-claims",
                    "Comments that claim a method",
                    "Check the code, not the comment.",
                    "",
                ),
                proposal("slip.named", "A named task", "Seen in drift-watch.", ""),
                proposal(
                    "stats.kernel-variants",
                    "Kernel estimator variants",
                    "Two kernel forms.",
                    "",
                ),
            ],
        },
        prompt: RefCell::new(String::new()),
    };
    let r = retriever(&dir);
    let result = harvest(
        &run_dir("write"),
        &dir,
        &model,
        Some(&r),
        &Corpus::default(),
    )
    .await
    .unwrap();
    assert!(
        model
            .prompt
            .borrow()
            .contains("- stats.estimators (method): Kernel estimators")
    );
    let outcomes: Vec<(&str, &Written)> = result
        .proposals
        .iter()
        .map(|(id, w)| (id.as_str(), w))
        .collect();
    assert_eq!(outcomes.len(), 3);

    // A new entry: a candidate, written from this run, authored by the model.
    assert_eq!(
        outcomes[0].1,
        &Written::New(dir.join("slip.comment-claims.md"))
    );
    let new = Entry::parse(&std::fs::read_to_string(dir.join("slip.comment-claims.md")).unwrap())
        .unwrap();
    assert_eq!(new.status, Status::Candidate);
    assert_eq!(new.written_from, ["drift-watch-1790000001"]);
    assert_eq!(new.tags, ["code-review"]);
    assert!(new.author.contains("fake/model"));

    // The run's own task name is refused even without an installed corpus.
    let Written::Refused(why) = outcomes[1].1 else {
        panic!("slip.named was written");
    };
    assert!(
        why.contains("names the benchmark task drift-watch"),
        "{why}"
    );

    // A near-duplicate of an admitted entry waits as its next version.
    assert_eq!(outcomes[2].0, "stats.estimators");
    let pending_path = dir.join("versions/stats.estimators.v2.md");
    assert_eq!(
        outcomes[2].1,
        &Written::Version {
            path: pending_path.clone(),
            pending: true
        }
    );
    let next = Entry::parse(&std::fs::read_to_string(&pending_path).unwrap()).unwrap();
    assert_eq!((next.version, next.status), (2, Status::Candidate));
    let current =
        Entry::parse(&std::fs::read_to_string(dir.join("stats.estimators.md")).unwrap()).unwrap();
    assert_eq!(current.status, Status::Admitted);
    assert!(result.usd().unwrap() > 0.002);
}

#[tokio::test]
async fn a_revision_of_a_candidate_replaces_it_and_keeps_the_old_file() {
    let dir = knowledge("revise");
    let model = FakeModel {
        reply: Proposals {
            entries: vec![
                proposal(
                    "slip.anything",
                    "Guessing formats, revised",
                    "Read a sample first.",
                    "slip.guessing",
                ),
                Proposal {
                    kind: "fact".to_string(),
                    ..proposal("slip.bad-kind", "T", "B.", "")
                },
            ],
        },
        prompt: RefCell::new(String::new()),
    };
    let result = harvest::<_, Words>(&run_dir("revise"), &dir, &model, None, &Corpus::default())
        .await
        .unwrap();
    assert_eq!(
        result.proposals[0],
        (
            "slip.guessing".to_string(),
            Written::Version {
                path: dir.join("slip.guessing.md"),
                pending: false
            }
        )
    );
    let revised =
        Entry::parse(&std::fs::read_to_string(dir.join("slip.guessing.md")).unwrap()).unwrap();
    assert_eq!(revised.version, 2);
    assert_eq!(revised.title, "Guessing formats, revised");
    assert!(dir.join("versions/slip.guessing.v1.md").exists());
    assert!(
        matches!(&result.proposals[1].1, Written::Refused(why) if why.contains("unknown kind"))
    );
}

#[test]
fn a_trajectory_reads_as_the_task_then_each_step() {
    let dir = scratch("trace");
    let path = dir.join("trace.json");
    let doc = json!({
        "schema_version": "ATIF-v1.7",
        "agent": {"model_name": "strong-model"},
        "steps": [
            {"source": "user", "message": "Fit the rules to the pairs."},
            {"source": "agent", "message": "Read the engine first.",
             "tool_calls": [{"function_name": "Bash", "arguments": {"command": "cat engine.py"}}],
             "observation": {"results": [{"content": "def apply(rule): ..."}]}},
            {"source": "agent", "message": [{"type": "text", "text": "Now score candidates."}]}
        ]
    });
    std::fs::write(&path, doc.to_string()).unwrap();
    let record = trace_record(&path, "some-task").unwrap();
    assert!(record.trace);
    assert_eq!(
        (record.run.as_str(), record.task.as_str()),
        ("some-task", "some-task")
    );
    assert!(
        record
            .text
            .starts_with("A winning trajectory by strong-model, 3 steps.")
    );
    assert!(record.text.contains("Fit the rules to the pairs."));
    assert!(record.text.contains("[Bash] cat engine.py"));
    assert!(record.text.contains("→ def apply(rule): ..."));
    assert!(record.text.contains("Now score candidates."));
    assert!(prompt(&record, &Base { entries: vec![] }).contains("# The trajectory"));
}

#[test]
fn a_contrast_pairs_the_failed_run_with_the_winning_trajectory() {
    let run = run_dir("contrast");
    let path = scratch("contrast-trace").join("trace.json");
    let doc = json!({
        "agent": {"model_name": "strong-model"},
        "steps": [
            {"source": "user", "message": "Fix the drift monitor."},
            {"source": "agent", "message": "The docstring's estimator is the bug."}
        ]
    });
    std::fs::write(&path, doc.to_string()).unwrap();
    let record = contrast_record(&run, &path, "drift-watch").unwrap();
    assert!(record.contrast && !record.trace);
    assert_eq!(record.run, "drift-watch-1790000001");
    let failed = record.text.find("## The failed run").unwrap();
    let won = record.text.find("## The winning trajectory").unwrap();
    assert!(failed < won);
    assert!(record.text.contains("Trust the docstring's estimator."));
    assert!(
        record
            .text
            .contains("The docstring's estimator is the bug.")
    );
    assert!(prompt(&record, &Base { entries: vec![] }).contains("# The two records"));
}

#[test]
fn a_revision_keeps_the_current_body_and_adds_the_proposal() {
    let body = merged_body(
        "## Details\n\nThe whole formula.\n\n## How to check\n\nRecompute it.",
        "## Details\n\nPuts use the other tail.",
        3,
    );
    assert!(body.starts_with("## Details\n\nThe whole formula."));
    assert!(body.contains("Recompute it.\n\n## Added in version 3\n\n### Details\n\nPuts use"));
}

fn codex(reply: microluna::Reply) -> CodexProposer<microluna::fake::FakeTransport> {
    CodexProposer {
        transport: microluna::fake::FakeTransport::new(vec![reply]),
        model: "gpt-6-luna".to_string(),
        effort: None,
    }
}

fn entries_call(arguments: &str) -> microluna::Reply {
    microluna::Reply {
        id: None,
        model: "gpt-6-luna".to_string(),
        items: vec![json!({
            "type": "function_call", "call_id": "c1", "name": TOOL, "arguments": arguments,
        })],
        usage: microluna::TokenUsage {
            input: 10_000,
            output: 2_000,
            ..Default::default()
        },
    }
}

#[tokio::test]
async fn a_codex_harvest_is_one_strict_tool_call_priced_at_list_price() {
    let p = proposal(
        "slip.trusting-comments",
        "Trusting comments",
        "Read the code.",
        "",
    );
    let arguments = json!({"entries": [{
        "id": p.id, "kind": p.kind, "title": p.title, "summary": p.summary, "tags": p.tags,
        "applies_when": p.applies_when, "body": p.body, "cites": p.cites, "updates": p.updates,
    }]})
    .to_string();
    let proposer = codex(entries_call(&arguments));
    let (proposals, cost) = proposer.propose(SYSTEM, "the record").await.unwrap();
    assert_eq!(proposals.entries, vec![p]);
    assert_eq!(cost.basis, Basis::ListPrice);
    // 10k input at $0.10/M and 2k output at $0.50/M.
    assert!((cost.usd.unwrap() - 0.002).abs() < 1e-12);
    let sent = proposer.transport.requests();
    assert_eq!(sent[0].model, "gpt-6-luna");
    assert_eq!(sent[0].tools[0]["name"], TOOL);
    assert_eq!(sent[0].tools[0]["strict"], true);
    assert_eq!(sent[0].tools[0]["parameters"], schema());
    assert_eq!(proposer.provider(), "codex");
}

#[tokio::test]
async fn a_failed_codex_harvest_reports_its_cost_as_unknown() {
    let proposer = CodexProposer {
        transport: microluna::fake::FakeTransport::new(Vec::new()),
        model: "gpt-6-luna".to_string(),
        effort: None,
    };
    proposer
        .transport
        .then_fail(microluna::TransportError::Failed(
            "server_error".to_string(),
        ));
    let error = proposer.propose(SYSTEM, "the record").await.unwrap_err();
    assert!(error.contains("the call cost unknown"), "{error}");
}

#[tokio::test]
async fn an_unpriced_model_leaves_the_harvest_total_unknown() {
    let dir = knowledge("unpriced");
    let mut proposer = codex(entries_call(r#"{"entries": []}"#));
    proposer.model = "gpt-9-mystery".to_string();
    let result = harvest(
        &run_dir("unpriced"),
        &dir,
        &proposer,
        None::<&Retriever<Words>>,
        &Corpus::default(),
    )
    .await
    .unwrap();
    assert_eq!(result.usd(), None);
    assert_eq!(result.known_usd(), 0.0);
    assert!(
        result
            .model_cost
            .unknown
            .as_deref()
            .unwrap()
            .contains("no known list price")
    );
}

#[tokio::test]
async fn a_lexical_near_duplicate_search_is_recorded() {
    let dir = knowledge("lexical");
    let lexical = Retriever::<Words>::lexical(
        Base {
            entries: Base::read(&dir).0,
        },
        "no key",
    );
    let model = FakeModel {
        reply: Proposals {
            entries: vec![proposal("slip.new-one", "A new slip", "Details.", "")],
        },
        prompt: RefCell::new(String::new()),
    };
    let result = harvest(
        &run_dir("lexical"),
        &dir,
        &model,
        Some(&lexical),
        &Corpus::default(),
    )
    .await
    .unwrap();
    assert_eq!(result.lexical, vec!["no key".to_string()]);
}
