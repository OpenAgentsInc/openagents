import {
  RuntimeInteractionKind,
  RuntimeInteractionStatus,
} from "@openagentsinc/agent-runtime-schema"
import { Schema as S } from "effect"

import { RuntimeIsoTimestamp, RuntimeOwnerUserId } from "./runtime.js"
import type { RuntimeInteractionEntity } from "./runtime-interaction.js"

/**
 * Owner-private, body-free attention metadata for the personal Sync scope.
 * Full questions, choices, tool details, plans, and decisions remain only in
 * the exact thread scope's `runtime_interaction` entity.
 */
export const RUNTIME_ATTENTION_ENTITY_TYPE = "runtime_attention" as const
export const RUNTIME_ATTENTION_SCHEMA = "openagents.runtime_attention.v1" as const

const AttentionRef = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)

export const RuntimeAttentionEntity = S.Struct({
  schema: S.Literal(RUNTIME_ATTENTION_SCHEMA),
  attentionRef: AttentionRef,
  ownerUserId: RuntimeOwnerUserId,
  interactionRef: AttentionRef,
  threadRef: AttentionRef,
  turnRef: AttentionRef,
  kind: RuntimeInteractionKind,
  status: RuntimeInteractionStatus,
  requestedAt: RuntimeIsoTimestamp,
  expiresAt: RuntimeIsoTimestamp,
  updatedAt: RuntimeIsoTimestamp,
}).pipe(
  S.check(S.makeFilter(
    value => value.attentionRef === value.interactionRef,
    { message: "runtime attention identity must match its interaction ref" },
  )),
)
export type RuntimeAttentionEntity = typeof RuntimeAttentionEntity.Type

export const decodeRuntimeAttentionEntity = S.decodeUnknownSync(RuntimeAttentionEntity)
export const encodeRuntimeAttentionEntity = S.encodeSync(RuntimeAttentionEntity)

export const runtimeAttentionFromInteraction = (
  entity: RuntimeInteractionEntity,
): RuntimeAttentionEntity => decodeRuntimeAttentionEntity({
  schema: RUNTIME_ATTENTION_SCHEMA,
  attentionRef: entity.interactionRef,
  ownerUserId: entity.ownerUserId,
  interactionRef: entity.interactionRef,
  threadRef: entity.threadId,
  turnRef: entity.turnId,
  kind: entity.kind,
  status: entity.status,
  requestedAt: entity.interaction.requestedAt,
  expiresAt: entity.interaction.expiresAt,
  updatedAt: entity.updatedAt,
})
