use super::*;
use policy::{Host, Method};
use proptest::prelude::*;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};

struct Memory(Mutex<Binding>);
#[async_trait]
impl Store for Memory {
    async fn get(&self, _: Uuid) -> Result<Option<Binding>, Failure> {
        Ok(Some(self.0.lock().unwrap().clone()))
    }
}
struct Auth {
    current: Mutex<Current>,
    unavailable: AtomicBool,
}
#[async_trait]
impl Authorizer for Auth {
    async fn current(&self, _: &Binding) -> Result<Current, Failure> {
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(Failure::Unavailable);
        }
        Ok(self.current.lock().unwrap().clone())
    }
}
struct Provider {
    reads: AtomicUsize,
    unavailable: AtomicBool,
    revoke_during_read: AtomicBool,
    auth: Arc<Auth>,
}
#[async_trait]
impl SecretProvider for Provider {
    async fn access(&self, _: &secret_manager::Version) -> Result<Secret, Failure> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(Failure::Unavailable);
        }
        if self.revoke_during_read.load(Ordering::SeqCst) {
            self.auth.current.lock().unwrap().grant_active = false;
        }
        Secret::new(b"synthetic-canary".to_vec())
    }
}
struct Time(AtomicU64);
impl Clock for Time {
    fn now_unix(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}
struct F {
    store: Arc<Memory>,
    auth: Arc<Auth>,
    provider: Arc<Provider>,
    clock: Arc<Time>,
    source: Source,
    request: Use,
}
impl F {
    fn new() -> Self {
        let host = Host::parse("api.example.com").unwrap();
        let profile = Profile::new("model-v1", [host.clone()].into());
        let source = Source {
            id: Uuid::from_u128(1),
            revision: 3,
            owner: Owner::Account {
                id: "account:owner".into(),
            },
            secret_version: secret_manager::Version::parse("projects/123/secrets/model/versions/7")
                .unwrap(),
            rules: [(
                "messages".into(),
                Rule {
                    destination: Destination {
                        host,
                        port: 443,
                        path_prefix: "/v1/messages".into(),
                        methods: [Method::Post].into(),
                    },
                    header: "authorization".into(),
                    prefix: "Bearer ".into(),
                    encoding: Encoding::Plain,
                },
            )]
            .into(),
        };
        let b = Binding {
            id: Uuid::from_u128(2),
            revision: 1,
            state: State::Active,
            source: source.id,
            source_revision: 3,
            source_digest: source.digest().unwrap(),
            rule: "messages".into(),
            owner: "account:owner".into(),
            workspace: Some(Workspace {
                id: "workspace".into(),
                members_epoch: 7,
            }),
            grant_revision: 5,
            computer: "cmp_computer".into(),
            native_resource: "sbx_native".into(),
            node: "node-one".into(),
            execution_generation: 3,
            native_policy_ref: "npv1:fixed".into(),
            command_key: "request-one".into(),
            command_digest: Some("a".repeat(64)),
            profile: profile.clone(),
            created_unix: 1000,
            expires_unix: 1600,
        };
        let current = Current {
            owner: b.owner.clone(),
            workspace: b.workspace.clone(),
            computer: b.computer.clone(),
            native_resource: b.native_resource.clone(),
            node: b.node.clone(),
            execution_generation: b.execution_generation,
            native_policy_ref: b.native_policy_ref.clone(),
            command_key: b.command_key.clone(),
            command_digest: b.command_digest.clone().unwrap(),
            policy_digest: profile.digest().unwrap(),
            source_revision: 3,
            grant_revision: 5,
            running: true,
            grant_active: true,
        };
        let request = Use {
            reference: b.id,
            native_resource: b.native_resource.clone(),
            node: b.node.clone(),
            execution_generation: 3,
            native_policy_ref: b.native_policy_ref.clone(),
            host: "api.example.com".into(),
            port: 443,
            path: "/v1/messages".into(),
            method: "POST".into(),
            tls_inspected: true,
        };
        let auth = Arc::new(Auth {
            current: Mutex::new(current),
            unavailable: AtomicBool::new(false),
        });
        let provider = Arc::new(Provider {
            reads: AtomicUsize::new(0),
            unavailable: AtomicBool::new(false),
            revoke_during_read: AtomicBool::new(false),
            auth: auth.clone(),
        });
        Self {
            store: Arc::new(Memory(Mutex::new(b))),
            auth,
            provider,
            clock: Arc::new(Time(AtomicU64::new(1100))),
            source,
            request,
        }
    }
    fn broker(&self) -> Broker {
        Broker::new(
            self.store.clone(),
            self.auth.clone(),
            self.provider.clone(),
            self.clock.clone(),
            vec![self.source.clone()],
            Secret::new(vec![29; 32]).unwrap(),
        )
        .unwrap()
    }
    fn reads(&self) -> usize {
        self.provider.reads.load(Ordering::SeqCst)
    }
}
#[tokio::test]
async fn only_the_host_token_and_current_exact_scope_can_read_a_value() {
    let f = F::new();
    let b = f.broker();
    let token = b.proxy_token(f.request.reference).await.unwrap();
    assert!(!format!("{token:?}").contains(token.expose()));
    assert_eq!(
        b.resolve("", &f.request).await.unwrap_err(),
        Failure::Denied
    );
    assert_eq!(f.reads(), 0);
    let (name, value) = b.resolve(token.expose(), &f.request).await.unwrap();
    assert_eq!(name, "authorization");
    assert_eq!(value.bytes(), b"Bearer synthetic-canary");
    assert!(!format!("{value:?}").contains("synthetic-canary"));
    assert_eq!(f.reads(), 1);
    b.check(token.expose(), &f.request).await.unwrap();
    assert_eq!(f.reads(), 1);
}
#[tokio::test]
async fn another_computer_generation_node_policy_method_path_or_port_never_reads() {
    let f = F::new();
    let b = f.broker();
    let token = b.proxy_token(f.request.reference).await.unwrap();
    let mut requests = vec![];
    let mut r = f.request.clone();
    r.native_resource = "sbx_other".into();
    requests.push(r);
    let mut r = f.request.clone();
    r.node = "node-other".into();
    requests.push(r);
    let mut r = f.request.clone();
    r.execution_generation += 1;
    requests.push(r);
    let mut r = f.request.clone();
    r.native_policy_ref = "npv1:other".into();
    requests.push(r);
    let mut r = f.request.clone();
    r.host = "api.example.com.evil.test".into();
    requests.push(r);
    let mut r = f.request.clone();
    r.port = 8443;
    requests.push(r);
    let mut r = f.request.clone();
    r.method = "GET".into();
    requests.push(r);
    let mut r = f.request.clone();
    r.path = "/v1/messages-elsewhere".into();
    requests.push(r);
    let mut r = f.request.clone();
    r.tls_inspected = false;
    requests.push(r);
    let mut r = f.request.clone();
    r.reference = Uuid::from_u128(3);
    requests.push(r);
    for r in requests {
        assert_eq!(
            b.resolve(token.expose(), &r).await.unwrap_err(),
            Failure::Denied
        );
    }
    assert_eq!(f.reads(), 0);
}
#[tokio::test]
async fn expiry_revocation_workspace_changes_and_command_exit_deny_before_access() {
    for change in 0..8 {
        let f = F::new();
        let b = f.broker();
        let token = b.proxy_token(f.request.reference).await.unwrap();
        match change {
            0 => f.clock.0.store(1600, Ordering::SeqCst),
            1 => f.store.0.lock().unwrap().state = State::Revoked,
            2 => {
                f.auth
                    .current
                    .lock()
                    .unwrap()
                    .workspace
                    .as_mut()
                    .unwrap()
                    .members_epoch += 1
            }
            3 => f.auth.current.lock().unwrap().running = false,
            4 => f.auth.current.lock().unwrap().grant_active = false,
            5 => f.auth.current.lock().unwrap().grant_revision += 1,
            6 => f.auth.current.lock().unwrap().command_key = "another-turn".into(),
            _ => f.auth.current.lock().unwrap().command_digest = "b".repeat(64),
        }
        assert_eq!(
            b.resolve(token.expose(), &f.request).await.unwrap_err(),
            Failure::Denied
        );
        assert_eq!(f.reads(), 0);
    }
}
#[tokio::test]
async fn outages_and_revocation_during_provider_io_never_return_material() {
    let f = F::new();
    let b = f.broker();
    let token = b.proxy_token(f.request.reference).await.unwrap();
    f.auth.unavailable.store(true, Ordering::SeqCst);
    assert_eq!(
        b.resolve(token.expose(), &f.request).await.unwrap_err(),
        Failure::Unavailable
    );
    assert_eq!(f.reads(), 0);
    f.auth.unavailable.store(false, Ordering::SeqCst);
    f.provider.unavailable.store(true, Ordering::SeqCst);
    assert_eq!(
        b.resolve(token.expose(), &f.request).await.unwrap_err(),
        Failure::Unavailable
    );
    f.provider.unavailable.store(false, Ordering::SeqCst);
    f.provider.revoke_during_read.store(true, Ordering::SeqCst);
    assert_eq!(
        b.resolve(token.expose(), &f.request).await.unwrap_err(),
        Failure::Denied
    );
}
#[tokio::test]
async fn source_and_signing_key_rotation_cannot_relabel_old_authority() {
    let mut f = F::new();
    let old = f.broker();
    let token = old.proxy_token(f.request.reference).await.unwrap();
    f.source.revision += 1;
    let updated = f.broker();
    assert_eq!(
        updated
            .resolve(token.expose(), &f.request)
            .await
            .unwrap_err(),
        Failure::Denied
    );
    f.source.revision -= 1;
    let rotated = Broker::new(
        f.store.clone(),
        f.auth.clone(),
        f.provider.clone(),
        f.clock.clone(),
        vec![f.source.clone()],
        Secret::new(vec![30; 32]).unwrap(),
    )
    .unwrap();
    assert_eq!(
        rotated
            .resolve(token.expose(), &f.request)
            .await
            .unwrap_err(),
        Failure::Denied
    );
    assert_eq!(f.reads(), 0);
}
#[tokio::test]
async fn changing_a_source_without_bumping_its_revision_still_invalidates_the_handle() {
    let mut f = F::new();
    let original = f.broker();
    let token = original.proxy_token(f.request.reference).await.unwrap();
    f.source.secret_version =
        secret_manager::Version::parse("projects/123/secrets/model/versions/8").unwrap();
    let changed = f.broker();
    assert_eq!(
        changed
            .resolve(token.expose(), &f.request)
            .await
            .unwrap_err(),
        Failure::Denied
    );
    assert_eq!(f.reads(), 0);
}
#[tokio::test]
async fn interception_exemption_and_prepared_handles_carry_no_secret_authority() {
    let f = F::new();
    let b = f.broker();
    f.store.0.lock().unwrap().state = State::Prepared;
    let token = b.proxy_token(f.request.reference).await.unwrap();
    assert_eq!(
        b.resolve(token.expose(), &f.request).await.unwrap_err(),
        Failure::Denied
    );
    {
        let mut binding = f.store.0.lock().unwrap();
        binding.state = State::Active;
        binding
            .profile
            .tls_exempt_hosts
            .insert(Host::parse("api.example.com").unwrap());
    }
    let token = b.proxy_token(f.request.reference).await.unwrap();
    assert_eq!(
        b.resolve(token.expose(), &f.request).await.unwrap_err(),
        Failure::Denied
    );
    assert_eq!(f.reads(), 0);
}
#[test]
fn public_use_requests_cannot_smuggle_an_owner_or_secret_path() {
    let f = F::new();
    let mut value = serde_json::to_value(f.request).unwrap();
    value["owner"] = serde_json::json!("account:another");
    assert!(serde_json::from_value::<Use>(value).is_err());
    assert!(
        serde_json::from_value::<Uuid>(serde_json::json!("projects/123/secrets/other/versions/1"))
            .is_err()
    );
}
proptest! {
    #[test]
    fn a_reference_never_resolves_for_another_owner(suffix in "[a-zA-Z0-9]{1,40}") {
        let runtime=tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            let f=F::new();let b=f.broker();let token=b.proxy_token(f.request.reference).await.unwrap();
            f.auth.current.lock().unwrap().owner=format!("account:other-{suffix}");
            assert_eq!(b.resolve(token.expose(),&f.request).await.unwrap_err(),Failure::Denied);
            assert_eq!(f.reads(),0);
        });
    }
}

