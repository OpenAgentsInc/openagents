use super::*;
use nostr::x402::test_invoice::{described, payee, payee_of, signed_at, signed_by};

const T0: u64 = 1_800_000_000;
const PHONE: &str = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
const HOST: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
const OTHER: &str = "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";

fn id(byte: u8) -> String {
    hex(&[byte; 32])
}

fn grant(epoch: u64) -> Grant {
    Grant::request_mode(id(0x10 + epoch as u8), PHONE, HOST, epoch, T0)
}

/// A mainnet invoice for `hrp`'s amount, made at `T0`, lasting 600 seconds.
fn invoice(hrp: &str, preimage: u8) -> String {
    signed_at(
        hrp,
        described([preimage; 32], "Search API call", 600),
        false,
        false,
        T0,
    )
}

fn request(grant: &Grant, byte: u8, hrp: &str, amount_msat: u64) -> SpendRequest {
    SpendRequest {
        v: REQUEST.into(),
        requires: vec![],
        request: id(byte),
        grant: grant.grant.clone(),
        epoch: grant.epoch,
        grantee: HOST.into(),
        payment: invoice(hrp, byte),
        amount_msat,
        fee_max_msat: 1_000,
        purpose: Purpose::X402Purchase,
        context: Context {
            task: Some(id(0xaa)),
            title: Some("Research the market".into()),
            resource: Some("https://tools.example.com/search".into()),
            note: None,
        },
        issued_at: T0,
        expires_at: T0 + 300,
    }
}

/// 25 sats.
fn small(grant: &Grant, byte: u8) -> SpendRequest {
    request(grant, byte, "lnbc250n", 25_000)
}

#[test]
fn the_default_grant_asks_only_and_validates() {
    let grant = grant(0);
    grant.validate().unwrap();
    assert_eq!(grant.mode, Mode::Request);
    assert!(grant.any_payee);
    assert_eq!(grant.rails, [Rail::Lightning]);
    let mut wide = grant.clone();
    wide.fee_max.ppm = 1_000_001;
    assert_eq!(wide.validate().unwrap_err().code, Code::Bounds);
    let mut long = grant.clone();
    long.expires_at = long.issued_at + MAX_GRANT_LIFETIME + 1;
    assert_eq!(long.validate().unwrap_err().code, Code::Malformed);
    let mut unsorted = grant.clone();
    unsorted.purposes.reverse();
    assert_eq!(unsorted.validate().unwrap_err().code, Code::Malformed);
    let mut other = grant;
    other.unit = "sat".into();
    assert_eq!(other.validate().unwrap_err().code, Code::Unsupported);
    // Unknown fields refuse.
    let mut json = serde_json::to_value(super::tests::grant(0)).unwrap();
    json["auto_approve_max"] = 1.into();
    assert!(serde_json::from_value::<Grant>(json).is_err());
}

#[test]
fn a_request_carries_its_exact_invoice_and_never_outlives_it() {
    let grant = grant(0);
    let good = small(&grant, 1);
    let decoded = good.validate().unwrap();
    assert_eq!(decoded.amount_msat, 25_000);
    assert_eq!(decoded.payee, payee());
    let mut long = good.clone();
    long.expires_at = T0 + 601;
    long.issued_at = T0 + 1;
    assert_eq!(long.validate().unwrap_err().code, Code::Malformed);
    let mut wrong = good.clone();
    wrong.amount_msat = 26_000;
    assert_eq!(wrong.validate().unwrap_err().code, Code::Malformed);
    let mut testnet = good;
    testnet.payment = signed_at("lntb250n", described([1; 32], "x", 600), false, false, T0);
    assert_eq!(testnet.validate().unwrap_err().code, Code::Unsupported);
    assert_eq!(
        SpendRequest::id_for(&invoice("lnbc250n", 1)),
        SpendRequest::id_for(&format!(" {} ", invoice("lnbc250n", 1)))
    );
}

