# Conformance fixture attribution

The fixtures under `valid/` and `invalid/` are vendored unmodified from the
ProductSpec open standard repository:

- Source: https://github.com/gokulrajaram/ProductSpec (`conformance/`)
- Source commit: `833d67d53d14f6026fafd03d30e53e3b7609421a`
- License: MIT

They are the compatibility oracle for `@openagentsinc/product-spec`: our
validator must accept every `valid/` fixture and reject every `invalid/`
fixture with the documented error code. Refresh them from the upstream clone
at `~/work/projects/repos/ProductSpec` when tracking a new upstream release,
and record the new source commit here.

OpenAgents-specific extension fixtures live in `../openagents/` and are ours.
