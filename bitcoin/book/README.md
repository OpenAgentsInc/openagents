# Bitcoin From Scratch

Building Bitcoin in [Coax](../../coax/book/README.md).

> **Status: design draft.** No chapter is written. The [plan](PLAN.md) lists
> the proposed chapters, the specifications each one implements and the code
> that already exists. [`bitcoin/README.md`](../README.md) explains why the
> book exists and which Nostr chapters move here.

This book builds Bitcoin in Coax from bytes up. It starts with big integers
and constant-time rules, builds SHA-256, RIPEMD-160, HMAC, secp256k1, ECDSA
and BIP-340 Schnorr from their specifications, then Base58Check, Bech32m,
seed phrases and HD keys. On those it builds transactions, the script
interpreter, every signature hash, Taproot, block headers and proof of work,
the P2P layer with BIP-324 encryption, PSBTs and wallets, multi-party
signatures (MuSig2, FROST, adaptor signatures) and the cryptography of
Lightning. The last parts check the stack against published vectors and other
implementations, and show where OpenAgents uses it.

[Nostr From Scratch](../../nostr/book/README.md) is the companion book. Nostr
uses Bitcoin's curve, hash and signature, so read Parts I–IV here first.

## Rules

- No outside cryptography or Bitcoin library. libsecp256k1 and Bitcoin Core
  appear only in the differential tests of Part XIV, never in the code the
  book builds.
- Secrets never decide a branch or an index (chapter 3).
- Every chapter names its BIP, BOLT or RFC and the test vectors that pin it.
- Bitcoin only: Lightning and on-chain Bitcoin are built; Cashu, Fedimint
  and other chains are not.
- The consensus chapters check data against vectors. They are not a node to
  replace Bitcoin Core.

You need the Coax language (its book's Parts I–III) and a built compiler
(`coax/build.sh`).

## Contents

See the [plan](PLAN.md). A `SUMMARY.md` and `chapters.json` registry come
with the first written chapter, in the same form as the Nostr book's.
