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

86 of the book's 124 chapters are implemented; the [status table](book/README.md#status) lists them.

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
42); chapter 43's Taproot signature hash, with the annex and the script
path's extension, is in [`sighash.coax`](../coax/lib/bitcoin/sighash.coax).
Chapter 65's
[`p2p.coax`](../coax/lib/bitcoin/p2p.coax) reads and writes the peer-to-peer
messages: the 24-byte header and the payloads a node exchanges. Chapter
66's [`ellswift.coax`](../coax/lib/bitcoin/ellswift.coax) encodes keys as
BIP-324's ElligatorSwift and derives its shared secret, and chapter 67's
[`v2transport.coax`](../coax/lib/bitcoin/v2transport.coax) is the v2
encrypted transport built on it. Chapter 68's
[`handshake.coax`](../coax/lib/bitcoin/handshake.coax) is the `version` and
`verack` handshake with its feature negotiation, over either transport. Chapter 73's
[`cmpctblock.coax`](../coax/lib/bitcoin/cmpctblock.coax) relays blocks as
BIP-152 compact blocks and rebuilds them from a mempool. Chapter 69's
[`addrman.coax`](../coax/lib/bitcoin/addrman.coax) is Core's address manager,
`peers.dat` and the rules for `addr` relay. Chapter 70's
[`connman.coax`](../coax/lib/bitcoin/connman.coax) chooses outbound
connections and evicts inbound peers as Core does. Chapter 71's
[`dos.coax`](../coax/lib/bitcoin/dos.coax) bounds each message,
discourages misbehaving peers and bounds each peer's memory as Core does. Chapter 81's
[`policy.coax`](../coax/lib/bitcoin/policy.coax) applies Bitcoin Core's
standardness rules, which refuse transactions that blocks may still hold.
Chapter 77's [`secp_glv.coax`](../coax/lib/crypto/secp_glv.coax) splits
each verification scalar with secp256k1's endomorphism, which makes ECDSA
and BIP-340 verification about 1.6 times faster.
Chapter 116 pins every vector file in
[`coax/tests/vectors/`](../coax/tests/vectors/SOURCES.md) by source and
SHA-256, and holds Bitcoin Core's files at one revision, `ddea9196`.
Chapter 59's
[`blockindex.coax`](../coax/lib/bitcoin/blockindex.coax) keeps every
accepted header as Core's block index and follows the most-work chain.
Chapter 50's
[`chainparams.coax`](../coax/lib/bitcoin/chainparams.coax) holds each
network's parameters, chapter 49's
[`witness.coax`](../coax/lib/bitcoin/witness.coax) checks a block's BIP-141
witness commitment, and chapter 51's
[`header.coax`](../coax/lib/bitcoin/header.coax) checks a header's time and
version against the chain it extends, with BIP-9's version bits. Chapter
52's [`txcheck.coax`](../coax/lib/bitcoin/txcheck.coax) holds the
transaction checks that run before any script: Core's `CheckTransaction`,
`CheckTxInputs` and the lock-time rule of a block.
Chapter 53's [`sigops.coax`](../coax/lib/bitcoin/sigops.coax) counts
signature operations and checks a block's limits: Core's `CheckBlock`, the
weight rule and the sigop cost `ConnectBlock` charges.
Chapter 54's [`subsidy.coax`](../coax/lib/bitcoin/subsidy.coax) holds the
coinbase rules: Core's `GetBlockSubsidy`, the BIP-34 height and the claim
of subsidy plus fees. Chapter 94's [`psbt.coax`](../coax/lib/bitcoin/psbt.coax)
reads and writes PSBTs (BIP-174, BIP-370 and BIP-371) as Core does, with
the Creator, Updater, Signer, Combiner, Input Finalizer and Transaction
Extractor. Chapter 95's [`coinselect.coax`](../coax/lib/bitcoin/coinselect.coax)
chooses which coins pay for a transaction as Core's wallet does: Branch
and Bound, CoinGrinder, Single Random Draw and Knapsack, judged by the
waste metric, with Core's change target and dust threshold.
Chapter 96's [`bip322.coax`](../coax/lib/bitcoin/bip322.coax)
signs and verifies BIP-322 messages for any address: it builds the
virtual `to_spend` and `to_sign` transactions and runs the address's
script, in the simple, full, proof-of-funds and legacy formats.
Chapter 100's [`multisig.coax`](../coax/lib/bitcoin/multisig.coax)
writes and matches m-of-n scripts as Core does: `OP_CHECKMULTISIG` bare,
under P2SH, P2WSH and P2SH-P2WSH, BIP-67's sorted keys, and BIP-387's
`multi_a` tapscript leaves with their script-path signatures.
Chapter 101's [`musig.coax`](../coax/lib/bitcoin/musig.coax) is BIP-327's
MuSig2: many signers make one BIP-340 signature under one aggregate key,
which can be a Taproot output key. Chapter 102's
[`frost.coax`](../coax/lib/bitcoin/frost.coax) is FROST, the draft
BIP-445: any t of n participants make one BIP-340 signature under one
threshold key, from shares made by a trusted dealer or a Pedersen DKG.
Chapter 103's
[`adaptor.coax`](../coax/lib/bitcoin/adaptor.coax) makes BIP-340
adaptor signatures: a pre-signature locked to a point, completed with
its secret, which the final signature then reveals.
Chapter 121's [`oneseed.coax`](../coax/lib/bitcoin/oneseed.coax) takes an
agent's NIP-06 Nostr key and its BIP-84 and BIP-86 wallets from one BIP-39
phrase, and checks a restore against a card of the `npub` and the wallets'
descriptors.
Chapter 122's [`treasury.coax`](../coax/lib/bitcoin/treasury.coax) holds an
agent's coins under a 2-of-3 of the agent, its owner and a recovery key,
with the recovery key alone after a delay, as P2WSH and as Taproot with a
MuSig2 key path. The agent signs a treasury PSBT only to refill its hot
wallet under the owner's ceiling and fee cap.

