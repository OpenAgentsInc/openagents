#![cfg(all(feature = "client", unix))]
//! The selected client talks to the actual HTTP service. Funds, Boat, task,
//! artifacts, and credentials are isolated synthetic collaborators.
use compute_workbench::retail::{self, Client, Config, Error};
use pay_ledger::{
    Ledger,
    compute::{Binding, PrincipalKind, Rights, credential_digest},
};
use retail_cloud::{
    authority::Source,
    cancel::{StopEvidence, StopOwner},
    dispatch::{DispatchSpec, OwnerError, TaskEvent, TaskOwner, TaskStatus},
    fake::{FakeProvider, FakeSandbox, FakeTaskOwner, FakeWallet},
    material::Sandbox,
    provision::{CreateSpec, Provider, ProviderError, Resource, ResourceState},
    retain::{Artifact, Artifacts, Kind, Manifest},
};
use retail_service::{
    Service,
    types::{Config as ServiceConfig, RetailGrant},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

const KEY: &str = "synthetic-client-openai-key-never-live";
#[derive(Default)]
struct Runtime {
    provider: FakeProvider,
    sandbox: FakeSandbox,
    owner: FakeTaskOwner,
    specs: Mutex<BTreeMap<String, DispatchSpec>>,
    resources: Mutex<BTreeMap<String, String>>,
    stops: Mutex<BTreeMap<String, StopEvidence>>,
}
impl Provider for Runtime {
    fn create(&self, s: &CreateSpec) -> std::result::Result<Resource, ProviderError> {
        self.provider.create(s)
    }
    fn find(&self, id: &str) -> std::result::Result<Option<Resource>, ProviderError> {
        self.provider.find(id)
    }
    fn state(&self, id: &str) -> std::result::Result<ResourceState, ProviderError> {
        self.provider.state(id)
    }
    fn delete(&self, id: &str) -> std::result::Result<(), ProviderError> {
        self.provider.delete(id)
    }
    fn usage_seconds(&self, id: &str) -> std::result::Result<Option<u64>, ProviderError> {
        self.provider.usage_seconds(id)
    }
}
impl Sandbox for Runtime {
    fn write_private(&self, r: &str, p: &str, c: &str) -> retail_cloud::Result<bool> {
        self.sandbox.write_private(r, p, c)
    }
    fn clone_source(&self, r: &str, s: &Source) -> retail_cloud::Result<(String, bool)> {
        self.sandbox.clone_source(r, s)
    }
    fn remove(&self, r: &str, p: &str) -> retail_cloud::Result<()> {
        self.sandbox.remove(r, p)
    }
    fn exists(&self, r: &str, p: &str) -> retail_cloud::Result<bool> {
        self.sandbox.exists(r, p)
    }
}
impl TaskOwner for Runtime {
    fn submit(&self, r: &str, s: &DispatchSpec) -> std::result::Result<(), OwnerError> {
        self.specs.lock().unwrap().insert(s.task.clone(), s.clone());
        self.resources
            .lock()
            .unwrap()
            .insert(s.task.clone(), r.into());
        self.owner.submit(r, s)
    }
    fn status(&self, r: &str, t: &str) -> std::result::Result<Option<TaskStatus>, OwnerError> {
        self.owner.status(r, t)
    }
    fn events(&self, r: &str, t: &str, a: u64) -> std::result::Result<Vec<TaskEvent>, OwnerError> {
        self.owner.events(r, t, a)
    }
}
impl StopOwner for Runtime {
    fn stop(&self, r: &str, t: &str, id: &str) -> std::result::Result<StopEvidence, OwnerError> {
        let mut stops = self.stops.lock().unwrap();
        if let Some(s) = stops.get(id) {
            return Ok(s.clone());
        }
        let started = self.owner.status(r, t)?.is_some();
        let status = self
            .owner
            .status(r, t)?
            .filter(|s| matches!(s, TaskStatus::Ended { .. }))
            .unwrap_or(TaskStatus::Cancelled);
        self.owner.set_status(r, t, status.clone());
        let s = StopEvidence {
            at: retail::now() as i64 + 30,
            started,
            status,
            effects: vec![],
        };
        stops.insert(id.into(), s.clone());
        Ok(s)
    }
    fn stopped(
        &self,
        _: &str,
        _: &str,
        id: &str,
    ) -> std::result::Result<Option<StopEvidence>, OwnerError> {
        Ok(self.stops.lock().unwrap().get(id).cloned())
    }
}
impl Artifacts for Runtime {
    fn manifest(&self, r: &str, t: &str) -> retail_cloud::Result<Manifest> {
        let s = self
            .specs
            .lock()
            .unwrap()
            .get(t)
            .cloned()
            .ok_or(retail_cloud::Error::Invalid("synthetic task absent"))?;
        let bytes = b"synthetic retail patch\n";
        Ok(Manifest {
            execution: s.execution,
            task: t.into(),
            resource: r.into(),
            source: Source {
                repository: "https://github.com/OpenAgentsInc/example".into(),
                commit: "c".repeat(40),
            },
            engine: "codex".into(),
            artifacts: vec![Artifact {
                name: "patch".into(),
                kind: Kind::Patch,
                digest: retail_cloud::sha256_hex(bytes),
                size: bytes.len(),
            }],
        })
    }
    fn read(&self, _: &str, _: &str, name: &str, _: usize) -> retail_cloud::Result<Vec<u8>> {
        if name == "patch" {
            Ok(b"synthetic retail patch\n".to_vec())
        } else {
            Err(retail_cloud::Error::Invalid("synthetic artifact absent"))
        }
    }
}
fn private(path: &Path, bytes: &[u8]) {
    use std::io::Write;
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    f.write_all(bytes).unwrap();
}
struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    config: ServiceConfig,
    runtime: Arc<Runtime>,
    wallet: Arc<FakeWallet>,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let ledger_path = root.join("compute.sqlite");
        private(&ledger_path, b"");
        let mut ledger = Ledger::open(&ledger_path).unwrap();
        ledger
            .create_compute_account("retail-customer", retail::now() as i64)
            .unwrap();
        for (id, spend) in [("buyer", true), ("reader", false)] {
            ledger
                .bind_principal(&Binding {
                    principal: id.into(),
                    account: "retail-customer".into(),
                    kind: PrincipalKind::Cli,
                    credential: credential_digest(id),
                    rights: Rights { read: true, spend },
                    at: retail::now() as i64,
                })
                .unwrap();
            private(&root.join(format!("{id}.bearer")), id.as_bytes());
        }
        private(&root.join("openai.key"), KEY.as_bytes());
        let plan = retail_qualify::qualify::fixture();
        let mut qualification = retail_qualify::qualify::run_fake(&plan);
        qualification.mode = retail_qualify::qualify::Mode::Funded;
        qualification.label = "Synthetic gate record; no production qualification or funds.".into();
        let config = ServiceConfig {
            schema: retail_service::types::SCHEMA.into(),
            state: root.join("service"),
            ledger: ledger_path,
            template: "oa-coder-main-2026-10-07".into(),
            grants: [("buyer", true), ("reader", false)]
                .into_iter()
                .map(|(p, e)| RetailGrant {
                    principal: p.into(),
                    account: "retail-customer".into(),
                    generation: 1,
                    observe: true,
                    execute: e,
                    disclose: e,
                })
                .collect(),
            contract_confirmed: true,
            qualification: Some(qualification),
            supported_plan: plan.digest(),
            plan_starts_left: Some(20),
            environments: None,
        };
        Self {
            _temp: temp,
            root,
            config,
            runtime: Arc::new(Runtime::default()),
            wallet: Arc::new(FakeWallet::new()),
        }
    }
    fn service(&self) -> Arc<Service<Runtime, FakeWallet>> {
        Arc::new(
            Service::open(
                self.config.clone(),
                self.runtime.clone(),
                self.wallet.clone(),
            )
            .unwrap(),
        )
    }
    fn client_config(&self, url: &str, principal: &str, read_only: bool) -> Config {
        Config {
            schema: retail::SCHEMA.into(),
            endpoint: url.into(),
            principal: principal.into(),
            bearer_file: self.root.join(format!("{principal}.bearer")),
            state: self.root.join(format!("client-{principal}-{read_only}")),
            read_only,
            development_loopback: true,
        }
    }
    fn task(&self) -> retail_cloud::contract::TaskRequest {
        serde_json::from_value(json!({"source":{"repository":"https://github.com/OpenAgentsInc/example","commit":"c".repeat(40)},"task":"Fix trailing commas.","checks":["cargo test -p parser"],"max_seconds":600,"ceiling_sats":null})).unwrap()
    }
    fn dispatch(&self, s: &Service<Runtime, FakeWallet>, execution: &str) -> String {
        for n in 0..12 {
            s.tick(retail::now() as i64 + n).unwrap();
            if let Some(spec) = self
                .runtime
                .specs
                .lock()
                .unwrap()
                .get(&retail_cloud::dispatch::task_id(execution))
                .cloned()
            {
                return self.runtime.resources.lock().unwrap()[&spec.task].clone();
            }
        }
        panic!("synthetic run did not dispatch");
    }
}
struct Server {
    url: String,
    lose_confirm: Arc<AtomicBool>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    fn new(service: Arc<Service<Runtime, FakeWallet>>) -> Self {
        Self::at(service, "127.0.0.1:0")
    }
    fn at(service: Arc<Service<Runtime, FakeWallet>>, address: &str) -> Self {
        let listener = std::net::TcpListener::bind(address).unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, signal) = tokio::sync::oneshot::channel();
        let lose_confirm = Arc::new(AtomicBool::new(false));
        let loss = lose_confirm.clone();
        let thread = std::thread::spawn(move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async move {
                    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                    let router = retail_service::http::router(service)
                        .layer(axum::middleware::from_fn_with_state(loss, lose_reply));
                    axum::serve(listener, router)
                        .with_graceful_shutdown(async {
                            let _ = signal.await;
                        })
                        .await
                        .unwrap();
                })
        });
        Self {
            url: format!("http://{address}/v1/retail"),
            lose_confirm,
            stop: Some(stop),
            thread: Some(thread),
        }
    }
}
async fn lose_reply(
    axum::extract::State(flag): axum::extract::State<Arc<AtomicBool>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let (parts, body) = request.into_parts();
    let bytes = axum::body::to_bytes(body, 32 * 1024).await.unwrap();
    let confirm = serde_json::from_slice::<Value>(&bytes).unwrap()["op"] == "confirm";
    let response = next
        .run(axum::http::Request::from_parts(
            parts,
            axum::body::Body::from(bytes),
        ))
        .await;
    if confirm && flag.swap(false, Ordering::SeqCst) {
        (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(json!({"schema":retail_service::types::SCHEMA,"error":"unavailable"})),
        )
            .into_response()
    } else {
        response
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}
fn binary(config: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_retail-client"))
        .arg("--config")
        .arg(config)
        .args(args)
        .env_clear()
        .env("HOME", config.parent().unwrap().join("unused-home"))
        .output()
        .unwrap()
}

#[test]
fn actual_native_client_fake_funding_review_confirm_reconnect_cancel_receipt() {
    let f = Fixture::new();
    let service = f.service();
    let server = Server::new(service.clone());
    let config = f.client_config(&server.url, "buyer", false);
    let config_path = f.root.join("client.json");
    private(&config_path, &serde_json::to_vec(&config).unwrap());
    let task_path = f.root.join("task.json");
    private(&task_path, &serde_json::to_vec(&f.task()).unwrap());
    let key = f.root.join("openai.key");
    let client = Client::from_file(&config_path).unwrap();
    let purchase = client.top_up("purchase-1", 1000).unwrap();
    assert_eq!(client.top_up("purchase-1", 1000).unwrap(), purchase);
    assert_eq!(f.wallet.issued(), 1);
    assert_eq!(client.account().unwrap().balance.credited_msat, 0);
    f.wallet.pay_in_full(&purchase.payment_hash);
    service.tick(retail::now() as i64).unwrap();
    assert_eq!(
        client.top_up_status(&purchase.purchase).unwrap().state,
        "paid"
    );
    assert_eq!(client.account().unwrap().balance.available_msat, 1_000_000);
    drop(client);
    let output = binary(
        &config_path,
        &[
            "quote",
            "--idempotency",
            "run-1",
            "--task",
            task_path.to_str().unwrap(),
            "--provider-key",
            key.to_str().unwrap(),
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    for required in [
        "Public GitHub source",
        "Declared checks",
        "retail-boat-large-v1",
        "Material recipients",
        "Effects:",
        "customer OpenAI key",
        "expense unknown and separate",
        "Sponsored hosted inference: off",
        "maximum",
        "expires at",
        "Service custody",
        "--service-custody",
        "cannot confirm",
    ] {
        assert!(text.contains(required), "{required}: {text}");
    }
    assert!(!text.contains(KEY));
    let digest = text
        .lines()
        .next()
        .unwrap()
        .strip_prefix("Review ")
        .unwrap();
    let refused = binary(
        &config_path,
        &[
            "confirm",
            "--review",
            digest,
            "--provider-key",
            key.to_str().unwrap(),
        ],
    );
    assert!(!refused.status.success());
    assert_eq!(f.runtime.owner.started(), 0);
    let confirmed = binary(
        &config_path,
        &[
            "confirm",
            "--review",
            digest,
            "--provider-key",
            key.to_str().unwrap(),
            "--service-custody",
        ],
    );
    assert!(
        confirmed.status.success(),
        "{}",
        String::from_utf8_lossy(&confirmed.stderr)
    );
    let accepted: retail::Accepted = serde_json::from_slice(&confirmed.stdout).unwrap();
    assert!(accepted.accepted);
    let duplicate = binary(
        &config_path,
        &[
            "confirm",
            "--review",
            digest,
            "--provider-key",
            key.to_str().unwrap(),
            "--service-custody",
        ],
    );
    assert!(duplicate.status.success());
    let duplicate: retail::Accepted = serde_json::from_slice(&duplicate.stdout).unwrap();
    assert_eq!(duplicate, accepted);
    let resource = f.dispatch(&service, &accepted.execution);
    assert_eq!(f.runtime.owner.started(), 1);
    assert_eq!(f.runtime.provider.create_calls(), 1);
    f.runtime.owner.emit(
        &resource,
        &retail_cloud::dispatch::task_id(&accepted.execution),
        "synthetic progress",
    );
    let mut client = Client::from_file(&config_path).unwrap();
    assert_eq!(
        client.progress(&accepted.execution).unwrap().events.len(),
        1
    );
    drop(client);
    let mut client = Client::from_file(&config_path).unwrap();
    assert_eq!(
        client.progress(&accepted.execution).unwrap().events.len(),
        0
    );
    let patch = retail_cloud::sha256_hex(b"synthetic retail patch\n");
    f.runtime.owner.set_status(
        &resource,
        &retail_cloud::dispatch::task_id(&accepted.execution),
        TaskStatus::Ended {
            end: retail_cloud::dispatch::ExecutorEnd::Completed,
            patch: Some(patch.clone()),
            checks: vec![retail_cloud::dispatch::CheckRun {
                command: "cargo test -p parser".into(),
                candidate: patch,
                exit_status: 0,
            }],
        },
    );
    let requested = client.cancel(&accepted.execution).unwrap();
    assert_eq!(requested["cancellation_requested"], true);
    assert_eq!(requested["stopped"], false);
    let receipt = client.receipt(&accepted.execution).unwrap().lines();
    assert!(receipt.contains("Stop requested: true"));
    assert!(receipt.contains("Executor acknowledged: false"));
    assert!(receipt.contains("Sandbox deleted: false"));
    f.runtime.provider.set_usage(&resource, 17);
    service.tick(retail::now() as i64 + 40).unwrap();
    let receipt = client.receipt(&accepted.execution).unwrap();
    assert!(receipt.retention.as_ref().unwrap().deleted());
    assert!(receipt.settlement.as_ref().unwrap().charge_msat.unwrap() > 0);
    assert_eq!(
        receipt.settlement.as_ref().unwrap().checks,
        Some(retail_cloud::dispatch::Verdict::Verified)
    );
    assert!(receipt.lines().contains("not a payment refund"));
    assert!(receipt.lines().contains("model expense: unknown"));
    assert_eq!(
        client.artifact(&accepted.execution, "patch").unwrap()["text"],
        "synthetic retail patch\n"
    );
    assert!(
        !fs::read_to_string(config.state.join("references.json"))
            .unwrap()
            .contains(KEY)
    );
    assert!(!f.root.join("unused-home").exists());
    drop(client);
    let address = server
        .url
        .strip_prefix("http://")
        .unwrap()
        .strip_suffix("/v1/retail")
        .unwrap()
        .to_owned();
    drop(server);
    drop(service);
    // The actual service and client both reopen durable records. Use the same
    // selected endpoint by rebinding its released address, without new spend.
    let service = f.service();
    let _server = Server::at(service.clone(), &address);
    let mut client = Client::from_file(&config_path).unwrap();
    let reopened = client.reconnect(&accepted.execution).unwrap();
    assert_eq!(reopened.execution, accepted.execution);
    let duplicate = client
        .confirm(
            &route_contract::Digest::try_from(digest.to_owned()).unwrap(),
            &key,
            true,
        )
        .unwrap();
    assert_eq!(duplicate.execution, accepted.execution);
    assert!(
        client
            .receipt(&accepted.execution)
            .unwrap()
            .retention
            .unwrap()
            .deleted()
    );
    assert_eq!(f.runtime.owner.started(), 1);
    assert_eq!(f.wallet.issued(), 1);
}

#[test]
fn read_only_remote_rights_and_gateway_keys_never_authorize_retail_spend() {
    let f = Fixture::new();
    let service = f.service();
    let server = Server::new(service.clone());
    let key = f.root.join("openai.key");
    let mut reader = Client::open(f.client_config(&server.url, "reader", false)).unwrap();
    assert!(reader.account().is_ok());
    assert!(reader.top_up("no", 10).is_err());
    assert!(reader.quote("no", f.task(), &key).is_err());
    assert!(reader.cancel("any").is_err());
    let mut observer = Client::open(f.client_config(&server.url, "buyer", true)).unwrap();
    assert!(observer.account().is_ok());
    assert!(observer.top_up("no", 10).is_err());
    assert!(observer.quote("no", f.task(), &key).is_err());
    assert!(observer.cancel("any").is_err());
    let wrong = f.root.join("gateway.key");
    private(&wrong, b"oak_synthetic_not_a_retail_credential");
    let mut config = f.client_config(&server.url, "buyer", false);
    config.bearer_file = wrong;
    config.state = f.root.join("wrong-credential");
    let wrong = Client::open(config).unwrap();
    assert!(matches!(wrong.account(), Err(Error::Service(_))));
    assert_eq!(f.wallet.issued(), 0);
    assert_eq!(f.runtime.owner.started(), 0);
}

#[test]
fn changed_key_generation_terms_and_expired_offers_require_new_review() {
    let f = Fixture::new();
    let service = f.service();
    let server = Server::new(service.clone());
    let key = f.root.join("openai.key");
    let mut client = Client::open(f.client_config(&server.url, "buyer", false)).unwrap();
    let review = client.quote("one", f.task(), &key).unwrap();
    assert!(
        client
            .confirm(
                &route_contract::Digest::of_bytes(b"ordinary approval"),
                &key,
                true
            )
            .is_err()
    );
    let other = f.root.join("other.key");
    private(&other, b"synthetic-replacement");
    assert!(client.confirm(&review.digest, &other, true).is_err());
    let mut changed = f.task();
    changed.source.commit = "d".repeat(40);
    assert!(client.quote("one", changed, &key).is_err());
    // Replace only the private review's offer timestamp and recompute neither
    // digest: corrupted/changed price or source cannot become confirmation.
    drop(client);
    let state_path = f.root.join("client-buyer-false/references.json");
    let mut state: Value = serde_json::from_slice(&fs::read(&state_path).unwrap()).unwrap();
    state["reviews"]["one"]["review"]["offer"]["quote"]["max_sats"] = json!(1);
    fs::write(&state_path, serde_json::to_vec(&state).unwrap()).unwrap();
    let mut client = Client::open(f.client_config(&server.url, "buyer", false)).unwrap();
    assert!(client.confirm(&review.digest, &key, true).is_err());
    assert_eq!(f.runtime.owner.started(), 0);
    Ledger::open(&f.config.ledger)
        .unwrap()
        .revoke_principal("buyer", retail::now() as i64)
        .unwrap();
    assert!(client.confirm(&review.digest, &key, true).is_err());
}

#[test]
fn private_client_and_endpoint_selection_refuse_adoption_or_redirect_authority() {
    let f = Fixture::new();
    let mut largest = f.task();
    largest.task = "x".repeat(16 * 1024);
    largest.checks = vec!["x".repeat(1024); 8];
    assert!(largest.check().is_ok());
    let path = f.root.join("largest-task.json");
    private(&path, &serde_json::to_vec(&largest).unwrap());
    assert_eq!(retail::read_task(&path).unwrap(), largest);
    let mut config = f.client_config("http://192.0.2.1/v1/retail", "buyer", false);
    assert!(Client::open(config.clone()).is_err());
    config.endpoint = "https://buyer:secret@example.invalid/v1/retail".into();
    assert!(Client::open(config.clone()).is_err());
    config.endpoint = "https://example.invalid/v1/retail?credential=hidden".into();
    assert!(Client::open(config.clone()).is_err());
    config.endpoint = "https://example.invalid/v1/retail".into();
    fs::set_permissions(&config.bearer_file, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(Client::open(config.clone()).is_err());
    assert_eq!(
        fs::metadata(&config.bearer_file)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o644
    );
    fs::set_permissions(&config.bearer_file, fs::Permissions::from_mode(0o600)).unwrap();
    let mut client = Client::open(config.clone()).unwrap();
    let lock = config.state.join("client.lock");
    fs::rename(&lock, config.state.join("retired.lock")).unwrap();
    private(&lock, b"");
    assert!(client.account().is_err());
    assert!(client.reconnect("any").is_err());
}

#[test]
fn failed_checks_keep_measured_charge_and_unknown_usage_keeps_the_hold() {
    let f = Fixture::new();
    let service = f.service();
    let server = Server::new(service.clone());
    let key = f.root.join("openai.key");
    let mut client = Client::open(f.client_config(&server.url, "buyer", false)).unwrap();
    let purchase = client.top_up("fund", 1000).unwrap();
    f.wallet.pay_in_full(&purchase.payment_hash);
    service.tick(retail::now() as i64).unwrap();
    let review = client.quote("failed-check", f.task(), &key).unwrap();
    let accepted = client.confirm(&review.digest, &key, true).unwrap();
    let resource = f.dispatch(&service, &accepted.execution);
    let patch = retail_cloud::sha256_hex(b"synthetic retail patch\n");
    f.runtime.owner.set_status(
        &resource,
        &retail_cloud::dispatch::task_id(&accepted.execution),
        TaskStatus::Ended {
            end: retail_cloud::dispatch::ExecutorEnd::Completed,
            patch: Some(patch.clone()),
            checks: vec![retail_cloud::dispatch::CheckRun {
                command: "cargo test -p parser".into(),
                candidate: patch,
                exit_status: 1,
            }],
        },
    );
    f.runtime.provider.set_usage(&resource, 17);
    service.tick(retail::now() as i64 + 40).unwrap();
    let receipt = client.receipt(&accepted.execution).unwrap();
    assert_eq!(
        receipt.settlement.as_ref().unwrap().checks,
        Some(retail_cloud::dispatch::Verdict::CheckFailed)
    );
    assert!(receipt.settlement.as_ref().unwrap().charge_msat.unwrap() > 0);
    assert!(
        receipt
            .lines()
            .contains("measured compute charges still apply")
    );
    let review = client.quote("unknown-usage", f.task(), &key).unwrap();
    let accepted = client.confirm(&review.digest, &key, true).unwrap();
    let resource = f.dispatch(&service, &accepted.execution);
    f.runtime.provider.set_usage_unreadable(true);
    client.cancel(&accepted.execution).unwrap();
    service.tick(retail::now() as i64 + 40).unwrap();
    let receipt = client.receipt(&accepted.execution).unwrap();
    assert!(receipt.retention.as_ref().unwrap().deleted());
    assert!(
        receipt
            .settlement
            .as_ref()
            .is_none_or(|s| s.charge_msat.is_none())
    );
    assert!(receipt.lines().contains("unknown; funds remain held"));
    assert!(client.account().unwrap().balance.held_msat > 0);
    f.runtime.provider.set_usage_unreadable(false);
    f.runtime.provider.set_usage(&resource, 17);
    service.tick(retail::now() as i64 + 41).unwrap();
    assert!(
        client
            .receipt(&accepted.execution)
            .unwrap()
            .settlement
            .unwrap()
            .charge_msat
            .unwrap()
            > 0
    );
}

#[test]
fn expired_server_offer_and_current_spend_rotation_cannot_confirm() {
    let f = Fixture::new();
    let service = f.service();
    let server = Server::new(service.clone());
    let key = f.root.join("openai.key");
    let mut client = Client::open(f.client_config(&server.url, "buyer", false)).unwrap();
    service
        .call(
            "buyer",
            "buyer",
            retail_service::types::Request::Offer {
                idempotency: "expired".into(),
                task: f.task(),
            },
            retail::now() as i64 - 601,
        )
        .unwrap();
    let review = client.quote("expired", f.task(), &key).unwrap();
    assert!(matches!(
        client.confirm(&review.digest, &key, true),
        Err(Error::Refused(_))
    ));
    let review = client.quote("fresh", f.task(), &key).unwrap();
    Ledger::open(&f.config.ledger)
        .unwrap()
        .rotate_principal("buyer", &credential_digest("new-buyer"))
        .unwrap();
    assert!(client.confirm(&review.digest, &key, true).is_err());
    assert_eq!(f.runtime.owner.started(), 0);
}

#[test]
fn lost_confirm_reply_reopens_the_exact_private_intent_and_one_funded_run() {
    let f = Fixture::new();
    let service = f.service();
    let server = Server::new(service.clone());
    let config = f.client_config(&server.url, "buyer", false);
    let key = f.root.join("openai.key");
    let mut client = Client::open(config.clone()).unwrap();
    let purchase = client.top_up("fund", 1000).unwrap();
    f.wallet.pay_in_full(&purchase.payment_hash);
    service.tick(retail::now() as i64).unwrap();
    let review = client.quote("lost", f.task(), &key).unwrap();
    server.lose_confirm.store(true, Ordering::SeqCst);
    assert!(matches!(
        client.confirm(&review.digest, &key, true),
        Err(Error::Service(_))
    ));
    let held = client.account().unwrap().balance.held_msat;
    assert!(held > 0);
    drop(client);
    let bytes = fs::read_to_string(config.state.join("references.json")).unwrap();
    assert!(bytes.contains("\"confirm_intent\":true"));
    assert!(!bytes.contains(KEY));
    let mut client = Client::open(config).unwrap();
    let accepted = client.confirm(&review.digest, &key, true).unwrap();
    assert_eq!(client.account().unwrap().balance.held_msat, held);
    f.dispatch(&service, &accepted.execution);
    assert_eq!(f.runtime.owner.started(), 1);
    assert_eq!(f.runtime.provider.create_calls(), 1);
    assert_eq!(f.wallet.issued(), 1);
}

#[test]
fn installed_client_freezes_reviewed_commercial_attribution_and_refuses_stale_conversion() {
    let f = Fixture::new();
    fs::set_permissions(&f.root, fs::Permissions::from_mode(0o700)).unwrap();
    let canonical = f.root.join("canonical");
    fs::create_dir(&canonical).unwrap();
    fs::set_permissions(&canonical, fs::Permissions::from_mode(0o700)).unwrap();
    let accounts = tenancy::Accounts::install(&canonical).unwrap();
    let customer = accounts
        .create_account("Alice", &["key:aaaaaaaaaaaaaaaa".into()])
        .unwrap();
    let personal = accounts
        .create_workspace(
            &customer.id,
            "Alice",
            tenancy::WorkspaceKind::Personal,
            "fixture",
            None,
        )
        .unwrap();
    let owner = accounts.authorize(&personal.id, &customer.id).unwrap();
    let mut entry = commercial_accounts::Entry {
        operator: "fixture-operator".into(),
        source: tenancy::accounts::commercial::Source {
            product: tenancy::accounts::commercial::Product::Retail,
            issuer: "selected-retail".into(),
            account: "retail-customer".into(),
            workspace: None,
        },
        customer: customer.id.clone(),
        workspace: personal.id.clone(),
        canonical_owner: customer.id.clone(),
        canonical_owner_epoch: owner.epoch,
        canonical_members_epoch: owner.members_epoch,
        principal: "buyer".into(),
        credential_file: f.root.join("buyer.bearer"),
        generation: 1,
        native_owner: None,
        native_owner_epoch: None,
        native_members_epoch: None,
        reviewed_at: retail::now(),
        valid_until: retail::now() + 3600,
        previous_authority: None,
    };
    let policy = f.root.join("commercial-policy.json");
    let write = |entry: &commercial_accounts::Entry| {
        fs::write(
            &policy,
            serde_json::to_vec(&commercial_accounts::Policy {
                schema: commercial_accounts::SCHEMA.into(),
                operator: "fixture-operator".into(),
                entries: vec![entry.clone()],
            })
            .unwrap(),
        )
        .unwrap();
        fs::set_permissions(&policy, fs::Permissions::from_mode(0o600)).unwrap();
    };
    write(&entry);
    let config = retail_service::commercial::Config {
        canonical_directory: canonical.clone(),
        issuer: "selected-retail".into(),
        native: commercial_accounts::Config {
            policy: policy.clone(),
            stores: vec![commercial_accounts::NativeStore::Retail {
                issuer: "selected-retail".into(),
                ledger: f.config.ledger.clone(),
            }],
        },
    };
    let adapter = commercial_accounts::NativeSources::open(&canonical, &config.native).unwrap();
    let original = accounts
        .review_commercial(
            "selected-retail",
            &customer.id,
            &personal.id,
            &customer.id,
            None,
            std::slice::from_ref(&entry.source),
            &adapter,
        )
        .unwrap();
    accounts
        .admit_commercial(&original, &original.digest, &adapter)
        .unwrap();
    let service = Arc::new(
        Service::open(f.config.clone(), f.runtime.clone(), f.wallet.clone())
            .unwrap()
            .with_commercial(config.clone())
            .unwrap(),
    );
    let server = Server::new(service.clone());
    let client_config = f.client_config(&server.url, "buyer", false);
    let path = f.root.join("client-commercial.json");
    private(&path, &serde_json::to_vec(&client_config).unwrap());
    let key = f.root.join("openai.key");
    let mut client = Client::from_file(&path).unwrap();
    let account = client.account().unwrap();
    assert_eq!(account.commercial.as_ref().unwrap().customer, customer.id);
    assert!(account.lines().contains(&customer.id));
    let purchase = client.top_up("commercial-funding", 1000).unwrap();
    assert_eq!(purchase.commercial.as_ref().unwrap().workspace, personal.id);
    assert!(purchase.lines().contains(&personal.id));
    f.wallet.pay_in_full(&purchase.payment_hash);
    service.tick(retail::now() as i64).unwrap();
    let review = client.quote("personal-review", f.task(), &key).unwrap();
    assert_eq!(review.custody.terms["commercial"]["workspace"], personal.id);
    assert!(review.lines().contains(&personal.id));
    assert!(review.lines().starts_with("Review "));
    drop(client);
    let team = accounts
        .create_workspace(
            &customer.id,
            "Team",
            tenancy::WorkspaceKind::Organization,
            "fixture",
            Some(4),
        )
        .unwrap();
    let owner = accounts.authorize(&team.id, &customer.id).unwrap();
    entry.workspace = team.id.clone();
    entry.canonical_owner_epoch = owner.epoch;
    entry.canonical_members_epoch = owner.members_epoch;
    entry.previous_authority = Some(original.sources[0].digest());
    write(&entry);
    let revision = accounts
        .review_commercial(
            "selected-retail",
            &customer.id,
            &team.id,
            &customer.id,
            Some(&customer.id),
            std::slice::from_ref(&entry.source),
            &adapter,
        )
        .unwrap();
    accounts
        .admit_commercial(&revision, &revision.digest, &adapter)
        .unwrap();
    let stale = binary(
        &path,
        &[
            "confirm",
            "--review",
            review.digest.as_str(),
            "--provider-key",
            key.to_str().unwrap(),
            "--service-custody",
        ],
    );
    assert!(!stale.status.success());
    assert_eq!(
        Ledger::open_read_only(&f.config.ledger)
            .unwrap()
            .compute_balance("retail-customer")
            .unwrap()
            .held_msat,
        0
    );
    assert_eq!(f.runtime.provider.create_calls(), 0);
    assert!(
        fs::read_dir(f.config.state.join("credentials"))
            .unwrap()
            .next()
            .is_none()
    );
    let mut client = Client::from_file(&path).unwrap();
    let reviewed_team = client.quote("team-review", f.task(), &key).unwrap();
    let original_funding = client.top_up("commercial-funding", 1000).unwrap();
    assert_eq!(original_funding.commercial, purchase.commercial);
    assert_eq!(original_funding.payment_hash, purchase.payment_hash);
    assert_eq!(f.wallet.issued(), 1);
    assert_eq!(
        reviewed_team.custody.terms["commercial"]["workspace"],
        team.id
    );
    drop(client);
    let accepted = binary(
        &path,
        &[
            "confirm",
            "--review",
            reviewed_team.digest.as_str(),
            "--provider-key",
            key.to_str().unwrap(),
            "--service-custody",
        ],
    );
    assert!(
        accepted.status.success(),
        "{}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    let execution = reviewed_team.offer.admission.execution.clone();
    f.dispatch(&service, &execution);
    assert_eq!(f.runtime.owner.started(), 1);
    let client = Client::from_file(&path).unwrap();
    let frozen = client.execution(&execution).unwrap().commercial.unwrap();
    assert_eq!(frozen.workspace, team.id);
    assert_eq!(frozen.source.account, "retail-customer");
    assert_eq!(
        client.receipt(&execution).unwrap().commercial.unwrap(),
        frozen
    );
    drop(client);
    let bob = accounts
        .create_account("Bob", &["key:bbbbbbbbbbbbbbbb".into()])
        .unwrap();
    let invitation = accounts
        .invite(&customer.id, &team.id, tenancy::Role::Member, 3600)
        .unwrap();
    accounts
        .accept_reviewed(&bob.id, &invitation.token, &team.id, tenancy::Role::Member)
        .unwrap();
    accounts
        .transfer_ownership(&customer.id, &team.id, &bob.id)
        .unwrap();
    accounts
        .remove_member(&bob.id, &team.id, &customer.id)
        .unwrap();
    let mut client = Client::from_file(&path).unwrap();
    assert!(client.quote("revoked", f.task(), &key).is_err());
    assert!(client.top_up("revoked-funding", 1).is_err());
    let funding_history = client.top_up_status(&purchase.purchase).unwrap();
    assert_eq!(funding_history.commercial, purchase.commercial);
    assert_eq!(funding_history.state, "paid");
    drop(client);
    let funding_history = binary(&path, &["top-up-status", &purchase.purchase]);
    assert!(
        funding_history.status.success(),
        "{}",
        String::from_utf8_lossy(&funding_history.stderr)
    );
    let funding_history: retail::Purchase =
        serde_json::from_slice(&funding_history.stdout).unwrap();
    assert_eq!(funding_history.commercial, purchase.commercial);
    let client = Client::from_file(&path).unwrap();
    assert_eq!(
        client.receipt(&execution).unwrap().commercial.unwrap(),
        frozen
    );
    assert_eq!(f.wallet.issued(), 1);
    assert_eq!(f.runtime.owner.started(), 1);
    assert!(!String::from_utf8_lossy(&accepted.stdout).contains(KEY));
}
