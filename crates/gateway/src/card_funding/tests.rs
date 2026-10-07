use super::*;
use axum::{Router, body::Body, http::Response, routing::get};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn secret() -> Vec<u8> {
    secp256k1::SecretKey::new(&mut secp256k1::rand::rng())
        .secret_bytes()
        .to_vec()
}

#[tokio::test]
async fn native_collection_and_failed_refund_returns_join_original_money_without_second_credit() {
    use axum::{
        extract::State,
        http::{HeaderMap, StatusCode, Uri},
        response::IntoResponse,
    };
    use std::{collections::BTreeMap, sync::Mutex};
    use tenancy::money::{Ledger, Mutation, Operation, funding::*};
    type Records = Arc<Mutex<BTreeMap<String, Value>>>;
    async fn provider(
        State(records): State<Records>,
        uri: Uri,
        headers: HeaderMap,
    ) -> axum::response::Response {
        assert_eq!(headers["stripe-version"], "fixture.v1");
        assert!(
            headers["authorization"]
                .to_str()
                .unwrap()
                .starts_with("Bearer rk_test_")
        );
        let records = records.lock().unwrap();
        match records.get(uri.path()) {
            Some(value) => axum::Json(value.clone()).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        }
    }
    let root = tempfile::tempdir().unwrap();
    let ledger_path = root.path().join("money.jsonl");
    let mut ledger = Ledger::open(&ledger_path).unwrap();
    let apply = |ledger: &mut Ledger, source: &str, operation: Operation| {
        ledger.apply(Mutation {
            workspace: "native-buyer".into(),
            source: source.into(),
            audit: "isolated native card fixture".into(),
            operation,
        })
    };
    apply(
        &mut ledger,
        "create",
        Operation::Create {
            currency: "USD".into(),
            spend_limit: 1_000_000_000,
            topups_allowed: true,
        },
    )
    .unwrap();
    let unit = Unit::CurrencyMillionths {
        currency: "USD".into(),
    };
    let policy = Policy {
        schema: POLICY_SCHEMA.into(),
        version: "card-fixture-v1".into(),
        unit: unit.clone(),
        conversions: vec![Conversion {
            version: "card-usd-fixture-v1".into(),
            source: unit.clone(),
            target: unit,
            numerator: 1,
            denominator: 1,
            source_ref: "fixture:no-real-processor-or-money".into(),
            valid_from: 0,
            valid_until: u64::MAX,
            rounding: Rounding::Exact,
            fee_payer: FeePayer::Customer,
            max_fee_units: 5_000_000,
        }],
        purchases: PurchaseTerms {
            required_finality: Finality::Final,
            refunds_allowed: true,
            disputes_allowed: true,
            spent_credit_loss: SpentCreditLoss::Operator,
        },
        promotions: PromotionTerms {
            total_cap: 1,
            grant_cap: 1,
            max_lifetime_seconds: 1,
            max_admissions: 1,
            price_policies: ["fixture-use-v1".into()].into(),
            reversible: true,
        },
    };
    apply(&mut ledger, "policy", Operation::FundingPolicy { policy }).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    apply(
        &mut ledger,
        "quote",
        Operation::QuoteFunding {
            quote: Quote {
                id: "original_checkout_quote".into(),
                origin: "stripe:acct_fixture:original-customer".into(),
                policy: "card-fixture-v1".into(),
                conversion: "card-usd-fixture-v1".into(),
                gross_units: 100_000_000,
                maximum_fee_units: 5_000_000,
                expires_at: now + 3600,
            },
        },
    )
    .unwrap();
    let statement = ledger.statement("native-buyer").unwrap();
    let quoted = &statement.funding_quotes[0];
    let at = quoted.quoted_at;
    let mut records = BTreeMap::from([
        (
            "/v1/account".into(),
            json!({"object":"account","id":"acct_fixture"}),
        ),
        (
            "/v1/customers/cus_original".into(),
            json!({"object":"customer","id":"cus_original","livemode":false,
            "metadata":{"oa_customer":"original_customer_reference"},"name":"seeded-private-card-customer"}),
        ),
        (
            "/v1/checkout/sessions/cs_test_original".into(),
            json!({"object":"checkout.session","id":"cs_test_original","livemode":false,
            "mode":"payment","customer":"cus_original","currency":"usd","amount_total":10000,"metadata":{"oa_quote":quoted.quote.id},
            "client_reference_id":quoted.quote.id,"expires_at":quoted.quote.expires_at,"status":"complete","payment_status":"paid","payment_intent":"pi_original"}),
        ),
        (
            "/v1/payment_intents/pi_original".into(),
            json!({"object":"payment_intent","id":"pi_original","livemode":false,
            "status":"succeeded","capture_method":"automatic","customer":"cus_original","currency":"usd",
            "amount":10000,"amount_received":10000,"amount_capturable":0,"metadata":{"oa_quote":quoted.quote.id},"latest_charge":"ch_original"}),
        ),
        (
            "/v1/charges/ch_original".into(),
            json!({"object":"charge","id":"ch_original","livemode":false,
            "payment_intent":"pi_original","customer":"cus_original","currency":"usd","status":"succeeded","paid":true,"captured":true,
            "payment_method_details":{"type":"card","card":{"last4":"seeded-private-card"}},"amount":10000,"amount_captured":10000,
            "created":at,"balance_transaction":"txn_pay","amount_refunded":2000,"refunded":false,"disputed":false}),
        ),
        (
            "/v1/balance_transactions/txn_pay".into(),
            json!({"object":"balance_transaction","id":"txn_pay","source":"ch_original",
            "type":"charge","currency":"usd","amount":10000,"fee":300,"net":9700,"status":"available","created":at,"available_on":at}),
        ),
        (
            "/v1/refunds".into(),
            json!({"object":"list","url":"/v1/refunds","has_more":false,"data":[{"object":"refund","id":"re_original","charge":"ch_original"}]}),
        ),
        (
            "/v1/disputes".into(),
            json!({"object":"list","url":"/v1/disputes","has_more":false,"data":[]}),
        ),
        (
            "/v1/refunds/re_original".into(),
            json!({"object":"refund","id":"re_original","charge":"ch_original","payment_intent":"pi_original",
            "currency":"usd","status":"succeeded","amount":2000,"balance_transaction":"txn_refund"}),
        ),
        (
            "/v1/balance_transactions/txn_refund".into(),
            json!({"object":"balance_transaction","id":"txn_refund","source":"re_original",
            "type":"refund","currency":"usd","amount":-2000,"fee":0,"net":-2000,"status":"available","created":at,"available_on":at}),
        ),
    ]);
    let baseline = records.clone();
    let records = Arc::new(Mutex::new(std::mem::take(&mut records)));
    let (origin, task) = server(Router::new().fallback(provider).with_state(records.clone())).await;
    let material = secret()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let mut client =
        Stripe::new_for_mode(format!("rk_test_{material}"), "fixture.v1".into(), false).unwrap();
    client.origin = origin;
    client.bind_account("acct_fixture").await.unwrap();
    let original = Original {
        checkout: "cs_test_original",
        customer: "cus_original",
        customer_reference: "original_customer_reference",
        quote: quoted,
    };
    let first = client.collect(&original, None, at).await.unwrap().unwrap();
    assert_eq!(
        (
            first.snapshot.funding.gross_units,
            first.snapshot.funding.fee_units,
            first.snapshot.refunded_source_units
        ),
        (100_000_000, 3_000_000, 20_000_000)
    );
    let json = serde_json::to_string(&first).unwrap();
    assert!(!json.contains("seeded-private"));
    apply(
        &mut ledger,
        "state-1",
        Operation::ReconcileQuotedFunding {
            snapshot: first.snapshot.clone(),
        },
    )
    .unwrap();
    let b = ledger.balance("native-buyer").unwrap();
    assert_eq!((b.credited, b.available), (97_000_000, 77_000_000));
    {
        let mut r = baseline.clone();
        r.get_mut("/v1/charges/ch_original").unwrap()["disputed"] = json!(true);
        r.get_mut("/v1/disputes").unwrap()["data"] = json!([{"object":"dispute","id":"du_original","charge":"ch_original","livemode":false}]);
        r.insert("/v1/disputes/du_original".into(),json!({"object":"dispute","id":"du_original","charge":"ch_original","payment_intent":"pi_original",
            "livemode":false,"currency":"usd","amount":5000,"status":"lost","balance_transactions":[{"id":"txn_dispute_out"}]}));
        r.insert("/v1/balance_transactions/txn_dispute_out".into(),json!({"object":"balance_transaction","id":"txn_dispute_out","source":"du_original",
            "type":"adjustment","currency":"usd","amount":-5000,"fee":1500,"net":-6500,"status":"available","created":at,"available_on":at}));
        *records.lock().unwrap() = r;
    }
    let withdrawn = client
        .collect(&original, Some(&first), at)
        .await
        .unwrap()
        .unwrap();
    // Fork the isolated sealed fixture journal, preserving original quote time.
    // This branch tests dispute expense; the main branch below tests refunds.
    let expense_path = root.path().join("expense-money.jsonl");
    std::fs::copy(&ledger_path, &expense_path).unwrap();
    let mut expense_ledger = Ledger::open(&expense_path).unwrap();
    apply(
        &mut expense_ledger,
        "native-dispute-expense",
        Operation::ReconcileQuotedFunding {
            snapshot: withdrawn.snapshot.clone(),
        },
    )
    .unwrap();
    let expense_balance = expense_ledger.balance("native-buyer").unwrap();
    assert_eq!(
        (
            expense_balance.credited,
            expense_balance.reversed_credit,
            expense_balance.processor_expense_units
        ),
        (97_000_000, 70_000_000, 15_000_000)
    );
    assert_eq!(
        (
            expense_balance.available,
            expense_balance.restricted_credit,
            expense_balance.operator_loss
        ),
        (0, 27_000_000, 0)
    );

    assert_eq!(
        (
            withdrawn.snapshot.disputed_source_units,
            withdrawn.adjustment_fee_units
        ),
        (50_000_000, 15_000_000)
    );
    {
        let mut r = records.lock().unwrap();
        let dispute = r.get_mut("/v1/disputes/du_original").unwrap();
        dispute["status"] = json!("won");
        dispute["balance_transactions"] =
            json!([{"id":"txn_dispute_out"},{"id":"txn_dispute_return"}]);
        r.insert("/v1/balance_transactions/txn_dispute_return".into(),json!({"object":"balance_transaction","id":"txn_dispute_return","source":"du_original",
            "type":"adjustment","currency":"usd","amount":5000,"fee":-1500,"net":6500,"status":"pending","created":at,"available_on":at}));
    }
    let waiting = client
        .collect(&original, Some(&withdrawn), at)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(waiting.snapshot.disputed_source_units, 50_000_000);
    assert_eq!(waiting.snapshot.processor_expense_units, 15_000_000);
    apply(
        &mut expense_ledger,
        "native-pending-fee-return",
        Operation::ReconcileQuotedFunding {
            snapshot: waiting.snapshot.clone(),
        },
    )
    .unwrap();
    assert_eq!(expense_ledger.balance("native-buyer").unwrap().available, 0);
    drop(expense_ledger);
    let mut expense_ledger = Ledger::open(&expense_path).unwrap();
    assert_eq!(
        expense_ledger
            .balance("native-buyer")
            .unwrap()
            .processor_expense_units,
        15_000_000
    );

    records
        .lock()
        .unwrap()
        .get_mut("/v1/balance_transactions/txn_dispute_return")
        .unwrap()["status"] = json!("available");
    let won = client
        .collect(&original, Some(&waiting), at)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(won.snapshot.disputed_source_units, 0);
    assert_eq!(won.snapshot.processor_expense_units, 0);
    let expense_return = Operation::ReconcileQuotedFunding {
        snapshot: won.snapshot.clone(),
    };
    apply(
        &mut expense_ledger,
        "native-available-fee-return",
        expense_return.clone(),
    )
    .unwrap();
    assert!(
        !apply(
            &mut expense_ledger,
            "native-available-fee-return",
            expense_return
        )
        .unwrap()
    );
    let expense_balance = expense_ledger.balance("native-buyer").unwrap();
    assert_eq!(
        (
            expense_balance.credited,
            expense_balance.reversed_credit,
            expense_balance.available,
            expense_balance.processor_expense_units
        ),
        (97_000_000, 20_000_000, 77_000_000, 0)
    );
    drop(expense_ledger);

    records
        .lock()
        .unwrap()
        .get_mut("/v1/balance_transactions/txn_dispute_return")
        .unwrap()["source"] = json!("du_other");
    assert!(client.collect(&original, Some(&waiting), at).await.is_err());
    *records.lock().unwrap() = baseline.clone();
    {
        let mut r = records.lock().unwrap();
        r.get_mut("/v1/charges/ch_original").unwrap()["amount_refunded"] = json!(0);
        let refund = r.get_mut("/v1/refunds/re_original").unwrap();
        refund["status"] = json!("failed");
        refund["failure_balance_transaction"] = json!("txn_return");
        r.insert("/v1/balance_transactions/txn_return".into(),json!({"object":"balance_transaction","id":"txn_return","source":"re_original",
            "type":"refund_failure","currency":"usd","amount":2000,"fee":0,"net":2000,"status":"pending","created":at,"available_on":at}));
    }
    let pending = client
        .collect(&original, Some(&first), at)
        .await
        .unwrap()
        .unwrap();
    assert!(pending.snapshot.reconciliation_pending);
    assert_eq!(pending.snapshot.refunded_source_units, 20_000_000);
    apply(
        &mut ledger,
        "state-2",
        Operation::ReconcileQuotedFunding {
            snapshot: pending.snapshot.clone(),
        },
    )
    .unwrap();
    assert_eq!(ledger.balance("native-buyer").unwrap().available, 0);
    records
        .lock()
        .unwrap()
        .get_mut("/v1/balance_transactions/txn_return")
        .unwrap()["status"] = json!("available");
    let returned = client
        .collect(&original, Some(&pending), at)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(returned.snapshot.refunded_source_units, 0);
    assert_eq!(
        returned.snapshot.refund_recovery_proofs["stripe:acct_fixture:txn_return"],
        20_000_000
    );
    let operation = Operation::ReconcileQuotedFunding {
        snapshot: returned.snapshot.clone(),
    };
    apply(&mut ledger, "state-3", operation.clone()).unwrap();
    assert!(!apply(&mut ledger, "state-3", operation.clone()).unwrap());
    let b = ledger.balance("native-buyer").unwrap();
    assert_eq!((b.credited, b.available), (97_000_000, 97_000_000));
    drop(ledger);
    let mut ledger = Ledger::open(&ledger_path).unwrap();
    assert!(!apply(&mut ledger, "state-3", operation).unwrap());
    assert_eq!(ledger.balance("native-buyer").unwrap().credited, 97_000_000);
    for (path, field, value) in [
        (
            "/v1/payment_intents/pi_original",
            "amount_received",
            json!(9999),
        ),
        (
            "/v1/payment_intents/pi_original",
            "capture_method",
            json!("manual"),
        ),
        (
            "/v1/charges/ch_original",
            "created",
            json!(quoted.quote.expires_at),
        ),
        (
            "/v1/balance_transactions/txn_pay",
            "source",
            json!("ch_other"),
        ),
        ("/v1/balance_transactions/txn_pay", "net", json!(10000)),
        ("/v1/balance_transactions/txn_pay", "currency", json!("eur")),
        (
            "/v1/customers/cus_original",
            "metadata",
            json!({"oa_customer":"foreign_reference"}),
        ),
        (
            "/v1/refunds/re_original",
            "payment_intent",
            json!("pi_other"),
        ),
        ("/v1/charges/ch_original", "livemode", json!(true)),
        ("/v1/refunds", "has_more", json!(true)),
    ] {
        let mut changed = baseline.clone();
        changed.get_mut(path).unwrap()[field] = value;
        *records.lock().unwrap() = changed;
        assert!(
            client.collect(&original, None, at).await.is_err(),
            "{path}/{field}"
        );
        assert_eq!(ledger.balance("native-buyer").unwrap().credited, 97_000_000);
    }
    *records.lock().unwrap() = baseline;
    records
        .lock()
        .unwrap()
        .get_mut("/v1/checkout/sessions/cs_test_original")
        .unwrap()["status"] = json!("open");
    records
        .lock()
        .unwrap()
        .get_mut("/v1/checkout/sessions/cs_test_original")
        .unwrap()["payment_status"] = json!("unpaid");
    assert!(client.collect(&original, None, at).await.unwrap().is_none());
    assert!(
        client
            .collect(&original, Some(&returned), at)
            .await
            .is_err()
    );
    task.abort();
}

