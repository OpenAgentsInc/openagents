use super::funding::{
    self, Conversion, FeePayer, Finality, Funding, Policy, Promotion, Rounding, Unit,
};
use super::*;

fn policy() -> Policy {
    let unit = Unit::CurrencyMillionths {
        currency: "USD".into(),
    };
    Policy {
        schema: funding::POLICY_SCHEMA.into(),
        version: "synthetic-funding-v1".into(),
        unit: unit.clone(),
        conversions: vec![
            Conversion {
                version: "synthetic-usd-v1".into(),
                source: unit.clone(),
                target: unit.clone(),
                numerator: 1,
                denominator: 1,
                source_ref: "fixture:no-real-payment".into(),
                valid_from: 0,
                valid_until: u64::MAX,
                rounding: Rounding::Exact,
                fee_payer: FeePayer::Customer,
                max_fee_units: 5,
            },
            Conversion {
                version: "synthetic-sat-v1".into(),
                source: Unit::Satoshis,
                target: unit,
                numerator: 3,
                denominator: 2,
                source_ref: "fixture:not-a-live-fx-rate".into(),
                valid_from: 0,
                valid_until: 100,
                rounding: Rounding::Down,
                fee_payer: FeePayer::Customer,
                max_fee_units: 5,
            },
        ],
        purchases: funding::PurchaseTerms {
            required_finality: Finality::Final,
            refunds_allowed: true,
            disputes_allowed: true,
            spent_credit_loss: funding::SpentCreditLoss::Operator,
        },
        promotions: funding::PromotionTerms {
            total_cap: 50,
            grant_cap: 30,
            max_lifetime_seconds: 100,
            max_admissions: 1,
            price_policies: ["test-use-v1".into()].into(),
            reversible: true,
        },
    }
}

fn mutation(source: &str, operation: Operation) -> Mutation {
    Mutation {
        workspace: "buyer".into(),
        source: source.into(),
        audit: format!("audit:{source}"),
        operation,
    }
}

fn apply(ledger: &mut Ledger, at: u64, source: &str, operation: Operation) -> Result<bool, String> {
    ledger.apply_at(mutation(source, operation), at)
}

fn account() -> (tempfile::TempDir, Ledger) {
    let root = tempfile::tempdir().unwrap();
    let mut ledger = Ledger::open(&root.path().join("money.jsonl")).unwrap();
    apply(
        &mut ledger,
        10,
        "create",
        Operation::Create {
            currency: "USD".into(),
            spend_limit: 1000,
            topups_allowed: true,
        },
    )
    .unwrap();
    apply(
        &mut ledger,
        10,
        "policy",
        Operation::FundingPolicy { policy: policy() },
    )
    .unwrap();
    (root, ledger)
}

fn funding(id: &str, amount: u64) -> Funding {
    Funding {
        id: id.into(),
        origin: "fixture-provider".into(),
        payment: format!("payment:{id}"),
        policy: policy().version,
        conversion: "synthetic-usd-v1".into(),
        gross_units: amount,
        fee_units: 0,
    }
}

fn fund(ledger: &mut Ledger, id: &str, amount: u64) {
    apply(
        ledger,
        11,
        &format!("begin:{id}"),
        Operation::BeginFunding {
            funding: funding(id, amount),
        },
    )
    .unwrap();
    apply(
        ledger,
        12,
        &format!("final:{id}"),
        Operation::FundingFinality {
            funding: id.into(),
            finality: Finality::Final,
            evidence: format!("verified:{id}"),
        },
    )
    .unwrap();
}

fn checkout_quote(id: &str) -> funding::Quote {
    funding::Quote {
        id: id.into(),
        origin: "fixture-provider".into(),
        policy: policy().version,
        conversion: "synthetic-usd-v1".into(),
        gross_units: 100,
        maximum_fee_units: 5,
        expires_at: 30,
    }
}

fn processor_snapshot() -> funding::Snapshot {
    let mut paid = funding("checkout", 100);
    paid.fee_units = 3;
    funding::Snapshot {
        quote: "checkout".into(),
        funding: paid,
        paid_at: 15,
        finality: Finality::Final,
        evidence: "fixture:verified-native-state-1".into(),
        revision: 1,
        refunded_source_units: 0,
        disputed_source_units: 0,
        reconciliation_pending: false,
    }
}

fn quoted_account() -> (tempfile::TempDir, Ledger) {
    let (root, mut ledger) = account();
    apply(
        &mut ledger,
        11,
        "quote",
        Operation::QuoteFunding {
            quote: checkout_quote("checkout"),
        },
    )
    .unwrap();
    (root, ledger)
}

#[test]
fn processor_confirmation_and_already_refunded_backing_are_one_restart_safe_row() {
    let (root, mut ledger) = quoted_account();
    let mut snapshot = processor_snapshot();
    snapshot.refunded_source_units = 97;
    let operation = Operation::ReconcileQuotedFunding { snapshot };
    let previous_rows = ledger.statement_at("buyer", 20).unwrap().events.len();
    apply(&mut ledger, 20, "processor-state-1", operation.clone()).unwrap();
    let statement = ledger.statement_at("buyer", 20).unwrap();
    assert_eq!(statement.events.len(), previous_rows + 1);
    assert_eq!(statement.balance.credited, 97);
    assert_eq!(statement.balance.reversed_credit, 97);
    assert_eq!(statement.balance.available, 0);
    assert_eq!(statement.funding_snapshots.len(), 1);
    let journal = std::fs::read_to_string(root.path().join("money.jsonl")).unwrap();
    let last: Entry = serde_json::from_str(journal.lines().last().unwrap()).unwrap();
    assert!(matches!(
        last.mutation.operation,
        Operation::ReconcileQuotedFunding { .. }
    ));
    equity(&statement.balance);
    drop(ledger);
    let mut ledger = Ledger::open(&root.path().join("money.jsonl")).unwrap();
    assert!(!apply(&mut ledger, 50, "processor-state-1", operation.clone()).unwrap());
    let head = ledger.head.clone();
    assert!(apply(&mut ledger, 50, "redelivery-new-label", operation).is_err());
    assert_eq!(head, ledger.head);
    assert_eq!(ledger.balance_at("buyer", 50).unwrap().available, 0);
    assert!(apply(&mut ledger, 50, "new-exposure", reserve("blocked", 1)).is_err());
}

