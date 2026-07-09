# MASTER ROADMAP — Khala Code MVP (Tested, Submitted) → Sarah → Codex → AI Employees → the Suite

Date: 2026-07-09 (rev 6.15 — future-agent issue execution instructions)
Status: **the single consolidated execution roadmap.** This document owns
top-level sequencing across everything designed in the 2026-07-07 strategy
set and its predecessors. The source docs remain authoritative for their
*content* (specs, evidence, arguments); when sequencing here and sequencing
there disagree, **this document wins**, and new issues are filed against the
phase lanes named here.

**Rev 6.15 changes (owner directive, 2026-07-09 — HOW FUTURE CODING
AGENTS MOVE THROUGH THE QUEUE):** this roadmap now carries the execution
instructions for any future coding agent burning down the ordered
open-issue list below. The loop is: read the repo and issue context,
work from a clean worktree, land directly on `main`, push with
`--no-verify` after local validation, comment/close the issue, clean up
only the worktree/branch the agent created, then continue to the next
listed issue unless the owner tells the agent to stop.

**Future coding-agent runbook for this issue queue:**

1. Start by reading `/Users/christopherdavid/work/AGENTS.md`,
   `/Users/christopherdavid/work/INVARIANTS.md`,
   `openagents/AGENTS.md`, `openagents/INVARIANTS.md`, the issue body,
   any linked PR, and the relevant local docs before editing. If the
   work touches a child invariant surface, read that surface's
   `INVARIANTS.md` too.
2. Review open PRs before starting the next issue. If a PR already
   implements the issue, inspect it, fix it if needed, merge/land it on
   `main`, comment with the verification, and close the issue.
3. Never work in the normal checkout if it is dirty, behind, or contains
   another agent's files. Create a fresh detached worktree from current
   `origin/main`, for example:
   `git fetch origin main && git worktree add --detach /Users/christopherdavid/work/openagents-worktrees/codex-issue-<issue>-<date> origin/main`.
4. Work the ordered list below from top to bottom. Parent epics are
   closeout steps: do not close an epic until its child issues and proof
   gates are actually complete.
5. Keep changes scoped to the issue. For UI work, build through Effect
   Native and upstream/register component demand instead of inventing
   local React/Foldkit/DOM one-offs. Preserve the Sarah/OpenAgents
   authority boundaries: no prompt-side filtering, no improvised
   pricing, no public claim without the proper receipt.
6. Update docs, behavior contracts, invariant ledgers, tests, smoke
   gates, and demand registers in the same change whenever the issue
   changes a contract, proof surface, public promise, or Effect Native
   demand.
7. Run the targeted tests first, then the smallest relevant full suite.
   A `--no-verify` push is not permission to skip validation; it is only
   the owner-mandated push mechanism for this queue. If a known
   pre-existing check still fails, report the exact file/error in the
   issue closeout.
8. Commit only scoped files from the worktree. Before pushing, fetch
   `origin/main`; if main moved, rebase or replay the scoped commit onto
   the new head without stashing or moving other agents' work.
9. Push the finished commit directly to `main` with:
   `git push --no-verify origin HEAD:main`.
10. After the push succeeds, comment on the issue with the commit hash,
    summary, docs updated, tests/smokes run, and any residual known
    blocker. Then close the issue with that comment.
11. Remove only the worktrees or branches created by that agent for that
    issue. Never clean up, stash, reset, or delete another agent's
    worktree, branch, or uncommitted files.
12. Continue immediately to the next open issue in this roadmap unless
    the owner gives a newer instruction to pause, stop, or reorder.

**Rev 6.14 changes (execution update, 2026-07-09 — BM-1..4 LANDED,
STOP POINT ORDERED):** open PR review found PR **#8632** carrying BM-1
(`blueprint_delta` SSE events). It was patched before merge so profile
refreshes publish only newly distilled facts, then landed on `main` as
`7838a9a1ca`. The Sarah Blueprint Map implementation now has BM-1..4
complete on `main`: BM-3 split layout + declutter + the verbatim
`sarah.split_screen_blueprint_map.v1` contract (`4fb1ec6ba7`), BM-2
the live Effect Native `GraphFigure` Blueprint map with prospect-scoped
seed reads and delta updates (`9ee02315f5`), and BM-4 the Effect Native
Actions tab plus Code/Receipts panel (`dc0c67948c`). BM-4 also records
owned tool results into Sarah's existing session-index receipt model,
adds the active-prospect `/sarah/api/session/receipts/current` read, and
extends the KHS-3 isolation oracle to cover tool receipts. All Sarah
unit suites passed after BM-4 (`bun test` in `apps/sarah`: 264 pass).
`bun run typecheck` remains blocked only by the pre-existing
`src/contracts/avatar-ux-contracts.ts` enum literal issue
(`"script"` / `"e2e"` / `"smoke"`). Open PR list is empty after this
pass.

**Ordered open-issue queue (OpenAgentsInc/openagents, checked
2026-07-09 after `dc0c67948c`):**

1. **#8621 SQ-4** — finish the owned realtime renderer hardening and
   deploy-smoke rail first; BM-5 rides this rail, and deploys stay
   frozen until the real-session simulator proof is recorded.
2. **#8631 BM-5** — owner playback/QA gate: desktop + mobile split
   screenshots, `sarah.split_screen_blueprint_map.v1` sweep, live
   `blueprint_delta` node-appears proof, and concurrent-ref isolation.
3. **#8626 Sarah Blueprint Map epic** — close only after #8631 passes;
   BM-1..4 are complete.
4. **#8612 OAV-2** — close or finish the render-service state machine,
   MuseTalk/WebRTC service evidence, and hydralisk receipts so the
   live renderer's base service is not half-accounted.
5. **#8615 OAV-5** — pre-rendered takes for opener/cache hits, using
   the now-owned renderer path and receipts.
6. **#8620 SQ-3** — perfect opener library: repair v1 defects, add
   missing scripts, integrate via owned renderer + KHS-6.
7. **#8619 SQ-2** — run the OAV experiment matrix to completion with
   tightened exit conditions.
8. **#8616 OAV-6** — quality ladder: Ditto A/B, SoulX-FlashHead,
   LatentSync offline renders, then choose the next production recipe.
9. **#8610 OAV epic** — close after OAV-2/5/6 and the SQ quality lanes
   above have receipts.
10. **#8600 KHS-1** — Sarah inference through Khala gateway: receipts,
    cost caps, quota fallback lanes; reconcile current production state
    with the issue and close only with proof.
11. **#8607 KHS-8** — in-conversation payments: card attach + pay in
    chat, after the no-improvised-pricing/receipt path remains green.
12. **#8543 P0.8** — launch readiness: seeded test account,
    unattended straight-line E2E, promises/copy pass.
13. **#8467 Khala Code Mobile-Only MVP epic** — close only after the
    P0.8 launch-readiness receipt and mobile MVP proof set are complete.
14. **#8571 EN hosting** — host effectnative.org on OpenAgents cloud:
    Cloud Run, DNS/TLS, redeploy script.
15. **#8595 WEB-1-EN** — fully Effect Native landing; upstream logical
    components instead of local React/Foldkit carryover.
16. **#8573 EN-4** — web absorption: product routes onto the Effect
    Native DOM renderer, deleting replaced legacy Foldkit/React.
17. **#8597 MB-EN** — rewrite the Khala Code mobile app on Effect
    Native, with cross-app Khala Sync as exit proof.
18. **#8574 EN-5** — Khala Code desktop conversion to Effect Native;
    this is also the substrate for PY-2.
19. **#8575 EN-6** — canvas/Verse under the Effect Native canvas
    contract; fold in the three-effect kernel and retire the Foldkit
    adapter.
20. **#8566 EN epic** — close after the EN child issues above produce
    receipts and downstream surfaces are actually on the substrate.
21. **#8578 PY-1** — extract `pylon-core` packages and delete the
    desktop stdout-subprocess seam.
22. **#8579 PY-2** — Khala Code desktop becomes the Pylon cockpit
    (Effect Native fleet pane: online toggle, accounts/capacity/runs/
    receipts).
23. **#8580 PY-3** — retire the Pylon OpenTUI surface only after PY-2
    cockpit-parity receipts and owner-gated exit proof.
24. **#8590 MH-4** — Grok Axis B worker executor + RL-1..6 probes.
25. **#8547 CX-3** — Codex inside the Agent Computer: broker
    redemption, scratch `CODEX_HOME`, `codex_app_server` org-cloud proof.
26. **#8588 MH-9** — cloud parity for Grok/Claude workers on Agent
    Computers; strictly after #8547 is genuinely green.
27. **#8546 CX-2** — mobile Codex connect: phone device-auth into token
    custody and accounts UI.
28. **#8548 CX-4** — harness/target selection UX: execution targets,
    per-thread pill, quota-aware auto.
29. **#8549 CX-5** — Claude account parity through the same broker and
    `claude_pylon` lane.
30. **#8550 CX-6** — Codex session continuity across ephemeral
    microVMs; thread resume survives reclaim.
31. **#8551 CX-7** — multi-account concurrency + rotation in the
    cloud: per-account serialization and typed queueing.
32. **#8552 CX-8** — daily-driver ergonomics from the phone:
    steer/interrupt/follow-up and monorepo-scale workspaces.
33. **#8553 CX-9** — dogfood cutover: our coding runs through Khala
    Code mobile on our own Codex.
34. **#8558 OB-1** — Sarah sending identity + deliverability foundation:
    subdomain, auth, arming, warm-up ramp.
35. **#8559 OB-2** — Apollo sourcing at volume into
    `business_pipeline_rows`.
36. **#8561 OB-4** — draft→approve→send loop at 100/day, batch
    approval UX, Sarah reply handling.

**Rev 6.13 changes (owner directive, 2026-07-09 — SARAH BLUEPRINT MAP
SURFACE):** "I want to build a Blueprint map as Sarah is talking. Visual
representation of what she knows / is trying to learn from the user…
her taking up ~half of the screen (left), other half the canvas with
graph / buttons / code / chat etc." Epic **#8626**, lanes **BM-1..5
(#8627–#8631)**, audit
`docs/sarah/2026-07-09-blueprint-map-surface-audit.md`. The Blueprint
spec's own named-but-deferred visual surface ("Blueprint Map") arrives
as Sarah's prospect-facing graph: the KHS-9 customer Blueprint draft +
KHS-2 distilled facts project onto the EXISTING Effect Native
`GraphFigure` (catalog v19, effect-native#37 — the arbiter-graph model;
the vendored v26 snapshot already includes the DOM/SVG path), streamed
live over the AV-3 SSE bus via new typed `blueprint_delta` events, on a
split desktop layout (video ~50vw full-height left; right pane = tabbed
canvas: Blueprint map / chat / actions / code+receipts) that also
settles the owner's "too much bullshit on the page" verdict (declutter
cuts listed in the audit; the AI-disclosure banner stays, copy
unchanged). Edges light only from real provenance
(matchedNeedTurnIds/receipts — the arbiter law); KHS-3 isolation,
no-improvised-pricing, and the EN demand-register discipline
(components upstream, never local one-offs) bind every lane.
Cross-linked to EN-6 #8575 (the same typed model moves onto the unified
canvas contract at CV4), OAV #8610 (the left pane is the owned-renderer
video seam), and SQ-4 #8621 (the deploy-smoke rail carries the BM-5
gate). See the P1 "Sarah Blueprint Map surface" entry.

