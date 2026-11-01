# 104. Agent treasuries and a network of keys

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

An agent that earns and spends bitcoin needs somewhere safe to keep it. One
hot key on the agent, or one wallet host, is a single point of failure. This
chapter builds the alternative: a wallet whose coins move only when enough
independent key agents sign, coordinated over Nostr.

Part XII, Nostr in OpenAgents. Result: NIP-KEYS wallets, enrollments, PSBT
signing requests and receipts, rotation and recovery carried as private `3188`
artifacts, with an attested co-signer that enforces a NIP-SOV treasury policy
with its own key.

## What this chapter teaches

- Why NIP-SOV's guardian quorum is a rule the host enforces, and why a
  multisig wallet moves that quorum into Bitcoin script.
- Wallet descriptors (BIP-380) with key origins, recomputed from a wallet's
  paths and enrollments, and the composition rules: no one operator meets a
  threshold, and a sovereign agent never holds a primary majority.
- Enrollment: an xpub, a BIP-322 possession proof bound to the wallet ID, and
  a signed independence declaration.
- Signing: a PSBT (BIP-174, BIP-370) whose outputs, change and fee the signer
  recomputes before it signs, with nonce idempotency and retained receipts.
- The attested co-signer: an ATT workload that checks the SOV treasury
  policy and a large-spend delay before releasing a partial signature.
- Recovery with miniscript timelocks, liveness and refresh; rotation sweeps
  that route around a failed key agent.
- The hot and cold split: a capped Lightning balance for daily spending and a
  multisig reserve.

## What exists today

- `coax/lib/crypto/bip32.coax` derives BIP-32 private and public child keys
  for NIP-06 ([chapter 46](ch046-keys-from-a-seed-phrase.md)). There is no
  xpub serialization, descriptor, PSBT or Bitcoin transaction code in Coax.
- `crates/sov-custody` and `crates/sov-treasury` implement SOV's NIP-46
  identity custody, guardian quorum and Lightning treasury in Rust. Nothing
  implements NIP-KEYS.

## Dependencies

- [13. Sign with Schnorr](ch013-sign-with-schnorr.md)
- [46. Keys from a seed phrase](ch046-keys-from-a-seed-phrase.md)
- [79. Agents, skills and automation](ch079-agents-skills-and-automation.md)
- [83. Attested work and sealed data](ch083-attested-work-and-sealed-data.md)

## Done means

- Three software key agents on signet sign a 2-of-3 spend from requests
  carried over a Coax relay, and a PSBT whose outputs differ from its request
  is refused by every signer.
- A coordinator outage is recovered from one key agent's copy of the
  descriptor with an independent wallet tool.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [OpenAgents NIP-KEYS](../nips/openagents/NIP-KEYS.md)
- [OpenAgents NIP-SOV](../nips/openagents/NIP-SOV.md)
- [OpenAgents NIP-ATT](../nips/openagents/NIP-ATT.md)
- Research: [A network of keys](../../docs/research/2026-11-01-network-of-keys.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
