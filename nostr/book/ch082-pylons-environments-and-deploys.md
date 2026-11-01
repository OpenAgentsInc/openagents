# 82. Pylons, environments and deploys

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

Pylons are the machines that serve models. They announce themselves with
signed beacons. This chapter builds the beacon reader and writer and the
related environment and deploy records.

Part X, Every NIP. Result: NIP-PYLON beacons (30200), service receipts (3201)
and pool aggregates (30201), NIP-ENV environment leases, and NIP-DEPLOY public
deploy status (3205, 30440).

## What this chapter teaches

- PYLON: beacon v1 and model-group v2, receipts, pool aggregates, validation
  and freshness.
- ENV: definitions, lease admission, expiry and binding before dispatch.
- DEPLOY: content, tags and reader rules.

## What exists today

- `psionic/lib/pylon/beacon.coax` and `psionic/lib/mesh/receipt.coax` handle
  beacons and receipts; the homepage verifies beacons in the browser
  ([2026-10-30-newland-coax-nostr.md](../../docs/verification/2026-10-30-newland-coax-nostr.md)).
  ENV and DEPLOY are not implemented.

## Dependencies

- [80. Jobs, labor and markets](ch080-jobs-labor-and-markets.md)
- [75. Hosts and reachability](ch075-hosts-and-reachability.md)

## Done means

- An expired beacon contributes no capacity.
- A receipt signed by a key other than the beacon's refuses.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [OpenAgents NIP-PYLON](../nips/openagents/NIP-PYLON.md)
- [OpenAgents NIP-ENV](../nips/openagents/NIP-ENV.md)
- [OpenAgents NIP-DEPLOY](../nips/openagents/NIP-DEPLOY.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
