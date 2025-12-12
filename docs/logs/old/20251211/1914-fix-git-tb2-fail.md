# TestGen v2 + fix-git - TB2 FAIL

**Date:** 2024-12-11 19:14
**Status:** FAIL
**Model:** `claude-haiku-4-5-20251001`

## Summary

fix-git task required two runs - first exposed infrastructure bug, second exposed agent bug.

| Run | Tests | Issue |
|-----|-------|-------|
| 1 | 0/2 | Infrastructure: directories not copied |
| 2 | 1/2 | Agent: merge conflicts not resolved |

## Run 1: Infrastructure Bug

| Metric | Value |
|--------|-------|
| Turns | 14 |
| Duration | 62.5s |
| Cost | $0.08 |
| TB2 Result | **FAIL (0/2 tests)** |

**Root Cause:** tb2-run.sh only copied files from environment/, not directories.

The task has `environment/resources/patch_files/` directory that tests need:
```
environment/
  Dockerfile
  setup.sh
  resources/
    patch_files/
      about.md
      default.html
```

**Infrastructure Fix Applied:**

```bash
# OLD: Only copied files (-f flag)
for f in "${ENV_DIR}"/*; do
    if [[ -f "$f" && ... ]]; then
        cp "$f" "${WORKSPACE}/app/"
    fi
done

# NEW: Copy both files and directories
for f in "${ENV_DIR}"/*; do
    basename_f="$(basename "$f")"
    if [[ "$basename_f" == "Dockerfile" || ... ]]; then
        continue
    fi
    cp -r "$f" "${WORKSPACE}/app/"  # -r for recursive
done
```

## Run 2: Agent Bug

| Metric | Value |
|--------|-------|
| Turns | 23 |
| Duration | 66.4s |
| Cost | $0.09 |
| TB2 Result | **FAIL (1/2 tests)** |

| Test | Result |
|------|--------|
| test_layout_file | **PASS** |
| test_about_file | **FAIL** |

**Root Cause:** Agent started merge but didn't resolve conflicts.

The agent:
1. Used `git reflog` to find lost commit
2. Created branch `lost-changes` from the detached HEAD commit
3. Attempted `git merge lost-changes`
4. **Left merge conflict markers in the file**

Contents of about.md after merge:
```markdown
<<<<<<< HEAD
I am a sixth PhD candidate at the [Paul G. Allen School...
=======
I am a Postdoctoral Researcher at Stanford CS.
>>>>>>> lost-changes
```

Expected (clean):
```markdown
I am a Postdoctoral Researcher at Stanford CS.
```

## Task Analysis

fix-git is a git forensics task that simulates:
1. User makes changes on detached HEAD
2. User commits those changes
3. User checkouts master - losing the commit
4. User needs to recover and merge the lost changes

The agent correctly did the hard part (git forensics) but failed the easy part (merge resolution).

## Pattern Recognition

| Task | Hard Part Done | Failed On |
|------|---------------|-----------|
| overfull-hbox | Find word substitutions | Needed tool feedback |
| prove-plus-comm | Complete Coq proof | File naming |
| fix-git | Git reflog recovery | Merge conflict resolution |

**Emerging Pattern:** Agent does the complex reasoning correctly but fails on mechanical completion steps.

## Infrastructure Fix Confirmed

The tb2-run.sh fix for copying directories is correct and necessary. Second run confirmed resources/ was copied properly (FileNotFoundError → hash mismatch).

## Files

| File | Location |
|------|----------|
| Run 1 Trajectory | `results/trajectories/fix-git/20251211-191218-2f0ba2c0/` |
| Run 2 Trajectory | `results/trajectories/fix-git/20251211-191425-7bb06fb5/` |
| Workspace | `/tmp/tmp.1n83XQfdXf/app` |

## Recommendation

This task could benefit from:
1. Clearer instruction that merge conflicts must be resolved
2. Or: Agent should verify file contents match expected after merge

The infrastructure is now correct. This is purely an agent comprehension issue.