Lightning's code is in [`coax/lib/lightning/`](../coax/lib/lightning/).
Chapter 104's [`bolt1.coax`](../coax/lib/lightning/bolt1.coax) reads and
writes BOLT 1's messages: BigSize, TLV streams, `init` with its feature
bits, `error`, `warning`, `ping` and `pong`. Chapter 105's
[`noise.coax`](../coax/lib/lightning/noise.coax) is BOLT 8's Noise
handshake and encrypted transport, with key rotation. Chapter 107's
[`shachain.coax`](../coax/lib/lightning/shachain.coax) makes BOLT 3's
per-commitment secrets and keeps a peer's in 49 slots. Chapter 106's
[`keys.coax`](../coax/lib/lightning/keys.coax) derives each commitment's
keys and the revocation key from the per-commitment point. Chapter 108's
[`commitment.coax`](../coax/lib/lightning/commitment.coax) builds and
signs BOLT 3's funding output, commitments and HTLC transactions, with
and without anchors. Chapter 109's
[`channel.coax`](../coax/lib/lightning/channel.coax) runs BOLT 2's peer
protocol for one channel, from `open_channel` to the mutual close, as a
pure state machine. Chapter 111's
[`onion.coax`](../coax/lib/lightning/onion.coax) builds and peels BOLT 4's
Sphinx onions, wraps and reads error onions, and makes and unblinds
blinded paths. Chapter 114's
[`invoice.coax`](../coax/lib/lightning/invoice.coax) reads, checks and
writes BOLT 11 invoices and recovers the payee from the signature. Chapter 115's
[`offers.coax`](../coax/lib/lightning/offers.coax) reads, checks and
writes BOLT 12 offers, invoice requests, invoices and payer proofs. Chapter 112's
[`gossip.coax`](../coax/lib/lightning/gossip.coax) reads, checks and
writes BOLT 7's gossip and query messages and keeps a channel graph. Chapter 113's
[`pay.coax`](../coax/lib/lightning/pay.coax) finds routes over that graph,
forwards and settles payments across chapter 109's channels, splits them
into parts and retries from error onions.

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
and multisig) are chapters 31–32, 94 and 100, all written. Chapter 104's
ECDSA signer is chapter 20, and its transaction reader is chapter 34's.

## Cryptography still to build

Everything Nostr used is built, and so are the five address types and the
Taproot key tweak, key path, script trees and signature hashes (chapters
41 to 43). Merkle trees with the duplicate-leaf flaw are built in chapter 46
(`coax/lib/bitcoin/merkle.coax`), compact targets in chapter 47
(`coax/lib/bitcoin/pow.coax`), retargeting and chainwork in chapter 48
(`coax/lib/bitcoin/retarget.coax`), and MuHash3072 with the UTXO set's
coin record and hashes in chapter 56 (`coax/lib/bitcoin/muhash.coax`,
`coax/lib/bitcoin/coins.coax`), and BIP-158 Golomb-coded block filters
with BIP-157 filter headers in chapter 74
(`coax/lib/bitcoin/blockfilter.coax`). Bitcoin
and Lightning also need:

- Batch Schnorr verification and tapscript. (ElligatorSwift and the
  BIP-324 transport are built in chapters 66 and 67.)
- Silent payments. (MuSig2 is built in chapter 101,
  `coax/lib/bitcoin/musig.coax`, FROST in chapter 102,
  `coax/lib/bitcoin/frost.coax`, and adaptor signatures in chapter
  103, `coax/lib/bitcoin/adaptor.coax`.)
- Lightning: watching the chain for a channel's transactions (chapter
  110). (BOLT 1's messages and TLVs are built in chapter 104, the BOLT 8 Noise handshake in chapter 105, BOLT 3's keys and
  revocation in chapter 106, shachain in chapter 107, commitment and HTLC
  transactions in chapter 108, BOLT 2 channels in chapter 109, BOLT 4
  onions with blinded paths in chapter 111, BOLT 7 gossip in chapter 112,
  routing and payment in chapter 113, BOLT 11 invoices in chapter 114,
  and BOLT 12 offers in chapter 115.)

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
