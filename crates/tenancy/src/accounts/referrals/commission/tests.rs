use super::*;

fn terms(version: &str) -> Terms {
    Terms {
        schema: SCHEMA.into(), version: version.into(), products: [Product::PluginCall, Product::AcceptedService].into(),
        base: Base::OpenagentsAvailableEarnedShare, share: Fraction { numerator: 1, denominator: 4 },
        unit: Unit::Millisatoshis, conversion: Conversion::SameUnitOnly, rounding: Rounding::Down,
        payout_precision: PayoutPrecision::WholeSatoshiRetainRemainder,
        hold_secs: 60, hold: Hold::VerifiedEarnedCostsAfterHold, minimum: 1000,
        destinations: [Destination::QualifiedSpark, Destination::QualifiedLightningAddress].into(),
        reversal: Reversal::VerifiedRefundDisputeAdjustsReferrerLiabilityPreservesAuthorShares,
        permanence: Permanence::RetainAcceptedVersionUntilBothReaccept,
        attribution_conflict: Conflict::SuspendNewEligibilityRetainHistory,
        exclusions: [Exclusion::UnusedFunding, Exclusion::PromotionalFreeCredit, Exclusion::SelfReferral, Exclusion::RecycledFunding, Exclusion::UnknownCosts, Exclusion::UnresolvedAttribution].into(),
        effective_from: 0, terms: "Synthetic fixture terms only. Verified earned OpenAgents share after every known cost and promotion forms the base. Hold new earnings for the declared interval; unknown outcomes remain held. Payout requires the declared minimum in the exact unit and a currently qualified destination. Refunds or disputes recompute the earned base and retain any paid referrer reversal liability without changing signed author shares. No accrual or payout is enabled by this contract.".into(), digest: String::new(),
    }.seal().unwrap()
}
fn fixture() -> (
    tempfile::TempDir,
    Accounts,
    Account,
    Account,
    Referrer,
    attribution::Policy,
    attribution::Binding,
) {
    let dir = tempfile::tempdir().unwrap();
    let accounts = Accounts::install(dir.path()).unwrap();
    let source = accounts.create_account("Synthetic source", &[]).unwrap();
    let customer = accounts.create_account("Synthetic customer", &[]).unwrap();
    let r = accounts
        .create_referrer(&source.id, Kind::Person, "Synthetic introduction")
        .unwrap();
    let p = attribution::Policy::new(
        "fixture".into(),
        "Synthetic mutual attribution, no payment rights.".into(),
    )
    .unwrap();
    accounts.publish_attribution_policy(&p).unwrap();
    let d = accounts
        .propose_attribution(&customer.id, &proposal(&p, &r.id, None))
        .unwrap();
    accounts
        .confirm_attribution(&source.id, &customer.id, &d.digest)
        .unwrap();
    let binding = accounts
        .attribution(&customer.id)
        .unwrap()
        .unwrap()
        .binding
        .unwrap();
    (dir, accounts, source, customer, r, p, binding)
}
fn proposal(
    p: &attribution::Policy,
    referrer: &str,
    expected: Option<String>,
) -> attribution::Proposal {
    attribution::Proposal {
        request: if expected.is_none() {
            "early"
        } else {
            "correct"
        }
        .into(),
        policy_digest: p.digest.clone(),
        introduction: if expected.is_none() {
            attribution::Introduction::EarlyAgreement
        } else {
            attribution::Introduction::Correction
        },
        referrer: Some(referrer.into()),
        evidence: vec![attribution::Evidence {
            reference: "private-mutual-agreement".into(),
            digest: format!("sha256:{}", "a".repeat(64)),
        }],
        reason: "The synthetic parties confirm this exact relationship.".into(),
        consent: true,
        expected_decision: expected,
    }
}
fn input(t: &Terms, b: &attribution::Binding, request: &str) -> Input {
    Input {
        request: request.into(),
        customer: b.customer.clone(),
        terms_digest: t.digest.clone(),
        attribution_decision: b.accepted_decision.clone(),
        consent: true,
    }
}
fn accepted(accounts: &Accounts, source: &str, t: &Terms, b: &attribution::Binding) -> View {
    accounts
        .publish_commission_terms(t, &t.digest, None)
        .unwrap();
    let one = accounts
        .accept_commission_terms_guarded(&b.customer, &input(t, b, "customer"), || true)
        .unwrap();
    assert!(!one.terms_qualified);
    accounts
        .accept_commission_terms_guarded(source, &input(t, b, "referrer"), || true)
        .unwrap()
}

