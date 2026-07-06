# Seam-Testing Audit: why the mobile WebSocket-auth bug survived every QA layer, and what to build so agents catch this class before users do

Date: 2026-07-06
Companion incident audit: `docs/khala-code/2026-07-06-mobile-loading-threads-websocket-auth-audit.md`

## 0. The incident, in one paragraph

Four consecutive TestFlight builds (10–13) shipped with an infinite
"Loading threads" spinner for every mobile user. The root cause was
server-side: `/api/sync/connect` never read the `?token=` query bearer the
client transport sends (WebSocket clients cannot set an `Authorization`
header), so every authenticated mobile live-tail connect 401'd, forever, and
the sync session silently retried without ever reaching `live`. Two
client-side fixes shipped against the symptom before the cause was found.
The cause was found only by writing a throwaway Bun script that drove the
REAL `createHttpKhalaSyncTransport` against production with a real
cookie-less bearer — a ~40-line script that reproduced the bug in under a
minute and confirmed the fix the same way.

This audit answers: why did every existing QA layer miss it, and what do we
build so a coding agent's normal loop would have caught it before build 10
ever uploaded?

## 1. The bug class, precisely

**A client-server contract mismatch invisible to both sides' unit tests.**
Its signature properties:

- Each side is individually correct against its OWN test doubles. The route
  tests injected a fake `authenticate` that ignored the request (so the
  wiring closure over the wrong request object was unobservable). The
  client tests injected fake transports (so the real transport's query-token
  convention was never exercised against anything).
- The failure only manifests at the REAL seam: real transport ↔ real route
  wiring ↔ real auth stack, under the one client condition no browser test
  produces (no session cookie).
- The runtime failure mode is silent-retry, not crash: infinitely retried
  401s presented as "loading," which invited symptom-level fixes.

Other live seams in this codebase with the same structure (candidates for
the same class of bug): the mobile-session token bridge, push-notification
registration, the OTA update manifest/fingerprint handshake (already bitten
once today — a font dependency silently changed the runtime fingerprint),
the credits REST routes (already bitten — #8480 shipped a client calling
routes that did not exist), and Aiur's sync proxy.

## 2. What each existing layer actually covers (verified inventory)

| Layer | What it actually exercises | Why it missed this bug |
|---|---|---|
| QA Swarm (desktop) — `docs/qa/qa-swarm-khala-code-standing-engagement.md`, `scripts/qa-nightly-matrix.ts`, `apps/qa-runner/` | Real, substantial: nightly matrix, visual baselines, perf budgets, findings ledger, computer-use driver for desktop/web | No mobile backend, no khala-sync transport adapter. Desktop connects over surfaces where browser cookies mask the auth gap anyway |
| QA Swarm (mobile) — `docs/khala-mobile/2026-07-05-qa-swarm-mobile-adaptation.md` | A design doc + the existing `bun test` suite. Its own table: seeded monkeys "none yet", perf probes "none yet", LLM explorers "this audit itself" | It is a plan, not a fleet. Nothing autonomous drives the app |
| Mobile RN-mount harness — `tests/support/rn-test-environment.ts` | Real React state/render/effect for mounted production components under `bun test` | Documented limits: no native rendering, no gestures, no Skia/Reanimated — and crucially, no network. Component tests stub `globalThis.fetch` |
| Maestro — `clients/khala-mobile/.maestro/flows/` | `LaunchFallback` and `LaunchGitHubSignInInteraction` genuinely ran on sim/emulator (dated receipts). Real launches, real taps | The ONE flow that reaches live sync (`SignedInThreadSmoke.yaml`) has NEVER run — blocked on a seeded public-safe GitHub test account that doesn't exist (`needs_seeded_public_safe_test_github_account`) |
| Server route tests — `khala-sync-connect-routes.test.ts` | Param validation, scope gate, hub forward — "All seams are injected — no network, no Durable Objects" (its own header) | The fake `authenticate` ignored which request it received, so the index.ts wiring bug (authenticating the RAW request instead of the normalized one) was structurally invisible |
| Client engine tests — `packages/khala-sync-client/` | Session/overlay/store logic over FAKE transports, thoroughly | `transport.ts` — the file that talks to real servers — is imported by ZERO test files. Literally untested code |
| Worker full-stack e2e — `khala-sync-access-revocation.e2e.test.ts` | Real local Postgres + real route handlers + real hub DO + real client store/session | Drives the revocation path via the DO directly; never crosses the `/api/sync/connect` route's auth with a cookie-less bearer |
| Predeploy smoke — `predeploy-parallel-dispatch-smoke.mjs` (in `deploy:safe`) | Real staging: registers an agent, dispatches parallel no-spend Codex assignments, checks dedup | Entirely about dispatch dedup. Zero sync/WS/bearer coverage — so `deploy:safe` green said nothing about this |
| Behavior contracts — `packages/behavior-contracts` | Coverage checker proves an oracle FILE exists and references its contractId | No contract binds "a cookie-less bearer client completes a real connect upgrade and reaches live". The semantically-nearest contract (`khala_mobile.platform.launched_app_interaction_smoke.v1`) is `pending`, `unenforced`, `oracles: []` |

