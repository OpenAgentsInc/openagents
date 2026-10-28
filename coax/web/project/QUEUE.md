# Working the project queue

How a person or agent takes work from a project's queue today, before the
`openagents` CLI exists. Every step leaves a signed Nostr event, so the
project page at https://openagents.com/project/coax shows it after the next
release. The tools run on the machine that holds the project key
(`~/work/.secrets/openagents-project-coax.key`); commands run from
`coax/web/project/`.

## The loop

1. **See what's ready.** `bun publish.ts list` prints the queue in order:
   ready tasks first (every blocker resolved), by rank, then waiting tasks.
   Take the first ready task that nobody is working on, unless the owner or
   the taskmaster named one for you.
2. **Claim it.** `bun publish.ts claim SLUG --by NAME`. NAME is how you
   appear on the page (`claude-queue-1`, `codex-2`, a person's handle). One
   task per worker at a time. A claim is a promise, not a lock: check
   `list` first so two workers don't take the same task.
3. **Read the task.** `bun publish.ts show SLUG` prints the body and the
   acceptance checks. The issue body is the contract: what to build, which
   files, the exact tests and vectors that must pass. Read `coax/nostr.md`
   for the design it belongs to.
4. **Do it.** Work on `main` in the openagents repo, in a fresh worktree if
   the checkout is busy. Follow the repo's AGENTS.md and INVARIANTS.md. Run
   every acceptance check named in the task, plus `coax test` for each Coax
   file you touched.
5. **Leave notes as you go.** `bun publish.ts note SLUG "text"` for anything
   the next worker or the owner should see: a decision, a surprise, a
   blocker. Keep notes short and factual.
6. **Finish.**
   - All acceptance checks pass: commit and push (scoped files,
     `--no-verify`, the commit message names the task alias), then
     `bun publish.ts done SLUG --commit HASH`. That marks the issue resolved
     and links the commit.
   - Blocked: `bun publish.ts note SLUG "blocked: why"` and
     `bun publish.ts release SLUG`. Name what would unblock it.
   - The task is wrong or too big: say so in a note, propose the split or the
     fix in the note, and release it. Only the taskmaster edits the work file
     (`apps/openagents-web/project/coax/work/nostr.json`) and republishes.
7. **Show it.** The taskmaster runs `coax/web/release.sh` after each change
   in state, so the page in production shows claims, notes and done tasks.

## Rules

- **Never renumber or reorder by hand in the work file.** Order is the queue
  order event; move with `bun publish.ts move SLUG --before SLUG`.
- **Never mark something done that didn't pass its acceptance checks.** If
  a check can't run here (a device, a key), say so in a note and leave it
  open.
- **No rewards or amounts on tasks.** We do this work ourselves.
- **Never print or commit the project key.**
- **Process improvements** go in a note on the task you were doing, starting
  with `process:`, and the taskmaster folds the good ones into this file.

## Roles

- **Worker:** an agent or person running the loop above on one task at a
  time.
- **Taskmaster:** keeps the queue honest. It reviews each finished task's
  commit against its acceptance checks, reopens what doesn't hold
  (`bun publish.ts status SLUG open` with a note), and adjusts order and
  scope. It releases the site so the page stays current, and folds process
  notes into this file.
