# 1941 Work Log (oa-bc898e)
- Added integration test in `src/agent/orchestrator/sandbox-runner.test.ts` to ensure `runCommand` streams stdout/stderr into HUD emitters with consistent executionId and sandbox flags.
- Test uses host execution (sandbox disabled) to avoid container dependency while verifying streaming callbacks.
