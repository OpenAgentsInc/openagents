# 71. Lightning zaps and goals

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Zaps are Lightning payments tied to Nostr events. This chapter builds the
request and the receipt check, under the Bitcoin-only rule.

Part X, Every NIP. Result: NIP-57 zap requests (9734) sent to an LNURL server
and zap receipts (9735) whose bolt11 description hash commits to the request,
and NIP-75 zap goals (9041).

## What this chapter teaches

- Profile `lud16` to an LNURL-pay endpoint with `allowsNostr` and
  `nostrPubkey`.
- Kind 9734: `relays`, `amount` in millisats, `lnurl`, `p`, optional `e` or
  `a`; sent as a URL parameter, never published.
- Kind 9735 receipt from `nostrPubkey`: `bolt11`, `description` (the request
  JSON), optional `preimage`.
- Checks: receipt signer equals `nostrPubkey`; the invoice's description hash
  equals SHA-256 of `description`; amounts agree.
- NIP-75: kind 9041 with `amount`, `relays`, optional `closed_at`.
- A receipt is a claim by the recipient's server, not proof of payment.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)
- [45. Profiles, follows and names](ch045-profiles-follows-and-names.md)

## Done means

- A receipt whose description hash does not match refuses.
- Goal progress sums only valid receipts.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-57](../nips/official/57.md)
- [NIP-75](../nips/official/75.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
