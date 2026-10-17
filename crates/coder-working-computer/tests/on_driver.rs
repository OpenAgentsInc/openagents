//! The chat lifecycle on the driver contract (#11624): a computer is
//! created on first use, a finished turn checkpoints by stopping, the next
//! prompt resumes the same computer with its files, and a driver without
//! Suspend is refused before anything is written.

use coder_working_computer::driver::{Driver, Settled};
use coder_working_computer::on_driver::OnDriver;
use coder_working_computer::store::Store;
use coder_working_computer::*;
use computer_driver::fake::{FakeDriver, Inject};
use computer_driver::{Driver as _, Exec, Profile, State};
use std::time::Duration;

fn spec() -> Spec {
    Spec {
        id: "computer-1".into(),
        owner: Principal {
            workspace: "ws-1".into(),
            principal: "user-1".into(),
        },
        chat: "chat-1".into(),
        project: coder_environment::ProjectLink {
            workspace: "ws-1".into(),
            project: "proj-1".into(),
        },
        source: coder_environment::SourcePin {
            repository: Some("example/repo".into()),
            revision: "a".repeat(40),
            digest: "b".repeat(64),
        },
        base: None,
        size: "small".into(),
        credential_names: Default::default(),
        services: vec![],
        bounds: Bounds {
            idle_ms: 600_000,
            observed_extension_ms: 300_000,
            absolute_ms: 7_200_000,
        },
    }
}

fn fast<D: computer_driver::Driver>(d: D) -> OnDriver<D> {
    let mut p = OnDriver::new(d, None);
    p.poll = Duration::from_millis(10);
    p.deadline = Duration::from_secs(10);
    p
}

#[tokio::test]
async fn a_chat_computer_keeps_its_files_between_turns() {
    let dir = tempfile::tempdir().unwrap();
    let fake = FakeDriver::new(dir.path().join("machines")).boot_observations(2);
    let store = Store::under(dir.path().join("store"));
    store.create(&Computer::new(spec(), 1).unwrap()).unwrap();
    let driver = Driver::new(store.clone(), fast(fake.clone()));

    let Settled::Dispatch {
        generation,
        computer,
    } = driver.prompt("computer-1", 10).await.unwrap()
    else {
        panic!("first prompt did not dispatch");
    };
    assert_eq!(generation, 1);
    let resource = computer.resource().unwrap().to_owned();
    let out = fake
        .exec(&resource, &Exec::new("echo turn-one > work.txt"))
        .await;
    assert!(out.is_done());

    // The finished turn checkpoints by stopping the computer.
    let settled = driver.turn_finished("computer-1", 1, 20).await.unwrap();
    assert!(
        matches!(settled.computer().phase, Phase::Stopped),
        "{settled:?}"
    );
    assert_eq!(fake.live(), vec![(resource.clone(), State::Stopped)]);

    // The next prompt resumes the same computer with the file.
    let Settled::Dispatch {
        generation,
        computer,
    } = driver.prompt("computer-1", 30).await.unwrap()
    else {
        panic!("second prompt did not dispatch");
    };
    assert_eq!(generation, 2);
    assert_eq!(computer.resource(), Some(resource.as_str()));
    let read = fake.exec(&resource, &Exec::new("cat work.txt")).await;
    assert_eq!(read.value().unwrap().stdout, "turn-one\n");

    // Delete settles to Deleted and the machine is gone.
    driver.turn_finished("computer-1", 2, 40).await.unwrap();
    let gone = driver.delete("computer-1", 50).await.unwrap();
    assert!(matches!(gone.computer().phase, Phase::Deleted), "{gone:?}");
    assert!(fake.live().is_empty());
}

