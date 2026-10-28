# Explicit unsigned wrapping operations

Task `coax-intrinsic-wrapping-ops` (#0ad5) adds `add_wrap`, `sub_wrap`,
`mul_wrap`, `rotl` and `rotr` for two matching u32/u64 values. Arithmetic is
modular in checked, optimized and compile-time execution; rotate counts reduce
modulo width. Arguments execute once, left to right. Ordinary arithmetic and
Metal's existing scalar admission are unchanged.

Coax owns type admission and its finite checks. The trusted C++ evaluator and C
emitter implement the target operation. Runtime helpers are emitted only when
used, preserving unchanged frontend/runtime seeds. Explicit wrapping also fixes
the earlier masked-select example: `0 - bit` must not rely on optimized-mode
underflow. It now uses `sub_wrap` and is tested in checked mode too.

## Evidence

- 555 checker checks pass, including 50 new unsigned-width/arity/error cases.
- The normal Python-free bootstrap reached its final release drift gate after
  the lexer/source/parser/checker/wire/fingerprint/importer gates and fixed-point
  comparisons. The final gate correctly refused a manually edited generated book
  table. It was not reported as a successful full bootstrap.
- Updated the canonical release catalogue, generated the snapshots and rebuilt
  the same compiler inputs without repeating unchanged mutation runs. Regenerated
  all four frontend C/header seeds byte for byte, regenerated the compiler C
  fixed point, and passed the final binary version/status drift check.
- The final coax0 0.15.0 binary passes five focused wrapping tests: 41,280
  independent native integer-oracle comparisons across checked/release builds,
  300 compile-time cases in both modes, 540 sanitizer cases, 1,060 actual wasm32
  executions, invalid type/arity cases and operand-order checks.
- Four constant-time-example tests pass: inspected native masked selection,
  checked-mode execution, Wasm fallback lowering and invalid barrier arguments.
  These are scoped code-generation observations, not a general timing guarantee.
- Ten release/status tests pass, including drift, preserved historical versions,
  invalid metadata and publication refusal before credentials/network.

The source, generated header, CLI status and book tables now share the canonical
0.15.0 catalogue. Schema version remains 1 and prior introduction versions remain
unchanged. No compiler-wide suite or iOS/TestFlight build was run for this slice.
