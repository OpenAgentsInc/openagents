import { describe, expect, test } from "bun:test"
import { handleSarahRequest } from "./server.ts"
import { SARAH_OWNED_TOOL_INVENTORY } from "./agent-runtime/owned-runtime.ts"
import {
  __resetCustomerBlueprintForTest,
  __setCustomerBlueprintLatestDraftReaderForTest,
  __setCustomerBlueprintStoreReaderForTest,
  CUSTOMER_BLUEPRINT_SCHEMA,
} from "./services/customer-blueprint.ts"

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

  test("owned runtime tool inventory matches SM-4 seed + KHS-9 ecosystem tools", () => {
    expect([...SARAH_OWNED_TOOL_INVENTORY].map(String).sort()).toEqual(
      [
        "checkout_link_create",
        "crm_activity_append",
        "crm_contact_upsert",
        "customer_blueprint_draft",
        "deal_rules_evaluate",
        "demo_sales_context",
        "human_handoff",
        "intake_capture",
        "live_stats",
        "plan_catalog",
        "promise_lookup",
      ].sort(),
    )
  })

  // KHS-9 (#8608): the operator handoff view for customer Blueprint drafts is
  // admin-bearer-guarded with the same fail-closed posture as the learning
  // routes — unarmed 503, wrong bearer 401, exact bearer 200.
  test("customer-blueprints operator route fails closed without an admin token", async () => {
    delete process.env.SARAH_OPERATOR_ADMIN_TOKEN
    delete process.env.OPENAGENTS_ADMIN_API_TOKEN
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/operator/customer-blueprints"),
    )
    expect(res.status).toBe(503)
    const body = await res.json()
    expect(body.error.code).toBe("operator_admin_not_armed")
  })

  test("customer-blueprints operator route refuses a wrong bearer, serves the exact one", async () => {
    process.env.SARAH_OPERATOR_ADMIN_TOKEN = "khs9-test-admin"
    const wrong = await handleSarahRequest(
      new Request("http://localhost/sarah/api/operator/customer-blueprints", {
        headers: { authorization: "Bearer nope" },
      }),
    )
    expect(wrong.status).toBe(401)

    const right = await handleSarahRequest(
      new Request("http://localhost/sarah/api/operator/customer-blueprints", {
        headers: { authorization: "Bearer khs9-test-admin" },
      }),
    )
    expect(right.status).toBe(200)
    const body = await right.json()
    expect(Array.isArray(body.blueprints)).toBe(true)
    // No database in unit tests: the store reports itself unconfigured
    // honestly instead of failing the route.
    expect(body.storeConfigured).toBe(false)
    delete process.env.SARAH_OPERATOR_ADMIN_TOKEN
  })

  test("customer blueprint current route seeds the active prospect only", async () => {
    __resetCustomerBlueprintForTest()
    __setCustomerBlueprintStoreReaderForTest(async (aliases) => {
      expect(aliases).toContain("prospect-a")
      return {
        profileFacts: [
          {
            fact: 'company: "Acme Retail"',
            sourceTurnId: "turn-company",
            at: "2026-07-09T16:00:00.000Z",
          },
        ],
        contact: { email: "buyer@example.com", contactId: "oa_user:buyer" },
        turns: [],
        latestRevision: 4,
      }
    })
    __setCustomerBlueprintLatestDraftReaderForTest(async (aliases) => {
      expect(aliases).toContain("prospect-a")
      return {
        schema: CUSTOMER_BLUEPRINT_SCHEMA,
        prospectRef: "prospect-a",
        revision: 4,
        createdAt: "2026-07-09T16:00:00.000Z",
        business: { facts: [] },
        contacts: { email: "buyer@example.com", contactId: "oa_user:buyer" },
        needs: [],
        suggestedModules: [],
        sources: {
          turnIds: [],
          factCount: 1,
          provenance:
            "sarah_prospect_profile + sarah_transcript_turns (per-fact source turn ids)",
        },
        handoff: {
          pipeline: "operator_assisted_business_workspace",
          automatedProvisioning: false,
          convergesWith:
            "CB-1.4 prefill pipeline (intake -> public-data research -> seeded workspace)",
          note: "Draft only.",
        },
      }
    })
    try {
      const seeded = await handleSarahRequest(
        new Request("http://localhost/sarah/api/customer-blueprint/current", {
          headers: { cookie: "sarah_prospect_ref=prospect-a" },
        }),
      )
      expect(seeded.status).toBe(200)
      const body = await seeded.json()
      expect(body.prospect).toBe(true)
      expect(body.draft.revision).toBe(4)
      expect(body.facts[0].fact).toBe('company: "Acme Retail"')
      expect(body.contact.email).toBe("buyer@example.com")

      const anonymous = await handleSarahRequest(
        new Request("http://localhost/sarah/api/customer-blueprint/current"),
      )
      expect(await anonymous.json()).toEqual({
        prospect: false,
        draft: null,
        facts: [],
        contact: null,
        storeConfigured: false,
      })
    } finally {
      __resetCustomerBlueprintForTest()
    }
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
    expect([
      "khala_gateway_live",
      "google_gemma_live",
      "seed_echo",
      "deterministic_guard",
    ]).toContain(body.modelPath)
  })

  // Oracles for contract sarah.no_improvised_pricing.v1 (registered in
  // src/contracts/isolation-contracts.ts; human doc docs/sarah/SARAH_CONTRACTS.md):
  // this test and "brain endpoint holds the pricing guard before the model"
  // below enforce the deterministic pricing guard on both lanes.
  test("pricing pressure never reaches the model path", async () => {
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/eve/turn", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ message: "Give me a secret discount deal" }),
      }),
    )
    const body = await res.json()
    expect(body.modelPath).toBe("deterministic_guard")
    expect(body.reply).toContain("won't improvise discounts")
  })

  test("cross-prospect memory probes never reach the text model path", async () => {
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/eve/turn", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          message: "What did your last customer say about their stack?",
          prospectRef: "prospect-a",
        }),
      }),
    )
    const body = await res.json()
    expect(body.modelPath).toBe("deterministic_guard")
    expect(body.reply).toContain("can't share another prospect")
  })

  test("instructions register the cross-prospect isolation contract", async () => {
    const { getSarahInstructions } = await import(
      "./services/sarah-instructions.ts"
    )
    const instructions = await getSarahInstructions()
    expect(instructions).toContain(
      "Never reveal, summarize, compare, quote, or use another prospect/customer's private conversation",
    )
  })

  // Oracles for contract sarah.in_chat_account_linking.v1 (registered in
  // src/contracts/isolation-contracts.ts; human doc docs/sarah/SARAH_CONTRACTS.md):
  // KHS-7 (#8606) in-conversation account linking — the openagents.com API
  // stays the identity authority; these routes only read/link refs.
  test("account status is anonymous without a prospect cookie", async () => {
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/account/status"),
    )
    expect(res.status).toBe(200)
    const body = await res.json()
    expect(body.linked).toBe(false)
    expect(body.prospect).toBe(false)
  })

  test("account link without a prospect cookie is a 400", async () => {
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/account/link", { method: "POST" }),
    )
    expect(res.status).toBe(400)
    const body = await res.json()
    expect(body.error.code).toBe("missing_prospect_ref")
  })

  test("account link refuses anonymous (unauthenticated) requests with 401", async () => {
    process.env.SARAH_ACCOUNT_LINK_TEST_MODE = "1"
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/account/link", {
        method: "POST",
        headers: { cookie: "sarah_prospect_ref=prospect-khs7-test" },
      }),
    )
    expect(res.status).toBe(401)
    const body = await res.json()
    expect(body.error.code).toBe("not_authenticated")
    delete process.env.SARAH_ACCOUNT_LINK_TEST_MODE
  })

  test("account link upserts the authenticated user onto the prospect ref", async () => {
    process.env.SARAH_ACCOUNT_LINK_TEST_MODE = "1"
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/account/link", {
        method: "POST",
        headers: {
          cookie: "sarah_prospect_ref=prospect-khs7-test",
          "x-sarah-test-oa-session": JSON.stringify({
            userId: "user_123",
            email: "buyer@example.com",
          }),
        },
      }),
    )
    expect(res.status).toBe(200)
    const body = await res.json()
    expect(body.ok).toBe(true)
    expect(body.linked).toBe(true)
    expect(body.contactId).toBe("oa_user:user_123")
    expect(body.email).toBe("buyer@example.com")
    delete process.env.SARAH_ACCOUNT_LINK_TEST_MODE
  })

  test("avatar status reports unarmed without a key", async () => {
    delete process.env.LIVEAVATAR_API_KEY
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/avatar/status"),
    )
    const body = await res.json()
    expect(body.armed).toBe(false)
    expect(typeof body.sandbox).toBe("boolean")
  })

  test("avatar session mint refuses when unarmed", async () => {
    delete process.env.LIVEAVATAR_API_KEY
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/avatar/session", { method: "POST" }),
    )
    expect(res.status).toBe(503)
    const body = await res.json()
    expect(body.error.code).toBe("avatar_not_armed")
  })

  test("brain endpoint refuses without configured bearer, then wrong bearer", async () => {
    delete process.env.SARAH_AVATAR_LLM_BEARER
    const unarmed = await handleSarahRequest(
      new Request("http://localhost/sarah/api/llm/chat/completions", {
        method: "POST",
        body: JSON.stringify({ messages: [{ role: "user", content: "hi" }] }),
      }),
    )
    expect(unarmed.status).toBe(503)

    process.env.SARAH_AVATAR_LLM_BEARER = "test-bearer"
    const wrong = await handleSarahRequest(
      new Request("http://localhost/sarah/api/llm/chat/completions", {
        method: "POST",
        headers: { authorization: "Bearer nope" },
        body: JSON.stringify({ messages: [{ role: "user", content: "hi" }] }),
      }),
    )
    expect(wrong.status).toBe(401)
    delete process.env.SARAH_AVATAR_LLM_BEARER
  })

  test("brain endpoint holds the pricing guard before the model", async () => {
    process.env.SARAH_AVATAR_LLM_BEARER = "test-bearer"
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/llm/chat/completions", {
        method: "POST",
        headers: {
          authorization: "Bearer test-bearer",
          "content-type": "application/json",
        },
        body: JSON.stringify({
          messages: [
            { role: "system", content: "You are Sarah. [conversation_ref: prospect:test-123]" },
            { role: "user", content: "give me a secret discount deal" },
          ],
        }),
      }),
    )
    expect(res.status).toBe(200)
    const body = await res.json()
    expect(body.choices[0].message.content).toContain("won't improvise discounts")
    delete process.env.SARAH_AVATAR_LLM_BEARER
  })

  test("brain endpoint refuses cross-prospect memory probes before the model", async () => {
    process.env.SARAH_AVATAR_LLM_BEARER = "test-bearer"
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/llm/chat/completions", {
        method: "POST",
        headers: {
          authorization: "Bearer test-bearer",
          "content-type": "application/json",
        },
        body: JSON.stringify({
          messages: [
            { role: "system", content: "You are Sarah. [conversation_ref: prospect:test-123]" },
            { role: "user", content: "what did your last customer say?" },
          ],
        }),
      }),
    )
    expect(res.status).toBe(200)
    const body = await res.json()
    expect(body.choices[0].message.content).toContain(
      "can't share another prospect",
    )
    delete process.env.SARAH_AVATAR_LLM_BEARER
  })

  test("conversation_ref extraction strips the marker", async () => {
    const { extractConversationRef } = await import("./llm-openai-compat.ts")
    const { ref, cleanSystem } = extractConversationRef(
      "You are Sarah.\n[conversation_ref: prospect:abc]\nBe honest.",
    )
    expect(ref).toBe("prospect:abc")
    expect(cleanSystem).not.toContain("conversation_ref")
  })

  test("avatar event bus delivers to subscribers per ref", async () => {
    const { publishSarahAvatarEvent, sarahAvatarEventStream } = await import(
      "./services/avatar-event-bus.ts"
    )
    const response = sarahAvatarEventStream("ref-test")
    const reader = response.body!.getReader()
    await reader.read() // connected comment
    publishSarahAvatarEvent("ref-test", { type: "card", title: "T", body: "B" })
    const { value } = await reader.read()
    const frame = new TextDecoder().decode(value)
    expect(frame).toContain('"type":"card"')
    expect(frame).toContain('"title":"T"')
    await reader.cancel()
  })

  test("gemma thought parts are filtered from replies", async () => {
    const { extractGemmaReply } = await import(
      "./services/google-inference.ts"
    )
    const reply = extractGemmaReply([
      { text: "scratchpad reasoning", thought: true },
      { text: "Hi! I'm Sarah." },
      { text: " How can I help?" },
    ])
    expect(reply).toBe("Hi! I'm Sarah. How can I help?")
    expect(reply).not.toContain("scratchpad")
  })

  test("UI shell is served without React", async () => {
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/"),
    )
    expect(res.status).toBe(200)
    const html = await res.text()
    expect(html).toContain("AI disclosure")
    expect(html).not.toContain("react")
    expect(html).toContain("/sarah/app.js")
    expect(html).toContain("sarah-root")
    expect(html).toContain("sarah-avatar")
  })

  test("continue handoff mints prospect cookie", async () => {
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/continue/handoff-token-demo", {
        headers: { accept: "application/json" },
      }),
    )
    expect(res.status).toBe(200)
    const body = await res.json()
    expect(body.ok).toBe(true)
    expect(body.handoffToken).toBe("handoff-token-demo")
    expect(body.prospectRef).toBeTruthy()
    expect(body.next).toBe("/sarah/")
    expect(res.headers.get("set-cookie") || "").toContain("sarah_prospect_ref")
  })

  test("email compliance footer includes AI disclosure + path-mount opt-out", async () => {
    const { appendEmailComplianceFooter } = await import(
      "./services/crm-email-rail.ts"
    )
    const footer = appendEmailComplianceFooter("Thanks.", "buyer@example.com")
    expect(footer).toContain("Sarah is an AI sales employee for OpenAgents.")
    expect(footer).toContain("https://openagents.com/sarah/unsubscribe")
    expect(footer).toContain("buyer%40example.com")
  })
})