#[test]
fn every_check_refuses_with_its_own_code() {
    let ledger = Ledger::default();
    let current = grant(0);
    let now = T0 + 10;
    let ok = small(&current, 1);
    assert!(ledger.check(Some(&current), &ok, now).is_ok());
    let refuse = |grant: Option<&Grant>, request: &SpendRequest, now: u64| {
        ledger.check(grant, request, now).unwrap_err()
    };
    // No grant, or the owner blocked the host.
    assert_eq!(refuse(None, &ok, now), Refusal::Revoked);
    // Another host's request.
    let mut foreign = ok.clone();
    foreign.grantee = OTHER.into();
    assert_eq!(refuse(Some(&current), &foreign, now), Refusal::Revoked);
    // A grant the phone never issued.
    let mut unknown = ok.clone();
    unknown.grant = id(0x77);
    assert_eq!(refuse(Some(&current), &unknown, now), Refusal::Revoked);
    // Expired: the request, the invoice, and the grant.
    assert_eq!(refuse(Some(&current), &ok, T0 + 300), Refusal::Expired);
    let mut expiring = current.clone();
    expiring.expires_at = now;
    assert_eq!(refuse(Some(&expiring), &ok, now), Refusal::Expired);
    // Purpose, payee, and fee.
    let mut narrow = current.clone();
    narrow.purposes = vec![Purpose::LaborPayment];
    assert_eq!(refuse(Some(&narrow), &ok, now), Refusal::PurposeNotAllowed);
    let mut listed = current.clone();
    listed.any_payee = false;
    listed.payees = vec![format!("02{}", "11".repeat(32))];
    assert_eq!(refuse(Some(&listed), &ok, now), Refusal::PayeeNotAllowed);
    listed.payees = vec![hex(&payee())];
    assert!(ledger.check(Some(&listed), &ok, now).is_ok());
    let mut greedy = ok.clone();
    greedy.fee_max_msat = 25_001; // above the 25,000 msat amount
    assert_eq!(refuse(Some(&current), &greedy, now), Refusal::FeeTooHigh);
    // Caps: one payment, the period, and the total.
    let big = request(&current, 2, "lnbc100u", 10_000_000);
    assert_eq!(refuse(Some(&current), &big, now), Refusal::OverPaymentCap);
    let mut daily = current.clone();
    daily.period_max = 30_000;
    daily.total_max = 1_000_000;
    let mut filled = Ledger::default();
    filled
        .reserve(Some(&daily), &small(&daily, 3), now)
        .unwrap();
    assert_eq!(
        filled
            .check(Some(&daily), &small(&daily, 4), now)
            .unwrap_err(),
        Refusal::OverPeriodCap
    );
    // A day later the period has room again; the total does not.
    let mut lifetime = daily.clone();
    lifetime.total_max = 30_000;
    lifetime.per_payment_max = 26_000;
    lifetime.period_max = 26_000;
    let later = now + daily.period + 1;
    let mut again = small(&lifetime, 4);
    again.issued_at = later;
    again.expires_at = later + 60;
    again.payment = signed_at(
        "lnbc250n",
        described([4; 32], "later", 600),
        false,
        false,
        later,
    );
    let mut used = Ledger::default();
    used.reserve(Some(&lifetime), &small(&lifetime, 3), now)
        .unwrap();
    assert_eq!(
        used.check(Some(&lifetime), &again, later).unwrap_err(),
        Refusal::OverTotalCap
    );
    // Rail: a testnet invoice is not a payment the phone makes.
    let mut testnet = ok.clone();
    testnet.payment = signed_at("lntb250n", described([1; 32], "x", 600), false, false, T0);
    assert_eq!(
        refuse(Some(&current), &testnet, now),
        Refusal::RailNotAllowed
    );
    // Malformed: the amount differs from the invoice's.
    let mut wrong = ok.clone();
    wrong.amount_msat = 1;
    assert_eq!(refuse(Some(&current), &wrong, now), Refusal::Malformed);
}

