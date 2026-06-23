# Inference-Engineering Book — Implementation Log

The overnight loop's audit log for turning the inference-engineering book's
investigation notes (`khala-investigation-notes.md`) into shipped Khala product
behavior. Each entry records what was done, where, the verification bar, and the
honest scope (what is real vs still `not_measured` / inert).

Conventions:

- Entries are append-only and dated newest-last within a priority lane.
- A priority tag (`P0-1`, `P0-2`, …) maps to the section in
  `khala-investigation-notes.md`.
- "DONE" means merged to `main` and (where noted) deployed. Branch-only work is
  "in progress", never DONE.

---

## P0-1 — Make the Khala scorecard production-complete — DONE, deployed `9b0c9b56`

- **Notes ref:** `khala-investigation-notes.md` §P0 item 1.
- **What shipped:** the canonical, public-safe Khala request-lifecycle telemetry
  schema (`openagents.khala.telemetry.v1`) and its production wiring — token
  counts, the latency split surface, request class, route/provider/served-model,
  verification class + executed verdict + scalar reward, and the cost/margin
  disclosure — recorded on the immediate `openagents` block (small summary) with
  the full record dereferenceable behind the public inference receipt.
- **Honesty discipline:** every numeric is either a real measured number or the
  explicit `not_measured` sentinel; a measured `0` and `not_measured` are
  distinct products. Nothing is fabricated.
- **Where:** `apps/openagents.com/workers/api/src/inference/khala-telemetry.ts`
  (schema + builders) + the `chat-completions-routes.ts` build sites; doc
  cross-reference `docs/inference/2026-06-23-khala-telemetry-scorecard-book-p0-1.md`
  and `docs/inference/khala.md` §3.
- **Merge / deploy:** PR #6085 (merge `f350c3bec5`), deployed `9b0c9b56`.
- **Left honestly `not_measured` for follow-on:** the provider/gateway/verifier/
  settlement time split, queue/batch wait, region, and (at P0-1) the
  cache-affinity hash + cached-input dimension — each recorded as a sentinel with
  a `blockerRef` rather than a fake number. P0-2 closes the cache-affinity hash +
  cached-input gaps.

---

## P0-2 — Treat prefix caching as a product feature — DONE (PR open, #6084)

- **Notes ref:** `khala-investigation-notes.md` §P0 item 2 ("Treat Prefix
  Caching As A Product Feature"); book §5.3 (caching) — prompt order controls
  whether a long shared prefix is reusable.
- **What shipped (the six deliverables):**
  1. **Stable prompt layout** — `assembleStablePromptLayout` orders the outgoing
     messages stable-first (acceptance contract → identity → tool schemas →
     stable policy), volatile/user content last. Gateway-injected blocks are
     tagged with a `StableBlockKind` (structural classification of our own
     blocks, not user-intent string matching). The `khala-identity.ts` injection
     stays in the stable prefix.
  2. **Deterministic ordering + hashing** — `canonicalJson` /
     `serializeToolSchemas` (sorted keys, stable tool order) → byte-identical
     prefix text → stable `stablePrefixHash` for the same logical inputs.
  3. **Cache-affinity keys** — `deriveCacheAffinityKey({account, session?,
     codebase?})`; recorded only as the one-way `hashCacheAffinityKey` digest
     (`cacheAffinityKeyHash`). Raw key never leaves the gateway.
  4. **Provider session affinity** — `sessionAffinityParams` sets Fireworks
     `x-session-affinity` and OpenAI-style `user` to the same opaque hash value,
     pinning a session to one cache-warm replica.
  5. **Cached input tokens + total reconciliation** — `cachedInputTokens` flows
     from provider usage into the telemetry block + record; the live discrepancy
     (`total` 679 ≠ prompt 347 + completion 20) is reconciled in the record
     builder: provider total is recorded receipt-first (authoritative), the gap
     is disclosed as `unaccountedTokens` (312 = billed reasoning/thinking/tool-use),
     never recomputed or dropped.
  6. **Cache-aware routing** — `decideCacheAwareRouting` reorders (never widens)
     the viable lane plan toward the cache-warm lane via a typed
     `CacheWarmthOracle` keyed by the affinity hash, gated by health + privacy/
     region pin policy. Inert by default (no oracle wired → plan unchanged).
- **Where:** `apps/openagents.com/workers/api/src/inference/prompt-prefix-cache.ts`
  (+ `.test.ts`), `cache-aware-routing.ts` (+ `.test.ts`), the
  `chat-completions-routes.ts` integration (+ `.test.ts`), and
  `khala-telemetry.ts` (new `cachedInputTokens` on the block + `unaccountedTokens`
  reconciliation field on the record). Docs: `docs/inference/khala.md` §3
  prefix-caching subsection.
- **Verification bar (green):** the inference test suites (698 tests),
  `typecheck`, `check:architecture`, `check:effect-topology`, and
  `check:public-projection-freshness`. Tests cover deterministic prefix ordering
  (same stable inputs → identical prefix + cache-key hash), volatile content
  never in the prefix, the one-way public-safe affinity hash, session-affinity
  headers set when supported, cached-token telemetry populating from a fixture
  provider-usage payload with correct totals reconciliation, and cache-aware
  routing picking the warm lane (and refusing an unhealthy / pin-forbidden one).
- **Honest scope — still `not_measured` / inert:** cached input tokens remain
  `not_measured` for providers/lanes that do not report a cached dimension; the
  cache-aware-routing seam is inert until the Worker wires a real
  `CacheWarmthOracle` / health / pin-policy (the warm-lane KV/DO record is a
  follow-on); region and the provider/gateway/verifier/settlement time split stay
  `not_measured` (P0-1 follow-on, unchanged here).
- **Status:** PR open against `main` (#6084); orchestrator reviews / merges /
  deploys / smokes. NOT deployed by this entry.
