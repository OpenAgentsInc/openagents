# TestGen v2 + overfull-hbox - TB2 PASS

**Date:** 2024-12-11 19:05
**Status:** PASS
**Model:** `claude-haiku-4-5-20251001`

## Summary

overfull-hbox passed after adding Docker access during agent execution.

| Metric | Value |
|--------|-------|
| Turns | 81 |
| Duration | 358.1s |
| Cost | $0.64 |
| TB2 Result | **PASS (4/4 tests)** |

## Test Results

| Test | Result |
|------|--------|
| test_main_synonyms_not_modified | PASS |
| test_compilation_successful | PASS |
| test_no_overfull_hboxes | **PASS** |
| test_input_file_matches | PASS |

## What Fixed It

**tb2-run.sh now starts a Docker container BEFORE tbench:**

```bash
# Start Docker container for agent to use during development
docker run --rm -d \
    --name "${CONTAINER_NAME}" \
    -v "${WORKSPACE}/app:/app" \
    -w /app \
    "${DOCKER_IMAGE}" \
    tail -f /dev/null
```

The instruction is augmented with:
```
## Docker Environment

A Docker container is running with the task environment. Use it to run tools:

docker exec tb2-overfull-hbox-20251211 <command>
```

This allowed the agent to:
1. Run `docker exec <container> pdflatex` to compile LaTeX
2. See overfull hbox warnings
3. Iteratively fix them by replacing words with shorter synonyms
4. Verify each fix worked before moving on

## Infrastructure Fixes in This Session

1. **Copy environment files** - tb2-run.sh now copies from `environment/tests/` to workspace
2. **Docker access during run** - Container started before tbench, shared workspace
3. **Instruction augmentation** - Added Docker exec examples to help agent

## Comparison

| Run | Docker During Run | Result | Cost |
|-----|-------------------|--------|------|
| 1st | No | FAIL (3/4) | $0.45 |
| 2nd | Yes | PASS (4/4) | $0.64 |

## Category Confirmation

overfull-hbox IS Category A (self-contained) - but requires iterative tool feedback.

The task is self-contained because:
- All rules are in the instruction (only use synonyms from synonyms.txt)
- No external data to parse
- Deterministic validation

But it requires **tool iteration** during development, not just final verification.

## Files

| File | Location |
|------|----------|
| ATIF Trajectory | `results/trajectories/overfull-hbox/20251211-185943-155bef1e/trajectory.json` |
| Workspace | `/tmp/tmp.Hkd0C7mh0S/app` |

## Lesson Learned

**Category A doesn't mean "no tools required"** - it means rules are self-contained.

Tasks may still need iterative tool access during development:
- LaTeX → pdflatex
- Python → pytest
- Rust → cargo build

The key fix is giving the agent access to the task's Docker container during execution, not just verification.
