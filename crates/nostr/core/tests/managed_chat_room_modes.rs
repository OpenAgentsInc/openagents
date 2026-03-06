#![allow(
    clippy::all,
    clippy::expect_used,
    clippy::panic,
    clippy::pedantic,
    clippy::unwrap_used
)]

use nostr::{
    ChannelMetadata, ManagedChannelCreateEvent, ManagedChannelHints, ManagedChatError,
    ManagedRoomMode,
};

#[test]
fn secure_group_room_mode_round_trips_as_a_reserved_label() {
    assert_eq!(ManagedRoomMode::SecureGroup.as_str(), "secure-group");
    assert_eq!(
        "secure-group"
            .parse::<ManagedRoomMode>()
            .expect("parse room mode"),
        ManagedRoomMode::SecureGroup
    );
}

#[test]
fn managed_channel_helpers_reject_secure_group_until_a_dedicated_adapter_exists() {
    let error = ManagedChannelCreateEvent::new("oa-main", ChannelMetadata::new("ops", "", ""), 10)
        .expect("channel create")
        .with_hints(ManagedChannelHints::new().with_room_mode(ManagedRoomMode::SecureGroup))
        .expect_err("secure-group should stay reserved for a separate adapter");

    assert!(matches!(
        error,
        ManagedChatError::UnsupportedRoomMode(value) if value == "secure-group"
    ));
}
