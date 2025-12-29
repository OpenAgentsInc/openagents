//! End-to-end exchange demo: Two agents transacting
//!
//! This test demonstrates the full exchange flow:
//! 1. Treasury Agent posts a sell order (sell BTC for USD)
//! 2. Regular Agent finds and accepts the order
//! 3. Settlement executes
//! 4. Both agents publish attestations
//! 5. Reputation can be queried

use neobank::{
    ExchangeClient, OrderParams, OrderSide, OrderStatus, SettlementMethod, TradeOutcome,
    TradeStatus,
};
use std::time::Duration;

/// Simulates a Treasury Agent pubkey
const TREASURY_AGENT_PUBKEY: &str = "treasury_agent_npub_0123456789abcdef";
/// Simulates a Regular Agent pubkey
const REGULAR_AGENT_PUBKEY: &str = "regular_agent_npub_fedcba9876543210";

/// Full e2e demo: Treasury Agent sells BTC to Regular Agent
#[tokio::test]
async fn test_agent_exchange_demo() {
    println!("\n=== Agent Exchange Demo ===\n");

    // ----- Setup -----
    // In production, these would be separate processes with different keys.
    // For the demo, we simulate two agents with shared order book.
    let treasury = ExchangeClient::new_mock(TREASURY_AGENT_PUBKEY);
    let regular = ExchangeClient::new_mock(REGULAR_AGENT_PUBKEY);

    println!(
        "Treasury Agent: {}...",
        &treasury.pubkey()[..20.min(treasury.pubkey().len())]
    );
    println!(
        "Regular Agent:  {}...",
        &regular.pubkey()[..20.min(regular.pubkey().len())]
    );

    // ----- Step 1: Treasury Agent posts sell order -----
    println!("\n[1] Treasury Agent posts sell order: 10,000 sats for $1.00 USD");

    let order_id = treasury
        .post_order(OrderParams {
            side: OrderSide::Sell,
            amount_sats: 10_000,
            fiat_amount: 100, // $1.00 in cents
            currency: "USD".to_string(),
            premium_pct: 0.0,
            payment_methods: vec!["cashu".to_string()],
            expires_in: Duration::from_secs(3600),
        })
        .await
        .expect("Failed to post order");

    println!("    Order posted: {}", order_id);

    // Build NIP-69 tags (for display, would be published to relay)
    let nip69_tags = treasury.build_order_tags(&OrderParams {
        side: OrderSide::Sell,
        amount_sats: 10_000,
        fiat_amount: 100,
        currency: "USD".to_string(),
        premium_pct: 0.0,
        payment_methods: vec!["cashu".to_string()],
        expires_in: Duration::from_secs(3600),
    });
    println!("    NIP-69 tags: {:?}", nip69_tags.len());
    for tag in &nip69_tags[..5.min(nip69_tags.len())] {
        println!("      {:?}", tag);
    }
    println!("      ...");

    // ----- Step 2: Regular Agent fetches orders -----
    println!("\n[2] Regular Agent fetches available sell orders");

    // In production, this would fetch from relays.
    // For demo, copy order to regular agent's view (simulating relay sync).
    let order = treasury
        .get_order(&order_id)
        .expect("Failed to get order")
        .expect("Order not found");
    regular.inject_order(order).expect("Failed to inject order");

    let orders = regular
        .fetch_orders(Some(OrderSide::Sell))
        .await
        .expect("Failed to fetch orders");

    println!("    Found {} sell order(s)", orders.len());
    assert_eq!(orders.len(), 1);

    let order = &orders[0];
    println!(
        "    Order: {} sats @ {} {} (maker: {}...)",
        order.amount_sats,
        order.fiat_amount as f64 / 100.0,
        order.currency,
        &order.maker_pubkey[..20.min(order.maker_pubkey.len())]
    );

    // ----- Step 3: Regular Agent accepts the order -----
    println!("\n[3] Regular Agent accepts the order");

    let trade = regular
        .accept_order(&order_id)
        .await
        .expect("Failed to accept order");

    println!("    Trade created: {}", trade.trade_id);
    println!(
        "    Status: {:?} -> {:?}",
        OrderStatus::Pending,
        OrderStatus::InProgress
    );
    assert_eq!(trade.status, TradeStatus::Matched);
    assert_eq!(trade.taker_pubkey, REGULAR_AGENT_PUBKEY);

    // ----- Step 4: Settlement executes -----
    println!("\n[4] Settlement executing...");

    let receipt = regular
        .settle(&trade)
        .await
        .expect("Settlement failed");

    println!("    Settlement complete!");
    println!("    Method: {:?}", receipt.method);
    println!("    Amount: {} sats", receipt.btc_amount_sats);
    println!("    Duration: {:?}", receipt.duration);
    assert_eq!(receipt.method, SettlementMethod::Mock);
    assert_eq!(receipt.btc_amount_sats, 10_000);

    // ----- Step 5: Both agents publish attestations -----
    println!("\n[5] Publishing trade attestations (NIP-32 labels)");

    let settlement_ms = receipt.duration.as_millis() as u64;

    // Treasury attests to the trade
    let treasury_attest_id = treasury
        .attest_trade(&trade, TradeOutcome::Success, settlement_ms)
        .await
        .expect("Treasury attestation failed");

    println!("    Treasury attestation: {}", treasury_attest_id);

    // Build NIP-32 tags (for display)
    let nip32_tags = treasury.build_attestation_tags(&trade, TradeOutcome::Success, settlement_ms);
    println!("    NIP-32 tags:");
    for tag in &nip32_tags {
        println!("      {:?}", tag);
    }

    // Regular agent attests too
    // Need to copy trade to treasury's view first for attestation lookup
    let trade_copy = regular
        .get_trade(&order_id)
        .expect("Failed to get trade")
        .expect("Trade not found");
    treasury.inject_trade(trade_copy).expect("Failed to inject trade");

    let regular_attest_id = regular
        .attest_trade(&trade, TradeOutcome::Success, settlement_ms)
        .await
        .expect("Regular attestation failed");

    println!("    Regular attestation: {}", regular_attest_id);

    // ----- Step 6: Query reputation -----
    println!("\n[6] Querying reputation");

    // In production, attestations would be on relays and queryable by anyone
    // For demo, attestations are stored locally

    // Regular agent attested about treasury
    let treasury_rep = regular
        .calculate_reputation(TREASURY_AGENT_PUBKEY)
        .expect("Failed to calculate reputation");
    println!(
        "    Treasury Agent reputation (from Regular's view): {:.0}%",
        treasury_rep * 100.0
    );

    // Treasury agent attested about regular
    let regular_rep = treasury
        .calculate_reputation(REGULAR_AGENT_PUBKEY)
        .expect("Failed to calculate reputation");
    println!(
        "    Regular Agent reputation (from Treasury's view): {:.0}%",
        regular_rep * 100.0
    );

    println!("\n=== Demo Complete ===\n");
    println!("Summary:");
    println!("  - Treasury Agent sold 10,000 sats for $1.00 USD");
    println!("  - Regular Agent bought at market rate (0% premium)");
    println!("  - Settlement completed in {:?}", receipt.duration);
    println!("  - Both agents published positive attestations");
    println!("  - Reputation: 100% success rate for both parties");
}

