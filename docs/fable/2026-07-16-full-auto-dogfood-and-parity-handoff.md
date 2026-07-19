# 2026-07-16 — Full Auto dogfood + multi-agent parity handoff (Fable)

Session-end handoff from the Fable coordinator session that landed the Full Auto
hardening program and began the parity/QA phase. Written at usage-limit cutoff
~10:20 CDT. Two subagent lanes were still running.

## What landed on main today (this session's lanes)

- **Full Auto epic #8873 CLOSED** — all 13 children (#8874–#8886) merged and
  individually closed. Final state: ProductSpec `specs/desktop/full-auto.product-spec.md`
  rev 7 (FA-AC-01..28) + regenerated AssuranceSpec. Durable exactly-once
  continuation, workspace fail-closed binding, failure backoff, live in-flight
  UI state, background question auto-resolve, two-process restart smoke
  (`pnpm run smoke:full-auto-restart`), and the loopback OpenAPI control
  surface with MCP + CLI clients (`pnpm run full-auto`, `smoke:full-auto-control`).
- **Programmatic bootstrap (FA-AC-28)** — `POST /v1/full-auto/start`
  (OpenAPI `startFullAuto`, MCP `full_auto_start`, CLI
  `start --workspace <path> [--title <t>]`) mints a brand-new thread, binds the
  resolved workspace, enables, and schedules the first continuation in one
  fail-closed call. This closed the gap where enable/continue-now could only
  operate on threads the UI had created (dispatch on an unknown threadRef
  failed with "That conversation no longer exists."). Commit `3967982eaa`.
- **/stats live in production** — the Start route now fetches
  `khala-tokens-served` (+history, model-mix, channel-mix), `pylon-stats`, and
  `forum/launch-status`. Deployed to the `openagents-monolith` Cloud Run
  service. Tip leaderboards honestly render the 410 `money_surface_retired`
  state. Gap filed as #8911: Desktop local turns (incl. Full Auto) never reach
  `token_usage_events` — the counter excludes them until consent-gated
  ingestion lands.
- **ACP-9 #8896 CLOSED** — trusted peer profiles + fail-closed registry
  admission (`packages/agent-client-protocol/src/profiles/`), Grok and Cursor
  reference profiles, 48 tests. Commit `1e5af93521`.
- **QA-2 #8907 CLOSED** — observer execution loop (`pnpm run qa:observer`):
  typed check registry (7 seeded production checks), dated JSON artifacts,
  honest pass | drift | unrunnable states, gated issue filing. First real
  production run committed: 7/7 pass. Commit `08096cae24`.
- **QA-5 #8910 CLOSED** — independent verifier (`pnpm run qa:verify`): claims
  re-run from clean checkouts, adversarial probes, self-verification refusal,
  unverifiable-here never auto-accepted. Demonstrated for real: ACCEPT verdicts
  posted on #8907 and #8886. Commit `11f6d6126d`. Recipe:
  `docs/qa/verifier/README.md`.

## The dogfood proof (the owner's directive: switch day-to-day dev to Full Auto)

A real end-to-end programmatic run happened on this Mac:

1. Desktop dev app launched with `OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL=1`,
   `OPENAGENTS_DESKTOP_LAUNCH_CWD=/Users/christopherdavid/work/oa-fullauto-dogfood`
   (clean worktree of origin/main), isolated `OPENAGENTS_DESKTOP_USER_DATA`.
2. `full-auto-cli start --workspace /Users/christopherdavid/work/oa-fullauto-dogfood
   --title "Full Auto dogfood"` → thread `828dc3c8-1c75-41bd-ad29-7ac75354241b`,
   first continuation dispatched by the reconcile pass, real Codex conversation.
3. **Turn 1 (autonomous):** the agent detected that the ACP conformance
   workflow files violated the repo's no-GitHub-hosted-CI invariant, made its
   own worktree, deleted the three `.github/workflows/*.yml`, updated
   `PEER_PROFILES.md` for owned-runner verification, filed AND closed #8905,
   and pushed `489dfbbf35` to main — catching a mistake the coordinator made
   landing ACP-9 via SSH.
