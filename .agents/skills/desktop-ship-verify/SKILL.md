---
name: desktop-ship-verify
description: >-
  Use when building, installing, shipping, or verifying changes to OpenAgents
  Desktop (/Applications/OpenAgents.app), Desktop workers (inky, agyworker),
  or paired Mac daemon / iOS Tailscale flows.
---

# OpenAgents Desktop Ship & Live Verification Runbook

## 1. Ground-Truth Check (Before Debugging or Editing)
Never guess what build is installed from `ls -ld /Applications/OpenAgents.app`. Inspect `Info.plist` directly:

```sh
/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" \
  -c "Print :CFBundleVersion" -c "Print :OACommit" -c "Print :OAProfile" \
  /Applications/OpenAgents.app/Contents/Info.plist
git fetch origin main && git rev-parse --short origin/main
```

## 2. Develop & Test in a Fresh Worktree
- Branch from `origin/main`: `git worktree add ../oa-<topic> origin/main`.
- Fast local compile check: `apps/openagents-desktop/build.sh --fast` (defaults to `--minimal`, `-Onone` Swift).
- Run touched Coax page/desktop unit suites:
  ```sh
  ./coax/bin/coax test apps/openagents-desktop/page.coax --library
  ./coax/bin/coax test coax/lib/inky/loop.coax --library
  ```
- Verify bundle layout:
  ```sh
  coax/web/tool.sh apps/openagents-desktop/v1-bundle-check.coax apps/openagents-desktop/build/OpenAgents.app
  ```

## 3. Mandatory Live E2E Verification (Do NOT Skip)
Verify the **actual user flow** affected by your change before claiming done:
- **UI / inputs / clicks:** Run interactive pointer/keyboard dispatch through AppKit/host (or Tier 3 UI eval `evals/openagents-desktop/run.sh --tier 3`), not just `cp_press` unit tests or static `--shot` PNGs. Check empty-state visuals and `file://` / worktree clicks.
- **Local Inkling (`--minimal` uses `inky`, NOT `mcworker`):** Run a timeout-bounded live `req#1` test (`timeout 60 ...`) against `/Applications/OpenAgents.app/Contents/Resources/inky` verifying streamed thoughts + non-empty reply.
- **Antigravity (`agyworker`):** Always set `SSH_CONNECTION=1` for diagnostic `agy` probes; verify real model list + escalated command execution in `auto` mode.
- **Mobile / Tailscale / Daemon:** Verify `DescribeEnvironment` -> `ListSessions` (against real `~/.openagents/coder/host/runs` session count, confirming `< JCS_MAX_KEYS`) -> `CreateSession` -> Desktop `sweepRuns()` -> `ReadSession`.

## 4. Canonical Ship & Install (Never Hand-Copy Branch Builds)
1. Rebase onto `origin/main` and push (`git push origin HEAD:main`, with `Co-Authored-By:` and `Agent:` trailers).
2. Run `apps/openagents-desktop/install.sh` (defaults to `--minimal`; installs from `origin/main`, runs `v1-bundle-check.coax`, syncs `~/.openagents/bin/openagents-environment`).
3. Check the exit status!
   - Exit `0`: installed and launched. Re-read `Info.plist` `OACommit` to confirm it matches your pushed SHA.
   - Exit `4`: deferred (active chat replies or lock held). Report honestly or, if the user explicitly requested an immediate reinstall, re-run with `OA_INSTALL_FORCE=1 apps/openagents-desktop/install.sh`.
4. Clean up your temporary worktree (`git worktree remove ../oa-<topic>`).
