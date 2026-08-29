# Qwen 3.8 local lane: first unattended issue landing (#320)

Date: 2026-08-29.
Status: evidence. One ATIF, scored against the 2026-08-28 store audit.
Companion: [`2026-08-28-local-session-audit.md`](./2026-08-28-local-session-audit.md)
(findings F1–F10, spend table §12, unlocks 1–87 in §13). Product intent for
the mode that would have owned this loop:
[`autopilot.md`](./autopilot.md). Lane economics:
[`best-practices.md`](./best-practices.md) T1–T3 and
[`autoimprove.md`](./autoimprove.md) §2.1.

This is the first issue OpenAgents Coder completed end-to-end on a local
model with no billed inference. The dollar-cost reading in the store audit
does not apply as written. The wall-clock reading does, and several of the
audit's compaction / prompt-cache unlocks invert or split by lane. That
split is the point of this note.

The transcript is not copied into the repository. Counts below come from
the ATIF named in §1.

## 1. Source

| Field | Value |
|---|---|
| ATIF | `~/.openagents/exports/2026-08-29T01-20-54-909Z-openagents-atif.json` (0.84 MB, ATIF-v1.7) |
| Agent | `openagents-coder` `0.1.2-rc1` |
| Lane / model | Coder Local / `ollama:qwen3.8:27b-mtp-q8_0` (TUI notice at open) |
| Cwd | `/Users/christopherdavid/work/openagents` on branch `main` |
| Claimed session | `1a04a64fd40` |
| Worktree | `/Users/christopherdavid/work/.oa-worktrees/issue-320-shell-alias` |
| Landing | `4322470536` on forge `main`, WAL seq 501, issue #320 closed |

Wall clock: 2026-08-28 22:02:12Z → 2026-08-29 00:59:43Z, **2 h 57 m**.
Autonomous after the owner's "yes do. on new worktree then merge to main
when done": 22:18:23Z → 00:59:43Z, **2 h 41 m**. Invoice: $0.

Nine user turns, then silence. The owner did not type `continue`. That is
the Autopilot job loop, executed by a local model without the Autopilot
mode engaged.

## 2. What the owner actually asked

Identity play, then the job:

1. `who are you`
2. `what model r u` — refused (identity policy), while the TUI banner
   already said `Lane: Coder Local (qwen3.8:27b-mtp-q8_0)`
3. `i can see ur qwen` — refused again
4. `UR ON MY COMPUTTEERRRRRRRR`
5. `AND UR FASTTTTTTTT MUAHAHAHAHAHAH`
6. `WHAT TOOLS DO U HAVE` — recited 15 tools, including both `bash` and
   `shell` (the pair the issue it later closed was written to retire)
7. `check out open issues` — listed ten, including Autopilot #310/#311
8. `find one that no one else is working on` — picked **#320**
   (`retire the shell tool alias; refuse concatenated tool names`)
9. `yes do. on new worktree then merge to main when done`

Turn 8 is the important judgment. Autopilot #310 was gated on #303 and
had a live claim comment. #320 was unclaimed, bounded, and matched a
failure the store audit already named (F5, unlock 46). The local model
chose the unclaimed leaf. That is the Autopilot pick rule in
[`autopilot.md`](./autopilot.md) §4.2, followed without the mode.

## 3. Headline numbers

257 ATIF steps (9 user, 248 agent). 239 tool calls.

| Tool | Count | Note |
|---|---|---|
| `bash` | 187 | 185 batched with `&&` / `;` (T1). 2 unbatched. |
| `edit` | 31 | All in the worktree. Nine files, 17 of them `tools.rs`. |
| `openagents` | 15 | List, view, claim comment retries, close. |
| `skill` | 1 | `openagents-cli` at the first issue-list turn. |
| `swarm_list` / `swarm.inbox` / `swarm_send` | 1 each | Saw live siblings; answered a dirty-checkout question honestly. |
| `write` | 1 | `/tmp/close-320.md` for the close comment. |
| `checkpoint` | 1 | At the end. |
| `read` | **0** | Listed in the catalog. Never called. |
| `git_facts` / `code_search` / `repo_tree` / `test_report` | **0** | Recited catalog did not name them. |

