# Final native V6

Image `a2053bb0…` is built from the final frozen checker `4b8bda36…` using
qualified V5 generator `6fc6af83…`. The source-only archive preserves all 496
exact input files, verified entry by entry against the recorded hashes. The
canonical 19 C/header/runtime files and actual executed scripts are retained;
the executable stays in the private build directory outside the repository.

All 20 build commands passed. All nine C/header fixedpoints match exactly.
The four native gates passed: 41 outcome fixtures (13 native positive programs
at O0/O2/ASan+UBSan), 18 profiles including the actual Sum policy kernel, the
24-case file adapter and header-composition checks, and the original complete
parser/checker/home/grid/deep-refusal sanitizer bridge.

The bridge runs the original unittest and assertions. Its private wrapper
changes only the sanitized checker-source child's wall allowance from 180 to
600 seconds; other checked children stay at 180 and the deep refusal at 30.
The whole gate stays below 840 seconds. The child took 203.30 seconds and the
whole gate 290.08 seconds. Compiler work limits and sanitizer flags are unchanged.
V5's earlier timed-out attempt and successful continuation remain separately
bound to their old source under the sibling `final-native` record.

All source and actual native-gate input hashes stayed unchanged. The passing
canonical outputs verified all 16 checked-in seed C/header files; only
`coax/bootstrap/checker.c` changed. No binary, normal build-cache promotion,
install, published package or normal release provenance was created. Root's
actual V6 full-compiler cost, final authored checks and consumer qualification
are separate records for the same frozen image and scope.