#[test]
fn processor_dispute_recovery_preserves_unknown_holds_original_credit_and_refund_floor() {
    let (root, mut ledger) = quoted_account();
    let mut snapshot = processor_snapshot();
    apply(
        &mut ledger,
        20,
        "processor-state-1",
        Operation::ReconcileQuotedFunding {
            snapshot: snapshot.clone(),
        },
    )
    .unwrap();
    apply(&mut ledger, 21, "held", reserve("held", 40)).unwrap();
    apply(&mut ledger, 21, "spent-reservation", reserve("spent", 30)).unwrap();
    apply(&mut ledger, 22, "spent", settle("spent", 30)).unwrap();
    snapshot.revision = 2;
    snapshot.evidence = "fixture:verified-withdrawal".into();
    snapshot.refunded_source_units = 20;
    snapshot.disputed_source_units = 60;
    apply(
        &mut ledger,
        23,
        "processor-state-2",
        Operation::ReconcileQuotedFunding {
            snapshot: snapshot.clone(),
        },
    )
    .unwrap();
    let balance = ledger.balance_at("buyer", 23).unwrap();
    assert_eq!(
        (balance.credited, balance.reserved, balance.settled),
        (97, 40, 30)
    );
    assert_eq!(
        (
            balance.available,
            balance.operator_loss,
            balance.uncovered_holds
        ),
        (0, 13, 40)
    );
    equity(&balance);
    drop(ledger);
    let mut ledger = Ledger::open(&root.path().join("money.jsonl")).unwrap();
    assert_eq!(ledger.holds("buyer")[0].1.phase, Phase::Unknown);
    assert!(apply(&mut ledger, 24, "unapproved-exposure", reserve("extra", 1)).is_err());
    snapshot.revision = 3;
    snapshot.evidence = "fixture:verified-dispute-recovery".into();
    snapshot.disputed_source_units = 0;
    let recovered = Operation::ReconcileQuotedFunding {
        snapshot: snapshot.clone(),
    };
    apply(&mut ledger, 25, "processor-state-3", recovered.clone()).unwrap();
    let balance = ledger.balance_at("buyer", 25).unwrap();
    assert_eq!(
        (balance.credited, balance.reversed_credit, balance.reserved),
        (97, 20, 40)
    );
    assert_eq!(
        (
            balance.available,
            balance.operator_loss,
            balance.uncovered_holds
        ),
        (7, 0, 0)
    );
    assert_eq!(ledger.holds("buyer")[0].1.phase, Phase::Unknown);
    equity(&balance);
    assert!(!apply(&mut ledger, 26, "processor-state-3", recovered).unwrap());
    snapshot.revision = 4;
    snapshot.refunded_source_units = 19;
    let head = ledger.head.clone();
    assert!(
        apply(
            &mut ledger,
            26,
            "erase-permanent-refund",
            Operation::ReconcileQuotedFunding { snapshot },
        )
        .is_err()
    );
    assert_eq!(head, ledger.head);
}

#[test]
fn processor_snapshots_refuse_changed_original_facts_revision_backing_and_legacy_bypass() {
    for failure in [
        "payment", "amount", "origin", "fee", "paid_at", "quote", "revision", "overflow",
    ] {
        let (_root, mut ledger) = quoted_account();
        let mut snapshot = processor_snapshot();
        apply(
            &mut ledger,
            20,
            "original",
            Operation::ReconcileQuotedFunding {
                snapshot: snapshot.clone(),
            },
        )
        .unwrap();
        snapshot.revision = 2;
        match failure {
            "payment" => snapshot.funding.payment = "another-payment".into(),
            "amount" => snapshot.funding.gross_units = 99,
            "origin" => snapshot.funding.origin = "another-origin".into(),
            "fee" => snapshot.funding.fee_units = 2,
            "paid_at" => snapshot.paid_at = 16,
            "quote" => snapshot.quote = "another-quote".into(),
            "revision" => snapshot.revision = 3,
            "overflow" => {
                snapshot.refunded_source_units = 97;
                snapshot.disputed_source_units = 1;
            }
            _ => unreachable!(),
        }
        let head = ledger.head.clone();
        assert!(
            apply(
                &mut ledger,
                21,
                failure,
                Operation::ReconcileQuotedFunding { snapshot }
            )
            .is_err(),
            "{failure}"
        );
        assert_eq!(head, ledger.head);
        assert!(
            apply(
                &mut ledger,
                21,
                "legacy-finality",
                Operation::FundingFinality {
                    funding: "checkout".into(),
                    finality: Finality::Final,
                    evidence: "legacy-source".into(),
                }
            )
            .is_err()
        );
        assert!(
            apply(
                &mut ledger,
                21,
                "legacy-refund",
                Operation::ReverseFunding {
                    funding: "checkout".into(),
                    source_units: 1,
                    reason: funding::Reversal::Refund,
                }
            )
            .is_err()
        );
        assert_eq!(head, ledger.head);
        equity(&ledger.balance_at("buyer", 21).unwrap());
    }
}

#[test]
fn pending_processor_state_removes_backing_before_later_confirmation_credits_anything() {
    let (_root, mut ledger) = quoted_account();
    let mut snapshot = processor_snapshot();
    snapshot.finality = Finality::Pending;
    snapshot.refunded_source_units = 97;
    apply(
        &mut ledger,
        20,
        "pending-refunded",
        Operation::ReconcileQuotedFunding {
            snapshot: snapshot.clone(),
        },
    )
    .unwrap();
    assert_eq!(ledger.balance_at("buyer", 20).unwrap().credited, 0);
    snapshot.revision = 2;
    snapshot.finality = Finality::Final;
    snapshot.evidence = "fixture:late-confirmation".into();
    apply(
        &mut ledger,
        21,
        "late-confirmed",
        Operation::ReconcileQuotedFunding { snapshot },
    )
    .unwrap();
    let balance = ledger.balance_at("buyer", 21).unwrap();
    assert_eq!(
        (balance.credited, balance.reversed_credit, balance.available),
        (97, 97, 0)
    );
    equity(&balance);
}

