# 1427 Sandbox Integration Complete

## Summary

Completed full sandbox integration for MechaCoder parallel execution. The sandbox system is now properly wired from CLI through to the orchestrator, with the project configured to use `macos-container` backend by default.

## Changes Made

### Phase 1: Remove Dead Code
- Removed `useContainers` field from `ParallelExecutionConfig` schema (src/tasks/schema.ts)
- Removed `containerImage` field from `ParallelRunnerConfig` (src/agent/orchestrator/parallel-runner.ts)
- Updated overnight.ts, do-one-task.ts, and regression test to remove dead field usage

### Phase 2: Wire Sandbox Config Through Parallel-Runner
- Added `sandbox?: SandboxConfig` to `ParallelRunnerConfig` interface
- Added `sandbox?: SandboxConfig` to `CreateParallelRunnerOptions` interface
- Added sandbox passthrough in `runParallelFromConfig()`
- Added sandbox passthrough in `runAgentInWorktree()` to orchestratorConfig

### Phase 3: Fix overnight.ts Config Flow
- Changed default `sandbox.enabled` from `false` to `true`
- Added `sandbox: projectConfig.sandbox` to parallel options
- Added sandbox passthrough to sequential orchestrator config

### Phase 4: Add CLI Flags
- Added `--sandbox` flag to force sandbox enabled (override project config)
- Added `--no-sandbox` flag to force sandbox disabled (override project config)
- Updated both overnight.ts and overnight-parallel.ts
- CLI flags properly override the `enabled` field while preserving other settings

### Phase 5: Update Project Config
- Updated `.openagents/project.json`:
  - Set `sandbox.enabled: true`
  - Set `sandbox.backend: "macos-container"`

### Phase 6: Add Tests
- Created `sandbox-config-flow.test.ts` with two test cases:
  1. Verifies sandbox config flows from parallel-runner to orchestrator
  2. Verifies CLI override works correctly
- All tests passing

### Phase 7: Update Documentation
- Added `--sandbox` and `--no-sandbox` flags to CLAUDE.md parallel MechaCoder section

## Files Modified

1. `src/tasks/schema.ts` - Removed useContainers
2. `src/agent/orchestrator/parallel-runner.ts` - Added sandbox config passthrough
3. `src/agent/overnight.ts` - Added CLI flags, sandbox config wiring
4. `src/agent/overnight-parallel.ts` - Added CLI flags, sandbox config wiring
5. `src/agent/do-one-task.ts` - Removed dead code
6. `src/agent/orchestrator/parallel-runner.regression.test.ts` - Updated fixture
7. `.openagents/project.json` - Enabled sandbox with macos-container
8. `CLAUDE.md` - Documented new CLI flags

## Files Created

1. `src/agent/orchestrator/sandbox-config-flow.test.ts` - Config flow tests

## Test Results

```bash
bun test src/agent/orchestrator/sandbox-config-flow.test.ts
# 2 pass, 0 fail

bun test src/agent/orchestrator/parallel-runner.regression.test.ts
# 1 pass, 0 fail
```

## Config Flow Diagram (After Changes)

```
CLI (overnight.ts / overnight-parallel.ts)
    |
    +-- --sandbox / --no-sandbox (optional override)
    |
    v
parseArgs() -> sandboxEnabled?: boolean
    |
    v
parallelOvernightLoop()
    |
    +-- sandboxConfig = override or projectConfig.sandbox
    |
    v
runParallelFromConfig({ sandbox: sandboxConfig, ... })
    |
    v
parallel-runner.ts -> runAgentInWorktree()
    |
    v
orchestrator.ts -> orchestratorConfig.sandbox
    |
    v
sandbox-runner.ts -> runVerificationWithSandbox()
    |
    v
src/sandbox/ -> macos-container.ts
```

## Usage

```bash
# Use project config (now enabled by default)
bun run mechacoder:parallel --max-agents 4 --max-tasks 50 --cc-only

# Force sandbox on regardless of project config
bun run mechacoder:parallel --sandbox --cc-only

# Force sandbox off regardless of project config
bun run mechacoder:parallel --no-sandbox --cc-only
```

## Next Steps

The sandbox is now wired and enabled. On macOS systems with the container CLI available, test/typecheck verification will automatically run in isolated containers.

If containers aren't available on the system, the sandbox-runner will gracefully fall back to host execution with appropriate logging.
