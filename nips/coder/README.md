# Coder lane

Specifications authored in this repository for the Coder product. Unlike
`official/` and `block/`, this lane is not synced from upstream — the files
here are the source of truth, and `nips/manifest.json` does not track them.

- [NIP-CJ](NIP-CJ.md) — Coder jobs: ephemeral kind-`25900`/`26900`/`27000`
  request, result, and feedback events between a terminal and a fulfillment
  worker, with NIP-44 payloads and NIP-42 socket authentication.
