# Open issues closure audit — 2026-07-05

A full audit of every open issue in `OpenAgentsInc/openagents` as of
2026-07-05, after a long multi-agent session that closed the large majority
of the backlog (KS-8.x D1 domain cutovers, KS-6.x Wave 3 sync-engine
adoption, TS-6/TS-8 mobile+desktop parity, the Arcade/Claude epics, a
systemic `Promise.all` reliability audit, and a mobile OTA round-trip
proof). This doc covers what's genuinely left, what (if anything) a future
agent can close outright, and precisely what each remaining issue needs.

One issue (`#8458`, a `mock.module` test-isolation bug) was closed directly
during this audit pass — the fix was already shipped and verified with
1000+ test executions, it just hadn't been marked closed.

## Epics — stay open by design, never close standalone

- **#8282** — EPIC: Khala Sync. Umbrella tracker for the whole D1→Postgres
  migration. Closes only when every KS-8.x/KS-6.x sub-issue is done and
  #8330 (the closing sweep) lands. Do not attempt to close directly.
- **#8339** — Epic: ONE-UI. Umbrella tracker for the React+Tailwind
  migration across web/desktop/mobile. Closes only when TS-6/TS-8/TS-10b/
  MC-5 and their successors are done. Do not attempt to close directly.

## Genuinely blocked / time-gated — cannot be closed by any amount of agent work right now

