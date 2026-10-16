use nostr::x402::{self, NetworkPolicy, test_invoice};
use openagents_x402::{
    Facilitator, FileReplayStore, PAYMENT_REQUIRED, PAYMENT_SIGNATURE, PaymentPayload,
    server::{Receiver, Request, Resource},
    wire::{decode_payment_required, encode_header},
};
use serde_json::{Map, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct SignetReceiver;
impl Receiver for SignetReceiver {
    fn pay_to(&self) -> String {
        hex::encode(test_invoice::payee())
    }
    fn invoice(&self, _: u64, hash: [u8; 32], _: u32) -> Result<String, String> {
        Ok(test_invoice::signed(
            "lntbs250n",
            test_invoice::fields(hash),
            false,
            false,
        ))
    }
}

#[test]
fn selected_signet_resource_binds_policy_and_consumes_the_original_proof_once() {
    let root = tempfile::tempdir().unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let executor_count = count.clone();
    let resource = Resource {
        url: "http://127.0.0.1/research".into(),
        network: x402::SIGNET,
        amount_msat: 25000,
        timeout_secs: 300,
        description: "Research counter".into(),
        mime_type: "text/plain".into(),
        receiver: Arc::new(SignetReceiver),
        executor: Arc::new(move |_: &[u8]| {
            executor_count.fetch_add(1, Ordering::SeqCst);
            Ok(b"done".to_vec())
        }),
        facilitator: Facilitator::new(FileReplayStore::open(root.path()).unwrap(), 60)
            .with_network_policy(NetworkPolicy::SignetResearch),
    };
    let mut request = Request {
        method: "POST".into(),
        target: "/research".into(),
        headers: vec![],
        body: b"one".to_vec(),
    };
    let (response, _) = resource.handle(&request, 1_700_000_000);
    assert_eq!(response.status, 402);
    let required = decode_payment_required(
        &response
            .headers
            .iter()
            .find(|(name, _)| name == PAYMENT_REQUIRED)
            .unwrap()
            .1,
    )
    .unwrap();
    let terms = required.accepts[0].clone();
    assert_eq!(
        terms.extra["researchProfile"],
        x402::SIGNET_RESEARCH_PROFILE
    );
    let mut payload = PaymentPayload {
        x402_version: 2,
        resource: Some(required.resource),
        accepted: terms.clone(),
        payload: Map::from_iter([("preimage".into(), json!(hex::encode([7; 32])))]),
        extensions: None,
    };
    let standard = Facilitator::new(FileReplayStore::open(root.path()).unwrap(), 60);
    assert_eq!(
        standard
            .settle(&terms, &payload, "one", 1_700_000_000)
            .unwrap_err()
            .error_reason
            .as_deref(),
        Some("unsupported_network")
    );
    payload.accepted.extra.remove("researchProfile");
    request.headers = vec![(PAYMENT_SIGNATURE.into(), encode_header(&payload).unwrap())];
    assert_eq!(resource.handle(&request, 1_700_000_000).0.status, 402);
    assert_eq!(count.load(Ordering::SeqCst), 0);
    payload.accepted = terms;
    request.headers = vec![(PAYMENT_SIGNATURE.into(), encode_header(&payload).unwrap())];
    request.body = b"changed".to_vec();
    assert_eq!(resource.handle(&request, 1_700_000_000).0.status, 402);
    request.body = b"one".to_vec();
    assert_eq!(resource.handle(&request, 1_700_000_000).0.status, 200);
    assert_eq!(resource.handle(&request, 1_700_000_000).0.status, 402);
    assert_eq!(count.load(Ordering::SeqCst), 1);
    // A newly opened facilitator shares the same durable proof consumption.
    let restarted = Facilitator::new(FileReplayStore::open(root.path()).unwrap(), 60)
        .with_network_policy(NetworkPolicy::SignetResearch);
    assert_eq!(
        restarted
            .settle(&payload.accepted, &payload, "one", 1_700_000_000)
            .unwrap_err()
            .error_reason
            .as_deref(),
        Some("duplicate_settlement")
    );
}

#[test]
fn companion_payment_challenge_requires_matching_network_selection() {
    use base64::Engine;
    use openagents_x402::payment_scheme::{Challenge, Terms};
    let terms = Terms {
        realm: "signet research",
        amount_sats: 25,
        invoice: "not paid in this test",
        payment_hash: "public hash",
        network: x402::SIGNET,
        http_method: "POST",
        url: "http://127.0.0.1/research",
        body: b"one",
        expires_at: 1_700_000_300,
        description: None,
    };
    assert!(
        Challenge::issue_with_network_policy(b"test key", &terms, NetworkPolicy::Standard).is_err()
    );
    let challenge =
        Challenge::issue_with_network_policy(b"test key", &terms, NetworkPolicy::SignetResearch)
            .unwrap();
    let request: serde_json::Value = serde_json::from_slice(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(challenge.request)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(request["methodDetails"]["network"], "signet");
    for network in [x402::MAINNET, x402::TESTNET, "lnbtc:regtest"] {
        let other = Terms { network, ..terms };
        assert!(
            Challenge::issue_with_network_policy(
                b"test key",
                &other,
                NetworkPolicy::SignetResearch
            )
            .is_err()
        );
    }
}

#[test]
fn mcp_research_terms_share_the_same_network_and_proof_checks() {
    use openagents_x402::{
        facilitator::MCP_ONLY,
        mcp::{self, Gate, PaidTools},
    };
    let root = tempfile::tempdir().unwrap();
    let tool = PaidTools {
        server: "http://127.0.0.1/mcp".into(),
        network: x402::SIGNET,
        amount_msat: 25000,
        timeout_secs: 300,
        description: "Research counter".into(),
        receiver: Arc::new(SignetReceiver),
        facilitator: Facilitator::with_profiles(
            FileReplayStore::open(root.path()).unwrap(),
            60,
            MCP_ONLY,
        )
        .with_network_policy(NetworkPolicy::SignetResearch),
    };
    let params = json!({"name":"counter","arguments":{"value":1}});
    let Gate::Challenge(result) = tool.gate(&params, 1_700_000_000).unwrap() else {
        panic!("expected challenge")
    };
    let required = mcp::payment_required_from_result(&result).unwrap();
    let accepted = required.accepts[0].clone();
    assert_eq!(
        accepted.extra["researchProfile"],
        x402::SIGNET_RESEARCH_PROFILE
    );
    let payload = PaymentPayload {
        x402_version: 2,
        resource: Some(required.resource),
        accepted,
        payload: Map::from_iter([("preimage".into(), json!(hex::encode([7; 32])))]),
        extensions: None,
    };
    let mut changed =
        mcp::with_payment(json!({"name":"counter","arguments":{"value":2}}), &payload).unwrap();
    assert!(matches!(
        tool.gate(&changed, 1_700_000_000).unwrap(),
        Gate::Refused(_)
    ));
    changed = mcp::with_payment(params, &payload).unwrap();
    assert!(matches!(
        tool.gate(&changed, 1_700_000_000).unwrap(),
        Gate::Admitted { .. }
    ));
    assert!(matches!(
        tool.gate(&changed, 1_700_000_000).unwrap(),
        Gate::Refused(_)
    ));
}
