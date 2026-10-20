# Enter the Verse: proposed issues

Status: proposal, 2026-10-20, updated with the owner's decisions. **None of
these is filed.** The strategy is in [Enter the Verse](2026-10-20-enter-the-verse.md),
the model plan is in [native DeepSeek-V4.1-Flash](../psionic/deepseek-v41-native.md),
and the software factory is in [the Forge](../forge/README.md).

Conventions for filing:

- One epic per workstream, with its children linked as sub-issues and native
  blocker links (`scripts/project-status.sh`).
- Put every issue on the boards it belongs to with `scripts/dev/issue-board.sh`.
- Each child names its acceptance check. A deploy, a live recording, or an
  owner step goes into "After-close checks" or `NEEDS_OWNER.md`, so it never
  holds the issue open.
- Size: S is under a day, M is one to three days, L is longer. Split L items
  further when one is claimed.
- Every UI item ships in the web app first, from GPUI and Verse pieces the
  desktop app reuses (decision D2).
- Every cloud machine is labeled, expiring, and counted against the $3,000
  cap (decision D4).
- The open issue #12368 (native Qwen2 CUDA for the R1 distill) is already in
  progress and is a dependency, not a duplicate.

## Epic P: Psionic serves DeepSeek-V4.1-Flash natively, fanned out across Pylons

| ID | Title | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| P0.1 | Managed recovery for a native Psionic worker | M | #12368 | The serial recovery checks from #12364 pass against `psionic-openai-server --single-generation`, including a crash and a frozen generation. |
| P0.2 | Real HTTP disconnect and hard-stop check for native serial serving (S2) | S | P0.1 | A caller disconnect leaves the slot busy until the worker finishes or is replaced. Evidence is retained. |
| P0.3 | Cloud spend guard and budget alerts | S | None | Every Psionic or Pylon cloud command labels, expires and records cost per machine; the run refuses to start a machine past the $3,000 cap. Billing alerts at $1,500 and $2,500 exist, or are in `NEEDS_OWNER.md`. |
| P1.0 | V4.1 oracle: golden activations from the official reference | M | P0.3 | Fixtures for the layers and parts listed in the plan's V0 are committed with digests. The rented machine is deleted and its cost recorded. |
| P1.1 | DS1: `deepseek-recipe` adapter for V4.1 prompts, reasoning and tool calls | M | None | The checkpoint's `encoding/tests` cases pass byte for byte. Mock streamed tool-call tests pass. |
| P1.2 | Safetensors loader with FP8 E4M3 and FP4 E2M1 block-scaled layouts | M | None | All 48 shards' tensors load with the right shape and dtype, including a layer-range-only load. Dequantized blocks match P1.0. |
| P1.3 | CPU reference: hyper-connections and Sinkhorn mixing | S | P1.0, P1.2 | Matches golden slices. |
| P1.4 | CPU reference: sliding-window, compressed and full attention with sink and shared KV | L | P1.0, P1.2 | Each `compress_ratios` mode matches golden slices. KV source-layer reuse is tested. |
| P1.5 | CPU reference: indexer and layer-20 candidate selection | M | P1.4 | Selected indices match exactly, including ties and short rows. |
| P1.6 | CPU reference: Engram hashing and lookup from host memory | M | P1.0, P1.2 | The compressed token map and n-gram hashes match the reference. Lookups match golden slices. |
| P1.7 | CPU reference: `sqrtsoftplus` gate, routed and shared experts | M | P1.0, P1.2 | Router ids match exactly. Combine weights and expert outputs match within tolerance. |
| P1.8 | CPU full model: 40 layers, greedy decode | M | P1.3–P1.7 | Logits for the first 32 tokens on the oracle prompts match within tolerance. The greedy text matches. |
| P1.9 | CUDA kernels: FP4 and FP8 block-scaled matrix-vector | M | P1.2 | Parity with CPU on the RTX 4080 and the target GPU. |
| P1.10 | CUDA kernels: attention modes, indexer, Sinkhorn, Engram gather | L | P1.4–P1.6 | Parity with CPU on both GPUs. |
| P2.1 | Stage-state protocol: generalize the Gemma 4 step and reset | M | P1.8 | One typed message carries residual streams, shared KV, indexer and candidate state. The Gemma 4 path uses it too. |
| P2.2 | Group manifest and placement for mixed GPUs | M | P2.1 | A manifest names each stage's layers, GPU and Engram location. Placement fits layer ranges to each GPU's free memory and refuses a plan that doesn't fit. |
| P2.3 | Stage Pylons on one machine: one Psionic process per GPU | L | P1.9, P1.10, P2.1, P2.2 | Full-model parity with P1.8 on one 8-GPU machine. DS0's six runtime checks pass. Speed and memory per GPU are recorded. |
| P2.4 | Stage packages: each stage fetches only its layers | M | P1.2, P2.2 | A stage's download is limited to its tensors and is verified by digest. |
| P2.5 | Group readiness, fencing and per-stage managed recovery | M | P2.3, P0.1 | A group is published only when every stage is loaded and checked. A killed stage fails the request visibly; the group recovers; a restarted stage can't serve a stale request. |
| P2.6 | Stage Pylons across machines, in two configurations | L | P2.4, P2.5 | Parity and the serial Pylon checks pass with eight single-GPU machines and with four two-GPU machines. Per-hop latency and per-token stage state are recorded. |
| P3.1 | `pylon cloud up` for a model group | M | P2.5, P0.3 | One command starts a group in a named configuration, checks it and publishes it, then stops it on expiry, on the spend cap, or on `down`. Spot loss shows as Pylons leaving. |

