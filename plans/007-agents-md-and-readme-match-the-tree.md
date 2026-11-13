# Plan 007: AGENTS.md, the README and the transcript index describe the tree as it is

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report. Do not improvise. When done, update the status row for this plan
> in `plans/README.md`, unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat 76084e17a9..HEAD -- AGENTS.md README.md docs/transcripts/README.md`
> These files change often. Re-read each section you are about to edit. If
> a fact this plan asks you to add is already there, skip that edit.

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: docs
- **Planned at**: commit `76084e17a9`, 2026-11-09

## Why this matters

Every agent that works in this repository reads the root `AGENTS.md` first
and treats it as the contract. Its table of what is in the repository
leaves out eight tracked areas, which together received over 300 commits
in two weeks. An agent working in one of them finds no stated purpose and
no check command. The README has one wrong path and tells a newcomer to
start with a command that takes over 15 minutes. The transcript index says
the archive ends at episode 290, and episodes 291 to 293 are in the tree.

## Current state

### AGENTS.md

The table is under `## What is here`, `AGENTS.md:12-25`. Its rows are:
`coax/`, `aethyr/`, `verse/`, `psionic/`, `apps/openagents-ios/`,
`apps/openagents-web/`, `apps/openagents-cli/`, `os/`, `proto/`,
`docs/protocol/`, a combined docs row, and `nostr/`.

Tracked areas with no row, and what each is (verified against the tree):

| Path | Tracked files | What it is |
| --- | --- | --- |
| `bitcoin/` | 501 | *Bitcoin From Scratch*, the book that builds Bitcoin and Lightning in Coax, and its examples. The code it teaches is under `coax/lib/crypto/`, `coax/lib/bitcoin/` and `coax/lib/lightning/`. Check: `python3 psionic/tests/book.py --book bitcoin`. |
| `coder/` | 143 | Coder, the coding agent: `book/` (*Coder: Coding Agents From Scratch*), `plugins/`, `tests/` and `tui/`. Check: `python3 psionic/tests/book.py --book coder`. |
| `everglade/` | 34 | Everglade, the MMORPG on the D&D 5E SRD: `book/`, `docs/` (the Housewright, Golemancy and mounts specs), `world/`, `hud/` and `features.coax`. Most of its game code is still in `apps/openagents-web/` and `verse/`. Check: `coax/web/tool.sh everglade-parity`. |
| `apps/openagents-desktop/` | 88 | The macOS desktop app in Coax, with `build.sh` and `check.sh`. Design: `docs/desktop/coax-desktop.md`. |
| `apps/aethyrforge-desktop/` | 113 | Metaverse Engine desktop (experimental): the native realm builder. It has its own `README.md`. |
| `apps/shared/` | 50 | Coax shared between apps: dictation (`dictation*.coax`) and the Zeron UI library (`zeron/`). |
| `tools/` | 133 | `coax-nostr/` (guard, fuzz and audit harnesses for the Nostr and cryptography libraries) and `coax-unicode/tables.py`. Not to be confused with `verse/tools/`. |
| `bench/` | 2 | Recorded voice benchmark results (`bench/voice/`). |

### README.md

`README.md:84-86`, in the bullet about `verse/`:

```text
  its shader math (`light.coax`). Start opens the Grid (`grid.coax`), where
  you walk (`player.coax`) as an animated character among patrolling robots
  (`cast.coax`). A thin C and Swift layer owns storage and
```

`verse/player.coax` does not exist. The file is `aethyr/move/player.coax`.

`README.md:188-189`, under `## Try it`:

```sh
coax/build.sh test                              # build coax0 and run its tests
```

`AGENTS.md:101` describes that same command as taking "over 15 minutes: use
parts", and `AGENTS.md` also states that one command takes at most 15
minutes. Plain `coax/build.sh` installs the published compiler in seconds.

### docs/transcripts/README.md

`docs/transcripts/README.md:3-4`:

```text
This guide covers the committed transcript archive as of 2026-10-30:
**290 numbered episode files covering every number from 001 through 290**. ...
```

The short table under `## Archive coverage and source limits` ends at line
55 with the row for 290:

```text
| [290](290.md) | **Enter the Verse.** A 34:22 live talk at a Pleb Lab-sponsored event: ... |
```

`docs/transcripts/291.md`, `292.md` and `293.md` exist and have no row.
There is also `docs/transcripts/293-show-notes.md`, which is a production
note and not an episode.

What each new file is:

- `291.md`: "Episode 291: Accelerate", dated November 4, 2026. A short
  script with a link to the post on X in its third line.
- `292.md`: "Episode 292: The Penultimate Frontier". A script transcribed
  from a handwritten page dated November 3, 2026, to be read on camera.
- `293.md`: "Episode 293: Everglade". A 4:04 edited talk. Its first
  paragraph is a summary.

Conventions:

- Write docs in plain, active, present-tense sentences. Address the reader
  as "you". No metaphors.
- Do not rewrite text you were not asked to change.

## Commands you will need

| Purpose | Command | Expected on success |
| --- | --- | --- |
| Paths in a table exist | see step 4 | no output |
| Markdown links resolve | see step 4 | no output |

Nothing here needs a compiler or the network.

## Scope

**In scope**:

- `AGENTS.md` (the `## What is here` table only)
- `README.md` (the two places named above only)
- `docs/transcripts/README.md` (the opening sentence, the coverage table,
  and the complete episode index)
- `plans/README.md` (status row only)

**Out of scope**:

- Every other section of `AGENTS.md`, including its rules and commands.
- The README's Books and "What we're building" sections, beyond the one
  path.
