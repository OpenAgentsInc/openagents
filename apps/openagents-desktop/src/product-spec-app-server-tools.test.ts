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
    expect(tools).toEqual(["get_run", "propose_edit", "propose_evidence_attachment", "propose_plan", "report_blocked", "record_evidence"])
    expect(tools).not.toContain("accept_plan")
    expect(tools).not.toContain("admit_packet")
    expect(tools).not.toContain("verify_evidence")
    expect(tools).not.toContain("owner_disposition")
    expect(tools).not.toContain("confirm_evidence_attachment")
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

  test("agents can only propose evidence attachments for later owner review", () => {
    const requested: unknown[] = []
    const service = {
      proposeEvidenceAttachment: (value: unknown) => {
        requested.push(value)
        return { ok: false, reason: "revision_not_incremented", message: "intent changed" }
      },
    } as unknown as ProductSpecWorkroom
    const expectedCurrent = {
      specRef: "product.spec.aaaaaaaaaaaaaaaaaaaaaaaa",
      relativePath: "specs/fixture.product-spec.md",
      revision: 1,
      digest: `sha256:${"a".repeat(64)}`,
    }
    const result = handleProductSpecDynamicTool(call("propose_evidence_attachment", {
      workContextRef: "work.context.1",
      expectedCurrent,
      proposedMarkdown: "review me",
    }), { workContextRef: "work.context.1", service })
    expect(requested).toEqual([{ workContextRef: "work.context.1", expectedCurrent, proposedMarkdown: "review me" }])
    expect(result).toMatchObject({ success: false })
  })
})
