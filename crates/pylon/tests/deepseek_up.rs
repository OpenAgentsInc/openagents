#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The persisted text-only profile and its artifact/admission boundaries.

use pylon::{
    online::{self, Up},
    serial::Profile,
    share,
};
use std::path::Path;

fn choice(dir: &Path) -> Up {
    let file = dir.join("model.gguf");
    let server = dir.join("llama-server");
    std::fs::write(&file, b"fixture").unwrap();
    std::fs::write(&server, b"fixture").unwrap();
    Up::parse(&[
        "--text-profile".into(),
        Profile::MODEL.into(),
        "--text-model".into(),
        file.display().to_string(),
        "--managed-llama".into(),
        server.display().to_string(),
        "--allow-any".into(),
    ])
    .unwrap()
}

#[test]
fn profile_reuses_serial_recovery_without_a_decision_lane() {
    let dir = tempfile::tempdir().unwrap();
    let mut up = choice(dir.path());
    up.generation_seconds = Some(10);
    up.startup_seconds = Some(120);
    let (settings, notes) = online::settings_for_up(
        None,
        "test",
        "ws://relay",
        &up,
        "cpu",
        Err("no room for Clef".into()),
    )
    .unwrap();
    assert!(notes.is_empty() && settings.decide.is_none());
    assert_eq!((settings.slots, settings.max_tokens), (1, 2048));
    assert_eq!(
        settings.config(dir.path()).class.family,
        nostr::pylon::Family::Gpu
    );
    let lane = settings.text.as_ref().unwrap();
    assert_eq!(lane.profile, Some(Profile::DeepSeekR1DistillQwen15B));
    assert_eq!(lane.engine, share::TextEngine::LlamaCpp);
    assert_eq!(lane.model, Profile::MODEL);
    assert_eq!(lane.device.as_deref(), Some("cuda"));
    let config = lane.serial.as_ref().unwrap();
    assert_eq!(
        (
            config.generation_seconds,
            config.startup_seconds,
            config.max_restarts
        ),
        (10, 120, 3)
    );
    assert_eq!(
        serde_json::to_value(config.sampling).unwrap(),
        serde_json::json!({"temperature":0.6,"top_p":0.95,"seed":1})
    );
    share::save(dir.path(), &settings).unwrap();
    let held = share::load(dir.path()).unwrap().unwrap();
    assert_eq!(held.schema, share::PROFILE_SCHEMA);
    let mut downgraded = serde_json::to_value(&held).unwrap();
    downgraded["schema"] = share::SERIAL_SCHEMA.into();
    std::fs::write(
        share::path(dir.path()),
        serde_json::to_vec(&downgraded).unwrap(),
    )
    .unwrap();
    assert!(share::load(dir.path()).is_err());
    share::save(dir.path(), &held).unwrap();
    let lane = online::lanes(&held).pop().unwrap();
    let command = online::psionic_command(&lane.serial.as_ref().unwrap().server, &lane);
    let args: Vec<_> = command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect();
    for pair in [
        ["--parallel", "1"],
        ["--device", "CUDA0"],
        ["--n-gpu-layers", "all"],
        ["--reasoning-format", "deepseek"],
    ] {
        assert!(args.windows(2).any(|args| args == pair));
    }

    let (resumed, _) = online::settings_for_up(
        Some(held.clone()),
        "test",
        "ws://relay",
        &Up::parse(&[]).unwrap(),
        "cpu",
        Err("no Clef".into()),
    )
    .unwrap();
    assert_eq!(held.text, resumed.text);
    assert!(resumed.decide.is_none());
    let (cleared, _) = online::settings_for_up(
        Some(held),
        "test",
        "ws://relay",
        &Up::parse(&["--text-profile".into(), "off".into()]).unwrap(),
        "cpu",
        Ok(()),
    )
    .unwrap();
    assert!(cleared.decide.is_some());
    assert!(cleared.text.unwrap().serial.is_none());
}