4. **Continuations 1–2 (autonomous):** scanned open issues, chose #8911
   (token ingestion), and began implementing it consent-gated with the repo's
   Effect skill, delegating claim scans to child agents.

The loop was still cycling at session end (cap 20, backoff, durable across
restarts). The app + control server were left RUNNING. Manage with
`pnpm --dir apps/openagents-desktop run full-auto -- status|disable <threadRef>
--user-data <userData dir>` (connection info in `<userData>/full-auto/control.json`).

Operational gotcha discovered (cost four relaunch attempts): a running dev
instance retitles to `OpenAgents Dev` and holds Electron's per-userData
`SingletonLock`. Later launches lose `requestSingleInstanceLock()` and quit(0)
silently ~1.6s in, AFTER logging "full-auto-control listening" — so the log
looks healthy while control.json points at a dead port. Do not kill by title,
port, process group, `pkill`, or `killall`, and do not remove `Singleton*`
while an owned process may still live. Follow the exact-PID and private-control
endpoint procedure in
[`docs/sol/2026-07-16-full-auto-shared-mac-dogfood-runbook.md`](../sol/2026-07-16-full-auto-shared-mac-dogfood-runbook.md).
verify liveness with the CLI, never the log line.

## Coordination protocol (multi-lane)

Parallel lanes (Codex sessions, Full Auto threads, Fable subagents) coordinate
via issue comments: post `CLAIM` (actor/session, base commit, worktree/branch,
scope) before working an issue. `CLAIM-RELEASE: #<n>` on close. This is not
optional: this session a parallel Codex lane landed #8891
(`packages/agent-client-runtime-bridge`, `31165ee130`) while an unclaimed
Fable subagent built a complete duplicate that had to be discarded. Check for
unreleased CLAIMs before dispatching any agent.

## In flight at cutoff (integrate these)

Two claimed subagent lanes commit locally and need integration (cherry-pick
into a fresh detach of origin/main → run their named tests → push --no-verify
→ comment + close with CLAIM-RELEASE):

- `/Users/christopherdavid/work/oa-qa-3` → **#8908** desktop visual baselines
  (claimed `fable-qa-visual-20260716`): headless capture probe + committed PNG
  baselines + `qa:visual` diff gate.
- `/Users/christopherdavid/work/oa-l1-spi` → **#8899** provider lane SPI
  (claimed `fable-lane-spi-20260716`): one typed adapter for codex-local +
  claude_agent + fixture lane, envelope mappable from the
  `agent-client-runtime-bridge` canonical vocabulary.

If those worktrees are gone or the issues were closed by another lane, read
the issue comments before redoing anything.

## Open next (priority order)

1. Finish integrating #8908 and #8899 (above).
2. **#8911** — likely being finished by the Full Auto lane itself. Verify via
   its thread/turns before claiming.
3. Parity epic **#8898**: L2 #8900 (capability truth), L6 #8901 (Full Auto per
   lane), L7 #8902 (specs cross-lane), L8 #8903 (lane registry/switching) —
   L2/L6/L8 depend on L1 landing.
4. ACP epic **#8887**: ACP-5 #8892 (session lifecycle) → ACP-6/7 #8893/#8894
   (Grok/Cursor peer profiles) → ACP-8 #8895 (UX) → ACP-10 #8897 (release
   gate). The `codex-acp-runtime-bridge-*` lane has momentum here — check
   CLAIMs first.
5. QA epic **#8904**: QA-1 #8906 (six-lane swarm real run — use the verifier
   for acceptance), QA-4 #8909 (QA board UI over QA-2's artifact vocabulary).

## Key refs

- Full Auto control surface: `apps/openagents-desktop/src/full-auto-control-*.ts`,
  clients in `apps/openagents-desktop/scripts/full-auto-{cli,mcp,control-client}.ts`
- Spec: `specs/desktop/full-auto.product-spec.md` (rev 7) + AssuranceSpec
- QA: `docs/qa/observer/README.md`, `docs/qa/verifier/README.md`
- ACP: `packages/agent-client-protocol{,-conformance}/`,
  `packages/agent-client-runtime-bridge/`, teardown in `docs/teardowns/`
