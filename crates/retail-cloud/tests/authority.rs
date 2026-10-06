//! #10708: the four authorities deny independently, before any side effect.

use pay_ledger::{
    Ledger,
    compute::{Binding, PrincipalKind, Rights, credential_digest},
};
use retail_cloud::authority::{
    AfterRevocation, Authority, Change, Current, DenialReason, DisclosureConsent, ExecuteGrant,
    GrantSource, ObserveGrant, RetailAdmission, SpendRight, Stage, Step, after_revocation, check,
    spend_right,
};
use retail_cloud::contract::{self, TaskRequest};
use route_contract::price_book::Placement;

const STEPS: [Step; 6] = [
    Step::Reserve,
    Step::Provision,
    Step::Dispatch,
    Step::Read,
    Step::Control,
    Step::UploadMaterial,
];
const AUTHORITIES: [Authority; 4] = [
    Authority::Observe,
    Authority::Execute,
    Authority::Disclose,
    Authority::Spend,
];

fn request() -> TaskRequest {
    TaskRequest {
        source: retail_cloud::authority::Source {
            repository: "https://github.com/OpenAgentsInc/example".into(),
            commit: "a".repeat(40),
        },
        task: "Fix the failing test.".into(),
        checks: vec!["cargo test".into()],
        max_seconds: 600,
        ceiling_sats: None,
    }
}

fn admission() -> RetailAdmission {
    let quote = contract::price_book()
        .quote(
            &Placement::Retail {
                computer: contract::COMPUTER_CLASS.into(),
                task: contract::TASK_CLASS.into(),
            },
            600,
            None,
        )
        .unwrap()
        .unwrap();
    contract::admission("acct-a", "exec-1", &request(), &quote, 1)
}

fn full(admission: &RetailAdmission) -> Current {
    Current {
        observe: Some(ObserveGrant {
            account: "acct-a".into(),
            execution: "exec-1".into(),
            revoked: false,
        }),
        execute: Some(ExecuteGrant {
            source: GrantSource::Retail,
            execution: "exec-1".into(),
            generation: 1,
            revoked: false,
        }),
        disclose: Some(DisclosureConsent {
            admission: admission.digest(),
            withdrawn: false,
        }),
        spend: Some(SpendRight {
            account: "acct-a".into(),
        }),
        ..Current::default()
    }
}

fn without(mut current: Current, authority: Authority) -> Current {
    match authority {
        Authority::Observe => current.observe = None,
        Authority::Execute => current.execute = None,
        Authority::Disclose => current.disclose = None,
        Authority::Spend => current.spend = None,
    }
    // None of these supplies a missing right.
    current.paired = true;
    current.world_member = true;
    current.balance_msat = 1_000_000;
    current.invoice_paid = true;
    current
}

#[test]
fn each_authority_denies_on_its_own_with_no_side_effect() {
    let admission = admission();
    for step in STEPS {
        assert!(
            check(step, &admission, &full(&admission)).is_ok(),
            "{step:?}"
        );
        for authority in AUTHORITIES {
            let current = without(full(&admission), authority);
            let mut side_effects = 0;
            let result = check(step, &admission, &current).inspect(|()| side_effects += 1);
            if step.needs().contains(&authority) {
                let denial = result.unwrap_err();
                assert_eq!(denial.authority, authority, "{step:?}");
                assert_eq!(denial.reason, DenialReason::Missing);
                assert_eq!(side_effects, 0, "{step:?} without {authority:?}");
            } else {
                assert!(result.is_ok(), "{step:?} does not need {authority:?}");
                assert_eq!(side_effects, 1);
            }
        }
    }
}

