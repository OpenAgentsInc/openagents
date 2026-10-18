//! The conformance suite and the lifecycle measurements against the real
//! Archil API (#12138). Ignored by default; needs `ARCHIL_API_KEY` and
//! `ARCHIL_REGION`, and creates and deletes real sandboxes:
//!
//! ```text
//! cargo test -p computer-conformance --features archil --test archil_live -- --ignored --nocapture
//! ```
#![cfg(feature = "archil")]

use computer_conformance::{Options, run};
use computer_driver::{archil::ArchilDriver, *};
use std::time::{Duration, Instant};

fn driver() -> Option<ArchilDriver> {
    std::env::var("ARCHIL_API_KEY")
        .ok()
        .filter(|v| !v.is_empty())?;
    Some(ArchilDriver::from_env().expect("driver"))
}

fn prefix(what: &str) -> String {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("oa-live-{what}-{n}")
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "creates real Archil sandboxes"]
async fn archil_passes_conformance() {
    let Some(d) = driver() else { return };
    let options = Options {
        poll: Duration::from_millis(500),
        deadline: Duration::from_secs(240),
        size: None,
        prefix: prefix("conf"),
    };
    let report = run(&d, &options).await;
    println!("{}", report.summary());
    assert!(report.passed());
}

/// Our sandboxes (names `oa-c-*` and `oa-f-*`): print them, and with
/// `ARCHIL_CLEANUP=1` stop and delete them. Never touches other names.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "lists real Archil sandboxes"]
async fn archil_list_ours() {
    let Some(d) = driver() else { return };
    let cleanup = std::env::var("ARCHIL_CLEANUP").as_deref() == Ok("1");
    for s in d
        .client()
        .sandboxes()
        .list(Default::default())
        .await
        .unwrap()
    {
        let ours = s.name.starts_with("oa-c-") || s.name.starts_with("oa-f-");
        println!("{} {} {} {:?}", s.id, s.name, s.status, s.exit_reason);
        if ours && cleanup {
            println!("  delete: {:?}", d.delete(&s.id).await);
        }
    }
}