/// Test multiple trades building reputation
#[tokio::test]
async fn test_reputation_accumulation() {
    let market_maker = ExchangeClient::new_mock("market_maker_pubkey");
    let trader = ExchangeClient::new_mock("trader_pubkey");

    // Execute 5 successful trades
    for i in 0..5 {
        let order_id = market_maker
            .post_order(OrderParams {
                side: OrderSide::Sell,
                amount_sats: 1000 * (i + 1),
                fiat_amount: 10 * (i + 1),
                ..Default::default()
            })
            .await
            .unwrap();

        // Sync order to trader (simulates relay)
        let order = market_maker.get_order(&order_id).unwrap().unwrap();
        trader.inject_order(order).unwrap();

        let trade = trader.accept_order(&order_id).await.unwrap();
        let receipt = trader.settle(&trade).await.unwrap();

        trader
            .attest_trade(&trade, TradeOutcome::Success, receipt.duration.as_millis() as u64)
            .await
            .unwrap();
    }

    // Check reputation - 5 successful trades
    let mm_rep = trader
        .calculate_reputation("market_maker_pubkey")
        .unwrap();
    assert_eq!(mm_rep, 1.0); // 100% success

    // Add one default
    let order_id = market_maker
        .post_order(OrderParams {
            side: OrderSide::Sell,
            amount_sats: 10000,
            fiat_amount: 100,
            ..Default::default()
        })
        .await
        .unwrap();

    // Sync order to trader (simulates relay)
    let order = market_maker.get_order(&order_id).unwrap().unwrap();
    trader.inject_order(order).unwrap();

    let trade = trader.accept_order(&order_id).await.unwrap();

    // Attest as default
    trader
        .attest_trade(&trade, TradeOutcome::Default, 0)
        .await
        .unwrap();

    // Now reputation is 5/6 = 83.3%
    let mm_rep = trader
        .calculate_reputation("market_maker_pubkey")
        .unwrap();
    assert!((mm_rep - 0.833).abs() < 0.01);
}

/// Test order cancellation
#[tokio::test]
async fn test_order_cancellation() {
    let maker = ExchangeClient::new_mock("maker_pubkey");

    let order_id = maker
        .post_order(OrderParams {
            side: OrderSide::Buy,
            amount_sats: 50_000,
            fiat_amount: 500,
            currency: "USD".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    // Order should be visible
    let orders = maker.fetch_orders(None).await.unwrap();
    assert_eq!(orders.len(), 1);

    // Cancel the order
    maker.cancel_order(&order_id).await.unwrap();

    // Order should no longer be fetched (not pending)
    let orders = maker.fetch_orders(None).await.unwrap();
    assert_eq!(orders.len(), 0);
}

/// Test filtering orders by side
#[tokio::test]
async fn test_order_filtering() {
    let exchange = ExchangeClient::new_mock("test_pubkey");

    // Post sell orders
    for _ in 0..3 {
        exchange
            .post_order(OrderParams {
                side: OrderSide::Sell,
                amount_sats: 10_000,
                fiat_amount: 100,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    // Post buy orders
    for _ in 0..2 {
        exchange
            .post_order(OrderParams {
                side: OrderSide::Buy,
                amount_sats: 20_000,
                fiat_amount: 200,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    // All orders
    let all = exchange.fetch_orders(None).await.unwrap();
    assert_eq!(all.len(), 5);

    // Only sells
    let sells = exchange.fetch_orders(Some(OrderSide::Sell)).await.unwrap();
    assert_eq!(sells.len(), 3);

    // Only buys
    let buys = exchange.fetch_orders(Some(OrderSide::Buy)).await.unwrap();
    assert_eq!(buys.len(), 2);
}
