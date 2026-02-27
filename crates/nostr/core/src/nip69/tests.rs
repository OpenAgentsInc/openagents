use super::*;
use crate::Event;
use std::str::FromStr;

fn create_test_event(kind: u16, tags: Vec<Vec<String>>) -> Event {
    Event {
        id: "test_id".to_string(),
        pubkey: "test_maker_pubkey".to_string(),
        created_at: 1702548701,
        kind,
        tags,
        content: String::new(),
        sig: "test_sig".to_string(),
    }
}

#[test]
fn test_order_type() {
    assert_eq!(OrderType::Buy.as_str(), "buy");
    assert_eq!(OrderType::Sell.as_str(), "sell");
    assert!(matches!(OrderType::from_str("buy"), Ok(OrderType::Buy)));
    assert!(matches!(OrderType::from_str("sell"), Ok(OrderType::Sell)));
}

#[test]
fn test_order_status() {
    assert_eq!(OrderStatus::Pending.as_str(), "pending");
    assert_eq!(OrderStatus::Canceled.as_str(), "canceled");
    assert_eq!(OrderStatus::InProgress.as_str(), "in-progress");
    assert_eq!(OrderStatus::Success.as_str(), "success");
    assert_eq!(OrderStatus::Expired.as_str(), "expired");
}

#[test]
fn test_bitcoin_layer() {
    assert_eq!(BitcoinLayer::Onchain.as_str(), "onchain");
    assert_eq!(BitcoinLayer::Lightning.as_str(), "lightning");
    assert_eq!(BitcoinLayer::Liquid.as_str(), "liquid");
}

#[test]
fn test_p2p_order_from_event() {
    let tags = vec![
        vec!["d".to_string(), "order-123".to_string()],
        vec!["k".to_string(), "sell".to_string()],
        vec!["f".to_string(), "VES".to_string()],
        vec!["s".to_string(), "pending".to_string()],
        vec!["amt".to_string(), "10000".to_string()],
        vec!["fa".to_string(), "100".to_string()],
        vec![
            "pm".to_string(),
            "face to face".to_string(),
            "bank transfer".to_string(),
        ],
        vec!["premium".to_string(), "1.5".to_string()],
        vec!["network".to_string(), "mainnet".to_string()],
        vec!["layer".to_string(), "lightning".to_string()],
        vec!["expires_at".to_string(), "1719391096".to_string()],
        vec!["expiration".to_string(), "1719995896".to_string()],
        vec!["y".to_string(), "lnp2pbot".to_string()],
        vec!["z".to_string(), "order".to_string()],
    ];

    let event = create_test_event(P2P_ORDER_KIND, tags);
    let order = P2POrder::from_event(event).unwrap();

    assert_eq!(order.get_order_id(), "order-123");
    assert_eq!(order.get_order_type(), &OrderType::Sell);
    assert_eq!(order.get_currency(), "VES");
    assert_eq!(order.get_status(), &OrderStatus::Pending);
    assert_eq!(order.get_amount_sats(), 10000);
    assert_eq!(order.get_fiat_amount(), &[100]);
    assert_eq!(order.get_payment_methods().len(), 2);
    assert_eq!(order.get_premium(), 1.5);
    assert_eq!(order.get_network(), "mainnet");
    assert_eq!(order.get_layer(), &BitcoinLayer::Lightning);
    assert_eq!(order.get_platform(), "lnp2pbot");
    assert!(!order.is_range_order());
}

#[test]
fn test_p2p_order_range() {
    let tags = vec![
        vec!["d".to_string(), "order-456".to_string()],
        vec!["k".to_string(), "buy".to_string()],
        vec!["f".to_string(), "USD".to_string()],
        vec!["s".to_string(), "pending".to_string()],
        vec!["amt".to_string(), "0".to_string()],
        vec!["fa".to_string(), "100".to_string(), "500".to_string()],
        vec!["pm".to_string(), "bank transfer".to_string()],
        vec!["premium".to_string(), "2.0".to_string()],
        vec!["network".to_string(), "mainnet".to_string()],
        vec!["layer".to_string(), "onchain".to_string()],
        vec!["expires_at".to_string(), "1719391096".to_string()],
        vec!["expiration".to_string(), "1719995896".to_string()],
        vec!["y".to_string(), "mostro".to_string()],
        vec!["z".to_string(), "order".to_string()],
    ];

    let event = create_test_event(P2P_ORDER_KIND, tags);
    let order = P2POrder::from_event(event).unwrap();

    assert_eq!(order.get_order_type(), &OrderType::Buy);
    assert_eq!(order.get_fiat_amount(), &[100, 500]);
    assert!(order.is_range_order());
    assert_eq!(order.get_layer(), &BitcoinLayer::Onchain);
}

#[test]
fn test_p2p_order_missing_field() {
    let tags = vec![
        vec!["d".to_string(), "order-123".to_string()],
        vec!["k".to_string(), "sell".to_string()],
    ];

    let event = create_test_event(P2P_ORDER_KIND, tags);
    let result = P2POrder::from_event(event);
    assert!(result.is_err());
}

#[test]
fn test_p2p_order_invalid_kind() {
    let event = create_test_event(1, vec![]);
    let result = P2POrder::from_event(event);
    assert!(result.is_err());
}

