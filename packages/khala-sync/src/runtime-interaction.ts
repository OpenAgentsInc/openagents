import {
  RuntimeInteraction,
  RuntimeInteractionKind,
  RuntimeInteractionStatus,
} from "@openagentsinc/agent-runtime-schema"
import { Schema as S } from "effect"

import { RuntimeIsoTimestamp, RuntimeOwnerUserId } from "./runtime.js"

/**
 * Owner-private durable provider interaction (CUT-16).
 *
 * The whole post-image belongs only in the exact `scope.thread.<threadId>`
 * scope. Personal/inbox projections must derive bounded attention rows rather
 * than copying question text, option labels, tool detail, or decisions.
 */
export const RUNTIME_INTERACTION_ENTITY_TYPE = "runtime_interaction" as const

const RuntimeInteractionEntityFields = {
  interactionRef: S.String,
  threadId: S.String,
  turnId: S.String,
  ownerUserId: RuntimeOwnerUserId,
  kind: RuntimeInteractionKind,
  status: RuntimeInteractionStatus,
  interaction: RuntimeInteraction,
  createdAt: RuntimeIsoTimestamp,
  updatedAt: RuntimeIsoTimestamp,
} as const

export const RuntimeInteractionEntity = S.Struct(
  RuntimeInteractionEntityFields,
).pipe(
  S.check(
    S.makeFilter(
      entity => entity.interactionRef === entity.interaction.interactionRef &&
        entity.threadId === entity.interaction.threadId &&
        entity.turnId === entity.interaction.turnId &&
        entity.kind === entity.interaction.payload.kind &&
        entity.status === entity.interaction.lifecycle.status,
      {
        message:
          "runtime interaction entity identity and lifecycle must match its post-image",
      },
    ),
  ),
)
export type RuntimeInteractionEntity = typeof RuntimeInteractionEntity.Type

export const decodeRuntimeInteractionEntity = S.decodeUnknownSync(
  RuntimeInteractionEntity,
)
export const encodeRuntimeInteractionEntity = S.encodeSync(
  RuntimeInteractionEntity,
)

export {
  applyRuntimeInteractionDecision,
  decodeRuntimeInteraction,
  decodeRuntimeInteractionDecisionEnvelope,
  projectRuntimeInteraction,
  RuntimeInteraction,
  RuntimeInteractionDecision,
  RuntimeInteractionDecisionEnvelope,
  RuntimeInteractionKind,
  RuntimeInteractionLifecycle,
  RuntimeInteractionPayload,
  RuntimeInteractionProjection,
  RuntimeInteractionSchemaLiteral,
  RuntimeInteractionStatus,
} from "@openagentsinc/agent-runtime-schema"
export type {
  RuntimeInteractionDecisionResult,
} from "@openagentsinc/agent-runtime-schema"
