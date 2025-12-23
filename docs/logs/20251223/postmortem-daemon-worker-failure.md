# Postmortem: Autopilot Daemon Worker Restart Failure

**Date:** 2025-12-23
**Duration:** ~83 minutes without commits
**Severity:** High - Complete autopilot stoppage

## Summary

The autopilot daemon failed to restart worker processes after a worker crashed due to compile errors in the codebase. The root cause was the daemon using `cargo run` to spawn workers, which requires compilation. When the worker introduced breaking changes before crashing, subsequent restart attempts failed because the build was broken.

## Timeline

- **~01:30 CST**: Worker completes issue #801, pushes commit
- **~01:40 CST**: Worker begins working on issue #821 (APM CLI integration)
- **~02:47 CST**: Worker crashes mid-task after introducing duplicate `BaselineCommands` enum
- **02:47-09:00 CST**: Daemon attempts to restart worker repeatedly, all fail due to 31 compile errors
- **10:34 CST**: Daemon hits max consecutive restarts (10), enters "Failed" state with 10-minute cooldown
- **10:41 CST**: Manual intervention - fixed compile errors, deployed known-good binary solution

## Root Cause

**Primary:** The autopilotd daemon used `cargo run` to spawn workers. When a worker introduced breaking changes to the code and crashed before fixing them, subsequent restart attempts failed because:

1. `cargo run` compiles from source before running
2. The source was broken (31 compile errors)
3. Build fails -> worker never starts -> daemon tries again with backoff
4. Eventually hits max_consecutive_restarts limit

**Contributing factors:**
1. Worker introduced duplicate enum definition (`BaselineCommands` defined at lines 1151 and 1291)
2. Worker added handler code referencing non-existent enum variants
3. Worker crashed/died before fixing the errors it introduced
4. No mechanism to "roll back" to a working binary

## What Went Wrong

1. **Daemon architecture flaw**: Using `cargo run` meant every worker restart required successful compilation of the current working tree state

2. **No isolation**: Worker process modifications affected its own restart capability

3. **Silent failure mode**: The daemon was correctly restarting (with backoff), but from the user's perspective nothing was happening because all restarts failed

4. **Poor observability**: User had to manually investigate to discover build failures were blocking restarts

## The Fix

### Immediate Fix (applied)
1. Fixed the 31 compile errors in `crates/autopilot/src/main.rs`:
   - Removed duplicate `BaselineCommands` enum (lines 1286-1331)
   - Removed duplicate `MetricsCommands::Baseline` variant
   - Removed handler code for removed feature
   - Added missing `Ok(())` return statements

2. Built and deployed known-good binary to `~/.autopilot/bin/autopilot`

### Permanent Fix (applied)
Changed daemon default behavior in `crates/autopilot/src/daemon/config.rs`:

```rust
impl Default for WorkerCommand {
    fn default() -> Self {
        // Check for AUTOPILOT_WORKER_BINARY env var first
        if let Ok(path) = std::env::var("AUTOPILOT_WORKER_BINARY") {
            return WorkerCommand::Binary { path: PathBuf::from(path) };
        }

        // Default to known-good binary location
        let binary_path = PathBuf::from(&home).join(".autopilot").join("bin").join("autopilot");

        // If the known-good binary exists, use it
        if binary_path.exists() {
            WorkerCommand::Binary { path: binary_path }
        } else {
            // Fall back to cargo run (for first-time setup only)
            WorkerCommand::Cargo { manifest_path: None }
        }
    }
}
```

**Key changes:**
1. Daemon now uses pre-built binary at `~/.autopilot/bin/autopilot` by default
2. Binary is separate from working tree - worker code changes don't affect restart capability
3. `AUTOPILOT_WORKER_BINARY` env var allows override for testing
4. Falls back to `cargo run` only for first-time setup

## Remediations

### Done
- [x] Fixed compile errors in autopilot crate
- [x] Deployed known-good binary to `~/.autopilot/bin/autopilot`
- [x] Changed daemon default to use pre-built binary
- [x] Added `AUTOPILOT_WORKER_BINARY` env var for configuration

### Recommended Future Work
1. **Add pre-commit hook**: Prevent pushing code that doesn't compile
   ```bash
   # .git/hooks/pre-push
   cargo build -p autopilot || exit 1
   ```

2. **Add binary deployment step**: After successful builds, update known-good binary
   ```bash
   # In CI or as part of workflow
   cargo build -p autopilot && cp target/debug/autopilot ~/.autopilot/bin/autopilot
   ```

3. **Add daemon alerting**: Emit metrics/logs when worker restarts fail due to build errors

4. **Consider containerization**: Run worker in container with fixed binary image

5. **Add build verification to autopilot**: Before committing, verify the build still works

## Lessons Learned

1. **Never couple runtime restart to build process**: A daemon that uses `cargo run` is fundamentally fragile - any code change can prevent restarts

2. **Pre-built binaries are essential for reliability**: The "known-good binary" pattern provides isolation between code changes and operational stability

3. **Workers should never break their own restart path**: The failure mode where a worker can modify code and then crash, leaving itself unable to restart, is dangerous

4. **Observability matters**: The failure was silent for 83 minutes because there was no clear signal that build failures were blocking restarts

## Verification

After the fix:
```
$ pgrep -af autopilot
757513 ./target/debug/autopilotd --workdir ... --project openagents
757571 /home/christopherdavid/.autopilot/bin/autopilot run --full-auto --with-issues ...
```

Worker is now running from pre-built binary, not `cargo run`. Future code breakage will not prevent daemon from restarting workers.
