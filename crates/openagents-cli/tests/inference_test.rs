//! Binary tests for `openagents inference run` through `map.done`.

use std::io::Write;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_openagents"))
}

fn fixture_gguf() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("qwen35.gguf");
    psionic_gguf::write_qwen35_fixture(&path).expect("write fixture");
    dir
}

fn stderr_ids(output: &std::process::Output) -> Vec<String> {
    let text = String::from_utf8_lossy(&output.stderr);
    text.lines()
        .filter_map(|line| {
            if line.starts_with('{') {
                let v: serde_json::Value = serde_json::from_str(line).ok()?;
                v.get("id")?.as_str().map(str::to_string)
            } else if let Some(rest) = line.strip_prefix('[') {
                rest.split(']').next().map(str::to_string)
            } else {
                None
            }
        })
        .collect()
}

fn json_states(output: &std::process::Output) -> Vec<(String, String)> {
    let text = String::from_utf8_lossy(&output.stderr);
    text.lines()
        .filter_map(|line| {
            let v: serde_json::Value = serde_json::from_str(line).ok()?;
            Some((
                v.get("id")?.as_str()?.to_string(),
                v.get("state")?.as_str()?.to_string(),
            ))
        })
        .collect()
}

#[test]
fn preview_prints_pending_script() {
    let output = bin()
        .args(["inference", "run", "--preview", "--json"])
        .output()
        .expect("run");
    assert!(output.status.success(), "{:?}", output);
    let states = json_states(&output);
    assert_eq!(states[0].0, "run.preview");
    assert_eq!(states[0].1, "ok");
    assert!(
        states
            .iter()
            .any(|(id, state)| id == "gguf.look" && state == "pending"),
        "{states:?}"
    );
    assert!(
        states
            .iter()
            .any(|(id, state)| id == "map.done" && state == "pending"),
        "{states:?}"
    );
    assert_eq!(states.last().map(|p| p.0.as_str()), Some("gen.done"));
}

#[test]
fn missing_gguf_fails_with_canonical_id() {
    let output = bin()
        .args(["inference", "run", "--json"])
        .output()
        .expect("run");
    assert!(!output.status.success());
    let states = json_states(&output);
    assert!(
        states
            .iter()
            .any(|(id, state)| id == "gguf.fail.arg" && state == "fail"),
        "{states:?}"
    );
}

#[test]
fn malformed_magic_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("not.gguf");
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(b"XXXX\x03\x00\x00\x00").unwrap();
    let output = bin()
        .args([
            "inference",
            "run",
            "--json",
            "--gguf",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("run");
    assert!(!output.status.success());
    let states = json_states(&output);
    assert!(
        states
            .iter()
            .any(|(id, state)| id == "gguf.fail.magic" && state == "fail"),
        "{states:?}"
    );
}

#[test]
fn until_meta_done_on_fixture() {
    let dir = fixture_gguf();
    let path = dir.path().join("qwen35.gguf");
    let output = bin()
        .args([
            "inference",
            "run",
            "--json",
            "--gguf",
            path.to_str().unwrap(),
            "--until",
            "meta.done",
        ])
        .output()
        .expect("run");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let states = json_states(&output);
    assert!(
        states
            .iter()
            .any(|(id, state)| id == "meta.done" && state == "ok"),
        "{states:?}"
    );
    assert!(
        !states.iter().any(|(id, _)| id == "tok.read"),
        "must stop at meta.done: {states:?}"
    );
}

#[test]
fn until_map_done_on_fixture() {
    let dir = fixture_gguf();
    let path = dir.path().join("qwen35.gguf");
    let output = bin()
        .args([
            "inference",
            "run",
            "--json",
            "--gguf",
            path.to_str().unwrap(),
            "--until",
            "map.done",
        ])
        .output()
        .expect("run");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let states = json_states(&output);
    assert!(
        states
            .iter()
            .any(|(id, state)| id == "map.done" && state == "ok"),
        "{states:?}"
    );
    assert!(
        !states
            .iter()
            .any(|(id, _)| id == "ctx.alloc" || id == "build.stop"),
        "{states:?}"
    );
}

#[test]
fn inspect_fixture() {
    let dir = fixture_gguf();
    let path = dir.path().join("qwen35.gguf");
    let output = bin()
        .args(["psionic", "inspect", path.to_str().unwrap(), "--json"])
        .output()
        .expect("run");
    assert!(output.status.success(), "{:?}", output);
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["architecture"], "qwen35");
}

