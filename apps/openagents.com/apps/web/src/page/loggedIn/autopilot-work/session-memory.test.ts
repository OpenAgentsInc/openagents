import { describe, expect, test } from 'vitest'

import {
  buildForgeSessionMemoryInput,
  projectForgeSessionMemory,
} from './session-memory'
import type { AutopilotWorkProjection } from '../model'

const baseInput = {
  generatedAt: '2026-06-17T20:00:00.000Z',
  snapshotRef: 'session-memory-snapshot.public.work_1',
  workOrderRef: 'work.public.work_1',
}

describe('Forge session memory projection', () => {
  test('projects public session memory entries as refs-only non-authoritative state', () => {
    const view = projectForgeSessionMemory({
      ...baseInput,
      entries: [
        {
          entryRef: 'memory-entry.public.repository.note',
          freshness: 'fresh',
          kind: 'repository_note',
          lifecycleState: 'active',
          policyRefs: ['policy.public.memory.retention'],
          redactionClass: 'public',
          retentionClass: 'project',
          retrievalRefs: ['retrieval.public.repository.note'],
          scope: 'repository',
          sourceRefs: ['event.public.memory.repository_note'],
          summaryRefs: ['summary.public.memory.repository_note'],
        },
        {
          entryRef: 'memory-entry.public.run.progress',
          freshness: 'fresh',
          kind: 'progress_note',
          lifecycleState: 'active',
          redactionClass: 'public',
          retentionClass: 'ephemeral',
          scope: 'run',
          sourceRefs: ['event.public.memory.progress'],
        },
      ],
      projectionRef: 'session-memory-projection.public.work_1',
      versionRef: 'session-memory-version.public.v1',
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      active: 2,
      conflicted: 0,
      localOnly: 0,
      retained: 1,
      stale: 0,
      total: 2,
    })
    expect(view.entries.map(entry => entry.entryRef)).toEqual([
      'memory-entry.public.repository.note',
      'memory-entry.public.run.progress',
    ])
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      deploymentAuthority: false,
      memoryCompactionAuthority: false,
      memoryRetentionPolicyWriteAuthority: false,
      memoryWriteAuthority: false,
      modelCallAuthority: false,
      promptAssemblyAuthority: false,
      publicClaimAuthority: false,
      settlementAuthority: false,
      skillCommandLoadAuthority: false,
      toolGrantAuthority: false,
      transcriptSummarizationAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing session memory as an empty projection', () => {
    const view = projectForgeSessionMemory({
      generatedAt: '2026-06-17T20:00:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.snapshotRef).toBeNull()
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks stale memory without explicit refresh blocker evidence', () => {
    const view = projectForgeSessionMemory({
      ...baseInput,
      entries: [
        {
          entryRef: 'memory-entry.public.stale.preference',
          freshness: 'stale',
          kind: 'operator_preference',
          lifecycleState: 'active',
          redactionClass: 'public',
          retentionClass: 'ephemeral',
          scope: 'session',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-session-memory-blocker:work.public.work_1:stale-memory-refresh-evidence-missing:memory-entry.public.stale.preference',
    )
  })

  test('blocks superseded memory without conflict evidence', () => {
    const view = projectForgeSessionMemory({
      ...baseInput,
      entries: [
        {
          entryRef: 'memory-entry.public.superseded.fact',
          freshness: 'fresh',
          kind: 'fact',
          lifecycleState: 'superseded',
          redactionClass: 'public',
          retentionClass: 'ephemeral',
          scope: 'session',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-session-memory-blocker:work.public.work_1:conflict-evidence-missing:memory-entry.public.superseded.fact',
    )
  })

  test('blocks retained memory entries without policy refs', () => {
    const view = projectForgeSessionMemory({
      ...baseInput,
      entries: [
        {
          entryRef: 'memory-entry.public.long_term.preference',
          freshness: 'fresh',
          kind: 'user_preference',
          lifecycleState: 'active',
          redactionClass: 'public',
          retentionClass: 'long_term',
          scope: 'user',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-session-memory-blocker:work.public.work_1:retention-policy-missing:memory-entry.public.long_term.preference',
    )
  })

  test('omits unsafe private memory material before projection', () => {
    const view = projectForgeSessionMemory({
      ...baseInput,
      blockerRefs: [
        'memory-blocker.public.safe',
        'raw memory /Users/christopher/memory.md',
      ],
      entries: [
        {
          blockerRefs: ['entry-blocker.public.safe', 'provider payload sk-private'],
          compactionRefs: ['compaction.public.safe', 'raw transcript private'],
          conflictRefs: ['conflict.public.safe'],
          entryRef: 'memory-entry.public.safe',
          freshness: 'fresh',
          kind: 'task_context',
          lifecycleState: 'active',
          policyRefs: ['policy.public.safe', 'bearer token private'],
          redactionClass: 'local_only',
          retentionClass: 'session',
          retrievalRefs: ['retrieval.public.safe'],
          scope: 'session',
          sourceRefs: ['event.public.safe', 'memory body /Users/christopher/private.md'],
          summaryRefs: ['summary.public.safe', 'prompt text sk-private'],
        },
      ],
    })

    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.sourceRefs).toEqual(['event.public.safe'])
    expect(view.entries[0]?.summaryRefs).toEqual(['summary.public.safe'])
    expect(view.entries[0]?.policyRefs).toEqual(['policy.public.safe'])
    expect(view.entries[0]?.compactionRefs).toEqual(['compaction.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-session-memory-blocker:work.public.work_1:unsafe-session-memory-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw memory')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('raw transcript')
    expect(payload).not.toContain('memory body')
    expect(payload).not.toContain('prompt text')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-17T20:01:00.000Z',
      sessionMemory: {
        entries: [
          {
            entryRef: 'memory-entry.public.work_2',
            freshness: 'fresh',
            kind: 'decision',
            lifecycleState: 'active',
            retentionClass: 'ephemeral',
            scope: 'run',
          },
        ],
        projectionRef: 'session-memory-projection.public.work_2',
        snapshotRef: 'session-memory-snapshot.public.work_2',
        versionRef: 'session-memory-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeSessionMemoryInput(work)).toEqual({
      entries: [
        {
          entryRef: 'memory-entry.public.work_2',
          freshness: 'fresh',
          kind: 'decision',
          lifecycleState: 'active',
          retentionClass: 'ephemeral',
          scope: 'run',
        },
      ],
      generatedAt: '2026-06-17T20:01:00.000Z',
      projectionRef: 'session-memory-projection.public.work_2',
      snapshotRef: 'session-memory-snapshot.public.work_2',
      versionRef: 'session-memory-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
