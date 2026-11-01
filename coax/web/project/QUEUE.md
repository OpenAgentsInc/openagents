# Working the project queue

How a person or agent takes work from a project's queue. You work the
whole loop with the `openagents` command line
([apps/openagents-cli](../../../apps/openagents-cli/README.md)), one Coax
program. Every step leaves a signed Nostr event, and the project page at
https://openagents.com/projects/coax reads its relay live: it shows the
event within seconds, with no release.

Build the CLI once in your checkout:

```sh
apps/openagents-cli/build.sh     # writes apps/openagents-cli/build/openagents
ln -sf "$PWD/apps/openagents-cli/build/openagents" ~/.local/bin/openagents
```

Commands run anywhere inside the openagents checkout. They work on the
`coax` project unless you pass `--project SLUG`. Reading needs no key.
Writing signs with the project key, which only the machine that works the
queue holds (`~/work/.secrets/openagents-project-SLUG.key`, or
`--key FILE`).

## The loop

1. **See what's ready.** `openagents project list` prints the queue in
   order: tasks in progress (`on:NAME`), then ready tasks (every blocker
   resolved) by rank, then waiting tasks, then done ones with their commit.
   Take the first `ready` task, unless the owner or the taskmaster named one
   for you. Every verb takes a slug or an alias (`#a7e7`). Add `--json` for
   a machine-readable answer.
2. **Claim it.** `openagents project claim SLUG --by NAME`. NAME is how you
   appear on the page (`claude-queue-1`, `codex-2`, a person's handle). One
   task per worker at a time. The claim fails when someone else holds the
   task (exit 5), so two workers don't take the same one.
3. **Read the task.** `openagents project show SLUG` prints the body and the
   acceptance checks, what it waits on, its plan and its notes. The issue
   body is the contract: what to build, which files, the exact tests and
   vectors that must pass. Read `coax/nostr.md` for the design it belongs
   to.
4. **Do it.** Work on `main` in the openagents repo, in a fresh worktree if
   the checkout is busy. Follow the repo's AGENTS.md and INVARIANTS.md. Run
   every acceptance check named in the task, plus `coax test` for each Coax
   file you touched.
   Other agents share the main checkout: never stash, reset or autostash
   their files (not even `git pull --rebase --autostash` to push). If the
   checkout is dirty in anything but your own files, commit and push from a
   worktree of `origin/main`. The CLI works from any worktree.
5. **Post your plan and keep it current.** Right after claiming, write the
   steps you'll take: `openagents project plan SLUG set "step" "step" ...`.
   As you work, `plan SLUG start N` marks the step you're on (one at a
   time), `plan SLUG check N` the ones done, `plan SLUG add "step"` what you
   find you need, `plan SLUG drop N` what turned out unneeded, and
   `plan SLUG file PATH` (or `-` for stdin) replaces it from a Markdown
   checklist. Update it at every step, not at the end: the page draws it
   under your task, and the owner reads it to see where you are.
   `openagents project plan SLUG` prints it.
6. **Leave notes as you go.** `openagents project note SLUG "text"` for
   anything the next worker or the owner should see: a decision, a
   surprise, a blocker. Keep notes short and factual. The same note twice
   within an hour is skipped.
7. **Finish.**
   - All acceptance checks pass: check every step of the plan, commit and
     push (scoped files, `--no-verify`, the commit message names the task
     alias), then `openagents project done SLUG --commit HASH`. That links
     the commit and marks the issue resolved. HASH is the full 40-digit
     hash (`git rev-parse --verify HEAD`). `done` refuses a short hash, and
     a hash that is not an ancestor of `origin/main` (it fetches once
     before it refuses), so push first. Correct a mistaken completion
     openly: `status SLUG open`, a note that explains the error, then
     `done` with the right hash.
   - Blocked: `openagents project note SLUG "blocked: why"` and
     `openagents project release SLUG`. Name what would unblock it.
   - The task is wrong or too big: say so in a note, propose the split or
     the fix in the note, and release it. Only the taskmaster edits the
     work files (`apps/openagents-web/projects/coax/work/*.json`) and
     republishes.
