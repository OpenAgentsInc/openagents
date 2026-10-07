//! Runtime telemetry and observation helpers for Psionic serving and transport
//! surfaces.

use std::{
    env,
    future::Future,
    path::PathBuf,
    sync::{OnceLock, RwLock},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{runtime::Runtime, task::JoinHandle};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str =
    "runtime telemetry and observation helpers for psionic serving and transport surfaces";

/// Environment key that enables traced Tokio runtime installation.
pub const TOKIO_TELEMETRY_ENABLED_ENV: &str = "PSIONIC_TOKIO_TELEMETRY_ENABLED";
/// Environment key that selects the trace output path.
pub const TOKIO_TELEMETRY_TRACE_PATH_ENV: &str = "PSIONIC_TOKIO_TELEMETRY_TRACE_PATH";
/// Environment key that enables Tokio task tracking.
pub const TOKIO_TELEMETRY_TASK_TRACKING_ENV: &str = "PSIONIC_TOKIO_TELEMETRY_TASK_TRACKING";
/// Environment key that enables Linux CPU profiling.
pub const TOKIO_TELEMETRY_CPU_PROFILING_ENV: &str = "PSIONIC_TOKIO_TELEMETRY_LINUX_CPU_PROFILING";
/// Environment key that enables Linux scheduler-event capture.
pub const TOKIO_TELEMETRY_SCHED_EVENTS_ENV: &str = "PSIONIC_TOKIO_TELEMETRY_LINUX_SCHED_EVENTS";
/// Environment key that selects the per-segment trace size ceiling.
pub const TOKIO_TELEMETRY_ROTATE_AFTER_BYTES_ENV: &str =
    "PSIONIC_TOKIO_TELEMETRY_ROTATE_AFTER_BYTES";
/// Environment key that selects the total on-disk trace size ceiling.
pub const TOKIO_TELEMETRY_MAX_TOTAL_BYTES_ENV: &str = "PSIONIC_TOKIO_TELEMETRY_MAX_TOTAL_BYTES";

const DEFAULT_ROTATE_AFTER_BYTES: u64 = 16 * 1024 * 1024;
const DEFAULT_MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;

#[cfg(all(feature = "tokio-runtime-telemetry", not(tokio_unstable)))]
compile_error!(
    "psionic-observe tokio-runtime-telemetry requires `--cfg tokio_unstable` in RUSTFLAGS"
);

/// Telemetry config shared by Psionic Tokio-owned serve and transport runtimes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokioRuntimeTelemetryConfig {
    /// Whether to install dial9-backed Tokio runtime hooks at all.
    pub enabled: bool,
    /// Output path prefix for rotating trace segments.
    pub trace_path: Option<PathBuf>,
    /// Whether to emit task spawn/terminate metadata and enable traced
    /// `spawn` helpers for wake-event capture.
    pub task_tracking: bool,
    /// Whether to request Linux perf-based CPU profiling.
    pub linux_cpu_profiling: bool,
    /// Whether to request Linux perf-based scheduler-event capture.
    pub linux_sched_events: bool,
    /// Maximum size for one rotated trace segment.
    pub rotate_after_bytes: u64,
    /// Maximum total on-disk trace size before old segments are evicted.
    pub max_total_bytes: u64,
}

impl Default for TokioRuntimeTelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            trace_path: None,
            task_tracking: false,
            linux_cpu_profiling: false,
            linux_sched_events: false,
            rotate_after_bytes: DEFAULT_ROTATE_AFTER_BYTES,
            max_total_bytes: DEFAULT_MAX_TOTAL_BYTES,
        }
    }
}

impl TokioRuntimeTelemetryConfig {
    /// Loads runtime telemetry config from process environment.
    pub fn from_env() -> Result<Self, TokioRuntimeTelemetryError> {
        Self::from_env_reader(|key| env::var(key).ok())
    }

