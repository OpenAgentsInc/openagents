# Enter the Verse: from the Grid to swarm-served agents

Status: proposal, 2026-10-20. Nothing here is filed as issues yet. The
proposed issues are in [the issue roadmap](2026-10-20-enter-the-verse-issues.md).
The model work is in [native DeepSeek-V4.1-Flash in
Psionic](../psionic/deepseek-v41-native.md).

## Where we want to be

Episode 289 introduced OpenAgents as a composable general agent. The first
Episode 290 draft ([`docs/transcripts/290.md`](../transcripts/290.md)) launched
Coder as a metaharness. The owner's new Episode 290, "Enter the Verse", makes a
different claim. The apps, CLI, SDK and API are infrastructure. The product
is a shared world, the Grid at openagents.com, where three kinds of
primitives become visible and composable:

- **Inference**, served by Pylons that anyone can run. This includes models no
  single contributor could run alone, such as DeepSeek-V4.1-Flash split across
  several Pylons.
- **Decisions**, from the Clef decision model, sealed inside an attested Pylon
  when the request is sensitive.
- **Harnesses** people already pay for, such as Codex and Claude Code, reached
  through their paired computers.

In the Agent Lab, a person combines those primitives into an agent with a
policy. For example: sensitive steps go to the attested Pylon, bulk steps go
to DeepSeek on the swarm, and the hard steps go to Opus through Claude Code.
Foreman is the first such agent. It loads a repository, spawns workers you can
watch in the parts of the code they touch, and ends in a pull request. Later,
other people join with their own compute and harnesses, and Bitcoin pays for
what they contribute.

## Ground rules

1. **Every claim on screen is real.** The demo shows live beacons, real jobs
   and real answers. Anything simulated carries a visible label, as the
   Episode 290 draft did with "demo payments". This follows from the honest
   copy rules in `AGENTS.md`.
2. **Psionic is the only inference engine in the DeepSeek path.** llama.cpp
   stays an explicit, labeled option for other models until native support
   replaces it, as #12368 does for the R1 distill.
3. **Pylons are the network.** A model in the Grid is a model some Pylon
   advertises in a verified beacon. There's no hidden hosted path pretending
   to be a Pylon. The `/home` router already falls back to hosted chat when
   no Pylon answers, and says so in the answer.
4. **Primitives come from domain crates.** The Grid, the Agent Lab and the apps
   link `pylon`, `psionic`, `agent-lab` and `coder-*`. They don't reimplement
   them.
5. **One request at a time, then scale.** The owner's order from the DeepSeek
   plan holds: personal, reliable, serial serving before batching, replicas or
   a public gateway.

## What exists today

