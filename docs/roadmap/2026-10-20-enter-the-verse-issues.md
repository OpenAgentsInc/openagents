# Enter the Verse: proposed issues

Status: proposal, 2026-10-20. **None of these is filed.** The owner reviews
this list first. The strategy is in [Enter the Verse](2026-10-20-enter-the-verse.md);
the model plan is in [native DeepSeek-V4.1-Flash](../psionic/deepseek-v41-native.md).

Conventions for filing:

- One epic per workstream, with its children linked as sub-issues and native
  blocker links (`scripts/project-status.sh`).
- Put every issue on the boards it belongs to with `scripts/dev/issue-board.sh`.
- Each child names its acceptance check. A deploy, a live recording, or an
  owner step goes into "After-close checks" or `NEEDS_OWNER.md`, so it never
  holds the issue open.
- Size: S is under a day, M is one to three days, L is longer. Split L items
  further when one is claimed.
- The open issue #12368 (native Qwen2 CUDA for the R1 distill) is already in
  progress and is a dependency, not a duplicate.

## Epic P: Psionic serves DeepSeek-V4.1-Flash natively

| ID | Title | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| P0.1 | Managed recovery for a native Psionic worker | M | #12368 | The serial recovery checks from #12364 pass against `psionic-openai-server --single-generation`, including a crash and a frozen generation. |
| P0.2 | Real HTTP disconnect and hard-stop check for native serial serving (S2) | S | P0.1 | A caller disconnect leaves the slot busy until the worker finishes or is replaced. Evidence is retained. |
| P1.0 | V4.1 oracle: golden activations from the official reference | M | Owner budget (D4) | Fixtures for the layers and parts listed in the plan's V0 are committed with digests. The rented machine is deleted and its cost recorded. |
| P1.1 | DS1: `deepseek-recipe` adapter for V4.1 prompts, reasoning and tool calls | M | None | The checkpoint's `encoding/tests` cases pass byte for byte. Mock streamed tool-call tests pass. |
| P1.2 | Safetensors loader with FP8 E4M3 and FP4 E2M1 block-scaled layouts | M | None | All 48 shards' tensors load with the right shape and dtype. Dequantized blocks match P1.0. |
| P1.3 | CPU reference: hyper-connections and Sinkhorn mixing | S | P1.0, P1.2 | Matches golden slices. |
| P1.4 | CPU reference: sliding-window, compressed and full attention with sink and shared KV | L | P1.0, P1.2 | Each `compress_ratios` mode matches golden slices. KV source-layer reuse is tested. |
| P1.5 | CPU reference: indexer and layer-20 candidate selection | M | P1.4 | Selected indices match exactly, including ties and short rows. |
| P1.6 | CPU reference: Engram hashing and lookup | M | P1.0, P1.2 | The compressed token map and n-gram hashes match the reference. Lookups match golden slices. |
| P1.7 | CPU reference: `sqrtsoftplus` gate, routed and shared experts | M | P1.0, P1.2 | Router ids match exactly. Combine weights and expert outputs match within tolerance. |
| P1.8 | CPU full model: 40 layers, greedy decode | M | P1.3–P1.7 | Logits for the first 32 tokens on the oracle prompts match within tolerance. The greedy text matches. |
| P1.9 | CUDA kernels: FP4 and FP8 block-scaled matrix-vector | M | P1.2 | Parity with CPU on the RTX 4080 and the target GPU. |
| P1.10 | CUDA kernels: attention modes, indexer, Sinkhorn, Engram gather | L | P1.4–P1.6 | Parity with CPU on both GPUs. |
| P1.11 | Multi-GPU layer stages in one Psionic process, with typed stage state | L | P1.8–P1.10 | Full-model parity with P1.8 on one 8-GPU machine. DS0's six runtime checks pass. Speed and memory per GPU are recorded. |
| P1.12 | V4.1 through Pylon: serial serving with managed native recovery | M | P1.11, P0.1 | Serial Pylon and recovery checks pass for V4.1 on the cloud machine. |
| P2.1 | Stage-state protocol: generalize the Gemma 4 step and reset | M | P1.11 | One typed message carries residual streams, shared KV, indexer and candidate state. The Gemma 4 path uses it too. |
| P2.2 | Stage packages: each stage Pylon fetches only its layers | M | P1.2, P2.1 | A stage's download is limited to its tensors and is verified by digest. |
| P2.3 | Stage readiness and group fencing | M | P2.1 | A group is published only when all stages are loaded and checked. A restarted stage can't serve a stale request. |
| P2.4 | Stage Pylons across machines | L | P2.1–P2.3, P1.12 | Parity and the serial checks pass with stages on separate VMs. A killed stage fails the request visibly, and the group recovers. |
| P3.1 | `pylon cloud up` for a model group: labeled, cost-capped and expiring | M | P1.12 | One command starts, checks and publishes a group, then stops it on expiry, on a spend cap, or on `down`. Spot loss shows as Pylons leaving. |

## Epic N: The Pylon network carries agent work

