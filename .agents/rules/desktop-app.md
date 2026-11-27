---
trigger: glob
globs: "apps/openagents-desktop/**, apps/shared/**, coax/lib/agent/desktop/**, openagents/runtime/**, openagents/daemon/**, openagents/workers/**"
description: OpenAgents Desktop --minimal build/install contract, live UI E2E verification, and worker hygiene.
---

# OpenAgents Desktop & Worker Rules

- **`--minimal` is the primary build:** `apps/openagents-desktop/build.sh` and `install.sh` default to `--minimal`. Never build `--full` unless explicitly requested. Remember `--minimal` runs `oa-worker` (`openagents/runtime/oa-worker.coax`), `claudeworker` (`openagents/workers/claudeworker.coax`), `gitguard` (`openagents/workers/gitguard.coax`), and `coder-host` (`openagents/daemon/host.coax`) — NOT legacy `mcworker`.
- **Single canonical install path:** Never hand-copy an uncommitted local build into `/Applications/OpenAgents.app` (the 15-minute LaunchAgent and concurrent `install.sh` runs overwrite it from `origin/main`). Always commit + push to `origin/main` first, then run `apps/openagents-desktop/install.sh` (verify exit code `0`, not deferred exit `4`; if deferred by idle/stale runs and user requested immediate install, inspect and use `OA_INSTALL_FORCE=1`).
- **Verify installed version truthfully:** Read `/Applications/OpenAgents.app/Contents/Info.plist` keys `OACommit`, `OAProfile`, and `CFBundleVersion` — never guess from `ls -ld OpenAgents.app` mtime.
- **No unasked feature flags:** User-requested core flows land enabled in `--minimal` by default unless the user explicitly asks to gate them behind `coax/lib/agent/desktop/flags.coax`.
- **Mandatory live E2E checklist before claiming done:**
  - UI/input/focus changes: verify real pointer/keyboard dispatch through AppKit/host (not only `cp_press` unit tests or static `--shot` PNGs), empty-state visual sanity, and `file://` / worktree path clicks where relevant.
  - Streaming/worker changes: verify live `req#1` token/thought streaming on the actual installed `/Applications/OpenAgents.app/Contents/Resources/{inky,agyworker}` binary with a hard timeout wrapper.
  - Provider/auth changes: always run diagnostic `agy` commands with `SSH_CONNECTION=1`; verify real model list & escalated command execution in `auto` mode.
  - Mobile/daemon protocol changes: merge & reinstall the Mac Desktop + `com.openagents.coax.environment` daemon too, and test `DescribeEnvironment` -> `ListSessions` (against real user session count, checking `JCS_MAX_KEYS`) -> `CreateSession` -> Desktop `sweepRuns()` -> `ReadSession`.
- **No third-party branding leaks:** Never surface retired/third-party product names or logos (`Zeron`, Anthropic logo next to Inkling) in UI or docs.
- **For the step-by-step release/verify checklist**, load the `desktop-ship-verify` skill.
