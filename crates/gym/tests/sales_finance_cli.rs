use gym::sales_evidence::{Reference, digest};
use gym::sales_finance::{
    CollectionKind, CommercialReceipt, Delivery, Entry, ExpenseClass, Inventory, Manifest, Offer,
    Review, Source, Terms,
};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

fn retain(root: &Path, name: &str, bytes: &[u8]) -> Reference {
    fs::write(root.join(name), bytes).unwrap();
    Reference {
        path: name.into(),
        sha256: digest(bytes),
    }
}

#[test]
fn actual_cli_keeps_agreement_private_and_requires_exact_review_for_export() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::set_permissions(root, fs::Permissions::from_mode(0o700)).unwrap();
    let terms = retain(root, "terms", b"synthetic prospective agreement");
    let payment = retain(
        root,
        "agreement",
        b"synthetic agreement; no payment or work",
    );
    let receipt = CommercialReceipt {
        schema: "openagents.sales.commercial-receipt.v1".into(),
        id: "agreement".into(),
        account: "private-account".into(),
        offer_version: "offer-v1".into(),
        at: 100,
        kind: CollectionKind::Agreement,
        unit: "USD_millionths".into(),
        contractual_charge: 250,
        collected: 0,
        terms_digest: terms.sha256.clone(),
        evidence: payment,
    };
    let receipt = retain(root, "receipt.json", &serde_json::to_vec(&receipt).unwrap());
    let no_cost = retain(
        root,
        "no-cost",
        b"synthetic agreement period has no delivery costs",
    );
    let inventory = retain(
        root,
        "inventory.json",
        &serde_json::to_vec(&Inventory {
            schema: "openagents.gym.sales-finance-inventory.v1".into(),
            entries: vec!["agreement".into()],
            complete: true,
        })
        .unwrap(),
    );
    let manifest = Manifest {
        schema: gym::sales_finance::SCHEMA.into(),
        owner: "private-owner".into(),
        period_start: 90,
        period_end: 110,
        inventory,
        comparisons: BTreeMap::new(),
        offers: vec![Offer {
            id: "private-offer".into(),
            version: "offer-v1".into(),
            account: "private-account".into(),
            cohort: "private-cohort".into(),
            entries: vec![Entry {
                id: "agreement".into(),
                at: 100,
                terms: Terms {
                    version: "terms-v1".into(),
                    evidence: terms,
                    unit: "USD_millionths".into(),
                    contractual_charge: 250,
                    billable_failure: false,
                },
                source: Source::Commercial { receipt },
                delivery: Delivery::Pending,
                delivery_evidence: no_cost.clone(),
                task: None,
                expenses: vec![],
                adjustments: vec![],
                incidents: vec![],
            }],
            no_cost: [
                ExpenseClass::Provider,
                ExpenseClass::Compute,
                ExpenseClass::Payment,
                ExpenseClass::Setup,
                ExpenseClass::Onboarding,
                ExpenseClass::Support,
                ExpenseClass::Repair,
                ExpenseClass::Promotion,
            ]
            .into_iter()
            .map(|c| (c, no_cost.clone()))
            .collect(),
            assumptions: vec![],
        }],
        gaps: vec![],
    };
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_gym"))
            .current_dir(root)
            .env("HOME", root)
            .args(args)
            .output()
            .unwrap()
    };
    let rebuild = [
        "sales-finance",
        "--root",
        ".",
        "--manifest",
        "manifest.json",
        "--output",
        "report.json",
    ];
    let output = run(&rebuild);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert_eq!(
        fs::metadata(root.join("report.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let report = fs::read(root.join("report.json")).unwrap();
    let value: gym::sales_finance::Report = serde_json::from_slice(&report).unwrap();
    assert_eq!(
        value.offers[0].revenue["USD_millionths"].agreed_future_charge,
        250
    );
    assert_eq!(
        value.offers[0].revenue["USD_millionths"].earned_openagents,
        0
    );
    assert!(!run(&rebuild).status.success());
    let mut review = Review {
        schema: "openagents.gym.sales-finance-review.v1".into(),
        report_digest: digest(&report),
        owner: "another-reviewer".into(),
        approved: true,
    };
    let export = [
        "sales-finance",
        "--report",
        "report.json",
        "--review",
        "review.json",
        "--output",
        "aggregate.json",
    ];
    fs::write(
        root.join("review.json"),
        serde_json::to_vec(&review).unwrap(),
    )
    .unwrap();
    assert!(!run(&export).status.success());
    assert!(!root.join("aggregate.json").exists());
    review.owner = manifest.owner;
    fs::write(
        root.join("review.json"),
        serde_json::to_vec(&review).unwrap(),
    )
    .unwrap();
    let output = run(&export);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    let aggregate = fs::read_to_string(root.join("aggregate.json")).unwrap();
    assert!(!aggregate.contains("private-"));
    assert_eq!(
        fs::metadata(root.join("aggregate.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}
