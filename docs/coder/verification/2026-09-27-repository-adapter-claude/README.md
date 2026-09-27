# Repository adapter: Claude provider setup and checker controls (2026-09-27)

Status: infrastructure evidence for the `claude` provider path of the
repository adapter (#9674). No model call, benchmark run, or candidate check
ran; the live independent-acceptance gate is still unmeasured.

## What ran

On `coderos-4080` (NixOS), from checkout `788bb1ba03` of the branch behind
the pull request that admits the `claude` provider, driven through
`openagents computer exec coderos -- ...`:

1. `repository_acceptance_setup` with `ACCEPTANCE_PROVIDER=claude`,
   `ACCEPTANCE_MODEL=claude-opus-5-5`, and the default
   `https://api.anthropic.com` generation endpoint. It created the `ceil` and
   `range` task stores, worktrees, grants, trust receipts, and installed the
   two pinned checkers. Every case reports `model_calls_started: false`.
2. `repository-check-ceil` and `repository-check-range` against the
   intentionally broken fixture sources and against known-correct copies.

[controls.json](checker-controls/controls.json) retains the setup output, the
container profile the grants name, and all four checker verdicts with the
bounded compile and test transcripts.

| Control | Input digest | Verdict |
| --- | --- | --- |
| `ceil` broken fixture | `078341cd49b6…` | `failed` |
| `range` broken fixture | `64923f6ebc7e…` | `failed` |
| `ceil` corrected copy | `559f4916bc08…` | `passed` |
| `range` corrected copy | `8f7c50a30b8a…` | `passed` |

The checker rejected both broken sources and accepted both corrected controls
under the read-confined boundary with the pinned Rust 1.97.1 compiler. These
are checker controls, not verification of any model candidate.

## Portability fixes the run exposed

Three fixed paths in the example programs assumed a merged-`/usr` host and
broke on NixOS:

- the setup ran `/usr/bin/git`; it now takes the first of
  `task::owner::GIT_PATHS` that exists.
- the setup granted `/bin/bash`; it now grants the first canonical system
  shell of `/bin/bash` and `/bin/sh`, the set the adapter admits.
- the checker searched `/usr/bin:/bin` for the linker; it now uses
  `task::owner::SYSTEM_PATH`, which includes `/run/current-system/sw/bin`.

The linker emitted a `Resource temporarily unavailable` warning under the
boundary on every compile and still linked; the retained transcripts keep it.

## Limits

- No Claude call was made. The `claude-opus-5-5` identity in the grants is
  the exact identity the adapter must see served; it is not a served
  observation.
- The fixtures are the original synthetic `ceil` and `range` repairs, not a
  benchmark.
- The originating stores are under `/home/christopherdavid/acceptance-9674`
  on `coderos-4080`; this copy records their content, not a runnable
  replacement.
