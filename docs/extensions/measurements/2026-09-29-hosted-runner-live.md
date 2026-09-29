# Hosted eval runner: live runs of the starter test sets, 2026-09-29

What we ran: the hosted eval runner
([#9935](https://github.com/OpenAgentsInc/openagents/issues/9935),
`crates/eval-runner`) deployed on `coderos-4080`, answering on
`wss://relay.openagents.com` as
`a7cff3ee1ff0209f971b9f24673db310ab858899c9d9a99b640e6cb29b1753f0`.
Each run was one signed `25920` from a fresh trainer key on a MacBook,
sent with `crates/eval-runner/examples/trainer.rs`, which builds requests
with the same `nostr::eval_ext::hosted` functions the phone uses. Every
run used 3 runs per arm and both arms, with live Coder (`coder -p`, built
from the deployed commit) on the Gemini Flash lane of the AI Gateway and
live Jev, inside `bwrap`, with the hosted grant of read and sandbox write.

The starter test sets are the `evals/` of `crates/plugin-repo-map`,
`crates/plugin-code-search`, and `crates/plugin-test-report`: six tests
each, four where the tool should help and two where it should stay out
of the way. The runner released them and the three tools as NIP-EXT
releases, all signed by its key:

| | Test set release | Tool release |
| --- | --- | --- |
| Project map | `8e4ae48b9d14…` (`project-map-tests`) | `7af8bd08bbff…` |
| Code finder | `086f3b7392cd…` (`code-finder-tests`) | `8402fae41568…` |
| Test reader | `e42741c2e8eb…` (`test-reader-tests`) | `f958dc54d712…` |

## Results

One trainer ran each test set, then published it; a second trainer
checked it through the same runner and published the check.

| | With the tool | Without it | Verdict | Time per run, with / without | Check by another trainer |
| --- | --- | --- | --- | --- | --- |
| Project map | 5 of 6 | 2 of 6 | **Better** | 10.7 s / 24.9 s | 5 of 6 / 2 of 6, **Better**: confirms |
| Code finder | 4 of 6 | 2 of 6 | **Better** | 16.1 s / 22.4 s | 4 of 6 / 2 of 6, **Better**: confirms |
| Test reader | 5 of 6 | 2 of 6 | **Better** | not reported (inside the spread) | 5 of 6 / 2 of 6, **Better**: confirms |

Cost is unknown: Coder doesn't price gateway lanes. Each run of a test
set is 36 agent turns and took 29 to 58 seconds from the tap to the
result, with two test sets at once and four turns at a time in each. The
runner acknowledged a request in 2 to 5 seconds (it fetches the release
and its files and checks the suite reproduces byte for byte first).

Per test, runs passed of 3, with and without the tool (the checks):

| Test set | Test | Kind | With | Without |
| --- | --- | --- | --- | --- |
| Project map | `largest-file` | should fire | 3 | 0 |
| | `languages` | should fire | 3 | 0 |
| | `build-files` | should fire | 3 | 0 |
| | `where-tests` | should fire | 0 | 0 |
| | `greeting`, `explain-404` | should not fire | 3, 3 | 3, 3 |
| Code finder | `open-todos` | should fire | 3 | 0 |
| | `review-markers` | should fire | 3 | 0 |
| | `known-bugs`, `workarounds` | should fire | 0, 0 | 0, 0 |
| | `greeting`, `explain-idempotent` | should not fire | 3, 2 | 3, 3 |
| Test reader | `cargo-failure` | should fire | 3 | 0 |
| | `pytest-failure` | should fire | 3 | 0 |
| | `green-build` | should fire | 3 | 0 |
| | `ci-failures` | should fire | 0 | 0 |
| | `greeting`, `explain-flaky` | should not fire | 3, 3 | 3, 3 |

The published results and checks, all `3189` on the relay:

| | Result | Check |
| --- | --- | --- |
| Project map | `5fdfd3606e3b…` | `542d5571d89c…` |
| Code finder | `13c645307f0b…` | `419ffe52b430…` |
| Test reader | `608562a1e1ab…` | `2fa29081642f…` |

Each carries its trainer's signed request in `meta.ext_eval_request`, and
`nostr::eval_ext::verified_trainer` credited the trainer from it with no
request on the relay. The XP referee on the same host then published one
`eval-check` quest version per test set and signed nine awards: 50 XP to
each checker, 25 to each result's trainer, and 25 to the test set's
author (the runner's key).

## What this says

- Hosted runs work end to end from a trainer's signed request with no
  computer of their own, and a second trainer's check reproduces each
  verdict on the same runner.
- The tests grade what the tool found, in the run's trajectory, because a
  program turn replies with its run summary. Without the tool, Coder can't
  look at the files in a hosted run (the grant has no `exec`, so its shell
  is off), so it asked a question or said it couldn't tell; with it, Jev
  chose the tool's program and the guest's output held the answer. That is
  the whole of the change measured here.
- Where a should-fire test failed in both arms (`where-tests`,
  `known-bugs`, `workarounds`, `ci-failures`), Jev didn't choose the
  tool's program for that wording. Those are the tests to work on to make
  each tool more useful.
- The time notes come from the program turn answering in about a second;
  under `ext-eval-v2` they're notes, never the verdict.

## Found and fixed on the way

- **Tools need releases for credit.** The first round of checks confirmed
  all three results, and the referee refused them: an `eval-check` quest
  names a test set release and a tool release, and the catalog tools had
  none. The runner now releases each catalog tool when it starts and
  names a result's subject by that release; the second round earned XP.
- **A grader that missed a right answer.** One Project map run read
  **Worse**: its should-not-fire test `explain-404` looked for "not found"
  and missed "the server cannot find the requested resource", so the
  subject arm lost that test on two of three runs by chance. The pattern
  now accepts the usual phrasings, and the fixed test set is release
  `8e4ae48b9d14…`; the earlier release `4aa8599a38a1…` stays readable.
- **No Blossom on the production relay.** `relay.openagents.com` runs
  without media, so the runner keeps suite files in the public-read bucket
  `openagentsgemini-eval-blobs`, written by a service account that may
  only create objects there.
- **The quota held live.** A trainer's fourth run of the day was refused
  `over_quota` in 3.9 seconds, before anything ran.
