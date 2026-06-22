import { Context, Effect, Layer } from "effect"

import {
  WORLD_DELTA_SCHEMA_VERSION,
  decodeWorldDelta,
  type WorldDelta,
} from "@openagentsinc/world-contract"

import { cursorForSequence, stableWorldRef } from "./protocol"
import type { WorldExpiringRef, WorldHotState } from "./commands"

export class WorldClock extends Context.Service<
  WorldClock,
  {
    readonly now: Effect.Effect<string>
  }
>()("WorldClock") {}

export const makeStaticWorldClockLayer = (now: string) =>
  Layer.succeed(WorldClock, { now: Effect.succeed(now) })

export type WorldExpiryPlan = Readonly<{
  state: WorldHotState
  delta: WorldDelta | null
  expiredRefs: ReadonlyArray<string>
  nextAlarmAt: string | null
}>

export const expireWorldHotState = (
  state: WorldHotState,
): Effect.Effect<WorldExpiryPlan, never, WorldClock> =>
  Effect.gen(function* () {
    const clock = yield* WorldClock
    const now = yield* clock.now
    return expireWorldHotStateAt(state, now)
  })

export const expireWorldHotStateAt = (
  state: WorldHotState,
  now: string,
): WorldExpiryPlan => {
  const nowMs = Date.parse(now)
  const expiringRefs = Object.values(state.expiringRefs)
  const expired = expiringRefs
    .filter(item => Date.parse(item.expiresAt) <= nowMs)
    .sort((a, b) => a.expiresAt.localeCompare(b.expiresAt) || a.ref.localeCompare(b.ref))
  const remaining = expiringRefs
    .filter(item => Date.parse(item.expiresAt) > nowMs)
    .sort((a, b) => a.expiresAt.localeCompare(b.expiresAt) || a.ref.localeCompare(b.ref))

  if (expired.length === 0) {
    return {
      state,
      delta: null,
      expiredRefs: [],
      nextAlarmAt: remaining[0]?.expiresAt ?? null,
    }
  }

  const expiredRefs = expired.map(item => item.ref)
  const expiredSet = new Set(expiredRefs)
  const sequence = state.sequence + 1
  const nextState: WorldHotState = {
    ...state,
    sequence,
    minReplaySeq: Math.max(0, sequence - 256),
    avatars: removeRefs(state.avatars, expired.filter(isAvatarExpiry).map(item => item.ref)),
    focusByActor: removeFocusForExpiredRefs(state.focusByActor, expiredSet),
    expiringRefs: Object.fromEntries(remaining.map(item => [item.ref, item])),
  }

  return {
    state: nextState,
    delta: decodeWorldDelta({
      schemaVersion: WORLD_DELTA_SCHEMA_VERSION,
      deltaRef: stableWorldRef("delta.world.expiry", `${state.regionRef}:${sequence}:${expiredRefs.join(",")}`),
      kind: "delete",
      regionRef: state.regionRef,
      cursor: cursorForSequence(state.regionRef, sequence),
      generatedAt: now,
      deletedRefs: expiredRefs,
    }),
    expiredRefs,
    nextAlarmAt: remaining[0]?.expiresAt ?? null,
  }
}

export const nextExpiryAlarmAt = (state: WorldHotState): string | null =>
  Object.values(state.expiringRefs)
    .sort((a, b) => a.expiresAt.localeCompare(b.expiresAt) || a.ref.localeCompare(b.ref))[0]
    ?.expiresAt ?? null

export const pruneExpiredStorageCheckpoints = (
  checkpointRefs: ReadonlyArray<string>,
  expiredRefs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const expired = new Set(expiredRefs)
  return checkpointRefs.filter(ref => !expired.has(ref))
}

const isAvatarExpiry = (item: WorldExpiringRef) => item.kind === "avatar"

const removeRefs = <Value>(
  input: Readonly<Record<string, Value>>,
  refs: ReadonlyArray<string>,
): Readonly<Record<string, Value>> => {
  const remove = new Set(refs)
  return Object.fromEntries(Object.entries(input).filter(([key]) => !remove.has(key)))
}

const removeFocusForExpiredRefs = (
  focusByActor: Readonly<Record<string, string>>,
  expiredRefs: ReadonlySet<string>,
): Readonly<Record<string, string>> =>
  Object.fromEntries(
    Object.entries(focusByActor).filter(([actorRef, focusRef]) =>
      !expiredRefs.has(focusRef) && !expiredRefs.has(stableWorldRef("intent.world.focus", actorRef))
    ),
  )

export const encodeAlarmTimestamp = (iso: string): number =>
  Date.parse(iso)

export type ExpiryBroadcast = Readonly<{
  frameKind: "delta"
  delta: WorldDelta
}>

export const expiryDeltaFrame = (delta: WorldDelta): ExpiryBroadcast => ({
  frameKind: "delta",
  delta,
})
