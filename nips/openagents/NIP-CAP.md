# NIP-CAP — Execution capabilities

`draft` `optional` — v1.

This NIP defines portable execution descriptions and operator preferences.
[NIP-PRG](NIP-PRG.md) defines workflows, [NIP-EXT](NIP-EXT.md) distributes
components, and the [shared contracts](contracts.md) define identity,
references, schemas, effects, limits, and refusal behavior.

Capabilities describe general agent operations. Coding executors and workspace
probes are one specialization. Other bindings can read documents, query datasets,
or act on scoped service resources. The host must implement the domain's
identity, freshness, credentials, effect confirmation, and enforcement rules;
publishing the same interface does not make those guarantees interchangeable.

## Description, binding, and grant

| Object | Meaning | Authority |
| --- | --- | --- |
| Capability definition | A portable typed interface and execution requirements. | Describes; grants nothing. |
| Host binding | The native operation, executable adapter, guest host, or remote worker satisfying that interface. | Host-owned configuration outside an untrusted checkout. |
| Grant | Permission for a principal to use a binding within a scope and budget. | Host/operator policy, never supplied by the component itself. |
| Presence | The binding's observed usability in this context. | Local observation, never inferred from publication. |

A definition is portable even when its implementation is machine-specific.
Public definitions MUST NOT contain local credentials or private inventories.
A program requests an interface; a host resolves and records an eligible
binding. Operation descriptors support discovery without replacing admission.

## Kinds and addressing

| Kind | Record | Content |
| --- | --- | --- |
| `30180` | Capability discovery head | Public definition or immutable definition reference. |
| `30181` | Operator preference head | Public JSON or owner-encrypted preference document. |

Both are addressable by signer, kind, and exactly one `d` slug. Replacement
updates discovery, never an installed or admitted execution. Resolve and retain
the exact signed event and definition bytes under the shared reference/lock
contract. A release can carry a definition without publishing a separate head.

A public `30180` has `t` tags `oa:cap:v1` and
`oa:profile:<profile>`. Transport hints use `t: oa:transport:<transport>`.
Duplicate semantic tags or disagreement with the resolved definition refuse.

## Capability body

`v` is `1`; `requires` is the common feature list. A body is exactly one of:

- A definition with `id`, `profile`, `summary`, `input`, `output`, `effects`,
  `minimum`, `support`, and `binding_contract`.
- A head with `definition: DefinitionRef`; the fetched definition contains
  those fields, and its publisher identity agrees with the head.

`id` is publisher-qualified. `summary` is inert selector text covered by the
digest. `input` and `output` are SchemaRefs. `effects` uses common effects;
`minimum` is a bounds object. Optional `meta` is inert.

| Profile | Binding contract |
| --- | --- |
| `native` | Stable host `operation` ID and `interface` version. |
| `executor` | `interface`, `transport` (`subprocess`, `acp`, `http`, or `nostr-cj`), task/context SchemaRefs, and accepted `isolation` modes. |
| `plugin` | Exact plugin DefinitionRef and supported ABI/profile IDs from NIP-PRG. |
| `adapter` | `interface`, `transport` (`mcp`, `http`, `subprocess`, or `nostr-cj`), and explicit supported operation IDs. |

Executor/adapter contracts may include `remote`, an object with optional
`worker` (exact pubkey), `relays` (bounded WebSocket URL list), `endpoint`
(public HTTP URL), and `identity` (ArtifactRef for the serving identity).
`nostr-cj` requires worker and relays; HTTP requires endpoint. These are
verified connection hints, not grants or permission to disclose tenant-only
models. Host bindings pin and verify the actual serving identity before use.

A host need not support every profile. A plugin profile cannot acquire the
effects of an executor. An MCP listing cannot create a binding. Decision
services use explicit native/adapter bindings with recipient and spend policy;
a guest cannot perform unrecorded inference.

`support` contains `bounds`, `cancellation`, `idempotency`, and `evidence`:

- `bounds` maps each stated common bound to `enforced`, `not_enforced`, or
  `unknown`. Omitted bounds are `unknown`.
- `cancellation` is `before_dispatch`, `cooperative`, `host_terminated`, or
  `unsupported`; it describes a mechanism, not guaranteed remote stop.
- `idempotency` is `none`, `request_attempt`, or `pure`, evaluated for the
  actual binding and inputs.
- `evidence` lists supported versioned receipt/result schema IDs.

## Binding and enforcement

A local binding records definition digest, implementation identity, adapters,
supported assurance, scope mappings, probe approval, and configuration digest.
Secrets remain outside receipts; record opaque references, not credentials.

Subprocess/ACP bindings can declare `detect`, `invoke`, `invoke_writing`, and
`workspace_probe` as fixed argv arrays. These are local binding fields, not
portable instructions to execute. Resolve executable and argument-file paths
to canonical identities and pin their contents. `invoke_writing` requires
`invoke`. Never concatenate an argv into a shell string. A binding without
an invocation contract may be inspected but MUST NOT receive work.

