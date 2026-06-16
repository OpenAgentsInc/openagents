import { describe, expect, test } from 'vitest'

import { projectForgeContextSnapshot } from './context-snapshot'
import { projectForgeExtensibilityEffectiveConfig } from './extensibility-effective-config'
import { projectForgeHookCatalog } from './hook-catalog'
import { projectForgeMcpCapabilityCatalog } from './mcp-capability-catalog'
import { projectForgeRetrievalPlan } from './retrieval-plan'
import { projectForgeSessionNavigation } from './session-navigation'
import { projectForgeSkillDescriptorCatalog } from './skill-descriptor-catalog'

const unsafeMarkers = [
  '/Users/christopher',
  'bearer token',
  'diff --git',
  'private.example',
  'provider payload',
  'raw prompt',
  'raw shell',
  'raw transcript',
  'sk-private',
] as const

describe('terminal-agent private material regressions', () => {
  test('omits private material across G1-G6 projection lanes', () => {
    const cases = [
      {
        blocker: 'unsafe-context-material-omitted',
        name: 'context',
        payload: projectForgeContextSnapshot({
          adapters: {
            refs: ['adapter.codex.ready', 'provider payload sk-private'],
          },
          currentJob: {
            jobRefs: ['assignment.public.work_1', 'raw shell command $(cat secret)'],
          },
          devDoctor: {
            refs: ['doctor.public.safe', 'diff --git a/private.ts b/private.ts'],
          },
          generatedAt: '2026-06-17T00:30:00.000Z',
          instructions: {
            refs: ['instructions.public.safe', 'raw prompt /Users/christopher/a.md'],
          },
          observedAt: '2026-06-17T00:29:00.000Z',
          repo: {
            dirtyState: 'clean',
            identityRefs: ['repo.github.OpenAgentsInc.openagents'],
          },
          workOrderRef: 'work_1',
        }),
        safeRef: 'adapter.codex.ready',
      },
      {
        blocker: 'unsafe-session-material-omitted',
        name: 'session navigation',
        payload: projectForgeSessionNavigation({
          codexSessions: [
            {
              artifactRefs: [
                'artifact.public.codex.safe',
                'raw transcript /Users/christopher/private.jsonl',
              ],
              eventRefs: ['diff --git a/private.ts b/private.ts'],
              sessionRef: '/Users/christopher/.codex/session.jsonl',
              state: 'running',
            },
            {
              artifactRefs: ['artifact.public.codex.safe'],
              sessionRef: 'codex.session.safe',
              state: 'running',
            },
          ],
          generatedAt: '2026-06-17T00:30:00.000Z',
          workOrderRef: 'work_1',
        }),
        safeRef: 'codex.session.safe',
      },
      {
        blocker: 'unsafe-retrieval-material-omitted',
        name: 'retrieval',
        payload: projectForgeRetrievalPlan({
          candidates: [
            {
              candidateRef: 'candidate.public.safe',
              provenanceRefs: [
                'retrieval-provenance.public.safe',
                'provider payload sk-private',
              ],
              sourceRef: 'raw file /Users/christopher/private.md',
            },
          ],
          generatedAt: '2026-06-17T00:30:00.000Z',
          mode: 'exact',
          planRef: 'retrieval-plan.public.work_1',
          queryRefs: ['query.public.safe', 'raw prompt /Users/christopher/a.md'],
          requestRef: 'retrieval-request.public.work_1',
          skippedCandidates: [
            {
              candidateRef: 'candidate.public.skipped',
              reason: 'filtered_private',
              sourceRef: 'https://private.example/repo',
            },
          ],
        }),
        safeRef: 'candidate.public.safe',
      },
      {
        blocker: 'unsafe-mcp-material-omitted',
        name: 'mcp catalog',
        payload: projectForgeMcpCapabilityCatalog({
          catalogRef: 'mcp-catalog.public.work_1',
          entries: [
            {
              authRefs: ['mcp-auth.public.safe', 'bearer token private'],
              capabilityRefs: [
                'mcp-capability.public.safe',
                'raw tool schema /Users/christopher/private.json',
              ],
              serverRef: 'mcp-server.public.safe',
              state: 'configured',
            },
          ],
          generatedAt: '2026-06-17T00:30:00.000Z',
          workOrderRef: 'work_1',
        }),
        safeRef: 'mcp-capability.public.safe',
      },
      {
        blocker: 'unsafe-skill-material-omitted',
        name: 'skill catalog',
        payload: projectForgeSkillDescriptorCatalog({
          catalogRef: 'skill-catalog.public.work_1',
          entries: [
            {
              bodyRequestRefs: [
                'skill-body-request.public.safe',
                'full skill body /Users/christopher/private/SKILL.md',
              ],
              descriptorRef: 'skill-descriptor.public.safe',
              policyRefs: ['skill-policy.public.safe', 'provider payload sk-private'],
              skillRef: 'skill.safe',
              state: 'available',
            },
          ],
          generatedAt: '2026-06-17T00:30:00.000Z',
          workOrderRef: 'work_1',
        }),
        safeRef: 'skill-body-request.public.safe',
      },
      {
        blocker: 'unsafe-hook-material-omitted',
        name: 'hook catalog',
        payload: projectForgeHookCatalog({
          catalogRef: 'hook-catalog.public.work_1',
          entries: [
            {
              descriptorRef: 'hook-descriptor.public.safe',
              doctorRefs: ['hook-doctor.public.safe', 'raw hook script /Users/christopher/a.sh'],
              eventRefs: ['hook-event.public.safe', 'raw shell command $(secret)'],
              hookRef: 'hook.safe',
              policyRefs: ['hook-policy.public.safe', 'provider payload sk-private'],
              state: 'configured',
              workspaceTrustRefs: ['workspace-trust.public.safe'],
            },
          ],
          generatedAt: '2026-06-17T00:30:00.000Z',
          workOrderRef: 'work_1',
        }),
        safeRef: 'hook-doctor.public.safe',
      },
      {
        blocker: 'unsafe-extensibility-material-omitted',
        name: 'effective config',
        payload: projectForgeExtensibilityEffectiveConfig({
          configRef: 'extensibility-config.public.work_1',
          entries: [
            {
              catalogRefs: ['plugin-catalog.public.safe', 'raw plugin code /Users/christopher/plugin.ts'],
              configRefs: ['plugin-config.public.safe', 'raw config /Users/christopher/.x'],
              domain: 'plugins',
              effectiveState: 'enabled',
              sourceRefs: ['plugin-source.public.safe', 'provider payload sk-private'],
            },
          ],
          generatedAt: '2026-06-17T00:30:00.000Z',
          workOrderRef: 'work_1',
        }),
        safeRef: 'plugin-config.public.safe',
      },
    ] as const

    for (const testCase of cases) {
      const payload = JSON.stringify(testCase.payload)

      expect(payload, testCase.name).toContain(testCase.safeRef)
      expect(payload, testCase.name).toContain(testCase.blocker)

      for (const marker of unsafeMarkers) {
        expect(payload, `${testCase.name} leaked ${marker}`).not.toContain(marker)
      }
    }
  })
})
