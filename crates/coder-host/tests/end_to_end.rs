//! The acceptance run with an in-memory task owner. The `coder` crate runs
//! the same scenario with its durable task inbox.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use coder_host::{Code, TaskCreate, TaskRef, Tasks};
use nostr::activity_summary::Phase;

#[path = "../../coder-control/src/tests/relay.rs"]
mod relay;
#[path = "support/scenario.rs"]
mod scenario;

/// Tasks keyed by ID, with each idempotency key applied once.
#[derive(Default)]
struct Memory {
    tasks: Mutex<BTreeMap<String, scenario::TaskView>>,
    applied: Mutex<BTreeMap<String, TaskRef>>,
}

impl Memory {
    fn once(
        &self,
        key: &str,
        apply: impl FnOnce(&mut BTreeMap<String, scenario::TaskView>) -> Result<TaskRef, Code>,
    ) -> Result<TaskRef, Code> {
        let mut applied = self.applied.lock().unwrap();
        if let Some(done) = applied.get(key) {
            return Ok(done.clone());
        }
        let done = apply(&mut self.tasks.lock().unwrap())?;
        applied.insert(key.to_owned(), done.clone());
        Ok(done)
    }
}

impl Tasks for Memory {
    fn create(&self, key: &str, _: &str, task: &TaskCreate) -> Result<TaskRef, Code> {
        if task.workspace != "checkout" {
            return Err(Code::Forbidden);
        }
        self.once(key, |tasks| {
            tasks.insert(
                key.to_owned(),
                scenario::TaskView {
                    revision: 1,
                    title: task.title.clone(),
                    prompt: task.prompt.clone(),
                    status: "queued".into(),
                    started: false,
                },
            );
            Ok(TaskRef {
                task: key.to_owned(),
                revision: 1,
                phase: Phase::Queued,
            })
        })
    }

    fn steer(
        &self,
        key: &str,
        _: &str,
        task: &str,
        revision: u64,
        prompt: &str,
    ) -> Result<TaskRef, Code> {
        self.once(key, |tasks| {
            let view = tasks.get_mut(task).ok_or(Code::Forbidden)?;
            if view.revision != revision {
                return Err(Code::Stale);
            }
            view.revision += 1;
            view.prompt = prompt.to_owned();
            Ok(TaskRef {
                task: task.to_owned(),
                revision: view.revision,
                phase: Phase::Queued,
            })
        })
    }

    fn cancel(
        &self,
        key: &str,
        _: &str,
        task: &str,
        revision: u64,
        _: &str,
    ) -> Result<TaskRef, Code> {
        self.once(key, |tasks| {
            let view = tasks.get_mut(task).ok_or(Code::Forbidden)?;
            if view.revision != revision {
                return Err(Code::Stale);
            }
            view.revision += 1;
            view.status = "cancelled".into();
            Ok(TaskRef {
                task: task.to_owned(),
                revision: view.revision,
                phase: Phase::Cancelled,
            })
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn enroll_discover_connect_fall_back_catch_up_and_revoke() {
    scenario::run(|_| {
        let memory = Arc::new(Memory::default());
        let view = memory.clone();
        let inspect: scenario::Inspect =
            Box::new(move |task| view.tasks.lock().unwrap().get(task).cloned());
        (memory as Arc<dyn Tasks>, inspect)
    })
    .await;
}
