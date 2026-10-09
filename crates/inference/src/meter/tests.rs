use std::sync::Arc;

use super::*;

const NOW: u64 = 20_735 * DAY_MS + 12 * 3_600_000; // 2026-10-09 12:00 UTC

fn config() -> Config {
    Config {
        rates: vec![RateRow {
            upstream: "zai".into(),
            model: "zai/glm-5.3-flash".into(),
            currency: "USD".into(),
            input: 150_000,
            cached_input: Some(30_000),
            cache_write: None,
            output: 500_000,
            margin_bps: 500,
            promotion: None,
        }],
        accounts: vec![CreditAccount {
            id: "zai".into(),
            upstream: "zai".into(),
            currency: "USD".into(),
            granted: 1_300_000,
            balance: 1_300_000,
            expires_at_ms: None,
            basis: Basis::Prepaid,
        }],
        ..Config::default()
    }
}

/// What a fake adapter reports for one answered GLM call.
fn glm(at_ms: u64, input: u64, output: u64) -> Attempt {
    Attempt {
        account: Some("zai".into()),
        class: Some("fast".into()),
        first_token_ms: Some(300),
        total_ms: 1_300,
        tokens: Tokens {
            input,
            output,
            ..Tokens::default()
        },
        ..Attempt::new("req", 1, "zai", "zai/glm-5.3-flash", at_ms)
    }
}

fn meter() -> (Meter, Arc<CollectAlerts>) {
    let alerts = Arc::new(CollectAlerts::default());
    (
        Meter::with_sink(&config(), Box::new(alerts.clone())),
        alerts,
    )
}

#[test]
fn records_price_burn_down_and_alert_at_half() {
    let (meter, alerts) = meter();
    // 1M in + 1M out = $0.65 of a $1.30 credit: half left.
    meter.record(glm(NOW, 1_000_000, 1_000_000));
    let status = meter.status(NOW + 2_000).unwrap();
    assert_eq!(status.v, STATUS_SCHEMA);
    assert_eq!(status.accounts[0].balance, 650_000);
    assert_eq!(status.accounts[0].spent_today, 650_000);
    let fired = alerts.taken();
    assert!(fired.iter().any(|a| matches!(
        a,
        Alert::Remaining {
            threshold_pct: 50,
            ..
        }
    )));
    assert!(fired.iter().any(|a| matches!(a, Alert::Runway { .. })));
    let five = &status.rates["5m"];
    assert_eq!(five.len(), 1);
    assert_eq!(five[0].ttft_p50_ms, Some(300));
    assert_eq!(five[0].cost_per_million, Some(325_000));
    assert_eq!(status.records.kept, 1);
    // The serialized status carries no text fields from the request.
    let json = serde_json::to_string(&status).unwrap();
    assert!(json.contains("\"remaining\""));
}

#[test]
fn windows_roll() {
    let (meter, _) = meter();
    meter.record(glm(NOW - 2 * 3_600_000, 10, 10));
    meter.record(glm(NOW - 30 * 60_000, 10, 10));
    meter.record(glm(NOW, 10, 10));
    let status = meter.status(NOW + 2_000).unwrap();
    assert_eq!(status.rates["5m"][0].attempts, 1);
    assert_eq!(status.rates["1h"][0].attempts, 2);
    assert_eq!(status.rates["24h"][0].attempts, 3);
    // Past the retention, nothing is kept.
    let later = meter.status(NOW + 26 * 3_600_000).unwrap();
    assert_eq!(later.records.kept, 0);
    assert!(later.rates["24h"].is_empty());
}

#[test]
fn unknown_models_are_kept_unpriced() {
    let (meter, _) = meter();
    let mut attempt = glm(NOW, 10, 10);
    attempt.model = "zai/other".into();
    meter.record(attempt);
    let status = meter.status(NOW).unwrap();
    assert_eq!(status.records.unpriced, 1);
    assert_eq!(status.accounts[0].balance, 1_300_000);
}

