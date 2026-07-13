# Conformance fixture attribution

The fixtures under `valid/` and `invalid/` are vendored unmodified from the
ProductSpec open standard repository:

- Source: https://github.com/gokulrajaram/ProductSpec (`conformance/`)
- Source commit: `9ef2654bdd01aef3985fef6ed5a9ab66365999e1` (parser `0.19.0`,
  document format `0.1`) — the pinned PSEL compatibility target per
  `docs/assurance/PRODUCTSPEC_EVIDENCE_LOOP.md`
- License: MIT

One byte-level normalization is applied on vendor: a trailing blank line at
end-of-file (upstream `valid/with-traceability.product-spec.md`) is trimmed to
a single final newline to satisfy this repository's whitespace policy. The
content is otherwise byte-identical, and the change does not affect any
conformance verdict.

They are the compatibility oracle for `@openagentsinc/product-spec`: under the
`upstream` validation profile our validator must accept every `valid/` fixture
and reject every `invalid/` fixture with the documented error code. Refresh
them from the upstream clone at `~/work/projects/repos/ProductSpec` only when
deliberately re-pinning a new upstream release, and record the new source
commit here.

Deliberately not vendored from the same commit (out of the package's scope):

- `conformance/graph/` — exercises the upstream ProductSpec graph resolver,
  which this package does not implement;
- `conformance/invalid/missing-required-decision-trace-field.decision-trace.json`
  — exercises upstream Decision Trace JSON validation, which this package does
  not implement.

OpenAgents-specific extension fixtures live in `../openagents/` and are ours.
