# autopilot.agent_world_scene.v1 — Vertex fleet worklog

Promise: `autopilot.agent_world_scene.v1` (state: **yellow** — unchanged by this work).

## What this change advances

Blocker: `blocker.product_promises.agent_world_scene_live_wiring_p2_5_pending`
(P2.5 live in-app wiring polish).

### The gap
In the merged P2 wiring, the live payment-beam set behind chat
(`model.chatWorldParticles`, fed by `subscribePaymentParticles`) was bounded only
by **count** — `MAX_CHAT_WORLD_PARTICLES = 24` in `apps/autopilot-desktop/src/ui/update.ts`.
On a quiet network that means a beam from an old `real_bitcoin_moved` /
`settlement_recorded` event keeps flying behind chat **forever**, until 24 newer
payments push it out. A beam that no longer represents recent activity
misrepresents liveness — a P2.5 polish gap.

### What I built
A pure, deterministic **recency window** for payment particles, wired into the
reducer so stale beams expire by time as well as by count.

- `apps/autopilot-desktop/src/shared/chat-world-scene.ts`
  - `PAYMENT_PARTICLE_WINDOW_MS = 90_000` — how long a beam stays after its event ts.
  - `paymentParticleTsMs(particle)` — parse a particle's ISO `ts` to epoch ms, or
    `null` when absent/unparseable (never invents a time).
  - `prunePaymentParticlesByRecency(particles, referenceTsMs, windowMs?)` — drop
    particles older than `referenceTsMs - windowMs`; KEEP particles with no
    parseable ts (count cap bounds them); preserve order; no-op on a non-finite
    reference. **Pure**: the caller passes the reference time, so there is no
    hidden wall clock and replaying a stream is deterministic.
- `apps/autopilot-desktop/src/ui/update.ts`
  - `appendChatWorldParticle` now dedupes → recency-prunes (using the NEW
    particle's own `ts` as the reference clock) → caps. A null/unknown ts skips
    the recency prune and falls back to the count cap, so behavior degrades safely.
- `apps/autopilot-desktop/tests/chat-world-scene.test.ts`
  - 11 new assertions across `paymentParticleTsMs` and
    `prunePaymentParticlesByRecency` (window edge, order, no-ts retention,
    non-finite no-op, custom window).

### Why it stays honest / in-bounds
- No promise state changed; no registry edits. Still yellow.
- Still **evidence-bound**: pruning only ever *removes* beams; it never creates,
  fakes, or re-times a beam. Every remaining beam still carries its real
  `sourceRefs` (the upstream `activityEventToParticle` already drops refless events).
- Flag-gated and default-OFF unchanged (`CHAT_WORLD_SCENE` / `CHAT_WORLD_PAYMENTS`).
- No spend, payout, settlement, or multiplayer capability touched.

## Validation
- `apps/openagents.com/workers/api`: `bunx tsc -p tsconfig.json --noEmit` → 0 errors.
- `apps/openagents.com`: `bun run check:deploy` → pass.
- `apps/autopilot-desktop`: `bun test tests/chat-world-scene.test.ts` → 33 pass / 0 fail.
- `git diff --check` → clean.

> Note: `apps/autopilot-desktop` is Vite-built; running bare `tsc` against its
> `node16`-resolution tsconfig surfaces repo-wide pre-existing `TS2835`
> (extensionless-import) and `TS6133/TS7006` errors unrelated to this change.
> This change adds no new error type at the lines it touches; `bun test` is the
> app's gate and passes.

## What genuinely remains for this blocker (still listed)
P2.5 is **not** fully cleared — this clears one slice (beam lifetime). Remaining:
- A periodic prune/tick so beams also expire on wall-clock idle (no new event to
  trigger `appendChatWorldParticle`); today expiry is event-driven only.
- End-to-end in-app verification of the live mounted scene (pylon poll + SSE
  beams + inspector click → receipt) in the running Electrobun webview.
- The default-on (or explicit stay-flag-gated) owner decision, which gates green
  per `proof.claim_upgrade_receipts.v1`.