#[tokio::test]
async fn a_lost_create_reply_reconciles_to_one_machine() {
    let dir = tempfile::tempdir().unwrap();
    let fake = FakeDriver::new(dir.path().join("machines"));
    fake.inject("create", Inject::LoseReply);
    let store = Store::under(dir.path().join("store"));
    store.create(&Computer::new(spec(), 1).unwrap()).unwrap();
    let driver = Driver::new(store, fast(fake.clone()));
    // The first settle sees Unknown and stops there or repeats the same
    // operation identity; either way one machine exists.
    let mut settled = driver.prompt("computer-1", 10).await.unwrap();
    if !matches!(settled, Settled::Dispatch { .. }) {
        settled = driver.prompt("computer-1", 11).await.unwrap();
    }
    assert!(matches!(settled, Settled::Dispatch { .. }), "{settled:?}");
    assert_eq!(fake.live().len(), 1);
}

#[test]
fn a_driver_without_suspend_is_refused_up_front() {
    let dir = tempfile::tempdir().unwrap();
    let core = FakeDriver::with_profiles(dir.path(), [Profile::Core]);
    let refused = OnDriver::admit(&core).unwrap_err();
    assert_eq!(refused.profile, Profile::Suspend);
    let full = FakeDriver::new(dir.path());
    assert!(OnDriver::admit(&full).is_ok());
}

/// Publishes the target state before the asynchronous operation completes.
#[derive(Clone)]
struct MemoryDriver {
    inner: FakeDriver,
    complete: std::sync::Arc<std::sync::atomic::AtomicBool>,
    delete_complete: std::sync::Arc<std::sync::atomic::AtomicBool>,
    operations: std::sync::Arc<std::sync::Mutex<std::collections::BTreeMap<String, String>>>,
    clock: std::sync::Arc<std::sync::Mutex<Option<String>>>,
}

impl computer_driver::Driver for MemoryDriver {
    fn capabilities(&self) -> computer_driver::Capabilities {
        let mut caps = self.inner.capabilities();
        caps.memory_suspend = true;
        caps
    }
    async fn create(&self, r: &computer_driver::CreateRequest) -> computer_driver::Outcome<String> {
        self.inner.create(r).await
    }
    async fn observe(&self, c: &str) -> computer_driver::Outcome<computer_driver::Observation> {
        self.inner.observe(c).await
    }
    async fn exec(
        &self,
        c: &str,
        e: &Exec,
    ) -> computer_driver::Outcome<computer_driver::ExecOutput> {
        if e.command == "date +%s" {
            if let Some(clock) = self.clock.lock().unwrap().clone() {
                return computer_driver::Outcome::done(computer_driver::ExecOutput {
                    code: 0,
                    stdout: clock,
                    stderr: String::new(),
                });
            }
        }
        self.inner.exec(c, e).await
    }
    async fn put_file(&self, c: &str, p: &str, b: &[u8]) -> computer_driver::Outcome<()> {
        self.inner.put_file(c, p, b).await
    }
    async fn get_file(&self, c: &str, p: &str) -> computer_driver::Outcome<Vec<u8>> {
        self.inner.get_file(c, p).await
    }
    async fn meter(&self, c: &str) -> computer_driver::Outcome<computer_driver::Meter> {
        self.inner.meter(c).await
    }
    async fn delete(&self, c: &str) -> computer_driver::Outcome<()> {
        let result = self.inner.delete(c).await;
        if self
            .delete_complete
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            result
        } else {
            computer_driver::Outcome::unknown("accepted deletion is still pending")
        }
    }
    async fn suspend_operation(&self, c: &str, key: &str) -> computer_driver::Outcome<String> {
        self.lifecycle(c, key, false).await
    }
    async fn resume_operation(&self, c: &str, key: &str) -> computer_driver::Outcome<String> {
        self.lifecycle(c, key, true).await
    }
}

impl MemoryDriver {
    async fn lifecycle(&self, c: &str, key: &str, wake: bool) -> computer_driver::Outcome<String> {
        let known = self.operations.lock().unwrap().contains_key(key);
        if !known {
            let result = if wake {
                self.inner.resume(c).await
            } else {
                self.inner.suspend(c).await
            };
            let value = result.value().unwrap();
            self.operations.lock().unwrap().insert(key.into(), value);
        }
        if self.complete.load(std::sync::atomic::Ordering::SeqCst) {
            computer_driver::Outcome::done(self.operations.lock().unwrap()[key].clone())
        } else {
            computer_driver::Outcome::unknown("accepted operation lookup unavailable")
        }
    }
}

