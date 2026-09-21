# Coder programs and extensions

Status: target specification, 2026-09-21. This directory adapts the plugin
system documented in the reference Coder repository to OpenAgents' TypeSafe
coding-agent architecture. It specifies new behavior; it does not install a
plugin host, enable a package, or claim a measured improvement.

The product is a **program and extension system**. Programs define reusable
workflows. Plugins provide bounded WebAssembly operations within those
workflows. Skills provide scoped guidance. Native and external adapters connect
host capabilities. A package distributes these components together without
making their execution or permission models interchangeable.

A developer should be able to install a Rust investigation package, ask Coder
to diagnose a failure, and inspect which workflow ran, what evidence it used,
what it changed, and what verified the result. The package's whole manual and
tool catalog should not occupy every generation request.

## Read the specification

| Document | Contract |
| --- | --- |
| [Architecture and terminology](architecture.md) | Component boundaries, operation descriptors, discovery, authority, and scoped activation. |
| [Programs and decisions](programs.md) | Program selection, typed dataflow, execution, composition, budgets, recovery, and verification. |
| [Wasm plugins](plugins.md) | Guest manifests, ABI, sandbox, host roles, evidence transformations, and authoring. |
| [Packages and distribution](packages.md) | Identity, dependencies, trust, installation, publication, updates, revocation, and offline operation. |
| [TypeSafe opportunities](opportunities.md) | How the system realizes the source proposal, with concrete workflows and falsifiable claims. |
| [Delivery and evaluation](delivery.md) | Current implementation, migration from Coder, acceptance, evaluation, and source provenance. |

These documents use **must** for requirements of the proposed implementation.
They are not a new wire protocol. Existing
[NIP-PRG](../../nips/openagents/NIP-PRG.md),
[NIP-CAP](../../nips/openagents/NIP-CAP.md), and the
[Decision API](../decision-models/api/decision-api.md) retain their contracts.
New serialized fields, bindings, ABI support, and protocol extensions require
versioned schemas and conformance fixtures before they can be advertised.
Unknown execution semantics must refuse rather than run a partial workflow.

## How this fits the Coder plan

The [source proposal](../coder/design/thoughts-on-a-typesafe-coding-agent.md)
and its [retained exports and images](../coder/thoughts-on-a-typesafe-coding-agent/)
make explicit state the central opportunity. The
[analysis](../coder/design/typesafe-agent-analysis.md) and
[roadmap](../coder/design/typesafe-agent-roadmap.md) turn that into evidence,
context, coding operations, routing, shared work, and durable programs.
This specification supplies the extension contracts for those phases.

The existing [program guide](../programs.md) remains the operational and
historical reference. This directory clarifies its terminology and supplies
the target integration specification. It does not replace the five existing
programs or silently upgrade their schema. The original source document,
experiment records, and transcript archive remain unchanged.

## Current implementation boundary

At OpenAgents revision `2ddabaaedd93ce085d572ccdea7ebd8c3d18fc0d`:

- Coder reads local capabilities, programs, questions, and sources. Its shared
  runtime supports `query`, `check`, `decide`, and `delegate` steps.
- Program grants, approved executable probes, bounded delegation, project
  claims, repository source references, and protected artifact checks exist.
- `run-suite` and `review-changes` require protected host verification inputs;
  their manifests alone do not make them ordinary chat operations.
- A standalone run-state store records pinned run/step/attempt identities and
  marks unfinished records unknown during recovery. Runtime integration and
  reconciliation remain work.
- General evidence/context storage, progressive operation discovery, portable
  packages, complete program recovery, and typed child composition remain work.
- `program` and `module` steps are recognized but refused by the current
  runtime. The reference Coder Wasm host, PDK, catalog, and plugin UI are not
  implemented here.

[Delivery and evaluation](delivery.md) separates the inherited design from
implemented behavior and gives each increment its own completion evidence.
