# Bitcoin

Bitcoin in [Coax](../coax/README.md): the cryptography, encodings,
transactions, script, consensus checks, network and payment protocols that
OpenAgents' money runs on.

This folder holds the design for a sixth book, **Bitcoin From Scratch**, the
companion to [Nostr From Scratch](../nostr/book/README.md). Nothing here is
built yet. The [book](book/README.md) states the scope and the
[plan](book/PLAN.md) lists the proposed chapters. This README records why the
book should exist and how it changes the Nostr book.

## Why a separate book

Most of the cryptography in Nostr From Scratch is Bitcoin's. Nostr chose
secp256k1, SHA-256, BIP-340 Schnorr signatures, Bech32 and BIP-39/BIP-32 seed
phrases because Bitcoin already had them. Parts I–IV of the Nostr book
(chapters 1–15) teach what Bitcoin specifies, and chapter 46 teaches BIP-39
and BIP-32 to reach one NIP-06 path. Chapter 104 has to teach PSBTs, BIP-143
and BIP-341 sighashes, ECDSA and P2WSH multisig inside a Nostr chapter.

Bitcoin code is already spread across the repository with no book that owns
it:

| Code | What it is | Taught today in |
| --- | --- | --- |
| `coax/lib/crypto/secp_*.coax`, `schnorr*.coax` | secp256k1 and BIP-340 | Nostr chapters 7–13 |
| `coax/lib/crypto/sha256*.coax`, `hmac*.coax`, `hkdf*.coax` | Hashes and MACs | Nostr chapters 4–6 |
| `coax/lib/crypto/bip39.coax`, `bip32.coax`, `pbkdf2*.coax`, `sha512.coax` | Seed phrases and HD keys | Nostr chapter 46 |
| `coax/lib/crypto/ecdsa_sign.coax` | RFC 6979 ECDSA, low-S, DER | Nostr chapter 104 |
| `coax/lib/bitcoin/sighash.coax` | Transaction reader, BIP-143 and BIP-341 sighashes, Taproot key tweak | Nostr chapter 104 |
| `coax/lib/crypto/poly1305*.coax`, `chacha20*.coax` | ChaCha20 and Poly1305 | Nostr chapter 18 (ChaCha20 only) |
| `coax/lib/nostr/nipkeys*.coax` | Base58Check xpubs, BIP-32 child keys, PSBT v2, descriptors | Nostr chapter 104 |
| `coax/lib/nostr/nipa3.coax` | Bech32m and segwit address checks | Nostr Part X |
| `coax/lib/nostr/nip57.coax`, `nip47.coax`, `coax/lib/x402/` | BOLT 11 invoices, Wallet Connect, x402 | Nostr chapters 71–73, 86 |

A Bitcoin book gives that code one home, one set of test vectors and one
place to fill the gaps. Nostr then reads like a protocol book: it imports a
finished signature scheme and spends its pages on events, relays and NIPs.

## The rule for what moves

**If a BIP or BOLT specifies it, or Bitcoin needs it, the Bitcoin book
teaches it. If only a NIP needs it, the Nostr book teaches it.**

Applied to the current Nostr chapters:

| Nostr chapter | Goes to | Why |
| --- | --- | --- |
| 1. Bytes, hex and base64 | Bitcoin 1 (bytes, hex); base64 stays in Nostr | Bitcoin needs hex and little-endian integers; base64 is a Nostr (NIP-44) need |
| 2. Big integers in fixed limbs | Bitcoin 2 | secp256k1 arithmetic |
| 3. Constant-time rules and secrets | Bitcoin 3 | Every key in both books |
| 4. SHA-256 | Bitcoin 4 | Txids, block hashes, PoW |
| 5. HMAC-SHA256 | Bitcoin 8 | RFC 6979, BIP-32 (HMAC-SHA512) |
| 6. HKDF | Bitcoin 9 | BIP-324 and BOLT 8 use HKDF-SHA256 |
| 7–10. Field, scalars, group law, scalar multiplication | Bitcoin 11–14 | The curve is Bitcoin's |
| 11. Verify a Schnorr signature | Bitcoin 21 | BIP-340 is a Bitcoin BIP |
| 12. Keys and randomness | Bitcoin 16 | |
| 13. Sign with Schnorr | Bitcoin 22 | BIP-340 |
| 14. Shared secrets with ECDH | Stays in Nostr | NIP-04/NIP-44's x-only ECDH; built on Bitcoin 14. Bitcoin teaches its own ECDH variants (BIP-324, BIP-352, BOLT 4, BOLT 8) |
| 15. Bech32 | Bitcoin 25 | BIP-173 and BIP-350 |
| 16. NIP-19 entities | Stays | Uses Bitcoin 25 |
| 17. JSON you can sign | Stays | |
| 18. ChaCha20 | Bitcoin 52 | BIP-324, BOLT 8 and BOLT 4 use it; NIP-44 imports it |
| 19, 20. NIP-04, NIP-44 | Stay | |
| 46. Keys from a seed phrase | BIP-39 and BIP-32 go to Bitcoin 28–30; Nostr keeps the NIP-06 path | NIP-06 is one derivation path |
| 56. Proof of work | Stays | NIP-13 counts leading zero bits; Bitcoin's PoW is its own chapter (44) |
| 71–73, 86. Zaps, Wallet Connect, payment targets, paid requests | Stay; BOLT 11 decoding goes to Bitcoin 69 | The NIPs stay; the invoice format is a BOLT |
| 104. Agent treasuries | Bitcoin half (PSBT, descriptors, sighashes, ECDSA, P2WSH) goes to Bitcoin 18, 27, 35, 40, 56, 61; NIP-KEYS records and relay transport stay | |

