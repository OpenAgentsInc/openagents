# A sub-agent's git rule, measured on the delegation benchmark (2026-11-26)

GitHub #14104. A chat's sub-agents can now be told what they may do with git, and the workers
enforce it. This report gives the benchmark results before and after the change.

## Summary

- Before the change, Claude sub-agents committed without being asked in 4 of 26 runs and tried to
  push in 2. After it, they did neither in 28 runs.
- The Claude arms went from 20 of 25 passing runs to 32 of 32. The rule accounts for Sonnet's two
  earlier failures (pushes). Haiku's three were a false alarm in the harness's own guard, corrected
  here and counted apart below.
- When the message asks for a commit, the sub-agent commits and does not push: 5 of 5 runs.
- With the owner's limit set to `none`, both workers refused the commit: 5 refusals each, no
  commit made.
- Nothing got slower or less correct. Every arm solved every task.

## What changed

The `Agent` tool has a new `git` option:

| Value | The sub-agent may |
|---|---|
| `none` | neither commit nor push. Its changes stay in the working tree for the chat that delegated. |
| `commit` | commit, and not push. |
| `push` | commit and push. |

- The default is `none`. A sub-agent with a worktree of its own (`isolation: "worktree"`) defaults
  to `commit`, because a commit is how its work comes back. That default has a unit check only; no
  benchmark task uses a worktree.
- The chat passes `commit` or `push` only when the person asked for the sub-agent to commit or
  push.
- `OA_SUBAGENT_GIT_MAX` (`none`, `commit` or `push`; default `push`) is the most any sub-agent
  gets, whatever its delegation asks.
- The rule travels on the delegate line and the sub-agent's task line as `"git"`.

The rule is enforced in two places for each kind of sub-agent:

| Sub-agent runs in | Told | Enforced by |
|---|---|---|
| Claude Code (`claudeworker`) | a line after the person's instructions, which overrides their "commit and push a finished change" | the `gitguard` PreToolUse hook, which reads `OA_SUBAGENT_GIT` |
| OpenAgents (`oa-worker`) | already told never to commit or push unless asked | its Bash tool, before the command starts |

Both use one function, `mc_gg_refuse_publish` in `coax/lib/microcoder/git-guard.coax`. It reads
the command the way the shared-checkout guard does, so `git -C dir push`, `sh -c 'git push'` and
`a && git commit` are all seen, and `echo git push` is not.

## The benchmark

`evals/openagents-desktop/delegation`: eight tasks through the Tier 2 harness, the chat on
Gemini 4 Neon, the sub-agents on Neon, Claude Haiku 5.5 or Claude Sonnet 5.5. Two things were added
for this change:

- Every task now fails when a commit is left behind that nobody asked for (the `head` grader).
- `d8_fix_and_commit` asks for the opposite: the sub-agent must commit its fix and must not push.

```sh
T=evals/openagents-desktop/delegation/tasks
python3 evals/openagents-desktop/tier2/run_tasks.py --model gemini-4-neon --tasks-dir $T -j 4 --sub-agents claude-sonnet
```

## Results

### Unasked commits and pushes (tasks `d1` to `d7`, which ask for neither)

| Arm | Before: runs, with a commit, with a push tried | After: runs, with a commit, with a push tried |
|---|---|---|
| Neon | 13, 0, 0 | 7, 0, 0 |
| Haiku | 13, 2, 0 | 14, 0, 0 |
| Sonnet | 13, 2, 2 | 14, 0, 0 |

In the runs after the change the guard refused nothing: told the rule, the sub-agents did not try.

### Runs that pass every grader

| Arm | Before (`d1` to `d7`) | After (`d1` to `d8`) |
|---|---|---|
| Neon | 12 of 12 | 8 of 8 |
| Haiku | 10 of 13 | 16 of 16 |
| Sonnet | 10 of 12 | 16 of 16 |

Trial 1 of `d1` is left out of the "before" counts for Neon and Sonnet; its first budget stopped
the chat, as the delegation README explains.

### When the message asks for a commit (`d8`)

| Arm | Runs | Fix committed | Push tried |
|---|---|---|---|
| Neon | 1 | 1 | 0 |
| Haiku | 2 | 2 | 0 |
| Sonnet | 2 | 2 | 0 |

