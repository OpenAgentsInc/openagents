# DIST-05 target-resolution and retained-slot receipt

Date: 2026-07-16
Issue: #8918
Scope: code-owned common update host plus the existing macOS applier

## What this receipts

- ReleaseSet v2 canonical bytes and pinned Ed25519 envelope are verified before
  host target selection. The native host architecture is separate from the
  running application architecture, so an x64 app under Rosetta selects the
  arm64 DMG as an explicit full-artifact migration.
- Unknown/missing/ambiguous targets or formats, channel/version regressions,
  minimum-OS failures, digest/length mismatch, platform-applier mismatch, and
  executable-architecture mismatch fail closed.
- Replacement does not begin until durable evidence proves all four owner-data
  roots are absolute and outside the replaceable application bundle, the
  migration reducer admits exact present-path/type evidence or the two typed
  legitimate absences (`no_sessions` and signed-out vault), and the bounded child-runtime drain
  completes. A wedged agent,
  PTY, server, helper, window, or WSL class prevents native mutation.
- The durable host retains interrupted staged state, treats interrupted apply
  as rollback recovery, and records native install as awaiting a transaction-
  bound health receipt. It does not project success at native swap time.
- Runtime B is allowed to continue startup. It must observe renderer load and
  provider startup, then complete the awaited six-class lifecycle drain. A
  failed drain writes no receipt. The receipt is the final durable action
  immediately before the drained process exits. It is never written from the
  start of Electron's `before-quit` event. Only Runtime C
  may accept the typed receipt (schema, app, exact version, exact transaction,
  renderer/provider readiness, clean-shutdown timestamp) and expose the
  retained slot as the one manual rollback option.
- The macOS detached watchdog verifies the rollback app before it is armed,
  parses every typed receipt field exactly, and re-verifies bundle ID, version,
  executable architecture, Developer ID signature, team, Gatekeeper policy,
  and notarization immediately before a timeout swap. A failed verification is
  persisted as `rollback_failed`. The unverified slot is never installed.
- The native transaction temporary file and parent directory are explicitly
  synchronized through Foundation `NSFileHandle.synchronizeFile` before the live app is replaced through
  Foundation's atomic `replaceItemAtURL` primitive. The exact JXA selector is
  exercised against disposable native directories. Manual and watchdog
  rollback use the same primitive and persist `rollback_prepared` first, so
  neither install nor rollback creates a process-crash window with no app at
  the launch path.
- Native `rolled_back` is consumed before interrupted-operation fallback. The
  host durably publishes `rollback_cleanup_pending` (state temp fsync, rename,
  and parent fsync) before deleting the native transaction or retained slot,
  then durably publishes idle. Every cleanup crash point resumes idempotently.
- The renderer receives only bounded phase, channel, versions, and typed reason.
  artifact URLs, local paths, and transport errors remain main-owned. Update
  IPC is request/response only, so there is no renderer subscription to leak.

## Verification commands

```sh
pnpm --dir apps/openagents-desktop run typecheck
pnpm exec vp test --run --max-concurrency 1 \
  apps/openagents-desktop/src/update-staging-host.test.ts \
  apps/openagents-desktop/src/update-runtime-drain.test.ts \
  apps/openagents-desktop/src/update-staging-integration.test.ts \
  apps/openagents-desktop/src/update-migration-evidence.test.ts \
  apps/openagents-desktop/src/macos-update-applier.test.ts \
  apps/openagents-desktop/tests/release-set-contract.test.ts \
  apps/openagents-desktop/tests/update-contract.test.ts \
  apps/openagents-desktop/tests/update-rollback.test.ts
```

Focused repair result at handoff: 64 tests passed across the eight update and
release contract files, zero failed. Desktop typecheck passed. The production
build passed. The full Electron smoke now registers before the renderer-ready
await and passes every step through `codex-trace-acceptance`. It then reaches
the same unrelated current-main settings/account-link assertion failure as an
untouched `origin/main` worktree (`openAgentsLinkPresent:true`) and tears down
with `{"ok":true,"active":0}`. The full Desktop suite reached 1,819 passed with 39
declared skips and one unrelated failure already present on `origin/main`: the
new account-linking renderer test expects a `data-session-phase` element that
the current shared renderer does not emit. DIST-05 does not modify that file.

## Honest boundary

No privileged Windows or Linux installer was invoked on this Darwin host.
DIST-05 supplies their shared target/applier/lifecycle contract and fake-host
coverage. Native NSIS, AppImage, DEB, and RPM execution receipts remain the
close rules of DIST-06 through DIST-08. Live promoted-feed proof also depends
on DIST-09. Those dependencies do not weaken the fail-closed local host path.