#[test]
fn unsafe_or_unqualified_profile_choices_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let original = choice(dir.path());
    let mut choices = vec![];
    let mut up = original.clone();
    up.text = false;
    choices.push(up);
    let mut up = original.clone();
    up.device = Some("cpu".into());
    choices.push(up);
    let mut up = original.clone();
    up.text_device = Some("metal".into());
    choices.push(up);
    let mut up = original.clone();
    up.text_engine = Some(share::TextEngine::Openai);
    choices.push(up);
    let mut up = original.clone();
    up.text_gpu_layers = Some("auto".into());
    choices.push(up);
    let mut up = original.clone();
    up.text_stop = Some(vec!["stop".into()]);
    choices.push(up);
    let mut up = original.clone();
    up.generation_seconds = Some(76);
    choices.push(up);
    let mut up = original.clone();
    up.startup_seconds = Some(0);
    choices.push(up);
    let mut up = original.clone();
    up.restart_limit = Some(0);
    choices.push(up);
    let mut up = original.clone();
    up.text_port = Some(0);
    choices.push(up);
    let mut up = original.clone();
    up.serve.extend(["--slots".into(), "2".into()]);
    choices.push(up);
    let mut up = original;
    up.serve.extend(["--max-tokens".into(), "1".into()]);
    choices.push(up);
    for up in choices {
        assert!(
            online::settings_for_up(None, "test", "ws://relay", &up, "cuda", Ok(())).is_err(),
            "{up:?}"
        );
    }
    assert!(Up::parse(&["--text-profile".into(), "unknown".into()]).is_err());
}

#[test]
fn changing_models_clears_the_saved_profile_and_its_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let up = choice(dir.path());
    let (held, _) =
        online::settings_for_up(None, "test", "ws://relay", &up, "cuda", Ok(())).unwrap();
    let other = dir.path().join("other.gguf");
    std::fs::write(&other, b"fixture").unwrap();
    let up = Up::parse(&["--text-model".into(), other.display().to_string()]).unwrap();
    let (changed, _) =
        online::settings_for_up(Some(held), "test", "ws://relay", &up, "cuda", Ok(())).unwrap();
    let lane = changed.text.unwrap();
    assert!(changed.decide.is_some() && lane.profile.is_none() && lane.serial.is_none());
}

#[test]
fn profile_pins_match_the_qualified_catalog_and_reject_replaced_files() {
    let catalog: serde_json::Value = serde_json::from_str(include_str!(
        "../../../scripts/bench/pylon-models/models.json"
    ))
    .unwrap();
    let row = catalog
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == Profile::MODEL)
        .unwrap();
    assert_eq!(row["sha256"], Profile::SHA256);
    assert_eq!(row["bytes"], Profile::BYTES);
    assert_eq!(row["file"], Profile::FILE);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("model.gguf");
    std::fs::write(&path, b"wrong").unwrap();
    assert!(
        Profile::DeepSeekR1DistillQwen15B
            .verify(&path)
            .unwrap_err()
            .contains("size")
    );
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(Profile::BYTES)
        .unwrap();
    assert!(
        Profile::DeepSeekR1DistillQwen15B
            .verify(&path)
            .unwrap_err()
            .contains("SHA-256")
    );
}

#[tokio::test]
async fn a_replaced_profile_artifact_pauses_before_any_worker_starts() {
    let dir = tempfile::tempdir().unwrap();
    let up = choice(dir.path());
    let (settings, _) =
        online::settings_for_up(None, "test", "ws://127.0.0.1:1", &up, "cuda", Ok(())).unwrap();
    share::save(dir.path(), &settings).unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let home = dir.path().to_path_buf();
    let task = tokio::spawn(online::supervise(
        home.clone(),
        std::sync::Arc::new(pylon::lease::Dedicated),
        online::Options::host(std::time::Duration::from_millis(20)),
        std::sync::Arc::new(|_| {}),
        async {
            let _ = stopped.await;
        },
    ));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        if let Some(live) = online::load_live(&home)
            && live.state == online::Phase::Paused
        {
            assert!(live.why.unwrap().contains("size"));
            assert!(live.lanes.is_empty());
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    stop.send(()).unwrap();
    task.await.unwrap();
}
