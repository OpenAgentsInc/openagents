use super::super::{CreditKind, Ledger, Mutation, Operation, Price, Rate, Resource};
use super::*;

const WORKSPACE: &str = "native-workspace";
const A: &str = "acct_aaaaaaaaaaaaaaaa";
const B: &str = "acct_bbbbbbbbbbbbbbbb";

fn limit(cap: u64) -> Limit {
    Limit {
        cap,
        alert_at: cap / 2,
    }
}
fn policy(workspace: u64, team: u64, person: u64) -> Policy {
    Policy {
        schema: SCHEMA.into(),
        version: 1,
        currency: "USD".into(),
        scale: SCALE,
        route: ROUTE.into(),
        effective_from: 0,
        workspace: limit(workspace),
        teams: [("delivery".into(), limit(team))].into(),
        people: [A, B]
            .into_iter()
            .map(|p| {
                (
                    p.into(),
                    Person {
                        team: "delivery".into(),
                        limit: limit(person),
                    },
                )
            })
            .collect(),
    }
}
fn price() -> Price {
    Price {
        version: "fixture-v1".into(),
        currency: "USD".into(),
        model: "native-fixture".into(),
        capacity: "dedicated".into(),
        policy: "observed-usage-v1".into(),
        rates: [(
            Resource::InputTokens,
            Rate {
                millionths: 7,
                per_units: 2,
            },
        )]
        .into(),
    }
}
fn apply(ledger: &mut Ledger, id: &str, operation: Operation) -> Result<bool, String> {
    ledger.apply(Mutation {
        workspace: WORKSPACE.into(),
        source: id.into(),
        audit: format!("synthetic:{id}"),
        operation,
    })
}
fn funded() -> (tempfile::TempDir, Ledger) {
    let root = tempfile::tempdir().unwrap();
    let mut ledger = Ledger::open(&root.path().join("money.jsonl")).unwrap();
    apply(
        &mut ledger,
        "create",
        Operation::Create {
            currency: "USD".into(),
            spend_limit: 100_000,
            topups_allowed: false,
        },
    )
    .unwrap();
    apply(
        &mut ledger,
        "credit",
        Operation::Credit {
            amount: 100_000,
            credit_kind: CreditKind::Grant,
        },
    )
    .unwrap();
    (root, ledger)
}
fn reserve_scoped_test(
    ledger: &mut Ledger,
    id: &str,
    person: &str,
    units: u64,
) -> Result<bool, String> {
    let operation = scoped(ledger, person, id, units);
    apply(ledger, id, operation)
}

fn scoped(ledger: &Ledger, person: &str, attempt: &str, units: u64) -> Operation {
    Operation::ReserveScoped {
        attempt: attempt.into(),
        request_digest: format!("digest:{attempt}"),
        price: price(),
        maximum_usage: [(Resource::InputTokens, units)].into(),
        budget: ledger
            .budget_admission(WORKSPACE, person, &format!("sha256:{}", "1".repeat(64)), 3)
            .unwrap(),
    }
}

#[test]
fn each_level_enforces_rounded_liability_and_replay_never_reserves_twice() {
    for (limits, level) in [
        ((500, 1_000, 1_000), Level::Workspace),
        ((1_000, 500, 1_000), Level::Team),
        ((1_000, 1_000, 500), Level::Person),
    ] {
        let (_root, mut ledger) = funded();
        apply(
            &mut ledger,
            "policy",
            Operation::BudgetPolicy {
                policy: policy(limits.0, limits.1, limits.2),
            },
        )
        .unwrap();
        let op = scoped(&ledger, A, "first", 128);
        assert!(apply(&mut ledger, "first", op.clone()).unwrap());
        assert!(!apply(&mut ledger, "first", op).unwrap());
        assert_eq!(
            ledger
                .budget_view(WORKSPACE, A, false, None)
                .unwrap()
                .workspace
                .used,
            448
        );
        let blocked = ledger
            .check_budget(
                WORKSPACE,
                &ledger
                    .budget_admission(WORKSPACE, A, &format!("sha256:{}", "1".repeat(64)), 3)
                    .unwrap(),
                53,
            )
            .unwrap()
            .unwrap();
        assert_eq!(blocked.bound.level, level);
        assert_eq!(blocked.bound.remaining, 52);
        assert!(reserve_scoped_test(&mut ledger, "second", A, 16).is_err()); // 56, rounded per attempt.
        assert_eq!(ledger.holds(WORKSPACE).len(), 1);
    }
}

