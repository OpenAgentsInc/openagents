#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stderr)]
//! Stage processes over loopback, driven by a minimal front that speaks
//! Pylon's stage-group control protocol. Pylon's own front runs these
//! processes in `crates/pylon/tests/group_stage_worker.rs`.

mod common;

use std::{net::TcpStream, process::Command, time::Duration};

use common::{BIN, Front, LIMIT, args, events, reference, run_bounded, weights};
use psionic_models::stage_state::{CodecLimits, StageMessage, StepKind, encode};
use psionic_v41_stage::{cpu_worker, load_model, serve::write_frame, synthetic_manifest};
use serde_json::json;

#[test]
fn groups_of_two_to_four_processes_answer_like_the_unsplit_model() {
    let prompt = json!([{"role": "user", "content": "tok3 tok5 tok0 tok6 tok4 tok7"}]);
    for split in [
        vec![(0, 20), (20, 40)],
        vec![(0, 2), (2, 21), (21, 40)],
        vec![(0, 10), (10, 20), (20, 30), (30, 40)],
    ] {
        let root = tempfile::tempdir().unwrap();
        let manifest = weights(root.path(), &split);
        // Stages find the manifest by its digest in the weights directory,
        // as `pylon cloud up` names it.
        let digest = format!(
            "sha256:{}",
            serde_json::from_slice::<psionic_models::deepseek_v41_stage::V41GroupManifest>(
                &std::fs::read(&manifest).unwrap()
            )
            .unwrap()
            .digest_hex()
        );
        let mut front = Front::start(root.path(), &digest, &split, None);
        let expected = reference(root.path(), &prompt, 10);
        assert_eq!(front.run(&prompt, 10), expected, "{split:?}");
    }
}

/// CPU stages that speak the CUDA stages' wire (#12418) announce it, their
/// device class, and answer: BF16 cache rows cross every boundary.
#[test]
fn cpu_stages_on_the_cuda_wire_announce_it_and_answer() {
    let prompt = json!([{"role": "user", "content": "tok3 tok5 tok0 tok6 tok4 tok7"}]);
    let split = [(0, 2), (2, 21), (21, 40)];
    let root = tempfile::tempdir().unwrap();
    let manifest = weights(root.path(), &split).display().to_string();
    let mut front = Front::start_with(
        root.path(),
        &manifest,
        &split,
        None,
        &["--wire", "cuda"],
        "psionic-v41-stage/1",
    );
    for stage in &front.stages {
        assert_eq!(
            stage.hello["wire"],
            json!({"residual": "f32", "cache": "bf16"})
        );
        assert_eq!(
            stage.hello["device"]["class"],
            format!("cpu-{}", std::env::consts::ARCH)
        );
    }
    let (tokens, _) = front.run(&prompt, 10);
    assert!(!tokens.is_empty());
    let mut native = Front::start(root.path(), &manifest, &split, None);
    assert_eq!(
        native.stages[0].hello["wire"],
        json!({"residual": "f32", "cache": "f32"})
    );
    let _ = native.run(&prompt, 10);
}

/// `plan` places a group over mixed GPU types from the measured memory
/// and prints a manifest whose stages name their devices (#12418).
#[test]
fn plan_places_a_mixed_device_group() {
    let out = run_bounded(
        Command::new(BIN).args([
            "plan",
            "--device",
            "g4:0:cuda-sm_120:101973819392",
            "--device",
            "g4:1:cuda-sm_120:101973819392",
            "--device",
            "h100:0:cuda-sm_90:84000000000",
            "--device",
            "a100:0:cuda-sm_80:42000000000",
            "--device",
            "home:0:cuda-sm_89:5500000000:host-experts",
            "--ram",
            "g4:380000000000",
            "--ram",
            "h100:240000000000",
            "--ram",
            "a100:90000000000",
            "--ram",
            "home:60000000000",
            "--split",
            "0-12,12-24,24-34,34-38,38-40",
        ]),
        LIMIT,
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let manifest: psionic_models::deepseek_v41_stage::V41GroupManifest =
        serde_json::from_slice(&out.stdout).unwrap();
    manifest.validate().unwrap();
    let classes: Vec<_> = manifest
        .stages
        .iter()
        .map(|s| s.device.as_ref().unwrap().class.as_str())
        .collect();
    assert_eq!(
        classes,
        [
            "cuda-sm_120",
            "cuda-sm_120",
            "cuda-sm_90",
            "cuda-sm_80",
            "cuda-sm_89"
        ]
    );
    // Groups run on GPUs only: a CPU stage needs the diagnostic flag.
    let cpu = [
        "plan",
        "--device",
        "a:0:cpu-x86_64:900000000000",
        "--ram",
        "a:900000000000",
    ];
    let refused = run_bounded(Command::new(BIN).args(cpu), LIMIT);
    assert_eq!(refused.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&refused.stderr).contains("GPUs only"));
    let diagnostic = run_bounded(Command::new(BIN).args(cpu).arg("--diagnostic-cpu"), LIMIT);
    assert!(diagnostic.status.success());
    // Without its experts on the host, the 16 GB card can't hold four layers.
    let short = run_bounded(
        Command::new(BIN).args([
            "plan",
            "--device",
            "a:0:cuda-sm_89:5500000000",
            "--device",
            "b:0:cuda-sm_120:101973819392",
            "--ram",
            "a:60000000000",
            "--ram",
            "b:380000000000",
            "--split",
            "0-4,4-40",
        ]),
        LIMIT,
    );
    assert_eq!(short.status.code(), Some(2));
}

