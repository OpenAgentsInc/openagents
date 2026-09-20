# OpenAgents lane

Specifications authored in this repository. Unlike `official/` and `block/`,
this lane is not synced from upstream — the files here are the source of
truth, and `nips/manifest.json` does not track them.

- [NIP-PRO](NIP-PRO.md) — Programs: addressable `30182` state machines of
  named steps with per-step bounds. A program carries no code, composes by
  reference under narrowing bounds, and says nothing about where it runs.
  The general primitive; nothing in it is specific to an agent or a product.
- [NIP-CAP](NIP-CAP.md) — Capabilities: `30180` manifests saying how to
  drive an executor and which bounds it will silently ignore, and `30181`
  operator policies saying which to prefer. Local presence stays local and
  is deliberately not an event.
- [NIP-CJ](NIP-CJ.md) — Coder jobs: ephemeral kind-`25900`/`26900`/`27000`
  request, result, and feedback events between a terminal and a fulfillment
  worker, with NIP-44 payloads and NIP-42 socket authentication.
