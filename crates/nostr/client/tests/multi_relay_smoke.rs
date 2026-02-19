//! Multi-relay smoke test for DM + NIP-28 channel message flow.
//!
//! Run manually:
//! cargo test -p nostr-client --test multi_relay_smoke -- --ignored --nocapture

use nostr::{
    ChannelMessageEvent, ChannelMetadata, Event, EventTemplate, KIND_CHANNEL_CREATION,
    KIND_CHANNEL_MESSAGE, decrypt_v2, derive_keypair, encrypt_v2, finalize_event,
};
use nostr_client::{PoolConfig, RelayPool};
use rand::random;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::timeout;

const RELAYS: [&str; 2] = ["wss://relay.damus.io", "wss://nos.lol"];

const PROVIDER_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const CUSTOMER_MNEMONIC: &str = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn xonly_to_compressed(xonly: &[u8; 32]) -> [u8; 33] {
    let mut compressed = [0u8; 33];
    compressed[0] = 0x02;
    compressed[1..].copy_from_slice(xonly);
    compressed
}

#[tokio::test]
#[ignore]
async fn multi_relay_dm_and_channel_smoke() -> Result<(), Box<dyn std::error::Error + Send + Sync>>
{
    let mut config = PoolConfig::default();
    config.min_write_confirmations = 1;
    let pool = RelayPool::new(config);

    for relay in RELAYS {
        pool.add_relay(relay).await?;
    }
    pool.connect_all().await?;

    let stats = pool.pool_stats().await;
    assert!(
        stats.connected_relays >= 2,
        "expected 2 relays connected, got {}",
        stats.connected_relays
    );

    let sender = derive_keypair(PROVIDER_MNEMONIC)?;
    let recipient = derive_keypair(CUSTOMER_MNEMONIC)?;
    let recipient_pubkey_hex = hex::encode(recipient.public_key);

    let dm_sub_id = format!("dm-smoke-{}", random::<u64>());
    let dm_filters = vec![serde_json::json!({
        "kinds": [4_u64],
        "#p": [recipient_pubkey_hex],
        "limit": 5
    })];
    let mut dm_rx = pool.subscribe(&dm_sub_id, &dm_filters).await?;

    let dm_plaintext = "multi-relay dm smoke";
    let recipient_pubkey = xonly_to_compressed(&recipient.public_key);
    let dm_ciphertext = encrypt_v2(&sender.private_key, &recipient_pubkey, dm_plaintext)?;

    let dm_template = EventTemplate {
        created_at: now(),
        kind: 4,
        tags: vec![vec!["p".to_string(), hex::encode(recipient.public_key)]],
        content: dm_ciphertext,
    };
    let dm_event = finalize_event(&dm_template, &sender.private_key)?;
    let dm_event_id = dm_event.id.clone();

    pool.publish(&dm_event).await?;

    let mut dm_received: Option<Event> = None;
    let dm_deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while tokio::time::Instant::now() < dm_deadline {
        let remaining = dm_deadline.saturating_duration_since(tokio::time::Instant::now());
        match timeout(remaining.max(Duration::from_millis(100)), dm_rx.recv()).await {
            Ok(Some(event)) => {
                if event.id == dm_event_id {
                    dm_received = Some(event);
                    break;
                }
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }

    let dm_received = dm_received.ok_or("Did not receive DM over relays")?;
    let sender_pubkey = xonly_to_compressed(&sender.public_key);
    let dm_decrypted = decrypt_v2(&recipient.private_key, &sender_pubkey, &dm_received.content)?;
    assert_eq!(dm_decrypted, dm_plaintext);

    let channel_metadata = ChannelMetadata::new(
        "OpenAgents Multi-Relay Smoke",
        "Channel smoke test for multiple relays",
        "",
    )
    .with_relays(RELAYS.iter().map(|relay| relay.to_string()).collect());

    let channel_template = EventTemplate {
        created_at: now(),
        kind: KIND_CHANNEL_CREATION,
        tags: vec![],
        content: channel_metadata.to_json()?,
    };
    let channel_event = finalize_event(&channel_template, &sender.private_key)?;
    let channel_id = channel_event.id.clone();

    pool.publish(&channel_event).await?;

    let channel_sub_id = format!("channel-smoke-{}", random::<u64>());
    let channel_filters = vec![serde_json::json!({
        "kinds": [KIND_CHANNEL_MESSAGE as u64],
        "#e": [channel_id],
        "limit": 5
    })];
    let mut channel_rx = pool.subscribe(&channel_sub_id, &channel_filters).await?;

    let channel_content = "multi-relay channel smoke";
    let channel_msg = ChannelMessageEvent::new(&channel_id, RELAYS[0], channel_content, now());
    let channel_template = EventTemplate {
        created_at: now(),
        kind: KIND_CHANNEL_MESSAGE,
        tags: channel_msg.to_tags(),
        content: channel_content.to_string(),
    };
    let channel_message = finalize_event(&channel_template, &sender.private_key)?;
    let channel_message_id = channel_message.id.clone();

    pool.publish(&channel_message).await?;

    let mut channel_received: Option<Event> = None;
    let channel_deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while tokio::time::Instant::now() < channel_deadline {
        let remaining = channel_deadline.saturating_duration_since(tokio::time::Instant::now());
        match timeout(remaining.max(Duration::from_millis(100)), channel_rx.recv()).await {
            Ok(Some(event)) => {
                if event.id == channel_message_id {
                    channel_received = Some(event);
                    break;
                }
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }

    let channel_received = channel_received.ok_or("Did not receive channel message")?;
    assert_eq!(channel_received.content, channel_content);

    Ok(())
}
