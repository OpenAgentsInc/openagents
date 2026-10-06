//! No-spend offer comparison under one synthetic operator; no dispatch or payment API.
use pay_ledger::markets::bids::{Bid, Request, Selection, compare};
fn pin(c: char) -> String {
    c.to_string().repeat(64)
}
fn request() -> Request {
    Request {
        rfq: pin('1'),
        capability: pin('2'),
        source: pin('3'),
        disclosure: pin('4'),
        max_all_in_msat: 1000,
        expires_at: 100,
    }
}
fn bid(c: char, price: i64) -> Bid {
    Bid {
        quote: pin(c),
        rfq: pin('1'),
        provider: pin('5'),
        labor_terms: pin('6'),
        capability: pin('2'),
        source: pin('3'),
        disclosure: pin('4'),
        price_msat: price,
        fee_limit_msat: 0,
        coordination_cost_msat: 0,
        expires_at: 100,
        available_capacity: 1,
    }
}
#[test]
fn stale_changed_unavailable_and_over_budget_offers_are_ineligible() {
    let request = request();
    let mut a = bid('a', 1);
    a.expires_at = 10;
    let mut b = bid('b', 1);
    b.source = pin('7');
    let mut c = bid('c', 1);
    c.available_capacity = 0;
    let d = bid('d', 1001);
    let e = bid('e', 900);
    let offers = vec![a, b, c, d, e.clone()];
    let result = compare(&request, &offers, 10).unwrap();
    assert_eq!(result.suggested_quote, e.quote);
    assert_eq!(result.nonwinners.len(), 4);
    assert!(compare(&request, &offers[..4], 10).is_err());
}
#[test]
fn selection_is_deterministic_and_only_exact_winner_can_be_accepted() {
    let a = bid('a', 0);
    let b = bid('b', 0);
    assert_eq!(
        compare(&request(), &[b.clone(), a.clone()], 1)
            .unwrap()
            .suggested_quote,
        a.quote
    );
    let selected = Selection {
        bid_fingerprint: a.fingerprint().unwrap(),
        quote: a.quote.clone(),
        order: pin('8'),
        admission: pin('9'),
        payer: pin('0'),
        approved_price_msat: 0,
        approved_fee_limit_msat: 0,
    };
    selected.validate(&a, 1).unwrap();
    assert!(selected.validate(&b, 1).is_err());
    let mut changed = a.clone();
    changed.price_msat = 1;
    assert!(selected.validate(&changed, 1).is_err());
    assert!(selected.validate(&a, 100).is_err());
    assert!(compare(&request(), &[a.clone(), a], 1).is_err());
}
#[test]
fn comparison_accounts_for_incremental_coordination_cost() {
    let a = bid('a', 100);
    let mut b = bid('b', 1);
    b.coordination_cost_msat = 200;
    let result = compare(&request(), &[a.clone(), b], 1).unwrap();
    assert_eq!(result.suggested_quote, a.quote);
    assert_eq!(result.all_in_msat, 100);
}
