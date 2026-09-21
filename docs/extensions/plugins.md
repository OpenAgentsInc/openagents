# Wasm plugins

Status: target specification. OpenAgents does not yet implement this host.
This design reworks the reference Coder plugin boundary for typed operations,
shared evidence, and [program execution](programs.md).

## Guest boundary

A plugin is a content-addressed WebAssembly guest with a manifest, typed input
and output, and bounded host imports. It computes or reads explicitly granted
snapshots. Rust owns invocation, authority, storage, decisions, and effects.
A module's bytes, manifest, schemas, and operation descriptors have separate
digests bound by the package release. Matching the module digest alone does
not establish its interface or permissions.

The first host profile supports two access modes:

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
evidence. NIP-PRG's wider module bounds do not require a host to support that
profile: unsupported required bounds cause admission refusal.

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

The initial engine should be Wasmtime, preserving the reference design's
practical Rust embedding. Bind the engine version and configuration in the
host receipt. Engine choice does not make guest output trustworthy or prove
determinism. No ambient WASI access is enabled by default.

## Typed packet ABI

Adopt a versioned packet interface, with a Rust PDK hiding allocation and
serialization. Treat the reference `packet-v0` interface as migration input,
not an ABI that OpenAgents already supports. Before implementation promotion,
publish an ABI schema and cross-language conformance vectors covering:

| Boundary | Required contract |
| --- | --- |
| Invocation | ABI version, host-generated invocation ID, exact operation ID, input schema version, typed value, and scoped evidence handles. |
| Return | A discriminated success, unsupported-input, or refusal value; exact output schema; bounded value and source references. |
| Memory | Explicit allocation/deallocation ownership; checked pointer/length arithmetic; valid guest-memory ranges; no reused stale references. |
| Encoding | One documented serialization, duplicate-key and unknown-field policy, numeric/string limits, and rejection of trailing or malformed input. |
| Imports | Versioned typed calls with per-call and aggregate bounds; scoped opaque handles rather than ambient paths. |
| Failure | Distinct host validation failure, guest refusal, trap, deadline, cancellation, memory exhaustion, and output-limit failure. |

This specification fixes the behavioral contract. Binary export names,
serialization details, and generated PDK types must be ratified together in
that ABI artifact; packages must not advertise compatibility before it exists.
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

Ceilings are explicit host configuration constrained by parent budgets, with
measured defaults established during implementation. Do not inherit legacy
numeric limits merely because they were once convenient. Records must show
the effective limits, consumption, and any unknown values.

## Host roles

The target host supports three roles inherited from Coder, with narrower
activation than a general event bus:

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

## Authoring and build provenance

Provide a Rust PDK, starter manifests, fixtures, and a shared authoring service
used by the CLI, terminal, and any model-facing tool. Expose closed typed
operations for initialize, build, inspect, test, package, install, list, and
uninstall. These are proposed surfaces, not commands available today. Creating
a project writes files under a selected directory; building runs native
tooling under ordinary host execution authority. A Wasm sandbox does not
sandbox Cargo build scripts.

A build receipt must bind the source snapshot, all relevant local dependencies,
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
