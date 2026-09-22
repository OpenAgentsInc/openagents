//! The shipped adapter measures a real pinned suite inside host verification.

use coder::capability::{Entry, Source, Trust};
use coder::verification::{Acceptance, Check, Plan as VerificationPlan, SCHEMA, Verdict};
use coder_project::gym_suite::{Door, Plan};
use gym::{
    gate::Gate,
    questions::QuestionSet,
    suite::{Partition, Suite},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn fixture(suite: &Suite, questions: &QuestionSet) -> (String, tokio::task::JoinHandle<()>) {
    let mut expected = BTreeMap::new();
    for (index, item) in suite
        .partition(Partition::Development)
        .unwrap()
        .into_iter()
        .enumerate()
    {
        let key =
            serde_json::to_string(&json!([item.state, questions.ask(item).unwrap()])).unwrap();
        assert!(expected.insert(key, (item.truth.clone(), index)).is_none());
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let (start, length) = loop {
                let mut chunk = [0; 8192];
                let read = stream.read(&mut chunk).await.unwrap();
                assert!(read > 0);
                bytes.extend_from_slice(&chunk[..read]);
                if let Some(start) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..start]);
                    assert!(!header.to_ascii_lowercase().contains("authorization:"));
                    let length = header
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap();
                    break (start + 4, length);
                }
            };
            while bytes.len() < start + length {
                let mut chunk = [0; 8192];
                let read = stream.read(&mut chunk).await.unwrap();
                assert!(read > 0);
                bytes.extend_from_slice(&chunk[..read]);
            }
            let request: Value = serde_json::from_slice(&bytes[start..start + length]).unwrap();
            let q = &request["questions"]["q"];
            let key = serde_json::to_string(&json!([request["state"], q])).unwrap();
            let (truth, index) = &expected[&key];
            let correct = if request["model"] == "good" {
                index % 5 != 0
            } else {
                index % 5 == 0
            };
            let answer = match q["type"].as_str().unwrap() {
                "noul" => {
                    json!({"type":"noul","noul":if (truth == "yes") == correct {0.95} else {0.05}})
                }
                "choice" => {
                    let labels: Vec<_> =
                        q["criteria"].as_object().unwrap().keys().cloned().collect();
                    let chosen = if correct {
                        truth.clone()
                    } else {
                        labels.iter().find(|l| *l != truth).unwrap().clone()
                    };
                    let probabilities: BTreeMap<_, _> = labels
                        .iter()
                        .map(|l| (l.clone(), if *l == chosen { 1.0 } else { 0.0 }))
                        .collect();
                    json!({"type":"choice","choice":chosen,"confidence":1.0,"probabilities":probabilities})
                }
                "score" => {
                    let levels = q["criteria"].as_array().unwrap();
                    let truth: usize = truth.parse().unwrap();
                    let chosen = if correct {
                        truth
                    } else {
                        (truth + 1) % levels.len()
                    };
                    let probabilities: BTreeMap<_, _> = (0..levels.len())
                        .map(|n| (n.to_string(), if n == chosen { 1.0 } else { 0.0 }))
                        .collect();
                    let legend: BTreeMap<_, _> = levels
                        .iter()
                        .enumerate()
                        .map(|(n, v)| (n.to_string(), v.clone()))
                        .collect();
                    json!({"type":"score","score":chosen,"confidence":1.0,"selected":chosen.to_string(),"probabilities":probabilities,"legend":legend})
                }
                kind => panic!("fixture needs an explicit answer for {kind}"),
            };
            let model = if request["model"] == "wrong-model" {
                json!("other-model")
            } else {
                request["model"].clone()
            };
            let body = serde_json::to_vec(&json!({"model":model,"answers":{"q":answer}})).unwrap();
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(header.as_bytes()).await.unwrap();
            stream.write_all(&body).await.unwrap();
        }
    });
    (url, handle)
}

