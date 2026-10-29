# Coax compile-time resources

Task #7287 adds a selected shared work budget and an independent live
aggregate-storage bound. `--run-steps` defaults to 100,000 and accepts positive
decimal values through 250,000,000; check and mutation budgets do not change.
`coax deps` records the selection and fixed storage limits. `--timings` records
consumed work and tracked aggregate peaks once evaluation starts.

The Coax `#authority` policy controls decimal admission, defaults and
allocation bounds. It passed four check groups and killed all 35 mutants.
The Python-free bootstrap includes the checked seed and regeneration checks.
The full compiler suite passed 98 fixtures and its integration checks, including
all ten resource tests; its optional pinned protoc oracle was skipped. The test
runner now propagates the resource test status, checked with both passing and
failing status values.
The evaluator charges the capacity requested by its vector allocator before
allocation and tracks every aggregate container. Copies are charged recursively,
views retain owners, frozen values belong to the compilation, failed partial
allocations unwind, and graph teardown breaks view cycles. Registry entries are
bounded and expired entries are pruned. Unreachable cycles can still consume
budget until compilation teardown; slots/containers are not a bound on total
RSS, strings, allocator metadata, syntax trees or host operations.

Nine driver and allocation tests passed against the 0.18.0 compiler with SHA-256
`4942904896b469e77840506319d5f1d09b5a35016a5c63f66fac6a022d0cbde0`.
They cover invalid and duplicate inputs, a shared counter across independent
`#run` sites, all compiler command entry points, retained constants, deeply
nested refusal, separate check limits, metadata, and ASan/UBSan owner, view,
cycle and partial-copy cleanup. Leak detection is disabled in this focused
harness because the existing compiler arena has a separate lifetime policy;
tracked slot/container counts are asserted directly. After those tests, a
one-line driver fix preserved the selected work limit in timing reports when
the frontend fails before evaluation. The previous source is byte-identical
after removing that line; every other compiler input matches the recorded
source hashes. The successful table/evaluation path is unchanged. A separately
built compiler with SHA-256
`4593dbc53bf9ea1d8c7a0c7b8e44e08517765635693ec715083a3933bc61629c`
passed focused success and failure timing tests. Both compilers also emitted
byte-identical actual combined-table C in checked and release profiles
(SHA-256 `6f904e3ad9953fe06eac5bf92e5ab0d16221ed3230327103f7c1bfed90b04797`
and `b0f909c1027c741ad0e6facc6c0c3168e6d4a6953235b888f2f4caabd129ed11`,
respectively). A fresh Python-free stage0
rebuilt from the final source and seeds passed the policy checks and mutation,
and emitted compiler C identical to the tested compiler graph. The arm64 iOS Simulator
app build also succeeded; this is not a physical-device runtime test.

The real combined public fixed-base table generator compiled under an explicit
250,000,000-step profile: 166,665,133 steps, peak 38,376 slots and 7,264
containers, in checked and release compiler profiles. Its independent correctness,
Wasm and mutation evidence belongs to the fixed-base task and must pass before
that task is resolved. No secret-safety or constant-time claim follows from the
compiler budget.
