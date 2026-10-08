//! Retail delegation acceptance against the actual retail HTTP service with
//! isolated synthetic funds, Boat, task owner, and credentials.

use super::*;
use pay_ledger::Ledger;
use pay_ledger::compute::{Binding, PrincipalKind, Rights, credential_digest};
use retail_cloud::authority::Source;
use retail_cloud::cancel::{StopEvidence, StopOwner};
use retail_cloud::dispatch::{DispatchSpec, OwnerError, TaskEvent, TaskOwner, TaskStatus};
use retail_cloud::fake::{FakeProvider, FakeSandbox, FakeTaskOwner, FakeWallet};
use retail_cloud::material::Sandbox;
use retail_cloud::provision::{CreateSpec, Provider, ProviderError, Resource, ResourceState};
use retail_cloud::retain::{Artifact, Artifacts, Kind, Manifest};
use retail_service::Service;
use retail_service::types::{Config as ServiceConfig, RetailGrant};
use std::sync::atomic::{AtomicBool, Ordering};

const KEY: &str = "synthetic-web-openai-key-never-live";
const PAGE: &str = "/cloud/app/billing/retail";

fn bearer(principal: &str) -> String {
    format!("synthetic-retail-bearer-{principal}")
}

#[derive(Default)]
struct Runtime {
    provider: FakeProvider,
    sandbox: FakeSandbox,
    owner: FakeTaskOwner,
    specs: Mutex<BTreeMap<String, DispatchSpec>>,
    stops: Mutex<BTreeMap<String, StopEvidence>>,
}
impl Provider for Runtime {
    fn create(&self, s: &CreateSpec) -> Result<Resource, ProviderError> {
        self.provider.create(s)
    }
    fn find(&self, id: &str) -> Result<Option<Resource>, ProviderError> {
        self.provider.find(id)
    }
    fn state(&self, id: &str) -> Result<ResourceState, ProviderError> {
        self.provider.state(id)
    }
    fn delete(&self, id: &str) -> Result<(), ProviderError> {
        self.provider.delete(id)
    }
    fn usage_seconds(&self, id: &str) -> Result<Option<u64>, ProviderError> {
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
    fn submit(&self, r: &str, s: &DispatchSpec) -> Result<(), OwnerError> {
        self.specs.lock().unwrap().insert(s.task.clone(), s.clone());
        self.owner.submit(r, s)
    }
    fn status(&self, r: &str, t: &str) -> Result<Option<TaskStatus>, OwnerError> {
        self.owner.status(r, t)
    }
    fn events(&self, r: &str, t: &str, a: u64) -> Result<Vec<TaskEvent>, OwnerError> {
        self.owner.events(r, t, a)
    }
}
impl StopOwner for Runtime {
    fn stop(&self, r: &str, t: &str, id: &str) -> Result<StopEvidence, OwnerError> {
        let mut stops = self.stops.lock().unwrap();
        if let Some(s) = stops.get(id) {
            return Ok(s.clone());
        }
        let started = self.owner.status(r, t)?.is_some();
        self.owner.set_status(r, t, TaskStatus::Cancelled);
        let s = StopEvidence {
            at: now() as i64 + 30,
            started,
            status: TaskStatus::Cancelled,
            effects: vec![],
        };
        stops.insert(id.into(), s.clone());
        Ok(s)
    }
    fn stopped(&self, _: &str, _: &str, id: &str) -> Result<Option<StopEvidence>, OwnerError> {
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
                digest: retail_cloud::sha256_hex(b"patch\n"),
                size: 6,
            }],
        })
    }
    fn read(&self, _: &str, _: &str, _: &str, _: usize) -> retail_cloud::Result<Vec<u8>> {
        Ok(b"patch\n".to_vec())
    }
}

