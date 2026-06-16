# JUNE 16 ROADMAP — post-launch stability + finish the loops

Date: 2026-06-16. Carries forward from `JUNE15_LAUNCH_PLAN.md` (now a closed-out
launch wrapup). June 15 shipped the launch; this is the remaining open work.

## Where we are (verified 2026-06-16)

- **Launched:** Autopilot 1.0 + Pylon v1.0 release candidates (signed/notarized,
  default-on auto-update), the Tassadar run (`run.tassadar.executor.20260615`,
  active), Episode 237 + essay, the forum/Nostr/Bitcoin-tip rails.
- **Live promises:** source registry at **`2026-06-16.5`**; the deployed worker
  still serves an earlier version (the `.1`–`.4` bumps from the 16th are
  **undeployed** — deploy from a clean `origin/main` to publish them). Counts shift
  as promises flip; see `/api/public/product-promises`.
- **Built-in hosted agent backend is LIVE:** `GEMINI_API_KEY` set + verified on
  prod (`generateContent` returns real output); the keyless quota-gated grant
  route `POST /api/provider-accounts/google-gemini/grants/builtin` is deployed
  (worker version `016c665d`+). `autopilot.builtin_compute_agent.v1` is **yellow**
  — green needs the desktop executor calling the live route + a from-install
  go-online smoke + a signed recut.
- **Closed on the 15th** (stability pivot + launch backend): #5052–#5060, #5062–#5067,
  plus the short-term fixes #5056/#5057/#5058/#5059. Tassadar trace backend
  (#5052/#5053/#5054) is built + inert behind `TASSADAR_TRACE_PAIRING`.
- **v1.0 release line (16th):** Pylon source is on the **v1.0** line; this RC is
  **rc3 = `1.0.0-rc.3`**. Stale `v0.3` labels scrubbed to v1.0 across the README,
  Pylon docs, and the promise registry (`738ea7d0f`, `c9c1059b2`), preserving the
  true published `0.3.0-rc2` npm receipts (the v1.0 RC was not on npm yet).
  **rc3 PUBLISHED (npm + GitHub):** leaf dep
  `@openagentsinc/autopilot-control-protocol@0.1.0` + `@openagentsinc/pylon@1.0.0-rc.3`
  on the npm `rc` dist-tag (`latest` stays `0.2.5`); GitHub prereleases
  `pylon-v1.0.0-rc.3` and `autopilot-desktop-v1.0.0-rc.3`. The desktop DMG is
  Developer-ID-signed + Apple-notarized + stapled (Gatekeeper-accepted), attached to
  the release and mirrored to `gs://openagentsgemini-oa-updates/desktop/`. A unified
  release hub now lives at **`docs/DEPLOYMENT.md`** (linked from `AGENTS.md`).
- **Spark offline-tipping chain → code-complete (16th):** #5078 (receive-only backup,
  slices 1-3) + #5080 (Bun storage) + #5085 (legacy `migrate-spark` rewire) all landed.
  See §E.
- **Forge product surface + component library (16th, assistant lane):** a large
  apps/web build wave landed on `main` — full status in §F. Epic A (shared Foldkit
  component library `@openagentsinc/ui`) **complete & deployed to prod**; Forge
  cockpit (`/autopilot`) + factory dashboard (`/forge`) live; `/business` landing
  live (+ public `/components` gallery); workspace-primitive backend landed. The
  repo-wide **`typecheck:api` gate is green again** (`872cf8c47` — fixed the forum
  post-list error channel + two other pre-existing own-source errors).

## A. Short-term bug fixes — ✅ all closed (16th)

- **#5077** — fixed: Pylon post-start heartbeat diagnostics can now name absent
  private-material classes without tripping `projection.reason`, while actual
  bearer/invoice/Spark/key-shaped payloads still fail. Regression covered in
  `apps/pylon/tests/presence.test.ts`; full `apps/pylon` suite passed.
