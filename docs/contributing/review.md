# Review and safety rules for pull requests from forks

This page says who reviews a pull request from outside the OpenAgents team,
how soon, and how its code is kept away from machines that hold secrets
(GitHub #13664). [CONTRIBUTING.md](../../CONTRIBUTING.md) is the
contributor's side; this page is the reviewer's. Agents on the team follow
it too ([AGENTS.md](../../AGENTS.md), "Pull requests from forks").

## Who reviews, and how soon

1. **A review agent reads the pull request within one working day**
   (Monday to Friday, US Central time) of when it is opened or last
   updated. It reads the diff, checks it against
   [what an engine change must include](../../CONTRIBUTING.md#what-an-engine-change-must-include),
   runs it in a sandbox as described below, and posts one review: it asks
   for changes, or it says the pull request is ready for a maintainer.
2. **A maintainer merges.** A maintainer is a person on the OpenAgents
   team. Only a maintainer approves and merges an outside pull request. An
   agent never merges one.
3. A pull request that waits on its contributor for 30 days after a
   request for changes may be closed, with a comment that says so. The
   contributor can reopen it.

## The hard rule

**Code from a fork is never built or run on a machine that holds secrets, a
production login or a wallet.**

That covers every team machine: the owner's Mac, the build hosts, the
release hosts, CoderOS and any machine with a `gh`, `gcloud` or cloud
login, an SSH key to a team host, a `.secrets/` folder, keys under
`~/.openagents/`, or a wallet's seed words in its Keychain. Until the pull
request is merged, its code is untrusted, like any downloaded code
([AGENTS.md](../../AGENTS.md), "Downloaded and cloned code is untrusted").

"Run" includes more than running a program. In this repository these also
run the branch's code:

- `coax/build.sh`, `scripts/check.sh` and `coax/web/tool.sh`, which build and
  run Coax tools and C from the tree.
- `coax test`, `coax mutate` and `coax drive`.
- `git checkout`, `git switch`, `gh pr checkout`, `git merge` or
  `git commit` in a clone whose `core.hooksPath` is `coax/remote/hooks`.
  Git then runs the hooks in the branch you check out, and a fork can change
  them. This Mac's clone has that setting, so every worktree made from it
  has it too.

## How the review agent reviews

1. **Read the diff first, without checking anything out:**

   ```sh
   gh pr diff N --repo OpenAgentsInc/openagents
   gh pr view N --repo OpenAgentsInc/openagents --json files,author,title,body
   ```

2. **Look for code that runs outside its module.** Changes to
   `coax/remote/hooks/`, `coax/build.sh`, `coax/web/tool.sh`, any `.sh`
   file, `coax/platform/*.c`, new `#foreign` procedures, network calls,
   binary files and files outside the change the description names. Any of
   these is a reason to ask what it is for before running anything.
3. **Run it only in a sandbox with no credentials.** A machine or container
   made for this review and thrown away after it, with no team login, no
   SSH key, no secrets, no wallet and no mounted home folder. In the
   sandbox, clone the repository, fetch the pull request's head
   (`git fetch origin pull/N/head:pr-N`), switch to it, and run
   `scripts/check.sh`. Compare its output with the output the contributor
   pasted.
4. **Post one review** with what passed, what failed and what to change.
   Name files and lines. Don't post the sandbox's paths or anything about
   team machines.

If no sandbox is available, the review agent reviews the diff only, says in
its review that it did not run the code, and leaves running it to the
maintainer's sandbox.

## No check gets secrets

No check on a pull request gets repository secrets. As of 2026-11-17 the
repository has no `.github/workflows/` folder, so GitHub runs nothing on a
pull request, and [AGENTS.md](../../AGENTS.md) forbids GitHub Actions. No
script in the repository fetches pull request branches (`refs/pull/*` or
`gh pr checkout`). If a workflow is ever added, it never uses
`pull_request_target` and never passes a secret to a run on fork code.

## Merging

A maintainer merges with **Squash and merge**. GitHub then writes one commit
on `main` whose author is the contributor. Keep it that way:

- Don't copy the change into a commit of your own, cherry-pick it under your
  name or push it to `main` by hand.
- The commit needs no `Agent:` line. The pre-push check that asks for one
  runs only on pushes from team clones.
- Keep the pull request number, `(#N)`, at the end of the subject, as
  GitHub writes it.

Once a change is on `main`, the after-landing check and the build hosts run
it like any other commit. That is why the review comes before the merge.

## After the merge

1. If the change adds or changes an engine book chapter, its
   `contributed_by` entry and credit line are in the pull request, or the
   maintainer adds them in a follow-up commit
   ([Contributed chapters](../../metaverse-engine/book/README.md#contributed-chapters)).
2. If the change is to the engine, the maintainer opens an issue at
   OpenAgentsInc/gamebuilder asking to move its engine pin. Print the issue
   first, then open it:

   ```sh
   coax/web/tool.sh metaverse-engine/tools/pin-bump-issue.coax --pr N --dry-run
   coax/web/tool.sh metaverse-engine/tools/pin-bump-issue.coax --pr N
   ```

   The issue names the merge commit, the contributor, the chapters and the
   engine files the change touches. Opening it needs a `gh` login that can
   write issues in that repository.
