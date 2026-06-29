import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PublicAgentTemplateProjection,
  PublicAgentTemplateUnsafe,
  projectPublicAgentTemplate,
  publicAgentTemplateHasPrivateMaterial,
  publicAgentTemplateSourceExample,
  type PublicAgentTemplateRecord,
} from './public-agent-template'

const baseRecord: PublicAgentTemplateRecord =
  publicAgentTemplateSourceExample('adjutant')

describe('public agent template projections', () => {
  test('projects public, customer, team, and operator audiences with scoped refs', () => {
    const publicProjection = projectPublicAgentTemplate(baseRecord, 'public')
    const customerProjection = projectPublicAgentTemplate(baseRecord, 'customer')
    const teamProjection = projectPublicAgentTemplate(baseRecord, 'team')
    const operatorProjection = projectPublicAgentTemplate(baseRecord, 'operator')

    expect(S.decodeUnknownSync(PublicAgentTemplateProjection)(publicProjection))
      .toEqual(publicProjection)
    expect(publicProjection).toMatchObject({
      agentRef: 'adjutant',
      audience: 'public',
      claim: {
        audience: 'public',
        state: {
          label: 'Measured',
          state: 'measured',
        },
      },
      displayName: 'Adjutant',
      health: 'running',
      publicUrls: ['https://openagents.com/adjutant'],
      source: 'adjutant',
    })
    expect(publicProjection.customerRefs).toEqual([])
    expect(publicProjection.teamRefs).toEqual([])
    expect(publicProjection.operatorRefs).toEqual([])
    expect(publicProjection.gates[0]?.customerRefs).toEqual([])
    expect(publicProjection.eventTimeline[0]?.operatorRefs).toEqual([])
    expect(customerProjection.customerRefs).toEqual(baseRecord.customerRefs)
    expect(customerProjection.teamRefs).toEqual([])
    expect(customerProjection.operatorRefs).toEqual([])
    expect(teamProjection.customerRefs).toEqual(baseRecord.customerRefs)
    expect(teamProjection.teamRefs).toEqual(baseRecord.teamRefs)
    expect(teamProjection.operatorRefs).toEqual([])
    expect(operatorProjection.customerRefs).toEqual(baseRecord.customerRefs)
    expect(operatorProjection.teamRefs).toEqual(baseRecord.teamRefs)
    expect(operatorProjection.operatorRefs).toEqual(baseRecord.operatorRefs)
    expect(publicAgentTemplateHasPrivateMaterial(publicProjection)).toBe(false)
  })

  test('keeps Artanis and Adjutant as source examples using the same contract', () => {
    const artanis = projectPublicAgentTemplate(
      publicAgentTemplateSourceExample('artanis'),
      'public',
    )
    const adjutant = projectPublicAgentTemplate(
      publicAgentTemplateSourceExample('adjutant'),
      'public',
    )

    expect(artanis).toMatchObject({
      agentRef: 'artanis',
      displayName: 'Artanis',
      objectiveRef: 'objective.public_agent.artanis.pylon_campaign',
      publicUrls: ['https://openagents.com/artanis'],
      source: 'artanis',
    })
    expect(adjutant).toMatchObject({
      agentRef: 'adjutant',
      displayName: 'Adjutant',
      objectiveRef: 'objective.public_agent.adjutant.sites_supervision',
      publicUrls: ['https://openagents.com/adjutant'],
      source: 'adjutant',
    })
    expect(S.decodeUnknownSync(PublicAgentTemplateProjection)(artanis))
      .toEqual(artanis)
    expect(S.decodeUnknownSync(PublicAgentTemplateProjection)(adjutant))
      .toEqual(adjutant)
  })

  test('uses claim-state wording and caveats for missing evidence', () => {
    const projection = projectPublicAgentTemplate({
      ...baseRecord,
      claim: {
        ...baseRecord.claim,
        desiredState: 'verified',
        evidenceRefs: [],
      },
    }, 'public')

    expect(projection.claim.state).toMatchObject({
      label: 'Planned',
      state: 'planned',
    })
    expect(projection.claim.copyRule).toMatchObject({
      allowedPublicVerb: 'planned',
      copyRuleRef: 'copy_rule.public_claim.planned',
    })
    expect(projection.claim.state.caveats).toContain(
      'Requested verified claim was lowered to planned because required evidence is missing.',
    )
  })

  test('rejects private prompt, provider, wallet, customer, and workroom refs', () => {
    expect(() =>
      projectPublicAgentTemplate({
        ...baseRecord,
        eventTimeline: [
          {
            ...baseRecord.eventTimeline[0]!,
            artifactRefs: ['raw_runner_payload:abc'],
          },
        ],
      }, 'public'),
    ).toThrow(PublicAgentTemplateUnsafe)
    expect(() =>
      projectPublicAgentTemplate({
        ...baseRecord,
        operatorRefs: ['provider_grant:abc'],
      }, 'operator'),
    ).toThrow(PublicAgentTemplateUnsafe)
    expect(() =>
      projectPublicAgentTemplate({
        ...baseRecord,
        proofRefs: ['lnbc1rawinvoice'],
      }, 'public'),
    ).toThrow(PublicAgentTemplateUnsafe)
    expect(() =>
      projectPublicAgentTemplate({
        ...baseRecord,
        customerRefs: ['ben@example.com'],
      }, 'customer'),
    ).toThrow(PublicAgentTemplateUnsafe)
    expect(() =>
      projectPublicAgentTemplate({
        ...baseRecord,
        artifactRefs: ['workroom_private:order_otec'],
      }, 'team'),
    ).toThrow(PublicAgentTemplateUnsafe)
  })

  test('rejects public URLs that carry private state', () => {
    expect(() =>
      projectPublicAgentTemplate({
        ...baseRecord,
        publicUrls: [
          'https://openagents.com/adjutant?token=raw',
        ],
      }, 'public'),
    ).toThrow(PublicAgentTemplateUnsafe)
  })
})
