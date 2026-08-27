# Owned tasks: closed issues as graded work

Issue [#34](https://openagents.com/OpenAgentsInc/openagents/issues/34) asks the
effectiveness suite to draw on two sources: a bounded public subset, and "an
owned set drawn from this tracker's closed issues with known accepted
outcomes". The public half is
`bench/suites/swebench-verified-subset.suite.json`, resolved from Harbor's
registry. This directory is the owned half.

## Where the tasks come from

The forge records a closing reference on an issue as evidence, with the commit
that closed it. `bench/build-suites.mjs --issues <closed.json>` reads a
`openagents issue list --state closed --json` body and turns every issue that
carries one into a task pinned to that commit:

```json
{
  "id": "owned-issue-31",
  "pin": {
    "kind": "tracker-closed-issue",
    "repo": "OpenAgentsInc/openagents",
    "issue": 31,
    "acceptedCommit": "cf1861c9cb..."
  },
  "environmentAvailable": false,
  "rationale": "Adopt the proxy's reasoning and tool-call fidelity in the coder"
}
```

The accepted outcome is a fact the tracker holds, not a judgement this tooling
makes. That is the whole reason these are worth grading against: unlike a
synthesised task, somebody already decided what "done" looked like and shipped
it, and the diff is on record. An issue closed without a closing reference is
skipped by number rather than guessed at — it may be perfectly well closed, but
there is nothing to grade against.

At the time this suite was recorded, six of the tracker's fifty-eight closed
issues carried a closing-reference commit. All six landed in the retired
`packages/openagents-cli` package, and five touched a test file in the same
commit. The immutable commit pins preserve those historical task definitions
after the package's removal.

## What is not built yet

**No container exists that can grade any of these.** The pin is real and the
suite is real; the environment is not. Every owned task therefore carries
`environmentAvailable: false`, and `parseSuiteManifest` refuses to let a task
with no gradeable environment into a `score`-tier suite — so
`owned-closed-issues.suite.json` is `smoke`, and `coder-effectiveness-v1` holds
the twenty registry-backed public tasks and none of these.

The registry tasks carry `environmentAvailable: true` because the dataset ships
their environment and verifier, which is what makes a public subset cost a
manifest entry rather than a harness. The flag says a gradeable definition
exists; it does not say the task has been run here, or that it passed, or that
its image builds on any particular machine.

That refusal is the point rather than a limitation. A score suite that included
a task nobody could run would report those trials as missing, and a missing
trial reads as the coder failing rather than as an absent environment. The
suite would get quietly worse the day the environment broke, for a reason that
has nothing to do with the thing being measured.

## The verifier these tasks want

The construction is SWE-bench's, applied to our own history. For an issue whose
closing commit touches both source and test files:

1. Base the environment at the closing commit's **parent**, with the test-file
   half of the closing commit applied and the source half left out. The test
   then exists and fails.
2. The instruction is the issue body, which is what a human coder was given.
3. The verifier runs the touched test files. Pass means the agent made the
   recorded test pass; it does not mean the agent reproduced the recorded diff,
   and it should not.

Two things have to be true before the first of these can flip
`environmentAvailable`, and neither is cheap:

- **The image.** Each historical `packages/openagents-cli` task requires its
  pinned repository snapshot and a pnpm workspace install. Build time and image
  size determine whether the owned lane runs per release or per week.
- **The test half really failing at base.** A closing commit that only added a
  test for behaviour that already worked is not a graded task at all — it is a
  task that passes before the agent starts. Each candidate needs that checked
  by running the test at the base commit, once, before it is admitted.

Until both hold for a task, it stays here: pinned, described, and out of every
published score.
