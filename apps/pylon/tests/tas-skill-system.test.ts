import { describe, expect, test } from "bun:test"

import {
  createSkillRegistry,
  registerSkill,
  resolveSkillInvocation,
} from "../src/tas/skill-system"

describe("tas skill system", () => {
  test("explicit name resolves", () => {
    const skill = {
      name: "review",
      description: "Review changes for defects.",
      enabled: true,
    }
    const registry = registerSkill(createSkillRegistry(), skill)

    expect(resolveSkillInvocation(registry, "/review")).toEqual({
      ok: true,
      skill,
    })
  })

  test("disabled skill returns disabled reason", () => {
    const registry = createSkillRegistry([
      {
        name: "deploy",
        description: "Deploy the current project.",
        enabled: false,
      },
    ])

    expect(resolveSkillInvocation(registry, "/deploy")).toEqual({
      ok: false,
      reason: "disabled",
    })
  })

  test("unknown skill returns unknown reason", () => {
    const registry = createSkillRegistry([
      {
        name: "review",
        description: "Review changes for defects.",
        enabled: true,
      },
    ])

    expect(resolveSkillInvocation(registry, "/revie")).toEqual({
      ok: false,
      reason: "unknown",
    })
  })

  test("duplicate registration is rejected", () => {
    const registry = createSkillRegistry([
      {
        name: "review",
        description: "Review changes for defects.",
        enabled: true,
      },
    ])

    expect(() =>
      registerSkill(registry, {
        name: "review",
        description: "Second descriptor with the same name.",
        enabled: true,
      }),
    ).toThrow("Skill already registered: review")
  })
})
