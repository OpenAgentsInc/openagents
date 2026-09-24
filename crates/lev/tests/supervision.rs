//! A helper that misbehaves costs the call it was holding and nothing after.
//!
//! The Swift helper is a child process, so the bridge has to survive the ways
//! a child process fails without waiting for Apple's runtime to fail that
//! way. A fake helper — a shell script that speaks the line protocol — stands
//! in for each fault: it hangs, floods stderr, answers with a line that never
//! ends, answers the wrong call, or exits. After each, the pool lane that
//! held it answers the next call on a fresh helper.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use lev::bridge::{Bridge, Call, MAX_RESPONSE_BYTES, Pool, Sampling};
use lev::error::RefusalCode;
use lev::schema::Compiled;

/// The fake helper. `$FAKE_MODE` picks the fault; `$FAKE_STATE` is a file
/// whose presence flips `hang-once` and `exit-once` from faulting to
/// answering, which is how the test tells a restarted helper apart from the
/// one that was retired.
const FAKE: &str = r#"#!/bin/sh
answer() {
    id=$(printf '%s' "$1" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
    printf '{"id":"%s","ok":true,"choice":"yes","latencyMs":1}\n' "$id"
}
while IFS= read -r line; do
    case "$line" in
        *'"op":"availability"'*)
            id=$(printf '%s' "$line" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
            printf '{"id":"%s","ok":true,"availability":{"status":"available"}}\n' "$id"
            continue ;;
        *'"op":"adapter_compat"'*)
            id=$(printf '%s' "$line" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
            printf '{"id":"%s","ok":true,"compatibleAdapters":["fmadapter-lev-fake"]}\n' "$id"
            continue ;;
    esac
    case "$FAKE_MODE" in
        ok) answer "$line" ;;
        hang) sleep 60 ;;
        hang-once)
            if [ -e "$FAKE_STATE" ]; then answer "$line"; else : > "$FAKE_STATE"; sleep 60; fi ;;
        flood)
            head -c 1048576 /dev/zero | tr '\0' 'e' >&2
            echo "fake: flooded" >&2
            answer "$line" ;;
        long)
            head -c 2097152 /dev/zero | tr '\0' 'x'
            sleep 60 ;;
        wrong-id) printf '{"id":"not-this-one","ok":true,"choice":"yes"}\n' ;;
        exit) echo "fake: leaving" >&2; exit 3 ;;
        exit-once)
            if [ -e "$FAKE_STATE" ]; then answer "$line"; else : > "$FAKE_STATE"; echo "fake: leaving" >&2; exit 3; fi ;;
        garbage) printf 'not json\n' ;;
        *) echo "fake: unknown mode $FAKE_MODE" >&2; exit 2 ;;
    esac
done
"#;

struct Fake {
    dir: tempfile::TempDir,
}

impl Fake {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("fake-helper");
        std::fs::write(&path, FAKE).expect("write the fake helper");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        Self { dir }
    }

    fn path(&self) -> PathBuf {
        self.dir.path().join("fake-helper")
    }

    fn state(&self) -> PathBuf {
        self.dir.path().join("state")
    }
}

/// Points the helper at a fault. The helper reads its mode from the
/// environment, so the test process sets it before each start; the tests in
/// this file therefore run in one thread, in sequence.
fn mode(fake: &Fake, mode: &str) {
    // SAFETY: this file's tests run sequentially in one test function, and
    // nothing else in the process reads these variables concurrently.
    unsafe {
        std::env::set_var("FAKE_MODE", mode);
        std::env::set_var("FAKE_STATE", fake.state());
        std::env::set_var("LEV_BRIDGE_ALLOW_UNSIGNED", "1");
    }
}

fn call() -> Call {
    let compiled = Compiled {
        kind: lev::schema::Kind::Noul,
        instructions: "Was it late?".to_string(),
        prompt: "It arrived two days late.".to_string(),
        options: vec!["yes".to_string(), "no".to_string()],
        legend: IndexMap::new(),
    };
    Call::decide(&compiled, Sampling::Greedy)
}

const DEADLINE: Duration = Duration::from_millis(500);

fn start(path: &Path) -> Bridge {
    Bridge::start_with_deadline(path, DEADLINE).expect("the fake helper starts")
}

fn bridge_error(bridge: &mut Bridge, needle: &str) {
    let refusal = bridge.decide(&call()).expect_err("the fault is refused");
    assert_eq!(refusal.code, RefusalCode::BridgeError, "{refusal:?}");
    assert!(
        refusal.message.contains(needle),
        "expected {needle:?} in {:?}",
        refusal.message
    );
    assert!(bridge.is_retired(), "the fault retires the helper");
    let again = bridge
        .decide(&call())
        .expect_err("a retired helper refuses");
    assert!(again.message.contains("retired"), "{again:?}");
}

