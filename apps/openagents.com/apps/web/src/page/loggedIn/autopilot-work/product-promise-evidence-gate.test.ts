import { describe, expect, test } from 'vitest'

import {
  type ForgeProductPromiseEvidenceGateInput,
  projectForgeProductPromiseEvidenceGate,
} from './product-promise-evidence-gate'

const completeInput = (
  overrides: Partial<ForgeProductPromiseEvidenceGateInput> = {},
): ForgeProductPromiseEvidenceGateInput => ({
  claimRefs: ['claim.public.5107.exact_surface'],
  deployRefs: ['deploy.public.openagents.2026_06_17'],
  freshness: 'fresh',
  gateRef: 'product-promise-evidence-gate.public.5107',
  generatedAt: '2026-06-17T00:30:00.000Z',
  liveSmokeRefs: ['live-smoke.public.openagents.5107'],
  productPromiseRefs: ['product-promise.public.forge_terminal_agent_systems'],
  publicSafetyRefs: ['public-safety.public.private_material_regressions'],
  signatureRefs: ['signature.public.release_manager'],
  ...overrides,
})

describe('Forge product-promise evidence gate', () => {
  test('is ready only when exact claim, deploy, live-smoke, signature, and safety refs exist', () => {
    const gate = projectForgeProductPromiseEvidenceGate(completeInput())

    expect(gate).toMatchObject({
      authority: {
        deploymentAuthority: false,
        productPromiseWriteAuthority: false,
        publicClaimAuthority: false,
        registryMutationAuthority: false,
        settlementAuthority: false,
      },
      counts: {
        claimRefs: 1,
        deployRefs: 1,
        liveSmokeRefs: 1,
        productPromiseRefs: 1,
        publicSafetyRefs: 1,
        signatureRefs: 1,
      },
      omittedUnsafeRefCount: 0,
      publicSafe: true,
      status: 'ready',
    })
    expect(gate.blockerRefs).toEqual([])
  })

  test('blocks product-promise evidence updates when required evidence is missing', () => {
    const gate = projectForgeProductPromiseEvidenceGate(
      completeInput({
        deployRefs: [],
        liveSmokeRefs: [],
        productPromiseRefs: [],
        publicSafetyRefs: [],
        signatureRefs: [],
      }),
    )

    expect(gate.status).toBe('blocked')
    expect(gate.blockerRefs).toEqual([
      'forge-product-promise-evidence-gate-blocker:product-promise-evidence-gate.public.5107:missing-product-promise-ref',
      'forge-product-promise-evidence-gate-blocker:product-promise-evidence-gate.public.5107:missing-deploy-ref',
      'forge-product-promise-evidence-gate-blocker:product-promise-evidence-gate.public.5107:missing-live-smoke-ref',
      'forge-product-promise-evidence-gate-blocker:product-promise-evidence-gate.public.5107:missing-signature-ref',
      'forge-product-promise-evidence-gate-blocker:product-promise-evidence-gate.public.5107:missing-public-safety-ref',
    ])
  })

  test('keeps stale evidence distinct from ready evidence', () => {
    const gate = projectForgeProductPromiseEvidenceGate(
      completeInput({
        freshness: 'stale',
      }),
    )

    expect(gate.status).toBe('stale')
    expect(gate.blockerRefs).toEqual([])
  })

  test('omits unsafe private evidence before projection', () => {
    const gate = projectForgeProductPromiseEvidenceGate(
      completeInput({
        blockerRefs: ['private repo content /Users/christopher/src/openagents'],
        claimRefs: [
          'claim.public.5107.exact_surface',
          'raw prompt /Users/christopher/private.md',
        ],
        deployRefs: [
          'deploy.public.openagents.2026_06_17',
          'raw shell command $(cat secret)',
        ],
        liveSmokeRefs: [
          'live-smoke.public.openagents.5107',
          'raw transcript /Users/christopher/private.jsonl',
        ],
        publicSafetyRefs: [
          'public-safety.public.private_material_regressions',
          'provider payload sk-private',
        ],
        signatureRefs: ['signature.public.release_manager', 'bearer token private'],
      }),
    )
    const payload = JSON.stringify(gate)

    expect(gate.status).toBe('blocked')
    expect(gate.omittedUnsafeRefCount).toBe(6)
    expect(gate.claimRefs).toEqual(['claim.public.5107.exact_surface'])
    expect(gate.deployRefs).toEqual(['deploy.public.openagents.2026_06_17'])
    expect(gate.liveSmokeRefs).toEqual(['live-smoke.public.openagents.5107'])
    expect(gate.blockerRefs).toContain(
      'forge-product-promise-evidence-gate-blocker:product-promise-evidence-gate.public.5107:unsafe-product-promise-evidence-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw prompt')
    expect(payload).not.toContain('raw shell')
    expect(payload).not.toContain('raw transcript')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('sk-private')
    expect(payload).not.toContain('bearer token')
  })
})