#[tokio::test]
async fn native_adjustment_lookup_is_charge_scoped_complete_mode_bound_and_stable() {
    use axum::extract::{Path, Query, State};
    use std::{collections::BTreeMap, sync::Mutex};
    #[derive(Clone)]
    struct Fixture {
        fault: Arc<AtomicUsize>,
        charge_reads: Arc<AtomicUsize>,
        requests: Arc<Mutex<Vec<String>>>,
    }
    async fn account(State(f): State<Fixture>) -> axum::Json<Value> {
        axum::Json(
            json!({"object":"account", "id":if f.fault.load(Ordering::SeqCst)==8 {"acct_other"} else {"acct_fixture"}}),
        )
    }
    async fn charge(State(f): State<Fixture>) -> axum::Json<Value> {
        let count = f.charge_reads.fetch_add(1, Ordering::SeqCst);
        axum::Json(
            json!({"object":"charge", "id":"ch_original", "livemode":false,
            "amount":10000, "amount_captured":10000, "currency":"usd", "customer":"cus_original",
            "payment_intent":"pi_original", "balance_transaction":"txn_original",
            "paid":true, "captured":true, "status":"succeeded", "refunded":false, "disputed":true,
            "amount_refunded":if f.fault.load(Ordering::SeqCst)==5 && count%2==1 {2000} else {1000},
            "billing_details":{"name":"seeded-protected-customer-name"}}),
        )
    }
    async fn related(
        State(f): State<Fixture>,
        Path(resource): Path<String>,
        Query(query): Query<BTreeMap<String, String>>,
        headers: axum::http::HeaderMap,
    ) -> axum::Json<Value> {
        assert_eq!(headers["stripe-version"], "fixture.v1");
        assert!(
            headers["authorization"]
                .to_str()
                .unwrap()
                .starts_with("Bearer rk_test_")
        );
        assert_eq!(query.len(), 2);
        assert_eq!(query["charge"], "ch_original");
        assert_eq!(query["limit"], "100");
        f.requests.lock().unwrap().push(resource.clone());
        let fault = f.fault.load(Ordering::SeqCst);
        let mut rows = if resource == "refunds" {
            vec![
                json!({"object":"refund", "id":"re_original", "charge":if fault==1 {"ch_foreign"} else {"ch_original"},
                "amount":1000, "currency":"usd", "status":"succeeded", "description":"seeded-private-refund-message"}),
            ]
        } else {
            vec![
                json!({"object":"dispute", "id":"du_original", "charge":"ch_original", "livemode":fault==2,
                "amount":1000, "currency":"usd", "status":"needs_response",
                "evidence":{"customer_email_address":"seeded-private-contact@example.invalid"}}),
            ]
        };
        if fault == 4 && resource == "refunds" {
            rows.push(rows[0].clone());
        }
        axum::Json(json!({"object":if fault==7 {"customer"} else {"list"},
            "url":if fault==6 {"/v1/unrelated".to_string()} else {format!("/v1/{resource}")},
            "has_more":fault==3, "data":rows}))
    }
    let f = Fixture {
        fault: Arc::new(AtomicUsize::new(0)),
        charge_reads: Arc::new(AtomicUsize::new(0)),
        requests: Arc::new(Mutex::new(Vec::new())),
    };
    let router = Router::new()
        .route("/v1/account", get(account))
        .route("/v1/charges/ch_original", get(charge))
        .route("/v1/{resource}", get(related))
        .with_state(f.clone());
    let (origin, task) = server(router).await;
    let material = secret()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let mut client =
        Stripe::new_for_mode(format!("rk_test_{material}"), "fixture.v1".into(), false).unwrap();
    client.origin = origin;
    assert!(client.adjustments("ch_original").await.is_err());
    assert!(f.requests.lock().unwrap().is_empty());
    client.bind_account("acct_fixture").await.unwrap();
    let refs = client.adjustments("ch_original").await.unwrap();
    assert_eq!(refs.refunds, vec!["re_original"]);
    assert_eq!(refs.disputes, vec!["du_original"]);
    assert_eq!(*f.requests.lock().unwrap(), vec!["refunds", "disputes"]);
    assert_eq!(f.charge_reads.load(Ordering::SeqCst), 2);
    for fault in 1..=8 {
        f.fault.store(fault, Ordering::SeqCst);
        f.charge_reads.store(0, Ordering::SeqCst);
        assert!(
            client.adjustments("ch_original").await.is_err(),
            "fault {fault}"
        );
    }
    f.fault.store(0, Ordering::SeqCst);
    let requests = f.requests.lock().unwrap().len();
    for id in [
        "pi_original",
        "ch_original/../foreign",
        "ch_original?customer=other",
    ] {
        assert!(client.adjustments(id).await.is_err());
    }
    assert_eq!(f.requests.lock().unwrap().len(), requests);
    task.abort();
}
fn body() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "id":"evt_fixture", "object":"event", "api_version":"fixture.v1",
        "livemode":false, "created":90, "type":"checkout.session.completed",
        "data":{"object":{"id":"cs_test_fixture", "object":"checkout.session"}}
    }))
    .unwrap()
}
fn sign(bytes: &[u8], key: &[u8], timestamp: &str) -> String {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).unwrap();
    mac.update(timestamp.as_bytes());
    mac.update(b".");
    mac.update(bytes);
    let tag = mac.finalize().into_bytes();
    format!(
        "t={timestamp},v1={}",
        tag.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}
