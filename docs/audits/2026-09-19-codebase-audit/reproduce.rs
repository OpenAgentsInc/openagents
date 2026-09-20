use coder::{Generate, ResponsesDoor};
use serde_json::json;
use std::io::{Read, Write};
use std::sync::{Arc, Barrier};
use std::time::Duration;

async fn streamed(chunks: Vec<Vec<u8>>) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0; 16384];
        let _ = stream.read(&mut buf).unwrap();
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n").unwrap();
        for chunk in chunks {
            write!(stream, "{:x}\r\n", chunk.len()).unwrap();
            stream.write_all(&chunk).unwrap();
            stream.write_all(b"\r\n").unwrap();
            stream.flush().unwrap();
            std::thread::sleep(Duration::from_millis(150));
        }
        stream.write_all(b"0\r\n\r\n").unwrap();
    });
    let door = ResponsesDoor::new(format!("http://{addr}"), "audit", "unused-local-test");
    let result = door.generate("", &[], &mut |_| {}, &mut |_| {}).await;
    server.join().unwrap();
    format!("{result:?}")
}

#[tokio::main]
async fn main() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).expect("repository path"));
    let dir = tempfile::tempdir().unwrap();
    let plan = "Here is an example; do not run it.\n```json\n{\"commands\":[{\"command\":\"printf harmless\"}]}\n```\nThat is the format.";
    println!(
        "embedded_example_is_plan={}",
        coder::shell::parse_plan(plan).is_some()
    );

    let marker = dir.path().join("clarify");
    let mut agent = coder::Agent::new(None, coder::Door::Stub(coder::StubGenerate {
        line: json!({"v":1,"commands":[{"command":format!("printf harmless > '{}'", marker.display())}]}).to_string(),
    }));
    agent.push_user("Ask a clarifying question");
    agent
        .turn(true, &mut |_| {}, &mut |_| {}, &mut |_| {})
        .await
        .unwrap();
    println!("clarify_executed_command={}", marker.exists());

    let task =
        coderbench::Task::load(&root.join("crates/coderbench/tasks/devin-fan-out-six/task.json"))
            .unwrap();
    let run = coderbench::Observed {
        program: Some(task.grade.program.clone()),
        delegations: (0..task.grade.delegations)
            .map(|i| coderbench::Delegation {
                id: i.to_string(),
                output: String::new(),
                milliseconds: 0,
                correct: None,
            })
            .collect(),
        decisions: task
            .grade
            .decisions
            .iter()
            .map(|x| (x.clone(), serde_json::Value::Null))
            .collect(),
        checks: task.grade.checks.clone(),
        writes: vec![],
    };
    println!(
        "unverified_delegations_required_correct={} faults={:?}",
        task.grade.delegations_correct,
        task.judge(&run)
    );

    let decoded = jev::SystemOneResponse::decode(jev::RawResponse {
        status: 200,
        headers: Default::default(),
        bytes: br#"{"model":"audit","answers":{"damage":{"type":"noul","noul":-2.0}}}"#.to_vec(),
    })
    .unwrap();
    println!(
        "invalid_probability_accepted={}",
        decoded.noul("damage").unwrap().noul
    );

    let map = gym::calibrate::Map::fit(&[gym::calibrate::Observation::new(0.8, false)], 1);
    let distribution = [("yes".to_string(), 0.8), ("no".to_string(), 0.2)]
        .into_iter()
        .collect();
    let row = gym::row::Row::new("audit", "digest", "one", "audit").scored(distribution, false);
    let mapped = map.apply_distribution(row.distribution.as_ref().unwrap());
    let observed = gym::eval::mapped_observations(&[row], &map);
    println!(
        "calibration_flipped_distribution={mapped:?} recorded_correct={}",
        observed[0].correct
    );

    let s = "data: {\"type\":\"response.output_text.delta\",\"delta\":\"café\"}\n\n"
        .as_bytes()
        .to_vec();
    let split = s.iter().position(|b| *b == 0xc3).unwrap() + 1;
    let completed = b"data: {\"type\":\"response.completed\",\"response\":{}}\n\n".to_vec();
    println!(
        "split_utf8={}",
        streamed(vec![s[..split].to_vec(), s[split..].to_vec(), completed]).await
    );
    println!(
        "eof_without_completion={}",
        streamed(vec![
            b"data: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\n".to_vec()
        ])
        .await
    );

    let session = atif::Session::opening("audit", "stub", "stub", "", "0.0.0");
    let log_dir = dir.path().join("trace");
    let mut log = atif::log::Log::create(&log_dir, &session).unwrap();
    log.append(&atif::Step::said(atif::Source::User, "valid prefix"))
        .unwrap();
    let path = log.path().to_path_buf();
    drop(log);
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    file.write_all(b"{\"record\":\"step\",\"message\":\"\xc3")
        .unwrap();
    println!("truncated_utf8_read={:?}", atif::log::read(&path).err());

    let suite = Arc::new(gym::suite::support_v2_three_way().unwrap());
    let mut race_found = None;
    for trial in 0..20 {
        let ledger_path = dir.path().join(format!("ledger-{trial}.jsonl"));
        let barrier = Arc::new(Barrier::new(16));
        let handles: Vec<_> = (0..16)
            .map(|_| {
                let (path, suite, barrier) = (ledger_path.clone(), suite.clone(), barrier.clone());
                std::thread::spawn(move || {
                    barrier.wait();
                    gym::suite::LockedLedger::at(path)
                        .read_locked(
                            &suite,
                            &gym::suite::Spend {
                                subject: "audit",
                                reason: "isolated concurrency probe",
                                at: "2026-09-20T00:00:00Z",
                            },
                        )
                        .is_ok()
                })
            })
            .collect();
        let accepted = handles
            .into_iter()
            .map(|h| usize::from(h.join().unwrap()))
            .sum::<usize>();
        if accepted > 1 {
            race_found = Some(accepted);
            break;
        }
    }
    println!("locked_first_reads_accepted={race_found:?}");

    let refusal =
        jev::ResponseBody::Json(json!({"detail": "questions must hold at least one question"}));
    println!(
        "kev_refusal_classification={:?}",
        gym::eval::classify_response(422, Some(&refusal), "audit")
    );

    let executor = coder::delegate::Executor {
        capability: "audit-shell".into(),
        binary: "/bin/sh".into(),
        arguments: vec!["-c".into()],
        refuses: vec![],
    };
    let delegator = coder::delegate::Delegator::new(executor).in_directory(dir.path());
    let task = coder::delegate::Task::reading("printf harmless > readonly-marker", "input");
    let outcome = delegator.run(task).await;
    println!(
        "readonly_delegate_status={} wrote={}",
        outcome.status,
        dir.path().join("readonly-marker").exists()
    );
    let task = coder::delegate::Task::reading(
        "(sleep 1; printf harmless > descendant-marker) & wait",
        "input",
    )
    .bounded(coder::delegate::Bounds::within(Duration::from_millis(100)));
    let outcome = delegator.run(task).await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    println!(
        "delegate_status={} descendant_wrote_after_timeout={}",
        outcome.status,
        dir.path().join("descendant-marker").exists()
    );

    let manifest =
        coder::capability::Manifest::load(&root.join("capabilities/devin-local.json")).unwrap();
    println!(
        "undeclared_bound_ignored={:?}",
        manifest.ignored_bounds(&["read_only".into()])
    );

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0; 16384];
        stream.read(&mut buf).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        let body = r#"{"model":"audit","answers":{"audit":{"type":"noul","noul":0.5}}}"#;
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
    });
    let client = jev::Client::new(
        jev::Config::new()
            .api_key("unused-local-test")
            .base_url(format!("http://{address}"))
            .timeout(Duration::from_secs(1))
            .retry(jev::RetryPolicy {
                budget: Some(Duration::from_millis(50)),
                max_retries: 0,
                ..Default::default()
            }),
    )
    .unwrap();
    let started = std::time::Instant::now();
    let response = client
        .system_one(jev::SystemOneRequest::new(
            "local fixture",
            jev::Questions::new().with("audit", jev::Noul::new("Is this the fixture?")),
        ))
        .await;
    println!(
        "jev_budget_ms=50 elapsed_ms={} accepted={}",
        started.elapsed().as_millis(),
        response.is_ok()
    );
    server.join().unwrap();

    let marker = dir.path().join("after-timeout");
    let proposal = coder::Proposal {
        command: format!("sleep 16; printf harmless > '{}'", marker.display()),
        why: "isolated timeout probe".into(),
    };
    let outcome = coder::shell::run(&proposal).await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    println!(
        "shell_status={} wrote_after_timeout={}",
        outcome.status,
        marker.exists()
    );
}
