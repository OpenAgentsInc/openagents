# JUNE 16 ROADMAP — post-launch stability + finish the loops

Date: 2026-06-16. Carries forward from `JUNE15_LAUNCH_PLAN.md` (now a closed-out
launch wrapup). June 15 shipped the launch; this is the remaining open work.

## Where we are (verified 2026-06-16)

- **Launched:** Autopilot 1.0 + Pylon v1.0 release candidates (signed/notarized,
  default-on auto-update), the Tassadar run (`run.tassadar.executor.20260615`,
  active), Episode 237 + essay, the forum/Nostr/Bitcoin-tip rails.
- **Live promises:** `/api/public/product-promises` at **`2026-06-15.11`** —
  17 green · 28 yellow · 11 red · 17 planned · 2 withdrawn (75 total).
- **Built-in hosted agent backend is LIVE:** `GEMINI_API_KEY` set + verified on
  prod (`generateContent` returns real output); the keyless quota-gated grant
  route `POST /api/provider-accounts/google-gemini/grants/builtin` is deployed
  (worker version `016c665d`+). `autopilot.builtin_compute_agent.v1` is **yellow**
  — green needs the desktop executor calling the live route + a from-install
  go-online smoke + a signed recut.
- **Closed on the 15th** (stability pivot + launch backend): #5052–#5060, #5062–#5067,
  plus the short-term fixes #5056/#5057/#5058/#5059. Tassadar trace backend
  (#5052/#5053/#5054) is built + inert behind `TASSADAR_TRACE_PAIRING`.

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

**June 16 goal — narrow, opt-in, receive-only Spark fallback:**
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

Two sessions are pushing to `main` in parallel (registry churned `.1`→`.11` on the
15th). Keep work in isolated worktrees with rebase-before-push; deploy only from a
clean `origin/main` checkout (never the shared working tree, which carries the
other session's uncommitted WIP).