#[test]
fn raw_signature_rotation_freshness_mode_version_and_duplicate_fields_are_checked() {
    let key = secret();
    let bytes = body();
    let header = sign(&bytes, &key, "100");
    let event = verify_webhook(&bytes, &header, &key, 100, 300, "fixture.v1", false).unwrap();
    assert_eq!(event.object, "cs_test_fixture");
    assert_eq!(event.body_sha256, format!("{:x}", Sha256::digest(&bytes)));
    let rotated = format!(
        "t=100,v1={},{}",
        "0".repeat(64),
        header.split_once(',').unwrap().1
    );
    assert!(verify_webhook(&bytes, &rotated, &key, 100, 300, "fixture.v1", false).is_ok());
    for altered in [
        format!("{header},t=100"),
        sign(&bytes, &key, "900"),
        sign(&bytes, &key, "1"),
    ] {
        assert!(verify_webhook(&bytes, &altered, &key, 400, 300, "fixture.v1", false).is_err());
    }
    assert!(verify_webhook(&bytes, &header, &secret(), 100, 300, "fixture.v1", false).is_err());
    assert!(verify_webhook(&bytes, &header, &key, 100, 300, "wrong", false).is_err());
    assert!(verify_webhook(&bytes, &header, &key, 100, 300, "fixture.v1", true).is_err());
    let mut changed = bytes.clone();
    changed.push(b' ');
    assert!(verify_webhook(&changed, &header, &key, 100, 300, "fixture.v1", false).is_err());
    let duplicate = br#"{"id":"evt_one","id":"evt_two"}"#;
    assert!(
        verify_webhook(
            duplicate,
            &sign(duplicate, &key, "100"),
            &key,
            100,
            300,
            "fixture.v1",
            false
        )
        .is_err()
    );
}
#[test]
fn native_scope_and_bounds_refuse_unrelated_connected_account_and_wrong_object() {
    let key = secret();
    for field in ["type", "account", "context", "data"] {
        let mut value: Value = serde_json::from_slice(&body()).unwrap();
        value[field] = match field {
            "type" => json!("invoice.paid"),
            "data" => json!({"object":{"object":"charge", "id":"ch_other"}}),
            _ => json!("acct_other"),
        };
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(
            verify_webhook(
                &bytes,
                &sign(&bytes, &key, "100"),
                &key,
                100,
                300,
                "fixture.v1",
                false
            )
            .is_err()
        );
    }
    let large = vec![b' '; MAX_BODY + 1];
    assert!(
        verify_webhook(
            &large,
            &sign(&large, &key, "100"),
            &key,
            100,
            300,
            "fixture.v1",
            false
        )
        .is_err()
    );
    assert!(identifier("cs_test_valid", "cs_").is_ok());
    for id in ["cs_", "cs_a/../other", "cs_a?token", "cs_a\n", "cs_é"] {
        assert!(identifier(id, "cs_").is_err());
    }
}

