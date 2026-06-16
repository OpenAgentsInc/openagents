# JUNE 15 LAUNCH PLAN — The Tassadar Run

## ✅ JUNE 15 — WRAPUP (closed out 2026-06-16)

**June 15 shipped.** Remaining/open work has moved to **`JUNE16_ROADMAP.md`** in
this folder; everything below is the historical launch record.

What landed on the 15th:
- **Launched:** Autopilot 1.0 + Pylon v1.0 release candidates (signed/notarized,
  default-on signed auto-update, Pylon OTA verified end-to-end), the **Tassadar run**
  (`run.tassadar.executor.20260615`, active), Episode 237 + its essay/X-post, and the
  forum / Nostr / MoneyDevKit Bitcoin-tip rails.
- **Pivoted (owner directive)** from "push the training run" to **basic install
  stability + a built-in no-key agent**, on real Discord feedback ("I don't
  understand how to download," "I don't have an agent").
- **Built-in hosted-Gemini agent backend went live:** `GEMINI_API_KEY` set +
  verified on prod; the keyless quota-gated grant route is deployed. Promise
  `autopilot.builtin_compute_agent.v1` is **yellow** (green pending a from-install
  smoke).
- **AO/kWh** metric defined + instrumented to a modeled seed; **openapi/registry
  freshness** fixed and live; presence-auth contract documented.
- **Closed:** #5052–#5060, #5062–#5067, and short-term fixes #5056/#5057/#5058/#5059
  (projection-freshness lane, openapi drift, presence 401 contract, homepage download
  CTA, forum post-sort toggle, the Tassadar trace backend, interim Forum tips — first
  real tips settled to Orrery + Comunero).
- **Live promises:** `2026-06-15.11` — **17 green** · 28 yellow · 11 red · 17 planned.

**Held on purpose (carried to June 16):** the receipt-first green flips
(#5012/#5014/#5015/#5018) need a real non-owner Go/No-Go + the #5061 two-device
dry-run; plus open bug fixes #5075/#5076/#5077/#5066. See `JUNE16_ROADMAP.md`.

---

Date authored: 2026-06-14 (Sunday). Target: **Monday 2026-06-15**.

We are launching the **Tassadar run** tomorrow. This is the plan to make the core
experience real: a public Tassadar training run goes live, real (non-owner)
people install **Autopilot** (our Electrobun desktop app), get dispatched
**executor-trace work**, it's verified by **exact replay**, and they **earn real
Bitcoin** for it — with public receipts — while their accepted work accumulates
the verified-trace corpus that trains Tassadar.

This is not CS336. CS336 is shared plumbing (training-run routes, the settlement
ladder, the verification-class registry). The **run we are launching is
Tassadar** — the Percepta Executor Class model direction from Episode 236.

---

## ⚠️ Accountability — broken contributor instructions (2026-06-15)

**What I got wrong:** the contributor docs I shipped (live `AGENTS.md`,
`INSTALL.md`, and the community guide `docs/2026-06-15-help-flip-the-green-gates.md`)
told contributors to run **`pylon training closeout --window-ref …`** to submit
their work. That is the **operator/admin window-closeout** (`POST
/api/training/windows/{ref}/closeout`, `requireAdmin`), **not** the contributor's
executor-trace submission. Anyone who followed it got an unauthorized/no-op — so
they could **claim work but never complete it.** That is why the run reached
3 devices / 5 claims but **0 verified, 0 paid**: not contributor failure, my
instructions sent them down a dead end.

**Deeper cause:** there is currently **no contributor-callable route** to submit
an executor-trace closeout — every training write except `/api/training/leases/claim`
is `requireAdmin`. So a self-serve contributor literally cannot finish the loop
with rc.1/rc.2. The recruitment funnel worked; the product couldn't accept the work.

**The real fix is bigger than a verb.** On closer reading, the closeout evidence
requires a **`validatorDeviceRef` distinct from the worker's `pylonDeviceRef` plus
a `replayDigestRef`** — `exact_trace_replay` needs the worker's trace **replayed on
a separate validator device** and paired server-side. So a single contributor node
*cannot* self-complete; this is a **distributed worker→validator pairing
orchestration** (every training write except `leases/claim` is `requireAdmin`).
That is a genuine backend build + a contributor client flow, not a quick verb —
and **I will not rush a new verification/payment path onto the live launch run in
the same breath as the mistake above.** Done now: stop the active harm (correct
the misdirecting docs, deployed) + own it publicly (this note). The
worker/validator self-serve completion is scoped as a proper effort, not
same-turn. Until it ships, contributors can install + claim + run, but closeout
completion is operator-orchestrated.

**Scoped for the next RC** — full design audit:
`docs/tassadar/2026-06-15-executor-trace-contributor-completion-design.md`. Epic
**[#5051](https://github.com/OpenAgentsInc/openagents/issues/5051)** with children:
- **[#5052](https://github.com/OpenAgentsInc/openagents/issues/5052)** agent-gated
  worker-submit + validator-replay routes.
- **[#5053](https://github.com/OpenAgentsInc/openagents/issues/5053)** worker↔validator
  pairing orchestration (Artanis-first → decentralized).
- **[#5054](https://github.com/OpenAgentsInc/openagents/issues/5054)** Pylon client
  `submit-trace`/`validate` verbs + assignment worker default-on.
- **[#5055](https://github.com/OpenAgentsInc/openagents/issues/5055)** **interim
  earning** — Forum registration + tips so RC testers get paid day one while the
  executor-trace payout loop is built (sporadic owner-funded tips are fine).

These build into the next RC and get tested fully (real worker + real validator →
Verified → payout) before any green-flip announcement.

---

## Current status (updated 2026-06-15, evening)

**rc1 is built, announced, and live; the headline green flips are HELD pending
real independent contributors; one OTA-feed deploy is blocked and being fixed.**
Live registry: **`2026-06-15.1`**.

### Shipped + verified this pass
- **Pylon v1.0-rc.1** — signed `bun --compile` binaries (4 platforms), default-on
  self-updater (verify→atomic-replace→relaunch), runs headless; `--json` fix
  (#5041/#5042/#5043/#5047/#5038). **CLOSED.**
- **Autopilot v1.0-rc.1** — signed + **Apple-notarized** `.app` (Gatekeeper-accepted),
  bundles the headless node, default-on Electrobun OTA, immersive **pylon-network
  home screen** (sidebar removed, activity-driven) (#5046/#5027/#5040). **CLOSED.**
- **Provenance** — ed25519 release key + GCP-SM backup + fail-closed pinned verify;
  Apple Developer ID in keychain + GCP-backed (#5044/#5048). OTA epic #5039. **CLOSED.**
- **rc1 announced** — forum post as **Raynor** on the Release Candidates forum
  (`/forum/f/release-candidates`, seeded by migration 0187) + blog post
  (`/blog/pylon-autopilot-v1-rc1`). Both honest + RC-scoped (not claiming the run
  is live or that you earn now).
- **Homepage (live, deployed):** central pylon glow tied to live activity + the
  **bezier network graph** of online pylons + a **live stats overlay** (online /
  working / sats 24h / training contributors) with the slot-text number-roll and a
  "…" loading state. Behind the countdown; becomes the homepage at launch (#5050
  **CLOSED**).
- **Recruitment funnel:** fetchable **`/INSTALL.md`** (install/test), shrunk
  **`/AGENTS.md`** (128KB→74KB) + extracted **`/SURFACES.md`**, and a community
  **delegation guide** on GitHub (`docs/2026-06-15-help-flip-the-green-gates.md`).
- **Product promises** refreshed to `2026-06-15.1`: the RC promises now reflect the
  real 1.0.0-rc.1 signed/notarized builds (state stays **yellow** — still gated).
- Tester guides: `docs/autopilot-coder/2026-06-15-rc-tester-install-guide.md` (human),
  `2026-06-15-rc-agent-test-guide.md` (agent; SDK-free core, verified by a fresh
  no-SDK Codex run), and the visual-language runbook
  `2026-06-15-autopilot-home-network-visual-language.md`.

### Autoupdate — Pylon LIVE + verified end-to-end ✅ (Autopilot desktop OTA pending)
- **Pylon auto-update is genuinely working in production.** Verified end-to-end:
  a behind-version binary (1.0.0-rc.0) ran `pylon update` against the live
  `updates.openagents.com` feed → downloaded the artifact from GCS → verified the
  ed25519 signature against the pinned key (fail closed) → atomically self-replaced
  → reported 1.0.0-rc.1. On by default; opt-out `PYLON_DISABLE_AUTOUPDATE=1`.
- **Feed serving (fixed):** `updates.openagents.com` → `oa-updates` Cloud Run
  (domain-mapped). Three fixes were needed: (1) Dockerfile `COPY pylon-dist`;
  (2) serve assets streamed from disk, not loaded into the in-memory store at boot
  (was OOMing); (3) **Cloud Run caps responses at 32 MiB**, so the 60–97 MB
  binaries are served from a **public GCS bucket** (`gs://openagentsgemini-oa-updates`)
  via `OA_ASSET_BASE_URL` — the feed JSON stays on Cloud Run, downloads go to GCS.
  All 4 platform feeds return 200 with GCS artifactUrls. **No rc2 needed.**
- **Autopilot desktop OTA — LIVE ✅.** `oa-updates` now serves the Electrobun feed
  at `/desktop/<prefix>-update.json` + the `.app.tar.zst` (19MB, under the 32 MiB
  cap, streamed from `OA_DESKTOP_OTA_DIR`). Confirmed: `update.json` → 200
  (`1.0.0-rc.1`, hash `1g5hvbicxvr9r`), tarball → 200. The rc1 `.app` already has
  `release.baseUrl=updates.openagents.com/desktop` baked in (no re-notarize). A
  real desktop self-update *delta* can only be exercised once an rc2 desktop build
  is published (rc1↔rc1 reports up-to-date), but the feed is wired + serving the
  exact format Electrobun fetches.

**Autoupdate is now fully set up for both binaries** (Pylon verified self-updating
end-to-end; Autopilot desktop feed live).

### v1.0-rc.2 cut (2026-06-15) — published + OTA-verified
Both binaries bumped to **1.0.0-rc.2** and published; auto-update confirmed:
- **Pylon rc.2:** signed 4-platform feed live (`updates.openagents.com/pylon/rc/<platform>/feed.json`,
  binaries on GCS). **Real rc.1 → rc.2 self-update verified on prod** (behind binary
  downloaded + signature-verified + atomically self-replaced → 1.0.0-rc.2).
- **Autopilot rc.2:** signed + **Apple-notarized** `.app`; desktop OTA feed serves
  the rc.2 `update.json` + tarball + a **3KB rc.1→rc.2 BSDIFF delta patch** (all 200);
  notarized `.dmg` (staple-validated) on GCS for fresh downloads.
- Feed deploys are now small/fast (binaries on GCS, only `pylon-releases.json`
  ships to Cloud Build).

**Community download links:**
- Canonical install/test (rc.2): <https://openagents.com/INSTALL.md>
- Autopilot macOS (arm64, notarized): <https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/AutopilotDesktop-1.0.0-rc.2-macos-arm64.dmg>
- Autopilot GitHub release: <https://github.com/OpenAgentsInc/openagents/releases/tag/autopilot-desktop-v1.0.0-rc.2>
- Pylon: per-platform feed `https://updates.openagents.com/pylon/rc/<platform>/feed.json`
  (`releases[0].artifactUrl` is the direct GCS binary). Existing rc.1 nodes auto-update.

### Held on purpose — the headline green flips
The Tassadar run **manifest itself** says *"owner-operated nodes do not count as
independent contributor proof,"* and the real-gradient closeout requires **2
distinct contributor devices**. So we cannot honestly flip
`training.monday_decentralized_training_launch.v1` (#5014) or
`pylon.install_without_wallet_knowledge.v1` (#5015) green via Raynor/Artanis (both
org-operated). **Current run state (autonomously watched):** `active`, devices
**1/2**, `closeoutSatisfied: false`, `verified 0`, `settledSats 0` — one CLI claim
exists (lease `1e278589…`, claimed 16:06 UTC) but no closeout was submitted. We
flip green only against a real independent contributor's dereferenceable receipt
(#5012/#5018 follow). A background watch reports the moment verified+paid fires.

### Open issues
- **Held (owner-gated live event):** #5012, #5014, #5015, #5018.
- **Closed fidelity follow-up:** #5049 (Autopilot home — desktop now composites
  the exact homepage `pylonDiamonds` shader as the center element, forwards
  `activityIntensity` into `setActivity`, and renders the full public-stat
  overlay with scoped slot-text digit-roll).
- **Server fix in progress:** the oa-updates feed deploy (see autoupdate above).

**Done + merged + (where applicable) deployed:**
- **Worker run lane A–E** — run authority/manifest, executor-trace admission +
  claimable work, closeout→exact-replay verification, settlement-receipt ledger +
  projection, verified-trace corpus surface (#5006–#5010).
- **The install seam is closed.** Autopilot Desktop now **launches/adopts the
  local Pylon node** itself (no separate setup) — dev build (#5011) and the
  **packaged mac `.app` bundles the headless Pylon node** (#5027, unsigned), with
  restart-on-crash supervision + honest launching/online/failed status badge
  (#5025).
- **Registry reconciled** to "Autopilot is the install" (#5013) + participant
  **count rule** `qualifiedContributorCount` (#5016) + the labor/fanout
  **transition receipts** recorded (#5017). New planned promises:
  `marketplace.wasm_plugins.v1`, `training.public_gradient_windows.v1` (W5).
  **`pylon.agent_steerable_cli.v1` flipped GREEN** (live `2026-06-14.9`),
  receipt-first, after a live round-trip (`pylon sessions/approvals list --json`
  returned real data from a running node).
- **Desktop UI is launch-ready** — dev scaffolding stripped (#5020–#5024); the
  full test suite runs via a per-file runner (#5026).
- **Pylon is headless / CLI-only / agent-steerable** (epic #5033): TUI deleted,
  CLI parity + `pylon help --json` catalog, live AGENTS.md "three paths"
  (download app / build from source / install Pylon to steer), runtime
  bundle-able headless.

**Remaining = the live flip + owner action (intentionally not done here):**
- **#5014** live non-owner Go/No-Go run-through → flips
  `training.monday_decentralized_training_launch.v1` green (the launch event).
- **#5018** the announcement (post Go/No-Go).
- **#5015** self-serve install→earn green flip (a launch event).
- **#5027 remainder** — codesign + notarization (NEEDS-OWNER), OTA pinning, Linux.
- Artanis evolution-loop epic **#5028** (separately delegated).
- The `apps/web/src/scene/pylonCountdown.ts` deploy-gate (`check:architecture`)
  fix (owner's `/pylon` work; blocks the canonical `bun run deploy` until routed
  through an approved time primitive).

The dependency-ordered detail and the launch-copy boundary follow; the full issue
tracker is **§12**.

---

## Day-of forward worklist (2026-06-15, evening — the day is not over)

The RCs are live and announced; the launch transcript (Episode 237,
`docs/transcripts/237.md`) is committed. A launch-accuracy audit of every concrete
claim in 237 found **most are already true** (Tassadar `RESEARCH_PLAN.md`, the
Percepta audit, all three `docs/autopilot-coder` essays, the AGI reading group,
the AGENTS.md Tassadar section, the product-promise registry + §11 copy map).

### ⭐ PRIORITY PIVOT (2026-06-15 evening, owner directive)

Real Discord feedback at launch reordered the plan: **basic stability and a
normal-person install path now outrank the training run.** Two users:
- *OrwellWas*: "i don't understand how to download autopilot … pretty confusing"
  — and crucially **"i don't have an agent."** The whole "let your agent install
  it" story assumes you already have an agent; most normal users don't.
- *Trigger*: worried about agent **slop** and an architecture that "constantly
  evolves" — wants stability and to understand the stack before contributing.

**New top priorities (in order), training is DEPRIORITIZED below these:**

1. **One standard desktop installer + real "Download" links.** Treat RC → **full
   release**: a prominent download on `openagents.com` AND a GitHub Release with a
   plain "Download here" for the notarized Autopilot Desktop build (Mac first, then
   Linux). No agent required to obtain or run it. (#5059 added a homepage CTA → it
   must point at an actual downloadable installer, not just `INSTALL.md`.) — **#5062**
2. **Built-in, out-of-the-box agent that uses OUR compute.** So a user with **no
   agent and no API key** installs one installer, clicks "go online," and has a
   working agent. "Our compute" is NOT limited to the device — it can be the
   device's own compute **and/or our free hosted cloud model set (a Gemini set we
   offer free to eligible users)**, so even a no-spare-compute machine (or a user
   with no agent at all) gets a working agent. It can also route around / fix rough
   edges on untested systems. This is the answer to "i don't have an agent." — **#5063**
3. **Basic install/runtime stability** — make the out-of-box experience solid on
   untested systems; fix the reported rough edges. Surface-driven (below). — **#5064**
4. **Autopilot built-in "surface issues, don't ship slop" forum flow.** Per owner:
   community agents should **surface gaps** (product-promise vs reality,
   broken/missing/insufficient) — *not* auto-ship code yet. Build the Orrery-style
   flow into Autopilot this week: read forum/promise posts, diff against the promise
   ledger, surface what's still broken vs already-fixed. (Our own agents keep fixing;
   community agents surface.) Helpful surfacers get tipped. — **#5065**

**Deprioritized (resume after stability):** the executor-trace training lane
(#5051 epic — backend is built + inert, see below), AO/kWh (#5060), and the
training green-flips (#5012/#5014/#5015 stay receipt-first but are no longer the
headline). The training backend is DONE and safely inert, so pausing it costs
nothing; the #5061 live dry-run waits until the install/agent path is solid.

### Deprioritized lane — executor-trace contributor completion (post-stability)

This is **the** launch-blocker: a real contributor can claim work but cannot
finish it (no contributor-callable submit route; closeout needs a *distinct*
validator device for `exact_trace_replay`). The full design is
**`docs/tassadar/2026-06-15-executor-trace-contributor-completion-design.md`** —
target flow: `admit → claim → WORKER submits trace (new agent-gated route) →
system pairs a DISTINCT validator → VALIDATOR replays + submits verdict → digests
compared → Verified → operator-funded payout + receipt`. Build it into the next RC
and **test it fully** (real worker + real validator → Verified → real small
payout) **before** any green flip. Epic + children (all OPEN):
- **#5051** epic — self-serve worker→validator executor-trace completion.
- **#5052** — agent-gated worker-submit + validator-replay routes.
- **#5053** — worker↔validator pairing orchestration (Artanis-first → decentralized).
- **#5054** — Pylon client `submit-trace`/`validate` verbs + assignment worker default-on.
- **#5055** — interim earning: Forum registration + tips for RC testers (ship now,
  so testers get paid day one while the executor payout loop is built).

When a genuine non-owner completes that loop with a dereferenceable receipt, it
flips `training.monday_decentralized_training_launch.v1` (#5014) +
`pylon.install_without_wallet_knowledge.v1` (#5015) — receipt-first.

### Episode 237 launch-truth follow-ups (from this pass)

- ✅ **Accepted Outcomes Per Kilowatt-Hour (AO/kWh)** — 237 names it "the metric
  we're defining and measuring primarily," but it was documented nowhere in the
  initial audit pass. It is now **defined and yellow-instrumented**:
  `docs/metrics/2026-06-15-accepted-outcomes-per-kwh.md` + promise
  `metrics.accepted_outcomes_per_kwh.v1` (source registry **`2026-06-15.5`**)
  publish one receipt-backed, explicitly modeled seed datapoint. It stays below
  green until measured/repeatable kWh telemetry and repeat datapoints exist.
- ⚠️ **Homepage download CTA** — 237 says *"you'll go to openagents.com and you'll
  see Autopilot: Download to get started."* The root `/` currently renders the
  Pylon network viz/stats (per owner's explicit earlier direction) with **no
  Autopilot download CTA**. `NEEDS-OWNER:` confirm adding a prominent "Download
  Autopilot" CTA **alongside** the Pylon viz (homepage copy is owner-gated per
  CLAUDE.md, so not changed unilaterally).
- ⚠️ **Fable analysis doc** — 237 says *"you can read Fable's analysis"* of the
  two AGI papers, but no such doc exists under `docs/agi/`
  (`docs/agi/openagents-analysis.md` makes the "same paper at two altitudes"
  argument but is not attributed to Fable). Either write the attributed Fable
  analysis or soften the transcript reference. (Minor.)
- §11.2 registry follow-ups status: WASM-plugin promise `marketplace.wasm_plugins.v1`
  ✅ exists (`planned`); install-positioning reconciled to "Autopilot is the
  install" (#5013); the wallet+Nostr-per-node claim is already covered by the
  green `agents.cursor_forum_wallet.v1` + `agents.nostr_fallback_coordination.v1`.

### Forum audit signals — Orrery + community (read 2026-06-15)

Traversed the Product Promises forum. **Orrery** (an independent external
auditor-agent; receipts hash-pre-committed to Nostr + Bitcoin) is a **live
non-owner contributor on the run right now** and has filed the strongest signal
we have. Folding it in:

- **Independent launch-day verification (Orrery, external/non-team node).** Matches
  our honest state exactly: provenance/install verifies (rc.2 sha + ed25519 pinned
  key; auto-update carried rc.1→rc.2), `register` returns `registered:true` + a
  pylonRef, token-authed presence reads `online`, and **claim grants in seconds**
  (held `training.window.tassadar.executor.20260615.w1` repeatedly with claim
  receipts). **Gated, as we say:** closeout is not self-serve (needs a separate
  validator device), settlement is OpenAgents-controlled (`settlementState=pending`).
  This is real external corroboration of "claim-live, closeout+settlement gated."
- **Orrery volunteered to be the FIRST external validator** when the
  worker→validator pairing (#5051/#5053) opens. Action: when #5053 lands, point the
  first self-serve pairing at Orrery's node and post the first externally-settled
  trace receipt — that is the #5014 green-flip evidence.
- **Presence bug (new, from Orrery + #4735 family):** a node's *own* heartbeat
  signed with its **unlinked Nostr key 401s**; only the token path authenticates.
  Self-signed presence should authenticate. `NEEDS-ISSUE` (or fold into the
  presence lane) — agents can't prove liveness without the token path.
- **The "writes-succeed / reads-never-learn" invariant (Raynor named it, #4744;
  Orrery wrote it up — 8 instances in ~24h).** One defect wearing many coats: a
  state-changing write commits to a source-of-truth table but a **public read
  projection never rebuilds** (or never existed). Instances: agent-profile
  projection frozen at registration (owner-claim approved but still
  `owner_claim_required`); X-verification proof verified but `verificationState`
  stuck; **`openapi.json` frozen at `info.version 2026-06-05`** while green routes
  shipped; pylon-stats `recentPylons` online from the 24h pool contradicting the
  5-min counters (#4735); **credited-rung tips invisible** (sats in `tipStats`/
  `totalPaidSats` but no `receiptRef`/tip-earnings row); stale
  `/api/public/artanis/report` tick projection. This is a **cross-cutting
  projection-freshness lane** — every green that asserts a read surface needs a
  rebuild-on-write guarantee (the registry already uses `live_at_read`; extend that
  discipline to agent profiles, openapi, pylon-stats, tip ledger, artanis report).
- **Green-promise audit (Orrery, registry `2026-06-10.23`): 8/10 verified, 2 gaps.**
  Gap 1: **stale `.well-known` manifest integrity hash** (high — agent-trust
  surface). Gap 2: **`openapi.json` `info.version` frozen** (same invariant as
  above). Both are projection-staleness; fix under the lane above.
- **Fable hardening (already shipped, confirmed by Orrery):** the hardcoded
  `claude-fable-5` string is gone; `apps/pylon/src/tas/model-provider.ts` (2026-06-13)
  added a `fallbackOrder` + `preferred_model_unavailable_using_fallback` path, so the
  `claude_agent` lane is model-agnostic. Fable identity marked deprecated/offline on
  the forum (Anthropic disabled the model). No further action beyond the public mark.

### Open work queue (→ GitHub issues, as of 2026-06-15 evening) — SUPERSEDED

> **Historical snapshot.** Live open work is now tracked in `JUNE16_ROADMAP.md`.
> Most items below closed on the 15th; the remainder carried over to June 16.

Filed from the launch-truth audit + the forum traversal. Live-verified before
filing (e.g. `openapi.json` still serves `info.version 2026-06-11`; product-promise
registry source is `2026-06-15.2` but the live endpoint still serves `.1`).

**⭐ NEW TOP PRIORITY — stability pivot (owner directive; see PRIORITY PIVOT above):**
- ✅ **#5062** one standard desktop installer + real "Download" links (RC → full release) —
  DONE: `/` Download Autopilot CTA points at the notarized macOS arm64 DMG,
  README + install docs carry the same "Download here" URL, and the GitHub
  release tag is the canonical repo-side download target. Intel macOS/Linux
  desktop installers remain owner-gated until signed/notarized or packaged.
- ✅ **#5063** built-in out-of-the-box agent using OpenAgents compute (no user API key) —
  SOURCE DONE/yellow: Desktop now has first-screen **Go online** + Agent pane,
  Bun-owned built-in-agent readiness/start RPC, managed scratch workspace,
  cloud-gcp/cloud-shc session lane, no user provider key in the webview, and
  bounded defaults (3 sessions/day, 600s/session). Registry
  `autopilot.builtin_compute_agent.v1` moved to yellow in `2026-06-15.4`.
  Remaining green proof: signed/notarized desktop recut with packaged hosted
  compute credentials/entitlement + public from-install smoke.
- ✅ **#5064** basic install + runtime stability for normal users (out-of-box,
  untested systems) — SOURCE DONE/yellow: Desktop now has a Bun-owned
  `installReadiness` projection, a first-screen health line, **Settings →
  First-run Health** rows, explicit blocker refs, startup/status-change refresh,
  public install-doc guidance, and an AGENTS.md entry-path trim so normal testers
  start at `INSTALL.md`. Remaining public proof: signed/notarized desktop recut
  containing the pane, then a clean-machine from-install smoke on a non-owner
  consumer setup.
- ✅ **#5065** Autopilot built-in "surface issues, don't ship slop" forum flow
  (Orrery-style) — SOURCE DONE/yellow: Agent pane now has **Surface Promise
  Gap**, Bun-owned live ledger + Product Promises Forum topic lookup, exact
  promise-id report drafting, optional registered-agent Forum posting, and docs
  that keep the posture "surface only; do not ship code." Remaining public
  proof: signed/notarized desktop recut with a configured registered-agent token
  plus a public Product Promises Forum post created from the app.

**Tassadar traces (executor-trace completion) — DEPRIORITIZED (backend COMPLETE + inert):**
- **#5051** epic (OPEN — holds until a real verified+paid run) →
  - ✅ **#5052** agent-gated submit/replay routes — **DONE** (`602e83e0b`):
    `/trace-submission` (requireAgent + lease-ownership) + `/replay-verdict` (requireAgent +
    device-distinctness) + migration `0188` + 12 tests.
  - ✅ **#5053** worker↔validator pairing orchestration (Artanis-first) — **DONE**
    (`ab80ca1dd`): oldest-pending → distinct validator → builds the `exact_trace_replay`
    challenge; 25 tests. **OFF by default** behind `TASSADAR_TRACE_PAIRING`; no validator
    candidate resolver yet, so nothing pairs until the dry-run supplies a distinct device.
  - ✅ **#5054** Pylon `submit-trace` + `validate` client verbs — **DONE** (`20a354b32`):
    agent-token (not admin), `--json`, 14 tests. **Assignment worker stays opt-in**
    (`PYLON_ASSIGNMENT_WORKER`), NOT default-on — no node behavior change.
  - ✅ **#5055** interim Forum-tip earning — **DONE/closed**: owner funded the payer wallet
    (BOLT11 10k paid; ~8.8k sats live), and the first real interim tip settled —
    **1,000 sats → Orrery** (receipt `receipt.forum.direct_tip.dda29604…`). Path documented
    in the community guide; more tips flow to good-faith audit/report posts as budget grows.
- **#5061** first external-validator dry-run with **Orrery** (live non-owner contributor,
  volunteered) → flip `TASSADAR_TRACE_PAIRING` on, run a real 2-device replay → first
  externally-settled trace receipt → flips **#5014**.

> **Where the headline lane stands (2026-06-15).** The entire executor-trace
> contributor-completion **backend is built, tested, merged, and inert**: a contributor
> can now submit a trace, a distinct validator can replay + submit a verdict, the server
> pairs them and builds the exact-replay challenge, and the Pylon client has the verbs —
> all behind flags/opt-in so **nothing in production behavior changed**. What remains is
> deliberately **not autonomously closable**: (1) the **#5061 live 2-device dry-run**
> (enable `TASSADAR_TRACE_PAIRING`, run a real worker+validator with Orrery) — the only
> thing that proves the loop and flips #5014/#5015 green, receipt-first; (2) **owner-funded
> spend** for #5055 tips (BOLT12 offer delivered); (3) **AO/kWh measured telemetry +
> repeat datapoints** after the modeled seed (#5060); (4) the **owner-run green-flips**
> (#5012/#5014/#5015/#5018).

**Short-term fixes (new, from the forum audit) — all DONE/closed:**
- **#5056** projection-freshness invariant umbrella (public reads rebuild on write;
  extend `live_at_read`). `#4744`/`#4735` already closed under it; `#5057` now done.
  Remaining instances to sweep: credited-rung tip visibility, artanis-report tick.
  - ✅ **#5057** `openapi.json` lag — **DONE/closed** (`9a16b7bc9`): `info.version` now
    derived from `PublicProductPromisesVersion` + a guard test prevents future drift.
- ✅ **#5058** Pylon self-signed heartbeat 401 — **DONE/closed** (`9edbd9b0a`): typed
  `pylon_api_presence_requires_agent_token` 401 + `WWW-Authenticate: Bearer` + reason;
  token-only presence contract documented in AGENTS.md (real NIP-98 self-signed presence
  left as a deliberate future enhancement). No authority broadened.
- ✅ **#5059** homepage "Download Autopilot" CTA — **DONE/closed** (`6de4140fe`): functional
  CTA → `INSTALL.md` on the real `/` route (`pylon.ts`, not `home.ts`), viz preserved,
  minimal copy; 410/410 web tests pass.
- ✅ **#5067** Forum topic post order toggle — **DONE/closed**: topic reads keep
  oldest-first as the default, add `sortDir=asc|desc` plus phpBB-compatible
  `sd=a|d`, and the browser topic view exposes **Oldest first / Newest first**
  links without changing the topic-list recency contract.
- ✅ **#5049** Autopilot fullscreen pylon-network home fidelity — **DONE/closed**:
  desktop composites the shared homepage `pylonDiamonds` shader over the
  three-effect network graph, activity drives `lightPulse` through the existing
  scene projection, every listed public stat is shown, and overlay numbers use
  the scoped slot-text digit-roll structure.
- ✅ **#5060** Accepted Outcomes per kWh instrumentation — **DONE/closed**:
  `/api/public/metrics/accepted-outcomes-per-kwh` publishes the frozen AO/kWh
  definition, a receipt-backed accepted-outcome counter, and one clearly
  labeled **modeled** seed datapoint from the first settled labor job (#4777).
  Registry source `2026-06-15.5` moves the promise to `yellow`; measured energy
  telemetry and repeat datapoints remain blockers before green.

**Held green-flips (owner-gated live event):** **#5012** epic · **#5014** launch flip ·
**#5015** self-serve install→earn · **#5018** announce. These flip only against a real
non-owner dereferenceable receipt (receipt-first).

---

## 0. The node software is Autopilot (Electrobun desktop), not a standalone Pylon CLI

**Clarification (2026-06-14):** the contributor-facing install is now **Autopilot
Desktop** — `@openagentsinc/autopilot-desktop`, an Electrobun + Foldkit app — not
a standalone "Pylon v0.3" CLI/TUI. Audited state of `apps/autopilot-desktop`:

- It is the **cockpit over the local node**: the Bun main process owns local node
  control over loopback (via the shared `autopilot-control-protocol` bridge /
  control token), and the webview renders the operator UI. The **Pylon runtime
  still exists as the local node Autopilot drives** — "Pylon v0.3" is superseded
  as a separate user-facing install, not deleted as a runtime.
- It already drives the **full training-contribution loop** through local node
  intents: request bootstrap, claim training lease, plan/activate/reconcile a
  training window, build the evidence packet, admit evidence, and queue training
  launch + closeout (`ClaimTrainingLease`, `ActivateTrainingWindow`,
  `ReconcileTrainingWindow`, `BuildTrainingEvidencePacket`, `QueueTrainingLaunch`,
  `QueueTrainingCloseout`), plus the lane selector (auto|local|cloud-gcp|cloud-shc)
  and session spawn. It has a Training cockpit pane (`oa-training-run`, gated by
  `verify:autopilot-desktop:training`).
- It ships as a **signed + notarized macOS `.app`** with an OTA update feed
  (`updates.openagents.com/desktop/stable/feed.json`, BSDIFF deltas). Pricing TBD.

**The seam is now closed (2026-06-15).** Autopilot Desktop **launches/adopts the
local Pylon node itself**: it adopts an already-running node or spawns the local
Pylon runtime into a managed `.pylon-local` home, with restart-on-crash
supervision and honest launching/online/failed status (#5011 dev build, #5025
status badge). The **packaged mac `.app` bundles the headless Pylon node**
(`Contents/Resources/app/pylon-node/index.js`) and launches it the same way
(#5027, unsigned). So "install Autopilot and contribute" is one step. Where this
plan says "the node" it means the local Pylon runtime Autopilot controls; "install"
means installing **Autopilot Desktop**.

> Follow-up DONE (#5013, deployed): the registry was reconciled to the
> Autopilot-is-the-install positioning (`pylon.v03_release_candidate.v1`,
> `pylon.release_tomorrow.v1`, `pylon.install_without_wallet_knowledge.v1`
> safeCopy now name Autopilot Desktop as the install surface with Pylon as the
> node it drives). Separately, Pylon itself is now headless/CLI-only and
> fully agent-steerable (`pylon.agent_steerable_cli.v1` green, registry
> `2026-06-14.9`) — an agent can install Pylon directly and steer it.

> From Episode 236 (`docs/transcripts/236.md`): "Monday we're launching the
> largest decentralized training run… you're just going to install our node
> software and you're going to get paid Bitcoin to contribute to a training run…
> a very fancy new architecture… Percepta Executor Class model… CPU computation
> transform… adding support for that to Pylon version 0.3 paired with the Bitcoin
> payments… one piece of software that's going to earn you Bitcoin in multiple
> different ways, including helping train this very experimental but we think very
> powerful new kind of model which we're calling **Tassadar**. We'll launch that
> Monday."

> Grounding: built from `docs/transcripts/236.md`, the Tassadar lane
> (`docs/tassadar/README.md`, `compute.tassadar_executor_poc.v1` green,
> `artanis.tassadar_evolution_loop.v1` yellow), the Episode 236 gap audit
> (`docs/2026-06-12-episode-236-training-launch-gap-audit.md`), the live surfaces
> below, and the promise registry (`/api/public/product-promises`, `2026-06-14.9`).

---

## 1. What the Tassadar run actually is

Tassadar is the **compiled exact-executor lane**: digest-pinned exact-program
workloads dispatched to contributor machines, run, and **verified by exact trace
replay** on a separate device (a verdict is just re-execution + a digest
comparison — the cheapest, strongest verification that exists). Accepted work is
paid in sats and its verified traces accumulate the corpus that trains the
Tassadar / Percepta Executor Class model.

Why Tassadar is the **right** run to launch into a public, paid, many-contributor
event — these are advantages, not hedges:

- **Verification is trivial and strong.** Exact replay means even the weakest
  device in the funnel can both contribute *and* validate. No gradient quorum, no
  statistical grading — replay + digest match.
- **It already settled real money.** `compute.tassadar_executor_poc.v1` is green:
  a real Pylon ran a digest-pinned workload, the worker re-executed it as a
  separate validator (Verified, plus a Rejected on a tampered digest), and one
  paid closeout settled over real Lightning.
- **The dispatch/verify loop is already live.** The Artanis evolution loop
  (`/api/public/artanis/admin-ticks`) is in production, dispatching executor-trace
  workloads and publishing per-tick receipts. Today it's mostly no-spend and
  dispatch-fails for lack of eligible online devices — which is exactly what a
  public launch fixes by bringing the fleet online.

The job for tomorrow is to turn that bounded, owner-driven loop into a **public
Tassadar run a stranger can join self-serve and earn from.**

---

## 2. Definition of Done (the scoreboard)

The Tassadar run launch is real — not just copy — when all of these are publicly
verifiable:

1. **The Tassadar run is RUNNING.** ✅ **met (#5006)** — the public run page
   reports `state: active` (not `planned`), with the stable `trainingRunRef`
   `run.tassadar.executor.20260615`, a published manifest, and a fresh
   `generatedAt` / `live_at_read` staleness contract.
2. **A non-owner joined self-serve.** Someone who is not the owner installed
   **Autopilot Desktop**, brought a node online, declared the executor capability,
   was admitted to the Tassadar run, and was dispatched real executor-trace work —
   without operator hand-staging.
3. **Their work was verified by exact replay.** A public
   `training.verification.challenge.<id>` `Verified` (exact_trace_replay) verdict
   references their submission.
4. **They earned real sats, and can see it.** Accepted executor-trace work
   settled a real Lightning payout to the contributor's wallet, with a
   dereferenceable public receipt, AND the public run / leaderboard shows
   `settledPayoutSats > 0`. **Seam met (#5009):** the settlement-receipt route +
   projection read are live — `settledPayoutSats` / `providerConfirmedSettledPayoutSats`
   now reflect run-linked settled receipts (no longer hardcoded `0`). Goes
   non-zero the moment the first real settlement lands — the launch event.
5. **The corpus grew.** The accepted verified traces are recorded toward the
   Tassadar training corpus (the evolution loop's accumulation), visible on the
   run. **Seam met (#5010, W2):** the run summary `corpus` block projects the
   count of accepted, replay-verified `exact_trace_replay` closed ticks with
   public-safe refs, rebuilding on verdict transitions. Goes non-zero on the
   first Verified `exact_trace_replay` trace — the launch event.

Hitting #2 + #4 for one stranger is the moment "install Autopilot, help train
Tassadar, get paid Bitcoin" becomes a fact instead of a promise.

---

## 3. Current reality (honest snapshot, 2026-06-14)

Green / proven:

- **`compute.tassadar_executor_poc.v1` (green):** one bounded executor-trace
  workload, dispatched to a real Pylon, replay-verified on a separate device,
  one paid Lightning closeout with balance receipts on both sides.
- **`artanis.tassadar_evolution_loop.v1` (yellow):** the automated
  dispatch → replay-verify → accumulate loop is deployed; the public tick monitor
  is live; it dispatched and closed out no-spend executor work autonomously once.
- **Verification:** `exact_trace_replay` is a live, exercised verification class;
  the **reliable-tips ladder** settles real sats and never drops them (green).
- The **node runtime** (Pylon v0.3-rc2) runs the agent surface from the device
  (green `pylon.v03_agent_economy.v1`) and is what **Autopilot Desktop** drives
  over loopback; operator-staged install-to-bitcoin settled a real 21-sat payout
  (yellow).

Shipped since this plan was written:

- ✅ **Step A — run authority + manifest is live** (#5006, deployed). The public
  Tassadar run `run.tassadar.executor.20260615` reports `state: active` (not
  `planned`) with its launch manifest (`workloadFamily: executor-trace`,
  `verifierPolicy: exact_trace_replay`, `paymentMode: operator_approved_small_sats`,
  spend cap, status URL, abort rule), a `live_at_read` staleness contract, and
  typed blockers; a run-level state-transition route now moves runs off `planned`
  without D1 patches. No promise flipped green.
- ✅ **Step B — executor-trace admission + claimable work is live** (#5007,
  deployed). `POST /api/training/runs/{ref}/admit` makes a **reasoned admission
  decision** (receipted executor capability + owner-operated check + the #4852
  host-RAM device gate, every branch with a stated measured reason), and the run
  now carries a claimable, **digest-pinned executor-trace work window**
  (`activeWindowCount: 1`); verification is already run-aware (`exact_trace_replay`
  challenge carries `trainingRunRef` + `windowRef`). The **live non-owner
  admit→claim→verify run-through is the launch event** (§6), not faked on the
  production run; `assignedContributorCount` stays 0 until a real contributor
  claims. No-spend only; no promise flipped green.
- ✅ **Step C — closeout → run-tied verification is live** (#5008, deployed).
  `POST /api/training/runs/{ref}/executor-trace-closeout` takes a contributor's
  executor-trace closeout and creates a **run+window-tied `exact_trace_replay`
  verification challenge** (the builder was previously unwired), enforcing the
  distinct-validator-device rule. On validator replay a `Verified` verdict
  surfaces in the run's `verifiedWorkCount` (a tampered digest → `rejectedWorkCount`).
  The live closeout→replay run-through is the launch event (§6);
  `verifiedWorkCount` stays 0 until then. No-spend only; no promise flipped green.

Resolved since (2026-06-15):

- ✅ **Install seam closed** — Autopilot Desktop launches/adopts the node itself
  (dev #5011 + packaged mac `.app` bundles the headless node #5027); status badge
  #5025. The install is now one step.
- ✅ **Settlement projection joined** (#5009) — `settledPayoutSats` /
  `providerConfirmedSettledPayoutSats` read real run-linked settled receipts (no
  longer hardcoded `0`); it goes non-zero on the first real settlement.
- ✅ **Pylon is headless/CLI-only/agent-steerable** (#5033 epic) — an agent can
  install Pylon and steer it from the CLI (`pylon.agent_steerable_cli.v1` green).

Red / the gap (what genuinely remains = the live launch event):

- **No self-serve contributor *run-through yet*** — the machinery is all live
  (admit→claim→verify→settle), but the fleet is thin and the loop dispatch-fails
  without eligible online devices; the launch's job is to bring devices online.
  The **first non-owner stranger carried end to end is the launch event** (#5014,
  §6) — not a code gap.
- **Payout leg — programmatic via the treasury wallet** (not blocked): the
  `/treasury` MDK wallet makes payouts and Artanis dispatches under bounded spend
  authority + the run spend cap; the #5009 settlement-receipt route records them.
- **`models.tassadar_percepta_executor.v1` stays red** — we launch the **run that
  trains it / earns contributors sats for contributing**, not a trained model.
  "Help train Tassadar" = contribute verified work. Do not claim Tassadar is
  trained or CPU-equivalent.

---

## 4. Critical path (dependency-ordered) — drive all the way to a paid stranger

Each step: **owner lane** · **done-when** · **promise it moves**. A→D is the
spine; E makes it honest; F→G make it usable and public.

### A. Tassadar run authority + manifest · worker-api + product — ✅ DONE (#5006)
- Shipped + deployed + verified live: the **Tassadar run**
  `run.tassadar.executor.20260615` exists with a **run-level state-transition
  route** (`POST /api/training/runs/{ref}/(activate|seal|reconcile)`) that moves
  runs `planned → active → sealed → reconciled` without D1 patches, a public-safe
  **launch manifest** (runRef, promiseRef, state, admission rule, workload family
  `executor-trace`, verifier policy `exact_trace_replay`, payment mode, settlement
  state, spend cap, status URL, abort rule, blockers), and a run projection
  carrying `generatedAt` + a `live_at_read` staleness contract + typed blockers
  (including the planned-with-reconciled-windows caveat).
- **Done (verified):** `GET /api/training/runs/run.tassadar.executor.20260615`
  returns `state: active` + manifest + staleness contract.
- **Moves:** `training.monday_decentralized_training_launch.v1` stays **red** until
  D lands (no promise flipped green).
- **Next:** Step B below — make a non-owner contributor able to join and be
  dispatched real executor-trace work from this run.

### B. Self-serve executor-capability admission + claimable work · ✅ machinery DONE (#5007)
- Shipped + deployed: `POST /api/training/runs/{ref}/admit` makes a reasoned
  admit/exclude decision (receipted **executor-trace capability** + owner-operated
  check + the reasoned device-admission gates #4852, every branch with a stated
  measured reason), and the run carries a claimable **digest-pinned executor-trace
  work window** (`activeWindowCount: 1`) that an admitted contributor claims via
  `POST /api/training/leases/claim`.
- **Done when (machinery, met):** the run gates contributors with measured
  reasons and exposes claimable executor-trace work. **The live non-owner
  admit→claim run-through is the launch event** (§6) — `assignedContributorCount`
  stays 0 until a real contributor claims; not faked on the production run.
- **Next:** Step C below — wire the contributor's executor-trace closeout
  submission to the run-tied `exact_trace_replay` verification so verified work
  surfaces in the run's `verifiedWorkCount`.

### C. Exact-replay verification · ✅ machinery DONE (#5008)
- Shipped + deployed: `POST /api/training/runs/{ref}/executor-trace-closeout`
  turns a contributor's executor-trace closeout into a run+window-tied
  `exact_trace_replay` challenge (the builder was previously unwired), enforcing
  the distinct-validator-device rule. The submission is re-executed on a separate
  validator device; a `Verified` verdict surfaces in the run's `verifiedWorkCount`
  and a tampered digest in `rejectedWorkCount`.
- **Done when (machinery, met):** a contributor closeout creates a run-tied
  `exact_trace_replay` challenge. **The live closeout→`Verified` run-through is
  the launch event** (§6) — `verifiedWorkCount` stays 0 until a real submission
  is replayed; not faked.
- **Next:** Step D below — pay the contributor real sats for accepted work.

### D. Payout + settlement to the contributor · ✅ ledger + projection seam DONE (#5009)
**The "earn Bitcoin" leg.**
- ✅ **Ledger + projection seam is live** (#5009, deployed). Accepted
  (`Verified`) `exact_trace_replay` work →
  `POST /api/training/runs/{runRef}/settlement-receipt` records the
  operator-approved treasury payout chain (intent → attempt → reconciliation →
  `settlement_recorded` receipt) under the run manifest `spendCapSats` + a hard
  per-payout cap, and links the **public settlement receipt** onto the run.
  Payout is **programmatic** via the `/treasury` MDK wallet (Artanis dispatches
  under bounded spend authority). `providerConfirmedSettledPayoutSats` and the A1
  `settledPayoutSats` now read the linked settled receipts (no longer hardcoded
  `0`); `paid ≠ accepted ≠ credited ≠ settled` stays strict.
- **Remaining (the launch event, §6):** a real **non-owner stranger** holding a
  real settled Lightning payout from the Tassadar run with a dereferenceable
  public receipt — the live send is performed by the treasury container/Artanis
  and recorded by the #5009 route. This is the Go/No-Go event, not a separate
  build step.

### E. Settlement + corpus projection consistency · ✅ read-paths DONE (#5009, #5010)
- ✅ **`settledPayoutSats` is joined into the projection** (#5009): the run page
  and A1 leaderboard read real provider-confirmed settled receipts linked to the
  run, and a reconciled/accepted trace cannot leave the run claiming `planned`
  (existing `TrainingRunPlannedWithReconciledWindowsBlocker`); settled receipts
  surface only when dereferenceable + redacted.
- ✅ **Verified-trace corpus surface is live** (#5010, W2, deployed). The run
  summary `corpus` block projects the count of accepted, replay-verified
  `exact_trace_replay` **closed ticks** (RESEARCH_PLAN §5 W2.5: tick closure as
  acceptance — intent + execution + state delta + evaluation; "closed ticks _are_
  training records"), with public-safe trace/verdict refs and a live-at-read
  staleness contract that **rebuilds on validation transitions, not registration**
  (RESEARCH_PLAN §6.3 + Standing Order 5; case law #4744–#4747). It stays bounded
  evidence — no Tassadar exactness / model-capability claim — and is **evidence
  toward** `artanis.tassadar_evolution_loop.v1`'s
  `tassadar_distillation_dataset_receipt_missing` blocker, not the dataset
  receipt itself. Count goes non-zero on the first Verified `exact_trace_replay`
  trace (today `0`).
- **Done when:** the public run shows a non-zero settled total equal to reality
  **and** a growing accepted verified-`exact_trace_replay` corpus count — both
  read-paths are now wired (#5009 settled, #5010 corpus); they populate as real
  accepted work lands at launch.

### F. Autopilot Desktop install path contributors can use · ✅ code DONE (#5011, #5025, #5027)
- ✅ **The §0 seam is closed:** Autopilot Desktop adopts a running node or
  **launches the local Pylon runtime itself** into a managed `.pylon-local` home
  (dev #5011), with restart-on-crash supervision + honest launching/online/failed
  status badge (#5025). The **packaged mac `.app` bundles the headless Pylon node**
  (`Contents/Resources/app/pylon-node/index.js`) and launches it the same way
  (#5027, `bun run build:canary`, unsigned). No separate Pylon-node setup step.
- The executor-trace lane runs through the desktop's training cockpit; unit-level
  lifecycle covered (adopt/spawn/restart/stop, packaged-entry resolution).
- **Remaining (owner/live):** codesign + notarization so a *downloaded* build
  passes Gatekeeper (NEEDS-OWNER), OTA pinning (announced==admitted), the Linux
  path, and a clean-machine first-run→node-online→admitted smoke — all under
  #5027. The build the announcement links must be the one the run admits and pays.

### G. Announce · product/forum
- After the Go/No-Go gate (§6), with the manifest/status URL, the live registry
  version, and exact promise IDs. See **§7 Copy gate**.

---

## 5. Scale ambition vs the "largest" claim

Bring the fleet online and admit as many contributors as the run can verify and
pay — that ambition is the whole point, and exact-replay work scales to weak
devices better than gradient work does. **But** the "largest decentralized
training run / beat 200 contributors" *claim* needs the participant-count rule
first (gap audit §4): count only **admitted contributors with accepted useful
work and public-safe receipt refs**, never raw registrations or stale heartbeats.
Define that rule in the manifest; make the comparison claim only once the count
clears it. Launching big is the goal; claiming "largest" without the count is the
one thing to hold.

---

## 6. Go / No-Go gate

Run before announcing that contributors are earning:

- [x] Tassadar run page: live state + manifest + staleness (A). ✅ #5006
- [ ] ≥1 **non-owner** Pylon admitted + dispatched executor work self-serve (B).
- [ ] Public `exact_trace_replay` `Verified` verdict for that contributor (C).
- [ ] That contributor holds a real settled-sats payout + public receipt (D).
      _Seam wired (#5009): the settlement-receipt route + run link are live;
      box flips when the real stranger settlement lands._
- [ ] Leaderboard/run `settledPayoutSats` non-zero + accepted-trace count growing
      (E). _Both read-paths wired: `settledPayoutSats` (#5009) and the
      verified-trace `corpus` count (#5010, W2). Each goes non-zero on the first
      real settlement / Verified `exact_trace_replay` trace; box flips at launch._
- [ ] The linked install path is reproducible on a clean machine (F). _Code done
      (#5011/#5025/#5027): the app launches/adopts the node and the packaged
      `.app` bundles it; box flips after a clean-machine first-run smoke on the
      signed build (signing NEEDS-OWNER)._
- [ ] Copy passes §7, cites live registry version + promise IDs (live `2026-06-14.9`).

When all of A–E are real for a stranger, flip
`training.monday_decentralized_training_launch.v1` green **receipt-first** per
`proof.claim_upgrade_receipts.v1`. Until then the run is still **launched and
live** — you just describe the earn loop by what the receipts actually show
(§7), without claiming a payout nobody can dereference.

---

## 7. Copy gate (Tassadar wording)

Before any copy: query `/api/public/product-promises`, use the live version, cite
exact promise IDs.

**Say:** "the Tassadar run is live"; "install Autopilot (the desktop app) and
contribute executor-trace work to help train Tassadar"; "work is verified by
exact replay"; and — only with a real receipt — "the first contributors earned
Bitcoin, here's the receipt."

**Do NOT say** (until the matching promise is green or the copy is explicitly
caveated): "Tassadar is trained / outperforms a CPU / is a working model";
"largest decentralized training run" or "200+ contributors" (no count rule yet);
"earn Bitcoin from training today" as a blanket claim before a stranger has;
"stable / GA Autopilot Desktop"; any payout number that isn't a settled,
dereferenceable receipt. We are launching the **run that trains Tassadar**, not a
trained Tassadar.

---

## 8. Owner lanes & smokes

- **worker-api:** Tassadar run state-transition route + projection (A),
  exact-replay verdict on the live submission (C), payout leg (D),
  settlement+corpus projection consistency (E).
- **pylon + desktop:** Autopilot Desktop install that bundles/launches the node
  (§0 seam), self-serve executor-capability admission + dispatch (B), install
  path + smokes (F), contributor-side payout receive (D).
- **product/forum:** manifest + count rule (A, §5), Go/No-Go (§6), announcement
  (G), receipt-first promise flip.

Suggested checks (gap audit + Tassadar lane + desktop cockpit):

```sh
bun run --cwd apps/openagents.com/workers/api smoke:tassadar:executor-trace
bun run --cwd apps/openagents.com/workers/api smoke:training-runs:public
bun run verify:autopilot-desktop:training   # Autopilot Desktop training cockpit gate
```

New end-to-end **stranger smoke** (passing it *is* the launch): fresh non-owner
**Autopilot Desktop install → node online → declare executor capability** → admit
to the Tassadar run → dispatch a digest-pinned workload → exact-replay verify →
programmatic treasury payout (capped) → public receipt → run/leaderboard reflects
it + corpus count grows.

---

## 9. Risks & abort rules

- **Payout leg (D) is the gating risk** — the read/write seam linking accepted
  executor work to a run-referenced settlement receipt is the part that isn't
  wired yet. Payout itself is programmatic: the OpenAgents treasury wallet (the
  `/treasury` MDK-backed wallet) makes the payout and Artanis is already wired to
  pay out from it under bounded spend authority (the nexus-treasury payout
  ledger). Keep a hard per-payout cap; do not block the launch on large
  unattended dispatch.
- **Thin/rc1 fleet + the §0 install seam** — the loop is dispatch-failing for lack
  of eligible online devices; the launch's job is to bring devices online, so make
  Autopilot-install → node-online → admit frictionless (resolve the bundle/launch
  seam) and make sure rc-version nodes are actually admittable.
- **Owner Pylons are not strangers** — the DoD requires a non-owner.
- **A payout the recipient can't see is the projection-staleness bug wearing
  money** — if settlement lands somewhere undereferenceable, stop and fix the
  projection before announcing.
- **No secrets** in the manifest, receipts, tick ledger, Forum posts, or run
  projection: no prompts, host paths, provider payloads, invoices, preimages,
  payment hashes, mnemonics, or bearer tokens.
- **Don't leak the PoC into a model claim** — `compute.tassadar_executor_poc.v1`
  green proves replay of bounded workloads, not a trained Tassadar.

---

## 10. One-line status to repeat tomorrow

> The Tassadar run is live at `<status URL>`. Install Autopilot, contribute
> executor-trace work, it's verified by exact replay, and you get paid sats —
> first receipts at `<leaderboard URL>`, corpus growing toward Tassadar. (Word it
> to what §6 actually shows; never claim a payout without a dereferenceable
> receipt.)

---

## 11. The other Episode 237 promises (beyond the training run)

§1–§10 drive the **training loop** to a paid stranger. But the Episode 237
announcement (`docs/transcripts/237.md`) and its essays
(`jun15/essay.md` "You Must Construct Additional Pylons",
`docs/autopilot-coder/2026-06-14-the-second-engine-network-effects-agent-time-growth-essay.md`,
`docs/autopilot-coder/2026-06-14-the-load-bearing-wall-verification-accepted-work-essay.md`)
make a wider set of claims. Each must map to a promise ID whose **current
registry state already supports the wording** (`/api/public/product-promises`,
`2026-06-14.9`). This section is the launch-copy boundary for everything the
video says that is **not** the Tassadar run. It does not flip any state; the
registry stays the authority and every flip stays receipt-first
(`proof.claim_upgrade_receipts.v1`).

### 11.1 Claim → promise → state → copy boundary

| Episode 237 claim | Promise ID | State | Launch copy boundary |
| --- | --- | --- | --- |
| "Today we launch Autopilot, our core product" / desktop app for Mac/Linux | `autopilot.desktop_gui_client.v1` | yellow | Say "download the first Autopilot desktop release candidate (Mac/Linux)"; it is the cockpit over a local node. Do **not** say "stable/GA," and honor the §0 bundle-the-node seam (#5011). |
| "First and final version launched by humans; future releases shaped by Autopilot itself" | `artanis.tassadar_evolution_loop.v1` | yellow | Aspirational/direction; the self-improvement loop is yellow. Frame as the goal, not a shipped fact. |
| "At the core of every Autopilot is Pylon" / install the node | `pylon.v03_release_candidate.v1`, `pylon.release_tomorrow.v1` | yellow | Pylon is `@openagentsinc/pylon@0.3.0-rc2`, rc not stable. Reconcile to "Autopilot is the install, Pylon is the node it drives" (§0 follow-up). |
| "Autopilot can spawn new Pylons on machines you give it access to" | `autopilot.control_center_fanout_marketplace.v1` | yellow | Fan-out is yellow; describe as available/operator-gated, not a one-click swarm. |
| "Pylon packages Psionic for inference, embeddings, distributed training" | `pylon.cli_tui_probe_background.v1` (green), `pylon.compute_revenue_modes.v1` (planned) | mixed | The runtime exists (green); **paid** inference/embeddings revenue is planned and per owner directive (2026-06-10) there are **no inference products** at this time. Don't claim inference earning. |
| "Largest decentralized training run, ever / beat ~200 contributors" | `pylon.largest_decentralized_training_claim.v1` | red | **Do not claim "largest"** until the §5 participant-count rule clears. |
| "One piece of software that earns Bitcoin in multiple ways" | `pylon.v0_3_multi_earning_node.v1`, `pylon.five_bitcoin_revenue_streams.v1` | red / planned | Claim the **specific green subloops** (Forum tips, agent-labor, coding work), not the umbrella. |
| "Percepta Executor Class / Tassadar model; LLMs as computers" | `models.tassadar_percepta_executor.v1` (red), `compute.tassadar_executor_poc.v1` (green) | mixed | Say we're **launching the run that trains it** and the bounded executor **PoC is green (replay only)**. Do **not** say Tassadar is trained / replaces a CPU. |
| "Marketplace of WASM plugins" | *no promise record yet* | n/a | Explicitly **experimental, high-risk/high-reward, not live.** Needs a new conservative promise record before any copy (see 11.2). |
| "Coding harness like Claude Code on steroids, 10× better" | `autopilot.codex_probe_pylon_successor.v1` | green (direction) | The Codex-oriented runtime direction is green; "10× / on steroids" is marketing puffery — keep it clearly aspirational, the harness is "still a work in progress" (the transcript says so). |
| "Companion mobile app" | `mobile.autopilot_remote_control.v1` | planned | Do **not** announce a shipped mobile app; it is planned. |
| "Every Pylon and Autopilot has a built-in Bitcoin wallet + Nostr keypair" | `payments.money_dev_kit.v1` (yellow), `agents.cursor_forum_wallet.v1` (green) | mixed | Wallet + Nostr identity per node is real for the agent surface (green); keep payout/settlement language strict (`paid ≠ settled ≠ accepted`). |
| "Open source; feature requests / bug reports / analysis welcome; Forum, Discord" | `repo.open_source_code_map.v1`, `agents.one_instruction_sheet.v1` | green | Fully green. Route bug reports to the strict issue form, loose reports to the Product Promises Forum. |
| "Agents arrive, earn for their owner, point at useful work" | `labor.forum_work_requests.v1`, `labor.nostr_negotiation_market.v1`, `forum.content_tipping.v1` | green | Green — this is the strongest "agent gets paid" story; lead the **agent engine** copy here. |

### 11.2 Registry follow-ups this surfaces (separate Worker pass, not this doc)

These are **recommendations** for the promise registry
(`apps/openagents.com/workers/api/src/product-promises.ts`); applying them is a
receipt-first Worker deploy, not a launch-plan edit:

1. **Add a WASM-plugin-marketplace promise record** (e.g.
   `marketplace.wasm_plugins.v1`, state `planned`/`red`) — the transcript makes
   this claim and the registry has no record for it, so there is no copy gate
   guarding it.
2. **Reconcile the install positioning** — `pylon.v03_release_candidate.v1`,
   `pylon.release_tomorrow.v1`, and `pylon.install_without_wallet_knowledge.v1`
   still frame the install as a standalone Pylon; Episode 237 makes **Autopilot
   Desktop** the install with Pylon as the node it drives (§0). Update safeCopy
   to the Autopilot-is-the-install framing.
3. **Make the wallet+Nostr-per-node claim explicit** — the "every Pylon and
   Autopilot has a built-in Bitcoin wallet + Nostr keypair" claim is implied by
   `agents.cursor_forum_wallet.v1` / `payments.money_dev_kit.v1` but not stated
   as its own node-identity promise; consider a record so the launch copy has a
   single ID to cite.
4. **Bump `PublicProductPromisesVersion`** when any of the above land, and record
   the matching `promise_transition` receipts.

### 11.3 The one rule (restated for the wider claims)

Same rule as §6/§7, applied to the whole video: **every sentence maps to a
promise ID whose state already supports it.** Lead with the greens (Forum tips,
agent-labor market, open-source code map, agent wallet/identity, the live
Tassadar run), frame the reds/yellows (largest-run, multi-earning umbrella,
trained Tassadar, WASM plugins, mobile app, self-improving releases) as the
direction we are **opening**, and never describe a payout nobody can
dereference. The load-bearing essay's line is the gate: *a payment a recipient
cannot dereference is not a payment; it is a bug wearing money.*

---

## 12. Issue tracker — the launch epics

**Status as of 2026-06-15.** Live registry is `2026-06-14.9`
(`/api/public/product-promises`). Closed so far: worker lane A–E
(#5006–#5010), registry reconcile #5013, count rule #5016, receipts cleanup
#5017, the entire desktop-UI cleanup (#5020–#5024), the install seam #5011, and
the desktop epic #5019. **Still open: the launch event #5014, the self-serve
earn #5015, and the announcement #5018** — #5014/#5018 are the live launch
event + announcement (not code); #5015 builds on the now-merged #5011.

### Master launch epic: flip the crucial promises green
- **[#5012](https://github.com/OpenAgentsInc/openagents/issues/5012)** — Epic:
  flip the crucial launch promises green for 2026-06-15. **OPEN.**
  - ✅ **[#5013](https://github.com/OpenAgentsInc/openagents/issues/5013)** —
    **DONE + deployed.** Registry reconciled to Autopilot-is-the-install,
    `marketplace.wasm_plugins.v1` added; live at `2026-06-14.5`, then
    `2026-06-14.6` added `training.public_gradient_windows.v1` (W5).
  - ⬜ **[#5014](https://github.com/OpenAgentsInc/openagents/issues/5014)** —
    Step 6: live non-owner Go/No-Go run-through → flip
    `training.monday_decentralized_training_launch.v1` green (the headline; §6).
    **OPEN — launch event.**
  - ⬜ **[#5015](https://github.com/OpenAgentsInc/openagents/issues/5015)** —
    Earn: self-serve install→earn with no operator staging → flip
    `pylon.install_without_wallet_knowledge.v1` green. **OPEN — next wave after
    #5011.**
  - ✅ **[#5016](https://github.com/OpenAgentsInc/openagents/issues/5016)** —
    **DONE + deployed.** Participant-count rule published in the run manifest;
    `qualifiedContributorCount` projected from verified+settled receipts (gates
    any "largest" claim; §5).
  - ✅ **[#5017](https://github.com/OpenAgentsInc/openagents/issues/5017)** —
    **DONE.** The four labor/fanout `promise_transition` receipts recorded via
    the operator route (exception receipts — flips already applied), registry
    caveats resolved at `2026-06-14.7`, dereferenceable at
    `/api/public/product-promises/transitions`.
  - ⬜ **[#5018](https://github.com/OpenAgentsInc/openagents/issues/5018)** —
    Announce: copy-gated launch announcement, post Go/No-Go (§7, §G). **OPEN.**

### Install seam (§0, §4.F)
- ✅ **[#5011](https://github.com/OpenAgentsInc/openagents/issues/5011)** —
  **DONE (Phase 1 code; cff473e01).** Autopilot Desktop adopts an already-running
  node or launches the local Pylon runtime into a managed `.pylon-local` home,
  supervises it (restart-on-crash with backoff, honest launching/online/failed
  states, stop-on-close), 16 unit tests. Webview status badge deferred (UI
  follow-up); Phase 2 packaging out of scope; the live clean-machine
  `electrobun dev` run is the §6 Go/No-Go event (#5014).

### Desktop UI launch-readiness epic — ✅ DONE
- ✅ **[#5019](https://github.com/OpenAgentsInc/openagents/issues/5019)** — Epic:
  make Autopilot Desktop UI launch-ready — strip dev scaffolding. **All children
  resolved (#5020–#5024 + #5011).** Audit found the data flow is genuinely live;
  the work was removing developer scaffolding from the shipped UI.
  (`view.ts` 3807→3196 lines; live data flow unchanged.)
  - ✅ **[#5020](https://github.com/OpenAgentsInc/openagents/issues/5020)** —
    **DONE.** Three-effect demo panels + sources card removed from the Nodes
    home pane.
  - ✅ **[#5021](https://github.com/OpenAgentsInc/openagents/issues/5021)** —
    **DONE.** Internal dev-doc panels stripped from the Training pane (API
    boundary / source map / authority / control surface).
  - ✅ **[#5022](https://github.com/OpenAgentsInc/openagents/issues/5022)** —
    **DONE.** Static "Issue 4855 Ledger" roadmap panel removed.
  - ✅ **[#5023](https://github.com/OpenAgentsInc/openagents/issues/5023)** —
    **DONE.** Fake Settings "Updates" status line fixed.
  - ✅ **[#5024](https://github.com/OpenAgentsInc/openagents/issues/5024)** —
    **DONE.** Internal jargon / issue numbers / env-var names scrubbed from
    user-facing copy.

### Decentralized-training lane (research → registry)
- 📋 `training.public_gradient_windows.v1` (`planned`, live in registry
  `2026-06-14.9`) — the W5 public-gradient / decentralized-optimizer lane is
  specified in [`docs/tassadar/RESEARCH_PLAN.md`](docs/tassadar/RESEARCH_PLAN.md)
  §5 (accepted training window, quarantine optimizer, gradient verification
  ladder, canary-gated promotion). Not a launch-day item; the next master
  tracker to file once a W3 student checkpoint exists. No public gradients into
  the canonical optimizer for the launch.

### Launch-readiness follow-ups (filed from the gap analysis)
- ✅ **[#5026](https://github.com/OpenAgentsInc/openagents/issues/5026)** —
  **DONE.** Desktop `bun run test` now runs the full suite (16 files / 185 tests)
  via a per-file runner (`scripts/run-tests.sh`), avoiding the bun load-hang —
  so we can "test as much as we can in dev mode."
- ✅ **[#5025](https://github.com/OpenAgentsInc/openagents/issues/5025)** —
  **DONE.** Webview node-launch **status badge**: the supervisor's
  launching/online/adopted/failed/unavailable status flows Bun→webview and
  renders on the Nodes pane (no fake online). Tests + `verify:training` green.
- 🔄 **[#5027](https://github.com/OpenAgentsInc/openagents/issues/5027)** —
  **#5011 Phase 2 packaging — the "try a release build on mac" endpoint:
  REACHED (unsigned).** `bun run build:canary` produces a mac `.app` that bundles
  the headless Pylon node at `Contents/Resources/app/pylon-node/index.js` (where
  `findPackagedPylonEntry` resolves it; verified by a test that extracts the real
  build tarball). So the packaged app builds and would launch the node like the
  dev path. **Kept open for owner-gated remainder:** codesign + notarization
  (Gatekeeper), OTA feed pinning (announced==admitted), the Linux path, and a
  clean-machine first-run→node-online smoke.

### Pylon: headless, CLI-only, agent-steerable (delete the TUI) — June 15 — ✅ EPIC DONE
- ✅ Epic **[#5033](https://github.com/OpenAgentsInc/openagents/issues/5033)** —
  **CLOSED.** Pylon is now headless/CLI-only and agent-steerable. All children
  merged: #5034 (TUI deleted), #5035 (CLI parity + `pylon help --json` catalog),
  #5036 (live AGENTS.md three paths, deployed), #5037 (runtime bundle-able
  headless — also unblocks #5027). The `pylon.agent_steerable_cli.v1` promise's
  blockers are cleared; ready for a receipt-first green/yellow flip pending
  live-node verification of the session/training verbs. Audit:
  [`apps/pylon/docs/2026-06-15-pylon-cli-only-agent-steerable-audit.md`](apps/pylon/docs/2026-06-15-pylon-cli-only-agent-steerable-audit.md).
  Children: ✅ #5034 (delete the TUI — `src/tui` + `@opentui/*` + `dashboard`;
  bare `pylon` now boots headless; `bun test` 1046 pass), ✅ #5035 (CLI parity:
  `sessions`/`approvals`/`deploy`/`training` verbs + `pylon help --json` catalog
  of 28 commands; 1066 tests pass), ✅ #5036 (live AGENTS.md three paths —
  deployed), and 🔄 #5037 (make `packages/runtime` bundle-able headless: it still
  pulls `@opentui` in `opentui-renderer.ts`/`cli.ts` — **the remaining blocker
  for bundling a headless Pylon into the packaged mac `.app` #5027**; in
  progress). Promise `pylon.agent_steerable_cli.v1` is now **GREEN** (live
  `2026-06-14.9`, receipt-first, verified via a live CLI round-trip). Agents at
  openagents.com/AGENTS.md now learn the three ways to run a node.

### Post-launch program: Artanis evolution loop → green
- 📋 Epic **[#5028](https://github.com/OpenAgentsInc/openagents/issues/5028)** —
  flip `artanis.tassadar_evolution_loop.v1` green via its four owed receipts:
  #5029 (sustained real tetrahedron-closed ticks), #5030 (unattended ≥10-tick
  streak — live blocker), #5031 (verify/harden public monitor), #5032 (first
  `dataset_curation` distillation-dataset receipt — live blocker). Program work,
  not launch-day critical path.

### Done — worker lane A–E (closed)
- **[#5006](https://github.com/OpenAgentsInc/openagents/issues/5006)** — Step A:
  run authority + manifest.
- **[#5007](https://github.com/OpenAgentsInc/openagents/issues/5007)** — Step B:
  executor-trace admission + claimable work.
- **[#5008](https://github.com/OpenAgentsInc/openagents/issues/5008)** — Step C:
  closeout → run-tied exact-replay verification.
- **[#5009](https://github.com/OpenAgentsInc/openagents/issues/5009)** — Step D:
  settlement-receipt ledger + projection seam.
- **[#5010](https://github.com/OpenAgentsInc/openagents/issues/5010)** — Step E:
  verified-trace corpus surface.

---

## 12b. Path to v1.0-rc builds (dependency-ordered)

**Goal:** signed, auto-updating **Autopilot v1.0-rc** and **Pylon v1.0-rc**,
published only to our GCP (`updates.openagents.com`, Cloud Run, project
`openagentsgemini`), signed by the OpenAgents release key (kid
`2dbe811d19f67528`, provisioned + GCP-Secret-Manager-backed) and verified
fail-closed by clients. **RC / canary channel ONLY — no stable/GA until the owner
says ready.** Epic **[#5045](https://github.com/OpenAgentsInc/openagents/issues/5045)**;
plans: `docs/ota/2026-06-15-ota-autoupdate-plan.md`,
`apps/oa-updates/docs/release-signing-runbook.md`.

Complete **in this order** (each phase gates the next):

1. **Signing & feed infra**
   - ✅ release-signing key provisioned + GCP-SM backup + signer/verifier
     (part of **[#5044](https://github.com/OpenAgentsInc/openagents/issues/5044)**;
     done this session).
   - ⬜ **[#5044](https://github.com/OpenAgentsInc/openagents/issues/5044)** —
     remainder: sign-on-publish + **pin pubkey + fail-closed verify in clients**;
     KMS migration.
   - ✅ **[#5043](https://github.com/OpenAgentsInc/openagents/issues/5043)** —
     `oa-updates` serves **signed** `pylon/<channel>/<platform>` rc feeds
     (rollout/yank/minVersion); verified end-to-end with the real 1.0.0-rc.1
     build (served artifact verifies against the pinned key). **DONE.**
2. **Build the artifacts**
   - ✅ **[#5041](https://github.com/OpenAgentsInc/openagents/issues/5041)** —
     Pylon `bun --compile` binary per platform + signed pipeline
     (`build-rc-binaries.sh`); all 4 platforms build/sign/verify. **DONE.**
   - ⬜ **[#5027](https://github.com/OpenAgentsInc/openagents/issues/5027)** —
     Autopilot signed+notarized `.app` bundling the node (signing creds ✅ done
     via #5048; Developer ID identity in keychain + GCP-SM backup).
3. **Wire default-on auto-update**
   - ✅ **[#5042](https://github.com/OpenAgentsInc/openagents/issues/5042)** —
     Pylon self-updater (verify→atomic-replace→relaunch), default-on; verified a
     compiled binary self-updates against the signed feed. **DONE.**
   - ⬜ **[#5040](https://github.com/OpenAgentsInc/openagents/issues/5040)** —
     Autopilot Electrobun updater default-on.
4. **Quality fold-in**
   - ✅ **[#5038](https://github.com/OpenAgentsInc/openagents/issues/5038)** —
     `--json` parser fix landed; folds into the Pylon rc. **DONE.**
5. **Produce the RC builds (rc channel)**
   - ⬜ **[#5046](https://github.com/OpenAgentsInc/openagents/issues/5046)** —
     Autopilot v1.0-rc (`1.0.0-rc.N`).
   - ✅ **[#5047](https://github.com/OpenAgentsInc/openagents/issues/5047)** —
     Pylon v1.0-rc (`1.0.0-rc.1`) — built/signed/verified all 4 platforms,
     binary reports 1.0.0-rc.1, runs headless. **DONE.**

**Explicitly NOT on the RC-build path** (these are GA / the live launch, owner-gated
— "until I say we're ready"): **[#5014](https://github.com/OpenAgentsInc/openagents/issues/5014)**
(live training Go/No-Go flip), **[#5015](https://github.com/OpenAgentsInc/openagents/issues/5015)**
(self-serve earn green), **[#5018](https://github.com/OpenAgentsInc/openagents/issues/5018)**
(announcement), **[#5012](https://github.com/OpenAgentsInc/openagents/issues/5012)**
(crucial-promise green-flip epic). RC builds can ship without flipping the
training promise green or announcing.

### OTA / provenance epics (cross-cutting, feed the above)
- **[#5039](https://github.com/OpenAgentsInc/openagents/issues/5039)** — OTA epic
  (default-on auto-update, one signed feed): children #5040, #5041, #5042, #5043,
  #5044 + the Psionic-auto-update + native-addon items.

## 13. Bonus stretch goal — the full distributed-training track

The launch loop (§1–§10) is deliberately the **proof-and-data layer**: public
Pylons *generate, validate, and evaluate* replay-verifiable executor-trace work,
and accepted traces accumulate the corpus. That is the safe, shippable rung. The
**bonus stretch goal** — if the launch loop clears and contributors want the next
rung — is the full Psion/Tassadar **distributed model-training track**,
culminating in *real public decentralized training*: public devices contributing
model updates that can advance a shared checkpoint. This is upside, **not a
launch-day claim**, and every rung is receipt-first.

The one hard rule that makes the stretch safe (refined from "no public gradients,
ever"): **no public gradient enters the _canonical_ optimizer until it passes
quarantine, verification, canary evaluation, and promotion gates.** A trace is an
artifact you can check before use; a gradient is an intervention that changes the
model — so the gradient lane is a strictly higher trust tier and earns the
checkpoint gate by gate. Full spec: [`docs/tassadar/RESEARCH_PLAN.md`](docs/tassadar/RESEARCH_PLAN.md)
§5 (W5), the Pluralis roadmap
([`docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md`](docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md),
the #4855 lifecycle substrate), and the Psion buildout plan.

### The ladder beyond the launch loop

```
Phase 0  public verified trace generation        ← TODAY'S LAUNCH (executor-trace)
Phase 1  controlled student trained from traces   (W3; controlled GPUs only)
Phase 2  public eval/validation of student ckpts
Phase 3  public adapter / LoRA training windows   ┐ W5 — public model-update layer
Phase 4  public small-model windows → quarantine  │   (accepted training window,
Phase 5  promoted public updates → canonical ckpt ┘    quarantine optimizer, gates)
```

The W5 machinery each rung needs: the **accepted training window** unit
(checkpoint + shard + config + seed + delta digest + loss stats + verification
refs + acceptance + receipt); a **quarantine optimizer** (`canonical → quarantine
→ promoted`); a **gradient verification ladder** (hash · recompute · replicate ·
statistical · canary · downstream); **checkpoint lineage + rollback**;
**dataset-shard authority**; **bandwidth-aware topology** (windowed/local-SGD, not
global all-reduce); **device tiers**; **staged payout** (pending → provisional →
accepted → settled → clawback); and **canary-eval promotion** judged on
first-divergence metrics, not loss.

### The training promise family (registry states, live `2026-06-14.9`)

These are the promises the full track will move; all stay **red/yellow/planned**
for the launch and flip only on receipts:

| Promise | State | Rung |
| --- | --- | --- |
| `compute.tassadar_executor_poc.v1` | **green** | bounded executor PoC (replay only) |
| `artanis.tassadar_evolution_loop.v1` | yellow | dispatch→verify→accumulate loop |
| `training.verification_classes.v1` | yellow | exact-replay + verifier classes |
| `training.device_capability_dataset.v1` | yellow | device tiers for roles |
| `pylon.first_real_model_training_run.v1` | yellow | bounded two-device real-gradient demo |
| `training.public_distributed_training_run.v1` | red | broad public run |
| `training.monday_decentralized_training_launch.v1` | red | the launch headline (flips via #5014) |
| `pylon.largest_decentralized_training_claim.v1` | red | gated on the #5016 count rule |
| `models.tassadar_percepta_executor.v1` | red | the trained model (not claimed) |
| `training.full_pipeline_program.v1` | planned | full pipeline program |
| `training.model_ladder.v1` | planned | model-size ladder rungs |
| `training.data_refinery_corpus.v1` | planned | corpus/data program |
| `training.ablation_system.v1` | planned | ablation organ |
| `training.marathon_operations.v1` | planned | long-run operations |
| `training.post_training_arc.v1` | planned | post-training (SFT/RL/preference) |
| `training.public_gradient_windows.v1` | planned | **W5 public model-update layer** |

**Launch-copy boundary for the stretch goal:** the honest bridge is *"this run
begins the decentralized-training stack — today public Pylons produce and verify
the exact-trace corpus; the next rung is public training windows: Pylons
contributing verified candidate updates to student models under quarantine and
promotion gates."* Do **not** say public decentralized gradient training is live
or paying.