#[test]
fn uncertain_provider_lookup_restricts_free_credit_without_releasing_unknown_liability() {
    let (root, mut ledger) = quoted_account();
    let mut snapshot = processor_snapshot();
    apply(
        &mut ledger,
        20,
        "processor-state-1",
        Operation::ReconcileQuotedFunding {
            snapshot: snapshot.clone(),
        },
    )
    .unwrap();
    apply(&mut ledger, 21, "held", reserve("held", 40)).unwrap();
    snapshot.revision = 2;
    snapshot.evidence = "fixture:lookup-incomplete".into();
    snapshot.reconciliation_pending = true;
    apply(
        &mut ledger,
        22,
        "native-lookup-pending",
        Operation::ReconcileQuotedFunding {
            snapshot: snapshot.clone(),
        },
    )
    .unwrap();
    let balance = ledger.balance_at("buyer", 22).unwrap();
    assert_eq!(
        (
            balance.credited,
            balance.reserved,
            balance.available,
            balance.restricted_credit
        ),
        (97, 40, 0, 57)
    );
    assert_eq!(
        (
            balance.operator_loss,
            balance.reversed_credit,
            balance.uncovered_holds
        ),
        (0, 0, 0)
    );
    equity(&balance);
    drop(ledger);
    let mut ledger = Ledger::open(&root.path().join("money.jsonl")).unwrap();
    assert_eq!(ledger.hold("buyer", "held").unwrap().phase, Phase::Unknown);
    assert!(apply(&mut ledger, 23, "new-exposure", reserve("new", 1)).is_err());
    snapshot.revision = 3;
    snapshot.evidence = "fixture:verified-native-reconciliation".into();
    snapshot.reconciliation_pending = false;
    apply(
        &mut ledger,
        24,
        "native-lookup-complete",
        Operation::ReconcileQuotedFunding { snapshot },
    )
    .unwrap();
    let balance = ledger.balance_at("buyer", 24).unwrap();
    assert_eq!(
        (
            balance.credited,
            balance.reserved,
            balance.available,
            balance.restricted_credit
        ),
        (97, 40, 57, 0)
    );
    assert_eq!(ledger.hold("buyer", "held").unwrap().phase, Phase::Unknown);
    equity(&balance);
}

#[test]
fn admitted_checkout_survives_policy_rollover_and_delayed_verified_payment_without_repricing() {
    let (root, mut ledger) = account();
    let mut original = policy();
    original.conversions[0].valid_until = 40;
    apply(
        &mut ledger,
        10,
        "bounded-policy",
        Operation::FundingPolicy {
            policy: {
                original.version = "bounded-v1".into();
                original.clone()
            },
        },
    )
    .unwrap();
    let mut quote = checkout_quote("checkout");
    quote.policy = original.version.clone();
    let admitted = Operation::QuoteFunding {
        quote: quote.clone(),
    };
    apply(&mut ledger, 11, "admit", admitted.clone()).unwrap();
    assert_eq!(ledger.balance_at("buyer", 11).unwrap().credited, 0);
    let mut next = policy();
    next.version = "new-v2".into();
    next.conversions[0].max_fee_units = 1;
    apply(
        &mut ledger,
        12,
        "rollover",
        Operation::FundingPolicy { policy: next },
    )
    .unwrap();
    drop(ledger);
    let mut ledger = Ledger::open(&root.path().join("money.jsonl")).unwrap();
    assert!(!apply(&mut ledger, 100, "admit", admitted).unwrap());
    let mut paid = funding("checkout", 100);
    paid.policy = original.version;
    paid.fee_units = 3;
    assert!(
        apply(
            &mut ledger,
            100,
            "unquoted",
            Operation::BeginFunding {
                funding: paid.clone()
            }
        )
        .is_err()
    );
    let begin = Operation::BeginQuotedFunding {
        quote: quote.id,
        funding: paid,
        paid_at: 20,
    };
    apply(&mut ledger, 100, "verified", begin.clone()).unwrap();
    assert_eq!(ledger.balance_at("buyer", 100).unwrap().credited, 0);
    let confirmed = Operation::FundingFinality {
        funding: "checkout".into(),
        finality: Finality::Final,
        evidence: "fixture:verified-native-payment".into(),
    };
    apply(&mut ledger, 101, "confirm", confirmed.clone()).unwrap();
    drop(ledger);
    let mut ledger = Ledger::open(&root.path().join("money.jsonl")).unwrap();
    assert!(!apply(&mut ledger, 102, "verified", begin).unwrap());
    assert!(!apply(&mut ledger, 102, "confirm", confirmed).unwrap());
    let statement = ledger.statement_at("buyer", 102).unwrap();
    assert_eq!(statement.balance.credited, 97);
    assert_eq!(statement.funding[0].quoted_at, 11);
    assert_eq!(statement.funding[0].conversion.max_fee_units, 5);
    assert_eq!(
        statement.funding_quotes[0].funding.as_deref(),
        Some("checkout")
    );
    equity(&statement.balance);
}

#[test]
fn quoted_payment_requires_exact_origin_amount_policy_fee_and_native_payment_window() {
    for failure in [
        "origin",
        "amount",
        "policy",
        "conversion",
        "fee",
        "before",
        "expired",
        "future",
        "id",
    ] {
        let (_root, mut ledger) = account();
        apply(
            &mut ledger,
            11,
            "quote",
            Operation::QuoteFunding {
                quote: checkout_quote("checkout"),
            },
        )
        .unwrap();
        let mut paid = funding("checkout", 100);
        let paid_at = match failure {
            "before" => 10,
            "expired" => 30,
            "future" => 25,
            _ => 15,
        };
        match failure {
            "origin" => paid.origin = "another-customer".into(),
            "amount" => paid.gross_units = 101,
            "policy" => paid.policy = "other-v1".into(),
            "conversion" => paid.conversion = "synthetic-sat-v1".into(),
            "fee" => paid.fee_units = 6,
            "id" => paid.id = "another-checkout".into(),
            _ => {}
        }
        let head = ledger.head.clone();
        assert!(
            apply(
                &mut ledger,
                20,
                "bad",
                Operation::BeginQuotedFunding {
                    quote: "checkout".into(),
                    funding: paid,
                    paid_at,
                }
            )
            .is_err(),
            "{failure}"
        );
        assert_eq!(head, ledger.head);
        let statement = ledger.statement_at("buyer", 20).unwrap();
        assert_eq!(statement.balance.credited, 0);
        assert!(statement.funding.is_empty());
        assert!(statement.funding_quotes[0].funding.is_none());
    }
}

