# TestGen v2 + largest-eigenval - TB2 FAIL (Partial Success)

**Date:** 2024-12-11 19:32
**Status:** FAIL (but close!)
**Model:** `claude-haiku-4-5-20251001`

## Summary

largest-eigenval required two runs - first exposed infrastructure bug, second showed partial success.

| Run | Tests | Issue |
|-----|-------|-------|
| 1 | ERROR | Infrastructure: src/ contents not copied directly to /app/ |
| 2 | 22/27 (81%) | Performance: Agent beat numpy on 4/9 matrix sizes |

## Infrastructure Fix

The `environment/src/` directory should have its CONTENTS copied directly to `/app/`, not the directory itself.

**Issue:**
- `cp -r environment/src "${WORKSPACE}/app/"` → creates `/app/src/eigen.py`

**Fix:**
- `cp -r environment/src/* "${WORKSPACE}/app/"` → creates `/app/eigen.py`

Tests do `os.chdir("/app")` then `import eigen`, so files must be at `/app/*.py`.

## Run 2 Results

| Metric | Value |
|--------|-------|
| Turns | 40 |
| Duration | 276.9s |
| Cost | $0.38 |
| TB2 Result | **FAIL (22/27 tests)** |

### Test Breakdown

| Test Category | Result |
|---------------|--------|
| test_eigen_pair[2-10] | 9/9 PASS |
| test_dominance_eigenvalue[2-10] | 9/9 PASS |
| test_speedup[2-10] | 4/9 PASS |

**Correctness: 100%** (all eigenvalues/eigenvectors are mathematically correct)

### Performance Results

| Size | Ref Time | Agent Time | Result |
|------|----------|------------|--------|
| 2x2 | 0.000009s | 0.000027s | FAIL (3x slower) |
| 3x3 | 0.000029s | 0.000010s | PASS (3x faster) |
| 4x4 | 0.000011s | 0.000011s | FAIL (same) |
| 5x5 | 0.000055s | 0.000017s | PASS (3x faster) |
| 6x6 | 0.000029s | 0.000039s | FAIL |
| 7x7 | 0.000044s | 0.000061s | FAIL |
| 8x8 | 0.000079s | 0.000047s | PASS (1.7x faster) |
| 9x9 | 0.000047s | 0.000076s | FAIL |
| 10x10 | 0.000027s | 0.000021s | PASS |

## Root Cause Analysis

The agent implemented a valid algorithm but:
1. Has overhead that dominates for small matrices
2. Numpy's LAPACK backend is highly optimized for certain sizes
3. Performance varies non-monotonically with size (cache effects?)

The agent did the hard part (correct implementation) but couldn't beat numpy's optimized code consistently.

## Task Classification

This is **Category C** (specialized tooling) - beating numpy's LAPACK requires:
1. Specialized algorithms (power iteration, QR, etc.)
2. Cache-aware implementations
3. Possibly compiled extensions (Cython, C)

## Pattern Update

| Task | Correct | Complete | Notes |
|------|---------|----------|-------|
| overfull-hbox | ✅ | ✅ | Infrastructure fix needed |
| prove-plus-comm | ✅ | ❌ | File naming issue |
| fix-git | ✅ | ❌ | Merge conflict unresolved |
| filter-js-from-html | Partial | ❌ | Specialized security lib needed |
| largest-eigenval | ✅ | Partial | 81% tests pass, 100% correct |

**Emerging Pattern:** Agent produces correct solutions but struggles with:
1. Mechanical completion steps (naming, merging)
2. Performance optimization against highly optimized baselines

## Files

| File | Location |
|------|----------|
| Run 1 Trajectory | `results/trajectories/largest-eigenval/20251211-192405-60013549/` |
| Run 2 Trajectory | `results/trajectories/largest-eigenval/20251211-193215-f5b6612c/` |
| Workspace | `/tmp/tmp.7vPSmWNKD5/app` |

## Recommendation

This task is borderline passable. With a longer timeout or better algorithm hints, it might pass fully. Consider:
1. Testing with Opus model (stronger reasoning)
2. Adding performance optimization skills
3. Accepting 81% as partial success

## Infrastructure Fix Details

Added to tb2-run.sh:
```bash
# Copy CONTENTS of environment/src/ directly into /app/ (src is the content root)
if [[ -d "${ENV_DIR}/src" ]]; then
    cp -r "${ENV_DIR}/src"/* "${WORKSPACE}/app/" 2>/dev/null || true
fi
```
