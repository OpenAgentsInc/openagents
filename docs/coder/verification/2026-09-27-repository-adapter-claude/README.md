# Repository adapter: Claude provider live acceptance (2026-09-27)

Status: the live independent-acceptance gate of the repository adapter
(#9674) passed on both synthetic cases through the `claude` provider. The
first section records the checker controls that ran before any model call;
[Live acceptance](#live-acceptance) records the two model runs and their
independent checks.

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

## Live acceptance

After the controls, checkout `958bb85f33` ran each case once through
`microcoder repository --grant … --detach` and then `coder task check` with
the case's own trust store (`CODER_CAPABILITY_TRUST`), again through
`openagents computer exec coderos`. [live/](live/) retains both task stores:
launch diagnostics, grants, ATIF traces, artifact manifests and blobs,
`tasks.json`, and the `coder task check` output.

| Case | Store | Model steps | Elapsed | Candidate snapshot | Check |
| --- | --- | --- | --- | --- | --- |
| `ceil` | `live/acceptance-9674` | 28 Claude, 10 Jev | 40 s | `e1f7b485fd97…` | `passed` |
| `range` | `live/acceptance-9674-b` | 23 Claude, 8 Jev | 39 s | `01d8ef57a67e…` | `passed` |

Every Claude record in both traces names `claude-opus-5-5` served; Jev
records name `jev-1.13.0`. Claude reported list-price cost per call
(`total_cost_usd`, at most $0.049 per call, about $0.16 per case). The
check verdicts come from `coder task check`: the `independent-rust` suite ran
against the frozen candidate snapshot, `before_snapshot` equals
`after_snapshot`, and the typed suite verdict matched its identities. The
checks are independent of the model's own completion statements.

The first `range` launch was refused before any model call with
`Docker executable or local socket differs from admission`: the profile named
`/var/run/docker.sock`, which NixOS links to `/run/docker.sock`, and the
adapter admits only a canonical socket path. The second setup
(`acceptance-9674-b`, [profile](live/acceptance-9674-b-profile.json)) names
the canonical path. The refusal is a correct fail-closed check, not a code
defect.

## Limits

- The fixtures are the original synthetic `ceil` and `range` repairs, not a
  benchmark; one run each is a gate observation, not a rate.
- Cost figures are provider-reported list prices; the operator's Claude
  subscription was the billing basis.
- The originating stores are under `/home/christopherdavid/acceptance-9674`
  and `acceptance-9674-b` on `coderos-4080`; this copy records their content,
  not a runnable replacement.
