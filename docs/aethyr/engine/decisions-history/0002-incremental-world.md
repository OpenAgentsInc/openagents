> Deprecated (owner, 2026-10-24): history only. Architecture decisions live in [`docs/adr/`](../../../adr/README.md).

# 0002: Compile semantic worlds incrementally

Status: selected, 2026-10-22.

## Decision

The source of an editable building is a stable structural graph. Graphics,
collision, navigation, selection, and streaming data derive from one document
digest and activate as one complete compatible generation. Unchanged artifacts
are shared by identity; a changed wall does not require copying the whole world.

Most creator changes update data. Engine code, behavior source, and material
source have separate explicit compilation and migration paths. Compatible live
state survives; invalid/stale candidates cannot replace the active world.

## Rationale and rejected alternatives

An opaque mesh cannot establish a room's editable topology or traversability.
Separate hand-authored visual/collision models drift during live changes. A
complete application rebuild for every edit prevents immediate iteration.
Unrestricted hot swapping every resource independently can produce a visible
doorway with a stale collider. None is accepted as the production contract.

Semantic reconstruction of imports is explicit, reviewed, and capability-labeled.
The compiler's dependency graph must prove changed-output closure and bound peak
residency. Invalid source may be saved as a draft while active play remains on
the last complete generation; the UI distinguishes these states.

## Revisit

Investigate finer activation units only when dependencies/seams preserve the same
cross-output consistency. Never weaken the invariant to improve a benchmark.
See [world model](../world-model.md), [compiler](../asset-compilation.md), and
[contracts](../contracts.md).