#[test]
fn daily_reconciliation_alerts_on_a_gap_over_two_percent() {
    let (meter, alerts) = meter();
    let yesterday = NOW / DAY_MS - 1;
    meter.record(glm(NOW - DAY_MS, 100_000, 100_000)); // 65,000 micros
    let billing = FakeBilling::new("zai-console");
    billing.bill(
        "zai",
        yesterday,
        Billed {
            cost: 66_000, // 1.5% off: passes
            currency: "USD".into(),
            balance: Some(1_234_000),
        },
    );
    meter.tick(&[&billing], NOW);
    let status = meter.status(NOW).unwrap();
    assert_eq!(status.reconciled_day.as_deref(), Some("2026-10-08"));
    assert!(status.reconciliations[0].ok);
    assert_eq!(status.accounts[0].balance, 1_234_000);
    assert!(
        !alerts
            .taken()
            .iter()
            .any(|a| matches!(a, Alert::ReconcileGap { .. }))
    );
    // A second tick the same day does nothing more.
    billing.bill(
        "zai",
        yesterday,
        Billed {
            cost: 90_000,
            currency: "USD".into(),
            balance: None,
        },
    );
    meter.tick(&[&billing], NOW + 3_600_000);
    assert!(meter.status(NOW).unwrap().reconciliations[0].ok);
    // Run explicitly: 27.8% off alerts, once.
    let results = meter.reconcile(&[&billing], yesterday, NOW);
    assert!(!results[0].ok);
    meter.reconcile(&[&billing], yesterday, NOW);
    let gaps = alerts
        .taken()
        .into_iter()
        .filter(|a| matches!(a, Alert::ReconcileGap { .. }))
        .count();
    assert_eq!(gaps, 1);
    assert!(
        meter
            .status(NOW)
            .unwrap()
            .alerts
            .iter()
            .any(|a| matches!(a, Alert::ReconcileGap { .. }))
    );
}

#[test]
fn upstream_reported_cost_is_reconciled_and_unreachable_billing_is_reported() {
    let (meter, alerts) = meter();
    let yesterday = NOW / DAY_MS - 1;
    let mut attempt = glm(NOW - DAY_MS, 100_000, 100_000);
    attempt.reported_cost = Some(80_000); // ours is 65,000
    meter.record(attempt);
    let billing = FakeBilling::new("zai-console");
    billing.bill(
        "zai",
        yesterday,
        Billed {
            cost: 65_000,
            currency: "USD".into(),
            balance: None,
        },
    );
    billing.fail(true);
    let results = meter.reconcile(&[&billing], yesterday, NOW);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].trouble.as_deref(), Some("billing unreachable"));
    assert_eq!(results[1].source, "upstream-reported");
    assert!(!results[1].ok);
    assert!(alerts.taken().iter().any(|a| matches!(
        a,
        Alert::ReconcileGap { source, .. } if source == "upstream-reported"
    )));
}

#[test]
fn expiry_alerts_without_new_attempts() {
    let mut config = config();
    config.accounts[0].expires_at_ms = Some(NOW + 10 * DAY_MS);
    let alerts = Arc::new(CollectAlerts::default());
    let meter = Meter::with_sink(&config, Box::new(alerts.clone()));
    meter.check(NOW);
    assert!(
        alerts
            .taken()
            .iter()
            .any(|a| matches!(a, Alert::ExpiringUnspent { .. }))
    );
}

#[test]
fn records_never_carry_text() {
    // The record's fields are ids, fixed words, counts, and amounts; a
    // serialized attempt has exactly these keys.
    let mut attempt = glm(NOW, 1, 1);
    attempt.tenant = Some("t".into());
    attempt.key_id = Some("k".into());
    attempt.error = Some(ErrorClass::Server);
    attempt.upstream_status = Some(500);
    attempt.reported_cost = Some(1);
    attempt.cost = Some(1);
    attempt.margin = Some(0);
    attempt.price = Some(1);
    attempt.currency = "USD".into();
    let value = serde_json::to_value(&attempt).unwrap();
    let mut keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "account",
            "api",
            "at_ms",
            "attempt",
            "class",
            "cost",
            "currency",
            "error",
            "first_token_ms",
            "key_id",
            "margin",
            "model",
            "outcome",
            "price",
            "queue_ms",
            "reported_cost",
            "request_id",
            "requested_model",
            "tenant",
            "tokens",
            "tokens_counted",
            "total_ms",
            "traffic",
            "upstream",
            "upstream_status",
            "usage_reported",
        ]
    );
}

#[test]
fn traffic_labels_are_closed_values_and_old_records_are_unknown() {
    let value = serde_json::to_value(Traffic::default()).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"audience":"unknown", "payment":"unknown", "synthetic":false})
    );
    let mut old = serde_json::to_value(glm(NOW, 1, 1)).unwrap();
    old.as_object_mut().unwrap().remove("traffic");
    old.as_object_mut().unwrap().remove("usage_reported");
    let attempt: Attempt = serde_json::from_value(old).unwrap();
    assert_eq!(attempt.traffic, Traffic::default());
    assert!(!attempt.usage_reported);
    assert!(
        serde_json::from_value::<Traffic>(
            serde_json::json!({"audience":"prompt text", "payment":"paid", "synthetic":false})
        )
        .is_err()
    );
}