async fn server(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (origin, task)
}

#[test]
fn native_dispute_references_accept_current_family_and_refuse_other_objects_and_paths() {
    let key = secret();
    for id in ["du_fixture", "dp_fixture"] {
        let mut value: Value = serde_json::from_slice(&body()).unwrap();
        value["type"] = json!("charge.dispute.funds_reinstated");
        value["data"]["object"] = json!({"object":"dispute", "id":id});
        let bytes = serde_json::to_vec(&value).unwrap();
        let event = verify_webhook(
            &bytes,
            &sign(&bytes, &key, "100"),
            &key,
            100,
            300,
            "fixture.v1",
            false,
        )
        .unwrap();
        assert_eq!(event.object, id);
        value["data"]["object"]["object"] = json!("charge");
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(
            verify_webhook(
                &bytes,
                &sign(&bytes, &key, "100"),
                &key,
                100,
                300,
                "fixture.v1",
                false,
            )
            .is_err()
        );
    }
    for id in [
        "du_",
        "du_a/../other",
        "du_a?token",
        "du_a\n",
        "du_é",
        "ch_other",
    ] {
        assert!(identifier(id, "dp_").is_err());
    }
}
#[tokio::test]
async fn actual_native_api_checks_account_identity_bounds_and_never_follows_redirects() {
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    let (other, other_task) = server(Router::new().fallback(get(move || {
        observed.fetch_add(1, Ordering::SeqCst);
        async { "credential must not reach this endpoint" }
    })))
    .await;
    let destination = other.clone();
    let router = Router::new()
        .route(
            "/v1/account",
            get(|| async { axum::Json(json!({"object":"account", "id":"acct_fixture"})) }),
        )
        .route(
            "/v1/charges/ch_wrong",
            get(|| async { axum::Json(json!({"object":"charge", "id":"ch_substituted"})) }),
        )
        .route(
            "/v1/charges/ch_redirect",
            get(move || {
                let destination = destination.clone();
                async move {
                    Response::builder()
                        .status(307)
                        .header("location", destination)
                        .body(Body::empty())
                        .unwrap()
                }
            }),
        )
        .route(
            "/v1/charges/ch_large",
            get(|| async { vec![b' '; MAX_BODY + 1] }),
        )
        .route(
            "/v1/disputes/du_fixture",
            get(|| async { axum::Json(json!({"object":"dispute", "id":"du_fixture"})) }),
        )
        .route(
            "/v1/disputes/du_wrong",
            get(|| async { axum::Json(json!({"object":"dispute", "id":"du_other"})) }),
        );
    let (origin, task) = server(router).await;
    let key = secret()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let mut client = Stripe::new(key, "fixture.v1".into()).unwrap();
    client.origin = origin;
    assert!(client.account("acct_fixture").await.is_ok());
    assert!(client.account("acct_other").await.is_err());
    assert!(client.get("charges", "ch_wrong").await.is_err());
    assert!(client.get("charges", "ch_redirect").await.is_err());
    assert!(client.get("charges", "ch_large").await.is_err());
    assert!(client.get("disputes", "du_fixture").await.is_ok());
    assert!(client.get("disputes", "du_wrong").await.is_err());
    assert!(client.get("../other", "ch_wrong").await.is_err());
    assert_eq!(count.load(Ordering::SeqCst), 0);
    task.abort();
    other_task.abort();
}

