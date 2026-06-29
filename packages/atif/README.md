# @openagentsinc/atif

Canonical in-repo home for **ATIF — the Agent Trajectory Interchange Format**
(ATIF-v1.7) so trace producers and consumers stop maintaining parallel
definitions (#6207, epic #6206).

Reference: `projects/repos/harbor/rfcs/0001-trajectory-format.md` (ATIF-v1.7);
spec `docs/traces/README.md`.

## Surfaces

| Import | What |
| --- | --- |
| `@openagentsinc/atif/trace` | The strict, pinned ingest/store **schema** (`AtifTrajectory` Effect-Schema class), `decodeAtifTrajectorySync`/`encodeAtifTrajectory`, the structural `validateAtifTrajectory`, and the value-based public-safety `atifTraceTripwire` (+ finding/issue types). The canonical trace contract; the API worker re-exports it. |
| `@openagentsinc/atif/emit` | Producer-facing dependency-free TypeScript types a trajectory emitter builds (`AtifTrajectory` interface, `AtifStep`, `Json`, `AtifVerdict`, `serializeTrajectory`, …). |
| `@openagentsinc/atif/validate` | Producer-facing (permissive) Effect-Schema validator: `AtifTrajectorySchema`, `validateAtif`, `assertValidAtif`, `AtifValidationError`. |

The barrel (`@openagentsinc/atif`) re-exports the trace surface plus the
validator names. The emitter TS types collide by name with the trace schema
classes (both expose `AtifTrajectory`, `AtifStep`, …), so import those from the
`/emit` subpath directly.

## Pinned version

`ATIF_PINNED_SCHEMA_VERSION` / `ATIF_SCHEMA_VERSION` = `ATIF-v1.7`. A producer
emitting a different revision is rejected at the boundary rather than silently
stored.

## Public-safe subset

We store and serve a **public-safe projection** of an ATIF trajectory — never
raw secrets, tokens, wallet/payment material, PII, local paths, or
raw/split provider payloads. `atifTraceTripwire` is the value-based backstop
(the redaction service is the primary scrubber). Note: a model id in a trace is
session **content**, not a leak, so it is allowed — the "openagents/khala only"
rule is a Khala gateway-projection invariant, not a trace one.
