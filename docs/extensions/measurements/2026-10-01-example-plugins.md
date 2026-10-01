# The example plugins on the hosted runner, 2026-10-01

What we ran: the test sets of the three
[example plugins](../../plugins/examples/README.md)
([#10086](https://github.com/OpenAgentsInc/openagents/issues/10086)) on the
hosted runner on `coderos-4080`, installed at `1437ede584` with
`deploy/eval-runner/install.sh` (agent
`sha256:71e9341daf436d6f78546905e739cedb53fb0a138f1fac002840ebdc959e2ece`),
answering on `wss://relay.openagents.com`. Each run was one signed `25920`
from a fresh trainer key made for it on a MacBook
(`crates/eval-runner/examples/trainer.rs`): 3 runs per arm, both arms,
live Coder on the Gemini Flash lane and live Jev, inside `bwrap`, with the
hosted grant of read and sandbox write. Coder's defaults release
`680720dd100f…` (Project map) was admitted in both arms, so each result is
marginal over Project map.

## Releases

The runner released each plugin and its test set under its key
(`a7cff3ee…`) with `eval-runner release`:

| Plugin | Plugin release (`3184`) | Test set release (`3184`) |
| --- | --- | --- |
| Explain this error | `667af97ebd6c466b69078f34060fb06562a1e3542a6f2e5e4e4101f753b0eab0` | `03a418dcb60731756f55d0e2a64ae82371d009e6f829937ac0177d7b33077ac2` (`explain-error-tests`) |
| Release notes | `378df8f45de57c4097bd477f7b915789ee21ab0405e6b395bce0560e0b89334f` | `d53dfd6fff1857689146a23d05519963dc5c0f984798f145fe82eaab9f452cb5` (`release-notes-tests`) |
| Dependency check | `15c8f23584b4d5cc5d860dc1c70435daca3bbfad8784199badeca7f57f708180` | `a6b800a63b4e463695a0da9bfff7194f21fe385994b6988f8199a7a1310a2b7c` (`dependency-check-tests`) |

## Results

| Plugin | With / without | Verdict | Time per run, with / without | Published result (`3189`) | Trainer |
| --- | --- | --- | --- | --- | --- |
| Explain this error | 7 of 7 / 2 of 7 | **Better** | 8.4 s / 40.9 s | `9abaf9a512512062fda2c8d10317e5a94291c4c8828264e43940bcff134070f9` | `d23f91fc…` |
| Release notes | 6 of 6 / 2 of 6 | **Better** | 8.4 s / 45.4 s | `6bedfaa6c78c04900f333b36aa2b3087e782fe9b1ed58d2140f730d0281bb0af` | `0aabc492…` |
| Dependency check | 6 of 6 / 2 of 6 | **Better** | 9.0 s / 20.8 s | `49357306398ef037f72bfa7c8a98a9295a7fe0e7c59e5bf2c3c1373d2d604536` | `287008f0…` |

Cost is unknown: Coder doesn't price gateway lanes. Each published result
carries its trainer's signed request, and `trainer verify` read all three
back from the relay with the trainer verified from the inline request.

Per test, runs passed of 3, with and without the plugin:

| Plugin | Test | Kind | With | Without |
| --- | --- | --- | --- | --- |
| Explain this error | `python-keyerror`, `rust-mismatch`, `node-undefined`, `go-index`, `saved-log` | should help | 3, 3, 3, 3, 3 | 0, 0, 0, 0, 0 |
| | `segfault-concept`, `commit-message` | should stay out of the way | 3, 3 | 3, 3 |
| Release notes | `saved-range`, `latest-release`, `pasted-oneline`, `breaking-footer` | should help | 3, 3, 3, 3 | 0, 0, 0, 0 |
| | `changelog-question`, `commit-message` | should stay out of the way | 3, 3 | 3, 3 |
| Dependency check | `cargo-duplicates`, `npm-licenses`, `loose-ranges`, `python-requirements` | should help | 3, 3, 3, 2 | 0, 0, 0, 0 |
| | `caret-question`, `add-dependency` | should stay out of the way | 3, 3 | 3, 3 |

## What this says, and what it doesn't

- **Most of the delta is reach.** A hosted run has no shell, so without
  the plugin Coder can't read the project's files. Every test whose facts
  are only in the fixtures (the source an error points at, a saved log, a
  lockfile, a policy) fails without the plugin by construction. That is
  the change these plugins make in a hosted run, and the tests say so
  rather than hide it; a run on a computer with a shell would compare
  differently.
- **Two release-notes tests are strict about wording.** In
  `pasted-oneline` and `breaking-footer` the log is in the prompt, so Coder
  without the plugin could do the work, and it wrote reasonable notes. It
  failed because it paraphrased subjects ("The v1 export API endpoint has
  been dropped") where the graders want each line to keep its commit's
  subject and cite its hash, and in `breaking-footer` it cited no commit at
  all. Part of that delta is the graders' strictness, which matches what
  the plugin promises (cite every commit, keep the author's words), not a
  difference in understanding.
- **One selection miss.** In one of three `python-requirements` runs with
  the plugin, Jev didn't choose the workflow for "Is our requirements file
  pinned well enough to deploy from?", so the run looked at nothing. That
  wording is the test to work on.
- **Should-not-fire held.** No workflow ran on a general question or a
  commit message in either arm.
- **Receipts replay with the request.** Every `receipt` grader
  (`python-keyerror`, `saved-log`, `saved-range`) replayed its Wasm call
  exactly on the runner, rebuilding the input and the granted files from
  the trajectory's request, the path the new `request` and `read_named`
  binding fields take.
- **Faster is a note, not the verdict.** A workflow turn answers in about
  a second; under `ext-eval-v2` time never makes a result **Better**.
- **One trainer, one operator.** Three fresh keys from one machine sent
  the requests. No check by another trainer and no second test set exist
  yet, so none of the three is a candidate for Coder's defaults.

## Read-only runs on the owner's repositories

On scratch copies (`git archive HEAD`, or a `git log` written to a scratch
folder; the real checkouts were only read), with `openagents plugin run`:

- **Explain this error** on `probe` with one comparison changed to
  `secret.len() >= "8"`: `cargo check -p probe-core` failed with `E0308`,
  and the plugin named `crates/probe-core/src/redact.rs:24`, in `register`,
  showed the code, and gave the expected and found types.
- **Release notes** on `git log --oneline 4467f24eb9..1c095e50d0` of
  `openagents`: 313 commits grouped and one merge left out, 275 of them
  under **Other changes** because the repository doesn't use Conventional
  Commits and most subjects open with a noun.
- **Dependency check** on `probe`: `syn`, `@types/node`, and
  `undici-types` each at two versions across `Cargo.lock` and `bun.lock`,
  and no license policy declared. On the `openagents` root: the
  `deny.toml` policy read, ten crates at more than one version, and
  `Cargo.lock` (313 KB) cut at the read limit after 160 packages, which the
  result says.

The release gate's `explain-error` scenario runs the same path on a
planted Python failure
([acceptance](../../release/acceptance.md#scenarios)).

## Found and fixed on the way

- **A tool read on a nearly spent budget got nothing.** `guest::read`
  asks for 64 KiB at a time, so a lockfile read after the manifests failed
  whole. The three plugins read in 4 KiB calls and keep what fits.
- **A manifest beside a lockfile read as having none.** The lockfile
  check counted only files that were read; it now counts every granted
  lockfile, and `bun.lock` is one.
- **A grader matched the prompt.** `breaking-footer` first graded a hash
  the pasted log already held, which any run passes; it now grades the
  rendered line.