**Rev 6.12 changes (2026-07-09 — OAV QUALITY PROGRAM adopts the owner's
research doc):** the owned avatar video lane family (epic **#8610**,
OAV-1..6 #8611–…; OAV-4 renderer seam #8614 landed) gets an explicit
quality program driven by `docs/sarah/research.md` (the owner's
prioritized paper/repo triage: LatentSync 1.6, FLAIR, RIFE,
CosyVoice2/3, MuseTalk tuning, BasicVSR++/RealBasicVSR, tamed GFPGAN,
Hallo2/3, the license-blocked set, and the A0–F2 experiment matrix) and
recorded as policy in `docs/sarah/2026-07-09-oav-quality-strategy.md`
(adopted plan + empirical status) and the pipeline spec's new §9
(enhancement/quality-tier policy). Empirical state: the tamed-GFPGAN v3
recipe (alpha-blend + feathered mask + temporal EMA) measured the
smoothest take (jerk 0.15); LatentSync 1.6 is the articulation winner
but shows a 16-frame chunk-boundary hitch; the opener library v1 is
rendered both ways awaiting QA; FLAIR (MIT — corrects the earlier
"every temporal restorer is license-blocked" claim), RIFE 48fps,
CosyVoice prosody variants, and Hallo2 remain queued. Full-strength
per-frame GFPGAN is banned as a default enhancer. See the P1 "OAV
quality program" entry.

**Rev 6.11 changes (owner directive, 2026-07-09 — SARAH × KHALA, KHS
lane family):** spoken to Sarah herself in her own production surface and
executed the same night. Epic **#8599**, lanes **KHS-1..9 (#8600–#8608)**,
grounded in `2026-07-09-sarah-khala-connection-assessment.md`: gateway
inference migration first (the owner's own session — 54 persisted turns —
was degraded by a per-VAD-fragment 429 quota storm on the raw key; an
interim per-model fallback chain shipped same night), then prospect
memory ("she learns the user she's talking to"), the cross-prospect
isolation law BEFORE owner-approved collective learning (the admin
approval queue for anything she learns generally), Sarah's own Blueprint
(the P4 `ai_employee.v1` seed as data migration), the semantic answer
cache (embedding-matched non-LLM responses; pre-recorded audio gated on
the LITE lane), and in-conversation account creation + payments (sign
up, log in, attach a card, and pay without leaving the chat). Sequenced
under P1/P4; the semantic-routing invariant, registry honesty, and
no-improvised-pricing bind every lane. Also same night: the avatar
opener simplified by owner copy edit ("Hello! I'm Sarah. What's on your
mind today?") with a base-context cache TTL so copy edits propagate
without instance rolls.

**Rev 6.10 changes (owner directive, 2026-07-09 — SARAH AVATAR ON EFFECT
NATIVE):** Sarah gets a real-time avatar face at openagents.com/sarah,
built per the assessment
(`../sarah/2026-07-09-liveavatar-integration-assessment.md`) and tracked
as epic **#8598** (AV-1..6): HeyGen LiveAvatar **FULL Mode** via the Web
SDK in our own page (the tested iframe embed is sealed — no events, no
tools — and is not built on), with the **Custom LLM add-on pointed at an
OpenAI-compatible endpoint we host** so the avatar's brain is the owned
runtime (Gemma 4 on our Google inference, deterministic pricing guard
BEFORE the model, turns receipted to the session index) — one brain for
text and voice. Components pop via SDK data-channel transcriptions + a
session-keyed SSE tool-effect bus into typed cards. **The /sarah surface
is reauthored in Effect Native** (`@effect-native/core` + DOM renderer,
zero React, the hand-rolled sarah.js shell deleted) — satisfying the
open SM-2 item on #8594; the avatar video mounts in a sibling container
pending a `media-video` Host kind (demand filed in
`docs/sarah/EN-GAPS.md` → EN-2 #8572). Session minting is S-3-guarded
(caps + usage JSONL — avatar minutes are credit-metered); LITE mode
(1 vs 2 credits/min, BYO STT/TTS) is deferred until usage data justifies
it. Owner gates: production avatar_id selection (sandbox default until
then) and the LiveAvatar credit budget.

**Rev 6.8 changes (2026-07-09 — Sarah consolidation landed + review):**
the SM epic #8594 executed same-night (`974490e7ba`, `31921bb3ab`):
`apps/sarah/` is live in-repo (Bun service, zero React, eve dependency
removed, CRM email rail replacing the local Resend/suppression stack,
migration receipt + redaction audit in `docs/sarah/MIGRATION.md`), and
the private repo is marked historical. **Serving amendment recorded
during execution:** no `sarah.openagents.com` subdomain — Sarah mounts
at **`openagents.com/sarah`** (`/sarah/api/*`; recorded as
owner-directed in `docs/sarah/MIGRATION.md` — flag to the owner if that
attribution is wrong). Forward references below are updated to the
`/sarah` path; historical paragraphs keep the subdomain as written.
A post-landing review re-ran every oracle from a clean checkout and
hardened two: the S-13 smoke was non-idempotent (state accumulation
under `.sarah/` failed reruns) and S-3 required a hand-configured
server and printed minted client tokens — both fixed (S-3 now spawns
its own isolated capped server and redacts tokens; S-12 re-verified
6/6 live). **Honest SM-2 status:** the shipped surface is a zero-React
DOM shell with EN demand recorded in `docs/sarah/EN-GAPS.md`; authoring
it in actual Effect Native components remains OPEN on #8594 — the
mandate is not satisfied by the interim shell. SM-5 mount on openagents-monolith ships with the path cutover
(`/sarah` → handleSarahRequest); live S-12 + Vercel teardown complete the
lane, then SM-6 final retirement receipts.

**Rev 6.7 changes (owner directive, 2026-07-09 — SARAH IN-REPO, EFFECT
NATIVE):** "All Sarah shit must be ported to Effect Native, moved into
this codebase — no more separate sarah repo." The 2026-07-07
separate-repo posture is **reversed**: `OpenAgentsInc/sarah` consolidates
into this monorepo as `apps/sarah/` (Bun/Effect service, Effect Native
UI on the DOM renderer, Cloud Run from monorepo-built images), following
the Cloud-consolidation pattern (#8591). Plan:
`2026-07-09-sarah-monorepo-effect-native-consolidation-plan.md`; epic
**#8594** (lanes SM-0..6: freeze/scrub → port-as-move → EN voice surface
→ email/CRM convergence onto the one approval-gated rail → owned Effect
agent runtime replacing eve → cutover → repo retirement). Superseded:
sarah#14 (Next lift-and-shift) and sarah#15 (TanStack port); remaining
sarah S-6/S-7/S-8/S-11 scope absorbs into the SM lanes. Authority is
unchanged (the openagents.com API stays the system of record; in-repo
Sarah calls
its public APIs), and the P1 SR exit receipts are unchanged — this
moves where Sarah lives and what renders her, not what she must prove.
Track C (OB-*) never blocks on the epic.

**Rev 6.6 changes (2026-07-08 night, owner-directed live execution — Phase
6 cutover, `546e3cf840`):** the owner directly authorized a live production
deploy of the `#8591` cloud consolidation ("Phase 6"), executed same-night:
linux/amd64 images for `oa-codex-control`/`oa-workroomd`/`oa-node` built and
pushed from **this monorepo** to Artifact Registry
(`us-central1-docker.pkg.dev/openagentsgemini/oa-cloud`, tag
`openagents-main-a82a8dd358e0`); production instance `oa-codex-control-1`
redeployed onto that image and confirmed live (`GET /healthz` → 200); a real
live smoke sequence passed against it (`cloud-gcp` placement →
`cloud.gce.provisioned` + `cloud.gce.resource_usage_receipt`, cancel 202,
fake Cloud-VM lifecycle ok); `oa-workroomd` staged for guest bake on
`agent-computer-gce-1`. Full receipt:
`docs/cloud/receipts/2026-07-09-phase6-openagents-cutover.md`.

**This confirms and sharpens rev 6.5's CX-3 finding, with one important
architectural correction:** the control host (`oa-codex-control-1`,
`e2-small`) has **no `/dev/kvm`** and by design never runs real Firecracker
— its Cloud-VM provisioner is intentionally `fake`. The real nested-virt
microVM lane is the **separate** host `agent-computer-gce-1`. Do not
conflate "the control plane is live" with "in-VM Codex execution is
proven" — they are different hosts and different proofs. Per the
receipt's own #8503 DoD table: control-plane-from-monorepo, live
placement+receipts, and fake-VM lifecycle are **done**; the full
mobile-dispatched Firecracker turn with writeback + exact token receipt is
**not re-run this pass** — prior substrate proofs exist on
`agent-computer-gce-1`, but a fresh one still needs `work_context` armed +
a live Cloud-VM run on that nested-virt host with the agent-computer
rootfs. This is the same "no source-controlled rootfs build script"
wall the rev 6.5 CX-3 assessment already identified — now pinned to the
correct host.

**Also flagged, not yet resolved — a live security exposure:** the receipt
records the control-plane firewall temporarily allowing `0.0.0.0/0` on
port 8787 for this cutover's smoke testing, with its own note to tighten
to IAP + office egress "when not actively testing." This is now recorded
to `NEEDS_OWNER.md` (rev 6.6) — an agent should not unilaterally narrow a
firewall rule on a just-cut-over live host without confirming testing has
concluded, but this should not sit open indefinitely either.

**Rev 6.5 changes (2026-07-08 night → 2026-07-09, overnight parallel fleet
burn-down — status + unblocks, no new policy):** a sustained multi-agent
parallel push closed **28 issues** across `openagents` + public
`effect-native`, fixed **3 live production incidents**, merged **1 PR**, and
correctly reopened **1** issue after independent re-verification found it
didn't meet its acceptance criteria on first close. Headlines:

- **The full MH wave (§MH) is COMPLETE**: MH-0 `#8581` → MH-1 `#8582` → MH-2
  `#8583` → MH-5 `#8584` → MH-6 `#8585` → MH-8 `#8587`, every one
  independently re-verified (fresh worktree, real test run, not just trusted
  self-report) after landing. MH-2 found and fixed a real bug blocking every
  real Claude-account run (the account's own OAuth token was mis-flagged as a
  leaked SCM credential). Grok's own MH-3 `#8589` / MH-4 `#8590` lane is
  progressing in parallel (their exclusive scope, not touched here).
- **PRUNE (`#8577`) executed**: the retired Tassadar/Psionic surfaces were
  physically removed — **−337,581 LOC**, archived to
  `backroom/openagents-prune-20260708-tassadar-psionic` (commit `a56fd270`)
  before deletion. A dangling-reference regression the prune left behind
  (`replay-signatures.ts` fixtures not schema-conformant) was found and fixed
  same-night (`473300f4b5`).
- **Three live production incidents fixed, all independently verified:**
  (1) Khala completions had been returning 502 **since 2026-07-05** — root
  cause was an exhausted OpenRouter platform credit balance whose 402 was
  misclassified non-retryable, so requests never overflowed to the healthy
  Vertex/Fireworks lanes; fixed + deployed
  (`docs/incidents/2026-07-08-khala-502-openrouter-credit-exhaustion-aar.md`).
  (2) That same investigation found a **latent deploy blocker** (dangling
  imports from the PRUNE) that would have blocked any deploy from `main` —
  fixed in the same pass. (3) `khala-sync-pg` had been hitting its
  100-connections-per-instance cap since ~2026-07-07 (10 call sites
  constructing a fresh `postgres.js` client per statement instead of sharing
  a pool); fixed, staged, promoted to prod, and verified against **live
  production traffic** (the error flood confirmed stopped at the exact
  revision cut).
- **Pylon fold (PY-1 `#8578`) — 3 sessions in, real incremental progress, not
  yet closed:** custody (most modules), executor (leaves + workspace-
  materializer + active-assignment-runs), and an RPC contract seed are
  extracted into `packages/pylon-core`. The Spark wallet is **deliberately
  still untouched** (twice now — needs RC-binary verification not available
  headlessly; the owner's live-rail mandate is being honored, not skipped
  for convenience). Presence extraction is blocked on a real, precisely
  diagnosed cross-package resolution issue (not the earning-code coupling
  the first session guessed — that turned out to be a non-blocker on
  re-trace). MCP consolidation has a written plan, correctly not attempted
  blind.
- **Cloud repo consolidation (`#8591`) landed** — the private
  `OpenAgentsInc/cloud` Rust crates (`oa-codex-control`, `oa-node`,
  `oa-workroomd`, `oa-cloud-run-bridge`, `openagents-cloud-contract`) are now
  **in this monorepo** under `crates/*`, with Effect Schema mirrors in
  `packages/cloud-contract`. This resolves CX-3 `#8547`'s stated blocker
  ("control-plane half lives in the private repo") — reassessed and
  confirmed: most of CX-3 was **already landed** (in-VM broker redemption,
  `CODEX_HOME`-on-scratch, org-capacity billing, the owner-local
  `codex_app_server` dispatch lane). The real remaining wall: no
  source-controlled rootfs build script exists yet (hand-`debootstrap`ped on
  the GCE host), and `provider_credential_policy: broker_only` isn't yet
  threaded into the Rust crates. A #8591 residual bug (a build script
  pointing at pre-move Dockerfile paths that would have broken the image
  build) was found and fixed in the same pass; a second #8591 residual (a
  smoke script landing non-executable) was fixed via PR `#8592`, reviewed
  and merged same night.
- **EN-1 → EN-3, Effect Native landing/mobile proofs**: the `/stage1` web
  route (EN-1) is the first production-adjacent Effect Native render; EN-3
  `#8568` proved the React Native adapter on one new (not migrated) mobile
  screen — discovering along the way that the premise "desktop already
  consumes effect-native" was false (only the web app did), and that landing
  required bumping the web app's vendored `core`/`render-dom` snapshot from
  v0 to v5 — verified independently to NOT regress `/stage1`.
- **Public `effect-native` repo**: Phase 2 is fully complete; Phase 3 is
  4/5 (DevTools, gallery, testkit, the guide — the guide was caught mid-close
  on a premature merge and correctly reopened, then genuinely completed on a
  second pass); Phase 4 (`#20` epic) is deep in progress — 12 of ~22 child
  issues closed in two large sequential batches (a monolith-file collision
  pattern: `packages/core/src/index.ts` is shared by every catalog issue, so
  one agent works the queue serially rather than many agents racing the same
  file), catalog now well past `v9`.

**Rev 6.4 changes (owner priority, 2026-07-08 night — MULTI-HARNESS
PARALLELIZATION):** Khala Code (mobile + desktop) parallelizing coding
work across **Codex, Claude Code, and Grok CLI** is a **now-priority
program**, run as the new cross-phase **MH lane family** (§MH below) in
parallel with P1 sales (separate capacity — sales agents never touch the
coding claim registry) and with **CX-3 `#8547` as the protected cloud
linchpin whose capacity is never reassigned**. Authorities: the Fable
analysis + Grok dialogue in
`2026-07-08-multi-harness-parallelization-effect-native-analysis.md`
(§1–§12; consensus complete) and the `docs/grok/` pair. Settled law:
Axis A (chat harness) ≠ Axis B (worker kind); Grok enters via ACP
(`grok agent stdio`), never TUI scraping; contract-first waves; the
June 29 claims/verify/refill/product-visibility laws hold at 3×
engines; **one typed intent/mutator vocabulary shared by Effect Native
UI and Khala Sync** (steering is serializable data end to end); the
multi-harness cockpit is the effect-native Phase 4 catalog build
(demand via EN-2 `#8572`); the Grok executor is born inside the rev 6.3
`pylon-core` boundary; an enum-driven **harness conformance suite**
makes adapter consistency mechanical; `auto` v1 is deliberately dumb
and fully typed. **Free-Grok economic window:** Grok 4.5 is currently
free for us — bias fixture/dogfood fan-out toward Grok under *measured*
rate-limit ceilings (RL-1..6 probes), encoded as
`marginal_cost_class: free|subscription|api_metered|not_measured` on
capacity rows, never hard-coded. Division of labor: **the Grok lane
files and executes its own adapters (MH-3/MH-4 + RL probes)**; the main
fleet owns the spine — filed tonight as **MH-0 `#8581` (serial Wave 0),
MH-1 `#8582`, MH-2 `#8583`, MH-5 `#8584`, MH-6 `#8585`, MH-7 `#8586`,
MH-8 `#8587`, MH-9 `#8588`**, dispatched as the disjoint-path subagent
work packages in the analysis doc §12.3. Effect Native status refresh
folded in: upstream **Phase 2 is COMPLETE** (#10–#14 closed: Link,
responsive, forms, modal/sheet, virtualized lists), Phase 3 is 2/5
(#15 DevTools + #18 gallery closed), Phase 4 (#20–#43) open, and bug
**effect-native#44** (style-schema exactness, hit by the `/stage1`
consumer) is on the near-critical path. Owner gates filed to
NEEDS_OWNER: the X.ai auth plane per capacity host and weekly
free-window verification.

**Rev 6.3 changes (owner decision, 2026-07-08 night — the Pylon fold):**
the Pylon-into-Khala-Code proposal
(`2026-07-08-pylon-into-khala-code-proposal.md`) is **ACCEPTED**:
**Khala Code desktop is the primary human surface for everything Pylon
does**, on the daemon-cockpit model — typed `pylon-core` engine packages
(custody, executor, presence, wallet) behind a typed RPC contract; a
desktop-optional engine daemon carries 24/7 standing capacity; the
desktop's stdout-subprocess seam is deleted; one MCP surface (Khala
Code's). Gate resolutions: the name stays **Pylon** (npm
`@openagentsinc/pylon` continuity, no rebrand); the **OpenTUI surface is
retired**, gated on desktop cockpit-parity receipts; **the Spark wallet
is preserved as a live rail by owner mandate** (never a retirement
candidate — it stays in the engine daemon, surfaced read-only in the
desktop); the non-Spark earning/labor rails go to `#8577`'s Wave-4
ask-first list. Lanes filed: **PY-1 `#8578`** (pylon-core extraction +
daemon + typed RPC; runs after `#8577` waves 1–2), **PY-2 `#8579`**
(desktop cockpit parity — Go online toggle, accounts/runs/receipts —
authored Effect Native, rides EN-5 `#8574`), **PY-3 `#8580`** (TUI
retirement, gated on PY-2). Boundaries unchanged: org-cloud CX lanes,
server dispatch gates, token accounting, payments/credits, and the
`khala fleet connect` CLI front door are untouched; owner-local capacity
and org-cloud remain two additive rails meeting only at the shared
custody registry.

