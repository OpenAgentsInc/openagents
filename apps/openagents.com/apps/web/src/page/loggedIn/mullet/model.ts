import { Schema as S } from 'effect'
import { ts } from 'foldkit/schema'

import {
  assumptionsForTemplate,
  defaultMulletScenarioTemplateId,
  defaultMulletSensitivityAxisId,
} from './workbench'

export const MulletAccess = S.Struct({
  operatorEmail: S.String,
  visibility: S.Literal('private'),
})
export type MulletAccess = typeof MulletAccess.Type

export const MulletAuthorityBoundary = S.Struct({
  canAssignLiveWork: S.Boolean,
  canMutateProviders: S.Boolean,
  canPromotePublicClaims: S.Boolean,
  canSettlePayouts: S.Boolean,
  canSpendWalletFunds: S.Boolean,
})
export type MulletAuthorityBoundary = typeof MulletAuthorityBoundary.Type

export const MulletBootstrapResponse = S.Struct({
  access: MulletAccess,
  authorityBoundary: MulletAuthorityBoundary,
  routes: S.Array(S.String),
  schemaVersion: S.String,
})
export type MulletBootstrapResponse = typeof MulletBootstrapResponse.Type

export const MulletBootstrapIdle = ts('MulletBootstrapIdle', {})
export const MulletBootstrapLoading = ts('MulletBootstrapLoading', {})
export const MulletBootstrapLoaded = ts('MulletBootstrapLoaded', {
  response: MulletBootstrapResponse,
})
export const MulletBootstrapFailed = ts('MulletBootstrapFailed', {
  error: S.String,
})
export const MulletBootstrapState = S.Union([
  MulletBootstrapIdle,
  MulletBootstrapLoading,
  MulletBootstrapLoaded,
  MulletBootstrapFailed,
])
export type MulletBootstrapState = typeof MulletBootstrapState.Type

export const MulletAssumptionGroup = S.Literals([
  'facility',
  'power',
  'mining fleet',
  'hardware',
  'work class',
  'provider floor',
  'party split',
  'capital',
])

export const MulletAssumptionProvenance = S.Literals([
  'public_claim',
  'customer_reported',
  'manual_input',
  'estimated',
  'modeled',
  'forecast',
  'observed',
  'measured',
  'verified',
  'accepted',
  'paid',
  'settled',
  'placeholder',
])

export const MulletValueState = S.Literals([
  'modeled',
  'measured',
  'accepted',
  'paid',
  'settled',
  'placeholder',
])

export const MulletAssumption = S.Struct({
  confidence: S.Number,
  draftValue: S.String,
  group: MulletAssumptionGroup,
  id: S.String,
  label: S.String,
  provenance: MulletAssumptionProvenance,
  requiredEvidence: S.String,
  sourceLabel: S.String,
  state: MulletValueState,
  unit: S.String,
})
export type MulletAssumption = typeof MulletAssumption.Type

export const MulletModel = ts('MulletModel', {
  assumptions: S.Array(MulletAssumption),
  bootstrap: MulletBootstrapState,
  selectedSensitivityAxisId: S.String,
  selectedTemplateId: S.String,
})
export type MulletModel = typeof MulletModel.Type

export const init = (): MulletModel =>
  MulletModel({
    assumptions: [...assumptionsForTemplate(defaultMulletScenarioTemplateId)],
    bootstrap: MulletBootstrapIdle(),
    selectedSensitivityAxisId: defaultMulletSensitivityAxisId,
    selectedTemplateId: defaultMulletScenarioTemplateId,
  })
