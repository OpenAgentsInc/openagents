# Bitcoin

This folder holds *Bitcoin From Scratch*, the book that builds Bitcoin and
Lightning in [Coax](../coax/README.md), and its examples.

| Path | What |
| --- | --- |
| [`book/`](book/README.md) | *Bitcoin From Scratch*: the cryptography, encodings, transactions, script, Taproot, consensus checks, network, wallets, multi-party signatures and Lightning, built in Coax |
| [`examples/`](examples/) | The book's example programs and their checks |
| [`fixtures/`](fixtures/) | Each example's expected output |

The book is the companion to [Nostr From Scratch](../nostr/book/README.md).
Nostr's cryptography is Bitcoin's, so the chapters that built it (Nostr
chapters 1–13, 15 and 18) moved here on 2026-11-02, and the Nostr book starts
after this book's Parts I–V.

The code the book teaches lives in the Coax library:
[`coax/lib/crypto/`](../coax/lib/crypto/) and
[`coax/lib/bitcoin/`](../coax/lib/bitcoin/).

```sh
python3 psionic/tests/book.py --book bitcoin                     # structure and registry
python3 psionic/tests/book.py --book bitcoin --coax coax/bin/coax   # also run the examples
```

## What moved from Nostr

The rule: if a BIP or BOLT specifies it, or Bitcoin needs it, this book
teaches it. If only a NIP needs it, the Nostr book teaches it.

| Nostr chapter | Bitcoin chapter |
| --- | --- |
| 1. Bytes, hex and base64 | 1 |
| 2. Big integers in fixed limbs | 2 |
| 3. Constant-time rules and secrets | 3 |
| 4. SHA-256 | 4 |
| 5. HMAC-SHA256 | 8 |
| 6. HKDF | 9 |
| 18. ChaCha20 | 11 |
| 7. Field arithmetic mod p | 13 |
| 8. Scalars mod n | 14 |
| 9. The secp256k1 group law | 15 |
| 10. Scalar multiplication | 16 |
| 12. Keys and randomness | 18 |
| 11. Verify a Schnorr signature | 23 |
| 13. Sign with Schnorr | 24 |
| 15. Bech32 | 27 |

The Nostr book keeps those numbers as moved entries, and its old pages
redirect here. What stays in Nostr: x-only ECDH for NIP-44 (chapter 14),
NIP-19, the JSON an event ID hashes, NIP-04, NIP-44, NIP-13 proof of work
and the NIP-06 seed path (chapter 46). The Bitcoin halves of Nostr chapter 46
(BIP-39 and BIP-32) and chapter 104 (PSBTs, descriptors, signature hashes,
ECDSA and multisig) are planned here as chapters 31–32, 58 and 63, with the
existing code named in the [plan](book/PLAN.md).

## Cryptography still to build

Everything Nostr used is built. Bitcoin and Lightning also need:

- ECDSA verification, public key recovery, strict DER and low-S.
- Base58Check and Bech32m.
- Batch Schnorr verification, Taproot script trees and tapscript.
- MuHash3072, Merkle trees with the duplicate-leaf flaw, compact targets and
  chainwork.
- ElligatorSwift and the BIP-324 transport, Golomb-coded block filters.
- MuSig2, FROST, adaptor signatures and silent payments.
- Lightning: BOLT 1 TLVs, the BOLT 8 Noise handshake, BOLT 3 keys,
  revocation and shachain, commitment and HTLC transactions, BOLT 2 channels,
  BOLT 4 onions with blinded paths, BOLT 7 gossip, routing, BOLT 11 invoices
  and BOLT 12 offers.

## Decisions

- **Title:** *Bitcoin From Scratch*.
- **Moved chapters keep their Nostr numbers** as moved entries, the way
  Psionic's harness chapters moved to Coder, so every "chapter N" reference
  in Nostr code stays valid. Nostr's parts were renumbered: its old Parts
  III–V became Part I, and VI–XII became II–VIII.
- **Consensus scope:** script, every signature hash, headers, proof of work
  and a bounded block-validation profile that states the rules it leaves out.
  No initial block download, and not a node to replace Bitcoin Core.
- **Lightning is built in Coax**, channels included. No outside Lightning
  implementation is a dependency.
- **Bitcoin only:** Cashu, Fedimint and other chains stay out
  ([Nostr chapter 74](../nostr/book/ch074-cashu-and-fedimint-are-excluded.md)).
