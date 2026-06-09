import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type BlueprintObjectiveRun,
  type BlueprintObjectiveType,
  blueprintAcceptedOutcomeWorkKindMatches,
  BlueprintObjectiveRun as BlueprintObjectiveRunSchema,
  blueprintObjectiveRequiredReleaseGateRefs,
  blueprintObjectiveRunHasAcceptedOutcome,
  blueprintObjectiveTypeAllowsSurface,
  BlueprintObjectiveType as BlueprintObjectiveTypeSchema,
} from './objective'

const objectiveTypeFixture: BlueprintObjectiveType = {
  allowedSurfaces: ['agent_api', 'customer_dashboard', 'public_site'],
  budgetPolicies: [
    {
      budgetKind: 'credits',
      budgetRef: 'budget.site_revision_credits',
      enforcement: 'hard',
      limit: 10,
    },
  ],
  defaultWorkKind: 'site',
  descriptionRef: 'brief.otc_site.description',
  guardrailPolicies: [
    {
      evidenceRefs: ['evidence.no_raw_customer_material'],
      policyRef: 'policy.public_safe_projection',
      severity: 'blocking',
    },
  ],
  id: 'objective_type.site_revision',
  metricRefs: [
    {
      metricRef: 'metric.customer_review_ready',
      required: true,
      weight: 1,
    },
  ],
  releaseGates: [
    {
      evidenceRefs: ['receipt.build_passed'],
      gateKind: 'build_passed',
      gateRef: 'gate.build_passed',
      required: true,
    },
    {
      evidenceRefs: ['receipt.email_sent'],
      gateKind: 'email_sent',
      gateRef: 'gate.review_email_sent',
      required: false,
    },
  ],
  rewardRef: 'reward.accepted_site_revision',
  riskPolicies: [
    {
      mitigationRefs: ['mitigation.operator_review'],
      riskKind: 'public_content',
      riskRef: 'risk.public_site_quality',
      severity: 'required',
    },
  ],
  titleRef: 'title.site_revision',
  utilityRef: 'utility.customer_visible_fulfillment',
}

const objectiveRunFixture: BlueprintObjectiveRun = {
  acceptedOutcomeLink: {
    acceptanceState: 'pending_review',
    acceptedOutcomeContractId: 'omni_accepted_outcome_contract_1',
    publicReceiptRef: 'receipt.site_revision_2',
    workKind: 'site',
  },
  allowedSurfaces: ['agent_api', 'customer_dashboard', 'public_site'],
  createdAt: '2026-06-05T00:00:00.000Z',
  id: 'objective_run.site_revision_2',
  objectiveTypeId: 'objective_type.site_revision',
  outcomeEvidenceRefs: ['artifact.site_revision_2'],
  programRunId: 'program_run.site_revision_2',
  releaseGateRefs: ['gate.build_passed'],
  status: 'waiting_review',
  updatedAt: '2026-06-05T00:00:00.000Z',
  workKind: 'site',
  workroomId: 'omni_workroom_1',
}

describe('Blueprint Objective and Outcome schemas', () => {
  test('decode Objective Type with surfaces, policy refs, budgets, risks, metrics, and gates', () => {
    expect(
      S.decodeUnknownSync(BlueprintObjectiveTypeSchema)(objectiveTypeFixture),
    ).toEqual(objectiveTypeFixture)
    expect(
      blueprintObjectiveTypeAllowsSurface(objectiveTypeFixture, 'public_site'),
    ).toBe(true)
    expect(blueprintObjectiveRequiredReleaseGateRefs(objectiveTypeFixture)).toEqual(
      ['gate.build_passed'],
    )
  })

  test('decode Objective Run with accepted outcome linkage', () => {
    expect(
      S.decodeUnknownSync(BlueprintObjectiveRunSchema)(objectiveRunFixture),
    ).toEqual(objectiveRunFixture)
    expect(blueprintObjectiveRunHasAcceptedOutcome(objectiveRunFixture)).toBe(
      true,
    )
    expect(
      blueprintAcceptedOutcomeWorkKindMatches(objectiveRunFixture, 'site'),
    ).toBe(true)
  })

  test('supports evidence-only runs before an outcome has been accepted', () => {
    const runWithoutOutcome: BlueprintObjectiveRun = {
      ...objectiveRunFixture,
      acceptedOutcomeLink: null,
      outcomeEvidenceRefs: [],
      status: 'active',
    }

    expect(
      S.decodeUnknownSync(BlueprintObjectiveRunSchema)(runWithoutOutcome),
    ).toEqual(runWithoutOutcome)
    expect(blueprintObjectiveRunHasAcceptedOutcome(runWithoutOutcome)).toBe(
      false,
    )
    expect(
      blueprintAcceptedOutcomeWorkKindMatches(runWithoutOutcome, 'site'),
    ).toBe(false)
  })
})
