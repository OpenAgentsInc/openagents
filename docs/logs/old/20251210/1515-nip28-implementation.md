# NIP-28 Implementation Log

**Date:** 2025-12-10
**Crate:** `crates/nostr`

## Summary

Implemented NIP-28 (Public Chat) for the nostr crate.

## NIP-28 Specification

NIP-28 defines event kinds for public chat channels, channel messages, and client-side moderation.

### Kinds
- 40: Channel creation
- 41: Channel metadata update
- 42: Channel message
- 43: Hide message (moderation)
- 44: Mute user (moderation)

### Key Features
- Channel metadata (name, about, picture, relays)
- NIP-10 style `e` tags with root/reply markers
- `p` tags for mentions in replies
- `t` tags for categories
- Moderation with optional reason

## Implementation

### Public API

```rust
// Types
pub struct ChannelMetadata { name, about, picture, relays }
pub struct ModerationReason { reason }
pub struct ChannelCreateEvent { metadata, created_at, tags }
pub struct ChannelMetadataEvent { channel_create_event_id, metadata, categories, ... }
pub struct ChannelMessageEvent { channel_create_event_id, reply_to_event_id, content, ... }
pub struct ChannelHideMessageEvent { message_event_id, reason, ... }
pub struct ChannelMuteUserEvent { pubkey_to_mute, reason, ... }

// Kind constants
pub const KIND_CHANNEL_CREATION: u16 = 40;
pub const KIND_CHANNEL_METADATA: u16 = 41;
pub const KIND_CHANNEL_MESSAGE: u16 = 42;
pub const KIND_CHANNEL_HIDE_MESSAGE: u16 = 43;
pub const KIND_CHANNEL_MUTE_USER: u16 = 44;

// Kind validation
pub fn is_channel_kind(kind: u16) -> bool;
pub fn is_channel_creation_kind(kind: u16) -> bool;
pub fn is_channel_metadata_kind(kind: u16) -> bool;
pub fn is_channel_message_kind(kind: u16) -> bool;
pub fn is_moderation_kind(kind: u16) -> bool;
```

### Usage Examples

```rust
// Create a channel
let metadata = ChannelMetadata::new("Bitcoin Discussion", "Talk about BTC", "https://pic.com")
    .with_relays(vec!["wss://relay.damus.io".to_string()]);
let create = ChannelCreateEvent::new(metadata, timestamp);

// Post a message
let msg = ChannelMessageEvent::new(channel_id, relay_url, "Hello!", timestamp);

// Reply to a message
let reply = ChannelMessageEvent::reply(channel_id, reply_to_id, relay_url, "Hi!", timestamp)
    .mention_pubkey(author_pubkey, None);

// Hide a message
let hide = ChannelHideMessageEvent::new(msg_id, timestamp)
    .with_reason("Spam");

// Mute a user
let mute = ChannelMuteUserEvent::new(pubkey, timestamp)
    .with_reason("Repeated spam");
```

### Test Coverage (30 NIP-28 tests)

#### Kind Validation Tests
- `test_channel_kinds` - Verify kind constants
- `test_is_channel_kind` - Range 40-44
- `test_is_channel_creation_kind` - Kind 40
- `test_is_channel_metadata_kind` - Kind 41
- `test_is_channel_message_kind` - Kind 42
- `test_is_moderation_kind` - Kinds 43-44

#### ChannelMetadata Tests
- `test_channel_metadata_new` - Basic creation
- `test_channel_metadata_with_relays` - Relay list
- `test_channel_metadata_json_roundtrip` - Serialization
- `test_channel_metadata_json_format` - JSON structure

#### ModerationReason Tests
- `test_moderation_reason` - Basic creation
- `test_moderation_reason_json` - JSON format

#### ChannelCreateEvent Tests (mirrors nostr-tools)
- `test_channel_create_event` - Basic creation
- `test_channel_create_event_content` - Content JSON

#### ChannelMetadataEvent Tests (mirrors nostr-tools)
- `test_channel_metadata_event` - Basic creation
- `test_channel_metadata_event_tags` - e tag generation
- `test_channel_metadata_event_with_categories` - t tags

#### ChannelMessageEvent Tests (mirrors nostr-tools)
- `test_channel_message_event_root` - Root message
- `test_channel_message_event_root_tags` - Root e tag with marker
- `test_channel_message_event_reply` - Reply message
- `test_channel_message_event_reply_tags` - Root + reply e tags
- `test_channel_message_event_with_mentions` - p tags

#### ChannelHideMessageEvent Tests (mirrors nostr-tools)
- `test_channel_hide_message_event` - Basic creation
- `test_channel_hide_message_event_tags` - e tag
- `test_channel_hide_message_event_content` - Reason JSON
- `test_channel_hide_message_event_no_reason` - Empty content

#### ChannelMuteUserEvent Tests (mirrors nostr-tools)
- `test_channel_mute_user_event` - Basic creation
- `test_channel_mute_user_event_tags` - p tag
- `test_channel_mute_user_event_content` - Reason JSON

#### Integration Tests
- `test_channel_workflow` - Full create→update→message→reply→hide→mute flow

## Files

- `crates/nostr/src/lib.rs` - Added NIP-28 exports
- `crates/nostr/src/nip28.rs` - Implementation and tests (new file)

## Test Results

```
running 115 tests (25 NIP-01 + 25 NIP-06 + 30 NIP-28 + 35 NIP-90)
test result: ok. 115 passed; 0 failed
```

## Notes

- Mirrors nostr-tools nip28.ts/nip28.test.ts structure
- Builder pattern for ergonomic API
- NIP-10 compliant e tags with root/reply markers
- Category support via t tags
- Optional moderation reasons
