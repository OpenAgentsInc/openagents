# Leyten `c0mpute` + `shard` — Harvest Audit for OpenAgents

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-19. Status: **reference-harvest audit**, not a spec. Point-in-time read of
two external reference repos for ideas/code worth porting into OpenAgents. Reference clones
(read-only, gitignored, not vendored):

- `projects/repos/shard` — HEAD `3ea8a18` ("upgrade to go-libp2p v0.48 + prove DCUtR
  hole-punch in a two-NAT lab (step 2.2)"), `2026-06-19`, Apache-2.0, © leyten.
- `projects/repos/c0mpute` — HEAD `827130a` ("worker: pin model in VRAM (keep_alive) +
  startup auto-update; free prompts for wallet logins"), `2026-06-19`.

Both repos cloned/refreshed successfully — neither is private. (Note: `projects/sync.sh`
mangled the manifest remote to `github.com/<name>.git`; corrected to
`github.com/leyten/<name>.git` locally before fetch. The manifest may need the same fix.)

This audit complements two prior 2026-06-19 docs:

- `psionic/docs/audits/2026-06-19-shard-wan-pipeline-implementation-roadmap.md` — the
  808-line Psionic implementation roadmap that already read shard's `phase0/` and proposed
  a Rust-native cluster inference lane. This audit does **not** restate that roadmap; it
  adds (a) the shard-vs-c0mpute relationship, (b) the **whole-product** inventory of
  c0mpute (which the Psionic roadmap deliberately scoped out), and (c) concrete
  Solana→Bitcoin substitutions for the marketplace/settlement layer.
- `openagents/docs/inference/2026-06-19-decentralized-serving-shard-wan.md` — the product
  fold-in (every-Pylon-serves, shard-WAN supply lane through the gateway).

---

## 1. Are `shard` and `c0mpute` the same thing?

**No — they are two distinct repos with a strict one-way dependency, and they are
explicitly designed that way.** They are not a fork, not a subset; they are an *engine*
and the *network that runs it*.

- **`shard`** is a **pure inference engine**: pipeline-parallel LLM serving across GPUs on
  separate machines (one contiguous layer-block per GPU), with WAN latency hidden by
  speculative decoding + pipelining + direct-return + topology-aware routing. Python
  engine (`phase0/`, `research/`) + a Go libp2p transport sidecar (`sidecar/main.go`).
- **`c0mpute`** is the **network/product**: a permissionless GPU marketplace
  (`c0mpute.ai`) — Next.js web app + API gateway + Socket.io orchestrator + a Node/TS
  provider worker + Solana settlement. Today c0mpute runs *single-GPU* whole-model
  inference across volunteer GPUs (via Ollama); shard is the **next layer** that lets
  those GPUs *swarm* to serve models too big for one card.

The relationship is stated as a hard law in `shard/docs/INTEGRATION.md` §0 and
`shard/STATE.md`:

> **Dependencies point one way: `c0mpute → shard`, never the reverse.** Shard knows
> nothing about `$ZERO`, `privy_id`, USDC, payments, reputation, or the orchestrator.
> "Shard is BitTorrent the protocol; c0mpute is the swarm that speaks it."

This is the single most important architectural takeaway, and it **directly validates the
OpenAgents split**: the engine (→ Psionic) must not import marketplace/identity/payment
concepts; the network brain (identity, money, reputation, scheduler, catalog → OpenAgents
product surfaces + Pylon) consumes the engine's evidence. The shard/c0mpute boundary is
the same boundary as Psionic ↔ (OpenAgents gateway / Pylon / Cloud).

**Maturity asymmetry:** shard's `SERVE` verb is **done** (real WAN receipts); its
`JOIN`/`FORM`/`PROVE`/`PAY` verbs are in progress or planned. c0mpute's marketplace
(orchestration, billing, anti-cheat, payout) is **live and real** for *single-GPU* serving;
its *swarm* integration with shard (signed per-stage receipts, scheduler-formed swarms,
per-node pay) is the not-yet-built seam between the two repos.

---

## 2. Are these two the entirety of the product? — Full component inventory

**No.** "A web app + some sharding components" undersells it. The full surface is a complete
two-sided compute marketplace plus a research-grade distributed-inference engine. Inventory:

### `shard` (the engine)

| Component | File(s) | Real / stub | What it is |
| --- | --- | --- | --- |
| Sealed activation wire | `phase0/wire.py` | **real** | `[u64 len][12B nonce][ChaCha20-Poly1305(JSON-header + raw tensor blobs)]`, pickle-free, PSK from `SHARD_PSK`. |
| PSK-free wire twin | `shard/transport.py` | **real** | Identical codec, no seal (libp2p sidecar supplies crypto/identity). Drop-in via `import shard.transport as wire`. |
| Edge RTT mesh | `phase0/mesh.py` | **real** | App-level TCP-echo RTT (median of 20, tiny 64B payload to measure latency not bandwidth); builds asymmetric matrix + `c_out`/`c_in` depot terms. |
| Topology solver | `shard/topology.py` | **real** | Held-Karp exact (k≤16) min-latency Hamiltonian *path* with coordinator as depot; NN+2-opt heuristic above 16. Pure Python, no deps. |
| N-stage pipeline | `phase0/pipeline.py` | **real** | Loads only assigned layers (rest → HF `device_map="meta"`); per-layer sliding/full causal masks; forward stage 0→…→tail. |
| Per-node KV stage | `phase0/node_kv.py`, `node.py` | **real** | 2-node split with/without KV cache; defines shared `EDGE_ERRORS`, `TransportError`; per-node 0-based `layer_idx` reindex. |
| Spec decode (2-node) | `phase0/specdec.py` | **real** | Draft-K → verify in one traversal → greedy accept `drafts[:n]+[r[n]]`; lazy tail KV crop piggybacked on next verify; adaptive K via EMA. |
| Pipelined spec decode | `phase0/specpipe.py` | **real** | ~840 lines; direct-return tail, `coordinate_pipe` keeps `depth` chunks in flight + async draft; tree variants. The big system. |
| Tree spec core | `phase0/tree.py` | **real** | Flat `tok/par/dep` arrays, ancestor additive mask, `accept_tree` longest-path walk, `gather_cache` via `index_select`. |
| Static-KV CUDA-graph fast verify | `phase0/fastverify.py` | **real** | Fixed `[1,heads,MAXLEN,dim]` cache owns its write index `cp`; **rollback = overwrite at `start`, no crop**; sliding window via mask only; 3× warmup-then-capture graph; tree-on-graph. |
| Run receipt | `phase0/proof_receipt.py` | **real** | Build + verify: token-id SHA-256 oracle + distinct-IP/distinct-GPU-UUID/multi-geo/WAN-min-RTT checklist. |
| libp2p sidecar | `sidecar/main.go` | **real** | Go daemon, per-node ed25519 PeerId, TCP↔libp2p tunnel; DCUtR hole-punch, circuit-relay-v2, AutoNAT, QUIC, relay→direct pre-connect, RELAY/DIRECT conn monitor. |
| GLM-5.2 / gpt-oss research drivers | `research/*.py` (~50 files) | **real (hardware-specific)** | NVFP4/MXFP4 staged loaders, CUDA-graph diff harnesses, draft-compat probes. The "how we got to 30/40 tok/s" lab. |
| Engine module scaffolding | `shard/{scheduler,node,specdec}.py` | **stub** | Typed contracts (`NotImplementedError`) awaiting phase0 logic to be ported in. Only `topology()` is wired. |

Verified WAN receipts on file: gpt-oss-120B MXFP4 ~40 tok/s (3×12-layer RTX 4090 stages +
in-region coordinator, 4 US states); GLM-5.2 744B NVFP4 ~30 tok/s (6× RTX PRO 6000, 6 US
states); and a libp2p parity receipt (44.79 tok/s, **bit-identical** output sha to the
ChaCha-wire run, `tokens_match_sync=True`).

### `c0mpute` (the network/product)

| Layer | File(s) | Solana? | What it is |
| --- | --- | --- | --- |
| Web app (Next.js 16) | `app/*` | — | Chat, dashboard, earn, staking, treasury, settings, referral pages. |
| OpenAI-compatible API gateway | `app/api/v1/{chat/completions,models,balance,images/generations}` | no | `sk-c0mpute-…` keys; bridges HTTP→Socket.io→worker pool with **full tool-call passthrough**; OpenAI↔Ollama shape mapping; SSE streaming. |
| Orchestrator (Socket.io) | `lib/orchestrator/{orchestrator,tools,types}.ts` (~1557 lines) | no | Job queue, worker registry, **weighted-random worker selection (weight = measured tok/s)**, server-side tool exec, safety scan, payment waterfall. The heart. |
| Anti-cheat / reputation | inside `orchestrator.ts` (`maybeDispatchCanary`, `buildCanary`, `checkCoherence`) + `db.ts` | no | **Canary nonce probes** (1-in-15, indistinguishable from real jobs, sent only when queue idle), throughput floor/ceiling, coherence heuristics, persistent SQLite strike/ban. |
| Provider worker (CLI) | `c0mpute-worker/src/*` (`@c0mpute/worker`) | no | Plug-and-play: auto-installs Ollama, VRAM-detect, adaptive `num_ctx`, benchmark self → register tok/s, **`keep_alive:-1` VRAM pinning**, self-update from npm, tool-call loop. Image worker = ComfyUI/Chroma1-HD. |
| Auth | `lib/privy-server.ts`, `providers/PrivyProvider.tsx`, `lib/anon-auth.ts` | identity only | Privy (wallet/social) primary; `cwt_` worker tokens; signed IP-bound anon tokens for free trials. Swappable. |
| Credit ledger | `lib/db.ts` (`addCredits`/`spendCredits`/`refundCredits`) | no | SQLite double-entry, 1 credit = $0.01. |
| Worker earnings ledger | `lib/db.ts` (`recordEarning`/`createWithdrawal`/`markPayout*`) | no | Atomic in-flight-guarded debit; only the *transfer* is Solana. |
| Treasury bucket ledger | `lib/treasury-ledger.ts` | no | USD buckets (buyback / staker_rewards / profit) with **clamp-at-zero liability protection** so user/worker liabilities can't be spent on buybacks. |
| Tokenomics config | `lib/tokenomics.ts` | no | Split/threshold/cap constants (70%/80% worker share, free-prompt caps, keeper hour). |
| Subsidy + staker allowance | `db.ts` + `lib/staker-allowance.ts` | reads chain # | Free-prompt engine + Venice-style "stake → daily free inference," hard-capped pro-rata daily pool. Engine generic; only consumes a "matured stake" number. |
| Staking lot accounting | `lib/staking.ts` | reads chain # | 24h-aging lots, LIFO unstake, pro-rata rewards. Math generic; takes `onChainAmount` input. |
| Referrals / API keys / usage | `lib/referrals.ts`, `db.ts`, `app/api/{referrals,usage,plan}` | no | 5% referral, max-5 keys, usage metering. |
| **Custodial USDC payout** | `lib/payout.ts` | **SOLANA** | `sendUsdc` SPL transfer from treasury keypair; mainnet USDC mint. **Live + real.** |
| **Custodial deposit wallets** | `db.ts` + `app/api/credits/check-deposit` | **SOLANA** | Per-user encrypted Solana keypair; reads SPL balance → credits ledger → sweeps to treasury. The dollar-in step. |
| **SOL refund edge** | `lib/sol-refund.ts` | **SOLANA** | Auto-refunds native SOL mis-sent to USDC-only deposit wallets. |
| **On-chain staking** | `lib/onchain-staking.ts`, `app/api/staking/*` | **SOLANA** | Two custom **Anchor programs** (staking `$ZERO`/Token-2022 + rewards USDC); hand-rolled ix encoders, PDA derivation, on-chain history replay for maturity. |
| **pump.fun keeper (buyback/burn)** | `lib/keeper/{onchain,onchain-rewards}.ts`, `pump-idl.json` | **SOLANA/pump.fun** | `claimCreatorFees` (bonding-curve + PumpSwap), `buyZeroWithUsdc`, `burnZero`, auto-compound. **Most crypto-specific file.** Dry-run by default. |
| Docs site (Docusaurus) | `docs-site/*` | — | Architecture, worker guides, staking, zero-token, API reference. |
| Marketing / data site | `data-site/*`, `pitch-script.{tex,pdf}` | — | Landing + pitch. |
| Keeper deploy units | `deploy/c0mpute-{keeper,backup}.{service,timer}` | — | systemd units for the daily keeper + backups. |

So: **engine + Go transport sidecar + web app + OpenAI API gateway + Socket.io orchestrator
+ anti-cheat + self-installing provider worker (text + image) + credit/earnings/treasury
ledgers + subsidy/staking economy + Solana settlement (deposit, payout, staking programs,
pump.fun buyback/burn) + docs + marketing.** A full product, not two scripts.

---

## 3. Architecture deep-dive — end-to-end

### 3a. shard: how the swarm serves a token

A transformer is a stack of layers split into N contiguous blocks, one block per GPU. A
**coordinator** holds *no* model layers — only token embed/head and a small CUDA-graphed
draft model.

1. **Topology selection (`shard/topology.py` + `phase0/mesh.py`).** Measure asymmetric
   app-level RTTs over the live transport. Per-token WAN cost =
   `c_out[head] + Σ L[node_i][node_{i+1}] + c_in[tail]`. Because any node can hold any
   contiguous block, the cheapest pipeline is the **min-latency Hamiltonian path with the
   coordinator as depot** — solved exactly (Held-Karp, `O(k²2^k)`, k≤16), heuristic
   (NN+2-opt) above. The single biggest real-world lever found: **placing the layer-less
   coordinator in-region** cut the ring 174→102 ms (~50% throughput) — because it holds no
   weights, it can live anywhere.
2. **Block load (`phase0/pipeline.py`).** Each node loads *only* its layer slice; every
   other layer is mapped to HF `device_map="meta"` (never materialized). 57 GB 120B over
   4 nodes ≈ 14 GB each.
3. **Speculative decode (`phase0/specpipe.py`).** Coordinator's draft proposes K tokens;
   the distributed target verifies `[cur, d₁..dₖ]` in **one** pipeline traversal; greedy
   acceptance commits `drafts[:n] + [r[n]]` (n accepted + 1 correction). Over WAN the
   round-trip is the scarce resource, so spec decode — marginal in a datacenter — becomes
   the whole game.
4. **Direct return.** The tail returns argmaxes straight to the coordinator (1 hop), not
   relayed backward through the ring. Return channel identified by *content*
   (`{"op":"hello_return"}`), not arrival order — fixing a libp2p race.
5. **Async pipelining (`coordinate_pipe`).** Keep `depth` verify chunks in flight (FIFO),
   exactly one outstanding draft request overlapping the WAN verify. Loop runs at pipeline
   *throughput*, not *latency*; WAN drops to ~5% of the loop. On divergence, drain stale
   in-flight chunks, re-prime from the corrected prefix. Greedy ⇒ output unchanged.
6. **CUDA-graphed draft (`phase0/fastverify.py`).** Once WAN is hidden, the small draft is
   94% of the loop. Capture it as a CUDA graph against a **static KV cache** that owns its
   write index (`cp`), so **speculative rollback = just overwrite at `start`, no crop** —
   byte-identical to eager, provably lossless. 3.8× on the draft.
7. **KV crop / rollback.** Linear: lazy — head/draft crop locally now, tail crop piggybacked
   on the next verify (`crop`/`tail_crop` field) so a round = one round-trip. Tree:
   `gather_cache`/`tree_gather` keep only accepted-path KV via `index_select`.
8. **Transport (`sidecar/main.go`).** Engine speaks plain TCP to localhost; the Go libp2p
   sidecar tunnels each connection to the right ring neighbor — Noise/TLS + QUIC, DCUtR
   hole-punch, circuit-relay-v2 fallback, per-node ed25519 identity (retires the shared
   PSK). Relay→direct pre-connect so DCUtR upgrades *before* data flows.
9. **Receipt (`phase0/proof_receipt.py`).** Every run emits a verifiable receipt: distinct
   public IPs, GPU UUIDs, geos, measured edge RTTs, output token-id SHA-256,
   within-engine lossless-optimization check.

### 3b. c0mpute: dollar-in → compute → dollar-out

1. **Dollar in** `[SOLANA]`: user sends USDC to a per-user custodial deposit wallet →
   `check-deposit` reads the SPL balance → `addCredits` (SQLite) `[GENERIC]` →
   `sweepDepositToken` to treasury `[SOLANA]`.
2. **Job submit** `[GENERIC]`: client opens Socket.io; auth middleware classifies token
   (internal API / `cwt_` / `anon` / Privy JWT); server-side safety scan + rate limit +
   tier→credit cost via `MODEL_CATALOG`; **payment waterfall** (anon free → onboarding free
   → staker allowance → paid credits); credits debited *before* dispatch.
3. **Worker selection** `[GENERIC]`: among idle workers serving the requested model, pick
   one by **weighted-random, weight = measured tok/s** (spreads earnings, favors speed).
4. **Inference** `[GENERIC]`: worker drives Ollama (`/api/chat`, NDJSON stream,
   `keep_alive:-1` VRAM pin, `num_gpu:999`); emits `job:token`; orchestrator relays + scans
   a rolling 600-char buffer; tool calls executed server-side or passed back to API clients.
5. **Complete** `[GENERIC]`: anti-cheat (throughput ceiling, coherence, periodic canary) →
   `recordCompletedJob` + `recordEarning` (worker 70% / 80% staked); `realizeMargin` books
   30% to treasury buckets.
6. **Dollar out (worker)** `[SOLANA]`: `createWithdrawal` atomically debits ledger
   `[GENERIC]` → `sendUsdc` SPL transfer from treasury `[SOLANA]`.
7. **`$ZERO` loop (daily keeper, dormant by default)** `[SOLANA/pump.fun]`:
   `claimCreatorFees` from pump.fun → `realizeFees` → reserve buckets → `buyZeroWithUsdc` +
   `burnZero`; fund staker reward vaults.

The orchestrator, worker, anti-cheat, ledgers, and treasury accounting are **entirely
rail-agnostic** — Solana appears only at the *edges* (USDC deposit, USDC payout, `$ZERO`
value-accrual loop). All ledgers express value as plain USD numbers.

---

## 4. Solana coupling callouts → Bitcoin/Lightning + OpenAgents-verification substitutions

**Hard constraint honored: nothing Solana, SPL, pump.fun, or Anchor gets ported.** The
coupling is concentrated and isolatable. Map of every Solana touchpoint and its OpenAgents
equivalent:

| c0mpute Solana mechanism | File(s) | OpenAgents substitution |
| --- | --- | --- |
| **Buy credits** via USDC sent to a custodial Solana deposit wallet; read SPL balance → credit ledger | `db.ts` deposit wallet, `app/api/credits/check-deposit`, `lib/sol-refund.ts` | **Buy credits via Lightning**: a BOLT11/BOLT12 invoice per top-up; on settled payment, credit the OpenAgents D1 credit ledger. This is exactly the existing customer-facing **credits/ledger/payment** surface in the public `openagents.com` monorepo (per workspace CLAUDE.md). No custodial chain wallets, no SOL-refund edge cases. |
| **Worker payout** via `sendUsdc` SPL transfer from treasury keypair | `lib/payout.ts`, `app/api/worker-payout` | **Pay providers in Bitcoin over the Nexus/MDK bridge** (the workspace's current outbound payout path) or BOLT12 direct tips (per memory: forum direct-tipping flow). The atomic `createWithdrawal` ledger-debit pattern ports as-is; only the transfer leg changes from SPL to Lightning. |
| **`$ZERO` token + pump.fun buyback/burn + creator-fee claim** | `lib/keeper/{onchain,onchain-rewards}.ts`, `pump-idl.json` | **Do not port.** No OpenAgents token, no AMM, no buyback/burn. Value accrual to contributors is **direct Bitcoin revshare through the revenue-loop spine** (per the openagents decentralized-serving doc), not a speculative token. This is the most crypto-specific file in c0mpute and has zero OpenAgents analogue. |
| **On-chain staking** (Anchor programs, stake `$ZERO` for revshare boost / free-inference allowance) | `lib/onchain-staking.ts`, `lib/staking.ts`, `app/api/staking/*` | **Drop on-chain staking entirely.** The *useful generic idea* — "contributors with skin in the game get a higher revshare tier and a daily free-inference allowance" — survives as a **reputation/standing tier in the OpenAgents ledger** (no token lockup). If a stake-like commitment is ever wanted, it would be a Lightning-channel/hold-invoice bond, not an SPL program — but treat that as out of scope. |
| **Privy wallet auth** | `lib/privy-server.ts` | **Already covered**: OpenAgents/Pylon identity + the workspace's existing auth (WorkOS in autopilot lanes; Pylon's own identity). Privy is identity-only and fully swappable; no porting needed. |
| **Solana RPC proxy** | `app/api/rpc` | Not applicable. |

**Where shard's "PROVE/PAY" verbs meet OpenAgents verification (the substantive
substitution):** shard's plan for trust is *economic-now → crypto-later* — signed per-stage
receipts (`{swarm_id, job_id, batch_id, layer_range, in_hash, out_hash}`) + a layer-block
challenge (feed a known activation, compare `out_hash` to a trusted recompute) +
graded-reputation slashing. This is **weaker than what OpenAgents already has the
vocabulary for.** Substitute:

- shard's per-stage `in_hash/out_hash` signed receipts → **OpenAgents
  exact-execution / replay verification + dereferenceable receipts**. The receipt's
  `in_hash/out_hash` slot is *exactly* where Tassadar-style exact-execution replay drops in
  (the Psionic roadmap already names `psionic.serve.pipeline_sharded_run_receipt.v1` and
  friends). OpenAgents can make the per-stage receipt **dereferenceable** (content-addressed,
  resolvable to its inputs/outputs) rather than just a signed tuple.
- shard's "layer-block challenge = recompute on a trusted node and compare hash" →
  **OpenAgents replay verification**: a verifier re-runs the stage's layer-block forward on
  the committed input and checks exact-greedy parity. Same idea, but anchored to
  OpenAgents' existing exact-execution substrate instead of an ad-hoc canary.
- shard's settlement of "pay each node per its signed receipts, coordinator can't fabricate"
  → settle in **Bitcoin/Lightning** keyed off verified dereferenceable receipts, through the
  revenue-loop spine. The anti-coordinator-takes-all property (a node can't be paid without
  producing its own signed receipt; the coordinator can't forge it) ports directly and is
  worth keeping.

---

## 5. Prioritized harvest list

Ranked by value × portability. "Lands in" maps to the owning OpenAgents-program repo.

### Tier 1 — port the idea now (high value, clean, no crypto)

1. **Held-Karp depot-path topology solver** (`shard/topology.py`). Pure, dependency-free,
   ~150 lines; trivial Rust port (`u16` bitmask DP). Asymmetric measured-RTT matrix with
   distinct `c_out`/`c_in` depot terms (coordinator entry + return hops). **Lands in:**
   `psionic-cluster` route optimizer (Psionic roadmap `SHARD-006`). The "in-region
   coordinator = ~50% win" lesson should be encoded as a scoring term, not folklore.

2. **Sealed activation frame design** (`phase0/wire.py`): `[u64 len][12B nonce]
   [ChaCha20-Poly1305(JSON-header + length-prefixed raw tensor blobs)]`, dtype allowlist,
   **all wire failures collapse to one dead-edge error type**. **Lands in:** `psionic-net`
   `ActivationFrame` (roadmap `SHARD-004`) — but typed Rust, session-keyed off cluster
   identity, replay-protected, with QUIC as the public-network target. Copy the *discipline*
   (no untyped/code-executing deserialization; one collapse error), not the Python.

3. **Spec-decode acceptance + lazy KV-crop protocol** (`phase0/specdec.py`,
   `specpipe.py`): greedy `committed = drafts[:n] + [r[n]]` as the **exact-output oracle**;
   local crop now + **downstream crop piggybacked on the next verify** so a round = one
   round-trip; `coordinate_pipe` keeps `depth` chunks in flight with one async draft and
   discards stale post-divergence chunks. **Lands in:** `psionic-serve` coordinator
   (roadmap `SHARD-008`/`SHARD-010`). Exact greedy parity is the non-negotiable gate.

4. **Static-KV CUDA-graph rollback trick** (`phase0/fastverify.py`): cache owns its write
   index (`cp`), so speculative **rollback = overwrite at `start`, no explicit crop**;
   sliding window applied purely by the mask so the buffer is a fixed MAXLEN; capture with
   3× side-stream warmup then re-set inputs (warmup dirties state). **Lands in:** Psionic
   backend fast-verify (roadmap `SHARD-011`), as a per-backend *capability*, gated by
   graph-on/off parity tests — never a default promise.

5. **Verifiable run receipt as a skeptic checklist** (`phase0/proof_receipt.py`): token-id
   SHA-256 determinism oracle + distinct-IP / distinct-GPU-UUID / multi-geo / WAN-min-RTT
   (>1 ms) checks, each printing PASS/FAIL, with explicit honesty about
   within-engine-vs-cross-engine reproducibility for quantized models. **Lands in:**
   `psionic-cluster` receipt schema (roadmap `SHARD-001`), then surfaced as
   **dereferenceable** receipts in the OpenAgents inference gateway. The "what would a fake
   look like, and which check kills it" framing in `shard/docs/PROOF.md` is a model for
   honest receipt design.

6. **Canary anti-cheat + graded reputation** (c0mpute `orchestrator.ts`
   `maybeDispatchCanary`/`buildCanary`/`checkCoherence` + SQLite strike/ban). Synthetic
   nonce-echo jobs injected ~1-in-15 *only when the queue is idle*, indistinguishable from
   real jobs, proving live inference (not a canned echo); throughput floor/ceiling; coherence
   floods; persistent bans surviving reconnect. Genuinely novel for contributor-compute.
   **Lands in:** Pylon fleet anti-cheat / OpenAgents provider reputation. **Substitution:**
   for *sharded* serving the whole-model canary can't probe a stage node — replace with the
   **layer-block replay challenge** (Tier-1 #3 of the verification map) backed by
   exact-execution, with the canary as the whole-model fallback for single-Pylon serving.

### Tier 2 — port soon (high value, more work or product-layer)

7. **libp2p sidecar NAT-traversal recipe** (`sidecar/main.go`): per-node ed25519 PeerId,
   TCP↔libp2p transparent tunnel, DCUtR hole-punch + circuit-relay-v2 fallback + AutoNAT +
   QUIC + **relay→direct pre-connect** + RELAY/DIRECT conn monitor. The proven recipe for
   home-GPU-behind-NAT, with the documented gotchas (full-cone NAT punches, datacenter
   Docker NAT doesn't, TEST-NET ranges silently blocked). **Lands in:** `psionic-net` /
   Pylon transport — but note the workspace already owns `iroh` (Rust QUIC P2P) and
   `nostr-effect` as references; prefer a Rust-native iroh/quinn path over a Go sidecar.
   Harvest the *NAT strategy and gotchas*, not the Go code.

8. **OpenAI-compatible API gateway that bridges HTTP → worker pool with full tool
   passthrough** (`app/api/v1/chat/completions/route.ts`): acts as a trusted internal client
   to the orchestrator, maps OpenAI↔backend message shapes (tool_calls JSON-string↔object,
   vision data-URLs↔base64), SSE streaming, `finish_reason: tool_calls` passthrough so an
   external agent drives the tool loop. **Lands in:** the OpenAgents inference gateway
   (`openagents/docs/inference/...`) — validates the "gateway routes to cheapest viable
   supply" design and shows a concrete OpenAI-shape adapter.

9. **Weighted-random worker selection (weight = measured tok/s)** + the
   **benchmark-self-then-register** worker lifecycle. Spreads earnings while favoring speed;
   simple and effective. **Lands in:** OpenAgents gateway / Pylon scheduler routing.

10. **Plug-and-play self-installing worker** (`c0mpute-worker/src/{setup,worker,inference,
    update}.ts`): auto-install Ollama, `nvidia-smi` VRAM detect → adaptive `num_ctx`,
    flash-attn + q8 KV on NVIDIA, build a custom model with baked-in system prompt,
    **`keep_alive:-1` VRAM pinning** to avoid cold reloads, self-update. **Lands in:** Pylon
    onboarding UX. **Caveat to flag (do NOT copy):** `update.ts` silently `npm install -g
    @latest` and re-execs — a supply-chain risk; OpenAgents Pylon updates should be
    signed/pinned and operator-consented, not silent.

11. **Treasury bucket ledger with clamp-at-zero liability protection** (`lib/treasury-
    ledger.ts`): buckets (buyback / rewards / profit) that can never be drawn below booked
    liabilities (user credits, worker payouts). The *accounting safety property* is valuable
    independent of any token. **Lands in:** OpenAgents private metering/settlement (`cloud`)
    or the public credit-ledger Worker — express in Bitcoin/USD, drop the buyback bucket.

12. **Staker-allowance "stake → daily free inference" engine** (`lib/staker-allowance.ts`),
    *de-tokenized*: a hard-capped, pro-rata, atomic daily free-inference pool gated by
    contributor standing. The engine is generic SQLite; only the input "matured stake number"
    is on-chain. **Lands in:** OpenAgents subsidy/free-tier policy — gate by reputation tier,
    not a token lockup.

### Tier 3 — reference / study only

13. **Tree speculative decoding** (`phase0/tree.py`, tree-on-graph in `fastverify.py`).
    Real and elegant (flat `tok/par/dep`, ancestor mask, non-prefix `gather_cache`), but the
    Psionic roadmap correctly sequences linear spec decode first. Study for a later phase.

14. **NVFP4/MXFP4 staged-loader research drivers** (`research/glm_*`, ~50 files).
    Hardware-specific, transformers-5.x-coupled, GLM/gpt-oss-specific. Reference for *how a
    skeptic-proof large-model receipt is produced*, not code to port — Psionic's large-model
    phases (`SHARD-012`) are hardware-blocked and must stay `planned` until real evidence.

### Explicitly NOT worth taking

- **All Solana/SPL/Anchor/pump.fun code** (`lib/payout.ts`, `sol-refund.ts`,
  `onchain-staking.ts`, `lib/keeper/*`, `pump-idl.json`, deposit/staking-wallet helpers,
  `app/api/{staking,treasury,credits/check-deposit,worker-payout}` chain legs). Per the hard
  constraint. Substitutes are in §4.
- **The `$ZERO` token economy** — no OpenAgents token; revshare is direct Bitcoin.
- **shard's Python engine as a runtime dependency** — Psionic builds Rust-native (per its
  roadmap's explicit boundary). Copy concepts, not the SGLang/transformers/CUDA-graph Python.
- **The Go sidecar as production code** — prefer Rust iroh/quinn; harvest the NAT strategy.
- **Silent `npm @latest` worker auto-update** — supply-chain anti-pattern.

---

## 6. One-paragraph bottom line

`shard` and `c0mpute` are **distinct repos with a deliberate one-way `c0mpute → shard`
dependency** — an inference *engine* and the *marketplace* that runs it — and that boundary
is the same one OpenAgents already draws between Psionic (execution) and its product/Pylon
surfaces. They are **not** the entirety of a thin product: c0mpute is a full two-sided
compute marketplace (web app + OpenAI API gateway + Socket.io orchestrator + canary
anti-cheat + self-installing text/image worker + credit/earnings/treasury ledgers +
subsidy/staking economy + Solana settlement + docs), and shard is a research-grade
WAN-pipeline inference engine with verified 30–40 tok/s frontier-model receipts. The
**top harvest items** are the Held-Karp topology solver, the sealed activation-frame
discipline, the spec-decode acceptance + lazy-KV-crop + async-pipelining protocol, the
static-KV CUDA-graph rollback trick, the verifiable run receipt, and the canary +
graded-reputation anti-cheat — most of which the Psionic shard-WAN roadmap already has
landing slots for, plus the gateway/worker/ledger patterns for the OpenAgents inference
gateway and Pylon. The **Solana coupling is concentrated at the settlement edges** (USDC
deposit-in, USDC payout-out, the dormant `$ZERO`/pump.fun loop) and is fully replaced by
**Lightning/Bitcoin for money movement + exact-execution replay verification with
dereferenceable receipts** for trust — shard's own `in_hash/out_hash` receipt slot is
precisely where OpenAgents' exact-execution verification drops in, making OpenAgents'
trust story stronger than shard's economic-now/crypto-later plan.
