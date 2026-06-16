import { describe, expect, test } from 'vitest'

import {
  type ForgeRepositoryProfileRefreshInput,
  projectForgeRepositoryProfileRefreshReceipt,
} from './repository-profile-refresh'

const baseInput = (
  overrides: Partial<ForgeRepositoryProfileRefreshInput> = {},
): ForgeRepositoryProfileRefreshInput => ({
  generatedAt: '2026-06-16T19:00:00.000Z',
  refreshedAt: '2026-06-16T18:55:00.000Z',
  workOrderRef: 'work_1',
  ...overrides,
})

describe('Forge repository profile refresh receipt projection', () => {
  test('projects a fresh no-change refs-only receipt', () => {
    const receipt = projectForgeRepositoryProfileRefreshReceipt(
      baseInput({
        commandProfileRefs: ['profile.command.public.package_scripts.sha256_abcd'],
        instructionRefs: ['profile.instruction.public.AGENTS.sha256_abcd'],
        invariantRefs: ['profile.invariant.public.INVARIANTS.sha256_abcd'],
        repoIdentityRefs: ['repo.github.OpenAgentsInc.openagents', 'branch.main'],
        testProfileRefs: ['profile.test.public.vitest.sha256_abcd'],
      }),
    )

    expect(receipt).toMatchObject({
      authority: {
        commandExecutionAuthority: false,
        invariantPolicyAuthority: false,
        repositoryScanProof: false,
        testExecutionAuthority: false,
      },
      changedProfileKinds: [],
      freshness: 'fresh',
      omittedUnsafeRefCount: 0,
      provenance: 'refs_only_repository_profile_refresh',
      publicSafe: true,
      receiptKind: 'forge_repository_profile_refresh.v1',
      status: 'fresh',
      workOrderRef: 'work_1',
    })
    expect(receipt.commandProfileRefs).toEqual([
      'profile.command.public.package_scripts.sha256_abcd',
    ])
    expect(receipt.testProfileRefs).toEqual([
      'profile.test.public.vitest.sha256_abcd',
    ])
    expect(receipt.blockerRefs).toEqual([])
  })

  test('marks changed stale profile refresh receipts', () => {
    const receipt = projectForgeRepositoryProfileRefreshReceipt(
      baseInput({
        changedProfileKinds: ['test', 'command', 'test'],
        commandProfileRefs: ['profile.command.public.package_scripts.sha256_new'],
        refreshedAt: '2026-06-15T18:00:00.000Z',
        repoIdentityRefs: ['repo.github.OpenAgentsInc.openagents'],
        testProfileRefs: ['profile.test.public.vitest.sha256_new'],
      }),
    )

    expect(receipt.status).toBe('stale')
    expect(receipt.freshness).toBe('stale')
    expect(receipt.changedProfileKinds).toEqual(['command', 'test'])
    expect(receipt.blockerRefs).toEqual([])
  })

  test('reports blockers when repository profile evidence is missing', () => {
    const receipt = projectForgeRepositoryProfileRefreshReceipt(
      baseInput({
        refreshedAt: null,
      }),
    )

    expect(receipt.status).toBe('blocked')
    expect(receipt.freshness).toBe('unknown')
    expect(receipt.blockerRefs).toEqual([
      'forge-repository-profile-refresh-blocker:work_1:missing-repository-profile-evidence',
      'forge-repository-profile-refresh-blocker:work_1:unknown-profile-refresh-freshness',
    ])
  })

  test('omits unsafe private profile material from receipts', () => {
    const receipt = projectForgeRepositoryProfileRefreshReceipt(
      baseInput({
        blockerRefs: ['private repo content /Users/christopher/src/openagents'],
        commandProfileRefs: [
          'profile.command.public.safe',
          'raw command $(cat ~/.ssh/id_rsa)',
        ],
        instructionRefs: [
          'profile.instruction.public.safe',
          'raw prompt /Users/christopher/private.md',
        ],
        invariantRefs: ['diff --git a/INVARIANTS.md b/INVARIANTS.md'],
        repoIdentityRefs: [
          'repo.github.OpenAgentsInc.openagents',
          '/Users/christopher/work/openagents',
        ],
        testProfileRefs: ['raw test /Users/christopher/private.test.ts'],
      }),
    )
    const payload = JSON.stringify(receipt)

    expect(receipt.status).toBe('blocked')
    expect(receipt.omittedUnsafeRefCount).toBe(6)
    expect(receipt.commandProfileRefs).toEqual(['profile.command.public.safe'])
    expect(receipt.instructionRefs).toEqual(['profile.instruction.public.safe'])
    expect(receipt.repoIdentityRefs).toEqual([
      'repo.github.OpenAgentsInc.openagents',
    ])
    expect(receipt.blockerRefs).toContain(
      'forge-repository-profile-refresh-blocker:work_1:unsafe-profile-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('diff --git')
    expect(payload).not.toContain('raw command')
    expect(payload).not.toContain('raw prompt')
    expect(payload).not.toContain('raw test')
  })
})
