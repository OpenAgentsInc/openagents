# Plan: Bifrost Full Relay Integration & E2E Tests

## Goal
Wire up existing Bifrost infrastructure to work E2E over the test relay, enabling threshold signing and ECDH coordination between multiple BifrostNodes communicating over Nostr.

## Current State Analysis (Updated 2025-12-23)

### What Already Exists (COMPLETE)

1. **FROST Cryptography** - `crates/frostr/src/signing.rs`
   - `round1_commit()`, `round2_sign()`, `aggregate_signatures()`, `verify_signature()`
   - Local 2-of-3 and 3-of-5 threshold signing works

2. **NostrTransport** - `crates/frostr/src/bifrost/transport.rs` (FULLY IMPLEMENTED)
   - `publish_and_wait()` with session-based response correlation (lines 414-473)
   - NIP-44 encryption/decryption for peer messages (lines 180-186)
   - Background subscription loop via `process_incoming_event()` (lines 151-263)
   - Relay pool management via `nostr-client::RelayPool`
   - Automatic Ping/Pong handling at transport layer

3. **BifrostNode** - `crates/frostr/src/bifrost/node.rs`
   - `sign()` coordinator flow (needs SigningPackage building)
   - `ecdh()` coordinator flow (COMPLETE)
   - `handle_ecdh_request()` responder handler (COMPLETE)
   - `handle_message()` routing (COMPLETE)

4. **nostr-client** - `crates/nostr/client/`
   - `RelayConnection` and `RelayPool` production-ready
   - Full publish/subscribe with callbacks and channels
   - Circuit breaker, exponential backoff, offline queue

5. **nostr-relay** - `crates/nostr/relay/`
   - `start_test_relay(port)` function in integration tests
   - Full NIP-01 WebSocket protocol
   - Event broadcasting to subscribers
   - SQLite database backend

6. **NIP-SA Types** - `crates/nostr/core/src/nip_sa/`
   - All 10 event kinds (38000-38031) implemented
   - Integration tests created (9 tests passing)

### What's Missing

1. **Test harness wiring** - E2E tests don't start the test relay or configure NostrTransport to connect
2. **Multi-node test setup** - Need 3+ BifrostNodes running concurrently, each with its own transport
3. **SigningPackage building** - `sign()` needs to deserialize peer commitments and build package
4. **FROST serialization helpers** - For SigningCommitments and SignatureShare wire format

---

## Implementation Plan

### Phase 1: Test Harness Setup

#### 1.1 Create Bifrost Test Helpers
**File:** `crates/frostr/tests/helpers.rs` (NEW)

```rust
use nostr_relay::{RelayServer, Database, DatabaseConfig, RelayConfig};

/// Start test relay on given port, return (server, url, temp_dir)
pub async fn start_bifrost_test_relay(port: u16) -> (Arc<RelayServer>, String, TempDir)

/// Create TransportConfig pointing to test relay with given keypair
pub fn create_transport_config(
    relay_url: &str,
    secret_key: [u8; 32],
    peer_pubkeys: Vec<[u8; 32]>,
) -> TransportConfig

/// Generate n threshold keypairs for testing (dealer-based for simplicity)
pub fn generate_test_shares(threshold: u8, num_shares: u8) -> Vec<FrostShare>

/// Create and start n BifrostNodes, all connected to same relay
pub async fn create_connected_nodes(
    num_nodes: usize,
    threshold: u8,
    relay_url: &str,
) -> Vec<BifrostNode>
```

#### 1.2 Update bifrost_e2e.rs Test Setup
**File:** `crates/frostr/tests/bifrost_e2e.rs`

Change tests from expecting failures to using real relay:

```rust
#[tokio::test]
async fn test_bifrost_signing_2_of_3_over_relay() {
    // 1. Start test relay
    let (server, relay_url, _temp) = start_bifrost_test_relay(19200).await;

    // 2. Generate 2-of-3 threshold shares
    let shares = generate_test_shares(2, 3);

    // 3. Create 3 BifrostNodes with transports pointing to relay
    let nodes = create_connected_nodes(3, 2, &relay_url).await;

    // 4. Start background responder loops for nodes 1 and 2
    spawn_responder(nodes[1].clone());
    spawn_responder(nodes[2].clone());

    // 5. Node 0 initiates signing as coordinator
    let message = b"test message";
    let signature = nodes[0].sign(message).await?;

    // 6. Verify signature against group public key
    verify_signature(&shares[0].group_public_key, message, &signature)?;
}
```

---

### Phase 2: FROST Wire Format Serialization

