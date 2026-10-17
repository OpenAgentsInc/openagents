//! Durable terminal launch identities. An uncertain launch is never repeated.

use std::collections::BTreeMap;
use std::path::PathBuf;

use coder_connect::store::Store;
use coder_pty::wire::{Reason, Refusal};
use serde::{Deserialize, Serialize};

const VERSION: &str = "openagents.host-terminal-opens.v1";
const MAX_OPENS: usize = 4096;

pub(super) struct OpenRequests(pub PathBuf);

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Book {
    v: String,
    requests: BTreeMap<String, String>,
}

impl coder_pty::host::OpenRequests for OpenRequests {
    fn reserve(&self, principal: &str, request: &str, digest: &str) -> Result<(), Refusal> {
        // Store holds a cross-process lock and fsyncs both the replacement
        // file and its directory. No command content is retained.
        let unavailable =
            |_| Refusal::new(Reason::Unavailable, "the command record cannot be saved");
        let mut store = Store::open(&self.0, true).map_err(unavailable)?;
        let mut book = store.load::<Book>().map_err(unavailable)?.unwrap_or(Book {
            v: VERSION.into(),
            requests: BTreeMap::new(),
        });
        if book.v != VERSION || book.requests.len() > MAX_OPENS {
            return Err(Refusal::new(
                Reason::Unavailable,
                "the command record cannot be read",
            ));
        }
        let key = format!("{principal} {request}");
        if let Some(previous) = book.requests.get(&key) {
            return Err(if previous == digest {
                Refusal::new(
                    Reason::Lost,
                    "this command may have already run; it will not run again",
                )
            } else {
                Refusal::new(
                    Reason::IdempotencyConflict,
                    "this request ID was used with different content",
                )
            });
        }
        // Do not evict: forgetting a launch would allow it to run again.
        if book.requests.len() == MAX_OPENS {
            return Err(Refusal::new(
                Reason::LimitExceeded,
                "the computer has reached its saved command limit",
            ));
        }
        book.requests.insert(key, digest.into());
        store.save(&book).map_err(unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_pty::host::OpenRequests as _;

    #[test]
    fn reservation_survives_reopening_and_conflicts_on_changed_content() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("commands");
        OpenRequests(path.clone())
            .reserve("device", "request", "digest")
            .unwrap();
        let reopened = OpenRequests(path);
        assert_eq!(
            reopened
                .reserve("device", "request", "digest")
                .unwrap_err()
                .reason,
            Reason::Lost
        );
        assert_eq!(
            reopened
                .reserve("device", "request", "changed")
                .unwrap_err()
                .reason,
            Reason::IdempotencyConflict
        );
        reopened
            .reserve("other-device", "request", "digest")
            .unwrap();
    }

    #[test]
    fn corrupt_or_full_records_refuse_without_forgetting_commands() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("commands");
        let recorder = OpenRequests(path.clone());
        let mut store = Store::open(&path, true).unwrap();
        let book = Book {
            v: VERSION.into(),
            requests: (0..MAX_OPENS)
                .map(|i| (format!("device {i}"), "digest".into()))
                .collect(),
        };
        store.save(&book).unwrap();
        drop(store);
        assert_eq!(
            recorder
                .reserve("device", "new", "digest")
                .unwrap_err()
                .reason,
            Reason::LimitExceeded
        );
        assert_eq!(
            recorder
                .reserve("device", "0", "digest")
                .unwrap_err()
                .reason,
            Reason::Lost
        );
        std::fs::write(path.join("observer.json"), b"broken").unwrap();
        assert_eq!(
            recorder
                .reserve("device", "new", "digest")
                .unwrap_err()
                .reason,
            Reason::Unavailable
        );
    }

    #[cfg(unix)]
    mod processes {
        use super::*;
        use coder_pty::host::{Config, Host, Right, Rights};
        use coder_pty::wire::{Launch, Open, Size, Status};
        use std::sync::{Arc, Barrier};
        use std::time::{Duration, Instant};

        struct Allowed;
        impl Rights for Allowed {
            fn holds(&self, _: &str, _: Right) -> bool {
                true
            }
        }

        fn config(root: &std::path::Path) -> Config {
            let mut config = Config::new().workspace("57".repeat(32), root);
            config.open_requests = Some(Arc::new(OpenRequests(root.join("commands"))));
            config.base_env.retain(|(name, _)| name != "HOME");
            config
                .base_env
                .push(("HOME".into(), root.display().to_string()));
            config
        }