#[test]
fn a_reservation_holds_the_amount_and_fee_until_settled_or_released() {
    let grant = grant(0);
    let now = T0 + 10;
    let mut ledger = Ledger::default();
    let full = ledger.remaining(&grant, now);
    assert_eq!(full.period_msat, grant.period_max);
    let first = small(&grant, 1);
    let entry = ledger.reserve(Some(&grant), &first, now).unwrap();
    assert_eq!(entry.state, State::Reserved);
    assert_eq!(entry.host, HOST);
    assert_eq!(entry.task.as_deref(), Some(id(0xaa).as_str()));
    assert_eq!(entry.payee, hex(&payee()));
    assert_eq!(
        ledger.remaining(&grant, now).period_msat,
        full.period_msat - 26_000
    );
    // The wallet reports it pending: still held.
    ledger.settle(&first.request, false, Some("pay-1".into()), None, now);
    assert_eq!(ledger.entries[&first.request].state, State::Pending);
    assert_eq!(
        ledger.remaining(&grant, now).period_msat,
        full.period_msat - 26_000
    );
    // Then paid with a smaller fee: the actual fee replaces the ceiling.
    ledger.settle(&first.request, true, None, Some(3), now);
    let paid = &ledger.entries[&first.request];
    assert_eq!(paid.state, State::Paid);
    assert_eq!(paid.payment_id.as_deref(), Some("pay-1"));
    assert_eq!(
        ledger.remaining(&grant, now).period_msat,
        full.period_msat - 25_003
    );
    // A refusal never releases a paid payment.
    ledger.refuse(&first, Refusal::DeclinedByOwner, now);
    assert_eq!(ledger.entries[&first.request].state, State::Paid);
    // A reservation the wallet never paid is released by a refusal.
    let second = small(&grant, 2);
    ledger.reserve(Some(&grant), &second, now).unwrap();
    ledger.refuse(&second, Refusal::InsufficientFunds, now);
    assert_eq!(ledger.entries[&second.request].state, State::Refused);
    assert_eq!(
        ledger.remaining(&grant, now).period_msat,
        full.period_msat - 25_003
    );
    // A declined request is recorded with its code and holds nothing.
    let third = small(&grant, 3);
    ledger.refuse(&third, Refusal::DeclinedByOwner, now);
    assert_eq!(
        ledger.entries[&third.request].code,
        Some(Refusal::DeclinedByOwner)
    );
    assert_eq!(
        ledger.reserve(Some(&grant), &third, now).unwrap_err(),
        Refusal::DeclinedByOwner
    );
}

#[test]
fn a_retried_request_reserves_once_and_a_reused_id_conflicts() {
    let grant = grant(0);
    let now = T0 + 10;
    let mut ledger = Ledger::default();
    let first = small(&grant, 1);
    let reserved = ledger.reserve(Some(&grant), &first, now).unwrap();
    let again = ledger.reserve(Some(&grant), &first, now + 5).unwrap();
    assert_eq!(reserved, again);
    assert_eq!(ledger.entries.len(), 1);
    // A retry is not counted against its own period room.
    let mut tight = grant.clone();
    tight.period_max = 26_000;
    tight.per_payment_max = 26_000;
    assert!(ledger.check(Some(&tight), &first, now).is_ok());
    // Paid stays paid on retry.
    ledger.settle(&first.request, true, Some("pay".into()), Some(0), now);
    assert_eq!(
        ledger.reserve(Some(&grant), &first, now).unwrap().state,
        State::Paid
    );
    // The same ID with different bytes.
    let mut reused = first.clone();
    reused.fee_max_msat = 2_000;
    assert_eq!(
        ledger.reserve(Some(&grant), &reused, now).unwrap_err(),
        Refusal::Conflict
    );
    assert_eq!(
        ledger.check(Some(&grant), &reused, now).unwrap_err(),
        Refusal::Conflict
    );
}

#[test]
fn revocation_advances_the_epoch_and_old_requests_are_stale() {
    let old = grant(0);
    let queued = small(&old, 1);
    let ledger = Ledger::default();
    assert!(ledger.check(Some(&old), &queued, T0 + 10).is_ok());
    // The owner revokes: the phone's grant for the host moves to epoch 1.
    let new = grant(1);
    assert_ne!(new.grant, old.grant);
    assert_eq!(
        ledger.check(Some(&new), &queued, T0 + 10).unwrap_err(),
        Refusal::Stale
    );
    // A request that names the current grant with an old epoch is stale too.
    let mut mixed = queued.clone();
    mixed.grant = new.grant.clone();
    assert_eq!(
        ledger.check(Some(&new), &mixed, T0 + 10).unwrap_err(),
        Refusal::Stale
    );
    // Blocked: no grant at all.
    assert_eq!(
        ledger.check(None, &small(&new, 2), T0 + 10).unwrap_err(),
        Refusal::Revoked
    );
    // Totals under the new grant start over.
    assert_eq!(ledger.remaining(&new, T0).total_msat, new.total_max);
}

