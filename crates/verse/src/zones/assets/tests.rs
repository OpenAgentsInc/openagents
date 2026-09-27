use super::*;

fn pack() -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/verse/forest/7c1535256a4687e70a0f624f4b97c651bfd0b36ba91347a09041698ef8d246a7.vzp"),
    )
    .expect("retained forest pack")
}

fn cache_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "verse-forest-test-{}-{}",
        std::process::id(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn original_models_decode_with_distinct_animated_poses_and_leaf_geometry() {
    let assets = LoadedAssets::decode(&pack()).expect("original forest geometry");
    assert_eq!(assets.tree.faces.len(), 26_715);
    assert_eq!(assets.wizard_still.faces.len(), 6_753);
    assert_eq!(assets.wizard.frames.len(), 12);
    assert_eq!(assets.zombie.frames.len(), 12);
    assert_eq!(assets.zombie_walk.frames.len(), 12);
    assert_eq!(assets.zombie.frames[0].faces.len(), 13_644);
    assert_ne!(assets.wizard.frames[0].faces, assets.wizard.frames[6].faces);
    assert_ne!(assets.zombie.frames[0].faces, assets.zombie.frames[6].faces);
    assert_ne!(
        assets.zombie_walk.frames[0].faces,
        assets.zombie_walk.frames[6].faces
    );
    let highest = assets
        .tree
        .faces
        .iter()
        .map(|v| v.pos[1])
        .fold(0.0f32, f32::max);
    assert!((highest - 8.0).abs() < 0.001);
    assert!(assets.tree.faces.iter().any(|v| v.color[1] > v.color[0]));
    assert_eq!(
        assets.wizard.sample(f32::NAN).faces,
        assets.wizard.frames[0].faces
    );
}

#[test]
fn altered_hash_truncation_and_growth_are_refused() {
    let mut bytes = pack();
    bytes[100] ^= 1;
    assert!(
        LoadedAssets::decode(&bytes)
            .unwrap_err()
            .contains("content check")
    );
    bytes.pop();
    assert!(LoadedAssets::decode(&bytes).unwrap_err().contains("size"));
    assert!(LoadedAssets::decode_structure(&bytes[..10]).is_err());
    let mut bytes = pack();
    bytes.push(0);
    assert!(
        LoadedAssets::decode_structure(&bytes)
            .unwrap_err()
            .contains("trailing")
    );
}

#[test]
fn malformed_geometry_and_allocation_bounds_are_refused_before_allocation() {
    let mut bytes = pack();
    bytes[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(
        LoadedAssets::decode_structure(&bytes)
            .unwrap_err()
            .contains("vertex count")
    );
    bytes[8..12].copy_from_slice(&3u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(
        LoadedAssets::decode_structure(&bytes)
            .unwrap_err()
            .contains("position")
    );
    let count = 3u32.to_le_bytes();
    let mut reader = PackReader {
        bytes: &count,
        offset: 0,
        decoded_bytes: MAX_DECODED_BYTES,
    };
    assert!(reader.mesh().unwrap_err().contains("memory limit"));
    let mut reader = PackReader {
        bytes: &[0; 6],
        offset: 0,
        decoded_bytes: 0,
    };
    assert!(reader.animation().unwrap_err().contains("timing"));
}

#[test]
fn construction_is_inert_and_cache_install_is_atomic_and_verified() {
    let cache = cache_dir();
    let loader = Loader::new(cache.clone());
    assert!(!cache.exists());
    assert!(loader.worker.is_none());
    let cancel = AtomicBool::new(false);
    let bytes = pack();
    install_cache(&cache, &bytes, &cancel).expect("install verified content");
    let path = cache.join(format!("{PACK_SHA256}.vzp"));
    assert_eq!(read_bounded(&path).unwrap(), bytes);
    assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 1);
    let mut bad = bytes;
    bad[100] ^= 1;
    assert!(install_cache(&cache, &bad, &cancel).is_err());
    assert!(LoadedAssets::load_local(&path).is_ok());
    std::fs::remove_dir_all(cache).unwrap();
}

#[test]
fn cached_entry_uses_worker_and_duplicate_requests_are_refused() {
    let cache = cache_dir();
    install_cache(&cache, &pack(), &AtomicBool::new(false)).unwrap();
    let mut loader = Loader::new(cache.clone());
    assert!(loader.request());
    assert!(!loader.request());
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        match loader.poll() {
            Some(LoadEvent::Ready(assets)) => {
                assert_eq!(assets.tree.faces.len(), 26_715);
                break;
            }
            Some(LoadEvent::Failed(error)) => panic!("cache read failed: {error}"),
            _ => {
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
    assert!(loader.worker.is_none());
    std::fs::remove_dir_all(cache).unwrap();
}

#[test]
fn canceled_generation_discards_late_results_without_overlapping_workers() {
    let mut loader = Loader::new(cache_dir());
    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, events) = mpsc::channel();
    let (finish_tx, finish_rx) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        finish_rx.recv().unwrap();
        tx.send(LoadEvent::Failed("late generation".into()))
            .unwrap();
    });
    loader.worker = Some(Worker {
        cancel: cancel.clone(),
        events,
        handle,
    });
    loader.cancel();
    assert!(cancel.load(Ordering::Acquire));
    assert!(!loader.request());
    assert!(loader.poll().is_none());
    finish_tx.send(()).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while loader.worker.is_some() {
        assert!(loader.poll().is_none());
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn canceled_cache_install_does_not_publish_an_entry() {
    let cache = cache_dir();
    assert!(install_cache(&cache, &pack(), &AtomicBool::new(true)).is_err());
    assert!(!cache.exists());
}

#[cfg(unix)]
#[test]
fn cache_symlinks_are_refused() {
    let cache = cache_dir();
    std::fs::create_dir_all(&cache).unwrap();
    let target = cache.join("target");
    let alias = cache.join("alias");
    std::fs::write(&target, pack()).unwrap();
    std::os::unix::fs::symlink(target, &alias).unwrap();
    assert!(read_bounded(&alias).is_err());
    std::fs::remove_dir_all(cache).unwrap();
}

#[cfg(unix)]
#[test]
fn pruning_keeps_unknown_files_links_active_temps_and_the_current_pack() {
    let cache = cache_dir();
    install_cache(&cache, &pack(), &AtomicBool::new(false)).unwrap();
    let old = "a".repeat(64);
    let unknown = "b".repeat(64);
    let linked = "c".repeat(64);
    let old_path = cache.join(format!("{old}.vzp"));
    let unknown_path = cache.join(format!("{unknown}.vzp"));
    let link_path = cache.join(format!("{linked}.vzp"));
    let stale = cache.join(".forest-12-45.part");
    let recent = cache.join(".forest-12-46.part");
    let strange = cache.join(".forest-unknown.part");
    for path in [&old_path, &unknown_path, &stale, &recent, &strange] {
        std::fs::write(path, b"retained test data").unwrap();
    }
    std::fs::File::options()
        .write(true)
        .open(&stale)
        .unwrap()
        .set_times(
            std::fs::FileTimes::new()
                .set_modified(std::time::SystemTime::now() - Duration::from_secs(25 * 3600)),
        )
        .unwrap();
    std::os::unix::fs::symlink(&unknown_path, &link_path).unwrap();
    prune_cache(&cache, &[PACK_SHA256, &old, &linked]);
    assert!(!old_path.exists() && !stale.exists());
    assert!(unknown_path.exists() && recent.exists() && strange.exists());
    assert!(std::fs::symlink_metadata(link_path).unwrap().is_symlink());
    assert!(LoadedAssets::load_local(&cache.join(format!("{PACK_SHA256}.vzp"))).is_ok());
    std::fs::remove_dir_all(cache).unwrap();
}

#[cfg(unix)]
#[test]
fn cache_directory_symlink_is_not_followed_by_loader_or_pruner() {
    let actual = cache_dir();
    let alias = cache_dir();
    std::fs::create_dir_all(&actual).unwrap();
    let old = "a".repeat(64);
    let old_path = actual.join(format!("{old}.vzp"));
    std::fs::write(&old_path, b"preserve").unwrap();
    std::os::unix::fs::symlink(&actual, &alias).unwrap();
    prune_cache(&alias, &[&old]);
    assert!(old_path.exists());
    assert!(install_cache(&alias, &pack(), &AtomicBool::new(false)).is_err());
    let (tx, _) = mpsc::channel();
    assert!(
        load(&alias, &AtomicBool::new(false), &tx)
            .unwrap_err()
            .contains("directory")
    );
    std::fs::remove_file(alias).unwrap();
    std::fs::remove_dir_all(actual).unwrap();
}
