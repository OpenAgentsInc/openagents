import { describe, expect, test } from 'vitest'

import {
  AGENTIC_NPM_COMPOSITION_RUNTIME_SCHEMA,
  AGENTIC_NPM_MODULE_REGISTRY_PROMISE,
  AgenticNpmCompositionUnsafe,
  resolveAgenticNpmComposition,
  type AgenticNpmModuleSpec,
} from './agentic-npm-composition-runtime'

const mod = (
  partial: Pick<AgenticNpmModuleSpec, 'moduleRef'> &
    Partial<AgenticNpmModuleSpec>,
): AgenticNpmModuleSpec => ({
  version: '1.0.0',
  moduleDigest: `sha256:${partial.moduleRef.replace(/[^a-z0-9]/g, '0')}`,
  replayVerified: true,
  compositionVerified: true,
  linkCompatibilityVerified: true,
  provides: [],
  dependsOn: [],
  requiresInterfaces: [],
  ...partial,
})

describe('resolveAgenticNpmComposition', () => {
  test('resolves a verified DAG into a deterministic topological plan', async () => {
    const registry: ReadonlyArray<AgenticNpmModuleSpec> = [
      mod({ moduleRef: 'module.app', dependsOn: ['module.lib'], provides: ['iface.app'] }),
      mod({ moduleRef: 'module.lib', dependsOn: ['module.base'], provides: ['iface.lib'] }),
      mod({ moduleRef: 'module.base', provides: ['iface.base'] }),
    ]

    const plan = await resolveAgenticNpmComposition({
      registry,
      requestedRootRefs: ['module.app'],
    })

    expect(plan.schema).toBe(AGENTIC_NPM_COMPOSITION_RUNTIME_SCHEMA)
    expect(plan.promiseId).toBe(AGENTIC_NPM_MODULE_REGISTRY_PROMISE)
    expect(plan.promiseState).toBe('planned')
    expect(plan.composable).toBe(true)
    expect(plan.blockerRefs).toEqual([])
    // Dependencies must come before dependents.
    expect(plan.resolved.map(m => m.moduleRef)).toEqual([
      'module.base',
      'module.lib',
      'module.app',
    ])
    expect(plan.resolved.map(m => m.order)).toEqual([0, 1, 2])
    // The resolver authorizes nothing.
    expect(plan.authority).toEqual({
      billingAuthority: false,
      executionAuthority: false,
      installAuthority: false,
      meteringAuthority: false,
      settlementAuthority: false,
    })
    expect(plan.inert).toBe(true)
  })

  test('is deterministic regardless of registry input order', async () => {
    const specs: ReadonlyArray<AgenticNpmModuleSpec> = [
      mod({ moduleRef: 'module.app', dependsOn: ['module.lib'] }),
      mod({ moduleRef: 'module.lib', dependsOn: ['module.base'] }),
      mod({ moduleRef: 'module.base' }),
    ]
    const planA = await resolveAgenticNpmComposition({
      registry: specs,
      requestedRootRefs: ['module.app'],
    })
    const planB = await resolveAgenticNpmComposition({
      registry: [...specs].reverse(),
      requestedRootRefs: ['module.app'],
    })

    expect(planB.planDigest).toBe(planA.planDigest)
    expect(planB.resolved).toEqual(planA.resolved)
  })

  test('gates the composition on verification-on-compose', async () => {
    const registry: ReadonlyArray<AgenticNpmModuleSpec> = [
      mod({ moduleRef: 'module.app', dependsOn: ['module.lib'] }),
      mod({ moduleRef: 'module.lib', replayVerified: false }),
    ]

    const plan = await resolveAgenticNpmComposition({
      registry,
      requestedRootRefs: ['module.app'],
    })

    expect(plan.composable).toBe(false)
    expect(plan.unverifiedModuleRefs).toEqual(['module.lib'])
    expect(plan.blockerRefs).toContain(
      'blocker.agentic_npm_composition.module_not_verified',
    )
  })

  test('reports missing modules', async () => {
    const plan = await resolveAgenticNpmComposition({
      registry: [mod({ moduleRef: 'module.app', dependsOn: ['module.gone'] })],
      requestedRootRefs: ['module.app'],
    })

    expect(plan.composable).toBe(false)
    expect(plan.missingModuleRefs).toEqual(['module.gone'])
    expect(plan.blockerRefs).toContain(
      'blocker.agentic_npm_composition.missing_module',
    )
  })

  test('detects dependency cycles', async () => {
    const registry: ReadonlyArray<AgenticNpmModuleSpec> = [
      mod({ moduleRef: 'module.a', dependsOn: ['module.b'] }),
      mod({ moduleRef: 'module.b', dependsOn: ['module.a'] }),
    ]

    const plan = await resolveAgenticNpmComposition({
      registry,
      requestedRootRefs: ['module.a'],
    })

    expect(plan.composable).toBe(false)
    expect(plan.cyclicModuleRefs).toEqual(['module.a', 'module.b'])
    expect(plan.blockerRefs).toContain(
      'blocker.agentic_npm_composition.dependency_cycle',
    )
    expect(plan.resolved).toEqual([])
  })

  test('flags an unsatisfied interface dependency', async () => {
    const registry: ReadonlyArray<AgenticNpmModuleSpec> = [
      mod({
        moduleRef: 'module.app',
        requiresInterfaces: ['iface.payments'],
        provides: ['iface.app'],
      }),
    ]

    const plan = await resolveAgenticNpmComposition({
      registry,
      requestedRootRefs: ['module.app'],
    })

    expect(plan.composable).toBe(false)
    expect(plan.unsatisfiedInterfaceRefs).toEqual(['iface.payments'])
    expect(plan.blockerRefs).toContain(
      'blocker.agentic_npm_composition.interface_unsatisfied',
    )
  })

  test('satisfies an interface dependency provided by another resolved module', async () => {
    const registry: ReadonlyArray<AgenticNpmModuleSpec> = [
      mod({
        moduleRef: 'module.app',
        dependsOn: ['module.pay'],
        requiresInterfaces: ['iface.payments'],
        provides: ['iface.app'],
      }),
      mod({ moduleRef: 'module.pay', provides: ['iface.payments'] }),
    ]

    const plan = await resolveAgenticNpmComposition({
      registry,
      requestedRootRefs: ['module.app'],
    })

    expect(plan.composable).toBe(true)
    expect(plan.unsatisfiedInterfaceRefs).toEqual([])
  })

  test('rejects unsafe refs that could leak credential/wallet material', async () => {
    await expect(
      resolveAgenticNpmComposition({
        registry: [mod({ moduleRef: 'module.app' })],
        requestedRootRefs: ['module.app', 'wallet.seed_phrase'],
      }),
    ).rejects.toBeInstanceOf(AgenticNpmCompositionUnsafe)
  })
})
