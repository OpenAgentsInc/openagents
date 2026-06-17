# JUNE 16 ROADMAP — CLOSED OUT (2026-06-17)

> **Status: CLOSED OUT on 2026-06-17.** This is the historical record of the
> June 16 post-launch stability push. Ongoing/open work has moved to
> [`JUNE17_ROADMAP.md`](./JUNE17_ROADMAP.md). Carries forward from
> `JUNE15_LAUNCH_PLAN.md` (closed-out launch wrapup).

## What shipped — June 16 closeout summary

- **Launch gate GREEN (#5012 closed):** both crucial promises live —
  `training.monday_decentralized_training_launch.v1` (#5014) and
  `pylon.install_without_wallet_knowledge.v1` (#5015). Announced; run showed
  `providerConfirmedSettledPayoutSats=5`, `qualifiedContributorCount=1`.
- **Spark offline-receive rail GREEN:**
  `payments.offline_receive_spark_fallback.v1` flipped green on recipient-visible
  rc.12 evidence (Trigger 50k + Whitefang 1k canary).
- **Spark wallet unification (epic #5176) substantially shipped:** Spark
  send/withdraw (#5177), Spark as the primary agent balance (#5178), chunked +
  Spark-native treasury payouts (#5179/#5183), recipient-attributed +
  confirmed-receipt ledger (#5180), MDK scoped to checkouts/treasury (#5181).
  Reconciliation closeout (#5182) carried to June 17.
- **Auto-update (Gate 3) PROVEN LIVE** end-to-end against the signed OTA feed.
- **Forge product-surface wave** landed (Epic A `@openagentsinc/ui`, `/autopilot`,
  `/forge`, `/business`, `/components` live-render, `/login`+OTP, `/animations`).
- **Homepage pylon-stats** fixed + deployed: instant load, near-realtime,
  multi-rail sats total, and real training-contributor count.

> Open items from the sections below (recognition closeout #5182, the next RC
> publish, the native-Spark-transfer fix, auto-payout Gate 2, the v1.0 cut, the
> built-in agent, and the other active lanes) now live in `JUNE17_ROADMAP.md`.
> The detailed June-16 record is preserved unchanged below.

## Where we are (verified 2026-06-16)

- **Launched:** Autopilot 1.0 + Pylon v1.0 release candidates (signed/notarized,
  default-on auto-update _config_), the Tassadar run (`run.tassadar.executor.20260615`,
  active), Episode 237 + essay, the forum/Nostr/Bitcoin-tip rails.
- **🟢 LAUNCH GATE GREEN (16th):** the crucial launch epic **#5012 is CLOSED /
  completed**. Both crucial promises flipped green and live:
  `training.monday_decentralized_training_launch.v1` (#5014 — first independent
  cross-owner Verified pairing `59ba1f30` + provider-confirmed settlement) and
  `pylon.install_without_wallet_knowledge.v1` (#5015 — self-serve install→earn,
  no operator staging of the contributor). Announced at
  `/blog/tassadar-run-is-live` (#5018) + the launch forum thread. #5124 verifier
  bug fixed+regression-tested; #5051/#5061/#5121 closed. Run shows
  `providerConfirmedSettledPayoutSats=5`, `qualifiedContributorCount=1`.
- **Live promises:** source now targets **`2026-06-17.1`**
  (verified live at `/api/public/product-promises`). `.6` flipped the training
  gate, `.7` flipped the install promise, `.8` added an honest auto-update
  caveat, `.9` proved signed-binary OTA, and `2026-06-17.1` flips the scoped
  Spark offline-receive promise green after recipient-visible rc.12 evidence
  (see §H). Counts shift as promises flip.
- **⏭ NEXT (before cutting stable v1.0): three live gates** — Spark fallback
  payout, auto-payout, and auto-update, all tested with the RC. See §H.
- **🧪 rc.7 SHIPPED + offline-payout rail LIVE (16th):**
  `@openagentsinc/pylon@1.0.0-rc.7` on the npm `rc` tag + GitHub prerelease
  (`latest` stays `0.2.5`). Carries #5078 (static Spark-hosted **Lightning
  Address**: `backup-receive --kind lightning-address` registers it,
  `report-readiness` publishes it on file beside the BOLT12 offer), #5151
  (heartbeat publishes live wallet readiness), and the Spark out-of-box
  credential fix. Worker side **deployed** (`8d2f8f8f`; migrations `0194`–`0196`
  applied) — readiness keeps `lightningAddress`, payout prefers BOLT12/MDK online
  and **falls back to paying the Lightning Address** (normal Lightning send, no
  Spark sender; LSP-backed so it lands offline). **Community testing is open**
  (forum CTA posted). Trigger is now recipient-confirmed on rc.12; Whitefang
  and Orrery are tracked as documented recipient blockers in
  `docs/payments/2026-06-17-launch-recognition-spark-recipient-status.md`.
  Auto-payout + auto-update gates still pending.
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
  **`rc.5` is the current npm/GitHub Pylon RC (CLI/npm-only):**
  `@openagentsinc/pylon@1.0.0-rc.5` on the `rc` dist-tag ships the #5121
  validator auto-run (`validate --auto`), the #5077 heartbeat projection fix,
  and the #5122 correction for rc4's immutable version-drift tarball plus
  unauthenticated headless daemon presence. `latest` stays `0.2.5`. The signed
  standalone RC feed remains a separate release surface and only advances when
  the signed binary publish path runs.
- **Spark offline-tipping chain → code-complete (16th):** #5078 (receive-only backup,
  slices 1-3) + #5080 (Bun storage) + #5085 (legacy `migrate-spark` rewire) all landed.
  See §E.
- **Forge product surface + component library (16th, assistant lane):** a large
  apps/web build wave landed on `main` — full status in §F. Epic A (shared Foldkit
  component library `@openagentsinc/ui`) **complete & deployed to prod**; Forge
  cockpit (`/autopilot`) + factory dashboard (`/forge`) live; `/business` landing
  live (+ public `/components` gallery, now rendering **live** component instances
  via #5108); workspace-primitive backend landed. The
  repo-wide **`typecheck:api` gate is green again** (`872cf8c47` — fixed the forum
  post-list error channel + two other pre-existing own-source errors). The wave
  continued with the real **`/login` page + email OTP** (#5111, deployed) and the
  **`/animations`** three.js playground — see §F.

## Late-16th updates (homepage stats realtime + offline-receive root cause)

- **🏠 Homepage pylon-stats — instant + near-realtime (#5050).** Owner reported the
  stats bar (PYLONS ONLINE / WORKING NOW / SATS SETTLED · 24H / TRAINING
  CONTRIBUTORS) showed `…` for ~15s and didn't update live. Root cause:
  `GET /api/public/pylon-stats` took ~5s/call from a **serial N+1** in the
  settlement-totals computation (up to 3 sequential D1 reads per settlement
  receipt) with **no caching** (`no-store`, recomputed per request/visitor), and
  the client only polled every **15s**. Fixed: parallelized the N+1
  (`Promise.all` + memoized intent reads, same semantics), added a ~4s in-isolate
  snapshot cache (test-bypassed), and dropped the client poll to **3s**. Warm
  latency ~5s → sub-200ms; refresh ~5× faster. Follow-up #5171 now embeds the
  same cached `/api/public/pylon-stats` payload into the `/` and `/pylon` app
  shells, so the slot-text stats seed from real values before the first client
  fetch when the snapshot is available. Remaining phase 2 work: presence Durable
  Object WebSocket/SSE for true push. Audit:
  `docs/2026-06-16-homepage-pylon-stats-realtime-audit.md`.
- **🏠 Homepage SATS SETTLED + TRAINING CONTRIBUTORS corrected (17th follow-up).**
  The first #5050 pass fixed latency but the value still undercounted reality:
  `SATS SETTLED · 24H` only summed NIP-90 market receipts, excluding the real
  direct treasury/tips-buffer payouts made during the launch-recognition work;
  `TRAINING CONTRIBUTORS` still read hardcoded zeros. Fixed in the public stats
  projection: `publicRealSatsSettled24h` composes real settled treasury outflows,
  NIP-90 settled receipts, and accepted-work settlement receipts while avoiding
  accepted-work/treasury double-counting; the homepage now prefers that field.
  Training contributors now read the live Tassadar run authority store's distinct
  contributor lease refs instead of stale registrations or constants. The older
  strict accepted-work gate remains separate: direct treasury outflows are not
  upgraded into accepted-work settlement. **17th compatibility fix (#5175):** the older
  `/api/public/tassadar-run-summary` endpoint used by the living-run/spatial view
  now resolves the same provider-confirmed settlement receipts as canonical
  `/api/public/training/runs/{runRef}`, so settled payout sats and qualified
  contributor counts no longer drop to zero on the compatibility surface.
- **🔌 Offline-receive (#5166) fully root-caused + proven on real infra.** The
  "sent-but-invisible" recognition sats were **unclaimed Lightning HTLCs**: a
  payment to a Spark Lightning Address arrives as a `waitingForPreimage` HTLC the
  recipient must **claim** before it credits — invisible in both `getInfo` balance
  and `unclaimedDepositCount`, and the one-shot helper never claimed. Ruled out
  the bigint-balance theory (direct SDK probe: `balanceSats` is a number) and
  fixed the separate binary bug (the compiled binary baked a build-machine WASM
  path; rc.11 embeds the WASM). **Proven end-to-end on `pylon-gcp-1`** (real
  Linux, full egress): treasury → Spark Lightning Address → `backup-claim` →
  balance credited. Shipped rc.8→**rc.12** (OTA feed + npm `rc` + GH): `backup-claim`,
  `backup-status` now reads the real balance + surfaces read-only
  `claimableHtlcCount/Sats`, WASM-embed + build guard. **17th audit follow-up:**
  `backup-status` now recommends `backup-claim` when read-only status shows
  pending `waitingForPreimage` HTLCs, and the non-zero claimable-HTLC counting
  path is regression-tested. **Trigger recipient-
  confirmed the real 50,000-sat backup balance on rc.12**, so
  `payments.offline_receive_spark_fallback.v1` is now green for the scoped
  receive/claim/visible-backup-balance promise (`2026-06-17.1`). Follow-up
  #5168 added the unified visible balance view, and #5169 implemented the real
  consented Spark→MDK sweep: create local MDK receive target, pay from the
  node's own credited Spark backup balance, and emit the reconcile receipt only
  after MDK balance verification. **#5177 now adds direct Spark spend/withdraw:**
  `pylon wallet send --rail spark --confirm-send` pays from the node's own Spark
  backup balance to a BOLT11/Spark payment request or Lightning Address while
  emitting public-safe digest refs only; this restores the old working Spark
  send shape under the new consent/redaction boundary. **#5178 makes Spark the
  primary agent balance:** `pylon wallet status`, local control wallet-status,
  heartbeat/readiness, and the operator snapshot now source the agent-facing
  balance from Spark; MDK is excluded from that public balance and remains
  auxiliary for treasury/checkouts/legacy paths. **Resolved #5172:** the green
  `training.monday` gate stays green only for the scoped launch/run/verification
  and settlement-record path. The Orrery settlement receipt remains explicitly
  **simulation-backed** (`realBitcoinMoved:false`) and does **not** count as real
  Bitcoin movement, accepted-work sats settled, or contributor spendable payout.
  Real payout copy requires a public-safe `realBitcoinMoved:true` receipt.
  **Treasury `pending` clarified:** outbound `pending` rows mean "internal MDK
  payment id recorded, no terminal succeeded/failed outcome recorded yet" — not
  recipient-confirmed sats and not proof the treasury permanently lost funds.
  Added admin API reconciliation at
  `POST /api/operator/treasury/transactions/reconcile`, which checks the stored
  payment id against the treasury/tips-buffer MDK container and only then moves
  the row to `settled` or `failed`; it never returns raw payment ids, hashes,
  invoices, preimages, destinations, or wallet material. Worker cron now runs
  the same bounded reconciliation over pending outbound rows so future terminal
  outcomes are persisted without a manual operator hit. **Resolved #5173
  hardening:** pre-dispatch payout failures now persist as `failed` outbound
  rows with `payment_ref:null` plus a public-safe `failure_reason_ref`, operator
  curl calls must preserve the JSON body (`--fail-with-body` or no `-f`) so 502
  details are not hidden, and the Worker-side treasury/tips-buffer Container
  Durable Objects journal terminal outbound outcomes (`succeeded` / `failed`)
  after `/pay` or `/payments/{paymentId}` observes them. The journal stores only
  terminal state and a public-safe `reasonRef`; it does not store raw daemon
  text, destinations, invoices, hashes, preimages, mnemonics, or tokens.
  Follow-up Orrery retries bracketed the Spark Lightning Address rail. The
  earlier post-#5173 run showed 25,000 sats succeeding while 30,000 / 40,000 /
  50,000 failed before dispatch with sufficient `preflightMaxSendableSat`, no
  payment id, no treasury balance movement, and the same MDK `GenericFailure`
  fingerprint. A later post-diagnostics pass changed the live result: 5,000
  sats succeeded, then 30,000 sats succeeded over the same Lightning Address
  rail. That pass also exposed and fixed a false `treasury_depleted` refusal
  when the sidecar briefly returned `maxSendableSat:null`; the Worker now
  retries the no-spend sendability read before refusing. Remaining honest test:
  refill treasury and retry full 40,000 / 50,000 single-invoice sends with the
  new balance-delta diagnostics. Policy:
  `docs/promises/2026-06-17-training-monday-simulation-settlement-policy.md`;
  payment status:
  `docs/payments/2026-06-17-launch-recognition-spark-recipient-status.md`.
  **Latest Orrery payout follow-up (17th):** `c81e3b494` / deployed Worker
  `964f051d-1676-4c41-8ae2-22b4682ae60c` adds a Worker-side timeout guard around
  operator `/pay` calls into the MDK containers, so a stuck container send now
  returns `reason.public.treasury_payout.timeout` with public-safe diagnostics
  instead of hanging the caller. Post-deploy tips-buffer retries against Orrery's
  Lightning Address succeeded for 5,000 sats and 20,000 sats, both settled in
  the safe ledger and both showed Lightning Address → BOLT11 resolution,
  `paymentIdPresent:true`, and `balanceChanged:true`. The earlier pre-guard
  25,000-sat tips-buffer hang showed no later balance movement and no ledger row.
  Remaining honest 40,000 / 50,000 single-invoice test still needs wallet refill;
  current treasury/tips-buffer max-sendable is below those amounts and the
  fractional fallback policy would otherwise change the test.
  **Latest diagnostics follow-up (17th, later):** `90910cd5d` / deployed Worker
  `02595ffe-ee65-4b10-8006-3dfc6151b3f4` extends the treasury and tips-buffer
  containers plus the operator route with redacted success/failure diagnostics:
  balance delta, route-coverage margin, route-available flag, event outcome,
  payment-id/hash/preimage presence booleans, timeout, and public reason class
  without exposing destinations, BOLT11s, raw payment ids, hashes, preimages,
  mnemonics, or daemon text. Post-deploy Orrery Lightning Address retries:
  intended 5,000 sats hit the depleted-wallet 10% fallback because tips-buffer
  only had ~3,898 sats max-sendable, so it paid 389 sats and settled; then
  1,000 sats settled full; then 2,500 sats settled full with only 14 sats of
  preflight route coverage remaining. Final live balances after the test:
  treasury ~82 sats max-sendable, tips-buffer ~15 sats max-sendable. Conclusion:
  Orrery's Lightning Address → BOLT11 path is live and settling at available
  liquidity; the full 5,000 / 50,000 diagnostic retry now requires refilling a
  payout wallet first. **#5179 follow-up:** hosted-MDK accepted-work payouts now
  formalize the chunk policy instead of forcing one fragile large send: reusable
  destinations (Spark Lightning Address, LNURL, BOLT12-style offers) above
  25,000 sats split into sequential per-chunk hosted-MDK sends with deterministic
  hash idempotency keys, and reconciliation waits on every chunk before claiming
  settlement. Fixed BOLT11 invoices stay single-payment because they are
  amount-bound and must not be split. The accepted-work payout route now also
  resolves the recipient endpoint from the pylon owner agent's saved Forum
  tip-recipient Spark Lightning Address when the operator request omits a
  private destination. Public/operator ledger projections keep only redacted
  payment refs, chunk counts, public-safe metadata refs, and the usual
  settlement state.
- **🧾 Treasury payout ledger attribution (#5180).** Direct treasury and
  tips-buffer payout rows now carry public-safe recipient attribution
  (`recipientRef` or destination-hash), optional keyed owed refs/amounts, and a
  recipient-confirmed state separate from treasury-side `settled`. Added
  operator-only recipient reporting for owed vs settled-sent vs
  confirmed-received totals, plus over-send detection when settled sent exceeds
  keyed owed. Added a recipient-confirmation endpoint for attaching public-safe
  balance/receipt evidence after the recipient proves the sats arrived.
- **⚡ MDK scoped out of the agent path (#5181).** Spark Lightning Address is now
  the preferred Forum tip-recipient readiness/payout destination for agents.
  Pylon primary wallet status, heartbeat readiness, paid assignment admission,
  and tip readiness no longer depend on a local MDK wallet; tip self-claim no
  longer mints an MDK BOLT12 offer. Forum readiness, reliable-tip ladder, sweeps,
  Artanis spend, and x-claim dispatch accept Spark Lightning Address first and
  retain BOLT12 only for legacy rows without a Spark destination, not as a
  fallback after Spark LA. MDK remains deliberately available for customer
  checkouts and operator treasury rails.
- **⚡ Spark treasury rail + single treasury balance (#5183).** The website
  treasury container now exposes Spark treasury service endpoints
  (`/spark/balance`, `/spark/funding-destination`, `/spark/pay`) backed by the
  Breez Spark SDK and the copied Bun SQLite storage adapter; it uses an explicit
  Spark treasury mnemonic when present, otherwise the canonical treasury
  mnemonic seeds the Spark rail too. Operator treasury payouts and Artanis spend
  prefer Spark treasury for Lightning Address/Spark destinations when Spark has
  enough spendable balance; if Spark is unavailable
  or preflight-insufficient they fall back to the existing MDK treasury path, but
  a real Spark dispatch failure stops instead of attempting a second MDK send.
  Raw Spark payment requests are accepted only as private operator/adapter input;
  public agent readiness remains the existing Spark Lightning Address projection
  because raw Spark addresses are treated as private wallet material.
  Accepted-work settlement can select `adapterKind: "spark_treasury"` for a
  single size-agnostic Spark treasury payment instead of the older hosted-MDK
  chunk policy. Public `/treasury` now shows one top-line balance summed across
  MDK + Spark rails, with the rail split kept as small secondary text. Live
  follow-up moved 75 sats from MDK treasury into Spark treasury and proved
  repeatable Spark-origin sends to Whitefang's Spark-backed Lightning Address
  (5 sats, then 25 sats, both settled). Spark-preferred BOLT11 dispatch exposed
  a Breez SDK `invalid_transferid_format` pre-spend failure; the container now
  retries that specific validation failure with `preferSpark:false`, so the
  payment still spends from Spark treasury while avoiding the broken
  Spark-preferred BOLT11 metadata path.
- **🧾 Launch-recognition closeout (#5182) partially closed, funding-gated.**
  Migration `0200_launch_recognition_treasury_backfill.sql` backfills public-safe
  recipient refs / owed refs / confirmation refs for the known Trigger,
  Whitefang, and Orrery treasury rows so the operator recipient-report API can
  produce owed vs settled-sent vs pending vs confirmed-received. Trigger is
  closed at 50k confirmed. Whitefang has a confirmed 1k canary plus 30 sats of
  settled Spark-treasury smoke sends, and still needs the remaining recognition
  closeout once the Spark treasury rail is funded.
  Orrery is marked do-not-resend: settled-sent is 234,639 sats against a 50k
  debt, with the owner decision that the overage stays as hazard pay; recipient
  confirmation still awaits Orrery's wallet-side read. The three legacy pending
  rows were reconciled through the supported API and remain `pending` because
  the containers still report no terminal outcome. Closeout:
  `docs/payments/2026-06-17-launch-recognition-closeout.md`.
- **🧰 Pylon wallet self-recovery (#5167) fixed.** Field report: an unclean MDK
  agent-wallet shutdown could leave `~/.mdk-wallet/daemon.pid` pointing at a
  dead process, stranding `wallet report-readiness` until an operator manually
  deleted the pidfile. Pylon now checks that pidfile before every MDK wallet
  command: malformed/dead-PID files are reclaimed, live PIDs (including EPERM
  cases) are honored, and no wallet/payment material is read or projected.
  Regression coverage: `apps/pylon/tests/wallet.test.ts`
  (`reclaimStaleMdkDaemonPidfile`). Full Pylon suite: 1199 pass / 3 skip.

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
- **Code audit (16th): the full loop is SHIPPED end-to-end; the only gap is
  automation, not infra.** Worker `submit-trace` + validator `validate` verbs are
  both in published rc3 (#5054); `/replay-verdict` builds the `exact_trace_replay`
  challenge that computes `Verified` on digest-match at creation (independent of
  the resolver stub); `buildTassadarRunSettlement` (Verified → capped payout chain
  → public `receipt.nexus.tassadar_run_settlement.…`) and the receipt-first
  promise flip are shipped. So a **manual two-device** proof needs no new code —
  only a real distinct validator device. Posted the code-grounded breakdown to the
  RC3 gates thread (`/forum/t/34bebe36-…`).
- **#5121 (automate the pairing) — IMPLEMENTED + worker endpoint DEPLOYED
  (`afcf1e13`).** Removes the manual `validate` coordination. As built:
  - **Worker:** new agent-gated `GET /api/training/contributions/next-unpaired?validatorDeviceRef=…`
    returns the oldest pending worker contribution from a **distinct** device
    (public-safe refs; skips same-device; GET-only). Live-verified: returns
    Trigger's pending `kernel_trace` lease from a distinct device.
  - **Pylon:** `discoverNextUnpaired()` + `runValidatorAuto()` + opt-in CLI
    **`pylon training validate --auto [--watch …]`** (loads the committed pinned
    fixture, discovers → replays → submits via `/replay-verdict`; no manual
    `--lease-ref`/`--workload`). **SHIPPED to contributors as
    `@openagentsinc/pylon@1.0.0-rc.5`** (npm `rc` dist-tag, `latest` stays
    `0.2.5`; GitHub prerelease `pylon-v1.0.0-rc.5`). rc5 carries the #5121
    auto-validator, the #5077 heartbeat projection fix, and the #5122 corrective
    fix for rc4's runtime version drift plus unauthenticated headless presence.
    CLI/npm-only RC (no new desktop DMG).
  - **`resolveValidatorCandidates()` stays intentionally empty by design** — the
    trust anchor is a separate-device replay, so the server must never fabricate a
    validator digest. Verdicts are produced by the validator **push** path
    (auto-discover → replay → `/replay-verdict`), not a server-assigned candidate.
  - Guardrails unchanged: device-distinctness enforced server-side, routes stay
    `requireAgent`, settlement stays `requireAdmin` + bounded-spend (pairing/
    discovery only — no payout-authority change). 6 new worker route tests + 6 new
    Pylon client tests; full deploy gate green.
  - Remaining to fully close: a live auto-validation on the current `@rc` that pairs a real
    distinct device → `Verified` (this is also the #5061 self-serve proof).
    Contributors can run it now: `npm i -g @openagentsinc/pylon@rc` →
    `pylon training validate --auto --watch`. Follow along on the issue + forum.
  - **2026-06-16 follow-up:** live rc.4 proved discovery/replay/pairing but the
    replay-verdict route still returned the newly created challenge in `Queued`.
    The Worker route now creates, leases, verifies, and finalizes the
    exact-replay challenge in the validator push path, so the auto loop can
    honestly reach `Verified`/`Rejected` without a manual finalize call.
  - **2026-06-16 follow-up 2:** production had two stale pre-fix paired
    challenges still stuck in `Queued`. They were repaired through the existing
    verification API (claim → admin finalize), reaching terminal `Rejected` with
    `ExecutorTraceMismatch`. Root cause: Pylon submits semantically matching
    namespaced refs (`trace.tassadar.commitment.<digest>` vs
    `trace.tassadar.replay.<digest>`), while the verifier compared whole ref
    strings. The verifier now compares the digest component for those two
    Tassadar namespaces and keeps mismatched digest components rejected. Next
    fresh auto-validation can reach the correct terminal state without manual
    repair.
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
- **Living-run Three-effect primitives (#5117):** the web pin now consumes
  `OpenAgentsInc/three-effect@238760e`, which carries the reusable text-label,
  entity-pool, flow-beam, payout-burst, and live-presence primitives with the
  follow-up exact-optional-property typing fix needed by the OpenAgents web
  build.
- **Living-run entity proof layer (#5115/#5116):** the web pin now consumes
  `OpenAgentsInc/three-effect@f1794af`, which adds the optional entity/beam/burst
  layer to `trainingRunView`. The Worker public summary exposes
  `realGradient.verifiedReplayPairs` from real Verified `exact_trace_replay`
  challenges only. The web adapter maps public leaderboard rows into Pylon
  entities, verified worker/validator replay pairs into beams, and settled rows
  into payout bursts. `oa-tassadar-run` now listens for the emitted
  `node-selected` event and opens the public-safe training-run proof or receipt
  URL when a node has one; unlinked nodes show a compact no-public-proof panel
  and do not fabricate a beam, burst, or click-through target.

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
  live-proven under Bun; Bun-storage blocker cleared; blockers remaining:
  `spark_backup_receive_live_smoke_missing`, `spark_receive_sync_reconcile_missing`).
- **Out-of-box credential fix landed (#5078, this pass):** the receive-status
  path (`classifySparkBackupReceive`) was gating on env-only credentials and
  reporting `credential-missing` even though the helper resolver + legacy path
  already honor the embedded owner-authorized default Breez key. Fixed: the
  receive backup is now credential-ready out-of-box once opt-in is enabled (no
  manual env key), matching the documented intent; runbook corrected; regression
  test added. Verified locally: `backup-status` no longer returns
  `credential-missing`; it proceeds to the Spark network init (which needs
  outbound Breez connectivity — see below).
- **Live smoke is environment-gated (needs Breez network):** the funded
  offline-recipient receive+reconcile is the owner-activation path and requires
  outbound connectivity to the Spark/Breez network plus real sats. It does not
  run in the sandboxed assistant environment (no Breez egress), so flipping the
  two remaining blockers green stays an owner-run live step on the new RC.
- **Held payouts (now the launch-recognition sats, not the old 250):** during the
  16th green-gate run, real treasury sends hit `treasury_pay_failed` for **Trigger
  (50,000-sat recognition)** and **Whitefang (50,000-sat recognition + 5-sat
  validator fee)** because their MDK/Lightning nodes weren't accepting inbound at
  that moment; **Orrery's** worker 5-sat + 50k recognition dispatched (BOLT12
  `pending`). These held payouts are the **live test material** for the Spark
  fallback below.
- **#5151 (Trigger) — FIX IMPLEMENTED (`0fcacbc6b`), server deployed:**
  `presence heartbeat` used to post `walletReadiness: "unknown"` with no
  `walletReady`, and the server's heartbeat schema carried no wallet field, so
  `/api/public/pylon-stats` kept `walletReadyNow=false` for an online node until
  a separate `pylon wallet report-readiness` — making Trigger's node _look_
  unavailable for the payout retry even though its local probe was receive-ready.
  Fixed both sides: the Pylon heartbeat now probes the local wallet
  (`classifyMdkWallet`, best-effort, injectable) and publishes real
  `walletReadiness` + a `walletReady` boolean (omitted on probe failure so the
  server keeps the last value — no flap); the Worker `PylonApiHeartbeatRequest`
  now accepts `walletReady` and the existing reducer projects it into
  `registration.walletReady` → public `walletReadyNow`. Worker deployed
  (`09c5a042`); regression tests on both sides. **User-facing once the Pylon
  change ships in the next RC** — the server already accepts it, but running
  rc5 binaries don't send it yet, so #5151 stays open until that RC publishes.
- **Remaining (the gates, owner/live — see §H):** the scoped
  receive/claim/visible-backup-balance promise is green via Trigger's rc.12
  proof. Whitefang and Orrery are no longer ambiguous; #5170 records them as
  documented blockers needing recipient Spark fallback target/proof collection.

**Original goal — narrow, opt-in, receive-only Spark fallback (historical,
superseded by #5177/#5178 Spark-primary agent wallet):**

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
  lands on its Spark fallback and reconciles on next sync. Trigger has proven the
  path; Whitefang and Orrery need the documented #5170 recipient target/proof
  follow-up before their recognition payouts are recipient-confirmed.

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
  - **#5108 (CLOSED `81bdc9497`) — gallery renders LIVE component instances:**
    every family page now leads with real rendered components on a black surface
    (`previewBox`/`familyShowcase`) and demotes the contract metadata to a
    secondary "Contract" section — fixing the "this is just descriptions/metadata,
    where are the components" gap. Login family shows the real WebGL light-beams
    showcase. `components-route.test.ts` asserts rendered instances appear.
- **Epic B — Forge cockpit (#5091):** **B1** (#5087 `5e1e9398f`) reframed
  `/autopilot` on `@openagentsinc/ui` (Runs / compute-routing / accepted-outcome
  receipts). **B2** (#5088 `abdb6c8ce`) shipped the **`/forge` factory dashboard**
  (signal→deploy pipeline over real Run+pool data, honest real-vs-seeded labeling).
  **B4** (#5090) locked the auditable metric definitions in
  `docs/blitz/forge/2026-06-16-forge-factory-metric-definitions.md` and fixed
  the scheduled-backlog triage double-count guard. **B3** (#5089) shipped the
  first **Forge Automations** surface: a configured stage-staffing catalog,
  per-stage configured automation counts, load/run controls, and an Add / tune
  form that creates real Autopilot work orders through the existing work-order
  API. Scope and authority limits are documented in
  `docs/blitz/forge/2026-06-16-forge-automations-surface.md`.
- **Epic C — prefilled workspaces + `/business` funnel (#5103):** **C3** (#5094
  `5e82b6a1d`) shipped the public **`/business`** landing + signup form (first-class
  phone field, opt-in Slack request, usage-based token-credit pricing copy). **C1**
  (#5092 `ba02c9d6b` plus follow-up) landed the **workspace primitive** end to end:
  schema + D1 + API, authenticated `/workspaces/:workspaceId` page, logged-out
  invite shell, and `/business` workspace-invite copy. **C4** (#5095) added the
  public business-signup intake endpoint + D1 queue for opt-in Slack Connect
  requests, with `manual_invite_pending` status and the automation boundary
  documented in
  `docs/blitz/forge/2026-06-16-business-slack-connect-intake.md`. **C2**
  (#5093) shipped the operator seed-to-invite loop: `POST /api/workspaces` now
  returns a personal `/workspaces/{workspaceId}` invite URL, signed-in holders
  can claim an unbound invited workspace, holder reads record first view/revisit
  engagement, and `/api/workspaces/{workspaceId}/engagement` records the first
  starter-run handoff. Operator reads expose engagement; holder projections still
  hide operator-only bindings. Contract:
  `docs/blitz/forge/2026-06-16-workspace-seeding-invite-engagement.md`.
- **Login surface — real `/login` page + email OTP (#5111, CLOSED, deployed
  `d9113b02`):** `/login` is now a branded SPA page (`apps/web/src/page/login.ts`,
  over the constellation animation) offering **email one-time-code** sign-in +
  GitHub — no longer a 302 to `/`. OpenAuth `CodeProvider`/`CodeUI` registered;
  `success()` accepts `provider:'code'`, upserts by verified email, issues the
  same session; `UserSubject` widened to `'github' | 'email'`. Auth-code email
  goes **direct via Resend** (interim, decoupled from the `EmailService` ledger).
  Gating preserved (login only authenticates; product stays downstream-gated) —
  recorded as the "Login Surface" invariant. The architecture rule was relaxed to
  allow the real login route while still banning the deleted _simulated_ auth
  symbols. Audit + posture: `docs/auth/2026-06-16-login-and-auth-audit.md`.
  **#5120 hardening is now shipped:** `/code/authorize` send/resend is guarded by
  D1-backed per-IP, per-normalized-email, and global hourly caps; stale code
  sessions are rejected after 10 minutes before session issuance; sender/storage
  failure fails closed; code subjects no longer include the raw code; and tests
  cover the fail-closed and throttle paths.
- **`/animations` internal three.js playground (deployed `d0cf8a5e`):** a scrollable
  `/animations` page of self-contained WebGL experiments via a `makeAnimationView`
  factory — the live pylon bezier network + WebGL permutations, R3F-/three.js-ported
  scenes (constellation, instanced field, flow field, tube flow, glow knot, wobble
  sphere, shader gradient, grid floor), and a TouchDesigner-style **blob-tracking**
  data-aesthetics scene built on `@openagentsinc/three-effect` instancing. Source
  the login/constellation backdrop and future viz reuse from here. (Also committed
  the previously-untracked `lightBeams.ts`/`lightBeamsElement.ts`.)
- **Auth + email strategy reconciled (docs):** the Cloudflare Email Service audit
  (`docs/auth/2026-06-16-cloudflare-email-automation-audit.md`) now owns the single
  **"Unified Email & Auth Strategy — what we do when"** roadmap (provider-role
  matrix + DONE/NOW/NEXT/LATER sequence). Net: **no change to shipped login** —
  Resend stays the auth transport now; Cloudflare (dedicated auth subdomain) is the
  forward target, reached only after a verified-destination smoke + a provider
  adapter behind `EmailService`. The provider-adapter code slice is now in source:
  `cloudflare_email` is allowed in the typed/D1 email ledger and rendered emails
  can flow through a Cloudflare `send_email` binding with the same idempotency key
  boundary. Cold/bulk/marketing stays off Cloudflare. Tracked as epic **#5119**
  (remaining phases: verified-destination smoke → auth → lifecycle+reconciliation
  → inbound routing).

**Targeted next (this session / soon — won't all land today):**

- **Epic D — customer-#1 dogfood (#5104):** **D2** (#5097) shipped the
  customer-#1 dogfood strip on `/forge`: live only when Runs + provider-pool
  capacity are loaded, with open work / eligible nodes / accepted outcomes /
  incident counters derived from existing operator-safe projections. Contract:
  `docs/blitz/forge/2026-06-16-customer-one-dogfood-factory.md`. **D1** (#5096)
  shipped the runtime-routing slice for internal AI/coding spend through the
  pool/nodes: the work-order list exposes placement-derived routing summaries
  and `/forge` renders owned-node, fallback-lane, metered, and blocked-routing
  counters. Contract:
  `docs/blitz/forge/2026-06-16-customer-one-spend-routing.md`. **D4** (#5106) fixed:
  Pylon now has lane-scoped change
  capture/commit guards, shared-file conflict refs, and dirty workspace
  retention for Autopilot Coder worktrees.
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
  resume, context+repo-memory, retrieval in the cockpit. **First Forge Coder
  systems wave is complete:** #5123-#5144 opened, implemented, documented,
  commented, and closed the projection/readiness foundation across diff review,
  plan/progress, session navigation, context/repo profile, retrieval/search,
  MCP/skills/hooks/plugins/settings, private-material regressions, and the
  product-promise evidence gate. Focused verification on current main:
  `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work`
  (14 files / 68 tests) + `bun run --cwd apps/openagents.com/apps/web typecheck`.
  **Second wave opened one-by-one:** #5145 public-safe diff artifact drilldown,
  #5146 plan/todo mutation receipts, #5147 session control verbs, #5148
  repository-memory persistence, #5149 bounded live retrieval adapters, and
  #5150 guarded extensibility execution-request receipts. **H1 landed:** #5145
  adds the public-safe diff artifact drilldown in `/autopilot`, composed from
  bounded Pack C/artifact-review refs with file/hunk summary refs, digest/
  provenance/caveat/blocker refs, explicit non-authority flags, and raw/private
  material omission tests. **H2 landed:** #5146 adds typed plan/todo mutation
  request and receipt projection beside Run progress, with actor/provenance/
  generatedAt refs, applied/requested/blocked/stale states, explicit
  non-authority flags, closeout-consistency blockers, and unsafe/private plan
  material omission tests. **H3 landed:** #5147 adds authority-gated
  session-control contracts to the Session navigation lane: supported
  resume/fork/rewind/cancel actions render as POST controls only when fresh
  public-safe authority and policy refs are present; unsupported, stale, and
  under-authorized actions remain disabled with explicit blocker refs; public-
  safe control receipts render without raw transcripts, local paths, prompts,
  provider payloads, or private repo content. **H4 landed:** #5148 adds the
  durable refs-only repository-memory profile inside the Context snapshot lane,
  carrying repo identity, command/test/instruction/invariant profile refs,
  refresh receipt refs, changed profile kinds, freshness, generated/refreshed
  times, and blockers. Profiles go stale on dirty worktrees or changed
  instruction refs, block without dev-doctor/profile evidence, and omit raw
  files, local paths, prompts, provider payloads, and private repo content.
  **H5 landed:** #5149 adds bounded live retrieval adapters for file,
  documentation, and diagnostic source refs. They feed the existing
  `ForgeRetrievalPlanInput`, require explicit workspace-boundary refs, block
  semantic/model-selected/hybrid modes without provider evidence, preserve
  deterministic selected/skipped candidates, and omit unsafe live source/query/
  provenance material before `/autopilot` renders it. **H6 landed:** #5150
  adds guarded extensibility execution-request receipts for MCP tool/resource
  calls, skill-body disclosure requests, hook enablement, plugin activation,
  and settings activation. The receipts render callable/disabled/needs-auth/
  needs-trust/blocked/failed/pending outcomes with policy/auth/trust/provider
  blockers, keep skill bodies explicit and body-free by default, and add no
  browser-side execution authority.

  **Completion assessment:** this moved #5107 from planning into a real
  operator-facing foundation, but it is not the whole "~all terminal-agent
  systems" parent epic. Roughly, the first wave touches the key starting
  systems named in #5107 and gives them safe projections, tests, and readiness
  gates; that is about a quarter of the long-arc product incorporation, not a
  final green. The H-wave action/integration layer is now complete: H1-H6 moved
  diff inspection, plan mutation receipts, session controls, repository memory,
  bounded live retrieval, and guarded extensibility requests from read-only
  evidence toward constrained operator workflows. That plausibly gets Forge to a
  usable P0/P1 cockpit foundation for bounded local/cloud coding work, but not a
  finished #5107 parent epic. One to two more similarly scoped waves could put
  the Forge Autopilot Coder around 50-60% of the full #5107 product arc. The
  rest remains live-device/product proof, deeper runtime authority,
  collaboration, IDE/browser/voice, release, enterprise, and polish work across
  the 62-system map.

## H. Next RC → final v1.0 release gates (audit — Spark payout, auto-payout, auto-update)

**The launch gate is green. Before cutting STABLE v1.0 and promoting, the owner
wants a new RC that closes the payout-reliability + self-maintenance loop, with
three things tested live on that RC.** Audit of each as of 2026-06-16:

### Gate 1 — Spark fallback **payout** (#5078) + land the held payouts

- **Built (code-complete):** receive-only Spark backup core + Breez SDK Spark
  adapter + Bun storage (#5080) + legacy `migrate-spark` rewire (#5085) +
  embedded owner-authorized Breez key. Receive path live-proven (returns a real
  mainnet static Spark address under Node and Bun). Promise
  `payments.offline_receive_spark_fallback.v1` = **yellow**.
- **✅ DONE — offline payout to a Lightning Address works end-to-end (2026-06-16).**
  The model landed in rc.7/rc.8 + the worker: a recipient registers a static
  **Lightning Address** via their Spark backup wallet (`backup-receive --kind
lightning-address`) and publishes it on file beside their BOLT12
  (`report-readiness`); the treasury payout path now **resolves a Lightning
  Address via LNURL-pay → BOLT11 (`lnurl-pay.ts`) before the MDK send**, in both
  `executeTreasuryPayout` and the operator endpoint, for primary + fallback. No
  Spark sender on our side. **Proven live: Trigger's held 50,000-sat recognition
  PAID** through this rail — address → LNURL → BOLT11 → `status: succeeded`,
  treasury balance moved (~105k→55k). The earlier `treasury_pay_failed` was the
  missing LNURL resolution, now shipped; failure reasons are also no longer
  masked. rc.8 also fixed the `appendLedgerEvent` crash + clean exit + mysql2
  (#5162).
- **Promise flip:** Trigger's recipient-side rc.12 proof confirms the real
  50,000-sat backup balance is visible after claim, so
  `payments.offline_receive_spark_fallback.v1` is now **green** for offline
  receive/claim/visible backup balance. **#5170 recipient closeout:** Whitefang
  and Orrery do not have recipient-confirmed Spark backup balances in repo
  evidence yet, but their remaining work is now classified as documented
  blockers instead of ambiguous "sent but invisible" payouts:
  `docs/payments/2026-06-17-launch-recognition-spark-recipient-status.md`.
  Whitefang needs rc.12+ Spark Lightning Address publish + treasury retry +
  `backup-claim` / `backup-status`; Orrery needs either the pending BOLT12
  settled/reconciled or the same Spark fallback publish/retry/claim/status
  path. They no longer block the scoped product promise. The single-balance
  follow-up is separate: #5168 shipped the unified balance view and #5169
  shipped the consented Spark→MDK sweep gate.
- **#5151** (heartbeat `walletReadyNow=false`) — **fix implemented + server
  deployed** (`0fcacbc6b` / worker `09c5a042`): the heartbeat now publishes live
  wallet receive-readiness so an online, receive-ready node is no longer skipped
  during a payout retry. Lands for users when the Pylon change ships in this RC.

### Gate 2 — Auto-payout

- **What it means:** verified work + the operator-approved settlement should
  dispatch the contributor payout automatically (within bounded spend caps),
  including selecting MDK-vs-Spark by recipient liveness, and recording the
  receipt — without a human running the payout command per pairing.
- **Status:** the settlement-receipt + treasury-payout rails exist and were
  exercised manually for the green flip; the **automatic** dispatch on Verified
  (bounded, with MDK→Spark fallback selection) is **not yet wired/tested**. Test
  on the RC against the held payouts.
- Boundary: payout _approval_ stays operator-gated / bounded-spend (a permanent
  safety control); "auto-payout" automates _dispatch within those bounds_, not
  unbounded self-spend.

### Gate 3 — Auto-update (in prod) — ✅ VERIFIED (2026-06-16)

- **Which build auto-update landed in, and what's required for it to work:** the
  default-on OTA self-updater shipped in **`1.0.0-rc.1`** (commit `9f5319c89`,
  #5042) and is in **every v1.0 RC signed binary since** (rc.1, rc.2, rc.7,
  rc.8 — all on the feed). It runs **on node startup** (`maybeAutoUpdate()` in
  `apps/pylon/src/index.ts`, before the node boots; opt out with
  `PYLON_DISABLE_AUTOUPDATE`). **It only updates a compiled standalone binary:**
  `resolveSelfBinaryPath()` returns null for a `bun src/index.ts` source run or
  the npm `-g` CLI, so auto-update is a deliberate no-op there. So **for
  auto-update to be expected to work, run the standalone signed Pylon binary (or
  Autopilot Desktop, which bundles it) as a node** — on each (re)launch it pulls
  the latest signed RC. Source checkouts update via `git pull`; npm via
  `npm install -g @openagentsinc/pylon@rc`. (This is why Trigger, on a source
  checkout, doesn't auto-update — not a build problem; his run mode.)
- **Built + PROVEN LIVE.** Both Pylon binaries + Autopilot Desktop ship
  **default-on, ed25519-signature-verified OTA** from `updates.openagents.com`,
  fail-closed. Verified end-to-end against the LIVE feed: driving the real
  `checkForUpdate` + `downloadAndApply` with `currentVersion=1.0.0-rc.1`
  (darwin-arm64) returned `update-available 1.0.0-rc.2` (rollout 100), then
  downloaded the real 63 MB signed artifact from the GCS asset store, verified
  its sha256 + ed25519 against the pinned key `2dbe811d19f67528` (fail-closed —
  `verifyArtifact` throws otherwise), and atomically swapped it into the target.
  Promise caveat removed (registry `2026-06-16.9`).
- **Feed currency — ✅ DONE (2026-06-16): the OTA feed now serves `1.0.0-rc.7`
  on all four platforms.** Built + ed25519-signed the rc.7 standalone binaries
  (`build-rc-binaries.sh 1.0.0-rc.7`, kid `2dbe811d`), uploaded the 4 signed
  assets to `gs://openagentsgemini-oa-updates/assets/…`, staged via
  `publish-pylon-release.ts`, and deployed the `oa-updates` Cloud Run service
  (revision `oa-updates-00014`, env preserved). **Verified end-to-end against the
  LIVE feed:** an installed `rc.2` (darwin-arm64) auto-selected `rc.7`, downloaded
  the real 63.8 MB signed artifact, sha256+ed25519-verified it against the pinned
  key, and atomically applied — so binary/Desktop users now auto-update to the
  rc.7 offline-payout build (Spark Lightning Address + #5151).

### v1.0 cut gate

Cut stable **v1.0.0** (and start promoting) only after Gate 1 (Spark fallback
payout proven + held payouts landed), Gate 2 (auto-payout dispatch tested), and
Gate 3 (live prod auto-update verified) all pass — receipt-first, with
dereferenceable evidence, same discipline as the launch gate.

**Open issues in this lane:** #5078 (Spark fallback — keep open until the live
payout proof + held payouts land), #5151 (heartbeat wallet-readiness). Auto-payout
and auto-update verification are tracked here until they have their own issues.

## Recommended next (assistant lane)

**✅ Launch gate is GREEN and DONE (see "Where we are" + §C).** The #5124 verifier
bug is fixed+proven, the first independent cross-owner Verified pairing settled,
both crucial promises flipped green, the launch is announced, and epic #5012 is
closed. That lane is complete.

**TOP PRIORITY NOW: the next RC + the three release gates before stable v1.0 —
see §H.** The owner's plan: cut a new RC that wires the **Spark fallback into the
payout path** (prefer MDK/BOLT12 when online, Spark when offline), then test
three things live with that RC — (1) Spark fallback payout (#5078, lands the
held contributor payouts), (2) auto-payout, (3) auto-update in prod — then cut
final v1.0 and start promoting. The held recognition payouts (Trigger + Whitefang
50k each, Whitefang's 5-sat validator fee) are the live test material for #5078.

Section **A** is fully closed; the apps/web wave (Epic A live-render #5108,
`/login` #5111 + OTP hardening #5120, `/animations`) has landed and deployed.
With B3 (#5089 Forge Automations), C2 (#5093 operator
seeding/invite/engagement), and D2 (#5097 customer-#1 dogfood strip) now landed,
the clean non-overlapping assistant-lane work is the remaining **email strategy
smoke** (onboard Cloudflare Email Sending, add the restricted staging
`send_email` binding, send one verified-destination operator notification through
the new `cloudflare_email` adapter; see the unified strategy doc), and Epic G
Forge Autopilot Coder surfaces. The owner-gated green-flips (**B/C** §C,
built-in agent §D) still need the owner + a real second device + the concurrent
desktop session. Coordinate to avoid the duplicate-work collisions seen on #5067.

## Coordination note

Multiple sessions are pushing to `main` in parallel (registry churned `.1`→`.11` on
the 15th, `.1`→`.4` on the 16th; many `.claude/worktrees` + `tmp/oa-*` agent worktrees
active). Worktree isolation holds for subagents — the contention people see is a
session's _own main-loop_ editing the shared checkout while its subagents run. Keep
work in isolated worktrees with rebase-before-push; deploy only from a
clean `origin/main` checkout (never the shared working tree, which carries the
other session's uncommitted WIP).