#[tokio::test]
async fn memory_lifecycle_waits_for_original_operations_across_restart_and_keeps_services() {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    let dir = tempfile::tempdir().unwrap();
    let memory = MemoryDriver {
        inner: FakeDriver::new(dir.path().join("machines")).boot_observations(0),
        complete: Arc::new(AtomicBool::new(true)),
        delete_complete: Arc::new(AtomicBool::new(true)),
        operations: Arc::new(Mutex::new(Default::default())),
        clock: Arc::new(Mutex::new(None)),
    };
    let store = Store::under(dir.path().join("records"));
    let mut request = spec();
    request.services.push(ServiceDecl {
        name: "one".into(),
        command: "printf 'started\\n' >> starts".into(),
        cwd: ".".into(),
        health: Health::Command {
            command: "test -s starts".into(),
        },
        ready_within_seconds: 1,
    });
    store.create(&Computer::new(request, 1).unwrap()).unwrap();
    let driver = Driver::new(store.clone(), fast(memory.clone()));
    let first = driver.prompt("computer-1", 10).await.unwrap();
    assert!(matches!(first, Settled::Dispatch { generation: 1, .. }));
    let resource = first.computer().resource().unwrap().to_owned();
    memory.complete.store(false, Ordering::SeqCst);
    let stopped = driver.turn_finished("computer-1", 1, 20).await.unwrap();
    assert!(matches!(stopped.computer().phase, Phase::Unknown { .. }));
    assert_eq!(memory.inner.live(), [(resource.clone(), State::Stopped)]);

    // A new owner reads the saved fence. The early stopped state is not
    // enough to dispatch a queued follow-up while teardown is unfinished.
    let restarted = Driver::new(store.clone(), fast(memory.clone()));
    let waiting = restarted.prompt("computer-1", 30).await.unwrap();
    assert!(matches!(waiting.computer().phase, Phase::Unknown { .. }));
    assert_eq!(memory.operations.lock().unwrap().len(), 1);
    memory.complete.store(true, Ordering::SeqCst);
    let second = restarted.prompt("computer-1", 31).await.unwrap();
    assert!(
        matches!(second, Settled::Dispatch { generation: 2, .. }),
        "{second:?}"
    );
    assert_eq!(second.computer().resource(), Some(resource.as_str()));
    restarted.turn_finished("computer-1", 2, 40).await.unwrap();

    // Wake publishes running early, but the incomplete operation remains a
    // barrier after another API restart and repeated observations.
    memory.complete.store(false, Ordering::SeqCst);
    let waking = restarted.prompt("computer-1", 50).await.unwrap();
    assert!(matches!(waking.computer().phase, Phase::Unknown { .. }));
    assert_eq!(memory.inner.live(), [(resource.clone(), State::Running)]);
    let restarted = Driver::new(store, fast(memory.clone()));
    assert!(matches!(
        restarted
            .prompt("computer-1", 51)
            .await
            .unwrap()
            .computer()
            .phase,
        Phase::Unknown { .. }
    ));
    assert_eq!(memory.operations.lock().unwrap().len(), 4);
    memory.complete.store(true, Ordering::SeqCst);
    let third = restarted.prompt("computer-1", 52).await.unwrap();
    assert!(matches!(third, Settled::Dispatch { generation: 3, .. }));
    let read = memory
        .inner
        .exec(&resource, &Exec::new("cat starts"))
        .await
        .value()
        .unwrap();
    assert_eq!(
        read.stdout, "started\n",
        "restored startup hooks must not run again"
    );
    restarted.turn_finished("computer-1", 3, 60).await.unwrap();
    restarted.delete("computer-1", 70).await.unwrap();
    assert!(memory.inner.live().is_empty());
}

