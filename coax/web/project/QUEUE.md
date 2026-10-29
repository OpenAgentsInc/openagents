# Working the project queue

How a person or agent takes work from a project's queue today, before the
`openagents` CLI exists. Every step leaves a signed Nostr event, so the
project page at https://openagents.com/projects/coax shows it after the next
release. The tools run on the machine that holds the project key
(`~/work/.secrets/openagents-project-coax.key`); commands run from
`coax/web/project/`.

## The loop

1. **See what's ready.** `bun publish.ts list` prints the queue in order:
   tasks in progress (`on:NAME`), then ready tasks (every blocker resolved)
   by rank, then waiting tasks, then done ones with their commit. Take the
   first `ready` task, unless the owner or the taskmaster named one for you.
   Every verb takes a slug or an alias (`#a7e7`).
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
   Other agents share the main checkout: never stash, reset or autostash
   their files (not even `git pull --rebase --autostash` to push). If the
   checkout is dirty in anything but your own files, commit and push from a
   worktree of `origin/main`, and run `publish.ts` from one too when your
   checkout lags the tools (copy `state/<slug>.json` and link
   `node_modules` into it).
5. **Post your plan and keep it current.** Right after claiming, write the
   steps you'll take: `bun publish.ts plan SLUG set "step" "step" ...`. As you
   work, `plan SLUG start N` marks the step you're on (one at a time),
   `plan SLUG check N` the ones done, `plan SLUG add "step"` what you find you
   need, `plan SLUG drop N` what turned out unneeded. Update it at every step,
   not at the end: the page draws it under your task, and the owner reads it
   to see where you are. `plan SLUG` prints it.
6. **Leave notes as you go.** `bun publish.ts note SLUG "text"` for anything
   the next worker or the owner should see: a decision, a surprise, a
   blocker. Keep notes short and factual.
7. **Finish.**
   - All acceptance checks pass: check every step of the plan, commit and push (scoped files,
     `--no-verify`, the commit message names the task alias), then
     `bun publish.ts done SLUG --commit HASH`. That marks the issue resolved
     and links the commit.
     Read HASH from `git rev-parse --verify HEAD` in the source worktree and
     check `git merge-base --is-ancestor HASH origin/main` after pushing.
     Never expand a short hash by hand. The publisher currently checks only
     hash syntax, so a nonexistent or unpushed hash can otherwise become a
     signed completion link. Correct a mistaken completion openly: reopen,
     explain the metadata error and resolve with the verified pushed hash.
   - Blocked: `bun publish.ts note SLUG "blocked: why"` and
     `bun publish.ts release SLUG`. Name what would unblock it.
   - The task is wrong or too big: say so in a note, propose the split or the
     fix in the note, and release it. Only the taskmaster edits the work file
     (`apps/openagents-web/projects/coax/work/nostr.json`) and republishes.
8. **Show it.** Run `coax/web/release.sh` from the repo root after each
   claim and each done (runs serialize, and the full staging and production
   walks have recently taken 15–20 minutes; it builds a clean worktree of
   `origin/main`, reads the relay afresh, deploys staging, smokes it,
   promotes and smokes production). The page shows the claim as IN PROGRESS
   with your name, the finished task under JUST FINISHED with its commit, and
   both in ACTIVITY. If a smoke fails, stop and tell the taskmaster: the
   release rolls itself back, but the page is now behind.

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
