# Final acceptance preflight

The final acceptance coordinator accepts the historical reference for reserved
A and rejects its base. Reserved B's independent checker also distinguishes
base from reference, but both versions fail the same ordinary crate test.
The current acceptance rule therefore rejects B's historical reference. B is therefore ineligible on this historical snapshot under the strict gate.
No check is skipped or weakened in this preflight; task replacement requires
prospective eligibility checks before scored trials.

These are no-model Linux feasibility checks on Boat, dated 2026-10-03. They
make no provider calls and produce no scored executor outcomes. The
[machine-readable record](acceptance-preflight.json) retains all five attempts,
phase timings, provenance digests, cleanup measurements, and the failed first
setup. Private checker contents, reference patches, and raw logs remain private.

## Results

Every attempt uses a 240-second outer acceptance budget. Scope and formatting
pass in all five attempts. Command times below include any compilation or
relinking performed by that command.

| Attempt | Acceptance total | Ordinary compile | Ordinary test command | Independent compile / test | Outcome |
| --- | ---: | ---: | ---: | ---: | --- |
| A base, initial setup | 26.735 s | 3.945 s | 2.827 s | 0.265 / 0.215 s | Harness incompatible: missing `rustdoc`; checker rejects base |
| A base, corrected setup | 25.613 s | 3.070 s | 2.769 s | 0.215 / 0.215 s | Ordinary tests pass; checker rejects base |
| A reference, corrected setup | 25.323 s | 2.820 s | 2.669 s | 0.215 / 0.215 s | Accepted |
| B base, corrected setup | 160.793 s | 113.937 s | 21.656 s | 1.068 / 0.315 s | Ordinary tests fail; checker rejects base |
| B reference, corrected setup | 176.745 s | 128.198 s | 22.072 s | 1.167 / 0.315 s | Ordinary tests fail; checker passes |

No attempt reaches the outer timeout. This establishes runtime feasibility for
these observed commands, including B's compilation and relinking. It does not
establish a passing full-suite gate for B, runtime feasibility for C or D, or a
latency distribution. C/D full acceptance remains unrun after B exposes the
ordinary-suite incompatibility.

### B's ordinary-suite failure

Both B variants fail
`delegate_door::tests::the_permit_decides_whether_the_executor_may_write_and_a_follow_up_resumes`.
The base library target reports 909 passed, one failed, and three ignored; the
reference reports 910 passed, one failed, and three ignored. Cargo stops before
later ordinary targets. These counts are not counts for the entire crate suite.

The historical public source calls
[`boundary` before entering `answer_wrapped`](https://github.com/OpenAgentsInc/openagents/blob/b39787700107a1605b3412c8653fad887adfbbf8/crates/coder-delegate/src/terminal.rs#L494).
The artifact directory is only
[created later in `answer_wrapped`](https://github.com/OpenAgentsInc/openagents/blob/b39787700107a1605b3412c8653fad887adfbbf8/crates/coder-delegate/src/terminal.rs#L611).
The [test fixture](https://github.com/OpenAgentsInc/openagents/blob/b39787700107a1605b3412c8653fad887adfbbf8/crates/coder/src/delegate_door.rs#L1701)
creates the workspace and stand-in executable, then names an artifact root.
On this Linux setup, boundary construction cannot canonicalize the nonexistent
turn artifact directory. Both retained failures report that missing directory.
No candidate, fixture, or checker is changed to bypass it.

## Retained setup corrections

The initial A attempt passes its unit and integration tests, then cannot start
`rustdoc` for doctests because the isolated environment does not expose it.
Its ordinary command fails; the completed receipt remains retained. The
subsequent scratch cleanup encounters a disappearing `gc.pid` from automatic
Git maintenance. The original cleanup duration is unknown, not zero. Recovery
verifies all 19 retained files before finishing cleanup in 0.070 seconds.

Before the corrected attempts:

- Shared Git snapshot initialization disables automatic GC and maintenance.
  Its regression verifies that the deterministic snapshot commit stays the same.
- All five trusted seed manifests and execution configurations bind the pinned
  `rustdoc` path, executable digest, and version. The metadata amendment verifies
  unchanged source and seed contents; it does not rebuild libraries or disable
  doctests. The [seed record](../../../../bench/delegation-study/seed-preflight.json)
  retains the preceding manifests and amendment measurements.
- The acceptance coordinator handles `SIGTERM` and `SIGINT`, stops child
  processes, and preserves the latest durable phase checkpoint before recording
  interruption. Actual Linux synthetic tests verify both signal paths and the
  declared `rustdoc` binding. They make no model calls and run no Cargo commands.

The two private bundle digests in the JSON distinguish the initial helper from
the corrected helper. The first failed attempt is not relabeled as a successful
setup check.

## Cache, timing, and cleanup

Acceptance verifies a source-bound, library-only baseline seed and copies it
into its own target directory. It never trusts the executor's mutated target.
Unchanged source retains archive timestamps; changed candidate files and the
injected checker receive fresh timestamps. The common build profile disables
debug information while preserving optimization, debug assertions, and overflow
checks, as recorded in the [seed setup](../../../../bench/delegation-study/seed-preflight.json).

Corrected seed-copy times are 0.019 seconds for both A variants and 1.040/1.629
seconds for B base/reference. B still spends 113.937/128.198 seconds in the
ordinary compile command: a library seed does not make verification compilation
free. Setup also includes source export, Git snapshot creation, and seed
validation. Their timings are nested inside setup; adding them again would
count them twice.

The four corrected scratch cleanups take 0.906, 0.858, 1.361, and 1.565 seconds.
They occur after durable receipts and confirmed process closure, outside the
acceptance timer. Separate wrapper timings include candidate preparation before
acceptance. The JSON preserves both boundaries.

Actual namespace probes confirm read-only source and checker mounts, unchanged
inputs, and absence of a delayed writer after phase timeout and outer watchdog
termination. These are bounded probes, not exhaustive isolation proof. At the
end, every preflight process has exited, the agent slot lock is independently
reacquired and released, and reconstructed workspace/home directories are absent.
Source archives, trusted seeds, raw logs, candidate identities, and receipts are
retained. A verified private archive binds the complete evidence without
publishing the independent oracle.