#[test]
fn accepted_quote_identity_cannot_be_reused_for_promotion_or_a_second_workspace_payment() {
    let (_root, mut ledger) = account();
    apply(
        &mut ledger,
        11,
        "quote",
        Operation::QuoteFunding {
            quote: checkout_quote("checkout"),
        },
    )
    .unwrap();
    assert!(promote(&mut ledger, "checkout", 10, 40).is_err());
    let mut bad = checkout_quote("checkout");
    bad.gross_units = 90;
    assert!(
        apply(
            &mut ledger,
            20,
            "rewrite",
            Operation::QuoteFunding { quote: bad }
        )
        .is_err()
    );
    apply(
        &mut ledger,
        21,
        "paid",
        Operation::BeginQuotedFunding {
            quote: "checkout".into(),
            funding: funding("checkout", 100),
            paid_at: 20,
        },
    )
    .unwrap();
    let mut other = mutation(
        "other-account",
        Operation::Create {
            currency: "USD".into(),
            spend_limit: 1000,
            topups_allowed: true,
        },
    );
    other.workspace = "other".into();
    ledger.apply_at(other, 22).unwrap();
    for (source, operation) in [
        (
            "other-policy",
            Operation::FundingPolicy { policy: policy() },
        ),
        (
            "other-quote",
            Operation::QuoteFunding {
                quote: checkout_quote("other-checkout"),
            },
        ),
    ] {
        let mut other = mutation(source, operation);
        other.workspace = "other".into();
        ledger.apply_at(other, 22).unwrap();
    }
    let mut recycled = funding("other-checkout", 100);
    recycled.payment = "payment:checkout".into();
    let mut attempt = mutation(
        "recycled",
        Operation::BeginQuotedFunding {
            quote: "other-checkout".into(),
            funding: recycled,
            paid_at: 23,
        },
    );
    attempt.workspace = "other".into();
    assert!(ledger.apply_at(attempt, 24).is_err());
    assert_eq!(ledger.balance_at("other", 24).unwrap().credited, 0);
}

fn promote(ledger: &mut Ledger, id: &str, amount: u64, expires_at: u64) -> Result<bool, String> {
    apply(
        ledger,
        20,
        &format!("promo:{id}"),
        Operation::Promotion {
            grant: Promotion {
                id: id.into(),
                origin: "fixture-campaign".into(),
                policy: policy().version,
                amount,
                expires_at,
            },
        },
    )
}

fn price() -> Price {
    Price {
        version: "synthetic-price-v1".into(),
        currency: "USD".into(),
        model: "fixture".into(),
        capacity: "fixture".into(),
        policy: "test-use-v1".into(),
        rates: [(
            Resource::InputTokens,
            Rate {
                millionths: 1,
                per_units: 1,
            },
        )]
        .into(),
    }
}

fn reserve(id: &str, amount: u64) -> Operation {
    Operation::Reserve {
        attempt: id.into(),
        request_digest: format!("request:{id}"),
        price: price(),
        maximum_usage: [(Resource::InputTokens, amount)].into(),
    }
}

fn settle(id: &str, amount: u64) -> Operation {
    Operation::Settle {
        attempt: id.into(),
        usage: [(Resource::InputTokens, amount)].into(),
        receipt: format!("receipt:{id}"),
        provider_cost: None,
        hosting_cost: None,
    }
}

fn equity(balance: &Balance) {
    assert_eq!(
        balance.credited + balance.operator_loss + balance.uncovered_holds,
        balance.reserved + balance.settled - balance.refunded
            + balance.available
            + balance.reversed_credit
            + balance.expired_credit
            + balance.restricted_credit
    );
}

fn fractional_account(source: u64, rounding: Rounding) -> (tempfile::TempDir, Ledger) {
    let (root, mut ledger) = account();
    let mut terms = policy();
    terms.version = "fractional-boundary-v1".into();
    terms.conversions[1].numerator = 1;
    terms.conversions[1].denominator = 2;
    terms.conversions[1].rounding = rounding;
    apply(
        &mut ledger,
        10,
        "fractional-policy",
        Operation::FundingPolicy {
            policy: terms.clone(),
        },
    )
    .unwrap();
    let mut purchase = funding("fractional", source);
    purchase.policy = terms.version;
    purchase.conversion = terms.conversions[1].version.clone();
    apply(
        &mut ledger,
        11,
        "begin",
        Operation::BeginFunding { funding: purchase },
    )
    .unwrap();
    apply(
        &mut ledger,
        12,
        "final",
        Operation::FundingFinality {
            funding: "fractional".into(),
            finality: Finality::Final,
            evidence: "fixture:verified-final".into(),
        },
    )
    .unwrap();
    (root, ledger)
}

fn fractional_reversal(units: u64) -> Operation {
    Operation::ReverseFunding {
        funding: "fractional".into(),
        source_units: units,
        reason: funding::Reversal::Refund,
    }
}

#[test]
fn partial_reversal_never_leaves_credit_above_remaining_fractional_backing() {
    for disposition in ["released", "spent", "held"] {
        let (root, mut ledger) = fractional_account(2, Rounding::Down);
        apply(&mut ledger, 20, "reserve", reserve("one", 1)).unwrap();
        match disposition {
            "released" => {
                apply(
                    &mut ledger,
                    21,
                    "release",
                    Operation::Release {
                        attempt: "one".into(),
                    },
                )
                .unwrap();
            }
            "spent" => {
                apply(&mut ledger, 21, "settle", settle("one", 1)).unwrap();
            }
            "held" => {}
            _ => unreachable!(),
        }
        let reverse = mutation("partial", fractional_reversal(1));
        ledger.apply_at(reverse.clone(), 22).unwrap();
        assert!(!ledger.apply_at(reverse, 23).unwrap());
        let balance = ledger.balance_at("buyer", 23).unwrap();
        assert_eq!(
            (balance.credited, balance.reversed_credit, balance.available),
            (1, 1, 0)
        );
        assert_eq!(balance.operator_loss, u64::from(disposition == "spent"));
        assert_eq!(balance.uncovered_holds, u64::from(disposition == "held"));
        equity(&balance);
        assert!(apply(&mut ledger, 23, "unsupported-credit", reserve("two", 1)).is_err());
        let record = &ledger.statement_at("buyer", 23).unwrap().funding[0];
        assert_eq!(
            (
                record.quote.remainder,
                record.reversed_source_units,
                record.reversal_remainder,
                record.remaining_remainder
            ),
            (0, 1, 1, 1)
        );
        drop(ledger);
        let mut ledger = Ledger::open(&root.path().join("money.jsonl")).unwrap();
        let replayed = ledger.balance_at("buyer", 23).unwrap();
        assert_eq!(
            (
                replayed.reversed_credit,
                replayed.available,
                replayed.operator_loss,
                replayed.uncovered_holds
            ),
            (1, 0, balance.operator_loss, balance.uncovered_holds)
        );
        equity(&replayed);
        if disposition == "held" {
            assert_eq!(ledger.hold("buyer", "one").unwrap().phase, Phase::Unknown);
            apply(&mut ledger, 24, "late-settle", settle("one", 1)).unwrap();
        }
        if disposition != "released" {
            apply(
                &mut ledger,
                25,
                "usage-refund",
                Operation::Refund {
                    attempt: "one".into(),
                    amount: 1,
                },
            )
            .unwrap();
            let refunded = ledger.balance_at("buyer", 25).unwrap();
            assert_eq!((refunded.operator_loss, refunded.available), (0, 0));
            equity(&refunded);
        }
    }
}