/// The actual retail service, its loopback HTTP listener, and the site's
/// private delegation directory.
struct Retail {
    _temp: tempfile::TempDir,
    root: PathBuf,
    service: Arc<Service<Runtime, FakeWallet>>,
    runtime: Arc<Runtime>,
    wallet: Arc<FakeWallet>,
    url: String,
    lose_confirm: Arc<AtomicBool>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Retail {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

fn private_dir(path: &std::path::Path) {
    std::fs::create_dir_all(path).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
}

fn retail() -> Retail {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("retail");
    private_dir(&root);
    let ledger_path = root.join("compute.sqlite");
    private_file(&ledger_path, b"");
    let mut ledger = Ledger::open(&ledger_path).unwrap();
    ledger
        .create_compute_account("retail-customer", now() as i64)
        .unwrap();
    for (id, spend) in [("buyer", true), ("reader", false)] {
        ledger
            .bind_principal(&Binding {
                principal: id.into(),
                account: "retail-customer".into(),
                kind: PrincipalKind::Cli,
                credential: credential_digest(&bearer(id)),
                rights: Rights { read: true, spend },
                at: now() as i64,
            })
            .unwrap();
        private_dir(&root.join(id));
        private_file(&root.join(id).join("bearer"), bearer(id).as_bytes());
    }
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
    };
    let runtime = Arc::new(Runtime::default());
    let wallet = Arc::new(FakeWallet::new());
    let service = Arc::new(Service::open(config, runtime.clone(), wallet.clone()).unwrap());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, signal) = tokio::sync::oneshot::channel();
    let lose_confirm = Arc::new(AtomicBool::new(false));
    let loss = lose_confirm.clone();
    let served = service.clone();
    let thread = std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                let router = retail_service::http::router(served)
                    .layer(axum::middleware::from_fn_with_state(loss, lose_reply));
                axum::serve(listener, router)
                    .with_graceful_shutdown(async {
                        let _ = signal.await;
                    })
                    .await
                    .unwrap();
            })
    });
    Retail {
        _temp: temp,
        root,
        service,
        runtime,
        wallet,
        url: format!("http://{address}/v1/retail"),
        lose_confirm,
        stop: Some(stop),
        thread: Some(thread),
    }
}

/// Process a confirmation and then lose its reply, as a dropped connection would.
async fn lose_reply(
    State(flag): State<Arc<AtomicBool>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let (parts, body) = request.into_parts();
    let bytes = to_bytes(body, 32 * 1024).await.unwrap();
    let confirm = serde_json::from_slice::<Value>(&bytes).unwrap()["op"] == "confirm";
    let response = next
        .run(Request::from_parts(parts, Body::from(bytes)))
        .await;
    if confirm && flag.swap(false, Ordering::SeqCst) {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"schema":retail_service::types::SCHEMA,"error":"unavailable"})),
        )
            .into_response()
    } else {
        response
    }
}

impl Retail {
    /// Write the site's delegation configuration and attach it.
    fn attach(&self, fixture: &mut Fixture, delegations: &[(&str, &str, &str, u64, &str, bool)]) {
        let site = self.root.join("site");
        private_dir(&site);
        let mut declared = Vec::new();
        for (id, account, workspace, epoch, principal, read_only) in delegations {
            let client = json!({
                "schema":"openagents.compute-retail-client.v1","endpoint":self.url,
                "principal":principal,"bearer_file":self.root.join(principal).join("bearer"),
                "state":self.root.join(format!("state-{id}")),"read_only":false,
                "development_loopback":true
            });
            let path = self.root.join(principal).join(format!("{id}.json"));
            private_file(&path, &serde_json::to_vec(&client).unwrap());
            declared.push(json!({"id":id,"account":account,"workspace":workspace,"members_epoch":epoch,"client":path,"read_only":read_only}));
        }
        let path = self.root.join("delegations.json");
        private_file(
            &path,
            &serde_json::to_vec(&json!({"schema":super::super::retail::SCHEMA,"directory":site,"delegations":declared})).unwrap(),
        );
        self.reload(fixture);
    }

    /// A site restart: a new process loads the same configuration.
    fn reload(&self, fixture: &mut Fixture) {
        fixture.config.cloud_retail = Some(Arc::new(
            super::super::retail::Delegations::load(&self.root.join("delegations.json")).unwrap(),
        ));
        fixture.site = crate::router(fixture.config.clone());
    }

    fn fund(&self, purchase: &Value) {
        self.wallet
            .pay_in_full(purchase["payment_hash"].as_str().unwrap());
        self.service.tick(now() as i64).unwrap();
    }

    fn dispatch(&self, execution: &str) {
        for n in 0..12 {
            self.service.tick(now() as i64 + n).unwrap();
            if self
                .runtime
                .specs
                .lock()
                .unwrap()
                .contains_key(&retail_cloud::dispatch::task_id(execution))
            {
                return;
            }
        }
        panic!("synthetic run did not dispatch");
    }
}

async fn get(fixture: &Fixture, cookies: &Cookies, path: &str) -> Answer {
    request(&fixture.site, Method::GET, path, cookies, None, None).await
}

