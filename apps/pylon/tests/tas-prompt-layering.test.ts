import { describe, expect, test } from "bun:test"

import {
  assembleInstructions,
  type InstructionLayer,
} from "../src/tas/prompt-layering"

const layer = (
  scope: InstructionLayer["scope"],
  ref: string,
  text: string,
): InstructionLayer => ({
  scope,
  ref,
  text,
})

describe("tas prompt instruction layering core", () => {
  test("orders instructions by scope precedence", () => {
    expect(
      assembleInstructions([
        layer("task", "task.current", "current task instruction"),
        layer("system", "system.runtime", "runtime policy"),
        layer("session", "session.memory", "session preference"),
        layer("project", "project.agents", "project instruction"),
      ]).ordered,
    ).toEqual([
      { scope: "system", ref: "system.runtime" },
      { scope: "project", ref: "project.agents" },
      { scope: "session", ref: "session.memory" },
      { scope: "task", ref: "task.current" },
    ])
  })

  test("dedupes instructions by ref", () => {
    expect(
      assembleInstructions([
        layer("system", "shared.ref", "lower scope text"),
        layer("task", "shared.ref", "higher scope text"),
        layer("project", "project.ref", "project text"),
      ]).ordered,
    ).toEqual([
      { scope: "project", ref: "project.ref" },
      { scope: "task", ref: "shared.ref" },
    ])
  })

  test("provenance lists scope and ref without raw text", () => {
    const snapshot = assembleInstructions([
      layer("system", "system.runtime", "do not leak this text"),
      layer("task", "task.current", "do not leak this either"),
    ])

    expect(snapshot.provenance).toEqual({
      layers: [
        { scope: "system", ref: "system.runtime" },
        { scope: "task", ref: "task.current" },
      ],
    })
    expect(JSON.stringify(snapshot.provenance)).not.toContain("do not leak")
  })
})