        fn request() -> Open {
            Open::new(
                "11".repeat(32),
                "57".repeat(32),
                "",
                Launch::Command {
                    program: "/bin/sh".into(),
                    args: vec!["-c".into(), "printf x >> marker".into()],
                },
                Size::new(24, 80),
            )
        }

        fn wait_marker(root: &std::path::Path) {
            let deadline = Instant::now() + Duration::from_secs(5);
            while std::fs::read(root.join("marker"))
                .unwrap_or_default()
                .is_empty()
            {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(10));
            }
        }

        #[test]
        fn concurrent_launch_and_restart_never_repeat_a_process() {
            let root = tempfile::tempdir().unwrap();
            let host = Host::new(config(root.path()), Arc::new(Allowed));
            let barrier = Barrier::new(2);
            std::thread::scope(|scope| {
                let launch = || {
                    barrier.wait();
                    host.open("device", &request()).unwrap().0
                };
                let a = scope.spawn(launch);
                let b = scope.spawn(launch);
                let statuses = [a.join().unwrap(), b.join().unwrap()];
                assert!(statuses.contains(&Status::Accepted));
                assert!(statuses.contains(&Status::Duplicate));
            });
            wait_marker(root.path());
            drop(host);
            let host = Host::new(config(root.path()), Arc::new(Allowed));
            assert_eq!(
                host.open("device", &request()).unwrap_err().reason,
                Reason::Lost
            );
            let mut changed = request();
            changed.dir = "other".into();
            assert_eq!(
                host.open("device", &changed).unwrap_err().reason,
                Reason::IdempotencyConflict
            );
            assert_eq!(std::fs::read(root.path().join("marker")).unwrap(), b"x");
        }

        #[test]
        fn a_crash_after_reservation_never_starts_the_command_on_retry() {
            struct Interrupted(OpenRequests);
            impl coder_pty::host::OpenRequests for Interrupted {
                fn reserve(
                    &self,
                    principal: &str,
                    request: &str,
                    digest: &str,
                ) -> Result<(), Refusal> {
                    self.0.reserve(principal, request, digest)?;
                    Err(Refusal::new(Reason::Unavailable, "simulated interruption"))
                }
            }
            let root = tempfile::tempdir().unwrap();
            let mut first = config(root.path());
            first.open_requests = Some(Arc::new(Interrupted(OpenRequests(
                root.path().join("commands"),
            ))));
            let host = Host::new(first, Arc::new(Allowed));
            assert_eq!(
                host.open("device", &request()).unwrap_err().reason,
                Reason::Unavailable
            );
            drop(host);
            let host = Host::new(config(root.path()), Arc::new(Allowed));
            assert_eq!(
                host.open("device", &request()).unwrap_err().reason,
                Reason::Lost
            );
            assert!(!root.path().join("marker").exists());
        }

        #[test]
        fn launch_child() {
            let Some(root) = std::env::var_os("OA_TEST_TERMINAL_OPEN_ROOT") else {
                return;
            };
            let root = std::path::PathBuf::from(root);
            let host = Host::new(config(&root), Arc::new(Allowed));
            match host.open("device", &request()) {
                Ok(_) => wait_marker(&root),
                Err(refusal) => {
                    assert!(matches!(refusal.reason, Reason::Lost | Reason::Unavailable))
                }
            }
        }

        #[test]
        fn independent_host_processes_cannot_launch_the_same_identity_twice() {
            let root = tempfile::tempdir().unwrap();
            let binary = std::env::current_exe().unwrap();
            let spawn = || {
                std::process::Command::new(&binary)
                    .args([
                        "--exact",
                        "serve::open_requests::tests::processes::launch_child",
                    ])
                    .env_clear()
                    .env("HOME", root.path())
                    .env("OA_TEST_TERMINAL_OPEN_ROOT", root.path())
                    .stdout(std::process::Stdio::null())
                    .spawn()
                    .unwrap()
            };
            let mut a = spawn();
            let mut b = spawn();
            assert!(a.wait().unwrap().success());
            assert!(b.wait().unwrap().success());
            assert_eq!(std::fs::read(root.path().join("marker")).unwrap(), b"x");
        }
    }
}