#[test]
fn only_a_matching_preimage_proves_a_paid_receipt() {
    let grant = grant(0);
    let request = small(&grant, 5);
    let mut receipt = Receipt {
        v: RECEIPT.into(),
        requires: vec![],
        request: request.request.clone(),
        grant: grant.grant.clone(),
        outcome: Settlement::Paid,
        code: None,
        payment_id: Some("pay".into()),
        amount_msat: Some(25_000),
        fees_msat: Some(0),
        proof: Some(hex(&[5; 32])),
        remaining: None,
        at: T0,
    };
    receipt.answers(&request).unwrap();
    receipt.proof = Some(hex(&[6; 32]));
    assert_eq!(receipt.answers(&request).unwrap_err().code, Code::Forbidden);
    receipt.proof = Some(hex(&[5; 32]));
    receipt.amount_msat = Some(1);
    assert_eq!(receipt.answers(&request).unwrap_err().code, Code::Forbidden);
    let refused = Receipt::refused(&request.request, &grant.grant, Refusal::DeclinedByOwner, T0);
    refused.answers(&request).unwrap();
    let mut bad = refused.clone();
    bad.code = None;
    assert_eq!(bad.validate().unwrap_err().code, Code::Malformed);
    let mut leaky = refused;
    leaky.proof = Some(hex(&[5; 32]));
    assert_eq!(leaky.validate().unwrap_err().code, Code::Malformed);
}

/// A standing grant trusting the test payee for `payee_max` a day.
fn standing(payee_max: u64) -> Grant {
    Grant::standing(
        id(0x30),
        PHONE,
        HOST,
        0,
        T0,
        BTreeMap::from([(hex(&payee()), payee_max)]),
    )
}

/// `small`, from the node whose secret is `node` repeated.
fn from_node(grant: &Grant, byte: u8, node: u8) -> SpendRequest {
    let mut request = small(grant, byte);
    request.payment = signed_by(
        [node; 32],
        "lnbc250n",
        described([byte; 32], "Search API call", 600),
        false,
        false,
        T0,
    );
    request
}

#[test]
fn a_standing_grant_names_automatic_payments_within_its_own_bounds() {
    let grant = standing(defaults::AUTO_PAYEE_MAX);
    grant.validate().unwrap();
    assert_eq!(grant.mode, Mode::Standing);
    let auto = grant.auto.clone().unwrap();
    // Automatic payments never exceed the amount the phone asks Face ID for.
    assert!(auto.per_payment_max <= 1_000_000);
    assert_eq!(auto.purposes[&Purpose::Tip], defaults::AUTO_TIP_MAX);
    // It round-trips, and a request-mode grant still has no `auto` on the wire.
    let json = serde_json::to_value(&grant).unwrap();
    assert_eq!(serde_json::from_value::<Grant>(json).unwrap(), grant);
    let plain = serde_json::to_value(super::tests::grant(0)).unwrap();
    assert!(plain.get("auto").is_none());
    let refuse = |change: &dyn Fn(&mut Grant)| {
        let mut bad = grant.clone();
        change(&mut bad);
        bad.validate().unwrap_err().code
    };
    // A standing grant names its automatic payments, and only it does.
    assert_eq!(refuse(&|g| g.auto = None), Code::Malformed);
    assert_eq!(refuse(&|g| g.mode = Mode::Request), Code::Malformed);
    // Automatic ceilings never exceed the grant's.
    assert_eq!(
        refuse(&|g| g.auto.as_mut().unwrap().per_payment_max = g.per_payment_max + 1),
        Code::Bounds
    );
    assert_eq!(
        refuse(&|g| g.auto.as_mut().unwrap().period_max = g.period_max + 1),
        Code::Bounds
    );
    assert_eq!(
        refuse(&|g| {
            g.auto
                .as_mut()
                .unwrap()
                .purposes
                .insert(Purpose::X402Purchase, defaults::AUTO_PERIOD_MAX + 1);
        }),
        Code::Bounds
    );
    // Only the grant's purposes, and only payees it admits.
    assert_eq!(
        refuse(&|g| g.purposes = vec![Purpose::LaborPayment]),
        Code::Bounds
    );
    assert_eq!(
        refuse(&|g| {
            g.any_payee = false;
            g.payees = vec![];
        }),
        Code::Bounds
    );
    assert_eq!(
        refuse(&|g| {
            g.auto.as_mut().unwrap().payees.insert("02zz".into(), 1);
        }),
        Code::Malformed
    );
    // Unknown fields in the section refuse.
    let mut json = serde_json::to_value(&grant).unwrap();
    json["auto"]["auto_approve_max"] = 1.into();
    assert!(serde_json::from_value::<Grant>(json).is_err());
}

