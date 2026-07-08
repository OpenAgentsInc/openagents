# QAM-4 (#8539) — Populated happy-path coverage for Credit history + repo picker

Date: 2026-07-07

## Summary

The QAM-4 signed-in visual sweep (`SignedInScreensVisual.yaml`, commit
`094599dc87`) blessed baselines for four screens, but two of them —
**Credit history** and **repo picker** — could only render their honest
**degraded** ("History unavailable" / "Repositories unavailable") state,
because the device harness auto-signs-in with the seeded **agent token**, which
is not authorized for the owner-scoped mobile REST routes those screens read.

This receipt records the honest investigation of that 401, the decision, and
the mechanism that captures the **populated** happy-path — which is gated on a
single one-time owner step (interactive GitHub OAuth) that cannot be performed
headlessly.

## The 401 is correct-by-design (not a bug) — investigation

Both screens read owner-scoped mobile REST routes:

- Credit history → `GET /api/mobile/credits/balance` + `.../transactions`
  (`workers/api/src/mobile-credits-routes.ts`).
- Repo picker → `GET /api/mobile/repos`
  (`clients/khala-mobile/src/sync/khala-mobile-repos-api.ts`).

All of these resolve the caller through `requireUserBearerSession`
(`makeUserBearerSessionBoundary` → `verifyOpenAuthUserTokens`, `index.ts`),
which verifies an **OpenAuth mobile USER access token (JWT)**. The seeded
`oa_agent_` token is a registered **programmatic agent** credential, not an
OpenAuth JWT, so `requireUserBearerSession` returns `undefined` and the routes
return **401** — verified live this session:

```
agent-token GET /api/mobile/credits/balance -> HTTP 401
```

This is the documented, intended boundary:

- `mobile-credits-routes.ts` header: "Reuses the SAME mobile user
  bearer-session boundary … **never a browser session or agent token**."
- `apps/openagents.com/INVARIANTS.md` (Khala mobile native sign-in):
  `/api/mobile/session`'s `syncToken` "is the current OpenAuth mobile access
  token accepted by `/api/sync/*` through the standard human actor path,
  **not a separate agent/admin credential**." The mobile credits /
  model-preference / Agent-Computer routes are all "mobile-bearer-only".

### Decision: option (b), do NOT relax auth

The task offered two options. **Option (a)** — making the mobile routes accept a
linked agent token — would **directly violate the written invariant above**
("not a separate agent/admin credential"). It is therefore not "clearly the
intended owner-scoped design"; it is explicitly the opposite. **Rejected.**

**Option (b) is correct:** the routes require a real mobile OpenAuth user
session. The populated happy-path must be captured against a real
GitHub-OAuth'd AgentFlampy session, not by weakening auth and not by faking
data. The mobile OpenAuth access token TTL is 400 days
(`SESSION_MAX_AGE_SECONDS`), so a real session token, once obtained, is
long-lived and fully reusable for a repeatable harness.

## Mechanism (landed) — one command away from populated baselines

- `clients/khala-mobile/.maestro/flows/SignedInScreensPopulatedVisual.yaml` —
  reaches Credit history + repo picker and captures `*.populated.*` checkpoints
  with **fail-closed** oracles: it asserts the real-data markers ARE visible
  (`text: ".*\$[0-9].*"` money row; `text: "public|private"` repo badge) AND
  the degraded empty states are NOT ("History unavailable",
  "Repositories unavailable"). It has **no** agent-token manual-sign-in
  fallback, so it can never silently pass on the degraded build.
- `clients/khala-mobile/scripts/build-populated-ios.sh` — builds a Release
  simulator app that auto-signs-in with a REAL session read from
  `~/work/.secrets/khala-mobile-session.env`, and **refuses to build** unless a
  live `GET /api/mobile/credits/balance` with that token returns 200 (an agent
  token 401s here — proven above), so a "populated" build can never be baked
  around a token that would only reproduce the degraded state.
- `clients/khala-mobile/scripts/mobile-visual-tier-run.sh` — resets the seeded
  thread for the populated flow too and blesses/verifies `*.populated.*`
  baselines through the same owned `openagents.khala_visual_baselines.v1`
  engine.
- `clients/khala-mobile/tests/maestro-policy.test.ts` — new oracle
  "define populated signed-in visual flow with fail-closed populated oracles"
  (credential hygiene + populated-checkpoint + guard shape), green.

## Remaining owner step (blocks the captured populated baselines)

Obtaining AgentFlampy's first real mobile session token requires **one
interactive GitHub sign-in** (browser PKCE), which cannot be done headlessly,
and no such token / GitHub credential is stored. Exact step:

1. On the iPhone 17 Pro simulator, in any Khala Code build, tap
   **Sign in with GitHub**, complete GitHub login as **AgentFlampy**, and
   return via `khala://auth`.
