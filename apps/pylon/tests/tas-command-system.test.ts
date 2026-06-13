import { describe, expect, test } from "bun:test"

import {
  createCommandRegistry,
  parseCommand,
  registerCommand,
} from "../src/tas/command-system"

describe("tas command system", () => {
  test("known command parses typed positional args", () => {
    const registry = registerCommand(createCommandRegistry(), {
      name: "spawn",
      args: [
        { name: "agent", type: "string", required: true },
        { name: "count", type: "number", required: true },
      ],
    })

    expect(parseCommand(registry, "/spawn worker 3")).toEqual({
      ok: true,
      name: "spawn",
      args: {
        agent: "worker",
        count: 3,
      },
    })
  })

  test("missing required arg returns typed error", () => {
    const registry = createCommandRegistry([
      {
        name: "cancel",
        args: [{ name: "taskId", type: "string", required: true }],
      },
    ])

    expect(parseCommand(registry, "/cancel")).toEqual({
      ok: false,
      error: {
        code: "missing_required_arg",
        argName: "taskId",
        message: "taskId is required",
      },
    })
  })

  test("unknown command returns typed error without guessing", () => {
    const registry = createCommandRegistry([
      {
        name: "spawn",
        args: [{ name: "agent", type: "string", required: true }],
      },
    ])

    expect(parseCommand(registry, "/spwan worker")).toEqual({
      ok: false,
      error: {
        code: "unknown_command",
        commandName: "spwan",
        message: "unknown command: /spwan",
      },
    })
  })

  test("non-command input returns typed error", () => {
    const registry = createCommandRegistry()

    expect(parseCommand(registry, "spawn worker")).toEqual({
      ok: false,
      error: {
        code: "not_command",
        message: "input must start with an explicit slash command",
      },
    })
  })
})