#[tokio::test]
async fn observed_deletion_settles_a_memory_operation_whose_result_cannot_be_read() {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    let dir = tempfile::tempdir().unwrap();
    let memory = MemoryDriver {
        inner: FakeDriver::new(dir.path().join("machines")).boot_observations(0),
        complete: Arc::new(AtomicBool::new(true)),
        delete_complete: Arc::new(AtomicBool::new(true)),
        operations: Arc::new(Mutex::new(Default::default())),
        clock: Arc::new(Mutex::new(None)),
    };
    let store = Store::under(dir.path().join("records"));
    store.create(&Computer::new(spec(), 1).unwrap()).unwrap();
    let driver = Driver::new(store, fast(memory.clone()));
    let first = driver.prompt("computer-1", 10).await.unwrap();
    let resource = first.computer().resource().unwrap().to_owned();
    memory.complete.store(false, Ordering::SeqCst);
    let waiting = driver.turn_finished("computer-1", 1, 20).await.unwrap();
    assert!(matches!(waiting.computer().phase, Phase::Unknown { .. }));
    assert!(memory.inner.delete(&resource).await.is_done());
    let deleted = driver.delete("computer-1", 30).await.unwrap();
    assert_eq!(deleted.computer().phase, Phase::Deleted, "{deleted:?}");
    assert_eq!(memory.operations.lock().unwrap().len(), 1);
    assert!(memory.inner.live().is_empty());
}

#[tokio::test]
async fn memory_idle_and_absolute_bounds_survive_restart_without_an_automatic_wake() {
    use std::sync::{Arc, Mutex, atomic::AtomicBool};
    for observed in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let memory = MemoryDriver {
            inner: FakeDriver::new(dir.path().join("machines")).boot_observations(0),
            complete: Arc::new(AtomicBool::new(true)),
            delete_complete: Arc::new(AtomicBool::new(true)),
            operations: Arc::new(Mutex::new(Default::default())),
            clock: Arc::new(Mutex::new(None)),
        };
        let store = Store::under(dir.path().join("records"));
        let mut request = spec();
        request.bounds = Bounds {
            idle_ms: 10,
            observed_extension_ms: 20,
            absolute_ms: 30,
        };
        store.create(&Computer::new(request, 0).unwrap()).unwrap();
        let provider = fast(memory.clone());
        {
            let lease = store.lease("computer-1").unwrap();
            let awake = coder_working_computer::driver::settle(&provider, &lease, Trigger::Wake, 0)
                .await
                .unwrap();
            assert_eq!(awake.computer().phase, Phase::Awake);
        }
        let driver = Driver::new(store.clone(), provider);
        let end = if observed {
            assert_eq!(
                driver
                    .tick("computer-1", true, 9)
                    .await
                    .unwrap()
                    .computer()
                    .boot()
                    .unwrap()
                    .idle_deadline_ms,
                29
            );
            assert_eq!(
                driver
                    .tick("computer-1", true, 29)
                    .await
                    .unwrap()
                    .computer()
                    .boot()
                    .unwrap()
                    .idle_deadline_ms,
                30
            );
            30
        } else {
            10
        };
        let stopped = driver.tick("computer-1", false, end).await.unwrap();
        assert_eq!(stopped.computer().phase, Phase::Stopped);
        assert_eq!(
            stopped.computer().boot().unwrap().stop_reason,
            Some(if observed {
                StopReason::Absolute
            } else {
                StopReason::Idle
            })
        );
        let calls = memory.operations.lock().unwrap().len();
        let restarted = Driver::new(store, fast(memory.clone()));
        assert_eq!(
            restarted
                .tick("computer-1", false, end + 1)
                .await
                .unwrap()
                .computer()
                .phase,
            Phase::Stopped
        );
        assert_eq!(memory.operations.lock().unwrap().len(), calls);
        restarted.delete("computer-1", end + 2).await.unwrap();
        assert!(memory.inner.live().is_empty());
    }
}