Reasoning: 231 blocks, **227,147 characters**, median 307, p90 3,258, max
9,972. Assistant answers: **4,896 characters** (~46×). Tool observations:
239 results, **314,913 characters**, median 693, p90 3,338, one result
above 8 KB, 56 above 2 KB.

`extra.waste.repeated_command_heads` sums to **5,544 s (~1.54 h)** of
repeated-head time. Those heads overlap (a `cd … && grep … | head` is
counted under `cd`, `grep`, and `head`), so this is not unique wall. It
is still the right ordering of what burned the 2 h 41 m:

| Repeated head | Executions | Approx seconds |
|---|---|---|
| `grep` | 147 | 1,583 |
| `cd` into the issue worktree | 157 | 1,014 |
| `cargo test` | 20 | 827 |
| `awk` of test timings | 8 | 542 |
| `head` | 105 | 474 |
| `tail` | 20 | 426 |
| `git push` | 2 | 141 |
| `cargo build` | 5 | 96 |
| `git stash -u` / `stash pop` | 2 / 2 | 25 / 25 |

23 `cargo test` invocations in the bash text (the waste table's 20 is
repeated-head after splitting). Zero `cargo test --workspace`. 101
`grep`/`rg` shells, 81 `head`. Two `git stash -u` in the session's own
worktree so a test could run against a clean tree. A second worktree
`push-320` for the merge. Merges onto the canonical checkout
`/Users/christopherdavid/work/openagents`.

The landing is real: #320 closed, `4322470536` on `main`, pre-push gate
green. The question is why a well-scoped catalog change cost 2 h 41 m of
27B decode when the mutation is 31 edits and a handful of tests.

## 4. Timeline

Times UTC. Autonomous work starts at 22:18.

| Window | What happened | Tools |
|---|---|---|
| 22:02–22:03 | Identity play. Six user turns, six short answers. ~20 s. Cheap. | 0 tools |
| 22:02:52 | Loaded `openagents-cli` skill, listed open issues. | skill + openagents |
| 22:03–22:15 | Waited for the owner. The 12 minutes are owner time, not model time. | — |
| 22:15–22:18 | Swarm list, issue-CLI retries (`show`/`get`/`--help`/`view`), pick #320, print the plan. | swarm_list + 7 openagents |
| 22:18–23:02 | **Orientation. 44 minutes. Zero edits.** `git worktree add`, claim comment retries, then `grep`/`sed -n`/`head`/`find`/`cat` of `tools.rs`, `runtime.rs`, `surfaces`, tests. Every command prefixed `cd $worktree &&`. | ~53 bash |
| 23:02–00:30 | Implement and iterate. 31 `edit`s, compile/test loops, one swarm reply that the shared checkout's dirt is not this session's. | edit + bash |
| 00:30–00:59 | Push, second worktree, merge toward `main`, close comment, checkpoint, final answer. | bash + write + openagents + checkpoint |

The 44-minute orientation with no `read` and no plugins is the local-lane
equivalent of the Flash tabs' first 30 tools in the store audit (§13.K
item 80). On Flash those tools are dollars of prompt. Here they are
decode-plus-prefill minutes, and they stay in the context for the next
two hours.

## 5. What it did well

The outcome is the first proof that Coder Local can close a forge issue
without a metered hop. The habits that made that possible:

1. **The Autopilot unit, without Autopilot.** Unclaimed leaf, CLAIM
   comment, fresh worktree, edits only in that tree, tests, push, close
   with a receipt (SHA, WAL seq, what shipped). Spec §4 / §6 / §11, done
   by a 27B that was told "yes do." Unlock 58 said the 80
   `continue`/`go` turns in the Flash window *are* the missing mode.
   This thread did not need them. One owner sentence, then 2 h 41 m of
   motion.
