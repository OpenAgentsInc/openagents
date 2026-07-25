# Exo Teardown — 2026-07-25

Read-only architecture and product audit of the public `exo-explore/exo`
source tree at an exact commit in the local reference clone
`~/work/projects/repos/exo`. Nothing tracked was modified and nothing was
executed: no node ran, no cluster formed, no model loaded, no benchmark
reproduced. Three source passes covered core architecture, the API and
product surfaces, and quality/operations posture. Exo is exo labs'
Apache-2.0 system that "connects all your devices into an AI cluster" —
automatic discovery, RDMA over Thunderbolt 5, topology-aware model
partitioning, and a broad OpenAI/Claude/Ollama-compatible API, aimed today
almost entirely at Apple Silicon Macs. [source]

The comparison targets are the Omega desktop surface (the Zed fork, where
the owner has directed direct exo support), the OpenAgents provider and lane
model (`language_models` providers in Omega, harness lanes and the engine
protocol in `omega-effectd`), Psionic as the owned execution substrate, and
the openagents #9258 public NIP-29 chat channel, where an exo agent adapter
is named as an interoperability target.

**Pin:** `b5375f8cee4368d09e1ce96a56b9f81fb0bc81aa` (2026-06-22, "Add Kimi
K2.7-Code model card (#2167)"). Python package version `0.3.70`
(`pyproject.toml:3`) against release tags at `v1.0.71` — package and product
versions have diverged by a whole major. [source]

## Summary

Exo makes one bet: a cluster of consumer devices should feel like one AI
computer. Every node runs the same binary — nodes find each other by custom
IPv6 UDP multicast — a bully-style election picks a master — the master plans
model placement over a ring topology — MLX executes shards with tensor or
pipeline parallelism, over TCP or RDMA-over-Thunderbolt, and a single HTTP
port speaks four API dialects so that existing tools — Claude Code, Codex,
OpenCode, Ollama clients — can point at the cluster with a one-line config
change. [source]

```text
   Mac #1                    Mac #2                    Mac #3
 ┌───────────┐            ┌───────────┐            ┌───────────┐
 │ exo node   │◄─UDPv6──► │ exo node   │◄─UDPv6──► │ exo node   │  discovery :52413
 │ (full set) │◄─zenoh──► │ (full set) │◄─zenoh──► │ (full set) │  transport :52414 (TCP)
 └─────┬─────┘            └─────┬─────┘            └─────┬─────┘
       │  election (bully) → one Master indexes events, plans placement
       │  topics: global/local events · commands · elections · downloads
       ▼
  MLX engines: ring (TCP) or JACCL (RDMA/TB5) collectives
  tensor ∥ / pipeline ∥ / CFG ∥ · KV prefix cache · prefill/decode split
       ▼
  HTTP :52415 (0.0.0.0, no auth, CORS *): OpenAI ∙ Claude ∙ Responses ∙
  Ollama dialects · SSE · dashboard at "/" · /state · /instance lifecycle
```

The system is two projects sharing one history. Exo 1.0 (June 2024 – March
2025, Python + gRPC) died with two consecutive zero-commit months. Exo 2.0
is a from-scratch rewrite begun 2025-06-28 on a new stack: event sourcing
over typed pub/sub, zenoh transport behind a small PyO3 Rust crate, MLX-only
engines, a SvelteKit dashboard, and a signed SwiftUI menu-bar app with
Sparkle auto-update. The often-cited "Rust rewrite" is real but narrow:
~1.5k lines of Rust (networking + pidfile) against ~56k lines of strictly
typed Python that hold all of the intelligence — election, state, placement,
sharding, inference, API. [source]

What exo is best at, on this read: turning heterogeneous Macs into one
OpenAI-compatible endpoint with genuinely sophisticated single-tenant
inference (per-architecture tensor-parallel sharding strategies, a real KV
prefix cache with memory-pressure eviction and multimodal safety, prefill/
decode disaggregation over a msgpack wire protocol), and shipping it with
release engineering — two-stage code signing, notarization, appcast
signing — that is stronger than most projects of its age. [inferred]

What it is not: multi-tenant, authenticated, cross-subnet, tested in public
CI at the distributed layer, or supported anywhere but four Apple Silicon
SKUs. Its Tier 1 platform list is Mac Studio M3 Ultra, Mac Mini M4 Pro, and
MacBook Pro M5/M4 Max — Tier 2 and Tier 3 are literally empty sections.
[source]

## 1. Repo identity and posture

- Apache-2.0, `Copyright 2025 Exo Technologies Ltd`. No CLA, no DCO, no
  SECURITY.md, no code of conduct. Global three-person CODEOWNERS. Bus
  factor ≈ 5, all exo labs employees (~93% of 2026 commits). [source]
- Language split: ~56k Python (`src/`, `bench/`, `tools/`), ~24.6k
  Svelte/TS dashboard, ~6.3k Swift app, ~1.5k Rust. Cargo workspace members
  `rust/exo_rs` and `rust/networking` — uv workspace binds `exo-rs` as a
  Python dependency. Python is hard-pinned to `==3.13.*`. [source]
- Nix is a first-class build path (`nix run .#exo`, Cachix caches, a Metal
  toolchain overlay) — `justfile` drives fmt/lint/test/package. [source]
- The agent-contribution posture is unusually explicit: `AGENTS.md` (with
  `CLAUDE.md` a symlink to it) carries build commands, a mandatory
  pre-commit gate (`basedpyright && ruff check && nix fmt && pytest`), an
  architecture tour, and even a Playwright screenshot recipe with a
  commit-then-delete workaround for attaching images to PRs. `RULES.md`
  mandates pure functions, injected effect handlers, frozen strict Pydantic
  models, `NewType` IDs, no new dependencies without asking — and the
  enforcement is real (`basedpyright` strict with `reportAny = "error"` —
  ~60 hand-picked clippy restriction lints with `unwrap_used` and `panic`
  denied). Three near-identical copies of the rules exist (`RULES.md`,
  `.clauderules`, `.cursorrules`). [source]
- Commit-message policy from RULES.md is de facto unenforced: only ~28% of
  recent commits follow any conventional prefix, and those mostly use
  Angular vocabulary rather than the repo's own. [source]

## 2. The distributed system

**Discovery and transport.** Commit `09f9ea31` (2026-06-03) replaced libp2p
with zenoh — the largest architectural change in the recent window, and it
deleted `docs/architecture.md` without replacement, leaving `AGENTS.md` as
the de facto spec. Every node runs a zenoh router-mode session listening on
TCP :52414 with zenoh's own multicast scouting disabled — discovery is exo's
own beacon — IPv6 UDP multicast to `ff12::e0a1:de89` on :52413, magic bytes
`EXO`, a 1-second tick, live interface re-joins via `netwatcher` (this is
what makes Thunderbolt bridges hot-pluggable), and namespace isolation as
the first 8 bytes of `blake3(namespace)`. A node only dials peers with a
lower ZenohId, avoiding duplicate links. The namespace defaults to the exo
version string, so version skew silently partitions a cluster. [source]

Consequences an integrator inherits: the cluster requires IPv6 multicast on
a shared L2 segment — it does not traverse subnets or most VPNs — there is no
Tailscale support, and the manual escape hatch is dead — `--bootstrap-peers`
raises "temporarily removed," and `TODO.md` opens with "EXO_BOOTSTRAP_PEERS
is currently broken." [source]

**Election and state.** Every node constructs a Master at startup and a
bully-style election (compare clock → seniority → commands-seen → node id —
3-second timeout — winners accumulate seniority — `--force-master` sets
seniority to 1,000,000) decides who keeps it. The master is the sole event
indexer: a pure `apply()` reducer folds a discriminated event union into a
frozen `State` (instances, runners, downloads, tasks, topology, and per-node
telemetry maps), persisted in an on-disk event log. An `EventRouter` handles
ordering with NACK/refetch and exponential backoff. Two defects are known
and accepted in code comments: the topic router cannot prevent self-
feedback of election messages, and the promote/demote path recreates the
Worker, DownloadCoordinator, and EventRouter wholesale ("This function needs
refactoring generally"). [source]

**Serialization** is Pydantic JSON over zenoh topics — not msgpack or
protobuf — except the prefill/decode KV wire protocol, which is msgspec
msgpack. The Python-facing Rust API still carries libp2p vocabulary
(`gossipsub_publish`) — `swarm.rs` self-describes as a compat shim. The
zenoh storage-manager plugin is loaded with a replicated in-memory store
that nothing in Python consumes yet — staged, unwired capability. [source]

## 3. Inference

**Engines: MLX only.** No tinygrad (exo 1.0's second engine is gone), no
candle, no llama.cpp. An `Engine`/`Builder` abstraction exists with exactly
two implementers: MLX text and mflux image (behind a default-off flag).
Backends `MlxMetal | MlxCuda | MlxCpu`. Every core ML dependency is a
personal fork on a branch: `mlx` (with the JACCL RDMA fixes), `mlx-lm`, and
`mflux`, plus hand-hosted CUDA wheels fetched from personal GitHub release
URLs. Nothing on the inference critical path runs upstream code. [source]

**Sharding.** Three placements: pipeline (layers allocated proportionally
to each node's available RAM, validated per node with GB-precise errors),
tensor parallel (all ranks hold all layers — intra-layer splits dispatched
to eleven per-architecture strategies — Llama, DeepSeek V3/V4, MiniMax,
GLM-4 MoE variants, Qwen, GPT-OSS, Step 3.5, Nemotron-H, Gemma 4 — over
`mx.distributed` all_sum/all_gather collectives, quantization-aware down to
scales and biases), and a CFG-parallel image mode running positive and
negative prompts as two pipeline groups. Placement targets ring cycles in a
`rustworkx` topology graph because the MLX ring and JACCL collectives need
a physical ring. The tensor-parallel eligibility gate carries the honest
admission "the condition here for tensor parallel is not correct, but it
works good enough for now," with hardcoded per-model exceptions. [source]

**RDMA.** JACCL is the RDMA-over-Thunderbolt path (Metal-only), gated at
both endpoints: the cycle must be an RDMA cycle and every node must report
`nodeRdmaCtl.enabled`, with absent telemetry defaulting to ineligible.
[source]

**KV cache.** A real prefix cache: per-group keying, prefix matching,
memory-pressure eviction, heterogeneous cache types (rotating, arrays,
DeepSeek V4 compressor branches), SSM/Mamba state snapshots with
non-trimmable-cache guards, and multimodal safety that refuses to reuse a
text prefix across different images. Prefill/decode disaggregation streams
KV per layer between instances over msgpack, master-routed. [source]

**Models.** 123 text + 18 image model cards as TOML files — the primary
community contribution surface. A card is declarative metadata (layers,
hidden size, KV heads, storage size as a hand-entered byte count, backends,
capabilities, context length, reasoning dialect, sampling defaults) — adding
frontier models (the pinned HEAD adds Kimi K2.7-Code at ~595 GB with a
vision tower hosted on a contributor's personal HF account) is a no-code
PR. Unknown models are auto-carded from HF `config.json` with an
architecture allowlist for tensor support. Placement math uses storage size
prorated by layers and ignores KV cache and activation headroom — admitted
in `TODO.md` ("memory pressure instead of memory used"). Downloads are
resumable, deduplicated, cluster-coordinated, and currently un-smart: an
unconditional early return makes every node download every file of every
model. [source]

## 4. API and product surfaces

**One port, four dialects, no auth.** Everything is Python/FastAPI on
:52415, bound to `0.0.0.0`, with CORS `allow_origins=["*"]` plus
`allow_credentials=True`, and zero inbound authentication of any kind — the
only Bearer token in the tree is exo's outbound HuggingFace token. The
dashboard is served from `/` on the same origin. [source]

Surfaces: OpenAI `/v1/chat/completions` (SSE with `data: [DONE]`, real
tool-call deltas, `parallel_tool_calls`), Anthropic `/v1/messages`
(tool_use blocks, stop_reason), OpenAI Responses `/v1/responses` (function
call items, an `mcp_list_tools` item type — exo is not an MCP host, just
schema-compatible), and a triple-aliased Ollama surface including
bug-compatibility routes. Non-standard SSE comment channels carry prefill
progress and generation stats in a way third-party clients silently drop.
Image generation, benchmarking variants, HF search proxy, traces, and
onboarding routes round it out. There is no `/v1/embeddings`. [source]

**The lifecycle gotcha.** `GET /v1/models` returns the entire 123-card
catalog regardless of what is downloaded or loaded, and chat 404s unless an
instance is already running for the model. The real flow is
`GET /instance/previews` → `POST /instance` (or `/place_instance`) →
`GET /instance/await` (SSE) → chat. Only the Ollama tags route reflects
downloaded state. Any integrating IDE must own this lifecycle plus download
progress. [source]

**Ops surface is poll-only.** `GET /state` is the single source of truth (a
camelCase Pydantic dump of instances, runners, per-node download progress,
tasks, topology, and hardware telemetry) — `GET /events` dumps the entire
event log as one JSON array — not incremental, not SSE. The only streaming
ops endpoint is `/instance/await`. Exo's own macOS app polls `/state` at
2 Hz and had to disable URLSession caching because that poll rate wrote
~500–620 KB/s to disk — a documented warning for any supervisor. [source]

**Dashboard and app.** The SvelteKit dashboard (topology graph with
per-node chip/RAM/GPU/temperature/power, chat with token heatmap, downloads,
traces, disaggregation UI) polls the same API — its `/integrations` page is
the de facto integration doc, generating configs for exactly eight tools —
Claude Code, OpenCode, Codex, OpenClaw, Pi, Open WebUI, n8n, Firefox — and
encoding real per-model knowledge (reasoning-dialect wiring, the boolean
`enable_thinking` toggle standing in for effort levels). Zed is not on the
list. The macOS menu-bar app does not embed exo — it spawns the PyInstaller
binary as a child process, injects settings as env vars, polls the same
HTTP API, auto-updates via Sparkle (a beta Sparkle build, 15-minute check
interval), and — critically for any bundling idea — installs a root
LaunchDaemon that destroys `bridge0` and rewrites macOS network locations
to make Thunderbolt RDMA work. App sandbox is off. [source]

## 5. Quality, security, and claims

**Tests.** ~410 Python unit-test functions colocated under `src/`, a real
multi-node integration harness under `tests/` — which requires `eco`, an
unpublished exo-labs-internal cluster-reservation CLI, making the entire
distributed suite dead code for outsiders — and one Rust test. CI runs
pytest on macOS only, with `-m "not slow"` and `--ignore=tests`, so all
real inference coverage (marked slow) and all cluster coverage are off by
default everywhere. The tensor-parallel bit-exactness test is skipped with
"TP=2 is currently very different to TP=1. This test will not pass."
`CONTRIBUTING.md` is candid: "EXO relies heavily on manual testing at this
point." [source]

**Security posture.** No inbound auth, open CORS with credentials, all-
interfaces bind, plaintext TCP zenoh with an admin space enabled, and a
discovery namespace that is a tag, not a secret — any host on the segment
that knows the namespace joins the cluster and can be assigned shards, or
can drive, load, and delete models over HTTP. No SECURITY.md, no
firewall/exposure guidance anywhere in the docs. The `trust_remote_code`
documentation contradicts the code: three doc locations claim it defaults
to false, while the `ModelCard` field default, the MLX constant, and the
vision path are all `True` — only user-added custom cards default to false.
Telemetry is genuinely absent (the only outbound path is an opt-in bug
report), and the one CVE fix in history arrived from an external scanner
bot nine minutes before the pinned HEAD — there is no dependency scanning
in CI. [source]

**Claims versus evidence.** The README's headline numbers — "99% reduction
in latency" from RDMA, "1.8x speedup on 2 devices and 3.2x on 4" from
tensor parallelism — have no in-repo evidence — the benchmarks section is
three screenshots of a third party's blog post. Meanwhile `bench/` contains
a genuinely rigorous harness (binary-search prompt construction to exact
token counts, EOS-banning logits processor, first-token-excluded decode
rates, thread-barrier concurrency, a self-critical METHODOLOGY.md) that
publishes no results. The topology-aware placement claim runs on estimated
link speeds — profiling real latency/bandwidth is an open TODO. [source]

**Dependency posture.** The sharpest structural risk: 24 zenoh crates
patched to one contributor's personal fork branch — mlx/mlx-lm/mflux forks
on branches (one resolved to a dev build) — `pidfile-rs` as a git dependency
with no rev pinned at all — a beta Sparkle in the auto-update path. Nothing
in the critical path — transport, inference, updates — runs released
upstream code. [source]

## 6. Comparison with the OpenAgents estate

| Dimension | exo | OpenAgents today |
| --- | --- | --- |
| Unit of product | A LAN cluster behaving as one model endpoint | One agent (Omega Agent) over disclosed executors — Khala as the network orchestrator |
| Inference substrate | MLX forks, single tenant, no auth | Psionic (owned), Khala gateway lanes (metered, authed, multi-tenant) |
| State plane | Event-sourced, master-indexed, LAN pub/sub | Event-sourced engine (`omega-effectd`), Cloud SQL authority, Khala Sync/Nostr projections |
| API posture | Four-dialect compatibility wedge, zero auth | Typed authority, scoped tokens, receipts |
| Distribution | Signed menu-bar appliance + root network daemon | Signed Omega RC discipline — no host mutation without deltas |
| Verification | Manual testing admitted — claims unbacked in-repo | Receipts, conformance matrices, promise gates |

The overlap is real but complementary: exo solves "many local devices, one
model" — a capability OpenAgents does not own and does not need to build —
while OpenAgents owns everything exo lacks: identity, authority, metering,
receipts, multi-tenancy, and verification. [inferred]

## 7. What Omega should do — direct exo support

Owner direction (2026-07-25): add direct exo support inside Omega. The
teardown supports a concrete shape, in three layers, honoring the standing
Omega laws (external systems keep their own homes and credentials — GPUI is
projection and command entry — every default change is a tested delta).

**Layer 1 — the provider, nearly free.** [EXISTS in exo, NEEDS BUILD in
Omega] Exo is a legitimate OpenAI-compatible SSE endpoint with real
tool-call support at `http://127.0.0.1:52415/v1` (and a real Anthropic
surface at `/v1/messages`). Omega's `language_models` crate already carries
eighteen direct providers including `open_ai_compatible` — a named `exo`
provider is a thin specialization: default base URL, dummy key, model
listing from `/v1/models` (rendering exo's genuinely useful card fields —
capabilities, context length, reasoning dialect, the boolean
`enable_thinking` toggle rather than effort levels), and tool-call format
already compatible. Exo's own integrations page ships configs for eight
tools and not Zed — being the first first-class Zed-family integration is
cheap distribution for both sides.

**Layer 2 — the lifecycle, the real product work.** [NEEDS BUILD] A chat
request 404s until an instance exists, so a naive provider entry fails the
first user. Omega must own the placement dance — previews → create instance
→ await ready (SSE) — plus download initiation and progress, driven from
`/state` polling at a gentle rate (exo's own app documents the disk-cache
hazard of 2 Hz polling — poll coarser, cache nothing). This belongs in
`omega-effectd` as typed capability probes and commands, consistent with the
engine owning readiness and capacity truth for every other lane — GPUI
renders instance state, download progress, and the cluster topology
projection, and never keeps a second durable copy. Exo's cluster remains
its own authority — Omega never edits exo's config, models directory, or
namespace, exactly as it never touches a Codex home.

**Layer 3 — the lane, later and gated.** [SPECULATION] Once the provider
and lifecycle exist, exo becomes an executor-class candidate in the Omega
Agent routing table (a local-cluster model lane with declared capabilities
and readiness from the engine), and — separately, behind Khala's own
admission — a candidate own-capacity lane the way local Codex capacity is
one today. Nothing in this teardown grants that — it needs the routing
packets and Khala-side admission on their own gates.

**Security boundary, stated hard.** Omega integrates exo as an
*unauthenticated LAN-wide service the user chose to run*. The integration
must: default to loopback URLs — surface (not hide) the fact that the
endpoint has no auth and the cluster namespace is not a secret — never
expose or proxy the exo API off-machine through any Omega surface — and
never bundle or auto-install the exo menu-bar app — the app installs a root
LaunchDaemon that rewrites network locations, which is host mutation Omega
must not perform silently. Detect a running node — offer install guidance —
let the user own the daemon decision. Attribution honesty applies: threads
executed via exo carry the executor line naming exo, the model, and the
instance. [inferred]

**The #9258 seam, unchanged.** The public NIP-29 chat issue names an exo
agent adapter as an interoperability target and is explicit that the
channel is a generic Nostr client contract — any compatible relay, no
OpenAuth coupling, exo joins with its own key. Nothing in this teardown
changes that boundary, and nothing in exo does either: exo ships no agent
and no Nostr support today, so the adapter is net-new work on our side of
the fence (or upstream), speaking the frozen `openagents.public_chat.v1`
profile like any other client. Keep the two integrations separate: the
Omega provider consumes exo's HTTP API — the chat adapter is an independent
Nostr client that happens to be backed by exo capacity. [source]

**Refuse list.**

1. Do not bundle the exo macOS app or replicate its root LaunchDaemon
   network mutation inside Omega.
2. Do not inherit exo's exposure defaults into any Omega surface — no
   proxying :52415 beyond loopback, no CORS-open passthroughs.
3. Do not treat the 123-card catalog as a tested model matrix, or the
   README speedup/latency numbers as evidence — cite measured receipts or
   nothing.
4. Do not build on exo's fork-pinned dependency graph for anything we
   ship — exo is a runtime peer reached over HTTP, never a build
   dependency.
5. Do not route work through an exo cluster whose namespace spans machines
   the owner does not control — the namespace is isolation, not security.
6. Do not wait on exo for embeddings — there are none — semantic features
   source elsewhere.

## 8. Lessons worth stealing, beyond the integration

1. **TOML model cards as the contribution surface.** Frontier-model support
   as a 36-line metadata PR with no code change is a superb community
   wedge — our provider/model catalogs should stay this declarative.
2. **The integrations page as living documentation.** Generating exact
   copy-paste configs per client, with per-model reasoning-dialect wiring
   encoded, beats prose docs. An Omega settings surface could do the same
   for its providers.
3. **Interface-watching discovery.** The netwatcher-driven multicast
   re-join is what makes cable hot-plug feel magical — any future local
   device-mesh work should treat interface churn as a first-class event.
4. **The bench methodology, not the bench claims.** EOS-banned exact-length
   generation, first-token-excluded decode rates, and barrier-synchronized
   concurrency are directly reusable measurement discipline for Psionic and
   Khala lane benchmarks.
5. **The cautionary pair.** Strict typing enforced by CI (basedpyright
   strict, clippy restriction lints) coexisting with disabled correctness
   tests and unbacked headline claims is a reminder that gate discipline
   and claim discipline are separate systems — we keep both.

## 9. Watch items

- The zenoh fork consolidation: whether exo's 24-crate patch set lands
  upstream or hardens into a permanent fork determines transport stability
  for any long-lived integration.
- Linux/CUDA Tier 1 promotion (DGX Spark is "planned"): would widen the
  Omega integration beyond Mac-only users and change the placement math.
- The `--bootstrap-peers` repair and any Tailscale/cross-subnet story:
  today's L2-multicast-only clustering caps the audience at one LAN.
- Auth on the HTTP API: any future exo auth scheme changes Layer 1 —
  integrate the base-URL/key fields now so it is a config change, not a
  redesign.
- Embeddings: if `/v1/embeddings` ever lands, the local-semantic-search
  calculus changes.
- The version-string discovery namespace: cluster partitions on upgrade are
  a support-load generator for any integration that manages exo instances —
  watch for a stable namespace default.
- exo 1.0 history (tinygrad engine, broader device support) as a reference
  for what 2.0 deliberately dropped — useful when users ask why older
  devices no longer cluster.