#[test]
fn lowered_policy_and_restart_keep_unknown_holds_originally_pinned() {
    let (root, mut ledger) = funded();
    let original = policy(1_000, 1_000, 1_000);
    apply(
        &mut ledger,
        "policy1",
        Operation::BudgetPolicy {
            policy: original.clone(),
        },
    )
    .unwrap();
    reserve_scoped_test(&mut ledger, "first", A, 128).unwrap();
    let pinned = ledger.hold(WORKSPACE, "first").unwrap().budget.clone();
    let mut lowered = policy(400, 400, 400);
    lowered.version = 2;
    apply(
        &mut ledger,
        "policy2",
        Operation::BudgetPolicy { policy: lowered },
    )
    .unwrap();
    assert_eq!(ledger.hold(WORKSPACE, "first").unwrap().budget, pinned);
    let view = ledger.budget_view(WORKSPACE, A, false, Some(1)).unwrap();
    assert_eq!(view.workspace.alert, Alert::Exceeded);
    assert_eq!(view.workspace.remaining, 0);
    assert!(reserve_scoped_test(&mut ledger, "second", B, 1).is_err());
    drop(ledger);
    let mut ledger = Ledger::open(&root.path().join("money.jsonl")).unwrap();
    assert_eq!(
        ledger.hold(WORKSPACE, "first").unwrap().phase,
        Phase::Unknown
    );
    assert_eq!(ledger.hold(WORKSPACE, "first").unwrap().budget, pinned);
    assert_eq!(
        ledger
            .budget_view(WORKSPACE, A, false, None)
            .unwrap()
            .workspace
            .unknown,
        448
    );
    assert!(reserve_scoped_test(&mut ledger, "second", B, 1).is_err());
    apply(
        &mut ledger,
        "verified-settlement",
        Operation::Settle {
            attempt: "first".into(),
            usage: [(Resource::InputTokens, 3)].into(),
            receipt: "native-observation".into(),
            provider_cost: None,
            hosting_cost: None,
        },
    )
    .unwrap();
    let view = ledger.budget_view(WORKSPACE, A, false, None).unwrap();
    assert_eq!(view.workspace.used, 11);
    assert_eq!(view.workspace.unknown, 0);
}

#[test]
fn preexisting_unattributed_liability_is_conservative_and_never_guessed() {
    let (root, mut ledger) = funded();
    apply(
        &mut ledger,
        "legacy",
        Operation::Reserve {
            attempt: "legacy".into(),
            request_digest: "legacy-input".into(),
            price: price(),
            maximum_usage: [(Resource::InputTokens, 128)].into(),
        },
    )
    .unwrap();
    drop(ledger);
    let mut ledger = Ledger::open(&root.path().join("money.jsonl")).unwrap();
    apply(
        &mut ledger,
        "activate",
        Operation::BudgetPolicy {
            policy: policy(1_000, 500, 500),
        },
    )
    .unwrap();
    for person in [A, B] {
        let view = ledger
            .budget_view(WORKSPACE, person, false, Some(100))
            .unwrap();
        assert_eq!(view.unattributed_used, 448);
        assert_eq!(view.workspace.unknown, 448);
        assert_eq!(view.people[person].used, 448);
        assert_eq!(view.blocked.unwrap().bound.level, Level::Team);
    }
    assert!(ledger.hold(WORKSPACE, "legacy").unwrap().budget.is_none());
    assert!(
        apply(
            &mut ledger,
            "bypass",
            Operation::Reserve {
                attempt: "bypass".into(),
                request_digest: "changed".into(),
                price: price(),
                maximum_usage: [(Resource::InputTokens, 1)].into()
            }
        )
        .is_err()
    );
}

