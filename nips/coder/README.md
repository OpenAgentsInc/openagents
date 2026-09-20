# Coder lane

Specifications authored in this repository for the Coder product. Unlike
`official/` and `block/`, this lane is not synced from upstream — the files
here are the source of truth, and `nips/manifest.json` does not track them.

- [NIP-CJ](NIP-CJ.md) — Coder jobs: ephemeral kind-`25900`/`26900`/`27000`
  request, result, and feedback events between a terminal and a fulfillment
  worker, with NIP-44 payloads and NIP-42 socket authentication.
- [NIP-CC](NIP-CC.md) — Coder capabilities and programs: addressable
  `30180` manifests saying how to drive an executor and which bounds it will
  silently ignore, `30181` operator policies saying which to prefer, and
  `30182` programs, the composable unit of named steps and per-step bounds
  the decision engine selects. Local presence stays local and is
  deliberately not an event.
