# Enter the Verse: from the Grid to swarm-served agents

Status: proposal, 2026-10-20, with the owner's decisions recorded the same
day. Nothing here is filed as issues yet. The proposed issues are in
[the issue roadmap](2026-10-20-enter-the-verse-issues.md). The model work is
in [native DeepSeek-V4.1-Flash in Psionic](../psionic/deepseek-v41-native.md),
and the software factory is in [the Forge](../forge/README.md).

## Where we want to be

Episode 289 introduced OpenAgents as a composable general agent. The first
Episode 290 draft ([`docs/transcripts/290.md`](../transcripts/290.md)) launched
Coder as a metaharness. The owner's new Episode 290, "Enter the Verse", makes a
different claim. The apps, CLI, SDK and API are infrastructure. The product
is a shared world, the Grid at openagents.com, where three kinds of
primitives become visible and composable:

- **Inference**, served by Pylons that anyone can run. This includes models no
  single contributor could run alone, such as DeepSeek-V4.1-Flash fanned out
  across several Pylons.
- **Decisions**, from the Clef decision model, sealed inside an attested Pylon
  when the request is sensitive.
- **Harnesses** people already pay for, such as Codex and Claude Code, reached
  through their paired computers.

The Grid has three places where people put those primitives to use:

- **The Agent Lab builds agents.** A person combines primitives into an agent
  with a policy. For example: sensitive steps go to the attested Pylon, bulk
  steps go to DeepSeek on the swarm, and the hard steps go to Opus through
  Claude Code.
- **The Forge puts agents to work.** It turns agent work into
  production-grade software: work orders in, verified pull requests with
  evidence and receipts out. Foreman is the first crew lead. It loads a
  repository and spawns workers you can watch in the parts of the code they
  touch.
- **The Gym makes agents better.** Every Forge outcome is a measured result
  that feeds back into the Lab.

Later, other people join with their own compute and harnesses, and Bitcoin
pays for what they contribute.

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
4. **Primitives come from domain crates.** The Grid, the Agent Lab, the Forge
   and the apps link `pylon`, `psionic`, `agent-lab` and `coder-*`. They don't
   reimplement them.
5. **One request at a time, then scale.** The owner's order from the DeepSeek
   plan holds: personal, reliable, serial serving before batching, replicas or
   a public gateway.
6. **Web first, reusable on desktop.** Every new surface ships in the web app
   first, built from GPUI and Verse pieces that the desktop app reuses.

## Decisions

Recorded 2026-10-20. D1–D4 are the owner's; D5 and D6 the owner left to this
plan.

