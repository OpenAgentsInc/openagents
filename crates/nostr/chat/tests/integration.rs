//! Integration tests for nostr-chat crate
//!
//! These tests verify end-to-end chat functionality including:
//! - Identity management (NIP-06)
//! - Connecting to relays and receiving events
//! - Channel operations (NIP-28)
//! - DVM job tracking (NIP-90)

use nostr_chat::{ChatEvent, ChatState, DvmJobStatus};
use std::time::Duration;
use tokio::time::timeout;

/// Test identity from mnemonic (NIP-06)
#[test]
fn test_identity_derivation() {
    let mut state = ChatState::new();

    // Known test mnemonic
    let mnemonic = "leader monkey parrot ring guide accident before fence cannon height naive bean";

    state.set_identity_from_mnemonic(mnemonic).unwrap();

    assert!(state.has_identity());

    // Verify npub matches expected value
    let npub = state.npub().unwrap();
    assert_eq!(
        npub,
        "npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu"
    );

    // Verify pubkey hex is present
    let pubkey = state.pubkey().unwrap();
    assert!(!pubkey.is_empty());
    assert_eq!(pubkey.len(), 64); // 32 bytes = 64 hex chars
}

/// Test identity with different mnemonics
#[test]
fn test_different_mnemonics_produce_different_keys() {
    let mut state1 = ChatState::new();
    let mut state2 = ChatState::new();

    state1
        .set_identity_from_mnemonic(
            "leader monkey parrot ring guide accident before fence cannon height naive bean",
        )
        .unwrap();

    state2
        .set_identity_from_mnemonic(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        )
        .unwrap();

    assert_ne!(state1.pubkey(), state2.pubkey());
    assert_ne!(state1.npub(), state2.npub());
}

/// Test invalid mnemonic handling
#[test]
fn test_invalid_mnemonic() {
    let mut state = ChatState::new();

    // Invalid mnemonic (wrong words)
    let result = state.set_identity_from_mnemonic("invalid mnemonic words that dont exist");
    assert!(result.is_err());

    // Identity should not be set
    assert!(!state.has_identity());
}

/// Test chat state initialization
#[tokio::test]
async fn test_chat_state_initialization() {
    let state = ChatState::new();

    // No identity initially
    assert!(!state.has_identity());
    assert!(state.pubkey().is_none());
    assert!(state.npub().is_none());

    // No channels initially
    let channels = state.channels().await;
    assert!(channels.is_empty());

    // No messages initially
    let messages = state.messages("any_channel").await;
    assert!(messages.is_empty());

    // No jobs initially
    let jobs = state.jobs().await;
    assert!(jobs.is_empty());
}

/// Test chat state with custom relays
#[tokio::test]
async fn test_chat_state_custom_relays() {
    let state = ChatState::with_relays(vec![
        "wss://custom-relay.example.com".to_string(),
        "wss://another-relay.example.com".to_string(),
    ]);

    // State should be initialized
    assert!(!state.has_identity());

    // Can set identity
    let mut state = state;
    state
        .set_identity_from_mnemonic(
            "leader monkey parrot ring guide accident before fence cannon height naive bean",
        )
        .unwrap();
    assert!(state.has_identity());
}

/// Test connecting to relays
#[tokio::test]
async fn test_connect_to_relays() {
    let mut state = ChatState::new();

    // Set identity first
    state
        .set_identity_from_mnemonic(
            "leader monkey parrot ring guide accident before fence cannon height naive bean",
        )
        .unwrap();

    // Subscribe to chat events
    let mut events = state.subscribe();

    // Try to connect with timeout
    let connect_result = timeout(Duration::from_secs(15), state.connect()).await;

    match connect_result {
        Ok(Ok(count)) => {
            println!("Connected to {} relays", count);

            // Should receive Connected event
            let event_result = timeout(Duration::from_secs(2), events.recv()).await;
            if let Ok(Ok(ChatEvent::Connected { relay_count })) = event_result {
                println!("Received Connected event: {} relays", relay_count);
                assert!(relay_count > 0);
            }
        }
        Ok(Err(e)) => {
            println!("Connection error: {} (acceptable in CI)", e);
        }
        Err(_) => {
            println!("Connection timed out (acceptable in CI)");
        }
    }
}

/// Test chat event subscription
#[tokio::test]
async fn test_chat_event_subscription() {
    let state = ChatState::new();

    // Subscribe multiple times
    let _rx1 = state.subscribe();
    let _rx2 = state.subscribe();
    let _rx3 = state.subscribe();

    // Should not panic or error
}

