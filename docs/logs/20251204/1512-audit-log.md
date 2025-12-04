# 1512 Work Log

Audit of codebase for refactors and code smells. Tests not run per user instruction.

## Findings
- src/agent/orchestrator/worktree-runner.ts:90-129 hard-codes merging to `main` (checkout/pull/push) instead of using `ProjectConfig.defaultBranch`, so repos with a different default branch or work branches will fail or merge to the wrong target.
- src/agent/orchestrator/worktree-runner.ts:241-255 always runs `bun install` with no timeout, install args, or lockfile enforcement and ignores `ParallelExecutionConfig.installArgs/installTimeoutMs`; this can hang overnight runs and produce drift vs the lockfile.
- src/agent/orchestrator/orchestrator.ts:92-125 runs verification via per-command `execSync` inside Effect.try, blocking the event loop and discarding structured exit info; there is no streaming output, and failures only flip a boolean, making debugging and timeout handling brittle.
- src/agent/orchestrator/orchestrator.ts:155-179 stages with `git add -A` and shells a double-quoted commit message; this risks scooping unrelated workspace changes (especially when other agents left files dirty) and can still break on messages with quotes/backticks. Consider scoping staged paths and building the commit message via stdin.
- src/agent/overnight.ts:837-850 guardrail auto-runs `git checkout -- .` and `git clean -fd` after failures, which will delete user/agent untracked files and contradicts the repo safety guidance to avoid destructive resets. Needs an opt-in or narrower cleanup (tracked files only, no clean).
- src/agent/orchestrator/types.ts:190-196 defines `e2eCommands`, but `rg` shows no usage in `orchestrator.ts`, so configured e2e flows are silently skipped; Golden Loop acceptance (tests+e2e) is not enforced.
- src/llm/openrouter.ts:157-185 emits console logs for every request/response id and runs fetch with no timeout or abort signal; noisy in production and susceptible to hangs on slow networks. Should use a logger with levels and wrap fetch in a cancellable timeout.
- src/sandbox/bootstrap.ts:99-150 fetches the latest installer from GitHub without a timeout, user-agent, checksum/signature verification, or cleanup of the downloaded pkg; exposes supply-chain risk and potential long-running downloads.
- src/sandbox/detect.ts:51-55 leaves Docker/Seatbelt detection as TODOs, so non-macOS systems always fall back to the noop backend even if container runtimes are present; sandboxed verification won’t run off macOS.
- src/hud/status-stream.ts:12-73 starts a WebSocket server when a port is provided even without a token, effectively exposing HUD events unauthenticated on localhost/LAN. Should default to disabled unless a token is set and refuse to start otherwise.

## Notes
- Tests were not executed (per explicit instruction).
