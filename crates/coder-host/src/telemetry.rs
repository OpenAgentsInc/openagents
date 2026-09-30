//! Coarse resource telemetry for presence.
//!
//! A host reports its logical CPU count, recent CPU use, and the share of
//! memory available to new work, so NIP-REACH placement can rank it. Each
//! value is a whole number, which limits what a sample reveals. A host that
//! cannot read a value withholds the whole sample rather than guess, and
//! placement then skips it.
//!
//! CPU use is the one-minute load average divided by the CPU count, capped
//! at 100 percent. Windows has no load average, so there it is the busy
//! share of CPU time since the previous sample (`GetSystemTimes`), and the
//! first sample is withheld. Available memory is `MemAvailable` over
//! `MemTotal` on Linux, the kernel's memory status level on macOS, and the
//! available share of physical memory on Windows (`GlobalMemoryStatusEx`).

use coder_reach::presence::{MAX_CPU_COUNT, Telemetry};

/// Take one sample, or `None` when this platform cannot supply every value.
#[must_use]
pub fn sample() -> Option<Telemetry> {
    let cpus = u32::try_from(std::thread::available_parallelism().ok()?.get())
        .ok()?
        .clamp(1, MAX_CPU_COUNT);
    Some(Telemetry {
        cpu_count: cpus,
        cpu_utilization_pct: cpu_use(cpus)?,
        memory_available_pct: memory_available_pct()?,
    })
}

/// Load per CPU as a whole percentage, capped at 100.
#[cfg(any(unix, test))]
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

/// Recent CPU use, as a whole percentage.
#[cfg(unix)]
fn cpu_use(cpus: u32) -> Option<u8> {
    Some(utilization(load_average()?, cpus))
}

/// The busy share of all CPUs' time since the previous sample, as a whole
/// percentage; `None` for the first sample, which has nothing to compare.
#[cfg(windows)]
fn cpu_use(_cpus: u32) -> Option<u8> {
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::GetSystemTimes;
    static PREVIOUS: Mutex<Option<CpuTimes>> = Mutex::new(None);
    let (mut idle, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    // SAFETY: three writable FILETIMEs.
    if unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } == 0 {
        return None;
    }
    let ticks =
        |time: FILETIME| (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
    let now = CpuTimes {
        idle: ticks(idle),
        kernel: ticks(kernel),
        user: ticks(user),
    };
    let previous = PREVIOUS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .replace(now);
    busy_pct(previous?, now)
}

/// Cumulative system CPU times, in 100-nanosecond ticks. Kernel time
/// includes idle time, as `GetSystemTimes` reports it.
#[cfg(any(windows, test))]
#[derive(Clone, Copy, Debug)]
struct CpuTimes {
    idle: u64,
    kernel: u64,
    user: u64,
}

/// The busy share of the time between two samples, as a whole percentage.
#[cfg(any(windows, test))]
fn busy_pct(before: CpuTimes, after: CpuTimes) -> Option<u8> {
    let idle = after.idle.checked_sub(before.idle)?;
    let total = after.kernel.checked_sub(before.kernel)? + after.user.checked_sub(before.user)?;
    if total == 0 || idle > total {
        return None;
    }
    u8::try_from((total - idle).saturating_mul(100) / total).ok()
}

/// The one-minute load average.
#[cfg(unix)]
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

#[cfg(windows)]
fn memory_available_pct() -> Option<u8> {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut status = MEMORYSTATUSEX {
        dwLength: u32::try_from(std::mem::size_of::<MEMORYSTATUSEX>()).ok()?,
        ..MEMORYSTATUSEX::default()
    };
    // SAFETY: a MEMORYSTATUSEX with its length set.
    if unsafe { GlobalMemoryStatusEx(&mut status) } == 0 {
        return None;
    }
    percent_of(status.ullAvailPhys, status.ullTotalPhys)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn memory_available_pct() -> Option<u8> {
    None
}

#[cfg(any(target_os = "linux", windows, test))]
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
    fn windows_cpu_use_is_the_busy_share_between_samples() {
        let at = |idle, kernel, user| CpuTimes { idle, kernel, user };
        // 400 ticks passed, 100 of them idle: 75 percent busy.
        assert_eq!(busy_pct(at(0, 0, 0), at(100, 300, 100)), Some(75));
        assert_eq!(busy_pct(at(0, 0, 0), at(100, 100, 0)), Some(0));
        assert_eq!(busy_pct(at(0, 0, 0), at(0, 50, 50)), Some(100));
        // No time passed, a counter that went backwards, or more idle time
        // than time: withheld.
        assert_eq!(busy_pct(at(5, 5, 5), at(5, 5, 5)), None);
        assert_eq!(busy_pct(at(5, 5, 5), at(4, 9, 9)), None);
        assert_eq!(busy_pct(at(0, 0, 0), at(10, 5, 0)), None);
    }

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