The one-line synthesis: **every layer stopped exactly at the seam.** Server
tests fake the client side; client tests fake the server side; the two
device flows that ran stop at the sign-in screen; the one flow that would
cross the seam has never executed; the deploy gate smokes an unrelated
subsystem.

## 3. Why the agent found it in minutes once it looked in the right place

The eventual diagnosis loop was: (1) drive the real client transport against
production for a public scope — works; (2) same for an authenticated user
scope — WS fails while HTTP succeeds; (3) curl the raw upgrade to capture
the 401 body; (4) read the route's auth wiring. Total new code: ~40 lines.
Nothing about this required a device, a simulator, credentials the repo
didn't already have (an agent bearer), or any new infrastructure. **The
capability gap is not tooling — it's that this loop wasn't encoded anywhere
an agent (or CI) runs by default.** The scratch script that found the bug
(`packages/khala-sync-client/scratch/khala-repro-user.ts`) is exactly the
artifact that should have existed as a standing test.

## 4. Recommendations (ordered; each names its concrete first artifact)

### R1 — Promote the incident repro into a standing live-seam smoke (highest leverage, smallest lift)

Turn the scratch script into
`packages/khala-sync-client/src/live-seam-smoke.e2e.test.ts`: drive the REAL
`createHttpKhalaSyncTransport` — bootstrap → logPage → **connectLive** — with
a real cookie-less agent bearer, assert the socket opens and a session
reaches `live`. Two run modes:

- **Gated production/staging mode** (env-gated like the existing live-key
  skips in `apps/pylon`): runs in `deploy:safe` right after the staging
  deploy, against staging, with a self-registered throwaway agent token
  (the parallel-dispatch smoke already self-registers agents — reuse that
  exact mechanism). Fails the gate → production never gets the regression.
- **Local full-stack mode**: the access-revocation e2e already assembles
  real Postgres + real route handlers + real hub DO; extend that harness to
  mount the REAL `/api/sync/connect` route (with the real index.ts wiring
  extracted into a testable route-table entry) and connect the REAL client
  transport to it.

