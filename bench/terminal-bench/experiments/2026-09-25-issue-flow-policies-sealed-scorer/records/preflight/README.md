# Preflight records

- `rust-gate/run.json` and its logs verify the integrated study build.
- `source.json` identifies that build's changed implementation files.
- `red-final.log`, `green-final.log`, and `repair-source.json` retain the
  scripted evaluator regression before the concurrent, unrelated
  `fresh_inputs` option was integrated. That option is off in both study
  policies. The integrated gate runs the same regression again.
- `graders.json` records each development entry's base and known-fix
  grades using the pinned binary. All four must discriminate before
  inference starts.

The [earlier historical compiler preflight](../../../2026-09-25-issue-flow-policies-read-confined/records/preflight/compiler-receipt.json)
compiled the actual `coder` crate through the same toolchain read scope.
These preflights do not contribute live-model successes to the comparison.
