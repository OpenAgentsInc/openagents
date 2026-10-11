# Fixed macOS 26.2 coordinate diagnostic

This is a separate attempt to test the deployment-contract explanation in
`../register-deployment-review`. The previous candidate and both coordinate
exports remain unchanged. This diagnostic does not execute any GEMM.

The candidate is byte-identical to the earlier source with SHA-256
`b5b5d77dc4406cd3023cd69f6838d670bfc774ad1e4196af16c2b536d47e9821`.
The coordinate host template is byte-identical to the reviewed earlier host.
The only generated host change is one pinned library hash literal. The wrapper
uses the new identity `register-target26_2-diagnostic-v1`; the reused host's older
identity string remains unchanged to preserve its exact one-line diff.

## Fixed sequence

The build wrapper first preprocesses `deployment_probe.metal` twice. The first
invocation uses the original `-std=metal4.0`. The second adds only
`-mmacosx-version-min=26.2`. The selected SDK path must resolve to the inspected SDK; its three relevant
header hashes and deployment-related environment values are retained.
Both raw macro outputs, stderr files, commands, hashes,
and selected compiler/deployment macros are retained. Both must compile cleanly.
Require original minimum below 260200 with operand-aware branch 0, and targeted
minimum exactly 260200 with branch 1. A different outcome stops the attempt before
shader compilation. There is no alternate target or private macro override.

Only after that proof, the wrapper compiles the unchanged candidate once at the
fixed 26.2 minimum, links a separate library, generates the host by replacing
exactly one hash literal, saves that one-line diff, and compiles the host with two
threads. It does not compile the old baseline. Every generated artifact gets a
hash in the new build record. Existing output files cause refusal rather than
replacement.

A separately launched export uses the reviewed coordinate guard and the new
build identity. The host runs exactly the same two cases: 1×1×1 and 32×32×32,
base zero, one 32-thread dispatch per case. Input bits, redzones, slot sentinels,
raw exports, shape-neutral validation, and offline hypothesis comparisons are
unchanged. The full JSON and raw UInt32 dumps are saved before interpretation.
No input mapping repair or GEMM execution is permitted, even if the coordinates
now match the original expectations.

## Limits

- Build: build and 4 GiB memory leases; start 16 GiB, floor 12 GiB, 180 seconds,
  two Swift compiler threads. Metal source commands run sequentially.
- Export: GPU and 2 GiB memory leases; start 14 GiB, floor 12 GiB, 30 seconds
  external, 10 seconds internal after startup. Explicit buffer peak remains
  273,408 bytes with one case live at a time.
- No quiet lease, retry, limit increase, model, real weights, desktop control,
  or system-service action. Cleanup controls only the new owned process group.

Root reported this host as macOS 26.4 (25E246). The build independently refuses a
host below the supported 26.2 target.

## Review and execution

This directory is source preparation only until root reviews its wrapper changes
and grants a workload window. `source_check.py` checks hashes, ASTs, exact copied
inputs, parser behavior, and the retained case/allocation constants without a
compiler or GPU. `wrapper-changes.patch` records changes from the first coordinate
wrapper; generated shader and host hashes cannot exist until the build succeeds.

After explicit handoff:

```sh
openagents lease build --keep-target-dir --receipt build-lease-1.json -- \
  openagents lease memory --amount 4 --receipt build-memory-lease-1.json -- \
  python3 guard.py build --output build-1
openagents lease gpu --receipt gpu-lease-1.json -- \
  openagents lease memory --amount 2 --receipt gpu-memory-lease-1.json -- \
  python3 guard.py export --output export-1
python3 analyze.py export-1
```

Run from this directory with the existing task target directory. Preserve any
failure and stop. The offline result cannot establish production runtime-compiler
behavior: the serving path uses `new_library_with_source` and default compile
options, while this attempt uses the offline Metal compiler. That distinction
needs separate verification before any later product integration.
