//! WEB-11 acceptance: original statements keep native units and records,
//! decision resources show their exact native payer and price, and every
//! read binds the current session, workspace, and membership.

use super::*;

const HASH_A: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn source(product: &str) -> Value {
    json!({"product":product,"issuer":"synthetic-issuer","account":"alice","workspace":"alice-personal"})
}

fn commercial(product: &str) -> Value {
    json!({"binding":format!("{product}-binding"),"revision":2,"digest":HASH_A,"customer":"customer-a","workspace":"canonical-a","source":source(product)})
}

fn conversion(source: Value) -> Value {
    json!({"version":"rate-7","source":source,"target":{"kind":"millisatoshis"},"numerator":3,"denominator":2,"source_ref":"rate-evidence-7","valid_from":1,"valid_until":4_000_000_000u64,"rounding":"down","fee_payer":"customer","max_fee_units":10})
}

fn row(key: &str, product: &str, unit: Value, scale: u64) -> Value {
    json!({"key":key,"kind":"liability","state":"settled","source":source(product),
        "commercial":commercial(product),"binding":format!("{product}-binding"),"unit":unit.clone(),
        "unit_scale":scale,"conversion":conversion(unit),"native_attempt":null,"intent_digest":null,
        "quote":null,"execution":null,"terms":null,"payment_reference":null,"units":null,
        "reserved_msat":0,"charged_msat":null,"credited_msat":null,"released_msat":0,
        "returned_msat":0,"loss_msat":0,"recovered_msat":0,"reduced_claim_msat":0,"fee_msat":null,
        "remainder":null,"denominator":null,"source_evidence":null,"settlement_reference":null,
        "allocation_rule_version":null,"allocations":[],"disclosure":[]})
}

/// One page of the original joined statement with unlike units.
fn statement() -> Value {
    let mut decision = row(
        "charge:decision-1",
        "gateway",
        json!({"kind":"currency-millionths","currency":"USD"}),
        1_000_000,
    );
    decision["native_attempt"] = json!("attempt-1");
    decision["quote"] = json!("quote-1");
    decision["terms"] = json!("terms-1");
    decision["units"] = json!(1_500_000);
    decision["reserved_msat"] = json!(5_000);
    decision["charged_msat"] = json!(3_000);
    decision["released_msat"] = json!(2_000);
    decision["remainder"] = json!(1);
    decision["denominator"] = json!(3);
    decision["fee_msat"] = json!(4);
    let mut plugin = row(
        "charge:plugin-1",
        "plugin",
        json!({"kind":"satoshis"}),
        100_000_000,
    );
    plugin["units"] = json!(21);
    plugin["terms"] = json!("release-terms-1");
    plugin["charged_msat"] = json!(21_000);
    plugin["reserved_msat"] = json!(21_000);
    plugin["allocations"] = json!([{"role":"author","party_reference":HASH_A,"amount_msat":700,
        "state":"original_declared_external_author_fee","plugin":"meeting-action-items",
        "release":"release-3184","merchant_reference":HASH_A,"payouts":[]}]);
    plugin["disclosure"] = json!([
        "external_receiver_accrual_and_author_payout_require_the_original_receiver_statement"
    ]);
    let mut unknown = row(
        "charge:decision-2",
        "gateway",
        json!({"kind":"currency-millionths","currency":"USD"}),
        1_000_000,
    );
    unknown["state"] = json!("unknown");
    unknown["reserved_msat"] = json!(9_000);
    json!({"native_workspace":"alice-personal","statement":{"schema":"openagents.joined-statement.v1",
        "origin":"canonical-origin","customer":"customer-a","workspace":"canonical-a",
        "unit":{"kind":"millisatoshis"},"unit_scale":100_000_000_000u64,
        "balance":{"credited_msat":50_000,"available_msat":17_000,"held_msat":9_000,"settled_msat":24_000,"released_msat":2_000},
        "snapshot":"b".repeat(64),"rows":[decision,plugin,unknown],"next":"c0ffee","scanned":3,
        "disclosure":["unused_release_is_not_a_refund"]},
        "payee":{"party":"alice-payee","unit":{"kind":"millisatoshis"},"statement":{"earnings":[{"amount_msat":700}],"payouts":[]}},
        "payee_disclosure":"Payee earnings remain separate.",
        "source_attribution":[{"binding":"gateway-binding","current_original_mapping":true},{"binding":"plugin-binding","current_original_mapping":false}],
        "attribution_disclosure":"A false mapping marks a historical original source.",
        "native_projection":[{"key":"charge:decision-1","source_head":"head-9","price":{},"cost":{"phase":"settled","reserved":5,"retail":3,"price_version":"price-1"},"receipt":HASH_A,"disclosure":"x"}],
        "native_projection_disclosure":"Missing costs remain unknown."})
}