#[tokio::test]
async fn a_restored_service_must_answer_before_dispatch_without_rerunning_startup() {
    use std::sync::{Arc, Mutex, atomic::AtomicBool};
    let dir = tempfile::tempdir().unwrap();
    let memory = MemoryDriver {
        inner: FakeDriver::new(dir.path().join("machines")).boot_observations(0),
        complete: Arc::new(AtomicBool::new(true)),
        delete_complete: Arc::new(AtomicBool::new(true)),
        operations: Arc::new(Mutex::new(Default::default())),
        clock: Arc::new(Mutex::new(None)),
    };
    let store = Store::under(dir.path().join("records"));
    let mut request = spec();
    request.services.push(ServiceDecl {
        name: "app".into(),
        command: "printf 'startup\\n' >> starts".into(),
        cwd: ".".into(),
        health: Health::Command {
            command: "test ! -e unhealthy".into(),
        },
        ready_within_seconds: 1,
    });
    store.create(&Computer::new(request, 1).unwrap()).unwrap();
    let driver = Driver::new(store, fast(memory.clone()));
    let first = driver.prompt("computer-1", 10).await.unwrap();
    let resource = first.computer().resource().unwrap().to_owned();
    assert!(
        memory
            .inner
            .exec(&resource, &Exec::new("touch unhealthy"))
            .await
            .is_done()
    );
    driver.turn_finished("computer-1", 1, 20).await.unwrap();
    *memory.clock.lock().unwrap() = Some("0".into());
    let stale_clock = driver.prompt("computer-1", 29).await.unwrap();
    assert_eq!(stale_clock.computer().phase, Phase::Booting);
    assert_eq!(stale_clock.computer().turn.dispatched, 1);
    *memory.clock.lock().unwrap() = None;
    let unavailable = driver.prompt("computer-1", 30).await.unwrap();
    assert_eq!(unavailable.computer().phase, Phase::Booting);
    assert_eq!(unavailable.computer().turn.dispatched, 1);
    assert!(
        memory
            .inner
            .exec(&resource, &Exec::new("rm unhealthy"))
            .await
            .is_done()
    );
    let ready = driver.prompt("computer-1", 31).await.unwrap();
    assert!(matches!(ready, Settled::Dispatch { generation: 2, .. }));
    let output = memory
        .inner
        .exec(&resource, &Exec::new("cat starts"))
        .await
        .value()
        .unwrap();
    assert_eq!(output.stdout, "startup\n");
    driver.turn_finished("computer-1", 2, 40).await.unwrap();
    driver.delete("computer-1", 50).await.unwrap();
    assert!(memory.inner.live().is_empty());
}

#[tokio::test]
async fn missing_resource_does_not_complete_a_pending_delete_after_restart() {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    let dir = tempfile::tempdir().unwrap();
    let memory = MemoryDriver {
        inner: FakeDriver::new(dir.path().join("machines")).boot_observations(0),
        complete: Arc::new(AtomicBool::new(true)),
        delete_complete: Arc::new(AtomicBool::new(false)),
        operations: Arc::new(Mutex::new(Default::default())),
        clock: Arc::new(Mutex::new(None)),
    };
    let records = dir.path().join("records");
    let store = Store::under(&records);
    store.create(&Computer::new(spec(), 1).unwrap()).unwrap();
    let driver = Driver::new(store, fast(memory.clone()));
    let first = driver.prompt("computer-1", 10).await.unwrap();
    let resource = first.computer().resource().unwrap().to_owned();
    driver.turn_finished("computer-1", 1, 15).await.unwrap();
    let pending = driver.delete("computer-1", 20).await.unwrap();
    assert!(
        pending
            .computer()
            .creates
            .last()
            .unwrap()
            .deletion
            .as_ref()
            .unwrap()
            .is_unknown()
    );
    assert_ne!(pending.computer().phase, Phase::Deleted);
    assert_eq!(
        memory.inner.observe(&resource).await.value().unwrap().state,
        State::Gone
    );
    let restarted = Driver::new(Store::under(records), fast(memory.clone()));
    let pending = restarted.tick("computer-1", false, 30).await.unwrap();
    assert_ne!(pending.computer().phase, Phase::Deleted);
    memory.delete_complete.store(true, Ordering::SeqCst);
    let done = restarted.tick("computer-1", false, 40).await.unwrap();
    assert_eq!(done.computer().phase, Phase::Deleted);
}
