//! The memory cap on one run (#12450): the runner runs one job at a time, so
//! every process below it belongs to that job. A watcher sums their resident
//! memory twice a second with `ps` and kills them all past the cap.

use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// A running watcher.
pub(crate) struct Watch {
    done: Arc<AtomicBool>,
    over: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

/// `(pid, parent, resident KiB)` for every process `ps` lists.
fn processes() -> Vec<(u32, u32, u64)> {
    let Ok(out) = Command::new("ps")
        .args(["-A", "-o", "pid=", "-o", "ppid=", "-o", "rss="])
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let mut w = line.split_whitespace();
            Some((
                w.next()?.parse().ok()?,
                w.next()?.parse().ok()?,
                w.next()?.parse().ok()?,
            ))
        })
        .collect()
}

/// The processes below `root`, and their resident memory in KiB.
pub(crate) fn descendants(table: &[(u32, u32, u64)], root: u32) -> (Vec<u32>, u64) {
    let mut found = vec![root];
    let mut i = 0;
    while i < found.len() {
        let parent = found[i];
        for (pid, ppid, _) in table {
            if *ppid == parent && !found.contains(pid) {
                found.push(*pid);
            }
        }
        i += 1;
    }
    found.remove(0);
    let kib = table
        .iter()
        .filter(|(pid, _, _)| found.contains(pid))
        .map(|(_, _, rss)| rss)
        .sum();
    (found, kib)
}

impl Watch {
    /// Watch this runner's child processes against `cap_mb`.
    pub(crate) fn start(cap_mb: u64) -> Self {
        let done = Arc::new(AtomicBool::new(false));
        let over = Arc::new(AtomicBool::new(false));
        let thread = {
            let (done, over) = (done.clone(), over.clone());
            std::thread::spawn(move || {
                let me = std::process::id();
                while !done.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(500));
                    let (pids, kib) = descendants(&processes(), me);
                    if kib / 1024 > cap_mb {
                        over.store(true, Ordering::SeqCst);
                        for pid in pids {
                            // SAFETY: each pid is a live child of this
                            // runner, listed a moment ago.
                            unsafe {
                                libc::kill(pid as i32, libc::SIGKILL);
                            }
                        }
                    }
                }
            })
        };
        Self {
            done,
            over,
            thread: Some(thread),
        }
    }

    /// Stop watching; whether the run passed its cap.
    pub(crate) fn stop(mut self) -> bool {
        self.done.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        self.over.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tree_sums_below_its_root_only() {
        let table = [
            (1, 0, 10),
            (5, 1, 100),
            (6, 5, 200),
            (7, 6, 300),
            (9, 1, 50),
        ];
        let (pids, kib) = descendants(&table, 5);
        assert_eq!(pids, vec![6, 7]);
        assert_eq!(kib, 500);
    }

    #[test]
    fn this_process_is_listed() {
        let table = processes();
        assert!(
            table.iter().any(|(pid, _, _)| *pid == std::process::id()),
            "ps lists this process"
        );
    }
}