    fn from_env_reader(
        mut read: impl FnMut(&str) -> Option<String>,
    ) -> Result<Self, TokioRuntimeTelemetryError> {
        Ok(Self {
            enabled: parse_optional_bool_env(TOKIO_TELEMETRY_ENABLED_ENV, &mut read)?
                .unwrap_or(false),
            trace_path: read(TOKIO_TELEMETRY_TRACE_PATH_ENV).map(PathBuf::from),
            task_tracking: parse_optional_bool_env(TOKIO_TELEMETRY_TASK_TRACKING_ENV, &mut read)?
                .unwrap_or(false),
            linux_cpu_profiling: parse_optional_bool_env(
                TOKIO_TELEMETRY_CPU_PROFILING_ENV,
                &mut read,
            )?
            .unwrap_or(false),
            linux_sched_events: parse_optional_bool_env(
                TOKIO_TELEMETRY_SCHED_EVENTS_ENV,
                &mut read,
            )?
            .unwrap_or(false),
            rotate_after_bytes: parse_optional_u64_env(
                TOKIO_TELEMETRY_ROTATE_AFTER_BYTES_ENV,
                &mut read,
            )?
            .unwrap_or(DEFAULT_ROTATE_AFTER_BYTES),
            max_total_bytes: parse_optional_u64_env(
                TOKIO_TELEMETRY_MAX_TOTAL_BYTES_ENV,
                &mut read,
            )?
            .unwrap_or(DEFAULT_MAX_TOTAL_BYTES),
        })
    }

    /// Returns whether this config should install a traced runtime.
    #[must_use]
    pub fn installs_runtime_hooks(&self) -> bool {
        self.enabled
    }
}

/// Errors returned while configuring or building traced Tokio runtimes.
#[derive(Debug, Error)]
pub enum TokioRuntimeTelemetryError {
    /// One environment variable contained an invalid boolean value.
    #[error(
        "invalid boolean value `{value}` for environment variable `{key}` (expected 1/0, true/false, yes/no)"
    )]
    InvalidBooleanEnv {
        /// Environment key.
        key: &'static str,
        /// Invalid environment value.
        value: String,
    },
    /// One environment variable contained an invalid integer value.
    #[error("invalid integer value `{value}` for environment variable `{key}`: {reason}")]
    InvalidIntegerEnv {
        /// Environment key.
        key: &'static str,
        /// Invalid environment value.
        value: String,
        /// Parse failure reason.
        reason: String,
    },
    /// Telemetry was requested without a trace path.
    #[error("Tokio telemetry is enabled but `{TOKIO_TELEMETRY_TRACE_PATH_ENV}` was not provided")]
    MissingTracePath,
    /// The helper crate was built without dial9 support.
    #[error(
        "Tokio telemetry was requested but `psionic-observe` was built without the `tokio-runtime-telemetry` feature"
    )]
    FeatureNotEnabled,
    /// Linux CPU profiling or sched events were requested without the CPU feature.
    #[error(
        "Linux CPU profiling or scheduler events were requested but `psionic-observe` was built without the `tokio-runtime-telemetry-cpu` feature"
    )]
    CpuFeatureNotEnabled,
    /// Building the Tokio runtime or trace writer failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[cfg(feature = "tokio-runtime-telemetry")]
#[derive(Clone)]
pub struct TokioRuntimeTelemetryHandle(dial9_tokio_telemetry::telemetry::TelemetryHandle);

#[cfg(not(feature = "tokio-runtime-telemetry"))]
#[derive(Clone, Debug, Default)]
pub struct TokioRuntimeTelemetryHandle;

#[cfg(feature = "tokio-runtime-telemetry")]
impl std::fmt::Debug for TokioRuntimeTelemetryHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TokioRuntimeTelemetryHandle(<dial9>)")
    }
}

#[cfg(feature = "tokio-runtime-telemetry")]
impl TokioRuntimeTelemetryHandle {
    fn from_inner(handle: dial9_tokio_telemetry::telemetry::TelemetryHandle) -> Self {
        Self(handle)
    }

    fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.0.spawn(future)
    }
}

/// Guard that keeps Tokio runtime telemetry registered for the process-owned
/// runtime lifetime.
pub struct TokioRuntimeTelemetryGuard {
    #[cfg(feature = "tokio-runtime-telemetry")]
    _inner: Option<dial9_tokio_telemetry::telemetry::TelemetryGuard>,
    registered_global: bool,
}

