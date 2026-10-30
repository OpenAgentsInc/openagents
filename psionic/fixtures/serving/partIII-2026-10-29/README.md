# Part III observations — 2026-10-29

Owned decision workloads and raw observations for chapters 14, 15, 17 and 18.
See the [verification record](../../../../docs/verification/psionic-partIII-2026-10-29.md)
for producers, commands, interpretation and limitations. No private user
requests or answers are retained; every request here is a published owned
fixture.

| Artifact | Content |
| --- | --- |
| `focused-checks.log`, `linux-checks.log` | Successful focused Coax/native/sanitizer/book and Linux operational checks |
| `authentication/` | Raw production authentication-only logs and parsed page/hash observations; no model inference |
| `cpu-prefix-pairs/` | 100 alternating pairs for each of three toy CPU workloads, exact requests/answers and private phase records |
| `cuda/` | 100 original Elephant observations at each of three lengths, raw phases, retained answers and actual producer metadata |
| `metal/` | 100 original Elephant observations for the seeded question and actual producer metadata |
| `metal-prefix-before/`, `metal-prefix-after/` | Ten-sample original three-question comparison, same binary with/without forks |
| `metal-operations/` | Original model busy, per-request fork and cancellation/recovery evidence |
| `managed/` | Actual user-unit readiness, original golden answer, SIGKILL/re-warm/restart and final clean stop |
| `host-probe.json` | Five actual copy/dot timing intervals with byte/operation counts |
| `manifest.json` | SHA-256 and length of every retained artifact except this manifest |

`service_observations.py` independently replays ranks, request digests, reuse
counters, parity, memory arithmetic and manifest integrity without a model.
The book/verification record interprets these shared-host observations; they
are not a public SLA, original CPU throughput claim or full mutation gate.
Build records retain the actual historical binary identities. Later build
tooling additionally binds the generated inventory header; old observations
are never relabeled as coming from that later binary.
