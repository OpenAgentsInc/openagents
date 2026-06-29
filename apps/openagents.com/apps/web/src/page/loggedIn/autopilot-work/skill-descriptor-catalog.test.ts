import { describe, expect, test } from 'vitest'

import {
  type ForgeSkillDescriptorCatalogInput,
  projectForgeSkillDescriptorCatalog,
} from './skill-descriptor-catalog'

const baseInput = (
  overrides: Partial<ForgeSkillDescriptorCatalogInput> = {},
): ForgeSkillDescriptorCatalogInput => ({
  catalogRef: 'skill-catalog.public.work_1',
  generatedAt: '2026-06-16T22:30:00.000Z',
  workOrderRef: 'work_1',
  ...overrides,
})

describe('Forge skill descriptor catalog projection', () => {
  test('projects mixed skill states into stable counts and sorted descriptors', () => {
    const catalog = projectForgeSkillDescriptorCatalog(
      baseInput({
        entries: [
          {
            descriptorRef: 'skill-descriptor.public.github',
            policyRefs: ['skill-policy.public.github.requires_auth'],
            skillRef: 'skill.github',
            state: 'needs_review',
            summaryRefs: ['skill-summary.public.github'],
            triggerRefs: ['skill-trigger.public.github_issue'],
          },
          {
            descriptorRef: 'skill-descriptor.public.ui',
            policyRefs: ['skill-policy.public.ui.readonly'],
            skillRef: 'skill.ui',
            state: 'available',
            summaryRefs: ['skill-summary.public.ui'],
            triggerRefs: ['skill-trigger.public.frontend'],
          },
          {
            blockerRefs: ['skill-blocker.public.local_missing'],
            descriptorRef: 'skill-descriptor.public.local',
            skillRef: 'skill.local',
            state: 'failed',
          },
          {
            descriptorRef: 'skill-descriptor.public.future',
            skillRef: 'skill.future',
            state: 'pending',
          },
          {
            descriptorRef: 'skill-descriptor.public.legacy',
            skillRef: 'skill.legacy',
            state: 'disabled',
          },
        ],
        freshness: 'fresh',
      }),
    )

    expect(catalog).toMatchObject({
      authority: {
        contextInjectionAuthority: false,
        providerAccountAuthority: false,
        settlementAuthority: false,
        toolCallAuthority: false,
        workspaceWriteAuthority: false,
      },
      counts: {
        available: 1,
        disabled: 1,
        failed: 1,
        needsReview: 1,
        pending: 1,
        total: 5,
      },
      disclosure: {
        bodyIncludedByDefault: false,
        defaultContextIncludesFullSkillBody: false,
        explicitBodyRequestRequired: true,
      },
      publicSafe: true,
      status: 'blocked',
    })
    expect(catalog.entries.map(entry => entry.skillRef)).toEqual([
      'skill.github',
      'skill.local',
      'skill.future',
      'skill.ui',
      'skill.legacy',
    ])
    expect(catalog.entries.every(entry => entry.bodyIncludedByDefault === false)).toBe(
      true,
    )
    expect(catalog.blockerRefs).toEqual(['skill-blocker.public.local_missing'])
  })

  test('distinguishes empty, stale, and ready skill catalogs', () => {
    const empty = projectForgeSkillDescriptorCatalog(baseInput())
    const stale = projectForgeSkillDescriptorCatalog(
      baseInput({
        entries: [
          {
            descriptorRef: 'skill-descriptor.public.ui',
            skillRef: 'skill.ui',
            state: 'available',
          },
        ],
        freshness: 'stale',
      }),
    )
    const ready = projectForgeSkillDescriptorCatalog(
      baseInput({
        entries: [
          {
            descriptorRef: 'skill-descriptor.public.ui',
            skillRef: 'skill.ui',
            state: 'available',
          },
        ],
        freshness: 'fresh',
      }),
    )

    expect(empty.status).toBe('empty')
    expect(stale.status).toBe('stale')
    expect(ready.status).toBe('ready')
  })

  test('omits unsafe full skill bodies and private descriptor material', () => {
    const catalog = projectForgeSkillDescriptorCatalog(
      baseInput({
        entries: [
          {
            blockerRefs: [
              'skill-blocker.public.safe',
              'diff --git a/private.md b/private.md',
            ],
            bodyRequestRefs: [
              'skill-body-request.public.safe',
              'full skill body /Users/christopher/private/SKILL.md',
            ],
            descriptorRef: 'skill-descriptor.public.safe',
            policyRefs: [
              'skill-policy.public.safe',
              'provider payload sk-private',
            ],
            skillRef: 'skill.safe',
            sourceRefs: [
              'skill-source.public.safe',
              '/Users/christopher/.codex/skills/private/SKILL.md',
            ],
            state: 'available',
            summaryRefs: [
              'skill-summary.public.safe',
              'raw skill body /Users/christopher/private.md',
            ],
            triggerRefs: ['skill-trigger.public.safe', 'bearer token private'],
          },
          {
            descriptorRef: '/Users/christopher/private/descriptor.json',
            skillRef: 'skill.private',
            state: 'failed',
          },
        ],
      }),
    )
    const payload = JSON.stringify(catalog)

    expect(catalog.status).toBe('blocked')
    expect(catalog.omittedUnsafeRefCount).toBe(7)
    expect(catalog.entries).toEqual([
      {
        blockerRefs: ['skill-blocker.public.safe'],
        bodyIncludedByDefault: false,
        bodyRequestRefs: ['skill-body-request.public.safe'],
        descriptorRef: 'skill-descriptor.public.safe',
        freshness: 'unknown',
        policyRefs: ['skill-policy.public.safe'],
        skillRef: 'skill.safe',
        sourceRefs: ['skill-source.public.safe'],
        state: 'available',
        summaryRefs: ['skill-summary.public.safe'],
        triggerRefs: ['skill-trigger.public.safe'],
      },
    ])
    expect(catalog.blockerRefs).toContain(
      'forge-skill-descriptor-catalog-blocker:skill-catalog.public.work_1:unsafe-skill-material-omitted',
    )
    expect(catalog.disclosure.defaultContextIncludesFullSkillBody).toBe(false)
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('diff --git')
    expect(payload).not.toContain('raw skill body')
    expect(payload).not.toContain('full skill body')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('sk-private')
    expect(payload).not.toContain('bearer token')
  })
})