struct GatewayAuthority {
    memory: Arc<Memory>,
    unavailable: AtomicBool,
}
#[async_trait]
impl gateway::Authority for GatewayAuthority {
    async fn admit(&self, _: &gateway::Context) -> Result<gateway::Admission, Failure> {
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(Failure::Unavailable);
        }
        let b = self.memory.0.lock().unwrap();
        Ok(gateway::Admission {
            profile: b.profile.clone(),
            references: vec![b.id],
        })
    }
}
#[tokio::test]
async fn a_real_http_adapter_checks_node_command_grant_and_each_request() {
    let f = F::new();
    let authority = Arc::new(GatewayAuthority {
        memory: f.store.clone(),
        unavailable: AtomicBool::new(false),
    });
    let token = "synthetic-node-token-for-host-only-testing";
    let service = Arc::new(gateway::Service {
        broker: Arc::new(f.broker()),
        authority: authority.clone(),
        nodes: [(Sha256::digest(token.as_bytes()).into(), "node-one".into())].into(),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, service.router()).await.unwrap();
    });
    let context = gateway::Context {
        native_resource: f.request.native_resource.clone(),
        node: f.request.node.clone(),
        execution_generation: f.request.execution_generation,
        native_policy_ref: f.request.native_policy_ref.clone(),
    };
    let client = gateway::Client::local(
        &format!("http://{address}"),
        Secret::new(token.as_bytes().to_vec()).unwrap(),
        context.clone(),
    )
    .unwrap();
    let policy = &context.native_policy_ref;
    let selectors = [("authorization".into(), f.request.reference)].into();
    client.check(policy).await.unwrap();
    assert_eq!(f.reads(), 0);
    // Git may make an anonymous request before answering its Basic challenge.
    assert!(
        client
            .headers(
                policy,
                "api.example.com",
                443,
                "/v1/messages",
                "POST",
                &BTreeMap::new()
            )
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(f.reads(), 0);
    let headers = client
        .headers(
            policy,
            "api.example.com",
            443,
            "/v1/messages",
            "POST",
            &selectors,
        )
        .await
        .unwrap();
    assert_eq!(headers.len(), 1);
    assert_eq!(headers[0].value, "Bearer synthetic-canary");
    assert!(!format!("{headers:?}").contains("synthetic-canary"));
    assert_eq!(f.reads(), 1);
    for (host, port, path) in [
        ("api.example.com", 8443, "/v1/messages"),
        ("api.example.com", 443, "/public"),
        ("api.example.com.evil.test", 443, "/v1/messages"),
    ] {
        assert!(
            client
                .headers(policy, host, port, path, "POST", &selectors)
                .await
                .is_err()
        );
    }
    let wrong_header = [("x-api-key".into(), f.request.reference)].into();
    assert!(
        client
            .headers(
                policy,
                "api.example.com",
                443,
                "/v1/messages",
                "POST",
                &wrong_header
            )
            .await
            .is_err()
    );
    assert_eq!(f.reads(), 1);
    let wrong = gateway::Client::local(
        &format!("http://{address}"),
        Secret::new(vec![b'z'; 48]).unwrap(),
        context.clone(),
    )
    .unwrap();
    assert!(wrong.check(policy).await.is_err());
    let mut wrong_context = context.clone();
    wrong_context.execution_generation += 1;
    let wrong = gateway::Client::local(
        &format!("http://{address}"),
        Secret::new(token.as_bytes().to_vec()).unwrap(),
        wrong_context,
    )
    .unwrap();
    assert!(wrong.check(policy).await.is_err());
    let old_scope = client.check_scope(policy, None).await.unwrap();
    f.store.0.lock().unwrap().id = Uuid::from_u128(333);
    // Neither a new connection with an old marker nor an old connection with
    // a replacement grant can exchange that old reference for a current key.
    assert!(
        client
            .headers(
                policy,
                "api.example.com",
                443,
                "/v1/messages",
                "POST",
                &selectors
            )
            .await
            .is_err()
    );
    assert!(client.check_scope(policy, Some(&old_scope)).await.is_err());
    assert!(
        client
            .headers_scoped(
                policy,
                "api.example.com",
                443,
                "/v1/messages",
                "POST",
                &selectors,
                Some(&old_scope)
            )
            .await
            .is_err()
    );
    assert_eq!(f.reads(), 1);
    f.auth.current.lock().unwrap().grant_active = false;
    assert!(client.check(policy).await.is_err());
    assert!(
        client
            .headers(
                policy,
                "api.example.com",
                443,
                "/v1/messages",
                "POST",
                &selectors
            )
            .await
            .is_err()
    );
    assert_eq!(f.reads(), 1);
    f.auth.current.lock().unwrap().grant_active = true;
    authority.unavailable.store(true, Ordering::SeqCst);
    assert!(client.check(policy).await.is_err());
    assert_eq!(f.reads(), 1);
    server.abort();
}
#[test]
fn only_explicit_markers_select_credentials_in_model_and_git_headers() {
    let id = Uuid::from_u128(7);
    let mut headers = http::HeaderMap::new();
    headers.insert("x-api-key", format!("oa-ref:{id}").parse().unwrap());
    headers.insert(
        "authorization",
        format!("Basic {}", STANDARD.encode(format!("git:oa-ref:{id}")))
            .parse()
            .unwrap(),
    );
    let selected = gateway::selectors(&headers).unwrap();
    assert_eq!(selected.len(), 2);
    assert_eq!(selected["authorization"], id);
    headers.insert(
        "authorization",
        format!("Bearer oa-ref:{id}").parse().unwrap(),
    );
    assert_eq!(gateway::selectors(&headers).unwrap()["authorization"], id);
    headers.insert(
        "authorization",
        format!("token oa-ref:{id}").parse().unwrap(),
    );
    assert_eq!(gateway::selectors(&headers).unwrap()["authorization"], id);
    headers.insert(
        "authorization",
        "Bearer oa-ref:projects/other/secrets/key/versions/1"
            .parse()
            .unwrap(),
    );
    assert!(gateway::selectors(&headers).is_err());
    headers.insert(
        "authorization",
        "Bearer caller-supplied-fixture".parse().unwrap(),
    );
    assert!(
        !gateway::selectors(&headers)
            .unwrap()
            .contains_key("authorization")
    );
    headers.append("x-api-key", format!("oa-ref:{id}").parse().unwrap());
    assert!(gateway::selectors(&headers).is_err());
}
#[test]
fn the_gateway_client_requires_tls_and_a_single_broker_origin() {
    let f = F::new();
    let r = f.request;
    let context = gateway::Context {
        native_resource: r.native_resource,
        node: r.node,
        execution_generation: r.execution_generation,
        native_policy_ref: r.native_policy_ref,
    };
    for origin in [
        "http://broker.example.com",
        "https://user@broker.example.com",
        "https://broker.example.com/path",
        "https://broker.example.com/?a=b",
        "https://broker.example.com/#fragment",
    ] {
        assert!(
            gateway::Client::new(origin, Secret::new(vec![42; 32]).unwrap(), context.clone())
                .is_err()
        );
    }
}
