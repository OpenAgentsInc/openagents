# P0.8 (#8543) unattended straight-line E2E — AgentFlampy wiring receipt

Date: 2026-07-09
Seed account: GitHub user `AgentFlampy` (public, created 2026-07-07)
Seed repo (repo-pick target): `AgentFlampy/openagents` (verified fork of
`OpenAgentsInc/openagents`)
Typed leg registry: `clients/khala-mobile/src/qa/straight-line-e2e.ts`
Runner: `clients/khala-mobile/scripts/straight-line-e2e-run.sh`
Receipt schema: `openagents.khala_mobile.straight_line_e2e_receipt.v1`

## What this receipt records

The owner-gated seeded test account for #8543 now exists and is wired into
the unattended harness: the seeded Khala credential (gitignored
`~/work/.secrets/khala-maestro.env`, never committed or printed) is the
sign-in identity baked by `scripts/build-seeded-ios.sh`, and the fork
`AgentFlampy/openagents` is the repo-pick target consumed by
`StraightLineRepoPick.yaml` through `KHALA_MAESTRO_REPO_FULL_NAME`.

Legs that cannot run yet are recorded as TYPED BLOCKED skips with named
blockers — never fake passes:

- `ios_repo_pick_fork_bind` + `credits_grant_visible_drain` — gated on the
  mobile-OpenAuth-USER-session invariant: `GET /api/mobile/repos` and the
  credits routes 401 the seeded agent token BY DESIGN (verified live this
  session; see docs/khala-code/receipts/2026-07-07-qam-4-populated-happy-path.md).
  One interactive GitHub sign-in as AgentFlampy (live NEEDS_OWNER.md ask)
  opens this gate; the runner probes the route each run and starts running
  the fail-closed fork-bind flow automatically once it opens.
- `push_writeback` — gated on CX-3's in-VM cloud-execution lane (#8547).

## Prod regression found and fixed during wiring (chat push 500s)

The headless seam probe (this harness's dispatch leg, run before the device
flows) found EVERY `chat.*` Khala Sync mutator returning
`500 {"code":"internal"}` in prod for ALL accounts (`chat.appendMessage`,
`chat.createThread`; `runtime.*` mutators unaffected) — the app's Send was
broken server-side. Root cause: Cloud SQL prod was missing khala-sync-server
migrations 0047–0050; commit `ffb157415f` (CX-6) shipped chat-mutator code
that SELECTs the migration-0049 `codex_continuity_*` columns, so every chat
mutator threw `PostgresError` into the push route's opaque 500 catch-all.
Fix applied this session (2026-07-09, documented migration runbook path,
direct connection via cloud-sql-proxy): applied `0047_github_signup_credit_grants`,
`0048_runtime_intents_events_jsonb_object_normalize`,
`0049_chat_thread_codex_continuity`, `0050_khala_sync_fleet_steering`.
Verified after: `chat.appendMessage` → 200 `applied`.

Note: migration 0047 also restores the $10 GitHub-signup grant-tracking
writes, and 0050 unblocks the MH-6 fleet-steering mutators — both had been
shipping against missing tables.

## Headless seam probe — dispatch → live reply (GREEN)

With the seeded AgentFlampy token against prod `https://openagents.com`:

1. `POST /api/sync/push` `chat.appendMessage` (deterministic capital-of-France
   prompt) + `runtime.startTurn` (lane `hosted_khala`) → both `applied`.
2. Turn observed via `POST /api/sync/bootstrap` on the seeded thread scope:
   `queued` → `running` (t+29s) → `completed` (t+37s).
3. Assistant reply streamed as `runtime_event` `text.delta` containing the
   expected token (`Paris`) at `2026-07-09T02:45:36.304Z`.

## iOS simulator run (unattended)

Platform: iPhone 17 Pro simulator, iOS 26.5, UDID
`2E5DFC26-DB79-4EE2-BF8E-2EB486A1AFBA`, Release configuration
`com.openagents.khala.mobile` built by `scripts/build-seeded-ios.sh` from this
commit's worktree with the CURRENT AgentFlampy creds baked (the previously
installed build was baked from the older `khala-mobile-emulator-test.env`
identity, which is why the 2026-07-08 baseline smoke honestly failed on the
thread-title assertion — build/runner identity drift this wiring eliminates).

## Result

**PASS on every runnable leg; every blocked leg recorded as a typed skip.**
Single unattended run of `scripts/straight-line-e2e-run.sh` (fresh per-run
threads, seeded turn-state reset, live user-session gate probe):