8. **Show it.** Nothing to run for the page: it reads the relay live, so
   your claim shows as IN PROGRESS with your name, your plan under it, the
   finished task under JUST FINISHED with its commit, and all of them in
   ACTIVITY, within seconds. Never run `coax/web/release.sh` just to show a
   claim, a plan, a note or a done. For the Markdown agents read
   (`/projects/<slug>/index.md`, `/projects/index.md`, `/llms-full.txt`),
   run `coax/web/project/publish-data.sh SLUG` (seconds; it writes only
   those files and checks production answers them). The baked first paint
   catches up with the next code release, whoever runs it.
9. **Before pushing a change to a page** (an app's `.coax`, `.wgsl`, face,
   assets, `coax/web/host.js` or anything else a page is built from), run
   `coax/web/golden-check.sh --commit HASH` on your commit. It renders only
   the pages your commit can change and compares them with their goldens; a
   change that needs approving fails it, named against your commit. Approve
   your own visual change in the same commit (`goldens.ts --update` against a
   `release.sh --staging-only` run, then commit `coax/web/goldens/`); never
   approve someone else's to get a release through. A release holds back a
   page that fails its golden (production keeps its last good version), ships
   everything else and names the likeliest breaking commit.

Every write prints each event it published (`published kind 1111: ...`).
When a command fails partway, it names what it sent before the error, and
running it again is safe: a claim, plan, note or status that already holds
is not sent twice.

## Publishing the work files

The taskmaster turns the work files into issues:

```sh
openagents project check [--project SLUG]              # the work files' rules, no relay
openagents project publish --dry-run [--project SLUG]  # what would change
openagents project publish [--project SLUG]
```

`publish` sends the repository and project announcements when they differ,
a new issue for each new work item, a revision when an item's fields or
body changed, a status when its status changed, and then the task order
(kind 30623) when it differs. A second run prints `nothing changed`.

## Rules

- **Never renumber or reorder by hand in the work file.** Order is the queue
  order event; move with `openagents project move SLUG --before SLUG` (or
  `--after SLUG`), and give every unfinished task fresh short ranks with
  `openagents project respread` (`--seed` for the work files' order,
  `--order FILE` for a list of slugs).
- **Queue ranks belong to unfinished tasks.** Initiative and milestone
  issues stay in deterministic creation order. Resolved and closed tasks
  keep their status and history, but publishing leaves them and hierarchy
  issues out of the task-order event. Active task ranks stay intact unless
  an overlong rank triggers the automatic rebalance. Once a finished task
  has left the order, reopening it appends it unranked at the end. Both
  move anchors must be unfinished tasks, and `respread --order` files must
  name tasks. The CLI refuses an order over the relay's tag limit rather
  than dropping active tasks.
- **Never mark something done that didn't pass its acceptance checks.** If
  a check can't run here (a device, a key), say so in a note and leave it
  open.
- **No rewards or amounts on tasks.** We do this work ourselves.
- **Never print or commit the project key.**
- **Weakening a check with its code needs an independent review.** A commit
  that removes checks (`#assert`, `#test`, `#forall`, `#authority`, test
  asserts), adds a `mutant: equivalent` mark, moves checks out of an
  `#authority` file into one without it, drops a gate from `tests/run.sh` or
  edits an `INVARIANTS.md`, and changes code in the same commit, is reviewed
  by a separate review agent that signs with its own Nostr key, never the
  project key (`coax/reviews/authors.json`):
  `openagents project review COMMIT --key ~/work/.secrets/openagents-coax-reviewer.key`
  writes `coax/reviews/COMMIT.json`; commit and push it. `release.sh` runs
  `openagents project review report` over the commits since the last live release, outside
  the compiler, puts the result in `releases/<time>/REVIEW` and notes each
  unreviewed commit on its task. It is a report, never a stop (owner,
  2026-10-31): releases are not blocked by it.
- **Process improvements** go in a note on the task you were doing, starting
  with `process:`, and the taskmaster folds the good ones into this file.

## Roles

- **Worker:** an agent or person running the loop above on one task at a
  time.
- **Taskmaster:** keeps the queue honest. It reviews each finished task's
  commit against its acceptance checks, reopens what doesn't hold
  (`openagents project status SLUG open` with a note), and adjusts order and
  scope. It runs `coax/web/release.sh` when code changed (data needs no
  release), and folds process notes into this file. It owns the full mutation
  check: after a release prints a `mutation` line, it runs
  `COAX_MUTATION_FULL=1 coax/build.sh` and turns each survivor in
  `releases/<time>/MUTATION` into a task (`coax/RELEASE.md`).