#[test]
fn test_is_p2p_order_kind() {
    assert!(is_p2p_order_kind(P2P_ORDER_KIND));
    assert!(!is_p2p_order_kind(1));
}

#[test]
fn test_create_tags() {
    let order_id_tag = create_order_id_tag("order-123".to_string());
    assert_eq!(order_id_tag, vec!["d", "order-123"]);

    let type_tag = create_order_type_tag(OrderType::Sell);
    assert_eq!(type_tag, vec!["k", "sell"]);

    let currency_tag = create_currency_tag("USD".to_string());
    assert_eq!(currency_tag, vec!["f", "USD"]);

    let status_tag = create_status_tag(OrderStatus::Pending);
    assert_eq!(status_tag, vec!["s", "pending"]);

    let amount_tag = create_amount_tag(10000);
    assert_eq!(amount_tag, vec!["amt", "10000"]);

    let fiat_tag = create_fiat_amount_tag(vec![100, 500]);
    assert_eq!(fiat_tag, vec!["fa", "100", "500"]);

    let pm_tag = create_payment_method_tag(vec!["bank".to_string(), "cash".to_string()]);
    assert_eq!(pm_tag, vec!["pm", "bank", "cash"]);

    let layer_tag = create_layer_tag(BitcoinLayer::Lightning);
    assert_eq!(layer_tag, vec!["layer", "lightning"]);

    let platform_tag = create_platform_tag("mostro".to_string());
    assert_eq!(platform_tag, vec!["y", "mostro"]);

    let doc_tag = create_document_tag();
    assert_eq!(doc_tag, vec!["z", "order"]);
}

#[test]
fn test_rating_serialization() {
    let rating = Rating {
        total_reviews: 10,
        total_rating: 4.5,
        last_rating: 5,
        max_rate: 5,
        min_rate: 1,
    };

    let json = serde_json::to_string(&rating).unwrap();
    let deserialized: Rating = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.total_reviews, 10);
    assert_eq!(deserialized.total_rating, 4.5);
}

#[test]
fn test_p2p_order_builder() {
    let builder = P2POrderBuilder::new("order-789")
        .order_type(OrderType::Sell)
        .currency("USD")
        .amount_sats(10000)
        .fiat_amount(100)
        .payment_methods(vec!["cashu".to_string(), "lightning".to_string()])
        .premium(0.5)
        .network("mainnet")
        .layer(BitcoinLayer::Lightning)
        .name("Treasury Agent");

    let tags = builder.build_tags();

    // Verify key tags
    assert!(
        tags.iter()
            .any(|t| t.len() >= 2 && t[0] == "d" && t[1] == "order-789")
    );
    assert!(
        tags.iter()
            .any(|t| t.len() >= 2 && t[0] == "k" && t[1] == "sell")
    );
    assert!(
        tags.iter()
            .any(|t| t.len() >= 2 && t[0] == "f" && t[1] == "USD")
    );
    assert!(
        tags.iter()
            .any(|t| t.len() >= 2 && t[0] == "s" && t[1] == "pending")
    );
    assert!(
        tags.iter()
            .any(|t| t.len() >= 2 && t[0] == "amt" && t[1] == "10000")
    );
    assert!(
        tags.iter()
            .any(|t| t.len() >= 2 && t[0] == "fa" && t[1] == "100")
    );
    assert!(
        tags.iter()
            .any(|t| t.len() >= 3 && t[0] == "pm" && t[1] == "cashu" && t[2] == "lightning")
    );
    assert!(
        tags.iter()
            .any(|t| t.len() >= 2 && t[0] == "premium" && t[1] == "0.5")
    );
    assert!(
        tags.iter()
            .any(|t| t.len() >= 2 && t[0] == "name" && t[1] == "Treasury Agent")
    );
    assert!(
        tags.iter()
            .any(|t| t.len() >= 2 && t[0] == "y" && t[1] == "openagents")
    );
    assert!(
        tags.iter()
            .any(|t| t.len() >= 2 && t[0] == "z" && t[1] == "order")
    );
}

#[test]
fn test_p2p_order_builder_range() {
    let builder = P2POrderBuilder::new("order-range")
        .order_type(OrderType::Buy)
        .fiat_amount_range(50, 200);

    let tags = builder.build_tags();

    // Verify range fiat amount
    assert!(
        tags.iter()
            .any(|t| t.len() >= 3 && t[0] == "fa" && t[1] == "50" && t[2] == "200")
    );
}

#[test]
fn test_p2p_order_builder_roundtrip() {
    let builder = P2POrderBuilder::new("roundtrip-test")
        .order_type(OrderType::Sell)
        .currency("USD")
        .amount_sats(50000)
        .fiat_amount(500)
        .payment_methods(vec!["cashu".to_string()])
        .premium(1.0)
        .name("Test Maker");

    let tags = builder.build_tags();
    let event = create_test_event(P2P_ORDER_KIND, tags);
    let order = P2POrder::from_event(event).unwrap();

    assert_eq!(order.get_order_id(), "roundtrip-test");
    assert_eq!(order.get_order_type(), &OrderType::Sell);
    assert_eq!(order.get_currency(), "USD");
    assert_eq!(order.get_amount_sats(), 50000);
    assert_eq!(order.get_fiat_amount(), &[500]);
    assert_eq!(order.get_premium(), 1.0);
    assert_eq!(order.name, Some("Test Maker".to_string()));
}
