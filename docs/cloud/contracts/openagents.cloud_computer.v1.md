# `openagents.cloud_computer.v1`

Status: The provider-neutral logical computer contract is implemented. Capacity
admission, checkpoint storage, recoverable execution, provider adapters, and
production qualification remain owned by cloud computer issues 4 through 11.

Owning crate: `crates/openagents-cloud-contract`

## Purpose

`openagents.cloud_computer.v1` separates a durable logical workspace from a
temporary runtime lease. Creating a logical computer records identity and policy
only. A provider effect can start only after a later start or execute request
wins capacity admission.

This contract does not replace `openagents.managed_sandbox.v1`. That contract
describes the existing owner-scoped managed-sandbox program, whose create path
can provision a GCE runtime. Cloud computer callers use this contract when they
need cold inventory, queued admission, runtime classes, or one-shot batch work.

## Identity and lifecycle

Each `CloudComputer` binds these values before a mutation:

- Owner and tenant refs.
- Conversation or program ref and work-unit ref.
- Requested runtime profile and public runtime class.
- Authority and budget snapshot digests.
- Capability refs.
- Resource generation and optimistic version.

The lifecycle uses these transitions:

| Current state | Admitted next state | Meaning |
| --- | --- | --- |
| `cold` | `queued`, `destroyed` | The logical workspace has no runtime lease. |
| `queued` | `starting`, `cold` | Capacity admission is pending or withdrawn. |
| `starting` | `active`, `failed` | One provider received the admitted start request. |
| `active` | `stopping`, `failed` | The current generation owns one runtime lease. |
| `stopping` | `cold`, `failed` | Checkpoint and cleanup settle before the lease clears. |
| `failed` | `destroyed` | Recovery policy owns any retained checkpoint. |
| `destroyed` | none | Cleanup evidence is terminal. |

The generation advances when provider work can begin and when a stopped
computer returns to cold inventory. A command for an older generation fails
with `generation_mismatch` before a provider effect.

## Operations and retries

The closed operation vocabulary is create, list, inspect, start, execute,
attach, cancel, stop, checkpoint, restore, fork, and destroy. Every mutation
carries an idempotency key and a digest of the canonical request bytes.

The adapter reserves each canonical command before a provider effect. An exact
retry returns the first receipt, or a typed pending result while the first
effect remains unsettled. Reusing the key with different canonical command
bytes returns `idempotency_conflict`, even when the caller repeats a supplied
digest. The reservation namespace includes the operation and complete scope.

The adapter supplies the independently authenticated actor to command
validation. Matching two caller-supplied owner fields does not authorize a
command. The contract test creates 30 logical computers and proves that each
stays `cold`, has no runtime lease, and has no effective provider.

## Provider boundary

`CloudComputerProvider` represents these adapters:

- Pooled Firecracker for `strong` interactive work.
- GKE Agent Sandbox for `standard` interactive work.
- Dedicated GCE for an admitted compatibility or isolation requirement.
- Cloud Run for `batch` one-shot work.

The product request contains `standard`, `strong`, or `batch`. Server policy
selects a provider. The provider publishes a capability matrix, and admission
must refuse `unsupported_capability` when the selected provider cannot satisfy
the requested class and required capabilities. The control plane must not
substitute another provider after this refusal.

`ComputerKind` distinguishes `interactive_retained` from `one_shot_batch`.
This prevents a batch execution receipt from claiming attach, stop, or retained
runtime semantics.

## Public receipts

Receipts may expose the effective provider, image digest, policy digest,
checkpoint digest, usage digest, cleanup digest, typed error, and observation
time. Public serialization rejects fields that name credentials, secrets,
private paths, guest addresses, raw provider payloads, access tokens, or cloud
administration handles.

Provider-specific resource names, regions, addresses, service accounts,
credentials, and raw payloads remain private adapter state. A digest is
evidence identity; it does not disclose the underlying private value.

Events and receipts use validated serializers. Public refs use a closed,
lowercase identifier alphabet and exclude addresses, paths, secret or token
labels, and cloud administration prefixes. Digests use `sha256:` followed by
exactly 64 lowercase hexadecimal characters. Provider checkpoint, cleanup, and
execution requests bind the stored active lease ref, scope, and generation
before the trait method can run.

## Schemas and verification

- Rust types and the executable lifecycle model:
  `crates/openagents-cloud-contract/src/cloud_computer_v1.rs`
- JSON Schema for the durable resource:
  `crates/openagents-cloud-contract/schema/cloud_computer.v1.schema.json`
- Verification: `cargo test -p openagents-cloud-contract`

The Phoenix adapter and runtime agents must exchange these canonical values.
They can add transport envelopes, pagination, and authentication, but they must
not reinterpret lifecycle state, generation, idempotency, runtime class, or
provider capability refusal.
