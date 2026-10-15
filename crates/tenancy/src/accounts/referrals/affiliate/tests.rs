use super::*;

const AFF: &str = "ref_affiliate";
const OWNER: &str = "acct_affiliate";
const BUYER: &str = "acct_buyer";

fn directory() -> Directory {
    let mut d = Directory::default();
    d.affiliates.insert(
        AFF.into(),
        Affiliate {
            owner: OWNER.into(),
            label: "Creator".into(),
            keys: vec!["aff-0123456789abcdef".into()],
        },
    );
    d.workspaces
        .insert(OWNER.into(), ["ws_affiliate".to_string()].into());
    d.workspaces
        .insert(BUYER.into(), ["ws_buyer".to_string()].into());
    d
}

/// A $20.00 month with no tax and a $1.00 recorded fee: net $19.00.
fn paid(invoice: &str, at: u64) -> Sale {
    Sale {
        kind: SaleKind::Paid,
        link: LinkKind::Affiliate,
        referrer: Some(AFF.into()),
        account: BUYER.into(),
        workspace: Some("ws_buyer".into()),
        invoice: invoice.into(),
        at,
        amount_cents: 2000,
        currency: Some("USD".into()),
        tax_cents: Some(0),
        fee_cents: Some(100),
    }
}

fn reversed(invoice: &str, at: u64) -> Sale {
    Sale {
        kind: SaleKind::Reversed,
        at,
        ..paid(invoice, 0)
    }
}

const T0: u64 = 1_800_000_000;
const HOLD: u64 = 30 * DAY;

#[test]
fn the_rate_is_20_percent_of_net_after_tax_and_fees() {
    // $19.00 net -> $3.80.
    let l = ledger(&PROGRAM, &[paid("in_1", T0)], &directory(), &[], T0 + HOLD);
    assert_eq!(l.items[0].net_cents, 1900);
    assert_eq!(l.items[0].commission_micros, 3_800_000);
    assert_eq!(l.balances[AFF].payable, 3_800_000);
    // Tax is left out; with no recorded fee, the card fee comes off.
    let mut sale = paid("in_2", T0);
    sale.amount_cents = 2200;
    sale.tax_cents = Some(200);
    sale.fee_cents = None;
    let l = ledger(&PROGRAM, &[sale], &directory(), &[], T0);
    // $20.00 - (2.9% of $22.00 rounded up = 64c + 30c) = $19.06.
    assert_eq!(l.items[0].net_cents, 2000 - 94);
    assert_eq!(l.items[0].commission_micros, 1906 * 10_000 / 5);
    assert_eq!(PROGRAM.rate_percent(), 20);
}

#[test]
fn only_the_first_12_paid_months_earn() {
    let sales: Vec<Sale> = (0..14)
        .map(|m| paid(&format!("in_{m:02}"), T0 + m * 30 * DAY))
        .collect();
    let now = T0 + 20 * 30 * DAY;
    let l = ledger(&PROGRAM, &sales, &directory(), &[], now);
    let earning = l.items.iter().filter(|i| i.excluded.is_none()).count();
    assert_eq!(earning, 12);
    assert_eq!(l.items[12].month, 13);
    assert_eq!(l.items[12].excluded, Some(Excluded::PastTerm));
    assert_eq!(l.items[12].commission_micros, 0);
    assert_eq!(l.balances[AFF].paid_conversions, 12);
    assert_eq!(l.balances[AFF].payable, 12 * 3_800_000);
}

#[test]
fn commission_is_held_for_30_days_after_the_payment() {
    let sales = [paid("in_1", T0)];
    let held = ledger(&PROGRAM, &sales, &directory(), &[], T0 + HOLD - 1);
    assert_eq!(held.items[0].state, State::Held);
    assert_eq!(held.balances[AFF].held, 3_800_000);
    assert_eq!(held.balances[AFF].payable, 0);
    assert_eq!(held.balances[AFF].accrued, 3_800_000);
    let free = ledger(&PROGRAM, &sales, &directory(), &[], T0 + HOLD);
    assert_eq!(free.items[0].state, State::Payable);
    assert_eq!(free.balances[AFF].held, 0);
    assert_eq!(free.balances[AFF].payable, 3_800_000);
}