#[tokio::test]
async fn pinned_gym_pass_fail_and_stale_results_cross_the_readonly_boundary() {
    assert!(
        coder::delegate::boundary_supported(),
        "this acceptance test requires the supported execution boundary"
    );
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let suite_path = repository.join("crates/gym/suites/support-v2-three-way.json");
    let question_path = repository.join("crates/gym/questions/support-v2-three-way-v1.json");
    let gate_path = repository.join("crates/gym/gates/decision-v1.json");
    let suite = Suite::load_file(&suite_path).unwrap();
    let questions = QuestionSet::load(&question_path).unwrap();
    let gate = Gate::load(&gate_path).unwrap();
    let (url, server) = fixture(&suite, &questions).await;
    let work = tempfile::tempdir().unwrap();
    let host = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("candidate.txt"), "immutable candidate").unwrap();
    std::fs::create_dir(work.path().join("capabilities")).unwrap();
    let binary = env!("CARGO_BIN_EXE_coder-project");
    let manifest = work.path().join("capabilities/gym-check.json");
    std::fs::write(
        &manifest,
        serde_json::to_vec(&capability::executor_document(
            "gym-check",
            binary,
            vec![binary.into(), "--help".into()],
            serde_json::json!({"name":"Gym fixture","invoke":[binary],"isolation":["directory"]}),
        ))
        .unwrap(),
    )
    .unwrap();
    let mut trust = Trust::load(&host.path().join("trust.json")).unwrap();
    trust.approve(Some(work.path()), "gym-check", &[]).unwrap();
    let entry = Entry::load(&manifest, Source::Repository).unwrap();
    let mut plan = Plan {
        suite: suite_path,
        suite_digest: suite.digest.clone(),
        questions: question_path,
        question_digest: questions.digest(),
        gate: gate_path,
        gate_digest: gate.digest(),
        input_digest: "fixture-artifact".into(),
        baseline: Door {
            url: url.clone(),
            model: "bad".into(),
        },
        candidate: Door {
            url,
            model: "good".into(),
        },
        max_items: 1000,
        seconds: 30,
    };
    for (case, expected) in [
        ("pass", Verdict::Passed),
        ("fail", Verdict::Failed),
        ("wrong-model", Verdict::Unverifiable),
        ("truncated", Verdict::Unverifiable),
        ("stale", Verdict::Unverifiable),
    ] {
        plan.baseline.model = "bad".into();
        plan.candidate.model = "good".into();
        if case == "wrong-model" {
            plan.candidate.model = "wrong-model".into();
        }
        if case == "fail" {
            std::mem::swap(&mut plan.baseline, &mut plan.candidate);
        }
        if case == "stale" {
            plan.question_digest = "changed".into();
        }
        let path = host.path().join(format!("{case}.json"));
        std::fs::write(&path, serde_json::to_vec(&plan).unwrap()).unwrap();
        let verification = VerificationPlan {
            schema: SCHEMA.into(),
            input_digest: plan.input_digest.clone(),
            seconds: 45,
            allow_unrestricted_reads: true,
            allow_network: true,
            checks: vec![Check {
                id: "measured-suite".into(),
                manifest: manifest.clone(),
                manifest_digest: entry.digest.clone(),
                arguments: vec!["gym-suite".into(), path.display().to_string()],
                seconds: 40,
                output_bytes: if case == "truncated" {
                    1024
                } else {
                    1024 * 1024
                },
                acceptance: Acceptance::Suite {
                    suite_digest: suite.digest.clone(),
                    input_digest: plan.input_digest.clone(),
                },
            }],
        };
        let report = coder::verification::run(work.path(), &verification, &trust)
            .await
            .unwrap();
        assert_eq!(
            report.verdict,
            expected,
            "{}",
            serde_json::to_string(&report).unwrap()
        );
        if case == "truncated" {
            assert!(report.checks[0].output_truncated);
            assert!(report.checks[0].suite_evidence.is_none());
            continue;
        }
        let evidence = report.checks[0].suite_evidence.as_ref().unwrap();
        assert_eq!(evidence.verdict, expected);
        if case != "stale" {
            let details = evidence.details.as_ref().unwrap();
            assert_eq!(details["gate_digest"], gate.digest());
            if case == "wrong-model" {
                assert!(details["measurements"]["lost"][1].as_u64().unwrap() > 0);
            } else {
                assert_eq!(details["measurements"]["lost"], json!([0, 0]));
            }
            assert_eq!(details["measurements"]["partition"], "development");
        }
        assert_eq!(
            std::fs::read_to_string(work.path().join("candidate.txt")).unwrap(),
            "immutable candidate"
        );
    }
    server.abort();
}
