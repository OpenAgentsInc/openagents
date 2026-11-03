# Bitcoin

This folder holds *Bitcoin From Scratch*, the book that builds Bitcoin and
Lightning in [Coax](../coax/README.md), and its examples.

| Path | What |
| --- | --- |
| [`book/`](book/README.md) | *Bitcoin From Scratch*: the cryptography, encodings, transactions, script, Taproot, a full node (consensus, chain state, network, mempool and RPC), wallets, multi-party signatures and Lightning, built in Coax |
| [`examples/`](examples/) | The book's example programs and their checks |
| [`fixtures/`](fixtures/) | Each example's expected output |

The book is the companion to [Nostr From Scratch](../nostr/book/README.md).
Nostr's cryptography is Bitcoin's, so the chapters that built it (Nostr
chapters 1–13, 15 and 18) moved here on 2026-11-02, and the Nostr book starts
after this book's Parts I–V.

47 of the book's 124 chapters are implemented; the [status table](book/README.md#status) lists them.

The code the book teaches lives in the Coax library:
[`coax/lib/crypto/`](../coax/lib/crypto/) and
[`coax/lib/bitcoin/`](../coax/lib/bitcoin/). Chapter 36's script
interpreter, [`script.coax`](../coax/lib/bitcoin/script.coax), runs the
scripts every later chapter checks, from legacy script through segwit v0.
Chapters 37 and 38 explain its legacy and BIP-143 signature hashes, and
chapter 39 its timelocks, with [`locktime.coax`](../coax/lib/bitcoin/locktime.coax). Chapter 33's
[`path.coax`](../coax/lib/bitcoin/path.coax) reads derivation paths and
makes the standard BIP-44, 49, 84 and 86 addresses, and chapter 30's
[`descriptor.coax`](../coax/lib/bitcoin/descriptor.coax) expands output
descriptors to the scripts a wallet watches. Chapter 41's
[`taproot.coax`](../coax/lib/bitcoin/taproot.coax) holds Taproot: the key
tweak and the key path, then script trees and control blocks (chapter
42). Chapter 65's
[`p2p.coax`](../coax/lib/bitcoin/p2p.coax) reads and writes the peer-to-peer
messages: the 24-byte header and the payloads a node exchanges. Chapter
66's [`ellswift.coax`](../coax/lib/bitcoin/ellswift.coax) encodes keys as
BIP-324's ElligatorSwift and derives its shared secret.

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
(BIP-39 and BIP-32) and chapter 104 (PSBTs, descriptors, signature hashes
and multisig) are chapters 31–32, 94 and 100. Chapters 31 (BIP-39) and 32
(BIP-32) are written; the rest are planned, with the existing code named in the
[plan](book/PLAN.md). Chapter 104's ECDSA signer is chapter 20, and its transaction reader is
chapter 34's.

## Cryptography still to build

Everything Nostr used is built, and so are the five address types and the
Taproot key tweak, key path and script trees (chapters 41 and 42). Bitcoin
and Lightning also need:

- Batch Schnorr verification and tapscript.
- MuHash3072. Merkle trees with the duplicate-leaf flaw are built in
  chapter 46 (`coax/lib/bitcoin/merkle.coax`), compact targets in chapter
  47 (`coax/lib/bitcoin/pow.coax`), and retargeting and chainwork in
  chapter 48 (`coax/lib/bitcoin/retarget.coax`).
- The BIP-324 transport (ElligatorSwift is built in chapter 66,
  `coax/lib/bitcoin/ellswift.coax`), Golomb-coded block filters.
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
- **A full node** (owner, 2026-11-02): the book builds a complete Bitcoin
  node in Coax. It covers every consensus rule, chain state and storage, P2P,
  initial block download, the mempool and RPC. It syncs mainnet to the same
  UTXO set as Bitcoin Core. This replaces the earlier bounded
  block-validation profile. Core stays the reference, and our node decides
  no money on its own until it passes shadow mode and Core's functional
  tests ([plan](book/PLAN.md#the-full-node)).
- **Lightning is built in Coax**, channels included. No outside Lightning
  implementation is a dependency.
- **Bitcoin only:** Cashu, Fedimint and other chains stay out
  ([Nostr chapter 74](../nostr/book/ch074-cashu-and-fedimint-are-excluded.md)).
