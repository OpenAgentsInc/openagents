# Everglade edit latency (#12960)

The final merged host's ten-run edit-to-running median is **899.5 ms** on
coderos-4080, with samples between 861 and 905 ms. Its records are in
`merged-host/`. The initial run has a 900.5 ms median and samples between 895
and 992 ms. The driver reports whole milliseconds; `phase-medians.json` retains
the half millisecond in each run.

The machine has an Intel Core i7-14700K, 28 logical CPUs and Linux x86-64.
Coax is the qualified 0.51.2 compiler. Builds and the driver run at nice 15.
`machine-and-artifacts.txt` records the compiler and executable hashes.

## What the script measures

`coax/web/tool.sh fast-edit-bench SESSION --port 9060 --keep-edit --refusals`
starts an owned native room, static page server and muted headless Chrome. It
walks the actual connected seat to the inn before measuring. Each sample starts
before writing a distinct body of `door_ease` in the real shared gameplay file.

The body retains its clamp, adds `0.01*k*(1-k)*(k-0.5)` to the swing curve, and
adds a distinct offset of 1 through 10 millionths. The offset changes even the
native room's closed-door collision calculation while retaining the existing
endpoint checks' tolerance. `gameplay-edit.patch` records the last edit.

Each observation requires the new revision in both hosts, the independently
computed rule value, ordinary gameplay calls since publication and a later
page frame. Observation probes do not increment gameplay counters. The page
keeps the same engine object, state address and seat. The ten raw
`measurement-N.json` records include the last ordinary input and result.

The script measures this closed scalar profile. General procedure replacement,
worker replacement, state migration and signed device updates remain separate.
The application source edit is a fixture and is not committed as gameplay.

## Where the time goes

These are independent medians from the merged-host run, so the rows do not
sum exactly.

| Step | Median |
| --- | ---: |
| Validate the body and sealed source/compiler/toolchain inputs | 321 ms |
| Check and emit the small Coax candidate | 4 ms |
| Native compilation | 80.5 ms |
| Wasm compilation | 85 ms |
| Both compilers in parallel | 86 ms |
| Recheck inputs and publish files | 306 ms |
| Candidate build and publication | 719 ms |
| Tool invocation and observing both hosts | 177.5 ms |
| Edit to both running hosts | 899.5 ms |

The full page check reuses **1,197** passing checks and runs **155**, for 1,352
checks total. Its 704.5-second report includes 148.6 seconds in parallel runs,
547.4 seconds in sequential verdict processing and 3.0 seconds in reuse lookup.
The background job takes 704.9 seconds. These checks remain off the reload path;
the commit gate requires their completed proof for the exact current source.

A cold initial Wasm base build takes 735 seconds. Its warm instrumented build
takes 12.9 seconds. A clean full release artifact build, with the Wasm artifact
cache disabled, takes 20.43 seconds: 5.6 seconds in Coax, 1.8 in clang, 7.1 in
linking and 2.3 in wasm-opt. The full native build takes 737.25 seconds, including
its changed checks. All commands stay below the 15-minute limit.

## Verification

- The admission policy kills 315/315 root mutants. Publication kills 6/6.
- The filesystem lock passes physical Mac and Linux checks. Linux also passes
  ASan, UBSan and LSan, including contention and killed-owner release.
- All 662 room-suite checks pass in 290.7 seconds. This unchanged room-check
  graph applies to both images; both native images also pass their startup check.
- The edited page passes all 1,352 checks. `valid-proof/` contains its exact
  source receipt, root reports and completed verification state.
- Both the replaced build and the clean full build pass drift at local,
  100 ms and 250 ms, with three bots and raids, on owned ports 9060–9064.
- The clean full Wasm is **2,030,740 bytes**, below the **2,150,000-byte**
  Everglade budget. The initial output basename made the builder look for an
  `oracle-everglade5e` budget; the artifact is renamed and the existing
  `everglade5e` budget command passes without repeating compilation.
- Browser replay, mismatched identity and malformed-module probes refuse.
  A native load failure preserves the old revision, and a newer valid revision
  recovers. `refusals.json` contains these physical observations.

The negative fixture replaces the body with `return 0.0f32`. Its real page check
fails `doors: E at each door opens it, E again shuts it, and nothing else happens`
with exit 1. `bad-proof/` retains the failed check and timing report. The combined
long checker/browser command reaches its 780-second bound, so the driver provides
separate staging and observation modes. Observation uses the completed failure
for the exact same source and does not rerun the checker. The observation
passes: the visible session reports failure, the guarded commit refuses, and
HEAD remains unchanged. `bad-check.json` records all four predicates as true.

The initial measurements use base `599558ee03` plus this change. The final
rebase preserves `e7e0b0e71c` and `5d4baa60c1`. Their overlaps with the
conservative input manifest are `valleycanyon-face.c` and `host.js`. The former
does not compile into Everglade. The latter adds blend-v1 branches guarded by
an export Everglade does not provide. The merged host receives a fresh session
identity, fresh native/Wasm bases and another ten-edit measurement. Its source
proof completes in 5.46 seconds using the existing complete Coax check verdict;
the Coax program and native C inputs remain unchanged. The failure-observation
and guarded-commit code also remain unchanged by these upstream commits.
Both runtime images also pass local drift with three bots and raids on the
merged host; their logs are in `merged-host/`.

See [the workflow](../../../coax/web/fast-edit.md) for commands and boundaries.