#[test]
fn bench_fixture_prints_json_summary() {
    let dir = fixture_gguf();
    let path = dir.path().join("qwen35.gguf");
    let output = bin()
        .args([
            "inference",
            "bench",
            "--gguf",
            path.to_str().unwrap(),
            "--prompt",
            "hello",
            "--max-tokens",
            "2",
        ])
        .output()
        .expect("run");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|_| panic!("bench stdout must be JSON, got {stdout:?}"));
    assert_eq!(v["engine"], "openagents");
    assert_eq!(v["graph"], "embed_lmhead");
    assert!(v["generated"].as_u64().unwrap() >= 1, "{v}");
    assert!(v.get("tok_per_s").is_some(), "{v}");
    assert!(v.get("map_ms").is_some(), "{v}");
}

#[test]
fn help_lists_inference() {
    let output = bin().args(["inference", "--help"]).output().expect("run");
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("run"), "{text}");
    assert!(text.contains("bench"), "{text}");
}

#[test]
fn text_preview_mentions_looking_for_gguf() {
    let output = bin()
        .args(["inference", "run", "--preview"])
        .output()
        .expect("run");
    assert!(output.status.success());
    let ids = stderr_ids(&output);
    assert!(ids.contains(&"run.preview".into()));
    assert!(ids.contains(&"gguf.look".into()));
}

#[test]
fn unload_with_nothing_loaded_still_prints_weights_unloaded() {
    let output = bin()
        .args(["inference", "unload", "--json"])
        .output()
        .expect("run");
    assert!(output.status.success(), "{:?}", output);
    let states = json_states(&output);
    assert!(
        states
            .iter()
            .any(|(id, state)| id == "unload.done" && state == "ok"),
        "{states:?}"
    );
}

#[test]
fn until_ctx_done_on_fixture() {
    let dir = fixture_gguf();
    let path = dir.path().join("qwen35.gguf");
    let output = bin()
        .args([
            "inference",
            "run",
            "--json",
            "--gguf",
            path.to_str().unwrap(),
            "--until",
            "ctx.done",
        ])
        .output()
        .expect("run");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let states = json_states(&output);
    assert!(
        states
            .iter()
            .any(|(id, state)| id == "ctx.done" && state == "ok"),
        "{states:?}"
    );
    assert!(
        !states
            .iter()
            .any(|(id, _)| id == "prompt.template" || id == "build.stop"),
        "{states:?}"
    );
}

#[test]
fn until_prompt_done_on_fixture() {
    let dir = fixture_gguf();
    let path = dir.path().join("qwen35.gguf");
    let output = bin()
        .args([
            "inference",
            "run",
            "--json",
            "--gguf",
            path.to_str().unwrap(),
            "--prompt",
            "hello",
            "--until",
            "prompt.done",
        ])
        .output()
        .expect("run");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let states = json_states(&output);
    assert!(
        states
            .iter()
            .any(|(id, state)| id == "prompt.done" && state == "ok"),
        "{states:?}"
    );
    assert!(
        !states.iter().any(|(id, _)| id == "prefill.start"),
        "{states:?}"
    );
}

#[test]
fn until_gen_done_on_fixture() {
    let dir = fixture_gguf();
    let path = dir.path().join("qwen35.gguf");
    let output = bin()
        .args([
            "inference",
            "run",
            "--json",
            "--gguf",
            path.to_str().unwrap(),
            "--prompt",
            "hello",
            "--max-tokens",
            "2",
            "--until",
            "gen.done",
        ])
        .output()
        .expect("run");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let states = json_states(&output);
    assert!(
        states
            .iter()
            .any(|(id, state)| id == "prefill.done" && state == "ok"),
        "{states:?}"
    );
    assert!(
        states
            .iter()
            .any(|(id, state)| id == "gen.done" && state == "ok"),
        "{states:?}"
    );
}

#[test]
fn missing_prompt_fails_after_ctx() {
    let dir = fixture_gguf();
    let path = dir.path().join("qwen35.gguf");
    let output = bin()
        .args([
            "inference",
            "run",
            "--json",
            "--gguf",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("run");
    assert!(!output.status.success());
    let states = json_states(&output);
    assert!(
        states
            .iter()
            .any(|(id, state)| id == "prompt.fail.empty" && state == "fail"),
        "{states:?}"
    );
}

#[test]
fn status_json_includes_memory_fields() {
    let output = bin()
        .args(["inference", "status", "--json"])
        .output()
        .expect("run");
    assert!(output.status.success(), "{:?}", output);
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["loaded"], false);
    assert_eq!(v["mmap_bytes"], 0);
    assert_eq!(v["metal_bytes"], 0);
    assert!(v.get("rss_bytes").is_some(), "{v}");
    assert_eq!(v["cache_kv_bytes"], 0);
    assert_eq!(v["cache_gdn_bytes"], 0);
}
