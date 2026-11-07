# CPU replay of actual native GPU histories — November 7, 2026

**#12990 remains open.** Starting at local HUD checkpoint `e31548810d`,
the actual CPU receiver now consumes Vulkan carrier histories directly,
without projecting them through a browser trace or substituting a browser
manifest. This addresses the preceding native-history replay gap. It does
not reproduce GPU effects, independently implement the simulation state
serializer or qualify automatic loss/rescue and all failure histories.

## Profile and actual receiver

`host/tests/stream-walk-replay.coax` accepts native GPU receipt v1/v2 with
backend 1, selects the original pinned Vulkan RCM1 companion, and reuses
the existing shared `SwWalk`, codecs, malloc/free face and retained fixture
sources. `stream-replay-profile.coax` admits exactly those schema/backend
pairs; unknown native versions and wrong backends refuse. Its root gate
kills four mutants, zero survivors. Existing browser-input replay retains
its v2 output identity. Native-input replay emits the separate
`aethyr.stream-walk-native-replay.v3`, with explicit backend 1.

The receiver reads the original native `warmup`, `route` and `cleanup`
records. It applies completion/destruction facts at their accepted
boundaries, then checks **every canonical paid-step digest**, including
the interior steps of a four-step frame. It also compares all 23 words of
every emitted record, every binary32 position bit, active/drawn/update/
budget observations and all five final actor fields for all twelve cells.
Final resource, decoded image/audio, inbox and renderer owners must drain.
The controlled presentation-only device replacement needs no CPU replay
effect: its handles and readbacks are outside the declared canonical state
profile, already checked separately by the actual GPU observer.

The retained-source, bounded integer and existing failed-browser-history
paths remain in this receiver. Native valid-source histories use their
original authenticated fixture bytes; this stage does not invent native
failure payloads. Native JSON needs more tree nodes, so the offline tool's
fixed JSON arena grows from 262,144 to 524,288 nodes; its 4 MiB input limit
is unchanged. This observer storage is separate from runtime resource
budgets and does not enlarge the browser product or host-v1 state region.

## Evidence

Both current RTX/Vulkan HUD histories pass through the actual
ASan/UBSan/LSan CPU executable. Each covers **3,232 ordinary frames,
three cleanup frames, 12,931 paid ticks and 114 attempts**. Every digest,
record and final actor value matches. A legacy native v1 history also
passes and rejects the same twelve altered histories, exercising the
retained native receipt identity. The changed
receiver passes an actual browser HUD history with its existing v2 output;
the browser independent oracle rejects all six modified histories.

The native independent oracle binds the entire original input SHA-256 and
reconstructs the concatenated observation SHA from each paid-step mark,
LE64 record word and position bit. For each current tier it actually runs
the same instrumented executable on **twelve altered histories**: changed
first or interior paid step, missing step, tick, position, completion
generation/digest, omitted completion, wrong backend, wrong manifest,
changed scene and changed final saved actor state. Each fails without
publishing a report and without sanitizer diagnostics.

| Native input | Exact CPU peak | Paid ticks | Observation SHA-256 |
| --- | ---: | ---: | --- |
| Current HUD tier 0 | 14,363 | 12,931 | `59fb76e35926c02eaab8525b671b6d22e54e747f5a41cc2d7f2b8f673a21dbe8` |
| Current HUD tier 1 | 14,427 | 12,931 | `027f66ab5512e89b61186ce666ecf71f7c519f0dea7ace675b374d610c157188` |

The first oracle incorrectly expected physical replay peak to equal the
synchronous Vulkan face's peak. The actual low-tier replay peak was
14,363 bytes, versus 14,139 in the GPU run. This was an oracle assumption,
not a canonical-state mismatch. CPU replay retains work/upload spans until
the recorded completion arrives and retains retiring resources until their
recorded destruction. The independent model now releases those extents at
the accepted event boundary, then charges the original ordered allocations.
Its **exact** reconstructed peaks match both observed CPU values, remain
within 32 KiB and finish with zero physical allocations. The original
failed checker log is retained; no memory equality or GPU allocator claim
is made from the CPU profile.

## Provenance and reproduction

The original scene is unchanged:
`ceefa05a481f8c8562646be74be333607075ae874fa3a076982ba23a81d00810`.
The source and artifact manifests accompanying this record pin the changed
receiver/policy/oracle, generated C/header, instrumented executable, raw
GPU inputs and CPU outputs. The [HUD record](walk-persistent-hud.md) pins
the actual GPU executables and all their source/readback observations.
Unchanged state serializer, coordinator and source-observation mutation
results remain those of the preceding committed records.

Artifacts are under
`~/.openagents/aethyrforge/streaming-12990/2026-11-07/native-history-replay-v1-san/`.
`replay.json` and `replay-high.json` are current native-input v3 reports;
`replay-browser.json` retains the browser-input v2 identity, and
`replay-legacy-native.json` uses the original native v1 input. The build
tool name's `v1` directory is a local attempt name, not the report schema.

Build/run with `coax/web/tool.sh aethyr/tools/stream-walk-native.coax
SCENE_DIR NATIVE_RECEIPT_JSON NEW_OUTPUT --sanitize`. Reuse its `replay`
binary for another history. For explicit sanitizer settings use
`ASAN_OPTIONS=detect_leaks=1:abort_on_error=1` and
`UBSAN_OPTIONS=halt_on_error=1`. Independently inspect it with
`python3 aethyr/tests/streaming_native_oracle.py --cpu-replay SCENE_DIR
NATIVE_OUTPUT CPU_REPORT_JSON`; the same binary must remain beside the
report for altered-history checks. Builds are bounded to 240 s with 180 s
subprocess limits, subsequent replays to 90 s and independent suites to
180 s; no command exceeds the
repository limit. Source/receipt fingerprints are checked after execution.
Tools reason and book registry results are retained; no chapter status is
promoted and no external issue completion is claimed.
The tools-reason result is reused from `e31548810d`: this stage adds only
Coax source, and the existing Python oracle's reason-2 header is unchanged.

Automatic/repeated GPU loss, deadline/rescue behavior, stale/superseded
callbacks, remaining allocator/SDK failures, category pressure, complete
transport/transient/backend cost planning, the independent world-shader
reference and final residual-adapter audit remain required for #12990.