#### 2.1 Add Serialization Helpers
**File:** `crates/frostr/src/bifrost/serialization.rs` (EXISTS - enhance)

Current file has placeholder. Add:

```rust
use frost_secp256k1::{SigningCommitments, SignatureShare, Identifier};

/// Serialize SigningCommitments to 66 bytes (hiding + binding points)
pub fn serialize_commitments(c: &SigningCommitments) -> [u8; 66] {
    let hiding = c.hiding().serialize(); // 33 bytes
    let binding = c.binding().serialize(); // 33 bytes
    let mut out = [0u8; 66];
    out[..33].copy_from_slice(&hiding);
    out[33..].copy_from_slice(&binding);
    out
}

/// Deserialize SigningCommitments from 66 bytes
pub fn deserialize_commitments(bytes: &[u8; 66]) -> Result<SigningCommitments>

/// Serialize SignatureShare to 32 bytes
pub fn serialize_sig_share(s: &SignatureShare) -> [u8; 32]

/// Deserialize SignatureShare from 32 bytes with identifier
pub fn deserialize_sig_share(bytes: &[u8; 32], id: Identifier) -> Result<SignatureShare>
```

#### 2.2 Update SignRequest/SignResponse Messages
**File:** `crates/frostr/src/bifrost/messages.rs`

Ensure message structs use serialized byte arrays:

```rust
pub struct SignRequest {
    pub session_id: String,
    pub message_hash: [u8; 32],
    pub participants: Vec<u8>,
    #[serde_as(as = "[_; 66]")]
    pub coordinator_commitments: [u8; 66],  // Serialized SigningCommitments
}

pub struct SignResponse {
    pub session_id: String,
    pub participant_id: u8,
    #[serde_as(as = "[_; 66]")]
    pub commitments: [u8; 66],  // Serialized SigningCommitments
    #[serde_as(as = "[_; 32]")]
    pub signature_share: [u8; 32],  // Serialized SignatureShare
}
```

---

### Phase 3: Complete sign() Coordinator Flow

#### 3.1 Update BifrostNode.sign()
**File:** `crates/frostr/src/bifrost/node.rs`

The current implementation broadcasts but doesn't aggregate. Fix:

```rust
pub async fn sign(&self, message: &[u8]) -> Result<[u8; 64]> {
    // 1. Get transport and frost_share
    let transport = self.transport.as_ref().ok_or(...)?;
    let frost_share = self.frost_share.as_ref().ok_or(...)?;

    // 2. Select k participants
    let participants = self.select_participants(frost_share.threshold as usize)?;

    // 3. Generate our nonces and commitments
    let (nonces, our_commitments) = round1_commit(frost_share)?;

    // 4. Serialize our commitments
    let our_commitments_bytes = serialize_commitments(&our_commitments);

    // 5. Create and broadcast SignRequest
    let message_hash = sha256(message);
    let request = SignRequest {
        session_id: self.generate_session_id(),
        message_hash,
        participants: participants.clone(),
        coordinator_commitments: our_commitments_bytes,
    };

    // 6. Wait for (k-1) SignResponse messages
    let responses = transport.publish_and_wait(
        &BifrostMessage::SignRequest(request),
        frost_share.threshold as usize - 1,
    ).await?;

    // 7. Collect all commitments (ours + peers)
    let mut all_commitments = BTreeMap::new();
    all_commitments.insert(frost_share.participant_id, our_commitments);
    for resp in responses {
        if let BifrostMessage::SignResponse(sr) = resp {
            let peer_commitments = deserialize_commitments(&sr.commitments)?;
            all_commitments.insert(sr.participant_id, peer_commitments);
        }
    }

    // 8. Build SigningPackage
    let signing_package = SigningPackage::new(all_commitments, &message_hash)?;

    // 9. Generate our signature share
    let our_share = round2_sign(&signing_package, &nonces, frost_share)?;

    // 10. Collect peer signature shares
    let mut all_shares = BTreeMap::new();
    all_shares.insert(frost_share.participant_id, our_share);
    for resp in responses {
        if let BifrostMessage::SignResponse(sr) = resp {
            let peer_share = deserialize_sig_share(&sr.signature_share, sr.participant_id)?;
            all_shares.insert(sr.participant_id, peer_share);
        }
    }

    // 11. Aggregate into final signature
    let signature = aggregate_signatures(&signing_package, &all_shares, &frost_share.group_public_key)?;

    Ok(signature.serialize())
}
```

#### 3.2 Add handle_sign_request() Responder
**File:** `crates/frostr/src/bifrost/node.rs`

