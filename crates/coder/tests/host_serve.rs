//! The remote-access acceptance run against the durable task inbox that
//! `coder host serve` hands task operations to. The scenario itself lives
//! with `coder-host`, which also runs it with an in-memory owner.

use std::collections::BTreeMap;
use std::sync::Arc;

use coder::task::remote::Inbox;
use coder::task::{Execution, Status, Store};
use coder_host::Tasks;

#[path = "../../coder-control/src/tests/relay.rs"]
mod relay;
#[path = "../../coder-host/tests/support/scenario.rs"]
mod scenario;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn host_serve_composes_access_reach_terminals_and_the_task_inbox() {
    scenario::run(|paths| {
        let workspace = std::fs::canonicalize(&paths.workspace).unwrap();
        let inbox = Inbox::new(
            paths.tasks.clone(),
            BTreeMap::from([("checkout".to_owned(), workspace)]),
        );
        let store = paths.tasks.clone();
        let inspect: scenario::Inspect = Box::new(move |task| {
            let task = Store::open(&store).ok()?.show(task).ok()?;
            Some(scenario::TaskView {
                revision: task.revision,
                title: task.intent.title.clone(),
                prompt: task.effective_prompt().to_owned(),
                status: match task.status {
                    Status::Queued => "queued",
                    Status::Cancelled => "cancelled",
                    Status::Running => "running",
                    Status::CancelRequested => "cancel_requested",
                    Status::Finished => "finished",
                    Status::Unknown => "unknown",
                }
                .into(),
                started: task.execution != Execution::NotStarted || task.run.is_some(),
            })
        });
        (Arc::new(inbox) as Arc<dyn Tasks>, inspect)
    })
    .await;
}
