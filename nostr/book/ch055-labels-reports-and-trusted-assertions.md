# 55. Labels, reports and trusted assertions

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Clients need signals from others: labels, reports and computed scores. This
chapter reads them as statements by their signers, never as facts.

Part X, Every NIP. Result: NIP-32 kind 1985 labels in `L` namespaces, NIP-56
kind 1984 reports with report types, and NIP-85 kinds 30382–30385 assertions
from declared providers (kind 10040).

## What this chapter teaches

- NIP-32: `["L", namespace]` and `["l", value, namespace]`; self-labels on any
  event.
- NIP-56: `p` and `e` tags with types `nudity`, `malware`, `profanity`,
  `illegal`, `spam`, `impersonation`, `other`.
- NIP-85: assertion kinds per subject type, and kind 10040 where a user names
  the providers they trust.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)

## Done means

- Labels are grouped by namespace and signer.
- An assertion from a provider the user did not name is not shown as trusted.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-32](../nips/official/32.md)
- [NIP-56](../nips/official/56.md)
- [NIP-85](../nips/official/85.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
