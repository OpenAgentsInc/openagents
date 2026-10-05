use super::*;
use crate::service::rewards::Transaction;
use std::{io::Write, process::Command, sync::Arc, time::Instant};

fn reward(g: &Gateway, n: u64) -> Transaction {
    let mut source = [5; 32];
    source[..8].copy_from_slice(&n.to_be_bytes());
    Transaction {
        acceptance: None,
        outfit: None,
        equipment: None,
        spent: vec![],
        instance: 120,
        actor: g.game().player_life().actor,
        source,
        experience: 1,
        items: vec![],
        quests: vec![],
    }
}

#[test]
fn unchanged_commit_flushes_staged_history_without_acknowledging_future_rewards() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("state");
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    let mut g = super::tests::prepared();
    store.commit(&mut g).unwrap();
    let copy = store.prepare(&mut g).unwrap();
    let first = reward(&g, 1);
    let receipt = g.grant_reward(first.clone()).unwrap();
    for n in 2..=140 {
        g.grant_reward(reward(&g, n)).unwrap();
    }
    assert_eq!(std::fs::read_dir(root.join("rewards")).unwrap().count(), 0);
    assert!(!store.commit_prepared(copy).unwrap().written);
    assert!(std::fs::read_dir(root.join("rewards")).unwrap().count() > 0);
    assert_eq!(g.grant_reward(first.clone()).unwrap(), receipt);
    drop(g);
    drop(store);
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    assert!(
        store
            .recover()
            .unwrap()
            .character_rewards(first.actor)
            .is_none()
    );
}

#[test]
fn prepared_world_matches_encoded_journal_state_including_signed_zero() {
    let mut gateway = super::tests::prepared();
    gateway.chamber.game.yaw = -0.0;
    gateway.chamber.game.message = "A quoted \"world\" with Unicode: λ".into();
    for _ in 0..4 {
        let copy = super::super::save::Prepared::capture(&gateway).unwrap();
        let (bytes, world) = copy.encode_with_world().unwrap();
        assert_eq!(bytes, copy.encode().unwrap());
        let expected = journal::expand(&bytes).unwrap();
        let mut reused: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        reused["world"] = world;
        assert_eq!(
            serde_json::to_vec(&reused).unwrap(),
            serde_json::to_vec(&expected).unwrap()
        );
        assert_eq!(
            journal::hash(&reused).unwrap(),
            journal::hash(&expected).unwrap()
        );
        gateway.tick(1. / 30.).unwrap();
    }
}

#[test]
fn persistence_copy_does_not_follow_later_authority_mutations() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("state");
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    let mut g = super::tests::prepared();
    store.commit(&mut g).unwrap();
    let first = reward(&g, 1);
    g.grant_reward(first.clone()).unwrap();
    let copy = store.prepare(&mut g).unwrap();
    g.tick(1. / 30.).unwrap();
    for n in 2..=140 {
        g.grant_reward(reward(&g, n)).unwrap();
    }
    store.commit_prepared(copy).unwrap();
    drop(g);
    drop(store);
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    let recovered = store.recover().unwrap();
    assert_eq!(recovered.game().authority_tick, 0);
    assert_eq!(
        recovered.character_rewards(first.actor).unwrap().experience,
        1
    );
}

#[test]
fn incremental_journal_compacts_and_recovers_a_growing_reward_history() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("state");
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    let mut g = super::tests::prepared();
    store.commit(&mut g).unwrap();
    let first = reward(&g, 1);
    let receipt = g.grant_reward(first.clone()).unwrap();
    store.commit(&mut g).unwrap();
    let base = std::fs::read(root.join("chamber.json")).unwrap();
    g.tick(1. / 30.).unwrap();
    let delta = store.commit(&mut g).unwrap();
    assert!(delta.bytes < base.len());
    assert_eq!(std::fs::read(root.join("chamber.json")).unwrap(), base);
    let mut timings = (Vec::new(), Vec::new(), Vec::new());
    for n in 2..=300 {
        g.grant_reward(reward(&g, n)).unwrap();
        let start = Instant::now();
        g.tick(1. / 30.).unwrap();
        timings.0.push(start.elapsed().as_secs_f64());
        let start = Instant::now();
        let copy = store.prepare(&mut g).unwrap();
        timings.1.push(start.elapsed().as_secs_f64());
        let start = Instant::now();
        store.commit_prepared(copy).unwrap();
        timings.2.push(start.elapsed().as_secs_f64());
    }
    let distribution = |mut samples: Vec<f64>| {
        samples.sort_by(f64::total_cmp);
        let p =
            |fraction: f64| samples[(samples.len() as f64 * fraction).ceil() as usize - 1] * 1000.;
        serde_json::json!({"observations":samples.len(),"p50_ms":p(0.50),"p95_ms":p(0.95),"p99_ms":p(0.99),"maximum_ms":samples.last().unwrap()*1000.})
    };
    eprintln!(
        "{}",
        serde_json::json!({"schema":"verse.durability.growing-state.v1","reward_transactions":300,
        "simulation":distribution(timings.0),"capture":distribution(timings.1),"commits":distribution(timings.2)})
    );
    let snapshot: Committed =
        serde_json::from_slice(&std::fs::read(root.join("chamber.json")).unwrap()).unwrap();
    assert_eq!(snapshot.revision, 257);
    assert!(store.records < journal::INTERVAL);
    assert!(root.join("journal.jsonl").metadata().unwrap().len() < journal::LOG_BYTES);
    let revision = store.revision;
    drop(g);
    drop(store);
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    assert_eq!(store.revision, revision);
    let mut recovered = store.recover().unwrap();
    assert_eq!(
        recovered.character_rewards(first.actor).unwrap().experience,
        300
    );
    assert_eq!(recovered.grant_reward(first).unwrap(), receipt);
}