| Area | State on 2026-10-20 | Where |
| --- | --- | --- |
| Grid on the web | `/home` paints the Verse Grid (`everglade-web`, WebGPU or WebGL2). It has live Pylon pillars from NIP-PYLON beacons and receipts, and one terminal you walk up to. The terminal opens the GPUI chat. `/` is unchanged. | #12345, #12352, #12353, #12356, #12362 |
| Pylon router | `/home` terminal turns go to a live Pylon over encrypted NIP-CJ. If a Clef Pylon is live, a typed Clef decision picks the Pylon. If no Pylon answers, hosted chat does. | `crates/pylon/src/router.rs`, #12366 |
| Pylon network | `pylon up/down/status` with a supervisor, beacons, receipts, pool aggregates and checkers. Three peers in the runbook: the owner's Mac (Metal), `coderos-4080-net` (CUDA) and `oa-pylon-gpu-1` (GCE L4). | `docs/compute/pylon-network-runbook.md` |
| Serial DeepSeek serving | R1-Distill-Qwen-1.5B: DS0 qualified, then serial Pylon (27/27), automatic recovery (13/13) and the `pylon up` profile (#12367). Native Qwen2 CUDA to replace llama.cpp is in progress. | #12358, #12360, #12364, #12367, #12368 |
| Attested Pylon | `pylon serve --attested` in Confidential Space (TDX). It runs sealed NIP-DEC decisions on Clef. `oa-att-tdx-1` is always on (CPU, about 8 s per decision). `oa-att-h100-1` is a spot H100 that is usually stopped and takes about 4 minutes to wake. `/att` shows a verified round in a separate scene. It publishes no NIP-PYLON beacon, so the Grid can't see it. | `crates/pylon/src/attested.rs`, `docs/security/private-inference.md` |
| Agent Lab | A GPUI page (web and desktop). It lists computers and their Codex and Claude runtimes, opens task chats, and has templates. Foreman is a template with check-ins, standing authority, a decision log and a GCE fan-out. AgentGraph draws an agent's structure. | `crates/agent-lab`, `docs/foreman/`, #12285 |
| Agents in a world | Only the desktop Everglade studio walks agent seats between stations. Nothing maps agents to areas of a repository. | `docs/verse/agent-studio.md` |
| Gym | The Gym building stands in the Grid, but its boards are empty on the web. Results show on agent pages. | `crates/verse-gym`, `docs/verse/gym.md` |
| Forge | Nothing exists. `docs/roadmap.md` lists an agent forge as a deferred direction. | |
| Payments | Paid Pylon jobs over NIP-X402 and a broker are tested on fake Lightning only. Lightning addresses and earnings pages are landing (#12307). Mainnet needs the owner's grant. | `crates/pylon/src/paid.rs`, `NEEDS_OWNER.md` |

## The demo, beat by beat

Each beat lists what must be true for the owner to record it honestly. The
epic codes refer to [the workstreams](#workstreams).

| # | Beat | Real today | Missing | Epics |
| --- | --- | --- | --- | --- |
| 1 | Sign in at openagents.com and land in the Grid. | `/home` shows the Grid. | Signed-in `/` opens the Grid. Login keeps the world loaded. | G1 |
| 2 | The Pylon Bay shows the Mac, Linux, a basic cloud GPU and a secure GPU. | Three peers can beacon. Pillars are an HTML overlay. | A Bay place in the Grid. Pylons drawn as Verse geometry by hardware class. The attested Pylon publishes a beacon and appears as the secure one. | G2, N2 |
| 3 | At the Pylon Terminal, a request goes encrypted to Clef in the enclave, which decrypts it, decides, and returns. | The router makes a real Clef decision and sends an encrypted job. The attested path is separate. | Route the routing decision through the attested Clef. Draw packets from the router's real steps: sealed packet, relay, enclave decrypt, answer. | N2, N3, G3 |
| 4 | "Can I run DeepSeek V4.1 Flash?" New Pylons come online from cloud credits. | Nothing. | Native V4.1 in Psionic, stage Pylons, and a one-command, cost-capped launch whose beacons arrive live. | P1, P2, P3, G4 |
| 5 | A big prompt goes to DeepSeek. Public tokens stream above the Pylons as it works. | Nothing. Jobs aren't streamed and are capped at 16 KiB and 90 s. | Model choice in the router. Streamed partial results. Longer jobs with larger inputs. A public or private flag that decides what the world shows. | N1, N4, G4 |
| 6 | DeepSeek V4.1 Flash is now a primitive I can use. | Beacons name models. | A catalog of network primitives (inference by model, decisions, attested decisions) that the Agent Lab and Coder can bind to. | N5 |
| 7 | The Agent Lab: connect Pylons, my Codex and Claude Code, and define an agent with a routing policy. | The Agent Lab page lists computers and runtimes. | An Agent Lab place in the Grid. Resource bindings for network primitives. A per-step routing policy (sensitive, bulk, hard). Pylon models as a provider for Microcoder and Coder. | L1, L2, L3 |
| 8 | Foreman loads the OpenAgents repository and spawns agents. You see where in the code they work, and they talk to each other. | Foreman fans out on GCE. AgentGraph exists. The studio exists on desktop only. | A repository view in the Grid built from the code graph. Worker avatars placed by the files they touch. Visible messages between workers. Live events streamed to the web. | R1, R2, R3 |
| 9 | It ends in a pull request, opened in a built-in browser. | Foreman lands through `openagents land`. The desktop has a browser pane. The web has none, by owner decision. | A pull request as Foreman's visible result. A decision on where the browser beat is recorded. | R4, D2 |
| 10 | "This is single-player. Others will join." | `/verse` has presence. `/home` doesn't. | Presence in the home Grid. A path for others' Pylons and harnesses to join. Paid jobs (shown as a promise, not claimed). | M1, M2, M3 |

The Gym and the Forge don't appear in the draft's beats beyond a mention. The
roadmap gives them a minimal, honest role ([G5](#g-the-grid-scene), [F1](#f-gym-and-forge)).

## Workstreams

### P: Psionic and DeepSeek

- **P0: Native serving foundation.** Finish #12368 (native Qwen2 CUDA for the
  R1 distill). Generalize managed recovery from llama.cpp to a native Psionic
  worker. Close S2's real HTTP-disconnect check. This proves the
  supervisor-plus-native-worker shape that V4.1 reuses.
- **P1: V4.1-Flash native.** Steps V0–V7 of
  [the V4.1 plan](../psionic/deepseek-v41-native.md#build-order): oracle
  fixtures, the `deepseek-recipe` protocol adapter, the safetensors loader,
  CPU components, the CPU full model, CUDA kernels, multi-GPU stages in one
  process, and Pylon serving.
- **P2: Stage Pylons.** Step V8: one model group across several Pylons, with
  typed stage state, stage packages, readiness, and fencing.
- **P3: Cloud launch.** One owner command creates a labeled, cost-capped,
  expiring GCE group, waits for the stages to be ready, and publishes their
  beacons. It stops on expiry, on a spend cap, or on `down`. Spot loss shows
  as Pylons leaving, not as a hang.

### N: Pylon network

- **N1: Jobs for agent work.** Larger inputs (sealed blobs referenced by hash
  instead of 16 KiB inline), streamed partial results over NIP-CJ feedback,
  and deadlines negotiated per model and declared in the beacon. This is a
  NIP-CJ and NIP-PYLON revision with fixtures, not a timeout change.
- **N2: Attested Pylons in the field.** The attested Pylon publishes a
  NIP-PYLON beacon that links its NIP-ATT endpoint. `RelayField` and the
  router show and select it. Keep the H100 warm during a recording window, or
  use the TDX CPU Pylon and accept its latency.
- **N3: Route trace events.** The router emits a typed trace per turn:
  decision requested, sealed, relayed, opened in the enclave, decided, job
  sent, tokens, answered. The Grid draws only from these events.
- **N4: Model-aware routing.** The router picks by requested model or
  capability, not only by free slots and tier. Clef's choice includes "the
  big model" when a prompt needs it.
- **N5: Primitive catalog.** One typed list of what the network offers:
  inference by model id and digest, decisions, attested decisions. Built from
  verified beacons, with measured latency and price. The Agent Lab, Coder
  and the Grid read it.

### G: The Grid scene

- **G1: Grid at `/` when signed in.** Promote `/home`'s world for signed-in
  visitors, keeping the first-paint budget.
- **G2: The Pylon Bay.** A Bay place in the Grid. Pylons are drawn in Verse by
  hardware class, reusing the Everglade Pylon Field shapes
  (`docs/compute/verse-compute.md`). The secure Pylon gets its own sealed
  look. Clicking one opens the existing GPUI card.
- **G3: Packets.** Port the `/att` sealed-packet visuals into Verse and drive
  them from N3. Encrypted traffic looks different from public traffic.
- **G4: Swarm moments.** Stage Pylons arrive as a group. Per-token activity
  flows along the stage chain. Public tokens rise above the group, and private
  turns show only that work is happening.
- **G5: Live Gym boards on the web.** Fill the Grid's Gym boards from
  published Gym results, including Pylon model benchmarks.

### L: The Agent Lab as a place

- **L1: Agent Lab building.** A Grid place that opens the existing GPUI Agent
  Lab, not a second Lab.
- **L2: Resources and policy.** An agent definition binds resources (network
  primitives from N5, the person's paired computers and harnesses, hosted
  models) and a per-step policy. The policy uses Clef judgments, as Coder's
  System 1 does today. Version the definition and store it like other Agent
  Lab templates.
- **L3: Pylon models as a provider.** Microcoder and Coder can run steps on
  a network model through the capacity book, next to Codex, Claude and Vertex.
  This needs N1 and either tool-call parsing or constrained output (DS1, R2).

### R: Repository work you can watch

- **R1: Repository view.** A Grid place laid out from `crates/code-graph`
  (crates as districts, modules as buildings). Generated on load, not
  hand-built.
- **R2: Worker presence.** Foreman's workers appear as avatars at the code
  they read and edit. Positions come from their real tool events.
- **R3: Worker messages.** Messages between Foreman and its workers show as
  visible exchanges, from the same event stream.
- **R4: Pull request as the result.** Foreman's run ends in a draft pull
  request, shown in the world and in the chat.

### F: Gym and Forge

- **F1: Forge, defined minimally.** Proposal: the Forge is where an agent
  definition from the Lab is tested in the Gym and published, building on the
  plugin flow from Episode 289. Build nothing until the owner confirms the
  definition (D1).

### M: Multiplayer and money

- **M1: Presence on the home Grid.** Reuse `/verse` presence.
- **M2: Others' Pylons and harnesses.** Onboarding for a contributor's Pylon
  is `pylon up`. Sharing a harness with others is a new authority question
  and needs a design before code.
- **M3: Paid jobs.** Run the existing NIP-X402 path with a real wallet under
  the owner's grant. Show sats in the Grid only from real receipts.

## Order and milestones

The critical path is the model: P0, then P1, then P2 and P3. Everything in the
Grid can proceed in parallel against the R1 distill and Clef, which already
run on real Pylons.

| Milestone | Contents | What it proves |
| --- | --- | --- |
| M-A: The Bay, honestly | G1, G2, N2, N3, G3 | Beats 1–3 recordable with today's Pylons and Clef |
| M-B: V4.1 for us | P0, P1 (V0–V7) on one cloud machine | One reliable native V4.1 answer through a Pylon |
| M-C: The swarm | P2, P3, N1, N4, G4 | Beats 4–5 with stage Pylons and streamed public tokens |
| M-D: Composition | N5, L1, L2, L3 | Beats 6–7: an agent that mixes network models and harnesses |
| M-E: Watchable work | R1–R4 | Beats 8–9 |
| M-F: Others join | M1, M2, M3, G5, F1 | Beat 10, and the network story after the episode |

M-A and M-B can start at once and don't depend on each other. M-C needs both.
M-D needs N1 for a useful context size. M-E needs only Foreman's existing
events and the code graph, so it can run alongside M-B.

## Decisions for the owner

- **D1: Forge.** Is the Forge "test in the Gym and publish an agent or
  plugin", or something else?
- **D2: The browser beat.** The web has no browser pane by an earlier owner
  decision. Record beats 8–9 in the desktop app, where Verse and the browser
  pane both exist? Or allow a pull-request preview panel on the web?
- **D3: The swarm shape.** For the recording, choose between one 8-GPU
  machine shown as one large Pylon, and several smaller machines shown as
  stage Pylons. Stage Pylons match the script but add P2's work and some
  latency. We recommend building one machine first (V6) and stage Pylons
  second (V8). Record with whichever is qualified.
- **D4: Budget.** Spend caps for the V0 oracle run (one 8-GPU machine for a
  few hours) and for development time on a G4 or A4 machine.
- **D5: Public tokens.** Is "public inference shows its tokens in the world"
  opt-in per request, per agent, or per Pylon?
- **D6: Attested routing latency.** The H100 attested Pylon answers in under
  a second but costs about $6.83/h and takes about 4 minutes to wake. The TDX
  CPU Pylon is always on but takes about 8 s. Keep the H100 warm only for
  recording windows?

## Risks

- **V4.1 is a large new graph.** Hyper-connections, three attention modes,
  cross-layer sharing, the indexer, Engram and FP4 experts are each new to
  Psionic. The golden-activation oracle keeps every step checkable, but the
  total is weeks of work, not days.
- **Spot capacity.** A4, G4 and H200 capacity in our regions is
  quota-limited and spot-only. A recording needs a reserved window or a
  fallback recording plan.
- **Context and deadlines.** Agent work on V4.1 isn't useful until N1 lands.
  Raising limits touches the protocol and every Pylon. Version the change.
- **Tool use.** Without DS1's tool-call parsing or R2's constrained
  decoding, V4.1 on Pylons can answer but can't act as a coding worker.