impl TokioRuntimeTelemetryGuard {
    fn inactive() -> Self {
        Self {
            #[cfg(feature = "tokio-runtime-telemetry")]
            _inner: None,
            registered_global: false,
        }
    }
}

impl Drop for TokioRuntimeTelemetryGuard {
    fn drop(&mut self) {
        if self.registered_global {
            set_current_runtime_telemetry_handle(None);
        }
    }
}

/// Builds a multi-threaded Tokio runtime with optional dial9 telemetry.
pub fn build_main_runtime(
    config: &TokioRuntimeTelemetryConfig,
) -> Result<(Runtime, TokioRuntimeTelemetryGuard), TokioRuntimeTelemetryError> {
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.enable_all();
    build_runtime(builder, config)
}

/// Builds a caller-supplied Tokio runtime with optional dial9 telemetry.
pub fn build_runtime(
    mut builder: tokio::runtime::Builder,
    config: &TokioRuntimeTelemetryConfig,
) -> Result<(Runtime, TokioRuntimeTelemetryGuard), TokioRuntimeTelemetryError> {
    if config.enabled && config.trace_path.is_none() {
        return Err(TokioRuntimeTelemetryError::MissingTracePath);
    }
    if !config.installs_runtime_hooks() {
        set_current_runtime_telemetry_handle(None);
        return Ok((builder.build()?, TokioRuntimeTelemetryGuard::inactive()));
    }

    #[cfg(not(feature = "tokio-runtime-telemetry"))]
    {
        let _ = builder;
        let _ = config;
        Err(TokioRuntimeTelemetryError::FeatureNotEnabled)
    }

    #[cfg(feature = "tokio-runtime-telemetry")]
    {
        build_traced_runtime(builder, config)
    }
}

/// Returns the currently registered runtime telemetry handle, when one exists.
#[must_use]
pub fn current_runtime_telemetry_handle() -> Option<TokioRuntimeTelemetryHandle> {
    global_handle_guard_read().clone()
}

/// Spawns a Tokio task through the current dial9 handle when telemetry is
/// active; otherwise falls back to `tokio::spawn`.
pub fn spawn_runtime_task<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    #[cfg(feature = "tokio-runtime-telemetry")]
    if let Some(handle) = current_runtime_telemetry_handle() {
        return handle.spawn(future);
    }

    tokio::spawn(future)
}

#[cfg(feature = "tokio-runtime-telemetry")]
fn build_traced_runtime(
    builder: tokio::runtime::Builder,
    config: &TokioRuntimeTelemetryConfig,
) -> Result<(Runtime, TokioRuntimeTelemetryGuard), TokioRuntimeTelemetryError> {
    use dial9_tokio_telemetry::telemetry::{RotatingWriter, TracedRuntime};

    let trace_path = config
        .trace_path
        .clone()
        .ok_or(TokioRuntimeTelemetryError::MissingTracePath)?;
    let writer = RotatingWriter::new(
        trace_path.clone(),
        config.rotate_after_bytes,
        config.max_total_bytes,
    )?;
    #[cfg(feature = "tokio-runtime-telemetry-cpu")]
    let mut traced_builder = TracedRuntime::builder()
        .with_trace_path(trace_path)
        .with_task_tracking(config.task_tracking);

    #[cfg(not(feature = "tokio-runtime-telemetry-cpu"))]
    let traced_builder = TracedRuntime::builder()
        .with_trace_path(trace_path)
        .with_task_tracking(config.task_tracking);

    #[cfg(feature = "tokio-runtime-telemetry-cpu")]
    {
        use dial9_tokio_telemetry::telemetry::{CpuProfilingConfig, SchedEventConfig};

        if config.linux_cpu_profiling {
            traced_builder = traced_builder.with_cpu_profiling(CpuProfilingConfig::default());
        }
        if config.linux_sched_events {
            traced_builder = traced_builder.with_sched_events(SchedEventConfig::default());
        }
    }

    #[cfg(not(feature = "tokio-runtime-telemetry-cpu"))]
    if config.linux_cpu_profiling || config.linux_sched_events {
        return Err(TokioRuntimeTelemetryError::CpuFeatureNotEnabled);
    }

    let (runtime, guard) = traced_builder.build_and_start(builder, writer)?;
    set_current_runtime_telemetry_handle(Some(TokioRuntimeTelemetryHandle::from_inner(
        guard.handle(),
    )));
    Ok((
        runtime,
        TokioRuntimeTelemetryGuard {
            _inner: Some(guard),
            registered_global: true,
        },
    ))
}

