//! A snapshot of a wide tree fits a GUI-launched process's open-file
//! limit (#10078).
//!
//! The desktop app's host is launched by launchd, whose soft
//! `RLIMIT_NOFILE` is 256. The walk once held a descriptor for every
//! directory it had found but not yet listed, so a checkout with a few
//! hundred sibling directories (`bench/terminal-bench/traces`) ran out
//! of descriptors, every `EMFILE` became a fault, and the incomplete
//! snapshot refused every Coder start with "the granted source snapshot
//! is unavailable or changed". This test lowers the limit for its own
//! process — it is its own test binary for that reason — and observes a
//! tree wider than the limit.

#![cfg(unix)]

use coder_boundary::snapshot::Snapshot;

#[test]
fn a_tree_wider_than_the_open_file_limit_is_observed_whole() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    // Wider than the limit at one level, and nested below it, the shape
    // of a checkout's trace directories.
    for i in 0..600 {
        let sibling = root.join("traces").join(format!("run-{i}"));
        std::fs::create_dir_all(sibling.join("logs")).unwrap();
        std::fs::write(sibling.join("logs").join("out.txt"), i.to_string()).unwrap();
    }

    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    assert_eq!(
        unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) },
        0
    );
    let lowered = libc::rlimit {
        rlim_cur: 256,
        rlim_max: limit.rlim_max,
    };
    assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &lowered) }, 0);
    let snapshot = Snapshot::observe(&root);
    unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limit) };

    assert!(
        snapshot.is_complete(),
        "faults: {:?}",
        snapshot.faults().iter().take(3).collect::<Vec<_>>()
    );
    // The root, `traces`, and per run its directory, `logs`, and file.
    assert_eq!(snapshot.len(), 2 + 600 * 3);
}
