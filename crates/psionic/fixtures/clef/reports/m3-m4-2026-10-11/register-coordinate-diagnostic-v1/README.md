# Cooperative coordinate diagnostic

The two diagnostic exports completed with unchanged compiled Metal code and
passed their input, redzone, slot-hole, and cleanup checks. They establish that
the original host's coordinate assumptions were wrong. They do not establish a
correct replacement input mapping. The earlier GEMM candidate remains
unqualified; no GEMM or performance test ran here.

The separate diagnostic used the prior `register_input_roundtrip` kernel from
metallib SHA-256 `ca0cc71eda8b8278d98ca0b2ca043104efe1352d35b65925aa232c6b652a3390`.
The new host executable hash is
`8fb8df0a67572763c2ce16ae7b66881c70bb8cbd61d64772c28982fdbb082b73`.
The original failed experiment, its v1 compile failure, one-line address-space
repair, and its first roundtrip failure remain in the separate
`gemm-register-input-probe/publication-v1` bundle.

## Observations

Both fixed cases, 1×1×1 and 32×32×32, export eight objects. Every object has
512 valid slots with capacity 16 per lane and complete coordinates spanning
`[0..31, 0..15]`. The first coordinate outside the original assumptions is
**left input `a00`, lane 0, slot 8, coordinate `[16,0]`**, not a right input.

| Objects | Original assumption | Observed coordinates | Interior data placement |
| --- | --- | --- | --- |
| `a00`, `a01`, `a10`, `a11` | 16×16 | 32×16 | 256 coordinates outside the assumed shape; 224 of the 256 in-shape values differ |
| `b0`, `b1` | Physical 16×32 | 32×16 | Physical proposal: 256 outside, 224 in-shape values differ. Logical 32×16 proposal: all coordinates fit, but 510 of 512 values differ |
| `c0`, `c1` | 32×16 | 32×16 | Complete unique coverage and exact seed bits |

For example, interior `a00`, lane 0, slots 0–3 export coordinates
`[0,0]`, `[1,0]`, `[2,0]`, `[3,0]` with half bits
`8192`, `8194`, `8448`, `8450`. The original coordinate interpretation expects
`8192`, `8193`, `8194`, `8195` there. These observations do not determine whether
the mismatch comes from the public input layout, its use in this shader, or
another source issue. No transpose or lane-mapping repair is justified yet.

The tail case's few nonzero values do not distinguish the competing right-input
proposals; the interior case is decisive against both. Each interior input
object exports 128 unique nonzero half patterns and 384 zeros, whereas each
destination object exports 512 distinct seeds. This is an observation of this
compiled exporter, not a claim about the storage required by a valid MPP input.

## Execution and bounds

The one Swift-host build completed in 2.595 seconds. The export completed in
0.313 seconds with exactly two command buffers and two 32-thread dispatches.
These wall durations describe cleanup and resource occupancy, not performance.
No GPU execution timestamps were collected.

The explicit buffer peak was 273,408 bytes, below the fixed 1 MiB limit.
`device.currentAllocatedSize` reached 999,424 bytes at the recorded checkpoints.
The process guard used the reviewed 2 GiB cap, 14 GiB available-memory preflight,
12 GiB ongoing floor, 30-second external deadline, and 10-second internal limit
after startup. The build used 4/16/12 GiB, 180 seconds, and two compiler threads.
The export was shorter than the sampler's 0.5-second interval; its one tiny
process-memory sample is not a credible measurement of peak residency.

Both owned groups stopped cleanly, with no sampling error, resource rejection,
or cleanup failure. All leases were released. There was no quiet lease, real
weight access, model execution, mapping repair, or desktop/service action.

## Files

- [Analysis](export-1/analysis.json) keeps all object comparisons, including
  mismatches under both proposed mappings.
- [Exporter events](export-1/stdout.jsonl) record buffer counts, input hashes,
  export hashes, and memory checks.
- [Export receipt](export-1/receipt.json) and [build receipt](build-1/receipt.json)
  record the fixed guards and cleanup.
- [Plan](PLAN.md), [source manifest](frozen-sources.json), and
  [build identity](build-artifacts.json) preserve the exact attempted sources.
- `coordinates/` holds every slot, capacity, and hole from both original dumps,
  split by object for review. Run `python3 restore_exports.py NEW_DIRECTORY` to
  reconstruct the complete original JSON and raw UInt32 files byte for byte.
- [Sanitization map](sanitization.json) records original hashes, published hashes,
  and the lossless splitting rules.

`<REPO>`, `<TASK_SCRATCH>`, and `<USER_HOME>` replace private absolute paths. Hashes
of original executed sources are not rewritten. Published source with a replaced
path requires that label to be supplied locally before reuse; no executable,
metallib, or weight artifact is included.