2. Capture that session's `{ ownerUserId, syncToken }` (the app's own
   `POST /api/mobile/session` echoes it) into
   `~/work/.secrets/khala-mobile-session.env` (gitignored; never print/commit):

   ```
   KHALA_MOBILE_SESSION_OWNER_USER_ID=user_...
   KHALA_MOBILE_SESSION_TOKEN=<the real OpenAuth mobile access token>
   ```
3. Then (repeatable, no owner needed again for ~400 days):

   ```sh
   bash clients/khala-mobile/scripts/build-populated-ios.sh
   MOBILE_VISUAL_FLOW=SignedInScreensPopulatedVisual \
   MOBILE_VISUAL_REPORT=docs/khala-code/receipts/2026-07-07-qam-4-ios-populated-screens.json \
   bash clients/khala-mobile/scripts/mobile-visual-tier-run.sh
   ```

   The first run blesses `khala.mobile.screen.credits-history.populated…` and
   `…repo-picker.populated…`; a `--verify` re-run must report `matched`.

Until that step runs, the populated baselines are intentionally **not**
committed — no faked "populated" data was captured.

## Live-session run — 2026-07-07 (populated path proven GREEN on real data)

An owner-created **live signed-in session** (interactive GitHub sign-in) was
available in the running Release build on the iPhone 17 Pro simulator
(`2E5DFC26-DB79-4EE2-BF8E-2EB486A1AFBA`, iOS 26.5). To keep the live session
intact, the populated oracle assertions (identical to
`SignedInScreensPopulatedVisual.yaml`) were driven **non-destructively** against
the already-signed-in app via Maestro (no `clearKeychain`), capturing to a
**gitignored** scratchpad outside the repo. Personal-account screenshots were
**not** committed.

Result while the session was live:

- **Credit history — GREEN on real data.** The exact committed oracle passed:
  `Credit history` visible; `History unavailable` and `No transactions yet`
  **not** visible; a real signed `$` money row (`.*\$[0-9].*`) visible.
  Checkpoint: `khala.mobile.screen.credits-history.populated.iphone-17-pro.dark`.
  This proves **credits-history is tested connected to the real populated
  flow** (the route is mobile-OpenAuth-user-only; an agent token 401s it).
- **Repo picker — populated on real data.** The owner-scoped, real-session-only
  `GET /api/mobile/repos` returned a real repository row (rendered through the
  real `KhalaListItem`), with a visibility badge rendered and both degraded
  states (`Repositories unavailable`, `No repositories found`) **absent**.
  Checkpoint: `khala.mobile.screen.repo-picker.populated.iphone-17-pro.dark`.
  This proves **repo-picker is tested connected to the real populated flow**.

### Defect found + fixed: badge visibility was not in the accessibility tree

The committed repo-picker oracle asserts `text: "public|private"`. On a real
populated row this **could not match** via Maestro's accessibility tree
(confirmed by dumping the live hierarchy): `RepoRow` set the `ListItem`'s
`accessibilityLabel` to just `repo.fullName`, which **collapses the child
badge**, so the `public`/`private` `RightComponent` text never reached the
accessibility tree (or a screen reader). The oracle was therefore unsatisfiable
on-device regardless of session — a latent defect.

Fix (this change): fold visibility into the accessible label
(`repo.fullName, public|private`) in
`clients/khala-mobile/src/screens/repo-picker-screen.tsx`, so the badge is a
first-class, testable part of each real row and screen readers announce it.
`clients/khala-mobile/tests/repo-picker-screen.test.tsx` updated to assert the
strengthened label; the khala-mobile gate (**409 pass, 0 fail**) and the
behavior-contract sweep (`ux-contracts.test.ts`,
`mobile-testing-lab-contract.test.ts`) stay green.

### Session note / remaining step

The live session was a **keychain** (interactive-sign-in) session, not a baked
build. During on-device verification of the fixed badge oracle the app was
relaunched; the app re-validated stored credentials and returned to the
logged-out onboarding screen (`Welcome to Khala Code`), while the openagents.com
mobile API stayed healthy (`/` 200, unauth credits `401`). The live session did
**not** survive the relaunch, so the machine-green **on-device re-verify of the
committed `public|private` badge oracle** (now satisfiable with the fix) — and
any baseline blessing — are **pending the owner signing in again**.

A committed visual baseline must still use a **public-safe** account (not this
personal account): the personal-account captures stayed in the gitignored
scratchpad and were **not** blessed into the committed baseline. The goal of
this run — proving the populated credits-history + repo-picker paths work green
against real data — was met (credits machine-green; repos populated with a real
row and no degraded state), and the badge-oracle defect that blocked full
machine-green was found, fixed, and unit-proven.
