# JUNE 18 ROADMAP — CLOSED 2026-06-19

> **CLOSED 2026-06-19.** This roadmap is closed. The day's work shipped (summary
> immediately below); the live thrust continues in
> [`JUNE19_ROADMAP.md`](./JUNE19_ROADMAP.md), headlined by the coding EPIC
> [#5461](https://github.com/OpenAgentsInc/openagents/issues/5461) (Autopilot
> Desktop → full coding agent). Nothing below is deleted — it is the authoritative
> June 18 record plus the overnight-into-June-19 close.

## CLOSE-OUT SUMMARY (what shipped June 18 → overnight June 19) — AUTHORITATIVE

All facts below are real and on `main`. This is the completion record for June 18
plus the overnight work that landed before the June 19 roll.

### Revenue loop — EPIC #5457 CLOSED (the clearing layer is wired)

The "$1 in → >$1 verified value out, fanned to the network" clearing layer is now
wired on live paths (was tested primitives imported on no live path):

- **RL-1 #5458 (CLOSED, `2c83afd4f`)** — referral payout ledger feed + dispatch:
  a paid event resolves attribution → one idempotent eligibility row →
  readiness-gated `approved → dispatched → settled` via the injected MDK/Spark
  adapter (settle-at-most-once, Bitcoin-only rev-share boundary).
- **RL-2 #5459 (CLOSED, `e9581e34a`)** — firm-up escrow → Bitcoin payout on an
  executed verification (fails closed through the shared gates).
- **RL-3 #5460 (CLOSED, `471be0f61`)** — `validateAssetBoundary` (credit↔Bitcoin)
  + no-resale (`authorizeInferenceMonetization`) now enforced as **live guards**
  on the value-movement paths, not just policy/tests.
- **Honest gap:** the **first real referral/firm-up payout is an owner-armed gated
  step** — no real dispatched payout receipt exists yet, so the matching promises
  (`sites.referral_bitcoin_stream.v1` yellow, `payments.accepted_outcome_economics.v1`
  red) do **not** flip. Carried to June 19 owner-action items.

### Desktop auto-onboarding — EPIC #5441 PROVEN + CLOSED (#5447 CLOSED)

The owner ran the **real from-DMG clean-Mac proof** (`cc27f122e`):

- Signed + **notarized DMG** (`...20260619T010148`, SHA-256
  `22db620c…`, app + DMG notarization `Accepted`, Gatekeeper `Notarized
  Developer ID`, `hdiutil verify` VALID).
- Clean first-launch rendered window (no black screen), AO-3 identity choice
  (create-new named), wallet receive-ready, payout target, presence — all from
  live observed state.
- **Production presence:** `pylon.fa4e9049a4329f3d56e2` active on
  the public pylon detail (`walletReady`, `sparkPayoutTargetReady`).
- **Verified Tassadar work + settled receipt:** challenge
  `training.verification.challenge.9fd49062-f82c-46ee-a2a0-242d36dd126e` →
  `Verified` (`exact_trace_replay`); settled receipt
  `receipt.nexus.tassadar_run_settlement.idempotency.tassadar.ao6.patched2.20260619T010148.manual.v1`
  (`amountSats:5`, `realBitcoinMoved:true`, `adapter:spark_treasury`). Artifacts
  in `docs/launch/artifacts/ao6-20260619T010148/`; runbook §3.
- **Honest caveat:** the settlement was the **manual/owner-driven** admin path
  (still not the first fully-autonomous auto-stream live receipt).

### product-promises registry → `2026-06-19.2` (destale only, no green flips)

`f4a97f73a` destaled the registry to `2026-06-19.2` (desktop onboarding #5441 +
referral RL-1 #5458 copy/evidence accuracy). **No gate flips.** (Earlier same-day
`.3`/`.4`/`.5` copy upgrades are recorded in the sections below.)

### Desktop → full coding agent roadmap filed (the June 19 main thrust)

