# Khala / OpenAgents Cloud Buildout — State & Next-Steps Audit

*Audit — 2026-06-22. Where the Khala inference-model buildout (EPIC #6017, the
M0–M8 ladder) actually stands after the first wave of parallel-agent work, and
what must happen next to fully implement it through to the head-to-head demo.
Source of record: [`docs/inference/khala-buildout-roadmap.md`](../inference/khala-buildout-roadmap.md).*

> Scope note: "Khala" here = the **inference model/gateway** (`docs/inference/khala.md`).
> The closed `KHALA-001…027` / `OA-SPACETIME-*` issues are the **deprecated**
> websocket sync engine and are unrelated.

## TL;DR

The **scaffolding and contracts are landing fast**; the **live money loop is
not on yet**. Four of nine milestones have real code on `main` (M0 catalog+receipt,
M2 verifier, M5 world projection + desktop scene + receipt-backed public timeline
source, M8 publication scaffold), but **the gateway is still INERT in production**
(`INFERENCE_GATEWAY_ENABLED=false`, no provider secrets), so nothing serves a
real metered completion yet. The two true long poles are **(1) owner-gated
production enablement of the gateway** (unblocks M0-live, M1, M3, M8) and
**(2) the learned coordinator** (M6/M7), which needs Psionic primitives that do
not exist. Everything upstream runs on the heuristic router until M6.

## Milestone status at a glance

| # | Milestone | State | What landed | Blocking next step |
|---|---|---|---|---|
| #6008 | **M0** serve metered + receipt | **OPEN — code landed** | `khala-mini` priced catalog alias (#6018); `openagents` disclosure block on non-stream responses; tests + typecheck green | **Owner**: provider secrets + flip `INFERENCE_GATEWAY_ENABLED` (staging→prod) + live SDK smoke |
| #6009 | **M1** Autopilot calls Khala | **OPEN — not started** | — | Cockpit POST to `khala-*` + render receipt; needs M0 enabled (staging ok) |
| #6010 | **M2** verified coding outcomes | **CLOSED — done & verified** | `khala-code` model; crossy-road rubric; Playwright headless verifier (`khala-code-verifier.ts`); route returns `verification`/`verified`/rubric/receipt; 88 tests, check:deploy green | — (tick the EPIC box) |
| #6011 | **M3** Bitcoin settlement | **OPEN — not started** | metering/referral/payout code exists but unproven E2E | **Bitcoin-only, Spark-primary** payout (**no Stripe**; MDK = checkout-only) → agent-testable now; first proof = pay the guinea-pig Pylon (`.secrets/khala-test-payout.env`) |
| #6012 | **M4** Pylon workers in pool | **OPEN — not started** | — | Fabric supply adapter (gateway↔Psionic ask-plan→execute→consume-receipt); whole-small-model serving first |
| #6013 | **M5** Verse serving view | **OPEN — partial** | world-contract Khala **gateway projection contract** + bridge + commands (`e0e33aad61`); `three-effect` **crackling arc + gateway portal** primitives; desktop **projects Khala inference into Verse** (`11a7c3ca98`); public activity timeline emits receipt-backed `khala_inference_served` events from paid Khala inference receipts; world worker scheduled poller + cursor persistence + D1 snapshot hydration | Deploy/run the live bridge path and capture an owner-enabled Khala smoke proving gateway → timeline → world → desktop |
| #6014 | **M6** learned coordinator (TRINITY) | **OPEN — not started** | — | **Largest pure-eng gap**: Psionic primitives P1–P5 (hidden-state extraction, sep-CMA-ES, SVF, reward adapter) do not exist; then a training run |
| #6015 | **M7** Conductor lane | **OPEN — not started** | — | GRPO NL planner; depends on M6 + real training compute |
| #6016 | **M8** head-to-head demo | **OPEN — scaffold (honestly blocked)** | `#6016-A…E` structure: metric **reducer** + **closure audit**, **publication renderer**, fixture manifest, and a **live-promotion audit** that blocks fake-live manifests carrying leftover `fixture.*` refs; runbook (`docs/inference/khala-head-to-head-demo.md`); 6 focused tests + check:deploy green | A real measured run — reducer returns `closureAudit.canClose:false` on fixture evidence by design. Blockers: live Khala + frontier runs, M7 conductor evidence, settlement receipts, Verse/artifact playback refs, measured energy telemetry, final publication refs |
| #6017 | **EPIC** | OPEN | checklist not yet ticking M2 | reconcile checklist |

