use super::*;
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
struct TestKms {
    calls: AtomicU64,
    offline: AtomicBool,
}
#[async_trait]
impl Kms for TestKms {
    async fn encrypt(&self, route: &str, plain: &[u8], aad: &[u8]) -> Result<Vec<u8>, Failure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.offline.load(Ordering::SeqCst) {
            return Err(Failure::Unavailable);
        }
        let mut nonce = [0u8; 12];
        getrandom::fill(&mut nonce).unwrap();
        let cipher = Aes256Gcm::new_from_slice(&Sha256::digest(route.as_bytes())).unwrap();
        let mut out = nonce.to_vec();
        out.extend(
            cipher
                .encrypt(Nonce::from_slice(&nonce), Payload { msg: plain, aad })
                .unwrap(),
        );
        Ok(out)
    }
    async fn decrypt(
        &self,
        route: &str,
        cipher: &[u8],
        aad: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, Failure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.offline.load(Ordering::SeqCst) {
            return Err(Failure::Unavailable);
        }
        if cipher.len() < 28 {
            return Err(Failure::InvalidKey);
        }
        Aes256Gcm::new_from_slice(&Sha256::digest(route.as_bytes()))
            .unwrap()
            .decrypt(
                Nonce::from_slice(&cipher[..12]),
                Payload {
                    msg: &cipher[12..],
                    aad,
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| Failure::InvalidKey)
    }
}
const TOKEN: &str = "synthetic-fixture-credential-11814-only";
const A: &str = "00000000-0000-0000-0000-000000000001";
const B: &str = "00000000-0000-0000-0000-000000000002";
fn setup() -> (Broker, Arc<TestKms>) {
    let kms = Arc::new(TestKms {
        calls: AtomicU64::new(0),
        offline: AtomicBool::new(false),
    });
    let policy = Policy {
        namespace: "urn:openagents:space:org".into(),
        tenants: [A.into(), B.into()].into(),
        active_key: "v1".into(),
        key_routes: [(
            "v1".into(),
            "projects/fixture/locations/us-central1/keyRings/space/cryptoKeys/intermediate".into(),
        )]
        .into(),
    };
    (
        Broker::new(policy, kms.clone(), TOKEN.as_bytes()).unwrap(),
        kms,
    )
}
fn tenant(id: &str) -> TenantRef {
    TenantRef {
        namespace: "urn:openagents:space:org".into(),
        id: id.into(),
    }
}
fn signed<T>(value: T) -> Request<T> {
    let mut req = Request::new(value);
    req.metadata_mut()
        .insert("authorization", format!("Bearer {TOKEN}").parse().unwrap());
    req
}
async fn issue(b: &Broker, id: &str) -> WrappedIntermediateKey {
    match b
        .issue_intermediate_key(signed(IssueIntermediateKeyRequest {
            tenant: Some(tenant(id)),
            authorization: None,
        }))
        .await
        .unwrap()
        .into_inner()
        .outcome
        .unwrap()
    {
        issue_intermediate_key_response::Outcome::Key(key) => key,
        _ => panic!("The fixture key was not issued."),
    }
}
async fn unwrap(
    b: &Broker,
    id: &str,
    key: &WrappedIntermediateKey,
) -> unwrap_intermediate_key_response::Outcome {
    b.unwrap_intermediate_key(signed(UnwrapIntermediateKeyRequest {
        tenant: Some(tenant(id)),
        key_id: key.key_id.clone(),
        wrapped_key: key.wrapped_key.clone(),
        authorization: None,
    }))
    .await
    .unwrap()
    .into_inner()
    .outcome
    .unwrap()
}
#[tokio::test]
async fn issued_keys_are_fresh_wrapped_bound_to_tenant_and_releasable_without_plaintext_caching() {
    let (broker, _) = setup();
    let first = issue(&broker, A).await;
    let second = issue(&broker, A).await;
    assert!(first.key_id != second.key_id && first.wrapped_key != second.wrapped_key);
    let unwrap_intermediate_key_response::Outcome::Key(material) = unwrap(&broker, A, &first).await
    else {
        panic!("Key release failed");
    };
    assert_eq!(material.plaintext_key.len(), 32);
    assert_eq!(material.max_cache_ttl.unwrap().seconds, 0);
    let unwrap_intermediate_key_response::Outcome::Key(again) = unwrap(&broker, A, &first).await
    else {
        panic!("Key release failed");
    };
    assert!(material.plaintext_key == again.plaintext_key);
    let unwrap_intermediate_key_response::Outcome::Failure(wrong) =
        unwrap(&broker, B, &first).await
    else {
        panic!("A different tenant opened the key");
    };
    assert_eq!(wrong.reason, UnwrapFailureReason::InvalidKey as i32);
    let wrong_id = WrappedIntermediateKey {
        key_id: second.key_id,
        wrapped_key: first.wrapped_key.clone(),
    };
    assert!(matches!(
        unwrap(&broker, A, &wrong_id).await,
        unwrap_intermediate_key_response::Outcome::Failure(_)
    ));
    let mut corrupt = first;
    corrupt.wrapped_key[20] ^= 1;
    assert!(matches!(
        unwrap(&broker, A, &corrupt).await,
        unwrap_intermediate_key_response::Outcome::Failure(_)
    ));
}
#[tokio::test]
async fn unknown_tenants_and_every_present_evidence_refuse_before_kms() {
    let (broker, kms) = setup();
    for authorization in [
        None,
        Some(AuthorizationEvidence {
            scheme: "unsupported".into(),
            subject: Some("subject".into()),
            evidence: b"private-evidence".to_vec(),
        }),
    ] {
        let req = IssueIntermediateKeyRequest {
            tenant: Some(tenant("00000000-0000-0000-0000-000000000003")),
            authorization,
        };
        let issue_intermediate_key_response::Outcome::Failure(f) = broker
            .issue_intermediate_key(signed(req))
            .await
            .unwrap()
            .into_inner()
            .outcome
            .unwrap()
        else {
            panic!("Unexpected issuance");
        };
        assert!(
            [
                IssueFailureReason::InvalidTenant as i32,
                IssueFailureReason::AuthorizationRejected as i32
            ]
            .contains(&f.reason)
        );
        assert!(!f.detail.contains("private-evidence"));
    }
    assert_eq!(kms.calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn bad_transport_credentials_and_kms_failures_never_report_revocation() {
    let (broker, kms) = setup();
    let key = issue(&broker, A).await;
    let req = IssueIntermediateKeyRequest {
        tenant: Some(tenant(A)),
        authorization: None,
    };
    assert_eq!(
        broker
            .issue_intermediate_key(Request::new(req.clone()))
            .await
            .unwrap_err()
            .code(),
        tonic::Code::Unauthenticated
    );
    kms.offline.store(true, Ordering::SeqCst);
    let issue_intermediate_key_response::Outcome::Failure(f) = broker
        .issue_intermediate_key(signed(req))
        .await
        .unwrap()
        .into_inner()
        .outcome
        .unwrap()
    else {
        panic!("Unexpected issuance");
    };
    assert_eq!(f.reason, IssueFailureReason::Unavailable as i32);
    let unwrap_intermediate_key_response::Outcome::Failure(f) = unwrap(&broker, A, &key).await
    else {
        panic!("Unexpected key release");
    };
    assert_eq!(f.reason, UnwrapFailureReason::Unavailable as i32);
}

#[tokio::test]
async fn rotating_the_active_route_keeps_old_keys_openable() {
    let (mut broker, _) = setup();
    let old = issue(&broker, A).await;
    broker.policy.key_routes.insert(
        "v2".into(),
        "projects/fixture/locations/us-central1/keyRings/space/cryptoKeys/next".into(),
    );
    broker.policy.active_key = "v2".into();
    let current = issue(&broker, A).await;
    assert!(old.key_id.starts_with("ik_v1_") && current.key_id.starts_with("ik_v2_"));
    assert!(matches!(
        unwrap(&broker, A, &old).await,
        unwrap_intermediate_key_response::Outcome::Key(_)
    ));
    assert!(matches!(
        unwrap(&broker, A, &current).await,
        unwrap_intermediate_key_response::Outcome::Key(_)
    ));
}

#[tokio::test]
async fn grpc_wire_authentication_and_outcomes_match_the_public_protocol() {
    use wire::intermediate_key_broker_client::IntermediateKeyBrokerClient;
    use wire::intermediate_key_broker_server::IntermediateKeyBrokerServer;
    let (broker, _) = setup();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(IntermediateKeyBrokerServer::new(broker))
            .serve_with_incoming_shutdown(
                tokio_stream::wrappers::TcpListenerStream::new(listener),
                async {
                    let _ = stopped.await;
                },
            )
            .await
            .unwrap();
    });
    let mut client = IntermediateKeyBrokerClient::connect(format!("http://{address}"))
        .await
        .unwrap();
    let value = IssueIntermediateKeyRequest {
        tenant: Some(tenant(A)),
        authorization: None,
    };
    assert_eq!(
        client
            .issue_intermediate_key(value.clone())
            .await
            .unwrap_err()
            .code(),
        tonic::Code::Unauthenticated
    );
    let issue_intermediate_key_response::Outcome::Key(key) = client
        .issue_intermediate_key(signed(value))
        .await
        .unwrap()
        .into_inner()
        .outcome
        .unwrap()
    else {
        panic!("Wire issuance failed");
    };
    let release = UnwrapIntermediateKeyRequest {
        tenant: Some(tenant(A)),
        key_id: key.key_id,
        wrapped_key: key.wrapped_key,
        authorization: None,
    };
    let unwrap_intermediate_key_response::Outcome::Key(key) = client
        .unwrap_intermediate_key(signed(release))
        .await
        .unwrap()
        .into_inner()
        .outcome
        .unwrap()
    else {
        panic!("Wire key release failed");
    };
    assert_eq!(key.plaintext_key.len(), 32);
    assert_eq!(key.max_cache_ttl.unwrap().seconds, 0);
    stop.send(()).unwrap();
    task.await.unwrap();
}

#[test]
fn configuration_rejects_key_path_traversal_and_a_missing_active_route() {
    let (broker, _) = setup();
    let mut policy = broker.policy;
    policy.key_routes.insert(
        "v1".into(),
        "projects/../locations/us-central1/keyRings/space/cryptoKeys/intermediate".into(),
    );
    assert!(policy.validate().is_err());
    policy.key_routes.clear();
    assert!(policy.validate().is_err());
}