#[tokio::test]
async fn actual_checkout_creation_is_native_account_bound_one_time_exact_and_retry_identical() {
    use axum::{
        extract::State,
        http::{HeaderMap, Uri},
        routing::post,
    };
    use std::{collections::BTreeMap, sync::Mutex};
    #[derive(Clone)]
    struct Fixture {
        requests: Arc<Mutex<Vec<(String, BTreeMap<String, String>, String)>>>,
        fault: Arc<AtomicUsize>,
    }
    async fn create(
        State(f): State<Fixture>,
        uri: Uri,
        headers: HeaderMap,
        bytes: axum::body::Bytes,
    ) -> axum::Json<Value> {
        assert_eq!(headers["stripe-version"], "fixture.v1");
        assert!(
            headers["authorization"]
                .to_str()
                .unwrap()
                .starts_with("Bearer rk_test_")
        );
        let encoded = std::str::from_utf8(&bytes).unwrap();
        let fields = reqwest::Url::parse(&format!("https://fixture.invalid/?{encoded}"))
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect::<BTreeMap<_, _>>();
        let idempotency = headers["idempotency-key"].to_str().unwrap().to_string();
        f.requests
            .lock()
            .unwrap()
            .push((uri.path().into(), fields.clone(), idempotency));
        if uri.path() == "/v1/customers" {
            assert_eq!(fields.len(), 1);
            return axum::Json(
                json!({"object":"customer", "id":"cus_fixture", "livemode":false,
                "metadata":{"oa_customer":fields["metadata[oa_customer]"]}}),
            );
        }
        assert_eq!(fields.len(), 14);
        assert_eq!(fields["mode"], "payment");
        assert_eq!(fields["payment_method_types[0]"], "card");
        assert_eq!(fields["payment_intent_data[capture_method]"], "automatic");
        assert_eq!(
            fields["payment_intent_data[metadata][oa_quote]"],
            fields["metadata[oa_quote]"]
        );
        assert_eq!(fields["line_items[0][price_data][currency]"], "usd");
        assert_eq!(fields["line_items[0][quantity]"], "1");
        let fault = f.fault.load(Ordering::SeqCst);
        let amount = fields["line_items[0][price_data][unit_amount]"]
            .parse::<u64>()
            .unwrap();
        axum::Json(
            json!({"object":"checkout.session", "id":"cs_test_fixture", "livemode":fault==2,
            "mode":"payment", "customer":fields["customer"], "currency":"usd",
            "amount_total":if fault==1 {amount+1} else {amount},
            "client_reference_id":fields["client_reference_id"],
            "metadata":{"oa_quote":fields["metadata[oa_quote]"]},
            "expires_at":fields["expires_at"].parse::<u64>().unwrap(),
            "subscription":null, "setup_intent":null,
            "url":if fault==3 {"https://unrelated.invalid/checkout"} else if fault==4 {"https://checkout.stripe.com/c/pay/cs_test_other"} else {"https://checkout.stripe.com/c/pay/cs_test_fixture#native-fragment"}}),
        )
    }
    let fixture = Fixture {
        requests: Arc::new(Mutex::new(Vec::new())),
        fault: Arc::new(AtomicUsize::new(0)),
    };
    let router = Router::new()
        .route(
            "/v1/account",
            get(|| async { axum::Json(json!({"object":"account","id":"acct_fixture"})) }),
        )
        .route("/v1/customers", post(create))
        .route("/v1/checkout/sessions", post(create))
        .with_state(fixture.clone());
    let (origin, task) = server(router).await;
    let material = secret()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let mut client =
        Stripe::new_for_mode(format!("rk_test_{material}"), "fixture.v1".into(), false).unwrap();
    client.origin = origin;
    let customer = "customer_original_fixture";
    assert!(
        client
            .create_customer(customer, "customer_request_fixture")
            .await
            .is_err()
    );
    assert!(fixture.requests.lock().unwrap().is_empty());
    assert!(client.bind_account("acct_other").await.is_err());
    client.bind_account("acct_fixture").await.unwrap();
    assert!(client.bind_account("acct_other").await.is_err());
    let value = client
        .create_customer(customer, "customer_request_fixture")
        .await
        .unwrap();
    assert_eq!(value["id"], "cus_fixture");
    let mut request = CheckoutRequest {
        customer: "cus_fixture".into(),
        quote: "checkout_quote_fixture".into(),
        amount_cents: 100,
        expires_at: 4000,
        return_origin: "https://fixture.invalid".into(),
        idempotency: "checkout_request_fixture".into(),
    };
    let first = client.create_checkout(&request, 100).await.unwrap();
    let repeated = client.create_checkout(&request, 100).await.unwrap();
    assert_eq!(first, repeated);
    let captured = fixture.requests.lock().unwrap().clone();
    assert_eq!(captured[1], captured[2]);
    assert_eq!(captured[1].2, "checkout_request_fixture");
    assert_eq!(
        captured[1].1["success_url"],
        "https://fixture.invalid/dashboard?funding=checkout_quote_fixture"
    );
    request.amount_cents = 49;
    assert!(client.create_checkout(&request, 100).await.is_err());
    request.amount_cents = 100;
    request.expires_at = 101;
    assert!(client.create_checkout(&request, 100).await.is_err());
    request.expires_at = 4000;
    request.return_origin = "https://other.invalid/?target=private".into();
    assert!(client.create_checkout(&request, 100).await.is_err());
    assert_eq!(fixture.requests.lock().unwrap().len(), captured.len());
    request.return_origin = "https://fixture.invalid".into();
    for fault in [1, 2, 3, 4] {
        fixture.fault.store(fault, Ordering::SeqCst);
        assert!(client.create_checkout(&request, 100).await.is_err());
    }
    assert!(
        Stripe::new_for_mode(format!("rk_test_{material}"), "fixture.v1".into(), true).is_err()
    );
    assert!(
        Stripe::new_for_mode(format!("rk_live_{material}"), "fixture.v1".into(), false).is_err()
    );
    assert!(Stripe::new_for_mode(material, "fixture.v1".into(), false).is_err());
    task.abort();
}