#[test]
fn faults_retire_a_helper_and_the_lane_recovers() {
    let fake = Fake::new();
    let path = fake.path();

    // The protocol itself, through the reader thread.
    mode(&fake, "ok");
    let mut bridge = start(&path);
    let outcome = bridge.decide(&call()).expect("the fake answers");
    assert_eq!(outcome.choice.as_deref(), Some("yes"));
    assert!(!bridge.is_retired());

    // A hang is cut at the deadline, not at the caller's patience.
    mode(&fake, "hang");
    let mut bridge = start(&path);
    let started = Instant::now();
    bridge_error(&mut bridge, "did not answer within");
    let waited = started.elapsed();
    assert!(
        waited >= DEADLINE && waited < DEADLINE * 8,
        "the deadline governed the wait: {waited:?}"
    );

    // A megabyte of stderr — sixteen pipe buffers — does not wedge a helper
    // that then answers, and the tail keeps the end of it.
    mode(&fake, "flood");
    let mut bridge = start(&path);
    let outcome = bridge
        .decide(&call())
        .expect("the flooding fake still answers");
    assert_eq!(outcome.choice.as_deref(), Some("yes"));
    let tail = bridge.stderr_tail();
    assert!(tail.ends_with("fake: flooded"), "tail: {tail:?}");
    assert!(
        tail.len() <= 4096 + 16,
        "the tail is bounded: {}",
        tail.len()
    );

    // A line that passes the frame cap without ending.
    mode(&fake, "long");
    let mut bridge = start(&path);
    bridge_error(&mut bridge, &format!("over {MAX_RESPONSE_BYTES} bytes"));

    // An answer to a call nobody made.
    mode(&fake, "wrong-id");
    let mut bridge = start(&path);
    bridge_error(&mut bridge, "answered call not-this-one");

    // A helper that leaves, with its last words in the refusal.
    mode(&fake, "exit");
    let mut bridge = start(&path);
    bridge_error(&mut bridge, "fake: leaving");

    // A helper that stops speaking JSON.
    mode(&fake, "garbage");
    let mut bridge = start(&path);
    bridge_error(&mut bridge, "not valid JSON");

    // Recovery: the first helper in the lane hangs and is retired; the
    // pool's next call on that lane runs on a fresh process and succeeds.
    mode(&fake, "hang-once");
    let pool = Pool::start(&path, 1, DEADLINE).expect("a one-lane pool starts");
    let first = pool.decide_all(&[call()]);
    let refusal = first[0].as_ref().expect_err("the first call hits the hang");
    assert!(
        refusal.message.contains("did not answer within"),
        "{refusal:?}"
    );
    let second = pool.decide_all(&[call(), call(), call()]);
    for outcome in &second {
        let outcome = outcome.as_ref().expect("the restarted helper answers");
        assert_eq!(outcome.choice.as_deref(), Some("yes"));
    }
    assert!(fake.state().exists(), "the first helper recorded its hang");

    // Recovery from a crash: the lane's helper exits mid-batch, the call it
    // held comes back as that fault with its last words, and the lane's
    // remaining calls run on the replacement.
    mode(&fake, "exit-once");
    let _ = std::fs::remove_file(fake.state());
    let pool = Pool::start(&path, 1, DEADLINE).expect("a one-lane pool starts");
    let batch = pool.decide_all(&[call(), call(), call()]);
    let crashed = batch[0].as_ref().expect_err("the first call hits the exit");
    assert_eq!(crashed.code, RefusalCode::BridgeError, "{crashed:?}");
    assert!(crashed.message.contains("fake: leaving"), "{crashed:?}");
    for outcome in &batch[1..] {
        let outcome = outcome.as_ref().expect("the replacement answers");
        assert_eq!(outcome.choice.as_deref(), Some("yes"));
    }
    assert!(fake.state().exists(), "the first helper recorded its exit");

    #[cfg(feature = "serve")]
    a_door_over_a_hung_helper_stays_responsive(&fake);
}

/// The door answers `busy` when every in-flight slot is held, keeps its
/// runtime threads free while a helper hangs, and answers the request after
/// the hang on a fresh helper.
///
/// Runs inside the sequential test above's process but on its own runtime,
/// so it is called from there rather than marked `#[tokio::test]`.
#[cfg(feature = "serve")]
fn a_door_over_a_hung_helper_stays_responsive(fake: &Fake) {
    use std::sync::Arc;

    use lev::serve::Door;
    use serde_json::{Value, json};

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("a runtime");
    runtime.block_on(async {
        mode(fake, "hang-once");
        let _ = std::fs::remove_file(fake.state());
        let pool = Pool::start(&fake.path(), 1, Duration::from_secs(2)).expect("the pool starts");
        let door = Arc::new(Door::new(pool, "lev-fake", 2).with_in_flight(1));
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("a port");
        let port = listener.local_addr().expect("an address").port();
        tokio::spawn(async move {
            let _ = axum::serve(listener, door.router()).await;
        });
        let url = format!("http://127.0.0.1:{port}/v1/systemone");
        let body = json!({
            "state": "It arrived two days late.",
            "questions": {"late": {"type": "noul", "instructions": "Was it late?"}},
        });

        // The first request holds the one slot for the deadline. Its helper
        // hangs, and the second request — sent while the first waits — is
        // refused as busy at once instead of queueing behind it.
        let hung = tokio::spawn({
            let url = url.clone();
            let body = body.clone();
            async move { reqwest::Client::new().post(url).json(&body).send().await }
        });
        tokio::time::sleep(Duration::from_millis(200)).await;
        let started = Instant::now();
        let busy = reqwest::Client::new()
            .post(&url)
            .json(&body)
            .send()
            .await
            .expect("the busy answer arrives");
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "busy was answered without waiting on the hung helper"
        );
        assert_eq!(busy.status().as_u16(), 429);
        let busy: Value = busy.json().await.expect("json");
        assert_eq!(busy["error"]["code"], "busy");

        let hung = hung
            .await
            .expect("joined")
            .expect("the hung answer arrives");
        assert_eq!(hung.status().as_u16(), 500);
        let hung: Value = hung.json().await.expect("json");
        assert_eq!(hung["error"]["code"], "bridge_error");
        assert!(
            hung["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("did not answer within"),
            "{hung}"
        );

        // The lane's helper was retired; the next request runs on a new one.
        let answered = reqwest::Client::new()
            .post(&url)
            .json(&body)
            .send()
            .await
            .expect("the recovered answer arrives");
        assert_eq!(
            answered.status().as_u16(),
            200,
            "{:?}",
            answered.text().await
        );
    });
}
