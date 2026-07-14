import { Schema as S } from 'effect'

import type { AcceptanceSpec } from './acceptance-spec'

export const AcceptanceJobSpecSchema = S.Struct({
  kind: S.Literal('crossy_road_single_html'),
  rubricRef: S.String,
  checks: S.Array(S.String),
  params: S.Struct({
    forwardMoves: S.Number,
    maxCameraDeltaPerMove: S.Number,
    expectedForwardAdvance: S.Number,
    minWorldRowsAhead: S.Number,
  }),
})

export class AcceptanceJobMessage extends S.Class<AcceptanceJobMessage>(
  'AcceptanceJobMessage',
)({
  schemaVersion: S.Literal('openagents.inference.acceptance_job.v1'),
  requestId: S.String,
  artifactRef: S.String,
  servedModel: S.String,
  worker: S.String,
  meteringReceiptRef: S.optionalKey(S.NullOr(S.String)),
  spec: AcceptanceJobSpecSchema,
}) {}

export type AcceptanceJobSpec = S.Schema.Type<typeof AcceptanceJobSpecSchema>

export const acceptanceJobSpecFromSpec = (
  spec: AcceptanceSpec,
): AcceptanceJobSpec => ({
  checks: spec.checks,
  kind: spec.kind,
  params: {
    expectedForwardAdvance: spec.params.expectedForwardAdvance,
    forwardMoves: spec.params.forwardMoves,
    maxCameraDeltaPerMove: spec.params.maxCameraDeltaPerMove,
    minWorldRowsAhead: spec.params.minWorldRowsAhead,
  },
  rubricRef: spec.rubricRef,
})
