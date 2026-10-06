//! Shared fixtures for the retail tests: a funded account, one confirmed
//! offer, and the full set of current rights. Everything is simulated.
#![allow(dead_code)]

use pay_ledger::Ledger;
use pay_ledger::compute::{Binding, PrincipalKind, Rights, credential_digest};
use retail_cloud::authority::{
    Current, DisclosureConsent, ExecuteGrant, GrantSource, ObserveGrant, Source, SpendRight,
};
use retail_cloud::contract::{self, TaskRequest};
use retail_cloud::fake::FakeWallet;
use retail_cloud::journal::Journal;
use retail_cloud::offer::{self, Capacity, ConfirmedVia, FundedRequest};
use retail_cloud::topup::{self, TopUpRequest};

pub const NOW: i64 = 1_791_200_000;
pub const FREE: Capacity = Capacity {
    running: 0,
    plan_starts_left: Some(10),
};

pub fn request(seconds: u64) -> TaskRequest {
    TaskRequest {
        source: Source {
            repository: "https://github.com/OpenAgentsInc/example".into(),
            commit: "c".repeat(40),
        },
        task: "Make the parser accept trailing commas.".into(),
        checks: vec!["cargo test -p parser".into()],
        max_seconds: seconds,
        ceiling_sats: None,
    }
}

/// An account `account` with a CLI principal that can spend, topped up by
/// `sats` through the fake wallet.
pub fn funded_account(ledger: &mut Ledger, account: &str, sats: u64) {
    ledger.create_compute_account(account, NOW).unwrap();
    ledger
        .bind_principal(&Binding {
            principal: format!("cli:{account}"),
            account: account.into(),
            kind: PrincipalKind::Cli,
            credential: credential_digest(account),
            rights: Rights {
                read: true,
                spend: true,
            },
            at: NOW,
        })
        .unwrap();
    if sats > 0 {
        let wallet = FakeWallet::new();
        let purchase = topup::request_top_up(
            ledger,
            &wallet,
            &TopUpRequest {
                principal: format!("cli:{account}"),
                credential: credential_digest(account),
                purchase: format!("buy-{account}"),
                amount_sats: sats,
                now: NOW,
            },
        )
        .unwrap();
        topup::on_paid(ledger, &purchase.top_up.payment_hash, sats * 1000, NOW + 1).unwrap();
    }
}

/// Confirm one offer for `request` on `account` as `offer_id`.
pub fn confirmed(
    journal: &mut Journal,
    account: &str,
    offer_id: &str,
    request: &TaskRequest,
) -> FundedRequest {
    let book = contract::price_book();
    let made = offer::make_offer(&book, account, offer_id, request, FREE, NOW as u64).unwrap();
    offer::confirm(
        journal,
        &made,
        &made.offer.digest,
        ConfirmedVia::OfferControl,
        &book,
        FREE,
        NOW as u64 + 1,
    )
    .unwrap()
}

/// Every right the funded request needs, as the host holds it now.
pub fn rights(funded: &FundedRequest) -> Current {
    Current {
        observe: Some(ObserveGrant {
            account: funded.account.clone(),
            execution: funded.execution.clone(),
            revoked: false,
        }),
        execute: Some(ExecuteGrant {
            source: GrantSource::Retail,
            execution: funded.execution.clone(),
            generation: funded.admission.grant_generation,
            revoked: false,
        }),
        disclose: Some(DisclosureConsent {
            admission: funded.admission.digest(),
            withdrawn: false,
        }),
        spend: Some(SpendRight {
            account: funded.account.clone(),
        }),
        ..Current::default()
    }
}

pub const TEMPLATE: &str = "oa-coder-main-20261006";

/// Reserve and provision `funded` until its sandbox is ready; returns the
/// sandbox.
pub fn ready(
    journal: &mut Journal,
    ledger: &mut Ledger,
    provider: &retail_cloud::fake::FakeProvider,
    funded: &FundedRequest,
) -> String {
    retail_cloud::reserve::reserve(ledger, funded, &rights(funded), NOW + 2).unwrap();
    for t in 0..5 {
        let record = retail_cloud::provision::advance(
            journal,
            ledger,
            provider,
            funded,
            &rights(funded),
            TEMPLATE,
            NOW + 3 + t,
        )
        .unwrap();
        if let retail_cloud::provision::ProvisionState::Ready { resource, .. } = record.state {
            return resource;
        }
    }
    panic!("the sandbox never became ready")
}
