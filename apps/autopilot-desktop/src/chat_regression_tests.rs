use std::path::PathBuf;

use crate::app_state::{
    DirectMessageProjectionState, ManagedChatProjectionState, direct_message_room_id,
};

fn repeated_hex(ch: char, len: usize) -> String {
    std::iter::repeat_n(ch, len).collect()
}

fn signed_event(
    id_ch: char,
    pubkey_ch: char,
    created_at: u64,
    kind: u16,
    tags: Vec<Vec<String>>,
    content: String,
) -> nostr::Event {
    nostr::Event {
        id: repeated_hex(id_ch, 64),
        pubkey: repeated_hex(pubkey_ch, 64),
        created_at,
        kind,
        tags,
        content,
        sig: repeated_hex('f', 128),
    }
}

fn fixture_identity() -> nostr::NostrIdentity {
    let private_key = [0x11_u8; 32];
    nostr::NostrIdentity {
        identity_path: PathBuf::from("/tmp/openagents-test-identity"),
        mnemonic: "test mnemonic".to_string(),
        npub: String::new(),
        nsec: String::new(),
        public_key_hex: nostr::get_public_key_hex(&private_key).expect("fixture pubkey"),
        private_key_hex: hex::encode(private_key),
    }
}

#[test]
fn managed_chat_projection_deduplicates_duplicate_relay_delivery_after_reload() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("managed-chat.json");
    let mut projection = ManagedChatProjectionState::from_projection_path_for_tests(path.clone());

    let group_metadata =
        nostr::GroupMetadataEvent::new("oa-main", nostr::GroupMetadata::new().with_name("Ops"), 10)
            .expect("group metadata");
    let channel = nostr::ManagedChannelCreateEvent::new(
        "oa-main",
        nostr::ChannelMetadata::new("ops", "", ""),
        20,
    )
    .expect("channel create");
    let channel_id = repeated_hex('b', 64);
    let message = nostr::ManagedChannelMessageEvent::new(
        "oa-main",
        channel_id.clone(),
        "wss://relay.openagents.test",
        "deploy now",
        30,
    )
    .expect("message");

    let group_event = signed_event('a', '1', 10, 39000, group_metadata.to_tags(), String::new());
    let channel_event = signed_event(
        'b',
        '2',
        20,
        40,
        channel.to_tags().expect("channel tags"),
        channel.content().expect("channel content"),
    );
    let message_event = signed_event(
        'c',
        '3',
        30,
        42,
        message.to_tags().expect("message tags"),
        "deploy now".to_string(),
    );

    projection.record_relay_events(vec![
        group_event.clone(),
        channel_event.clone(),
        message_event.clone(),
        message_event.clone(),
    ]);

    assert_eq!(projection.relay_events.len(), 3);
    assert_eq!(projection.snapshot.messages.len(), 1);
    let projected_channel = projection
        .snapshot
        .channels
        .iter()
        .find(|channel| channel.channel_id == channel_id)
        .expect("projected channel");
    assert_eq!(
        projected_channel.message_ids,
        vec![message_event.id.clone()]
    );
    assert_eq!(
        projected_channel.latest_message_id.as_deref(),
        Some(message_event.id.as_str())
    );

    let reloaded = ManagedChatProjectionState::from_projection_path_for_tests(path);
    assert_eq!(reloaded.snapshot.messages.len(), 1);
    let reloaded_channel = reloaded
        .snapshot
        .channels
        .iter()
        .find(|channel| channel.channel_id == channel_id)
        .expect("reloaded channel");
    assert_eq!(reloaded_channel.message_ids, vec![message_event.id]);
}

#[test]
fn direct_message_projection_skips_invalid_gift_wrap_and_rebuilds_valid_history() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("direct-messages.json");
    let mut projection = DirectMessageProjectionState::from_projection_path_for_tests(path.clone());
    let identity = fixture_identity();
    projection.set_identity(Some(&identity));

    let sender_secret = [0x22_u8; 32];
    let sender_pubkey = nostr::get_public_key_hex(&sender_secret).expect("sender pubkey");
    let dm_message = nostr::nip17::ChatMessage::new("hello there").add_recipient(
        identity.public_key_hex.clone(),
        Some("wss://relay.dm".to_string()),
    );
    let valid_wrap =
        nostr::nip17::send_chat_message(&dm_message, &sender_secret, &identity.public_key_hex, 101)
            .expect("valid wrap");
    let mut invalid_wrap = valid_wrap.clone();
    invalid_wrap.id = repeated_hex('f', 64);
    invalid_wrap.content = "not-a-valid-gift-wrap".to_string();

    projection.record_relay_events(vec![invalid_wrap, valid_wrap.clone()]);

    let room_id = direct_message_room_id(
        None,
        &[identity.public_key_hex.clone(), sender_pubkey.clone()],
    );
    let room = projection
        .snapshot
        .rooms
        .iter()
        .find(|room| room.room_id == room_id)
        .expect("direct message room");
    assert_eq!(room.message_ids.len(), 1);
    let message = projection
        .snapshot
        .messages
        .get(room.message_ids[0].as_str())
        .expect("projected message");
    assert_eq!(message.content, "hello there");
    assert_eq!(message.wrapped_event_ids, vec![valid_wrap.id.clone()]);

    let mut reloaded = DirectMessageProjectionState::from_projection_path_for_tests(path);
    reloaded.set_identity(Some(&identity));
    let reloaded_room = reloaded
        .snapshot
        .rooms
        .iter()
        .find(|room| room.room_id == room_id)
        .expect("reloaded room");
    assert_eq!(reloaded_room.message_ids.len(), 1);
    let reloaded_message = reloaded
        .snapshot
        .messages
        .get(reloaded_room.message_ids[0].as_str())
        .expect("reloaded message");
    assert_eq!(reloaded_message.content, "hello there");
}
