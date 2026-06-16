import { describe, expect, test } from 'vitest'

import {
  type ForgeContextSnapshotInput,
  projectForgeContextSnapshot,
} from './context-snapshot'

const baseInput = (
  overrides: Partial<ForgeContextSnapshotInput> = {},
): ForgeContextSnapshotInput => ({
  generatedAt: '2026-06-16T18:00:00.000Z',
  observedAt: '2026-06-16T17:55:00.000Z',
  workOrderRef: 'work_1',
  ...overrides,
})

describe('Forge context snapshot projection', () => {
  test('projects a ready refs-only context snapshot', () => {
    const snapshot = projectForgeContextSnapshot(
      baseInput({
        adapters: {
          capabilityRefs: ['capability.codex_agent_task.v1'],
          refs: ['adapter.codex.ready'],
        },
        currentJob: {
          capabilityRefs: ['capability.codex_agent_task.v1'],
          jobRefs: ['assignment.public.work_1', 'workspace.public.work_1'],
          verificationRefs: ['verification-command.public.bun_test'],
        },
        devDoctor: {
          refs: ['doctor.public.pylon.context.v0_3'],
        },
        instructions: {
          configRefs: ['config.pylon.default_adapter.codex'],
          refs: ['instructions.public.AGENTS.md.sha256_abcd'],
        },
        repo: {
          changedCount: 0,
          dirtyState: 'clean',
          identityRefs: [
            'repo.github.OpenAgentsInc.openagents',
            'branch.main',
            'commit.f9793718e000',
          ],
        },
      }),
    )

    expect(snapshot).toMatchObject({
      freshness: 'fresh',
      omittedUnsafeRefCount: 0,
      status: 'ready',
      workOrderRef: 'work_1',
    })
    expect(snapshot.repo.identityRefs).toEqual([
      'repo.github.OpenAgentsInc.openagents',
      'branch.main',
      'commit.f9793718e000',
    ])
    expect(snapshot.instructions.instructionRefs).toEqual([
      'instructions.public.AGENTS.md.sha256_abcd',
    ])
    expect(snapshot.adapters.capabilityRefs).toEqual([
      'capability.codex_agent_task.v1',
    ])
    expect(snapshot.currentJob.verificationRefs).toEqual([
      'verification-command.public.bun_test',
    ])
    expect(snapshot.blockerRefs).toEqual([])
  })

  test('marks stale snapshots without treating dirty state as completed context', () => {
    const snapshot = projectForgeContextSnapshot(
      baseInput({
        observedAt: '2026-06-16T17:00:00.000Z',
        repo: {
          changedCount: 3,
          dirtyState: 'dirty',
          dirtyStateRefs: ['repo-dirty.public.work_1.changed_3'],
          identityRefs: ['repo.github.OpenAgentsInc.openagents'],
        },
      }),
    )

    expect(snapshot.status).toBe('stale')
    expect(snapshot.freshness).toBe('stale')
    expect(snapshot.repo).toMatchObject({
      changedCount: 3,
      dirtyState: 'dirty',
      dirtyStateRefs: ['repo-dirty.public.work_1.changed_3'],
    })
    expect(snapshot.blockerRefs).toEqual([])
  })

  test('reports blockers when context evidence is missing', () => {
    const snapshot = projectForgeContextSnapshot(
      baseInput({
        observedAt: null,
      }),
    )

    expect(snapshot.status).toBe('blocked')
    expect(snapshot.freshness).toBe('unknown')
    expect(snapshot.blockerRefs).toEqual([
      'forge-context-snapshot-blocker:work_1:missing-context-evidence',
      'forge-context-snapshot-blocker:work_1:unknown-context-freshness',
    ])
  })

  test('omits unsafe context refs and private material before projection', () => {
    const snapshot = projectForgeContextSnapshot(
      baseInput({
        adapters: {
          refs: ['adapter.codex.ready', 'provider payload sk-private'],
        },
        blockerRefs: ['private repo content /Users/christopher/src/openagents'],
        currentJob: {
          jobRefs: [
            'assignment.public.work_1',
            'raw shell command $(cat ~/.ssh/id_rsa)',
          ],
        },
        devDoctor: {
          refs: ['doctor.public.safe', 'diff --git a/private.ts b/private.ts'],
        },
        instructions: {
          refs: [
            'instructions.public.AGENTS.md.sha256_abcd',
            'raw prompt /Users/christopher/private.md',
          ],
        },
        repo: {
          dirtyState: 'clean',
          identityRefs: [
            'repo.github.OpenAgentsInc.openagents',
            '/Users/christopher/work/openagents',
          ],
        },
      }),
    )
    const payload = JSON.stringify(snapshot)

    expect(snapshot.status).toBe('blocked')
    expect(snapshot.omittedUnsafeRefCount).toBe(6)
    expect(snapshot.repo.identityRefs).toEqual([
      'repo.github.OpenAgentsInc.openagents',
    ])
    expect(snapshot.adapters.readinessRefs).toEqual(['adapter.codex.ready'])
    expect(snapshot.currentJob.jobRefs).toEqual(['assignment.public.work_1'])
    expect(snapshot.blockerRefs).toContain(
      'forge-context-snapshot-blocker:work_1:unsafe-context-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('diff --git')
    expect(payload).not.toContain('raw prompt')
    expect(payload).not.toContain('raw shell')
    expect(payload).not.toContain('sk-private')
  })
})