#[test]
fn replay_discards_only_an_incomplete_tail_and_refuses_complete_corruption() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("state");
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    let mut g = super::tests::prepared();
    store.commit(&mut g).unwrap();
    let tx = reward(&g, 1);
    let receipt = g.grant_reward(tx.clone()).unwrap();
    store.commit(&mut g).unwrap();
    drop(g);
    drop(store);
    let path = root.join("journal.jsonl");
    let valid = std::fs::read(&path).unwrap();
    OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"{\"version\":1")
        .unwrap();
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    let mut g = store.recover().unwrap();
    assert_eq!(g.grant_reward(tx).unwrap(), receipt);
    assert_eq!(std::fs::read(&path).unwrap(), valid);
    drop(g);
    drop(store);
    let mut corrupt: serde_json::Value = serde_json::from_slice(&valid).unwrap();
    corrupt["revision"] = 99.into();
    let mut bytes = serde_json::to_vec(&corrupt).unwrap();
    bytes.push(b'\n');
    std::fs::write(&path, bytes).unwrap();
    assert!(matches!(Store::open(&root, [8; 32], 120), Err(error) if error.contains("digest")));
    std::fs::write(&path, b"{broken}\n").unwrap();
    assert!(Store::open(&root, [8; 32], 120).is_err());
    std::fs::write(&path, &valid).unwrap();
    // A complete duplicated revision cannot be silently discarded above the base snapshot.
    OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(&valid)
        .unwrap();
    assert!(Store::open(&root, [8; 32], 120).is_err());
}

#[test]
fn process_exit_at_commit_boundaries_preserves_acknowledged_rewards() {
    let stages = [
        "before_encode",
        "after_history",
        "after_journal",
        "before_snapshot_sync",
        "after_snapshot_sync",
        "after_snapshot_rename",
        "after_snapshot_directory",
        "before_journal_clear",
        "after_journal_clear",
    ];
    for stage in stages {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("state");
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "service::persistence::checks::crash_child",
                "--ignored",
                "--nocapture",
            ])
            .env("VERSE_TEST_STORAGE_ROOT", &root)
            .env("VERSE_TEST_STORAGE_BOUNDARY", stage)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(86),
            "{stage}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut store = Store::open(&root, [8; 32], 120).unwrap_or_else(|e| panic!("{stage}: {e}"));
        let mut g = store.recover().unwrap();
        let first = reward(&g, 1);
        let ack: crate::service::rewards::Receipt =
            serde_json::from_slice(&std::fs::read(dir.path().join("ack.json")).unwrap()).unwrap();
        assert_eq!(g.grant_reward(first.clone()).unwrap(), ack, "{stage}");
        let count = if matches!(stage, "before_encode" | "after_history") {
            1
        } else {
            140
        };
        assert_eq!(
            g.character_rewards(first.actor).unwrap().experience,
            count,
            "{stage}"
        );
        // The uncertain operation either exists whole or can be applied once with the same identity.
        let candidate = reward(&g, 2);
        let original = g.grant_reward(candidate.clone()).unwrap();
        assert_eq!(g.grant_reward(candidate).unwrap(), original, "{stage}");
    }
}

#[test]
#[ignore = "Launched by the commit-boundary recovery test with an isolated storage root"]
fn crash_child() {
    let root = PathBuf::from(std::env::var_os("VERSE_TEST_STORAGE_ROOT").unwrap());
    let stage = std::env::var("VERSE_TEST_STORAGE_BOUNDARY").unwrap();
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    let mut g = super::tests::prepared();
    store.commit(&mut g).unwrap();
    let first = reward(&g, 1);
    let ack = g.grant_reward(first).unwrap();
    store.commit(&mut g).unwrap();
    std::fs::write(
        root.parent().unwrap().join("ack.json"),
        serde_json::to_vec(&ack).unwrap(),
    )
    .unwrap();
    for _ in 0..254 {
        g.tick(1. / 30.).unwrap();
        store.commit(&mut g).unwrap();
    }
    assert_eq!(store.records, 255);
    for n in 2..=140 {
        g.grant_reward(reward(&g, n)).unwrap();
    }
    store.inject(Arc::new(move |at| {
        if at == stage {
            std::process::exit(86);
        }
    }));
    store.commit(&mut g).unwrap();
    panic!("Boundary was not reached");
}
