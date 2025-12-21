# FROSTR - FROST for Nostr

Native Rust implementation of the FROSTR protocol for threshold Schnorr signatures over Nostr.

## Overview

FROSTR enables threshold signing where k-of-n participants can sign without reconstructing the private key. This is the cryptographic foundation for NIP-SA Sovereign Agents, providing:

- **Threshold Security**: Agent private keys are split across multiple parties
- **Invisible Multi-sig**: Signatures look identical to single-key signatures
- **Key Rotation**: Compromised shares can be replaced without changing identity
- **Distributed Coordination**: Peers communicate via encrypted Nostr events

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         FROSTR                               │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────────┐    ┌─────────────────┐                 │
│  │    Keygen       │    │    Signing      │                 │
│  │  (Shamir SSS)   │    │    (FROST)      │                 │
│  └────────┬────────┘    └────────┬────────┘                 │
│           │                      │                           │
│           └──────────────────────┼──────────────────────┐    │
│                                  │                      │    │
│                    ┌─────────────▼─────────────┐        │    │
│                    │      Bifrost Node         │        │    │
│                    │                           │        │    │
│                    │  • Peer coordination      │        │    │
│                    │  • Nostr transport        │        │    │
│                    │  • Request routing        │        │    │
│                    │  • Share aggregation      │        │    │
│                    └─────────────┬─────────────┘        │    │
│                                  │                      │    │
│                    ┌─────────────▼─────────────┐        │    │
│                    │      Nostr Relays         │        │    │
│                    │   (NIP-44 encrypted)      │◄───────┘    │
│                    └───────────────────────────┘             │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Features

- **Shamir Secret Sharing** for key splitting
- **FROST signing protocol** for threshold signatures
- **Bifrost coordination** for peer-to-peer threshold operations
- **Nostr transport** with NIP-44 encryption (in progress)
- **Credential encoding** with bech32 (bfgroup1, bfshare1)

## Usage

```rust
use frostr::{keygen, signing, BifrostNode};

// Generate threshold keys (dealer mode)
let (group_key, shares) = keygen::dealer_keygen(2, 3)?;

// Create Bifrost node for threshold signing
let node = BifrostNode::new(config)?;

// Sign a message with threshold (requires 2 of 3 shares)
let signature = node.sign(&message_hash).await?;
```

## Credential Format

```
GROUP_CRED: "bfgroup1..." - Threshold group public key + config
  └─ threshold: k (e.g., 2)
  └─ total: n (e.g., 3)
  └─ group_pk: 32-byte public key

SHARE_CRED: "bfshare1..." - Individual secret share
  └─ index: share number (1..n)
  └─ secret_share: 32-byte scalar
  └─ group_pk: must match GROUP_CRED
```

## Encryption

FROSTR uses NIP-44 encryption for all Bifrost messages transmitted over Nostr relays. See [docs/nip44-integration.md](docs/nip44-integration.md) for implementation details.

## Status

- ✅ Shamir Secret Sharing
- ✅ FROST key generation
- ✅ FROST signing protocol
- ✅ Bifrost node infrastructure
- ✅ NIP-44 encryption support
- 🚧 Nostr relay transport (in progress)

## References

- [FROST Paper](https://eprint.iacr.org/2020/852)
- [FROSTR Protocol](https://github.com/FROSTR-ORG)
- [NIP-SA Specification](../nostr/nips/SA.md)
- [Directive d-007](../../docs/directives/d-007.md)

## License

MIT