**Rev 6.2 changes (2026-07-08 evening — status refresh, no policy
change):** the status snapshot and review addendum are reconciled
against `origin/main` and live production: `#8569` pylon-stats is
repaired and CLOSED (live probe `available:true`); the docs/direction
cleanup `#8576` is CLOSED (RETIRED FOR NOW / SUPERSEDED / POSTPONED /
HISTORICAL banners landed, `docs/RETIRED.md` ledger live); **the
Tassadar/Psionic code-removal gate on `#8577` is LIFTED by owner
direction** — physical removal to the `backroom` archive is now
mandated execution work; **EN-1's first render receipt landed** (the
`/stage1` Effect Native staging route,
`2026-07-08-en-1-stage1-effect-native-receipt.md`); and OB-2's Apollo
wave-ingest path landed as code. The "Retired programs" subsection is
re-homed to the end of the snapshot (it had split the review addendum).

**Rev 6 changes (owner directive, 2026-07-08 later same day — FULL
CONVERSION, ASAP):** the **entire repository converts to Effect Native as
fast as the substrate allows.** Rev 5's pacing rule ("greenfield adopts
first, shipping code migrates on touch, nothing working is rewritten just
to move it; EN rides inside product work") is **revoked as pacing** — it
survives only as the safety floor: every conversion PR keeps its
surface's tests, QAM gates, and behavior contracts green, and nothing
destabilizes the P0 store-submission artifacts. Concretely: (1) §EN gains
a **conversion program** (waves CV0–CV5) run as dedicated parallel lanes
— conversion is first-class scheduled work, no longer a rider inside
product phases; (2) the previously deferred EN lane issues are **filed
now** — EN-2 #8572 (catalog demand), EN-4 #8573 (web absorption), EN-5
#8574 (Khala Code desktop conversion), EN-6 #8575 (canvas/Verse) — and
EN-3 #8568 is upgraded from migrate-on-touch to a **scheduled mobile
burn-down**; (3) Khala Code desktop's shell target is **Effect Native**
via the effect-native Phase 4 epic (effect-native#20, children #21–#43,
filed 2026-07-08: desktop adapter, canvas renderer, foreign-host `Host`
node, interaction expansion, theme, streaming binding, and the full
desktop component set) — **the earlier React+Tailwind shell-rewrite plan
is cancelled**; (4) discipline 5 and the repo agent contract (CLAUDE.md
UI clause) are amended in the same commit. Dependency order still binds:
the public `OpenAgentsInc/effect-native` repo is the substrate critical
path (Phase 2/3 issues #9–#19, Phase 4 epic #20–#43 gate the waves) —
**ASAP means maximal parallel lanes, never skipping the substrate.**
Non-UI systems (voice runtime, eve, services, brokers, runtimes) are
untouched throughout.

**Rev 5 changes (owner decision, 2026-07-08 — Effect Native):** every UI
surface in this repo converges on **Effect Native** — one typed
Effect-Schema component set with typed intents, an Effect v4 runtime, and
thin swappable renderers (see `../effect-native/` and the public framework
repo `OpenAgentsInc/effect-native`). This supersedes the 2026-07-04 ONE-UI
"React + Tailwind everywhere" clause **for new surfaces**; React/TanStack
Start and React Native remain as renderer adapters and serving hosts, never
the architecture. Concretely in this rev: (1) a new cross-phase **EN lane
family** (§EN below) binds the public effect-native ROADMAP phases 0–6 to
the product phases here, so each substrate piece is built exactly when it
unlocks the next product surface's refactor; (2) **WEB-1 (#8565) forward
work is rescoped** — the landed Phase 1 React replica (`/demo`, `/new`)
stands as the visual reference/baseline, and the production root-cutover
landing is authored in the Effect Native component set with launch-ui's
theme ported into tokens, not adopted as React components; (3) Sarah's UI
work (sarah#15 / S-10) folds into the Effect Native web renderer;
(4) cross-cutting discipline 5 is amended. The governing rule of this
rev — greenfield adopts first, shipping code migrates on touch, EN rides
inside product work — **is superseded as pacing by rev 6 above**; only
its safety floor survives. (Rev 4.4's status snapshot below is retained
unchanged.)

**Rev 2 changes (owner direction):** (1) P0's exit is strengthened from
"launch-ready" to **submitted to the app stores**; (2) **Sarah** — the AI
sales agent (`2026-07-07-sarah-sales-agent-spec.md`) — is inserted as
**Phase P1**, immediately after the MVP and ahead of Codex; (3) Sarah's web
surface ships as part of **the new openagents.com app on React/TanStack
Start** (the ONE-UI stack), which begins its route-by-route life in P1;
(4) all later phases renumber (Codex P1→P2, employees P2→P3, brain P3→P4,
templates P4→P5, trust P5→P6, scale P6→P7). Cross-referencing docs are
updated in the same commit.

Consolidates:

- Mobile MVP + Agent Computers substrate — epic #8467, AC-1 #8503
  (`2026-07-05-khala-code-mobile-only-mvp-launch-audit.md`,
  `../khala-code/2026-07-06-agent-computers-strategy.md`)
- The mobile testing system (Blueprint-modeled) — QAM-1..7
  (`../khala-code/2026-07-07-mobile-testing-audit-and-plan.md`)
- **Sarah, the sales agent — SR-1..6**
  (`2026-07-07-sarah-sales-agent-spec.md`)
- Codex on Agent Computers + post-MVP directions — CX-1..5
  (`2026-07-07-beyond-mvp-codex-agent-computers-and-ai-employees.md`)
- The horizon ladder H0–H6 and lane reconciliation
  (`2026-07-07-overarching-roadmap-khala-code-agent-computers-ai-employees.md`)
- AI employees / company brain phases — AE-x / CB-x
  (`../agenticsociety/2026-07-03-integration.md`)
- The product suite (mobile/desktop/web/Reactor)
  (`2026-07-07-product-suite-khala-code-openagents-com-reactor.md`)
- The sovereignty analysis deltas (Blueprint-lite brain, assurance-level
  vocabulary, corpus canaries, the 15-step assessment)
  (`2026-07-07-palantir-institutional-sovereignty-smb-analysis.md`)
- The web-stack decision: **one UI ecosystem — React + Tailwind on
  TanStack Start** for web (owner decision 2026-07-04,
  `2026-07-04-tanstack-start-sites-and-web-app-evaluation.md`; ONE-UI
  epic #8339 and TS-6 web app-shell migration #8348 reopen here) —
  **amended 2026-07-08 by the Effect Native decision** (§EN): the typed
  component set is the substrate; React/RN are renderer adapters
- **Effect Native — the UI substrate (§EN, EN-0..EN-9)**
  (`../effect-native/README.md` + the six 2026-07-08 docs; public
  framework repo `OpenAgentsInc/effect-native` with its ROADMAP
  phases 0–6)
- The standing lane families: ROADMAP.md (desktop/harness),
  ROADMAP_QA.md (QA engine), ROADMAP_BIZ.md (BF-*), ROADMAP_AFTER.md
  (AW-*), ROADMAP_BACKGROUND_AGENTS.md (BA-*), Reactor RX-*.

## Status snapshot (2026-07-08 evening, rev 6.2 code+issue audit)

Reconciled against `origin/main`, GitHub issue state, and live production
probes. **26 issues open, all ≥ #8467** — the rev 6 EN lane set (#8572–#8575)
and the #8577 removal-execution lane joined the backlog; `#8569` and `#8576`
closed. The backlog is entirely current-direction work: the Effect Native
conversion program, the Khala Code mobile P2 lanes, the outbound sales engine,
and the retired-program physical removal. What is genuinely DONE vs what is
only DIRECTION:

**Proven / shipped (closed, evidence on main):**
- **P0 substrate:** the Agent Computer foundation — `#8503` (first real
  Firecracker microVM turn on our GCE host: admitted turn → exact
  owner-attributed `token_usage_events` row → full reclaim lifecycle) and
  `#8477` (real branch **+ PR** writeback from inside the microVM under the
  owner's brokered GitHub identity) — both CLOSED and proven live. QAM-1..7
  (`#8536–#8542`) CLOSED; store artifacts `#8544` CLOSED (public review
  submissions deferred by owner decision).
- **Khala Sync realtime + credits (hardened 2026-07-08):** `#8554` (capture
  daemon moved to the `khala-capture` Cloud Run service — restored the dead
  ~32h live-tail pipe, 0 backlog), `#8555` (hosted mobile-chat exact per-turn
  credit metering), `#8556` (capture liveness alerting/auto-heal), `#8557`
  (credit-balance projection backfill skip-not-fail for legacy `email:` IDs),
  `#8564` (deploy-script cron-bearer leak) — ALL CLOSED. The credit-balance
  projection was backfilled (42 real users seeded). Live send→reply→**metered
  credit debit** proven end-to-end on a real Android device (turn recorded
  server-side with an exact `usage_truth=exact` row + a `paid` charge). Honest
  caveat: one Flash turn is sub-cent so it does not move a *whole-cent* display;
  any ≥1¢ change pushes live.
- **GCP migration (`#8515`) CLOSED** — MVP path fully on Cloud Run + Cloud SQL.
- **CX foundation:** **CX-1 (`#8545`) CLOSED** — the broker-only provider-
  credential law (`provider_credential_policy: broker_only`, never-pooled/
  never-resale, fail-closed placement, scanner coverage).
- **`#8569` pylon-stats production repair CLOSED** — `GET
  /api/public/pylon-stats` serves `available:true` live again (probed at
  this audit), unblocking WEB-1's live-counter acceptance.
- **Docs/direction cleanup `#8576` CLOSED** (commits `97b600f4d7` …
  `34c2352e1b`): the RETIRED FOR NOW / SUPERSEDED / POSTPONED /
  HISTORICAL banners are on every directory named by the cleanup audit,
  `docs/RETIRED.md` is the live ledger, and the fable indexes lead with
  this roadmap.
- **EN-1 first render receipt** (`02a28d49a5`,
  `2026-07-08-en-1-stage1-effect-native-receipt.md`): the `/stage1`
  route renders the public launch slice through the **Effect Native DOM
  renderer** on a Cloud Run staging service
  (`openagents-com-start-stage1`) with live public projections (tokens
  served, pylon stats, Khala Code plans) — using an app-local snapshot
  of effect-native `6dda1d44` while upstream packages are pre-alpha.
  `/`, `/new`, `/demo` untouched as baselines; upstream schema/style
  exactness bug filed as effect-native#44.

**Build-forward / owner-gated (open; partially landed as code):**
- **WEB-1 (`#8565`) sales landing — React/Tailwind split preview live on Cloud
  Run.** Commit `7899f3ec15` ports the Launch UI homepage replica into
  `apps/openagents.com/apps/start` as the reference milestone. The production
  Cloud Run monolith now serves exact React-rendered document routes before the
  legacy app shell: `/demo` preserves the original Launch UI replica for
  comparison, while `/new` adapts the same fold to the four-prong OpenAgents
  suite (Khala Code mobile, Khala Code desktop, openagents.com, Reactor).
  Verified at landing: React API route tests, API/Cloud Run typechecks, web
  route guards, Start typecheck/budget, and Google Cloud Run deploy smoke.
  **Rev 6.2 addition:** the EN-1 `/stage1` staging route (above) is the first
  Effect Native render of this surface — the forward path per the §EN rescope.
  Still open: EN/`/new` visual parity (icon/media primitives, hero — paced by
  the upstream catalog + effect-native#44), owner-approved root cutover, final
  sales-copy sign-off, credit-tier pricing wiring, production CTAs to business
  intake + Sarah, and root-route rollback notes. The `#8569` live-counter
  blocker is resolved.

**Direction only (open; NOT started as code):**
- **OB-1..6 (`#8558–#8563`) outbound engine — all OPEN; first code landed.**
  Sarah consolidates into `apps/sarah/` (rev 6.7, epic #8594). A pre-existing
  *operator-assisted* CRM/pipeline substrate exists in-repo
  (`business_pipeline_rows`, outreach/email plumbing), and **OB-2's Apollo
  wave-ingest path is now on main** (`6c7b9cdfe4`:
  `business-pipeline-queue.ts` + routes/tests — segment waves into
  `business_pipeline_rows`). Still to build: the wave runner at volume,
  audit-first personalization (OB-3), the draft→approve→send loop (OB-4),
  Stripe close (OB-5), the daily ledger (OB-6). Owner-gated: OB-1 (sending
  subdomain + DNS + Resend arming), OB-5 (Stripe keys).
- **CX-2..9 (`#8546–#8553`) "your own Codex in the cloud" — all OPEN,
  unstarted.** The proven `#8503` microVM turn runs the **hosted Khala gateway
  model (Gemini), not Codex**. **CX-3 (`#8547`) is the unstarted linchpin:** the
  in-guest turn-runner has no Codex path at all (no in-VM grant redemption, no
  `CODEX_HOME` on scratch, no `codex_app_server` lane). CX-3's control-plane half
  lives in the private `cloud/` repo, so it cannot be fully landed from here.
- **`#8543` (P0.8 launch readiness) OPEN** — blocked on the owner-gated seeded
  public-safe test account, the full dual-platform straight-line E2E (the
  chat→reply→credit half is now device-proven; the pick-repo→push→writeback half
  depends on CX-3), and the promises/copy pass with owner sign-off.
- **`#8467` (Khala Code Mobile MVP epic) OPEN** — the live multi-workstream P0
  program that `#8543` gates.

**Review addendum / recommended next work (updated rev 6.5, 2026-07-09
early morning):** MH-0 through MH-8 (except Grok's own MH-3/MH-4) are
CLOSED and independently re-verified. Current recommended order:

- **effect-native Phase 4 catalog (continuous, one sequential agent —
  file-collision constraint):** 12 of ~22 issues closed; the remaining
  queue (`#35`/`#36`/`#37` next, then `#42` proof, `#43` docs) stays a
  single serial lane against `packages/core/src/index.ts` — do not
  fan multiple agents across that file.
- **CX-3 `#8547` — resume implementation, not just assessment:** the
  private-repo blocker is resolved (`#8591` landed the crates in-repo) AND
  the control plane is now live in production from those crates (rev 6.6
  Phase 6 cutover, `546e3cf840`). Safe next slice: thread
  `provider_credential_policy: broker_only` into
  `crates/oa-node`/`crates/oa-codex-control` (source-only, no live infra).
  The genuine remaining wall, now pinned precisely: no source-controlled
  rootfs build script for the `agent-computer-gce-1` guest image, and
  in-VM `codex_app_server` wiring on that (separate, KVM-capable) host —
  see rev 6.6 for the control-plane-vs-execution-host distinction.
- **CX-5 `#8549` (Claude cloud parity):** ready — MH-2 already fixed
  the Claude-account SCM-credential false-positive tonight, so the
  broker/lane pattern CX-3 established has a proven Claude precedent to
  mirror.
- **MH-7 `#8586` (multi-harness cockpit) + EN-5 `#8574` first screen:**
  DONE for the cockpit screen (2026-07-08) — a `/stage1`-style first
  Effect Native render landed as a dev-only `en-cockpit.html` page in
  Khala Code desktop (chip strip + per-harness readiness rows + worker/run
  list + pause/resume/drain/stop + approvals), bound to the existing
  `KhalaCodeDesktopFleetStatus` data via a pure adapter, dispatching typed
  `@openagentsinc/khala-fleet-intents` values. Mounted via the DOM renderer
  (not `platform-desktop` — that adapter isn't in the vendored EN snapshot
  and `DesktopBridge` doesn't fit Electrobun's `Electroview` bridge; see
  `docs/fable/2026-07-08-mh7-en-cockpit-desktop-receipt.md`). Phase 4
  data-display/chip/tabs + shared `khalaTheme` were NOT yet in the vendored
  snapshot, so the cockpit composes from `Card`/`Text`/`Button` (like
  `/stage1`) and skips `GraphFigure`; swap for first-class components on the
  next re-vendor. EN-5's full desktop shell conversion remains its own scope.
- **EN-4 `#8573` (web absorption):** start the route inventory +
  burn-down table, then the first 1–2 real route conversions — the
  catalog is deep enough now that this is no longer blocked on Phase 2.
- **PY-1 `#8578` continuation:** the presence blocker has two
  documented unblock options (make `pylon-runtime` resolvable by name
  and verify the monorepo-install change; or inject the apple-fm
  functions the way presence already injects its probe) — try the
  injection option first, it's the lower-risk one.
- **CX-2/CX-4 `#8546`/`#8548`:** code-complete, correctly left open —
  closing needs a real human device-auth tap-through
  (`~/work/NEEDS_OWNER.md` has the exact steps); do not re-dispatch
  agent work here, this is an owner action.

- **CV0 substrate lanes (continuous, fleet-parallel):** effect-native
  Phase 0/1 (`#1–#8`) are CLOSED; work the open Phase 2/3 set
  (`#11–#19`) and the Phase 4 epic (`#20`, children `#21–#43`) as
  parallel lanes — this is the critical path for every conversion wave.
  Fix effect-native#44 (schema/style exactness) early — the `/stage1`
  consumer already depends on it.
- **CV1 next steps:** `/stage1` (EN-1 first render) is landed; drive it
  to `/new` visual parity (icon/media/hero primitives arrive via the
  upstream catalog per EN-2 #8572 demand), replace the app-local
  effect-native snapshot with the published packages when they
  stabilize, then walk the owner cutover gates on `#8565` (copy
  sign-off, pricing, CTAs, root flip + rollback notes). Start the EN-4
  `#8573` route inventory/burn-down table in parallel (inventory needs
  no new substrate).
- **Retired-program removal (#8577, gate lifted):** execute the
  archive-to-backroom physical removal per the PRUNE brief — heavy LOC
  reduction with archive-before-delete, production kept green. Hard
  guardrail: the Spark wallet modules are live-rail code, never removal
  candidates (rev 6.3).
- **Pylon fold (rev 6.3, accepted):** after #8577 waves 1–2 shrink
  `apps/pylon`, run PY-1 #8578 (pylon-core extraction, daemon, typed
  RPC, one MCP surface), then PY-2 #8579 (desktop cockpit parity as
  Effect Native surfaces, riding EN-5 #8574), then PY-3 #8580 (TUI
  retirement, gated on PY-2 receipts).
- **In parallel, non-EN:** the sales push (OB-1..6; OB-2 ingest is
  landed, continue to the wave runner + OB-3 reports) and P2 Codex
  lanes (CX-2..) continue unchanged — they are mostly backend; their UI
  surfaces are authored EN-native as their turn arrives.

**Bottom line:** the substrate is real and proven, and the first
production surface now renders through it. Most open non-EN work still
spans the private `cloud/` repo (CX-3) or owner gates (seeded account,
DNS/Resend, Stripe, prod arming, copy sign-offs). Under rev 6 the
conversion program itself is first-class scheduled work — the substrate
lanes, CV1 parity work, the retired-code removal, and the sales push
proceed at maximum parallel capacity.

### Retired programs (rev 6.1; execution status rev 6.2)

Tassadar/Psionic and their training, gym, inference, proof/replay, and
speculative compute-market satellites are retired from active OpenAgents and
archived to backroom at
`openagents-prune-20260708-tassadar-psionic` (backroom commit `a56fd270`).
Revival requires an explicit owner decision, with earliest reconsideration
after cashflow-positive. See
[`2026-07-08-repo-docs-direction-cleanup-audit.md`](2026-07-08-repo-docs-direction-cleanup-audit.md)
and [`../RETIRED.md`](../RETIRED.md) for the status ledger.

**Execution status (rev 6.2):** the docs retirement landed and `#8576` is
CLOSED. The `#8577` code inventory's "no action" gate is **lifted by owner
direction (2026-07-08)**: retired-program code and its archived docs move
physically out of this repo into the workspace archive repo
(`backroom/openagents-prune-20260708-tassadar-psionic/`), archive-before-delete
with paired commit hashes, waves per the PRUNE brief on `#8577` — whole
retired packages first, then surgical removal inside live apps (never touching
exact token accounting, live counters, promises evidence, or payments rails),
then a reference scrub. POSTPONED programs (Reactor, Verse/world, labor,
Nostr) are **not** deprecated and are removed only by a separate explicit
owner decision.

## 0. The one-page shape

```
P0  MVP: tested + SUBMITTED ──► P1  Sarah: inbound + ──► P2  Your Codex ──► P3  Standing
    (QAM gate + suites +            outbound sales          (CX-1..9 →        employees
     #8503/#8477 proofs +           (SR-1..3 + OB-1..6                        (AE-1)
     store submission)              sales engine)        dogfood cutover)         │
                                                                                 ▼
P7  Scale / GTM / suite ◄── P6  Trust layer ◄── P5  Templates & ◄── P4  Employees &
    (assessment, IAP,           (skills registry,    integrations        the brain
     pairing, Reactor            canaries, input     (AE-3, BI-1..5,     (AE-2..4, CB-1;
     tiers, network)             ceiling)            SR-6 template)      generalizes Sarah)
```

Testing is not a phase that ends: the P0 gate and feature-ladder
discipline apply to **every** phase after it — a lane's exit receipts
include its Eval Suite green at the target ladder rungs. Sarah's suite
(SR) is authored fixture-first under the same law.

Running underneath every phase from P1 onward is the **Effect Native
substrate track** (§EN). As of rev 6 this is a **full-conversion
program run ASAP**: the substrate is the critical path (CV0), and each
piece unlocks a conversion wave the moment it lands — no longer
just-in-time pacing:

```
EN  Phase 0 core + DOM renderer ──► RN adapter ──► catalog growth ──► desktop + canvas ──► native Swift/Compose
    (inside P1: WEB-1 + Sarah UI)   (post-P0:      (P4 cockpit/        (Khala Code desk-    (P5+ fidelity,
                                     mobile on      dashboard;          top panels; Verse    value-gated,
                                     touch)         web absorption)     under contract)      per component)
```

## EN — the Effect Native substrate (cross-phase lane family)

**The decision (owner, 2026-07-08):** all UI in this repo converges on
**Effect Native** — a closed, versioned Effect-Schema component catalog
with a typed intent algebra (interactions as data, never closures), an
Effect v4 runtime that binds data and dispatches intents as Effect
programs, typed style values on one token set (StyleX-model +
Tailwind-derived tokens; no class strings in any contract), and thin
per-platform renderers as the *only* platform-specific UI code. Design
docs: `../effect-native/` (read the framing doc first, then the
UI-layer analysis with the internal EN-0..EN-9 phases). The framework
itself is built in the open in the **public repo
`OpenAgentsInc/effect-native`** (workspace sibling
`~/work/effect-native`; MIT; keep it public-safe — no internal
codenames); its public ROADMAP phases 0–6 are the substrate build
order, and this section binds them to the product phases.

**Issue index (updated rev 6, 2026-07-08):** epic **#8566**; lanes
**EN-1 #8567** (production landing on the DOM renderer), **EN-2 #8572**
(catalog demand register + version adoption), **EN-3 #8568** (RN adapter
#1 — proved; superseded as pacing by MB-EN below), **MB-EN #8597** (rev
6.9 — full Khala mobile app rewrite on Effect Native, iOS + Android;
pulled by upstream mobile epic **effect-native#52** children #53–#65;
cross-app Khala Sync messaging exit test),
**EN-4 #8573** (web absorption route-by-route, legacy deleted), **EN-5
#8574** (Khala Code desktop conversion; cancels the React+Tailwind shell
rewrite), **EN-6 #8575** (canvas/Verse under the canvas contract; Foldkit
adapter retired), plus the deploy lanes **#8570** (component gallery on
Cloud Run) and **#8571** (effectnative.org hosting). Substrate in the
public repo: **#1–#8** (Phase 0 core + Phase 1 renderers/proof — CLOSED),
**#9–#14** (Phase 2 catalog growth: process, Link, responsive, forms,
modal/sheet, virtualized lists), **#15–#19** (Phase 3 DX: DevTools,
testkit, guide, gallery, effectnative.org), and the **Phase 4 epic #20
with children #21–#43** (desktop adapter, canvas renderer, foreign-host
`Host` node, interaction expansion, Protoss-blue theme, streaming
binding, the full desktop component set, the port proof, docs). WEB-1
#8565 carries the rescope comment and keeps the owner cutover gates.
Only EN-7 (native Swift/Compose fidelity) remains deferred — it is
value-gated per component and its substrate (Phase 5) does not exist yet.

