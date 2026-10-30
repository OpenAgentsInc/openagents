# Original GPT-OSS20B answers through a cloud Pylon

On 2026-10-30, the actual original GPT-OSS20B checkpoint was downloaded,
authenticated and served by the existing Coax CUDA Responses runtime on
`psionic-rtx6000-pylon` in `openagentsgemini/us-central1-b`. The new native
Coax Pylon completed real signed NIP-90 requests through
`wss://relay.openagents.com`.

The [sanitized receipt](../../psionic/fixtures/cloud-pylons/2026-10-30/gptoss-nip90.json)
records exact checkpoint assets, generated C/source hashes, native binary
hashes, public provider/request/result identifiers and unit state. Raw model
reasoning, private demo labels, bearer keys and authentication headers are
excluded.

## What changed

`pylon/text.coax` owns admission of the bounded free public-text NIP-90
profile and extraction of completed assistant final output. The Pylon's
existing owned Nostr signing, relay/TLS, NIP-42, queue, reconnect/outbox and
durable provider generation are reused. The new `--nip90` branch calls the
actual native `/v1/responses` engine with a private file-supplied bearer and
advertises the existing `nip90-5050` lane. Default decision mode stays the
existing SystemOne path.

`text-probe-policy.coax` owns one-shot buyer pins, input/deadline limits,
standard free targeted tags and feedback handling. The native buyer waits
for the matching fresh signed beacon, signs a unique request and accepts
only the pinned provider's verified signed correlated final result with
the exact checkpoint identity, zero amount and expected control word.

The profile uses existing NIP-90 event kinds 5050/6050/7000 and NIP-01
envelopes. NIP-PYLON and the proto README document the implementation
subset; no replacement event schema or SystemOne payload was introduced.

## Real model and infrastructure

The original checkpoint is `openai/gpt-oss-20b` revision
`6cee5e81ee83917806bbde320786a8fb61efebee`. Its original MXFP4 tensor file
is 13,761,300,984 bytes, SHA-256
`3340a61d1a0391e8c5b5d3463d18d4c48129a84bbc04a554c762c99020aa06ed`.
The complete pinned config/dtype/weight bytes and original tensor descriptors
are authenticated by the existing Coax reader before its listener opens.
No GPT model arithmetic or GPU kernels changed in this slice.

The runtime retains its 16 GiB budget on the existing 96 GB RTX PRO card.
Measured combined use after loading is 80,783 MiB of 97,887 MiB, alongside
Elephant and Mammoth. No new GPU/VM was created. All six plain-model units
were active at 21:11:32 UTC; both retained model providers were untouched.
The retired L4 stayed stopped. The confidential worker remains a separate
provider on its existing TDX VM, owned by the fleet monitor.

New provider:
`23a62947ce9913e8d61d166fa72a2e4d2e11b153f9c47c5ee37e583ec702f599`,
slug `oct30-gcloud-gptoss20b`, model identity
`gpt-oss-20b@sha256:3340a61d1a0391e8c5b5d3463d18d4c48129a84bbc04a554c762c99020aa06ed`.
The Pylon's real final-output PONG startup doctor completed in 323 ms before
its generation 2 online beacon. Refresh is 60 seconds and validity 180 seconds.
These are declared/checkpoint-checked identities; this plain GPU lane does
not assert remote hardware/model attestation or bypass the production-key
audit gate. It uses a fresh retained private demo identity and free work.

## Focused checks and native proofs

| Qualification | Result |
| --- | --- |
| Text authority checks | 134/134 in debug and release |
| Text authority root-only mutations | 406/406 killed, zero survivors |
| Probe authority checks | 49/49 |
| Probe authority root-only mutations | 85/85 killed, zero survivors |
| Native invalid pin |configuration refused, exit 2 |
| Native unreachable loopback relay |finite timeout, exit 1 |
| New Pylon binary, legacy default decision doctor |two frozen toy cases, 103 ms, zero error, healthy; no identity state written |
| Mac plain, real signed NIP-90 | PONG, 907 ms |
| Mac ASan/UBSan, real signed NIP-90 | PONG, 837 ms; no sanitizer error |
| Linux GCC13 portable C context, real signed NIP-90 | PONG, 525 ms |

The frozen text authority source SHA-256 is
`07a13286da39fc505bef8c01b0aa5cde1f0cece8111b3a9e9d36069bc2343ed8`;
the probe authority is
`031f61eae3fb6f86b12b8b54fcae608fdad0b141f2aa27de52aebe29b6426a3c`.
Native C was emitted with the qualified Coax compiler whose SHA-256 is
`f0f088f04c992655a1c119ba229240288ccf67f755c78b065017c1441e825eca`.
The existing Responses runtime source base was `37eecad7b1`.

The three independent public probes used fresh ephemeral buyers and unique
prompts. Their exact request and result IDs are in the receipt. The Linux
client was compiled from the same portable generated Coax C plus owned
native network/TLS/entropy effects that the managed watcher consumes. Its
runtime needs neither the model nor Python.

Unchanged numerical benchmarks, complete compiler suites and unrelated
model gates were not rerun. The existing decision regression specifically
exercised the changed shared HTTP path using the newly compiled Pylon.

## Continue operation

Both new units are enabled and remain running:
`psionic-gptoss20b` and `openagents-pylon-oct30-gcloud-gptoss20b`.
Private model access stays on loopback 8795. Native monitored request and
recovery details are in the [operator runbook](../../psionic/docs/serving/gptoss-cloud-pylon.md).
The root fleet watcher owns the six-hour keepalive window and repeated live
tests. This is initial qualification evidence, not a claim that the full
six-hour availability period has already completed.
