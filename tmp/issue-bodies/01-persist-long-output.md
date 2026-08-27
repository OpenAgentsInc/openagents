## Problem

Long-running commands lose their output unless the model predicted every future question in advance. Evidence from trajectory `2026-08-27T12:29:02` (openagents coder session on this repo):

- Step 48 ran the full monorepo suite (`npx vp test --run`, 2301 files / 19,769 tests, **152s**) piped through `tail -8`. The tail kept the summary line ("8 failed | 2289 passed") and destroyed the failure names.
- Step 49 then re-executed the identical 150s suite run with a different grep purely to recover those 8 failure names.
- Step 55 ran `pnpm run test:rust` three times inside one invocation for three different greps (~4m49s); two of the three executions were redundant, and the raw output was still never persisted anywhere addressable.

Root cause: output has no lifetime beyond the tool response. There is no standing place for "the output of the command I ran 90 seconds ago," so any new question about old output forces re-execution.

## Recommendation

Wrap long commands in the shell/harness tool layer so output becomes an addressable artifact:

- Any command exceeding a wall-clock threshold (e.g. 30s) or declaring `timeout_seconds >= 120` is executed as roughly:
  ```
  cmd > "$SESSION_DIR/cmd-N.log" 2>&1; echo "exit=$?"; tail -c 4000 "$SESSION_DIR/cmd-N.log"
  ```
- What the model sees stays bounded; what it might want lives on disk at a path named in the response.
- Follow-up questions (`grep FAIL cmd-48.log`) cost milliseconds and zero re-execution.

## Acceptance criteria

- [ ] Commands crossing the threshold write their full stdout/stderr to `$SESSION_DIR/cmd-N.log` and the tool response includes that path plus a bounded excerpt and exit code.
- [ ] An agent that greps a persisted log instead of rerunning the command completes the same information-gathering steps with no second execution (verify against a replayed trajectory like `2026-08-27T12:29:02`: steps 49 and two-thirds of step 55 become file reads).