#[test]
fn team_rename_or_person_move_cannot_restart_current_roster_capacity() {
    let (_root, mut ledger) = funded();
    apply(
        &mut ledger,
        "policy1",
        Operation::BudgetPolicy {
            policy: policy(1_000, 500, 1_000),
        },
    )
    .unwrap();
    reserve_scoped_test(&mut ledger, "first", A, 128).unwrap();
    let mut next = policy(1_000, 500, 1_000);
    next.version = 2;
    next.teams = [("renamed".into(), limit(500))].into();
    for person in next.people.values_mut() {
        person.team = "renamed".into();
    }
    apply(
        &mut ledger,
        "policy2",
        Operation::BudgetPolicy { policy: next },
    )
    .unwrap();
    assert_eq!(
        ledger
            .hold(WORKSPACE, "first")
            .unwrap()
            .budget
            .as_ref()
            .unwrap()
            .team,
        "delivery"
    );
    assert_eq!(
        ledger
            .budget_view(WORKSPACE, B, false, Some(100))
            .unwrap()
            .blocked
            .unwrap()
            .bound
            .level,
        Level::Team
    );
}

#[test]
fn only_verified_release_or_refund_restores_capacity_and_member_views_are_private() {
    let (_root, mut ledger) = funded();
    apply(
        &mut ledger,
        "policy",
        Operation::BudgetPolicy {
            policy: policy(1_000, 1_000, 500),
        },
    )
    .unwrap();
    reserve_scoped_test(&mut ledger, "first", A, 128).unwrap();
    apply(
        &mut ledger,
        "unknown",
        Operation::Unknown {
            attempt: "first".into(),
        },
    )
    .unwrap();
    assert_eq!(
        ledger
            .budget_view(WORKSPACE, A, false, None)
            .unwrap()
            .workspace
            .used,
        448
    );
    apply(
        &mut ledger,
        "evidence-release",
        Operation::Release {
            attempt: "first".into(),
        },
    )
    .unwrap();
    reserve_scoped_test(&mut ledger, "second", B, 128).unwrap();
    apply(
        &mut ledger,
        "observed",
        Operation::Settle {
            attempt: "second".into(),
            usage: [(Resource::InputTokens, 3)].into(),
            receipt: "observed".into(),
            provider_cost: None,
            hosting_cost: None,
        },
    )
    .unwrap();
    apply(
        &mut ledger,
        "verified-refund",
        Operation::Refund {
            attempt: "second".into(),
            amount: 5,
        },
    )
    .unwrap();
    let view = ledger.budget_view(WORKSPACE, A, false, None).unwrap();
    assert_eq!(view.people.len(), 1);
    assert!(!view.people.contains_key(B));
    assert_eq!(view.workspace.used, 6);
    assert_eq!(view.teams["delivery"].used, 6);
    assert_eq!(view.people[A].used, 0);
    assert_eq!(
        ledger.budget_view(WORKSPACE, A, true, None).unwrap().people[B].used,
        6
    );
}

#[test]
fn stale_policy_admission_wrong_currency_scale_roster_and_clock_refuse() {
    let (_root, mut ledger) = funded();
    let p = policy(1_000, 1_000, 1_000);
    apply(
        &mut ledger,
        "policy1",
        Operation::BudgetPolicy { policy: p.clone() },
    )
    .unwrap();
    let stale = scoped(&ledger, A, "stale", 1);
    let mut next = p.clone();
    next.version = 2;
    apply(
        &mut ledger,
        "policy2",
        Operation::BudgetPolicy {
            policy: next.clone(),
        },
    )
    .unwrap();
    assert!(apply(&mut ledger, "stale", stale).is_err());
    for case in [
        "currency",
        "scale",
        "future",
        "threshold",
        "roster",
        "version",
    ] {
        let mut invalid = next.clone();
        invalid.version = 3;
        match case {
            "currency" => invalid.currency = "BTC".into(),
            "scale" => invalid.scale = 100,
            "future" => invalid.effective_from = u64::MAX,
            "threshold" => invalid.workspace.alert_at = invalid.workspace.cap + 1,
            "roster" => {
                invalid.people.insert(
                    "display-name".into(),
                    Person {
                        team: "delivery".into(),
                        limit: limit(100),
                    },
                );
            }
            "version" => {
                invalid.version = 2;
                invalid.workspace.cap += 1;
            }
            _ => unreachable!(),
        }
        assert!(
            apply(
                &mut ledger,
                case,
                Operation::BudgetPolicy { policy: invalid }
            )
            .is_err(),
            "{case}"
        );
    }
    assert_eq!(ledger.budget_policy(WORKSPACE).unwrap(), &next);
}