```rust
pub fn handle_sign_request(&self, request: &SignRequest) -> Result<SignResponse> {
    let frost_share = self.frost_share.as_ref().ok_or(...)?;

    // 1. Generate our nonces and commitments
    let (nonces, our_commitments) = round1_commit(frost_share)?;

    // 2. Store nonces for later (keyed by session_id)
    self.pending_nonces.insert(request.session_id.clone(), nonces);

    // 3. Deserialize coordinator's commitments
    let coord_commitments = deserialize_commitments(&request.coordinator_commitments)?;

    // 4. Build partial SigningPackage (just coordinator + us for now)
    // Note: In practice need all participants' commitments

    // 5. Generate signature share
    let our_share = round2_sign(...)?;

    // 6. Return response
    Ok(SignResponse {
        session_id: request.session_id.clone(),
        participant_id: frost_share.participant_id,
        commitments: serialize_commitments(&our_commitments),
        signature_share: serialize_sig_share(&our_share),
    })
}
```

---

### Phase 4: E2E Tests with Real Relay

#### 4.1 Core Signing Test
**File:** `crates/frostr/tests/bifrost_e2e.rs`

```rust
#[tokio::test]
async fn test_bifrost_2_of_3_signing_over_relay() {
    let (_, relay_url, _temp) = start_bifrost_test_relay(19200).await;
    let nodes = create_connected_nodes(3, 2, &relay_url).await;

    // Spawn responder tasks for non-coordinator nodes
    for node in &nodes[1..] {
        spawn_responder_loop(node.clone());
    }

    // Coordinator signs
    let message = b"Hello, threshold world!";
    let signature = nodes[0].sign(message).await.expect("signing should succeed");

    // Verify
    let group_pk = nodes[0].group_public_key();
    verify_signature(&group_pk, message, &signature).expect("signature should verify");
}

#[tokio::test]
async fn test_bifrost_2_of_3_ecdh_over_relay() {
    let (_, relay_url, _temp) = start_bifrost_test_relay(19201).await;
    let nodes = create_connected_nodes(3, 2, &relay_url).await;

    for node in &nodes[1..] {
        spawn_responder_loop(node.clone());
    }

    // External party's public key
    let external_pk = generate_test_keypair().public_key;

    // Coordinator performs threshold ECDH
    let shared_secret = nodes[0].ecdh(&external_pk).await.expect("ecdh should succeed");

    // Verify it's 32 bytes
    assert_eq!(shared_secret.len(), 32);
}
```

#### 4.2 Quorum Determinism Test
```rust
#[tokio::test]
async fn test_any_quorum_produces_same_signature() {
    // Test that nodes {0,1}, {0,2}, and {1,2} all produce valid signatures
}
```

#### 4.3 Timeout Handling Test
```rust
#[tokio::test]
async fn test_signing_timeout_when_peer_offline() {
    // Start only 1 of 2 required peers, expect timeout error
}
```

---

## Files to Modify/Create

| File | Action | Description |
|------|--------|-------------|
| `crates/frostr/tests/helpers.rs` | CREATE | Test harness utilities |
| `crates/frostr/src/bifrost/serialization.rs` | MODIFY | Add FROST type serialization |
| `crates/frostr/src/bifrost/messages.rs` | MODIFY | Update SignRequest/Response with serialized fields |
| `crates/frostr/src/bifrost/node.rs` | MODIFY | Complete sign() flow, add handle_sign_request() |
| `crates/frostr/tests/bifrost_e2e.rs` | MODIFY | Update tests to use real relay |
| `crates/frostr/Cargo.toml` | MODIFY | Add dev-dependency on nostr-relay |

---

## Test Execution

```bash
# Run Bifrost E2E tests with real relay
cargo test -p frostr --test bifrost_e2e -- --nocapture

# Run with debug logging
RUST_LOG=frostr=debug,nostr_client=debug cargo test -p frostr --test bifrost_e2e -- --nocapture

# Run specific test
cargo test -p frostr --test bifrost_e2e test_bifrost_2_of_3_signing_over_relay
```

---

## Success Criteria

1. `test_bifrost_2_of_3_signing_over_relay` passes - threshold signing works E2E
2. `test_bifrost_2_of_3_ecdh_over_relay` passes - threshold ECDH works E2E
3. `test_any_quorum_produces_same_signature` passes - different quorums produce valid signatures
4. All tests use real nostr-relay, no mocks
5. NIP-44 encryption verified between nodes
6. Timeout handling works when peers are offline
