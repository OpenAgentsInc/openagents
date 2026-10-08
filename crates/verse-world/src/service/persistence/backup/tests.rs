use super::*;
use crate::service::auth::Gateway;
use crate::service::rewards::Transaction;
fn transaction(gateway: &Gateway, n: u64) -> Transaction {
    let mut source = [5; 32];
    source[..8].copy_from_slice(&n.to_be_bytes());
    Transaction {
        acceptance: None,
        outfit: None,
        equipment: None,
        spent: vec![],
        instance: 120,
        actor: gateway.game().player_life().actor,
        source,
        experience: 1,
        items: vec![],
        quests: vec![],
    }
}
#[test]
fn backup_restores_acknowledged_characters_and_archived_retry_receipts() {
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = dir.path().join("state");
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    let mut gateway = super::super::tests::prepared();
    store.commit(&mut gateway).unwrap();
    let first = transaction(&gateway, 1);
    let receipt = gateway.grant_reward(first.clone()).unwrap();
    for n in 2..=300 {
        gateway.grant_reward(transaction(&gateway, n)).unwrap();
    }
    gateway.tick(1. / 30.).unwrap();
    store.commit(&mut gateway).unwrap();
    let expected = gateway.character_rewards(first.actor).unwrap().clone();
    let backup = dir.path().join("backup");
    let restore_path = dir.path().join("restored");
    let report = store.export_backup(&backup, Budget::default()).unwrap();
    assert!(report.files > 1);
    assert!(Store::open(&backup, [8; 32], 120).is_err());
    assert_eq!(
        restore(&backup, &restore_path, [8; 32], 120, Budget::default()).unwrap(),
        report
    );
    let mut restored = Store::open(&restore_path, [8; 32], 120).unwrap();
    let mut recovered = restored.recover().unwrap();
    assert_eq!(recovered.character_rewards(first.actor), Some(&expected));
    assert_eq!(
        recovered.game().authority_tick,
        gateway.game().authority_tick
    );
    assert_eq!(recovered.grant_reward(first).unwrap(), receipt);
    assert_eq!(restored.revision, report.revision);
    assert!(restore(&backup, &root, [8; 32], 120, Budget::default()).is_err());
    assert!(restore(&backup, &restore_path, [8; 32], 120, Budget::default()).is_err());
    assert!(Store::open(&restore_path, [8; 32], 120).is_err());
    eprintln!(
        "{}",
        serde_json::json!({"schema":"verse.operations.recovery.v1", "report":report, "reward_transactions":300, "exact_retry_receipt":true, "exclusive_writer":true})
    );
}
#[test]
fn corruption_scope_budget_and_incomplete_publication_refuse_without_overwrite() {
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let mut store = Store::open(&dir.path().join("state"), [8; 32], 120).unwrap();
    let mut gateway = super::super::tests::prepared();
    store.commit(&mut gateway).unwrap();
    let backup = dir.path().join("backup");
    store.export_backup(&backup, Budget::default()).unwrap();
    assert!(verify(&backup, [7; 32], 120, Budget::default()).is_err());
    assert!(verify(&backup, [8; 32], 121, Budget::default()).is_err());
    assert!(verify(&backup, [8; 32], 120, Budget { files: 1, bytes: 1 }).is_err());
    let saved = std::fs::read(backup.join("chamber.json")).unwrap();
    std::fs::write(backup.join("chamber.json"), b"corruption").unwrap();
    let target = dir.path().join("refused");
    assert!(restore(&backup, &target, [8; 32], 120, Budget::default()).is_err());
    assert!(!target.exists());
    std::fs::write(backup.join("chamber.json"), saved).unwrap();
    new_dir(&backup.join("migrations")).unwrap();
    new_dir(&backup.join("migrations").join("a".repeat(64))).unwrap();
    assert!(verify(&backup, [8; 32], 120, Budget::default()).is_err());
    std::fs::remove_dir_all(backup.join("migrations")).unwrap();
    write(&backup.join("restore.pending"), b"pending").unwrap();
    assert!(verify(&backup, [8; 32], 120, Budget::default()).is_err());
    std::fs::remove_file(backup.join("restore.pending")).unwrap();
    write(&backup.join("backup.pending"), b"pending").unwrap();
    assert!(verify(&backup, [8; 32], 120, Budget::default()).is_err());
    let unfinished = dir.path().join("unfinished");
    new_dir(&unfinished).unwrap();
    write(&unfinished.join("restore.pending"), b"pending").unwrap();
    assert!(Store::open(&unfinished, [8; 32], 120).is_err());
}
#[cfg(unix)]
#[test]
fn symlinked_history_and_parent_paths_are_refused() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = dir.path().join("state");
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    let mut gateway = super::super::tests::prepared();
    store.commit(&mut gateway).unwrap();
    let backup = dir.path().join("backup");
    store.export_backup(&backup, Budget::default()).unwrap();
    symlink(&backup, dir.path().join("link")).unwrap();
    assert!(verify(&dir.path().join("link"), [8; 32], 120, Budget::default()).is_err());
    symlink(
        backup.join("chamber.json"),
        backup.join("rewards").join("0".repeat(64)),
    )
    .unwrap();
    assert!(verify(&backup, [8; 32], 120, Budget::default()).is_err());
    symlink(&root, dir.path().join("source-link")).unwrap();
    assert!(Store::open(&dir.path().join("source-link"), [8; 32], 120).is_err());
}

