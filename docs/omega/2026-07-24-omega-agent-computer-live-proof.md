# Omega Agent Computer live proof (AC-03)

- Date: 2026-07-24
- Packet: `OMEGA-AC-03`
- Omega issue: [OpenAgentsInc/omega#30](https://github.com/OpenAgentsInc/omega/issues/30)
- Live run: `ccs_0ce9a05c48134942864e0aa646c46158`
- Writeback: [OpenAgentsInc/openagents#9213](https://github.com/OpenAgentsInc/openagents/pull/9213)
- Machine evidence: [2026-07-24-omega-ac03-live-proof.json](./2026-07-24-omega-ac03-live-proof.json)

## Result

Omega dispatched a real coding turn through `omega-effectd` and the shared
`openagents_cloud` environment. A live `cloud-gcp` Agent Computer microVM
checked out immutable commit `1b13175cd516c71a5b9c5cacc32459b1e233fd1c`.
It ran Codex, staged one repository change, and passed the pinned verifier. It
recorded exact token usage, opened a pull request, and completed verified
teardown.

The Agent Computer writeback commit `e4de4450ec3c1a63d285ecf84fb1d327e43597ff`
was merged as `ecabc813e5698f32c1246b26bad80b18c8a8002d` through
[pull request #9213](https://github.com/OpenAgentsInc/openagents/pull/9213).

## Gate matrix

| Gate | Status | Evidence |
| --- | --- | --- |
| Omega dispatch | green | The live objective digest matches the `omega-effectd` AC-03 driver exactly |
| Live Firecracker microVM | green | `cloud-gcp`, `provisionerKind=live`, exit 0, provision receipt `receipt.cloud.gce.provision.26f11e3429d0df04` |
| Staged change | green | One file, three insertions, 382-byte content-addressed staged diff |
| Verifier | green | `git diff --cached --check`, exit 0, `verification.recorded` |
| Exact usage | green | Exact Codex usage row with 95,166 total tokens and 90 measured VM-seconds |
| Writeback | green | Recorded owner-scoped writeback, PR #9213, immutable commit and merge commit |
| Cleanup | green | Artifacts extracted, scratch wiped, microVM destroyed, teardown verified |
| Shared Sarah authority | green | Same `cloud-gcp` Agent Computer placement authority and `cloud.gce` receipt class as Sarah #9191 |

## Immutable evidence

- Result artifact: `artifact.sha256.43f319b58d0c02ae3e0e9dfcc9e69184430ace343db519c15d7d5ba9400d9f27`
- Staged-diff artifact: `artifact.sha256.6777ed2199fbcab502b6de587db0c9443b8744a79f2ea5c13e69f5c387e16376`
- Verification command ref: `verify.agent-computer.public_cloud_coding_request`
- Usage ref: `usage.codex_app_server.5439b84c-7bb6-429d-acf7-5d69a3c835b9`
- Token event ref: `event.inference.served-tokens.khala-cloud-runtime.c86e266dc8bb48404b6ba3327bbc941e`
- Resource receipt: `receipt.cloud.gce.resource_usage.26f11e3429d0df04`
- Writeback event: `event.private.agent_computer.writeback.16eee685-1f1f-4053-86c7-5eadb0752d7b`
- Cleanup receipt: `sha256:86f28ab5acc779ae70ec0515542c9dde594f1abf51033f8189d5cd01ae02cd56`
- Scratch-wipe receipt: `receipt.cloud.gce.scratch_wipe.26f11e3429d0df04`
- MicroVM-destroy receipt: `receipt.cloud.gce.microvm_destroy.26f11e3429d0df04`

Exact Codex usage was 94,650 input, 465 output, 51 reasoning, 81,920
cache-read, and 95,166 total tokens. The durable usage event was inserted. The
subscription-backed Codex turn did not create a separate token charge.

## Independent verification and production

An independent verifier matched both extracted artifact SHA-256 values to the
control-plane refs and inspected the public-safe retained result. The verifier
also confirmed the merged GitHub writeback and reran four focused test files.
All 81 tests passed.

Production revision `openagents-monolith-00241-drv` was Ready and Active with
100% traffic for the live run. Sarah and Omega do not have separate capacity
planes. Both use the same server-owned `cloud-gcp` Agent Computer placement
authority. They also use the same provision, resource-usage, artifact, and
cleanup receipt classes.

This proof completes `OMEGA-AC-03`. Closed OpenAgents issues `#9190` and `#9191`
remain closed. Later Full Auto cloud-lane work remains scoped to `OMEGA-FA-*`.