#[test]
fn original_dust_and_remaining_dust_stay_distinct_through_split_reversals() {
    let (_root, mut ledger) = fractional_account(3, Rounding::Down);
    apply(&mut ledger, 20, "first", fractional_reversal(1)).unwrap();
    let balance = ledger.balance_at("buyer", 20).unwrap();
    assert_eq!(
        (balance.credited, balance.reversed_credit, balance.available),
        (1, 0, 1)
    );
    equity(&balance);
    let first = &ledger.statement_at("buyer", 20).unwrap().funding[0];
    assert_eq!(
        (
            first.quote.remainder,
            first.reversal_remainder,
            first.remaining_remainder
        ),
        (1, 1, 0)
    );
    apply(&mut ledger, 21, "second", fractional_reversal(1)).unwrap();
    let second = &ledger.statement_at("buyer", 21).unwrap().funding[0];
    assert_eq!(
        (
            second.reversed_credit,
            second.reversal_remainder,
            second.remaining_remainder
        ),
        (1, 0, 1)
    );
    assert_eq!(ledger.balance_at("buyer", 21).unwrap().available, 0);
    apply(&mut ledger, 22, "last", fractional_reversal(1)).unwrap();
    let last = &ledger.statement_at("buyer", 22).unwrap().funding[0];
    assert_eq!(
        (
            last.reversed_source_units,
            last.reversed_credit,
            last.quote.remainder,
            last.reversal_remainder,
            last.remaining_remainder
        ),
        (3, 1, 1, 1, 0)
    );
    equity(&ledger.balance_at("buyer", 22).unwrap());
}

