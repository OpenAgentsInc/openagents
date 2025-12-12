# TB2 Docker Support - Full Implementation

**Date:** 2025-12-08
**Time:** 21:16 CT
**Goal:** Implement proper Docker support for Terminal-Bench 2 verification

---

## Summary

Implemented comprehensive Docker support for TB2 task verification, removing all benchmark gaming code and using proper task-specific Docker images.

---

## Files Created

### 1. `src/bench/tb2-config.ts`
- Parses `task.toml` files from TB2 task directories
- Extracts environment configuration (docker_image, cpus, memory, storage)
- Uses `smol-toml` parser (already in package.json)
- Provides `TB2EnvironmentConfig` type and `loadTaskConfig()` function
- Helper function `parseMemoryLimit()` for converting memory strings to MB

### 2. `src/bench/tb2-image-manager.ts`
- Manages Docker image availability for TB2 tasks
- Strategy: prebuilt image → build from Dockerfile → fallback to python:3.11-slim
- Functions:
  - `ensureTaskImage()`: Main entry point, returns image name to use
  - `imageExists()`: Check if image exists locally
  - `pullImage()`: Pull prebuilt image from Docker Hub
  - `buildImage()`: Build from Dockerfile with resource limits
- Handles timeouts and errors gracefully

### 3. `src/bench/tb2-container.ts`
- Maps TB2 configuration to sandbox `ContainerConfig` format
- `createTB2ContainerConfig()`: Factory function for container config
- Sets up `/app/` as working directory (TB2 standard)
- Configures resource limits from task.toml
- Sets `PYTHONUNBUFFERED=1` for pytest output

---

## Files Modified

### 1. `src/bench/tb2-docker-runner.ts`
**Changes:**
- Added `taskId` parameter to `TB2DockerRunnerOptions`
- Load task.toml configuration using `loadTaskConfig()`
- Use `ensureTaskImage()` to get proper Docker image (not hardcoded python:3.11-slim)
- Add resource limits (--memory, --cpus) from task configuration
- Install Python and pytest in container (task images are bare Ubuntu):
  ```bash
  command -v python3 >/dev/null 2>&1 || (apt-get update -qq && apt-get install -y -qq python3 python3-pip) &&
  python3 -m pip install -q --break-system-packages pytest &&
  python3 -m pytest tests/ -v 2>&1
  ```
- Added debug logging for Docker exit code and output
- **Key Fix:** Use `--break-system-packages` flag for pip (Python 3.12 externally-managed environment)

### 2. `src/hillclimber/evaluator.ts`
**Changes:**
- Updated `runVerificationWithDocker()` to pass `taskId` to `runTB2InDocker()`
- Line 129: `taskId: task.id`

### 3. `src/hillclimber/e2e-regex-log.test.ts`
**Changes:**
- **REMOVED gaming code** (lines 71-88): No longer replacing `/app/` paths in test files
- Updated `beforeAll()` to only copy environment files (not tests)
- Tests stay in source_path - Docker runner copies them
- Added comment: "DO NOT copy or modify test files - that would be gaming the benchmark"

---

## Architecture

```
Task Execution Flow:
1. MAP orchestrator detects task has source_path with tests
2. Calls evaluateProgressWithDocker(task, workspace)
3. Evaluator calls runTB2InDocker({taskId, taskDir, workspace})
4. TB2 Docker Runner:
   a. Load task.toml → get environment config
   b. ensureTaskImage() → pull/build proper Docker image
   c. Create temp directory
   d. Copy workspace + tests to temp directory
   e. Run Docker container:
      - Mount temp dir to /app/
      - Install Python + pytest
      - Run pytest tests/ -v
   f. Parse pytest output → return results
5. Returns blind verification (pass/fail, test counts, no expected values)
```

---

## Key Insights

### Problem 1: TB2 Images Don't Have Python
- TB2 Dockerfiles are bare Ubuntu (`FROM ubuntu:24.04`)
- Prebuilt images (e.g., `alexgshaw/regex-log:20251031`) also lack Python
- **Solution:** Install Python and pytest at runtime in container

### Problem 2: Python 3.12 Externally-Managed Environment
- Ubuntu 24.04 uses Python 3.12 with PEP 668 restrictions
- `pip install` fails without `--break-system-packages`
- **Solution:** Add `--break-system-packages` flag (safe in disposable containers)

### Problem 3: Gaming the Benchmark
- Previous code replaced `/app/` paths in test files for local execution
- This is benchmark gaming - modifying expected test behavior
- **Solution:** Run tests in Docker with proper `/app/` mount, don't modify tests

---

## Verification

Manual testing showed pytest running correctly in Docker:
```bash
docker run --rm -v "/tmp/test-pytest:/app" -w /app \
  alexgshaw/regex-log:20251031 sh -c \
  "command -v python3 >/dev/null 2>&1 || \
   (apt-get update -qq && apt-get install -y -qq python3 python3-pip) && \
   python3 -m pip install -q --break-system-packages pytest && \
   python3 -m pytest tests/ -v 2>&1"
```

Output showed:
- Python installed successfully
- Pytest running
- Test results: "1 failed in 0.03s" (with bad regex, expected)
- parsePytestSummary can parse this output

---

## Integration Test Status

Running full e2e test now to verify:
1. Docker pulls proper task-specific image
2. Pytest runs and produces output
3. Test counts are correct (expecting 9 tests for regex-log)
4. FM receives verification feedback

---

## Success Criteria

✅ Created TOML config loader
✅ Created image manager
✅ Created container config factory
✅ Refactored Docker runner to use proper images
✅ Updated evaluator to pass taskId
✅ Removed gaming code from e2e test
✅ Fixed Python/pytest installation in containers
🔄 Integration test running (in progress)

**Next:** Verify integration test passes and shows correct test counts.

---

## Harbor Alignment

This implementation aligns with Harbor's execution model:
- ✅ Uses task-specific Docker images (from docker_image in task.toml)
- ✅ Mounts workspace to `/app/` (TB2 standard)
- ✅ Runs pytest in container
- ✅ Supports resource limits (CPU, memory from task.toml)
- ✅ Blind verification (no expected values leaked)
- ✅ Can swap backends (Docker → Daytona/E2B) by changing image manager

**Future Enhancement:** Use `src/sandbox/` ContainerBackend infrastructure for unified execution across all container providers.
