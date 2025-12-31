//! Relay pool integration tests

use super::*;
use nostr::{EventTemplate, KIND_SHORT_TEXT_NOTE, finalize_event, generate_secret_key};
use nostr_client::{PoolConfig, RelayPool};

#[tokio::test]
async fn test_pool_publishes_to_multiple_relays() {
    let port_a = 17100;
    let port_b = 17101;
    let (_server_a, _addr_a, _temp_a) = start_test_relay(port_a).await;
    let (_server_b, _addr_b, _temp_b) = start_test_relay(port_b).await;

    let config = PoolConfig {
        min_write_confirmations: 2,
        ..Default::default()
    };
    let pool = RelayPool::new(config);

    pool.add_relay(&test_relay_url(port_a)).await.unwrap();
    pool.add_relay(&test_relay_url(port_b)).await.unwrap();

    pool.connect_all().await.unwrap();

    let connected = pool.connected_relays().await;
    assert_eq!(connected.len(), 2);

    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![],
        content: "pool publish".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    let event = finalize_event(&template, &secret_key).unwrap();

    let confirmations = pool.publish(&event).await.unwrap();
    assert_eq!(confirmations.len(), 2);
    assert!(confirmations.iter().all(|c| c.accepted));
    assert!(confirmations.iter().all(|c| c.event_id == event.id));

    pool.disconnect_all().await.ok();
}
