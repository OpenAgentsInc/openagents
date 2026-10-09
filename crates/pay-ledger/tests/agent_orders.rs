//! Agent orders on pooled compute (P4): the seller's fee first, the
//! provider's share of the rest, OpenAgents the remainder, one settlement
//! per order and per receipt, and forfeits that never take the seller's
//! fee.

use pay_ledger::{Error, Ledger, OPENAGENTS, Rail, SettlementInput, Split};

/// After v2 takes effect.
const AT: i64 = 1_792_022_400;
const PROVIDER: &str = "npub-provider";
const SELLER: &str = "npub-seller";

fn id(n: u32) -> String {
    format!("{n:064x}")
}

fn order(key: &str, order: &str, receipt: &str, price: i64, fee: i64) -> SettlementInput {
    SettlementInput {
        key: key.into(),
        resource: pay_ledger::agent_order::RESOURCE.into(),
        plugin_id: None,
        release_id: None,
        price_msat: price,
        received_msat: price,
        rail: Rail::Lightning,
        payer_alias: None,
        settled_at: AT,
        split: Split::AgentOrder {
            seller: SELLER.into(),
            fee_msat: fee,
            provider: PROVIDER.into(),
            receipt: receipt.into(),
            order: order.into(),
        },
    }
}

#[test]
fn an_order_pays_the_seller_first_then_the_provider_and_names_both_records() {
    let mut ledger = Ledger::in_memory().unwrap();
    ledger.install_pylon_rule().unwrap();
    let recorded = ledger
        .record_settlement(order(&id(1), &id(11), &id(101), 20_000, 12_000))
        .unwrap();
    let share = |party: &str, role: &str| {
        recorded
            .shares
            .iter()
            .find(|s| s.party == party && s.role == role)
            .map_or(0, |s| s.amount_msat)
    };
    assert_eq!(share(SELLER, "author"), 12_000);
    // 85 percent of the 8,000 that remains.
    assert_eq!(share(PROVIDER, "provider"), 6_800);
    assert_eq!(share(OPENAGENTS, "openagents"), 1_200);
    let row = ledger.agent_order(&id(11)).unwrap().unwrap();
    assert_eq!(row.receipt, id(101));
    assert_eq!(row.settlement, id(1));
    assert_eq!(
        (row.seller_msat, row.provider_msat, row.openagents_msat),
        (12_000, 6_800, 1_200)
    );
    // The receipt is a pylon job like any other.
    assert_eq!(
        ledger.pylon_job(&id(101)).unwrap().unwrap().provider_msat,
        6_800
    );
    assert_eq!(ledger.agent_orders().unwrap(), vec![row]);

    // A replay is the same row; a second payment for the order or for the
    // receipt conflicts.
    assert_eq!(
        ledger
            .record_settlement(order(&id(1), &id(11), &id(101), 20_000, 12_000))
            .unwrap(),
        recorded
    );
    assert!(matches!(
        ledger.record_settlement(order(&id(2), &id(11), &id(102), 20_000, 12_000)),
        Err(Error::Conflict(_))
    ));
    assert!(matches!(
        ledger.record_settlement(order(&id(3), &id(13), &id(101), 20_000, 12_000)),
        Err(Error::Conflict(_))
    ));
    // A fee over the price, a malformed order, and another resource refuse.
    assert!(
        ledger
            .record_settlement(order(&id(4), &id(14), &id(104), 1_000, 2_000))
            .is_err()
    );
    assert!(
        ledger
            .record_settlement(order(&id(5), "nope", &id(105), 1_000, 0))
            .is_err()
    );
    let mut other = order(&id(6), &id(16), &id(106), 1_000, 0);
    other.resource = pay_ledger::pylon::RESOURCE.into();
    assert!(ledger.record_settlement(other).is_err());
}

#[test]
fn a_failed_check_forfeits_the_provider_share_but_never_the_sellers_fee() {
    let mut ledger = Ledger::in_memory().unwrap();
    ledger.install_pylon_rule().unwrap();
    ledger
        .record_settlement(order(&id(1), &id(11), &id(101), 20_000, 12_000))
        .unwrap();
    let adjustment = ledger.forfeit_pylon_job(&id(101), &id(900), AT).unwrap();
    assert_eq!(adjustment.reduced_msat, 6_800);
    let row = ledger.agent_order(&id(11)).unwrap().unwrap();
    assert_eq!(row.seller_msat, 12_000);
    assert_eq!(
        ledger.pylon_job(&id(101)).unwrap().unwrap().forfeited_msat,
        6_800
    );
}