For each required bound the host constructs an enforcement plan. Executor
`not_enforced` or `unknown` cannot satisfy a requirement by itself. A host
may supply an independently enforced mechanism such as a supervisor deadline
or filesystem boundary. Missing coverage at the required assurance refuses.
A signed claim does not prove enforcement. Read isolation, write isolation,
network confinement, and spend accounting are distinct properties; an isolated
writing worktree does not establish all four.

Record effective limits/grants before dispatch. Recheck revocation, binding
identity, source freshness, and reservations at dispatch. Preference, semantic
confidence, and publisher signatures cannot override admission. Stricter host
limits are permitted and recorded.

## Discovery and probes

Reading a definition MUST NOT execute probes, install packages, start adapters,
contact models, or mint grants. An executable probe requires a host-owned
approval outside the checkout naming exact definition, binding, executable,
and argument-file identities. Resolve them again in the intended directory;
changed bytes or retargeted paths invalidate approval.

Bound probes by time, output, and effects through the host supervisor. A probe
MUST NOT start paid or effectful task execution. A typed response declares presence directly. A
textual workspace probe uses declared refusal/acceptance matches, with
refusal precedence; unexplained nonzero exit, timeout, or excess output is
unknown. A probe approval is not approval for subsequent delegated work.

| Presence | Meaning |
| --- | --- |
| `present` | Approved observation establishes usability in this context. |
| `absent` | No implementation is installed/resolvable. |
| `unavailable` | Installed but explicitly refuses this context. |
| `unprobed` | No valid approval or observation; no probe executed. |
| `unknown` | Observation could not establish usability. |

Only `present` can become a route, with separate admission. Cached observations
bind definition, implementation, resource/account scope, and freshness identity;
the workspace is that scope for a coding binding. Local
presence MUST NOT appear in public heads. Deliberate encrypted fleet inventory
requires a separately consented application.

## Operator policy

A preference body has `v: 1`, `requires`, `prefer`, `deny`, `ceilings`,
`assurance`, `disclosure_policy`, and optional `meta`. `prefer` orders qualified
definition IDs; `deny` lists those IDs; `ceilings` uses common bounds.
`assurance` is `host_enforced` or `trusted_contract`. `disclosure_policy` is
an ArtifactRef evaluated by a supported host policy engine. Unsupported policy
semantics refuse; policy prose is not executable.

Preferences break ties among admissible bindings; they grant nothing. An
operator pin still undergoes admission. Hard known conflicts cannot be
overridden by semantic independence judgments. No policy requires inference
when mechanical admission already settles the question.

Public policies have `t: oa:cap-policy:public:v1`. Private policies have
`t: oa:cap-policy:private:v1`, one `p` equal to the signer, random 64-hex `d`,
and NIP-44 v2 self-encrypted content. A conforming relay MUST exclude them
from search and restrict reads, COUNT, and fanout to that authenticated key.
Private tags must not leak machine names. Sync to another principal requires
explicit re-encryption and its own local grants; an owner signature is not
automatic permission on every device.

## Optimization and inference capabilities

An optimizer, compiler/exporter, evaluator, or compiled inference operation
uses the same native, adapter, or executor profiles. The host MUST distinguish
the role and supported input/output schemas in its operation definitions;
hosting an optimizer does not imply support for its compiled output.

For [OPT](NIP-OPT.md), bindings record compiler/exporter identity, supported
search surfaces, inference strategies, model recipients, and the mechanisms
that isolate candidates from protected evaluation data. A remote declaration
is an assurance claim to evaluate under policy, not proof of isolation.

Proposal, reflection, student inference, grading, builds, and tools all use
admitted bindings and shared reservations. An evaluation-only grant cannot
authorize training, export, or deployment. A compiled implementation cannot
change its model recipient, invoke an undeclared tool, or extend its effects
because its optimizer selected it. A changed functional binding requires a
new pin and scoped evaluation; cache/probe state cannot certify its quality.

## Relationships and conformance

NIP-89 describes event handlers; NIP-AP describes personas. Neither grants
task execution. NIP-OA/NIP-AA establish relationships that a host may consume,
not universal operator authority. NIP-CJ binds remote invocation to an exact
signer and worker. NIP-EXT binds definitions to immutable releases.

Required fixtures cover all profiles, unsupported bounds, changed executable
and argument files, missing/stale approvals, five presence states, conflicting
effects, host-versus-executor enforcement, private-policy ACLs, and replacement
without automatic updates. Both interfaces must use the same admission path.

Relays advertise `nip-cap-v1` only for conforming validation/discovery and
configured privacy behavior. Host admission is separate.
