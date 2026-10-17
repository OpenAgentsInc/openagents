//! Core-only placement uses logical lifecycle semantics in the public API.
use computer_driver::{
    Capabilities, CommandCursor, CommandProgress, CommandRead, CommandSpec, CreateRequest, Driver,
    Exec, ExecOutput, Meter, Observation, Outcome, State,
};
use oa_computers::registry::{Catalog, EnrolledAuthority, Placement, Registry, Target};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone, Default)]
struct Workspace(Arc<Mutex<(bool, BTreeMap<String, String>)>>);
impl Driver for Workspace {
    fn capabilities(&self) -> Capabilities {
        let mut caps = Capabilities::new("synthetic-enrolled", [], &["workspace"]);
        caps.workspace_attachment = true;
        caps.command_sessions = true;
        caps
    }
    async fn create(&self, request: &CreateRequest) -> Outcome<String> {
        assert!(request.image.is_none() && request.ttl_seconds.is_none());
        Outcome::done("logical".into())
    }
    async fn observe(&self, _: &str) -> Outcome<Observation> {
        Outcome::done(Observation {
            state: if self.0.lock().unwrap().0 {
                State::Gone
            } else {
                State::Running
            },
            detail: None,
        })
    }
    async fn exec(&self, _: &str, _: &Exec) -> Outcome<ExecOutput> {
        panic!("attachment lifecycle cannot run shell cleanup")
    }
    async fn put_file(&self, _: &str, _: &str, _: &[u8]) -> Outcome<()> {
        panic!("unexpected file write")
    }
    async fn get_file(&self, _: &str, _: &str) -> Outcome<Vec<u8>> {
        panic!("unexpected file read")
    }
    async fn meter(&self, _: &str) -> Outcome<Meter> {
        Outcome::done(Meter {
            running: !self.0.lock().unwrap().0,
            evidence: "synthetic workspace".into(),
            seconds: None,
        })
    }
    async fn delete(&self, _: &str) -> Outcome<()> {
        self.0.lock().unwrap().0 = true;
        Outcome::done(())
    }
    async fn start_command(&self, _: &str, spec: &CommandSpec) -> Outcome<String> {
        self.0
            .lock()
            .unwrap()
            .1
            .insert(spec.id.clone(), spec.digest.clone());
        Outcome::done(spec.id.clone())
    }
    async fn read_command(
        &self,
        _: &str,
        id: &str,
        cursor: CommandCursor,
        _: u64,
    ) -> Outcome<CommandRead> {
        let digest = self.0.lock().unwrap().1.get(id).cloned();
        Outcome::done(CommandRead {
            progress: CommandProgress::Exited { code: 0 },
            timed_out: false,
            digest,
            stdout: if cursor.stdout == 0 {
                b"ok".to_vec()
            } else {
                vec![]
            },
            stderr: vec![],
        })
    }
}
#[tokio::test]
async fn public_core_attach_rejects_cloud_controls_and_detaches_without_host_cleanup() {
    let Ok(admin) = std::env::var("OA_COMPUTERS_TEST_DATABASE_URL") else {
        return;
    };
    let (db, _name) = oa_computers::Db::scratch(&admin).await.unwrap();
    let store = oa_computers::Store::new(db.clone());
    let workspace = Workspace::default();
    let caps = workspace.capabilities();
    let mut target = Target::new(
        "own-project",
        "synthetic-connection",
        caps.clone(),
        computer_driver::ArtifactKind::EnrolledWorkspace,
        Placement::OurCloud,
    )
    .unwrap();
    target.placement = Placement::Enrolled;
    target.enrolled = Some(EnrolledAuthority {
        account: "alice".into(),
        host: "ab".repeat(32),
        workspace: "project".into(),
    });
    let catalog = Catalog::new("cloud-default", "test", [target]).unwrap();
    let registry = Registry::new(
        catalog.clone(),
        BTreeMap::from([("own-project".into(), workspace.clone())]),
    )
    .unwrap();
    let app = oa_computers::app_with_registry(
        store.clone(),
        catalog.clone(),
        "synthetic-service-token",
        oa_computers::turns::Secrets::default(),
        None,
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = reqwest::Client::new();
    let url = format!("http://{address}/v1/computers");
    let body =
        json!({"placement":"enrolled","enrolled_host":"own-project","idempotency_key":"attach"});
    for (account, body, status) in [
        ("bob", body.clone(), 403),
        (
            "alice",
            json!({"placement":"enrolled","enrolled_host":"guessed","idempotency_key":"guess"}),
            403,
        ),
        (
            "alice",
            json!({"placement":"enrolled","enrolled_host":"own-project","needs":{"profiles":["suspend"]}}),
            422,
        ),
    ] {
        assert_eq!(
            client
                .post(&url)
                .bearer_auth("synthetic-service-token")
                .header("X-OpenAgents-Account", account)
                .json(&body)
                .send()
                .await
                .unwrap()
                .status()
                .as_u16(),
            status
        );
    }
    let created: Value = client
        .post(&url)
        .bearer_auth("synthetic-service-token")
        .header("X-OpenAgents-Account", "alice")
        .json(&body)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = created["id"].as_str().unwrap();
    let entry = store.get("alice", id).await.unwrap().unwrap();
    assert_eq!(entry.size, "workspace");
    assert!(entry.image.is_none());
    assert!(entry.binding.as_ref().unwrap().artifact.is_none());
    let repeated: Value = client
        .post(&url)
        .bearer_auth("synthetic-service-token")
        .header("X-OpenAgents-Account", "alice")
        .json(&body)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(repeated["id"], id);
    for suffix in ["stop", "wake", "turns"] {
        let response = client
            .post(format!("{url}/{id}/{suffix}"))
            .bearer_auth("synthetic-service-token")
            .header("X-OpenAgents-Account", "alice")
            .json(&json!({"command":"true"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 422, "{suffix}");
    }
    // Run the normal reconciler on a LocalSet, using only the disposable DB.
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let reconciler = std::rc::Rc::new(oa_computers::reconcile::Reconciler {
                store: store.clone(),
                driver: registry,
                settings: {
                    let mut settings = oa_computers::reconcile::Settings::new("synthetic-enrolled");
                    settings.poll = Duration::from_millis(20);
                    settings.revisit = Duration::from_millis(50);
                    settings.driver_poll = Duration::from_millis(10);
                    settings.driver_deadline = Duration::from_secs(5);
                    settings
                },
                turns: oa_computers::turns::Secrets::default(),
            });
            let task = tokio::task::spawn_local(reconciler.run());
            for _ in 0..100 {
                if store
                    .get("alice", id)
                    .await
                    .unwrap()
                    .unwrap()
                    .record
                    .resource()
                    .is_some()
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            assert!(
                store
                    .get("alice", id)
                    .await
                    .unwrap()
                    .unwrap()
                    .record
                    .resource()
                    .is_some()
            );
            let command_body =
                json!({"command":"printf ok","timeout_seconds":5,"idempotency_key":"command-1"});
            let command: Value = client
                .post(format!("{url}/{id}/commands"))
                .bearer_auth("synthetic-service-token")
                .header("X-OpenAgents-Account", "alice")
                .json(&command_body)
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .json()
                .await
                .unwrap();
            let command_id = command["id"].as_str().unwrap();
            for _ in 0..150 {
                if store
                    .command("alice", id, command_id)
                    .await
                    .unwrap()
                    .unwrap()
                    .status
                    == "succeeded"
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            assert_eq!(
                store
                    .command("alice", id, command_id)
                    .await
                    .unwrap()
                    .unwrap()
                    .status,
                "succeeded"
            );
            let current = store.get("alice", id).await.unwrap().unwrap();
            assert!(current.record.checkpoints.is_empty());
            assert_eq!(current.desired, oa_computers::store::Desired::Running);
            let repeated: Value = client
                .post(format!("{url}/{id}/commands"))
                .bearer_auth("synthetic-service-token")
                .header("X-OpenAgents-Account", "alice")
                .json(&command_body)
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .json()
                .await
                .unwrap();
            assert_eq!(repeated["id"], command_id);
            assert_eq!(workspace.0.lock().unwrap().1.len(), 1);
            client
                .delete(format!("{url}/{id}"))
                .bearer_auth("synthetic-service-token")
                .header("X-OpenAgents-Account", "alice")
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap();
            for _ in 0..200 {
                if store.get("alice", id).await.unwrap().unwrap().phase == "deleted" {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            assert!(workspace.0.lock().unwrap().0);
            assert_eq!(
                store.get("alice", id).await.unwrap().unwrap().phase,
                "deleted"
            );
            task.abort();
        })
        .await;
    server.abort();
    // The test runner stops and removes its private database cluster.
}