#[test]
fn a_refund_inside_the_hold_reverses_and_a_later_one_is_clawed_back() {
    // Inside the hold: the month never earns.
    let sales = [paid("in_1", T0), reversed("in_1", T0 + DAY)];
    let l = ledger(&PROGRAM, &sales, &directory(), &[], T0 + 2 * HOLD);
    assert_eq!(l.items[0].state, State::Reversed);
    let b = l.balances[AFF];
    assert_eq!((b.accrued, b.held, b.payable, b.clawed_back), (0, 0, 0, 0));
    assert_eq!(b.refunds, 1);
    // After the hold: taken back from the other month's commission.
    let sales = [
        paid("in_1", T0),
        paid("in_2", T0 + 30 * DAY),
        reversed("in_1", T0 + HOLD + DAY),
    ];
    let l = ledger(&PROGRAM, &sales, &directory(), &[], T0 + 30 * DAY + HOLD);
    assert_eq!(l.items[0].state, State::ClawedBack);
    let b = l.balances[AFF];
    assert_eq!(b.clawed_back, 3_800_000);
    assert_eq!(b.payable, 3_800_000);
    assert_eq!(b.owed, 0);
    // With nothing else earned, the clawback is owed against the future.
    let sales = [paid("in_1", T0), reversed("in_1", T0 + HOLD + DAY)];
    let l = ledger(&PROGRAM, &sales, &directory(), &[], T0 + 2 * HOLD);
    let b = l.balances[AFF];
    assert_eq!((b.payable, b.owed), (0, 0));
    // ...and once it was already paid out, the clawback is owed.
    let payout = Payout {
        referrer: AFF.into(),
        usd_micros: 3_800_000,
        sats: 3_800,
        at: T0 + HOLD,
    };
    let l = ledger(&PROGRAM, &sales, &directory(), &[payout], T0 + 2 * HOLD);
    let b = l.balances[AFF];
    assert_eq!((b.payable, b.paid, b.owed), (0, 3_800_000, 3_800_000));
}

#[test]
fn self_referral_earns_nothing() {
    let d = directory();
    // The affiliate's own account.
    let mut own = paid("in_1", T0);
    own.account = OWNER.into();
    own.workspace = Some("ws_affiliate".into());
    // An account sharing a workspace with the affiliate.
    let mut linked = paid("in_2", T0);
    linked.account = "acct_teammate".into();
    linked.workspace = Some("ws_team".into());
    let mut d2 = d.clone();
    d2.workspaces
        .get_mut(OWNER)
        .unwrap()
        .insert("ws_team".into());
    d2.workspaces
        .insert("acct_teammate".into(), ["ws_team".to_string()].into());
    // A workspace the affiliate is a member of, paid by someone else.
    let mut theirs = paid("in_3", T0);
    theirs.account = "acct_stranger".into();
    theirs.workspace = Some("ws_affiliate".into());
    let l = ledger(&PROGRAM, &[own, linked, theirs], &d2, &[], T0 + HOLD);
    assert!(
        l.items
            .iter()
            .all(|i| i.excluded == Some(Excluded::SelfReferral) && i.commission_micros == 0)
    );
    assert!(l.balances.is_empty());
    // An unknown affiliate is paid nothing either.
    let mut ghost = paid("in_4", T0);
    ghost.referrer = Some("ref_gone".into());
    let l = ledger(&PROGRAM, &[ghost], &d, &[], T0 + HOLD);
    assert_eq!(l.items[0].excluded, Some(Excluded::SelfReferral));
}