fn refusal(args: &[String]) -> String {
    let out = run_bounded(Command::new(BIN).args(args), LIMIT);
    assert_eq!(out.status.code(), Some(2));
    String::from_utf8(out.stderr).unwrap()
}

fn with_front(mut a: Vec<String>) -> Vec<String> {
    a.extend(
        [
            "--stage",
            "s0",
            "--generation",
            "1",
            "--front",
            "127.0.0.1:9",
            "--token",
            "t",
        ]
        .map(String::from),
    );
    a
}

#[test]
fn missing_weights_and_a_mismatched_layer_range_are_refused() {
    let root = tempfile::tempdir().unwrap();
    let split = [(0, 20), (20, 40)];
    let manifest = weights(root.path(), &split);
    let manifest = manifest.display().to_string();
    let empty = root.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    let mut a = args(&empty, &manifest, &split, 0);
    a.truncate(8);
    a.push("--embeddings".into());
    assert!(refusal(&with_front(a)).contains("weights are missing"));
    let mut a = args(root.path(), &manifest, &split, 0);
    a[3] = "0-10".into();
    assert!(refusal(&with_front(a)).contains("don't match a stage of the manifest"));
    let mut a = args(root.path(), &manifest, &split, 1);
    a.push("--embeddings".into());
    assert!(refusal(&with_front(a)).contains("command line disagrees"));
    let a = args(root.path(), "sha256:00", &split, 0);
    assert!(refusal(&with_front(a)).contains("no group manifest with digest"));
    if cfg!(not(feature = "cuda")) {
        let mut a = args(root.path(), &manifest, &split, 0);
        a.extend(["--backend".into(), "cuda".into()]);
        assert!(refusal(&with_front(a)).contains("no CUDA stage executor"));
    }
}

