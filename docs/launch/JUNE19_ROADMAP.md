# JUNE 19 ROADMAP — Autopilot Desktop → full coding agent (the day's main thrust)

Date: 2026-06-19. Rolls forward from
[`JUNE18_ROADMAP.md`](./JUNE18_ROADMAP.md) (CLOSED 2026-06-19). Overnight the
revenue-loop clearing layer (EPIC #5457) and the desktop auto-onboarding from-DMG
proof (EPIC #5441) both landed and closed; the product-promises registry destaled
to `2026-06-19.2`. Since then: **EPIC #5461 (Autopilot Desktop → full coding agent)
is COMPLETE** (be2378066), the post-EPIC **styles.css launch-fix** landed
(`b6e523a77` — repaired styles.css mangled by the #5461 serial merges), the three
**live coding-agent execution lanes were independently re-verified** (Claude +
Codex + Tassadar), and the registry destaled again to **`2026-06-19.3`** (no
flips). See the June 18 close-out summary for the full earlier shipped record and
the section below for the live-verification record.

> **Operating posture:** honest-scope, receipt-first. No green promise flip
> without dereferenceable receipts **and** owner sign-off. Close issues only after
> merge to `main`. **NEVER add GitHub Actions / CI workflow files.** `src/ui/*` in
> `apps/autopilot-desktop` is a hot/shared directory — coordinate (the
> Blueprint×Tassadar chat lane is concurrently editing chat files).

---

## P0 — CLOSE THE REVENUE LOOP ($1 in → >$1 out)

**THE priority, full stop. If we close the revenue loop we have a company; if not, we're
wasting time.** Grounded in the "Let's Make Money" thesis
(`../../../launch-videos/2026-06-18-video-2-referral-revenue-share.md`): OpenAgents has
repeatedly shipped *supply* and left the *buy-side* open — agentic coding is the first demand
people already pay for. The loop has three legs; honest status as of 2026-06-19:

- **MONEY IN** — credits purchase. Stripe checkout is real + fully wired; gated only on keys
  (NEEDS-OWNER: Stripe test keys → staging, live → prod; no Stripe secrets on prod yet). BTC
  top-up is future. **#5497 (MERGED)** bridges purchased USD credits → inference-spendable msat
  (asset-boundary enforced: USD-funded balance is spendable on inference, never withdrawable as
  Bitcoin).
- **VALUE — 🟢 LIVE:** the inference gateway is enabled in prod (Gemini 3.5 Flash free tier,
  $10/owner-claim Sybil-resistant pool) and the Autopilot coding agent is the demand wedge.
- **MONEY OUT / MARGIN** — serving-node + referral revshare (built, owner-armed); margin = the
  spread over our owned supply cost.
- **CLOSED =** real $/sat in → value delivered → margin + revshare out, end-to-end. Today value
  is LIVE; money-in waits on Stripe keys; the full buy→fund→spend loop is exercisable now on the
  isolated **staging** env (own D1/KV/R2, migration 0211 applied, inference keys set).

The afternoon thrust turned the day's product spine toward the **buy-side**. Two pieces:

### Inference gateway — ✅ BUILT (EPIC #5474 CLOSED), now going live (not inert)

The entire OpenAgents inference gateway shipped to `main` behind
`INFERENCE_GATEWAY_ENABLED` (12 issues): OpenAI-compatible `/v1/chat/completions` →
key-auth → balance gate → **cheapest-viable routing** (claude→Vertex, open→Fireworks,
+passthrough) with 429/503 overflow → adapter dispatch → **live credit decrement metered
from real `usage`** via the pricing engine (idempotent, never-negative); plus the
`openagents-network` **fabric supply lane** + **serving-node payout** (parity-gated,
owner-armed), the **referral subsystem** (#5475: attribution → ongoing-accrual-on-ALL →
three-way split → payout → dashboard), and **abuse/KYC/rate-limit** controls. Design +
strategy: `docs/inference/` (7 docs incl. the Agent-Cloud capstone). Docs `pricing-vs-factory`
+ `pricing-model` set the multiplier/margin/BTC-discount mechanics.

**Go-live (in flight):** enable Autopilot's **free inference on Gemini 3.5 Flash** — free
until **$10 per verified owner-claim identity** (Sybil-resistant shared pool; unclaimed gets
only a taste), earn more via contribution, **premium models (Opus/etc.) owner-grant
allowlist only**. Server build (Vertex Gemini adapter + free pool + allowlist) + the
Autopilot client (#5485) are landing; then flip the flag + deploy + live-verify a real
free-Gemini call. **Posture: nothing stays inert — we enable it.**

### Collect money now — credits purchase path (audit in flight)

Owner mandate: we must be able to **collect money now**. There is real payment infra on
`main` (`stripe-billing.ts`, `/api/billing/checkout|summary|stripe/webhook|setup-intents|
auto-top-up|coupons`, `payments-ledger`/`buyer-payment-ledger`; web `credits-panel.ts` +
`page/loggedIn/billing/`). An audit (`docs/launch/2026-06-19-credits-purchase-collect-money-audit.md`)
is determining whether a real card/BTC payment → usable credit balance → inference-spend
loop is **live today or stubbed**, and the exact gap list to turn collection on. This is
the top near-term priority: a customer/business can pay → get credits → use inference.

### Mobile remote-control gap — EPIC #5492 Wave-1 (in progress)

Audit `docs/launch/2026-06-19-desktop-mobile-remote-control-gap-audit.md`. G2 (live
session streaming) + G1 (6 steer-actions onto the secure bridge) merged; G3 (artifact/diff
viewer) reconciling. G4 (chat/turn.steer) is a follow-on. Goal: mobile steers all relevant
desktop capabilities over the capability-scoped bridge (no dev token on the wire).

### Desktop chat UX + auth fixes — ✅ LANDED

Owner-reported chat-pane cleanup (`c57585a45`: replay out of chat, scoped-steps collapsed,
palette unified to openagents.com) + the durable control-token **401 fix** (`7b844170f`:
canonical Pylon homes + server-probe fall-through). Chat now works and looks right.

---

## P0 — HEADLINE: Autopilot Desktop → full coding agent (EPIC #5461) — ✅ COMPLETE (be2378066)

**DONE 2026-06-19.** Phase 1 nav shell #5462 (grouped nav + Cmd-K palette + shortcuts +
pane-registration seam, b0c3aa554) and all 7 Phase-2 connection issues integrated to main:
#5466 chat-live, #5467 autonomous-loop view, #5468 bounded auto-approve, #5469 swarm
batch/failover/nesting, #5470 diff/artifact browser, #5471 repo/worktree picker, #5472
functional settings. Built in parallel on branches, integrated serially against the seam;
every merge kept builds + the full desktop test suite + the black-screen guard green; no
GitHub Actions. EPIC #5461 closed.

This was the day's main thrust. Past onboarding (now proven from a clean-Mac DMG),
turn the Electrobun desktop app (`apps/autopilot-desktop`, Bun + Foldkit) into a
real day-to-day coding agent by **connecting** the runtime systems we already
built to clean, uncluttered UI. The runtime substrate is real and most panes are
already built and wired — the remaining work is **connection + organization**, not
building panes from scratch.

Audit: `docs/launch/2026-06-19-autopilot-desktop-coding-agent-audit.md`.

### P0.0 — Coding-agent live re-verification + registry destale to `2026-06-19.3` — ✅ DONE

Independently re-verified the three live coding-agent execution lanes from clean
`origin/main` (`b6e523a77`, the post-#5461 styles.css launch-fix commit) — did not
trust prior claims, re-ran each and produced fresh evidence. **All three passed.**
Receipt (the dereferenceable ref promises now cite):
`docs/launch/2026-06-19-coding-agent-live-verification.md`.

- **Claude bridge** — `apps/pylon sessions exec --adapter claude_agent` →
  `ok:true`, `verify.passed:true`, `verify.exitCode:0`, file created, elapsed
  ~10.65s, process exit 0, auth on-device `~/.claude`.
- **Codex bridge** — `--adapter codex` (workspace-write sandbox, network disabled)
  → `ok:true`, `verify.passed:true`, `verify.exitCode:0`, file created, elapsed
  ~13.17s, process exit 0, auth on-device `~/.codex/auth.json`.
- **Tassadar executor** — `bun test packages/tassadar-executor` → **23 pass / 0
  fail** across 5 files (execute + `exact_trace_replay`), exit 0.

**Honest scope:** local single-task exec proof only — NOT production-scale,
at-volume, packaged-stable-binary, or public-settlement.

Registry edit (bumped `PublicProductPromisesVersion` `2026-06-19.2` →
`2026-06-19.3`; new `Registry 2026-06-19.3` caveat note added):

- **STAYED GREEN, evidence re-anchored on the new receipt (green→green, no flip,
  no `promise_transition` required):**
  - `pylon.local_claude_agent_bridge.v1`
  - `autopilot.codex_probe_pylon_successor.v1`
  - `compute.tassadar_executor_poc.v1`
- **GREEN-CANDIDATES, FLAGGED but NOT flipped (receipt added to evidence + a
  stays-yellow note appended to `safeCopy`):**
  - `autopilot.builtin_compute_agent.v1` (yellow) — gap: signed/notarized recut
    carrying the built-in-agent source + packaged OpenAgents compute credentials +
    a **metered from-install go-online useful-work smoke**. A local single-task
    exec does not satisfy those gates.
  - `autopilot.desktop_gui_client.v1` (yellow) — gap: the owner-gated **from-DMG
    clean-Mac** render/presence/settled-Bitcoin proof plus the live
    PDF/preview/ingest/browser runtime wiring. (Note: a separate overnight DMG
    proof `cc27f122e` addresses the from-DMG blocker independently of this lane;
    this live-verification receipt only re-anchors the execution-lane dependency.)

No yellow→green flips were made here, consistent with the green-flip guardrail:
the only promises this receipt genuinely satisfies were already green, and the
yellow candidates' gates need more than local single-task exec.

Scope guard: stay inside the **yellow, local-only** promise
`autopilot.desktop_gui_client.v1`; cloud lanes are the separate **red** promise
`autopilot.cloud_coding_sessions.v1`. Semantic routing only for intent→signature
(Blueprint `signature-lookup`); no ad-hoc keyword matching. Foldkit +
`@openagentsinc/autopilot-ui` — no hand-DOM, no Cargo/Tauri.

### P0.1 — Nav shell + command palette (anti-clutter foundation) — sub-EPIC #5462

The structural fix every other connection routes through. The sidebar is today a
flat 13-button wall with **no command palette and no keyboard shortcuts**. Adding
more top-level buttons makes clutter worse — so this lands first.

- **#5463** — group the flat 13-button NAV into ~5 grouped sections
  (Chat · Code · Supervise · Explore · Settings) with a secondary in-section tab
  strip; top level never grows past the group count.
- **#5464** — command palette (Cmd-K) over a typed command registry
  (navigate / spawn / approve / intent / coordinator / replay commands).
- **#5465** — keyboard shortcut layer + Settings shortcut listing.

> Every other P0 child must route its new surface into a group, the palette, or an
> existing pane's secondary strip — never a new top-level button.

### P0.2 — Connection children (route through the nav shell above)

- **#5466 — Chat-live (continues CLOSED EPIC #5449).** Drive the desktop `chatPane`
  from the **real Blueprint chat-program runtime** (it is presentational/seeded
  today — `blueprintChatScopedSteps` uses hardcoded refs/digests and marks the
  Tassadar step `verified` the instant a session spawns). Live signature selection
  + real Tassadar-module steps + inline replay receipts.
  *(Coordinate: the Blueprint chat files in `apps/autopilot-desktop` and
  `workers/api/src/blueprint` are concurrently edited by a separate lane.)*
- **#5467 — Autonomous-loop view.** Surface intent → plan → fanout → reconcile →
  ship-gate as a first-class view (today it's only an "ask" card + header
  pause/resume toggle).
- **#5468 — Bounded auto-approve.** Expose the `--on-approval auto` policy + audit
  trail in the approvals roll-up.
- **#5469 — Swarm.** Batch launch + account-failover/routing visibility +
  sub-agent tree.
- **#5470 — Diff/transcript fidelity** + artifact & receipt browser in
  session-detail/composer.
- **#5471 — Repo/worktree picker** in the composer (give it a repo + a task).
- **#5472 — Functional settings.** Defaults / theme / notifications + the shortcut
  listing from #5465.

**Definition of done (EPIC #5461):** a Blueprint-driven chat with live signature
selection + real Tassadar-module steps + inline replay receipts; the composer loop
as the day-to-day CLI replacement; swarm with batch/failover/nesting visible;
approvals (incl. bounded auto-approve) and the autonomous loop first-class and
honest; inspectable diff/artifacts — all behind a clean ~5-group nav + Cmd-K
palette, no sidebar clutter.

---

## P1 — Blueprint × Tassadar chat (EPIC #5449 CLOSED; live wiring continues as #5466)

The backend seams + presentational chat pane landed under #5449 (CLOSED). The
remaining work — making the chat **real** rather than seeded — is tracked as
**#5466** above (a P0 child). A separate lane is actively building the chat pane;
coordinate on the shared `src/ui/*` and `workers/api/src/blueprint` files and do
not stomp concurrent edits.

---

## P2 — Owner-action items (gated; pull other work while these wait)

These are owner-armed/owner-gated. Write a clear `NEEDS-OWNER:` note and keep
moving on P0 — none of these stall the day.

- **AO-5 — flip the desktop download to one-click ready.** Set
  `AUTOPILOT_DESKTOP_DMG_URL` and flip `DOWNLOAD_ONE_CLICK_READY=true`.
  **Blocked on:** a published, dereferenceable DMG asset URL. The notarized DMG
  exists locally (`...20260619T010148`, SHA-256 `22db620c…`,
  `docs/launch/artifacts/ao6-20260619T010148/`) but is not yet published at a
  stable download URL. `NEEDS-OWNER:` publish the asset + provide the URL.
- **First real revenue-loop payout.** The clearing layer is wired (RL-1/2/3, EPIC
  #5457 CLOSED) but the **first real dispatched payout is owner-armed**. Until a
  real Bitcoin-revenue production event produces a dereferenceable settled
  referral/firm-up payout receipt, `sites.referral_bitcoin_stream.v1` (yellow) and
  `payments.accepted_outcome_economics.v1` (red) cannot advance. `NEEDS-OWNER:` arm
  the first gated payout.
- **Windows Authenticode cert.** Pylon + desktop are proven on darwin-arm64;
  Windows is not a supported signed-install target. The Windows/WSL coverage gap is
  one of the explicit blockers on
  `pylon.consumer_compute_earns_bitcoin_self_serve.v1` (red). `NEEDS-OWNER:` procure
  the Authenticode signing certificate.

---

## P3 — Test-suite reconciliation (8 pre-existing api-suite failures)

Eight pre-existing api-suite failures predate today and are **not** introduced by
the overnight work; they are doc/fixture drift, not product regressions. Reconcile
honestly (and coordinate — do **not** touch the concurrently-edited Blueprint chat
files in `apps/autopilot-desktop` or `workers/api/src/blueprint`):

- **Blueprint `tassadar-modules` route drift** — the route/contract moved ahead of
  its test expectations; realign the test fixtures to the current route shape
  (after the concurrent Blueprint lane settles, to avoid churn).
- **Stale `AGENTS.md` doc-link assertions** — link-coverage tests reference doc
  paths that moved; refresh the expected link set.
- **Artanis fixtures** — fixture data drifted from the current tick/monitor
  projection; refresh the fixtures to the live shape.

Goal: api-suite green from a clean `origin/main` so money-movement and
public-claim changes keep a trustworthy `check:deploy` gate.

---

## P4 — Other genuinely-open work

- **EPIC #5335 — Codebase hygiene & refactoring lane** (OPEN; open for a lead
  contributor). The funded, benchmark-verified hygiene lane keeps producing
  (first 75-sat hygiene Bitcoin settled June 18 on the `hygiene_merged_reviewed`
  basis). Feed it studied-knowledge (the studying track is built + dogfood-proven)
  so passes start from real codebase understanding.

> The full open issue set as of this roll (13): EPIC **#5461**, sub-EPIC
> **#5462**, children **#5463–5472**, and EPIC **#5335**.

---

## Product-promises review + next-5 green-flip analysis

Reviewed `apps/openagents.com/workers/api/src/product-promises.ts` (registry
version **`2026-06-19.3`** after the coding-agent live-verification destale in
P0.0 above; this analysis was authored against `2026-06-19.2`), `docs/promises/`,
and how the gate computes
green/red. The registry shape is `{ promiseId, state, claim, safeCopy, unsafeCopy,
evidenceRefs, blockerRefs, verification, authorityBoundary }`; a promise is GREEN
only when its `blockerRefs` are cleared and its `verification` bar is met with
dereferenceable evidence, and a state change is recorded receipt-first via
`proof.claim_upgrade_receipts.v1` (a `promise_transition` exception receipt + owner
sign-off).

### 1. Is the registry up to date with overnight reality?

Mostly yes — `f4a97f73a` already destaled to `2026-06-19.2` for desktop onboarding
(#5441) and referral RL-1 (#5458). Promises whose copy/evidence is **now stale**
given the overnight from-DMG proof + revenue-loop wiring (flagged **needs a destale
touch — DO NOT edit the registry here**, owner/registry-lane to update):

- **`autopilot.desktop_gui_client.v1`** (yellow) — `safeCopy` still says the final
  from-DMG proof on a clean external Mac is **"owner-gated and pending"** and lists
  `blocker.product_promises.autopilot_desktop_from_dmg_proof_owner_gated`. That
  blocker is now **cleared** by `cc27f122e` (rendered window + production presence
  `pylon.fa4e9049a4329f3d56e2` + Verified `exact_trace_replay` challenge
  `9fd49062…` + settled real-Bitcoin receipt `…ao6.patched2.20260619T010148.manual.v1`,
  artifacts in `docs/launch/artifacts/ao6-20260619T010148/`). The copy understates
  reality and the blocker should be retired.
- **`sites.referral_bitcoin_stream.v1`** (yellow) — copy is accurate post-RL-1 but
  could note RL-3's live asset-boundary guard now enforces the Bitcoin-only
  rev-share path (`471be0f61`). Minor; still honestly yellow (no real payout).
- **`payments.accepted_outcome_economics.v1`** (red) — the clearing-layer wiring
  (RL-1/2/3) is new evidence the copy doesn't yet cite; still correctly red (no
  end-to-end real settled accepted-outcome receipt).

### 2. Next 5 promises to assess for a GREEN flip (training / Autopilot / coder / adjacent)

**Headline finding:** only **one** promise has a genuinely new, dereferenceable
receipt from the overnight work that clears its gating blocker —
`autopilot.desktop_gui_client.v1`. The other four are the next-closest
training/Autopilot/coder-adjacent candidates, but each still has a concrete
missing receipt, so they are **assessed honestly with the gap named, not
recommended for a flip**.

| # | Promise | Current | Receipt that would flip it (or the gap) | Gate it must satisfy | Confidence |
|---|---------|---------|------------------------------------------|----------------------|------------|
| 1 | `autopilot.desktop_gui_client.v1` | **yellow** | **Receipt EXISTS for the from-DMG blocker:** `cc27f122e` — notarized DMG `…20260619T010148` (SHA-256 `22db620c…`, Gatekeeper Notarized Developer ID), rendered clean-Mac window, **production presence** `pylon.fa4e9049a4329f3d56e2`, Verified `exact_trace_replay` challenge `training.verification.challenge.9fd49062-f82c-46ee-a2a0-242d36dd126e`, settled receipt `receipt.nexus.tassadar_run_settlement.idempotency.tassadar.ao6.patched2.20260619T010148.manual.v1` (`realBitcoinMoved:true`, `spark_treasury`). Artifacts: `docs/launch/artifacts/ao6-20260619T010148/`. **Remaining gap:** the promise's *stated* green bar also lists PDF/preview/ingest/browser live runtimes wired + cloud-lane sessions + decided distribution/pricing — those are NOT met. **Honest verdict: the from-DMG blocker is cleared and the copy should be destaled; a full GREEN requires either narrowing the green bar to "local-only onboarding + coding client proven from DMG" (a scoped green is defensible and owner-decidable) or clearing the runtime/distribution blockers.** | `blocker…from_dmg_proof_owner_gated` cleared; `verification` from-DMG clause met; remaining `live_runtimes_not_wired` / `remote_cloud_lane_not_wired` / `pricing_distribution_undecided` blockers for full scope. Receipt-first transition + owner sign-off. | **HIGH** for the from-DMG blocker / destale + scoped green; **LOW** for an unscoped green (other blockers open). |
| 2 | `autopilot.builtin_compute_agent.v1` | **yellow** | The from-DMG proof produced a **signed/notarized recut** (`…20260619T010148`), which clears the `builtin_compute_agent_signed_recut_missing` blocker *if* that build carries the built-in-agent source. **Gap:** the proof was a **Tassadar earning** run, not a from-install **Go-online built-in-agent session doing useful coding work with no user API key**; `openagents_compute_metering_live_smoke` and `live_from_install_smoke` blockers are still uncleared. | Signed recut containing built-in-agent source + packaged OpenAgents compute credentials + metered/bounded path + a from-install go-online useful-work smoke. | **LOW** (recut exists; the go-online useful-work + metering smokes do not). |
| 3 | `pylon.consumer_compute_earns_bitcoin_self_serve.v1` | **red** | The Episode 238 core promise. Overnight didn't move its blockers. **Gap (all three open):** Windows/WSL install coverage missing; Spark-helper auto-start/readiness not receipt-proven for normal contributors; participant/scale methodology missing. The from-DMG proof is a single owner-run node + a manual settlement, not multi-contributor no-operator earning at scale. | Documented install proven on named platforms (incl. Windows/WSL); helper-readiness evidence; replay/receipt for >1 normal contributor; scale methodology. Receipt-first. | **LOW** (no blocker cleared; needs Windows cert + helper auto-start + scale). |
| 4 | `training.verification_classes.v1` | **yellow** | Registry, three classes on real work, and a paid weak-device validator closeout are already met. **Gap:** one open blocker — `aggregate_only_policy_redecision_missing` (#4674): the April-era aggregate-only validation compromise must be **re-decided per class in writing**; `seeded_replication` + `statistical_cross_check` also have not run on real dispatched work. Overnight produced no such decision/receipt. | Written per-class aggregate-vs-per-contribution decision + (ideally) the two unexercised classes on real work. | **LOW** (gated on a written policy decision, not a receipt the overnight work produced). |
| 5 | `artanis.tassadar_evolution_loop.v1` | **yellow** | The spine is deployed and the first autonomous dispatch-execute-closeout span ran (2026-06-11). **Gap:** two open blockers — `artanis_unattended_tick_streak_missing` (needs ≥10 consecutive unattended ticks with executor-dispatch + exact-replay verdicts) and `tassadar_distillation_dataset_receipt_missing` (the first `dataset_curation` receipt). Overnight produced neither. | ≥10 consecutive unattended ticks with replay verdicts + first distillation-dataset receipt. Receipt-first. | **LOW** (needs a sustained unattended streak + a dataset receipt). |

### 3. Recommendation (NO flips here — owner approval required)

- **Recommend to the owner (HIGH confidence):** retire the
  `autopilot_desktop_from_dmg_proof_owner_gated` blocker on
  `autopilot.desktop_gui_client.v1`, destale its `safeCopy` from "owner-gated and
  pending" to the proven from-DMG state, and **decide** whether to record a scoped
  GREEN (local-only onboarding + coding client proven from DMG) — the receipt chain
  is dereferenceable. A scoped green keeps the cloud lane red
  (`autopilot.cloud_coding_sessions.v1`) honest. Record the transition receipt-first
  per `proof.claim_upgrade_receipts.v1` with owner sign-off.
- **Do NOT flip** candidates 2–5: each still lacks a dereferenceable receipt for at
  least one open blocker (built-in-agent go-online smoke; Windows/helper/scale;
  written per-class verification decision; unattended tick streak + distillation
  receipt). The honest move is to clear the named blocker first, then upgrade
  receipt-first.
- The **green-flip guardrail stands**: receipts + owner sign-off required; this
  section is analysis + recommendation only. No registry edits were made in this
  roadmap.
