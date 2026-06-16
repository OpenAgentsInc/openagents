import { describe, expect, test } from 'vitest'

import {
  type ForgeTerminalAgentSystemsReadinessInput,
  projectForgeTerminalAgentSystemsReadiness,
} from './terminal-agent-systems-readiness'

const baseInput = (
  overrides: Partial<ForgeTerminalAgentSystemsReadinessInput> = {},
): ForgeTerminalAgentSystemsReadinessInput => ({
  generatedAt: '2026-06-17T00:00:00.000Z',
  readinessRef: 'terminal-agent-systems-readiness.public.5107',
  ...overrides,
})

describe('Forge terminal-agent systems readiness projection', () => {
  test('projects mixed readiness entries into stable counts and order', () => {
    const readiness = projectForgeTerminalAgentSystemsReadiness(
      baseInput({
        systems: [
          {
            evidenceRefs: ['evidence.public.g5.retrieval_panel'],
            freshness: 'fresh',
            groupRef: 'group.g5',
            publicSafe: true,
            publicSafetyRefs: ['public-safety.public.g5.refs_only'],
            surfaced: true,
            systemRef: 'system.19.semantic_retrieval',
            tested: true,
            testRefs: ['test.public.g5.retrieval_panel'],
          },
          {
            evidenceRefs: ['evidence.public.g6.mcp_catalog'],
            freshness: 'fresh',
            groupRef: 'group.g6',
            publicSafe: true,
            publicSafetyRefs: ['public-safety.public.g6.refs_only'],
            surfaced: true,
            systemRef: 'system.28.mcp_client',
            tested: true,
            testRefs: ['test.public.g6.mcp_catalog'],
          },
          {
            evidenceRefs: ['evidence.public.g7.readiness'],
            freshness: 'unknown',
            groupRef: 'group.g7',
            publicSafe: false,
            surfaced: false,
            systemRef: 'system.52.evaluation_regression',
            tested: false,
          },
        ],
      }),
    )

    expect(readiness.counts).toEqual({
      blocked: 0,
      publicSafe: 2,
      stale: 0,
      surfaced: 2,
      tested: 2,
      total: 3,
    })
    expect(readiness.status).toBe('partial')
    expect(readiness.entries.map(entry => entry.systemRef)).toEqual([
      'system.19.semantic_retrieval',
      'system.28.mcp_client',
      'system.52.evaluation_regression',
    ])
  })

  test('distinguishes empty, stale, ready, and blocked readiness states', () => {
    const empty = projectForgeTerminalAgentSystemsReadiness(baseInput())
    const stale = projectForgeTerminalAgentSystemsReadiness(
      baseInput({
        systems: [
          {
            freshness: 'stale',
            groupRef: 'group.g1',
            publicSafe: true,
            surfaced: true,
            systemRef: 'system.24.diff_review',
            tested: true,
          },
        ],
      }),
    )
    const ready = projectForgeTerminalAgentSystemsReadiness(
      baseInput({
        systems: [
          {
            freshness: 'fresh',
            groupRef: 'group.g1',
            publicSafe: true,
            surfaced: true,
            systemRef: 'system.24.diff_review',
            tested: true,
          },
        ],
      }),
    )
    const blocked = projectForgeTerminalAgentSystemsReadiness(
      baseInput({
        systems: [
          {
            blockerRefs: ['readiness-blocker.public.no_smoke'],
            groupRef: 'group.g7',
            publicSafe: true,
            surfaced: true,
            systemRef: 'system.51.testing_smoke',
            tested: false,
          },
        ],
      }),
    )

    expect(empty.status).toBe('empty')
    expect(stale.status).toBe('stale')
    expect(ready.status).toBe('ready')
    expect(blocked.status).toBe('blocked')
    expect(blocked.counts.blocked).toBe(1)
  })

  test('omits unsafe private readiness material before projection', () => {
    const readiness = projectForgeTerminalAgentSystemsReadiness(
      baseInput({
        blockerRefs: ['private repo content /Users/christopher/src/openagents'],
        systems: [
          {
            blockerRefs: [
              'readiness-blocker.public.safe',
              'diff --git a/private.ts b/private.ts',
            ],
            evidenceRefs: [
              'evidence.public.safe',
              'raw transcript /Users/christopher/private.jsonl',
            ],
            freshness: 'fresh',
            groupRef: 'group.g7',
            publicSafe: true,
            publicSafetyRefs: [
              'public-safety.public.safe',
              'provider payload sk-private',
            ],
            surfaced: true,
            systemRef: 'system.53.security_review',
            tested: true,
            testRefs: [
              'test.public.safe',
              'customer private /Users/christopher/private.log',
            ],
          },
          {
            groupRef: '/Users/christopher/private/group',
            systemRef: 'system.private',
          },
        ],
      }),
    )
    const payload = JSON.stringify(readiness)

    expect(readiness.status).toBe('blocked')
    expect(readiness.omittedUnsafeRefCount).toBe(6)
    expect(readiness.entries).toEqual([
      {
        blockerRefs: ['readiness-blocker.public.safe'],
        evidenceRefs: ['evidence.public.safe'],
        freshness: 'fresh',
        groupRef: 'group.g7',
        publicSafe: true,
        publicSafetyRefs: ['public-safety.public.safe'],
        surfaced: true,
        systemRef: 'system.53.security_review',
        tested: true,
        testRefs: ['test.public.safe'],
      },
    ])
    expect(readiness.blockerRefs).toContain(
      'forge-terminal-agent-systems-readiness-blocker:terminal-agent-systems-readiness.public.5107:unsafe-readiness-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('diff --git')
    expect(payload).not.toContain('raw transcript')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('sk-private')
    expect(payload).not.toContain('customer private')
  })
})
