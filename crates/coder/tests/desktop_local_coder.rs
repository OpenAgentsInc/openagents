//! The desktop's real socket client against a scratch durable Coder inbox.
#![cfg(unix)]
use coder_host::{
    access::{
        Code, RelayPolicy,
        protocol::{Operation, TaskCreate},
    },
    config::{Config, Control},
    serve::keys::Keys,
};
use openagents_connect::keys::FileKeySource;
use openagents_desktop::control::SocketControl;
use std::{collections::BTreeMap, sync::Arc};
#[path = "../../coder-control/src/tests/relay.rs"]
mod relay;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn desktop_creates_retries_reads_and_cancels_through_the_phone_clients() {
    let temp = tempfile::tempdir().unwrap();
    let (relay, relay_task, _events) = relay::start().await;
    let root = temp.path().join("host");
    let tasks = temp.path().join("tasks");
    let checkout = temp.path().join("checkout");
    std::fs::create_dir_all(&checkout).unwrap();
    drop(coder::task::Store::open(&tasks).unwrap());
    let socket = temp.path().join("c/control.sock");
    let workspaces = BTreeMap::from([("checkout".to_owned(), checkout)]);
    let inbox = Arc::new(coder::task::remote::Inbox::new(&tasks, workspaces.clone()));
    let mut config = Config::new(temp.path().join("access"), vec![relay], 1);
    config.policy = RelayPolicy::LoopbackTest;
    config.keys = Some(Keys(Arc::new(FileKeySource::new(temp.path().join("keys")))));
    config.workspaces = workspaces;
    config.control = Some(Control {
        path: socket.clone(),
        root: root.clone(),
        autostart: None,
        uid: coder_host::control::own_uid(),
    });
    config.chats = Some(coder_host::tailnet::Chats {
        observer: temp.path().join("observer"),
        sources: coder_history::Config {
            coder: Some(tasks.clone()),
            ..Default::default()
        },
    });
    let running = coder_host::start(config.clone(), inbox.clone())
        .await
        .unwrap();
    let task_root = tasks.clone();
    let task_socket = socket.clone();
    let keys = config.keys.as_ref().unwrap().0.clone();
    let (id, revision) = tokio::task::spawn_blocking(move || {
        let mut client = SocketControl::new(task_socket);
        let request = "1".repeat(64);
        let create = TaskCreate {
            title: "Scratch task".into(),
            prompt: "Synthetic request".into(),
            workspace: "checkout".into(),
        };
        let id = client.create_task(&request, create.clone()).unwrap();
        assert_eq!(client.create_task(&request, create.clone()).unwrap(), id);
        let mut changed = create.clone();
        changed.prompt = "Changed request".into();
        assert_eq!(
            client.create_task(&request, changed).unwrap_err().code,
            Code::Conflict
        );
        assert_eq!(
            client
                .task_operation(&"2".repeat(64), Operation::ListDevices {})
                .unwrap_err()
                .code,
            Code::Forbidden
        );
        assert_eq!(
            client.create_task("bad", create.clone()).unwrap_err().code,
            Code::Malformed
        );
        let mut other = create.clone();
        other.workspace = "missing".into();
        assert_eq!(
            client.create_task(&"3".repeat(64), other).unwrap_err().code,
            Code::Forbidden
        );
        let store = coder::task::Store::open(&task_root).unwrap();
        assert_eq!(store.list().unwrap().len(), 1);
        let task = store.show(&id).unwrap();
        assert_eq!(task.intent.prompt, "Synthetic request");
        assert_eq!(task.status, coder::task::Status::Queued);
        assert_eq!(task.execution, coder::task::Execution::NotStarted);
        drop(store);
        // Journal failure must stop before task-owner dispatch.
        let journal = root.join("task-calls");
        let saved = root.join("saved-task-calls");
        std::fs::rename(&journal, &saved).unwrap();
        std::fs::write(&journal, b"blocked scratch journal").unwrap();
        assert_eq!(
            client
                .create_task(&"5".repeat(64), create.clone())
                .unwrap_err()
                .code,
            Code::Unavailable
        );
        std::fs::remove_file(&journal).unwrap();
        std::fs::rename(saved, journal).unwrap();
        // The normal NIP-HOST admission rejects a signing key that differs
        // from the established owner. Restoring the fixture key restores access.
        use openagents_connect::keys::{KeyName, Secret};
        let owner = keys.load(KeyName::Owner).unwrap().unwrap();
        let other = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
        keys.store(KeyName::Owner, &Secret::from_bytes(other.secret_bytes()))
            .unwrap();
        assert_eq!(
            client
                .create_task(&"6".repeat(64), create)
                .unwrap_err()
                .code,
            Code::Forbidden
        );
        keys.store(KeyName::Owner, &owner).unwrap();
        assert_eq!(
            coder::task::Store::open(&task_root)
                .unwrap()
                .list()
                .unwrap()
                .len(),
            1
        );
        (id, task.revision)
    })
    .await
    .unwrap();
    running.shutdown().await;
    let running = coder_host::start(config, inbox).await.unwrap();
    let observed_tasks = tasks.clone();
    tokio::task::spawn_blocking(move || {
        use coder_host::access::protocol::TaskCreate;
        use coder_history::{CatalogRequest, TranscriptRequest};
        use openagents_desktop::control::{Op, Reply};
        let mut client = SocketControl::new(socket);
        assert_eq!(client.create_task(&"1".repeat(64), TaskCreate { title: "Scratch task".into(), prompt: "Synthetic request".into(), workspace: "checkout".into() }).unwrap(), id);
        // A recorded engine fixture supplies ATIF evidence without running an
        // engine, touching a login, or executing the inert submitted request.
        let trace = format!("{{\"record\":\"session\",\"schema_version\":\"ATIF-v1.8\",\"at\":1790570162020,\"session\":{{\"id\":\"{id}\",\"model\":\"synthetic\",\"door\":\"synthetic\",\"repository\":\"/synthetic\",\"directive\":\"\",\"state\":\"\",\"seconds\":0,\"version\":\"0.1.0\"}}}}\n{{\"record\":\"step\",\"step\":{{\"at\":1790570162024,\"source\":\"User\",\"message\":\"Synthetic request\"}}}}\n");
        std::fs::write(observed_tasks.join(format!("{id}.1.atif.jsonl")), &trace).unwrap();
        let coder_connect::protocol::Observation::Catalog(catalog) = client.task_history(coder_connect::protocol::Query::Catalog(CatalogRequest::default())).unwrap() else { panic!("catalog") };
        assert_eq!(catalog.entries.len(), 1);
        assert_eq!(catalog.entries[0].native_id.as_deref(), Some(id.as_str()));
        let source_id = catalog.entries[0].source_id.clone().unwrap();
        let coder_connect::protocol::Observation::Page(page) = client.task_history(coder_connect::protocol::Query::Page(TranscriptRequest { source_id, cursor: None, max_bytes: 32 * 1024, end: None })).unwrap() else { panic!("page") };
        assert!(!page.chunks.is_empty());
        let serialized = serde_json::to_string(&page).unwrap();
        assert!(serialized.contains("Synthetic request"));
        assert!(client.task_history(coder_connect::protocol::Query::Page(TranscriptRequest { source_id: "/outside/secret".into(), cursor: None, max_bytes: 32 * 1024, end: None })).is_err());
        client.cancel_task(&"4".repeat(64), &id, revision, "Scratch verification complete").unwrap();
        client.cancel_task(&"4".repeat(64), &id, revision, "Scratch verification complete").unwrap();
        let store = coder::task::Store::open(&observed_tasks).unwrap();
        assert_eq!(store.show(&id).unwrap().status, coder::task::Status::Cancelled);
        drop(store);
        let serialized_reply = serde_json::to_string(&client.call(Op::Status {}).unwrap()).unwrap();
        assert!(!serialized_reply.contains("secret"));
        assert!(matches!(client.call(Op::TaskHistory { query: coder_connect::protocol::Query::Catalog(CatalogRequest::default()) }).unwrap(), Reply::TaskHistory { .. }));
    }).await.unwrap();
    running.shutdown().await;
    relay_task.abort();
}
