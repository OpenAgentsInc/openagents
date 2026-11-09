# Agent workflow audit, 2026-11-09

This audit looks for ways to speed up how agents build, land and release
work in this repository. It starts from the green-main pipeline
([green-main-pipeline.md](green-main-pipeline.md), #13102), answers the six
open questions about its cutover, and then lists what else costs time.

The main finding: breakage on `main` is real, but it isn't the largest cost.
Measured from the transcripts, agents spend about 60% of their running time
waiting, and every wait is paid for in tokens. The pipeline fixes the
breakage. It doesn't fix the waiting, the context size or the shared files
that every agent edits.

## What I measured

| Source | What it gave |
| --- | --- |
| The coordinator session of 2026-11-07 to 11-09 (57 hours) and its 107 subagent transcripts | Time per tool call, tokens per turn, rebases, conflicts, and the owner's messages |
| 40 other sessions from the last 8 days | Counts only |
| `git log` on `origin/main`, last 7 days | Commits a day and the files most commits touch |
| `~/.openagents/remote/ledger.tsv` | Remote build jobs and their billed cost |
| `coax/remote/*.sh`, `coax/web/release.sh`, #13102 | The pipeline as built |

Times come from transcript timestamps: a tool call's time is the gap between
the call and its result. Token counts come from each reply's usage record,
counted once per reply.

## Where agent time went

The 107 subagents ran for 216 agent-hours. The median agent ran 81 minutes,
one in ten ran over 4 hours, 37 ran over 2 hours, and the longest ran 12
hours.

| Where the time went | Hours | Share |
| --- | --- | --- |
| `sleep` and `until` loops that watch a log file or a process | 49.2 | 23% |
| Stopped, waiting for a message or a background job to finish | 69.3 | 32% |
| Inside the Mac's build-slot wrapper (`heavy.sh`), queued or building | 17.2 | 8% |
| `remote.sh`, `wasm.sh`, `tool.sh` and `coax test` calls | 14.7 | 7% |
| Every other tool call | 8.3 | 4% |
| The model reading and writing | about 57 | 26% |

Three more counts from the same transcripts:

- **Rebases:** 467 `git rebase` commands, 29 results with a conflict, and 1
  rejected push out of 82. Agents don't lose pushes. They rebase, re-check and
  rebase again until they get through.
- **Compile limits:** 103 tool results held an `error[E0400]`.
- **Relays:** before the remote hosts went live on 11-09, subagents sent the
  coordinator 39 "please run this build" messages, and the coordinator ran
  each one and sent the output back.

## Where the tokens went

The owner ran out of weekly capacity at 4:53 PM Chicago time on 11-08 and
stopped new subagents. Tokens are now a limit on throughput, as build slots
were last week.

| Measure | Subagents | Coordinator |
| --- | --- | --- |
| Replies | 24,736 | 2,398 |
| Cached input read | 5.37 billion tokens | 545 million |
| Cached input written | 96 million | 4.3 million |
| Output | 16.1 million | 1.1 million |
| Average context per reply | about 220,000 tokens | about 229,000 |
| Context summarised ("compacted") | 88 times | 11 times |

- A subagent's first reply already carries a median of **77,000 tokens**
  before it has read any code.
- The median subagent peaks at **361,000 tokens**. 101 of 107 pass 150,000.
- Each `sleep` loop is one reply. 1,811 of them at about 220,000 tokens each
  is about 400 million tokens read only to wait.

## The six cutover questions

### 1. Fix first, or accept a baseline?

Accept a baseline and set `main-green` now.

- `release.sh` already works this way. A check stops a release only when it
  fails on staging and passes on production. Apply the same rule to `main`:
  a full run is green when it has no failure that the baseline doesn't have.
- `after.sh` already has the place to record it. On its first run it puts
  today's failures on the red list, owned by nobody, under one issue. Treat
  that list as the baseline and move `main-green` when a run adds nothing to
  it.
- Fixing first means `main-green` waits days while `main` takes 130 to 400
  commits a day. Agents keep starting from a broken `main` the whole time,
  which is problem 2 of the pipeline's own list.
- Burn the baseline down as ordinary issues. An entry leaves the baseline
  when it passes, and can't come back.

### 2. Should the 227 count as failures?

Mostly no. From the description in #13102, they are a gap in how suites are
found. I didn't reproduce the run.

Two fixes in `tool.sh suites`, which already builds a `#load` graph:

- A file that another file loads isn't a suite. Run it only through its
  loader.
- A loader counts as a suite when any file it loads has tests, even if it has
  none of its own. Today `aethyr/actors/checks.coax` is skipped, so
  `tests/journey.coax` runs only on its own, and fails.

After both, whatever still fails alone is a real failure and belongs on the
baseline. The second fix matters more than the count: those tests run nowhere
today.

### 3. Is one full run an hour enough?

Start runs by landings, not by the clock: start a run when 10 commits have
landed or 60 minutes have passed, whichever comes first, and never run two
at once.

- At $0.30 a run, back-to-back runs through 20 busy hours cost about $12 a
  day. That fits the $50 cap and the design's own estimate.
- With `OA_BISECT_WIDTH=4`, a range of 27 commits takes 3 rounds, and each
  round runs only the failing checks. A long range is affordable.
- The larger problem is the run's length. It took 31 minutes, against 17 on
  free hosts, because agents' jobs shared the slots. Let pipeline jobs go
  ahead of agents' jobs in `remote.sh`'s slot search.

### 4. Should the ledger bill by slot?

Yes. Better, bill hosts and not jobs.

- The cap exists to limit real spend. Billing each job a whole host's price
  means the $50 cap stops work at about $17 of real spend.
- The exact figure is host uptime times the hourly rate. `remote.sh` already
  records each start and creation. Record each stop too, or read uptime from
  `gcloud`, and compute spend from that.
- Keep a per-job line at `rate / slots` so `remote.sh cost` can still show
  what each kind of work costs.

Ledger today: 389 jobs, 43.4 billed host-hours and $11.94, which is about $4
of real host time.

### 5. What could go wrong at cutover?

| Risk | Why it's likely | What to do |
| --- | --- | --- |
| Shared files send most branches back | The integrator returns a branch that conflicts with `main`. `INVARIANTS.md` took 330 commits in 7 days and `wasm-budgets.txt` took 120. Today agents absorb these conflicts with rebases. After cutover each one is a rejected branch. | Fix the shared files first. See "Shared files" below. |
| Old worktrees keep pushing `main` | Every agent uses one Git identity, so GitHub can't tell the integrator from a builder. The median agent runs 81 minutes and some run 12 hours, so agents briefed before the cutover stay alive well after it. | Give the integrator its own GitHub identity and restrict pushes to `main` to it. Until then, add a `pre-push` hook: `core.hooksPath` already points every worktree on the Mac at one `.githooks` directory. Message every live agent at the cutover. |
| The integrator stops and nothing lands | It's a shell loop on the owner's laptop. That Mac froze and was rebooted on 11-07, and ran at a load of 110 during a release on 11-09. | Run `integrate.sh` and `after.sh --watch` on an always-on VM, since their own work is light. Write a heartbeat each round. State the fallback in `AGENTS.md`: when the heartbeat is over 15 minutes old, agents may push `main` directly. |
| The revert undoes work in progress | A fix needs a rebase and a check, and 30 minutes is shorter than most agents' cycle. | Revert only when no branch in the queue names the red-list issue. If the revert itself conflicts with later commits, don't force it: flag the issue for the coordinator. |
| `main-green` falls far behind | One flaky check stops it from moving, and every new worktree starts further from `main`. | Report a check that fails and then passes as flaky, as `after.sh` does, and don't let it hold `main-green`. Alert when `main-green` is over 3 hours behind `main`. |
| Failures can't reach their author | 8 of the last 220 commits carry an `Agent:` trailer. | `submit.sh` adds the trailer, so this fixes itself at cutover. Before then, add a `commit-msg` hook. |

### 6. Is this the right size for our load?

The design is right and small: about 1,300 lines of shell. Turn it on in
two steps, because the two halves carry different risk.

1. **Now, with no change for agents:** run `after.sh --watch` with the
   baseline rule. This gives `main-green`, releases from it, worktrees that
   start from it, the bisect, the red list and per-commit size history. It
   covers four of the five problems the design lists.
2. **After the shared files are fixed:** switch agents to `submit.sh`. This
   step removes the rebase-and-recheck loop (467 rebases in one session). It's
   also the step with every risk in the table above.

Part of the load is self-inflicted. Many of the 161 pushes a day exist to
edit a shared ledger file or raise a budget line. Fixing those cuts the load
the pipeline has to carry.

## Other opportunities, largest first

### 1. Replace polling with blocking waits

**Cost:** 49 agent-hours in one session, and about 400 million tokens.

Agents start a job, then loop on `sleep 10` for about 100 seconds, read the
log, and loop again. Each loop is a full reply. The pattern comes from two
rules: a foreground command is cut off at 10 minutes, and briefs tell agents
not to block for long.

- Tell agents to start any job over 2 minutes with `run_in_background` and
  then stop. The harness wakes the agent when the job exits. This costs no
  tokens while it waits.
- Add `remote.sh run --notify`, which prints one line when the job ends:
  exit status, seconds, and the last error lines. Agents then never need to
  read a log to learn that a job finished.
- Put the rule in `AGENTS.md`: don't loop on `sleep`.

### 2. Start agents smaller and keep them shorter

**Cost:** an average of 220,000 tokens on every reply.

- **The workspace `CLAUDE.md` is 94 KB.** Most of it lists reference
  repositories that Coax and Everglade work never uses. Every agent loads it
  on every reply. Keep the routing rules and move the catalogue to a file
  that agents read when they need it.
- **`INVARIANTS.md` is 491 KB**, and `AGENTS.md` sends agents to it before
  they change anything it covers. Split it by area so an agent reads only its
  own part.
- **Agents run too long.** 88 compactions across 107 agents means many
  outlived their context more than once. Give each agent one issue. When an
  issue has phases, start a new agent for each phase with a short handoff.
- **Every subagent ran on Opus 5.5.** Rebases, budget lines, ledger rows and
  issue filing don't need it. Send that work to a smaller model.

### 3. Shared files that every agent edits

**Cost:** most of the 467 rebases, and most rejections after cutover.

Commits that touched each file in the last 7 days, out of 1,819:

| File | Commits | Kind |
| --- | --- | --- |
| `INVARIANTS.md` | 330 | Append-only ledger |
| `apps/openagents-web/fivelab.coax` | 121 | One large unit |
| `apps/openagents-web/wasm-budgets.txt` | 120 | One line per module |
| `aethyr/rules/srd/battle.coax` | 117 | One large unit |
| `coax/web/room/room-everglade.coax` | 113 | One large unit |
| `docs/dnd/srd-compliance-ledger.md` | 108 | Append-only ledger |
| `bitcoin/book/chapters.json`, `SUMMARY.md`, `PLAN.md`, `examples.json` | 85 to 90 each | Indexes |
| `NEEDS_OWNER.md` | 79 | Append-only ledger |
| `apps/openagents-web/realms.json` | 54 | Registry |

Generated files conflict too: `fivemmo.wgsl` and `gpu.bindings` four times
each in one session, and a binary asset pack once.

- **Ledgers and registries:** one file per entry in a directory, and a tool
  that assembles the full document. Two agents adding entries then never
  touch the same file.
- **Generated files:** the integrator regenerates shaders, bindings and packs
  after it stacks a branch. A conflict in a generated file isn't a reason to
  send a branch back.
- **Large units:** see the next item.

### 4. Budgets and compile limits that only ever go up

**Cost:** at least three failed release runs on 11-08 and 11-09, and 103
`E0400` errors in one session.

- `wasm-budgets.txt` changed in 120 commits in a week. When a module passes
  its budget, an agent raises the line and adds a reason. The budget stops
  releases, but it doesn't hold sizes down.
- Replace the per-module line with two checks. First, flag a commit that
  grows a module by more than a set percentage, using the per-commit history
  `after.sh` already writes. Second, keep one hard ceiling per module, set by
  what a phone can load and changed only by the owner.
- `fivelab.coax` is 4,533 lines and took 121 commits in a week. Its full
  test takes 988 seconds on a remote host. #13094 brought it under 75% of
  its limits. Keep it there: warn at 75% in `check.sh`, where the design
  says 90%, and split Experimental's features into their own units so a
  branch check covers less.
- Publish the compiler with the check-reuse fix to the Mac. The coordinator
  measured that 17 checks that always re-run take 180 of each 223-second
  rebuild.

### 5. Build the release once, on the remote hosts

**Cost:** 30 to 40 minutes a release on 11-08, one Mac build slot held for
all of it, and a laptop too slow to use.

- A release compiled about 56 modules on the Mac at a load of 110. The owner
  asked why the computer was laggy, and had to reboot it on 11-07.
- `after.sh` already builds every web module at the commit that becomes
  `main-green`, to check sizes. Keep those files. `release.sh` then uploads
  the files that passed, and the Mac runs only the checks that need its GPU
  and credentials.
- Put every new `main-green` on staging automatically. The owner asked "what's
  the URL" or "open it so I can test" at least eight times in this session.
  With a staging train the answer is always the same link. Production stays
  on the owner's word.

### 6. A preview link for each branch

**Cost:** the owner finds most defects by hand, one message at a time.

Most of the owner's messages in this session report something seen in play:
a collider that doesn't match its mesh, grass that pops in, a health bar
drawn through a hill, a bowstring through an arm. Each one costs a message,
an agent, a build and a local dev server before the owner can look again.

- Build a preview for each queued branch that touches a world, on the remote
  hosts, and post its link to the issue. The owner tests in real Chrome
  without asking the coordinator to start a server.
- Keep building the engine checks filed on 11-09 (#13072 to #13078). They
  turn whole classes of these defects into failing tests: overlap, visibility,
  one path for every action, HUD layout.

### 7. A status page in place of status questions

**Cost:** at least eight "what's the status" questions in 57 hours, each
answered by the coordinator with a 229,000-token reply.

- `integrate.sh status` already prints the queue, the last rounds and the red
  list. Add the live agents (issue, worktree, last commit, what each waits
  on), the hosts and the last release, and publish it as one page.
- The same page stops agents from pinging the coordinator. On 11-08 a release
  agent's "still waiting" pings drew about 30 replies before the owner
  stopped it.

### 8. Let agents find each other's work

**Cost:** duplicated modules and waits on unpushed code.

- Two agents built lead pursuit and circling at once, in
  `aethyr/ai/pursuit.coax` and `aethyr/ai/steer.coax`. One was dropped.
- One agent couldn't run a check because it needed another agent's
  uncommitted edits to seven files.
- One agent couldn't message another because it knew only its topic.

Add `submit.sh --draft`. It pushes the branch to
`refs/heads/wip/<agent>/<topic>` without queueing it. Other agents can fetch
it and build on it, and the status page lists every draft with the new files
it adds. An agent checks that list before it creates a module.

## Suggested order

| Step | Change | Removes |
| --- | --- | --- |
| 1 | No `sleep` loops: background jobs and `remote.sh run --notify` | About 49 agent-hours and 400 million tokens per 57-hour session |
| 2 | Baseline rule in `after.sh`; run `after.sh --watch`; set `main-green` | Releases blocked by `main`; agents starting from a broken base |
| 3 | Suite discovery follows the `#load` graph | The 227 false failures, and tests that run nowhere |
| 4 | Ledger bills host uptime | A cap that stops work at a third of its figure |
| 5 | Split `INVARIANTS.md` and the other ledgers into one file per entry | Most rebases, and most rejections after cutover |
| 6 | Trim the workspace `CLAUDE.md`; one issue per agent | Tokens on every reply |
| 7 | Growth check and hard ceilings in place of budget lines | Failed releases over budget lines; 120 commits a week |
| 8 | Integrator identity, hooks and heartbeat; then switch agents to `submit.sh` | The rebase-and-recheck loop |
| 9 | Release from the files `after.sh` built; staging train | 30-minute Mac builds; "what's the URL" |
| 10 | Status page, drafts and branch previews | Status questions, duplicate work, hand-testing round trips |

Steps 1 to 4 are each under a day and need no change in how agents land
work.

## What this audit didn't check

- I read one coordinator session in depth. For the other 40 sessions I took
  counts only.
- I didn't read the Codex threads, so Codex agents' waiting isn't measured.
- I didn't reproduce the 227 failing suites or read `tool.sh suites`' source.
  The answer to question 2 rests on the description in #13102.
- "Stopped, waiting" covers both useful waits, such as a background build,
  and idle ones, such as waiting for the coordinator. The transcripts don't
  separate them.
- I didn't measure current release times. The 30 to 40 minutes is the
  coordinator's own figure from the evening of 11-08.
- Token counts aren't converted to dollars.
