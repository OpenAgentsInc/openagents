# 1244 New Tasks Log

## New Tasks Created

Created 3 real (non-infra) tasks for the openagents repo:

| ID | Title | Priority | Labels |
|---|---|---|---|
| oa-c9003e | Add OPENAGENTS-TASKS-SPEC link to mechacoder README | P2 | docs, mechacoder |
| oa-c9db6e | Add example do-one-task usage to MECHACODER-OPS | P2 | docs, mechacoder |
| oa-73016a | Add minimal MechaCoder desktop demo task for Golden Loop v2 | P1 | golden-loop, demo |

## Purpose

These are the first "real" tasks in the openagents repo task system. They will be used to:

1. **oa-73016a (P1)**: Prove Golden Loop v2 works by having MechaCoder pick up and complete this task
2. **oa-c9003e, oa-c9db6e (P2)**: Documentation improvements that can be worked on after the Golden Loop demo

## Next Steps

1. Run `bun src/agent/do-one-bead.ts --dir .` to have MechaCoder pick up the P1 task
2. Verify the task is completed, committed, and marked closed in tasks.jsonl
3. Log the results in a Golden Loop v2 verification log
