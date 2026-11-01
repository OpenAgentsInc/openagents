# 98. Projects on Nostr

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

The OpenAgents work queue lives on Nostr. This chapter shows how the website
reads it with this book's code.

Part XII, Nostr in OpenAgents. Result: OpenAgents' project pages fold NIP-34
repositories, issues and statuses and NIP-22 comments from the relay, verify
each event in the browser, and replace a snapshot only with a complete bounded
capture.

## What this chapter teaches

- The project fold: tables built from repositories, issues, statuses, comments
  and ordering records.
- Live updates with verification of each event before trusted admission.
- Catch-up and the rule that a stalled capture keeps the previous view.

## What exists today

- `apps/openagents-web/project-fold.coax` and `project-live.coax`.
- Record:
  [2026-10-30-newland-coax-nostr.md](../../docs/verification/2026-10-30-newland-coax-nostr.md).

## Dependencies

- [63. Git on Nostr](ch063-git-on-nostr.md)
- [30. Subscriptions and EOSE](ch030-subscriptions-and-eose.md)
- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)

## Done means

- A forged or out-of-scope event never enters the verified view.
- The fold of a captured relay matches its recorded tables.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-34](../nips/official/34.md)
- [NIP-22](../nips/official/22.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
