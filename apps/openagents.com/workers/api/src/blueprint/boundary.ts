import { Schema as S } from 'effect'

export const BlueprintKernelModuleKind = S.Literals([
  'schema',
  'service',
  'repository',
  'projection',
  'export',
  'fixture',
  'documentation',
])
export type BlueprintKernelModuleKind = typeof BlueprintKernelModuleKind.Type

export const BlueprintKernelAuthorityMode = S.Literals([
  'evidence_only',
  'approval_gated',
  'export_only',
])
export type BlueprintKernelAuthorityMode =
  typeof BlueprintKernelAuthorityMode.Type

export const BlueprintKernelModule = S.Struct({
  authorityMode: BlueprintKernelAuthorityMode,
  descriptionRef: S.String,
  kind: BlueprintKernelModuleKind,
  moduleRef: S.String,
})
export type BlueprintKernelModule = typeof BlueprintKernelModule.Type

export const BlueprintKernelBoundary = S.Struct({
  deprecatedDependencyAllowed: S.Boolean,
  kernelRef: S.String,
  modules: S.Array(BlueprintKernelModule),
  ownerRef: S.String,
  publicDocsRef: S.String,
})
export type BlueprintKernelBoundary = typeof BlueprintKernelBoundary.Type

export const BLUEPRINT_KERNEL_BOUNDARY: BlueprintKernelBoundary = {
  deprecatedDependencyAllowed: false,
  kernelRef: 'omega.blueprint.kernel.v1',
  modules: [
    {
      authorityMode: 'evidence_only',
      descriptionRef: 'blueprint.boundary.schemas',
      kind: 'schema',
      moduleRef: 'workers/api/src/blueprint/schemas',
    },
    {
      authorityMode: 'evidence_only',
      descriptionRef: 'blueprint.boundary.repositories',
      kind: 'repository',
      moduleRef: 'workers/api/src/blueprint/repositories',
    },
    {
      authorityMode: 'approval_gated',
      descriptionRef: 'blueprint.boundary.services',
      kind: 'service',
      moduleRef: 'workers/api/src/blueprint/services',
    },
    {
      authorityMode: 'evidence_only',
      descriptionRef: 'blueprint.boundary.projections',
      kind: 'projection',
      moduleRef: 'workers/api/src/blueprint/projections',
    },
    {
      authorityMode: 'export_only',
      descriptionRef: 'blueprint.boundary.exports',
      kind: 'export',
      moduleRef: 'workers/api/src/blueprint/exports',
    },
    {
      authorityMode: 'evidence_only',
      descriptionRef: 'blueprint.boundary.fixtures',
      kind: 'fixture',
      moduleRef: 'workers/api/src/blueprint/fixtures',
    },
    {
      authorityMode: 'export_only',
      descriptionRef: 'blueprint.boundary.docs',
      kind: 'documentation',
      moduleRef: 'docs/blueprint',
    },
  ],
  ownerRef: 'omega',
  publicDocsRef:
    'docs/blueprint/2026-06-05-omega-blueprint-package-boundary.md',
}

export const blueprintKernelModuleRefs = (
  boundary: BlueprintKernelBoundary = BLUEPRINT_KERNEL_BOUNDARY,
): ReadonlyArray<string> => boundary.modules.map(module => module.moduleRef)

export const blueprintKernelHasDeprecatedDependency = (
  boundary: BlueprintKernelBoundary = BLUEPRINT_KERNEL_BOUNDARY,
): boolean => boundary.deprecatedDependencyAllowed

export const blueprintKernelModulesByAuthority = (
  authorityMode: BlueprintKernelAuthorityMode,
  boundary: BlueprintKernelBoundary = BLUEPRINT_KERNEL_BOUNDARY,
): ReadonlyArray<BlueprintKernelModule> =>
  boundary.modules.filter(module => module.authorityMode === authorityMode)
