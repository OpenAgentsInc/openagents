# 86. Lightning payments for requests

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

Some OpenAgents routes charge per request. NIP-X402 binds the payment to the
request with Lightning only.

Part X, Every NIP. Result: NIP-X402: x402 v2 over HTTP with the three base64
JSON headers, Bitcoin mainnet Lightning with a bolt11 invoice in millisats,
and the `openagents:spark-receiver:1` settlement profile.

## What this chapter teaches

- `PAYMENT-REQUIRED` terms, `PAYMENT-SIGNATURE` proof, `PAYMENT-RESPONSE`
  settlement.
- Accepted terms only: version 2, scheme `exact`, network
  `lnbtc:000000000019d6689c085ae165831e93`, a bolt11 invoice.
- Settlement: the receiver's Spark record of the invoice paid and a preimage
  that hashes to the payment hash.

## What exists today

- `coax/lib/x402/x402.coax` implements the client's half.

## Dependencies

- [49. HTTP auth](ch049-http-auth.md)
- [71. Lightning zaps and goals](ch071-lightning-zaps-and-goals.md)

## Done means

- Any other scheme or network refuses.
- A preimage that does not hash to the payment hash refuses.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [OpenAgents NIP-X402](../nips/openagents/NIP-X402.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
