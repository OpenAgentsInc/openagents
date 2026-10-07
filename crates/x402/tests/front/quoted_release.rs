//! Buyer approval binds a mutable catalog's immutable release to both proofs.

use super::*;
use openagents_x402::front::{PricePart, Quote};

#[test]
fn same_price_release_change_requires_new_body_approval_before_an_invoice() {
    let tmp = tempfile::tempdir().unwrap();
    let node = Arc::new(TestNode::new(Some(NOW)));
    let ledger = Arc::new(Ledger::default());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let current = Arc::new(Mutex::new(Quote {
        price_msat: 6000,
        parts: vec![
            PricePart {
                name: "endpoint".into(),
                msat: 5000,
            },
            PricePart {
                name: "author_fee".into(),
                msat: 1000,
            },
        ],
        plugin: Some("publisher:meeting-action-items".into()),
        release: Some("release1".into()),
        author: Some("publisher".into()),
        fee_msat: Some(1000),
        resource: None,
    }));
    let mut paid_route = route(
        "invoke",
        "/v1/plugins/{id}/invoke",
        6,
        Arc::new(Counted::default()),
    );
    let catalog = current.clone();
    paid_route.price = Price::Quote(Arc::new(move |_| Ok(catalog.lock().unwrap().clone())));
    let executed = seen.clone();
    paid_route.executor = Arc::new(move |call: &Call<'_>| {
        executed
            .lock()
            .unwrap()
            .push(call.quote.unwrap().release.clone().unwrap());
        Ok(Output {
            body: b"useful result".to_vec(),
            content_type: None,
        })
    });
    let front = Front::new(
        Config {
            base_url: BASE.into(),
            network: nostr::x402::MAINNET,
            realm: "api.example.com".into(),
            challenge_key: vec![7; 32],
            timeout_secs: 300,
        },
        node.clone(),
        Facilitator::new(FileReplayStore::open(tmp.path()).unwrap(), 60),
        ledger.clone(),
        vec![paid_route],
    )
    .unwrap();
    let target = "/v1/plugins/publisher:meeting-action-items/invoke";
    let (preview, _) = front.handle(&post(target, b"{}", vec![]), NOW);
    assert_eq!(preview.status, 409);
    assert!(header(&preview, PAYMENT_REQUIRED).is_none());
    assert_eq!(node.counter.load(Ordering::SeqCst), 0);
    let initial: Value = serde_json::from_slice(&preview.body).unwrap();
    let approved =
        json!({"quote_digest":initial["quote_digest"],"request":"ACTION: Ana sends notes."})
            .to_string();
    let (challenge, _) = front.handle(&post(target, approved.as_bytes(), vec![]), NOW);
    assert_eq!(challenge.status, 402);
    let signature = x402_signature(&node, &challenge);
    let payment = payment_credential(&node, &challenge);
    current.lock().unwrap().release = Some("release2".into());
    let (changed, _) = front.handle(&post(target, approved.as_bytes(), vec![]), NOW);
    assert_eq!(changed.status, 409);
    assert!(header(&changed, PAYMENT_REQUIRED).is_none());
    let replacement: Value = serde_json::from_slice(&changed.body).unwrap();
    assert_ne!(replacement["quote_digest"], initial["quote_digest"]);
    assert_eq!(
        replacement["quote"]["price_msat"],
        initial["quote"]["price_msat"]
    );
    for (name, proof) in [
        (PAYMENT_SIGNATURE, signature.clone()),
        (payment_scheme::AUTHORIZATION, payment),
    ] {
        let (refused, _) =
            front.handle(&post(target, approved.as_bytes(), vec![(name, proof)]), NOW);
        assert_eq!(refused.status, 409);
    }
    assert_eq!(node.counter.load(Ordering::SeqCst), 1);
    assert!(seen.lock().unwrap().is_empty());
    assert!(ledger.rows.lock().unwrap().is_empty());

    // Even after new approval, an invoice for the former body cannot be reused.
    let newly_approved =
        json!({"quote_digest":replacement["quote_digest"],"request":"ACTION: Ana sends notes."})
            .to_string();
    let (rebound, _) = front.handle(
        &post(
            target,
            newly_approved.as_bytes(),
            vec![(PAYMENT_SIGNATURE, signature)],
        ),
        NOW,
    );
    assert_eq!(rebound.status, 402);
    let body: Value = serde_json::from_slice(&rebound.body).unwrap();
    assert_eq!(body["error"]["type"], "payment_refused");
    assert!(seen.lock().unwrap().is_empty());
    assert!(ledger.rows.lock().unwrap().is_empty());
    let fresh = x402_signature(&node, &rebound);
    let (result, _) = front.handle(
        &post(
            target,
            newly_approved.as_bytes(),
            vec![(PAYMENT_SIGNATURE, fresh)],
        ),
        NOW,
    );
    assert_eq!(result.status, 200);
    assert_eq!(*seen.lock().unwrap(), vec!["release2"]);
    let settlements = ledger.rows.lock().unwrap();
    assert_eq!(settlements.len(), 1);
    assert_eq!(settlements[0].release.as_deref(), Some("release2"));
    assert_eq!(settlements[0].fee_msat, Some(1000));
}
