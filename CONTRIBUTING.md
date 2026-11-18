# Contributing to OpenAgents

You can propose a change to this repository with a pull request from your
own fork. This page covers how to set up the fork, what an engine change
must include, the one check you run before you open the pull request, and
what happens after you open it.

Agents on the OpenAgents team follow [AGENTS.md](AGENTS.md) instead: they
push to `main` from their own worktrees. This page is for everyone else,
people and agents alike.

## Before you start

You need:

- A GitHub account and the [`gh` command line](https://cli.github.com/),
  logged in with `gh auth login`.
- `git`, `python3` and a C compiler: Clang on macOS (from the Xcode command
  line tools, `xcode-select --install`), or GCC on Linux.

You don't need any OpenAgents key, login or server. Nothing in the check
below reads a secret or reaches a private host.

## Fork and branch

1. Fork the repository and clone your fork:

   ```sh
   gh repo fork OpenAgentsInc/openagents --clone
   cd openagents
   ```

   `gh` names your fork `origin` and this repository `upstream`.

2. Make a branch for your change from the newest `main`:

   ```sh
   git fetch upstream main
   git switch -c water-reflections upstream/main
   ```

   Name the branch for the feature. Keep one change in one branch.

3. Commit your work and push the branch to your fork:

   ```sh
   git push -u origin water-reflections
   ```

Your commits need no `Agent:` line. That line is for team agents that push
to `main`; the pre-push check that asks for it doesn't run on your fork.
Write commit messages in plain, present-tense English that say what the
change does.

## What an engine change must include

The OpenAgents Metaverse Engine lives in [`metaverse-engine/`](metaverse-engine/README.md).
It is written in [Coax](coax/start-here.md). A new engine primitive, the
smallest piece of the engine that any world can use, has three parts:

1. **A module.** One `.coax` file in the engine folder it belongs to, such
   as `metaverse-engine/physics/` or `metaverse-engine/render/`. It loads
   only the engine and `coax/`, never `metaverse/`, `apps/`, `everglade/` or
   `coax/web/` ([layers](metaverse-engine/README.md#layers)). It works for
   any world, not only the one you are building.
2. **Checks.** `#test` or `#forall` checks for what the module does,
   written before the code. Load them from the module with
   `#load "FILE" #test;`, or keep them in the module. A file that decides
   money, admission or policy is `#authority;`, and `coax/bin/coax mutate FILE`
   leaves no surviving mutant, except one marked equivalent with a reason.
3. **A book chapter.** A chapter, or a section of one, in the engine's
   book, [`metaverse-engine/book/`](metaverse-engine/book/README.md). Add or
   update its row in `chapters.json`, its line in `SUMMARY.md` and its row in
   the book's README. Credit yourself in the chapter with `contributed_by`
   ([Contributed chapters](metaverse-engine/book/README.md#contributed-chapters)).

A new engine module also needs a user: a product file that loads it in the
same change, or a line in [`metaverse-engine/consumers.txt`](metaverse-engine/consumers.txt)
that says why it is kept. Before you write something new, look for the
shared module that already does it in [docs/reuse.md](docs/reuse.md), and
extend that one instead.

## Run the check

Run one command from the repository root before you open the pull request:

```sh
scripts/check.sh
```

It installs this tree's Coax compiler (`coax/build.sh`: a published build
when there is one, else a build from source, which takes longer), then runs
these checks against the point where your branch left `upstream/main`:

- `tools-check`, `everglade-parity`, `wire-check`, `layer-check` and
  `agent-reference --check`: the repository-wide rules.
- The shader check when you change Coax or a shader, and the changelog
  page check when you change the changelog.
- The engine book's chapter registry when you change the engine book.
- `coax test` of every suite your change touches, and for `#authority`
  suites, the mutants of the procedures you changed.

The last line says whether every check passed. Copy the output into your
pull request. To see the commands without running them, run
`scripts/check.sh --list`. To compare with another base, run
`scripts/check.sh --base REF`.

The check leaves out the checks that need the team's machines or extra
tools: the API suite (it needs Postgres), the page driver check (it needs
headless Chrome) and the state-keep test (it needs Bun). The team runs those
after the merge.

## Open the pull request

```sh
gh pr create --repo OpenAgentsInc/openagents
```

The description template asks for:

- What the change is for: what a world needs, and what it can do now.
- The book chapter it adds or changes.
- The output of `scripts/check.sh`.
- Your OpenAgents account, if you want the credit to name it.

A pull request is public, in your GitHub name.

## Review

An OpenAgents review agent reads your pull request within one working day
and asks for changes or passes it to a maintainer. A maintainer, a person on
the OpenAgents team, approves and merges it. A pull request may be changed
or declined. [Review and safety rules](docs/contributing/review.md) says
what the reviewer looks at and how fork code is kept away from machines
that hold secrets.

## After the merge

- The commit on `main` keeps you as its author.
- Your chapter's credit line shows in the engine book.
- A maintainer opens an issue for GameBuilder to move its engine pin to
  your commit. Your change reaches GameBuilder in the release after that.

## Licence

This repository is under the [Apache License, Version 2.0](LICENSE). By
opening a pull request, you agree that your contribution is licensed under
the same terms (section 5 of the licence). Don't contribute code you don't
have the right to license that way, and don't include copied code from
projects under other licences.