- The transcripts themselves.
- `docs/transcripts/293-show-notes.md`.

## Git workflow

- Work in a worktree: `git worktree add ../openagents-plan-007 origin/main`.
  This repository forbids branches other than `main` and edits in the main
  checkout.
- One commit, for example:
  `Docs: AGENTS.md lists every tracked area, and the transcript index reaches 293`.
- Do not push unless the operator tells you to.

## Steps

### Step 1: Add the missing rows to AGENTS.md

In the table under `## What is here`, add one row for each of the eight
paths in "Current state", using the description given there. Put
`apps/openagents-desktop/`, `apps/aethyrforge-desktop/` and `apps/shared/`
directly after the `apps/openagents-cli/` row. Put `bitcoin/` directly after
`nostr/`. Put `coder/`, `everglade/`, `tools/` and `bench/` at the end.
Match the existing rows' format: the path in backticks, then one cell of
prose.

Before you write each row, confirm the facts: run `ls <path>` and check
that each file or directory the description names exists. Drop any that do
not.

**Verify**:
`for p in bitcoin coder everglade apps/openagents-desktop apps/aethyrforge-desktop apps/shared tools bench; do grep -q "^| \`$p/\`" AGENTS.md || echo "missing $p"; done`
prints nothing.

### Step 2: Fix the README

1. In the `verse/` bullet, change `` (`player.coax`) `` to
   `` (`aethyr/move/player.coax`) ``.
2. Under `## Try it`, replace the first command line with these two:

   ```sh
   coax/build.sh                                   # install the published compiler (seconds)
   coax/tests/run.sh --part 1/8                    # one eighth of the Coax suites (under 15 minutes)
   ```

   Keep the comment column aligned with the lines below it.

**Verify**: `grep -c 'coax/build.sh test' README.md` prints `0`, and
`grep -c 'aethyr/move/player.coax' README.md` prints `1`.

### Step 3: Bring the transcript index to 293

1. Change the opening sentence to say that the archive is as of 2026-11-09
   and holds 293 numbered episode files covering every number from 001
   through 293. Keep the rest of that paragraph.
2. In the table under `## Archive coverage and source limits`, add rows for
   291, 292 and 293 after the 290 row, in the same format: a linked number,
   then the title in bold and one or two sentences. Write each from the
   file's own text. For 293, shorten its first paragraph. For 291 and 292,
   say that each is a script and state its subject in one sentence. Use
   "plans", "proposes", "announces", "demonstrates" or "reports"
   deliberately, as the paragraph below the table explains.
3. Go to `## Complete episode index`. Find its last row and the heading of
   the group that contains it. Add rows for any of 291, 292 and 293 that
   are missing, in exactly that table's column format. If the table has a
   link column and a transcript gives no link, leave that cell as the other
   rows do when a link is absent. If the last group's heading names a
   number range that ends at 290, extend the range.
4. Search the file for other statements that the archive ends at 290:
   `grep -n '290' docs/transcripts/README.md`. Update each one that is a
   count or a range. Leave references to episode 290 itself.

**Verify**: `grep -c '(291.md)\|(292.md)\|(293.md)' docs/transcripts/README.md`
prints at least `3`, and
`grep -c 'from 001 through 290' docs/transcripts/README.md` prints `0`.

### Step 4: Check every path and link you touched

```bash
# every path in the AGENTS.md table's first column exists
sed -n '/^## What is here/,/^## /p' AGENTS.md | grep -o '^| `[^`]*`' | tr -d '|` ' | while read p; do [ -e "$p" ] || echo "no such path: $p"; done
# every relative link in the transcript index resolves
grep -o '](\([0-9a-z-]*\.md\))' docs/transcripts/README.md | tr -d '()]' | sort -u | while read f; do [ -e "docs/transcripts/$f" ] || echo "broken: $f"; done
```

**Verify**: both loops print nothing.

## Test plan

No tests. Step 4's two loops are the checks.

## Done criteria

All must hold:

- [ ] The loop in step 1's Verify prints nothing
- [ ] `grep -c 'coax/build.sh test' README.md` prints `0`
- [ ] `grep -c 'from 001 through 290' docs/transcripts/README.md` prints `0`
- [ ] Both loops in step 4 print nothing
- [ ] `git diff --name-only origin/main` lists only `AGENTS.md`, `README.md`,
      `docs/transcripts/README.md` and `plans/README.md`
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back if:

- A path in "Current state" no longer exists, or holds something different
  from its description. Report what you found; do not guess a description.
- `AGENTS.md` no longer has a `## What is here` table.
- The complete episode index in `docs/transcripts/README.md` has a
  structure you cannot extend by copying its last row, for example a
  generated block with a marker comment.
- You find you want to change a rule or a command in `AGENTS.md`. That file
  is the agents' contract; changes beyond the table need the owner.

## Maintenance notes

- Nothing checks that the `AGENTS.md` table covers the tree. A small check
  that compares `git ls-files | cut -d/ -f1 | sort -u` and `ls apps/` with
  the table's first column would keep it current. It would be a new tool,
  which this repository requires to be written in Coax.
- The transcript index needs a row whenever an episode file is added.
- Two more doc problems were found and left out of this plan:
  `docs/Oct30MVP.md:115` and `:162` describe `coax/web/project/pylons.ts`,
  which was deleted on 2026-11-01, and the dated status files at the root of
  `docs/` (`issue4828.md`, `Oct30MVP.md`, `Oct30-demo-runbook.md`) have no
  note that they are historical.
