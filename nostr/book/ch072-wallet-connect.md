# 72. Wallet Connect

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

NWC lets an app ask a wallet to pay. This chapter builds the client and a
service that fronts the Spark wallet's rules in `verse/wallet.coax`, Lightning
only.

Part X, Every NIP. Result: NIP-47 Nostr Wallet Connect:
`nostr+walletconnect://` URIs, info (13194), requests (23194) and responses
(23195) encrypted with NIP-44, and the commands `pay_invoice`, `make_invoice`,
`get_balance` and others.

## What this chapter teaches

- URI: wallet pubkey, `relay`, `secret`, optional `lud16`.
- Kind 13194 lists supported commands and `encryption` `nip44_v2`.
- Commands: `pay_invoice`, `multi_pay_invoice`, `pay_keysend`, `make_invoice`,
  `lookup_invoice`, `list_transactions`, `get_balance`, `get_info`.
- Errors: `RATE_LIMITED`, `NOT_IMPLEMENTED`, `INSUFFICIENT_BALANCE`,
  `QUOTA_EXCEEDED`, `RESTRICTED`, `UNAUTHORIZED`, `INTERNAL`, `OTHER`.
- Budgets and permissions per connection, decided in an `#authority` file.

## What exists today

- Nothing in Coax for NIP-47. The wallet's payment rules are in
  [verse/wallet.coax](../../verse/wallet.coax) (`#authority`); Swift talks to
  the Breez Spark SDK.

## Dependencies

- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)
- [31. Publish and wait for OK](ch031-publish-and-wait-for-ok.md)

## Done means

- Every command's request and response round-trips with an independent client.
- A payment over the connection's budget refuses with `QUOTA_EXCEEDED`.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-47](../nips/official/47.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
