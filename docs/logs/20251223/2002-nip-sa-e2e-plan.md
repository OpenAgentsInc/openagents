# Plan: NIP-SA and Bifrost E2E Tests + Implementation

## Goal
Implement comprehensive end-to-end tests for the NIP-SA (Sovereign Agents) protocol AND complete the missing Bifrost threshold signing/ECDH coordination over Nostr relays.

## Current State Analysis

### What Exists
- **FROST Cryptography** (complete): `crates/frostr/src/signing.rs`
  - `round1_commit()`, `round2_sign()`, `aggregate_signatures()`, `verify_signature()`
  - Local 2-of-3 and 3-of-5 threshold signing works perfectly

- **BifrostNode Structure** (partial): `crates/frostr/src/bifrost/node.rs`
  - Configuration, peer management, lifecycle (start/stop)
  - `sign()` method exists but fails at aggregation (line 501: "aggregation requires full FROST type serialization")
  - `ecdh()` method returns "not yet implemented"

- **NIP-SA Types** (complete): `crates/nostr/core/src/nip_sa/`
  - All 10 event kinds (38000-38031) fully implemented
  - Existing in-memory tests (no relay)

- **Test Relay** (complete): `crates/nostr/tests/integration/mod.rs`
  - `start_test_relay(port)` function works

### What's Missing
1. **NostrTransport.publish_and_wait()** - Placeholder, doesn't actually publish/subscribe
2. **FROST type serialization** - Can't send commitments/signatures over wire
3. **Response message handling** - No subscription loop for incoming messages
4. **NIP-SA relay tests** - Events tested in-memory only, not over relays

## Implementation Plan

### Phase 1: Complete Bifrost Relay Coordination (crates/frostr/)

#### 1.1 Implement FROST Type Serialization
**File:** `crates/frostr/src/bifrost/serialization.rs` (NEW)

Add serde support for FROST types to enable network transmission:
```rust
// Serialize SigningCommitments to 66 bytes (2x33-byte compressed points)
pub fn serialize_commitments(c: &SigningCommitments) -> [u8; 66]
pub fn deserialize_commitments(bytes: &[u8; 66]) -> Result<SigningCommitments>

// Serialize SignatureShare to 32 bytes
pub fn serialize_sig_share(s: &SignatureShare) -> [u8; 32]
pub fn deserialize_sig_share(bytes: &[u8; 32], id: Identifier) -> Result<SignatureShare>
```

Reference: `/Users/christopherdavid/code/frost/src/types/commit.ts` (hidden_pn + binder_pn format)

#### 1.2 Implement NostrTransport.publish_and_wait()
**File:** `crates/frostr/src/bifrost/transport.rs`

Replace placeholder with actual implementation:
1. Connect to relay pool using nostr-client
2. Publish NIP-44 encrypted Bifrost event (kind 28000)
3. Subscribe for responses with matching session_id
4. Collect responses until threshold or timeout
5. Return collected BifrostMessage responses

Reference: `/Users/christopherdavid/code/igloo-server/src/node/manager.ts` (lines 1309-1601)

#### 1.3 Complete BifrostNode.sign() Coordinator Flow
**File:** `crates/frostr/src/bifrost/node.rs`

Update `sign()` method (lines 447-505):
1. Generate nonces/commitments with `round1_commit()`
2. Serialize commitments using new serialization
3. Broadcast SignRequest via transport
4. Receive SignResponse messages from peers
5. Deserialize partial signatures
6. Build SigningPackage with all commitments
7. Call `aggregate_signatures()` to produce final signature
8. Return 64-byte Schnorr signature

#### 1.4 Implement Response Handler for Peer Role
**File:** `crates/frostr/src/bifrost/node.rs`

Add message handling for when THIS node is a responder (not coordinator):
```rust
pub async fn handle_incoming_message(&mut self, msg: BifrostMessage) -> Result<()>
```
- On `/sign/req`: Generate commitment + partial sig, send `/sign/res`
- On `/ecdh/req`: Generate partial ECDH, send `/ecdh/res`

#### 1.5 Complete BifrostNode.ecdh()
**File:** `crates/frostr/src/bifrost/node.rs`

Similar to sign(), but for ECDH:
1. Broadcast EcdhRequest
2. Collect EcdhResponse from peers
3. Use `combine_ecdh_shares()` for aggregation
4. Return 32-byte shared secret

---

### Phase 2: Bifrost E2E Tests (crates/frostr/tests/)

#### 2.1 Update bifrost_e2e.rs
**File:** `crates/frostr/tests/bifrost_e2e.rs`

Update existing tests to expect SUCCESS (not failure):
- `test_bifrost_signing_2_of_3_over_relay` → Verify produces valid signature
- `test_bifrost_ecdh_2_of_3_over_relay` → Verify produces valid shared secret

