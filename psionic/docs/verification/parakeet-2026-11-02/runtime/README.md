# Resident Parakeet baseline v1

This is the complete first product baseline, before any row-parallel decoder
optimization. The public API owns one pinned model/vocabulary and serializes
calls; the CLI and static library need no Python, NeMo or ONNX runtime.

- [Model policy](model-policy-final/evidence.json): 147/147 root mutants killed,
  zero equivalents, all 275 baseline checks and authority execution coverage.
  Earlier 165/135/30 and 146/142/4 observations stay in sibling folders.
- [Bank regression](parakeet-bank-red.log): the first actual GPU attempt produced
  zero features because the mel bank was not initialized. The new resident
  frontend composition check fails before the fix and passes after it, including
  changed clip lengths over reused workspace. The failed product artifacts are
  preserved under the independent reference record.
- [Actual full GPU corpus](gpu-corpus-v1-passing/process.json): one resident model
  open, all 29 frozen PCM clips, features, final encoder tensors, tokens and frame
  IDs. [Independent comparison](../reference/full-gpu-comparison.json) matches every
  token/frame/text exactly and every tensor value within the frozen tolerance.
- [Native API](native-api/record.json): full generated runtime and C face at O0/O2,
  then fatal ASan/UBSan boundary checks. The initial sanitizer launch aborted
  before main because macOS does not support `detect_leaks=1`; only the same
  already-built sanitizer executable was continued with that unsupported option
  disabled. That actual failure and continuation are retained. Leak checking is
  not claimed, and passing compile/O0/O2 steps were not repeated.
- [Performance](performance-v1.json): 29 clips in one model lifetime, 20.272 s
  fresh engine authentication/loading (filesystem cache uncontrolled), 1.555 s median over the ordered mixed-length cohort, weighted
  real-time factor 0.162. This does not meet a 300 ms dictation target. It is
  offline latency; microphone/key-release/streaming latency is not measured.

The passing producer's generated C/header and complete Coax source snapshot
are retained. The source-only snapshot includes no model weights or executable.
The native image stays outside Git. The foreign-working-directory build was
followed only by rebuilding changed face/runtime objects when snapshot support
and the bank fix landed; identical GPU compilation results were reused.

Input is 320..327520 mono 16 kHz samples (20 ms..20.47 s). GPU requests fail
explicitly when Metal cannot run; there is no silent CPU fallback. Actual
CPU decoder and GPU encoder observations have their own producers and scopes.
The separately recorded generic scalar 20.47 s profile remains historical.