fn parse_optional_bool_env(
    key: &'static str,
    read: &mut impl FnMut(&str) -> Option<String>,
) -> Result<Option<bool>, TokioRuntimeTelemetryError> {
    let Some(value) = read(key) else {
        return Ok(None);
    };
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "1" | "true" | "yes" => Ok(Some(true)),
        "0" | "false" | "no" => Ok(Some(false)),
        _ => Err(TokioRuntimeTelemetryError::InvalidBooleanEnv { key, value }),
    }
}

fn parse_optional_u64_env(
    key: &'static str,
    read: &mut impl FnMut(&str) -> Option<String>,
) -> Result<Option<u64>, TokioRuntimeTelemetryError> {
    let Some(value) = read(key) else {
        return Ok(None);
    };
    value.trim().parse::<u64>().map(Some).map_err(|error| {
        TokioRuntimeTelemetryError::InvalidIntegerEnv {
            key,
            value,
            reason: error.to_string(),
        }
    })
}

fn global_handle_lock() -> &'static RwLock<Option<TokioRuntimeTelemetryHandle>> {
    static GLOBAL_HANDLE: OnceLock<RwLock<Option<TokioRuntimeTelemetryHandle>>> = OnceLock::new();
    GLOBAL_HANDLE.get_or_init(|| RwLock::new(None))
}

fn global_handle_guard_read()
-> std::sync::RwLockReadGuard<'static, Option<TokioRuntimeTelemetryHandle>> {
    match global_handle_lock().read() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn global_handle_guard_write()
