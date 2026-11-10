# Audit: the $38 coding run on issue #13134

Written 2026-11-10. This audits one run of the Everglade desktop app's coding
flow: the Opus 5.5 agent that fixed
[#13134](https://github.com/OpenAgentsInc/openagents/issues/13134) ("Green-main
gate is red on origin/main: 229 suites and 29 compiler units fail").

Sources: the Claude Code conversation
`~/.claude/projects/-Users-christopherdavid-work-openagents-issue-13134/cfa0947b-dbb4-4c19-bc59-5c0247c46806.jsonl`,
and the run's line in the AI call ledger, `~/.openagents/usage/ai-calls.jsonl`.

## Result

The run finished the issue. It pushed `fbfd1607f8` to `main` (91 files) and
closed #13134. It left four gaps, which it reported itself:

- It did not run the full gate (`check.sh suites 1/4` to `4/4`), because the
  build hosts' daily budget was nearly spent ($48.80 of $50).
- The compiler has no published build for the new tree until the release hosts
  rebuild it.
- It skipped mutation runs on a few `#authority` files where it changed only
  check budgets.
- Its commit has no real agent ID.

So the claim "main is green" is not yet proven end to end.

## What it cost

| Measure | Value |
| --- | --- |
| Cost | $38.37 |
| Wall time | 2 h 14 min (9:19 to 11:34 AM Chicago) |
| Time the model was working | 32 min |
| Time waiting on tools | 1 h 43 min |
| Model calls | 354 |
| Tool calls | 368 (360 of them Bash) |
| Tokens read from cache | 94,725,082 |
| Tokens written to cache | 3,219,189 |
| Output tokens | 166,358 (61,193 of them thinking) |
| Fresh input tokens | 708 |
| Context at the end | 462,000 tokens |

The run also spent most of the build hosts' $50 daily budget. That is not in
the $38.

## Where the money went

The split below shares the $38.37 out by token kind, at the usual price ratios
(a cached read at 0.1 of input, a cache write at 1.25, output at 5). The
ledger records only the total, so the split is an estimate.

| Kind | Share |
| --- | --- |
| Re-reading the conversation from cache | $25.37 |
| Writing the conversation to cache | $10.78 |
| Output, thinking included | $2.23 |

**The model's own writing was 6% of the bill. The other 94% was paying to
carry the conversation.**

### Cause 1: the conversation was never trimmed

The session ran with a 1,000,000-token context window and never compacted.
The context grew from 58,000 tokens to 462,000. Every model call re-reads all
of it.

- 254 of the 354 calls ran with more than 200,000 tokens of context. Those
  calls cost $33.80.
- If the context had been held to 120,000 tokens, the cached reads would have
  cost about $10.77 instead of $25.37.

### Cause 2: one small lookup per model call

| What the call did | Calls | Cost |
| --- | --- | --- |
| Looked something up (`grep`, `sed -n`, `git show`, `cat`) | 184 | $15.48 |
| Ran checks | 102 | $10.00 |
| Waited on a background job | 12 | $6.91 |
| Started a remote or background run | 25 | $2.04 |
| Edited a file | 22 | $1.94 |
| Other | 9 | $2.00 |

Half the calls were single lookups: one `grep`, then a full re-read of the
conversation, then the next `grep`. A lookup that returned 200 characters cost
the same as any other call at that context size, about 8 to 12 cents late in
the run.

### Cause 3: waits longer than five minutes threw the cache away

The prompt cache lasts five minutes. The agent waited on remote builds with
`until ...; do sleep 15; done` loops capped at 590 seconds. Each time a wait
ran past five minutes, the next call found no cache and wrote the whole
conversation again at the cache-write price.

That happened 7 times. Those 7 calls wrote 2,762,518 of the run's 3,219,189
cache-write tokens: about $9.20, a quarter of the bill, for nothing.

### Cause 4: the issue was not one issue

#13134 was a bundle of about ten unrelated faults behind one red gate: the
suite runner running files with no `main`, 108 check files that can't run
alone, a sanitizer runtime missing from the static toolchain, constants that
don't parse, a missing include, `rg` not installed on hosts, short timeouts,
stale recorded hashes, compile-time budgets, and a query unit the checker
could not index. The fix touched 91 files.

One session carried all ten. Every fault's dead ends, logs and diffs stayed in
the context and were re-read on every later call for the other nine.

The picker chose it for a sound reason (it blocked #13102 and #13135). It had
no way to know the size. Nothing in the flow estimates an issue or splits one.

## Why it took so long

The model worked for 32 minutes. The other 1 hour 43 minutes was waiting.

- About 75 minutes went to the wait loops on remote suite runs. The suites ran
  in groups on build hosts, each group taking 5 to 10 minutes, and the agent
  ran the groups several times as fixes landed.
- One local command took 334 seconds, another 300, another 255.
- It ran a `git bisect` in the background to find which commit changed a
  recorded hash.

The "looping of building" was real. The agent ran a suite group, read the
failures, fixed some, and ran the group again. With 229 failing suites that
is the honest shape of the work, but two things made it worse:

- It learned the suite runner's main fault (files with no `main`, 188 of the
  failures) by running suites, not by reading the runner first.
- It waited inside the conversation. Every wake-up was a model call at full
  context.

## What to change

In order of how much each saves.

1. **Hold the context down.** Run the coder with the standard context window,
   not the 1,000,000-token one, so the harness compacts at about 150,000
   tokens. Expect this alone to cut the bill by a third to a half.
2. **Never sleep past the cache.** Tell the coder to wait at most 270 seconds
   per call, or better, to end its turn while a long job runs and have the app
   wake it when the job exits. The app already owns the process; it can hold
   the wait at no token cost. Saves about $9 on a run like this.
3. **Split before coding.** Before the coding flow starts, have the picker
   size the issue. If the issue names more than one fault, write one
   sub-task per fault and run each in a fresh session, with a short note
   handed from one to the next. Ten sessions of 60,000 tokens cost far less
   than one of 462,000.
4. **Batch lookups.** Say in the coder's brief: gather what you need in one
   command, not ten. A helper that takes several patterns and files and
   prints trimmed results would turn the 184 lookup calls into about 40.
5. **Give lookups to a cheap reader.** The brief bans subagents. Allow one
   kind: a read-only Haiku reader that searches and returns a short answer.
   It keeps file dumps and logs out of the Opus context.
6. **Read the gate before running it.** For a red-gate issue, the brief should
   say: read the runner and one failing log per error code first, and group
   failures by cause, before any full run.
7. **Set a spending limit the owner chooses.** The flow has none. Show the
   running cost in the panel (it does) and stop at the limit the owner sets,
   with the work so far committed to the worktree. The limit is the owner's
   setting, shown on screen, not a hidden cap.
8. **Count the build hosts' spend.** The ledger line should carry the remote
   budget a run used, next to its tokens.
9. **Pick the model by task.** The diagnosis needed Opus. The mechanical
   part (renaming a clashing `atan2`, refreshing hashes, adding `#run_steps`
   lines to suites) did not. A plan written by Opus and carried out by a
   cheaper model in fresh sessions fits this shape of issue.

With changes 1 to 4 the same work should cost $8 to $12. That figure is an
estimate from the numbers above, not a measurement.

## What to measure next

The ledger records one line per run. To hill-climb, it needs per run:

- the largest context reached,
- the count of calls that wrote more than 60,000 tokens to cache (a lost
  cache),
- seconds spent waiting on tools,
- the count of tool calls by kind.

All four are in the conversation file today. The script that produced this
audit's tables read them from there.
