# ARC Upstream Targets
Status: canonical first-pass upstream benchmark and protocol target set
Date: 2026-03-15

## Purpose

This document freezes the upstream benchmark and protocol versions the ARC
subtree targets first.

It exists to stop three kinds of drift:

- "current upstream" changing underneath in-flight ARC port work
- benchmark and protocol work silently targeting different source snapshots
- parity claims being made without one named upstream baseline

This is a first-pass target freeze, not a promise of permanent compatibility
with every later upstream change.

## Supported First Target Set

The ARC subtree currently targets the following upstream source snapshots first.

### ARC-AGI-1 task baseline

- repo: `fchollet/ARC-AGI`
- default branch: `master`
- repo commit: `399030444e0ab0cc8b4e199870fb20b863846f34`
- reviewed entrypoint blob: `README.md`
  - blob SHA: `9db5b0b71ffb55f0af5556ca4e354a1b05e4cc30`

This is the canonical first-pass baseline for ARC-AGI-1 task and dataset
expectations.

### ARC-AGI-2 task baseline

- repo: `arcprize/ARC-AGI-2`
- default branch: `main`
- repo commit: `f3283f727488ad98fe575ea6a5ac981e4a188e49`
- reviewed entrypoint blob: `readme.md`
  - blob SHA: `439fae268d64b727aba88a8aac601ae341ce372b`

This is the canonical first-pass baseline for ARC-AGI-2 dataset semantics.

### ARC-AGI-3 protocol and scoring baseline

- repo: `arcprize/docs`
- default branch: `main`
- repo commit: `05a6e0d43754f4ad0ef2a519a5a4d5af6725d3bf`

Reviewed protocol/scoring files:

- `arc3v1.yaml`
  - blob SHA: `4d4f36d78c174511166c8eb6752fd8a27f9657f3`
- `scoring.md`
  - blob SHA: `1cbc2f1cf99a77b741f8d21b35d34ba0a45b20c3`
- `rest_overview.mdx`
  - blob SHA: `11a2d2fbc6296712c30b14ad77ee698446a4e457`
- `methodology.mdx`
  - blob SHA: `567d5532420b7321579b9ed31b253e2546f653a4`

This is the canonical first-pass baseline for:

- ARC-AGI-3 REST and compatibility-server behavior
- operation-mode and competition-mode policy interpretation
- interactive scoring policy and methodology language

### Benchmark runtime baseline

- repo: `arcprize/arc-agi-benchmarking`
- default branch: `main`
- repo commit: `7a2efa0f65a55a57bd8da08ef02d826e882cfec8`
- reviewed entrypoint blob: `README.md`
  - blob SHA: `14c0d6eacc85a85e92499e61f1d8c8019117c09d`

This is the canonical first-pass baseline for benchmark-run, checkpoint, and
benchmark-summary behavior.

## ARCEngine Freeze Rule

The 2026-03-15 ARC audit reviewed an ARCEngine source snapshot from a local
`arcprize` checkout, but that same local checkout is not present in this repo
workspace today.

Until an equivalent public ARCEngine source pin is added to this document, the
engine-semantics freeze is:

- the reviewed source set named in
  `docs/audits/2026-03-15-arcprize-rust-port-and-psionic-integration-audit.md`
- interpreted as the canonical first-pass engine parity target for
  `ARC-201` and `ARC-202`

This exception is explicit on purpose. Engine parity work MUST NOT quietly
float to a different upstream source set without updating this document.

## Update Rule

If ARC work needs to target a different upstream commit, blob, or protocol
revision, update this document in the same change and explain:

- which upstream target changed
- which ARC crates or fixtures are affected
- whether existing parity artifacts remain valid or must be regenerated

## Non-Goals

- claiming compatibility with every newer ARC Prize revision automatically
- pretending one upstream repo covers every ARC concern
- hiding engine/protocol drift behind vague "latest upstream" language
