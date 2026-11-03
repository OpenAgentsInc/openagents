# Coder repository pilot evidence

Recorded 2026-11-03 on the local macOS 26.4 arm64 host. The live product
basis was `1bfc93819eb20e097d1d1729e31019a392f58de3`, with the new evaluation
harness in an uncommitted worktree (`dirty: true`). Subsequent changes in
this delivery add the public `bench journey` dispatch and correct replay
help; they do not change engine execution or routing decisions.

The [pilot instructions](../../coder/repository-pilot.md) define the cases,
partitions, budgets and outcome meanings. The exact measured harness is
`coder/tests/repository.coax`, SHA-256
`f3a47b580c070ba778eec4ce6baba20b090feb524442cd49333950c08ac6af8e`.
The qualified compiler bytes had SHA-256
`cdaceb89e69c8d523e41242a940af34774e8b6b4f945998c2d3bf38486c357fb`.
Receipts bind both identities, the suite and each prompt.

## Offline journey

The native Coax harness passed ten fixture qualifications (broken baseline
fails, known repair passes external acceptance, protected files unchanged)
and seven CLI journey assertions:

1. A visible failing check appears as an error result in a completed session.
2. A conversation fork edits the source and passes external acceptance.
3. `coder replay` verifies the fork's engine input without rerunning its edit.
4. `coder log` retains the edit evidence.
5. `coder stop` interrupts a detached command before its marker write.
6. A fork after interruption completes a new turn.
7. The reviewed diff changes source only and is captured in a local sink.

The stopped commands' marker files also remained absent after their original
30-second sleep windows had elapsed. This checks those cancellation paths,
not every process or power-loss recovery scenario.

The public integration was checked first before implementation: `coder
bench journey --json` returned usage exit 2, since the suite did not exist.
After adding the dispatch it passed, with the retained
[JSON bench report](journey-bench.json). The journey itself took 2,599 ms
in that report. Its [reviewed diff](journey.diff) is committed here.
The default compiler path was also checked: without an exported `COAX`,
the new suite initially failed to pass its compiler to child invocations.
The dispatch now forwards the qualified override or checkout default.
The [default-path bench report](journey-bench-default.json) passed in 2,354 ms.

## Live coding outcomes

The ten tasks, split five development/five held, were fixed before inference.
No prompt or routing policy was tuned after inspecting outcomes. Each engine
used the same task specification, original fixture and external acceptance
checks. Case 9 includes a diagnostic turn followed by a repair fork. Every
accepted patch also preserved visible tests, instructions, the unrelated
file and the original baseline history, with no extra untracked files.

| Configuration | Recorded trials | Accepted patches | Held accepted | Provider failures | Missing cases |
| --- | ---: | ---: | ---: | ---: | ---: |
| Microcoder gateway plan | 10 | 10 | 5 of 5 | 0 | 0 |
| Codex CLI 0.163.0 | 10 | 10 | 5 of 5 | 0 | 0 |
| Claude Code 2.1.317 directly | 1 | 0 | Not run | 1 | 9 |
| Automatic routing | 10 | 0 | No eligible task outcomes | 10 | 0 |
| Psionic local engine | 0 | Not qualified | Not run | Not inferred | 10 |

The direct Claude observation reported a provider failure; it was not
repeated for the other nine fixed-engine cases. The automatic trials are
distinct tasks through the normal routing path. System One answered all
ten and selected Claude Code every time. Those selected runs returned
provider failures, so these are ten availability opportunity misses against
the successful fixed configurations and zero measured coding-quality
misses. Zero quality misses here does not mean the router is perfect: no
automatic run reached an eligible coding outcome.

The default local Psionic model-list endpoint on port 8080 returned HTTP 404;
no qualified Psionic model was selected or measured. Missing coverage is
retained rather than counted as a pass or a model failure.

Both fixed eligible configurations achieved the same acceptance count on
these elementary tasks. Their eligible-run P50/P95 times were 10.1/23.9
seconds for Microcoder and 12.7/27.3 seconds for Codex. Some trials overlapped
on the same machine, each case has only one observation per configuration,
and model/effort defaults were not controlled across engines. These timings
are observations, not evidence of a general speed ranking. At ten values,
nearest-rank P95 is the largest observed value.

Model identity, tokens, cached tokens and subscription price are not exposed
by this evaluator and remain null. The engine versions and harness/compiler
identities are recorded. `tool_calls` means observed Coder tool log rows;
Codex emitted zero such start rows despite producing working patches and
error results. The receipts therefore do not establish equal tool usage or
absence of Codex tool activity. No manual intervention was used.

The next routing experiment should include provider health/quota eligibility
before engine selection, and measure an authorized fallback independently.
The current router's installed-CLI check is not an availability check. The
result does not justify changing quality labels or silently moving private
tasks to a new provider.

## Retained records

- [Microcoder receipts](micro.jsonl), [Codex receipts](codex.jsonl),
  [direct Claude receipt](claude.jsonl), [automatic receipts](auto.jsonl).
- [Machine-readable summary](summary.json), derived without model inference.
- Representative held-out multi-file repairs:
  [Microcoder case 5](micro-case5.diff) and [Codex case 5](codex-case5.diff).

Only synthetic fixture diffs and aggregate receipts are committed. Full
events and external acceptance files remain in the harness's disposable
local directories, subject to normal temporary-file housekeeping. They
contain no imported private repository or conversation inputs.

## Validation

- `coax test coder/tests/repository.coax --library`: 145/145 checks passed.
- `coax test coder/tests/repository-summary.coax --library`: 103/103 passed,
  including duplicate/incompatible receipt refusal and a synthetic paired
  availability-versus-quality comparison.
- Offline CLI journey: all 17 assertions passed; public `bench journey`
  dispatch also passed after its initial missing-suite check.
- Coder book check passed: 16 programs, 109 exact source-linked listings and
  local links. Whitespace check passed.
- Required tools-in-Coax check was run. It reports eight pre-existing files
  missing reason headers (`coax/tests/vectors/check_pins.py`, `test_pins.py`,
  `verse/tools/fivemmo_hotbar.py` and five `verse/tools/humans/` tools), plus
  a stale port-list entry for `psionic/tests/decoder_oracle.py`. This change
  introduces only Coax tooling and does not modify those unrelated files.