#[test]
fn only_trusted_payees_and_purposes_within_their_caps_pay_automatically() {
    // The trusted payee may take 60 sats a day automatically.
    let grant = standing(60_000);
    let now = T0 + 10;
    let mut ledger = Ledger::default();
    let automatic = |ledger: &Ledger, request: &SpendRequest| {
        let admitted = ledger.check(Some(&grant), request, now).unwrap();
        ledger.automatic(&grant, request, &admitted, now)
    };
    // 25 sats plus a 1 sat fee ceiling to the trusted payee: automatic.
    let first = small(&grant, 1);
    assert!(automatic(&ledger, &first));
    ledger
        .reserve_in(Some(&grant), Earlier::default(), &first, now, true)
        .unwrap();
    assert!(ledger.entries[&first.request].auto);
    // An untrusted payee goes to the sheet; the grant still admits it.
    assert!(!automatic(&ledger, &from_node(&grant, 2, 9)));
    // A purpose with no automatic ceiling goes to the sheet.
    let mut labor = small(&grant, 3);
    labor.purpose = Purpose::LaborPayment;
    let mut narrow = grant.clone();
    narrow
        .auto
        .as_mut()
        .unwrap()
        .purposes
        .remove(&Purpose::LaborPayment);
    let admitted = ledger.check(Some(&narrow), &labor, now).unwrap();
    assert!(!ledger.automatic(&narrow, &labor, &admitted, now));
    // Above the automatic ceiling for one payment: the sheet.
    let big = request(&grant, 4, "lnbc10u", 1_000_000);
    assert!(!automatic(&ledger, &big));
    // A second 26 sats fits the payee's 60; a third would not.
    let second = small(&grant, 5);
    assert!(automatic(&ledger, &second));
    ledger
        .reserve_in(Some(&grant), Earlier::default(), &second, now, true)
        .unwrap();
    assert!(!automatic(&ledger, &small(&grant, 6)));
    // The owner's own approvals don't use the automatic ceilings...
    let mut roomy = standing(80_000);
    roomy.grant = grant.grant.clone();
    let tapped = from_node(&grant, 7, 9);
    ledger.reserve(Some(&grant), &tapped, now).unwrap();
    let admitted = ledger.check(Some(&roomy), &small(&roomy, 8), now).unwrap();
    assert!(ledger.automatic(&roomy, &small(&roomy, 8), &admitted, now));
    // ...and a day later the window has room again.
    let later = now + grant.period + 1;
    assert_eq!(
        ledger.automatic_remaining(&grant, later),
        Some(defaults::AUTO_PERIOD_MAX)
    );
    // A request-mode grant pays nothing without a tap.
    let plain = super::tests::grant(0);
    let asked = small(&plain, 9);
    let admitted = Ledger::default().check(Some(&plain), &asked, now).unwrap();
    assert!(!Ledger::default().automatic(&plain, &asked, &admitted, now));
    assert_eq!(Ledger::default().automatic_remaining(&plain, now), None);
    // A request already in the ledger is never paid automatically again.
    assert!(!automatic(&ledger, &first));
    let _ = payee_of([9; 32]);
}

#[test]
fn a_replaced_grant_keeps_its_requests_and_never_refills_a_window() {
    let old = grant(0);
    let now = T0 + 10;
    let mut ledger = Ledger::default();
    ledger.reserve(Some(&old), &small(&old, 1), now).unwrap();
    // The owner changes a setting: a new grant at the same epoch.
    let mut new = standing(defaults::AUTO_PAYEE_MAX);
    new.grant = id(0x31);
    let waiting = small(&old, 2);
    // Without the lineage the old grant's request is not the phone's.
    assert_eq!(
        ledger.check(Some(&new), &waiting, now).unwrap_err(),
        Refusal::Revoked
    );
    let ids = [old.grant.clone()];
    let earlier = Earlier {
        accepted: &ids,
        counted: &ids,
    };
    assert!(ledger.check_in(Some(&new), earlier, &waiting, now).is_ok());
    // The window counts the host's payments under any grant, and the total
    // counts the replaced grant's.
    let held = small(&old, 1).amount_msat + small(&old, 1).fee_max_msat;
    let remaining = ledger.remaining_in(&new, earlier, now);
    assert_eq!(remaining.period_msat, new.period_max - held);
    assert_eq!(remaining.total_msat, new.total_max - held);
    // A renewal accepts the waiting request but starts a new total.
    let renewal = Earlier {
        accepted: &ids,
        counted: &[],
    };
    let remaining = ledger.remaining_in(&new, renewal, now);
    assert_eq!(remaining.period_msat, new.period_max - held);
    assert_eq!(remaining.total_msat, new.total_max);
    // An older epoch's grant is never accepted.
    let mut stale = small(&old, 3);
    stale.epoch = 0;
    let mut next = new.clone();
    next.epoch = 1;
    assert_eq!(
        ledger
            .check_in(Some(&next), earlier, &stale, now)
            .unwrap_err(),
        Refusal::Stale
    );
}
