import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import { applyWorldCommand, makeEmptyHotState } from "./commands"
import {
  encodeAlarmTimestamp,
  expireWorldHotState,
  expireWorldHotStateAt,
  makeStaticWorldClockLayer,
  nextExpiryAlarmAt,
  pruneExpiredStorageCheckpoints,
} from "./expiry"

const regionRef = "region.run.1"
const actorRef = "actor.public.alice"

const command = (name: string, payload: unknown, seq: number, issuedAt: string) => ({
  schemaVersion: "openagents.world_contract.v1",
  commandRef: `command.${name}.${seq}`,
  command: name,
  actorClass: "browser",
  actorRef,
  regionRef,
  seq,
  issuedAt,
  payload,
})

const apply = (state: ReturnType<typeof makeEmptyHotState>, name: string, payload: unknown, seq: number, observedAt: string) =>
  Effect.runPromise(applyWorldCommand(state, command(name, payload, seq, observedAt), observedAt))

describe("world hot-state expiry", () => {
  test("fake clock advances TTLs and emits exactly one delete delta per cursor window", async () => {
    const joined = await apply(makeEmptyHotState(regionRef), "join_region", { characterId: "pilot" }, 1, "2026-06-22T00:00:00.000Z")
    const before = await Effect.runPromise(
      Effect.provide(
        expireWorldHotState(joined.state),
        makeStaticWorldClockLayer("2026-06-22T00:00:29.999Z"),
      ),
    )
    const expired = await Effect.runPromise(
      Effect.provide(
        expireWorldHotState(joined.state),
        makeStaticWorldClockLayer("2026-06-22T00:00:30.000Z"),
      ),
    )
    const again = expireWorldHotStateAt(expired.state, "2026-06-22T00:00:31.000Z")

    expect(before.delta).toBeNull()
    expect(expired.delta?.kind).toBe("delete")
    expect(expired.expiredRefs).toHaveLength(1)
    expect(again.delta).toBeNull()
  })

  test("expires chat, emote, focus, and intent refs deterministically", async () => {
    let state = (await apply(makeEmptyHotState(regionRef), "join_region", { characterId: "pilot" }, 1, "2026-06-22T00:00:00.000Z")).state
    state = (await apply(state, "send_emote", { emote: "wave" }, 2, "2026-06-22T00:00:01.000Z")).state
    state = (await apply(state, "set_agent_intent", { text: "watching" }, 3, "2026-06-22T00:00:02.000Z")).state
    state = (await apply(state, "focus_pylon", { pylonRef: "pylon.station.alpha" }, 4, "2026-06-22T00:00:03.000Z")).state
    state = (await apply(state, "send_local_message", { text: "hello" }, 5, "2026-06-22T00:00:04.000Z")).state

    const emoteExpired = expireWorldHotStateAt(state, "2026-06-22T00:00:11.000Z")
    const intentExpired = expireWorldHotStateAt(emoteExpired.state, "2026-06-22T00:00:17.000Z")
    const focusExpired = expireWorldHotStateAt(intentExpired.state, "2026-06-22T00:00:33.000Z")
    const chatExpired = expireWorldHotStateAt(focusExpired.state, "2026-06-22T00:01:04.000Z")

    expect(emoteExpired.expiredRefs.some(ref => ref.startsWith("emote.world.local."))).toBe(true)
    expect(intentExpired.expiredRefs.some(ref => ref.startsWith("intent.world.agent."))).toBe(true)
    expect(focusExpired.expiredRefs.some(ref => ref.startsWith("intent.world.focus."))).toBe(true)
    expect(chatExpired.expiredRefs.some(ref => ref.startsWith("message.world.local."))).toBe(true)
  })

  test("hibernation or restart can resume expiry from persisted metadata", async () => {
    const joined = await apply(makeEmptyHotState(regionRef), "join_region", { characterId: "pilot" }, 1, "2026-06-22T00:00:00.000Z")
    const restored = {
      ...makeEmptyHotState(regionRef),
      sequence: joined.state.sequence,
      minReplaySeq: joined.state.minReplaySeq,
      expiringRefs: joined.state.expiringRefs,
    }
    const expired = expireWorldHotStateAt(restored, "2026-06-22T00:00:30.000Z")

    expect(expired.delta?.kind).toBe("delete")
    expect(expired.expiredRefs).toEqual(Object.keys(joined.state.expiringRefs))
  })

  test("alarm scheduling returns null when idle and the next deadline when work remains", async () => {
    const idle = makeEmptyHotState(regionRef)
    const joined = await apply(idle, "join_region", { characterId: "pilot" }, 1, "2026-06-22T00:00:00.000Z")

    expect(nextExpiryAlarmAt(idle)).toBeNull()
    expect(nextExpiryAlarmAt(joined.state)).toBe("2026-06-22T00:00:30.000Z")
    expect(encodeAlarmTimestamp("2026-06-22T00:00:30.000Z")).toBe(Date.parse("2026-06-22T00:00:30.000Z"))
  })

  test("storage pruning removes only hot checkpoints and never projection rows", () => {
    const pruned = pruneExpiredStorageCheckpoints(
      ["avatar.hot.1", "projection.training_run.1", "message.hot.1"],
      ["avatar.hot.1", "message.hot.1"],
    )

    expect(pruned).toEqual(["projection.training_run.1"])
  })
})