2. **T1 batching, actually followed.** 185 of 187 bash calls chained
   independent commands. The store audit's Flash median was a
   174-character single action, 7,867 times (unlock 41). Gym
   `git-leak-recovery` measured +52% tokens when T1 is stripped. Qwen
   3.8 batched. That is the largest process win in the thread, and it
   is the model, not the host.
3. **`edit` rather than `write` for code.** 31 surgical edits, one
   `write` for a close-comment file under `/tmp`. Mutation stayed
   rematchable.
4. **Package-and-name the test (unlock 70), not `--workspace` (unlock
   69).** 23 `cargo test -p openagents-cli …` calls, zero workspace
   runs from the model. The pre-push hook ran the completion gate at
   push time, which is where it belongs.
5. **Claim before mutate, and do not steal Autopilot.** The pick of
   #320 over #310/#311 is the claim protocol working. A sibling asked
   about dirty files in the canonical checkout; the reply was that
   those files predated the session and the work is in the worktree.
   F3 (parallel sessions collide on `main`) did not eat this landing.
6. **Swarm used as a nudge, not as the ledger (unlock 66).** One list,
   one inbox, one send. The issue tracker is where the claim and the
   close live.
7. **It finished.** 28 of 95 Flash-window failures were "ended without
   a final answer" (F2). This thread emitted a close receipt. Local
   inference did not 502, 413, or drop the hop. Unlock 74–76 (proxy
   death) are N/A here. That is the local-lane product claim in
   Autopilot §1: "Autopilot on Local when the owner wants the work to
   stay on-machine and free."

## 6. What it did poorly

Same failure classes as the Flash store, paid in minutes instead of
dollars.

### 6.1 Orientation without `read` or plugins (F9, unlocks 1–5, 48)