#### 2.2 Add New Bifrost Tests
**File:** `crates/frostr/tests/bifrost_e2e.rs`

Add tests per d-014:
- `test_bifrost_peer_discovery` - Verify nodes can ping each other
- `test_bifrost_timeout_handling` - Verify timeout when peer offline
- `test_bifrost_3_of_5_signing` - Verify 3-of-5 configuration
- `test_bifrost_any_quorum_produces_same_result` - Verify different quorums work

---

### Phase 3: NIP-SA Relay Integration Tests

#### 3.1 Create NIP-SA Integration Test Module
**File:** `crates/nostr/tests/integration/nip_sa.rs` (NEW)

Tests that publish/fetch NIP-SA events over real relay:
- `test_agent_profile_publish_and_fetch` - kind 38000
- `test_agent_state_encrypt_decrypt` - kind 38001 with NIP-44
- `test_agent_schedule_replace` - kind 38002 replaceable semantics
- `test_tick_request_result_flow` - kinds 38010/38011
- `test_trajectory_session_and_events` - kinds 38030/38031
- `test_skill_license_delivery` - kinds 38020/38021

**File:** `crates/nostr/tests/integration/mod.rs`
Add: `pub mod nip_sa;`

---

### Phase 4: Full Agent Lifecycle Tests

#### 4.1 Create E2E Agent Test
**File:** `crates/nostr/core/tests/e2e_agent.rs` (NEW)

Full sovereign agent lifecycle:
```rust
#[tokio::test]
async fn test_sovereign_agent_lifecycle() {
    // 1. Generate 2-of-3 threshold identity
    // 2. Start test relay
    // 3. Create and publish AgentProfile (kind 38000)
    // 4. Create encrypted AgentState (kind 38001)
    // 5. Fetch and decrypt state
    // 6. Execute tick with trajectory
    // 7. Verify full round-trip
}
```

#### 4.2 Agent Signs with Bifrost
**File:** `crates/nostr/core/tests/e2e_agent.rs`

```rust
#[tokio::test]
async fn test_agent_signs_event_with_bifrost() {
    // Use BifrostNode to sign a Nostr event
    // Verify signature validates against group pubkey
}
```

#### 4.3 Agent Decrypts with Threshold ECDH
**File:** `crates/nostr/core/tests/e2e_agent.rs`

```rust
#[tokio::test]
async fn test_agent_decrypts_dm_with_threshold_ecdh() {
    // External party sends NIP-44 encrypted DM to agent
    // Agent uses threshold ECDH to derive shared secret
    // Agent decrypts and verifies content
}
```

---

## Files to Modify/Create

| File | Action | Description |
|------|--------|-------------|
| `crates/frostr/src/bifrost/serialization.rs` | CREATE | FROST type serialization |
| `crates/frostr/src/bifrost/transport.rs` | MODIFY | Implement publish_and_wait() |
| `crates/frostr/src/bifrost/node.rs` | MODIFY | Complete sign(), ecdh(), add response handler |
| `crates/frostr/src/bifrost/mod.rs` | MODIFY | Export serialization module |
| `crates/frostr/tests/bifrost_e2e.rs` | MODIFY | Update tests to expect success, add new tests |
| `crates/nostr/tests/integration/nip_sa.rs` | CREATE | NIP-SA relay tests |
| `crates/nostr/tests/integration/mod.rs` | MODIFY | Add `pub mod nip_sa;` |
| `crates/nostr/core/tests/e2e_agent.rs` | CREATE | Full agent lifecycle tests |

---

## Dependencies

### Required Crates (already in workspace)
- `nostr-client` - For relay connections
- `nostr-relay` - For test relay server
- `frost-secp256k1` - FROST primitives
- `tokio` - Async runtime

### Reference Implementations
- `/Users/christopherdavid/code/igloo-server/` - TypeScript Bifrost (message flow, event handling)
- `/Users/christopherdavid/code/frost/` - FROST protocol (serialization formats, round flow)

---

## Test Execution

```bash
# Run all Bifrost E2E tests
cargo test -p frostr --test bifrost_e2e

# Run NIP-SA integration tests
cargo test -p nostr-tests --test integration_tests nip_sa

# Run agent lifecycle tests
cargo test -p nostr --test e2e_agent

# Run with logging
RUST_LOG=debug cargo test -p frostr --test bifrost_e2e -- --nocapture
```

---

## Success Criteria

1. All Bifrost E2E tests pass with real relay connections
2. 2-of-3 and 3-of-5 threshold signing produces valid Schnorr signatures
3. Threshold ECDH produces deterministic shared secrets
4. All 10 NIP-SA event kinds tested over relay
5. Full agent lifecycle (profile → state → tick → trajectory) works E2E
6. Tests are deterministic and don't require external relays
