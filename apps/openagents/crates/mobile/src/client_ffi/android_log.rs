//! Warnings from the Rust core in Android's log (logcat, tag `OpenAgents`).
//!
//! Without this the core's `tracing` output went nowhere on Android, so a
//! failed pairing or sync left no trace to read. Only warnings and errors
//! are written; they carry no codes, keys, or message text.

/// Install the logcat writer once. Later calls do nothing.
pub(crate) fn install() {
    #[cfg(target_os = "android")]
    {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::new(
                    "warn,swarm_discovery=off,iroh=error,noq=error",
                ))
                .with_ansi(false)
                .without_time()
                .with_target(true)
                .with_writer(|| Logcat(Vec::new()))
                .try_init();
        });
    }
}

#[cfg(target_os = "android")]
struct Logcat(Vec<u8>);

#[cfg(target_os = "android")]
impl std::io::Write for Logcat {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(target_os = "android")]
impl Drop for Logcat {
    fn drop(&mut self) {
        #[link(name = "log")]
        unsafe extern "C" {
            fn __android_log_write(
                priority: std::ffi::c_int,
                tag: *const std::ffi::c_char,
                text: *const std::ffi::c_char,
            ) -> std::ffi::c_int;
        }
        const WARN: std::ffi::c_int = 5;
        let text: Vec<u8> = self.0.iter().copied().filter(|b| *b != 0).collect();
        let text = String::from_utf8_lossy(&text);
        let Ok(text) = std::ffi::CString::new(text.trim_end().as_bytes()) else {
            return;
        };
        // SAFETY: both pointers are NUL-terminated strings that outlive the call.
        unsafe {
            __android_log_write(WARN, c"OpenAgents".as_ptr(), text.as_ptr());
        }
    }
}
