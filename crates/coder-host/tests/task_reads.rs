//! Canonical task reads pass through native signed admission and disclosure.

#[path = "support/connect.rs"]
mod connect;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use coder_host::access::protocol::{Operation, Outcome};
use coder_host::access::task_read::{
    List, ListQuery, Original, OriginalChunk, OriginalQuery, Page, PageQuery, Scope,
};
use coder_host::access::{Code, Right, Rights};
use coder_host::client::{Device, Link};
use coder_host::{Error, TaskCreate, TaskRef, Tasks};
use tokio::net::TcpStream;

#[derive(Default)]
struct Reader {
    reads: AtomicUsize,
    effects: AtomicUsize,
}
impl Tasks for Reader {
    fn task_list(&self, query: &ListQuery) -> Result<List, Code> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(List {
            workspace: query.workspace.clone(),
            snapshot_digest: format!("sha256:{}", "a".repeat(64)),
            rows: vec![],
            next: None,
            more_available: false,
        })
    }
    fn task_read(&self, _: &PageQuery) -> Result<Page, Code> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Err(Code::Unavailable)
    }
    fn task_original(&self, _: &OriginalQuery) -> Result<OriginalChunk, Code> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Err(Code::Unavailable)
    }
    fn create(&self, request: &str, _: &str, _: &TaskCreate) -> Result<TaskRef, Code> {
        self.effects.fetch_add(1, Ordering::SeqCst);
        Ok(TaskRef {
            task: request.into(),
            revision: 1,
            phase: nostr::activity_summary::Phase::Queued,
        })
    }
    fn steer(&self, _: &str, _: &str, _: &str, _: u64, _: &str) -> Result<TaskRef, Code> {
        self.effects.fetch_add(1, Ordering::SeqCst);
        Err(Code::Unsupported)
    }
    fn cancel(&self, _: &str, _: &str, _: &str, _: u64, _: &str) -> Result<TaskRef, Code> {
        self.effects.fetch_add(1, Ordering::SeqCst);
        Err(Code::Unsupported)
    }
}

fn operations(workspace: &str) -> [Operation; 3] {
    [
        Operation::ListTasks {
            query: ListQuery {
                workspace: workspace.into(),
                cursor: None,
                limit: 1,
            },
        },
        Operation::ReadTask {
            query: PageQuery {
                workspace: workspace.into(),
                task: "fixture-task".into(),
                revision: None,
                cursor: None,
                limit: 1,
            },
        },
        Operation::ReadTaskOriginal {
            query: OriginalQuery {
                scope: Scope {
                    workspace: workspace.into(),
                    task: "fixture-task".into(),
                    revision: 1,
                    attempt: Some(1),
                    intent_digest: format!("sha256:{}", "a".repeat(64)),
                },
                original: Original {
                    source: "task".into(),
                    digest: format!("sha256:{}", "b".repeat(64)),
                    bytes: 3,
                    media_type: "application/json".into(),
                },
                cursor: None,
                limit: 3,
            },
        },
    ]
}

async fn link(host: &connect::Host, right: Right) -> Link {
    let secret = connect::key();
    let at = connect::now();
    let invitation = host
        .store
        .invite(&host.relay, Rights::new([right]).unwrap(), at, at + 3600)
        .unwrap();
    let access = coder_host::access::client::redeem(&invitation.code, &secret, connect::POLICY)
        .await
        .unwrap();
    let device = Arc::new(Device::new(access, secret, connect::POLICY).unwrap());
    let address = host.running.local_addr();
    let stream = TcpStream::connect(address).await.unwrap();
    Link::direct(
        device,
        stream,
        address.to_string(),
        host.running.generation(),
        Duration::from_secs(5),
    )
    .await
    .unwrap()
}

fn denied(error: Error, code: Code) {
    assert!(
        matches!(&error, Error::Access(error) if error.code == code),
        "{error:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_read_dispatch_checks_native_workspace_disclosure_before_the_owner() {
    let reader = Arc::new(Reader::default());
    let host = connect::host_with(connect::Options {
        tasks: Some(reader.clone()),
        ..connect::Options::default()
    })
    .await;
    let observer = link(&host, Right::Observe).await;
    for operation in operations("unadmitted") {
        denied(observer.call(operation).await.unwrap_err(), Code::Forbidden);
    }
    assert_eq!(reader.reads.load(Ordering::SeqCst), 0);
    let [list, page, original] = operations("checkout");
    assert!(matches!(
        observer.call(list).await.unwrap(),
        Outcome::Tasks { .. }
    ));
    for operation in [page, original] {
        denied(
            observer.call(operation).await.unwrap_err(),
            Code::Unavailable,
        );
    }
    let operator = link(&host, Right::Operate).await;
    denied(
        operator
            .call(operations("checkout")[0].clone())
            .await
            .unwrap_err(),
        Code::MissingRight,
    );
    assert_eq!(reader.reads.load(Ordering::SeqCst), 3);
    assert_eq!(reader.effects.load(Ordering::SeqCst), 0);
    host.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reconnected_link_sends_the_original_packet_and_rejects_changed_bytes() {
    let reader = Arc::new(Reader::default());
    let host = connect::host_with(connect::Options {
        tasks: Some(reader.clone()),
        ..connect::Options::default()
    })
    .await;
    let first = link(&host, Right::Operate).await;
    let action = Operation::CreateTask {
        task: TaskCreate {
            title: "Synthetic exact retry".into(),
            prompt: "Original bytes".into(),
            workspace: "checkout".into(),
            images: vec![],
            engine: None,
        },
    };
    let pending = first
        .device()
        .prepare_operation(action.clone(), "c".repeat(64))
        .unwrap();
    let mut forged = pending.clone();
    let Operation::CreateTask { task } = &mut forged.request.op else {
        panic!("create")
    };
    task.prompt = "Forged unsigned replacement".into();
    denied(
        first.send_pending(&forged).await.unwrap_err(),
        Code::Forbidden,
    );
    assert_eq!(reader.effects.load(Ordering::SeqCst), 0);
    // Losing this response does not authorize another operation identity.
    first.send_pending(&pending).await.unwrap();
    let device = first.device().clone();
    drop(first);
    let address = host.running.local_addr();
    let reconnect = Link::direct(
        device.clone(),
        TcpStream::connect(address).await.unwrap(),
        address.to_string(),
        host.running.generation(),
        Duration::from_secs(5),
    )
    .await
    .unwrap();
    reconnect.send_pending(&pending).await.unwrap();
    assert_eq!(reader.effects.load(Ordering::SeqCst), 1);
    let replacement = device.prepare_operation(action, "c".repeat(64)).unwrap();
    assert_ne!(replacement.event.id, pending.event.id);
    denied(
        reconnect.send_pending(&replacement).await.unwrap_err(),
        Code::Conflict,
    );
    assert_eq!(reader.effects.load(Ordering::SeqCst), 1);
    // A saved result does not let the revoked device bypass fresh admission.
    host.store.revoke(&device.key(), connect::now()).unwrap();
    assert!(reconnect.send_pending(&pending).await.is_err());
    assert_eq!(reader.effects.load(Ordering::SeqCst), 1);
    host.running.shutdown().await;
}
