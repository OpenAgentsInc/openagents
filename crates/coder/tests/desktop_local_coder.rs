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
        use openagents_chat_app::task_chat::{self, Action as ChatAction, Answer};
        use openagents_desktop::control::HostControl;
        use coder_host::access::protocol::QueueEdit;
        let Answer::Activity(summary) = client.task_chat(task_chat::Request::Activity { task: id.clone() }).unwrap() else { panic!("activity") };
        assert_eq!(summary.sequence, revision);
        assert_eq!(summary.phase, nostr::activity_summary::Phase::Queued);
        let mut chat = task_chat::Session::new(openagents_chat::basic_chats::Spawned { host: summary.host.clone(), task: id.clone(), project: Some("checkout".into()), at: None }, std::time::Instant::now());
        let (ticket, query) = chat.tick(std::time::Instant::now()).unwrap();
        chat.outcome(ticket, Ok(client.task_chat(query).unwrap()));
        let (ticket, queued) = chat.submit("Retained next turn  ", None, task_chat::unix_now()).unwrap();
        let task_chat::Request::Operation { operation: Operation::CommandTask { command }, .. } = &queued else { panic!("queued command") };
        let command_id = command.command.clone();
        let answer = client.task_chat(queued.clone()).unwrap();
        assert!(chat.outcome(ticket, Ok(answer.clone())));
        assert_eq!(client.task_chat(queued).unwrap(), answer, "a lost acknowledgment replays the admitted receipt");
        let coder_host::access::protocol::Outcome::Queue { queue } = client.task_operation(&"7".repeat(64), Operation::QueueTask { task: id.clone(), edit: QueueEdit::Lease {} }).unwrap() else { panic!("lease") };
        assert_eq!(queue.items.len(), 1);
        assert_eq!(queue.items[0].text.as_deref(), Some("Retained next turn  "));
        assert!(queue.lease.is_some());
        let coder_host::access::protocol::Outcome::Queue { queue } = client.task_operation(&"8".repeat(64), Operation::QueueTask { task: id.clone(), edit: QueueEdit::Edit { command: command_id.clone(), text: "Edited next turn".into() } }).unwrap() else { panic!("edit") };
        assert_eq!(queue.items[0].text.as_deref(), Some("Edited next turn"));
        client.task_operation(&"9".repeat(64), Operation::QueueTask { task: id.clone(), edit: QueueEdit::Remove { command: command_id } }).unwrap();
        client.task_operation(&"a".repeat(64), Operation::QueueTask { task: id.clone(), edit: QueueEdit::Release {} }).unwrap();
        let (ticket, steer) = chat.action(ChatAction::Steer, "Corrected scratch instructions", task_chat::unix_now()).unwrap();
        assert!(chat.outcome(ticket, Ok(client.task_chat(steer).unwrap())));
        let Answer::Activity(summary) = client.task_chat(task_chat::Request::Activity { task: id.clone() }).unwrap() else { panic!("activity") };
        assert!(summary.sequence > revision);
        chat.summary = Some(summary);
        let (ticket, stop) = chat.action(ChatAction::Stop, "", task_chat::unix_now()).unwrap();
        assert!(!chat.outcome(ticket, Ok(client.task_chat(stop).unwrap())));
        let Answer::Activity(summary) = client.task_chat(task_chat::Request::Activity { task: id.clone() }).unwrap() else { panic!("activity") };
        assert_eq!(summary.phase, nostr::activity_summary::Phase::Cancelled);
        assert!(client.task_chat(task_chat::Request::Activity { task: "bad".into() }).is_err());
        client.task_operation(&"4".repeat(64), Operation::ArchiveTask { task: id.clone() }).unwrap();
        client.task_operation(&"4".repeat(64), Operation::ArchiveTask { task: id.clone() }).unwrap();
        assert!(client.task_chat(task_chat::Request::Activity { task: id.clone() }).is_err(), "archived tasks are excluded from activity");
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn desktop_handoff_keeps_the_phone_prompt_project_policy_and_restart_identity() {
    use coder::task::{adapter, autostart, capacity};
    use openagents_chat::{
        basic_chats::BasicChats, basic_coder::Turn, cache::Cache, service::Command,
    };
    use openagents_connect::keys::{KeyName, KeySource, Secret};
    use openagents_desktop::control::HostControl;
    use std::{
        path::{Path, PathBuf},
        sync::Mutex,
    };
    #[derive(Clone)]
    struct Launcher(Arc<Mutex<Vec<String>>>);
    impl autostart::Launch for Launcher {
        fn launch(
            &self,
            _: &autostart::Engine,
            grant: &Path,
            _: &Path,
        ) -> Result<autostart::Launched, String> {
            let grant = coder::task::owner::Grant::parse(&std::fs::read(grant).unwrap()).unwrap();
            self.0.lock().unwrap().push(grant.task_id);
            Ok(autostart::Launched {
                owner_process: std::process::id(),
                grant_digest: "sha256:fixture".into(),
            })
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let (relay, relay_task, _) = relay::start().await;
    let root = temp.path().join("host");
    std::fs::create_dir_all(&root).unwrap();
    let tasks = temp.path().join("tasks");
    drop(coder::task::Store::open(&tasks).unwrap());
    let project = temp.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    let projects = BTreeMap::from([("openagents".into(), project)]);
    let keys = Arc::new(FileKeySource::new(temp.path().join("keys")));
    let secret = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
    keys.store(KeyName::Host, &Secret::from_bytes(secret.secret_bytes()))
        .unwrap();
    let cache = Cache::open(&root.join("basic-chats"), &secret).unwrap();
    let id = "a".repeat(32);
    let archived = "b".repeat(32);
    let plain = "c".repeat(32);
    let turns = vec![
        Turn::user("fix the flaky test in openagents"),
        Turn::assistant("I'll hand this conversation to Coder.", None),
    ];
    let mut chats = BasicChats::new(
        None,
        None,
        Some(Cache::open(&root.join("basic-chats"), &secret).unwrap()),
    );
    for chat in [&id, &archived, &plain] {
        assert!(chats.create(chat, 1));
    }
    chats.rename(&id, "Fix flaky test").unwrap();
    chats.archive(&archived, 2);
    let summary = chats.get(&id).unwrap().clone();
    drop(chats);
    cache
        .write(
            &format!("basic-{id}"),
            &serde_json::json!({"summary":summary, "turns":turns, "lane":"computer"}),
        )
        .unwrap();
    let expected = openagents_chat::delegation::prompt("Fix flaky test", &turns);
    let launched = Arc::new(Mutex::new(vec![]));
    let autostart = Arc::new(
        autostart::Autostart::new(
            root.clone(),
            tasks.clone(),
            projects.clone(),
            Box::new(Launcher(launched.clone())),
            || 1000,
        )
        .with_probe(|_| capacity::Connection::Connected)
        .with_usage_fetch(|_| Err(coder::task::usage::Failure::Network))
        .foreground(),
    );
    let policy = autostart::Policy {
        schema: autostart::POLICY_SCHEMA.into(),
        enabled: true,
        workspaces: vec!["openagents".into()],
        max_running: 1,
        changed_at: 1,
        engine: autostart::Engine {
            adapter: adapter::NAME.into(),
            controller: PathBuf::from("/synthetic/microcoder"),
            model: "gpt-6-luna".into(),
            effort: None,
            max_steps: 4,
            wall_seconds: 60,
            memory_bytes: 1024 * 1024 * 1024,
            write_workspace: false,
            decision_endpoint: "https://api.typesafe.ai".into(),
            decision_model: "jev-1.13.0".into(),
            routes: vec![],
            usage_probe: None,
            access: adapter::Access::Boundary,
        },
    };
    policy.save(&root).unwrap();
    let inbox = Arc::new(
        coder::task::remote::Inbox::new(&tasks, projects.clone()).with_autostart(autostart),
    );
    let socket = temp.path().join("c/control.sock");
    let mut config = Config::new(temp.path().join("access"), vec![relay], 1);
    config.label = "Scratch Mac".into();
    config.policy = RelayPolicy::LoopbackTest;
    config.keys = Some(Keys(keys));
    config.workspaces = projects;
    config.control = Some(Control {
        path: socket.clone(),
        root: root.clone(),
        autostart: Some(root.clone()),
        uid: coder_host::control::own_uid(),
    });
    let running = coder_host::start(config.clone(), inbox.clone())
        .await
        .unwrap();
    let first_socket = socket.clone();
    let first_id = id.clone();
    let first_tasks = tasks.clone();
    let (binding, original_record) = tokio::task::spawn_blocking(move || {
        let mut client = SocketControl::new(first_socket);
        let snapshot = client
            .chat(Command::Read {
                chat: first_id.clone(),
                before: None,
            })
            .unwrap();
        assert_eq!(snapshot.ready_computer.as_deref(), Some("Scratch Mac"));
        assert!(snapshot.computer);
        assert!(client.chat(Command::RunCoder { chat: archived }).is_err());
        assert!(client.chat(Command::RunCoder { chat: plain }).is_err());
        let original = cache
            .read::<serde_json::Value>(&format!("basic-{first_id}"))
            .unwrap()
            .unwrap();
        let snapshot = client
            .chat(Command::RunCoder {
                chat: first_id.clone(),
            })
            .unwrap();
        let binding = snapshot.coder.unwrap();
        assert_eq!(binding.project.as_deref(), Some("openagents"));
        assert_eq!(
            client
                .chat(Command::RunCoder { chat: first_id })
                .unwrap()
                .coder
                .as_ref(),
            Some(&binding)
        );
        let store = coder::task::Store::open(&first_tasks).unwrap();
        assert_eq!(store.list().unwrap().len(), 1);
        let task = store.show(&binding.task).unwrap();
        assert_eq!(task.intent.prompt, expected);
        assert_eq!(
            task.intent.title,
            coder_host::access::client::tasks::title(&expected)
        );
        assert_eq!(
            task.intent.configuration.model.as_deref(),
            Some("gpt-6-luna")
        );
        (binding, original)
    })
    .await
    .unwrap();
    assert_eq!(*launched.lock().unwrap(), vec![binding.task.clone()]);
    assert!(
        autostart::journal(&root)
            .iter()
            .any(|entry| entry.event == "started" && entry.task.as_deref() == Some(&binding.task))
    );
    running.shutdown().await;
    // Lose only the local binding acknowledgment. The persisted handoff and
    // admitted request still return the original task after a host restart.
    let cache = Cache::open(&root.join("basic-chats"), &secret).unwrap();
    cache
        .write(&format!("basic-{id}"), &original_record)
        .unwrap();
    let mut index: Vec<openagents_chat::basic_chats::Summary> =
        cache.read("basic-index").unwrap().unwrap();
    let row = index.iter_mut().find(|row| row.id == id).unwrap();
    row.coder = None;
    row.updated = 1;
    cache.write("basic-index", &index).unwrap();
    let running = coder_host::start(config, inbox).await.unwrap();
    tokio::task::spawn_blocking(move || {
        let mut client = SocketControl::new(socket);
        assert!(
            client
                .chat(Command::Read {
                    chat: id.clone(),
                    before: None
                })
                .unwrap()
                .coder
                .is_none()
        );
        let snapshot = client.chat(Command::RunCoder { chat: id }).unwrap();
        assert_eq!(snapshot.coder.unwrap().task, binding.task);
        assert_eq!(
            coder::task::Store::open(&tasks)
                .unwrap()
                .list()
                .unwrap()
                .len(),
            1
        );
    })
    .await
    .unwrap();
    assert_eq!(launched.lock().unwrap().len(), 1);
    running.shutdown().await;
    relay_task.abort();
}
