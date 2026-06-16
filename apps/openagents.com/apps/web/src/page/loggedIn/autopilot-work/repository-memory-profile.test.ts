import { describe, expect, test } from 'vitest'

import {
  type ForgeRepositoryMemoryProfileInput,
  projectForgeRepositoryMemoryProfile,
} from './repository-memory-profile'

const baseInput = (
  overrides: Partial<ForgeRepositoryMemoryProfileInput> = {},
): ForgeRepositoryMemoryProfileInput => ({
  devDoctorRefs: ['doctor.public.pylon.context.v0_3'],
  generatedAt: '2026-06-16T19:30:00.000Z',
  profileRef: 'repository-profile.public.openagents.main',
  refreshedAt: '2026-06-16T19:20:00.000Z',
  workOrderRef: 'work_1',
  ...overrides,
})

describe('Forge repository memory profile projection', () => {
  test('projects a ready durable repository profile with refresh receipts', () => {
    const profile = projectForgeRepositoryMemoryProfile(
      baseInput({
        commandProfileRefs: ['profile.command.public.package_scripts.sha256_abcd'],
        currentInstructionRefs: ['profile.instruction.public.AGENTS.sha256_abcd'],
        instructionRefs: ['profile.instruction.public.AGENTS.sha256_abcd'],
        invariantRefs: ['profile.invariant.public.INVARIANTS.sha256_abcd'],
        refreshEvents: [
          {
            commandProfileRefs: [
              'profile.command.public.package_scripts.sha256_abcd',
            ],
            generatedAt: '2026-06-16T19:25:00.000Z',
            instructionRefs: ['profile.instruction.public.AGENTS.sha256_abcd'],
            refreshedAt: '2026-06-16T19:20:00.000Z',
            repoIdentityRefs: ['repo.github.OpenAgentsInc.openagents'],
            testProfileRefs: ['profile.test.public.vitest.sha256_abcd'],
            workOrderRef: 'work_1',
          },
        ],
        refreshReceiptRefs: ['receipt.public.previous_profile_refresh'],
        repoIdentityRefs: ['repo.github.OpenAgentsInc.openagents', 'branch.main'],
        testProfileRefs: ['profile.test.public.vitest.sha256_abcd'],
      }),
    )

    expect(profile).toMatchObject({
      freshness: 'fresh',
      omittedUnsafeRefCount: 0,
      profileRef: 'repository-profile.public.openagents.main',
      status: 'ready',
      workOrderRef: 'work_1',
    })
    expect(profile.changedProfileKinds).toEqual([])
    expect(profile.refreshReceipts).toHaveLength(1)
    expect(profile.refreshReceipts[0]).toMatchObject({
      publicSafe: true,
      receiptKind: 'forge_repository_profile_refresh.v1',
      status: 'fresh',
    })
    expect(profile.refreshReceiptRefs).toEqual([
      'receipt.public.previous_profile_refresh',
      'forge.repository_profile_refresh.work_1.2026-06-16T19_25_00.000Z',
    ])
    expect(profile.blockerRefs).toEqual([])
  })

  test('marks dirty or instruction-changed profiles stale until refreshed', () => {
    const profile = projectForgeRepositoryMemoryProfile(
      baseInput({
        commandProfileRefs: ['profile.command.public.package_scripts.sha256_abcd'],
        currentInstructionRefs: ['profile.instruction.public.AGENTS.sha256_new'],
        dirtyState: 'dirty',
        instructionRefs: ['profile.instruction.public.AGENTS.sha256_old'],
        repoIdentityRefs: ['repo.github.OpenAgentsInc.openagents'],
      }),
    )

    expect(profile.status).toBe('stale')
    expect(profile.blockerRefs).toEqual([
      'forge-repository-memory-profile-blocker:repository-profile.public.openagents.main:dirty-worktree-invalidates-profile',
      'forge-repository-memory-profile-blocker:repository-profile.public.openagents.main:instruction-refs-changed',
    ])
  })

  test('blocks repository memory when profile evidence or dev-doctor evidence is missing', () => {
    const profile = projectForgeRepositoryMemoryProfile(
      baseInput({
        devDoctorRefs: [],
        refreshedAt: null,
      }),
    )

    expect(profile.status).toBe('blocked')
    expect(profile.freshness).toBe('unknown')
    expect(profile.blockerRefs).toEqual([
      'forge-repository-memory-profile-blocker:repository-profile.public.openagents.main:missing-repository-profile-evidence',
      'forge-repository-memory-profile-blocker:repository-profile.public.openagents.main:missing-dev-doctor-evidence',
      'forge-repository-memory-profile-blocker:repository-profile.public.openagents.main:unknown-profile-freshness',
    ])
  })

  test('omits unsafe private profile material before rendering or persistence', () => {
    const profile = projectForgeRepositoryMemoryProfile(
      baseInput({
        blockerRefs: ['private repo content /Users/christopher/src/openagents'],
        commandProfileRefs: [
          'profile.command.public.safe',
          'raw command $(cat ~/.ssh/id_rsa)',
        ],
        currentInstructionRefs: [
          'profile.instruction.public.safe',
          'raw prompt /Users/christopher/private.md',
        ],
        instructionRefs: [
          'profile.instruction.public.safe',
          'raw prompt /Users/christopher/private.md',
        ],
        refreshEvents: [
          {
            commandProfileRefs: [
              'profile.command.public.safe',
              'raw command /Users/christopher/private.sh',
            ],
            generatedAt: '2026-06-16T19:25:00.000Z',
            refreshedAt: '2026-06-16T19:20:00.000Z',
            repoIdentityRefs: ['repo.github.OpenAgentsInc.openagents'],
            workOrderRef: 'work_1',
          },
        ],
        repoIdentityRefs: [
          'repo.github.OpenAgentsInc.openagents',
          '/Users/christopher/work/openagents',
        ],
        testProfileRefs: ['raw test /Users/christopher/private.test.ts'],
      }),
    )
    const payload = JSON.stringify(profile)

    expect(profile.status).toBe('blocked')
    expect(profile.omittedUnsafeRefCount).toBeGreaterThanOrEqual(7)
    expect(profile.commandProfileRefs).toEqual(['profile.command.public.safe'])
    expect(profile.currentInstructionRefs).toEqual([
      'profile.instruction.public.safe',
    ])
    expect(profile.repoIdentityRefs).toEqual([
      'repo.github.OpenAgentsInc.openagents',
    ])
    expect(profile.blockerRefs).toContain(
      'forge-repository-memory-profile-blocker:repository-profile.public.openagents.main:unsafe-profile-material-omitted',
    )
    expect(profile.refreshReceipts[0]?.blockerRefs).toContain(
      'forge-repository-profile-refresh-blocker:work_1:unsafe-profile-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw command')
    expect(payload).not.toContain('raw prompt')
    expect(payload).not.toContain('raw test')
  })
})