#[test]
fn only_a_retail_grant_for_this_execution_admits_execution() {
    let admission = admission();
    for source in [
        GrantSource::Operator,
        GrantSource::Pool,
        GrantSource::Autostart,
        GrantSource::Pairing,
    ] {
        let mut current = full(&admission);
        current.execute.as_mut().unwrap().source = source;
        let denial = check(Step::Dispatch, &admission, &current).unwrap_err();
        assert_eq!(denial.reason, DenialReason::NotRetail);
    }
    let mut current = full(&admission);
    current.execute.as_mut().unwrap().execution = "exec-2".into();
    assert_eq!(
        check(Step::Provision, &admission, &current)
            .unwrap_err()
            .reason,
        DenialReason::Mismatch
    );
    let mut current = full(&admission);
    current.execute.as_mut().unwrap().generation = 2;
    assert_eq!(
        check(Step::Dispatch, &admission, &current)
            .unwrap_err()
            .reason,
        DenialReason::Mismatch
    );
    let mut current = full(&admission);
    current.observe.as_mut().unwrap().account = "acct-b".into();
    assert_eq!(
        check(Step::Read, &admission, &current).unwrap_err().reason,
        DenialReason::Mismatch
    );
    let mut current = full(&admission);
    current.spend.as_mut().unwrap().account = "acct-b".into();
    assert_eq!(
        check(Step::Reserve, &admission, &current)
            .unwrap_err()
            .reason,
        DenialReason::Mismatch
    );
}

#[test]
fn every_material_change_needs_a_new_admission() {
    let base = admission();
    assert!(base.changes(&base).is_empty());
    let mut cases: Vec<(RetailAdmission, Change)> = Vec::new();
    let mut a = base.clone();
    a.recipients.push(route_contract::snapshot::Recipient {
        kind: route_contract::snapshot::RecipientKind::ModelProvider,
        id: "anthropic".into(),
    });
    cases.push((a, Change::Recipients));
    let mut a = base.clone();
    a.source.commit = "b".repeat(40);
    cases.push((a, Change::Source));
    let mut a = base.clone();
    a.grant_generation = 2;
    cases.push((a, Change::Computer));
    let mut a = base.clone();
    a.effects.publication = vec![route_contract::snapshot::Publication::Push];
    cases.push((a, Change::Effects));
    let mut a = base.clone();
    a.model_payer = route_contract::snapshot::Payer::OpenAgents;
    cases.push((a, Change::Payer));
    let mut a = base.clone();
    a.max_charge_sats += 1;
    cases.push((a, Change::Quote));
    for (proposed, change) in cases {
        assert_eq!(base.changes(&proposed), vec![change]);
        // Consent given for the old admission does not cover the new one.
        let current = full(&base);
        assert_eq!(
            check(Step::Dispatch, &proposed, &current)
                .unwrap_err()
                .reason,
            DenialReason::Mismatch
        );
    }
}

#[test]
fn revocation_blocks_new_dispatch_and_keeps_obligations() {
    let admission = admission();
    let mut current = full(&admission);
    current.execute.as_mut().unwrap().revoked = true;
    let denial = check(Step::Dispatch, &admission, &current).unwrap_err();
    assert_eq!(denial.reason, DenialReason::Revoked);
    assert!(check(Step::Control, &admission, &current).is_err());
    // Observation is its own right: the customer still sees what happens.
    assert!(check(Step::Read, &admission, &current).is_ok());
    assert_eq!(
        after_revocation(Stage::Reserved),
        AfterRevocation::ReleaseHold
    );
    assert_eq!(
        after_revocation(Stage::Provisioned),
        AfterRevocation::TeardownWithoutCharge
    );
    assert_eq!(
        after_revocation(Stage::ExecutorStarted),
        AfterRevocation::StopAndSettle
    );
    assert_eq!(after_revocation(Stage::Unknown), AfterRevocation::Reconcile);

    let mut current = full(&admission);
    current.disclose.as_mut().unwrap().withdrawn = true;
    assert_eq!(
        check(Step::UploadMaterial, &admission, &current)
            .unwrap_err()
            .reason,
        DenialReason::Revoked
    );
}

#[test]
fn the_spend_right_is_read_from_the_ledger_now() {
    let mut ledger = Ledger::in_memory().unwrap();
    ledger.create_compute_account("acct-a", 0).unwrap();
    ledger
        .bind_principal(&Binding {
            principal: "cli:laptop".into(),
            account: "acct-a".into(),
            kind: PrincipalKind::Cli,
            credential: credential_digest("s"),
            rights: Rights {
                read: true,
                spend: true,
            },
            at: 0,
        })
        .unwrap();
    assert_eq!(
        spend_right(&ledger, "cli:laptop", &credential_digest("s")),
        Some(SpendRight {
            account: "acct-a".into()
        })
    );
    ledger.revoke_principal("cli:laptop", 1).unwrap();
    assert_eq!(
        spend_right(&ledger, "cli:laptop", &credential_digest("s")),
        None
    );
}