async fn post(fixture: &Fixture, cookies: &Cookies, path: &str, fields: &[(&str, &str)]) -> Answer {
    request(
        &fixture.site,
        Method::POST,
        path,
        cookies,
        Some(&form(fields)),
        Some(ORIGIN),
    )
    .await
}

/// The hidden fields of the first form posting to `action`.
fn hidden(html: &str, action: &str) -> Vec<(String, String)> {
    let form = html
        .split("<form ")
        .skip(1)
        .find(|form| form.contains(&format!("action=\"{action}\"")))
        .unwrap_or_else(|| panic!("no form for {action}"))
        .split("</form>")
        .next()
        .unwrap();
    form.split("<input type=\"hidden\" name=\"")
        .skip(1)
        .map(|part| {
            let (name, rest) = part.split_once("\" value=\"").unwrap();
            (
                name.into(),
                rest.split('"').next().unwrap().replace("&amp;", "&"),
            )
        })
        .collect()
}

async fn submit(
    fixture: &Fixture,
    cookies: &Cookies,
    page: &str,
    action: &str,
    extra: &[(&str, &str)],
) -> (Answer, Vec<(String, String)>) {
    let fields = hidden(page, action);
    let mut all: Vec<(&str, &str)> = fields
        .iter()
        .map(|(n, v)| (n.as_str(), v.as_str()))
        .collect();
    all.extend_from_slice(extra);
    (post(fixture, cookies, action, &all).await, fields)
}

fn no_secret(text: &str) {
    for value in [KEY, &bearer("buyer"), &bearer("reader")] {
        assert!(!text.contains(value), "disclosed a credential");
    }
}

async fn viewer(
    fixture: &Fixture,
    cookies: &Cookies,
) -> (HeaderMap, super::super::session::Viewer) {
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, HOST.parse().unwrap());
    headers.insert(header::COOKIE, cookies.header().parse().unwrap());
    let viewer = fixture
        .config
        .cloud
        .as_ref()
        .unwrap()
        .authenticate(&headers)
        .await
        .unwrap();
    (headers, viewer)
}

