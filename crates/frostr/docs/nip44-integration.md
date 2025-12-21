# NIP-44 Encryption in FROSTR

## Overview

FROSTR uses NIP-44 versioned encryption for all peer-to-peer communication over Nostr relays. This ensures threshold signing messages are confidential even when transmitted over public relays.

## Encryption Pattern

### Broadcast to Peers

When broadcasting a Bifrost message to threshold peers:

```rust
use nostr::encrypt_v2;

// Serialize message
let message_json = serde_json::to_string(&bifrost_message)?;

// For each peer, encrypt with NIP-44
for peer_pubkey in &config.peer_pubkeys {
    let encrypted = encrypt_v2(
        &our_secret_key,     // [u8; 32]
        &peer_pubkey,        // &[u8] (compressed 33-byte pubkey)
        &message_json        // plaintext
    )?;

    // Build and publish Nostr event with encrypted content
    publish_to_relay(encrypted, peer_pubkey)?;
}
```

### Receive from Peers

When receiving encrypted messages from relays:

```rust
use nostr::decrypt_v2;

// Get event from relay
let event = relay.receive_event()?;

// Decrypt content
let plaintext = decrypt_v2(
    &our_secret_key,          // [u8; 32]
    &sender_pubkey,           // &[u8] (compressed 33-byte pubkey)
    &event.content            // base64 ciphertext
)?;

// Parse Bifrost message
let message: BifrostMessage = serde_json::from_str(&plaintext)?;
```

## Security Properties

- **Confidentiality**: Message content hidden from relays and observers
- **Authentication**: Sender identity verified via ECDH shared secret
- **Integrity**: HMAC-SHA256 prevents tampering
- **Forward Secrecy**: Not provided (use ephemeral keys if needed)

## Key Format Requirements

FROSTR uses secp256k1 keys compatible with Nostr:

- **Secret keys**: 32 bytes (x-only private key)
- **Public keys**: 33 bytes (compressed format: 0x02/0x03 prefix + x-coordinate)

Convert x-only pubkey to compressed for NIP-44:

```rust
fn xonly_to_compressed(xonly: &[u8; 32]) -> Vec<u8> {
    let mut compressed = vec![0x02]; // Even parity
    compressed.extend_from_slice(xonly);
    compressed
}
```

## Performance Considerations

- Encryption adds ~50-100μs per message
- Relay latency dominates (100-500ms typical)
- Consider batching small messages if throughput critical

## Message Format

Bifrost messages are encrypted and transmitted as Nostr events:

```json
{
  "kind": 28000,
  "content": "<base64-encoded NIP-44 ciphertext>",
  "tags": [
    ["p", "<recipient_pubkey_hex>"],
    ["protocol", "bifrost"],
    ["msg_type", "sign_req"]
  ],
  "pubkey": "<sender_pubkey_hex>",
  "created_at": 1234567890,
  "id": "...",
  "sig": "..."
}
```

## Testing

See `crates/nostr/core/tests/nip44_integration.rs` for comprehensive tests:

- Encrypt/decrypt roundtrip with various message sizes
- Unicode and emoji support
- Tamper detection (MAC verification)
- Wrong key rejection
- Different nonces produce different ciphertexts

All tests pass, confirming NIP-44 is ready for FROSTR integration.

## Implementation Status

- ✅ NIP-44 encryption/decryption available via `nostr::encrypt_v2` and `nostr::decrypt_v2`
- ✅ Integration tests passing
- ✅ nostr-client dependency added to FROSTR
- 🚧 NostrTransport implementation in progress (see #320)

## References

- [NIP-44 Specification](https://github.com/nostr-protocol/nips/blob/master/44.md)
- [FROSTR Protocol](../README.md)
- [NIP-44 Integration Tests](../../nostr/core/tests/nip44_integration.rs)