| ID | Title | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| N1.1 | NIP-CJ: sealed input blobs referenced by hash | M | None | Jobs carry inputs far beyond 16 KiB. Old Pylons refuse the new form clearly. Fixtures cover both. |
| N1.2 | NIP-CJ: streamed partial results | M | None | A buyer receives ordered, sealed chunks. Missing or duplicate chunks are detected. Public jobs can mark chunks as showable. |
| N1.3 | Beacon-declared deadlines and output limits per model | S | N1.2 | The router and buyers use the beacon's limits. The 90 s default stays for models that don't declare one. |
| N2.1 | Attested Pylons publish a NIP-PYLON beacon linked to their NIP-ATT endpoint | M | None | `RelayField` lists the attested Pylon as secure, and the field check verifies the link. |
| N2.2 | Router uses the attested Clef for routing decisions when present | S | N2.1 | `/home` turns record an attested decision when the secure Pylon is up, and fall back cleanly when it isn't. |
| N3.1 | Router trace events over SSE | M | None | Each `/home` turn emits typed steps: decision, seal, relay, open, decide, job, tokens, answer. No message text appears in the events unless the job is public. |
| N4.1 | Model-aware routing | M | N5.1 | A request for a model or capability reaches a Pylon that serves it. Clef can choose the large model for a prompt. |
| N5.1 | Network primitive catalog | M | None | One typed list built from verified beacons: model id and digest, decision lanes, attested endpoints, measured latency and price. Exposed to the web, Agent Lab and Coder. |

## Epic G: The Grid scene shows the network

| ID | Title | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| G1.1 | Signed-in `/` opens the Grid | S | None | The first-paint budget holds. Signed-out `/` is unchanged unless the owner decides otherwise. |
| G2.1 | The Pylon Bay: a place in the Grid | M | None | The Bay is part of the Grid layout. Pylons stand in it, grouped by owner. |
| G2.2 | Pylons drawn in Verse by hardware class, including a secure look | M | G2.1, N2.1 | Pylons are Verse geometry, not an HTML overlay. Shapes follow `docs/compute/verse-compute.md`. A capture check covers each class. |
| G3.1 | Sealed and public packets driven by router traces | M | N3.1, G2.2 | Packets move only when trace events arrive. Captures show a sealed decision round. |
| G4.1 | Stage groups: arrival, the per-token chain, and public tokens | M | G3.1, P2.3, N1.2 | A group arrives as one event. Tokens from a public job rise above it. A private job shows activity only. |
| G5.1 | Live Gym boards on the web Grid | M | None | The boards show published Gym results, including Pylon model benchmarks, without a native host. |

## Epic L: The Agent Lab composes primitives

| ID | Title | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| L1.1 | Agent Lab place in the Grid | S | None | Walking in opens the existing GPUI Agent Lab. There's no second Lab. |
| L2.1 | Agent definitions bind resources and a per-step policy | L | N5.1 | A definition names network primitives, paired harnesses and hosted models, and a policy (sensitive, bulk, hard) evaluated by Clef judgments. Versioned and stored like templates. |
| L3.1 | Pylon models as a capacity-book provider for Microcoder and Coder | M | N1.1, N1.2, N5.1 | A Microcoder step runs on a network model and fails over like other providers. |
| L3.2 | Structured steps on network models | M | L3.1, P1.1 or R2 in the Psionic roadmap | Commands from a network model are parsed or constrained, never guessed. Unsupported models are refused. |

## Epic R: Repository work you can watch

| ID | Title | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| R1.1 | Repository view generated from the code graph | L | None | Loading a repository builds a Grid place: crates as districts and modules as buildings. The layout is stable across loads. |
| R2.1 | Foreman worker events to the web Grid | M | None | Workers' file reads, edits and commands stream to the page under the owner's authority. |
| R2.2 | Worker avatars placed by the code they touch | M | R1.1, R2.1 | Avatars move to the files their events name. |
| R3.1 | Visible messages between Foreman and workers | S | R2.1 | Messages show as exchanges in the world, from the same stream. |
| R4.1 | Foreman ends in a draft pull request shown in the world | M | R2.1 | The pull request link appears in the chat and the world. Test repositories only during checks. |

## Epic M: Others join, and contributions are paid

| ID | Title | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| M1.1 | Presence on the home Grid | S | G1.1 | Other signed-in people appear, as on `/verse`. |
| M2.1 | Design: sharing a harness with other people | M | None | A design note on authority, consent, metering and refusal. No code until the owner approves it. |
| M3.1 | Real-wallet paid Pylon jobs under the owner's grant | M | Owner grant | One real paid job settles end to end. Recorded in `NEEDS_OWNER.md` until the owner funds it. |
| M3.2 | Sats in the Grid from real receipts only | S | M3.1, G3.1 | Payment visuals appear only for receipts carrying a payment. Demo visuals stay labeled. |

## Epic F: Forge

| ID | Title | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| F1.1 | Decide what the Forge is (owner decision D1) | S | None | A one-paragraph definition, recorded in the strategy doc. Follow-on issues are filed only after it. |

## First wave

To start the work without waiting on decisions, file and claim these first.
They're independent and unblock the most:

1. P1.0 (after the owner sets D4's budget), P1.1, P1.2
2. P0.1
3. N1.1, N1.2, N2.1, N3.1, N5.1
4. G1.1, G2.1, L1.1
5. R1.1, R2.1