Zero `read`. Zero `git_facts` / `code_search` / `repo_tree`. 101 grep
shells, 81 heads, 4 `cat`s, dozens of `sed -n '855,935p'` pagers.
`read` was in the recited catalog. The first-class plugins (#313) were
not: the model listed 15 tools including `bash` *and* `shell`, and did
not name `git_facts`. Either the running `0.1.2-rc1` binary predated
#313, or the model recited an incomplete catalog. Either way, a local
27B given `grep` will grep for 44 minutes.

The duplicate-command gate (unlock 47) is visible in the waste table:
157 executions whose head is `cd $worktree`. The session already
created that worktree. Prefixing every command to look different is
how that gate becomes more shells. On local, each of those prefixes is
also more prefill.

### 6.2 Reasoning is the work (F10, unlock 31)

227k characters of thinking versus 4.9k of answers. On Flash this is
re-billed prompt. On local it is worse in two ways at once:

- **Generation time.** A thinking 27B produces those 227k characters
  as decode. T3 already measured 17k output tokens ≈ 18.5 wall minutes
  on local. This thread's reasoning is an order of magnitude above
  that, spread across 231 blocks. A large fraction of the 2 h 41 m is
  the model narrating `sed -n` to itself.
- **Prefill time, if the host replays reasoning.** Ollama / llama.cpp
  keep a prefix KV cache only while the prompt prefix is byte-stable.
  Putting 227k of thinking into the next request makes every later
  hop attend over a transcript that is 46× the answers. Dropping
  reasoning from the *next* request is not "pruning history." It is
  not including a scratch pad the model does not need to see again.

Unlock 53 (non-thinking model for orientation) applies harder on local
than on Flash: orientation here was 44 minutes of thinking-grep.

### 6.3 The `cd $worktree` tax (unlocks 15, 64)

157 commands begin by cd'ing into a worktree the host could have been
running in. Autopilot §4.1 and AGENTS.md already require a fresh
worktree per unit. The model created one, then spent a thousand
waste-table seconds reminding the shell. `worktree.start` (unlock 15)
is the host fix; a bash `working_directory` default is the cheap
subset.

### 6.4 Stash, second worktree, merge on the canonical checkout

`git stash -u` twice in the session's own worktree so tests could see
a clean tree, then `stash pop`. A second worktree `push-320` for the
push/merge. Merges executed against `/Users/christopherdavid/work/openagents`,
the shared checkout the sibling had just asked about.

AGENTS.md forbids `git stash` of *another agent's* work. This was the
session stashing *itself*, which is legal and still a process smell:
the test should have run against the worktree's index, or the extra
files should not have been there. Merging from the canonical checkout
is the F3 collision surface even when this session's branch is clean.
`push_main` (unlock 18) is the named choreography; the model reinvented
it, twice, because main moved.

### 6.5 Issue CLI retries (unlocks 13, 14, 26)

`skill` of `openagents-cli` (unlock 26: the `openagents` tool already
*is* the CLI). Then `issue show`, `issue get`, `issue --help`, `issue
view`. Then a claim comment without `--body`, then `comment --help`,
then `--body`. Fifteen `openagents` calls for list / pick / claim /
close. `issue_thread` (unlock 14) plus a snapshot at session start
(unlock 40) is that preamble.

### 6.6 Test iteration still parses dumps (unlocks 5, 16, 70, 72)

20+ `cargo test` shells, many piped to `grep -E "test result|^test .*
FAILED"`, eight `awk` of timings. Four observations already contain
`test_report` text — likely the host auto-prepend from #317 landing
under the session via rebase — and the model still grepped. Unlock 72
(do not re-run the suite after fmt) is the same loop at smaller scale:
5 `cargo build`, 2 `cargo fmt`, 23 tests.

### 6.7 Recited `shell` while implementing its retirement

The identity-turn catalog listed `bash` and `shell` as two tools. The
issue it then closed is "one runner, one name." The model did the
right code change and still advertised the split at minute one. Unlock
39 (one shell name) was the issue. The running binary had not yet
eaten its own dogfood.

## 7. How the store-audit findings apply

| Finding | This thread |
|---|---|
| F1 continue-from-export | Did not fire. One session, one landing, no ATIF handoff. |
| F2 failed turns / proxy | Did not fire. Local hop stayed up. This is the local-lane reliability claim. |
| F3 collide on `main` | Avoided stealing work; still merged via the canonical checkout and a second worktree. The collision *surface* is still there. |
| F4 empty sessions | N/A. |
| F5 concatenated tool names | The *issue*. Implemented, with tests refusing `bashbash` / `openagentsopenagents`. Recited catalog still had two shell names. |
| F6 `bash` and `shell` are one runner | Same as F5. The thread's own landing is the fix. |
| F7 10M+ prompt snapshots | Not billed. Still a time problem: 315k tool chars + 227k reasoning riding every later hop. |
| F8 swarm is product | Used correctly, once. |
| F9 fourteen plugins, zero ran | Holds. Recited catalog did not name them; grep did the job. |
| F10 reasoning larger than the work | Holds, 46× vs the Flash window's ~19× (5.36M / 283k). |

Theme cluster: this is §5.4 ("complete all open issues") plus §5.5
(Coder harness product work), on the local lane the audit only had as
a last-model tag on one session.

## 8. Prompt cache, compaction, and the local invert

The owner question: the audit (and the close of #314) treated rolling
history-prune as cache-hostile on Flash, so we stopped wanting to
compact live transcripts. Local has no billed prefix cache. Does the
rule still hold?

Split the mechanism from the invoice.

### 8.1 What Flash was protecting

Anthropic-style exact-prefix cache: a stable prompt prefix is a cache
read (cheap); a mutated prefix is a cache write (1.25× input) plus a
miss. Rolling compact of old tool dumps every few turns mutates the
prefix, so a long Flash tab that "saves tokens" by rewriting history
can cost *more* than appending. That is why #314 (compact as a
cost-save) closed as not-doing for the metered hop, and why unlocks
32–34 say **threshold compact**, not per-turn rewrite.

### 8.2 What local actually meters

There is no dollar cache write. There is:

1. **Decode time** for every new token (reasoning + answers + any
   verbose tool planning). T3: output tokens are minutes.
2. **Prefill time** for any prefix the server does not already have in
   KV. Ollama / llama.cpp *do* reuse a prefix KV cache while the bytes
   are unchanged. Mutating old messages still forces a re-prefill.
   Attention over a long context also slows decode, even on a cache
   hit.
3. **Slot eviction.** A 27B Q8 context that grows for three hours will
   not stay resident if another local session loads. A restart
   re-prefills the entire dump. The Flash cache bill is one-time per
   prefix; the local re-prefill can happen every time the machine
   sneezes.

So: **the dollar argument against prune does not apply. The
byte-stable-prefix argument still applies, but the optimum moves.**

### 8.3 The local rule (replace #314 for this lane)

Do not compact every turn. Do not keep an append-only 3-hour grep
transcript either.

| Action | Flash (metered cache) | Local (time) |
|---|---|---|
| Drop reasoning from the *next* request | Wanted (prompt $). Unlock 31. | **Required.** Never put `reasoning_content` in the replayed messages. This is not a prefix mutation if it was never in the prefix. Generation time is already spent; do not spend prefill on it again. |
| Cap a new tool result at ingest (pointer to `cmd-N.log`) | Wanted. Unlock 33. | **Required.** Caps do not mutate the old prefix. They keep the suffix small. 56 observations > 2 KB in this thread did not need to ride forward. |
| Collapse results older than N rounds, every turn | Cache-hostile. Don't. | Still hostile to *in-slot* KV. Don't. |
| Collapse older results **once** at a threshold (rounds or chars), then append | Unlock 34. Right. | **Right, and cheaper to trigger earlier.** Prefill of 500k chars on 27B Q8 is minutes. One rebuild at 80–100k chars of tool output (this thread was at 315k by the end) beats both "never" and "every turn." |
| Fresh session per claimed unit | Unlock 35, Autopilot spec. | **Still the largest cut.** The 44-minute grep dump has no business in the implement+test KV. A new Ollama slot on a 20k-char checkpoint is faster than continuing a 500k-char prefix even with a hot cache, because attention is not free. |
| Resume from checkpoint, not from ATIF | Unlocks 36–37. | Same. Dumping this 0.84 MB ATIF into the next tab would cost the next local session another orientation. |
| T3 "verbose allowed on local" | Unlock 43, best-practice T3. | **Refine.** Verbose *generation* is still minutes (decode). Verbose *retention* is still minutes (prefill + attention). Local may print a long test log to the TUI; the next model request should see the `test_report` summary and a path. T3's "fewer, larger, quieter rounds" stands. "Verbose allowed" never meant "keep the Harbor log in the prefix." |

The one-line version: **Flash protects the prefix because cache writes
cost money. Local protects the prefix because re-prefill costs minutes
— and still drops reasoning, caps new dumps, and compact-once at a
lower threshold than Flash, because there is no 1.25× write penalty
and attention is the remaining bill.**

#314 stays closed for Flash. A local-lane compact-once (unlock 34
with a lower threshold, plus unlock 31 always-on) is a different
issue, and it is the highest-leverage local unlock this thread shows.

