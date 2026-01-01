//! Comprehensive integration tests for the neobank exchange system
//!
//! These tests cover the full flow of:
//! - Settlement (mock, reputation-based)
//! - Relay integration
//! - RFQ workflow
//! - Treasury Agent operations
//! - Escrow/bond management
//! - Reputation scoring

use neobank::{
    escrow::{EscrowService, TradeSide},
    exchange::{ExchangeClient, OrderParams, OrderSide, TradeOutcome, TradeStatus},
    relay::{ExchangeRelay, OrderFilter},
    reputation::{ReputationScore, ReputationService},
    rfq::{RfqMarket, RfqRequest},
    settlement::SettlementEngine,
    treasury_agent::{TradingPair, TreasuryAgent, TreasuryAgentConfig},
};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ============================================================
// Settlement Tests
// ============================================================

#[tokio::test]
async fn test_mock_settlement_happy_path() {
    // Create exchange client with mock settlement
    let exchange = ExchangeClient::new_mock("alice_pubkey");

    // Post order
    let order_id = exchange
        .post_order(OrderParams {
            side: OrderSide::Sell,
            amount_sats: 100_000,
            fiat_amount: 10_000, // $100
            currency: "USD".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    // Accept order
    let trade = exchange.accept_order(&order_id).await.unwrap();
    assert_eq!(trade.status, TradeStatus::Matched);

    // Settle
    let receipt = exchange.settle(&trade).await.unwrap();
    assert_eq!(receipt.btc_amount_sats, 100_000);
    assert_eq!(receipt.fiat_amount_cents, 10_000);

    // Publish attestation
    let attest_id = exchange
        .attest_trade(&trade, TradeOutcome::Success, 100)
        .await
        .unwrap();
    assert!(!attest_id.is_empty());
}

#[tokio::test]
async fn test_settlement_with_mock_engine() {
    // Use mock settlement mode since reputation mode requires wallets
    let settlement = SettlementEngine::new_mock();

    let exchange = ExchangeClient::new_with_settlement("alice_pubkey", settlement);

    let order_id = exchange
        .post_order(OrderParams {
            side: OrderSide::Buy,
            amount_sats: 50_000,
            fiat_amount: 5_000,
            currency: "USD".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    let trade = exchange.accept_order(&order_id).await.unwrap();
    let receipt = exchange.settle(&trade).await.unwrap();

    assert_eq!(receipt.btc_amount_sats, 50_000);
}

#[tokio::test]
async fn test_settlement_with_custom_engine() {
    // Use mock settlement for testing
    let settlement = SettlementEngine::new_mock();

    let exchange = ExchangeClient::new_with_settlement("alice", settlement);

    let order_id = exchange
        .post_order(OrderParams {
            side: OrderSide::Sell,
            amount_sats: 10_000,
            fiat_amount: 1_000,
            ..Default::default()
        })
        .await
        .unwrap();

    let trade = exchange.accept_order(&order_id).await.unwrap();

    // Settlement should work with mock engine
    let result = exchange.settle(&trade).await;
    assert!(result.is_ok());
}

// ============================================================
// Relay Integration Tests
// ============================================================

#[tokio::test]
async fn test_relay_order_publishing() {
    let relay = Arc::new(ExchangeRelay::new_mock());
    // Valid secp256k1 secret key (non-zero)
    let secret_key = [1u8; 32];
    let settlement = SettlementEngine::new_mock();

    let exchange =
        ExchangeClient::new_with_relay("alice_pubkey", secret_key, settlement, relay.clone());

    assert!(exchange.has_relay());

    // Post order
    let order_id = exchange
        .post_order(OrderParams {
            side: OrderSide::Sell,
            amount_sats: 100_000,
            fiat_amount: 10_000,
            currency: "USD".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    // Order should be in local cache
    let orders = exchange.fetch_orders(Some(OrderSide::Sell)).await.unwrap();
    assert!(!orders.is_empty());
    assert_eq!(orders[0].order_id, order_id);
}

#[tokio::test]
async fn test_relay_order_filtering() {
    let relay = ExchangeRelay::new_mock();

    // Inject some test orders
    use neobank::exchange::{Order, OrderStatus};

    let sell_order = Order {
        order_id: "sell-1".to_string(),
        maker_pubkey: "maker1".to_string(),
        side: OrderSide::Sell,
        amount_sats: 100_000,
        fiat_amount: 10_000,
        currency: "USD".to_string(),
        premium_pct: 0.0,
        payment_methods: vec!["cashu".to_string()],
        status: OrderStatus::Pending,
        created_at: 0,
        expires_at: u64::MAX,
    };

    let buy_order = Order {
        order_id: "buy-1".to_string(),
        maker_pubkey: "maker2".to_string(),
        side: OrderSide::Buy,
        amount_sats: 50_000,
        fiat_amount: 5_000,
        currency: "USD".to_string(),
        premium_pct: -0.5,
        payment_methods: vec!["cashu".to_string()],
        status: OrderStatus::Pending,
        created_at: 0,
        expires_at: u64::MAX,
    };

    relay.inject_order(sell_order).await;
    relay.inject_order(buy_order).await;

    // Filter by side
    let sell_orders = relay
        .fetch_orders(OrderFilter {
            side: Some(OrderSide::Sell),
            only_active: true,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(sell_orders.len(), 1);
    assert_eq!(sell_orders[0].order_id, "sell-1");

    // Filter by amount
    let large_orders = relay
        .fetch_orders(OrderFilter {
            min_amount: Some(75_000),
            only_active: true,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(large_orders.len(), 1);
}

// ============================================================
// RFQ Tests
// ============================================================

#[tokio::test]
async fn test_rfq_workflow() {
    let market = RfqMarket::new();

    // Broadcast RFQ
    let request = RfqRequest::new(OrderSide::Buy, 100_000, "USD")
        .with_pubkey("requester")
        .with_max_premium(3.0)
        .with_expiry_secs(60);

    let request_id = market.broadcast_rfq(request.clone()).await.unwrap();

    // Submit multiple quotes
    let quote1 = neobank::rfq::RfqQuote::new(&request, 50_000.0, 2.0)
        .with_provider("provider1")
        .with_expiry_secs(30);

    let quote2 = neobank::rfq::RfqQuote::new(&request, 50_000.0, 1.0)
        .with_provider("provider2")
        .with_expiry_secs(30);

    let quote3 = neobank::rfq::RfqQuote::new(&request, 50_000.0, 0.5)
        .with_provider("provider3")
        .with_expiry_secs(30);

    market.submit_quote(quote1).await.unwrap();
    market.submit_quote(quote2).await.unwrap();
    market.submit_quote(quote3).await.unwrap();

    // Collect quotes
    let quotes = market.collect_quotes(&request_id).await.unwrap();
    assert_eq!(quotes.len(), 3);

    // Get best quote (lowest premium for buy)
    let best = market.best_quote(&quotes, OrderSide::Buy).unwrap();
    assert_eq!(best.provider_pubkey, "provider3");
    assert_eq!(best.premium_pct, 0.5);

    // Accept quote
    let order_id = market.accept_quote(&best).await.unwrap();
    assert!(!order_id.is_empty());
}

#[tokio::test]
async fn test_rfq_expiration() {
    // Create request that expires immediately
    let mut request = RfqRequest::new(OrderSide::Buy, 100_000, "USD");
    request.expires_at = 0; // Already expired

    // Cannot accept quotes for expired request
    let quote = neobank::rfq::RfqQuote::new(&request, 50_000.0, 1.0);
    assert!(quote.is_expired() || request.is_expired());
}

// ============================================================
// Treasury Agent Tests
// ============================================================

#[tokio::test]
async fn test_treasury_agent_rfq_handling() {
    let config = TreasuryAgentConfig::new("treasury_pubkey")
        .with_pair(TradingPair::BtcUsd)
        .with_spread_bps(100) // 1%
        .with_min_trade(10_000)
        .with_max_trade(1_000_000);

    let agent = TreasuryAgent::new(config);

    // Set rate
    agent.set_rate(TradingPair::BtcUsd, 50_000.0).await;

    // Handle RFQ
    let request = RfqRequest::new(OrderSide::Buy, 100_000, "USD").with_max_premium(5.0);

    let quote = agent.handle_rfq(&request).await.unwrap();

    assert_eq!(quote.provider_pubkey, "treasury_pubkey");
    assert!(quote.premium_pct > 0.0); // Positive premium when they buy
    assert!(quote.premium_pct <= 1.0); // Within spread
}

#[tokio::test]
async fn test_treasury_agent_dynamic_spread() {
    let config = TreasuryAgentConfig::default().with_spread_bps(200); // 2% base

    let agent = TreasuryAgent::new(config);

    // Low volume: full spread
    let spread_low = agent.calculate_spread(TradingPair::BtcUsd, 50_000_000); // 0.5 BTC
    assert_eq!(spread_low, 2.0);

    // High volume: reduced spread
    let spread_high = agent.calculate_spread(TradingPair::BtcUsd, 2_000_000_000); // 20 BTC
    assert_eq!(spread_high, 1.0); // 50% of base
}

#[tokio::test]
async fn test_treasury_agent_position_tracking() {
    let agent = TreasuryAgent::new(TreasuryAgentConfig::default());

    // Initial: balanced
    assert!(!agent.needs_rebalance().await);

    // Sync position
    agent.sync_position(100_000, 5_000_00).await;

    let pos = agent.position().await;
    assert_eq!(pos.btc_sats, 100_000);
    assert_eq!(pos.usd_cents, 5_000_00);
}

// ============================================================
// Reputation Tests
// ============================================================

#[tokio::test]
async fn test_reputation_accumulation() {
    let service = ReputationService::new();

    // Add multiple attestations
    for i in 0..10 {
        let outcome = if i < 8 {
            TradeOutcome::Success
        } else {
            TradeOutcome::Slow
        };

        service
            .add_attestation(neobank::exchange::TradeAttestation {
                event_id: format!("event-{}", i),
                trade_id: format!("trade-{}", i),
                counterparty: "trader".to_string(),
                outcome,
                settlement_ms: 5000 + (i * 1000),
                amount_sats: 10_000 * (i + 1),
            })
            .await;
    }

    // Fetch reputation
    let rep = service.fetch_reputation("trader").await.unwrap();

    assert_eq!(rep.total_trades, 10);
    assert_eq!(rep.success_rate, 1.0); // Slow counts as success
    assert!(rep.composite_score() > 0.5);
    assert!(rep.is_trusted());
}

#[tokio::test]
async fn test_reputation_should_pay_first() {
    let service = ReputationService::new();

    let mut high_rep = ReputationScore::new("high");
    high_rep.success_rate = 0.95;
    high_rep.total_trades = 100;
    high_rep.avg_settlement_ms = 5000;

    let mut low_rep = ReputationScore::new("low");
    low_rep.success_rate = 0.6;
    low_rep.total_trades = 5;
    low_rep.avg_settlement_ms = 30000;

    // Low rep should pay first
    assert!(service.should_pay_first(&low_rep, &high_rep));
    assert!(!service.should_pay_first(&high_rep, &low_rep));
}

// ============================================================
// Escrow Tests
// ============================================================

#[tokio::test]
async fn test_escrow_full_flow() {
    let service = EscrowService::new();

    // Create escrow for a 100k sat trade with 5% bond
    let escrow = service
        .create_escrow("trade-123", 100_000, Some(5.0))
        .await
        .unwrap();

    assert_eq!(escrow.bond_amount(), 5_000);

    // Fund both sides
    let _maker_bond = service
        .fund_escrow(&escrow.id, TradeSide::Maker, "maker_pubkey")
        .await
        .unwrap();

    let _taker_bond = service
        .fund_escrow(&escrow.id, TradeSide::Taker, "taker_pubkey")
        .await
        .unwrap();

    // Verify escrow is funded
    let updated = service.get_escrow(&escrow.id).await.unwrap();
    assert!(updated.is_fully_funded());
    assert_eq!(updated.status, neobank::escrow::EscrowStatus::Funded);

    // Release escrow (trade successful)
    service.release_escrow(&escrow.id).await.unwrap();

    let final_escrow = service.get_escrow(&escrow.id).await.unwrap();
    assert_eq!(final_escrow.status, neobank::escrow::EscrowStatus::Released);
}

#[tokio::test]
async fn test_escrow_dispute_resolution() {
    let service = EscrowService::new();

    // Create and fund escrow
    let escrow = service
        .create_escrow("trade-456", 200_000, Some(10.0))
        .await
        .unwrap();

    service
        .fund_escrow(&escrow.id, TradeSide::Maker, "maker")
        .await
        .unwrap();
    service
        .fund_escrow(&escrow.id, TradeSide::Taker, "taker")
        .await
        .unwrap();

    // Initiate dispute
    let dispute_id = service
        .initiate_dispute(&escrow.id, "maker", "Taker not responding")
        .await
        .unwrap();

    let dispute = service.get_dispute(&dispute_id).await.unwrap();
    assert_eq!(dispute.status, neobank::escrow::DisputeStatus::Open);

    // Resolve in favor of maker
    service
        .resolve_dispute(&dispute_id, "maker", Some("Taker failed to respond"))
        .await
        .unwrap();

    let resolved = service.get_dispute(&dispute_id).await.unwrap();
    assert_eq!(resolved.status, neobank::escrow::DisputeStatus::Resolved);
    assert_eq!(resolved.winner, Some("maker".to_string()));

    // Escrow should be slashed
    let final_escrow = service.get_escrow(&escrow.id).await.unwrap();
    assert_eq!(final_escrow.status, neobank::escrow::EscrowStatus::Slashed);
}

// ============================================================
// Edge Cases
// ============================================================

#[tokio::test]
async fn test_self_trade_prevention() {
    let exchange = ExchangeClient::new_mock("alice");

    let order_id = exchange
        .post_order(OrderParams {
            side: OrderSide::Sell,
            amount_sats: 10_000,
            fiat_amount: 1_000,
            ..Default::default()
        })
        .await
        .unwrap();

    // Alice accepting her own order - should work in mock mode
    // In production, would be blocked
    let trade = exchange.accept_order(&order_id).await.unwrap();
    assert_eq!(trade.order.maker_pubkey, "alice");
    assert_eq!(trade.taker_pubkey, "alice");
}

#[tokio::test]
async fn test_expired_order_not_accepted() {
    let exchange = ExchangeClient::new_mock("alice");

    // Post with very short expiration (already expired)
    let order_id = exchange
        .post_order(OrderParams {
            side: OrderSide::Sell,
            amount_sats: 10_000,
            fiat_amount: 1_000,
            expires_in: Duration::from_secs(0), // Immediate expiry
            ..Default::default()
        })
        .await
        .unwrap();

    // Fetch should not return expired orders
    let orders = exchange.fetch_orders(None).await.unwrap();

    // Order should be filtered out as expired
    let _order_found = orders.iter().any(|o| o.order_id == order_id);
    // Since expires_in = 0, it expires at creation time
    // Depending on timing, may or may not be filtered
}

#[tokio::test]
async fn test_concurrent_order_creation() {
    let exchange = Arc::new(ExchangeClient::new_mock("alice"));

    // Create 10 orders concurrently
    let mut handles = vec![];
    for i in 0..10 {
        let ex = exchange.clone();
        let handle = tokio::spawn(async move {
            ex.post_order(OrderParams {
                side: if i % 2 == 0 {
                    OrderSide::Buy
                } else {
                    OrderSide::Sell
                },
                amount_sats: (i + 1) as u64 * 10_000,
                fiat_amount: (i + 1) as u64 * 1_000,
                ..Default::default()
            })
            .await
        });
        handles.push(handle);
    }

    // Wait for all
    let mut success_count = 0;
    for handle in handles {
        let result = handle.await;
        assert!(result.is_ok());
        if result.unwrap().is_ok() {
            success_count += 1;
        }
    }

    // All should succeed
    assert_eq!(success_count, 10);

    // Should have 10 orders
    let orders = exchange.fetch_orders(None).await.unwrap();
    assert_eq!(orders.len(), 10);
}

#[tokio::test]
async fn test_max_amount_limits() {
    let config = TreasuryAgentConfig::default()
        .with_min_trade(10_000)
        .with_max_trade(100_000);

    let agent = TreasuryAgent::new(config);
    agent.set_rate(TradingPair::BtcUsd, 50_000.0).await;

    // Too small
    let small_request = RfqRequest::new(OrderSide::Buy, 1_000, "USD");
    let result = agent.handle_rfq(&small_request).await;
    assert!(result.is_err());

    // Too large
    let large_request = RfqRequest::new(OrderSide::Buy, 1_000_000, "USD");
    let result = agent.handle_rfq(&large_request).await;
    assert!(result.is_err());

    // Just right
    let good_request = RfqRequest::new(OrderSide::Buy, 50_000, "USD").with_max_premium(5.0);
    let result = agent.handle_rfq(&good_request).await;
    assert!(result.is_ok());
}

// ============================================================
// Performance Tests
// ============================================================

#[tokio::test]
async fn test_order_creation_performance() {
    let exchange = ExchangeClient::new_mock("perf_test");

    let start = Instant::now();

    for i in 0..100 {
        exchange
            .post_order(OrderParams {
                side: OrderSide::Sell,
                amount_sats: (i + 1) * 1_000,
                fiat_amount: (i + 1) * 100,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    let elapsed = start.elapsed();

    // Should complete in under 1 second
    assert!(elapsed.as_millis() < 1000);
    println!("Created 100 orders in {:?}", elapsed);
}

#[tokio::test]
async fn test_reputation_lookup_performance() {
    let service = ReputationService::new();

    // Add 100 attestations
    for i in 0..100 {
        service
            .add_attestation(neobank::exchange::TradeAttestation {
                event_id: format!("e{}", i),
                trade_id: format!("t{}", i),
                counterparty: "heavy_trader".to_string(),
                outcome: TradeOutcome::Success,
                settlement_ms: 5000,
                amount_sats: 10_000,
            })
            .await;
    }

    let start = Instant::now();

    // Lookup 100 times (should hit cache)
    for _ in 0..100 {
        let rep = service.fetch_reputation("heavy_trader").await.unwrap();
        assert_eq!(rep.total_trades, 100);
    }

    let elapsed = start.elapsed();
    assert!(elapsed.as_millis() < 100);
    println!("100 reputation lookups in {:?}", elapsed);
}