- `ios_signed_in_thread_smoke` — PASS (`SignedInThreadSmoke.yaml`: auto
  sign-in resolves, thread list renders, the fresh run thread opens, the
  composer lane picker ("Send with Claude") is visible, a typed message sends
  and renders).
- `ios_repo_picker_reachable` — PASS (`RepoPickerReachable.yaml`: the thread's
  repo chip opens the real RepoPickerScreen; "Pick a repo" header + search
  field element visible; list contents deliberately unasserted — user-session
  gated).
- `ios_dispatch_reply` — PASS (`SignedInThreadReply.yaml`: deterministic
  prompt sent on the default `hosted_khala` lane; the assistant reply token
  ("Paris") became visible in the transcript within the bounded wait — a real
  dispatch → live-update round trip on-screen, not just the sent bubble).
- `ios_repo_pick_fork_bind` — BLOCKED
  (`blocker.khala_mobile.repo_list_requires_github_backed_mobile_session`).
- `credits_grant_visible_drain` — BLOCKED
  (`blocker.khala_mobile.credits_routes_require_github_backed_mobile_session`).
- `push_writeback` — BLOCKED
  (`blocker.cx3.in_vm_cloud_execution_lane_missing.openagents#8547`).

An earlier same-day run hit one infra flake worth recording: the
`ios_repo_picker_reachable` Maestro invocation failed with an XCUITest-driver
`Connection refused` between back-to-back flows (no app assertion failed; the
next flow re-provisioned the driver and passed). The leg passed standalone and
in the clean full run below.

Machine receipt: `clients/khala-mobile/var/straight-line-e2e/straight-line-e2e.latest.json`
(schema `openagents.khala_mobile.straight_line_e2e_receipt.v1`; gitignored
run artifact — the clean 2026-07-09 run is inlined verbatim below).

```json
{
  "schema": "openagents.khala_mobile.straight_line_e2e_receipt.v1",
  "issue": 8543,
  "generatedAt": "2026-07-09T03:18:40Z",
  "platform": "ios-simulator",
  "simUdid": "2E5DFC26-DB79-4EE2-BF8E-2EB486A1AFBA",
  "seedAccount": "AgentFlampy",
  "seedRepo": "AgentFlampy/openagents",
  "mobileUserSessionGate": "blocked",
  "legs": [
    {"id": "ios_signed_in_thread_smoke", "status": "PASS", "detail": "SignedInThreadSmoke.yaml"},
    {"id": "ios_repo_picker_reachable", "status": "PASS", "detail": "RepoPickerReachable.yaml"},
    {"id": "ios_dispatch_reply", "status": "PASS", "detail": "SignedInThreadReply.yaml"},
    {"id": "ios_repo_pick_fork_bind", "status": "BLOCKED", "detail": "blocker.khala_mobile.repo_list_requires_github_backed_mobile_session"},
    {"id": "credits_grant_visible_drain", "status": "BLOCKED", "detail": "blocker.khala_mobile.credits_routes_require_github_backed_mobile_session"},
    {"id": "push_writeback", "status": "BLOCKED", "detail": "blocker.cx3.in_vm_cloud_execution_lane_missing.openagents#8547"}
  ]
}
```

## Android emulator run (unattended)

Platform: Android emulator `emulator-5554` (Pixel-class AVD), Release
`app-release.apk` built by `scripts/signed-in-thread-smoke-android-run.sh`'s
bake path from this commit's worktree with the CURRENT AgentFlampy creds
baked (verified in `assets/index.android.bundle` via the strings-based
check), installed via adb. Each flow drove a fresh per-run thread created
through `chat.createThread`.

- `SignedInThreadSmoke.yaml` — **PASS** (auto sign-in resolves, fresh thread
  opens, "Send with Claude" lane picker visible, message sends and renders).
  Screencap kept in the local run-artifact dir
  (`clients/khala-mobile/var/straight-line-e2e/`, gitignored).
- `SignedInThreadReply.yaml` — **PASS** (deterministic prompt sent on
  `hosted_khala`; the assistant reply token ("Paris") became visible in the
  transcript within the bounded wait).

This upgrades the Android evidence from QAM-6's launch + GitHub sign-in
handoff to a full seeded SIGNED-IN interaction + live server reply on the
emulator — the same runnable legs as iOS, on the same seed identity.

## What remains for #8543 (honest)

- One interactive GitHub sign-in as AgentFlampy (live NEEDS_OWNER.md,
  "Khala Mobile P0.8") → opens the repo-bind + credits legs.
- CX-3 #8547 in-VM lane → opens the push/writeback leg.
- The repo-bind + credits legs repeated on Android once the session gate
  opens (the runnable Android legs — smoke + reply — are green above).
- Owner promises/copy pass against the completed dual-platform receipts.
