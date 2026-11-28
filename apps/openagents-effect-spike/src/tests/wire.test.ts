// Firm reason: 1 (Effect UI / Effect v4 browser and web-server prototype for docs/effect/half-day-sprint.md)
import { describe, expect, it } from "vitest"
import {
  encodeAnswerLine,
  encodeSteerLine,
  encodeStopLine,
  encodeTaskLine,
  parseWorkerLine
} from "../shared/wire.ts"

describe("mcworker-wire codecs", () => {
  it("encodes owner -> worker lines with exact mcworker-wire field names", () => {
    const taskLine = encodeTaskLine({
      task: "hello",
      session: "demo",
      cwd: "/tmp/work",
      gated: true,
      model: "google/gemini-4-neon",
      effort: "low"
    })
    expect(taskLine.endsWith("\n")).toBe(true)
    expect(JSON.parse(taskLine)).toEqual({
      task: "hello",
      session: "demo",
      cwd: "/tmp/work",
      gated: true,
      model: "google/gemini-4-neon",
      effort: "low"
    })

    expect(JSON.parse(encodeAnswerLine({ answer: true, ticket: 7, why: "ok" }))).toEqual({
      answer: true,
      ticket: 7,
      why: "ok"
    })

    expect(JSON.parse(encodeSteerLine({ id: "s1", steer: "focus", turn: 2 }))).toEqual({
      steer: "focus",
      id: "s1",
      turn: 2
    })

    expect(encodeStopLine()).toBe("{\"stop\":true}\n")
  })

  it("parses worker -> owner frames and skips unknown kinds forward-compatibly", () => {
    expect(parseWorkerLine("{\"kind\":\"ready\"}")).toEqual({ kind: "ready" })
    expect(parseWorkerLine("{\"kind\":\"reply\",\"text\":\"hi\"}")).toEqual({
      kind: "reply",
      text: "hi"
    })
    expect(
      parseWorkerLine("{\"kind\":\"ask\",\"command\":\"ls -la\",\"ticket\":3}")
    ).toEqual({ kind: "ask", command: "ls -la", ticket: 3 })
    expect(
      parseWorkerLine(
        "{\"kind\":\"permit\",\"ticket\":4,\"question\":false,\"title\":\"Run bash\",\"command\":\"git status\",\"options\":[\"Allow\",\"Deny\"]}"
      )
    ).toEqual({
      kind: "permit",
      ticket: 4,
      question: false,
      title: "Run bash",
      command: "git status",
      options: ["Allow", "Deny"]
    })
    expect(parseWorkerLine("{\"kind\":\"future_unknown_kind\",\"x\":1}")).toBeNull()
    expect(parseWorkerLine("not json")).toBeNull()
  })
})