## Epic N: The Pylon network carries agent work

| ID | Title | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| N1.1 | NIP-CJ: sealed input blobs referenced by hash | M | None | Jobs carry inputs far beyond 16 KiB. Old Pylons refuse the new form clearly. Fixtures cover both. |
| N1.2 | NIP-CJ: streamed partial results with a public flag | M | None | A buyer receives ordered, sealed chunks; missing or duplicate chunks are detected. A request's sealed public flag (decision D5) lets the provider also publish chunks the world may show. Private jobs publish activity only. |
| N1.3 | Beacon-declared deadlines and output limits per model | S | N1.2 | The router and buyers use the beacon's limits. The 90 s default stays for models that don't declare one. |
| N2.1 | Attested Pylons publish a NIP-PYLON beacon linked to their NIP-ATT endpoint | M | None | `RelayField` lists the attested TDX Pylon as secure, and the field check verifies the link. |
| N2.2 | Sensitive turns go to the attested Clef | S | N2.1 | A turn marked sensitive (composer lock or agent policy) gets its decision sealed to the attested Pylon. Ordinary turns keep using the regular Clef Pylon (decision D6). |
| N2.3 | Owner command to wake the attested H100 for a window | S | N2.1 | One command starts `oa-att-h100-1`, waits for its endpoint, and reports when it will idle-stop. |
| N3.1 | Router trace events over SSE | M | None | Each `/home` turn emits typed steps: decision, seal, relay, open, decide, job, tokens, answer. No message text appears in the events unless the job is public. |
| N4.1 | Model-aware routing | M | N5.1 | A request for a model or capability reaches a Pylon that serves it. Clef can choose the large model for a prompt. |
| N5.1 | Network primitive catalog | M | None | One typed list built from verified beacons: model id and digest, decision lanes, attested endpoints, model groups, measured latency and price. Exposed to the web, the Agent Lab, the Forge and Coder. |

## Epic G: The Grid scene shows the network

| ID | Title | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| G1.1 | Signed-in `/` opens the Grid | S | None | The first-paint budget holds. Signed-out `/` is unchanged unless the owner decides otherwise. |
| G2.1 | The Pylon Bay: a place in the Grid | M | None | The Bay is part of the Grid layout. Pylons stand in it, grouped by owner. |
| G2.2 | Pylons drawn in Verse by hardware class, including a secure look | M | G2.1, N2.1 | Pylons are Verse geometry, not an HTML overlay. Shapes follow `docs/compute/verse-compute.md`. A capture check covers each class. |
| G3.1 | Composer lock and sealed or public packets driven by router traces | M | N3.1, N2.2, G2.2 | The composer has a sensitive lock. Packets move only when trace events arrive. Captures show a sealed decision round. |
| G4.1 | Model groups: arrival, the per-token chain, and public tokens | M | G3.1, P2.5, N1.2 | A group's stage Pylons arrive together. Per-token activity moves along the chain. Tokens from a public job rise above it; a private job shows activity only. |
| G5.1 | Live Gym boards on the web Grid | M | None | The boards show published Gym results, including Pylon model benchmarks and Forge outcomes, without a native host. |

