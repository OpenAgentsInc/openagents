use super::*;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Default)]
pub(crate) struct WalletState {
    pub settled: AtomicBool,
    pub bad_proof: AtomicBool,
    pub wrong_amount: AtomicBool,
    pub wrong_metadata: AtomicBool,
    pub no_verify: AtomicBool,
    pub duplicate: AtomicBool,
    pub short_expiry: AtomicBool,
    pub invoices: AtomicUsize,
    entries: Mutex<HashMap<String, (String, String)>>,
}
pub(crate) struct FakeWallet {
    pub endpoint: String,
    pub state: Arc<WalletState>,
}
const METADATA: &str = "[[\"text/plain\",\"Design seller\"]]";
async fn offer() -> Json<Value> {
    Json(
        json!({"tag":"payRequest","minSendable":1000,"maxSendable":10_000_000_000u64,"metadata":METADATA,"callback":"https://wallet.test/callback"}),
    )
}
async fn issue(
    State(s): State<Arc<WalletState>>,
    Query(query): Query<HashMap<String, String>>,
) -> Json<Value> {
    use bitcoin::hashes::{Hash as _, sha256};
    use lightning_invoice::{Currency, InvoiceBuilder, PaymentSecret};
    let count = s.invoices.fetch_add(1, Ordering::SeqCst) + 1;
    let id = if s.duplicate.load(Ordering::SeqCst) {
        1
    } else {
        count
    };
    let preimage = Sha256::digest(format!("fixture-lightning-payment-{id}").as_bytes());
    let hash = sha256::Hash::hash(&preimage);
    let amount = query["amount"].parse::<u64>().unwrap()
        + u64::from(s.wrong_amount.load(Ordering::SeqCst)) * 1000;
    let metadata = if s.wrong_metadata.load(Ordering::SeqCst) {
        "different metadata"
    } else {
        METADATA
    };
    let key = bitcoin::secp256k1::SecretKey::from_slice(&[42; 32]).unwrap();
    let invoice = InvoiceBuilder::new(Currency::Bitcoin)
        .amount_milli_satoshis(amount)
        .description_hash(sha256::Hash::hash(metadata.as_bytes()))
        .payment_hash(hash)
        .payment_secret(PaymentSecret([31; 32]))
        .duration_since_epoch(Duration::from_secs(now()))
        .expiry_time(Duration::from_secs(
            if s.short_expiry.load(Ordering::SeqCst) {
                1
            } else {
                3600
            },
        ))
        .min_final_cltv_expiry_delta(18)
        .build_signed(|m| bitcoin::secp256k1::Secp256k1::new().sign_ecdsa_recoverable(m, &key))
        .unwrap()
        .to_string();
    s.entries
        .lock()
        .unwrap()
        .insert(id.to_string(), (invoice.clone(), hex(&preimage)));
    let mut reply = json!({"pr":invoice,"routes":[]});
    if !s.no_verify.load(Ordering::SeqCst) {
        reply["verify"] = json!(format!("https://wallet.test/verify/{id}"));
    }
    Json(reply)
}
async fn verify(State(s): State<Arc<WalletState>>, Path(id): Path<String>) -> Json<Value> {
    let entry = s.entries.lock().unwrap().get(&id).cloned();
    let Some((invoice, preimage)) = entry else {
        return Json(json!({"status":"ERROR","reason":"Not found"}));
    };
    let settled = s.settled.load(Ordering::SeqCst);
    Json(
        json!({"status":"OK","settled":settled,"pr":invoice,"preimage":if !settled{Value::Null}else{json!(if s.bad_proof.load(Ordering::SeqCst){"00".repeat(32)}else{preimage})}}),
    )
}
pub(crate) async fn fake_wallet() -> FakeWallet {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let state = Arc::new(WalletState::default());
    let site = Router::new()
        .route("/.well-known/lnurlp/designer", get(offer))
        .route("/callback", get(issue))
        .route("/verify/{id}", get(verify))
        .route(
            "/settle",
            post(|State(s): State<Arc<WalletState>>| async move {
                s.settled.store(true, Ordering::SeqCst);
                Json(json!({"paid":true}))
            }),
        )
        .route("/stats",get(|State(s):State<Arc<WalletState>>|async move{Json(json!({"invoices":s.invoices.load(Ordering::SeqCst),"settled":s.settled.load(Ordering::SeqCst)}))}))
        .with_state(state.clone());
    lightning::TEST_SERVICE.set(endpoint.clone()).unwrap();
    tokio::spawn(async move { axum::serve(listener, site).await.unwrap() });
    FakeWallet { endpoint, state }
}
#[tokio::test]
async fn lightning_proofs_bind_the_price_metadata_order_and_buyer_and_survive_unlisting() {
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use tower::ServiceExt;
    let wallet = fake_wallet().await;
    for flag in [
        &wallet.state.wrong_amount,
        &wallet.state.wrong_metadata,
        &wallet.state.no_verify,
        &wallet.state.short_expiry,
    ] {
        flag.store(true, Ordering::SeqCst);
        assert!(
            lightning::create("designer@wallet.test", 500, now())
                .await
                .is_err()
        );
        flag.store(false, Ordering::SeqCst);
    }
    let invoice = lightning::create("designer@wallet.test", 500, now())
        .await
        .unwrap();
    assert!(!lightning::settled(&invoice).await.unwrap());
    let dir = tempfile::tempdir().unwrap();
    let store = Store::local(dir.path().join("store"));
    let owner = crate::chat_store::account_owner("acct_seller");
    let buyer = crate::chat_store::account_owner("acct_buyer");
    let scope = Scope::personal(&owner).unwrap();
    let data = json!({"title":"Bracket","version":1,"document":{"secret":"This source is private"},"assets":["private import"],"view":{"kind":"cad-view.v1","solids":[]},"controls":[{"path":"/features/0/params/height","label":"Thickness","min":1,"max":10,"step":1}],"values":[3]});
    let listing = create(&store, &scope, "bracket", data, 500, "designer@wallet.test")
        .await
        .unwrap();
    let mut config = crate::Config::development(dir.path().join("tasks"));
    config.chat_store = Arc::new(store.clone());
    config.public_hosts = vec!["openagents.test".into()];
    let site = crate::router(config);
    let page = site
        .clone()
        .oneshot(
            Request::get(format!("/cad-shop/{listing}"))
                .header(header::HOST, "openagents.test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(page.status(), StatusCode::OK);
    let html =
        String::from_utf8(to_bytes(page.into_body(), 1 << 20).await.unwrap().to_vec()).unwrap();
    assert!(!html.contains("This source is private"));
    assert!(!html.contains("private import"));
    assert!(
        oa_copy::violations(&html, &[]).is_empty(),
        "{:?}",
        oa_copy::violations(&html, &[])
    );
    let view = site
        .clone()
        .oneshot(
            Request::get(format!("/cad-shop/{listing}/view"))
                .header(header::HOST, "openagents.test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let data: Value =
        serde_json::from_slice(&to_bytes(view.into_body(), 1 << 20).await.unwrap()).unwrap();
    assert_eq!(data["kind"], "cad-view.v1");
    assert!(data.get("document").is_none());
    assert!(data.get("assets").is_none());
    let id = token();
    let order = Order {
        buyer: buyer.clone(),
        listing: listing.clone(),
        title: "Bracket".into(),
        version: 1,
        labels: vec!["Thickness".into()],
        values: vec![6.],
        sats: 500,
        address: "designer@wallet.test".into(),
        created: now(),
        invoice: Some(invoice.clone()),
        package_hash: Some(hex(&Sha256::digest(b"prepared files"))),
        paid: false,
        paid_at: None,
    };
    store
        .write_key(&order_key(&id), serde_json::to_vec(&order).unwrap(), None)
        .await
        .unwrap();
    store
        .write_key(
            &format!("cad-payment-hashes/{}.json", invoice.hash),
            id.as_bytes().to_vec(),
            None,
        )
        .await
        .unwrap();
    assert!(!pay_check(&store, &buyer, &id).await.unwrap());
    assert!(pay_check(&store, &owner, &id).await.is_err());
    wallet.state.settled.store(true, Ordering::SeqCst);
    wallet.state.bad_proof.store(true, Ordering::SeqCst);
    assert!(pay_check(&store, &buyer, &id).await.is_err());
    assert!(
        !read::<Order>(&store, &order_key(&id))
            .await
            .unwrap()
            .unwrap()
            .0
            .paid
    );
    wallet.state.bad_proof.store(false, Ordering::SeqCst);
    assert!(pay_check(&store, &buyer, &id).await.unwrap());
    wallet.state.bad_proof.store(true, Ordering::SeqCst);
    assert!(pay_check(&store, &buyer, &id).await.unwrap());
    let second = token();
    store
        .write_key(
            &order_key(&second),
            serde_json::to_vec(&order).unwrap(),
            None,
        )
        .await
        .unwrap();
    wallet.state.bad_proof.store(false, Ordering::SeqCst);
    assert!(pay_check(&store, &buyer, &second).await.is_err());
    stop(&store, &scope, "bracket", &listing).await.unwrap();
    assert!(
        !read::<Listing>(&store, &listing_key(&listing))
            .await
            .unwrap()
            .unwrap()
            .0
            .active
    );
    assert!(
        read::<Order>(&store, &order_key(&id))
            .await
            .unwrap()
            .unwrap()
            .0
            .paid
    );
    let unavailable = site
        .oneshot(
            Request::get(format!("/cad-shop/{listing}"))
                .header(header::HOST, "openagents.test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unavailable.status(), StatusCode::NOT_FOUND);
}