#[test]
fn offline_retention_keeps_current_retry_roots_and_removes_only_unreferenced_files() {
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = dir.path().join("state");
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    let mut gateway = super::super::tests::prepared();
    store.commit(&mut gateway).unwrap();
    let first = transaction(&gateway, 1);
    let receipt = gateway.grant_reward(first.clone()).unwrap();
    for n in 2..=300 {
        gateway.grant_reward(transaction(&gateway, n)).unwrap();
        if n % 100 == 0 {
            store.commit(&mut gateway).unwrap();
        }
    }
    assert!(store.prune_history(Budget::default()).is_err());
    drop(gateway);
    drop(store);
    let store = Store::open(&root, [8; 32], 120).unwrap();
    write(
        &root
            .join("rewards")
            .join(format!("next-{}", "0".repeat(32))),
        b"incomplete publication",
    )
    .unwrap();
    let report = store.prune_history(Budget::default()).unwrap();
    assert!(report.removed_files > 1);
    let backup = dir.path().join("backup");
    store.export_backup(&backup, Budget::default()).unwrap();
    drop(store);
    let mut store = Store::open(&root, [8; 32], 120).unwrap();
    assert_eq!(
        store.recover().unwrap().grant_reward(first).unwrap(),
        receipt
    );
}

#[test]
fn interrupted_restore_refuses_partial_state_and_keeps_the_verified_source() {
    let actor = super::super::tests::prepared().game().player_life().actor;
    for stage in [
        "reserved",
        "copied",
        "durable",
        "before_publish",
        "published",
    ] {
        let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "service::persistence::backup::tests::restore_crash_child",
                "--ignored",
                "--nocapture",
            ])
            .env("VERSE_TEST_RESTORE_ROOT", dir.path())
            .env("VERSE_TEST_RESTORE_BOUNDARY", stage)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(86),
            "{stage}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let backup = dir.path().join("backup");
        let restored = dir.path().join("restored");
        verify(&backup, [8; 32], 120, Budget::default()).unwrap();
        if stage == "published" {
            let mut store = Store::open(&restored, [8; 32], 120).unwrap();
            assert_eq!(
                store
                    .recover()
                    .unwrap()
                    .character_rewards(actor)
                    .unwrap()
                    .experience,
                140
            );
        } else {
            assert!(Store::open(&restored, [8; 32], 120).is_err(), "{stage}");
        }
        let retry = dir.path().join("retry");
        restore(&backup, &retry, [8; 32], 120, Budget::default()).unwrap();
        let mut store = Store::open(&retry, [8; 32], 120).unwrap();
        assert_eq!(
            store
                .recover()
                .unwrap()
                .character_rewards(actor)
                .unwrap()
                .experience,
            140
        );
        eprintln!(
            "{}",
            serde_json::json!({"schema":"verse.operations.restore.crash.v1", "boundary":stage, "partial_host_admission_refused":stage != "published", "verified_source_retained":true, "retry_reward_transactions":140})
        );
    }
}
#[test]
#[ignore = "Launched with an isolated root by the restore-boundary recovery test"]
fn restore_crash_child() {
    let dir = std::path::PathBuf::from(std::env::var_os("VERSE_TEST_RESTORE_ROOT").unwrap());
    let stage = std::env::var("VERSE_TEST_RESTORE_BOUNDARY").unwrap();
    let mut store = Store::open(&dir.join("state"), [8; 32], 120).unwrap();
    let mut gateway = super::super::tests::prepared();
    store.commit(&mut gateway).unwrap();
    for n in 1..=140 {
        gateway.grant_reward(transaction(&gateway, n)).unwrap();
    }
    store.commit(&mut gateway).unwrap();
    let backup = dir.join("backup");
    let target = dir.join("restored");
    store.export_backup(&backup, Budget::default()).unwrap();
    restore_observed(&backup, &target, [8; 32], 120, Budget::default(), |at| {
        // Even the fully published destination is excluded until this restore drops its lock.
        let refused = Store::open(&target, [8; 32], 120).err().unwrap();
        assert!(refused.contains("writer"));
        if at == stage {
            std::process::exit(86);
        }
    })
    .unwrap();
    panic!("Restore boundary was not reached");
}