async fn until_running(d: &ArchilDriver, id: &str) {
    let end = Instant::now() + Duration::from_secs(300);
    while !matches!(d.observe(id).await, Outcome::Done { value } if value.state == State::Running) {
        assert!(Instant::now() < end, "{id} never ran");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// The first command that succeeds, retried while the computer settles.
async fn first_command(d: &ArchilDriver, id: &str) {
    until_running(d, id).await;
    let end = Instant::now() + Duration::from_secs(120);
    loop {
        if let Outcome::Done { value } = d.exec(id, &Exec::new("echo ok")).await
            && value.stdout == "ok\n"
        {
            return;
        }
        assert!(Instant::now() < end, "{id} never ran a command");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn spec(id: &str, command: &str) -> CommandSpec {
    CommandSpec {
        id: id.into(),
        command: command.into(),
        cwd: ".".into(),
        credential_names: Default::default(),
        env: Default::default(),
        timeout_seconds: 120,
        digest: format!("{:0>64}", id.len()),
    }
}

async fn exited(d: &ArchilDriver, computer: &str, id: &str) -> CommandRead {
    let end = Instant::now() + Duration::from_secs(120);
    loop {
        if let Outcome::Done { value } = d
            .read_command(computer, id, CommandCursor::default(), 65536)
            .await
            && matches!(value.progress, CommandProgress::Exited { .. })
        {
            return value;
        }
        assert!(Instant::now() < end, "{id} never exited");
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

fn stats(name: &str, mut v: Vec<f64>) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |p: f64| v[((v.len() - 1) as f64 * p).round() as usize];
    println!(
        "MEASURE {name}: n={} p50={:.2}s p90={:.2}s min={:.2}s max={:.2}s all={:?}",
        v.len(),
        q(0.5),
        q(0.9),
        v[0],
        v[v.len() - 1],
        v.iter()
            .map(|x| (x * 100.0).round() / 100.0)
            .collect::<Vec<_>>()
    );
}

/// Create, stop/start, pause/resume, and fork, each to its first command,
/// through the driver; plus a retried identified command and a command
/// that lives through a pause. `ARCHIL_MEASURE_RUNS` (default 10).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "creates real Archil sandboxes"]
async fn archil_measure_lifecycle() {
    let Some(d) = driver() else { return };
    let stop = d.clone().with_stop_suspend();
    let runs: usize = std::env::var("ARCHIL_MEASURE_RUNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let (mut create, mut start, mut resume, mut fork, mut failures) =
        (vec![], vec![], vec![], vec![], vec![]);
    let mut made = Vec::new();
    for n in 0..runs {
        let op = prefix(&format!("measure-{n}"));
        let t = Instant::now();
        let id = match d
            .create(&CreateRequest {
                operation: op.clone(),
                ttl_seconds: Some(1800),
                ..Default::default()
            })
            .await
        {
            Outcome::Done { value } => value,
            other => {
                failures.push(format!("create {n}: {other:?}"));
                continue;
            }
        };
        made.push(id.clone());
        first_command(&d, &id).await;
        create.push(t.elapsed().as_secs_f64());

        if n == 0 {
            // A retried start runs once; a command lives through a pause.
            let once = spec("once", "echo run >> invocations");
            assert!(d.start_command(&id, &once).await.is_done());
            assert!(d.start_command(&id, &once).await.is_done());
            exited(&d, &id, "once").await;
            let count = d
                .exec(&id, &Exec::new("cat workspace/invocations"))
                .await
                .value()
                .unwrap();
            assert_eq!(count.stdout, "run\n", "a retried start ran twice");
            println!("MEASURE retried identified start ran once: ok");
            let long = spec("through-pause", "sleep 20; echo survived");
            assert!(d.start_command(&id, &long).await.is_done());
            tokio::time::sleep(Duration::from_secs(3)).await;
            assert!(d.suspend(&id).await.is_done());
            tokio::time::sleep(Duration::from_secs(5)).await;
            assert!(d.resume(&id).await.is_done());
            let read = exited(&d, &id, "through-pause").await;
            assert_eq!(read.stdout, b"survived\n");
            assert_eq!(read.progress, CommandProgress::Exited { code: 0 });
            println!("MEASURE command lived through pause/resume: ok");
        }

        // Pause, then resume to first command.
        assert!(d.suspend(&id).await.is_done(), "pause {n}");
        let t = Instant::now();
        match d.resume(&id).await {
            Outcome::Done { .. } => {
                first_command(&d, &id).await;
                resume.push(t.elapsed().as_secs_f64());
            }
            other => failures.push(format!("resume {n}: {other:?}")),
        }

        // Stop, then start to first command.
        assert!(stop.suspend(&id).await.is_done(), "stop {n}");
        let t = Instant::now();
        match stop.resume(&id).await {
            Outcome::Done { .. } => {
                first_command(&d, &id).await;
                start.push(t.elapsed().as_secs_f64());
            }
            other => failures.push(format!("start {n}: {other:?}")),
        }

        // Fork the running computer to the child's first command.
        let t = Instant::now();
        match d.fork(&id, &format!("{op}-fork")).await {
            Outcome::Done { value } => {
                made.push(value.clone());
                first_command(&d, &value).await;
                fork.push(t.elapsed().as_secs_f64());
            }
            other => failures.push(format!("fork {n}: {other:?}")),
        }
    }
    // Copies before the computers they came from.
    for id in made.into_iter().rev() {
        let out = d.delete(&id).await;
        if !out.is_done() {
            failures.push(format!("delete {id}: {out:?}"));
        }
    }
    stats("create to first command", create);
    stats("pause, resume to first command", resume);
    stats("stop, start to first command", start);
    stats("fork to first command", fork);
    println!("MEASURE failures: {failures:?}");
}