- **#8351** (TS-10b: React-era UI velocity receipt) — explicitly time-gated:
  needs ≥30 days of UI PR history after TS-2 (#8344) and the first TS-7
  phase merge. Given how recently this session's TS-6 work has been
  landing, that window has not passed. **Action for a future agent:** just
  re-check the gate date before doing anything; do not attempt the
  comparison early. Nothing else to do until then.
- **#8354** (MC-5: cross-device chat dogfood) — the evidence bundle is
  fully prepared (`docs/khala-sync/2026-07-05-mc5-cross-device-dogfood-evidence-compilation.md`
  and its JSON sibling). The one remaining acceptance criterion is a real
  owner-run device test (phone + desktop + web, same account, confirm a
  message sent on one appears on the others) — this is explicitly the
  owner's own action, not something an agent can perform. **Action for a
  future agent:** nothing — wait for the owner to run the test and report
  back, or close it once they confirm. Do not fabricate an "owner_signed"
  result.

## Money/auth-domain D1 cutovers — genuinely need long soak time, not more agent passes

These three have each had 3-5 real, careful passes this session (bounded
read allowlists shipped, backfills verified, constraints ported). Further
progress is now gated on either real elapsed soak time under production
traffic, or an owner decision to accept less evidence than the domain's own
stated bar requires — **not** on any remaining code/investigation work a
future agent can just do faster.

- **#8335** (KS-8.6 Artanis read cutover evidence) — the underlying data
  bugs are fixed (issue #8409, closed; both the original column-clobber
  race and the deeper systemic `Promise.all` write-loss are resolved and
  verified). What remains is accumulating enough clean soak time on
  `KHALA_SYNC_ARTANIS_READS=compare` to justify flipping to `postgres`.
  **Action for a future agent:** run a fresh `backfill-artanis.ts --verify`
  and check how long the clean window has been since the last fix deployed
  (was ~40+ min clean as of this session's last check, several hours ago —
  should be much longer now). If clean for many hours/days with the
  historical clobber-rate math no longer showing any gap, that's real
  evidence to flip. Do not flip on a short window.
- **#8336** (KS-8.9 entitlements) — the bounded non-gate read allowlist is
  live in production. The 6 enforcement-gate reads (real charge/consume
  decisions) are now wired into the durable compare-soak-metrics tool in
  observation-only mode (commit `6c2cf72b1a`'s follow-up) — this was the
  literal blocker ("no durable log/metrics surface for a genuine
  multi-hour soak") that's now removed. **Action for a future agent:** flip
  `KHALA_SYNC_ENTITLEMENTS_READS` to `compare` in production, wait a real
  multi-hour+ window, then run
  `packages/khala-sync-server/scripts/query-compare-soak.ts` for the
  `entitlements_gate` domain and evaluate the mismatch count before ever
  considering `postgres`. Do not skip the soak.
- **#8337** (KS-8.7 billing) — bounded read allowlist for 4 display-only
  reads is live (recent ledger entries, auto-top-up state, Stripe checkout
  receipts, inference pay-in receipts). The buyer-payment pipeline and
  forum tip-earnings reads are intentionally, permanently kept on D1 (real
  dedupe/decision hazards, not oversight). **Action for a future agent:**
  there isn't a clear next increment here beyond an eventual owner decision
  on whether to pursue the buyer-payment pipeline read cutover at all —
  given its dedupe-hazard nature, this may simply stay D1-forever by
  design. Don't force it.
- **#8362** (KS-8.18 identity/auth) — writers confirmed live, bounded
  non-gate read allowlist implemented and deployed (only 1 of 6 candidate
  reads survived re-audit as genuinely safe; the rest are correctly
  D1-only). `KHALA_SYNC_IDENTITY_READS` (the actual auth-decision gate)
  stays D1 forever per the issue's own explicit guardrail — this is highest
  blast-radius (auth), and the issue's stated acceptance bar (KV/cache
  layer, auth-matrix shadow-read replay, session-revocation drill) is
  intentionally very high. **Action for a future agent:** this issue may
  never fully "close" in the traditional sense without a dedicated
  auth-hardening project scoped by the owner. Don't force a flip here under
  any circumstances without explicit fresh owner sign-off given the blast
  radius.

## The closing sweep — cannot start yet, has a hard dependency list

- **#8330** (KS-8.19: cron consolidation sweep + D1 retirement) — explicitly
  depends on ALL of #8335, #8336, #8337, #8362 (and originally #8358/#8361,
  both now closed) being done. Given #8335/#8336/#8337/#8362 above are all
  genuinely soak-gated or auth-hardening-gated, **do not dispatch #8330
  until those close for real** — starting the closing sweep early would be
  premature and risky (it includes the actual D1 table drops). This is the
  correct final gate in the whole epic.

## Wave 3 sync-engine adoption — real remaining work, not blocked, needs an agent

- **#8348** (TS-6: web app-shell migration) — every standalone `loggedOut`
  public page is migrated (24 slices landed). The remaining tree
  (`apps/web/src/page/loggedIn/`, ~105,700 lines) needs real session/auth
  infrastructure built on `apps/start` FIRST — `apps/start` has zero
  session/auth code today (confirmed by direct inspection), and at least
  one page (`gymOss.ts`, hourly-billed live inference) has no auth gate
  branch at all, so porting it without real auth would be a genuine billing
  exposure. **Action for a future agent:** this is its own scoped
  infrastructure project — build real session verification on `apps/start`
  (likely reusing the OpenAuth session logic already in
  `workers/api/src/auth/session.ts`) before attempting to port any
  `loggedIn/` page. Do not fabricate session data to work around this.
  Separately: the `pylon.ts` WebGL 3D-scene bundle-budget decision (three.js
  would blow the 760 KiB Start funnel budget several times over) needs an
  explicit owner call on whether to grant a budget exemption for that one
  route — not resolvable by an agent alone.
- **#8420** (KS-6.10 capstone) — blocked specifically on #8422/#8423/#8424/
  #8425 below, not on anything else. Re-check those four before dispatching
  this again.
- **#8422** (KS-6.11 parent: team chat + thread files + agent-goal CRUD) —
  team chat + thread files have live producers and a shipped client
  repoint (#8423, below). Agent-goal CRUD has zero existing producer
  (#8424, below) — confirmed via direct grep, no `KhalaCodeAgentGoal*Entity`
  contract exists yet. **Action for a future agent:** this parent issue
  should stay open until both #8423 and #8424 are fully done.
- **#8423** (KS-6.11a: team chat + thread files client repoint) — **the
  most concrete, well-scoped remaining item in the whole backlog.** Client
  repoint is shipped and live in production (Worker `81fb3a2c...`), verified
  against real production data (author-hydration join resolves correctly).
  The ONE remaining step: delete the legacy `publishTeamChatMessageSync`/
  `publishTeamThreadFileSync` producers in `index.ts` (3 call sites) and
  `thread-file-routes.ts`, the now-dead `sync-notifier.ts` functions, and
  `team-sync.test.ts`. This is deliberately gated on a real fresh
  authenticated `scope.team.<teamId>` WebSocket frame being observed
  end-to-end first (to prove the new path truly replaces the old one before
  removing the safety net) — no headless verification script exists yet for
  this specific check. **Action for a future agent:** EITHER (a) get a real
  owner browser session and observe one live team-chat message/thread-file
  resolve through the new path, then delete the legacy producers
  immediately after with that proof in hand, OR (b) build a headless
  verification script (a scripted authenticated WS client hitting
  `/api/sync/connect?scope=scope.team.<teamId>` against a real or staging
  team) so this doesn't need a human every time. Once verified, the
  deletion itself is a quick, low-risk mechanical cleanup.
- **#8424** (KS-6.11c: agent-goal CRUD sync-engine cutover, from-scratch
  build) — confirmed zero existing producer. This needs the full KS-6.x
  pattern built from scratch: a new `KhalaCodeAgentGoal*Entity` contract in
  `packages/khala-sync/src/khala-code.ts`, a projector, Worker glue, and a
  dual-write wired into `publishAgentGoalSync`/`publishAgentGoalEventSync`'s
  call sites in `omni-handlers.ts` (currently the only, fully-legacy
  producers). Follow the exact same shape KS-6.6 (#8416, closed) used for
  `scope.agent_run.<runId>` — that's the freshest, most complete reference
  implementation for "build a from-scratch producer + eventually repoint
  the client." **Action for a future agent:** treat this as a real,
  bounded, one-pass-doable feature build, not a scoping exercise — the
  scoping is already done in the issue body itself.
- **#8425** (verify `notifyAgentRunSyncScopes`'s remaining legacy
  consumers before touching it) — a genuinely different function from the
  one KS-6.6 already deleted (confirmed by grep: the old
  `syncScopeForAgentRun` export no longer exists). This one is still called
  from 5 live sites and fans out to FOUR legacy scopes at once
  (`personalWorkroomScope`, `teamScope`, `agentRunScope`, `syncThreadScope`)
  — the issue's own body flags that the `agentRunScope` leg is "very likely
  dead weight now" since KS-6.6's client repoint means `subscriptions.ts`
  no longer listens on it, but this needs confirming, not assuming.
  **Action for a future agent:** grep `apps/web/src/subscriptions.ts` for
  any remaining `agentRunScope`/legacy agent-run scope consumer; if
  genuinely none, that ONE leg of the 4-way fan-out can be dropped safely
  (the other 3 — workroom, team, thread — likely still have real
  consumers and must stay). Do not delete the whole function; this is a
  scoped partial cleanup, not a full removal.

## Summary table

| Issue | Status | What's needed |
|---|---|---|
| #8282, #8339 | Epic trackers | Nothing — close automatically when children close |
| #8330 | Blocked | Wait for #8335/#8336/#8337/#8362 |
| #8335 | Soak-gated | More elapsed clean time, then re-verify |
| #8336 | Soak-gated | Flip to `compare`, wait, query soak tool |
| #8337 | Likely permanent | Owner call on buyer-payment pipeline, may stay D1-forever |
| #8362 | Owner-hardening-gated | Needs a dedicated auth-hardening project, not a quick pass |
| #8348 | Real work needed | Build `apps/start` session/auth infra first |
| #8351 | Time-gated | Re-check date, nothing else |
| #8354 | Owner-gated | Wait for real owner device test |
| #8420 | Blocked | Wait for #8422/#8423/#8424/#8425 |
| #8422 | Blocked | Wait for #8423/#8424 |
| **#8423** | **Most actionable** | **Verify one live WS frame (or build a headless verifier), then delete legacy producers** |
| **#8424** | **Most actionable** | **Build the from-scratch agent-goal CRUD producer, following KS-6.6's pattern** |
| #8425 | Small, scoped | Confirm `agentRunScope` leg is dead, drop just that one |

`#8423` and `#8424` are the two highest-value, most concretely actionable
items for a future agent pass — everything else is either genuinely
time/soak/owner-gated, or an epic tracker that closes on its own.