-> std::sync::RwLockWriteGuard<'static, Option<TokioRuntimeTelemetryHandle>> {
    match global_handle_lock().write() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn set_current_runtime_telemetry_handle(handle: Option<TokioRuntimeTelemetryHandle>) {
    *global_handle_guard_write() = handle;
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_MAX_TOTAL_BYTES, DEFAULT_ROTATE_AFTER_BYTES, TOKIO_TELEMETRY_CPU_PROFILING_ENV,
        TOKIO_TELEMETRY_ENABLED_ENV, TOKIO_TELEMETRY_MAX_TOTAL_BYTES_ENV,
        TOKIO_TELEMETRY_ROTATE_AFTER_BYTES_ENV, TOKIO_TELEMETRY_SCHED_EVENTS_ENV,
        TOKIO_TELEMETRY_TASK_TRACKING_ENV, TOKIO_TELEMETRY_TRACE_PATH_ENV,
        TokioRuntimeTelemetryConfig, TokioRuntimeTelemetryError, build_main_runtime,
        current_runtime_telemetry_handle, spawn_runtime_task,
    };
    use std::collections::BTreeMap;
    use std::sync::{Mutex, OnceLock};

    fn telemetry_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn config_defaults_to_disabled_when_env_is_empty() {
        let config = TokioRuntimeTelemetryConfig::from_env_reader(|_| None).expect("config");

        assert!(!config.enabled);
        assert_eq!(config.trace_path, None);
        assert!(!config.task_tracking);
        assert!(!config.linux_cpu_profiling);
        assert!(!config.linux_sched_events);
        assert_eq!(config.rotate_after_bytes, DEFAULT_ROTATE_AFTER_BYTES);
        assert_eq!(config.max_total_bytes, DEFAULT_MAX_TOTAL_BYTES);
    }

    #[test]
    fn config_reads_all_supported_environment_keys() {
        let vars = BTreeMap::from([
            (TOKIO_TELEMETRY_ENABLED_ENV, String::from("true")),
            (
                TOKIO_TELEMETRY_TRACE_PATH_ENV,
                String::from("/tmp/psionic/trace.bin"),
            ),
            (TOKIO_TELEMETRY_TASK_TRACKING_ENV, String::from("yes")),
            (TOKIO_TELEMETRY_CPU_PROFILING_ENV, String::from("1")),
            (TOKIO_TELEMETRY_SCHED_EVENTS_ENV, String::from("1")),
            (TOKIO_TELEMETRY_ROTATE_AFTER_BYTES_ENV, String::from("4096")),
            (TOKIO_TELEMETRY_MAX_TOTAL_BYTES_ENV, String::from("8192")),
        ]);
        let config = TokioRuntimeTelemetryConfig::from_env_reader(|key| vars.get(key).cloned())
            .expect("config");

        assert!(config.enabled);
        assert_eq!(
            config
                .trace_path
                .map(|path| path.to_string_lossy().to_string()),
            Some(String::from("/tmp/psionic/trace.bin"))
        );
        assert!(config.task_tracking);
        assert!(config.linux_cpu_profiling);
        assert!(config.linux_sched_events);
        assert_eq!(config.rotate_after_bytes, 4096);
        assert_eq!(config.max_total_bytes, 8192);
    }

    #[test]
    fn config_rejects_invalid_boolean_environment_values() {
        let vars = BTreeMap::from([(TOKIO_TELEMETRY_ENABLED_ENV, String::from("maybe"))]);
        let error = TokioRuntimeTelemetryConfig::from_env_reader(|key| vars.get(key).cloned())
            .expect_err("invalid bool should fail");

        assert!(matches!(
            error,
            TokioRuntimeTelemetryError::InvalidBooleanEnv {
                key: TOKIO_TELEMETRY_ENABLED_ENV,
                ..
            }
        ));
    }

    #[test]
    fn config_rejects_invalid_integer_environment_values() {
        let vars = BTreeMap::from([(
            TOKIO_TELEMETRY_ROTATE_AFTER_BYTES_ENV,
            String::from("twelve"),
        )]);
        let error = TokioRuntimeTelemetryConfig::from_env_reader(|key| vars.get(key).cloned())
            .expect_err("invalid int should fail");

        assert!(matches!(
            error,
            TokioRuntimeTelemetryError::InvalidIntegerEnv {
                key: TOKIO_TELEMETRY_ROTATE_AFTER_BYTES_ENV,
                ..
            }
        ));
    }

    #[test]
    fn build_main_runtime_without_telemetry_supports_spawn_runtime_task()
    -> Result<(), Box<dyn std::error::Error>> {
        let _test_guard = match telemetry_test_lock().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let (runtime, _runtime_guard) =
            build_main_runtime(&TokioRuntimeTelemetryConfig::default())?;
        let value = runtime.block_on(async {
            let join = spawn_runtime_task(async { 7_u8 });
            join.await
        })?;

        assert_eq!(value, 7);
        assert!(current_runtime_telemetry_handle().is_none());
        Ok(())
    }

    #[cfg(feature = "tokio-runtime-telemetry")]
    #[test]
    fn build_main_runtime_registers_global_handle_when_enabled()
    -> Result<(), Box<dyn std::error::Error>> {
        let _test_guard = match telemetry_test_lock().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let temp = tempfile::tempdir()?;
        let trace_path = temp.path().join("psionic-trace.bin");
        let config = TokioRuntimeTelemetryConfig {
            enabled: true,
            trace_path: Some(trace_path),
            task_tracking: true,
            ..TokioRuntimeTelemetryConfig::default()
        };
        let (runtime, guard) = build_main_runtime(&config)?;

        assert!(current_runtime_telemetry_handle().is_some());
        let value = runtime.block_on(async {
            let join = spawn_runtime_task(async { 11_u8 });
            join.await
        })?;
        assert_eq!(value, 11);
        drop(guard);
        assert!(current_runtime_telemetry_handle().is_none());
        Ok(())
    }
}
