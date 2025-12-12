# TestGen v2 + overfull-hbox - TB2 FAIL

**Date:** 2024-12-11 18:55
**Status:** FAIL
**Model:** `claude-haiku-4-5-20251001`

## Summary

overfull-hbox task failed - agent made progress but couldn't eliminate all LaTeX overfull hbox warnings.

| Metric | Value |
|--------|-------|
| Turns | 70 |
| Duration | 243.4s |
| Cost | $0.45 |
| TB2 Result | **FAIL (3/4 tests)** |
| Remaining Warnings | 2 (down from 7) |

## Test Results

| Test | Result |
|------|--------|
| test_main_synonyms_not_modified | PASS |
| test_compilation_successful | PASS |
| test_input_file_matches | PASS |
| test_no_overfull_hboxes | **FAIL** |

## Root Cause Analysis

**Problem:** Task requires iterative testing with `pdflatex` compiler to verify fixes.

The agent:
1. ✅ Read synonyms.txt and identified word substitutions
2. ✅ Made valid word replacements (passed test_input_file_matches)
3. ✅ Did not modify main.tex or synonyms.txt
4. ❌ Could not verify fixes work (no pdflatex locally)

**Remaining Overfull Warnings:**
```
Overfull \hbox (2.3994pt too wide) in paragraph at lines 5--6
Overfull \hbox (4.07532pt too wide) in paragraph at lines 7--8
Overfull \hbox (7.10309pt too wide) in paragraph at lines 7--8
```

## Why This Happened

This is a **Category B task** (requires iterative tool feedback):

1. tbench runs Claude locally, not inside Docker
2. Agent has no access to pdflatex to test iterations
3. Can only make educated guesses about which word swaps fix warnings
4. Without feedback loop, agent can't converge to solution

## Fix Required

This task needs **Docker-in-Docker** or **in-container agent execution**:

Option A: Run tbench inside the Docker container (has pdflatex)
Option B: Mount Docker socket so agent can run pdflatex via docker exec
Option C: Pre-build workspace with tools installed locally

## Infrastructure Bug Fixed

During this run, discovered and fixed `scripts/tb2-run.sh`:
- Was not copying environment files to workspace
- Added code to copy from `environment/tests/` before running tbench

## Categorization

| Category | Reason |
|----------|--------|
| **B: Enhanced TestGen** | Requires iterative tool execution during development |

This task is NOT Category A (self-contained) because success requires running pdflatex repeatedly to verify fixes work.

## Files

| File | Location |
|------|----------|
| ATIF Trajectory | `results/trajectories/overfull-hbox/20251211-185204-1dc33d05/trajectory.json` |
| Events Log | `results/trajectories/overfull-hbox/20251211-185204-1dc33d05/events.jsonl` |
| Metrics | `results/trajectories/overfull-hbox/20251211-185204-1dc33d05/metrics.json` |
| Workspace | `/tmp/tmp.Se7lgb0wqv/app` |

## Lessons Learned

1. **Easy ≠ Self-contained**: Task difficulty doesn't predict infrastructure requirements
2. **Tool feedback essential**: Some tasks require iterative tool execution during development
3. **Environment file copy needed**: tb2-run.sh now copies environment files to workspace
4. **Category A validation needed**: Not all "easy" tasks work with current tbench architecture

## Recommendation

Move overfull-hbox from Category A to **Category B** in strategic assessment. Requires either:
1. Enhanced tb2-run.sh that runs agent inside Docker container
2. Or skip until we implement in-container execution
