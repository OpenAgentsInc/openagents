# JUNE 16 ROADMAP — post-launch stability + finish the loops

Date: 2026-06-16. Carries forward from `JUNE15_LAUNCH_PLAN.md` (now a closed-out
launch wrapup). June 15 shipped the launch; this is the remaining open work.

## Where we are (verified 2026-06-16)

- **Launched:** Autopilot 1.0 + Pylon v1.0 release candidates (signed/notarized,
  default-on auto-update), the Tassadar run (`run.tassadar.executor.20260615`,
  active), Episode 237 + essay, the forum/Nostr/Bitcoin-tip rails.
- **Live promises:** source registry at **`2026-06-16.4`**; the deployed worker
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
  **Publishing rc3 (in progress):** the new leaf dep
  `@openagentsinc/autopilot-control-protocol@0.1.0` is **published** to npm; Pylon
  `1.0.0-rc.3` is packing/publishing to the `rc` dist-tag (`release:gate` green;
  awaiting corgi-manifest propagation). `latest` stays `0.2.5`.
- **Spark offline-tipping chain → code-complete (16th):** #5078 (receive-only backup,
  slices 1-3) + #5080 (Bun storage) + #5085 (legacy `migrate-spark` rewire) all landed.
  See §E.
- **`/business`, `/autopilot` Forge cockpit, `/components` (16th):** the
  apps/web Epic A/B/C wave (#5081–#5094) is landing on `main` in parallel
  (`5e1e9398f` /autopilot, `5e82b6a1d` /business, `/components`); driven by the
  concurrent session — verified `/business` route tests pass under vitest/happy-dom.

## A. Short-term bug fixes (cleanest closeable work — projection/freshness family)

- **#5077** — Pylon post-start heartbeat blocked by a private-data **false positive**
  on `projection.reason` (`assertPublicProjectionSafe`, `apps/pylon/src/state.ts`).
  Repro: Whitefang Hermes rc2 test. Fix: narrow the pattern OR make `reason` a safe
  ref + regression test.
- **#5076** — Forum **recent-posts API** shows **stale tip-recipient readiness** vs
  the topic API (a read projection that didn't rebuild on write — same invariant as
  the closed #5056 lane; file under that discipline).
- **#5075** — Manifest/onboarding **`AGENTS-CORE.md` sha256 fields are stale**
  (sha-drift; same family as the AGENTS.md sha fix landed on the 15th — recompute +
  add a guard so it can't drift).
- **#5066** — Forum category **topic lists not ordered by latest post activity**
  (concurrent-session lane; coordinate before editing `forum/repository.ts`).

## B. Tassadar executor-trace completion (deprioritized; backend built + inert)

- **#5051** epic → **#5061** first external-validator dry-run with **Orrery**
  (volunteered, live non-owner node). This is the one thing that proves the loop:
  enable `TASSADAR_TRACE_PAIRING`, pair a real worker + a **distinct** validator
  device, produce the first externally-settled trace receipt. Needs a real 2nd
  device + owner "go". Flips the headline green (below).

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
tip-*ready* with BOLT12 offers, but `agent_wallet_send_failed` because their wallets
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

## Recommended next (assistant lane)

Section **A** is the cleanest non-overlapping closeable work I own (worker/pylon
projection fixes: #5077, #5076, #5075). **B/C** need the owner + a real second
device. **D** needs the concurrent desktop session. Coordinate to avoid the
duplicate-work collisions seen on #5067.

## Coordination note

Multiple sessions are pushing to `main` in parallel (registry churned `.1`→`.11` on
the 15th, `.1`→`.4` on the 16th; many `.claude/worktrees` + `tmp/oa-*` agent worktrees
active). Worktree isolation holds for subagents — the contention people see is a
session's *own main-loop* editing the shared checkout while its subagents run. Keep
work in isolated worktrees with rebase-before-push; deploy only from a
clean `origin/main` checkout (never the shared working tree, which carries the
other session's uncommitted WIP).
