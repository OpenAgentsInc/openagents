# Cooperative coordinate export

This separate diagnostic investigates the first failed input roundtrip from
`../gemm-register-input-probe/run-1`. That experiment remains unqualified: it
failed before any GEMM or timing. Its sources, compiled library, and results
stay unchanged.

The host loads the previously built library with SHA-256
`ca0cc71eda8b8278d98ca0b2ca043104efe1352d35b65925aa232c6b652a3390`
and creates only `register_input_roundtrip`. Building this diagnostic compiles
only `exporter.swift`; it does not compile Metal source.

## Fixed cases and output

Exactly two command buffers each dispatch one 32-thread group:

1. `tail-1`: `n=m=k=1`, base `(0,0,0)`, the shape that failed first.
2. `interior-32`: `n=m=k=32`, base `(0,0,0)`. All left and right input fragments
   use the existing collective `load` path instead of its manual tail path.

Inputs use unique positive finite half bit patterns, `0x2000+i` for X and
`0x3000+i` for weights. Destination seeds use unique finite f32 bits
`0x3f000000+i`. Here `i` is the physical row-major element index. This changes
only the diagnostic data, not the failed kernel's indexing. The largest case
has 1,024 elements per source; the half ranges do not overlap.

After each completed dispatch, the host saves the entire 263,168-byte record
buffer and a JSON dump of every non-hole slot, including object, lane, slot,
coordinates, raw bits, and validity flag. Both files are written before slot
validation or coordinate interpretation. The raw buffer preserves partial or
malformed records if validation fails. The host then checks capacities, validity
flags, all untouched holes, finite values, every buffer redzone, and exact input
hashes. It imposes no coordinate-shape or uniqueness assumption.

`analyze.py` verifies the raw/JSON exports agree and reports both proposed right
input shapes: physical `(K,N)=(16,32)` and logical `(N,K)=(32,16)`. It tests coverage
and data placement separately for each proposal. The tail case observes the
existing manual mapping; the interior case observes collective loading. These
are diagnostic observations, not acceptance of a repaired GEMM. No mapping is
changed and no performance timestamps are collected.

## Bounds and lifetimes

Only one case's four buffers are live at a time. The interior case allocates:

- X: 2,048 bytes.
- Weights: 2,048 bytes.
- Destination seeds: 4,096 bytes.
- Export records: 263,168 bytes.
- Eight 256-byte redzones: 2,048 bytes.

The explicit peak is 273,408 bytes, below the fixed 1 MiB limit. Raw and JSON
copies are owned CPU data; the process memory guard includes them. Each case's
command completes before reading or releasing its buffers. The host records
`device.currentAllocatedSize` but makes no memory-bandwidth or hardware claim.

The host build uses build and 4 GiB memory leases, a 16 GiB available-memory
preflight, a 12 GiB ongoing floor, two compiler threads, and a 180-second deadline.
The export uses GPU and 2 GiB memory leases, a 14 GiB preflight, a 12 GiB floor,
a 30-second external deadline, and a 10-second internal deadline after pipeline
startup. No quiet lease is permitted. The existing guard stops only its new
owned process group. No model, weight payload, desktop app, or system service is
involved. There is no automatic retry or limit increase.

## Review and execution

`frozen-sources.json` pins this source set and the shared guard dependencies.
`manifest.json` independently pins the unchanged compiled library, its source,
and its original build record. `source_check.py` validates source structure and
bit-pattern bounds without compiling or dispatching anything.

Source preparation does not authorize execution. After root review and an
explicit workload handoff, the commands are:

```sh
openagents lease build --keep-target-dir --receipt build-lease-1.json -- \
  openagents lease memory --amount 4 --receipt build-memory-lease-1.json -- \
  python3 guard.py build --output build-1
openagents lease gpu --receipt gpu-lease-1.json -- \
  openagents lease memory --amount 2 --receipt gpu-memory-lease-1.json -- \
  python3 guard.py export --output export-1
python3 analyze.py export-1
```

Run these from this diagnostic directory using the existing task target path.
The analysis retains incomplete exports as failures and does not reinterpret a
completed export as a numerical or speed pass.
