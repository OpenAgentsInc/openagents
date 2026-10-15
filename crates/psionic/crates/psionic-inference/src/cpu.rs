//! CPU workers preserve the caller's allowed CPU mask.
use crate::{Error, Result};

pub(crate) fn pool(threads: usize) -> Result<rayon::ThreadPool> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .map_err(|e| Error(format!("Cannot start model workers: {e}")))
}

pub(crate) fn affinity() -> Option<Vec<usize>> {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: the kernel writes only the initialized mask; every queried bit is inside it.
        unsafe {
            let mut mask: libc::cpu_set_t = std::mem::zeroed();
            if libc::sched_getaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &mut mask) != 0 {
                return None;
            }
            Some(
                (0..libc::CPU_SETSIZE as usize)
                    .filter(|cpu| libc::CPU_ISSET(*cpu, &mask))
                    .collect(),
            )
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workers_preserve_the_callers_allowed_cpu_mask() {
        let caller = affinity();
        std::thread::spawn(|| {
            #[cfg(target_os = "linux")]
            if let Some(cpus) = affinity() {
                // SAFETY: narrow only this test thread, using IDs from its current mask.
                unsafe {
                    let mut mask: libc::cpu_set_t = std::mem::zeroed();
                    for cpu in cpus.into_iter().take(4) {
                        libc::CPU_SET(cpu, &mut mask);
                    }
                    assert_eq!(
                        libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &mask),
                        0
                    );
                }
            }
            let before = affinity();
            let count = before.as_ref().map_or(2, |cpus| cpus.len().min(4));
            let workers = pool(count).unwrap();
            workers.install(|| assert_eq!(rayon::current_num_threads(), count));
            assert_eq!(
                workers.broadcast(|_| affinity()),
                vec![before.clone(); count]
            );
            assert_eq!(affinity(), before);
        })
        .join()
        .unwrap();
        assert_eq!(affinity(), caller);
    }
}