fn context(epoch: u64) -> Value {
    json!({"schema":receipts::purchase::SCHEMA,"account":"alice","workspace":"alice-personal",
        "payer_workspace":"alice-personal","tenant":"synthetic","credential_reference":"session-a",
        "membership_epoch":1,"workspace_members_epoch":epoch,"role":"owner","door":"decision-a",
        "registry_digest":HASH_A,"artifact_digest":HASH_A,
        "price":{"version":"price-1","currency":"USD","policy":"observed-usage-v1","terms_digest":HASH_A,"maximum_usage_digest":HASH_A,"maximum_charge":250},
        "can_invoke":true})
}

fn receipt(model: &str) -> Value {
    let mut receipt =
        receipts::execution::ExecutionReceipt::for_attempt("http", "request-1", 1, HASH_A);
    receipt.attempt_id = "attempt-1".into();
    receipt.workspace = Some("alice-personal".into());
    receipt.requested.model = model.into();
    receipt.served.model = model.into();
    receipt.outcome = receipts::execution::Outcome::Answered;
    receipt.seal();
    json!({"receipt":receipt,"cost":{"reserved":250,"retail":120,"phase":"settled","price_version":"price-1"}})
}

async fn get(fixture: &Fixture, cookies: &Cookies, path: &str) -> Answer {
    request(&fixture.site, Method::GET, path, cookies, None, None).await
}

