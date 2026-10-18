//! Harness commands and their in-memory fixture implementation.

use std::path::Path;
use std::time::Duration;

#[async_trait::async_trait]
pub(super) trait CommandRunner: Send + Sync {
    async fn output(
        &self,
        executable: &Path,
        args: &[&str],
        timeout: Duration,
        env: &[(&str, &str)],
    ) -> Result<String, String>;
}

pub(super) struct SystemCommands;

#[async_trait::async_trait]
impl CommandRunner for SystemCommands {
    async fn output(
        &self,
        executable: &Path,
        args: &[&str],
        timeout: Duration,
        env: &[(&str, &str)],
    ) -> Result<String, String> {
        super::run_command_output_env(executable, args, timeout, env).await
    }
}

#[cfg(all(test, unix))]
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

#[cfg(all(test, unix))]
pub(super) struct TestCommands {
    installed: Mutex<String>,
    fail_check: AtomicBool,
    calls: Mutex<Vec<Vec<String>>>,
}

#[cfg(all(test, unix))]
impl Default for TestCommands {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, unix))]
impl TestCommands {
    pub(super) fn new() -> Self {
        Self {
            installed: Mutex::new("1.0.0\n".into()),
            fail_check: AtomicBool::new(false),
            calls: Mutex::default(),
        }
    }

    pub(super) fn version(&self) -> String {
        self.installed.lock().unwrap().clone()
    }

    pub(super) fn fail_check(&self, fail: bool) {
        self.fail_check.store(fail, Ordering::SeqCst);
    }

    pub(super) fn calls(&self) -> Vec<Vec<String>> {
        self.calls.lock().unwrap().clone()
    }
}

#[cfg(all(test, unix))]
#[async_trait::async_trait]
impl CommandRunner for TestCommands {
    async fn output(
        &self,
        _executable: &Path,
        args: &[&str],
        _timeout: Duration,
        _env: &[(&str, &str)],
    ) -> Result<String, String> {
        self.calls
            .lock()
            .unwrap()
            .push(args.iter().map(|arg| (*arg).to_owned()).collect());
        match args {
            ["version"] | ["--version"] => Ok(self.version().trim().to_owned()),
            ["update", "--check"] if self.fail_check.load(Ordering::SeqCst) => {
                Err("fixture version check failed".into())
            }
            ["update", "--check"] => Ok("2.0.0".into()),
            ["update"] => {
                *self.installed.lock().unwrap() = "2.0.0\n".into();
                Ok(String::new())
            }
            _ => Err(format!("unexpected fixture command arguments: {args:?}")),
        }
    }
}
