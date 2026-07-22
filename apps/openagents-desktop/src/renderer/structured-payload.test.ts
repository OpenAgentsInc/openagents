/**
 * Oracle for openagents_desktop.chat.structured_payload_card.v1
 *
 * Detection layer: a conversation message body that IS (or embeds) a JSON
 * payload is classified for card rendering, the Full Auto mission packet is
 * recognized specifically, and ordinary prose is never mistaken for a payload.
 */
import { describe, expect, test } from "vite-plus/test"

import { compileFullAutoMissionPacket, FULL_AUTO_MISSION_SCHEMA, renderFullAutoMissionPrompt } from "../full-auto-mission.ts"
import type { FullAutoRecord } from "../full-auto-registry.ts"
import {
  detectStructuredPayload,
  FULL_AUTO_MISSION_SCHEMA_ID,
} from "./structured-payload.ts"

describe("openagents_desktop.chat.structured_payload_card.v1 — detection", () => {
  test("the renderer discriminator stays in sync with the producer schema literal", () => {
    // The renderer keeps its own copy so the renderer bundle never imports the
    // main-process producer; this asserts they can never silently drift apart.
    expect(FULL_AUTO_MISSION_SCHEMA_ID).toBe(FULL_AUTO_MISSION_SCHEMA)
  })

  test("a whole-body JSON object renders as a generic structured payload", () => {
    const detection = detectStructuredPayload('{"alpha":1,"beta":"two","ok":true,"none":null}')
    expect(detection?.kind).toBe("json")
    if (detection?.kind !== "json") throw new Error("expected json")
    expect(detection.value).toEqual({ alpha: 1, beta: "two", ok: true, none: null })
    expect(JSON.parse(detection.json)).toEqual({ alpha: 1, beta: "two", ok: true, none: null })
  })

  test("a whole-body JSON array is a structured payload too", () => {
    const detection = detectStructuredPayload("[1, 2, 3]")
    expect(detection?.kind).toBe("json")
  })

  test("a generic payload's chip label prefers its own schema/type/kind field", () => {
    const detection = detectStructuredPayload('{"schema":"acme.thing.v2","value":9}')
    expect(detection?.kind).toBe("json")
    if (detection?.kind !== "json") throw new Error("expected json")
    expect(detection.schemaLabel).toBe("acme.thing.v2")
  })

  test("ordinary prose — even with braces — is NOT a payload and stays text", () => {
    expect(detectStructuredPayload("Use the {placeholder} syntax when wiring it up.")).toBeNull()
    expect(detectStructuredPayload("")).toBeNull()
    expect(detectStructuredPayload("just a normal reply")).toBeNull()
    // A code-ish snippet that is not valid JSON must not become a card.
    expect(detectStructuredPayload("const config = { a: 1 }")).toBeNull()
  })

  test("a bare mission packet (whole-body) renders as a mission card", () => {
    const packet = {
      schema: FULL_AUTO_MISSION_SCHEMA_ID,
      runRef: "run-1",
      threadRef: "thread-1",
      objective: "Ship the JSON card renderer",
      doneCondition: "The mission packet renders as a card, not raw JSON",
      objectiveSource: "user",
      workspaceRef: null,
      currentLane: "codex-local",
      accountRef: null,
      continuationOrdinal: 1,
      turnCap: 40,
      remainingTurnsIncludingThisOne: 40,
      priorAcceptedOutcome: null,
      previousHandoff: null,
      responseObligations: ["preserve_owner_objective"],
      completionAuthority:
        "provider completion is self-reported evidence only; the host or owner verifies the done condition",
    }
    const detection = detectStructuredPayload(JSON.stringify(packet, null, 2))
    expect(detection?.kind).toBe("mission")
    if (detection?.kind !== "mission") throw new Error("expected mission")
    expect(detection.mission.objective).toBe("Ship the JSON card renderer")
    expect(detection.mission.doneCondition).toBe(
      "The mission packet renders as a card, not raw JSON",
    )
    expect(detection.mission.currentLane).toBe("codex-local")
    expect(detection.mission.turnCap).toBe(40)
  })

  test("the REAL produced mission prompt (packet embedded in prose) is a mission card", () => {
    const record: FullAutoRecord = {
      threadRef: "thread-real",
      enabled: true,
      continuationCount: 0,
      updatedAt: "2026-07-22T00:00:00.000Z",
      enabledAt: "2026-07-22T00:00:00.000Z",
      workspaceRef: "/workspace",
      profile: { lane: "codex-local", accountRef: "codex-account" },
    }
    const prompt = renderFullAutoMissionPrompt(
      compileFullAutoMissionPacket({
        run: null,
        record,
        threadRef: "thread-real",
        profile: undefined,
        turnCap: 40,
        priorAcceptedOutcome: null,
        previousHandoff: null,
      }),
    )
    // The producer wraps the JSON packet in preamble prose plus verbatim
    // OWNER OBJECTIVE / DONE CONDITION sections whose own text can contain
    // braces — the string-aware scanner must still find the packet.
    expect(prompt).toContain(FULL_AUTO_MISSION_SCHEMA)
    const detection = detectStructuredPayload(prompt)
    expect(detection?.kind).toBe("mission")
    if (detection?.kind !== "mission") throw new Error("expected mission")
    expect(detection.mission.schema).toBe(FULL_AUTO_MISSION_SCHEMA_ID)
    expect(detection.mission.objective.length).toBeGreaterThan(0)
    expect(detection.mission.doneCondition.length).toBeGreaterThan(0)
    // "copy raw" carries valid, canonical JSON of the packet only.
    expect(() => JSON.parse(detection.json)).not.toThrow()
    expect((JSON.parse(detection.json) as { schema: string }).schema).toBe(FULL_AUTO_MISSION_SCHEMA_ID)
  })

  test("a mission packet whose objective embeds braces keeps the full objective", () => {
    const packet = {
      schema: FULL_AUTO_MISSION_SCHEMA_ID,
      objective: 'Fix the parser so `{ "a": 1 }` renders; keep {curly} text intact',
      doneCondition: "All {edge} cases pass",
      currentLane: "codex-local",
    }
    const detection = detectStructuredPayload(
      `Execute this host-authoritative Full Auto mission packet.\n\n${JSON.stringify(packet, null, 2)}\n\nOWNER OBJECTIVE (VERBATIM)\n${packet.objective}`,
    )
    expect(detection?.kind).toBe("mission")
    if (detection?.kind !== "mission") throw new Error("expected mission")
    expect(detection.mission.objective).toBe(packet.objective)
  })
})