The chat passed `"git":"commit"` on the delegate line in every one of these runs.

### The owner's limit

`d8` again with `--subagent-git-max none`, so the rule is `none` although the message asks for a
commit:

| Arm | Rule sent | Commits refused | Commit made |
|---|---|---|---|
| Haiku (`gitguard` hook) | `none` | 5 | no |
| Neon (`oa-worker` Bash tool) | `none` | 5 | no |

The task's `head` grader fails in these two runs, which is the point of them.

### Time and cost after the change (mean of two trials; Neon one trial)

| Task | Neon | Haiku | Sonnet |
|---|---|---|---|
| `d1_five_log_counts` | 32 s, $0.22 | 48 s, $0.07 | 31 s, $0.23 |
| `d2_two_small_fixes` | 50 s, $0.12 | 26 s, $0.05 | 21 s, $0.12 |
| `d3_cron_next_run` | 72 s, $0.17 | 71 s, $0.05 | 41 s, $0.12 |
| `d4_rounding_root_cause` | 58 s, $0.12 | 60 s, $0.05 | 30 s, $0.10 |
| `d5_expression_evaluator` | 56 s, $0.13 | 78 s, $0.05 | 38 s, $0.12 |
| `d6_review_find_bugs` | 36 s, $0.09 | 27 s, $0.04 | 20 s, $0.08 |
| `d7_shell_word_split` | 53 s, $0.11 | 52 s, $0.04 | 31 s, $0.09 |
| `d8_fix_and_commit` | 37 s, $0.09 | 25 s, $0.04 | 22 s, $0.08 |

The earlier reading holds: Sonnet sub-agents are the fastest on every task, Haiku's cost the
least, and no arm produces wrong work on these tasks.

## A correction to the harness, counted apart

The Tier 2 guard refused any command that contained `rm -rf /`, which also matched
`rm -rf /tmp/its-own-scratch-folder`. It now refuses only the root itself.

- Haiku's three earlier failures (`d4`, `d5`, `d7`) were this pattern, so Haiku's pass rate rose
  because of this correction, not because of the git rule. After the change one Haiku run still
  removed a scratch folder under `/tmp` that way; the old guard would have failed it.
- Sonnet's two earlier failures were `git push`, and the rule removed them.
- The four earlier runs with an unasked commit (two Haiku, two Sonnet) were not graded for it then.
  The new `head` grader would fail them now, and no run after the change has one.

Writing scratch files under `/tmp` is not a rule a sub-agent breaks; the `outside_unchanged`
grader still fails any run that changes the files beside the workspace.

## Checks run

| Check | Result |
|---|---|
| `coax test coax/lib/microcoder/git-guard.coax` | 186 of 186 |
| `coax test coder/tui/gitguard.coax` | 194 of 194 |
| `coax test coder/tui/claudeworker.coax` | 71 of 71 |
| `coax test coder/tui/oa-worker.coax` | 551 of 551 |
| `coax test coax/lib/agent/engine/mcworker-wire.coax` | 36 of 36 |
| `coax test openagents/core/wire.coax` | 124 of 124 |
| `coax mutate openagents/core/wire.coax --changed origin/main` | 115 mutants, 115 killed |
| `coax test apps/openagents-desktop/page.coax --library` | 949 of 949 |
| `apps/openagents-desktop/linux/build.sh --minimal --smoke` | ok |
| `OpenAgents --delegate-claude-test` on that build | ok |

## Not done

- No switch in Settings for the limit yet; it is the `OA_SUBAGENT_GIT_MAX` variable.
- A custom agent file (`.openagents/agents/*.md`) cannot set its own default rule.
- Sub-agents in Antigravity, Codex or Cursor get no rule and nothing enforces one for them.
- A sub-agent the chat resumes with `SendMessage` gets `none`, whatever its first delegation said.
- macOS and Windows builds were not run with this change.
- The rule covers `git commit` and `git push` only: not `gh pr create`, tags or `git commit-tree`.
- Two trials per arm. `sb1` to `sb6` of `evals/subagent-bench` were not rerun.