#[test]
fn exact_precision_refuses_partial_dust_without_changing_funding() {
    let (_root, mut ledger) = fractional_account(4, Rounding::Exact);
    assert!(apply(&mut ledger, 20, "first-dust", fractional_reversal(1)).is_err());
    let before = &ledger.statement_at("buyer", 20).unwrap().funding[0];
    assert_eq!(
        (
            before.reversed_source_units,
            before.reversed_credit,
            before.reversal_remainder,
            before.remaining_remainder
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(ledger.balance_at("buyer", 20).unwrap().available, 2);
    apply(&mut ledger, 20, "first-exact", fractional_reversal(2)).unwrap();
    assert!(apply(&mut ledger, 21, "remaining-dust", fractional_reversal(1)).is_err());
    let partial = &ledger.statement_at("buyer", 21).unwrap().funding[0];
    assert_eq!(
        (
            partial.reversed_source_units,
            partial.reversed_credit,
            partial.remaining_remainder
        ),
        (2, 1, 0)
    );
    assert_eq!(ledger.balance_at("buyer", 21).unwrap().available, 1);
    apply(&mut ledger, 22, "last-exact", fractional_reversal(2)).unwrap();
    assert_eq!(ledger.balance_at("buyer", 22).unwrap().available, 0);
    equity(&ledger.balance_at("buyer", 22).unwrap());
}

#[test]
fn monetary_scales_rounding_fees_dust_and_overflow_are_exact() {
    let mut rate = policy().conversions.remove(1);
    let quote = rate.quote(4, 1, 50).unwrap();
    assert_eq!(
        (
            quote.convertible_units,
            quote.credited_units,
            quote.remainder,
            quote.denominator
        ),
        (3, 4, 1, 2)
    );
    assert_eq!(
        u128::from(quote.credited_units) * 2 + u128::from(quote.remainder),
        u128::from(quote.convertible_units) * 3
    );
    rate.rounding = Rounding::Exact;
    assert!(rate.quote(4, 1, 50).is_err());
    assert_eq!(rate.quote(5, 1, 50).unwrap().credited_units, 6);
    rate.fee_payer = FeePayer::Operator;
    assert_eq!(rate.quote(4, 1, 50).unwrap().credited_units, 6);
    assert!(rate.quote(4, 6, 50).is_err());
    assert!(rate.quote(4, 0, 100).is_err());
    rate.numerator = u64::MAX;
    rate.denominator = 1;
    assert!(rate.quote(2, 0, 50).is_err());
    rate.numerator = 1;
    rate.denominator = u64::MAX;
    rate.rounding = Rounding::Down;
    assert!(rate.quote(1, 0, 50).is_err());

    rate.source = Unit::Millisatoshis;
    rate.target = Unit::Satoshis;
    rate.numerator = 1;
    rate.denominator = 1000;
    rate.rounding = Rounding::Down;
    let tiny = rate.quote(1001, 0, 50).unwrap();
    assert_eq!((tiny.credited_units, tiny.remainder), (1, 1));
    rate.numerator = 2;
    assert!(rate.validate().is_err());
    assert!(serde_json::from_str::<Unit>(r#"{"kind":"xp"}"#).is_err());
}

#[test]
fn pending_or_insufficient_finality_never_creates_spendable_credit() {
    let (_root, mut ledger) = account();
    let begin = mutation(
        "begin",
        Operation::BeginFunding {
            funding: funding("p", 100),
        },
    );
    assert!(ledger.apply_at(begin.clone(), 11).unwrap());
    assert!(!ledger.apply_at(begin, 99).unwrap());
    assert_eq!(ledger.balance_at("buyer", 11).unwrap().available, 0);
    assert!(apply(&mut ledger, 11, "dispatch", reserve("one", 1)).is_err());
    apply(
        &mut ledger,
        12,
        "unknown",
        Operation::FundingFinality {
            funding: "p".into(),
            finality: Finality::Pending,
            evidence: "provider:unknown".into(),
        },
    )
    .unwrap();
    apply(
        &mut ledger,
        13,
        "confirmed",
        Operation::FundingFinality {
            funding: "p".into(),
            finality: Finality::Confirmed,
            evidence: "provider:confirmed".into(),
        },
    )
    .unwrap();
    assert_eq!(ledger.balance_at("buyer", 13).unwrap().credited, 0);
    let finality = mutation(
        "final",
        Operation::FundingFinality {
            funding: "p".into(),
            finality: Finality::Final,
            evidence: "provider:final".into(),
        },
    );
    assert!(ledger.apply_at(finality.clone(), 14).unwrap());
    assert!(!ledger.apply_at(finality, 15).unwrap());
    apply(
        &mut ledger,
        15,
        "another-final-event",
        Operation::FundingFinality {
            funding: "p".into(),
            finality: Finality::Final,
            evidence: "provider:final-recheck".into(),
        },
    )
    .unwrap();
    assert_eq!(ledger.balance_at("buyer", 15).unwrap().credited, 100);
    assert!(
        apply(
            &mut ledger,
            16,
            "backwards-finality",
            Operation::FundingFinality {
                funding: "p".into(),
                finality: Finality::Pending,
                evidence: "unknown".into()
            }
        )
        .is_err()
    );
    let statement = ledger.statement_at("buyer", 16).unwrap();
    assert!(statement.funding[0].credited);
    assert!(statement.wallet_liquidity.is_none());
}

#[test]
fn unknown_conversion_duplicate_payment_and_unscoped_credit_cannot_bypass_policy() {
    let (_root, mut ledger) = account();
    let mut unsupported = funding("unknown", 100);
    unsupported.conversion = "no-quote".into();
    assert!(
        apply(
            &mut ledger,
            11,
            "unsupported",
            Operation::BeginFunding {
                funding: unsupported
            }
        )
        .is_err()
    );
    let mut expired = funding("expired", 100);
    expired.conversion = "synthetic-sat-v1".into();
    assert!(
        apply(
            &mut ledger,
            100,
            "expired-rate",
            Operation::BeginFunding { funding: expired }
        )
        .is_err()
    );
    fund(&mut ledger, "first", 100);
    let mut alias = funding("alias", 100);
    alias.payment = "payment:first".into();
    assert!(
        apply(
            &mut ledger,
            12,
            "alias",
            Operation::BeginFunding {
                funding: alias.clone()
            }
        )
        .is_err()
    );
    let mut create = mutation(
        "create",
        Operation::Create {
            currency: "USD".into(),
            spend_limit: 1000,
            topups_allowed: true,
        },
    );
    create.workspace = "other".into();
    ledger.apply_at(create, 12).unwrap();
    let mut install = mutation("policy", Operation::FundingPolicy { policy: policy() });
    install.workspace = "other".into();
    ledger.apply_at(install, 12).unwrap();
    let mut replay = mutation("other-funding", Operation::BeginFunding { funding: alias });
    replay.workspace = "other".into();
    assert!(ledger.apply_at(replay, 12).is_err());
    for kind in [CreditKind::Grant, CreditKind::TopUp, CreditKind::Adjustment] {
        assert!(
            apply(
                &mut ledger,
                12,
                "unscoped",
                Operation::Credit {
                    amount: 1,
                    credit_kind: kind
                }
            )
            .is_err()
        );
    }
    assert!(apply(&mut ledger, 12, "debit", Operation::Debit { amount: 1 }).is_err());
    assert_eq!(ledger.balance_at("buyer", 12).unwrap().credited, 100);
}

#[test]
fn normal_reserve_enforces_promotion_scope_expiry_and_exhaustion() {
    let (_root, mut ledger) = account();
    promote(&mut ledger, "trial", 30, 50).unwrap();
    let mut wrong = reserve("wrong", 1);
    if let Operation::Reserve { price, .. } = &mut wrong {
        price.policy = "not-permitted".into();
    }
    assert!(apply(&mut ledger, 21, "wrong", wrong).is_err());
    let account = &ledger.state.accounts["buyer"];
    let mut denied = price();
    denied.policy = "not-permitted".into();
    let scoped = account.balance_for_price(21, Some(&denied)).unwrap();
    assert_eq!((scoped.available, scoped.restricted_credit), (0, 30));
    assert!(apply(&mut ledger, 21, "too-much", reserve("too-much", 31)).is_err());
    apply(&mut ledger, 22, "reserve", reserve("one", 10)).unwrap();
    apply(&mut ledger, 23, "settle", settle("one", 5)).unwrap();
    let hold = ledger.hold("buyer", "one").unwrap();
    assert_eq!(hold.commissionable_charge, Some(0));
    let b = ledger.balance_at("buyer", 23).unwrap();
    assert_eq!((b.available, b.restricted_credit, b.settled), (0, 25, 5));
    equity(&b);
    assert!(apply(&mut ledger, 24, "exhausted", reserve("two", 1)).is_err());
    apply(
        &mut ledger,
        24,
        "refund",
        Operation::Refund {
            attempt: "one".into(),
            amount: 5,
        },
    )
    .unwrap();
    assert!(apply(&mut ledger, 25, "refund-not-new-trial", reserve("three", 1)).is_err());
    let b = ledger.balance_at("buyer", 50).unwrap();
    assert_eq!((b.available, b.expired_credit, b.refunded), (0, 30, 5));
    equity(&b);
    let statement = ledger.statement_at("buyer", 50).unwrap();
    assert!(statement.grants[0].expired);
    assert_eq!(statement.grants[0].admissions_remaining, Some(0));
}

#[test]
fn promotion_issuance_cap_includes_expired_reversed_and_released_grants() {
    let (_root, mut ledger) = account();
    promote(&mut ledger, "trial", 30, 40).unwrap();
    assert!(promote(&mut ledger, "oversize", 31, 40).is_err());
    assert!(promote(&mut ledger, "past", 1, 20).is_err());
    assert!(promote(&mut ledger, "late", 1, 121).is_err());
    apply(&mut ledger, 21, "held", reserve("one", 1)).unwrap();
    apply(
        &mut ledger,
        22,
        "release",
        Operation::Release {
            attempt: "one".into(),
        },
    )
    .unwrap();
    assert!(apply(&mut ledger, 23, "reuse", reserve("two", 1)).is_err());
    apply(
        &mut ledger,
        24,
        "revoke",
        Operation::ReversePromotion {
            grant: "trial".into(),
            amount: 30,
        },
    )
    .unwrap();
    // The second grant is a new source, but cannot reset lifetime issuance.
    assert!(
        apply(
            &mut ledger,
            40,
            "next",
            Operation::Promotion {
                grant: Promotion {
                    id: "next".into(),
                    origin: "another-campaign".into(),
                    policy: policy().version,
                    amount: 21,
                    expires_at: 100,
                }
            }
        )
        .is_err()
    );
    let b = ledger.balance_at("buyer", 40).unwrap();
    assert_eq!((b.credited, b.reversed_credit, b.available), (30, 30, 0));
    equity(&b);
}

#[test]
fn expiring_trial_keeps_its_unknown_hold_and_refund_restores_no_spendable_credit() {
    let (root, mut ledger) = account();
    promote(&mut ledger, "trial", 30, 40).unwrap();
    apply(&mut ledger, 21, "hold", reserve("one", 20)).unwrap();
    let expired = ledger.balance_at("buyer", 40).unwrap();
    assert_eq!(
        (expired.reserved, expired.expired_credit, expired.available),
        (20, 10, 0)
    );
    equity(&expired);
    drop(ledger);
    let mut ledger = Ledger::open(&root.path().join("money.jsonl")).unwrap();
    assert_eq!(ledger.hold("buyer", "one").unwrap().phase, Phase::Unknown);
    assert!(apply(&mut ledger, 40, "expired-reserve", reserve("two", 1)).is_err());
    apply(&mut ledger, 41, "settle", settle("one", 10)).unwrap();
    let settled = ledger.balance_at("buyer", 41).unwrap();
    assert_eq!(
        (settled.settled, settled.expired_credit, settled.available),
        (10, 20, 0)
    );
    equity(&settled);
    apply(
        &mut ledger,
        42,
        "refund",
        Operation::Refund {
            attempt: "one".into(),
            amount: 10,
        },
    )
    .unwrap();
    let refunded = ledger.balance_at("buyer", 42).unwrap();
    assert_eq!(
        (
            refunded.refunded,
            refunded.expired_credit,
            refunded.available
        ),
        (10, 30, 0)
    );
    equity(&refunded);
    assert_eq!(
        ledger.hold("buyer", "one").unwrap().commissionable_charge,
        Some(0)
    );
}

#[test]
fn partial_refunds_preserve_purchased_and_promotional_origins() {
    let (_root, mut ledger) = account();
    fund(&mut ledger, "paid", 100);
    promote(&mut ledger, "trial", 30, 80).unwrap();
    apply(&mut ledger, 21, "reserve", reserve("one", 60)).unwrap();
    apply(&mut ledger, 22, "settle", settle("one", 45)).unwrap();
    let h = ledger.hold("buyer", "one").unwrap();
    assert_eq!(
        h.allocations.iter().map(|a| a.charged).collect::<Vec<_>>(),
        vec![30, 15]
    );
    assert_eq!(h.commissionable_charge, Some(15));
    apply(
        &mut ledger,
        23,
        "refund",
        Operation::Refund {
            attempt: "one".into(),
            amount: 20,
        },
    )
    .unwrap();
    let h = ledger.hold("buyer", "one").unwrap();
    assert_eq!(
        h.allocations.iter().map(|a| a.refunded).collect::<Vec<_>>(),
        vec![5, 15]
    );
    assert_eq!(h.commissionable_charge, Some(0));
    apply(
        &mut ledger,
        24,
        "reverse",
        Operation::ReverseRefund {
            attempt: "one".into(),
            amount: 7,
        },
    )
    .unwrap();
    let h = ledger.hold("buyer", "one").unwrap();
    assert_eq!(h.commissionable_charge, Some(7));
    let b = ledger.balance_at("buyer", 24).unwrap();
    equity(&b);
    assert_eq!((b.purchased_funding, b.promotional_credit), (100, 30));
    let s = ledger.statement_at("buyer", 24).unwrap();
    assert!(
        s.events
            .iter()
            .any(|e| matches!(e.mutation.operation, Operation::Refund { .. }))
    );
    assert!(
        s.events
            .iter()
            .any(|e| matches!(e.mutation.operation, Operation::Settle { .. }))
    );
    assert!(
        s.events
            .iter()
            .any(|e| matches!(e.mutation.operation, Operation::Promotion { .. }))
    );
}

#[test]
fn funding_reversals_conserve_spent_and_unknown_held_liabilities() {
    let (root, mut ledger) = account();
    fund(&mut ledger, "paid", 100);
    apply(&mut ledger, 20, "spent-hold", reserve("spent", 30)).unwrap();
    apply(&mut ledger, 21, "spent", settle("spent", 30)).unwrap();
    apply(&mut ledger, 22, "unknown-hold", reserve("unknown", 40)).unwrap();
    apply(
        &mut ledger,
        23,
        "dispute",
        Operation::ReverseFunding {
            funding: "paid".into(),
            source_units: 80,
            reason: funding::Reversal::Dispute,
        },
    )
    .unwrap();
    let b = ledger.balance_at("buyer", 23).unwrap();
    assert_eq!(
        (
            b.available,
            b.reversed_credit,
            b.operator_loss,
            b.uncovered_holds
        ),
        (0, 80, 10, 40)
    );
    equity(&b);
    assert!(
        apply(
            &mut ledger,
            24,
            "over-refund",
            Operation::ReverseFunding {
                funding: "paid".into(),
                source_units: 21,
                reason: funding::Reversal::Refund
            }
        )
        .is_err()
    );
    drop(ledger);
    let mut ledger = Ledger::open(&root.path().join("money.jsonl")).unwrap();
    assert_eq!(
        ledger.hold("buyer", "unknown").unwrap().phase,
        Phase::Unknown
    );
    apply(&mut ledger, 25, "settle-unknown", settle("unknown", 5)).unwrap();
    let b = ledger.balance_at("buyer", 25).unwrap();
    assert_eq!((b.operator_loss, b.uncovered_holds, b.settled), (15, 0, 35));
    equity(&b);
    apply(
        &mut ledger,
        26,
        "usage-refund",
        Operation::Refund {
            attempt: "spent".into(),
            amount: 30,
        },
    )
    .unwrap();
    let b = ledger.balance_at("buyer", 26).unwrap();
    assert_eq!((b.available, b.operator_loss), (15, 0));
    equity(&b);
}

#[test]
fn conversion_and_purchase_terms_survive_policy_changes_and_split_reversals() {
    let (root, mut ledger) = account();
    let mut purchase = funding("converted", 5);
    purchase.conversion = "synthetic-sat-v1".into();
    purchase.fee_units = 1;
    let begin = mutation("begin", Operation::BeginFunding { funding: purchase });
    ledger.apply_at(begin.clone(), 11).unwrap();
    let original = ledger.state.accounts["buyer"]
        .funding
        .as_ref()
        .unwrap()
        .funding["converted"]
        .clone();
    let mut changed = policy();
    changed.conversions[1].numerator = 9;
    assert!(
        apply(
            &mut ledger,
            12,
            "reuse-version",
            Operation::FundingPolicy {
                policy: changed.clone()
            }
        )
        .is_err()
    );
    changed.version = "synthetic-funding-v2".into();
    changed.conversions[1].version = "synthetic-sat-v2".into();
    changed.purchases.refunds_allowed = false;
    apply(
        &mut ledger,
        12,
        "new-policy",
        Operation::FundingPolicy { policy: changed },
    )
    .unwrap();
    apply(
        &mut ledger,
        101,
        "final-after-rate-expiry",
        Operation::FundingFinality {
            funding: "converted".into(),
            finality: Finality::Final,
            evidence: "late-final".into(),
        },
    )
    .unwrap();
    assert_eq!(ledger.balance_at("buyer", 101).unwrap().credited, 6);
    let first = mutation(
        "refund1",
        Operation::ReverseFunding {
            funding: "converted".into(),
            source_units: 1,
            reason: funding::Reversal::Refund,
        },
    );
    ledger.apply_at(first.clone(), 102).unwrap();
    assert!(!ledger.apply_at(first, 103).unwrap());
    apply(
        &mut ledger,
        103,
        "refund2",
        Operation::ReverseFunding {
            funding: "converted".into(),
            source_units: 1,
            reason: funding::Reversal::Refund,
        },
    )
    .unwrap();
    let record = &ledger.state.accounts["buyer"]
        .funding
        .as_ref()
        .unwrap()
        .funding["converted"];
    assert_eq!(
        (
            record.reversed_source_units,
            record.reversed_credit,
            record.reversal_remainder
        ),
        (2, 3, 0)
    );
    assert_eq!(record.policy_digest, original.policy_digest);
    assert_eq!(record.conversion, original.conversion);
    apply(
        &mut ledger,
        104,
        "refund-rest",
        Operation::ReverseFunding {
            funding: "converted".into(),
            source_units: 2,
            reason: funding::Reversal::Refund,
        },
    )
    .unwrap();
    assert_eq!(ledger.balance_at("buyer", 104).unwrap().available, 0);
    drop(ledger);
    let mut ledger = Ledger::open(&root.path().join("money.jsonl")).unwrap();
    assert!(!ledger.apply_at(begin, 105).unwrap());
    let record = &ledger.state.accounts["buyer"]
        .funding
        .as_ref()
        .unwrap()
        .funding["converted"];
    assert_eq!(
        (record.quote.credited_units, record.reversed_credit),
        (6, 6)
    );
}

#[test]
fn fractional_conversion_partitioning_never_creates_or_loses_integer_credit() {
    for numerator in 1..=9_u64 {
        for denominator in 1..=11_u64 {
            for source in 1..=25_u64 {
                let mut terms = policy();
                terms.conversions[1].numerator = numerator;
                terms.conversions[1].denominator = denominator;
                let Ok(quote) = terms.conversions[1].quote(source, 0, 10) else {
                    // A quote smaller than one integer credit is refused.
                    continue;
                };
                assert_eq!(
                    u128::from(quote.credited_units) * u128::from(denominator)
                        + u128::from(quote.remainder),
                    u128::from(source) * u128::from(numerator)
                );
                let mut original = funding::Book::default();
                original.install(&terms, "USD").unwrap();
                let mut purchase = funding("property", source);
                purchase.conversion = terms.conversions[1].version.clone();
                original.begin(&purchase, 10).unwrap();
                original
                    .confirm("property", Finality::Final, "fixture:final")
                    .unwrap();
                let mut split = original.clone();
                for cumulative in 1..=source {
                    split
                        .reverse_funding("property", 1, funding::Reversal::Refund)
                        .unwrap();
                    let mut batched = original.clone();
                    batched
                        .reverse_funding("property", cumulative, funding::Reversal::Refund)
                        .unwrap();
                    let remaining_value = u128::from(source - cumulative) * u128::from(numerator);
                    let expected_credit =
                        u64::try_from(remaining_value / u128::from(denominator)).unwrap();
                    let expected_dust =
                        u64::try_from(remaining_value % u128::from(denominator)).unwrap();
                    let record = &split.funding["property"];
                    let batch = &batched.funding["property"];
                    assert_eq!(
                        record.reversed_credit,
                        quote.credited_units - expected_credit
                    );
                    assert_eq!(record.remaining_remainder, expected_dust);
                    assert_eq!(
                        (
                            record.reversed_credit,
                            record.reversal_remainder,
                            record.remaining_remainder
                        ),
                        (
                            batch.reversed_credit,
                            batch.reversal_remainder,
                            batch.remaining_remainder
                        )
                    );
                    assert_eq!(
                        split.summary(&BTreeMap::new(), 10, None).unwrap().available,
                        expected_credit
                    );
                    // Original credit and dust reconcile with refunded source
                    // and the uncredited dust that still backs the remainder.
                    assert_eq!(
                        u128::from(record.reversed_credit) * u128::from(denominator)
                            + u128::from(quote.remainder),
                        u128::from(cumulative) * u128::from(numerator)
                            + u128::from(record.remaining_remainder)
                    );
                    assert_eq!(
                        u128::from(expected_credit) * u128::from(denominator)
                            + u128::from(record.remaining_remainder),
                        remaining_value
                    );
                }
            }
        }
    }
}

#[test]
fn legacy_chain_reads_without_revaluation_and_v2_clock_cannot_roll_back() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("money.jsonl");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .unwrap();
    let mut head = String::new();
    for op in [
        Operation::Create {
            currency: "USD".into(),
            spend_limit: 100,
            topups_allowed: true,
        },
        Operation::Credit {
            amount: 100,
            credit_kind: CreditKind::TopUp,
        },
    ]
    .into_iter()
    .enumerate()
    {
        let mut entry = Entry {
            schema: LEGACY_SCHEMA.into(),
            previous: head,
            mutation: mutation(&format!("legacy:{}", op.0), op.1),
            digest: String::new(),
            recorded_at: None,
        };
        entry.digest = entry.computed().unwrap();
        head = entry.digest.clone();
        serde_json::to_writer(&mut file, &entry).unwrap();
        file.write_all(b"\n").unwrap();
    }
    drop(file);
    let mut ledger = Ledger::open(&path).unwrap();
    assert_eq!(ledger.balance_at("buyer", 10).unwrap().available, 100);
    assert!(
        ledger
            .statement_at("buyer", 10)
            .unwrap()
            .events
            .iter()
            .all(|event| event.recorded_at.is_none())
    );
    assert!(
        apply(
            &mut ledger,
            10,
            "reinterpret",
            Operation::FundingPolicy { policy: policy() }
        )
        .is_err()
    );
    apply(&mut ledger, 20, "hold", reserve("one", 1)).unwrap();
    assert!(apply(&mut ledger, 19, "rollback", reserve("two", 1)).is_err());
    drop(ledger);
    let ledger = Ledger::open(&path).unwrap();
    assert_eq!(ledger.balance_at("buyer", 30).unwrap().reserved, 1);
    assert!(
        ledger
            .hold("buyer", "one")
            .unwrap()
            .funding_policy
            .is_none()
    );
}