## What is actually LIVE vs INERT (honest line)

- **Live / merged code:** the gateway request surface, auth + balance gate,
  cheapest-viable router, real provider adapters (Fireworks verified; Vertex
  Anthropic/Gemini; passthrough), receipt-first metering, `khala-mini` +
  `khala-code` virtual models, the `openagents` disclosure block, the M2 rubric
  verifier, the world-contract Khala gateway projection, the public activity
  timeline's receipt-backed Khala inference source, the desktop Verse
  projection, and the M8 publication scaffold.
- **INERT / not proven:** `INFERENCE_GATEWAY_ENABLED` is **off** in prod and no
  provider secrets are wired, so **no real metered completion, no credit
  decrement, no Bitcoin settlement, and no live head-to-head has happened.** All
  current proof is unit/fixture/scaffold evidence, not product proof. This is
  correct and honestly labeled — it is the gate, not a regression.

## Cross-cutting blockers, ranked

1. **Production gateway enablement (owner-gated) — the #1 unlock.** Until an
   owner wires a provider secret and flips `INFERENCE_GATEWAY_ENABLED` (staging
   first), M0-live, M1, M3, and M8-live are all blocked. This single step turns
   on the entire upstream money loop. *Recommend: enable on a staging/preview
   Worker first, run the M0 live smoke, then prod.*
2. **Payments are Bitcoin-only this wave, Spark-primary (owner direction
   2026-06-22).** Settle payouts over **Spark** (Lightning as the rail); **no
   Stripe / no card funding** (MDK is checkout-only, not used for payouts). This
   *un-gates* M3 — Bitcoin moves in testing without prod card secrets — so M3 is
   agent-testable now. The first proof must **pay the guinea-pig Pylon** (Spark
   address in `/Users/christopherdavid/work/.secrets/khala-test-payout.env`) with
   a settled `realBitcoinMoved:true` receipt. Keep amounts small and
   treasury-bounded; it is still real money.
3. **Learned coordinator substrate missing (pure-eng long pole).** Psionic has
   no hidden-state extraction, sep-CMA-ES, or SVF (the P1–P5 primitives), so M6
   (and therefore M7 and the "learned composition" framing of M8) cannot start
   in earnest. This is the largest engineering investment and needs real
   ML-training compute. Until it lands, the product runs on the heuristic router
   — which is still a valid (just not learned) head-to-head.
4. **M5 still needs live bridge proof.** The contract, public timeline source,
   world bridge poller, render primitives, D1 snapshot hydration, and desktop
   projection exist, but a deployed owner-enabled Khala receipt still needs to
   prove gateway → timeline → world → desktop.

## Critical path to the north-star (head-to-head)

```
[OWNER] enable gateway (staging) ──► M0 live smoke ──► M1 cockpit
                                          │
   M2 verifier (DONE) ──────────────────► │
                                          ▼
   M3 Bitcoin/Spark payout (pay guinea-pig Pylon) ───────────┐
   M4 whole-model Pylon serving ─────────────────────────────┤
   M5 live bridge + scene proof ─────────────────────────────┼─► M8 head-to-head
   M6 Psionic P1–P5 + ES training ──► M7 Conductor (GRPO) ────┘   (heuristic first;
                                                                    learned after M6)
```

The demo can run **heuristic-router-first** as soon as M0 is enabled + M1 cockpit
exists + M3 settles in staging — it does **not** have to wait for M6/M7. The
learned-composition version is the upgraded second cut after M6.

## What should be done next (prioritized)

