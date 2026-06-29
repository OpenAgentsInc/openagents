import { describe, expect, test } from "bun:test"

import {
  createToolRegistry,
  validateToolCall,
  type ToolContract,
} from "../src/tas/tool-registry"

const contract: ToolContract = {
  name: "repo.search",
  inputSchema: {
    query: { type: "string", required: true },
    limit: { type: "number" },
    includeHidden: { type: "boolean" },
    filters: { type: "object" },
  },
  readOnly: true,
}

describe("tas tool registry core", () => {
  test("valid call ok", () => {
    expect(
      validateToolCall(contract, {
        query: "tool registry",
        limit: 10,
        includeHidden: false,
        filters: { path: "apps/pylon" },
      }),
    ).toEqual({
      ok: true,
      errors: [],
    })
  })

  test("missing required and type mismatch errors", () => {
    expect(
      validateToolCall(contract, {
        limit: "10",
        includeHidden: "false",
      }),
    ).toEqual({
      ok: false,
      errors: [
        "query is required",
        "limit must be number",
        "includeHidden must be boolean",
      ],
    })
  })

  test("duplicate registration rejected", () => {
    const registry = createToolRegistry()

    registry.registerTool(contract)

    expect(() => registry.registerTool(contract)).toThrow(
      "Tool already registered: repo.search",
    )
  })

  test("readOnly flag carried", () => {
    const registry = createToolRegistry([contract])

    expect(registry.getTool("repo.search")?.readOnly).toBe(true)
  })
})
