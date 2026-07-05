import {
  KhalaRuntimeControlIntent,
  KhalaRuntimeControlIntentId,
  KhalaRuntimeControlIntentKind,
  KhalaRuntimeEvent,
  KhalaRuntimeEventId,
  KhalaRuntimeEventKind,
  KhalaRuntimeLane,
  KhalaRuntimeThreadId,
  KhalaRuntimeTurnId,
} from "@openagentsinc/agent-runtime-schema"
import { Schema as S } from "effect"

/**
 * Khala Code runtime entities for AI SDK-shaped control/event sync (#8370).
 *
 * Scope layout:
 * - `scope.user.<owner>` carries `runtime_turn` and body-free
 *   `runtime_control_intent` post-images for owner lists/queues.
 * - `scope.thread.<threadId>` carries those post-images plus full
 *   `runtime_event` entries, including private text/tool stream material.
 * - No runtime entity is ever projected into `scope.public.*`.
 */

export const RUNTIME_TURN_ENTITY_TYPE = "runtime_turn"
export const RUNTIME_CONTROL_INTENT_ENTITY_TYPE = "runtime_control_intent"
export const RUNTIME_EVENT_ENTITY_TYPE = "runtime_event"

export const RuntimeIsoTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/),
)
export type RuntimeIsoTimestamp = typeof RuntimeIsoTimestamp.Type

export const RuntimeOwnerUserId = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
)
export type RuntimeOwnerUserId = typeof RuntimeOwnerUserId.Type

export const RuntimeObservedAt = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(128),
)
export type RuntimeObservedAt = typeof RuntimeObservedAt.Type

export const RuntimeTurnStatus = S.Literals([
  "queued",
  "running",
  "waiting_for_input",
  "completed",
  "failed",
  "interrupted",
  "closed",
])
export type RuntimeTurnStatus = typeof RuntimeTurnStatus.Type

export const RuntimeControlIntentStatus = S.Literals([
  "accepted",
  "settled",
])
export type RuntimeControlIntentStatus =
  typeof RuntimeControlIntentStatus.Type

export class RuntimeTurnEntity extends S.Class<RuntimeTurnEntity>(
  "RuntimeTurnEntity",
)({
  turnId: KhalaRuntimeTurnId,
  threadId: KhalaRuntimeThreadId,
  ownerUserId: RuntimeOwnerUserId,
  lane: KhalaRuntimeLane,
  status: RuntimeTurnStatus,
  eventCount: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  latestIntentId: S.NullOr(KhalaRuntimeControlIntentId),
  startedAt: S.NullOr(RuntimeIsoTimestamp),
  settledAt: S.NullOr(RuntimeIsoTimestamp),
  createdAt: RuntimeIsoTimestamp,
  updatedAt: RuntimeIsoTimestamp,
}) {}

export class RuntimeControlIntentEntity extends S.Class<RuntimeControlIntentEntity>(
  "RuntimeControlIntentEntity",
)({
  intentId: KhalaRuntimeControlIntentId,
  threadId: KhalaRuntimeThreadId,
  turnId: S.NullOr(KhalaRuntimeTurnId),
  ownerUserId: RuntimeOwnerUserId,
  kind: KhalaRuntimeControlIntentKind,
  status: RuntimeControlIntentStatus,
  intent: KhalaRuntimeControlIntent,
  createdAt: RuntimeIsoTimestamp,
  updatedAt: RuntimeIsoTimestamp,
}) {}

export class RuntimeEventEntity extends S.Class<RuntimeEventEntity>(
  "RuntimeEventEntity",
)({
  eventId: KhalaRuntimeEventId,
  turnId: KhalaRuntimeTurnId,
  threadId: KhalaRuntimeThreadId,
  ownerUserId: RuntimeOwnerUserId,
  kind: KhalaRuntimeEventKind,
  sequence: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  observedAt: RuntimeObservedAt,
  event: KhalaRuntimeEvent,
  createdAt: RuntimeIsoTimestamp,
}) {}

export const decodeRuntimeTurnEntity = S.decodeUnknownSync(RuntimeTurnEntity)
export const decodeRuntimeControlIntentEntity = S.decodeUnknownSync(
  RuntimeControlIntentEntity,
)
export const decodeRuntimeEventEntity = S.decodeUnknownSync(RuntimeEventEntity)
export const encodeRuntimeTurnEntity = S.encodeSync(RuntimeTurnEntity)
export const encodeRuntimeControlIntentEntity = S.encodeSync(
  RuntimeControlIntentEntity,
)
export const encodeRuntimeEventEntity = S.encodeSync(RuntimeEventEntity)

/**
 * One durable runtime control-intent row as served by the dispatch-consumer
 * polling seam (#8388): `readPendingRuntimeControlIntents` in
 * `@openagentsinc/khala-sync-server` and the Worker's admin-guarded
 * `GET /api/internal/khala-sync/runtime-intents` route. Mirrors
 * `FleetIntentRow` from ./fleet.ts — NOT a sync-protocol message, the
 * polling contract for the Pylon-side runtime dispatch consumer. `seq` is
 * the monotonic identity column added by khala-sync-server migration 0032
 * (the control-intents table's own primary key, `intentId`, is a
 * client-minted text id, not a resumable watermark).
 */
export class RuntimeControlIntentRow extends S.Class<RuntimeControlIntentRow>(
  "RuntimeControlIntentRow",
)({
  seq: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  intentId: KhalaRuntimeControlIntentId,
  threadId: KhalaRuntimeThreadId,
  turnId: S.NullOr(KhalaRuntimeTurnId),
  ownerUserId: RuntimeOwnerUserId,
  kind: KhalaRuntimeControlIntentKind,
  status: RuntimeControlIntentStatus,
  intent: KhalaRuntimeControlIntent,
  createdAt: RuntimeIsoTimestamp,
  updatedAt: RuntimeIsoTimestamp,
}) {}

export const decodeRuntimeControlIntentRow = S.decodeUnknownSync(
  RuntimeControlIntentRow,
)
export const encodeRuntimeControlIntentRow = S.encodeSync(
  RuntimeControlIntentRow,
)

export {
  decodeKhalaRuntimeControlIntent,
  decodeKhalaRuntimeEvent,
  KhalaRuntimeControlIntentSchemaLiteral,
  KhalaRuntimeEventSchemaLiteral,
  KhalaRuntimeSafeRef,
} from "@openagentsinc/agent-runtime-schema"
export type {
  KhalaRuntimeControlIntent,
  KhalaRuntimeControlIntentKind,
  KhalaRuntimeEvent,
  KhalaRuntimeEventKind,
  KhalaRuntimeFinishReason,
  KhalaRuntimeLane,
} from "@openagentsinc/agent-runtime-schema"
