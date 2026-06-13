# AFK Autonomous Loop — standing instructions + live state

Date: 2026-06-13. Purpose: let the coordinator (Claude) drive the Autopilot
clients + self-driving-loop + OTA buildout autonomously for hours while the owner
is AFK, via `/loop`. This doc is the **source of truth** the loop re-reads each
iteration (so it survives context compaction). Append progress to §6.

## 0. The end-to-end goal ("works end to end")

The machine-economy loop, all on **our own machine + Cloud**, no Expo cloud:
phone-composed intent → Pylon hears it → coordinator fans out coding agents →
on merge, classify ship-mode (OTA vs rebuild) → ship back to the phone (OTA via
our own **OpenAgents Updates** server; native via a **local** build → App Store
Connect). "Done for now" = (a) the OTA MVP server runs locally and serves a
valid Expo-Updates manifest from a real `expo export`, and (b) `eas build
--local` produces an `.ipa` on this Mac. Full on-device OTA needs a build whose
`updates.url` → our server (gated on a working local build).

## 1. Each loop iteration — do this, in order

1. **Reconcile in-flight work.** For any running `multi-session-run` (check
   `/tmp/*-proofs/multi-session-summary.json` + the runner PID): if its summary
   exists, review each session's diff, **run its verify from `main` yourself**
   (a session marked `failed` is often a transient runner/redaction hiccup — the
   code may be fine; trust the direct verify), then commit + merge the clean
   ones into `main`, run the touched tests on `main`, push, comment/close the
   issue, and remove the worktree + branch + `.pylon-*`/`/tmp` artifacts.
   **Never merge a red verify.**
2. **Monitor builds.** For any EAS/local build in flight: poll status. On
   `FINISHED`, proceed (e.g. it auto-submits). On `ERRORED`, fetch the build log
   (`mcp__expo__build_logs` or the local log), grep the bundle/error phase,
   diagnose, fix on `main`, push, and rebuild — the same self-correct loop that
   already fixed babel-preset-expo + metro hierarchical-lookup.
3. **Advance the OTA end-to-end.** Once the OTA pure cores merge
   (manifest-resolver, asset-store, publish-builder, code-signing), wire the
   `apps/oa-updates` Bun HTTP server (manifest endpoint + `/assets/:hash`),
   run it locally, do `expo export` → publish one update → curl the manifest
   endpoint with the right `Expo-*` headers and confirm a valid manifest. Log
   the result. (This is owner-visible proof the protocol works on our infra.)
4. **Keep the local build moving.** Get `eas build --platform ios --profile
   production --local` to produce an `.ipa`. Known prereqs installed this
   session: fastlane ✓. cocoapods (installing). Install any further missing
   local toolchain via `brew`. Run a local `expo export` pre-check before builds.
5. **Keep fanning out.** If no fanout is running and there is unblocked,
   bounded, file-disjoint work on the roadmap/CND backlog, launch the next batch
   (≤6 sessions, codex pool below, pre-provisioned worktrees off `main`,
   `noNetwork:false`, exact test-file verify, "keep edits to these files",
   "don't touch src/index.ts / root package.json"). Always pre-provision each
   worktree with `bun install`. This keeps the machine producing while AFK.
6. **Pace yourself.** Use ScheduleWakeup to re-enter: ~270s while actively
   watching a build/fanout, ~1200s when idle/waiting. When a tracked background
   watcher will re-invoke you on completion, prefer the long fallback.

## 2. Guardrails (hard)

- Work on `main` only; commit + push every merge (each repo separately:
  `openagents` and `cloud`). Co-author trailer as usual.
- **Cost:** prefer **local** builds; do NOT spin up more than one EAS *cloud*
  build per real need, and do not auto-submit repeatedly — only submit a build
  that's a meaningful new test target. Owning OTA is the whole point; lean on it.
- **Escalate, don't stall.** If blocked on something only the owner can do
  (Apple 2FA prompt, a spend/credential decision, a GitHub-org/connection,
  anything needing a human at apple.com), STOP that one thread, write a clear
  `NEEDS-OWNER:` line in §6, and keep working the other threads. Never idle.
- Never print raw tokens/secrets/credentials/mnemonics into files, commits,
  issues, or logs. Keep proof artifacts refs-only.
