//! Coarse resource telemetry for presence.
//!
//! A host reports its logical CPU count, recent CPU use, and the share of
//! memory available to new work, so NIP-REACH placement can rank it. Each
//! value is a whole number, which limits what a sample reveals. A host that
//! cannot read a value withholds the whole sample rather than guess, and
//! placement then skips it.
//!
//! CPU use is the one-minute load average divided by the CPU count, capped
//! at 100 percent. Available memory is `MemAvailable` over `MemTotal` on
//! Linux and the kernel's memory status level on macOS.

use coder_reach::presence::{MAX_CPU_COUNT, Telemetry};

/// Take one sample, or `None` when this platform cannot supply every value.
#[must_use]
pub fn sample() -> Option<Telemetry> {
    let cpus = u32::try_from(std::thread::available_parallelism().ok()?.get())
        .ok()?
        .clamp(1, MAX_CPU_COUNT);
    Some(Telemetry {
        cpu_count: cpus,
        cpu_utilization_pct: utilization(load_average()?, cpus),
        memory_available_pct: memory_available_pct()?,
    })
}

/// Load per CPU as a whole percentage, capped at 100.
fn utilization(load: f64, cpus: u32) -> u8 {
    if !load.is_finite() || load <= 0.0 {
        return 0;
    }
    let percent = (load / f64::from(cpus) * 100.0).round();
    if percent >= 100.0 {
        100
    } else {
        // In range 0..100 after the checks above.
        percent as u8
    }
}

/// The one-minute load average.
fn load_average() -> Option<f64> {
    let mut loads = [0.0_f64; 3];
    // SAFETY: `loads` has room for the three samples requested.
    let read = unsafe { libc::getloadavg(loads.as_mut_ptr(), 3) };
    (read >= 1).then_some(loads[0])
}

#[cfg(target_os = "linux")]
fn memory_available_pct() -> Option<u8> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let field = |name: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(name))
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|value| value.parse::<u64>().ok())
    };
    let total = field("MemTotal:")?;
    let available = field("MemAvailable:")?;
    percent_of(available, total)
}

#[cfg(target_os = "macos")]
fn memory_available_pct() -> Option<u8> {
    // `kern.memorystatus_level` is the percentage of memory the kernel
    // considers available, the value memory pressure reports.
    let mut level: libc::c_int = 0;
    let mut size = std::mem::size_of::<libc::c_int>();
    // SAFETY: the name is NUL-terminated, `level` and `size` describe one
    // writable `c_int`, and no new value is set.
    let status = unsafe {
        libc::sysctlbyname(
            c"kern.memorystatus_level".as_ptr(),
            (&raw mut level).cast(),
            &raw mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if status != 0 || size != std::mem::size_of::<libc::c_int>() {
        return None;
    }
    u8::try_from(level).ok().filter(|level| *level <= 100)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn memory_available_pct() -> Option<u8> {
    None
}

#[cfg(any(target_os = "linux", test))]
fn percent_of(part: u64, whole: u64) -> Option<u8> {
    if whole == 0 || part > whole {
        return None;
    }
    u8::try_from(part.saturating_mul(100) / whole).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utilization_is_load_per_cpu_capped() {
        assert_eq!(utilization(0.0, 8), 0);
        assert_eq!(utilization(-1.0, 8), 0);
        assert_eq!(utilization(f64::NAN, 8), 0);
        assert_eq!(utilization(2.0, 8), 25);
        assert_eq!(utilization(7.96, 8), 100);
        assert_eq!(utilization(40.0, 8), 100);
    }

    #[test]
    fn percentages_refuse_impossible_inputs() {
        assert_eq!(percent_of(50, 200), Some(25));
        assert_eq!(percent_of(0, 0), None);
        assert_eq!(percent_of(3, 2), None);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn this_machine_supplies_a_valid_sample() {
        let sample = sample().expect("telemetry on a supported platform");
        assert!((1..=MAX_CPU_COUNT).contains(&sample.cpu_count));
        assert!(sample.cpu_utilization_pct <= 100);
        assert!(sample.memory_available_pct <= 100);
    }
}
