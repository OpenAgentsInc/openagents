## Problem

The agent has no stated policy distinguishing verification scopes, so green-on-target escalates straight to a full sweep and repeats it mid-loop. Trajectory `2026-08-27T12:29:02`:

- Step 47: targeted run of the one changed test file — 0.2s, green.
- Step 48: immediately ran the entire monorepo suite (152s) in the same minute.
- Step 49: ran the entire suite *again* (~150s) just to read failure names step 48 had truncated away.

Mid-loop recovery work kept paying full-suite price. Nothing in the harness or docs tells the agent that "read the failures from the previous result" is never a reason to re-execute a suite.

## Recommendation

Add an explicit test-execution escalation ladder to the coder system prompt / skills:

1. Changed files only.
2. Owning package / project.
3. Full suite exactly once, at the completion gate (pre-handoff), never mid-loop for information recovery.

Plus two hard rules alongside the ladder:

- Failure-name recovery must come from persisted logs or structured reports, never from a rerun. ("Never re-execute a command to view different columns of output you already received.")
- If the previous output was truncated past usefulness, that is a harness bug to route around via saved reports — not a reason to pay for the computation twice.

## Acceptance criteria

- [ ] Ladder + rerun rule present in coder system prompt or equivalent skill doc.
- [ ] Trace review on future sessions shows full-suite executions at most once per turn after the change; zero mid-loop reruns triggered by truncation.