- Verify before merge; bounded fanout; clean up worktrees; don't create
  duplicate issues or re-do merged work.
- Don't change user-facing marketing copy.

## 3. Fanout mechanics (reference)

- Runner: `bun apps/pylon/scripts/multi-session-run.ts --plan <plan.json>
  --proofs-dir <dir> --pylon-home <dir> --concurrency N --run-id <id>` (nohup,
  background). Object-shaped plan with run-level `accountPool`.
- Codex account pool (all 7, for failover):
  `~/.codex`, `~/.codex-pylon-b`, `~/.codex-pylon-c`, `~/.codex-pylon-d`,
  `~/.codex-pylon-e`, `~/.codex-pylon-f`, `~/.codex-pylon-g`.
- Runbook: `docs/autopilot-coder/pylon-multi-session-agent-runbook.md`.
- A fresh worktree has no `node_modules` → `bun install` it before the run so
  `bun test` verifies pass.

## 4. Live backlog (fannable when unblocked)

- **OTA server (cloud `OpenAgentsInc/cloud` #78–85):** pure cores fanned out now
  (manifest-resolver/asset-store/publish-builder/code-signing → `apps/oa-updates`);
  then HTTP wiring (coordinator-side), then port to Cloud Rust.
- **M6 integration (openagents #4940–4947):** CL-34 RN compose screen, CL-36
  coordinator dispatch wiring, CL-38 publish via our server, CL-39 local build,
  CL-40 status round-trip, CL-41 gating wiring.
- **M3 read surfaces (#4921–4930), M5 polish (#4935–4939):** mostly need M2
  transport for live data; pure view-models/components are fannable.
- **Client (#4949 updates.url→ours + cert, #4950 local build pipeline).**

## 5. Current state snapshot (update as it changes)

- TestFlight: build #4 (`46596803`) FINISHED + auto-submitting (new Control icon
  + OTA runtime `d36a2a5b`, channel `production`). Earlier #2/#3 errored
  (babel-preset-expo, metro hierarchical-lookup) — fixed.
- expo-updates wired on `main` (updates.url currently → `u.expo.dev`; switch to
  ours is #4949).
- OTA pure-core fanout RUNNING (run-id `run.local.ota`, watcher active).
- Local build: **WORKING** — `eas build --local` produced an 8.7 MB `.ipa` on
  this Mac (`/tmp/autopilot-local.ipa`), no Expo cloud. Prereqs: fastlane +
  cocoapods (both installed via brew). The "no Expo cloud build" half is proven.
- Merged so far: protocol (CL-0/1), autopilot-ui (CL-2) + web wiring, desktop
  P0 + RPC (CL-5), mobile control core + metro + token-store (CL-6), expo-updates
  config + policy (#4920), M6 cores (CL-34/35/36/37/41) + tokens (CL-42),
  CL-29/30/33 cores. Cloud OTA audit committed (`e083509`).

## 6. Progress log (append one line per iteration; newest at bottom)

- 2026-06-13: loop initialized; OTA pure-core fanout launched; cocoapods
  installing for local build; build #4 → TestFlight.
- 2026-06-13 iter 1: merged OTA pure cores CND-OTA-1..4 (`917e2bd16`, 14 tests),
  commented cloud #78–81; registered `apps/oa-updates` package (`8936bd4d2`);
  launched OTA server+CLI fanout (`run.local.ota2`); **LOCAL BUILD WORKS** —
  8.7 MB `.ipa` built on-machine (fastlane+cocoapods), no Expo cloud. Next:
  merge server/CLI, wire end-to-end manifest test, then a build with
  updates.url→ours.
- 2026-06-13 iter 2: merged OTA HTTP server + publishExport (`331e70563`, 18
  tests); registered `apps/oa-updates`; **E2E PASS** (`3ae455ee4`) —
  `scripts/e2e-local.ts` serves a real Expo-Updates manifest + asset over HTTP on
  our own infra (fingerprint gate works). OTA protocol proven, no Expo cloud.
  Remaining for on-device: sign manifests, deploy server publicly, switch
  updates.url (#4949), rebuild (local build works). Launching next OTA fanout:
  real-export CLI + signed manifest response.
