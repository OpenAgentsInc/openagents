//! The counter's rules: standalone and service starts interleave without a
//! lower or repeated generation, concurrent starts serialize, and a crash
//! between a reservation and its use skips the value.

use std::collections::BTreeSet;
use std::sync::{Arc, Barrier};

use super::*;

fn root() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("host");
    (temp, root)
}

/// A service start: the launcher reserves, then its host claims.
fn service_start(root: &Path) -> u64 {
    let generation = reserve(root, 0).unwrap();
    claim(root, generation).unwrap();
    generation
}

#[test]
fn standalone_and_service_starts_never_decrease() {
    let (_temp, root) = root();
    let mut seen = Vec::new();
    for turn in 0..12 {
        let generation = if turn % 3 == 1 {
            service_start(&root)
        } else {
            advance(&root).unwrap()
        };
        seen.push(generation);
    }
    assert!(
        seen.windows(2).all(|pair| pair[0] < pair[1]),
        "generations must strictly increase: {seen:?}"
    );
    let record = current(&root).unwrap().unwrap();
    assert_eq!(record.generation, *seen.last().unwrap());
    assert!(record.claimed);
}

#[test]
fn a_generation_follows_the_clock_floor_and_a_caller_floor() {
    let (_temp, root) = root();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(advance(&root).unwrap() > now);
    // An older launcher record carried a generation past the clock.
    let far = now + 1_000_000;
    assert_eq!(reserve(&root, far).unwrap(), far + 1);
    claim(&root, far + 1).unwrap();
    assert_eq!(advance(&root).unwrap(), far + 2);
}

#[test]
fn a_claim_refuses_a_lower_or_used_generation() {
    let (_temp, root) = root();
    let used = advance(&root).unwrap();
    // A standalone host already used this value.
    assert!(claim(&root, used).is_err());
    assert!(claim(&root, used - 1).is_err());
    // An explicit higher generation is admitted once and becomes the floor.
    claim(&root, used + 10).unwrap();
    assert!(claim(&root, used + 10).is_err());
    assert_eq!(advance(&root).unwrap(), used + 11);
}

#[test]
fn a_crash_between_reserve_and_use_skips_the_value() {
    let (_temp, root) = root();
    let lost = reserve(&root, 0).unwrap();
    // The launcher stopped before its host claimed `lost`; the restarted
    // launcher reserves again.
    let next = reserve(&root, 0).unwrap();
    assert!(next > lost);
    // A host that still carries the lost value cannot serve with it.
    assert!(claim(&root, lost).is_err());
    claim(&root, next).unwrap();
    // A standalone start after an unclaimed reservation also moves past it.
    let reserved = reserve(&root, 0).unwrap();
    assert!(advance(&root).unwrap() > reserved);
    assert!(claim(&root, reserved).is_err());
}

#[test]
fn a_crash_during_a_write_leaves_the_previous_record() {
    let (_temp, root) = root();
    let first = advance(&root).unwrap();
    // A write that stopped before its rename leaves a temporary file; the
    // record still reads as the last complete value.
    let stale = root.join(format!(".{FILE}.pending-{}", std::process::id()));
    std::fs::write(&stale, b"{\"partial\"").unwrap();
    assert_eq!(current(&root).unwrap().unwrap().generation, first);
    assert!(advance(&root).unwrap() > first);
    assert!(!stale.exists());
}

#[test]
fn concurrent_starts_serialize_to_distinct_values() {
    let (_temp, root) = root();
    let threads = 8;
    let barrier = Arc::new(Barrier::new(threads));
    let handles: Vec<_> = (0..threads)
        .map(|index| {
            let (root, barrier) = (root.clone(), barrier.clone());
            std::thread::spawn(move || {
                barrier.wait();
                (0..16)
                    .map(|_| {
                        if index % 2 == 0 {
                            advance(&root).unwrap()
                        } else {
                            reserve(&root, 0).unwrap()
                        }
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let mut all = BTreeSet::new();
    for handle in handles {
        let values = handle.join().unwrap();
        assert!(values.windows(2).all(|pair| pair[0] < pair[1]));
        for value in values {
            assert!(all.insert(value), "generation {value} was handed out twice");
        }
    }
    assert_eq!(
        current(&root).unwrap().unwrap().generation,
        *all.last().unwrap()
    );
}

#[test]
fn a_legacy_number_is_read_and_a_damaged_record_refuses() {
    let (_temp, root) = root();
    std::fs::create_dir_all(&root).unwrap();
    let legacy = 4_000_000_000_u64;
    std::fs::write(path(&root), format!("{legacy}\n")).unwrap();
    let record = current(&root).unwrap().unwrap();
    assert_eq!((record.generation, record.claimed), (legacy, true));
    assert_eq!(advance(&root).unwrap(), legacy + 1);

    std::fs::write(path(&root), b"not a generation").unwrap();
    assert!(advance(&root).is_err());
    assert!(reserve(&root, 0).is_err());
    assert_eq!(std::fs::read(path(&root)).unwrap(), b"not a generation");
}
