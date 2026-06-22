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
  actor = actorRef,
) => ({
  schemaVersion: "openagents.world_contract.v1",
  commandRef: `command.${name}.${seq}`,
  command: name,
  actorClass: "browser",
  actorRef: actor,
  regionRef,
  seq,
  issuedAt,
  payload,
})

const commandFromActorClass = (
  actorClass: "browser" | "agent" | "operator" | "service",
  name: string,
  payload: unknown,
  seq = 1,
  issuedAt = "2026-06-22T00:00:00.000Z",
) => ({
  ...command(name, payload, seq, issuedAt, `${actorClass}.public.test`),
  actorClass,
})

const serviceCommand = (
  name: string,
  payload: unknown,
  seq = 1,
  issuedAt = "2026-06-22T00:00:00.000Z",
) => ({
  ...command(name, payload, seq, issuedAt, "service.openagents.world"),
  actorClass: "service",
})

const apply = async (
  state: WorldHotState,
  name: string,
  payload: unknown,
  seq = 1,
  observedAt = "2026-06-22T00:00:00.000Z",
  options: {
    readonly actorRef?: string
    readonly sessionRef?: string
  } = {},
) =>
  Effect.runPromise(applyWorldCommand(
    state,
    command(name, payload, seq, observedAt, options.actorRef ?? actorRef),
    observedAt,
    options.sessionRef === undefined ? {} : { sessionRef: options.sessionRef },
  ))

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

  test("agent and operator actors cannot cross into service projection authority", async () => {
    const state = makeEmptyHotState(regionRef)
    const agent = await Effect.runPromise(applyWorldCommand(
      state,
      commandFromActorClass("agent", "append_world_event", {
        row: {
          kind: "world_event",
          eventRef: "world_event.public.agent.1",
          eventKind: "attempted_projection",
          label: "Agent attempted projection",
          occurredAt: "2026-06-22T00:00:00.000Z",
          sourceRefs: ["source.public.test"],
          safety: {
            publicProjectionAllowed: true,
            sourceRefs: ["source.public.test"],
            blockerRefs: [],
            caveatRefs: [],
          },
        },
      }),
      "2026-06-22T00:00:00.000Z",
    ))
    const operator = await Effect.runPromise(applyWorldCommand(
      state,
      commandFromActorClass("operator", "record_bridge_health", {
        row: {
          kind: "bridge_health",
          bridgeRef: "bridge.public.operator",
          status: "ok",
          checkedAt: "2026-06-22T00:00:00.000Z",
          sourceRef: "source.public.test",
          message: "operator attempted bridge write",
          safety: {
            publicProjectionAllowed: true,
            sourceRefs: ["source.public.test"],
            blockerRefs: [],
            caveatRefs: [],
          },
        },
      }),
      "2026-06-22T00:00:00.000Z",
    ))

    expect(agent.receipt.status).toBe("rejected")
    expect(agent.receipt.error?.tag).toBe("auth")
    expect(agent.delta.rows).toBeUndefined()
    expect(operator.receipt.status).toBe("rejected")
    expect(operator.receipt.error?.tag).toBe("auth")
    expect(operator.delta.rows).toBeUndefined()
  })

  test("service actors cannot send browser interaction commands", async () => {
    const result = await Effect.runPromise(applyWorldCommand(
      makeEmptyHotState(regionRef),
      serviceCommand("send_local_message", { text: "service pretending to chat" }),
      "2026-06-22T00:00:00.000Z",
    ))

    expect(result.receipt.status).toBe("rejected")
    expect(result.receipt.error?.tag).toBe("auth")
    expect(result.delta.rows).toBeUndefined()
  })

  test("service actors can write public projection rows only through service commands", async () => {
    const row = {
      kind: "training_run",
      runRef: "run.public.1",
      label: "Public Run",
      state: "pending",
      updatedAt: "2026-06-22T00:00:00.000Z",
      safety: {
        publicProjectionAllowed: true,
        sourceRefs: ["source.public.test"],
        blockerRefs: [],
        caveatRefs: [],
      },
    }
    const result = await Effect.runPromise(applyWorldCommand(
      makeEmptyHotState(regionRef),
      serviceCommand("upsert_training_run", { row }),
      "2026-06-22T00:00:00.000Z",
    ))

    expect(result.receipt.status).toBe("applied")
    expect(result.delta.rows?.[0]?.kind).toBe("training_run")
    expect(result.receipt.changedRefs.map(String)).toEqual(["run.public.1"])
  })

  test("service projection commands reject private or unsafe rows without echoing payloads", async () => {
    const row = {
      kind: "training_run",
      runRef: "run.private.1",
      label: "raw_prompt: do not leak this",
      state: "pending",
      updatedAt: "2026-06-22T00:00:00.000Z",
      safety: {
        publicProjectionAllowed: true,
        sourceRefs: ["source.public.test"],
        blockerRefs: [],
        caveatRefs: [],
      },
    }
    const result = await Effect.runPromise(applyWorldCommand(
      makeEmptyHotState(regionRef),
      serviceCommand("upsert_training_run", { row }),
      "2026-06-22T00:00:00.000Z",
    ))

    expect(result.receipt.status).toBe("rejected")
    expect(result.receipt.error?.tag).toBe("redaction")
    expect(result.receipt.error?.message).not.toContain("raw_prompt")
    expect(result.delta.diagnostic?.message).not.toContain("raw_prompt")
  })

  test("service commands can emit system messages and expire interaction refs", async () => {
    const message = await Effect.runPromise(applyWorldCommand(
      makeEmptyHotState(regionRef),
      serviceCommand("record_system_world_message", { text: "bridge caught up" }),
      "2026-06-22T00:00:00.000Z",
    ))
    expect(message.receipt.status).toBe("applied")
    expect(message.delta.rows?.[0]?.kind).toBe("local_chat_message")
    if (message.delta.rows?.[0]?.kind === "local_chat_message") {
      expect(message.delta.rows[0].channel).toBe("system")
    }

    const withExpiry = {
      ...message.state,
      expiringRefs: {
        "message.hot.1": {
          ref: "message.hot.1",
          kind: "chat" as const,
          expiresAt: "2026-06-22T00:01:00.000Z",
        },
      },
    }
    const expired = await Effect.runPromise(applyWorldCommand(
      withExpiry,
      serviceCommand("expire_interaction_rows", { refs: ["message.hot.1"] }, 2),
      "2026-06-22T00:00:01.000Z",
    ))

    expect(expired.delta.kind).toBe("delete")
    expect(expired.delta.deletedRefs?.map(String)).toEqual(["message.hot.1"])
    expect(expired.state.expiringRefs["message.hot.1"]).toBeUndefined()
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

  test("chat throttles account and session lanes separately from edge/IP policy", async () => {
    let result = await apply(makeEmptyHotState(regionRef), "join_region", { characterId: "pilot" }, 1, "2026-06-22T00:00:00.000Z", {
      sessionRef: "session.one",
    })
    result = await apply(result.state, "send_local_message", {
      text: "first",
    }, 2, "2026-06-22T00:00:01.000Z", {
      sessionRef: "session.one",
    })

    const sameActorDifferentSession = await apply(result.state, "send_local_message", {
      text: "actor throttle",
    }, 3, "2026-06-22T00:00:01.100Z", {
      sessionRef: "session.two",
    })
    const differentActorSameSession = await apply(result.state, "send_local_message", {
      text: "session throttle",
    }, 3, "2026-06-22T00:00:01.100Z", {
      actorRef: "actor.public.bob",
      sessionRef: "session.one",
    })
    const differentActorDifferentSession = await apply(result.state, "send_local_message", {
      text: "separate lane",
    }, 3, "2026-06-22T00:00:01.100Z", {
      actorRef: "actor.public.bob",
      sessionRef: "session.two",
    })

    expect(sameActorDifferentSession.receipt.status).toBe("rejected")
    expect(sameActorDifferentSession.receipt.error?.message).toBe("Chat cadence exceeded.")
    expect(differentActorSameSession.receipt.status).toBe("rejected")
    expect(differentActorSameSession.receipt.error?.message).toBe("Chat session cadence exceeded.")
    expect(differentActorDifferentSession.receipt.status).toBe("applied")
  })

  test("moderation blocks local and pylon chat before rows are emitted", async () => {
    const moderationConfig = {
      hardBlockedTokens: ["badword"],
      softMaskedTokens: [],
    }
    let result = await Effect.runPromise(applyWorldCommand(
      makeEmptyHotState(regionRef),
      command("join_region", { characterId: "pilot" }, 1),
      "2026-06-22T00:00:00.000Z",
      { sessionRef: "session.one", moderationConfig },
    ))

    const local = await Effect.runPromise(applyWorldCommand(
      result.state,
      command("send_local_message", { text: "badword private body" }, 2, "2026-06-22T00:00:01.000Z"),
      "2026-06-22T00:00:01.000Z",
      { sessionRef: "session.one", moderationConfig },
    ))
    result = local
    const pylon = await Effect.runPromise(applyWorldCommand(
      result.state,
      command("send_pylon_message", { text: "b@dw0rd private body" }, 3, "2026-06-22T00:00:03.000Z"),
      "2026-06-22T00:00:03.000Z",
      { sessionRef: "session.one", moderationConfig },
    ))

    expect(local.receipt.status).toBe("rejected")
    expect(local.delta.rows).toBeUndefined()
    expect(local.receipt.error?.tag).toBe("redaction")
    expect(local.receipt.error?.message).not.toContain("private body")
    expect(pylon.receipt.status).toBe("rejected")
    expect(pylon.delta.rows).toBeUndefined()
    expect(pylon.state.moderation.byActor[actorRef]?.mutedUntil).toBe("2026-06-22T00:10:03.000Z")
  })
})