EPIC **#5461** (Autopilot Desktop → full coding agent) + nav sub-EPIC **#5462**
(children **#5463** grouped nav / **#5464** Cmd-K palette / **#5465** shortcuts) +
connection children **#5466** chat-live / **#5467** autonomous-loop view / **#5468**
bounded auto-approve / **#5469** swarm batch/failover/nesting / **#5470**
diff-artifact fidelity / **#5471** repo/worktree picker / **#5472** functional
settings. Audit: `docs/launch/2026-06-19-autopilot-desktop-coding-agent-audit.md`.

### Blueprint × Tassadar chat — EPIC #5449 CLOSED

The Blueprint × Tassadar chat EPIC (#5449, + #5450–5456) is CLOSED — the backend
seams + presentational chat pane landed there. The **chat-live wiring** (drive the
pane from the real Blueprint chat-program runtime) continues as **#5466** under the
coding EPIC.

### Platform: Lathe — MERGED + deployed

Lathe route-dedup **#5448** and forum-theme **#5473** merged + deployed.

### Still open carrying into June 19

Open issue set (13): the coding EPIC **#5461** + nav sub-EPIC **#5462** + their 10
children (**#5463–5472**), and the hygiene/refactoring EPIC **#5335**. Owner-action
items (AO-5 download flip, first real revenue-loop payout, Windows Authenticode
cert) and the 8 pre-existing api-suite test failures carry to `JUNE19_ROADMAP.md`.

---

# JUNE 18 ROADMAP - launch rc.31 and test the real Tassadar run logic (historical record)

Date: 2026-06-18, 07:18 CT. Carries forward the still-live launch/test work
from [`JUNE17_ROADMAP.md`](./JUNE17_ROADMAP.md), now that the Tassadar
LLM-computer roadmap is implemented on `main`.

## END-OF-DAY UPDATE 3 (2026-06-18, latest) — v1.0.1 shipped + visibility/replay audit — AUTHORITATIVE

> Newest layer on top of all sections below. Honest split: **shipped** vs **in
> flight** vs **owner-gated / not-yet**. Supersedes earlier sections where they
> differ on these specific items.

### Shipped (since UPDATE 2)

- **Pylon v1.0.1 cut (`e8131c9`)** — `release(pylon): cut v1.0.1 — balance --json
  fix + payout-target surfacing (Gap #2)`. The `balance --json` output is fixed
  and the payout-target is now surfaced (closes the Gap #2 surfacing item). This
  is the current `origin/main` tip.
- **Self-serve open-window producer — DEPLOYED.** The scheduled producer
  (`workers/api/src/index.ts` `runSelfServeWindowProducerScheduled`, `#5396`)
  maintains a pool of openly-claimable Tassadar windows
  (`run.tassadar.executor.20260615`, `homeworkKind:auto_starter`) so a fresh
  contributor always has a window to claim without an operator opening one. Live.
- **World-firsts — VERIFIED + finalized** (carried from UPDATE 2, §C / L-3,
  `#5395` CLOSED): both claims defensible **only with their full qualifiers**;
  registry promises stay RED pending an owner-signed receipt-first upgrade.
- **Visibility/replay audit (this lane).** A full inventory of live + replay
  visibility/observability infrastructure landed at
  [`docs/launch/2026-06-18-pylon-visibility-replay-audit.md`](./2026-06-18-pylon-visibility-replay-audit.md).
  Headline: the **data layer is built and live** (pylon-stats, capacity funnel +
  history, tassadar-run-summary, enumerable settlements feed, verification
  challenges, settlement receipts, artanis tick log, proof-replay bundle, the
  SpacetimeDB live world) — all proven HTTP 200 live; but the **consumption
  layer is partial**: no single live-activity surface, no real-time push (web is
  polling), replay is two curated stories (no general "replay any
  run/window/pair/range"), no unified cross-domain event timeline, and **desktop
  + CLI see almost none of the fleet/money/forum live picture** (CLI is
  self-only). Recommended path: Phase 0 composed live page → Phase 1 unified
  public activity-timeline endpoint (keystone) → Phase 2 general replay generator
  + fleet/forum replay → Phase 3 push + desktop/CLI fan-out → Phase 4 clip
  productization (`#5346`, in flight). No new spend/settlement authority; reuse
  the existing staleness contract + public-safe scrub.

### Open items (honest)

- **Spark-helper auto-start for non-technical users — OPEN.** The default
  install still assumes the operator can bring up the Spark wallet helper; a
  one-step auto-start for non-technical users is not yet shipped. Gates the
  "anybody plugs in and earns" promise for non-technical contributors.
- **Windows support — `#5404` OPEN.** Pylon is proven on darwin-arm64; Windows
  is not yet a supported install target.
- **Visibility/replay productization — OPEN (this audit's recommended path).**
  Phases 0–4 above; Phase 1 (unified activity timeline) is the keystone.
- **First fully-autonomous auto-stream live settlement — still OPEN** (L-2
  `#5394`): the resolver is fixed and both legs are proven in test, but no
  hands-off live receipt (gate firing at verdict, no operator) has been
  dereferenced yet. Flag the first when it lands.
- **Worker redeploy to serve registry `2026-06-18.5`** + updated AGENTS.md/
  INSTALL.md — owner-gated (carried from UPDATE 2).

## END-OF-DAY UPDATE 2 (2026-06-18, late) — launch-readiness program + first hygiene Bitcoin — AUTHORITATIVE

> Latest whole-day picture, layered on top of the FULL-DAY STATUS and the
> earlier END-OF-DAY UPDATE below (both remain accurate for their lanes). Honest
> split throughout: **shipped** vs **in flight** vs **owner-gated / not-yet**.
> No green claim without dereferenceable receipts.

### A. Launch-readiness program — EPIC `#5392` (OPEN)

The "anybody plugs in consumer compute → gets paid Bitcoin" launch
(`docs/transcripts/238.md`, "The Tassadar Run is Live") is now a tracked program
with six explicit launch blockers. Audit:
[`docs/launch/2026-06-18-pylon-v1-launch-readiness-audit.md`](./2026-06-18-pylon-v1-launch-readiness-audit.md).
V1.0 = the contributor earning path only (NOT the five revenue streams, the
module marketplace, or our headless coding workflow).

- **L-1 — working default install — IN FLIGHT.** `npx @openagentsinc/pylon` must
  install a node that can join the run and earn. The npm dist-tag was stale
  (`latest = 0.2.5` bootstrap stub vs the working in-repo build); the **stable
  v1.0 cut landed in-repo** (`e738443e1`, `release(pylon): cut stable v1.0.0 —
  fix Launch L-1 npx install`) and the **v1.0 npm publish is landing now** (the
  publish + Worker deploy are owned by the concurrent v1.0-publish lane). Until
  `latest` resolves the v1.0 node on npm, treat L-1 as not-fully-closed.
- **L-2 — fully-autonomous self-serve settlement — done (code), live-receipt
  still pending.** The validator-leg payout-target bug is **fixed and proven**
  (`24cb8f30a`, `fix(settlement): resolve validator-leg payout target by
  device-ref for autonomous auto-stream`, `#5394`/`#5310`/`#5306`): the validator
  submitted its verdict under a device-ref (not a pylonRef), so the owner-scoped
  Spark resolver never found its target and forced an operator retro-settle. The
  fix adds a device-ref → most-recent-worker-pylonRef backstop, fails closed,
  crosses no ownership, and arms no new authority. A real test
  (`tassadar-auto-settlement-validator-resolution.test.ts`) exercises the actual
  resolver chain and proves **both legs (worker + validator) auto-settle
  hands-off** to the correct private destinations. `#5310` (validator per-window
  5 sats) and `#5306` (default Spark payout target) are CLOSED. **Honest caveat:**
  the L-2 issue `#5394` stays OPEN because no fully-autonomous live receipt
  (gate firing at verdict, no operator) has been dereferenced yet — flag the
  first one explicitly when it lands.
- **L-3 — world-firsts independently verified — DONE, `#5395` CLOSED.** An
  independent prior-art / competing-claim review landed
  ([`docs/launch/2026-06-18-world-firsts-verification.md`](./2026-06-18-world-firsts-verification.md),
  `d354361ff`); prior art checked includes Spirit of Satoshi, Bittensor/Templar,
  Gensyn, Prime Intellect, Nous/Psyche, Salad, Percepta, Tracr. Verdict: both
  claims are defensible **only with their full qualifiers** — (1) first as
  "Bitcoin + replay-verified training compute + own consumer devices" together;
  (2) first as "public/open-contributor LLM-computer training run," crediting
  Percepta as the paradigm originator. The verification work is finalized and
  `#5395` is CLOSED. **Honest caveat:** the matching registry promises
  (`claims.world_first_*`) stay **RED** pending an evidence pack + owner-signed
  receipt-first upgrade; any public use must carry the full qualifiers, not bare
  "world first" phrasing.
- **L-4 — define + cut Pylon v1.0 — done (stable cut `e738443e1`);** npm publish
  landing now (concurrent lane).
- **L-5 — contributor onboarding path end-to-end** (agents.md front door → join
  run → install → claim → earn): **in flight** — this docs update flips the live
  AGENTS.md + INSTALL.md to Pylon-first; homepage is already Pylon-only.
- **L-6 — public evidence pack** (run summary, settled receipts, verification,
  promise registry, all linkable): partially present (settled feed +
  verification challenges + registry are live); consolidation in flight.

### B. First real hygiene-lane Bitcoin — hygiene canary SETTLED (75 sats)

The hygiene/refactoring lane (EPIC `#5335`, `#5372`) produced its **first real
Bitcoin settlement**: a **75-sat** payout to a contributor for a merged,
benchmark-verified hygiene debt receipt, settled native over the proven
`#5232` Spark treasury rail. Verified `realBitcoinMoved:true`, **idempotent**,
with **duplicate-replay rejection** (one settlement per receipt). Honest
verification basis: **`hygiene_merged_reviewed`** — hygiene PRs are verified by
tests + reviewer acceptance + the merged debt receipt, **not** by exact trace
replay, so this path never emits an `exact_trace_replay` verdict or a
`verificationChallengeRef`. The churn-tax formula (size/depth-scaled, ≤100 sats)
and idempotency-ref hashing landed (`#5385`, `#5388` — both MERGED;
`ae515ee9f`, `2f1586eee`). This is distinct from the Tassadar run settlements:
it is the **first paid hygiene-lane outcome**. EPIC `#5335` and `#5372` stay
OPEN (the lane keeps producing).

### C. World-firsts — VERIFIED + finalized

See L-3 above. The two world-first claims are independently verified with
defensible narrowed wording (Bitcoin + replay-verified + own consumer devices;
public/open-contributor LLM-computer run, crediting Percepta). `#5395` CLOSED.
Registry promises remain RED until an owner-signed receipt-first upgrade — the
qualified wording is the only safe public form.

### D. Product-promises registry → `2026-06-18.5`

`apps/openagents.com/workers/api/src/product-promises.ts` is at
**`2026-06-18.5`**, adding four honest records (no green, no flip of existing
promises):

- `claims.world_first_ai_training_paid_bitcoin.v1` — **RED**
  (gated-pending-verification; independent search cited; needs evidence pack +
  owner-signed upgrade; full qualifiers only).
- `claims.world_first_public_llm_computer_training_run.v1` — **RED** (same gate;
  credits Percepta; plus the "no gradient descent" / executor-PoC accuracy
  boundary).
- `pylon.consumer_compute_earns_bitcoin_self_serve.v1` — **RED** (the video's
  core promise; gated on a proven fully-autonomous self-serve settlement (L-2)
  AND a working default install (L-1)).
- `marketplace.agentic_npm_module_registry.v1` — **PLANNED** (the transcript's
  "upcoming video" module-marketplace reboot; roadmap language only, not live).

**Worker redeploy required** to serve `2026-06-18.5` at
`/api/public/product-promises`; that deploy is owner-gated and is owned by the
concurrent v1.0-publish lane.

### E. Headless coding-workflow program — EPIC `#5376` (OPEN)

Running OpenAgents' own coding workflow headlessly through Pylon is the real
operational bar (it gates distributable downloaded apps).

- **W-1 `#5377` — SHIPPED** — headless run-to-completion task primitive
  (`sessions exec` / `spawn --wait --json`) (`b9694b311`); the W-1 verify-CWD bug
  `#5389` is CLOSED (`7354c4f42`).
- **W-3 `#5379` — SHIPPED** — bounded autonomous approval policy for headless
  `sessions exec` runs (`cd75c94ae`).
- **W-7 `#5383` — mechanism proven (issue OPEN)** — the end-to-end dogfood (run a
  real OpenAgents coding task as a Pylon-managed session) mechanism is proven;
  the dogfood-proof issue stays OPEN pending the recorded end-to-end pass.

### F. Homepage — Pylon-only

The homepage install surface is **Pylon-only**: the Autopilot Desktop DMG was
removed and the install focus is Pylon (`b85391e2b`, `feat(web): focus homepage
install on Pylon, remove Autopilot DMG`). This docs update brings the live
AGENTS.md and INSTALL.md into line (Pylon-first; Autopilot Desktop secondary).

### Honest shipped / in-flight / owner-gated split

- **Shipped:** validator-leg auto-settle fix with both-legs-proven test
  (`#5310`/`#5306` closed); world-first independent verification (`#5395`
  closed); first real hygiene-lane Bitcoin (75-sat canary, idempotent,
  duplicate-replay-rejected, `hygiene_merged_reviewed` basis); hygiene churn-tax
  backtest + idempotency-ref hashing (`#5385`, `#5388`); W-1 + W-3 headless
  primitives (`#5377`, `#5379`); stable Pylon v1.0 cut (`e738443e1`);
  homepage Pylon-only; registry `2026-06-18.5` (source).
- **In flight:** L-1 npm v1.0 publish (concurrent lane); L-5 onboarding path
  (this docs flip); L-6 evidence-pack consolidation; W-7 recorded dogfood pass;
  EPIC `#5335` / `#5372` hygiene lane keeps producing.
- **Owner-gated / not-yet:** first fully-autonomous auto-stream live settlement
  (L-2 issue `#5394` still open — flag the first when it lands); world-first
  green flips (need owner-signed receipt-first upgrade; RED until then);
  `pylon.consumer_compute_earns_bitcoin_self_serve.v1` (RED, needs L-1 + L-2);
  Worker redeploy to serve `2026-06-18.5` and the updated AGENTS.md/INSTALL.md
  (owner-gated; owned by the concurrent v1.0-publish lane).

---

## FULL-DAY STATUS (2026-06-18, end of day) — authoritative

> This is the consolidated end-of-day picture across every lane that moved
> today. The **END-OF-DAY UPDATE** section below it is the detailed RC +
> settlement record from earlier in the day and remains accurate for that lane;
> read this section first for the whole-day view. Honest split throughout:
> **shipped** vs **in flight** vs **owner-gated**.

### 1. Forum tipping — native Spark-address destinations — SHIPPED, `#5345` CLOSED

- **Shipped:** native Spark-address (Spark→Spark) forum tip destinations
  (`268f50601`, `feat(forum): accept native Spark-address tip destinations`).
  Worker deployed (version `34def6d2`, D1 migration `0206`).
- Raynor's tip wallet is registered: `tippingAvailable=true`,
  `directPayment.kind=spark_address`, Spark offline-receive.
- **Root cause of the earlier "pending" tip** was Tailscale MagicDNS failing to
  resolve the Spark backend — an environment/DNS issue, **not** a code bug. The
  payment rail itself was correct.
- **`#5345` is CLOSED.**

### 2. Studying activation — EPIC `#5337` CLOSED (complete)

The studying capability is now wired end-to-end into the live agent loop, not
just benchmarked. EPIC `#5337` and SA-1 through SA-4 are closed; SA-5 remains
open and owner-gated.

- **SA-1 `#5338` (CLOSED)** — live, current `openagents` study packet artifact +
  regenerate CLI (`fd1ba44414`).
- **SA-2 `#5339` (CLOSED)** — the Autopilot-coder consumes the study artifact in
  the **live tool-menu plan path**, with measured lift (`7823e83f5`).
- **SA-3 `#5340` (CLOSED)** — studied-knowledge wired into the
  hygiene/refactoring lane (typed debt-receipt key model + studied-knowledge
  wiring, `a2f3fc428`; follow-on hygiene commits across the
  `ffccc6f4..f15d8332e` range).
- **SA-4 `#5341` (CLOSED)** — standing freshness signal (fresh / stale /
  gate-failed) + automatic study-index refresh on change (`8a8339304`,
  `f15d8332e`).
- **SA-5 `#5342` — OPEN and OWNER-GATED.** Advancing
  `autopilot.repo_study_packets.v1` toward its gated capabilities
  (customer-repo studying, marketplace packaging, payout) is **held**; those
  gates (privacy / metering / pricing / payout) are not cleared.

### 3. Debt-receipt model + hygiene lane — EPIC `#5335` OPEN, lane producing

The hygiene/refactoring lane is now a funded, benchmarked, verified-contributor
lane with a typed receipt model and an explicit production/verification role
split.

- **Typed model (shipped):** `DebtReceiptKey` / `PatchNoveltyKey` — **one
  settlement per receipt**, with duplicate-replay rejection. Plus a fail-closed
  fix to an optional gate and a new **"Debt Receipt Hygiene Settlement"**
  invariant.
- **Role split adopted:** **Trigger = production** (hygiene patches),
  **Orrery = verification / churn-probe** (independent verification + scanning
  for churn classes).
- **`#5334` retired** via **PR `#5336` (MERGED)** — the first verified
  contributor pass (de-dup dual-format Tassadar generated fix).
- **Merged Trigger hygiene PRs:** `#5352`, `#5354`, `#5356` (canary receipt
  test fixed), `#5357`, `#5358`, `#5359`, `#5365` — all **MERGED**.
- **In review:** PR `#5366` (forum work-request route contract) and PR `#5367`
  (reuse stable hash helpers in StudyBench) — both **OPEN**.
- **Closed-as-incorporated (Trigger credited):** PRs `#5343`, `#5344`, `#5355`
  — **CLOSED**, work folded into the merged set.
- **New churn-probe scan class banked:** "dual-source-of-truth divergence" is
  now an enumerable churn-probe scan class.
- **Verification:** Orrery verified the **1,005-sat settled-total reconciliation
  is clean across all three endpoints** (resolving the earlier 1,005-vs-1,010
  watch item — the simulation row is now correctly excluded in the reconciled
  view).
- **EPIC `#5335` stays OPEN** (the lane keeps producing).

### 4. Autopilot Desktop coding service — NEW EPIC `#5360` — CURRENT TOP PRIORITY

Making Autopilot Desktop an operational day-to-day coding surface is **today's
top priority** going forward.

- **SHIPPED — interactive coding composer pane (`4587b9c82`):** foreground
  spawn → streamed transcript → inline approvals → reply / continue → cancel.
  Proven against `pylon dev`.
- **Audit doc (`9bc8b1563`):**
  [`docs/launch/2026-06-18-autopilot-desktop-coding-surface-audit.md`](./2026-06-18-autopilot-desktop-coding-surface-audit.md).
- **Bucket A (make the surface good):**
  - **CS-A1 `#5361` — IN FLIGHT** — provider / account picker + multi-account.
  - **CS-A2 `#5362` — OPEN** — swarm / multi-session view.
  - **CS-A3 `#5363` — OPEN** — diff / turn fidelity + transcript.
- **Bucket B (make it shippable):**
  - **CS-B1 `#5364` — OPEN** — packaged headless node + signing / notarization.
    This is **the operational gate** for downloaded apps; until it lands, the
    coding surface is dev-proven but not distributable as a signed app.
- **EPIC `#5360` is OPEN.**

### 5. Replay clips — NEW EPIC `#5346` — pipeline shipped, true-3D port closed, more in flight

- **R-1 headless-render spike (`a90b1cf28`, `#5347` OPEN):** proved headless
  rendering works (one-frame Playwright render spike).
- **Pipeline R-3 / R-4 / R-5 — DONE (`60a5e640a`):** headless clip pipeline,
  `render-clip.mjs` → mp4.
- **Remotion port audit (`docs/launch/2026-06-18-remotion-port-audit-for-replay-clips.md`):**
  verdict **PORT THE PATTERN, do not fork or take a runtime dependency** — the
  Remotion license forbids fork-to-sell. Cross-reference:
  [`2026-06-18-remotion-port-audit-for-replay-clips.md`](./2026-06-18-remotion-port-audit-for-replay-clips.md).
- **R-1a `#5353` — CLOSED** — true-3D `three-effect` port of the proof-replay
  scene.
- **R-2 `#5348` — CLOSED** — programmatic camera-path input (call-time camera
  control). (Both R-1a and R-2 were driven by a separate agent and have since
  landed/closed.)
- **Replay gap-analysis doc landed (`f5fb994c2`).**
- **EPIC `#5346` is OPEN** (clip-generation productization continues).

### 6. Platform

- **Social-preview cards LIVE:** OG / Twitter cards for forum thread pages are
  live (`f3461c7d0`, `feat(forum): social preview cards for forum thread
  pages`).
- **Product-promises registry bumped to `2026-06-18.4`**
  (`apps/openagents.com/workers/api/src/product-promises.ts`):
  - the **paid-work promise copy was corrected** (the two-contributor / 1,005
    real-sats accuracy upgrade, no gate flip), and
  - a new **`compute.agentic_kernel_optimization_at_scale.v1` record was added
    as RED** — coding agents continuously writing and optimizing inference
    kernels across open models/devices, scored on **both** throughput **and**
    output-parity (exact-replay verified), dispatched/paid through the
    verified-work market. Direction, not a shipped network capability; the only
    demonstrated piece is a historical single-machine dev result, so it stays
    red.
- **New INVARIANT — "No GitHub-Hosted CI / Cloud Actions":** removed
  `.github/workflows/restudy-openagents.yml` and banned GitHub-hosted CI
  (`c43992567`). CI / cron / study-freshness now runs on **owned GCE infra**,
  consistent with the "our cloud = OpenAgents GCE" posture.

### 7. Launch video series — IN PREP (recording today)

- **Two launch videos are in prep**, recording today:
  - **Video 1 — "The Tassadar run is live"** (reframed to the **learning-engine
    narrative** with precise, verifiable world-firsts):
    - **first open contributor network for the exact-execution / Percepta-class
      paradigm**, and
    - **first training run paying contributors in Bitcoin for verified
      training-compute on consumer devices** (distinct from Spirit of Satoshi).
  - **Video 2 — referral / revenue-share** planning.
- **Note on location:** the launch-video *planning docs* live in the **private
  root workspace**, not in this public repo. This roadmap references the
  **launch-video effort and the public-safe Video 1 framing above** only; it
  does **not** reproduce private planning-doc contents or paths.

---

## END-OF-DAY UPDATE (2026-06-18) — what shipped (authoritative current state)

The morning plan (launch the RC, self-test, invite testers) is **done**, and the
RC moved rc.31 → **rc.33** as self-testing and a real tester (Trigger) shook out
bugs. All on npm `rc` + signed OTA 4-platform rollout 100 + GitHub prereleases;
`latest` stays `0.2.5`.

**RC progression (why each cut):**

- **rc.32** — pre-invite self-test fixes so basic commands don't break for
  testers: `pylon --version`/`-V` and bare `--help`/`-h` were booting the node and
  crashing on the control port (now short-circuit + exit); raw port-in-use crash →
  clear actionable error; stale "Pylon v0.3" crash banner → real version; the
  bundled Breez SDK storage banner was corrupting `wallet status --json` /
  `backup-status --json` stdout (guard added).
- **rc.33** — the Breez guard installed too late vs module-eval order in the
  compiled binary, so `wallet status --json` still leaked the banner (Trigger hit
  it). Fixed by installing the stdout guard eval-first (top-level side effect on the
  first import). Verified on the signed darwin-arm64 binary; **Trigger confirmed
  `wallet status --json` parses clean on rc.33.**

**RC thread — POSTED (owner approved).** Release Candidates thread:
<https://openagents.com/forum/t/6cb2d165-7a65-495d-a21c-6a3a546ad759> (title
corrected to rc.32; consolidated to one clean OP; my redundant reply deleted —
which required building real post-deletion + a topic-rename endpoint, below).

**Self-tested the full path before inviting anyone.** On the live run
`run.tassadar.executor.20260615`: worker `claim` → `submit-trace` (pinned
loop-sum, digest match, 80 steps) → an independent validator (`validate --auto`,
distinct device) replayed (exact digest match) → challenge **`Verified`**. Caught
5+ tester-facing bugs ourselves first.

**Settlements (precise, per Orrery's dereference):** two real-Bitcoin settled
receipts on the run —

- **Orrery 1,000 sats** (`pylon.448ba824…`, ~01:34Z) — the owner-armed canary;
  **first real settlement, full stop** (it proved the *rail*).
- **Trigger 5 sats** (`pylon.81f0facfe…`, ~14:13Z) — **first independent
  contributor through the rc.32 self-serve public path** (install → register →
  claim → submit → independent validation → paid). It proved the *door*.
  **Operator-retro-settled** via the admin settlement endpoint because the
  auto-stream *skipped* at verdict time (the payout-target resolution bug, fixed
  below).
- **The first fully-autonomous auto-stream settlement (gate firing at verdict, no
  operator POST) has NOT happened yet** — the next Verified pair should be it, now
  the resolver is fixed. Flag it explicitly when it lands.
- **Real settled total = 1,005 sats** (1000 + 5). The public aggregate
  `providerConfirmedSettledPayoutSats` currently reads 1,010 because it counts
  `state:settled` without filtering movement, so it includes an old 5-sat
  **simulation** receipt (`…59ba1f30…`, `mdk_agent_wallet`,
  `realBitcoinMoved:false`). Reconcile fix in flight (below).

**Bugs fixed + deployed today (openagents.com Worker):**

- **Settlement payout-target resolution** — Verified contributors with a *ready*
  Spark target weren't paid because settlement resolved by the lease's worker
  device-ref while the target is registered under the pylonRef; added an
  owner-scoped canonical fallback (fail-closed). Unblocked Trigger's payout.
- **Real forum post deletion** — tombstoned posts were rendering a broken
  `content.forum.post.<id>` placeholder; now excluded from the topic projection
  (audit row kept, counts corrected). Cleared the bad post.
- **#5333 self-serve agent displayName** — `PATCH /api/agents/me` so an agent can
  rename itself (propagates live to `/api/pylons`; Forum author names are per-post
  snapshots — flagged, no risky backfill). Closed.
- **Forum topic-title rename** — `PATCH /api/forum/topics/{id}` (author-only),
  built to fix the stale RC thread title.

**In flight (last open item):** reconcile the run-level settled state from settled
receipts (stop showing stale manifest `settlementState:pending`), add an
enumerable REST settled-feed endpoint keyed by run, and **fix the real-vs-sim
total so the run reads 1,005 real, not 1,010** — all per Orrery's reconciliation
asks. On deploy, post the corrected numbers + the feed URL for re-dereference.

> **Resolved later in the day (see FULL-DAY STATUS §3):** Orrery verified the
> **1,005-sat settled-total reconciliation is clean across all three
> endpoints** — the simulation row is excluded in the reconciled view. The
> "1,010 vs 1,005" discrepancy was a not-yet-filtered aggregate, now reconciled.

**Monitoring:** all-day forum-reply + GitHub-issue watchers armed; regular
public-safe progress updates posted as Raynor as each fix lands. Two independent
contributors are actively stress-testing — Trigger (client + settlement path),
Orrery (receipt-trail + projection reconciliation, with sha256/Nostr/OTS
pre-commitments).

**Overnight (context):** the Tassadar roadmap EPIC #5313 + child issues
#5314–#5332 were built (C/V/E/H tracks) and the run-state audit was reframed to the
Percepta **LLM-computer** paradigm — training = compiling programs into
transformer weights, verified by exact replay, not gradient descent.

**Morning closeout criteria — MET:** RC verified across npm/OTA/GitHub (rc.33);
the live Worker includes the current Tassadar code (deployed); an actual-run Pylon
path was exercised through contribution + independent replay → `Verified` +
settled; the RC thread is posted (owner-approved); launch docs updated (this
section). The remaining reconciliation fix is tracked above.

## Product-promise status (end of day)

Audit of the product-promises registry (`apps/openagents.com/workers/api/src/product-promises.ts`)
and `docs/promises/` against today's dereferenceable receipts. Registry bumped
`2026-06-18.2 → 2026-06-18.3` (copy/evidence accuracy upgrade only — no gate flip).

> **Later in the day the registry advanced again to `2026-06-18.4`** (see
> FULL-DAY STATUS §6): the paid-work promise copy correction landed and a new
> **`compute.agentic_kernel_optimization_at_scale.v1`** record was added as
> **RED** (direction, not a shipped capability — no gate flip). The analysis
> below remains accurate for the `.3` copy upgrade and the no-flip verdict.

### Q1 — is "paying people for verified work" fully + accurately reflected?

**Verdict: covered, and now accurate (after this update).** Paid-verified-work /
streaming-settlement / paid-contributions is reflected by a *set* of promises, not
one named "streaming settlement" promise (the registry models claims/capabilities,
not mechanisms — that is correct):

- `training.decentralized_training_launch.v1` (**green**) — the primary one for
  today: install Pylon → verified Tassadar exact-trace-replay work → independent
  validator replay → real Bitcoin settlement over Spark. This is where the
  streaming-settlement gate (run-scoped, 5w/5v, 100/payout, 50k/day) and the
  real settled receipts live.
- `labor.forum_work_requests.v1` + `labor.nostr_negotiation_market.v1` (**green**)
  — the live labor/work-request market (credit-ledger settlement; external-wallet
  labor payout still gated under `provider.compliant_usage_labor.v1`, yellow).
- `payments.accepted_outcome_economics.v1` (**red**) — the formal
  paid/accepted/payable/dispatched/settled state-machine gate (correctly red).
- `pylon.five_bitcoin_revenue_streams.v1`, `pylon.compute_revenue_modes.v1`,
  `pylon.data_trace_revenue.v1` (**planned**) — broad earning, correctly planned.

**Gap found and corrected (under-statement):** the green
`training.decentralized_training_launch.v1` and the registry notes were frozen at
the Orrery-canary state — they said "**one** contributor has now been paid real
Bitcoin" and cited only the 1,000-sat canary receipt + the simulation receipt.
Today's reality is **two** distinct real settled receipts to **two** distinct
independent contributors, and a live enumerable settled feed. The copy understated
the proof. Corrected to "two distinct independent contributors paid", 1,005 real
sats total, with the second (self-serve) receipt + the settled feed added as
evidence, and the operator-retro-settled / no-auto-stream-yet caveat made explicit.
No scope widened; codebase-contribution paid work ("soon") remains a future claim
and was not added.

Dereferenceable evidence used:
- `GET /api/public/training/runs/run.tassadar.executor.20260615/settlements` — three rows:
  1,000-sat **real** (canary, `pylon.448ba824…`), 5-sat **real** (self-serve,
  `pylon.81f0facfe…`, `…retro.10c3b01b.trigger.v1`), 5-sat **simulation**
  (`realBitcoinMoved:false`, excluded). Real total = **1,005**.
- Second real receipt `receipt.nexus.tassadar_run_settlement.settlement.tassadar.retro.10c3b01b.trigger.v1`
  (`realBitcoinMoved:true`, `state:settled`, `adapter:spark_treasury`), backed by
  Verified challenge `training.verification.challenge.10c3b01b-c781-4a03-a8ed-4ae6c6195fe4`.

Honesty caveats preserved: the **first fully-autonomous auto-stream settlement
has not happened** (the 5-sat self-serve payout was operator-retro-settled because
the auto-stream skipped at verdict, payout-target bug since fixed); and the
run-summary `settledPayoutSats` / `/api/public/pylon-stats` 24h aggregate still
read **1,010** because they have not yet excluded the simulation row — the
`/settlements` feed (1,005) is the reconciled per-run truth and the aggregate
reconciliation is tracked separately (Orrery is actively dereferencing it).

### Q2 — today's gates: flip-status + evidence

| Promise | State | Flip today? | Evidence / why |
| --- | --- | --- | --- |
| `training.decentralized_training_launch.v1` | green | **No flip — copy upgrade only (done)** | Already green. Two real settled receipts (1,005 sats) + live settled feed. Updated copy/evidence in registry `2026-06-18.3`. State unchanged green→green, so no `promise_transition` required; an optional exception receipt for the copy upgrade is **owner-gated** per `proof.claim_upgrade_receipts.v1`. |
| `training.public_distributed_training_run.v1` | red | **No** | Correctly red. Two bounded settlements + one verified pairing do not prove network-scale participation, a participant-count methodology, or broad multi-contributor accepted-work receipts. Stays red. |
| `pylon.first_real_model_training_run.v1` | yellow | **No** | Unrelated evidence base (CS336 A1 two-device real-gradient run). Today's Tassadar receipts do not bear on its model-ladder-network-rungs blocker. Stays yellow. |
| `training.public_gradient_windows.v1` | planned | **No** | H1 has code-backed psionic frozen-core validation + quarantine→recompute→canary→promotion gate, but no public contributor gradient window has been accepted/promoted/paid/settled. Public devices do generation/validation/eval only. Stays planned. |
| `payments.accepted_outcome_economics.v1` | red | **No** | The formal settlement state-machine + contributor-ledger + gross-margin gates are not met. Stays red. |
| `autopilot.repo_study_packets.v1` / `autopilot.external_repo_studying_pilot.v1` (studying→Autopilot-coder) | yellow | **No** | Internal-dogfood / refs-only pilot. Customer-repo studying, marketplace packaging, pricing, payout, and settlement remain blocked. Stays yellow. |

**No gate is a warranted flip today.** The single warranted registry change was the
green-promise copy/evidence accuracy upgrade above (no state change). Conservative
per the 2026-06-18 read: the run constructs **no new capability** beyond the fixed
executor workload, and the first fully-autonomous auto-stream settlement has not
landed — so no red/yellow/planned promise advances on today's evidence.

### Owner sign-off needed

- **Optional:** record a `promise_transition` *exception* receipt for the
  `2026-06-18.3` green→green copy upgrade via the operator route (per
  `proof.claim_upgrade_receipts.v1`), if the owner wants the copy upgrade itself
  dereferenceable as a transition. Not required for correctness (state unchanged).
- **Worker redeploy required:** `product-promises.ts` changed, so
  `/api/public/product-promises` will not serve `2026-06-18.3` (Trigger receipt,
  settled feed, two-contributor copy) until the `openagents.com` Worker is
  redeployed. **Not deployed by this change** — deploy is owner-gated.
- **Watch item (not a registry change):** flip-to-1,005 of the run-summary
  `settledPayoutSats` field and the pylon-stats 24h aggregate (currently 1,010,
  simulation row not yet excluded) is the in-flight reconciliation already tracked
  in the END-OF-DAY UPDATE; the registry now points readers to the `/settlements`
  feed as the reconciled truth in the meantime.

> The sections below are the **morning handoff record** (07:18 CT) and are
> superseded by the END-OF-DAY UPDATE above where they differ.

## Status at morning handoff

- Public AGENTS reviewed at <https://openagents.com/AGENTS.md>. The Release
  Candidates forum is the right public feedback surface:
  <https://openagents.com/forum/f/release-candidates>. Do not publish the
  Raynor thread until the owner explicitly says to post it.
- Local release runbook reviewed: [`docs/DEPLOYMENT.md`](../DEPLOYMENT.md).
  RCs stay prerelease-only; npm `latest` must remain the stable launcher
  (`0.2.5` as of this morning).
- Current Pylon source version is `1.0.0-rc.31`
  (`apps/pylon/package.json`, `apps/pylon/src/version.ts`). `npm view
  @openagentsinc/pylon dist-tags` reports `rc: 1.0.0-rc.31`, `latest: 0.2.5`.
  GitHub prerelease `pylon-v1.0.0-rc.31` is published
  (2026-06-18 04:04 UTC). Verify signed OTA/feed state before claiming the
  auto-update surface is on rc.31.
- The Tassadar roadmap EPIC #5313 and child issues #5314-#5332 are closed as
  completed. Final commits on `main`:
  - `93b4ecbbf` through `657cd9270` in `openagents`: C1-C5, V1-V3, E1-E3, H1
    public execution, replay, settlement-simulation, marketplace, labor,
    adversarial verification, and gradient-window gates.
  - `8e74fc2d` in `psionic`: H1 frozen-core learned-interface validator over
    the W3 Baseline D evidence.
- The live run to test remains `run.tassadar.executor.20260615`. The point of
  today's RC test is not to claim a new trained model prematurely; it is to
  prove that current Pylon + current Worker can exercise the now-correct
  Tassadar construction/replay logic against the actual run path and produce
  public-safe receipts.

## Today's launch objective

Launch the current RC to testers, then run a receipt-first Tassadar test window
with the proper logic now present:

1. Start from a clean `origin/main` checkout.
2. Verify the published RC surfaces:
   - npm `rc` dist-tag resolves to `@openagentsinc/pylon@1.0.0-rc.31`;
   - GitHub prerelease `pylon-v1.0.0-rc.31` exists and is prerelease-only;
   - signed OTA/feed status is checked before any auto-update claim;
   - a fresh install reports `pylon --version` / `pylon status --json` as
     `1.0.0-rc.31`.
3. Deploy or verify the `openagents.com` Worker from clean `main` before the
   live run test if the latest Tassadar code has not already reached
   production. Do not call a local `check:deploy` pass a production deploy.
4. Smoke the public run surfaces:
   - `/api/training/runs/run.tassadar.executor.20260615`;
   - `/api/public/tassadar-run-summary`;
   - `/tassadar`;
   - `/api/public/product-promises`.
5. Run the Pylon contributor path against the actual run:
   - `pylon training status --base-url https://openagents.com`;
   - `pylon training claim`;
   - execute the assigned digest-pinned workload;
   - pair it with a separate validator device for `exact_trace_replay`;
   - record only public-safe contribution, replay, verifier, and receipt refs.
6. Confirm the run is exercising the new logic by evidence, not vibe:
   - C-track: real compiled-program corpus / dense module / linked module refs
     are present where the window expects them.
   - V-track: exact replay and construction-settlement simulation gates produce
     deterministic public-safe refs.
   - E-track: any labor/curation/adversarial market hooks stay typed and
     operator-gated; no Forum keyword routing.
   - H-track: learned-interface gradient windows remain quarantine/canary/
     promotion candidates only; no canonical checkpoint mutation or gradient
     payout claim without the full gate.
7. Only after the above, post the Release Candidates forum thread as Raynor.

## Release Candidates forum thread prep - Raynor

> POSTED (owner-approved) — live at
> <https://openagents.com/forum/t/6cb2d165-7a65-495d-a21c-6a3a546ad759>, now
> rc.33 + consolidated. The draft below is the historical morning prep.

No post yet. Prepare this as a draft for the owner/posting step.

Suggested title:

```text
Pylon v1.0.0-rc.31 - Tassadar actual-run test window
```

Draft body shape:

```text
Raynor here. This is the rc.31 test window for the live Tassadar run.

What changed:
- Pylon rc.31 is the current RC install target.
- The Tassadar construction/verification roadmap is now on main: compiled
  program corpus, dense/loadable modules, linked module verification,
  construction settlement simulation, edge work directions, demand ranking,
  adversarial verification, and the frozen-core learned-interface quarantine
  gate.

What we need testers to try:
- Install/update to the RC.
- Confirm the local Pylon version.
- Check the live Tassadar run status.
- Claim and execute an assigned training window if admitted.
- Leave the node available for independent exact replay validation.
- Post only public-safe refs, version output, OS/platform, run/window refs,
  verifier verdict refs, and receipt refs.

Important caveats:
- Do not post wallet seeds, mnemonics, invoices, preimages, tokens, raw logs,
  raw traces, private prompts, provider material, or payout targets.
- Installing a node is not an earning claim.
- Accepted work and payouts require dereferenceable receipt evidence.
- Learned-interface gradient windows are candidate/quarantine-gated only; this
  RC does not claim public decentralized gradient training is live.
```

Before posting, replace the placeholders with verified live refs:

- exact RC install command/result;
- GitHub release URL;
- signed OTA/feed evidence if included;
- live `/api/training/runs/...` state;
- current known blockers;
- the first successful public-safe contribution/replay/receipt refs, if any.

## Carry-forward from June 17

### Gate 2 - real settlement / auto-payout dispatch (#5232)

Still the largest v1.0 release gate unless a newer receipt proves otherwise.
The default remains simulation/no-money movement. Real Bitcoin movement requires
the explicit owner gate, run allowlist, caps, payout target approval, idempotent
dispatch, reconciliation, and public-safe receipt refs. Do not arm or broaden
this gate as part of the docs/forum prep.

### Spark-native routing (#5225)

Verify final state before release copy. If complete, smoke that Spark-address
destinations use native Spark routing with zero Lightning fallback and honest
method labels. If incomplete, keep it as a non-blocking RC caveat unless the
specific RC test depends on internal Spark-to-Spark payout flow.

### Remaining wallet retests from June 17

Check whether #5208 Lightning Address send and #5194 helper-unavailable retests
have owner/contributor confirmation. Do not block the Tassadar logic RC unless
the failing path is part of the test install/run/receipt loop.

### Built-in hosted agent promise (#5063)

Still separate from today's Tassadar RC test. Green requires desktop executor
use of the live keyless grant route, a from-install go-online smoke, signed
recut evidence, and product-promise refs.

### Email strategy smoke

Still separate. Useful for operator notifications, not a blocker for the
Tassadar actual-run RC.

## Do not overclaim

- Do not say the public run has trained a new model until the run evidence
  proves model construction beyond the fixed executor workload.
- Do not say public gradient training is live. H1 is a quarantine/promotion
  gate around learned-interface candidate windows.
- Do not say a contributor earned Bitcoin until accepted-work and settlement
  receipts are dereferenceable.
- Do not present owner-operated nodes as independent contributor proof.
- Do not post as Raynor until the owner explicitly moves from roadmap prep to
  posting.

## Closeout for today

> Status: criteria MET (see END-OF-DAY UPDATE). One reconciliation fix
> (real-only settled total + run-level reconcile + enumerable settled feed)
> remains in flight; settlement gate stays armed as-is.

June 18 is done when:

- rc.31 (or a newer explicitly bumped RC, if code changes again) is verified
  across the intended install surfaces;
- the live Worker is verified to include the current Tassadar code;
- at least one actual-run Pylon path is exercised through contribution plus
  independent replay validation, or the blocker is captured with public-safe
  evidence;
- the Release Candidates thread is posted as Raynor only after owner approval;
- product promises and launch docs are updated to reflect only what receipts
  prove.

## Studying track — built + dogfood-proven; pull it forward into the active lanes (2026-06-18)

Reviewed the MSB-MVP / repo-studying work (the question: do we have enough now to pull the studying track in?). **Verdict: yes — it's past "plan," into shipped + dogfood-proven; the move now is to _activate_ it, not rebuild it.**

**What's shipped:**

- `autopilot.repo_study_packets.v1` — **yellow** (internal dogfood): the public StudyBench MVP shows source-grounded lift on OpenAgents refs (gate-review: `docs/promises/2026-06-17-repo-studying-product-promise-gate-review.md`). Customer-repo studying / trained repo-expert / marketplace / payout stay gated (correctly red/blocked).
- Shipped primitives: StudyBench contracts + the `#5284` repo-corpus manifest/entry/evidence-span; the MSB-MVP sequence is complete (`#5297`; no open MSB-MVP issues).
- EPIC #5313 **track-S (studying → Autopilot-coder)** issues #5314–#5320 are built + closed (substrate, knowledge-graph, verification, eval harness, Autopilot-coder consumption, paid-contribution, external-repo generalization).

**Decision — pull it forward as the near-term thrust, wired to the lanes already moving:**

1. **Feed the hygiene/refactoring lane (EPIC #5335).** Studied-knowledge is the prerequisite for _safe_ refactoring ("you can't safely refactor what you don't understand"). Point the studied-knowledge substrate at the lane so passes start from real codebase understanding, not grep-and-guess.
2. **Feed Autopilot-coder.** Track-S S5 wired studied knowledge into the coding-agent context; activate it on real coding work, dogfooding on this repo first.
3. **Advance the yellow promise toward its gated capabilities** (customer repo studying, marketplace, payout) only as the privacy / metering / pricing / payout gates clear — receipt-first, no premature green.

**Net:** studying, the hygiene lane (#5335), and Autopilot-coder are **one near-term program** — _agents that deeply know the codebase, paid to improve it, starting with ours._ The foundation is built; the active work is integration (studying → hygiene lane + Autopilot-coder) and advancing the gated promise — not a new build track.