- **#5076** — closed: Forum recent-posts API stale tip-recipient readiness fixed
  (the read projection now rebuilds on write, same discipline as the closed #5056 lane).
- **#5075** — closed: manifest/onboarding `AGENTS-CORE.md` sha256 drift fixed
  (recompute + a guard so it can't drift again).
- **#5066** — closed: Forum category topic lists now ordered by latest post activity
  (`5f6769df6`).

## B. Tassadar executor-trace completion (backend built; pairing now ARMED in prod)

- **`TASSADAR_TRACE_PAIRING=1` is now LIVE in prod** (set as a secret on the
  `openagents-autopilot` Worker on the 16th — no code redeploy; secrets survive
  future deploys). The pairing orchestration is no longer inert. It relaxes no
  `requireAdmin` and touches no settlement/payout, and the validator-candidate
  resolver still returns `[]`, so a pairing only _completes_ once a real,
  **distinct** validator device is present.
- **#5051** epic → **#5061** first external-validator dry-run with **Orrery**
  (volunteered, live non-owner node). This is the one thing that proves the loop:
  pair a real worker + a **distinct** validator device, produce the first
  externally-settled trace receipt. Now needs only a real 2nd device + an
  independent contributor — recruited via the Tassadar Release-Candidates posts
  (`/forum/t/594a1aea-…`). Flips the headline green (below).
- **Living-run public projection (#5114):** the live Tassadar run now has an
  explicit public, read-only feed at
  `GET /api/public/training/runs/run.tassadar.executor.20260615`, plus the
  compatibility summary feed `GET /api/public/tassadar-run-summary` used by the
  #5113/#5118 spatial snapshot path. Both are backed by the existing
  Worker-authoritative training-run summary builder and return `generatedAt`, a
  top-level `live_at_read` staleness contract, public run projection/summary
  data, and provenance-labeled metrics for the spatial `oa-training-run` view.

## C. Owner-gated launch green-flips (receipt-first — only the owner/live event can)

- **#5012** epic · **#5014** live non-owner Go/No-Go → flips
  `training.monday_decentralized_training_launch.v1` · **#5015** self-serve
  install→earn → flips `pylon.install_without_wallet_knowledge.v1` · **#5018**
  the copy-gated announcement (post Go/No-Go). These flip only against a real
  non-owner dereferenceable receipt.

## D. Finish the built-in agent to green (#5063 backend closed)

- Desktop built-in-agent executor calls the live `…/grants/builtin` route; run one
  **from-install "go online" smoke** on a clean machine (this Mac is macOS 26.4
  arm64 — Apple-FM/Gemini-capable) → flips `autopilot.builtin_compute_agent.v1`
  green. (Concurrent session owns the desktop side; coordinate.)
- Apple FM local lane epic **#5068** (children #5069–#5073) — local on-device
  Foundation Models path; concurrent-session-owned; this Mac can host the
  admitted-Mac smoke once the Swift bridge (#5069) lands.

## E. Offline-wallet receive resilience — bring back Spark as a backup receive (+ owed tips)

**Root cause, not just a retry.** The owed tips (Whitefang Hermes + Trigger — both
tip-_ready_ with BOLT12 offers, but `agent_wallet_send_failed` because their wallets
weren't online/routable; 250 sats each owed) are a symptom of a real gap: a recipient
must be **online with inbound liquidity** to receive a Lightning tip/payout. The fix
is the **Spark backup-receive fallback** in
`apps/pylon/docs/2026-06-15-spark-backup-receive-fallback-audit.md`.

**Status (2026-06-16): code-complete across #5078 + #5080 + #5085. One live gate left.**

- **#5078 slices 1-3 (`381c10966`, `10ee7f9bb`):** the receive-only core
  (`SparkBackupReceiveState`/`Projection`, injectable `SparkBackupHelper`,
  MDK-offline classification, `receiveWithFallback` behind `PYLON_SPARK_BACKUP_ENABLED`
  off-by-default, projection redaction) + the Breez SDK Spark adapter + the
  `backup-receive`/`backup-status`/`migrate-spark` CLI + the consented sweep + runbook.
- **Embedded key (`7c43deabd`):** owner-authorized default Breez/Spark API key
  (committed historically at `783f33d5f`) wired as the **env-overridable** fallback —
  so the backup works out-of-box, no manual key. Live-verified valid (returns a real
  mainnet static Spark address).
- **#5080 — Bun support (CLOSED, `ef2986eae`):** the Breez SDK's default storage needs
  `better-sqlite3` (unsupported in Bun); fixed with a faithful **`bun:sqlite`** port of
  the SDK storage injected via `SdkBuilder.withStorage()`. Independently smoke-verified
  under Bun 1.3.11 — real Spark address returned, no better-sqlite3 in the path.
- **#5085 — legacy `migrate-spark` rewire (CLOSED, `d56480f40`):** the v0.2.5 RC-tester
  dead-end ("Missing Breez API key") is gone — `migrate-spark` now inits the user's old
  Spark wallet from their **12-word identity mnemonic** via the embedded-key Bun helper,
  detects balance, and sweeps to MDK on consent. Smoke-verified (no env key → no
  `breez_api_key_missing`; `helperInitReady: true`).
- Promise `payments.offline_receive_spark_fallback.v1`: **yellow** (receive path
  live-proven under Bun; Bun-storage blocker cleared).
- **Owed tips (Whitefang Hermes + Trigger, 250 each):** unblocked once a real node runs
  the receive+reconcile — ready to complete.
- **Remaining (the only gate, owner/live):** one **live offline-recipient
  receive+reconcile in real Pylon** (real sats → an offline node's Spark address →
  sync/claim/`migrate-spark`/receipt) → flips the promise green and lands the owed tips.
  No code work left; this is a live-event proof.

**Original goal — narrow, opt-in, receive-only Spark fallback:**

- MDK stays the primary wallet rail. Spark is a **backup receive target** only —
  when MDK is offline or can't mint a receive request, Pylon can still hand out a
  **static Spark address / single-use Spark invoice** (Spark addresses are static, so
  no liveness needed to receive), then **sync → detect → claim → sweep → reconcile**
  later under the existing legacy-Spark migration consent model.
- **Strictly receive-only:** Spark does **not** regain send/payout, accepted-work
  settlement, or public payout-target authority without a separate gate. No raw
  historical Spark credential material reused.
- Revive only the receive surface (derive signer from the Pylon identity mnemonic;
  `wallet address`/`invoice` receive + deposit-claim lifecycle), per the audit's
  "External SDK Reality Check" (Breez SDK Spark `receivePayment` modes).
- Once shipped, the offline-recipient case is solved: a tip/payout to an offline node
  lands on its Spark fallback and reconciles on next sync — and I complete the owed
  Whitefang + Trigger tips (no waiting on them to come online).

(Comunero + Orrery tips already settled.) Files an issue when scoped into work.

## F. Forge product surface + component library (apps/web build wave)

Building the **Forge** product (our software-factory category entry) on a shared
component library, evolving `/autopilot` into the cockpit, and standing up the
prefilled-workspace + `/business` funnel. Sequenced backlog is tracked under the
epics below; not all lands today — the aim is the main spine.

**Landed today (merged to `main`):**

- **Epic A — `@openagentsinc/ui` component library (#5084, COMPLETE):** extracted
  the shared Foldkit kit into `packages/ui` (#5081 `9658a8be1`); added the **AI
  Elements** family — prompt-input/message/code-block/task/sources/tool/confirmation/
  reasoning/web-preview (#5083 `70c522782`); shipped the public **`/components`**
  gallery (#5082 `a93ede881`); icon-path follow-up (#5086 `34ab4237d`).
  **Deployed to prod** + `/components` made publicly servable.
- **Epic B — Forge cockpit (#5091):** **B1** (#5087 `5e1e9398f`) reframed
  `/autopilot` on `@openagentsinc/ui` (Runs / compute-routing / accepted-outcome
  receipts). **B2** (#5088 `abdb6c8ce`) shipped the **`/forge` factory dashboard**
  (signal→deploy pipeline over real Run+pool data, honest real-vs-seeded labeling).
  **B4** (#5090) locked the auditable metric definitions in
  `docs/blitz/forge/2026-06-16-forge-factory-metric-definitions.md` and fixed
  the scheduled-backlog triage double-count guard.
- **Epic C — prefilled workspaces + `/business` funnel (#5103):** **C3** (#5094
  `5e82b6a1d`) shipped the public **`/business`** landing + signup form (first-class
  phone field, opt-in Slack request, usage-based token-credit pricing copy). **C1**
  (#5092 `ba02c9d6b` plus follow-up) landed the **workspace primitive** end to end:
  schema + D1 + API, authenticated `/workspaces/:workspaceId` page, logged-out
  invite shell, and `/business` workspace-invite copy. **C4** (#5095) added the
  public business-signup intake endpoint + D1 queue for opt-in Slack Connect
  requests, with `manual_invite_pending` status and the automation boundary
  documented in
  `docs/blitz/forge/2026-06-16-business-slack-connect-intake.md`.

**Targeted next (this session / soon — won't all land today):**

- **B3** (#5089) Forge **Automations** surface.
- **C2** (#5093) operator seeding + invite-link + engagement tracking.
- **Epic D — customer-#1 dogfood (#5104):** **D1** (#5096) route our own AI/coding
  spend through the pool/nodes (ship now) · **D4** (#5106) fixed: Pylon now has
  lane-scoped change capture/commit guards, shared-file conflict refs, and dirty
  workspace retention for Autopilot Coder worktrees.
- **Epic E — first design-partner deliverables (#5105):** prefilled workspaces per
  vertical (e-commerce / legal / marketing-agency) + per-vertical stage templates
  (locked in
  `docs/blitz/forge/2026-06-16-per-vertical-forge-stage-templates.md`). **E1
  e-commerce (#5099) landed:** typed seed template
  `forge.template.ecommerce.inventory_campaign.v1` now creates the public-safe
  inventory-aware ad-campaign workspace input, with stock/imagery/spend-cap,
  Commerce QA, authority blocker, and stats/receipt gates documented in
  `docs/blitz/forge/2026-06-16-ecommerce-prefilled-workspace.md`. **E2 legal
  (#5100) landed:** typed seed template
  `forge.template.legal.forms_intake_copilot.v1` now creates the public-safe
  forms/intake copilot workspace input, with NDA-style packet, review checklist,
  source-link, suggested time-entry, no-legal-advice, human-review, and
  authority-blocker gates documented in
  `docs/blitz/forge/2026-06-16-legal-prefilled-workspace.md`. **E3
  marketing-agency (#5102) landed:** typed seed template
  `forge.template.marketing_agency.white_label_launch.v1` now creates the
  public-safe agency workspace input, with landing page, welcome email,
  white-label subdomain, operator-on-Autopilot admin lane, client approval,
  DNS/publish, and channel-authority gates documented in
  `docs/blitz/forge/2026-06-16-marketing-agency-prefilled-workspace.md`.
- **Epic G — fold the terminal-agent-systems into the Forge Autopilot Coder
  (#5107, long arc):** runtime spine already built (Agent Runtime Kernel + tools,
  Pack A/B/C, worktree materialization); next, surface diff-review, plan/todo,
  resume, context+repo-memory, retrieval in the cockpit.

## Recommended next (assistant lane)

Section **A** is the cleanest non-overlapping closeable work I own (worker/pylon
projection fixes: #5076, #5075). **B/C** need the owner + a real second
device. **D** needs the concurrent desktop session. Coordinate to avoid the
duplicate-work collisions seen on #5067.

## Coordination note

Multiple sessions are pushing to `main` in parallel (registry churned `.1`→`.11` on
the 15th, `.1`→`.4` on the 16th; many `.claude/worktrees` + `tmp/oa-*` agent worktrees
active). Worktree isolation holds for subagents — the contention people see is a
session's _own main-loop_ editing the shared checkout while its subagents run. Keep
work in isolated worktrees with rebase-before-push; deploy only from a
clean `origin/main` checkout (never the shared working tree, which carries the
other session's uncommitted WIP).
