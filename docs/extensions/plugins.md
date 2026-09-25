# Wasm plugins

Status: partly implemented. The host core and the program `module` step
landed in #9519 on 2026-09-21. The rest of this document is the target
specification for typed operations, shared evidence, and
[program execution](programs.md); [What is built](#what-is-built) says which
parts exist today.

In this document, *plugin* means an OpenAgents WebAssembly guest. It doesn't
mean a Claude Code or Codex client package under `plugins/`; the
[glossary](../glossary.md#plugins-and-skills) separates the meanings.

## What is built

Three crates implement the host core:

- [`crates/plugin`](../../crates/plugin/) is the host. `plugin::invoke`
  compiles the guest with Wasmtime and runs it in a fresh store and instance
  for every call, so no memory or handle survives between calls. It links no
  WASI and no other ambient interface.
- [`crates/plugin-pdk`](../../crates/plugin-pdk/) holds the
  `openagents.plugin-packet.v1` request and response types that the host
  and a guest share.
- [`crates/plugin-outline`](../../crates/plugin-outline/) is a diagnostic
  guest. Its `echo` operation returns its input, and its `outline`
  operation lists a granted snapshot. The compiled guests are the fixtures
  in `crates/plugin/fixtures/`.

The host implements the two access modes as `plugin::Profile`:

- `Pure` links no host imports. A guest that declares any import is
  refused with `HostError::Denied`.
- `SnapshotRead` links one import, `oa_host.call`, which lists and reads
  the handles issued for that invocation. A handle from another invocation
  is `HostError::Stale`, and path traversal, symlink escape, and a partial
  capture are refused.

`plugin::Limits` bounds each call. The defaults are the following:

| Limit | Default | What it bounds |
| --- | --- | --- |
| `fuel` | 1,000,000 | Guest instructions, including the start function. |
| `memory_bytes` | 8 MiB | Guest linear memory. |
| `output_bytes` | 64 KiB | The response body. |
| `read_bytes` | 64 KiB | Snapshot bytes that one call may read. |
| `module_bytes` | 2 MiB | Guest module size accepted for compilation. |

A call that doesn't return a value fails with a typed `HostError`:
`Malformed`, `Denied`, `Limit`, `Stale`, `Failed`, `Cancelled`, or
`Refused`. The host checks pointer and length ranges, refuses output that
overlaps the input, and reports a spent `fuel` allowance as a limit rather
than a trap. A returned value always carries `verification: not_run`,
because a guest doesn't verify its own output.

`plugin::build_receipt` binds a guest build to the PDK source it was
compiled against: the SHA-256 digests of the PDK source and the guest bytes,
and the profile the guest was built for.

In `crates/coder`, the program runtime runs a `module` step through
`plugin::invoke` (`run_module` in `crates/coder/src/runtime.rs`), on the same
path the terminal and `coder -p` share. Admission refuses a `module` step
that carries no guest bytes (`bytes_base64`) or names a profile other than
`pure` or `snapshot-read`. The step's `fuel` and `memory_bytes` bounds
override the defaults, and a step that states no `fuel` gets 50,000,000.
Admission also accepts `output_bytes` and `read_bytes` bounds, but the
runtime doesn't pass them to the host, so the host defaults apply.
A guest refusal, a crossed limit, and a cancellation map to the step's
`refused`, `limit_exceeded`, and `cancelled` refusals.

The following aren't built yet:

- The manifest, operation schemas, and compatibility checks in
  [Manifest and compatibility](#manifest-and-compatibility). The host takes
  guest bytes and an operation name; it doesn't read a manifest.
- Snapshot grants from a program. The runtime passes an empty snapshot to
  a `snapshot-read` module step, so that step has nothing to read.
- A wall-time deadline, a host-call count bound, and a compilation cache.
  Each call compiles the module again. The host checks cancellation before
  the guest starts and at each host call, but the runtime doesn't yet
  connect a run's cancellation to a module step.
- The [host roles](#host-roles): evidence preparation, output processing,
  and hook registration.
- The full [build provenance](#authoring-and-build-provenance) and the
  [invocation receipt](#invocation-receipt). The build receipt holds only the
  three fields above, and the run records a module step's output, not a
  digested invocation receipt.
- The authoring surface: no command initializes, builds, packages,
  installs, or lists a plugin.
- Resolving a module by digest in the product, and a remote plugin
  catalog. A program carries its guest's bytes inline, and no program in
  `programs/` uses a `module` step yet. The
  [interoperability suite](../coder/verification/2026-09-22-relay-interoperability.md)
  locates a guest over a relay and runs it, but only in a test.

## Guest boundary

A plugin is a content-addressed WebAssembly guest with a manifest, typed input
and output, and bounded host imports. It computes or reads explicitly granted
snapshots. Rust owns invocation, authority, storage, decisions, and effects.
A module's bytes, manifest, schemas, and operation descriptors have separate
digests bound by the package release. Matching the module digest alone does
not establish its interface or permissions.

The [NIP-PRG guest profile](../../nips/openagents/NIP-PRG.md#wasm-module-profile)
specifies two access modes:

| Mode | Available operations | Admission |
| --- | --- | --- |
| Pure computation | Transform the supplied packet; no ambient filesystem, process, network, clock, randomness, or credentials. | Installed and enabled component, supported ABI, validated inputs, and resource allowance. |
| Snapshot reading | Pure computation plus bounded reads from named, immutable evidence or repository snapshots. | All pure-mode checks plus explicit host grants for the named snapshots and readable scope. |

Neither mode permits writes to the workspace, process spawning, network
requests, model calls, package installation, or changes to policy. Refuse a
manifest requiring these effects. A plugin may return a proposed patch or
structured test invocation as data; a separately authorized native operation
validates and applies it. The plugin cannot turn its return value into a
command by choosing a special string.

Network-capable adapters and agent executors remain separate host components.
A future guest profile requires its own version, threat model, and acceptance
evidence. The revised NIP-PRG profile refuses these broader effects;
unsupported required bounds cause admission refusal.

## Manifest and compatibility

Each plugin manifest must declare:

- Publisher-qualified identity, version label, exact module digest and byte
  size, license, source provenance, and compatible host/ABI versions.
- Named operation exports and their versioned input/output schemas, maximum
  packet sizes, supported formats, and partial-result semantics.
- Required access mode, supported imports, read scopes, and resource minima
  and requested maxima. Optional access must be explicitly distinguished;
  absence changes a typed result, never silently broadens access.
- Host roles, event/input schemas, eligibility conditions, and output
  representation types. Exporting an operation grants no automatic role.
- Determinism and idempotency claims, including any dependencies on parser
  versions, snapshot metadata, locale, or other supplied inputs.
- Fixtures and conformance evidence references. These are author claims until
  the host or an independent evaluator verifies the identified artifact.

Reject unsupported manifest versions, unknown required imports, ambiguous
exports, schema/digest mismatches, duplicate identities, and limits the host
cannot enforce. Version labels are for people; digests select executable
bytes. An ABI-compatible update is still a different artifact requiring an
explicit update and fresh admission.

The engine is Wasmtime, which preserves the reference design's practical
Rust embedding. `crates/plugin` pins Wasmtime 48. Bind the engine version and
configuration in the host receipt; the current host doesn't record them yet. Engine choice does not make guest output trustworthy or prove
determinism. No ambient WASI access is enabled by default.

## Typed packet ABI

Implement the [NIP-PRG packet ABI](../../nips/openagents/NIP-PRG.md#packet-abi),
with a Rust PDK hiding allocation and serialization. `crates/plugin-pdk`
provides the packet types, and `crates/plugin/tests/abi.rs` covers the
memory, import, fuel, stale-handle, and cancellation cases. Conformance
vectors for the full contract cover:

| Boundary | Required contract |
| --- | --- |
| Invocation | ABI version, host-generated invocation ID, exact operation ID, input schema version, typed value, and scoped evidence handles. |
| Return | A discriminated success, unsupported-input, or refusal value; exact output schema; bounded value and source references. |
| Memory | Explicit allocation/deallocation ownership; checked pointer/length arithmetic; valid guest-memory ranges; no reused stale references. |
| Encoding | One documented serialization, duplicate-key and unknown-field policy, numeric/string limits, and rejection of trailing or malformed input. |
| Imports | Versioned typed calls with per-call and aggregate bounds; scoped opaque handles rather than ambient paths. |
| Failure | Distinct host validation failure, guest refusal, trap, deadline, cancellation, memory exhaustion, and output-limit failure. |

NIP-PRG fixes binary exports and serialization. Generated PDK types and
fixtures must agree with that contract; packages must not advertise
compatibility before their implementation passes it.
The PDK's public programming model is a typed input-to-result function. It
must not encourage plugins to write tool-call envelopes or imitate a model.

Validate input before entering the guest. Validate output before committing
evidence or passing it to a consumer. The host assigns source scope,
invocation identity, capture completeness, and authority metadata; a guest
cannot assert these fields to certify itself. Treat strings inside a valid
packet as untrusted data when building later model inputs.

## Reads and resource accounting

Snapshot imports expose only named handles and bounded operations such as
metadata lookup, directory listing, and byte-range reads. Reject path
traversal, symlink escape, out-of-range reads, and handles from other runs.
The host must resolve a grant against stable object identities. If it cannot
provide a stable snapshot, record versions before and after and reject stale
results rather than claim an immutable read.

Bound compilation, wall time, guest memory, instruction work, packet sizes,
host-call count, aggregate bytes read, output bytes, and retained evidence.
Check declared lengths before allocation. Host imports consume the same
deadline and budget as guest work. An instruction meter alone does not bound
a blocked import. The host must be able to interrupt a guest and cancel its
imports; admission refuses unenforceable requirements.

Use a fresh invocation state. A compilation cache may reuse verified module
code by module digest, engine version, and configuration; it must not share
mutable memory or evidence handles across tasks. Snapshot access contributes
to the cache identity of a result. A manifest's deterministic claim is not
sufficient to cache a result across incompatible inputs or host policies.

Ceilings are explicit host configuration constrained by parent budgets. The
current defaults are in [What is built](#what-is-built); they aren't yet
measured against real guests. Do not assume universal
numeric limits merely because they were once convenient. Records must show
the effective limits, consumption, and any unknown values.

## Host roles

The target host supports three roles inherited from Coder, with narrower
activation than a general event bus. Only the explicit operation, through a
program `module` step, is built:

| Role | Trigger and result | Control |
| --- | --- | --- |
| Evidence preparation | An admitted context request asks for a repository map, outline, facts, or another derived representation. | Host selects bounded operations and stores results with source references; no unconditional per-turn prelude. |
| Output processing | A supported native operation captures output and requests a typed derivative. | Host checks command/output format eligibility and binds the derivative to that capture. |
| Explicit operation | A program `module` step or approved operation call supplies typed inputs. | Runtime admits the exact component, arguments, authority, and limits. |

Loading a package does not register arbitrary executable hooks. Role
registration declares supported event and schema IDs; activation requires
host policy and a bounded task or session scope. A hook cannot recursively
trigger itself, invoke other hooks, or extend its own lifetime. Programs
express explicit multi-step composition instead.

For automatic output processing, one host rule owns the selected
representation for a capture. Multiple derived views may be computed
explicitly when useful, but completion order cannot determine which view
reaches the model. Record the winning rule and alternatives. Typed program
dataflow can chain transformations; installation order cannot create a chain.

## Evidence transformations and fallback

Retain the original bounded capture under the host's evidence policy and
store derivatives separately. Each derivative names the original evidence
version, plugin and schema digests, transformation parameters, capture limits,
and any omissions. A ten-line summary of truncated output is still derived
from incomplete evidence. It cannot restore lines that were never captured.

The context builder may use an outline, diagnostic summary, matching spans,
or raw text and retain a path to expand the original. Format validation is
mechanical; whether a representation answers the task may require a separate
admitted decision function. A guest does not declare its output sufficient
for every consumer. Never replace mandatory error, verification, instruction,
or acceptance evidence solely because a shorter representation exists.

On unsupported input, malformed output, trap, timeout, or optional-role
failure, preserve the original and follow the host's bounded fallback policy.
Fallback may select a safe bounded raw view or report that more evidence is
needed. It must not inject an unbounded original into a model context. An
explicit required module step follows its declared program failure policy;
it does not silently become a successful no-op.

## Plugins in optimization studies

A plugin may be an implementation dependency or prepare evidence for a study.
An optimizer can propose new guest bytes only within an admitted source/build
surface. Build, schema, sandbox, and effect validation apply before measurement.
The study identifies the exact loaded guest and host imports.

Pure and snapshot-read guests do not acquire model, network, or process access
by being generated by DSPy, GEPA, or another authoring tool. Model inference
runs through a separately admitted host operation. Optimizer and evaluator
execution also require their own capabilities and reservations.

Prompts and examples are not Wasm modules. Export only supported component
types and pinned assets, and refuse executable foreign serialization. The
[optimization architecture](../optimization/architecture.md) defines those
boundaries and the evidence needed before adopting a generated component.

## Authoring and build provenance

Provide a Rust PDK, starter manifests, fixtures, and a shared authoring service
used by the CLI, terminal, and any model-facing tool. Expose closed typed
operations for initialize, build, inspect, test, package, install, list, and
uninstall. These are proposed surfaces, not commands available today. Creating
a project writes files under a selected directory; building runs native
tooling under ordinary host execution authority. A Wasm sandbox does not
sandbox Cargo build scripts.

The current `plugin::BuildReceipt` binds the PDK source digest, the guest
digest, and the profile. The target build receipt must bind the source snapshot, all relevant local dependencies,
lockfile, hidden configuration, build scripts, toolchain, build inputs,
artifact and manifest digests, and completed validation. Record pending,
failed, and completed builds distinctly. A fresh isolated build directory and
locked dependencies reduce stale-artifact mistakes; they do not establish
hermeticity or supply-chain attestation.

Check source identity before and after building. Refuse packaging or
publication from a stale, failed, or unmatched artifact. Fixture success on
one binary cannot certify another. Local installation may use an independently
verified prebuilt release without a local build receipt; its provenance must
say so. No model-facing authoring call gains broader filesystem or publication
authority than the equivalent human-facing operation.

## Invocation receipt

Record component/manifest/schema digests, host and ABI versions, invocation
and originating program/step/call IDs, input/output digests, source evidence,
grants and policy, effective limits, timing, consumption, and typed outcome.
Record fallback and whether a derivative was selected, merely produced, or
rejected. Sensitive payloads stay in scoped evidence storage; a receipt need
not duplicate them into a globally readable log.

Receipts establish attribution and local consistency. They are not remote
attestation, a correctness proof, or evidence that fewer output bytes improved
the complete task. See [evaluation](delivery.md#evaluation-and-default-admission).
