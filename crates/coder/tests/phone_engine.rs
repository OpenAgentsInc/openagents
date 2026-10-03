//! A phone asks a computer for the coding engine the person named (#10081).
//!
//! A phone pairs with a real host over iroh and sends NIP-HOST
//! `task.create` with the typed `engine` of its chat's `run_coder` offer.
//! The host hands it to the durable task inbox under the owner's auto-start
//! policy (Codex, then Claude Code), whose launcher is a fake that records
//! each grant instead of starting an engine. A requested engine the policy
//! admits starts first; one it does not admit falls back to the policy's own
//! route and the task's summary says why; a request without the field, as a
//! phone sends to a host that predates it, runs the policy's default.
//! Nothing reaches a public relay, a model, or the real home.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use coder::task::autostart::{self, Autostart, Launch, Launched, Policy, Route};
use coder::task::capacity::{Connection, Provider};
use coder::task::owner::Grant;
use coder::task::remote::Inbox;
use coder::task::usage;
use coder_host::Tasks;
use coder_host::access::protocol::{Operation, Outcome, TASK_ENGINE, TaskCreate};
use nostr::cj_conversation::Engine;

#[path = "../../coder-host/tests/support/connect.rs"]
mod support;

use support::{Options, Phone, host_with, now};

/// Records each grant instead of starting an engine.
struct Fake(Arc<Mutex<Vec<PathBuf>>>);

impl Launch for Fake {
    fn launch(&self, _: &autostart::Engine, grant: &Path, _: &Path) -> Result<Launched, String> {
        self.0.lock().unwrap().push(grant.to_path_buf());
        Ok(Launched {
            owner_process: std::process::id(),
            grant_digest: "sha256:fake".into(),
        })
    }
}

fn clock() -> u64 {
    coder_host::unix_time().unwrap()
}

fn offline(_: Provider) -> Result<usage::Response, usage::Failure> {
    Err(usage::Failure::Network)
}

/// The owner's policy: Codex first, then Claude Code, in `checkout`.
fn policy() -> Policy {
    Policy {
        schema: autostart::POLICY_SCHEMA.into(),
        enabled: true,
        workspaces: vec!["checkout".into()],
        max_running: 8,
        engine: autostart::Engine {
            adapter: coder::task::adapter::NAME.into(),
            controller: PathBuf::from(if cfg!(windows) {
                r"C:\opt\coder\microcoder.exe"
            } else {
                "/opt/coder/microcoder"
            }),
            model: "gpt-6-luna".into(),
            effort: Some("medium".into()),
            max_steps: None,
            wall_seconds: None,
            memory_bytes: 4 * 1024 * 1024 * 1024,
            write_workspace: true,
            decision_endpoint: "https://api.typesafe.ai".into(),
            decision_model: "jev-latest".into(),
            routes: vec![
                Route {
                    provider: Provider::Codex,
                    model: "gpt-6-luna".into(),
                    effort: None,
                },
                Route {
                    provider: Provider::Claude,
                    model: "claude-opus-5-5".into(),
                    effort: Some("high".into()),
                },
            ],
            usage_probe: None,
            access: coder::task::adapter::Access::Boundary,
            claude: coder::task::autostart::ClaudeRuns::default(),
        },
        changed_at: 1,
    }
}

fn create(prompt: &str, engine: Option<Engine>) -> TaskCreate {
    let mut task = coder_host::access::client::tasks::input(prompt, "checkout");
    task.engine = engine;
    task
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_phone_asks_the_computer_for_the_engine_the_person_named() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("host");
    let store = temp.path().join("tasks");
    let checkout = temp.path().join("checkout");
    std::fs::create_dir_all(&checkout).unwrap();
    let workspaces = BTreeMap::from([("checkout".to_owned(), checkout.canonicalize().unwrap())]);
    policy().save(&root).unwrap();
    let launched = Arc::new(Mutex::new(Vec::new()));
    let autostart = Arc::new(
        Autostart::new(
            root.clone(),
            store.clone(),
            workspaces.clone(),
            Box::new(Fake(launched.clone())),
            clock,
        )
        .with_probe(|_| Connection::Connected)
        .with_usage_fetch(offline)
        .foreground(),
    );
    let inbox = Arc::new(Inbox::new(&store, workspaces).with_autostart(autostart));
    let host = host_with(Options {
        tasks: Some(inbox.clone() as Arc<dyn Tasks>),
        ..Options::default()
    })
    .await;
    // The host says its `task.create` reads the field, so a phone sends it.
    assert!(coder_host::CAPABILITIES.contains(&TASK_ENGINE));

    let phone = Phone::new().await;
    let (_, code) = host.code().await;
    let (_, access, _) = phone.redeem(&code, &host.relay, now()).await;
    let device = phone.device(access.unwrap());
    let link = phone.link(&host, &device).await.unwrap();
    let start = |task: TaskCreate| {
        let link = &link;
        async move {
            match link.call(Operation::CreateTask { task }).await.unwrap() {
                Outcome::Dispatched { receipt } => receipt.reference,
                other => panic!("not dispatched: {other:?}"),
            }
        }
    };
    let grant = |index: usize| {
        let path = launched.lock().unwrap()[index].clone();
        Grant::parse(&std::fs::read(path).unwrap())
            .unwrap()
            .adapter_configuration
            .unwrap()
    };

    // Asked for Claude Code, which the policy admits: it starts first, and
    // Codex stays its fallback.
    let claude = start(create("Do a test delegation", Some(Engine::ClaudeCode))).await;
    let first = grant(0);
    assert_eq!(first.provider, "claude");
    assert_eq!(first.model, "claude-opus-5-5");
    assert_eq!(first.fallbacks.len(), 1);
    assert_eq!(first.fallbacks[0].provider, "codex");
    assert_eq!(inbox.note(&claude), None);

    // Asked for Devin, which the policy does not admit: no route is added,
    // the policy's own first route runs, and the summary says why.
    let devin = start(create("Do a test delegation to devin", Some(Engine::Devin))).await;
    let second = grant(1);
    assert_eq!(second.provider, "codex");
    assert_eq!(second.fallbacks.len(), 1);
    assert_eq!(second.fallbacks[0].provider, "claude");
    let note = inbox.note(&devin).expect("a reason");
    assert_eq!(
        note.headline(),
        "You asked for Devin; it is not one of the engines this computer's Coder policy allows, so Codex is running."
    );

    // The request a phone sends a host that predates the field: the
    // policy's default, with nothing to say.
    let plain = start(create("Fix the flaky test", None)).await;
    let third = grant(2);
    assert_eq!(third.provider, "codex");
    assert_eq!(inbox.note(&plain), None);
    assert_eq!(launched.lock().unwrap().len(), 3);
    host.running.shutdown().await;
}
