# @openagentsinc/blueprint-contracts

Canonical, single-source home for the **Blueprint contract-export security
contract** — the narrow security-critical slice only:

- the `BlueprintContractExportSeed` shape and its catalog entry schemas, and
- the `IsPrivateDataSafe` private-data-safety predicate family
  (`isBlueprintProjectionPrivateDataSafe`,
  `blueprintContractExportSeedIsPrivateDataSafe`, `blueprintPrivateFieldKey`,
  `sanitizeBlueprintProjection`) that decides whether a Blueprint projection or
  contract-export seed is safe to expose.

This package is the ONE authority for the `IsPrivateDataSafe` predicate. It
previously existed as drifted copies:

1. `apps/openagents.com/workers/api/src/blueprint/exports/contract-export.ts` —
   a WEAK regex on `JSON.stringify(seed)` checking only a handful of fields.
2. `apps/pylon/packages/runtime/src/blueprint/contracts.ts` — the stronger
   recursive field+value walk.
3. `packages/probe/packages/runtime/src/blueprint/contracts.ts` — the same
   recursive walk (which had itself further drifted from Pylon's copy on the
   surrounding data-model types).

The stronger recursive predicate is now the single authority here; the
consumers re-export it. The weak regex variant is deleted. The drift guard
(`scripts/check-contract-drift.mjs`, wired into `check:architecture`) fails the
build if a duplicate `IsPrivateDataSafe` authority is reintroduced.

## Scope

This is intentionally NARROW. It does NOT absorb the full Blueprint data model
(`BlueprintProgramType`, registry projections, Tassadar/Replay module bindings,
etc.) — those stay in the Probe/Pylon runtime `blueprint/contracts.ts`, which
now imports the security predicate from here.
