//! The deterministic variant: synthetic Coder transcripts, the loopback
//! relay, and a loopback direct listener. No network, nothing private.
//! It checks that every phase ran; it asserts no timing.

use chat_load_bench::bench::{self, Options, Relay, Source};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_fixture_benchmark_runs_every_phase() {
    let report = bench::run(Options {
        source: Source::CoderFixture(40),
        relay: Relay::Fixture,
        runs: 1,
        chats: 2,
        use_relay: true,
        use_direct: true,
        basic_coder: 0,
        basic_coder_local: 0,
        exe: Some(env!("CARGO_BIN_EXE_chat-load-bench").into()),
    })
    .await
    .expect("the benchmark runs");
    let names: Vec<&str> = report.phases.iter().map(|p| p.name.as_str()).collect();
    for wanted in [
        "catalog page 1, first read (scan + every head)",
        "relay link: connect + NIP-42 AUTH + standing subscription",
        "request EVENT to its reply (relay in, host answers, relay out)",
        "list: load done (new client, link opened by the first read)",
        "chat open: batch done (10 messages or 12 pages)",
        "direct connect: TCP + hello/welcome",
        "list: load done (new client)",
        "chat open: batch done",
        "layout: first layout of an opened chat",
        "list: load done after Client::warm",
    ] {
        assert!(names.contains(&wanted), "no phase {wanted:?} in {names:?}");
    }
    assert!(
        !names.iter().any(|name| name.contains("FAILED")),
        "a read failed: {names:?}"
    );
    assert!(report.phases.iter().all(|p| !p.samples.is_empty()));
}
