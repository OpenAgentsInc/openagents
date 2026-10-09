//! Published fixture terms and private mutual consent use actual native owners.
use super::*;
use tenancy::accounts::referrals::commission::{self, Terms};

fn terms(version: &str) -> Terms {
    serde_json::from_value::<Terms>(json!({
        "schema":commission::SCHEMA,"version":version,"products":["plugin-call","accepted-service"],
        "base":"openagents-available-earned-share","share":{"numerator":1,"denominator":4},
        "unit":{"kind":"millisatoshis"},"conversion":"same-unit-only","rounding":"down","payout_precision":"whole-satoshi-retain-remainder",
        "hold_secs":60,"hold":"verified-earned-costs-after-hold","minimum":1000,
        "destinations":["qualified-spark","qualified-lightning-address"],
        "reversal":"verified-refund-dispute-adjusts-referrer-liability-preserves-author-shares",
        "permanence":"retain-accepted-version-until-both-reaccept",
        "attribution_conflict":"suspend-new-eligibility-retain-history",
        "exclusions":["unused-funding","promotional-free-credit","self-referral","recycled-funding","unknown-costs","unresolved-attribution"],
        "effective_from":0,"terms":"Synthetic fixture economics only. Hold verified earned share after all costs and promotions; unknown outcomes remain held. A current qualified destination and the declared minimum in the same unit are required before a later payout engine acts. Verified refunds or disputes adjust the referrer liability without reducing signed author fees. Both parties must accept any replacement terms; attribution conflict suspends new eligibility. This publication and acceptance enable no accrual or payout.","digest":""
    })).unwrap().seal().unwrap()
}
async fn admitted(d: &Deployment) -> (Joined, Joined, String) {
    let source = join(d, "Synthetic source").await;
    let customer = join(d, "Synthetic customer").await;
    let p = attribution::policy();
    let store = Accounts::open(d.dir.path()).unwrap();
    store.publish_attribution_policy(&p).unwrap();
    let r = store
        .create_referrer(
            &source.account,
            tenancy::accounts::referrals::Kind::Person,
            "Synthetic source",
        )
        .unwrap();
    let (_, pending) = post(
        d,
        "/v1/account/attribution",
        Some(&customer.key_token),
        &attribution::proposal(&p, &r.id, "prior-agreement"),
    )
    .await;
    let (status, accepted) = post(
        d,
        "/v1/account/attribution/confirm",
        Some(&source.key_token),
        &json!({"customer":customer.account,"decision":pending["referral"]["digest"]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{accepted}");
    (
        source,
        customer,
        accepted["referral"]["decision"].as_str().unwrap().into(),
    )
}
fn input(t: &Terms, customer: &str, decision: &str, request: &str) -> Value {
    json!({"request":request,"customer":customer,"terms_digest":t.digest,"attribution_decision":decision,"consent":true})
}

#[tokio::test]
async fn native_terms_and_mutual_acceptance_preserve_version_privacy_and_central_author_fees() {
    let mut d = deploy(Some(account_config(None)), true).await;
    let (source, customer, decision) = admitted(&d).await;
    let other = join(&d, "Unrelated customer").await;
    let t = terms("one");
    let store = Accounts::open(d.dir.path()).unwrap();
    store.publish_commission_terms(&t, &t.digest, None).unwrap();
    let sdk = jev::Client::new(
        jev::Config::new()
            .base_url(&d.address)
            .api_key(customer.key_token.as_str()),
    )
    .unwrap();
    let account = sdk.account().for_referrals_account(&customer.account);
    let publication = account.commission_terms(None).await.unwrap().unwrap();
    assert_eq!(publication.terms["digest"], t.digest);
    assert!(
        !serde_json::to_string(&publication)
            .unwrap()
            .contains(&customer.account)
    );
    let v: jev::CommissionInput =
        serde_json::from_value(input(&t, &customer.account, &decision, "customer-consent"))
            .unwrap();
    let pending = account.accept_commission_terms(&v).await.unwrap();
    assert!(!pending.terms_qualified);
    assert!(!pending.accrual_enabled);
    let (status, accepted) = post(
        &d,
        "/v1/account/referral-agreement",
        Some(&source.key_token),
        &input(&t, &customer.account, &decision, "source-consent"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{accepted}");
    let accepted = &accepted["referral"];
    assert!(
        accepted.get("current_referrer_owner").is_none(),
        "{accepted}"
    );
    let agreement = accepted["agreement"]["id"].as_str().unwrap();
    assert_eq!(accepted["terms_qualified"], true);
    assert_eq!(accepted["accrual_enabled"], false);
    assert_eq!(accepted["payout_qualified"], false);
    assert_eq!(accepted["payout_enabled"], false);
    let route = format!(
        "/v1/account/referral-agreement?customer={}&agreement={agreement}",
        customer.account
    );
    let (status, denied) = get(&d, &route, &other.key_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(!denied.to_string().contains(&customer.account));
    assert!(!denied.to_string().contains(agreement));
    let (status, _) = exchange(
        reqwest::Client::new()
            .get(format!("{}/v1/account/referral-terms", d.address))
            .bearer_auth(&customer.key_token)
            .header("x-openagents-referral-account", &other.account),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let next = terms("two");
    store
        .publish_commission_terms(&next, &next.digest, Some(&t.digest))
        .unwrap();
    assert_eq!(
        account
            .commission_agreement(&customer.account, Some(agreement))
            .await
            .unwrap()
            .unwrap()
            .terms
            .terms["digest"],
        t.digest
    );
    assert_eq!(
        account.commission_terms(None).await.unwrap().unwrap().terms["digest"],
        next.digest
    );
    attribution::restart(&mut d).await;
    assert_eq!(
        get(&d, &route, &customer.key_token).await.1["referral"],
        *accepted
    );

    // This is an actual central-ledger settlement with synthetic funding, not
    // evidence that a commercial customer paid. The contract preview is inert.
    let mut ledger = pay_ledger::Ledger::open(d.dir.path().join("synthetic-pay.sqlite")).unwrap();
    let record = ledger
        .record_settlement(pay_ledger::SettlementInput {
            key: "synthetic-rev29-payment".into(),
            resource: "synthetic-plugin".into(),
            plugin_id: Some("synthetic-plugin".into()),
            release_id: Some("synthetic-release".into()),
            price_msat: 20_000,
            received_msat: 20_000,
            rail: pay_ledger::Rail::Lightning,
            payer_alias: None,
            settled_at: 2_000_000_000,
            split: pay_ledger::Split::Plugin {
                author: "synthetic-author".into(),
                fee_msat: 7_000,
            },
        })
        .unwrap();
    let author = record
        .shares
        .iter()
        .filter(|s| s.party == "synthetic-author")
        .map(|s| s.amount_msat as u64)
        .sum::<u64>();
    let oa = record
        .shares
        .iter()
        .filter(|s| s.party == pay_ledger::OPENAGENTS)
        .map(|s| s.amount_msat as u64)
        .sum::<u64>();
    let before = ledger.available_shares("synthetic-author").unwrap();
    let facts = commission::EconomicFacts {
        product: commission::Product::PluginCall,
        unit: tenancy::money::funding::Unit::Millisatoshis,
        earned: true,
        admitted_attribution_verified: true,
        promotional_or_free: false,
        self_or_recycled: false,
        settled_distributable: record.received_msat as u64,
        author_resource_shares: author,
        openagents_share: oa,
        costs: [Some(1000), Some(1000), Some(0), Some(0), Some(0), Some(0)],
        promotions: 1000,
    };
    let preview = t.preview(&facts).unwrap().unwrap();
    assert_eq!(
        (author, preview.commission, preview.openagents_remaining),
        (7000, 2500, 7500)
    );
    assert_eq!(
        preview.author_resource_shares
            + preview.costs
            + preview.promotions
            + preview.commission
            + preview.openagents_remaining,
        record.received_msat as u64
    );
    assert_eq!(ledger.available_shares("synthetic-author").unwrap(), before);
    assert_eq!(ledger.accrued("synthetic-referrer").unwrap(), 0);
    assert!(!preview.accrual_enabled);
    d.server.abort();
}
async fn referral(
    cli: &super::super::team::Installed,
    cmd: &str,
    input: Option<Value>,
    options: &[(&str, &str)],
) -> (bool, Value) {
    let mut words = vec!["referral".into(), cmd.into()];
    if let Some(value) = input {
        let file = cli.directory.path().join("referral-input.json");
        super::super::team::private_file(&file, &serde_json::to_vec(&value).unwrap());
        words.extend(["--input".into(), file.to_str().unwrap().into()]);
    }
    for (name, value) in options {
        words.extend([format!("--{name}"), (*value).into()]);
    }
    cli.run(words).await
}
#[tokio::test]
#[ignore = "requires freshly built installed customer and explicit operator binaries"]
async fn installed_terms_check_publication_consent_and_recovery_preserve_exact_old_agreement() {
    use super::super::team::{Installed, private_file};
    use std::os::unix::fs::PermissionsExt;
    let binary =
        std::env::var_os("OPENAGENTS_REV29_TEST_CLI").expect("Set OPENAGENTS_REV29_TEST_CLI.");
    let operator =
        std::env::var_os("OPENAGENTS_REV29_POLICY_CLI").expect("Set OPENAGENTS_REV29_POLICY_CLI.");
    let mut d = deploy(Some(account_config(None)), true).await;
    std::fs::set_permissions(d.dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let (source, customer, decision) = admitted(&d).await;
    let money = attribution::configure_money(&mut d, &[&source, &customer]);
    attribution::restart(&mut d).await;
    let unchanged_money = std::fs::read(&money).unwrap();
    let a = Installed::new(&binary);
    let b = Installed::new(&binary);
    a.import("key", &source.key_token).await;
    b.import("key", &customer.key_token).await;
    a.select(&d.address, &source.account, &source.workspace, "key")
        .await;
    b.select(&d.address, &customer.account, &customer.workspace, "key")
        .await;
    let t = terms("installed-one");
    let file = a.directory.path().join("commission.json");
    let mut draft = t.clone();
    draft.digest.clear();
    private_file(&file, &serde_json::to_vec(&draft).unwrap());
    let operate = |words: &[&str]| {
        let o = std::process::Command::new(&operator)
            .args(words)
            .env("HOME", a.directory.path())
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&o.stdout)
            .unwrap_or_else(|_| json!({"stderr":String::from_utf8_lossy(&o.stderr)}));
        (o.status.success(), v)
    };
    let sequence = Accounts::open(d.dir.path())
        .unwrap()
        .store()
        .unwrap()
        .sequence;
    let (ok, checked) = operate(&["commission-check", "--input", file.to_str().unwrap()]);
    assert!(ok, "{checked}");
    assert_eq!(checked["terms"]["digest"], t.digest);
    assert_eq!(checked["published"], false);
    assert_eq!(
        Accounts::open(d.dir.path())
            .unwrap()
            .store()
            .unwrap()
            .sequence,
        sequence
    );
    let mut usd = t.clone();
    usd.unit = tenancy::money::funding::Unit::CurrencyMillionths {
        currency: "USD".into(),
    };
    usd.digest.clear();
    private_file(&file, &serde_json::to_vec(&usd).unwrap());
    assert!(!operate(&["commission-check", "--input", file.to_str().unwrap()]).0);
    assert_eq!(
        Accounts::open(d.dir.path())
            .unwrap()
            .store()
            .unwrap()
            .sequence,
        sequence
    );
    private_file(&file, &serde_json::to_vec(&t).unwrap());
    let publish = |approve: &str, expected: &str| {
        operate(&[
            "commission-publish",
            "--registry",
            d.dir.path().to_str().unwrap(),
            "--input",
            file.to_str().unwrap(),
            "--approve",
            approve,
            "--expected",
            expected,
        ])
    };
    assert!(!publish("wrong", "none").0);
    let (ok, published) = publish(&t.digest, "none");
    assert!(ok, "{published}");
    let (ok, read) = referral(&b, "terms", None, &[]).await;
    assert!(ok, "{read}");
    assert_eq!(read["terms"]["digest"], t.digest);
    let (ok, first) = referral(
        &b,
        "accept-terms",
        Some(input(&t, &customer.account, &decision, "customer")),
        &[],
    )
    .await;
    assert!(ok, "{first}");
    assert_eq!(first["terms_qualified"], false);
    let (ok, accepted) = referral(
        &a,
        "accept-terms",
        Some(input(&t, &customer.account, &decision, "source")),
        &[],
    )
    .await;
    assert!(ok, "{accepted}");
    assert_eq!(accepted["terms_qualified"], true);
    assert_eq!(accepted["accrual_enabled"], false);
    assert_eq!(accepted["payout_qualified"], false);
    assert_eq!(accepted["payout_enabled"], false);
    let agreement = accepted["agreement"]["id"].as_str().unwrap();
    let next = terms("installed-two");
    private_file(&file, &serde_json::to_vec(&next).unwrap());
    assert!(publish(&next.digest, &t.digest).0);
    assert_eq!(
        referral(
            &b,
            "commission",
            None,
            &[("customer", &customer.account), ("agreement", agreement)]
        )
        .await
        .1,
        accepted
    );
    let (status, recovery) = post(
        &d,
        &format!("/v1/workspaces/{}/recovery", customer.workspace),
        Some(&customer.key_token),
        &json!({"account":customer.account}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{recovery}");
    let secret = b.directory.path().join("recovery.secret");
    private_file(&secret, recovery["token"].as_str().unwrap().as_bytes());
    let intent = b.directory.path().join("recover.json");
    private_file(&intent,&serde_json::to_vec(&json!({"id":"recover-agreement-reader","origin":d.address,"account":customer.account,"credential_alias":null,"action":{"kind":"recover","workspace":customer.workspace,"output_alias":"restored"}})).unwrap());
    b.ok(vec![
        "change".into(),
        "--input".into(),
        intent.to_str().unwrap().into(),
        "--recovery-token".into(),
        secret.to_str().unwrap().into(),
    ])
    .await;
    assert!(
        !referral(
            &b,
            "commission",
            None,
            &[("customer", &customer.account), ("agreement", agreement)]
        )
        .await
        .0
    );
    b.select(
        &d.address,
        &customer.account,
        &customer.workspace,
        "restored",
    )
    .await;
    assert_eq!(
        referral(
            &b,
            "commission",
            None,
            &[("customer", &customer.account), ("agreement", agreement)]
        )
        .await
        .1,
        accepted
    );
    attribution::restart(&mut d).await;
    assert_eq!(
        referral(
            &b,
            "commission",
            None,
            &[("customer", &customer.account), ("agreement", agreement)]
        )
        .await
        .1,
        accepted
    );
    assert_eq!(
        referral(&b, "terms", None, &[]).await.1["terms"]["digest"],
        next.digest
    );
    assert_eq!(std::fs::read(money).unwrap(), unchanged_money);
    d.server.abort();
}
