# Native V5 observations — preserved before final checker cleanup

V5 built the frozen checker `11d2c2a9…` with generator `710c4f57…` and produced
image `6fc6af83…`. All 496 original source files are preserved in the source-only
archive, whose entries were independently hashed against `source-pins.json`.
The canonical 19 C/header/runtime source files are retained; the executable
remains outside the repository. Nine C/header fixedpoints match exactly.

The outcomes, 18-profile (including the actual Sum kernel at O0/O2/ASan+UBSan),
and file-adapter gates passed. The raw sanitizer-bridge unittest failed on its
180-second wall timeout while checking `checker.coax`; that failure is retained.
Its native compilation, link and first parser-source check had succeeded.

The bounded continuation reused those exact hash-pinned artifacts and successful
steps. It completed the failed checker check (172.31 seconds), previously unrun
home/grid checks, and the exact deep-type E0381 refusal assertions. Only the
checker check's wall bound changed to 600 seconds; the continuation stayed below
840 seconds. Compiler work limits, flags and all original oracle assertions
were unchanged. `record.json` composes these observations explicitly; it does
not relabel the failed raw unittest as passed.

V5 seeds were intentionally **not promoted**. The subsequent one-line checker
cleanup receives a separate V6 build/fixedpoint/cost/qualification record. No
normal build-cache, install, published package or release provenance was created.