Acceptance: deleting `withBearerFromQueryToken` (reintroducing today's bug)
must fail both modes.

### R2 — Kill the untested-transport hole

`transport.ts` gets its own test file: unit-level (URL/query/header
construction against a recording fetch/WebSocket fake — would have caught
"token goes in query but server reads header" the moment anyone wrote the
server assertion) plus the R1 e2e. Add a repo guard (same pattern as the
existing doc-coverage/architecture checks) that fails when a `src/` file in
`packages/khala-sync-client` is imported by zero test files — that is how
"the file that talks to production" stayed untested for weeks.

### R3 — Wiring-level route tests, not just handler-level

The post-fix connect tests still inject a fake `authenticate`; the real
`index.ts` closure remains uncovered. Extract per-route wiring (the object
literal currently inline in `index.ts`) into importable factories so a test
can assert: "the connect route's `authenticate` resolves an actor from a
request whose ONLY credential is the `?token=` query param," using the real
`authenticateRequestActor` against a fake env. This converts a whole class
of "wired the wrong request/env into the dependency" bugs from invisible to
one-line assertions. Apply to the sync route family first (connect, log,
bootstrap, push), then the mobile-session bridge.

### R4 — Unblock and gate `SignedInThreadSmoke.yaml` (the seeded account)

The single owner action this audit needs: create the seeded public-safe
GitHub test account (`needs_seeded_public_safe_test_github_account`, already
tracked in NEEDS_OWNER). Then: run the flow nightly against a Release sim
build pointing at staging, promote
`khala_mobile.platform.launched_app_interaction_smoke.v1` from
`pending/oracles: []` to enforced with the Maestro receipt as its oracle,
and add the run to the (currently desktop-only) nightly matrix as the first
mobile row. This is the layer that catches whatever R1–R3 structurally
can't: symptoms only visible through the real app (today's watchdog
flip-back flash would have shown up here as "error text appears then
disappears").

### R5 — A "seam contract" convention in behavior-contracts

Today's contracts bind one side's behavior. Add a contract kind (or naming
convention + authority-boundary requirement) for two-sided seams: the
contract names BOTH the client artifact and server artifact, and its oracle
must be a test that imports the real code from both sides (or an e2e
receipt). First three seam contracts to write:
`khala_sync.seam.bearer_ws_connect_reaches_live.v1` (R1's test as oracle),
`khala_mobile.seam.mobile_session_token_bridge.v1`, and
`khala_mobile.seam.ota_manifest_fingerprint_roundtrip.v1` (the OTA
fingerprint mismatch from earlier today is the same class — client and
server each "worked").

### R6 — Teach the QA swarm the diagnosis loop itself (agent-facing)

The desktop QA swarm's findings-ledger pattern is right; what mobile needs
is the swarm's *explorer* role pointed at seams, not screens. Add to
`apps/qa-runner` a `khala-sync-transport` backend: a headless target that
(like the incident script) drives real transports with real/seeded bearers
against staging and classifies outcomes (connect 401/403, silent-retry
loops, phase-never-reaches-live). Cheap to run per-PR for any diff touching
`khala-sync*`/`transport`/`auth`. This is the generalization of "the agent
found it in minutes once it drove the real seam" into standing automation.

### R7 — Runtime tripwire for silent-retry loops (catch it in prod in minutes, not builds)

Independent of tests: the session's `driveScope` retries connect failures
forever with no cap and no telemetry. Two changes: (a) treat a 401 on
connect like the existing 403 handling — park the scope as `denied` instead
of retrying an unauthenticatable connect forever (a 401 loop is never going
to self-heal); (b) emit a client-side counter/log ref after N consecutive
connect failures, surfaced through the existing Worker observability, so a
fleet-wide connect-failure spike pages within minutes of a bad deploy. Had
(a) existed, build 10 would have shown a crisp "access denied" instead of
an infinite spinner — and the server bug would have been diagnosed from the
first user report.

## 5. Sequencing

1. **Now / this week**: R1 (staging mode inside `deploy:safe`) + R2 (transport
   tests + zero-test-imports guard). These two alone would have prevented
   every build of this incident.
2. **Next**: R3 (route-wiring factories for the sync family), R7 (401 parking
   + connect-failure telemetry).
3. **Owner-gated**: R4 (seeded account → run the blocked Maestro flow →
   promote the pending platform contract).
4. **Structural**: R5 (seam contracts), R6 (qa-runner transport backend),
   then fold mobile rows into the nightly matrix.

## 6. What this does NOT claim

- None of this replaces device-level testing; R1–R3 are deliberately
  cheaper layers that catch the *contract* class, while R4 remains the only
  layer that sees real-device-only failures.
- The behavior-contract coverage checker still only proves oracle files
  exist and reference their contract; R5 narrows but does not eliminate the
  gap between "an oracle exists" and "the oracle proves the statement."
- Staging-gated smokes depend on staging remaining representative
  (same wiring, same auth stack). The staging Worker currently shares the
  route code but has separate secrets/bindings; drift there would silently
  weaken R1's gate.