**The unlock chain — build this substrate piece, unlock that product
refactor.** Dependency order remains the law for EN work. **Pacing is
rev 6's conversion program below** — the "rides inside" column records
rev 5's original just-in-time pacing and no longer gates when a wave
starts; a wave starts the moment its substrate dependency lands.

| # | Effect Native piece (public repo phase) | Internal lane | Unlocks (the next OpenAgents surface on the substrate) | Rides inside |
|---|---|---|---|---|
| 1 | **Phase 0 — the core**: catalog v0 (~8 components), typed intent algebra, Effect runtime, `@effect-native/tokens` (Protoss blue; launch-ui's Tailwind theme — already vendored in the WEB-1 Phase 1 replica — ported as typed token *values*) | EN-0 | Everything below — gates all adoption; exit = one screen, two renderers, snapshot-tested | P1 Track A (a focused sprint on WEB-1's critical path) |
| 2 | **Phase 1 — DOM renderer** (no React) | EN-0/EN-1 | **WEB-1 #8565 forward work**: the production root-cutover landing is the first Effect Native surface (the landed React replica at `/demo`/`/new` stays as the visual baseline it's compared against); then **Sarah's branded UI** (SM-2 #8594, was sarah#15 / S-10) — the `/sarah` surface and the landing are one component system *by construction* (rev 6.8: interim DOM shell shipped; EN authoring open) | P1 Track A / SR-0.5 |
| 3 | **Phase 1 — RN renderer** (wrapping the ~94 shipping khala-mobile primitives as adapter #1; zero new native work) | EN-0/EN-3 | New/changed mobile screens author the component set from then on — the P2 surfaces (CX-2 accounts UI, CX-4 harness pill) are the first candidates; existing screens convert on the **scheduled burn-down** (rev 6 — was on-touch), with the safety floor that the launch straight line and store artifacts stay green through every PR | P2 (CX-2/CX-4) |
| 4 | **Phase 2 — catalog growth**: forms/validation, virtualized + section lists, modals/sheets/tabs, images/media, **typed navigation intent**, typed variants (state/platform/breakpoint) | EN-2 | **P4 phone-cockpit web twin + Agents panel (AE-2.3)** and the business-dashboard build-out; the deeper mobile migration; demand-driven — a component enters the catalog when one of these screens needs it | P4 (AE-2.3) |
| 5 | **Phase 3 — DX**: DevTools (view-tree inspection, intent log/replay, time-travel), deterministic snapshot + intent-driven testing, visual baselines per renderer | EN-2/EN-9 | The QAM discipline extended natively to Effect Native surfaces; **agent-authored UI validated against the catalog by construction** (the substrate's whole point at 1000 edits/day) | parallel with #4 |
| 6 | **Phase 4 — desktop + canvas** (epic effect-native#20, children #21–#43, filed 2026-07-08): desktop adapter (Electrobun consumes the DOM renderer), `@effect-native/render-canvas` (three-effect's ~600-LOC reconciler/frame-clock/scope kernel reimplemented on Effect `Scope`/`Stream`/`Layer`; the 24k-LOC VFX/Verse domain library stays standalone as the catalogue), the foreign-host `Host` node (Monaco/terminal), interaction expansion, theme, streaming binding, and the full desktop component set with the Khala-chat port proof (#42) | EN-5/EN-6 | **Khala Code desktop — full shell + panel conversion** (EN-5 #8574; the React+Tailwind shell rewrite is cancelled); **Verse/3D surfaces under the same contract** (EN-6 #8575); the Foldkit three-effect adapter retired | after web is solid (P4+) |
| 7 | **Phase 5 — native Swift/Compose renderers**, per component, RN as the long-tail fallback | EN-7 | Fidelity/perf upgrades on the highest-value mobile components — a migration, never a rewrite, because the contract is renderer-agnostic | value-gated, multi-quarter (P5+) |
| 8 | **Phase 6 + governance**: server-driven-UI option, lint boundary | EN-8/EN-9 | Steady state: **new UI authors the component set only**; direct React/RN/DOM authoring outside renderer adapters is deprecated and lint-guarded | continuous once #2–#3 land |

**Full-conversion scope (every part of this repo, per the analysis
doc's EN-0..EN-9):** the openagents.com landing (WEB-1) and Sarah UI
(greenfield — first); the web product routes — dashboard, Forum, Sites,
operator/Aiur, Autopilot — absorbed route-by-route from legacy Foldkit
and interim React onto the DOM renderer, deleting legacy surfaces as
replaced (EN-4 #8573); the mobile app's ~94 `.tsx` screens (EN-3 #8568,
scheduled burn-down); Khala Code desktop (EN-5 #8574); canvas/Verse
(EN-6 #8575). **Definition of done:** every UI surface renders from the
one component set; renderer adapters are the only platform-specific UI
code in the tree; a bad UI change fails at the schema boundary.

**The conversion program (rev 6 — supersedes rev 5's sequencing
discipline as pacing).** Conversion is the program, not a rider. Waves
run as dedicated parallel fleet lanes; a wave starts the moment its
substrate dependency lands in the public repo; every conversion PR lands
green (tests, QAM gates, behavior contracts, `check:deploy`) and
**deletes the legacy surface it replaces** in the same change:

- **CV0 — substrate (continuous; the critical path):** the public repo's
  Phase 2/3 issues (#11–#19) and the Phase 4 epic (#20–#43), worked as
  parallel lanes with maximal fleet concurrency; internal demand routes
  through EN-2 #8572 into `GAPS.md` under the growth rule.
- **CV1 — web greenfield (unblocked NOW; DOM renderer shipped):** the
  WEB-1 root landing authored in EN (EN-1 #8567), Sarah's branded UI
  (SM-2 #8594, was sarah#15 / S-10), the component gallery deploy (#8570), and
  effectnative.org (#8571).
- **CV2 — web absorption (starts as Phase 2 catalog pieces land):**
  every product route converted route-by-route on the EN-4 #8573
  burn-down table, legacy Foldkit/interim React deleted as replaced;
  the P4 cockpit web twin and business dashboard are authored EN-native
  when they arrive.
- **CV3 — mobile full rewrite (MB-EN #8597; rev 6.9 — upgraded from
  burn-down to a full rewrite program).** EN-3 #8568 proved adapter #1
  (vendored `render-rn` + the about-effect-native screen); MB-EN #8597
  now rewrites **the entire Khala mobile app** on Effect Native (iOS +
  Android), pulled by the upstream mobile epic **effect-native#52** (the
  peer of the Phase 4 desktop epic #20): the RN renderer parity program
  (#53, closing the desktop-first declared-subset matrix),
  `@effect-native/platform-mobile` (#54), navigation/gesture/virtualization
  pillars (#55–#57), RN foreign-`Host` drivers (#58), RN pixel baselines
  (#59), and the mobile catalog (#60–#63). Convert screen-by-screen after
  the vendor re-bump (`v5→v20`+ via the #8595 guard), deleting each legacy
  RN/khala-primitive screen on cutover. Safety floor: `qa:mobile:gate`
  (typecheck + depcruise + tests incl. `src/contracts/ux-contracts.ts`
  behavior contracts + generator conformance), QAM visual baselines,
  Maestro e2e, the P0 launch straight line, and the store-submission
  artifacts (#8543/#8544) stay green through **every** PR; native STT +
  Apple FM modules and the owned OTA layer are hosts/services, untouched.
  **Exit test (owner-named):** a message travels **desktop↔mobile live over
  Khala Sync** with both UIs rendered by Effect Native (effect-native#64).
- **CV4 — desktop + canvas (starts as effect-native Phase 4 lanes
  land):** Khala Code desktop shell + panels (EN-5 #8574) on the desktop
  adapter, Monaco/terminal via the `Host` node, fleet/gym graphs and
  Verse under the canvas contract (EN-6 #8575); the Foldkit three-effect
  adapter retired; the legacy desktop shell stays buildable until the
  owner-gated cutover.
- **CV5 — native fidelity (the one demand-paced wave):** Swift/Compose
  per component (EN-7), value-gated; unchanged from rev 5.

Governance is immediate, not phased: from rev 6, new UI anywhere in the
repo authors the component set wherever a renderer exists for that
surface; direct React/RN/DOM authoring outside renderer adapters is
deprecated now and lint-guarded as EN-8/EN-9 land. Non-UI systems — the
voice runtime, eve, services, brokers, runtimes, sync — are not UI and
are untouched throughout.

## MH — multi-harness parallelization (cross-phase, now-priority; rev 6.4)

**The program:** Khala Code mobile + desktop steer parallel coding
agents across `codex | claude | grok | auto`, on one claim registry, one
Sync plane, one credits/token ledger, one neutral chat-event model
(`khala.chat_turn_event.v1`), and **one typed intent/mutator vocabulary**
shared by Effect Native UI and Khala Sync. Content authorities: the
Fable↔Grok dialogue
(`2026-07-08-multi-harness-parallelization-effect-native-analysis.md`,
consensus closed at §12) and `docs/grok/`.

**Lane index (filed 2026-07-08):**

| Lane | Issue | Scope | Owner |
|---|---|---|---|
| MH-0 | `#8581` | Wave 0 contracts — **SERIAL critical path**; additive only; `grok_cli` literals, workerKind enum, `khala.chat_turn_event.v1`, shared intent/mutator package, opaque `sessionRef`+capabilities, `marginal_cost_class` | main fleet (one agent) |
| MH-1 | `#8582` | Enum-driven harness conformance suite (red-until-proven; rate-limit failure fixtures) | main fleet |
| MH-2 | `#8583` | Claude Axis A/B parity to 100% — the fastest real two-harness fleet | main fleet |
| MH-3 | *(Grok files)* | `GrokAcpChatRuntime` — Axis A, mock-ACP fixture first | **Grok lane** |
| MH-4 | *(Grok files)* | Grok worker executor in the `pylon-core` boundary + RL-1..6 rate-limit probes + plane labeling | **Grok lane** |
| MH-5 | `#8584` | Mixed-kind FleetRun under one claim registry (simulated; zero collisions ≥3) | main fleet |
| MH-6 | `#8585` | Sync fleet projections + the three mutators as MH-0 intents; phone pause/approve/steer dogfood | main fleet |
| MH-7 | `#8586` | The one cockpit (merges into PY-2 `#8579`); EN chrome; data-first exceptions | main fleet |
| MH-8 | `#8587` | Typed dumb `auto` v1 + per-harness economics (`marginal_cost_class`) | main fleet |
| MH-9 | `#8588` | Cloud parity for Grok/Claude workers — **strictly after CX-3 `#8547`** | held |

**Dispatch shape (analysis doc §12.3):** WP-A (MH-0) runs alone; then
Batch 1 in parallel on disjoint mutable paths — MH-1, MH-2, MH-5, Grok's
MH-3/MH-4, upstream effect-native#44, and the EN-2 demand rows; Batch 2
(MH-6/7/8) follows on Batch-1 partials; MH-9 held for CX-3. One claim
per lane; no two agents on `agent-runtime-schema` simultaneously; every
PR green under the safety floor.

**Standing rules:** CX-3 keeps dedicated capacity throughout; sales
(OB-*) capacity is separate; free-Grok burn only under measured RL
ceilings with honest metering (`not_measured` over invented tokens);
harness-internal auto-approve only when the *product* approval posture
grants it (one authority surface — the Inbox); public multi-agent copy
stays promise-gated until live receipts exist.

## P0 — Khala Code mobile MVP: fully tested, submitted to the stores

Goal: the MVP straight line (GitHub sign-in → repo → cloud turn → live
updates → push → writeback → credits) **proven end-to-end, held green by
the typed testing system, and submitted to the App Store and Play
Store.** Testing spec:
`../khala-code/2026-07-07-mobile-testing-audit-and-plan.md`.

**P0 STATUS (2026-07-07 night): largely COMPLETE.** #8503 and #8477
CLOSED (the microVM proof landed); QAM-1..7 all CLOSED (#8536–#8542);
P0.9 #8544 CLOSED with an owner decision recorded on the issue: the P0
exit is the **shippable/testable submission artifacts** — TestFlight
build 20 VALID with a reviewer demo-login, installable Android release
APK + AAB — while the actual public store *review submissions* are
deferred to broad-release readiness (listing metadata/screenshots, Play
app record + upload keystore, promise/copy green sign-offs). Remaining
open: **#8543** (launch readiness — owner-gated seeded test account →
unattended straight-line E2E, promises/copy pass) and the epic #8467.

P0 issue index: QAM-1..7 = #8536 #8537 #8538 #8539 #8540 #8541 #8542
(closed); launch readiness #8543 (OPEN); store artifacts #8544 (closed,
review submissions deferred); proofs #8503, #8477 (closed) under epic
#8467.

Order inside P0 (testing starts immediately; nothing waits on infra):

- **P0.1 = QAM-1 (#8536)** The gate. `qa:mobile:gate` (static → units → mounts →
  contracts → generator conformance → fixture tier), typecheck/depcruise
  promoted to blocking, pre-push wiring.
- **P0.2 = QAM-2 (#8537)** Mount debt + fixture suites. Thread-list,
  thread-messages, credits-history, settings mounted (or typed waivers);
  the **agent-computer streaming fixture suite** (runtime events →
  thread UI, typed refusals, writeback card).
- **P0.3 = QAM-3 (#8538)** Generators as the enforced path (screen gen emits
  mount test + stories + contract stub + flow stub + visual
  registration; conformance policy test).
- **P0.4** MVP proofs (existing scope, unchanged; in flight with the
  other agent): the full #8503 receipt bundle verified and the issue
  closed; #8477 writeback E2E; Aiur #8500/#8501 (closed 2026-07-07).
- **P0.5 = QAM-4 (#8539)** Visual tier: story screenshots + screen checkpoints
  into `openagents.khala_visual_baselines.v1`; blessing workflow.
- **P0.6 = QAM-5 (#8540)** Nightly mobile row on an owned Mac (Maestro flows,
  device monkey, visual capture, perf budgets, seam probes) reporting
  into the QA nightly/status/strict-issue discipline; mobile nodes on
  the QA Swarm board.
- **P0.7 = QAM-6 (#8541)** Android lane (emulator boot proof, flows, capture
  parity).
- **P0.8 (#8543)** Launch readiness: seeded test account (owner-gated, R4) →
  unattended straight-line E2E green
  (`khala_mobile.platform.launched_app_interaction_smoke.v1` enforced);
  promises/copy pass.
- **P0.9 (#8544) Store submission (the exit).** App Store submission executed
  (build uploaded, metadata/screenshots final, review answers prepared,
  account-deletion + 3.1.1 compliance verified — #8483/#8502/#8491
  packs current) and Play submission executed through the Play lane
  (#8490 owner actions). "Submitted" means in review at both stores —
  approval timing is theirs; ours is the submission receipt.

**P0 exit receipts:** the verified #8503 proof bundle; the release gate
refusing an unbundled screen; 7 consecutive nightly mobile receipts
incl. one auto-filed strict issue; every MVP feature at its target
ladder rung; straight-line E2E green on iOS + Android emulator; **both
store submissions recorded** (submission IDs + review states in the
registry evidence); QAM-7 (#8542) fixture-first suites authored (red/waived) for
every P1+ feature named below.

## P1 — Sarah: inbound + OUTBOUND sales (in-repo `apps/sarah` + the LG engine)

**P1 REPO/STACK UPDATE (2026-07-09, owner directive — rev 6.7):** the
separate-repo posture below is **reversed**. Sarah consolidates into
this monorepo as `apps/sarah/` — Bun/Effect service, **Effect Native UI**
on the DOM renderer, the eve brain retained only through the move and
replaced by an owned Effect agent runtime (converging with
`agent_definition.v1` / P4 `ai_employee.v1`, since Sarah is the
generalization seed), email folded onto the one approval-gated CRM send
rail, Cloud Run serving from monorepo-built images, and
`OpenAgentsInc/sarah` retired read-only historical at cutover. Plan:
`2026-07-09-sarah-monorepo-effect-native-consolidation-plan.md`; epic
**#8594** (SM-0..6). sarah#14/#15 are superseded; open sarah S-lane
scope absorbs into SM. The two paragraphs below are retained as the
decision history they are.

**P1 STATUS (2026-07-07 night, owner decision — repo posture SUPERSEDED
by rev 6.7 above):** Sarah lives in a
**separate private repo, `OpenAgentsInc/sarah`**, built on the standard
Vercel + AI SDK stack (Next.js 16, `ai`/`@ai-sdk/react`/
`@ai-sdk/gateway` canary realtime), deploying to
**sarah.openagents.com** — so the newest voice-agent surface works
unmodified before any monorepo integration; a later merge into the
monorepo is possible but not assumed.

**Hosting update (2026-07-08, owner decision — the sarah#14/#15 filings
here are SUPERSEDED by rev 6.7: still Cloud Run, but via the SM-1
Bun/Effect port-as-move and the SM-2 Effect Native UI, #8594):** the
Vercel deployment
was a bootstrap, not the home. Sarah moves to **our own infrastructure
(Cloud Run)** — eve officially self-hosts (`eve build && eve start`,
workflow state on `@workflow/world-postgres` against our Cloud SQL;
its deploy guide's "Deploy without Vercel" checklist is the spec), the
Next app lifts-and-shifts as a container, and the realtime WebSocket is
browser↔provider direct so no server-side WS infra is needed. Filed as
**sarah#14** (Cloud Run migration; the AI Gateway key may remain
short-term as the one Vercel service, with its removal path recorded)
and **sarah#15** (front-end ported to **TanStack Start** with/before
S-10's branded UI, per the ONE-UI standard — eve untouched). Sarah
selling sovereignty runs on sovereign infrastructure. **The realtime voice loop already
works locally** (server-minted gateway token for `openai/gpt-realtime-2`,
browser `useRealtime` with mic capture/server VAD/playback/text send) —
which **pulls SR-4 voice to the front**: v1 Sarah is voice+text from day
one, inverting the spec's original deferral. The monorepo remains the
system of record (CRM, credits, checkout, receipts, promise registry);
the sarah repo calls its APIs and never re-implements them.

**Track A (revised 2026-07-08, HIGH PRIORITY) — the new openagents.com
sales landing, built from launch-ui.** **STATUS (2026-07-08): REACT/TAILWIND
PREVIEW PAIR LIVE ON CLOUD RUN** — commit `7899f3ec15` ports the exact Launch
UI homepage replica into the TanStack Start app at
`apps/openagents.com/apps/start`, including the vendored Launch UI
components/sections, blue glow + minimal radius token set, dark-only shell,
placeholder dashboard assets, MIT attribution, route tests, and Start funnel
budget. The production Cloud Run monolith now serves exact React-rendered
document routes ahead of the legacy app shell: `/demo` keeps the original Launch
UI replica, and `/new` adapts that fold to the OpenAgents suite (Khala Code
mobile, Khala Code desktop, openagents.com, Reactor). Owner copy sign-off, live
public counters, credit-tier pricing, production CTAs, and owner-approved root
cutover remain. Owner directive: switch the
openagents.com landing/marketing site from this replica milestone to a
sales-focused site built from
**launch-ui** (`projects/repos/launch-ui` — MIT; shadcn/ui + Tailwind 4 +
React 19; a complete landing kit: hero/navbar/pricing/stats/logos/items/
faq/cta/footer; only **4 files** import `next/*`, so the TanStack Start
port is link/image swaps, not a rewrite). Phase 1 establishes the
**TanStack Start openagents.com app**: launch-ui components themed to
Protoss blue (one theme — next-themes/mode-toggle dropped), the stats
section wired to the LIVE public counters, pricing wired to the credit
tiers, CTAs → business intake + **"Talk to Sarah"**, copy through the
promise-registry gates with owner sign-off before the switch. The new landing
serves through the production Cloud Run monolith; existing product routes stay
on their current Cloud Run monolith paths until absorbed route-by-route. Filed as
**#8565**. Synergy: sarah#15's TanStack port shares the same
launch-ui/shadcn base, so the Sarah surface and openagents.com become
one component system. (Rev 6.8: Sarah's home is now
**openagents.com/sarah** in this repo — no subdomain; the dashboard
shell still lands with P4.)

**Track A rescope (2026-07-08, Effect Native — §EN):** the landed
Phase 1 React replica is a legitimate milestone and **stays as the
visual reference/baseline** (`/demo`, `/new` untouched), but it is not
the forward path. The **production root-cutover landing is authored in
the Effect Native component set** rendered by the DOM renderer (§EN
unlocks #1–#2, which gate this track and are built inside it):
launch-ui's theme and section designs — already vendored and themed in
the replica — port into `@effect-native/tokens` and catalog components;
zero further React components are authored for the landing. The
TanStack Start app remains the serving shell/host (routing, SSR entry)
— a host, not the architecture. Everything else stands: owner copy
sign-off, live counters, credit-tier pricing, production CTAs, and
owner-approved root cutover remain the open items, now landing on the
Effect Native surface. sarah#15 is superseded by SM-2 #8594: Sarah's UI is authored in the
Effect Native web renderer in-repo (same catalog, same tokens) — the
one-component-system synergy becomes true by construction. Rescope to
be recorded on #8565.

**Track A′ — SR-0, deploy readiness (the near-term work list, in
order):**

1. Persona + honesty grounding: Sarah instructions (AI disclosure,
   one-question-at-a-time, sales posture) via session config;
   promise-registry state fetched server-side into the instructions.
2. Token-route hardening before public deploy: origin/rate limits,
   session caps + TTLs, gateway spend alerts (today it mints client
   secrets unauthenticated).
3. Durable sessions + opaque prospect ref; transcript persistence;
   CRM summary sync over the monorepo API.
4. First tools on the realtime tool channel: `human_handoff`, intake
   capture → business-pipeline API, pack-priced checkout link.
5. Branded Sarah UI (Protoss blue, disclosure, mic states, text
   fallback) replacing the quickstart surface.
6. Production wiring: the `/sarah` mount on the production Cloud Run
   monolith (no DNS needed — rev 6.8 serving amendment) + env + model
   pin + cost caps (owner/infra actions → NEEDS_OWNER as they arise) —
   **SM-5 #8594 (was sarah#14)**; the interim Vercel deploy is
   decommissioned at cutover.
7. The Sarah Eval Suite (authored under QAM-7 #8542) pointed at the
   deployment; discount-pressure/honesty/injection probes green.

**Framework — eve (DECIDED owner 2026-07-07; rev 6.7: retained only
through the SM-1..3 move, then replaced by the owned Effect agent
runtime in SM-4 #8594, S-12 suite as the conformance oracle):**
Sarah's brain is
vercel/eve (filesystem-first durable agents; reference clone
`projects/repos/eve`) — durable sessions, `instructions.md` persona,
typed tools, cron schedules, and channels including the Chat SDK
**Resend email adapter** (SR-3's inbound/outbound email continuity) and
Twilio (future phone lane), same Vercel deploy target. It is **not** a
voice runtime — the realtime loop stays as built; realtime = voice I/O,
eve = sessions/tools/email/schedules, realtime tool calls execute
against eve. Setup work is filed as **OpenAgentsInc/sarah issues
S-1..S-13** (#1–#13): eve integration, honesty grounding, token
hardening (blocks public deploy), durable sessions/prospect ref, tool
bridge, first tools, CRM sync, email channel, deal rules v0, branded
UI, Vercel/DNS wiring, eval suite, schedules/receipts.

**Track B — Sarah lanes (from the spec; SR-1 partially underway in the
sarah repo):**

- **SR-1** Sarah v1 (text, on-site): durable sessions + prospect refs;
  persona program (public-scoped Artanis pattern); qualification on the
  shipped intake spine; registry-bound honesty; component channel on;
  `human_handoff`; pack-priced `credit_kickoff` checkout; behavior
  contracts registered; Eval Suite green at the fixture tier.
- **SR-2** Deal engine + checkout tool: `sarah.deal_rules.v1` (rate
  card owner-signed; volume tiers + Bitcoin stack imported; bundle
  rules; `close_on_call` tactic armed or parked);
  `sales.checkout_link.create` (arbitrary amount ≤ cap, split support,
  Lightning, honest TTLs); `quote_card`/`deal_summary` components.
- **SR-3** Email + CRM continuity: Sarah's mailbox; inbound routing →
  `event_ledger` **email source** → `inbox_match`; prospect↔contact
  binding; approval-gated continuation replies; one relationship
  thread across web and email.

**Resequenced within the Sarah family:** SR-4 voice is **pulled to the
front** (the realtime loop already works; voice ships with v1). SR-5
contracts + custom bundles land with P5's template work; SR-6
Sarah-as-template stays P5, under the catalog gate.

**Track C — the outbound sales engine (OB-1..6; owner directive
2026-07-08: "sales sales sales — start getting targeted outreach to
people NOW").** Sarah does outbound too. The substrate is **already
built and closed** — the SELL epic (#8261, LG-1..9 all closed) shipped
the agent-readiness prober (`packages/agent-readiness`), the pipeline
queue + commitment ledger (`business_pipeline_rows`), receipted starter
credits, **approval-gated suppression-aware sequence tooling**
(`email-sequence-authoring.ts` / `email-sequence-send-service.ts`), the
prospect report renderer, source attribution, the standing drafting-only
lead-gen definition (send denied without an approval receipt — enforced
contracts `lead_gen_agent.drafting_only_toolset.v1` /
`no_send_without_approval_receipt.v1`), and affiliate/partner
bookkeeping. Apollo.io is connected via MCP (owner OAuth) with the
audit-first plan and segments written
(`2026-07-03-apollo-outbound-sales-plan.md`; blitz §5.3 clone-segments;
own-your-ai target list). Track C connects that engine to Sarah's
identity and turns the volume knob. Issues filed: **#8558–#8563**.

- **OB-1 (#8558) Sending identity + deliverability foundation.** Sarah's
  outbound address on a **dedicated sending subdomain** (protect the
  root domain's reputation) with SPF/DKIM/DMARC; Resend arming
  (`CRM_RESEND_SEND_ENABLED` + keys — owner gate); CAN-SPAM
  identification + working opt-out on every send; suppression list
  live; **warm-up ramp** to the 100/day target (field discipline says
  10–20/day/identity sub-spam at the start — ramp gated on
  bounce/complaint rates, and add identities rather than burning one).
- **OB-2 (#8559) Apollo sourcing at volume.** Segment waves from the written
  plans (mastermind clone-segments, own-your-ai sovereignty targets,
  legal/agency verticals) enriched through the Apollo MCP into
  `business_pipeline_rows` with LG-6 source attribution; Apollo stays a
  mirror — our pipeline is the system of record (BF-9.2 law).
- **OB-3 (#8560) Audit-first personalization at fleet scale.** Every email
  leads with the prospect's own agent-readiness report (LG-1/LG-5),
  upgraded with the 15-step assessment rubric (P7.1 pulled forward as
  content); fleet lane renders one report per prospect — no generic
  blasts, ever (compliance-guardrails law: value before ask).
- **OB-4 (#8561) The draft→approve→send loop at 100/day.** Sarah drafts
  sequences into the **existing CRM approval queue** (propose →
  operator approve → `dispatchCrmSend`) — the owner's "queue drafts,
  send manually" is already the enforced law; what's new is a **batch
  approval UX** (review/edit/approve a day's queue in minutes, one-tap
  per batch with receipts) so 100/day is operable by one human.
  Replies route to Sarah's inbox (sarah repo S-8 email channel) and the
  CRM; every send/reply is a `crm_activity` + email-ledger row.
- **OB-5 (#8562) Close via Stripe.** Reply → Sarah conversation (email or a
  link into openagents.com/sarah) → qualification → checkout link
  (pack-priced now; deal-rules quotes when SR-2 lands) → settled
  Stripe receipt → provision; pipeline states tracked on
  `crm_opportunities`; funnel counters per LG-6 attribution.
- **OB-6 (#8563) The daily sales ledger.** Sent / delivered / replies /
  report-clicks / conversations / quotes / closes per day, per segment,
  with deliverability health (bounce/complaint) gating the ramp;
  surfaced to the owner daily (ops view + a one-line digest). The
  agency-trap discipline applies: operator-minutes per send must fall
  as volume rises — the batch UX and Sarah's drafting quality are the
  levers.

Authority posture (unchanged law): outbound send remains
**approval-gated** — the existing contracts stay enforced; promotion of
any send class to policy-bound is a later, receipted owner decision.
Owner gates for Track C: Resend/domain arming + DNS, the sending
subdomain choice, daily-cap ramp sign-offs, and pricing on anything
Sarah quotes beyond existing packs.

**OAV quality program (epic #8610; lanes OAV-1 #8611, OAV-2 #8612,
OAV-3 #8613; OAV-4 seam #8614 landed):** the owned avatar renderer's
"beautiful audio and video" bar is run as an experiment program, not
taste. Authority: `docs/sarah/research.md` (the owner's prioritized
triage — P0: LatentSync 1.6 / FLAIR / RIFE / CosyVoice2/3 / MuseTalk
tuning / BasicVSR++/RealBasicVSR / tamed-only GFPGAN; P1: Hallo2/3
openers; blocked: KEEP/PGTFormer/CodeFormer et al. — plus the A0–F2
matrix) + `docs/sarah/2026-07-09-oav-quality-strategy.md` (adopted plan
and measurements) + spec §9 (binding enhancement policy — full-strength
per-frame GFPGAN banned as a default enhancer). Empirical state: the
tamed-GFPGAN v3 recipe measured smoothest (jerk 0.15); LatentSync 1.6
is the articulation winner with a 16-frame chunk-boundary hitch to
smooth; the opener library v1 is rendered both ways awaiting QA;
FLAIR/RIFE/CosyVoice-prosody/Hallo2 experiments are queued. Reference
clones live in the workspace `projects/repos/`; papers in
`projects/papers/`.

**Sarah Blueprint Map surface (epic #8626; lanes BM-1 #8627, BM-2
#8628, BM-3 #8629, BM-4 #8630, BM-5 #8631; rev 6.13):** the live
per-prospect Blueprint map rendered while Sarah talks — what she knows
about this prospect and what she is still trying to learn, plus which
offerings the stated needs map onto. Authority:
`docs/sarah/2026-07-09-blueprint-map-surface-audit.md` (Unit/Arbiter
findings, data inventory, page keep/cut verdicts, layout spec,
streaming architecture). **Status after rev 6.14:** BM-1 is landed via
PR #8632 plus `7838a9a1ca`, BM-2 is landed in `9ee02315f5`, BM-3 is
landed in `4fb1ec6ba7`, and BM-4 is landed in `dc0c67948c`. The
implementation now has typed `blueprint_delta` events on the existing
avatar SSE bus, an Effect Native `GraphFigure` Blueprint map seeded
from the stored draft and animated from deltas, the owner's split
layout with the verbatim `sarah.split_screen_blueprint_map.v1`
contract, and the Effect Native Actions + Code/Receipts panel (account
link, book-a-human through `human_handoff`, checkout opener when a
recorded checkout receipt exists, current Blueprint draft JSON, and
prospect-scoped tool/account/card/guard receipts). BM-5 remains: the
owner playback/QA gate (split-layout screenshot smoke on the SQ-4
#8621 rail, the contract in the sweep, and a live-learning proof that a
fact spoken mid-session appears as a node before the session ends).
Laws: KHS-3 isolation on every read, receipt, and delta; no improvised
pricing; semantic matching only; edges light only from dereferenceable
provenance; EN catalog components only.

**P1 exit receipts:** openagents.com/sarah live and hardened (SR-0
list complete, token route protected, spend-capped); a stranger
completes qualification → quote → settled starter credit purchase
entirely with Sarah (voice or text); a composed multi-module quote with
a bundle rule applied closes via an agent-created link, with property
tests proving no unruled price is reachable; a web conversation resumed
by prospect email and answered through the approval queue; Sarah's Eval
Suite green against the live deployment. **Track C exits:** the sending
identity warm and healthy at the 100/day cap (deliverability receipts);
a full week of the daily sales ledger; and **the first outbound-sourced,
Sarah-conversed, Stripe-settled sale** with its attribution chain
(segment → email → reply → conversation → checkout receipt) dereferencing
end to end.

## P2 — Your Codex on your agent computer (CX-1..9) — the workflow cutover sprint

Spec: `2026-07-07-beyond-mvp-codex-agent-computers-and-ai-employees.md`
§2/§6. **Sprint goal (owner, 2026-07-08): switch our own coding to Khala
Code mobile — we use Codex primarily and want all our coding through
Khala Code.** That raises the bar from "one proven turn" to a daily
driver, so the lane set grows from CX-1..5 to CX-1..9. Issues filed:
**#8545–#8553**.

**STATUS (2026-07-08): mainline slices landed; owner proof gates remain.**
CX-1 (`#8545`) is CLOSED (`7bd4fb2eb1`). CX-2..CX-8 have contract/UI/runtime
slices landed on `main` (`9b963db890`, `b9b56d8e0f`, `4aa8ca37db`,
`92dbe36614`, `ffb157415f`, `4d3506eb89`, `ecbacec2d5`) and remain open only
where live proof is still required: real phone device-auth, rootfs/dispatch
proof, account proof, Claude proof, multi-provision continuity, live account
serialization/concurrency/rotation, and phone dispatch → watch → interrupt /
steer / resume → PR plus monorepo latency numbers. CX-9 (`#8553`) has the
cutover ledger seeded and is blocked on the owner's full dogfood-day receipt:
at least five real mobile-Codex tasks, zero desktop fallback, billing sanity,
friction report, and owner sign-off.

**Owner test build (2026-07-08):** Khala Code iOS TestFlight build **22** is
uploaded from `main` (`99098e4b15`, receipt `0578374bbf`; Delivery UUID
`f30897f8-ee48-448f-b7bd-f37aa51dc626`). It is the current phone artifact for
CX-9: test fresh GitHub sign-in, Codex account connect, account-targeted
dispatch, interrupt / steer / resume / retry, multi-account busy/quota truth,
repo/writeback deep links, Claude parity if available, and the five-task
dogfood ledger. Receipt:
`docs/khala-mobile/2026-07-08-khala-mobile-testflight-build-22-receipt.md`.

- **CX-1 (#8545)** Provider-credential invariant + broker contract
  (`provider_credential_policy: broker_only`; never-pooled /
  never-cross-owner law; scanner coverage; fail-closed tests; ToS
  position documented). *An INVARIANTS change; write the law first.*
- **CX-2 (#8546)** Mobile Codex connect (device-auth → the existing
  custody rail — device-login/start, local-auth/import, auth-material,
  grants/resolve routes all exist; accounts UI with readiness/quota;
  revocation; multi-account from day one). QAM-7's fixture suite turns
  green here.
- **CX-3 (#8547)** Codex inside the agent computer: codex baked into the
  rootfs (re-pinned digest), `auth_grant_ref` populated in placement,
  in-VM broker redemption → isolated `CODEX_HOME` on scratch,
  `codex_app_server` added to the org-cloud dispatch lanes. **The P2
  proof:** one real mobile-dispatched turn on the user's own Codex
  inside Firecracker — `tokenChargeMetered: false` model rows +
  compute-time receipts + reclaim evidence; grant replay after reclaim
  structurally impossible.
- **CX-4 (#8548)** Harness/target selection UX (model-preference store →
  execution targets `codex:<accountRefHash>`; per-thread harness pill;
  quota-aware `auto` with typed fallback events — never silent
  substitution).
- **CX-5 (#8549)** Claude account parity (same broker,
  `CLAUDE_CODE_OAUTH_TOKEN` delivery, `claude_pylon` lane).
- **CX-6 (#8550)** Session continuity across ephemeral microVMs:
  durable per-thread account pin + re-prime-and-replay resume from
  Khala Sync history (persisted-home volumes deferred, recorded).
- **CX-7 (#8551)** Multi-account concurrency: serialize per account with
  typed `account_busy_queued` events; more accounts = real concurrency;
  health/quota truth on the phone. (Resolves the spec's open question
  on concurrency semantics.)
- **CX-8 (#8552)** Daily-driver ergonomics: interrupt/continue/steer
  from the phone on running turns; long-turn streaming quality;
  **monorepo-scale workspaces** (the openagents repo via the SCM broker
  at real size, measured checkout/turn-latency envelope); verify
  commands + writeback + push deep-link loop.
- **CX-9 (#8553)** **Dogfood cutover — the P2 exit.** Owner accounts
  connected (custody import; never the live `~/.codex`), real backlog
  work on our own repos through Khala Code mobile; cutover ledger +
  friction report. Build 22 is uploaded for this proof window. *Exit receipt:
  one full working day — ≥5 real tasks completed through Khala Code mobile on
  our own Codex, zero desktop fallback, receipts linked, owner sign-off.*

Standing policy throughout: `subscription_capacity_resale` blocked
unconditionally — connected accounts serve their owner's work only.
Provider-ToS diligence documented in CX-1. Dependency spine: CX-1 →
CX-2/CX-3 → CX-4 → CX-5; CX-6/CX-7/CX-8 after CX-3, parallel; CX-9
last.

## P3 — Standing employees (AE-1: the cloud lane unification)

The definition cloud lane IS the Agent Computer (retires the parked
`cloud_workroom` framing). Spec: integration doc Phase 1 = overarching
roadmap H2.

- **AE-1.1** Dispatch `agent_definition.v1` runs through the same
  admission gate + metering rail as mobile turns; definition-run
  work-context kind; compiled toolset policy in the placement payload.
- **AE-1.2** Trigger-latency measurement first; warm pool only if
  receipts justify it.
- **AE-1.3** Budgets as payroll (`maxCreditsPerDay` at admission,
  per-employee rollups in balance UI + Aiur).

**Exit:** one cron-triggered definition running nightly on an agent
computer for 7 consecutive days, zero desktop involvement, exact token +
lifecycle receipts nightly, auto-pause proven on budget exhaustion.

## P4 — The employee and the brain (AE-2..4 + CB-1, Blueprint-lite)

Specs: integration doc Phases 2–3; post-MVP doc §3; sovereignty analysis
§3 (the brain adopts the **Blueprint-lite typed vocabulary** — typed
objects/properties/links with per-fact provenance, Action-Submission
writes, Access Explanation as the permission surface, versioned/forkable
entries). **Sarah is the generalization seed:** the P1 persona program,
CRM-mapped memory, and authority posture harden into the formal
`ai_employee.v1` record here; Sarah is migrated onto the formal record
as its flagship instance.

- **AE-2.1** `ai_employee.v1` (persona + authority state
  `observe|draft|act_with_approval|act_within_policy` + identity
  bindings; promotions typed and receipted).
- **AE-2.2** Identity bindings via the broker pattern (mailbox/calendar
  grants scoped to the employee's own address — Sarah's mailbox from
  SR-3 is the precedent).
- **AE-2.3** The phone cockpit: Agents panel + event-ledger inbox +
  one-tap push approvals; pending contracts land with it. The **web
  twin lands on the P1 Track A web codebase authored in the Effect
  Native component set** (§EN unlock #4 — the catalog's forms/lists/
  modals/navigation growth is paced by exactly this surface), beginning
  the business-dashboard build-out. The mobile cockpit screens are new
  screens and therefore author the component set via the RN adapter
  (§EN unlock #3).
- **AE-4 (scopes)** Authority scoping law before the cockpit ships:
  `owner_self | shared_fleet | owner_operator`.
- **CB-1.1–1.3** `company_brain.v1` (named owner-scoped collections on
  Khala Sync; ingestion in trust-cost order; role-scoped slices
  compiled into toolset policy).
- **CB-1.4** The prefill pipeline as a fleet lane (intake → public-data
  research run → seeded brain + starter employee in `observe` → intro
  receipt naming every source) — fed directly by Sarah's intake specs.

**Exit:** one employee promoted observe→draft→act-with-approval from the
phone, each transition receipted, one push-approved outbound action; one
brain serving two employees with disjoint slices; one prospect workspace
prefilled end-to-end with zero hand-editing; Sarah running on the formal
employee record.

## P5 — Templates and business integrations (AE-3 + BI-1..5 + SR-5/6)

Specs: integration doc Phase 4; post-MVP doc §4; BF-6 connector lanes;
Sarah spec §13.

- **AE-3.1** Template = preset bundle (definition preset + persona +
  brain slices + schedule + verification rubric + authority floor).
  Ship order: **Outreach Rep first** (lead-gen definition generalized),
  then Controller, Content Engine, Ops Triage, Knowledge Concierge.
- **SR-5** Sarah contracts + custom bundles (order-form generation,
  `contract_review`, e-sign handoff, milestone-escalation priced as
  "costs extra").
- **SR-6** **Sarah as product**: the sales-employee template extracted
  into the catalog — customers hire their own Sarah on their brain and
  rate card; her outcome ledger is the template's receipted proof.
- **AE-3.2** Catalog gate, promise-registry style: no template lists
  without a receipted *external* outcome; template pages carry live
  outcome ledgers.
- **AE-3.3** Hiring flow; time-to-first-receipt as activation metric.
- **BI-1..5** Connector grants on the custody rail; GitHub sidecar
  first, Slack second; CRM-as-mirror lane; ingestion + redaction; the
  owner-priced connector/orchestration margin as a third labeled
  receipt kind.

**Exit:** three templates listed with receipted external outcomes (the
sales-employee template among them); one customer running two employees
off one brain against a real business system through a brokered
connector; one signed Sarah-originated order form.

## P6 — The trust layer (AE-4.x/CB-2 + sovereignty deltas)

Specs: integration doc Phase 5; sovereignty analysis §6.

- **AE-4.1** Provenance-receipted skill registry (content hash, source,
  injection-audit receipt, capability manifest, regenerate-under-audit).
- **AE-4.2** Head-of-Security as a built-in template.
- **CB-2.1** Input-path ceiling enforced in the behavior-contract sweep
  (untrusted-input triggers cap outbound/spend at `act_with_approval`
  absent an owner waiver receipt) — Sarah's inbound-email posture from
  SR-3 is the working precedent.
- **P6.4** Corpus canaries (seeded facts + periodic external probes →
  misappropriation-detection receipts).
- **P6.5** Data-posture policy objects per inference lane (typed,
  receipt-backed retention statements). *Cheap; may land earlier
  opportunistically.*
- **SR-4 voice** lands here at the latest (push-to-talk web voice with
  the transport-invariant safety fixtures), unless pulled forward by
  owner priority after P2.

**Exit:** registry live with our own templates' skills as the first
audited entries; input ceiling sweep-enforced; first canary receipt;
Sarah voice v1 receipts if not earlier.

## P7 — Scale, the suite, and the network

Specs: product suite doc; integration doc Phase 6 + §5 campaign;
sovereignty analysis §5–6; suite/pricing owner gates.

- **P7.1** The assessment instrument upgraded to the 15-step
  sovereignty rubric (SMB translation) feeding the prefill lane —
  audit-first outbound at fleet scale, with Sarah as the landing
  conversation for every assessment link.
- **P7.2** Suite arming, owner-timed: IAP reopen (#8481/#8482) with the
  credits-brand decision (*"minerals"* gate); desktop pairing reopen
  (MC-5); the two-register design spec; the **openagents.com business
  dashboard** completed on the P1/P4 web codebase in the Effect Native
  component set (spend, receipts, roster, approvals, team) with
  legacy-web retirement per the reopen ledger — this is where the EN-4
  route-by-route absorption of the remaining legacy web surfaces
  (Forum, Sites, Aiur/operator, Autopilot) completes.
- **P7.3** Sovereignty ladder as quoted **assurance levels**
  (structural vs contractual): hosted → BYO subscription →
  `regulated_private` (BF-3.4) → **Reactor** (RX-* lanes, sales-led).
  Reactor Zero serving share as a tracked internal metric (public claim
  owner-gated).
- **P7.4** Network graduation: employee outcome ledgers → consented
  public outcome stories → forum identity → tips → routed work; partner
  prong fulfillment receipted under LG-8/LG-9 bookkeeping.
- **P7.5** The agency-trap tripwire watched continuously
  (operator-minutes per engagement falling per cohort, in Aiur).

**Exit:** falling operator-minutes across a growing cohort; first
partner-fulfilled prong receipted; first assessment-sourced customer
closed by Sarah running a templated employee.

## Cross-cutting disciplines (bind every phase)

1. **Blueprint-modeled quality**: every phase's lanes ship with Eval
   Suites (expected-* fixtures as oracles), pass the release gate, and
   record receipts incl. the could-not-prove list. Feature ladder rungs
   cap claims.
2. **Receipt-first, exact-only; owner-gated greens; public-safe
   projections.**
3. **Config, not fork** — verticals, templates, customers, deal rules
   are config.
4. **Subscription no-resale, never waivable.** Org-cloud never touches
   user-owned machines.
5. **One UI substrate — Effect Native, full conversion (rev 6)**: every
   UI surface — web, mobile, desktop, canvas — authors the **Effect
   Native typed component set** on the one Protoss-blue token set (§EN).
   React/TanStack Start and React Native serve as renderer adapters and
   serving hosts, never the architecture; no new UI authors platform
   primitives outside a renderer adapter. The whole estate converts on
   the scheduled §EN wave program (ASAP, substrate-gated — never
   substrate-skipping); every conversion PR lands green and deletes the
   legacy surface it replaces; legacy UI never grows.
6. **Owned vocabulary** — Blueprint (never "ontology"); no third-party
   company names in public copy.
7. **No hosted CI / no third-party build-update-visual SaaS** — owned
   runners, owned OTA, owned engines.
8. **Constant motion**: owner-gated steps go to NEEDS_OWNER and work
   continues on the next non-blocked lane.

## Current owner gates (as of 2026-07-08 rev 6.4)

Rev 6.4 additions (§MH; also filed to NEEDS_OWNER):

- **X.ai auth plane per fleet capacity host** — plane A (free
  `grok login` / grok.com session) vs plane B (`XAI_API_KEY`, published
  pricing); pick the burn plane while the free window holds; hosts must
  record which plane each account uses.
- **Weekly free-window verification** for Grok 4.5; on expiry flip
  `marginal_cost_class` and let MH-8 re-rank `auto` (no code change).

Rev 6.2 note: the `#8577` Tassadar/Psionic code-removal gate is **no longer
open — the owner lifted it 2026-07-08** (removal is now mandated work). The
gates below remain open:

New this pass (Khala Sync realtime hardening, 2026-07-08):

- **`#8556` alert channel:** one click to verify the capture-staleness alert
  email channel (verification email sent to the owner) — in `NEEDS_OWNER.md`.
- **`#8564` (optional, precautionary):** rotate the cron bearer if the pre-fix
  deploy log was ever persisted/shared (it was local scratchpad only) — can be
  done non-interactively via the automation SA on the owner's say-so.

Standing gates (carried; still open):


- #8503 DoD verification + production arming decisions (P0.4).
- Seeded public-safe test account (P0.8 / R4).
- **Store submission actions** (App Store Connect + Play Console) at
  P0.9.
- **Sarah SR-2 sign-offs**: rate card, bundle rules, tactics registry
  parameters, per-transaction cap; the tactic-vs-no-discounts
  reconciliation confirmation (Sarah spec §14.4).
- Sarah surname/IP check before any public use beyond "Sarah";
  investor-routing posture (qualify-and-route only) confirmation.
- **Track C (outbound) gates:** Resend send arming
  (`CRM_RESEND_SEND_ENABLED` + keys) and the dedicated sending
  subdomain/DNS; daily-cap ramp sign-offs on deliverability receipts;
  send-class promotion beyond approval-gated (never without a receipted
  owner decision).
- Agent-computer compute rate; IAP arming + credits-brand
  ("minerals"); template/services pricing; any promise green flips.

## Document map (content authorities under this roadmap)

| Area | Doc |
|---|---|
| Mobile MVP audit | `2026-07-05-khala-code-mobile-only-mvp-launch-audit.md` |
| Agent Computers strategy | `../khala-code/2026-07-06-agent-computers-strategy.md` |
| Mobile testing system (P0) | `../khala-code/2026-07-07-mobile-testing-audit-and-plan.md` |
| **Sarah (P1, SR-*)** | `2026-07-07-sarah-sales-agent-spec.md` |
| **Sarah implementation home** | `apps/sarah/` in this monorepo (rev 6.7, epic #8594; private `OpenAgentsInc/sarah` freezes at SM-0, retired at SM-6) |
| **Sarah consolidation plan (SM-0..6)** | `2026-07-09-sarah-monorepo-effect-native-consolidation-plan.md` |
| **Multi-harness parallelization (§MH, MH-0..9, rev 6.4)** | `2026-07-08-multi-harness-parallelization-effect-native-analysis.md` (Fable §1–10 + Grok §11 + Fable §12 — consensus + dispatch plan); `docs/grok/parallel-multi-harness-asap.md` + `docs/grok/grok-cli-as-third-harness.md` (adapter design); `docs/grok-cli/` (CLI reference) |
| **Pylon fold (PY-1..3, rev 6.3)** | `2026-07-08-pylon-into-khala-code-proposal.md` (ACCEPTED — daemon-cockpit model; lanes #8578/#8579/#8580; Spark wallet preserved by owner mandate) |
| **Effect Native substrate (§EN, EN-*)** | `../effect-native/README.md` + the six 2026-07-08 docs (framing doc first; UI-layer analysis holds EN-0..EN-9); public framework repo `OpenAgentsInc/effect-native` (ROADMAP phases 0–6 = the substrate build order; issues: #1–#8 Phase 0/1 closed, #9–#19 Phase 2/3, #20–#43 the Phase 4 desktop/canvas epic; #52 + #53–#65 the mobile epic — RN full-peer renderer + mobile catalog + cross-app Khala Sync exit test). Internal lanes: epic #8566, EN-1 #8567, EN-2 #8572, EN-3 #8568, MB-EN #8597 (full mobile rewrite), EN-4 #8573, EN-5 #8574, EN-6 #8575, deploys #8570/#8571 |
| **Landing site kit (P1 Track A, WEB-1 #8565)** | `projects/repos/launch-ui` (MIT reference; **design/tokens reference per §EN** — theme ports into `@effect-native/tokens`; the vendored React replica at `/demo`/`/new` is the visual baseline, not the forward path) |
| **Outbound engine (P1 Track C)** | `2026-07-03-apollo-outbound-sales-plan.md` (audit-first motion, segments); SELL epic #8261 / LG-1..9 (closed substrate); blitz compliance-guardrails (binding) |
| Web stack decision (P1 Track A) | `2026-07-04-tanstack-start-sites-and-web-app-evaluation.md` |
| Codex/BYO harness (P2) | `2026-07-07-beyond-mvp-codex-agent-computers-and-ai-employees.md` |
| Horizon ladder + lane reconciliation | `2026-07-07-overarching-roadmap-khala-code-agent-computers-ai-employees.md` |
| Employees/brain phases (P3–P6 detail) | `../agenticsociety/2026-07-03-integration.md` |
| Product suite | `2026-07-07-product-suite-khala-code-openagents-com-reactor.md` |
| Sovereignty analysis + deltas | `2026-07-07-palantir-institutional-sovereignty-smb-analysis.md` |
| Narrative / talking points | `2026-07-07-what-openagents-is-essay-and-talking-points.md` |
| Reactor plan | `2026-07-04-reactor-open-model-private-deployment-plan.md` |
| Desktop/harness lanes | `ROADMAP.md` |
| QA engine lanes | `ROADMAP_QA.md` |
| Business fulfillment lanes | `ROADMAP_BIZ.md` |
| Market-contact lanes | `ROADMAP_AFTER.md` |
| Background-agent lanes | `ROADMAP_BACKGROUND_AGENTS.md` |
