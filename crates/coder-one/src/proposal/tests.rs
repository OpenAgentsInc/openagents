use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::*;

const RUN: &str =
    "tb4--coder-one-tunable-luna-v2--log-summary-date-ranges/log-summary-date-ranges__abc";
const OTHER: &str = "tb4--coder-one-tunable-luna-v2--fix-git/fix-git__def";

fn context() -> Context {
    Context {
        ask_id: "ask-1790000000000".to_string(),
        question: "why did Luna fail log-summary-date-ranges?".to_string(),
        ask_dir: "/asks/ask-1790000000000".to_string(),
        policies: Path::new(env!("CARGO_MANIFEST_DIR")).join("policies"),
        run_tasks: BTreeMap::from([
            (RUN.to_string(), "log-summary-date-ranges".to_string()),
            (OTHER.to_string(), "fix-git".to_string()),
        ]),
    }
}

fn policy_draft(kind: &str, patch: &str) -> Draft {
    Draft {
        kind: kind.to_string(),
        title: "Run the behavior scenarios".to_string(),
        rationale: "The checks passed a summary that counted words in messages.".to_string(),
        source_runs: vec![RUN.to_string()],
        expected_tasks: vec!["log-summary-date-ranges".to_string()],
        base: "tunable-luna-v2".to_string(),
        patch: patch.to_string(),
        ..Draft::default()
    }
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

#[test]
fn a_policy_patch_applies_to_a_checked_in_manifest_and_yields_a_new_digest() {
    let (record, file) = validate(
        &policy_draft("check", r#"{"policy":{"verify":{"behavior":true}}}"#),
        "prop-1790000000000-1",
        &context(),
    );
    assert_eq!(record["valid"], true, "{record}");
    assert_eq!(record["kind"], "check");
    assert_eq!(record["mini_tasks"], json!(["log-severity"]));
    let materialized = &record["materialized"];
    assert_eq!(materialized["base"], "tunable-luna-v2");
    assert_ne!(materialized["digest"], materialized["base_digest"]);
    assert_eq!(materialized["fields"], json!(["policy.verify.behavior"]));
    let (name, manifest) = file.unwrap();
    assert_eq!(name, POLICY_FILE);
    let parsed = Manifest::parse(&manifest.to_string()).unwrap();
    assert_eq!(parsed.digest(), materialized["digest"].as_str().unwrap());
    assert_eq!(
        parsed.name.as_deref(),
        Some("tunable-luna-v2-prop-1790000000000-1")
    );
    assert!(parsed.policy.verify.unwrap().behavior);
    // The record seals itself.
    assert_eq!(record["digest"].as_str().unwrap(), digest(&record));
}

#[test]
fn validation_refuses_what_it_should() {
    let refused = |draft: Draft, needle: &str| {
        let (record, file) = validate(&draft, "prop-1-1", &context());
        assert_eq!(record["valid"], false, "{record}");
        assert!(file.is_none() || record["kind"] == "minitask");
        let problems = record["problems"].to_string();
        assert!(problems.contains(needle), "{needle:?} not in {problems}");
    };
    // The protected part is the host's.
    refused(
        policy_draft("policy", r#"{"protected":{"acceptance":"none"}}"#),
        "only fields under policy",
    );
    // A check changes only policy.verify.
    refused(
        policy_draft("check", r#"{"policy":{"brief":{"cap":9000}}}"#),
        "only policy.verify",
    );
    // An unknown field doesn't parse.
    refused(
        policy_draft("policy", r#"{"policy":{"verify":{"bogus":true}}}"#),
        "invalid",
    );
    // A manifest that isn't checked in.
    let mut draft = policy_draft("policy", r#"{"policy":{"brief":{"cap":9000}}}"#);
    draft.base = "no-such-policy".to_string();
    refused(draft, "no manifest no-such-policy.json");
    // A patch that changes nothing that runs.
    refused(
        policy_draft("policy", r#"{"policy":{"verify":{"checks":true}}}"#),
        "digest unchanged",
    );
    // Not JSON.
    refused(
        policy_draft("policy", "verify.behavior = true"),
        "isn't JSON",
    );
    // A run the ask didn't read, and a task none of its runs attempted.
    let mut draft = policy_draft("policy", r#"{"policy":{"brief":{"cap":9000}}}"#);
    draft.source_runs = vec!["tb4--x--y/y__z".to_string()];
    refused(draft, "isn't a run the ask read");
    let mut draft = policy_draft("policy", r#"{"policy":{"brief":{"cap":9000}}}"#);
    draft.expected_tasks = vec!["build-cython-ext".to_string()];
    refused(draft, "isn't the task of any source run");
    // A kind that isn't one.
    refused(policy_draft("vibes", "{}"), "a proposal's kind");
    // A questions proposal names the pinned set.
    refused(
        Draft {
            kind: "questions".to_string(),
            question_set: "builtin-v2".to_string(),
            issue: "Ask whether the counts come from the severity field.".to_string(),
            ..policy_draft("questions", "")
        },
        "isn't one this build has",
    );
}

#[test]
fn a_code_proposal_becomes_a_drafted_issue() {
    let draft = Draft {
        kind: "code".to_string(),
        issue: "Add a scenario that swaps severity words inside messages.".to_string(),
        ..policy_draft("code", "")
    };
    let (record, file) = validate(&draft, "prop-1790000000000-2", &context());
    assert_eq!(record["valid"], true, "{record}");
    assert_eq!(record["needs_code"], true);
    assert!(file.is_none());
    let body = record["issue_draft"]["body"].as_str().unwrap();
    assert!(body.contains("swaps severity words"), "{body}");
    assert!(body.contains(RUN), "{body}");
    assert!(body.contains("ask-1790000000000"), "{body}");
}

fn minitask_json(id: &str, path: &str) -> String {
    json!({
        "id": id,
        "family": "field meaning in data",
        "instruction": "Count ERROR lines by their severity field into count.txt.",
        "files": { "app.log": "2025-08-12 [ERROR] a\n2025-08-12 [INFO] ERROR in message\n" },
        "grader": "test \"$(cat count.txt)\" = 1",
        "good": { path: "1\n" },
        "bad": { path: "2\n" },
    })
    .to_string()
}

#[test]
fn a_minitask_must_parse_be_new_and_stay_inside_its_directory() {
    let spec = MiniSpec::parse(&minitask_json("severity-words", "count.txt")).unwrap();
    assert_eq!(spec.id, "severity-words");
    let problems = MiniSpec::parse(&minitask_json("log-severity", "/etc/count.txt")).unwrap_err();
    let joined = problems.join("; ");
    assert!(joined.contains("already exists"), "{joined}");
    assert!(joined.contains("must be relative"), "{joined}");
    assert!(MiniSpec::parse("{\"id\":\"x\"}").is_err());

    let draft = Draft {
        kind: "minitask".to_string(),
        minitask: minitask_json("severity-words", "count.txt"),
        ..policy_draft("minitask", "")
    };
    let (record, file) = validate(&draft, "prop-1790000000000-3", &context());
    assert_eq!(record["valid"], true, "{record}");
    assert_eq!(file.unwrap().0, MINITASK_FILE);
    assert!(
        record["issue_draft"]["body"]
            .as_str()
            .unwrap()
            .contains("severity-words")
    );
}

fn decide(dir: &Path, proposal: &Value, verdict: &str, digest: Option<&str>) {
    std::fs::write(
        dir.join(DECISION_FILE),
        json!({
            "schema": DECISION_SCHEMA,
            "proposal": proposal["id"],
            "proposal_digest": digest.unwrap_or(proposal["digest"].as_str().unwrap()),
            "verdict": verdict,
            "note": "",
            "by": "test",
            "at": "2026-09-23T00:00:00Z",
        })
        .to_string(),
    )
    .unwrap();
}

fn recorded(root: &Path, drafts: &[Draft]) -> Vec<(PathBuf, Value)> {
    let answer = json!({
        "answer": "A.",
        "claims": [],
        "proposed_change": "",
        "proposals": drafts.iter().map(|d| json!({
            "kind": d.kind, "title": d.title, "rationale": d.rationale,
            "source_runs": d.source_runs, "expected_tasks": d.expected_tasks,
            "base": d.base, "patch": d.patch, "question_set": d.question_set,
            "minitask": d.minitask, "issue": d.issue,
        })).collect::<Vec<_>>(),
    });
    let records = record_all(&answer, &context(), root).unwrap();
    records
        .iter()
        .map(|r| load(root, r["id"].as_str().unwrap()).unwrap())
        .collect()
}

#[test]
fn nothing_runs_without_an_approval_of_this_exact_proposal() {
    let root = tempfile::tempdir().unwrap();
    let written = recorded(
        root.path(),
        &[
            policy_draft("check", r#"{"policy":{"verify":{"behavior":true}}}"#),
            policy_draft("policy", r#"{"protected":{"acceptance":"none"}}"#),
            Draft {
                kind: "code".to_string(),
                issue: "Change the checker.".to_string(),
                ..policy_draft("code", "")
            },
        ],
    );
    assert_eq!(written.len(), 3);
    let (dir, proposal) = &written[0];
    assert_eq!(proposal["id"], "prop-1790000000000-1");
    assert!(dir.join(POLICY_FILE).is_file());
    let why = unapproved(dir, proposal).unwrap();
    assert!(why.contains("gym coder proposals approve"), "{why}");
    let refused = block_on(cli::run(dir, proposal, None)).unwrap_err();
    assert!(refused.contains("nobody has approved it"), "{refused}");
    decide(dir, proposal, "rejected", None);
    assert!(unapproved(dir, proposal).unwrap().contains("rejected"));
    decide(dir, proposal, "approved", Some("0000"));
    assert!(
        unapproved(dir, proposal)
            .unwrap()
            .contains("another version")
    );
    decide(dir, proposal, "approved", None);
    assert!(unapproved(dir, proposal).is_none());
    // An edited proposal no longer matches its digest.
    let mut edited = proposal.clone();
    edited["expected_tasks"] = json!(["fix-git", "log-summary-date-ranges"]);
    assert!(unapproved(dir, &edited).unwrap().contains("changed after"));

    // An invalid proposal never runs, approved or not.
    let (dir, invalid) = &written[1];
    decide(dir, invalid, "approved", None);
    assert!(
        unapproved(dir, invalid)
            .unwrap()
            .contains("didn't validate")
    );

    // A code proposal runs nothing; it has a drafted issue.
    let (dir, code) = &written[2];
    decide(dir, code, "approved", None);
    let refused = block_on(cli::run(dir, code, None)).unwrap_err();
    assert!(refused.contains("needs code"), "{refused}");
}

#[test]
fn an_approved_policy_runs_its_mini_stage_on_the_cited_tasks_mini_tasks() {
    let root = tempfile::tempdir().unwrap();
    let written = recorded(
        root.path(),
        &[policy_draft(
            "policy",
            r#"{"policy":{"verify":{"repair":null}}}"#,
        )],
    );
    let (dir, proposal) = &written[0];
    assert_eq!(proposal["valid"], true, "{proposal}");
    decide(dir, proposal, "approved", None);
    let started = std::time::Instant::now();
    let result = block_on(cli::run(dir, proposal, None)).unwrap();
    assert!(started.elapsed().as_secs() < 60, "{:?}", started.elapsed());
    let mini = &result["mini"];
    assert_eq!(mini["covered"], true);
    assert_eq!(mini["tasks"], json!(["log-severity"]));
    let runs = mini["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 4, "{mini}");
    // Dropping the repair loses the base's scripted fix of the bad
    // script, so the stage reports a regression and says where.
    assert_eq!(mini["verdict"], "regressed", "{mini}");
    assert!(
        mini["findings"].to_string().contains("log-severity bad"),
        "{mini}"
    );
    let written: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join(RESULT_FILE)).unwrap()).unwrap();
    assert_eq!(written["schema"], RESULT_SCHEMA);
    assert_eq!(written["ask"], "ask-1790000000000");
    assert_eq!(written["proposal_digest"], proposal["digest"]);
    // A regressed mini stage never starts a live one.
    let live = stage::Live {
        repo: PathBuf::from("/nonexistent"),
        profile: "tb4".to_string(),
        quota_usd: 1.0,
        tasks: vec!["log-summary-date-ranges".to_string()],
        base_arm: Some("coder-one-tunable-luna-v2".to_string()),
        min_free_disk_gb: None,
        plan_only: true,
        artifact: None,
    };
    let refused = block_on(cli::run(dir, proposal, Some(&live))).unwrap_err();
    assert!(refused.contains("regressed"), "{refused}");
}

#[test]
fn the_mini_stage_compares_arms_cell_by_cell() {
    let row = |task: &str, script: &str, arm: &str, verdict: &str, failed: u64| json!({"task": task, "script": script, "arm": arm, "verdict": verdict, "checks_failed": failed});
    let (verdict, findings) = stage::compare(&[
        row("log-severity", "good", "base", "passed", 0),
        row("log-severity", "good", "proposal", "passed", 0),
        row("log-severity", "bad", "base", "failed", 0),
        row("log-severity", "bad", "proposal", "failed", 1),
    ]);
    assert_eq!(verdict, "improved");
    assert!(findings[0].contains("flag the failure"));
    let (verdict, _) = stage::compare(&[
        row("log-severity", "good", "base", "passed", 0),
        row("log-severity", "good", "proposal", "passed", 2),
    ]);
    assert_eq!(verdict, "regressed");
    let (verdict, findings) = stage::compare(&[
        row("git-recovery", "good", "base", "passed", 0),
        row("git-recovery", "good", "proposal", "passed", 0),
    ]);
    assert_eq!((verdict, findings.len()), ("unchanged", 0));
}

#[test]
fn a_minitask_proposal_reproduces_when_its_grader_tells_the_candidates_apart() {
    if coder_boundary::Boundary::readonly().build().is_err() {
        // No enforced boundary on this host: the grader refuses to run.
        return;
    }
    let out = tempfile::tempdir().unwrap();
    let spec = MiniSpec::parse(&minitask_json("severity-words", "count.txt")).unwrap();
    let result = block_on(stage::mini_task(&spec, out.path())).unwrap();
    assert_eq!(result["verdict"], "reproduces", "{result}");
    let same = MiniSpec {
        bad: spec.good.clone(),
        good: BTreeMap::from([("count.txt".to_string(), "1\n".to_string())]),
        ..spec
    };
    let result = block_on(stage::mini_task(&same, out.path())).unwrap();
    assert_eq!(result["verdict"], "does-not-reproduce", "{result}");
}

#[test]
fn the_live_stage_runs_the_cited_tasks_against_the_base_policys_arm() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    assert_eq!(
        stage::base_arm(&repo, "tunable-luna-v2").unwrap(),
        "coder-one-tunable-luna-v2"
    );
    assert!(stage::base_arm(&repo, "no-such-policy").is_err());
    let live = stage::Live {
        repo,
        profile: "tb4".to_string(),
        quota_usd: 40.0,
        tasks: vec!["log-summary-date-ranges".to_string(), "fix-git".to_string()],
        base_arm: None,
        min_free_disk_gb: Some(15.0),
        plan_only: false,
        artifact: Some((PathBuf::from("/a/coder-one"), "ab12".to_string())),
    };
    let args = stage::experiment_args(
        "prop-1-1",
        "coder-one-tunable-luna-v2",
        Path::new("/p/policy.json"),
        &live,
    );
    let line = args.join(" ");
    assert_eq!(
        line,
        "run tbench experiment run --id prop-1-1 --profile tb4 \
         --arm coder-one-tunable-luna-v2 --arm prop-1-1=coder-one-tunable-luna-v2 \
         --arm-kwarg prop-1-1:policy=/p/policy.json \
         --arm-kwarg coder-one-tunable-luna-v2:artifact_path=/a/coder-one \
         --arm-kwarg coder-one-tunable-luna-v2:artifact_sha256=ab12 \
         --arm-kwarg prop-1-1:artifact_path=/a/coder-one \
         --arm-kwarg prop-1-1:artifact_sha256=ab12 \
         --tasks log-summary-date-ranges,fix-git --attempts 3 --quota-usd 40 \
         --min-free-disk-gb 15 --detach"
    );
}