#[test]
fn shapes_units_shares_reversal_and_exclusions_are_explicit_and_digested() {
    let t = terms("shape");
    let mut changed = t.clone();
    changed.minimum += 1;
    assert_eq!(changed.validate(), Err(Error::Invalid));
    let mut invalid = t.clone();
    invalid.share.denominator = 0;
    assert_eq!(invalid.seal(), Err(Error::Invalid));
    let mut invalid = t.clone();
    invalid.share.numerator = 5;
    assert_eq!(invalid.seal(), Err(Error::Invalid));
    let mut invalid = t.clone();
    invalid.exclusions.remove(&Exclusion::UnusedFunding);
    assert_eq!(invalid.seal(), Err(Error::Invalid));
    let mut invalid = t.clone();
    invalid.unit = Unit::CurrencyMillionths {
        currency: "usd".into(),
    };
    assert_eq!(invalid.seal(), Err(Error::Invalid));
    let mut json = serde_json::to_value(&t).unwrap();
    json.as_object_mut().unwrap().remove("reversal");
    assert!(serde_json::from_value::<Terms>(json).is_err());
    let mut zero = t;
    zero.share.numerator = 0;
    assert_eq!(zero.seal().unwrap().share.numerator, 0);
}
#[test]
fn btc_rails_refuse_foreign_currency_fractional_minimum_and_native_overflow_before_publication() {
    let (_dir, a, _, _, _, _, _) = fixture();
    let t = terms("precision");
    let sequence = a.store().unwrap().sequence;
    let mut usd = t.clone();
    usd.unit = Unit::CurrencyMillionths {
        currency: "USD".into(),
    };
    assert_eq!(usd.seal(), Err(Error::Invalid));
    usd = t.clone();
    usd.unit = Unit::CurrencyMillionths {
        currency: "USD".into(),
    };
    usd.digest = usd.computed();
    assert_eq!(
        a.publish_commission_terms(&usd, &usd.digest, None),
        Err(Error::Invalid)
    );
    assert_eq!(a.store().unwrap().sequence, sequence);
    let mut fractional = t.clone();
    fractional.minimum = 1500;
    assert_eq!(fractional.seal(), Err(Error::Invalid));
    let mut overflow = t.clone();
    overflow.unit = Unit::Satoshis;
    overflow.minimum = u64::MAX;
    assert_eq!(overflow.seal(), Err(Error::Invalid));
    let mut sats = t.clone();
    sats.unit = Unit::Satoshis;
    sats.minimum = 1;
    sats = sats.seal().unwrap();
    assert_eq!(sats.amount_msat(sats.minimum).unwrap(), 1000);
    let mut btc = t.clone();
    btc.unit = Unit::CurrencyMillionths {
        currency: "BTC".into(),
    };
    btc.minimum = 1;
    btc = btc.seal().unwrap();
    assert_eq!(btc.amount_msat(btc.minimum).unwrap(), 100_000);
    assert_eq!(t.amount_msat(t.minimum).unwrap(), 1000);
    assert_eq!(t.amount_msat(u64::MAX), Err(Error::Invalid));
}
#[test]
fn preview_preserves_author_shares_and_refuses_unknown_or_ineligible_economics() {
    let mut t = terms("economics");
    let mut f = EconomicFacts {
        product: Product::PluginCall,
        unit: Unit::Millisatoshis,
        earned: true,
        admitted_attribution_verified: true,
        promotional_or_free: false,
        self_or_recycled: false,
        settled_distributable: 20_000,
        author_resource_shares: 7_000,
        openagents_share: 13_000,
        costs: [Some(1000), Some(1000), Some(0), Some(0), Some(0), Some(0)],
        promotions: 1000,
    };
    let p = t.preview(&f).unwrap().unwrap();
    assert_eq!(
        (
            p.author_resource_shares,
            p.base,
            p.commission,
            p.openagents_remaining
        ),
        (7000, 10000, 2500, 7500)
    );
    assert_eq!(
        p.author_resource_shares + p.costs + p.promotions + p.commission + p.openagents_remaining,
        f.settled_distributable
    );
    assert!(!p.accrual_enabled);
    f.costs[4] = None;
    assert_eq!(t.preview(&f).unwrap(), None);
    f.costs[4] = Some(0);
    f.earned = false;
    assert_eq!(t.preview(&f).unwrap(), None);
    f.earned = true;
    f.admitted_attribution_verified = false;
    assert_eq!(t.preview(&f).unwrap(), None);
    f.admitted_attribution_verified = true;
    f.promotional_or_free = true;
    assert_eq!(t.preview(&f).unwrap(), None);
    f.promotional_or_free = false;
    f.self_or_recycled = true;
    assert_eq!(t.preview(&f).unwrap(), None);
    f.self_or_recycled = false;
    f.unit = Unit::Satoshis;
    assert_eq!(t.preview(&f), Err(Error::Invalid));
    f.unit = Unit::Millisatoshis;
    f.settled_distributable += 1;
    assert_eq!(t.preview(&f), Err(Error::Invalid));
    f.settled_distributable -= 1;
    f.promotions += 1;
    assert_eq!(t.preview(&f).unwrap().unwrap().remainder, 3);
    t.rounding = Rounding::Exact;
    t = t.seal().unwrap();
    assert_eq!(t.preview(&f), Err(Error::Invalid));
}
#[test]
fn publication_requires_explicit_digest_cas_and_preserves_immutable_versions() {
    let (_dir, a, _, _, _, _, _) = fixture();
    let t = terms("one");
    assert_eq!(
        a.publish_commission_terms(&t, "wrong", None),
        Err(Error::Invalid)
    );
    let first = a.publish_commission_terms(&t, &t.digest, None).unwrap();
    let mut changed = t.clone();
    changed.minimum += 1000;
    changed = changed.seal().unwrap();
    assert_eq!(
        a.publish_commission_terms(&changed, &changed.digest, Some(&t.digest)),
        Err(Error::Conflict)
    );
    let next = terms("two");
    assert_eq!(
        a.publish_commission_terms(&next, &next.digest, None),
        Err(Error::Conflict)
    );
    a.publish_commission_terms(&next, &next.digest, Some(&t.digest))
        .unwrap();
    assert_eq!(
        a.publish_commission_terms(&t, &t.digest, None).unwrap(),
        first
    );
    assert_eq!(a.commission_publication(None).unwrap().unwrap().terms, next);
    assert_eq!(
        a.commission_publication(Some(&t.digest)).unwrap(),
        Some(first)
    );
}
#[test]
fn consent_pins_native_parties_exact_terms_and_survives_publication_migration_restart() {
    let (dir, a, source, customer, r, _, b) = fixture();
    let t = terms("one");
    let original = accepted(&a, &source.id, &t, &b);
    assert!(original.terms_qualified);
    assert!(!original.accrual_enabled);
    let sequence = a.store().unwrap().sequence;
    assert_eq!(
        a.accept_commission_terms_guarded(&source.id, &input(&t, &b, "referrer"), || true)
            .unwrap(),
        original
    );
    assert_eq!(a.store().unwrap().sequence, sequence);
    let other = a.create_account("Unrelated", &[]).unwrap();
    assert_eq!(
        a.commission_agreement_guarded(
            &other.id,
            &customer.id,
            Some(&original.agreement.id),
            || true
        ),
        Err(Error::Unauthorized)
    );
    assert_eq!(
        a.accept_commission_terms_guarded(&other.id, &input(&t, &b, "foreign"), || true),
        Err(Error::Unauthorized)
    );
    let next = terms("two");
    a.publish_commission_terms(&next, &next.digest, Some(&t.digest))
        .unwrap();
    let pending = a
        .accept_commission_terms_guarded(&customer.id, &input(&next, &b, "customer-new"), || true)
        .unwrap();
    assert!(!pending.terms_qualified);
    assert_eq!(
        a.commission_agreement_guarded(&customer.id, &customer.id, None, || true)
            .unwrap(),
        Some(original.clone())
    );
    a.offer_referrer_migration(&source.id, &r.id, &other.id)
        .unwrap();
    a.accept_referrer_migration(&other.id, &r.id).unwrap();
    let mut migrated = original.clone();
    migrated.current_referrer_owner = Some(other.id.clone());
    let reopened = Accounts::open(dir.path()).unwrap();
    assert_eq!(
        reopened
            .commission_agreement_guarded(
                &other.id,
                &customer.id,
                Some(&original.agreement.id),
                || true
            )
            .unwrap(),
        Some(migrated)
    );
    assert_eq!(
        reopened.commission_agreement_guarded(
            &source.id,
            &customer.id,
            Some(&original.agreement.id),
            || true
        ),
        Err(Error::Unauthorized)
    );
    let newer = reopened
        .accept_commission_terms_guarded(&other.id, &input(&next, &b, "successor"), || true)
        .unwrap();
    assert!(newer.terms_qualified);
    assert!(newer.active_for_new_transactions);
    assert!(
        newer.agreement.acceptances[&Party::Referrer]
            .manager_successor
            .is_some()
    );
    let retained = reopened
        .commission_agreement_guarded(
            &customer.id,
            &customer.id,
            Some(&original.agreement.id),
            || true,
        )
        .unwrap()
        .unwrap();
    assert_eq!(retained.agreement, original.agreement);
    assert_eq!(retained.terms, original.terms);
    assert!(retained.terms_qualified);
    assert!(!retained.active_for_new_transactions);
}
#[test]
fn current_authority_future_effective_time_and_review_prevent_new_eligibility() {
    let (_dir, a, source, customer, r, p, b) = fixture();
    let t = terms("one");
    let original = accepted(&a, &source.id, &t, &b);
    let before = a.store().unwrap().sequence;
    assert_eq!(
        a.accept_commission_terms_guarded(&customer.id, &input(&t, &b, "revoked"), || false),
        Err(Error::Unauthorized)
    );
    assert_eq!(
        a.commission_agreement_guarded(&source.id, &customer.id, None, || false),
        Err(Error::Unauthorized)
    );
    assert_eq!(
        a.commission_publication_guarded(&customer.id, None, || false),
        Err(Error::Unauthorized)
    );
    assert_eq!(a.store().unwrap().sequence, before);
    let mut future = terms("future");
    future.effective_from = unix_now() + 3600;
    future = future.seal().unwrap();
    a.publish_commission_terms(&future, &future.digest, Some(&t.digest))
        .unwrap();
    assert_eq!(
        a.accept_commission_terms_guarded(&customer.id, &input(&future, &b, "future"), || true),
        Err(Error::Conflict)
    );
    a.propose_attribution(
        &customer.id,
        &proposal(&p, &r.id, Some(b.accepted_decision.clone())),
    )
    .unwrap();
    let suspended = a
        .commission_agreement_guarded(
            &customer.id,
            &customer.id,
            Some(&original.agreement.id),
            || true,
        )
        .unwrap()
        .unwrap();
    assert!(!suspended.terms_qualified);
    assert_eq!(suspended.state, "suspended-attribution-review");
    assert_eq!(suspended.agreement, original.agreement);
    assert_eq!(
        a.accept_commission_terms_guarded(&source.id, &input(&t, &b, "referrer"), || true),
        Err(Error::Conflict)
    );
}

