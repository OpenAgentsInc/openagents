//! CPU workers keep their own slots when the caller supplies exactly one CPU per worker.
use crate::{Error, Result};

pub(crate) fn pool(threads: usize) -> Result<rayon::ThreadPool> {
    let cpus = affinity().filter(|cpus| threads > 1 && cpus.len() == threads);
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .start_handler(move |index| {
            if let Some(cpus) = &cpus {
                #[cfg(target_os = "linux")]
                {
                    // SAFETY: IDs came from a CPU_SETSIZE-sized mask. PID zero affects only this worker.
                    unsafe {
                        let mut mask: libc::cpu_set_t = std::mem::zeroed();
                        libc::CPU_SET(cpus[index], &mut mask);
                        libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &mask);
                    }
                }
                #[cfg(not(target_os = "linux"))]
                let _ = (cpus, index);
            }
        })
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
    fn workers_do_not_change_the_callers_affinity() {
        let before = affinity();
        let workers = pool(2).unwrap();
        workers.install(|| assert_eq!(rayon::current_num_threads(), 2));
        assert_eq!(affinity(), before);
    }
}