#[tokio::test]
async fn statements_keep_original_units_and_records_without_summing() {
    let fixture = fixture().await;
    // The fixture rows are the native owner's exact record shape.
    for row in statement()["statement"]["rows"].as_array().unwrap() {
        serde_json::from_value::<pay_ledger::shared::statement::StatementRow>(row.clone()).unwrap();
    }
    fixture.state.lock().unwrap().statement = Some(statement());
    let mut cookies = login(&fixture, "alice").await;

    // Without a selected workspace, billing names why it is unavailable.
    let overview = get(&fixture, &cookies, "/cloud/app").await;
    assert!(overview.body.contains("Billing · Unavailable"));
    assert_eq!(
        get(&fixture, &cookies, "/cloud/app/billing/statements")
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    choose_personal(&fixture, &mut cookies).await;
    let overview = get(&fixture, &cookies, "/cloud/app").await;
    assert!(overview.body.contains("href=\"/cloud/app/billing\""));
    let index = get(&fixture, &cookies, "/cloud/app/billing").await;
    assert_eq!(index.status, StatusCode::OK, "{}", index.body);
    assert!(index.body.contains("Retail compute · Unavailable"));

    let page = get(&fixture, &cookies, "/cloud/app/billing/statements").await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    private(&page);
    let body = &page.body;
    // Each unit stays its own: USD millionths, satoshis, and millisatoshis.
    assert!(body.contains("<dt>Source units</dt><dd>1500000 millionths of USD</dd>"));
    assert!(body.contains("<dt>Source units</dt><dd>21 sat (BTC)</dd>"));
    assert!(body.contains("<dt>Charged</dt><dd>3000 msat</dd>"));
    assert!(body.contains("<dt>Charged</dt><dd>21000 msat</dd>"));
    // Nothing is summed across rows or units.
    for sum in ["1500021", "1500000.021", "1521000"] {
        assert!(!body.contains(sum), "{sum}");
    }
    assert!(body.contains("<dt>settled</dt><dd>24000 msat</dd>"));
    // An unknown charge stays unknown; an unused release is not a refund.
    assert!(body.contains("<dt>Charged</dt><dd>unknown</dd>"));
    assert!(body.contains("<dt>Unused hold released</dt><dd>2000 msat (not a refund)</dd>"));
    assert!(body.contains("<dt>Remainder</dt><dd>1/3 of one msat, never spendable credit</dd>"));
    // Plugin release, author fee, and payee earnings are distinct records.
    assert!(body.contains("Plugin releases"));
    assert!(
        body.contains("Author fee 700 msat · plugin meeting-action-items release release-3184")
    );
    assert!(body.contains("Party alice-payee"));
    assert!(body.contains("historical original source"));
    assert!(body.contains("attribution only, no right"));
    // The original gateway projection joins only by its own record key.
    assert!(body.contains("Original gateway projection: phase settled · reserved 5 · retail charge 3 at price version price-1"));
    assert_eq!(body.matches("Original gateway projection").count(), 1);
    assert!(body.contains("/cloud/app/billing/statements?cursor=c0ffee"));

    // Export returns the same original document, private and uncached.
    let export = get(&fixture, &cookies, "/cloud/app/billing/statements/export").await;
    assert_eq!(export.status, StatusCode::OK);
    assert_eq!(export.headers[header::CONTENT_TYPE], "application/x-ndjson");
    assert_eq!(export.headers[header::CACHE_CONTROL], "no-store, private");
    let exported: Value = serde_json::from_str(export.body.trim_end()).unwrap();
    assert_eq!(
        exported["statement"]["rows"],
        statement()["statement"]["rows"]
    );

    // Bounded cursors only.
    assert_eq!(
        get(
            &fixture,
            &cookies,
            "/cloud/app/billing/statements?cursor=zz"
        )
        .await
        .status,
        StatusCode::BAD_REQUEST
    );

    // A record this page cannot type exactly is refused, not partly shown.
    let mut changed = statement();
    changed["statement"]["rows"][0]["guessed_cost"] = json!(1);
    fixture.state.lock().unwrap().statement = Some(changed);
    let refused = get(&fixture, &cookies, "/cloud/app/billing/statements").await;
    assert_eq!(refused.status, StatusCode::CONFLICT);
    assert!(refused.body.contains("Statement record unreadable"));
    assert!(!refused.body.contains("3000 msat"));

    // Another account sees only its own unavailable lane.
    fixture.state.lock().unwrap().statement = Some(statement());
    let mut bob = login(&fixture, "bob").await;
    let page = get(&fixture, &bob, "/cloud/app").await;
    let csrf = action_token(&page.body, "/cloud/select-workspace", Some("bob-personal"));
    let answer = request(
        &fixture.site,
        Method::POST,
        "/cloud/select-workspace",
        &bob,
        Some(&form(&[("workspace", "bob-personal"), ("csrf", &csrf)])),
        Some(ORIGIN),
    )
    .await;
    bob.apply(&answer);
    let theirs = get(&fixture, &bob, "/cloud/app/billing/statements").await;
    assert_eq!(theirs.status, StatusCode::OK, "{}", theirs.body);
    private(&theirs);
    assert!(theirs.body.contains("Joined statement · Unavailable"));
    assert!(!theirs.body.contains("charge:decision-1"));

    // A revoked session cannot read any billing record.
    fixture.state.lock().unwrap().revoked.insert("alice".into());
    let revoked = get(&fixture, &cookies, "/cloud/app/billing/statements").await;
    assert_ne!(revoked.status, StatusCode::OK);
    assert!(!revoked.body.contains("charge:decision-1"));
}

#[tokio::test]
async fn decision_resources_show_exact_payer_price_and_original_receipts() {
    let fixture = fixture().await;
    {
        let mut state = fixture.state.lock().unwrap();
        state.decision = Some(context(3));
        state.receipt = Some(receipt("decision-a"));
    }
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    let page = get(
        &fixture,
        &cookies,
        "/cloud/app/billing/decisions?door=decision-a",
    )
    .await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    private(&page);
    assert!(page.body.contains(
        "<dt>Maximum charge</dt><dd>250 integer units of USD under price version price-1</dd>"
    ));
    assert!(
        page.body
            .contains("Current: this account may invoke after approving an exact quote")
    );
    assert!(
        page.body
            .contains("<dt>Payer workspace</dt><dd>alice-personal</dd>")
    );
    // The browser offers no invocation, payment, or approval.
    assert!(
        !page
            .body
            .contains("method=\"post\" action=\"/cloud/app/billing")
    );

    // An unadmitted resource is unavailable, never estimated.
    let other = get(
        &fixture,
        &cookies,
        "/cloud/app/billing/decisions?door=decision-b",
    )
    .await;
    assert_eq!(other.status, StatusCode::OK);
    assert!(other.body.contains("Purchase · Unavailable"));
    assert!(!other.body.contains("Maximum charge"));
    assert_eq!(
        get(&fixture, &cookies, "/cloud/app/billing/decisions?door=../x")
            .await
            .status,
        StatusCode::BAD_REQUEST
    );

    // Recovery reads the original receipt and its settlement once.
    let digest = fixture.state.lock().unwrap().receipt.as_ref().unwrap()["receipt"]["digest"]
        .as_str()
        .unwrap()
        .to_owned();
    let path = format!("/cloud/app/billing/decisions/decision-a/receipt?digest={digest}");
    let recovered = get(&fixture, &cookies, &path).await;
    assert_eq!(recovered.status, StatusCode::OK, "{}", recovered.body);
    assert!(
        recovered
            .body
            .contains("<dt>Charge</dt><dd>120 at price version price-1</dd>")
    );
    assert!(recovered.body.contains("<dt>Outcome</dt><dd>answered</dd>"));
    let again = get(&fixture, &cookies, &path).await;
    assert_eq!(again.body, recovered.body);
    // A receipt for another resource, or an unknown digest, is refused.
    assert_eq!(
        get(
            &fixture,
            &cookies,
            &format!("/cloud/app/billing/decisions/decision-b/receipt?digest={digest}")
        )
        .await
        .status,
        StatusCode::CONFLICT
    );
    assert_eq!(
        get(
            &fixture,
            &cookies,
            &format!("/cloud/app/billing/decisions/decision-a/receipt?digest={HASH_A}")
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );

    // A context naming another membership epoch is a changed qualification.
    fixture.state.lock().unwrap().decision = Some(context(9));
    assert_eq!(
        get(
            &fixture,
            &cookies,
            "/cloud/app/billing/decisions?door=decision-a"
        )
        .await
        .status,
        StatusCode::CONFLICT
    );
}