## Epic L: The Agent Lab composes primitives

| ID | Title | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| L1.1 | Agent Lab place in the Grid | S | None | Walking in opens the existing GPUI Agent Lab. There's no second Lab. |
| L2.1 | Agent definitions bind resources and a per-step policy | L | N5.1 | A definition names network primitives, paired harnesses and hosted models, a per-step policy (sensitive, bulk, hard) evaluated by Clef judgments, and the "show work in the Grid" setting. Versioned and stored like templates. |
| L3.1 | Pylon models as a capacity-book provider for Microcoder and Coder | M | N1.1, N1.2, N5.1 | A Microcoder step runs on a network model and fails over like other providers. |
| L3.2 | Structured steps on network models | M | L3.1, P1.1 or R2 in the Psionic roadmap | Commands from a network model are parsed or constrained, never guessed. Unsupported models are refused. |

## Epic F: The Forge

| ID | Title | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| F1.1 | `forge` crate: work order, run, evidence, verification and delivery records | M | None | Typed records with the transitions in [the Forge](../forge/README.md#objects), unit-tested, stored with the Agent Lab's records. |
| F2.1 | The work order loop with Foreman as crew lead | L | F1.1 | Foreman takes a work order on a test repository, fans out workers, collects evidence, runs the checks and a reviewer agent, and opens a draft pull request with a delivery receipt. |
| F2.2 | Leases and lost-worker handling for runs | M | F2.1 | A lost worker is noticed, its run fails visibly, and its work isn't retried without a new run. |
| F3.1 | Forge place in the Grid with the work order board | M | F1.1 | Walking in shows queued, running and delivered work from the records. |
| F3.2 | Repository floor generated from the code graph | L | F3.1 | Loading a repository builds a floor: crates as districts and modules as buildings. The layout is stable across loads. |
| F3.3 | Worker events to the web, avatars at the code they touch, and visible messages | M | F2.1, F3.2 | Workers' file reads, edits and commands stream to the page under the owner's authority. Avatars move to the files named. Messages show as exchanges. |
| F4.1 | Delivery view and preview panel on the web | M | F2.1 | A GPUI view shows a run's evidence, verification and pull request, with a preview panel. The desktop browser pane can replace the panel later. |
| F5.1 | Forge outcomes to the Gym | S | F2.1 | Each delivery writes a Gym result with agents, models, cost, time and verdict. |

## Epic M: Others join, and contributions are paid

| ID | Title | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| M1.1 | Presence on the home Grid | S | G1.1 | Other signed-in people appear, as on `/verse`. |
| M2.1 | Design: sharing a harness with other people | M | None | A design note on authority, consent, metering and refusal. No code until the owner approves it. |
| M2.2 | Contributor GPUs as stage Pylons | M | P2.6 | A contributor's GPU joins a model group through `pylon up` once V8's hop measurements set a latency floor. |
| M3.1 | Real-wallet paid Pylon jobs under the owner's grant | M | Owner grant | One real paid job settles end to end. Recorded in `NEEDS_OWNER.md` until the owner funds it. |
| M3.2 | Sats in the Grid from real receipts only | S | M3.1, G3.1 | Payment visuals appear only for receipts carrying a payment. Demo visuals stay labeled. |

## First wave

These are independent and unblock the most. File and claim them first:

1. P0.3, then P1.0; P1.1 and P1.2 alongside
2. P0.1
3. N1.1, N1.2, N2.1, N3.1, N5.1
4. G1.1, G2.1, L1.1
5. F1.1, then F2.1 and F3.1