#[test]
fn simultaneous_members_share_one_atomic_authoritative_cap() {
    let (_root, mut ledger) = funded();
    apply(
        &mut ledger,
        "policy",
        Operation::BudgetPolicy {
            policy: policy(600, 1_000, 1_000),
        },
    )
    .unwrap();
    let ledger = std::sync::Arc::new(std::sync::Mutex::new(ledger));
    let start = std::sync::Arc::new(std::sync::Barrier::new(3));
    let handles: Vec<_> = [A, B]
        .into_iter()
        .enumerate()
        .map(|(n, person)| {
            let ledger = ledger.clone();
            let start = start.clone();
            std::thread::spawn(move || {
                start.wait();
                let mut ledger = ledger.lock().unwrap();
                let op = scoped(&ledger, person, &format!("member-{n}"), 128);
                apply(&mut ledger, &format!("member-{n}"), op).is_ok()
            })
        })
        .collect();
    start.wait();
    assert_eq!(
        handles
            .into_iter()
            .map(|h| usize::from(h.join().unwrap()))
            .sum::<usize>(),
        1
    );
    let ledger = ledger.lock().unwrap();
    assert_eq!(
        ledger
            .budget_view(WORKSPACE, A, true, None)
            .unwrap()
            .workspace
            .used,
        448
    );
    assert_eq!(ledger.holds(WORKSPACE).len(), 1);
}

#[test]
fn changing_native_payer_cannot_duplicate_the_gateway_attempt_namespace() {
    let (_root, mut ledger) = funded();
    apply(
        &mut ledger,
        "policy",
        Operation::BudgetPolicy {
            policy: policy(1_000, 1_000, 1_000),
        },
    )
    .unwrap();
    reserve_scoped_test(&mut ledger, "original", A, 128).unwrap();
    for (source, operation) in [
        (
            "other-create",
            Operation::Create {
                currency: "USD".into(),
                spend_limit: 10_000,
                topups_allowed: false,
            },
        ),
        (
            "other-credit",
            Operation::Credit {
                amount: 10_000,
                credit_kind: CreditKind::Grant,
            },
        ),
        (
            "other-policy",
            Operation::BudgetPolicy {
                policy: policy(1_000, 1_000, 1_000),
            },
        ),
    ] {
        ledger
            .apply(Mutation {
                workspace: "other-workspace".into(),
                source: source.into(),
                audit: "Synthetic second native payer".into(),
                operation,
            })
            .unwrap();
    }
    let budget = ledger
        .budget_admission(
            "other-workspace",
            B,
            &format!("sha256:{}", "2".repeat(64)),
            1,
        )
        .unwrap();
    let result = ledger.apply(Mutation {
        workspace: "other-workspace".into(),
        source: "changed-payer".into(),
        audit: "Synthetic replay".into(),
        operation: Operation::ReserveScoped {
            attempt: "original".into(),
            request_digest: "digest:original".into(),
            price: price(),
            maximum_usage: [(Resource::InputTokens, 128)].into(),
            budget,
        },
    });
    assert!(result.is_err());
    assert!(ledger.hold("other-workspace", "original").is_none());
    assert_eq!(ledger.balance("other-workspace").unwrap().reserved, 0);
    assert_eq!(
        ledger
            .hold(WORKSPACE, "original")
            .unwrap()
            .budget
            .as_ref()
            .unwrap()
            .person,
        A
    );
}
