//! The privileged operator CLI writes and reads the same policy-enforced ledger.

use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use tenancy::money::funding::{self, FeePayer, Finality, Policy, Rounding, Unit};
use tenancy::money::{CreditKind, Mutation, Operation, Price, Rate, Resource};

fn invoke(root: &std::path::Path, source: &str, operation: Operation) -> Output {
    let input = root.join("mutation.json");
    let mutation = Mutation {
        workspace: "buyer".into(),
        source: source.into(),
        audit: format!("fixture:{source}"),
        operation,
    };
    std::fs::write(&input, serde_json::to_vec(&mutation).unwrap()).unwrap();
    Command::new(env!("CARGO_BIN_EXE_tenant-money"))
        .args([
            "--ledger",
            root.join("money.jsonl").to_str().unwrap(),
            "--workspace",
            "buyer",
            "--json",
            "--apply",
            input.to_str().unwrap(),
        ])
        .output()
        .unwrap()
}

fn statement(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn policy() -> Policy {
    let unit = Unit::CurrencyMillionths {
        currency: "USD".into(),
    };
    Policy {
        schema: funding::POLICY_SCHEMA.into(),
        version: "operator-fixture-v1".into(),
        unit: unit.clone(),
        conversions: vec![funding::Conversion {
            version: "same-unit-v1".into(),
            source: unit.clone(),
            target: unit,
            numerator: 1,
            denominator: 1,
            source_ref: "synthetic:not-a-payment".into(),
            valid_from: 0,
            valid_until: u64::MAX,
            rounding: Rounding::Exact,
            fee_payer: FeePayer::Operator,
            max_fee_units: 0,
        }],
        purchases: funding::PurchaseTerms {
            required_finality: Finality::Final,
            refunds_allowed: true,
            disputes_allowed: true,
            spent_credit_loss: funding::SpentCreditLoss::Operator,
        },
        promotions: funding::PromotionTerms {
            total_cap: 20,
            grant_cap: 20,
            max_lifetime_seconds: 600,
            max_admissions: 1,
            price_policies: ["trial-use-v1".into()].into(),
            reversible: true,
        },
    }
}

fn reserve(id: &str, policy: &str, amount: u64) -> Operation {
    Operation::Reserve {
        attempt: id.into(),
        request_digest: format!("fixture:{id}"),
        price: Price {
            version: format!("fixture-price:{policy}"),
            currency: "USD".into(),
            model: "fixture".into(),
            capacity: "fixture".into(),
            policy: policy.into(),
            rates: [(
                Resource::InputTokens,
                Rate {
                    millionths: 1,
                    per_units: 1,
                },
            )]
            .into(),
        },
        maximum_usage: [(Resource::InputTokens, amount)].into(),
    }
}

#[test]
fn operator_policy_funding_and_normal_reserve_survive_process_restarts() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path();
    statement(invoke(
        path,
        "create",
        Operation::Create {
            currency: "USD".into(),
            spend_limit: 100,
            topups_allowed: true,
        },
    ));
    statement(invoke(
        path,
        "policy",
        Operation::FundingPolicy { policy: policy() },
    ));
    let pending = statement(invoke(
        path,
        "begin",
        Operation::BeginFunding {
            funding: funding::Funding {
                id: "paid".into(),
                origin: "fixture-processor".into(),
                payment: "fixture-payment".into(),
                policy: policy().version,
                conversion: "same-unit-v1".into(),
                gross_units: 10,
                fee_units: 0,
            },
        },
    ));
    assert_eq!(pending["balance"]["available"], 0);
    assert_eq!(pending["workspace"], "buyer");
    assert!(pending["as_of"].as_u64().is_some());
    assert_eq!(pending["funding"][0]["finality"], "pending");
    let bypass = invoke(
        path,
        "bypass",
        Operation::Credit {
            amount: 50,
            credit_kind: CreditKind::TopUp,
        },
    );
    assert!(!bypass.status.success());
    statement(invoke(
        path,
        "final",
        Operation::FundingFinality {
            funding: "paid".into(),
            finality: Finality::Final,
            evidence: "fixture-verified-final".into(),
        },
    ));
    let expires_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 600;
    statement(invoke(
        path,
        "promotion",
        Operation::Promotion {
            grant: funding::Promotion {
                id: "trial".into(),
                origin: "fixture-campaign".into(),
                policy: policy().version,
                amount: 20,
                expires_at,
            },
        },
    ));
    let denied = invoke(path, "wrong-policy", reserve("wrong", "not-trial", 11));
    assert!(!denied.status.success());
    let held = statement(invoke(path, "held", reserve("one", "trial-use-v1", 15)));
    assert_eq!(
        held["holds"]["one"]["allocations"][0]["kind"],
        "promotional"
    );
    let released = statement(invoke(
        path,
        "release",
        Operation::Release {
            attempt: "one".into(),
        },
    ));
    assert_eq!(released["balance"]["restricted_credit"], 20);
    assert_eq!(released["balance"]["available"], 10);
    assert_eq!(released["grants"][1]["admissions_remaining"], 0);
    assert_eq!(released["wallet_liquidity"], Value::Null);
    assert!(
        released["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["mutation"]["operation"]["kind"] == json!("release"))
    );
    let replay = statement(invoke(path, "held", reserve("one", "trial-use-v1", 15)));
    assert_eq!(replay["balance"], released["balance"]);
    assert!(
        !invoke(path, "trial-again", reserve("two", "trial-use-v1", 11))
            .status
            .success()
    );
}
