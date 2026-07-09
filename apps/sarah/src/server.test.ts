import { describe, expect, test } from "bun:test"
import { handleSarahRequest } from "./server.ts"
import { SARAH_OWNED_TOOL_INVENTORY } from "./agent-runtime/owned-runtime.ts"

describe("apps/sarah monorepo service", () => {
  test("ops endpoint describes /sarah mount and rails", async () => {
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/operator/ops"),
    )
    expect(res.status).toBe(200)
    const body = await res.json()
    expect(body.mount).toBe("/sarah")
    expect(body.emailRail).toBe("crm_operator_rail")
    expect(body.agentRuntime).toBe("owned_effect_seed")
    expect(body.ui).toBe("effect_native_dom_zero_react")
  })

  test("prospect session mints cookie + thread", async () => {
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/prospect/session", {
        method: "POST",
      }),
    )
    expect(res.status).toBe(200)
    const body = await res.json()
    expect(body.prospectRef).toBeTruthy()
    expect(body.threadId).toStartWith("prospect:")
    expect(res.headers.get("set-cookie") || "").toContain("sarah_prospect_ref")
  })

  test("owned runtime tool inventory matches SM-4 seed", () => {
    expect([...SARAH_OWNED_TOOL_INVENTORY].sort()).toEqual(
      [
        "checkout_link_create",
        "crm_activity_append",
        "crm_contact_upsert",
        "deal_rules_evaluate",
        "demo_sales_context",
        "human_handoff",
        "intake_capture",
      ].sort(),
    )
  })

  test("text turn uses owned runtime", async () => {
    process.env.SARAH_REALTIME_TOKEN_TEST_MODE = "1"
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/eve/turn", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ message: "What is OpenAgents?" }),
      }),
    )
    expect(res.status).toBe(200)
    const body = await res.json()
    expect(body.runtime).toBe("owned_effect_seed")
    expect(body.reply).toBeTruthy()
  })

  test("UI shell is served without React", async () => {
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/"),
    )
    expect(res.status).toBe(200)
    const html = await res.text()
    expect(html).toContain("AI disclosure")
    expect(html).not.toContain("react")
    expect(html).toContain("/sarah/sarah.js")
  })
})
