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

## Review addendum (2026-10-20): filed issues and changes

Snapshot checked against GitHub and `75392db83e`. This supersedes the
proposal's "none filed" status. These are suggested issue amendments and
follow-up scopes, not a second set of assignments. Preserve current claims;
do not reopen code-complete issues merely to wait for device evidence.

| Proposed item | Actual issue and state | Recommended update |
| --- | --- | --- |
| P0.1, P0.2 | [#12368](https://github.com/OpenAgentsInc/openagents/issues/12368), closed | Already covered for the pinned R1 native greedy profile: six model, four real HTTP, and 20 managed recovery checks. Do not file duplicates. Carry the same ownership/reaping rules into V4.1 stages, with a newly qualified deadline. |
| P0.3 | [#12370](https://github.com/OpenAgentsInc/openagents/issues/12370), closed | Keep the implementation and expiry evidence. File focused follow-ups for one cross-host budget authority and refusal on malformed interior ledger records; meanwhile allocate from one ledger host. Include storage/network reserve and current price coverage. See the [budget review](../psionic/deepseek-v41-native.md#bound-the-budget-guards-claims-before-larger-rentals). |
| Epic P | [#12369](https://github.com/OpenAgentsInc/openagents/issues/12369), open | Replace the old approximate split with #12373's header-derived totals. Separate implementation closure, byte/oracle qualification, complete native answers, and agent use. Record the four-GPU placement as a candidate, not a promised fit. |
| P1.0 / V0 | [#12371](https://github.com/OpenAgentsInc/openagents/issues/12371), open | Preserve its B200 smoke and 20k-token workload. Add teacher-forced full-logit comparisons, precision/tie rules, boundary/cache cases, and #12373's original-weight byte/hash/dequantization checks on the same rental. Retain dumps incrementally for spot recovery. |
| P1.1 / V1 | [#12372](https://github.com/OpenAgentsInc/openagents/issues/12372), open | Keep protocol qualification separate from serving. Add chunking-invariant token/text/tool parsing, cancellation/reconnect terminal states, and special-token injection cases. A mock stream does not qualify tool execution, constrained output, or model quality. |
| P1.2 / V2 | [#12373](https://github.com/OpenAgentsInc/openagents/issues/12373), closed | Reader and offline tests landed. The retained `headers_only` report opened zero tensor bytes and has no oracle parity; complete the listed after-close checks with #12371's downloaded checkpoint. Dependent numerical gates require that evidence, not an issue-state check. |

### Adjust dependencies before filing the remaining waves

- Split M/L work into independently checkable changes before claiming it,
  consistent with the repository's 24-hour issue rule. For example, split
  P1.4 by attention mode and shared-cache behavior, P1.10 by kernel, and
  F2.1 into one-worker delivery followed by crew fan-out. Track required
  device evidence separately from administrative issue closure.
- Split P2.1 into early codec/fault fixtures and later binding to real V4.1
  state. Start the former now; do not make P1.8's full CPU decode its only
  prerequisite. Prototype group cancellation/fencing with P0's native worker
  before renting a whole group. Preserve the one-GPU-per-stage shape.
- P2.2 plans from exact selected weight bytes **plus peak runtime memory**.
  P2.6 accepts two genuinely different placements with a justified GPU
  count, rather than requiring eight GPUs in both. P3.1 also depends on the
  budget follow-ups and the published group's N1/N4/N5 client contract.
- N1.1–N1.3 should first compare the current NIP-CJ conversation/execution
  contracts and implemented clients. Reuse their applicable sequencing,
  terminal-state, and artifact rules; version the Pylon-specific extension.
  Cover blob size/digest/authorization/expiry, ordered replay, duplicate/gap
  detection, bounded buffering, cancellation, and old-client refusal.
  Declared model limits are ceilings intersected with caller budgets;
  heartbeats do not renew the execution budget. Transport tests can start
  on R1 or a fixture without V4.1 weights.
- N2/N3 and D5 need an end-to-end privacy acceptance check: sensitive work
  cannot fall back to ordinary workers; a public display stream cannot
  expose reasoning or tool payloads; a beacon does not prove attestation.
  For N4/N5, match the exact requested artifact and group readiness, and
  count one serial group slot rather than summing internal-stage slots.
- Split L3.1's typed text provider from L3.2's actual tool execution. Model
  selection or text parsing grants no filesystem, shell, wallet, or sharing
  authority. A requested V4.1 job must not silently become another model.
- F1/F2 reuse existing tasks, grants, claims, traces, and leases. F5 records
  every run's outcome, including failure, cancellation, and unknown cost,
  rather than only deliveries. Protect Gym holdouts and private run data.
- F4.1 first delivers the web Forge evidence/receipt card with a PR link.
  Record how any full PR pane or app preview changes the earlier scope of
  closed #12276/#12278 before including it. UI cards and sheets use Rust
  Native through GPUI; Verse remains the world scene.

The next focused work is #12371 and #12372, the guard follow-ups, and early
stage/transport fixtures. Bay and Forge work can proceed independently.
The [model addendum](../psionic/deepseek-v41-native.md#review-addendum-2026-10-20)
and [strategy addendum](2026-10-20-enter-the-verse.md#review-addendum-2026-10-20)
explain the revised gates without changing the owner's product direction.

## Filed independent CPU components (2026-10-20)

The owner requested serial implementation of the first independent native Rust
port phase. [#12377](https://github.com/OpenAgentsInc/openagents/issues/12377)
(P1.3), [#12378](https://github.com/OpenAgentsInc/openagents/issues/12378)
(P1.7), [#12379](https://github.com/OpenAgentsInc/openagents/issues/12379)
(P1.4a/P1.5a), and
[#12380](https://github.com/OpenAgentsInc/openagents/issues/12380) (P1.6)
are children of #12369 on project 19. This chat claims one at a time, pushes
each passing implementation to `main`, and closes it before starting the next.
The existing oracle and protocol scopes stay separate. See the
[component status](../psionic/deepseek-v41-native.md#independent-cpu-port-first-implementation-phase-2026-10-20)
for completed checks and remaining integration. Small synthetic fixtures unblock
implementation; they do not replace P1.0's full-checkpoint numerical gates.
