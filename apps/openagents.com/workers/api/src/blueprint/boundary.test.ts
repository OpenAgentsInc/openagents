import { describe, expect, test } from 'vitest'

import {
  BLUEPRINT_KERNEL_BOUNDARY,
  blueprintKernelHasDeprecatedDependency,
  blueprintKernelModuleRefs,
  blueprintKernelModulesByAuthority,
} from './boundary'

describe('Blueprint kernel boundary', () => {
  test('declares Omega ownership with no deprecated Blueprint dependency', () => {
    expect(BLUEPRINT_KERNEL_BOUNDARY.ownerRef).toBe('omega')
    expect(blueprintKernelHasDeprecatedDependency()).toBe(false)
    expect(BLUEPRINT_KERNEL_BOUNDARY.kernelRef).toBe(
      'omega.blueprint.kernel.v1',
    )
  })

  test('declares source, repository, service, projection, export, fixture, and docs modules', () => {
    expect(blueprintKernelModuleRefs()).toEqual([
      'workers/api/src/blueprint/schemas',
      'workers/api/src/blueprint/repositories',
      'workers/api/src/blueprint/services',
      'workers/api/src/blueprint/projections',
      'workers/api/src/blueprint/exports',
      'workers/api/src/blueprint/fixtures',
      'docs/blueprint',
    ])
  })

  test('keeps exports export-only and write-side services approval-gated', () => {
    expect(
      blueprintKernelModulesByAuthority('export_only').map(
        module => module.kind,
      ),
    ).toEqual(['export', 'documentation'])
    expect(
      blueprintKernelModulesByAuthority('approval_gated').map(
        module => module.kind,
      ),
    ).toEqual(['service'])
  })
})