#[test]
fn go_campaign_codes_earn_nothing() {
    let mut go = paid("in_1", T0);
    go.link = LinkKind::Go;
    go.referrer = None;
    let l = ledger(&PROGRAM, &[go], &directory(), &[], T0 + HOLD);
    assert_eq!(l.items[0].excluded, Some(Excluded::Campaign));
    assert_eq!(l.items[0].commission_micros, 0);
    assert!(l.balances.is_empty());
}

#[test]
fn payouts_are_due_only_from_10000_sats() {
    // $3.80 payable at $40,000 per bitcoin is 9,500 sats: below the minimum.
    let one = ledger(&PROGRAM, &[paid("in_1", T0)], &directory(), &[], T0 + HOLD);
    assert_eq!(sats(3_800_000, 40_000), Some(9_500));
    assert!(payouts_due(&PROGRAM, &one, 40_000).is_empty());
    // At $38,000 it is exactly 10,000 sats: due.
    let due = payouts_due(&PROGRAM, &one, 38_000);
    assert_eq!(
        due,
        vec![Due {
            referrer: AFF.into(),
            usd_micros: 3_800_000,
            sats: 10_000
        }]
    );
    // Held commission never counts toward it.
    let held = ledger(&PROGRAM, &[paid("in_1", T0)], &directory(), &[], T0);
    assert!(payouts_due(&PROGRAM, &held, 1).is_empty());
    assert!(payouts_due(&PROGRAM, &one, 0).is_empty());
}

#[test]
fn a_resent_payment_counts_once_and_other_currencies_earn_nothing() {
    let l = ledger(
        &PROGRAM,
        &[paid("in_1", T0), paid("in_1", T0 + 5)],
        &directory(),
        &[],
        T0 + HOLD,
    );
    assert_eq!(l.items.len(), 1);
    let mut eur = paid("in_2", T0);
    eur.currency = Some("EUR".into());
    let l = ledger(&PROGRAM, &[eur], &directory(), &[], T0 + HOLD);
    assert_eq!(l.items[0].excluded, Some(Excluded::Currency));
}

#[test]
fn the_terms_publish_as_the_attribution_policy_and_name_the_numbers() {
    let p = policy().unwrap();
    assert_eq!(p.version, TERMS_VERSION);
    assert_eq!(p.terms, terms());
    assert!(terms().len() <= 4096);
    let t = terms();
    for needle in [
        "20%",
        "first 12 paid months",
        "30 days",
        "10,000 sats",
        "Lightning",
        "/go/",
        "#ad",
        "AI persona",
        "30 days' notice",
    ] {
        assert!(t.contains(needle), "terms name {needle}");
    }
    assert_eq!(PROGRAM.paid_months, 12);
    assert_eq!(PROGRAM.hold_days(), 30);
    assert_eq!(PROGRAM.minimum_sats, 10_000);
    assert_eq!(PROGRAM.notice_days, 30);
}

#[test]
fn the_directory_reads_affiliates_their_keys_and_workspaces() {
    let dir = tempfile::tempdir().unwrap();
    let accounts = crate::Accounts::install(dir.path()).unwrap();
    let owner = accounts.create_account("Owner", &[]).unwrap();
    let referrer = accounts
        .create_referrer(&owner.id, Kind::Person, "Creator")
        .unwrap();
    let issued = accounts
        .issue_referral_link(&owner.id, &referrer.id)
        .unwrap();
    let d = Directory::of(&accounts.store().unwrap());
    let a = &d.affiliates[&referrer.id];
    assert_eq!(a.owner, owner.id);
    let short = &hash(issued.token.as_bytes())[..16];
    assert_eq!(a.keys, vec![format!("aff-{short}")]);
    assert_eq!(
        d.owned_by(&owner.id).collect::<Vec<_>>(),
        vec![&referrer.id]
    );
    assert!(d.self_referral(&referrer.id, &owner.id, None));
    assert!(!d.self_referral(&referrer.id, "acct_someone_else", None));
}