/// Test DVM job status transitions
#[test]
fn test_dvm_job_status() {
    use nostr_chat::DvmJob;

    let job = DvmJob {
        id: "test_job_123".to_string(),
        kind: 5050,
        status: DvmJobStatus::Pending,
        input: "summarize this text".to_string(),
        result: None,
        created_at: 1234567890,
    };

    assert_eq!(job.status, DvmJobStatus::Pending);
    assert!(job.result.is_none());

    // Simulate status transitions
    let mut job = job;
    job.status = DvmJobStatus::Processing;
    assert_eq!(job.status, DvmJobStatus::Processing);

    job.status = DvmJobStatus::Completed;
    job.result = Some("Summary: This is a test.".to_string());
    assert_eq!(job.status, DvmJobStatus::Completed);
    assert!(job.result.is_some());
}

/// Test DVM job failure status
#[test]
fn test_dvm_job_failure() {
    let status = DvmJobStatus::Failed("Service unavailable".to_string());

    match status {
        DvmJobStatus::Failed(msg) => {
            assert_eq!(msg, "Service unavailable");
        }
        _ => panic!("Expected Failed status"),
    }
}

/// Test channel message retrieval
#[tokio::test]
async fn test_channel_messages() {
    let state = ChatState::new();

    // Messages for non-existent channel should be empty
    let messages = state.messages("non_existent_channel_id").await;
    assert!(messages.is_empty());

    // Multiple calls should be consistent
    let messages1 = state.messages("channel_1").await;
    let messages2 = state.messages("channel_1").await;
    assert_eq!(messages1.len(), messages2.len());
}

/// Test job retrieval
#[tokio::test]
async fn test_job_retrieval() {
    let state = ChatState::new();

    // Jobs should be empty initially
    let jobs = state.jobs().await;
    assert!(jobs.is_empty());

    // Specific job lookup should return None
    let job = state.job("non_existent_job").await;
    assert!(job.is_none());
}

/// Test channel retrieval
#[tokio::test]
async fn test_channel_retrieval() {
    let state = ChatState::new();

    // Channels should be empty initially
    let channels = state.channels().await;
    assert!(channels.is_empty());

    // Specific channel lookup should return None
    let channel = state.channel("non_existent_channel").await;
    assert!(channel.is_none());
}

/// Test disconnect
#[tokio::test]
async fn test_disconnect() {
    let state = ChatState::new();

    // Subscribe to events
    let mut events = state.subscribe();

    // Disconnect (should work even if not connected)
    state.disconnect().await;

    // Should receive Disconnected event
    let event_result = timeout(Duration::from_millis(100), events.recv()).await;
    if let Ok(Ok(ChatEvent::Disconnected)) = event_result {
        println!("Received Disconnected event");
    }
}

/// Test connected count
#[tokio::test]
async fn test_connected_count() {
    let state = ChatState::new();

    // Should be 0 before connecting
    let count = state.connected_count().await;
    assert_eq!(count, 0);
}

/// Test full e2e flow: identity -> connect -> join channel
#[tokio::test]
async fn test_full_e2e_flow() {
    let mut state = ChatState::new();

    // Step 1: Set identity
    state
        .set_identity_from_mnemonic(
            "leader monkey parrot ring guide accident before fence cannon height naive bean",
        )
        .unwrap();
    assert!(state.has_identity());
    println!("Identity set: {}", state.npub().unwrap());

    // Step 2: Subscribe to events
    let mut events = state.subscribe();

    // Step 3: Connect to relays
    let connect_result = timeout(Duration::from_secs(15), state.connect()).await;
    match connect_result {
        Ok(Ok(count)) => {
            println!("Connected to {} relays", count);

            if count > 0 {
                // Step 4: Wait for Connected event
                loop {
                    match timeout(Duration::from_secs(2), events.recv()).await {
                        Ok(Ok(ChatEvent::Connected { relay_count })) => {
                            println!("Got Connected event: {} relays", relay_count);
                            break;
                        }
                        Ok(Ok(other)) => {
                            println!("Got other event: {:?}", other);
                        }
                        _ => break,
                    }
                }

                // Step 5: Try to join a channel (will fail if channel doesn't exist, but tests the flow)
                // Using a known public channel ID (this is a test channel)
                let join_result = timeout(
                    Duration::from_secs(5),
                    state.join_channel(
                        "25e5c82273a271cb1a840d0060391a0bf4965cafeb029d5ab55350b418953fbb",
                    ),
                )
                .await;

                match join_result {
                    Ok(Ok(())) => {
                        println!("Joined channel successfully");

                        // Check for ChannelJoined event
                        if let Ok(Ok(ChatEvent::ChannelJoined { channel_id })) =
                            timeout(Duration::from_secs(2), events.recv()).await
                        {
                            println!("Got ChannelJoined event: {}", channel_id);
                        }
                    }
                    Ok(Err(e)) => {
                        println!("Join channel error: {} (expected if not connected)", e);
                    }
                    Err(_) => {
                        println!("Join channel timed out");
                    }
                }
            }
        }
        Ok(Err(e)) => {
            println!("Connection error: {}", e);
        }
        Err(_) => {
            println!("Connection timed out (acceptable in CI)");
        }
    }

    // Step 6: Disconnect
    state.disconnect().await;
    println!("Disconnected");
}