After the move, Nostr Parts I–IV become one chapter that names what it uses
from Bitcoin From Scratch, and the Nostr book starts at encodings and the
event model. Reading order: Bitcoin Parts I–IV, then Nostr.

## Cryptography Bitcoin needs that we haven't built

Everything in Nostr's Parts I–III carries over. Bitcoin also needs:

- **RIPEMD-160 and HASH160** for P2PKH, P2SH, P2WPKH and BIP-32 fingerprints.
- **SHA-256d and tagged hashes** (txids, block hashes, P2P checksums;
  BIP-340/341 tags are already built).
- **SipHash-2-4** for BIP-152 short IDs and BIP-158 filters.
- **ECDSA verification**, public key recovery (signed messages, BOLT 11),
  strict DER (BIP-66) and low-S (BIP-146). Signing exists.
- **Base58Check** encoding as a first-class codec, not a NIP helper.
- **Batch Schnorr verification** (BIP-340's batch equation).
- **Taproot script trees**: tapleaf and tapbranch hashes, control blocks,
  BIP-342 tapscript sighash. The key path exists.
- **MuHash3072**, Bitcoin Core's rolling UTXO-set hash: multiplication
  modulo 2^3072 − 1103717, a second big-integer workload.
- **Merkle trees** with the duplicate-last-leaf flaw (CVE-2012-2459), and the
  segwit witness commitment.
- **Compact targets (nBits), retargeting and chainwork**: 256-bit unsigned
  arithmetic.
- **ElligatorSwift** encoding and its x-only ECDH for BIP-324, plus
  **FSChaCha20Poly1305** (ChaCha20-Poly1305 with rekeying). Poly1305 exists.
- **Golomb-Rice coded sets** for BIP-158 compact block filters.
- **MuSig2** (BIP-327) key aggregation and two-round signing, with its nonce
  rules.
- **FROST** threshold Schnorr (draft BIPs), which also describes the
  signing model behind Spark.
- **Adaptor signatures**, the primitive under PTLCs and atomic swaps.
- **Silent payments** (BIP-352): ECDH, tagged hashes and output scanning.
- **Lightning**: BOLT 8's Noise_XK handshake, BOLT 3 per-commitment keys,
  revocation keys and shachain storage, BOLT 4 Sphinx onions with blinded
  paths, BOLT 11 and BOLT 12 (Schnorr over a Merkle root of TLVs).

## Scope

In scope: the cryptography above; transaction and script serialization; a
script interpreter that passes Bitcoin Core's `script_tests.json`; every
sighash; headers, proof of work and a bounded block-validation profile; the
P2P message layer and BIP-324; PSBT, descriptors and wallets; the Lightning
cryptography; and how OpenAgents uses Bitcoin (the Spark wallet, agent
treasuries, x402).

Out of scope, following the repository's Bitcoin-only rule
([Nostr chapter 74](../nostr/book/ch074-cashu-and-fedimint-are-excluded.md)):
Cashu and Fedimint blind signatures, other chains, and their cryptography.

The book's consensus chapters check our own data against published vectors.
They do not make a node to replace Bitcoin Core: re-implementing consensus is
a known way to fork yourself off the network. A full-node chapter, if we
write one, states that boundary in its title.

## Moving chapters, later

None of this is done yet. When it is:

1. Move each chapter's text and its `chapters.json` entry, keeping the
   chapter's code where it is (`coax/lib/crypto/` stays shared; new Bitcoin
   code goes in `coax/lib/bitcoin/`).
2. Give `chapters.json` dependencies a `bitcoin:N` form, as Nostr's
   `nostr:N` already works, and let `psionic/tests/book.py` and
   `coax/book/check-structure.py` check across books.
3. Add the book to `scripts/publish-coax-book.py`. Its redirects only match
   pages inside one book, so a moved Nostr page needs an explicit redirect to
   its Bitcoin URL, or it falls back to the Nostr book's first page.
4. Update [Paths through the books](../docs/books/paths.md) and the root
   README's book list.

## Decisions to make

- **Title.** *Bitcoin From Scratch*, matching Nostr's.
- **Stub pages or renumbering.** Recommended: Nostr keeps one chapter for
  the imported cryptography and renumbers, with redirects from the old URLs.
- **How far consensus goes.** Recommended: script, sighash, headers and a
  bounded block-validation profile; no initial block download.
- **Lightning.** Recommended: the cryptography and message formats here; a
  full channel state machine belongs with `tap-ldk` and the wallet work.
