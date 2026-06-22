import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  applyWorldCommand,
  commandNamesImplementedInRegionDo,
  makeEmptyHotState,
  type WorldHotState,
} from "./commands"

const regionRef = "region.run.1"
const actorRef = "actor.public.alice"

const command = (
  name: string,
  payload: unknown,
  seq = 1,
  issuedAt = "2026-06-22T00:00:00.000Z",
) => ({
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

const apply = async (
  state: WorldHotState,
  name: string,
  payload: unknown,
  seq = 1,
  observedAt = "2026-06-22T00:00:00.000Z",
) =>
  Effect.runPromise(applyWorldCommand(state, command(name, payload, seq, observedAt), observedAt))

describe("world command handlers", () => {
  test("implements the P4 browser command set", () => {
    expect(commandNamesImplementedInRegionDo).toEqual([
      "join_region",
      "leave_region",
      "set_avatar_position",
      "focus_pylon",
      "clear_pylon_focus",
      "send_local_message",
      "send_pylon_message",
      "send_emote",
      "set_agent_intent",
    ])
  })

  test("browser actors cannot write service-only projection rows", async () => {
    const result = await apply(
      makeEmptyHotState(regionRef),
      "upsert_training_run",
      { runRef: "run.public.1" },
    )

    expect(result.receipt.status).toBe("rejected")
    expect(result.receipt.error?.tag).toBe("auth")
    expect(result.delta.kind).toBe("diagnostic")
  })

  test("join, move, focus, chat, emote, intent, and leave return typed receipts", async () => {
    let result = await apply(makeEmptyHotState(regionRef), "join_region", {
      characterId: "pilot",
      label: "Pilot",
    }, 1)
    expect(result.receipt.status).toBe("applied")
    expect(result.delta.rows?.[0]?.kind).toBe("agent_avatar")

    result = await apply(result.state, "set_avatar_position", {
      position: { x: 1, y: 0, z: 1 },
      rotationY: 0,
      animation: "walk",
    }, 2, "2026-06-22T00:00:01.000Z")
    expect(Number(result.receipt.appliedSeq)).toBe(2)
    expect(result.delta.rows?.[0]?.kind).toBe("avatar_position")

    result = await apply(result.state, "focus_pylon", {
      pylonRef: "pylon.station.alpha",
    }, 3, "2026-06-22T00:00:02.000Z")
    expect(result.delta.rows?.[0]?.kind).toBe("agent_intent")

    result = await apply(result.state, "send_local_message", {
      text: "hello world",
    }, 4, "2026-06-22T00:00:03.000Z")
    expect(result.delta.rows?.[0]?.kind).toBe("local_chat_message")

    result = await apply(result.state, "send_emote", {
      emote: "wave",
    }, 5, "2026-06-22T00:00:04.000Z")
    expect(result.delta.rows?.[0]?.kind).toBe("local_emote")

    result = await apply(result.state, "set_agent_intent", {
      text: "inspecting pylon",
    }, 6, "2026-06-22T00:00:05.000Z")
    expect(result.delta.rows?.[0]?.kind).toBe("agent_intent")

    result = await apply(result.state, "leave_region", {}, 7, "2026-06-22T00:00:06.000Z")
    expect(result.delta.kind).toBe("delete")
    expect(result.delta.deletedRefs).toHaveLength(1)
  })

  test("pose commands reject bounds, duplicate sequence, cadence, and velocity", async () => {
    let result = await apply(makeEmptyHotState(regionRef), "join_region", { characterId: "pilot" }, 1)
    result = await apply(result.state, "set_avatar_position", {
      position: { x: 0, y: 0, z: 0 },
    }, 2, "2026-06-22T00:00:01.000Z")

    const duplicate = await apply(result.state, "set_avatar_position", {
      position: { x: 0.1, y: 0, z: 0.1 },
    }, 2, "2026-06-22T00:00:02.000Z")
    const outOfBounds = await apply(result.state, "set_avatar_position", {
      position: { x: 999, y: 0, z: 0 },
    }, 3, "2026-06-22T00:00:02.000Z")
    const cadence = await apply(result.state, "set_avatar_position", {
      position: { x: 0.1, y: 0, z: 0.1 },
    }, 3, "2026-06-22T00:00:01.010Z")
    const velocity = await apply(result.state, "set_avatar_position", {
      position: { x: 100, y: 0, z: 0 },
    }, 3, "2026-06-22T00:00:02.000Z")

    expect(duplicate.receipt.status).toBe("rejected")
    expect(outOfBounds.receipt.status).toBe("rejected")
    expect(cadence.receipt.status).toBe("rejected")
    expect(velocity.receipt.status).toBe("rejected")
  })

  test("chat, emote, and intent enforce plain text and cadence", async () => {
    let result = await apply(makeEmptyHotState(regionRef), "join_region", { characterId: "pilot" }, 1)
    result = await apply(result.state, "send_local_message", {
      text: "hello <script>",
    }, 2, "2026-06-22T00:00:01.000Z")
    expect(result.delta.rows?.[0]?.kind).toBe("local_chat_message")
    if (result.delta.rows?.[0]?.kind === "local_chat_message") {
      expect(result.delta.rows[0].text).toBe("hello script")
    }

    const chatCadence = await apply(result.state, "send_local_message", {
      text: "too soon",
    }, 3, "2026-06-22T00:00:01.100Z")
    expect(chatCadence.receipt.status).toBe("rejected")

    result = await apply(result.state, "send_emote", {
      emote: "wave",
    }, 3, "2026-06-22T00:00:02.100Z")
    const emoteCadence = await apply(result.state, "send_emote", {
      emote: "wave",
    }, 4, "2026-06-22T00:00:02.200Z")
    expect(emoteCadence.receipt.status).toBe("rejected")

    result = await apply(result.state, "set_agent_intent", {
      text: "watch pylon",
    }, 4, "2026-06-22T00:00:03.000Z")
    const intentCadence = await apply(result.state, "set_agent_intent", {
      text: "watch pylon",
    }, 5, "2026-06-22T00:00:03.100Z")
    expect(intentCadence.receipt.status).toBe("rejected")
  })
})
