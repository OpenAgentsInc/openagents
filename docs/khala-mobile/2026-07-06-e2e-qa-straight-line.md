# Khala Mobile E2E QA for the straight line (MM-I3, #8492)

Date: 2026-07-06

Status: covers what is genuinely testable end to end today, on both
platforms, without the cloud-execution lane (#8474-#8479, still in progress
behind the landed #8473 org-executor spine) and without a seeded public-safe
test GitHub account. Extends `docs/khala-mobile/2026-07-05-qa-swarm-mobile-
adaptation.md`'s four-check-kind framing (scripted scenarios / seeded
monkeys / LLM explorers / perf probes) and the existing Maestro smoke work
from #8490.

## 1. What's covered end to end today (both platforms)

### 1.1 Scripted device-level Maestro flows (real emulator/simulator, real build)

Both flows below are the SAME `.yaml` file run unmodified on iOS (simulator)
and Android (emulator) — real cross-platform reuse, not a duplicated flow per
platform:

- `clients/khala-mobile/.maestro/flows/LaunchFallback.yaml` — fresh install,
  clear state, launch, assert the sign-in screen renders (title, primary
  CTA, tagline).
- `clients/khala-mobile/.maestro/flows/LaunchGitHubSignInInteraction.yaml`
  (added in #8490) — taps "Sign in with GitHub" and asserts the app hands off
  to a real external browser surface, proving the control is genuinely
  interactive.

Receipts: `docs/khala-mobile/2026-07-05-maestro-launched-app-smoke-receipt.md`
(iOS) and `docs/khala-mobile/2026-07-06-android-emulator-launch-smoke-
receipt.md` (Android, this pass's predecessor issue).

This is the honest ceiling for a **fresh, credential-less install** on either
platform: GitHub OAuth cannot be scripted headlessly (no test GitHub account
credentials exist in this environment, and completing real OAuth requires a
real account, a real browser session, and — for a fresh GitHub account —
possibly 2FA). `SignedInThreadSmoke.yaml` therefore stays blocked on a seeded
account exactly as already documented; see §3 for exactly what unblocks it.

### 1.2 Real React Native component-mount coverage (bun test, no device needed)

Extends the harness `tests/support/rn-test-environment.ts` first proved out
by `tests/chat-composer.test.tsx` (#8489/prior work) one screen further along
the straight line:

- **New this pass:** `tests/repo-picker-screen.test.tsx` mounts the REAL
  `RepoPickerScreen` — including the REAL (unmocked) `KhalaListItem` and the
  REAL (unmocked) `khala-mobile-repos-api` client against a scripted
  `globalThis.fetch` — and proves: real load-and-render, real search-filter
  wiring through the real pure `khala-mobile-repo-search-core` functions, a
  real repo-select calling the sync runtime's real `bindThreadRepo()` with
  the correct payload, and a real error-branch render on a failed fetch.
  Landed as the new `khala_mobile.repo_picker.rn_component_mount_coverage.v1`
  enforced contract.
- This required extending the shared harness with a **`FlatList` leaf stub**
  (`tests/support/rn-test-environment.ts`): real `FlatList` needs a real
  `ScrollView`/native scroll bridge with no meaning under `bun test`, so the
  stub renders `data.map(renderItem)` eagerly inside a plain `View` — no
  virtualization/windowing, which has no non-native equivalent anyway. This
  is now available for any future screen test that needs a real list.
- **A real, hard-won lesson recorded for future test authors:** `bun:test`'s
  `mock.module` mutates the GLOBAL module registry for the WHOLE `bun test`
  process, not just the file that calls it. An earlier draft of this test
  globally mocked `khala-list-item` and `khala-mobile-repos-api`, which
  silently broke `khala-ui-primitives.test.tsx` and
  `khala-mobile-repos-api.test.ts` (both need the REAL modules) the moment
  the FULL suite ran, even though the new file passed in isolation. Fixed by
  a rule now documented in the test file's own header: only `mock.module` a
  dependency that NO other test file needs for real (verified by grep before
  writing each mock); use a locally-scoped fake (a `globalThis.fetch` swap,
  restored in `afterAll`) for anything else. A second, subtler instance of
  the same class of bug surfaced during this same pass: `tests/crash-
  reporting.test.tsx` globally mocks `khala-text` to a different host-tag
  name, which flipped a `findAllByType("Text")` assertion to empty in the
  full-suite context — fixed by asserting on rendered text CONTENT across
  all node types rather than a specific host tag name. Any future screen-mount
  test in this package should budget time to run `bun test` (the FULL suite,
  not just the new file) before considering the work done, precisely because
  this class of bug only reproduces there.

### 1.3 What this proves about the straight line, honestly

Per the audit's straight-line definition (`docs/fable/2026-07-05-khala-code-
mobile-only-mvp-launch-audit.md` §4), steps 1-4 (install, sign in, land with
$10 visible, pick a repo) now have real coverage split across two layers:

- Real, device-level, cross-platform proof that the app **launches** and the
  sign-in control is **interactive** (§1.1).
- Real, component-level proof that the **repo-picker step's own logic**
  (load/search/select/error) is correct (§1.2).

What remains uncovered by either layer, honestly: a single continuous run
from a FRESH install through a completed GitHub sign-in, through landing on
the $10-grant onboarding screen, through picking a real repo, through
composing and dispatching a real task — because that continuous run needs a
real, seeded, public-safe GitHub test account, which does not exist in this
environment. See §3.

## 2. A real signed-in session was found on this development Mac (bonus evidence, handled carefully)

While preparing this pass's iOS build, an iPhone 17 Pro simulator on this
machine (UDID `2E5DFC26-DB79-4EE2-BF8E-2EB486A1AFBA`) was found to already
have a **persisted, real signed-in Khala Mobile session** from prior
dogfood/testing work (visible threads: "Claude desktop-to-mobile sync test",
"Mobile-started Claude session test", "Mobile-started Codex session test",
"Codex desktop-to-mobile sync test", "Issue 8413 web Khala Sync verify",
"8405 QA: lane picker + badge" — clearly prior real dogfood/QA threads, not
anything created by this pass).

This was used, carefully, as READ-ONLY bonus evidence:

- Installed this pass's freshly built app on that simulator (`simctl install`
  — does not clear existing app data/Keychain on the same simulator UDID)
  and launched it: the real signed-in thread list rendered
  ("6 threads | 11 messages | latest 13h").
- Opened an existing real thread ("Claude desktop-to-mobile sync test") via a
  scoped, non-destructive Maestro flow (deliberately NOT using the shared
  `_OnFlowStart.yaml`, which always clears state — that would have destroyed
  this session): real synced messages rendered, plus the real
  "No repo — tap to pick one" repo-binding chip.

**What this pass deliberately did NOT do, and why:** it did not extract,
print, or persist the session's bearer token/owner-user-id from Keychain/
SecureStore for reuse (e.g., as `EXPO_PUBLIC_KHALA_SYNC_DEMO_*` env vars to
make `SignedInThreadSmoke.yaml` scriptable) — that credential belongs to
whoever ran the prior dogfood session, and extracting a live user's bearer
token for reuse in an automated test flow is a real credential-handling
overreach regardless of how it was obtained. It also did not send a new chat
message or dispatch a new task through this session: with #8473 (the
org-cloud executor) now landed, doing so could trigger a REAL cloud-executed
turn and REAL credit spend against what appears to be a real/shared dogfood
account — not something this launch-ops/QA lane has authorization to spend
without the owner's explicit say-so. This finding is recorded as evidence
that the straight line's sync/thread-render layer works for real, live, today
— not as a reusable or scriptable test fixture.

**The actionable takeaway:** a genuinely disposable, public-safe demo GitHub
account (with its Khala Sync `ownerUserId`/token captured once) would let
`SignedInThreadSmoke.yaml` — and a real full-straight-line Maestro flow —
run as a normal, repeatable, CI-style regression test instead of depending on
an ambient, undocumented, hard-to-reproduce signed-in simulator state. See
`~/work/NEEDS_OWNER.md`.

## 3. What's blocked, and exactly what unblocks it

| Gap | Blocker | What unblocks it |
|---|---|---|
| A scriptable, repeatable `SignedInThreadSmoke.yaml` run (either platform) | No seeded public-safe test GitHub account exists in this environment | Owner provisions a disposable test GitHub account, signs in once via the app, and captures `EXPO_PUBLIC_KHALA_SYNC_DEMO_OWNER_USER_ID`/`EXPO_PUBLIC_KHALA_SYNC_DEMO_TOKEN` (or an equivalent captured Khala Sync bearer) for CI/agent use — see `~/work/NEEDS_OWNER.md` |
| A full straight-line Maestro flow (sign-in through a completed cloud-executed turn + push notification) | (a) the seeded-account gap above, AND (b) #8474-#8479 (credit-gated dispatch policy, private-repo checkout, isolation posture, branch/PR writeback, metering) are not all landed yet behind #8473's org-executor spine | Land (a) plus the remaining C-lane issues; this doc's flow should be extended the moment both are true — tracked here explicitly rather than silently assumed |
| The zero-balance -> purchase-sheet paywall moment | IAP is postponed for the first MVP build (2026-07-06 owner decision) | N/A until IAP client work (#8481) resumes post-MVP |
| A nightly nightly-matrix analog for mobile | Needs (1) above plus a scheduling surface | Post-MVP-adjacent follow-up once the seeded account exists |

## 4. Contract oracles from MM-H3 in the normal sweep

Confirmed still true this pass: `clients/khala-mobile/tests/ux-contracts.ts`
(registry, now version `2026-07-06.1`) and `tests/ux-contracts.test.ts` (the
oracle file) run inside the normal `bun test` sweep for this package, which
is itself part of the repo-root `bun run test` sweep before pushes to `main`.
The registry now carries 10 enforced contracts (up from 9 — this pass added
`khala_mobile.repo_picker.rn_component_mount_coverage.v1`) and the same 4
honest `pending` contracts with named blockers as before this pass.

## 5. Verification run this pass

```sh
cd clients/khala-mobile
bun test          # 309 pass, 0 fail (up from 305; 4 new repo-picker-screen tests)
bun run typecheck # clean
bun run architecture:check  # no dependency violations (251 modules, 698 deps)
```

Re-ran the full `bun test` suite 4 times in a row to confirm the fixes above
were not order-dependent flakes; all 4 runs green.
