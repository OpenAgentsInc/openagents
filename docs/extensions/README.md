# OpenAgents programs and extensions

Status: target specification, partly implemented. The Wasm plugin host
core and the program `program` and `module` steps are built;
[What is built](plugins.md#what-is-built) lists the plugin parts.
OpenAgents defines general agent infrastructure; Coder is its first
specialization. Programs compose typed work. Extensions
distribute programs, operations, Wasm plugins, skills, semantic AI signatures,
and immutable AI implementations. Every component has an explicit identity,
interface, effect boundary, and lifecycle.

The [AI programming design](../optimization/README.md) separates what an
operation means from how a model-backed implementation realizes it. DSPy and
GEPA can help search implementation choices. Gym and domain evaluators measure
them; host policy controls adoption and all execution authority.

## Implemented entry points

Use [capabilities and programs](../programs.md) for the original operational
path, [program authority](../coder/guides/program-authority.md) for execution
grants, and [the Wasm host inventory](plugins.md#what-is-built) for supported
profiles and limits. The local package resolver is described in
[packages](packages.md); a signed catalog, automatic import, and general
optimization lifecycle are wider targets.

The [migration tracker](../coder/migration-status.md) and
[master roadmap](../roadmap.md) own current delivery priorities. An EXT document,
installed plugin, or benchmark result does not silently activate another
component or grant a provider access.

## Read the specification

| Document | Scope |
| --- | --- |
| [Architecture and terminology](architecture.md) | Component meanings, ownership, discovery, and authority. |
| [Programs](programs.md) | Workflow selection, typed composition, decision functions, and AI implementations. |
| [Wasm plugins](plugins.md) | Guest ABI, bounded imports, evidence transformations, and build provenance. |
| [Packages and distribution](packages.md) | Immutable contents, identity, installation, publication, revocation, and adoption. |
| [TypeSafe opportunities](opportunities.md) | Concrete applications of explicit state and economical semantic operations. |
| [Delivery and evaluation](delivery.md) | Implementation sequence and acceptance requirements. |
| [Optimization architecture](../optimization/architecture.md) | Stable semantic contracts and replaceable inference implementations. |

The [OpenAgents NIPs](../../nips/openagents/README.md) are standalone v1
protocol specifications. They govern wire encoding, identity, validation,
effects, and interoperation. These documents explain product and host design.
A specification does not imply an implemented runtime or measured benefit.

## Programs, plugins, and AI implementations

A program is a workflow, not a model prompt. A Wasm plugin is a bounded
executable guest, not every kind of extension. An AI signature describes
semantic behavior; an AI implementation supplies a pinned realization through
a decision function, program, or admitted operation. An extension package can
contain several of these components without merging their authority.

Discovery, installation, selection, grants, execution admission, and promotion
are separate actions. Optimizer output is a candidate until evaluation and
operator policy establish where it may be adopted. Installed bytes remain inert.

## Domain scope

Coding profiles supply repository sources, compiler/test tools, patch schemas,
and coding acceptance. Document or research profiles supply different sources
and checkers through the same contracts. Neither the package format nor the
optimization lifecycle requires a repository, shell, or terminal.

The [general architecture](../agents/README.md) defines domain responsibilities.
The [complete opportunity map](../optimization/architecture.md#map-the-typesafe-opportunities-to-learnable-behavior)
covers context, routing, tool discovery, parallel work, hierarchy, and background
assistance. The [unfiled proposals](../optimization/proposed-issues.md) define
full integration and the evidence required for each delivery slice.