#[test]
fn corrected_relationship_preserves_exact_history_without_disclosing_it_to_new_referrer() {
    let (dir, a, source, customer, _, p, b) = fixture();
    let t = terms("one");
    let original = accepted(&a, &source.id, &t, &b);
    let other = a.create_account("Other referrer", &[]).unwrap();
    let r = a
        .create_referrer(&other.id, Kind::Person, "Reviewed replacement")
        .unwrap();
    let pending = a
        .propose_attribution(
            &customer.id,
            &proposal(&p, &r.id, Some(b.accepted_decision.clone())),
        )
        .unwrap();
    a.confirm_attribution(&other.id, &customer.id, &pending.digest)
        .unwrap();
    let reopened = Accounts::open(dir.path()).unwrap();
    let history = reopened
        .commission_agreement_guarded(
            &source.id,
            &customer.id,
            Some(&original.agreement.id),
            || true,
        )
        .unwrap()
        .unwrap();
    assert_eq!(history.agreement, original.agreement);
    assert!(!history.terms_qualified);
    assert_eq!(
        reopened.commission_agreement_guarded(
            &other.id,
            &customer.id,
            Some(&original.agreement.id),
            || true
        ),
        Err(Error::Unauthorized)
    );
    let new = reopened
        .attribution(&customer.id)
        .unwrap()
        .unwrap()
        .binding
        .unwrap();
    assert_eq!(
        reopened.accept_commission_terms_guarded(
            &source.id,
            &input(&t, &new, "foreign-stable-manager"),
            || true
        ),
        Err(Error::Unauthorized)
    );
    let pending = reopened
        .accept_commission_terms_guarded(&customer.id, &input(&t, &new, "new-customer"), || true)
        .unwrap();
    assert!(!pending.terms_qualified);
    assert_eq!(
        reopened
            .commission_agreement_guarded(
                &other.id,
                &customer.id,
                Some(&pending.agreement.id),
                || true
            )
            .unwrap(),
        Some(pending)
    );
}

#[test]
fn self_referral_and_source_only_agents_cannot_accept_a_commission_contract() {
    let (_dir, a, source, _, _, p, _) = fixture();
    let t = terms("one");
    a.publish_commission_terms(&t, &t.digest, None).unwrap();
    for sales_agent in [true, false] {
        let customer = a
            .create_account("Synthetic unqualified customer", &[])
            .unwrap();
        let r = if sales_agent {
            a.create_sales_referrer(&source.id, "Source-only agent")
                .unwrap()
        } else {
            a.create_referrer(&customer.id, Kind::Person, "Self")
                .unwrap()
        };
        let d = a
            .propose_attribution(&customer.id, &proposal(&p, &r.id, None))
            .unwrap();
        assert_eq!(d.status, attribution::Status::Review);
        let value = Input {
            request: "no-commission".into(),
            customer: customer.id.clone(),
            terms_digest: t.digest.clone(),
            attribution_decision: d.digest,
            consent: true,
        };
        let sequence = a.store().unwrap().sequence;
        assert_eq!(
            a.accept_commission_terms_guarded(&customer.id, &value, || true),
            Err(Error::Conflict)
        );
        assert_eq!(a.store().unwrap().sequence, sequence);
    }
}
