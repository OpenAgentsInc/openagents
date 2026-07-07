# Khala Mobile SignedInThreadSmoke Maestro Receipt

Date: 2026-07-07, America/Chicago (Central)

Issue: [#8510](https://github.com/OpenAgentsInc/openagents/issues/8510)
(ST-4 under epic [#8506](https://github.com/OpenAgentsInc/openagents/issues/8506))

## Result

PASS (green).

`clients/khala-mobile/.maestro/flows/SignedInThreadSmoke.yaml` ran on a Release
iOS-Simulator build and, end to end:

1. resolved a signed-in session (auto-signed-in as the seeded public-safe owner
   account **AgentFlampy** via baked `EXPO_PUBLIC_KHALA_SYNC_DEMO_*` — the flow's
   `when: visible "Sign in manually instead"` manual block is correctly skipped
   because that state never renders once signed in),
2. asserted the `Khala` thread-list shell rendered,
3. asserted the seeded thread title `Maestro smoke thread` is listed and opened
   it,
4. asserted the thread header rendered,
5. opened the composer options and asserted the lane picker (`Send with Claude`)
   is visible,
6. typed and sent a public-safe message and confirmed a message with that body
   renders in the thread.

Two green runs recorded:

- Direct `maestro test` run: `PASS (36s)`.
- Repeatable runner run (`clients/khala-mobile/scripts/signed-in-thread-smoke-run.sh`,
  which resets the thread's turn state first): `PASS (31s)`.

JUnit artifacts (local, not committed): `/tmp/khala-maestro/report/PASS-run1.xml`,
`/tmp/khala-maestro/report/PASS-runner-final.xml`. Maestro local artifact dirs
under `~/.maestro/tests/2026-07-07_*`.

## Environment

- Repo commit at receipt time: `87a84fbdee`
- App id: `com.openagents.khala.mobile`
- App name: Khala Code
- App version: `0.1.0`, iOS build number `17`
- Configuration: **Release** simulator build
  (`xcodebuild -configuration Release -sdk iphonesimulator`, `CODE_SIGNING_ALLOWED=NO`)
- Simulator: iPhone 17 Pro, iOS 26.5, UDID `2E5DFC26-DB79-4EE2-BF8E-2EB486A1AFBA`
  (a different device from the iPhone 17 another agent was using)
- Maestro: `~/.maestro/bin/maestro`; JDK: Homebrew `openjdk@17`
- Flow: `clients/khala-mobile/.maestro/flows/SignedInThreadSmoke.yaml`

## Seeded Public-Safe Preconditions (AgentFlampy)

- Owner user id: `github:300914913` (GitHub login `AgentFlampy`; the user exists
  in Khala Sync prod Postgres `users`/`auth_identities` because the owner signed
  into Khala Code mobile with it).
- Auth: a programmatic agent was registered via
  `POST https://openagents.com/api/agents/register` (writes to Postgres since
  commit `57515fa7fa`), then its `agent_credentials.openauth_user_id` was set to
  `github:300914913` so the `oa_agent_` token authenticates as AgentFlampy for
  `personalScope(github:300914913)`. Verified by a `POST /api/sync/bootstrap`
  returning 200 for `scope.user.github:300914913` — exactly the app's sign-in
  check (`khala-auth-validate.ts`).
- Seeded thread: title `Maestro smoke thread`
  (`scope.thread.maestro-smoke-thread-20260707`), created via the
  `chat.createThread` mutator authenticated as AgentFlampy.
- Credentials live only in `~/work/.secrets/khala-maestro.env` (gitignored). No
  token, credential, or chat body appears in this receipt or any committed file.

## Turn-State Reset (repeatability)

Sending a message starts a `runtime_turn` on the `hosted_khala` lane. No Pylon
processes that lane for this public-safe account, so the turn stays `queued` and
the composer flips to its active-turn (Steer/Queue) state, which hides the lane
picker and makes a subsequent run's lane-picker assertion flake. The runner
`signed-in-thread-smoke-run.sh` therefore closes any active/queued turn
(`runtime.closeTurn`) before each run so the composer renders the lane picker
deterministically. This reset is a run precondition, like seeding — the smoke
flow itself is unchanged and honest.

## Flow drift fixed against the current UI

The flow as previously written referenced UI that no longer matches the app; the
following corrections were made (no weakening of the smoke):

- composer placeholder `Message…` -> `Message`;
- added `tapOn: "Show composer options"` before the picker assertion (the
  lane/provider row is collapsed behind the `+` toggle by default);
- `assertVisible "Provider"` -> `assertVisible "Send with Claude"` (the row's
  own `Provider` is a non-leaf View accessibilityLabel Maestro cannot match; the
  lane pills are accessible buttons whose child text is absorbed into their
  `Send with <lane>` label) then collapse the row so the bare `Send` button is
  unambiguous.

## Public-Safe Boundary

Records only public-safe metadata and visible labels. Contains no tokens,
credentials, chat bodies beyond the intentional public smoke message text, raw
sync rows, or private local machine data.