**Now (this week), unblocks the most:**
1. **Owner: enable the gateway on staging** — wire one provider secret (Fireworks
   is verified-live and cheapest to prove), set `INFERENCE_GATEWAY_ENABLED=on`
   in a preview Worker, and run the **M0 live SDK smoke** against
   `openagents/khala-mini`. Capture the metered completion + receipt → closes
   #6008's real acceptance.
2. **M1 (Agent Cockpit):** wire Autopilot to POST a prompt to `khala-*` and
   render the `openagents` block (route/worker/cost/verification). This is small
   once staging serves; it also gives the demo its front door.
3. **EPIC hygiene:** tick M2 in #6017; have each active lane post a one-line
   status comment on its issue (M5/#6013 and M8/#6016 have landed code but no
   issue comment).

**Next (unblocks money + supply):**
4. **M3 (Agent Ledger):** stand up the Stripe **TEST** chain (#5520) →
   funded-balance → metered Khala spend → dereferenceable receipt, all in
   staging; keep prod keys/payout owner-gated. Prove the credit decrement against
   a real (test) charge.
5. **M4 (Agent Pylon):** build the fabric supply adapter (whole-small-model
   serving first), with the exact-parity receipt as the trust gate; defer
   shard-WAN.
6. **M5 (Agent Verse):** deploy/run the bridge path and prove the crackling-arc
   / gateway-portal scene against a **real** receipt-backed Khala inference event
   from the activity timeline (the contract, primitives, public source, bridge
   mapper, scheduled poller, snapshot hydration, and desktop projection are
   already in).

**The long pole (start in parallel, it gates the "learned" story):**
7. **M6 (Agent Psion):** implement Psionic primitives **P1–P5** (hidden-state
   extraction → coordinator head + SVF → sep-CMA-ES optimizer → scalar
   terminal-reward adapter → typed worker-pool binding), then a first shadow
   training run rewarded by the M2 verifier verdict. This is weeks of work + real
   compute; begin now so it is ready when M0–M5 are green.
8. **M7 (Conductor):** after M6, GRPO NL planner (DPPO + FP32 head per the TMAX
   recipe) for multi-worker composition.

**The payoff:**
9. **M8 (Agent Demo):** run the head-to-head — heuristic-router cut first
   (as soon as M0+M1+M3-staging are green), then the learned-coordinator cut
   after M6/M7. The harness is already in and **self-gating**: the reducer's
   `closureAudit.canClose` stays `false` on fixture evidence, and the
   live-promotion audit rejects any manifest still carrying `fixture.*` refs — so
   M8 cannot be closed on scaffolding. Feeding it real evidence (the six blockers
   above) is the only path to a publishable result.

## Risks & honesty notes

- **Premature-completion risk — and a good antidote already in place.** M2
  closed correctly (real verifier + green gates), and M8 ships its own guard:
  the reducer's `closureAudit.canClose` and the live-promotion audit mechanically
  refuse to close on fixture evidence. That self-gating pattern is exactly what
  M0 and M3 should adopt — don't close them until their *live* acceptance
  (metered completion / real test-charge receipt) exists. The buildout's whole
  thesis is "verified, not vibes," so its own milestones must clear the same bar.
- **Concurrency hygiene.** Multiple agents now share this checkout; the CLAUDE.md
  rule (never move another agent's uncommitted work; use a fresh worktree off
  clean `origin/main`) is in force and was the right fix after a near-miss stash
  incident. This audit was written on such a worktree.
- **Owner is on the critical path twice** (gateway enablement, payments TEST
  chain). Neither is an agent workaround; both should be scheduled explicitly as
  `NEEDS-OWNER`.

## One-line bottom line

The contracts, catalog, verifier, world projection, and demo harness are real and
landing; the buildout is now gated on **one owner action (enable the gateway in
staging)** to light the upstream loop, and **one big engineering investment
(Psionic P1–P5 + training)** to make Khala a *learned* coordinator rather than a
heuristic router. Do the owner enablement first; start M6 in parallel.
