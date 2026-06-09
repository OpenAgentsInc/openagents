import { Schema as S } from 'effect'

import type { OmniAcceptedOutcomeEconomicsRecord } from './omni-accepted-outcome-economics'
import {
  operatorOmniAcceptedOutcomeEconomicsProjection,
  publicOmniAcceptedOutcomeEconomicsProjection,
} from './omni-accepted-outcome-economics'
import type { OmniProjectionAudience } from './omni-data-classification'
import { omniClassificationProjection } from './omni-data-classification'
import type { OmniEvidenceBundleRecord } from './omni-evidence-bundles'
import {
  customerOmniEvidenceBundleProjection,
  operatorOmniEvidenceBundleProjection,
  publicOmniEvidenceBundleProjection,
} from './omni-evidence-bundles'
import type { OmniRouteScorecardRecord } from './omni-route-scorecards'
import {
  customerOmniRouteScorecardProjection,
  operatorOmniRouteScorecardProjection,
  publicOmniRouteScorecardProjection,
} from './omni-route-scorecards'
import type { OmniWorkroomRecord } from './omni-workrooms'
import {
  customerOmniWorkroomProjection,
  operatorOmniWorkroomProjection,
  publicOmniWorkroomProjection,
} from './omni-workrooms'
import type { OmniWorkroomLifecycleDecisionRecord } from './omni-workroom-lifecycle'
import {
  customerOmniWorkroomLifecycleProjection,
  operatorOmniWorkroomLifecycleProjection,
  publicOmniWorkroomLifecycleProjection,
} from './omni-workroom-lifecycle'

export const OmniWorkroomProjectionSurface = S.Literals([
  'public',
  'customer',
  'team',
  'agent',
  'operator',
])
export type OmniWorkroomProjectionSurface =
  typeof OmniWorkroomProjectionSurface.Type

export type BuildOmniWorkroomSurfaceProjectionInput = Readonly<{
  economics?: ReadonlyArray<OmniAcceptedOutcomeEconomicsRecord> | undefined
  evidenceBundles?: ReadonlyArray<OmniEvidenceBundleRecord> | undefined
  lifecycleDecisions?:
    | ReadonlyArray<OmniWorkroomLifecycleDecisionRecord>
    | undefined
  routeScorecards?: ReadonlyArray<OmniRouteScorecardRecord> | undefined
  surface: OmniWorkroomProjectionSurface
  workroom: OmniWorkroomRecord
}>

const audienceForSurface = (
  surface: OmniWorkroomProjectionSurface,
): OmniProjectionAudience => {
  switch (surface) {
    case 'agent':
      return 'customer'
    case 'customer':
      return 'customer'
    case 'operator':
      return 'operator'
    case 'public':
      return 'public'
    case 'team':
      return 'team'
  }
}

const teamWorkroomProjection = (workroom: OmniWorkroomRecord) => {
  const classification = omniClassificationProjection(workroom, 'team')

  return {
    ...classification,
    acceptedOutcomeContractId: workroom.acceptedOutcomeContractId,
    artifactRefs: workroom.artifactRefs,
    blockerRefs: workroom.blockerRefs,
    publicReceiptRef: workroom.publicReceiptRef,
    receiptRefs: workroom.receiptRefs,
    siteId: workroom.siteId,
    softwareOrderId: workroom.softwareOrderId,
    sourceRefs: workroom.sourceRefs,
    status: workroom.status,
    workKind: workroom.workKind,
  }
}

const agentWorkroomProjection = (workroom: OmniWorkroomRecord) => {
  const customerProjection = customerOmniWorkroomProjection(workroom)

  return {
    artifactRefs: customerProjection.artifactRefs,
    classificationCaveatRef: customerProjection.classificationCaveatRef,
    dataClassification: customerProjection.dataClassification,
    publicReceiptRef: customerProjection.publicReceiptRef,
    receiptRefs: customerProjection.receiptRefs,
    siteId: customerProjection.siteId,
    softwareOrderId: customerProjection.softwareOrderId,
    status: customerProjection.status,
    trustTier: customerProjection.trustTier,
    workKind: customerProjection.workKind,
  }
}

export const buildOmniWorkroomSurfaceProjection = (
  input: BuildOmniWorkroomSurfaceProjectionInput,
) => {
  const audience = audienceForSurface(input.surface)
  omniClassificationProjection(input.workroom, audience)

  const evidenceBundles = input.evidenceBundles ?? []
  const lifecycleDecisions = input.lifecycleDecisions ?? []
  const economics = input.economics ?? []
  const routeScorecards = input.routeScorecards ?? []

  switch (input.surface) {
    case 'public':
      return {
        economics: economics.map(publicOmniAcceptedOutcomeEconomicsProjection),
        evidenceBundles: evidenceBundles.map(publicOmniEvidenceBundleProjection),
        lifecycleDecisions: lifecycleDecisions.map(
          publicOmniWorkroomLifecycleProjection,
        ),
        routeScorecards: routeScorecards.map(publicOmniRouteScorecardProjection),
        surface: input.surface,
        workroom: publicOmniWorkroomProjection(input.workroom),
      }
    case 'customer':
      return {
        economics: economics.map(publicOmniAcceptedOutcomeEconomicsProjection),
        evidenceBundles: evidenceBundles.map(customerOmniEvidenceBundleProjection),
        lifecycleDecisions: lifecycleDecisions.map(
          customerOmniWorkroomLifecycleProjection,
        ),
        routeScorecards: routeScorecards.map(customerOmniRouteScorecardProjection),
        surface: input.surface,
        workroom: customerOmniWorkroomProjection(input.workroom),
      }
    case 'team':
      return {
        economics: economics.map(publicOmniAcceptedOutcomeEconomicsProjection),
        evidenceBundles: evidenceBundles.map(customerOmniEvidenceBundleProjection),
        lifecycleDecisions: lifecycleDecisions.map(
          customerOmniWorkroomLifecycleProjection,
        ),
        routeScorecards: routeScorecards.map(customerOmniRouteScorecardProjection),
        surface: input.surface,
        workroom: teamWorkroomProjection(input.workroom),
      }
    case 'agent':
      return {
        economics: economics.map(publicOmniAcceptedOutcomeEconomicsProjection),
        evidenceBundles: evidenceBundles.map(customerOmniEvidenceBundleProjection),
        lifecycleDecisions: lifecycleDecisions.map(
          customerOmniWorkroomLifecycleProjection,
        ),
        routeScorecards: routeScorecards.map(publicOmniRouteScorecardProjection),
        surface: input.surface,
        workroom: agentWorkroomProjection(input.workroom),
      }
    case 'operator':
      return {
        economics: economics.map(operatorOmniAcceptedOutcomeEconomicsProjection),
        evidenceBundles: evidenceBundles.map(operatorOmniEvidenceBundleProjection),
        lifecycleDecisions: lifecycleDecisions.map(
          operatorOmniWorkroomLifecycleProjection,
        ),
        routeScorecards: routeScorecards.map(operatorOmniRouteScorecardProjection),
        surface: input.surface,
        workroom: operatorOmniWorkroomProjection(input.workroom),
      }
  }
}
