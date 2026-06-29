# @openagentsinc/provider-account-schema

Canonical, single-source home for the **provider-account security contract**:
the `ProviderSecretRef` brand and the secret-detection / public-projection
safety predicates that decide what provider-credential material is safe to
expose.

This package is the ONE authority for these contracts. It previously existed as
three hand-maintained copies that silently drifted (a secret-leak hazard):

1. `apps/openagents.com/packages/provider-account-schema` (web contract)
2. `apps/pylon/packages/runtime/src/contracts/provider-account.ts`
3. `packages/probe/packages/runtime/src/contracts/provider-account.ts`

Those copies are now thin re-exports of this package. The drift guard
(`scripts/check-contract-drift.mjs`, wired into `check:architecture`) fails the
build if a duplicate contract authority is reintroduced.

## Entry points

- `@openagentsinc/provider-account-schema` — the `openagents.com` Worker web
  contract surface: `ProviderSecretRef`, `containsProviderSecretMaterial`,
  `isPublicSecretReference`, `requirePublicSecretReference`,
  `sanitizeProviderAccountText`, `redactProviderAccountSecretMaterial`,
  `assertNoProviderSecretMaterial`, the `PublicProviderAccount*` Schema classes,
  and the OpenAI provider-payload decoders.
- `@openagentsinc/provider-account-schema/runtime` — the Probe/Pylon runtime
  contract surface: `isPublicSecretRef`, `containsSecretMaterial`,
  `validateProbePublicProjection`, `sanitizeProbePublicProjection`,
  `canIssueProviderAccountGrant`, `canSelectProviderAccountForLease`, and the
  `PublicProviderAccount` / `ProbeAuthGrantRequest` Schemas.

Both surfaces share the SAME `ProviderSecretRef` brand authority (`runtime`
re-uses the brand defined in the package root), so the nominal type is
identical everywhere.

## Publishability

Published under the `@openagentsinc` scope, `effect` resolved via `catalog:`.
Pylon (`@openagentsinc/pylon-runtime`) takes this as a `workspace:*` dependency
— the same pattern Pylon already uses for `@openagentsinc/agent-runtime-schema`
— so `bun pm pack` rewrites it to the concrete version on publish. See
`apps/pylon/docs/npm-publishing-runbook.md`.
