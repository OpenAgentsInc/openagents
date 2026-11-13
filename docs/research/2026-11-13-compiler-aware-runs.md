# Microcoder with the compiler's answers and a working memory, on a real issue

Written 2026-11-13 for GitHub #13444 and #13445. This covers what Microcoder's
JSON-action loop did on one real issue with and without the two modes
(docs/coder/microcoder.md, "Compiler questions and working memory"), and
what changed because of it. The RLM coder's runs on the same issue are in
the #13444 comments.

## Setup

| | |
| --- | --- |
| Issue | #13435, "Command rows carry an effect kind and an approval rule": add an effect kind and an approval rule to every `REALM_COMMANDS` row, build CLI help and MCP descriptions from them, and add a check |
| Tree | `4fe15b46a0^`, the commit before the issue's fix, in a scratch worktree. Nothing from the runs was committed |
| Why this issue | It is real and wide (three files, 37 rows), the RLM coder failed on it three times (#13444), and its fix on main is a reference to grade against |
| Task text | The issue's title and body, then: "Do the issue in this checkout. The check is `coax/bin/coax test apps/openagents-cli/commands.coax`: it must pass when you finish. Do not commit." |
| Model | `gemini-4-carbon` on Vertex at low thinking, every call |
| Driver | `coder/tui/mcworker.coax`, one message a run, no approval gate |
| Graded by | The task's check, `coax test apps/openagents-cli/main.coax` (the CLI's whole unit, 1,593 checks before), and the issue's four requirements read against the diff |

## Runs

| Run | Build | Modes | Ending | Calls | Cost | Wall | Views (repeats) | Questions (unanswered) | Edits (refused stale) | Commands |
| --- | --- | --- | --- | ---: | ---: | ---: | --- | --- | --- | ---: |
| A | 6d88d1bd64 | none | stopped by me, no edit | 52 | $7.43 | 635 s | 111 (25) | none | 0 | 77 |
| B | 6d88d1bd64 | intel, memory | stopped by me, no edit | 45 | $7.34 | 618 s | 160 (43) | 8 (1) | 0 | 39 |
| C | B + the note at step 12 | intel, memory | finished, passes | 30 | $4.71 | 515 s | 77 (14) | 3 (1) | 27 (13) | 37 |
| D | e3a3d20afc | intel, memory | finished, passes | 18 | $2.76 | 341 s | 60 (9) | 3 (1) | 7 (0) | 22 |

Repeats are views of a path and range already viewed in the run. Edits count
each edit an action asked for. The memory's own count of repeat reads
(lines it already held, unchanged) was 28 of 65 in C and 24 of 47 in D.

Total spend: $22.24.

### What the traces showed

- **A and B never started editing.** Both read the three files over and
  over and ran the existing checks, and every rationale was more reading
  ("View lines 8670-8730 ... to see how ... implement the two-step";
  "check every requirement word for word before writing the exact code").
  B asked the compiler 8 questions (`uses`, `exports`) and ran half as many
  shell commands as A, but read more. With both modes on and nothing else,
  the run read differently, not less, and still did not commit to an edit.
  This is the failure the RLM coder showed on the same issue.
- **C and D made their first edit at call 13**, the call after the note
  ("You have taken 12 steps and changed nothing ... Make your first edit in
  the next step"), and finished.
- **12 of C's 13 stale refusals were the loop's own bug.** Each action
  edited one file several times and named, for each edit, the hash the file
  had when the model read it. After the first edit applied, the rest no
  longer matched and were refused. The 13th named an 8-digit hash, which is
  under the 12 the library takes. D, with both fixed, had none.
- **The finish rule never refused.** C and D ran the task's check after
  their last edit before they said finished.
- **No cache numbers.** Every call's `cached_tokens` is `null` in the
  worker's observation, so the cached share could not be read from these
  runs (the RLM coder's runs saw almost no cache hits on Carbon; #13444).

### C and D against the fix on main

Both meet the issue's four points: every row has `effect` and `approval`;
CLI help lines end with them (`[write, owner]`); MCP descriptions come from
the row (D: `delete an ask [destructive, two-step]`) with `readOnlyHint`
and `destructiveHint`; and a check asserts every non-read row has a rule and
destructive rows are two-step. The task's check passed in both, and the
CLI's unit passed 1,594 of 1,594 (one check added). Neither built what
4fe15b46a0 added beyond the issue's text (a two-step row doing nothing
without `--yes`, `--dry-run`). D also moved the MCP server's read-only filter
onto `effect == "read"`, which the issue implies but does not ask for.

## What was kept

| Change | Commit | Kept because |
| --- | --- | --- |
| The two modes in the JSON-action loop, and the worker's `"intel"`, `"memory"` | 6d88d1bd64 (restored in c046166119 after 0f2d65714d reverted it) | They are what C and D ran on; off by default, so a run without them is unchanged |
| The note at step 12 with no change (memory mode) | e3a3d20afc | No edit in 45 to 52 calls without it (A, B); first edit at call 13 with it (C, D) |
| An edit's `seen` may be the hash at the action's start | e3a3d20afc | 12 of 13 refusals in C were this; 0 in D |

## Measured, and still a guess

Measured: on this one issue, at this tree, on Carbon at low, the runs above
did what the table says. The note was followed by the first edit in both
runs that had it.

Still a guess:

- **That the note is what made the difference.** C and D are one issue and
  two runs; A and B are one run each. The note only exists in the memory
  mode, so there is no arm with the note and no modes.
- **That the modes help.** B read more than A. C and D read less than both,
  but they also stopped reading at step 12. Nothing here separates the
  compiler's answers or the memory from the note.
- **That it holds on other issues and models.** Not run.
- **Repeat reads.** The memory says 24 to 28 reads in C and D were lines it
  already held; whether showing them less often would have helped is not
  known.

## Next

- Run the same four arms on two more real issues, two runs each, before
  reading anything into the gaps: #13435 is one data point.
- Give the note to the no-modes loop too, so its effect can be measured alone.
- Find why `cached_tokens` is `null` in the worker's observation for Vertex.
- G27 (docs/coder/2026-11-13-compiler-for-agents.md): `checks NAME` in this
  loop, once the checker no longer stops `coder/tui/coder.coax` with it.