| # | Question | Decision |
| --- | --- | --- |
| D1 | What is the Forge? | The software factory. The Agent Lab builds agents, the Forge puts them to work turning agentic workflows into production-grade software, and the Gym makes them better. It isn't a git host. See [the Forge](../forge/README.md). |
| D2 | Where does the UI live? | Everything is planned for the web app first, and the desktop app reuses it. That includes the browser beat: the web gets a pull-request view and a preview panel ([R4](#f-the-forge)). The desktop's native browser pane is the later, reused version. |
| D3 | One big machine or several? | Fanning out across several devices matters most, and different mixes must work. Every stage of a split model is its own Pylon, one GPU each, from the start. One machine with eight GPUs is just eight stage Pylons that happen to share a host. |
| D4 | Cloud budget | Up to a few thousand dollars of Google Compute. This plan caps it at $3,000 ([budget](#cloud-budget)). |
| D5 | How is a request marked public? | Private by default. An agent's owner can set "show work in the Grid" on the agent, and the composer can set it per request. Sensitive requests are never public. The flag travels sealed inside the request. Only chunks a job marks public carry text the world shows. Private work appears only as activity: which Pylons are busy, and for how long. |
| D6 | When does the attested Pylon answer? | Only for sensitive turns: those the person marks with the composer's lock, or that an agent's policy marks sensitive. Ordinary routing decisions stay on the regular Clef Pylon, so normal turns don't wait 8 seconds. The always-on TDX Pylon (`oa-att-tdx-1`) is the secure Pylon in the Bay day to day. The H100 (`oa-att-h100-1`) wakes on an owner command for recording windows and stops itself after 15 idle minutes, as it does today. |

## What exists today

| Area | State on 2026-10-20 | Where |
| --- | --- | --- |
| Grid on the web | `/home` paints the Verse Grid (`everglade-web`, WebGPU or WebGL2). It has live Pylon pillars from NIP-PYLON beacons and receipts, and one terminal you walk up to. The terminal opens the GPUI chat. `/` is unchanged. | #12345, #12352, #12353, #12356, #12362 |
| Pylon router | `/home` terminal turns go to a live Pylon over encrypted NIP-CJ. If a Clef Pylon is live, a typed Clef decision picks the Pylon. If no Pylon answers, hosted chat does. | `crates/pylon/src/router.rs`, #12366 |
| Pylon network | `pylon up/down/status` with a supervisor, beacons, receipts, pool aggregates and checkers. Three peers in the runbook: the owner's Mac (Metal), `coderos-4080-net` (CUDA) and `oa-pylon-gpu-1` (GCE L4). | `docs/compute/pylon-network-runbook.md` |
| Serial DeepSeek serving | R1-Distill-Qwen-1.5B: DS0 qualified, then serial Pylon (27/27), automatic recovery (13/13) and the `pylon up` profile (#12367). Native Qwen2 CUDA to replace llama.cpp is in progress. | #12358, #12360, #12364, #12367, #12368 |
| Attested Pylon | `pylon serve --attested` in Confidential Space (TDX). It runs sealed NIP-DEC decisions on Clef. `oa-att-tdx-1` is always on (CPU, about 8 s per decision). `oa-att-h100-1` is a spot H100 that is usually stopped and takes about 4 minutes to wake. `/att` shows a verified round in a separate scene. It publishes no NIP-PYLON beacon, so the Grid can't see it. | `crates/pylon/src/attested.rs`, `docs/security/private-inference.md` |
| Agent Lab | A GPUI page (web and desktop). It lists computers and their Codex and Claude runtimes, opens task chats, and has templates. Foreman is a template with check-ins, standing authority, a decision log and a GCE fan-out, and it lands work through `openagents land`. AgentGraph draws an agent's structure. | `crates/agent-lab`, `docs/foreman/`, #12285 |
| Forge | Nothing in this repository except the `Forge` trait that opens draft pull requests (`crates/coder-task/src/task/publish.rs`). An April 2026 private control plane exists as reference only. | [the Forge](../forge/README.md#where-this-came-from) |
| Agents in a world | Only the desktop Everglade studio walks agent seats between stations. Nothing maps agents to areas of a repository. | `docs/verse/agent-studio.md` |
| Gym | The Gym building stands in the Grid, but its boards are empty on the web. Results show on agent pages. | `crates/verse-gym`, `docs/verse/gym.md` |
| Payments | Paid Pylon jobs over NIP-X402 and a broker are tested on fake Lightning only. Lightning addresses and earnings pages are landing (#12307). Mainnet needs the owner's grant. | `crates/pylon/src/paid.rs`, `NEEDS_OWNER.md` |

## The demo, beat by beat

Each beat lists what must be true for the owner to record it honestly. The
epic codes refer to [the workstreams](#workstreams).

| # | Beat | Real today | Missing | Epics |
| --- | --- | --- | --- | --- |
| 1 | Sign in at openagents.com and land in the Grid. | `/home` shows the Grid. | Signed-in `/` opens the Grid. Login keeps the world loaded. | G1 |
| 2 | The Pylon Bay shows the Mac, Linux, a basic cloud GPU and a secure GPU. | Three peers can beacon. Pillars are an HTML overlay. | A Bay place in the Grid. Pylons drawn as Verse geometry by hardware class. The attested TDX Pylon publishes a beacon and appears as the secure one. | G2, N2 |
| 3 | At the Pylon Terminal, a locked (sensitive) request goes encrypted to Clef in the enclave, which decrypts it, decides, and returns. | The router makes a real Clef decision and sends an encrypted job. The attested path is separate. | A lock in the composer. Sensitive turns go to the attested Clef. Packets drawn from the router's real steps: sealed packet, relay, enclave decrypt, answer. Wake the H100 for the recording. | N2, N3, G3 |
| 4 | "Can I run DeepSeek V4.1 Flash?" Several new Pylons come online from cloud credits. | Nothing. | Native V4.1 in Psionic as stage Pylons, one GPU each, and a one-command, cost-capped launch whose beacons arrive live. | P1, P2, P3, G4 |
| 5 | A big prompt goes to DeepSeek. Public tokens stream above the Pylons as it works. | Nothing. Jobs aren't streamed and are capped at 16 KiB and 90 s. | Model choice in the router. Streamed partial results. Longer jobs with larger inputs. The public flag from D5. | N1, N4, G4 |
| 6 | DeepSeek V4.1 Flash is now a primitive I can use. | Beacons name models. | A catalog of network primitives (inference by model, decisions, attested decisions) that the Agent Lab, the Forge and Coder can bind to. | N5 |
| 7 | The Agent Lab: connect Pylons, my Codex and Claude Code, and define an agent with a routing policy. | The Agent Lab page lists computers and runtimes. | An Agent Lab place in the Grid. Resource bindings for network primitives. A per-step routing policy (sensitive, bulk, hard). Pylon models as a provider for Microcoder and Coder. | L1, L2, L3 |
| 8 | In the Forge, Foreman loads the OpenAgents repository and spawns agents. You see where in the code they work, and they talk to each other. | Foreman fans out on GCE. AgentGraph exists. The studio exists on desktop only. | Forge records and a work order loop. A repository floor built from the code graph. Worker avatars placed by the files they touch. Visible messages between workers. Live events streamed to the web. | F1–F3 |
| 9 | It ends in a pull request, shown in a browser panel. | Foreman lands through `openagents land`. The desktop has a browser pane. The web has none. | A delivery with evidence and a receipt. A web pull-request view and preview panel. | F4, F5 |
| 10 | "This is single-player. Others will join." | `/verse` has presence. `/home` doesn't. | Presence in the home Grid. A path for others' Pylons and harnesses to join. Paid jobs (shown as a promise, not claimed). | M1, M2, M3 |

## Workstreams

### P: Psionic and DeepSeek

- **P0: Native serving foundation.** Finish #12368 (native Qwen2 CUDA for the
  R1 distill). Generalize managed recovery from llama.cpp to a native Psionic
  worker. Close S2's real HTTP-disconnect check. This proves the
  supervisor-plus-native-worker shape that every stage Pylon reuses.
- **P1: V4.1-Flash native.** Steps V0–V5 of
  [the V4.1 plan](../psionic/deepseek-v41-native.md#build-order): oracle
  fixtures, the `deepseek-recipe` protocol adapter, the safetensors loader,
  CPU components, the CPU full model, and CUDA kernels.
- **P2: Stage Pylons.** Steps V6–V8: one model group fanned out as one
  stage Pylon per GPU, with typed stage state, stage packages, readiness and
  fencing. The same protocol runs over loopback on one machine and over the
  network across machines, so mixed configurations are a placement choice,
  not new code.
- **P3: Cloud launch.** One owner command creates a labeled, cost-capped,
  expiring GCE group in a chosen configuration, waits for the stages to be
  ready, and publishes their beacons. It stops on expiry, on a spend cap, or
  on `down`. Spot loss shows as Pylons leaving, not as a hang.

### N: Pylon network

- **N1: Jobs for agent work.** Larger inputs (sealed blobs referenced by hash
  instead of 16 KiB inline), streamed partial results over NIP-CJ feedback,
  the D5 public flag and public chunks, and deadlines declared per model in
  the beacon. This is a NIP-CJ and NIP-PYLON revision with fixtures, not a
  timeout change.
- **N2: Attested Pylons in the field.** The attested Pylon publishes a
  NIP-PYLON beacon that links its NIP-ATT endpoint. `RelayField` and the
  router show it. Sensitive turns (D6) go to it. An owner command wakes the
  H100 for a recording window.
- **N3: Route trace events.** The router emits a typed trace per turn:
  decision requested, sealed, relayed, opened in the enclave, decided, job
  sent, tokens, answered. The Grid draws only from these events.
- **N4: Model-aware routing.** The router picks by requested model or
  capability, not only by free slots and tier. Clef's choice includes "the
  big model" when a prompt needs it.
- **N5: Primitive catalog.** One typed list of what the network offers:
  inference by model id and digest, decisions, attested decisions. Built from
  verified beacons, with measured latency and price. The Agent Lab, the Forge,
  Coder and the Grid read it.

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
  turns show only activity.
- **G5: Live Gym boards on the web.** Fill the Grid's Gym boards from
  published Gym results, including Pylon model benchmarks and Forge outcomes.

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

### F: The Forge

[The Forge](../forge/README.md) describes the objects and the loop.

- **F1: Records.** Work order, run, evidence bundle, verification report and
  delivery receipt, with transitions, in a `forge` crate over `agent-lab` and
  `coder-task`.
- **F2: The work order loop.** Foreman takes a work order, fans out workers,
  collects evidence, runs the checks and a reviewer, and delivers a draft
  pull request with a receipt.
- **F3: The Forge floor in the Grid.** A Forge place with the work order
  board. A loaded repository is a floor laid out from `crates/code-graph`
  (crates as districts, modules as buildings). Worker avatars stand at the
  code their real events name, and messages between Foreman and its workers
  show as exchanges.
- **F4: Deliveries on the web.** A GPUI view of a run's evidence, its
  verification and its pull request. A preview panel shows the pull request
  and, where the work order has one, a running preview of the changed app.
- **F5: Outcomes to the Gym.** Each delivery writes a Gym result. A Gym board
  shows Forge outcomes by agent and by model.

### M: Multiplayer and money

- **M1: Presence on the home Grid.** Reuse `/verse` presence.
- **M2: Others' Pylons and harnesses.** Onboarding for a contributor's Pylon
  is `pylon up`. Sharing a harness with others is a new authority question
  and needs a design before code.
- **M3: Paid jobs.** Run the existing NIP-X402 path with a real wallet under
  the owner's grant. Show sats in the Grid only from real receipts.

## Order and milestones

The critical path is the model: P0, then P1, then P2 and P3. Everything in the
Grid, the Lab and the Forge can proceed in parallel against the R1 distill and
Clef, which already run on real Pylons.

| Milestone | Contents | What it proves |
| --- | --- | --- |
| M-A: The Bay, honestly | G1, G2, N2, N3, G3 | Beats 1–3 recordable with today's Pylons and Clef |
| M-B: V4.1 reference | P0, P1 | One correct native V4.1 answer, checked against DeepSeek's reference |
| M-C: The swarm | P2, P3, N1, N4, G4 | Beats 4–5 with stage Pylons on several machines and streamed public tokens |
| M-D: Composition | N5, L1, L2, L3 | Beats 6–7: an agent that mixes network models and harnesses |
| M-E: The Forge | F1–F5 | Beats 8–9: a verified pull request from a watched crew |
| M-F: Others join | M1, M2, M3, G5 | Beat 10, and the network story after the episode |

M-A, M-B and M-E can start at once and don't depend on each other. M-C needs
M-B. M-D needs N1 for a useful context size.

## Cloud budget

The owner allowed up to a few thousand dollars of Google Compute. This plan
caps it at **$3,000**, using spot capacity where it exists:

| Use | Machines | Hours | Estimate |
| --- | --- | --- | --- |
| V0 oracle run | One `a4-highgpu-8g` (8× B200) spot, or a `g4-standard-384` | 8 | About $320 |
| V4–V5 CPU and kernel work on real tensors | One `g4-standard-384` (8× RTX PRO 6000, 1,440 GiB host RAM) | 25 | About $900 |
| V6–V8 stage Pylons and recordings | 4–8 single-GPU G4 machines, or one `g4-standard-384` | 30 | About $1,100 |
| Reserve | Retries, spot loss, the H100 attested Pylon during recording | | About $680 |

Rules for every machine:

- Label it with the run (`agent-run.py label-vm`), so the reaper can stop it.
- Give it an expiry and stop it when its bounded task ends.
- Record its machine type, hours and cost in the run's evidence.
- Check spot capacity and price on the day; G4 spot pricing isn't recorded
  yet.
- Never use the production Clef GPUs (`clef-decisions-h100`,
  `clef-decisions-a100`) or `oa-pylon-gpu-1` as spare capacity.
- Set a billing budget alert at $1,500 and $2,500. Creating the alert may need
  the owner's billing access; if so, it goes in `NEEDS_OWNER.md`.

## Risks

- **V4.1 is a large new graph.** Hyper-connections, three attention modes,
  cross-layer sharing, the indexer, Engram and FP4 experts are each new to
  Psionic. The golden-activation oracle keeps every step checkable, but the
  total is weeks of work, not days.
- **Stage state across machines.** V4.1's later layers read KV, indexer and
  candidate state from earlier layers. Fanning out across many machines
  means sending some of that state between stages every token. Measure it
  before promising latency.
- **Spot capacity.** A4, G4 and H200 capacity in our regions is
  quota-limited and spot-only. A recording needs a reserved window or a
  fallback plan.
- **Context and deadlines.** Agent work on V4.1 isn't useful until N1 lands.
  Raising limits touches the protocol and every Pylon. Version the change.
- **Tool use.** Without DS1's tool-call parsing or R2's constrained
  decoding, V4.1 on Pylons can answer but can't act as a Forge worker.
- **The Forge's history.** It was started four times and parked each time,
  mostly for scope. The first version here is deliberately one loop over
  existing crates, with no git hosting.
