# TestGen v2 + prove-plus-comm - TB2 FAIL

**Date:** 2024-12-11 19:07
**Status:** FAIL
**Model:** `claude-haiku-4-5-20251001`

## Summary

prove-plus-comm task failed - agent completed the proof correctly but saved to wrong filename.

| Metric | Value |
|--------|-------|
| Turns | 20 |
| Duration | 71.8s |
| Cost | $0.10 |
| TB2 Result | **FAIL (0/4 tests)** |

## Test Results

| Test | Result |
|------|--------|
| test_proof_file_exists | **FAIL** |
| test_compiled_proof_exists | **FAIL** |
| test_proof_contents | **FAIL** |
| test_compiled_proof_content | **FAIL** |

## Root Cause Analysis

**Problem:** Agent misinterpreted file naming requirement.

The instruction says:
> Fix the incomplete proof of addition commutativity in the file **plus_comm.v**

The environment provides:
- `partial_proof.v` (the template to complete)

The agent should have:
1. Read `partial_proof.v` (template)
2. Written completed proof to `plus_comm.v` (as instructed)
3. Compiled `plus_comm.v` to get `plus_comm.vo`

Instead, the agent:
1. Read `partial_proof.v`
2. Edited `partial_proof.v` in place
3. Compiled `partial_proof.v` → got `partial_proof.vo`

## What The Agent Actually Produced

The completed proof in `partial_proof.v` (correct Coq code):

```coq
Require Import Arith.

Theorem plus_comm : forall n m : nat,
  n + m = m + n.
Proof.
  intros n m.
  induction n as [|n' IHn'].
  - simpl.
    (* Base case: 0 + m = m + 0 *)
    symmetry.
    apply Nat.add_0_r.
  - simpl.
    (* Inductive case: S(n') + m = m + S(n') *)
    (* We have IHn' : n' + m = m + n' *)
    rewrite IHn'.
    (* Now we need: S(m + n') = m + S(n') *)
    symmetry.
    apply Nat.add_succ_r.
Qed.
```

The proof is **mathematically correct** and compiles successfully (partial_proof.vo exists).

## Workspace Contents After Agent

```
.partial_proof.aux
partial_proof.glob
partial_proof.v     (completed proof - correct!)
partial_proof.vo    (compiled - exists!)
partial_proof.vok
partial_proof.vos
```

Tests expected: `plus_comm.v` and `plus_comm.vo`

## Category Analysis

This is NOT an infrastructure issue. The environment file was copied correctly.

This is an **agent comprehension issue**:
- Instruction says "fix **plus_comm.v**" but file doesn't exist
- Agent found `partial_proof.v` and assumed it was the same
- Should have created `plus_comm.v` from the template

## Possible Fixes

1. **TB2 Task Bug?** - Instruction/environment filename mismatch could be intentional test of following instructions precisely

2. **Agent Guidance** - Could add to instruction: "Save your completed proof as plus_comm.v"

3. **More Explicit Instructions** - Augment Docker Environment section with: "Note: The template may be named differently than the expected output file"

## Lesson Learned

**Task success requires precise output naming, not just correct implementation.**

The agent did the hard part (completing a formal Coq proof) but failed on the easy part (file naming). This highlights the importance of reading instructions literally.

## Comparison with overfull-hbox

| Task | Hard Part | Easy Part That Failed |
|------|-----------|----------------------|
| overfull-hbox | Iterative tool feedback | N/A (infrastructure fix needed) |
| prove-plus-comm | None (proof was correct) | File naming |

## Files

| File | Location |
|------|----------|
| ATIF Trajectory | `results/trajectories/prove-plus-comm/20251211-190716-88cd80bf/trajectory.json` |
| Metrics | `results/trajectories/prove-plus-comm/20251211-190716-88cd80bf/metrics.json` |
| Workspace | `/tmp/tmp.NC8sBImb9q/app` |

## Infrastructure Status

- Environment files copied: YES (partial_proof.v present)
- Docker container started: YES (tb2-prove-plus-comm-20251211)
- Agent had tool access: YES (compiled the proof successfully)

No infrastructure changes needed. This is a comprehension failure, not an execution failure.
