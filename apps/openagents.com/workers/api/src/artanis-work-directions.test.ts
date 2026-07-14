import { describe, expect, test } from 'vitest'

import {
  buildArtanisWorkDirectionRequestFromRoutingProposal,
  filterArtanisWorkRoutingProposalsIntoRequests,
  handleArtanisWorkDirectionDelivery,
  projectArtanisWorkDirectionVerificationGate,
} from './artanis-work-directions'

const proposal = () => ({
  corpusRef: 'corpus.public.artanis.programs.v1',
  deadlineRef: 'deadline.public.artanis.next_cycle',
  directionKind: 'program_authorship' as const,
  moduleFamilyRef: 'module.public.artanis.calm_wasm',
  objectiveRef: 'objective.public.artanis.program_authorship',
  proposalRef: 'proposal.public.artanis.program_authorship.001',
  repositoryRefs: ['repo.public.openagents'],
  selectorRef: 'selector.public.artanis.work_directions',
  sourceRefs: ['source.public.artanis.report'],
  title: 'Author a bounded CALM module',
})

const delivery = () => ({
  acceptanceEventRef: 'event.public.artanis.acceptance.001',
  directionKind: 'program_authorship' as const,
  programVerification: {
    blockerRefs: [] as string[],
    constructionVerified: true,
    moduleDigest: 'a'.repeat(64),
    moduleKind: 'calm_wasm',
    replayVerified: true,
    verificationClass: 'v1_construction' as const,
    verificationRef: 'receipt.public.artanis.verification.001',
  },
  providerActorRef: 'agent:artanis-builder',
  resultRef: 'result.public.artanis.program.001',
  verificationCommandRef: 'command.public.artanis.v1_construction',
  workRequestId: 'work_request_artanis_001',
})

describe('Artanis non-money work directions', () => {
  test('builds evidence requests without a budget or paid-market contract', () => {
    const request = buildArtanisWorkDirectionRequestFromRoutingProposal(proposal())
    expect(request).toMatchObject({
      directionKind: 'program_authorship',
      verificationClass: 'v1_construction',
    })
    expect('budgetSats' in request).toBe(false)
  })

  test('retains the operator routing gate', () => {
    expect(
      filterArtanisWorkRoutingProposalsIntoRequests({ proposals: [proposal()] }),
    ).toMatchObject({ enabled: false, requestCount: 0 })
    expect(
      filterArtanisWorkRoutingProposalsIntoRequests({
        operatorEnabled: true,
        proposals: [proposal()],
      }),
    ).toMatchObject({ enabled: true, requestCount: 1 })
  })

  test('records evidence acceptance without releasing or refunding escrow', async () => {
    const lifecycleKinds: string[] = []
    expect(projectArtanisWorkDirectionVerificationGate(delivery()).accepted).toBe(
      true,
    )
    const outcome = await handleArtanisWorkDirectionDelivery(delivery(), {
      recordLifecycle: async event => {
        lifecycleKinds.push(event.lifecycleKind)
      },
    })
    expect(outcome).toMatchObject({
      kind: 'verified',
      paymentMode: 'no-spend',
    })
    expect(lifecycleKinds).toEqual(['delivered', 'accepted'])
    expect(JSON.stringify(outcome)).not.toMatch(/settled|refund|releaseReceipt/)
  })
})
