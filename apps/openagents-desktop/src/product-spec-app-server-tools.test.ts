import { describe, expect, test } from "bun:test"

import {
  ProductSpecDynamicTools,
  handleProductSpecDynamicTool,
} from "./product-spec-app-server-tools.ts"
import type { ProductSpecWorkroom } from "./product-spec-workroom.ts"

const call = (tool: string, args: unknown) => ({
  id: "call.1",
  method: "item/tool/call",
  params: { namespace: "product_spec", tool, arguments: args },
})

describe("ProductSpec Codex app-server tools", () => {
  test("exposes proposal/report tools but no authority-bearing approval operation", () => {
    const tools = ProductSpecDynamicTools[0].tools.map(tool => tool.name)
    expect(tools).toEqual(["get_run", "propose_edit", "propose_plan", "report_blocked", "record_evidence"])
    expect(tools).not.toContain("accept_plan")
    expect(tools).not.toContain("admit_packet")
    expect(tools).not.toContain("verify_evidence")
    expect(tools).not.toContain("owner_disposition")
  })

  test("fails with explicit incompatible_workflow when no admitted work context exists", () => {
    const result = handleProductSpecDynamicTool(call("get_run", { runRef: "run.1" }), null)
    expect(result).toEqual({
      contentItems: [{ type: "inputText", text: JSON.stringify({
        ok: false,
        reason: "incompatible_workflow",
        message: "No admitted ProductSpec work context is selected.",
      }) }],
      success: false,
    })
  })

  test("decodes an exact run request before invoking host authority", () => {
    const requested: string[] = []
    const service = {
      run: (runRef: string) => {
        requested.push(runRef)
        return { ok: false, reason: "not_found", message: "missing" }
      },
    } as unknown as ProductSpecWorkroom
    const result = handleProductSpecDynamicTool(call("get_run", { runRef: "run.accepted.1" }), {
      workContextRef: "work.context.1",
      service,
    })
    expect(requested).toEqual(["run.accepted.1"])
    expect(result).toEqual({
      contentItems: [{ type: "inputText", text: JSON.stringify({ ok: false, reason: "not_found", message: "missing" }) }],
      success: false,
    })
    expect(handleProductSpecDynamicTool(call("get_run", { runRef: "../escape" }), {
      workContextRef: "work.context.1",
      service,
    })).toMatchObject({ success: false })
  })
})
