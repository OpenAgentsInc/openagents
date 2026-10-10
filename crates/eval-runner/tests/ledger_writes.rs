//! The ledger file under concurrent writers: every write lands whole
//! through its own temporary file, and snapshots land in the order they
//! were taken, so `service.json` never goes back in time (audit GY-02).

use std::sync::{Arc, Barrier};

use eval_runner::store::LedgerWriter;

#[test]
fn concurrent_atomic_writes_never_tear_or_collide() {
    let dir = tempfile::tempdir().unwrap();
    let path = Arc::new(dir.path().join("service.json"));
    let writers = 16;
    let barrier = Arc::new(Barrier::new(writers));
    let handles: Vec<_> = (0..writers)
        .map(|writer| {
            let (path, barrier) = (Arc::clone(&path), Arc::clone(&barrier));
            std::thread::spawn(move || {
                // Large enough that a shared, truncated temporary file
                // would interleave.
                let body = format!("{writer:02}").repeat(64 * 1024);
                barrier.wait();
                for _ in 0..20 {
                    eval_runner::write_atomic(&path, body.as_bytes()).unwrap();
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
    let bytes = std::fs::read(&*path).unwrap();
    assert_eq!(bytes.len(), 2 * 64 * 1024);
    let first = &bytes[..2];
    assert!(bytes.chunks(2).all(|chunk| chunk == first), "a torn write");
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .filter(|name| name != "service.json")
        .collect();
    assert!(leftovers.is_empty(), "temporary files left: {leftovers:?}");
}

#[test]
fn ledger_writer_skips_snapshots_older_than_the_one_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("service.json");
    let writer = LedgerWriter::default();
    let write = |generation: u64| {
        writer
            .persist_with(generation, || {
                eval_runner::write_atomic(&path, generation.to_string().as_bytes())
            })
            .unwrap();
    };
    write(2);
    // An older snapshot whose writer was slower arrives last.
    write(1);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "2");
    write(3);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "3");
}

#[test]
fn concurrent_ledger_writers_leave_the_newest_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let path = Arc::new(dir.path().join("service.json"));
    let writer = Arc::new(LedgerWriter::default());
    let generations = 64_u64;
    let barrier = Arc::new(Barrier::new(usize::try_from(generations).unwrap()));
    // Spawned newest first, so older snapshots tend to arrive late.
    let handles: Vec<_> = (1..=generations)
        .rev()
        .map(|generation| {
            let (path, writer, barrier) =
                (Arc::clone(&path), Arc::clone(&writer), Arc::clone(&barrier));
            std::thread::spawn(move || {
                barrier.wait();
                writer
                    .persist_with(generation, || {
                        eval_runner::write_atomic(&path, generation.to_string().as_bytes())
                    })
                    .unwrap();
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(&*path).unwrap(),
        generations.to_string()
    );
}

#[test]
fn a_failed_write_does_not_advance_the_ledger() {
    let writer = LedgerWriter::default();
    let failed = writer.persist_with(1, || Err(std::io::Error::other("disk full")));
    assert!(failed.is_err());
    let mut wrote = false;
    writer
        .persist_with(1, || {
            wrote = true;
            Ok(())
        })
        .unwrap();
    assert!(wrote, "the same generation is retried after a failure");
}