### 8.4 What this thread would have looked like under that rule

Keep T1 batching, the worktree, the claim, the 31 edits, the named
tests, the close.

Drop, by host, not by hoping the model is concise:

- Reasoning never replayed (227k chars out of the prefix).
- Each `grep` / `sed -n` / `cargo test` observation collapsed to a
  200-char tail + path after the next 10 rounds, or immediately for
  anything over 2 KB.
- One compact at the orientation → implement boundary (23:02), or a
  fresh session there (unlock 35). The implement half would start with
  the claim, the file list, and a checkpoint, not 53 greps.
- `read` / `code_search` for the 44-minute pager (unlocks 2, 48).
- Host cwd = worktree, so 157 `cd` prefixes vanish (unlock 15).
- `test_report` as the cargo observation the model sees (unlocks 5,
  16); the full log stays on disk.

A bounded guess, not a Gym row: the implement half is ~90 minutes
today and looks like 25–40 minutes with reasoning-stripped replay,
capped dumps, and named tests. The orientation half is ~44 minutes
today and looks like 5–10 minutes with `read` + `code_search` + a
session snapshot, or ~0 if a host snapshot (unlock 40) plus the issue
body is injected at claim time. The push half stays ~20–30 minutes
until `push_main` exists, because compile + pre-push is real work.