#[test]
fn wrong_generation_and_stale_state_are_refused_before_use() {
    let root = tempfile::tempdir().unwrap();
    let split = [(0, 20), (20, 40)];
    let manifest_path = weights(root.path(), &split);
    let mut front = Front::start(
        root.path(),
        &manifest_path.display().to_string(),
        &split,
        Some(1),
    );
    // Make stage 0's real output in this process.
    let model: &'static _ = Box::leak(Box::new(load_model(root.path()).unwrap()));
    let manifest = synthetic_manifest(&[0..20, 20..40]).unwrap();
    let mut first = cpu_worker(
        model,
        &manifest,
        &manifest.stages[0],
        model.arithmetic,
        64,
        1,
    )
    .unwrap();
    let message = StageMessage {
        manifest_digest: manifest.digest(),
        generation: 1,
        request_id: "r".into(),
        attempt: 1,
        stage: 0,
        layers: 0..20,
        step: 0,
        kind: StepKind::Prefill,
        position: 0,
        tokens: vec![3, 5, 0, 6],
        sections: Vec::new(),
    };
    let sections = first.step(message.clone()).unwrap();
    let hop = message.next_hop(1, 20..40, sections);
    let stage = &mut front.stages[0];
    let listen = stage.hello["listen"].as_str().unwrap().to_owned();
    let mut link = TcpStream::connect(&listen).unwrap();
    link.set_write_timeout(Some(LIMIT)).unwrap();
    let limits = CodecLimits::default();
    stage.send(&json!({
        "kind": "step", "generation": 1, "request_id": "r", "attempt": 1, "stage": 1, "step": 0,
        "sampling": {"temperature": 0.0, "top_p": 1.0, "seed": 0},
    }));
    // From a generation the group has left.
    let mut old = hop.clone();
    old.generation = 0;
    write_frame(&mut link, &encode(&old, &limits).unwrap()).unwrap();
    // From another group.
    let mut other = hop.clone();
    other.manifest_digest = [9; 32];
    write_frame(&mut link, &encode(&other, &limits).unwrap()).unwrap();
    // A step the stage has already taken: its state is stale.
    let mut early = hop.clone();
    early.position = 4;
    write_frame(&mut link, &encode(&early, &limits).unwrap()).unwrap();
    // The real state: the stage acknowledges with a token.
    write_frame(&mut link, &encode(&hop, &limits).unwrap()).unwrap();
    let ack = stage.recv();
    assert_eq!(ack["kind"], "ack");
    assert!(ack["token"].is_u64());
    // The same state again is a duplicate.
    write_frame(&mut link, &encode(&hop, &limits).unwrap()).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let reasons: Vec<String> = events(root.path())
        .into_iter()
        .filter(|e| e["event"] == "refused")
        .map(|e| e["reason"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(reasons.len(), 4, "{reasons:#?}");
    assert!(reasons[0].contains("generation 0 is older"), "{reasons:#?}");
    assert!(reasons[1].contains("manifest"), "{reasons:#?}");
    // The stale and duplicate messages name the step the stage expects.
    assert!(
        reasons[2..]
            .iter()
            .all(|r| r.contains("position") || r.contains("step")),
        "{reasons:#?}"
    );
}

#[test]
fn an_oversized_request_is_rejected_before_any_stage_runs_it() {
    let root = tempfile::tempdir().unwrap();
    let split = [(0, 20), (20, 40)];
    let manifest = weights(root.path(), &split).display().to_string();
    let mut front = Front::start_with(
        root.path(),
        &manifest,
        &split,
        None,
        &["--max-context", "64", "--max-input-tokens", "4"],
        "psionic-v41-stage/1",
    );
    for stage in &front.stages {
        let limits = &stage.hello["limits"];
        assert_eq!(limits["max_context_tokens"], 64, "{limits}");
        assert_eq!(limits["max_input_tokens"], 4, "{limits}");
        assert_eq!(limits["modes"], json!(["thinking", "chat"]), "{limits}");
    }
    let next = front.stages[1].hello["listen"].clone();
    let step = |request: &str, prompt: serde_json::Value, max_tokens: u64| {
        json!({
            "kind": "step", "generation": 1, "request_id": request, "attempt": 1,
            "stage": 0, "step": 0, "prompt": prompt, "max_tokens": max_tokens,
            "mode": "chat", "next": next,
        })
    };
    // Six prompt tokens, over the operator's four.
    let long = json!([{"role": "user", "content": "tok3 tok5 tok0 tok6 tok4 tok7"}]);
    let message = step("big", long, 5);
    front.stages[0].send(&message);
    let rejected = front.stages[0].recv();
    assert_eq!(rejected["kind"], "rejected", "{rejected}");
    assert_eq!(rejected["code"], "request_limit");
    assert_eq!(rejected["request_id"], "big");
    assert_eq!(
        (rejected["limit"].as_u64(), rejected["actual"].as_u64()),
        (Some(4), Some(6))
    );
    // A short prompt whose answer could pass the context.
    let short = json!([{"role": "user", "content": "tok3 tok5"}]);
    let message = step("long", short.clone(), 63);
    front.stages[0].send(&message);
    let rejected = front.stages[0].recv();
    assert_eq!(
        (rejected["kind"].as_str(), rejected["code"].as_str()),
        (Some("rejected"), Some("request_limit"))
    );
    assert_eq!(
        (rejected["limit"].as_u64(), rejected["actual"].as_u64()),
        (Some(64), Some(65))
    );
    // A prompt the stage can't read is rejected too, not left to time out.
    let message = step("bad", json!("not a conversation"), 5);
    front.stages[0].send(&message);
    let rejected = front.stages[0].recv();
    assert_eq!(
        (rejected["kind"].as_str(), rejected["code"].as_str()),
        (Some("rejected"), Some("invalid_request"))
    );
    // The stages stay healthy and answer the next request as before.
    let expected = reference(root.path(), &short, 4);
    assert_eq!(front.run(&short, 4), expected);
    let events = events(root.path());
    assert_eq!(
        events.iter().filter(|e| e["event"] == "rejected").count(),
        3,
        "{events:#?}"
    );
    assert!(
        !events
            .iter()
            .any(|e| e["event"] == "step" && e["stage"] == "s1" && e["request"] == "big"),
        "{events:#?}"
    );
}
