# Fixed deployment target: coordinate proof

The fixed macOS 26.2 deployment target corrected every exported input and seed
coordinate in both tiny cases, with no shader math or mapping changes. The
preprocessor comparison confirms the SDK compatibility branch responsible for
the earlier export failure. This validates the coordinate use in these two
cases only. No GEMM, real weights, or performance measurements ran.

## Measured compiler selection

| Compiler flags | OS minimum macro | Operand-aware SDK branch |
| --- | --- | --- |
| Original `-std=metal4.0` | `260000` | `0` |
| Add only `-mmacosx-version-min=26.2` | `260200` | `1` |

Both invocations used language version 400 and compiler
`32023.883 (metalfe-32023.883)`. The selected SDK resolved to macOS 26.5; the
running host was macOS 26.4. The inspected SDK header hashes were checked before
and after the build. Full macro outputs, stderr, commands, environment selections,
and hashes are preserved in [the proof](preprocessor-proof.json).

Below 26.2, this SDK's storage, load, capacity, element-pointer, validity,
coordinate, and initialization methods call destination-only helpers. At 26.2
they pass the operand index. That source distinction and the measured compiler
selection explain the prior common destination shape without inventing a lane
mapping or changing the shader's transpose.

## Exact export results

Every object has complete unique coordinate coverage and exact input or seed bits
in both the 1×1×1 tail case and the 32×32×32 interior case:

| Objects | Coordinates | Valid slots per object | Bit mismatches |
| --- | --- | --- | --- |
| Four left inputs | 16×16, `(K,M)` | 256 | 0 |
| Two right inputs | 16×32, physical `(K,N)` for this NT descriptor | 512 | 0 |
| Two destinations | 32×16, `(N,M)` | 512 | 0 |

All 16 object/case combinations match the original expected layout. The earlier
alternative logical `(N,K)` proposal is rejected by the right-input exports.
This confirms that a transpose/mapping edit would have been the wrong repair.
[Full analysis](export-1/analysis.json) preserves the successful and unsuccessful
hypotheses. Both runs retained all input bytes, redzones, and slot holes.

The candidate source remains byte-identical, SHA-256
`b5b5d77dc4406cd3023cd69f6838d670bfc774ad1e4196af16c2b536d47e9821`.
The coordinate host changed only its pinned library hash, as shown in
[the exact one-line diff](host-library-hash.patch). The target library hash is
`eab78a332c74906ef892664845785dc2d555c8239c97d17a6633e5dacdfdb12a`;
the new host hash is
`c8464382b7a907c3daf8158148a88cdb5f413fb2a9e9d5bc43d205caf10e46a6`.
Original failed sources and outcomes remain unchanged in the previous bundles.

## Bounds and limits

The one guarded proof/build completed in 3.424 seconds. The two-command export
completed in 0.435 seconds. These durations describe resource occupancy and
cleanup; no GPU execution timestamps or speed results were collected.

Explicit buffer peak was 273,408 bytes. Build limits remained 4 GiB / start
16 GiB / floor 12 GiB / 180 seconds / two threads. Export limits remained
2 GiB / start 14 GiB / floor 12 GiB / 30 seconds external / 10 seconds internal
after startup. The short export had one process-memory sample; its small sampled
value is not a measurement of peak residency. Both owned groups stopped, all
leases were released, and no resource, sampler, or cleanup failure occurred.

No quiet lease, app/service control, model, real weights, retry, alternate target,
or bound adjustment was used. Production compiles Metal source at runtime with
default compile options; this offline result does not establish that path's
minimum or operand-helper selection. Runtime compilation and full GEMM/weighted
qualification remain separate requirements before any product integration.

## Evidence

- [Source and guard plan](PLAN.md), [freeze](frozen-sources.json), and
  [build identities](build-artifacts.json).
- [Build receipt](build-1/receipt.json), [export receipt](export-1/receipt.json),
  and [complete exporter events](export-1/stdout.jsonl).
- `default.macros` and `target26_2.macros` preserve the complete preprocessor
  outputs; both corresponding stderr files are empty.
- `coordinates/` contains every slot and hole, split by object. Run
  `python3 restore_exports.py NEW_DIRECTORY` to reproduce both original JSON and
  raw UInt32 dumps byte for byte, with original hash verification.
- [Sanitization map](sanitization.json) records every original and published hash.

Private absolute paths become `<REPO>`, `<TASK_SCRATCH>`, and `<USER_HOME>`.
Original executed-source hashes are retained unchanged. Path substitutions in
published scripts need local replacement before reuse. No executable, metallib,
or model-weight artifact is included.