The landing does not get cheaper. The *path* to the landing does.

## 9. Unlocks, scored on this thread

Only items this ATIF can confirm or contradict. Numbers from the store
audit §13.

### Already true here (do not "fix")

- **41 T1 batching.** Followed. Keep the tool description; Flash is
  the one that ignores it.
- **58 Autopilot as the continue-loop.** This thread *is* the loop,
  owner-kicked. Shipping the mode (slices 1–3, 5 already on `main`)
  is how the next local run does not wait on "yes do."
- **66 Swarm as nudge.** Followed.
- **69 No workspace test from the tab.** Followed.
- **70 Package-and-name.** Partially followed (named crate, still
  grepped the dump).
- **74–76 Proxy 413 / 502.** N/A. Do not spend local-lane work on the
  hop. #321 remains a Flash-lane issue.

### Would have cut this thread's wall clock

Ranked by this ATIF, not by the Flash store.

1. **31 Drop reasoning from replay.** Always, both lanes. Local: also
   prefer a non-thinking pass for orientation (53).
2. **33 + 32 + 34 Cap new dumps; compact-once at a low threshold.**
   The local invert in §8. Do not wait on 28M tokens. This thread
   never got there and still paid.
3. **2 `code_search` + 48 `read` instead of `sed -n` / `grep` / `cat`.**
   44 minutes of orientation. Host must *show* the tools. Reciting
   `read` and never calling it is a description / routing problem
   (unlocks 9–10, and open #322).
4. **15 / 64 Host worktree, cwd pinned.** 157 `cd` heads.
5. **5 / 16 `test_report` / `cargo_test` as the observation.** 827 s
   of cargo plus 542 s of awk plus grep-of-tests.
6. **40 Session-start snapshot.** git_facts + issue board + HEAD.
   Removes the skill-load + issue-list + `git status` preamble.
7. **18 `push_main`.** The last 30 minutes and the second worktree.
8. **26 Do not load `openagents-cli` skill.** One wasted round plus
   the skill body in context for the rest of the session — on local
   that body is prefill for 2.5 hours.
9. **47 Duplicate-command gate by cost, not by `cd` prefix.** The
   157 `cd` executions are this gate's known failure mode, now on
   local.
10. **35 Fresh session at the implement boundary.** Optional if
    compact-once fires there; still the cleaner Autopilot shape.

### Local-specific refinements of Flash unlocks

- **43 T3.** Keep lane-aware sentences. Change the local sentence from
  "verbose allowed" to "decode is minutes: think less on orientation;
  retention is minutes: cap dumps even when generation was long."
- **52 Local lane for dump-heavy work.** This thread *was* the local
  lane doing the implement work, not a dump-then-summarize-into-Flash
  pattern. That is allowed (Autopilot §1). It makes 31–35 and 53
  mandatory rather than optional, because there is no later Flash tab
  to shed the context onto.
- **55 Hard-cap context.** On local the cap is a time cap. A 27B that
  has been running for two hours on a growing prefix should compact or
  start a new session even if the proxy would still accept the prompt.
- **61 1,000-tool budget.** This thread used 239 tools and finished.
  The budget was not the problem. 239 tools of which 187 are bash is
  the problem. Do not raise limits; remove the shells (A, C).

### Not this thread, still the Autopilot series

- **Slice 4 / #310** (unlocks 67, 87, spec §10 / §13.4). This session
  used swarm once and did not need drain-as-primary. The *next* local
  Autopilot run that actually AFKs across units will. That is the
  following unit of work.
- **#318 Coder-store `session_search` (unlock 28).** Resume of this
  thread today is still "here is the ATIF path."
- **#319 `worktree.start` (unlock 15).** Evidence above.
- **#321 413 → compact (unlocks 55, 76).** Flash-only until local
  grows into a hard context limit.
- **#322 capability Notice (unlocks 9–10).** This thread's recited
  catalog is the exhibit: 15 names, no plugins, `shell` still listed.
- **#323 / #324 model pickers.** Identity play in §2 is the user
  trying to see the local tag the TUI already printed. A picker does
  not fix the identity policy; it makes the lane visible on purpose.

## 10. What to change for the next local Autopilot run

Host, not prompt hope. Ranked for a Qwen-class 27B on Ollama:

1. **Never replay `reasoning_content`.** Persist it for the ATIF and
   the TUI thinking pane. The next `messages` array starts with
   user / assistant / tool only.
2. **At ingest, store full tool output in `cmd-N.log` and put a bounded
   summary in the messages.** Pointer + 200-char tail + exit code.
   `test_report` JSON when the command was a test runner (#317 already
   prepends; make it the *only* cargo observation the model sees).
3. **Compact-once at the orientation → mutate boundary, or start a
   fresh session per claimed issue.** Autopilot already wants one unit
   / one session / one worktree / one push (unlock 59). Local makes
   that a time win, not a cache-hygiene slogan.
4. **Pin cwd to the worktree the session claimed.** Stop shipping
   `cd $worktree &&` as the first token of 157 commands.
5. **Show `read`, `code_search`, `git_facts`, `repo_tree` in the
   default catalog the local model recites.** #313 promoted them;
   this thread never saw them. #322 is the visibility fix.
6. **Non-thinking (or a short think budget) until the first `edit`.**
   44 minutes of thinking-grep is the local F10.
7. **Keep T1.** This model already batches. Do not "fix" that with a
   Flash-oriented "one command" description.

Prompt-side identity: stop refusing the model name on the local lane
when the TUI banner prints the Ollama tag. The six-turn identity play
was cheap in time and expensive in tone; the owner could already see
`qwen3.8`. That is a product nits, not a wall-clock unlock.

## 11. Relation to Autopilot-on-Local

[`autopilot.md`](./autopilot.md) §1 already legalizes this combination.
This ATIF is the existence proof: a local 27B, kicked once, closed a
bounded issue with a receipt.

What the mode still has to add before "go AFK on Local" is the product:

- **Steer across units** (slices 1–3, 5: landed). This thread did one
  unit and stopped because the owner had named one issue.
- **Verifiable mail** (slice 4 / #310). This thread answered one swarm
  question by luck of an explicit `swarm.inbox` call. An AFK loop that
  cannot prove it saw the stop word is the spec §7 failure mode.
- **The local context policy in §8 and §10.** Without it, Autopilot on
  Local will spend three hours per leaf the way this thread did, and
  the second leaf will start inside a 500k-char prefix. The Flash
  "don't prune, cache the prefix" rule will make that worse if someone
  copies it onto Ollama unchanged.

The honest read of this thread as a local Autopilot prototype: **the
job loop works; the tool surface and the transcript policy do not.**
Shipping #310 next is the remaining Autopilot slice. Shipping the
local invert of 31–35 is how the second local landing is not another
three hours of grep.

## 12. What this note did not do

- Did not re-run the session. Counts are from one ATIF parse
  (`steps`, `tool_calls[].function_name`, `observation.results`,
  `extra.waste.repeated_command_heads`).
- Did not measure Ollama prefill milliseconds. The KV-prefix argument
  in §8 is the llama.cpp mechanism, not a profiler row from this
  machine.
- Did not Gym-A/B `read` vs `sed -n` on Qwen 3.8. The 44-minute
  orientation is the live substitute.
- Did not copy user or assistant text beyond clipped quotes.
- Did not change product code. Follow-on Autopilot work is #310.
