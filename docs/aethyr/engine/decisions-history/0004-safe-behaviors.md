> Deprecated (owner, 2026-10-24): history only. Architecture decisions live in [`docs/adr/`](../../../adr/README.md).

# 0004: Metered creator behaviors with familiar syntax

Status: selected design, 2026-10-22. Language/compiler feasibility remains a
blocking [experiment](../risks-and-experiments.md) before implementation qualification.

## Decision

Creators use a documented TypeScript syntax/type subset. Compile it to verified
owned bytecode executed by a metered Rust/Wasm VM in an isolated worker. Typed
state persists outside the guest; host calls require scoped capabilities. Heavy
work uses bounded trusted engine kernels.

## Alternatives and consequences

Arbitrary native JavaScript in a worker offers broad ecosystem compatibility but
no instruction-fuel mechanism. Terminating a worker is a recovery backstop, not
bounded per-invocation control. Wasm alone also does not meter execution or make
powerful host imports safe. Full Rust gameplay requires compilation/tooling that
is unnecessary for common creator behaviors. A new surface language would add
learning cost without a demonstrated benefit over familiar syntax.

The selected subset excludes ambient DOM/network, dynamic code execution,
unbounded library imports, and silent native-JavaScript fallback. This is a real
compatibility restriction. The editor must explain unsupported constructs with
source locations. Instruction costs, limits, type semantics, source maps,
migrations, and supported libraries need conformance fixtures. Trusted engine
plugins use a separately reviewed installation model and are outside this safe
profile.

## Revisit

Measure author usability, compilation latency, VM overhead, and escape/denial
cases. A replacement VM or language compiler must satisfy the same state,
metering, capability, and migration contract. See [scripting](../scripting.md) and
[bytecode](../behavior-bytecode.md).