const ALICE: (&str, &str, &str, u64, &str, bool) =
    ("alice-retail", "alice", "alice-personal", 3, "buyer", false);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn delegation_refuses_other_accounts_workspaces_and_epochs() {
    let mut fixture = fixture().await;
    let retail = retail();
    retail.attach(&mut fixture, &[ALICE]);
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    let page = get(&fixture, &cookies, PAGE).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    private(&page);
    no_secret(&page.body);
    assert!(page.body.contains("Delegation alice-retail"));
    assert!(page.body.contains("Account retail-customer"));
    assert!(page.body.contains("href=\"/cloud/app/billing/retail\""));
    let top_up = format!("{PAGE}/alice-retail/top-up");
    let fields = hidden(&page.body, &top_up);

    // Another account sees no delegation and cannot use Alice's.
    let bob = login(&fixture, "bob").await;
    let other = get(&fixture, &bob, PAGE).await;
    assert!(other.body.contains("no retail delegation is provisioned"));
    let mut all: Vec<(&str, &str)> = fields
        .iter()
        .map(|(n, v)| (n.as_str(), v.as_str()))
        .collect();
    all.push(("amount_sats", "1000"));
    assert_eq!(
        post(&fixture, &bob, &top_up, &all).await.status,
        StatusCode::FORBIDDEN
    );

    // A membership epoch change fences the delegation and its old tickets.
    fixture.state.lock().unwrap().epoch = 4;
    let fenced = post(&fixture, &cookies, &top_up, &all).await;
    assert_eq!(fenced.status, StatusCode::FORBIDDEN, "{}", fenced.body);
    assert!(
        get(&fixture, &cookies, PAGE)
            .await
            .body
            .contains("no retail delegation is provisioned")
    );
    assert_eq!(retail.wallet.issued(), 0);

    // The service's own browser guard is unchanged.
    let direct = reqwest::Client::new()
        .post(&retail.url)
        .header("origin", ORIGIN)
        .bearer_auth(bearer("buyer"))
        .header("x-retail-principal", "buyer")
        .body(r#"{"op":"account"}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(direct.status(), 403);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn observation_only_scopes_cannot_spend_dispatch_or_cancel() {
    let mut fixture = fixture().await;
    let retail = retail();
    retail.attach(
        &mut fixture,
        &[
            ("narrowed", "alice", "alice-personal", 3, "buyer", true),
            ("reader", "alice", "alice-personal", 3, "reader", false),
        ],
    );
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    let page = get(&fixture, &cookies, PAGE).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(page.body.contains("Observation only"));
    for route in [
        "narrowed/top-up",
        "narrowed/quote",
        "narrowed/cancel",
        "narrowed/key",
        "reader/top-up",
        "reader/quote",
        "reader/cancel",
    ] {
        assert!(
            !page.body.contains(&format!("action=\"{PAGE}/{route}\"")),
            "{route}"
        );
    }
    // Even a correctly minted ticket cannot reach a native effect.
    let (headers, viewer) = viewer(&fixture, &cookies).await;
    let session = fixture.config.cloud.clone().unwrap();
    let retail_config = fixture.config.cloud_retail.clone().unwrap();
    for (id, route, extra) in [
        ("narrowed", "top-up", ("amount_sats", "1000")),
        ("narrowed", "cancel", ("execution", "exec-1")),
        ("reader", "top-up", ("amount_sats", "1000")),
        ("reader", "cancel", ("execution", "exec-1")),
    ] {
        let delegation = retail_config.get(&viewer, id).unwrap();
        let request = "ab".repeat(16);
        let identity = super::super::retail::tests_identity(delegation);
        let csrf = session
            .csrf(
                &headers,
                &viewer,
                &format!("retail-{route}"),
                &format!("{identity}:{request}"),
            )
            .unwrap();
        let answer = post(
            &fixture,
            &cookies,
            &format!("{PAGE}/{id}/{route}"),
            &[("csrf", &csrf), ("request", &request), extra],
        )
        .await;
        assert!(
            matches!(answer.status, StatusCode::FORBIDDEN | StatusCode::CONFLICT),
            "{id}/{route}: {} {}",
            answer.status,
            answer.body
        );
    }
    assert_eq!(retail.wallet.issued(), 0);
    assert_eq!(retail.runtime.owner.started(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn custody_retries_and_restart_keep_one_purchase_and_one_dispatch() {
    let mut fixture = fixture().await;
    let retail = retail();
    retail.attach(&mut fixture, &[ALICE]);
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    let base = format!("{PAGE}/alice-retail");

    // Funding: an exact retry recovers the original invoice; changed bytes conflict.
    let page = get(&fixture, &cookies, PAGE).await;
    let (funded, fields) = submit(
        &fixture,
        &cookies,
        &page.body,
        &format!("{base}/top-up"),
        &[("amount_sats", "1000")],
    )
    .await;
    assert_eq!(funded.status, StatusCode::SEE_OTHER, "{}", funded.body);
    let mut again: Vec<(&str, &str)> = fields
        .iter()
        .map(|(n, v)| (n.as_str(), v.as_str()))
        .collect();
    again.push(("amount_sats", "1000"));
    assert_eq!(
        post(&fixture, &cookies, &format!("{base}/top-up"), &again)
            .await
            .status,
        StatusCode::SEE_OTHER
    );
    again.pop();
    again.push(("amount_sats", "2000"));
    assert_eq!(
        post(&fixture, &cookies, &format!("{base}/top-up"), &again)
            .await
            .status,
        StatusCode::CONFLICT
    );
    assert_eq!(retail.wallet.issued(), 1);
    let site = retail.root.join("site");
    let journal = std::fs::read_dir(site.join("requests"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let recorded: Value = serde_json::from_slice(&std::fs::read(&journal).unwrap()).unwrap();
    let purchase = recorded["records"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap()["outcome"]
        .clone();
    retail.fund(&purchase);

    // Custody needs consent, refuses Claude logins, and shows only a digest.
    let page = get(&fixture, &cookies, PAGE).await;
    assert!(page.body.contains("No key in custody"));
    let (refused, _) = submit(
        &fixture,
        &cookies,
        &page.body,
        &format!("{base}/key"),
        &[("key", KEY)],
    )
    .await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    let (refused, _) = submit(
        &fixture,
        &cookies,
        &page.body,
        &format!("{base}/key"),
        &[("key", "sk-ant-oat01-synthetic"), ("consent", "custody")],
    )
    .await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    no_secret(&refused.body);
    let (stored, _) = submit(
        &fixture,
        &cookies,
        &page.body,
        &format!("{base}/key"),
        &[("key", KEY), ("consent", "custody")],
    )
    .await;
    assert_eq!(stored.status, StatusCode::SEE_OTHER, "{}", stored.body);
    no_secret(&stored.body);
    let page = get(&fixture, &cookies, PAGE).await;
    no_secret(&page.body);
    assert!(page.body.contains("In custody: your own OpenAI API key"));
    assert!(
        page.body.contains("Available 1000.000 credits"),
        "{}",
        page.body
    );

    // Quote through the reviewed custody key, then lose the confirmation reply.
    let (quoted, _) = submit(
        &fixture,
        &cookies,
        &page.body,
        &format!("{base}/quote"),
        &[
            ("repository", "https://github.com/OpenAgentsInc/example"),
            ("commit", &"c".repeat(40)),
            ("task", "Fix trailing commas."),
            ("checks", "cargo test -p parser"),
            ("max_seconds", "600"),
        ],
    )
    .await;
    assert_eq!(quoted.status, StatusCode::SEE_OTHER, "{}", quoted.body);
    let page = get(&fixture, &cookies, PAGE).await;
    no_secret(&page.body);
    for line in [
        "Public GitHub source",
        "Sponsored hosted inference: off",
        "Customer OpenAI key SHA-256",
    ] {
        assert!(page.body.contains(line), "{line}");
    }
    let confirm = format!("{base}/confirm");
    let (missing, _) = submit(&fixture, &cookies, &page.body, &confirm, &[]).await;
    assert_eq!(missing.status, StatusCode::BAD_REQUEST);
    retail.lose_confirm.store(true, Ordering::SeqCst);
    let (lost, fields) = submit(
        &fixture,
        &cookies,
        &page.body,
        &confirm,
        &[("custody", "service")],
    )
    .await;
    assert_eq!(
        lost.status,
        StatusCode::SERVICE_UNAVAILABLE,
        "{}",
        lost.body
    );
    let held = {
        let page = get(&fixture, &cookies, PAGE).await;
        assert!(page.body.contains("Outcome unknown"));
        assert!(page.body.contains("Retry the same request"));
        page.body
    };
    assert!(!held.contains("Held 0.000 credits"), "{held}");

    // Restart the site: the same request recovers the one funded execution.
    retail.reload(&mut fixture);
    let retry: Vec<(&str, &str)> = fields
        .iter()
        .map(|(n, v)| (n.as_str(), v.as_str()))
        .chain([("custody", "service")])
        .collect();
    let recovered = post(&fixture, &cookies, &confirm, &retry).await;
    assert_eq!(
        recovered.status,
        StatusCode::SEE_OTHER,
        "{}",
        recovered.body
    );
    assert_eq!(
        post(&fixture, &cookies, &confirm, &retry).await.status,
        StatusCode::SEE_OTHER
    );
    let recorded: Value = serde_json::from_slice(&std::fs::read(&journal).unwrap()).unwrap();
    let accepted = recorded["records"]
        .as_object()
        .unwrap()
        .values()
        .find(|record| record["op"] == "confirm")
        .unwrap()["outcome"]
        .clone();
    let execution = accepted["execution"].as_str().unwrap().to_owned();
    retail.dispatch(&execution);
    assert_eq!(retail.runtime.owner.started(), 1);
    assert_eq!(retail.runtime.provider.create_calls(), 1);
    assert_eq!(retail.wallet.issued(), 1);

    // Cancellation is a durable stop request under current execution rights.
    let page = get(&fixture, &cookies, PAGE).await;
    let (stopped, _) = submit(
        &fixture,
        &cookies,
        &page.body,
        &format!("{base}/cancel"),
        &[("execution", &execution)],
    )
    .await;
    assert_eq!(stopped.status, StatusCode::SEE_OTHER, "{}", stopped.body);

    // Removing the key is immediate; the key never reached private site files.
    let page = get(&fixture, &cookies, PAGE).await;
    let (removed, _) = submit(
        &fixture,
        &cookies,
        &page.body,
        &format!("{base}/key/remove"),
        &[],
    )
    .await;
    assert_eq!(removed.status, StatusCode::SEE_OTHER);
    assert!(
        get(&fixture, &cookies, PAGE)
            .await
            .body
            .contains("No key in custody")
    );
    for entry in walk(&retail.root) {
        if entry.starts_with(retail.root.join("service")) {
            continue;
        }
        let bytes = std::fs::read(&entry).unwrap();
        assert!(
            !String::from_utf8_lossy(&bytes).contains(KEY),
            "{} retained the key",
            entry.display()
        );
    }
}

fn walk(root: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else {
            out.push(path);
        }
    }
    out
}
